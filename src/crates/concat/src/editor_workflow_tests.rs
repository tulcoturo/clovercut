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
    let mut studio = Studio::new(Host::start(gpu).unwrap());
    let app = App::new().unwrap();
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
                    studio.handle(Msg::Export(ExportMsg::NameEdited("journey".into())));
                    studio.handle(Msg::Export(ExportMsg::Start));
                    assert_eq!(studio.export.phase, ExportPhase::Running);
                    stage = 2;
                    entered = Instant::now();
                }
                2 => {
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
