// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! State-driven controller journey with actual asynchronous import, relink and
//! export. This does not simulate mouse/keyboard input or qualify accessibility.
use super::{Models, Studio};
use crate::host::{Host, Shell};
use crate::panes::{Msg, export::ExportMsg, media_bin::MediaMsg, relink::RelinkMsg};
use crate::ui::{App, ExportPhase};
use concat_host::projects;
use concat_project::{Command, model::TextStyle};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires an isolated graphical/audio session, FFmpeg and GPU"]
fn import_edit_reopen_relink_export() {
    const CHILD: &str = "CLOVERCUT_EDITOR_JOURNEY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let root =
            std::env::temp_dir().join(format!("clovercut-editor-journey-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "studio::editor_workflow_tests::import_edit_reopen_relink_export",
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .env("TMPDIR", &root)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("XDG_STATE_HOME", root.join("state"))
            .output()
            .unwrap();
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        let success = output.status.success()
            && String::from_utf8_lossy(&output.stdout).contains("editor journey: ok");
        if success {
            inspect_export(&root.join("journey.mp4"));
            std::fs::remove_dir_all(&root).unwrap();
        }
        assert!(
            success,
            "editor journey failed; fixtures retained at {}",
            root.display()
        );
        return;
    }
    let root = std::env::temp_dir();
    let source = root.join("source.mkv");
    assert!(
        std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=160x90:rate=10:duration=2",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=2",
                "-c:v",
                "ffv1",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    let gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let dirs = concat_host::AppDirs::locate().unwrap();
    let mut prefs = crate::prefs::Preferences::load(&dirs);
    prefs.caption_language = Some("pt-BR".into());
    prefs.layout = Some(crate::prefs::SavedLayout {
        preset: crate::dock::LayoutPreset::Audio,
        wide: Some(crate::workspace_layout::SavedDock::Pane("preview".into())),
        compact: Some(crate::workspace_layout::SavedDock::Pane("media".into())),
        ..Default::default()
    });
    prefs.save(&dirs);
    let mut studio = Studio::new(Host::start(gpu).unwrap());
    assert_eq!(
        studio.current_layout_preset,
        crate::dock::LayoutPreset::Audio
    );
    assert_eq!(
        studio.dock.to_saved(),
        crate::workspace_layout::SavedDock::Pane("preview".into())
    );
    studio.reset_current_layout();
    assert_eq!(
        studio.dock.to_saved(),
        crate::dock::LayoutPreset::Audio.build_dock().to_saved()
    );
    assert_eq!(
        studio.captions.language.as_deref(),
        Some("pt"),
        "the inline panel restores and normalizes the saved language before any sheet opens"
    );
    let indonesian = concat_speech::languages::LANGUAGES
        .iter()
        .position(|(code, _)| *code == "id")
        .unwrap() as i32
        + 1;
    studio.handle(crate::panes::Msg::Captions(
        crate::panes::captions::CaptionsMsg::LanguageChanged(indonesian),
    ));
    assert_eq!(
        crate::prefs::Preferences::load(&dirs)
            .caption_language
            .as_deref(),
        Some("id")
    );
    assert!(
        !studio.captions.open,
        "inline selection does not open the sheet"
    );
    let app = App::new().unwrap();
    studio.set_compact(true);
    studio.save_layout(app.window());
    let saved = crate::prefs::Preferences::load(&dirs).layout.unwrap();
    assert_eq!(saved.preset, crate::dock::LayoutPreset::Audio);
    assert_eq!(
        saved.wide,
        Some(crate::dock::LayoutPreset::Audio.build_dock().to_saved())
    );
    assert_eq!(
        saved.compact,
        Some(crate::workspace_layout::SavedDock::Pane("media".into()))
    );
    studio.handle(Msg::Settings(
        crate::panes::settings::SettingsMsg::RememberLayoutChanged(false),
    ));
    assert!(crate::prefs::Preferences::load(&dirs).layout.is_none());
    studio.set_compact(false);
    studio.set_layout_preset(crate::dock::LayoutPreset::Default);

    let info = projects::create(root.to_str().unwrap(), "Journey", 160, 90, 10, 1).unwrap();
    studio.open_project(info.clone()).unwrap();
    let shell = Rc::new(Shell {
        app: app.as_weak(),
        studio: RefCell::new(studio),
        models: Models::new(),
    });
    Shell::install(shell.clone());
    shell
        .studio
        .borrow_mut()
        .handle(Msg::Media(MediaMsg::Import(vec![source.clone()])));
    let timer = slint::Timer::default();
    let run_app = app.clone_strong();
    let moved_folder = root.join("moved");
    std::fs::create_dir(&moved_folder).unwrap();
    let moved_source = moved_folder.join("source.mkv");
    let mut stage = 0;
    let mut cancelled_export = 0;
    let mut entered = Instant::now();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(20),
        move || {
            assert!(
                entered.elapsed() < Duration::from_secs(120),
                "editor journey timed out in stage {stage}"
            );
            let mut studio = shell.studio.borrow_mut();
            match stage {
                0 => {
                    if studio.project().media.is_empty() {
                        assert!(
                            !studio.toast.failed,
                            "import failed: {}",
                            studio.toast.message
                        );
                        return;
                    }
                    assert_eq!(studio.project().media.len(), 1);
                    let id = studio.project().media[0].id.clone();
                    assert_eq!(studio.project().media[0].path, source.to_string_lossy());
                    let track_id = studio.timeline().tracks[0].id.clone();
                    studio
                        .apply(Command::AddClip {
                            media_id: id,
                            track_id,
                            start: 0.0,
                            ripple: false,
                        })
                        .unwrap();
                    drop(studio);
                    check_media_selection(&shell);
                    studio = shell.studio.borrow_mut();
                    check_speed_selection(&mut studio);
                    check_shapes_and_preview_hold(&mut studio);
                    check_transition_layers(&mut studio);
                    check_batch_inspector(&mut studio);
                    studio
                        .apply(Command::AddCaptionSource {
                            name: "journey.srt".into(),
                            srt: "1\n00:00:00,000 --> 00:00:01,500\nJourney caption\n".into(),
                            style: TextStyle::default(),
                            offset_y: 0.35,
                            place_at: Some(0.0),
                        })
                        .unwrap();
                    let before = studio.timeline().clips.len();
                    let id = studio.timeline().clips[0].id.clone();
                    studio.selection = vec![id];
                    studio.split_at(1.0, true);
                    assert_eq!(studio.timeline().clips.len(), before + 1);
                    studio.undo();
                    assert_eq!(studio.timeline().clips.len(), before);
                    studio.redo();
                    assert_eq!(studio.timeline().clips.len(), before + 1);
                    studio.rename_project("Journey renamed");
                    let saved = studio.project().clone();
                    studio.close_project().unwrap();
                    studio
                        .open_project(projects::open(&info.path).unwrap())
                        .unwrap();
                    assert_eq!(
                        studio.project(),
                        &saved,
                        "save/reopen preserves edits and captions"
                    );
                    assert_eq!(studio.project_name, "Journey renamed");
                    studio.close_project().unwrap();
                    std::fs::rename(&source, &moved_source).unwrap();
                    studio
                        .open_project(projects::open(&info.path).unwrap())
                        .unwrap();
                    assert_eq!(studio.project().missing_media().len(), 1);
                    assert!(studio.relink.open, "real missing source opens recovery");
                    let missing_id = studio.project().missing_media()[0].id.clone();
                    let before_remove = studio.project().clone();
                    // A queued search owns the old missing-list snapshot.
                    studio.handle(Msg::Relink(RelinkMsg::RelinkIn(moved_folder.clone())));
                    studio.handle(Msg::Relink(RelinkMsg::Remove(missing_id.clone())));
                    assert!(!studio.project().media.iter().any(|m| m.id == missing_id));
                    assert!(studio.project().timelines.iter().all(|t|
                        t.clips.iter().all(|c| c.media_id != missing_id)));
                    assert!(!studio.relink.open);
                    studio.undo();
                    assert_eq!(studio.project(), &before_remove, "one undo restores source and clips");
                    studio.redo();
                    assert!(!studio.project().media.iter().any(|m| m.id == missing_id));
                    studio.undo();
                    assert_eq!(studio.project(), &before_remove);
                    let missing = studio.project().missing_media();
                    studio.handle(Msg::Relink(RelinkMsg::Show(missing)));
                    studio.handle(Msg::Relink(RelinkMsg::Remove("obsolete-row".into())));
                    assert_eq!(studio.project(), &before_remove, "obsolete rows cannot edit the project");
                    studio.handle(Msg::Relink(RelinkMsg::RelinkIn(moved_folder.clone())));
                    stage = 1;
                    entered = Instant::now();
                }
                1 => {
                    if !studio.project().missing_media().is_empty() {
                        return;
                    }
                    assert_eq!(
                        studio.project().media[0].path,
                        moved_source.to_string_lossy()
                    );
                    assert!(!studio.relink.open);
                    studio.undo();
                    assert_eq!(studio.project().media[0].path, source.to_string_lossy());
                    assert_eq!(studio.project().missing_media().len(), 1);
                    studio.redo();
                    assert!(studio.project().missing_media().is_empty());
                    studio.close_project().unwrap();
                    studio
                        .open_project(projects::open(&info.path).unwrap())
                        .unwrap();
                    assert!(
                        studio.project().missing_media().is_empty(),
                        "relink survives reopen"
                    );
                    studio.handle(Msg::Export(ExportMsg::Open));
                    studio.handle(Msg::Export(ExportMsg::FolderChosen(
                        root.to_string_lossy().into_owned(),
                    )));
                    let other_folder = root.join("other-export-folder");
                    std::fs::create_dir_all(&other_folder).unwrap();
                    let original = root.join("existing-a.mp4");
                    std::fs::write(&original, b"original export").unwrap();
                    for (folder, name) in [(&root, "existing-b"), (&other_folder, "existing-a")] {
                        let replacement = folder.join(format!("{name}.mp4"));
                        std::fs::write(&replacement, b"another export").unwrap();
                        studio.handle(Msg::Export(ExportMsg::FolderChosen(root.to_string_lossy().into_owned())));
                        studio.handle(Msg::Export(ExportMsg::NameEdited("existing-a".into())));
                        studio.handle(Msg::Export(ExportMsg::Start));
                        assert!(matches!(&studio.confirm, Some(super::Confirm::ReplaceExport { path }) if Path::new(path) == original));
                        studio.handle(Msg::Export(ExportMsg::FolderChosen(folder.to_string_lossy().into_owned())));
                        studio.handle(Msg::Export(ExportMsg::NameEdited(name.into())));
                        studio.confirm_accepted();
                        assert!(matches!(&studio.confirm, Some(super::Confirm::ReplaceExport { path }) if Path::new(path) == replacement), "changed destination requires its own confirmation");
                        assert!(!studio.host.exporter.is_busy(), "stale consent must not start a render");
                        assert_eq!(std::fs::read(&original).unwrap(), b"original export");
                        assert_eq!(std::fs::read(&replacement).unwrap(), b"another export");
                        studio.confirm = None;
                    }
                    studio.handle(Msg::Export(ExportMsg::FolderChosen(root.to_string_lossy().into_owned())));
                    studio.handle(Msg::Export(ExportMsg::NameEdited(
                        "cancelled-journey".into(),
                    )));
                    studio.handle(Msg::Export(ExportMsg::Start));
                    assert_eq!(studio.export.phase, ExportPhase::Running);
                    cancelled_export = studio.export.request_id().unwrap();
                    studio.handle(Msg::Export(ExportMsg::Progress {
                        request_id: cancelled_export,
                        fraction: 0.25,
                        stage: "regression checkpoint".into(),
                    }));
                    studio.handle(Msg::Export(ExportMsg::Close));
                    studio.handle(Msg::Export(ExportMsg::Open));
                    studio.handle(Msg::Export(ExportMsg::Again));
                    studio.handle(Msg::Export(ExportMsg::Start));
                    studio.handle(Msg::Export(ExportMsg::Overwrite("obsolete-target.mp4".into())));
                    assert!(studio.export.open);
                    assert_eq!(studio.export.phase, ExportPhase::Running);
                    assert_eq!(studio.export.progress, 0.25);
                    assert_eq!(studio.export.stage, "regression checkpoint");
                    assert_eq!(studio.export.request_id(), Some(cancelled_export));
                    studio.handle(Msg::Export(ExportMsg::Cancel));
                    studio.handle(Msg::Export(ExportMsg::Finished(
                        cancelled_export,
                        Ok("obsolete.mp4".into()),
                    )));
                    assert_eq!(studio.export.phase, ExportPhase::Idle);
                    assert!(studio.export.written.is_empty());
                    stage = 2;
                    entered = Instant::now();
                }
                2 => {
                    // Cancellation releases the real worker's slot asynchronously.
                    if studio.host.exporter.is_busy() {
                        return;
                    }
                    studio.handle(Msg::Export(ExportMsg::NameEdited("journey".into())));
                    let final_output = root.join("journey.mp4");
                    std::fs::write(&final_output, b"previous completed export").unwrap();
                    studio.handle(Msg::Export(ExportMsg::Start));
                    assert!(matches!(&studio.confirm, Some(super::Confirm::ReplaceExport { path }) if Path::new(path) == final_output));
                    assert!(!studio.host.exporter.is_busy());
                    studio.confirm_accepted();
                    assert_eq!(studio.export.phase, ExportPhase::Running);
                    let replacement = studio.export.request_id().unwrap();
                    assert_ne!(replacement, cancelled_export);
                    for obsolete in [
                        ExportMsg::Progress {
                            request_id: cancelled_export,
                            fraction: 0.9,
                            stage: "obsolete".into(),
                        },
                        ExportMsg::Finished(cancelled_export, Err("obsolete failure".into())),
                        ExportMsg::Finished(cancelled_export, Ok("obsolete.mp4".into())),
                        ExportMsg::Published(cancelled_export, Ok("obsolete gallery path".into())),
                        ExportMsg::Published(
                            cancelled_export,
                            Err("obsolete publication failure".into()),
                        ),
                    ] {
                        studio.handle(Msg::Export(obsolete));
                        assert_eq!(studio.export.phase, ExportPhase::Running);
                        assert_eq!(studio.export.request_id(), Some(replacement));
                        assert_eq!(studio.export.progress, 0.0);
                        assert!(studio.export.message.is_empty());
                        assert!(studio.export.written.is_empty());
                    }
                    stage = 3;
                    entered = Instant::now();
                }
                3 => {
                    assert_ne!(
                        studio.export.phase,
                        ExportPhase::Failed,
                        "export failed: {}",
                        studio.export.message
                    );
                    if studio.export.phase != ExportPhase::Done {
                        return;
                    }
                    assert!(Path::new(&studio.export.written).is_file());
                    studio.close_project().unwrap();
                    println!("editor journey: ok");
                    crate::leave(0);
                }
                _ => unreachable!(),
            }
            studio.publish(&app, &shell.models);
        },
    );
    run_app.run().unwrap();
    timer.stop();
    panic!("journey event loop ended before completion");
}

fn inspect_export(path: &Path) {
    let output = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_streams", "-of", "json"])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let streams = data["streams"].as_array().unwrap();
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"] == "video")
        .unwrap();
    assert_eq!(video["codec_name"], "h264");
    assert_eq!(video["width"], 160);
    assert_eq!(video["height"], 90);
    assert_eq!(video["r_frame_rate"], "10/1");
    assert_eq!(video["nb_frames"], "20");
    assert!(streams.iter().any(|stream| stream["codec_type"] == "audio"));
    // Independently decode the last frame: a successful mux alone is insufficient.
    let frame = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args([
            "-vf",
            "select=eq(n\\,19)",
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .output()
        .unwrap();
    assert!(frame.status.success());
    assert_eq!(frame.stdout.len(), 160 * 90 * 3);
    assert!(
        frame
            .stdout
            .chunks_exact(3)
            .any(|pixel| pixel[0].abs_diff(pixel[1]) > 40)
    );
}

// Exercise the actual keyboard dispatch with live controller state. The UI
// fixture separately verifies delivery/focus; these assertions cover the shared
// keymap, project guards and undoable removal without touching timeline clips.
fn check_media_selection(shell: &Rc<Shell>) {
    let app = shell.app.upgrade().expect("the journey window is alive");
    let (extra, timeline_selection, before, bindings) = {
        let mut state = shell.studio.borrow_mut();
        let first_path = &state.project().media[0].path;
        let second_path = Path::new(first_path).with_file_name("selection-source.mkv");
        std::fs::copy(first_path, &second_path).unwrap();
        let mut item = concat_host::media::probe(second_path.to_str().unwrap())
            .unwrap()
            .to_new_media();
        item.name = "Second source".into();
        let extra = state.apply(Command::AddMedia { item }).unwrap();
        state.selection = vec![state.timeline().clips[0].id.clone()];
        state.media.selected.clear();
        state.publish(&app, &shell.models);
        (
            extra,
            state.selection.clone(),
            state.project().clone(),
            state.prefs.keybinds.clone(),
        )
    };
    assert!(crate::media_key_chord(
        "a".into(),
        false,
        false,
        true,
        false
    ));
    {
        let mut state = shell.studio.borrow_mut();
        assert_eq!(state.media.selected.len(), 2);
        assert_eq!(
            state.project(),
            &before,
            "select all does not edit the project"
        );
        assert_eq!(
            state.selection, timeline_selection,
            "media selection leaves timeline selection alone"
        );
        let _ = crate::keymap::bind(&mut state.prefs, "select-all", "ctrl+j");
        state.media.selected.clear();
    }
    assert!(!crate::media_key_chord(
        "a".into(),
        false,
        false,
        true,
        false
    ));
    assert!(crate::media_key_chord(
        "j".into(),
        false,
        false,
        true,
        false
    ));
    assert_eq!(shell.studio.borrow().media.selected.len(), 2);
    shell.studio.borrow_mut().settings.open = true;
    assert!(!crate::media_key_chord(
        "j".into(),
        false,
        false,
        true,
        false
    ));
    shell.studio.borrow_mut().settings.open = false;
    shell.studio.borrow_mut().settings.capturing = Some("select-all".into());
    assert!(!crate::media_key_chord(
        "j".into(),
        false,
        false,
        true,
        false
    ));
    {
        let mut state = shell.studio.borrow_mut();
        state.settings.capturing = None;
        state.media.selected = std::collections::HashSet::from([extra.clone()]);
    }
    assert!(crate::media_key_chord(
        slint::platform::Key::Delete.into(),
        false,
        false,
        false,
        false
    ));
    {
        let mut state = shell.studio.borrow_mut();
        assert!(state.project().media_by_id(&extra).is_none());
        assert_eq!(
            state.timeline().clips.len(),
            1,
            "Delete in the bin preserves the other file's clip"
        );
        assert_eq!(state.selection, timeline_selection);
        state.undo();
        assert!(
            state.project().media_by_id(&extra).is_some(),
            "media deletion is undoable"
        );
        state.redo();
        assert!(state.project().media_by_id(&extra).is_none());
        state.prefs.keybinds = bindings;
        state.media.selected.clear();
    }
}

// A graph selection belongs to its clip, including when Delete arrives after
// a different clip became current. The actual echo/commit/undo path is used.
fn check_speed_selection(state: &mut Studio) {
    use concat_project::model::SpeedPoint;
    let baseline = state.project().clone();
    let selection = state.selection.clone();
    let first = state.timeline().clips[0].id.clone();
    let source = state.timeline().clips[0].media_id.clone();
    let track = state.timeline().clips[0].track_id.clone();
    let second = state
        .apply(Command::AddClip {
            media_id: source,
            track_id: track,
            start: 3.0,
            ripple: false,
        })
        .unwrap();
    let curve: Vec<_> = [(0.0, 1.0), (0.5, 2.0), (1.0, 1.0)]
        .into_iter()
        .map(|(at, speed)| SpeedPoint {
            at,
            speed,
            slope: None,
        })
        .collect();
    for id in [&first, &second] {
        state.apply(Command::SetClipSpeedCurve {
            clip_id: id.clone(),
            curve: Some(curve.clone()),
            ripple: false,
        });
    }
    let curved = state.project().clone();
    state.selection = vec![first.clone()];
    state.speed_choose(1, false);
    state.selection = vec![second.clone()];
    state.speed_remove_chosen();
    state.clip_commit();
    state.flush_commit();
    assert_eq!(
        state.project(),
        &curved,
        "a former clip's chosen points cannot edit the current clip"
    );
    state.speed_choose(1, false);
    state.speed_remove_chosen();
    state.clip_commit();
    state.flush_commit();
    assert_eq!(
        state
            .clip(&first)
            .unwrap()
            .speed_curve
            .as_ref()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        state
            .clip(&second)
            .unwrap()
            .speed_curve
            .as_ref()
            .unwrap()
            .len(),
        2
    );
    state.undo();
    assert_eq!(
        state.project(),
        &curved,
        "one undo restores the chosen-point edit"
    );
    // Restore the journey's exact project after the three setup commands.
    for _ in 0..3 {
        state.undo();
    }
    assert_eq!(state.project(), &baseline);
    state.selection = selection;
    state.speed_choose(-1, false);
}

// Real inspector transactions, plus temporary preview hold and seek bounds.
fn check_shapes_and_preview_hold(state: &mut Studio) {
    let baseline = state.project().clone();
    let selection = state.selection.clone();
    let playhead = state.playhead;
    let axis = state.prefs.preview_axis;
    let bounded = state.prefs.playhead_stops_at_end;
    let id = state
        .apply(Command::AddShapeClip {
            track_id: None,
            above: true,
            start: 0.0,
            style: Some(concat_project::model::ShapeStyle::default()),
            duration: Some(1.0),
            name: "Editable figure".into(),
        })
        .unwrap();
    state.selection = vec![id.clone()];
    let original = state.clip(&id).unwrap().shape.clone();
    state.clip_shape_colour(0, slint::Color::from_argb_u8(128, 18, 52, 86));
    state.clip_commit();
    state.flush_commit();
    let changed = state.clip(&id).unwrap().shape.clone().unwrap();
    assert_eq!(changed.fill, "#12345680");
    state.undo();
    assert_eq!(
        state.clip(&id).unwrap().shape,
        original,
        "one undo restores the shape's colour"
    );
    state.redo();
    assert_eq!(state.clip(&id).unwrap().shape.as_ref(), Some(&changed));
    state.undo();
    state.undo();
    assert_eq!(state.project(), &baseline);
    state.selection = selection;
    state.prefs.preview_axis = true;
    state.hover(0.25);
    assert_eq!(state.hover, Some(0.25));
    state.preview_held = true;
    state.hover(0.75);
    state.end_hover();
    assert_eq!(
        state.hover,
        Some(0.25),
        "Shift holds the currently shown hover frame"
    );
    state.preview_held = false;
    state.hover(0.75);
    assert_eq!(state.hover, Some(0.75));
    state.end_hover();
    state.prefs.playhead_stops_at_end = true;
    state.seek(1_000_000.0);
    assert!(state.playhead <= state.duration());
    state.seek(-1.0);
    assert_eq!(state.playhead, 0.0);
    state.prefs.playhead_stops_at_end = bounded;
    state.prefs.preview_axis = axis;
    state.seek(playhead);
}

fn check_transition_layers(state: &mut Studio) {
    use concat_project::commands::{ClipMove, TrimEdge};
    use concat_project::model::{ClipKind, TrackKind};
    let baseline = state.project().clone();
    let selection = state.selection.clone();
    let playhead = state.playhead;
    state.apply_transition("concat.push");
    let id = state.sole_selection().expect("the transition is selected");
    let clip = state.clip(&id).unwrap().clone();
    assert_eq!(clip.kind, ClipKind::Transition);
    assert_eq!(clip.video_effects[0].id, "concat.push");
    assert_eq!(
        state.timeline().track(&clip.track_id).unwrap().kind,
        TrackKind::Filter
    );
    state.chain_add(false, "concat.zoom-transition");
    assert_eq!(state.clip(&id).unwrap().video_effects.len(), 1);
    assert_eq!(
        state.clip(&id).unwrap().video_effects[0].id,
        "concat.zoom-transition"
    );
    let before_copy = state.project().clone();
    state.duplicate_selected();
    let copied = state.sole_selection().expect("the copy is selected");
    assert_ne!(copied, id);
    assert_eq!(state.clip(&copied).unwrap().kind, ClipKind::Transition);
    assert_eq!(
        state.clip(&copied).unwrap().video_effects,
        state.clip(&id).unwrap().video_effects
    );
    state.undo();
    assert_eq!(
        state.project(),
        &before_copy,
        "one undo removes a transition duplicate"
    );
    state.selection = vec![id.clone()];
    state.apply(Command::MoveClips {
        moves: vec![ClipMove {
            clip_id: id.clone(),
            start: 3.0,
            track_id: clip.track_id.clone(),
        }],
    });
    state.apply(Command::TrimClip {
        clip_id: id.clone(),
        edge: TrimEdge::Start,
        delta: -0.25,
        ripple: false,
    });
    assert_eq!(state.clip(&id).unwrap().start, 2.75);
    assert_eq!(state.clip(&id).unwrap().duration, 1.25);
    for _ in 0..4 {
        state.undo();
    }
    assert_eq!(state.project(), &baseline);
    state.selection = selection;
    state.seek(playhead);
}

// Actual inspector commands affect compatible peers and undo as whole gestures.
fn check_batch_inspector(state: &mut Studio) {
    let baseline = state.project().clone();
    let selection = state.selection.clone();
    let ids: Vec<String> = (0..2)
        .map(|number| {
            state
                .apply(Command::AddShapeClip {
                    track_id: None,
                    above: true,
                    start: 0.0,
                    style: None,
                    duration: Some(1.0),
                    name: format!("Batch shape {number}"),
                })
                .unwrap()
        })
        .collect();
    state.selection = ids.clone();
    assert_eq!(state.edit_targets(), ids);
    let before_width = state.project().clone();
    state.clip_set(crate::ui::ClipField::ShapeWidth, 0.4);
    state.clip_commit();
    assert!(
        ids.iter()
            .all(|id| (state.clip(id).unwrap().shape.as_ref().unwrap().width - 0.4).abs() < 1e-6)
    );
    state.undo();
    assert_eq!(
        state.project(),
        &before_width,
        "one undo restores the complete shape gesture"
    );
    state.selection = ids.clone();
    state.apply_catalogue("concat.adjust", true);
    assert!(
        ids.iter()
            .all(|id| state.clip(id).unwrap().video_effects.len() == 1)
    );
    let before_knob = state.project().clone();
    state.chain_set_param(false, 0, "contrast", 0.75);
    state.clip_commit();
    assert!(ids.iter().all(|id| {
        state.clip(id).unwrap().video_effects[0]
            .params
            .get("contrast")
            == Some(&0.75)
    }));
    state.undo();
    assert_eq!(
        state.project(),
        &before_knob,
        "one undo restores every peer's effect knob"
    );
    state.selection = ids.clone();
    state.chain_toggle(false, 0);
    assert!(
        ids.iter()
            .all(|id| !state.clip(id).unwrap().video_effects[0].enabled)
    );
    state.undo();
    state.selection = ids.clone();
    state.chain_remove(false, 0);
    assert!(
        ids.iter()
            .all(|id| state.clip(id).unwrap().video_effects.is_empty())
    );
    state.undo();
    state.undo(); // both catalogue additions
    state.undo(); // second shape
    state.undo(); // first shape
    assert_eq!(state.project(), &baseline);
    state.selection = selection;
}
