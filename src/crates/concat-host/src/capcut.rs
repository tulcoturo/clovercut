// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah

//! Importing a CapCut draft as a new Clovercut project (#477).
//!
//! The file half of `concat_project::capcut`: where CapCut keeps its
//! drafts, what makes a folder one, reading a draft's timelines, and
//! writing the project they make into a folder of its own. The draft is
//! only ever read. A CapCut folder may be synced to CapCut's cloud and is
//! CapCut's to manage, so nothing of Concat's is written into it.

use std::path::{Component, Path, PathBuf};

use concat_project::capcut::{DRAFT_FILES, Draft, DraftTimeline};
use serde_json::Value;

use crate::projects::{self, ProjectInfo};

/// CapCut's drafts on this machine, where it keeps them: the folder holding
/// one folder a draft. `None` where there is none.
pub fn drafts_folder() -> Option<PathBuf> {
    let under = |base: Option<PathBuf>, tail: &str| base.map(|base| base.join(tail));
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let candidates = [
        // The download from capcut.com, then the App Store's, which is
        // sandboxed into a container of its own.
        under(
            home.clone(),
            "Movies/CapCut/User Data/Projects/com.lveditor.draft",
        ),
        under(
            home,
            "Library/Containers/com.lemon.lvoverseas/Data/Movies/CapCut/User Data/Projects/com.lveditor.draft",
        ),
        under(
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
            "CapCut/User Data/Projects/com.lveditor.draft",
        ),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|folder| folder.is_dir())
}

/// Whether `folder` is a CapCut draft.
pub fn is_draft(folder: &Path) -> bool {
    DRAFT_FILES.iter().any(|file| folder.join(file).is_file())
}

/// A JSON file of the draft's, or `None` when it is missing.
fn json(path: &Path) -> Option<Result<Value, String>> {
    let bytes = std::fs::read(path).ok()?;
    Some(serde_json::from_slice(&bytes).map_err(|_| {
        // Not JSON: CapCut encrypts the drafts of some of its builds and
        // every draft made from one of its templates.
        "this CapCut draft is encrypted or is not valid JSON; try an unencrypted draft from CapCut"
            .to_owned()
    }))
}

/// The edit in `folder`: its first draft file there.
fn edit_in(folder: &Path) -> Option<Result<Value, String>> {
    DRAFT_FILES.iter().find_map(|file| json(&folder.join(file)))
}

/// Reads the draft in `folder`: its name and every timeline it has, the
/// one CapCut opens first first.
pub fn read(folder: &Path) -> Result<Draft, String> {
    let main =
        edit_in(folder).ok_or_else(|| format!("{} is not a CapCut draft", folder.display()))??;
    let meta = json(&folder.join("draft_meta_info.json")).and_then(Result::ok);
    let name = meta
        .as_ref()
        .and_then(|meta| meta.get("draft_name"))
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            folder
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "CapCut draft".to_owned());

    // A draft of several timelines lists them in `Timelines/project.json`,
    // each in a folder of its own; the draft's own file is the main one's.
    let mut timelines = Vec::new();
    let listed = json(&folder.join("Timelines").join("project.json")).and_then(Result::ok);
    if let Some(listed) = listed {
        let main_id = listed
            .get("main_timeline_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let mut entries: Vec<&Value> = listed
            .get("timelines")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|entry| entry.get("is_marked_delete").and_then(Value::as_bool) != Some(true))
            .collect();
        entries.sort_by_key(|entry| entry.get("id").and_then(Value::as_str) != Some(main_id));
        for entry in entries {
            let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
            if id.is_empty()
                || id.contains(['/', '\\'])
                || id == "."
                || id == ".."
                || !matches!(
                    Path::new(id).components().next(),
                    Some(Component::Normal(_))
                )
            {
                return Err("the CapCut draft contains an invalid timeline folder".into());
            }
            let edit = if id == main_id {
                Some(Ok(main.clone()))
            } else {
                edit_in(&folder.join("Timelines").join(id))
            };
            if let Some(Ok(edit)) = edit {
                timelines.push(DraftTimeline {
                    name: entry
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    edit,
                });
            }
        }
    }
    if timelines.is_empty() {
        timelines.push(DraftTimeline {
            name: String::new(),
            edit: main,
        });
    }
    Ok(Draft {
        name,
        folder: folder.to_string_lossy().into_owned(),
        timelines,
    })
}

/// What an import made: the new project, and what of the draft did not
/// come across.
#[derive(Debug)]
pub struct Import {
    /// The project, ready to open.
    pub project: ProjectInfo,
    /// One line for each kind of thing left out or changed.
    pub notes: Vec<String>,
}

// Resolve existing ancestors as well as a not-yet-created destination. Symlink
// parents are followed before checking that the source remains read-only.
fn destination(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    for ancestor in absolute.ancestors() {
        if let Ok(mut resolved) = ancestor.canonicalize() {
            for component in absolute
                .strip_prefix(ancestor)
                .map_err(|error| error.to_string())?
                .components()
            {
                match component {
                    Component::Normal(part) => resolved.push(part),
                    Component::ParentDir => {
                        resolved.pop();
                    }
                    Component::CurDir => {}
                    _ => return Err("the project destination is not a directory path".into()),
                }
            }
            return Ok(resolved);
        }
    }
    Err("the project destination could not be resolved".into())
}

/// Imports the CapCut draft in `folder` as a new project under `location`,
/// in a folder named after the draft - "Reel", then "Reel 2" if that is
/// taken - and writes its whole document there.
pub fn import(folder: &Path, location: &str) -> Result<Import, String> {
    let source = folder.canonicalize().map_err(|error| error.to_string())?;
    if destination(Path::new(location))?.starts_with(&source) {
        return Err("choose a project location outside the original CapCut draft".into());
    }
    let draft = read(folder)?;
    let mut imported = concat_project::capcut::import(&draft).map_err(|error| error.to_string())?;
    // CapCut often omits audio metadata for video. Probe existing files so
    // sound is not silently dropped and real stream choices remain available.
    for item in &mut imported.project.media {
        if !Path::new(&item.path).is_file() {
            continue;
        }
        if let Ok(summary) = crate::media::probe(&item.path) {
            let actual = summary.to_new_media();
            item.duration = actual.duration;
            item.width = actual.width;
            item.height = actual.height;
            item.frame_rate = actual.frame_rate;
            item.frame_rate_fraction = actual.frame_rate_fraction;
            item.video_codec = actual.video_codec;
            item.audio_codec = actual.audio_codec;
            item.has_audio = actual.has_audio;
            item.audio_tracks = actual.audio_tracks;
            item.color_space = actual.color_space;
        }
    }
    let settings = &imported.settings;
    let base = settings.name.trim();
    let base = if base.is_empty() {
        "CapCut draft"
    } else {
        base
    };
    let name = (1..)
        .map(|number| {
            if number == 1 {
                base.to_owned()
            } else {
                format!("{base} {number}")
            }
        })
        .find(|name| {
            !Path::new(location)
                .join(projects::folder_name(name))
                .exists()
        })
        .expect("a free name");
    let project = projects::create_with_canvas(
        location,
        &name,
        settings.width,
        settings.height,
        settings.rate_num,
        settings.rate_den,
        settings.canvas,
    )?;
    let mut settings = imported.settings.clone();
    settings.name = name;
    let document = concat_project::to_document(&settings, &imported.project);
    projects::save(&project.path, &document)?;
    log::info!(
        "capcut: imported {} as {} ({} timeline(s), {} media)",
        folder.display(),
        project.path,
        imported.project.timelines.len(),
        imported.project.media.len()
    );
    for note in &imported.notes {
        log::info!("capcut: not carried: {note}");
    }
    Ok(Import {
        project,
        notes: imported.notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft_folder(root: &Path) -> PathBuf {
        let folder = root.join("Original draft");
        std::fs::create_dir(&folder).unwrap();
        let edit = serde_json::json!({
            "fps":30.0, "canvas_config":{"width":640,"height":360},
            "materials":{"texts":[{"id":"text", "content":"Imported words", "font_size":15}]},
            "tracks":[{"type":"text", "segments":[{"material_id":"text", "target_timerange":{"start":0,"duration":1000000}}]}]
        });
        std::fs::write(
            folder.join("draft_info.json"),
            serde_json::to_vec(&edit).unwrap(),
        )
        .unwrap();
        folder
    }

    #[test]
    fn importing_makes_a_separate_project_and_leaves_the_draft_untouched() {
        let root = tempfile::tempdir().unwrap();
        let source = draft_folder(root.path());
        let bytes = std::fs::read(source.join("draft_info.json")).unwrap();
        let location = root.path().join("Projects");
        let first = import(&source, location.to_str().unwrap()).unwrap();
        let second = import(&source, location.to_str().unwrap()).unwrap();
        assert_ne!(first.project.path, second.project.path);
        assert!(!Path::new(&first.project.path).starts_with(&source));
        let document = projects::read_document(&first.project.path)
            .unwrap()
            .unwrap();
        let project = concat_project::from_document(&document).unwrap();
        assert_eq!(
            project.active().clips[0].text.as_ref().unwrap().content,
            "Imported words"
        );
        assert_eq!(
            project.active().main_track().unwrap().kind,
            concat_project::model::TrackKind::Video
        );
        assert_eq!(
            project
                .active()
                .track(&project.active().clips[0].track_id)
                .unwrap()
                .kind,
            concat_project::model::TrackKind::Text
        );
        assert_eq!(
            std::fs::read(source.join("draft_info.json")).unwrap(),
            bytes
        );
        assert_eq!(std::fs::read_dir(&source).unwrap().count(), 1);
    }

    #[test]
    fn a_destination_inside_the_draft_is_refused_before_any_write() {
        let root = tempfile::tempdir().unwrap();
        let source = draft_folder(root.path());
        let location = source.join("new").join("Projects");
        assert!(
            import(&source, location.to_str().unwrap())
                .unwrap_err()
                .contains("outside")
        );
        assert!(!source.join("new").exists());
        #[cfg(unix)]
        {
            let alias = root.path().join("alias");
            std::os::unix::fs::symlink(&source, &alias).unwrap();
            assert!(import(&source, alias.join("new").to_str().unwrap()).is_err());
            assert!(!source.join("new").exists());
        }
    }

    #[test]
    fn a_timeline_id_cannot_escape_the_selected_draft() {
        let root = tempfile::tempdir().unwrap();
        let source = draft_folder(root.path());
        let timelines = source.join("Timelines");
        std::fs::create_dir(&timelines).unwrap();
        for id in ["../outside", "..\\outside", "/outside", ".", "..", ""] {
            let meta = serde_json::json!({"main_timeline_id":"main", "timelines":[{"id":"main"},{"id":id}]});
            std::fs::write(
                timelines.join("project.json"),
                serde_json::to_vec(&meta).unwrap(),
            )
            .unwrap();
            assert!(
                read(&source).unwrap_err().contains("invalid timeline"),
                "{id}"
            );
        }
    }

    #[test]
    fn existing_video_is_probed_so_omitted_audio_metadata_does_not_mute_it() {
        let root = tempfile::tempdir().unwrap();
        let source = draft_folder(root.path());
        let media = root.path().join("picture-and-sound.mkv");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=32x32:r=10:d=0.2",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=0.2",
                "-c:v",
                "ffv1",
                "-c:a",
                "pcm_s16le",
                "-shortest",
            ])
            .arg(&media)
            .status()
            .expect("FFmpeg fixture generator");
        assert!(status.success());
        let edit = serde_json::json!({
            "fps":10.0, "canvas_config":{"width":32,"height":32},
            "materials":{"videos":[{"id":"v", "type":"video", "path":media,
                "duration":200000,"width":32,"height":32}]},
            "tracks":[{"type":"video", "segments":[{"material_id":"v",
                "target_timerange":{"start":0,"duration":200000},
                "source_timerange":{"start":0,"duration":200000}}]}]
        });
        std::fs::write(
            source.join("draft_info.json"),
            serde_json::to_vec(&edit).unwrap(),
        )
        .unwrap();
        let imported = import(&source, root.path().join("Projects").to_str().unwrap()).unwrap();
        let document = projects::read_document(&imported.project.path)
            .unwrap()
            .unwrap();
        let project = concat_project::from_document(&document).unwrap();
        assert_eq!(project.media.len(), 1);
        assert!(project.media[0].has_audio);
        assert_eq!(project.media[0].audio_tracks.len(), 1);
        assert_eq!(project.media[0].width, Some(32));
        assert!(
            project.media[0]
                .duration
                .is_some_and(|duration| duration > 0.0)
        );
    }

    #[test]
    fn encrypted_or_malformed_json_creates_no_project() {
        let root = tempfile::tempdir().unwrap();
        let source = draft_folder(root.path());
        std::fs::write(source.join("draft_info.json"), [0u8, 255, 42, 13]).unwrap();
        let location = root.path().join("Projects");
        assert!(
            import(&source, location.to_str().unwrap())
                .unwrap_err()
                .contains("encrypted")
        );
        assert!(!location.exists());
    }

    /// Every draft CapCut has on this machine imports, into a scratch
    /// folder, and opens. Ignored: it needs CapCut's drafts, and reads
    /// them where CapCut keeps them. `cargo test -p concat-host --lib
    /// capcut -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs CapCut's drafts on this machine"]
    fn the_drafts_on_this_machine_import() {
        let Some(drafts) = drafts_folder() else {
            eprintln!("no CapCut drafts here");
            return;
        };
        let location = std::env::temp_dir().join(format!("concat-capcut-{}", std::process::id()));
        std::fs::create_dir_all(&location).expect("a scratch folder");
        for entry in std::fs::read_dir(&drafts).expect("the drafts").flatten() {
            let folder = entry.path();
            if !is_draft(&folder) {
                continue;
            }
            match import(&folder, &location.to_string_lossy()) {
                Ok(import) => {
                    let opened = projects::open(&import.project.path).expect("opens");
                    let document = projects::read_document(&opened.path)
                        .expect("reads")
                        .expect("a document");
                    let project =
                        concat_project::from_document(&document).expect("the document loads");
                    eprintln!(
                        "{}: {} timeline(s), {} clip(s) on the first, {} media; notes: {:?}",
                        folder.display(),
                        project.timelines.len(),
                        project.timelines[0].clips.len(),
                        project.media.len(),
                        import.notes
                    );
                }
                Err(error) => eprintln!("{}: {error}", folder.display()),
            }
        }
        let _ = std::fs::remove_dir_all(&location);
    }

    #[test]
    fn a_folder_without_a_draft_is_said_to_be_one() {
        let error = read(Path::new("/nonexistent/folder")).expect_err("refused");
        assert!(error.contains("not a CapCut draft"), "{error}");
    }
}
