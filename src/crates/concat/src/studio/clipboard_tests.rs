// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors
//! Exercise the real asynchronous window clipboard with isolated projects.

use super::*;

#[test]
#[ignore = "requires an isolated graphical/audio session"]
fn cross_project_paste_and_group_duplicate_preserve_state_and_undo() {
    const CHILD: &str = "CLOVERCUT_CLIPBOARD_UI_CHILD";
    const TEST: &str =
        "studio::clipboard_tests::cross_project_paste_and_group_duplicate_preserve_state_and_undo";
    if std::env::var_os(CHILD).is_none() {
        let root =
            std::env::temp_dir().join(format!("clovercut-clipboard-ui-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                TEST,
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, &root)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("XDG_STATE_HOME", root.join("state"))
            .output()
            .unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert!(
            output.status.success(),
            "child status: {}\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("clipboard-ui: ok"));
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os(CHILD).unwrap());
    let gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(gpu).unwrap());
    let app = App::new().unwrap();
    let a = projects::create(root.to_str().unwrap(), "Copy A", 64, 36, 10, 1).unwrap();
    let b = projects::create(root.to_str().unwrap(), "Paste B", 64, 36, 10, 1).unwrap();
    let source = std::path::Path::new(&a.path).join("source.mkv");
    assert!(
        std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=64x36:r=10",
                "-t",
                "3",
                "-an",
                "-c:v",
                "ffv1"
            ])
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    let other = root.join("other.mkv");
    std::fs::copy(&source, &other).unwrap();
    studio.open_project(a).unwrap();
    let media = studio
        .apply(Command::AddMedia {
            item: concat_host::media::probe(source.to_str().unwrap())
                .unwrap()
                .to_new_media(),
        })
        .unwrap();
    let lane = studio.timeline().tracks[studio.timeline().main_track_index()]
        .id
        .clone();
    let video = studio
        .apply(Command::AddClip {
            media_id: media.clone(),
            track_id: lane,
            start: 0.0,
            ripple: false,
        })
        .unwrap();
    studio.apply(Command::UpdateClip {
        clip_id: video.clone(),
        patch: ClipPatch {
            masks: Some(vec![model::Mask::of("m1", "", model::MaskShape::Circle)]),
            opacity: Some(0.4),
            ..Default::default()
        },
    });
    let captions = studio
        .apply(Command::AddTrack {
            bottom: false,
            kind: Some(model::TrackKind::Captions),
            insert_at: None,
        })
        .unwrap();
    let mut selected = vec![video];
    for (at, text) in [(0.0, "One"), (1.0, "Two")] {
        selected.push(
            studio
                .apply(Command::AddTextClip {
                    track_id: Some(captions.clone()),
                    above: true,
                    start: at,
                    duration: Some(0.5),
                    offset_y: None,
                    style: Some(TextStyle {
                        content: text.into(),
                        caption_group: Some("source-group".into()),
                        ..Default::default()
                    }),
                })
                .unwrap(),
        );
    }
    studio.selection = selected;
    studio.copy_clips(None);
    studio.open_project(b.clone()).unwrap();
    let unrelated = studio
        .apply(Command::AddMedia {
            item: concat_host::media::probe(other.to_str().unwrap())
                .unwrap()
                .to_new_media(),
        })
        .unwrap();
    assert_eq!(
        media, unrelated,
        "fixture must reproduce equal media IDs in different projects"
    );
    let before = studio.project().clone();
    studio.paste_clips(None);
    assert!(studio.paste_cancel.is_some());
    let shell = Rc::new(crate::host::Shell {
        app: app.as_weak(),
        studio: RefCell::new(studio),
        models: Models::new(),
    });
    crate::host::Shell::install(shell.clone());
    let timer = slint::Timer::default();
    let finished = Rc::new(std::cell::Cell::new(false));
    let completed = finished.clone();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut before_anchor_removal = None;
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(10),
        move || {
            assert!(
                std::time::Instant::now() < until,
                "asynchronous paste did not finish"
            );
            let mut studio = shell.studio.borrow_mut();
            if studio.paste_cancel.is_some() {
                return;
            }
            if let Some(copied) = before_anchor_removal.take() {
                assert_eq!(
                    studio.timeline().clips.len(),
                    2,
                    "a removed paste anchor must reject the pending delivery"
                );
                studio.undo();
                assert_eq!(studio.project(), &copied);
                std::fs::remove_file(&source).unwrap();
                studio.close_project().unwrap();
                let reopened = concat_project::Editor::from_document(
                    &projects::read_document(&b.path).unwrap().unwrap(),
                )
                .unwrap();
                assert_eq!(reopened.project().active().clips.len(), 3);
                assert!(
                    reopened
                        .project()
                        .media
                        .iter()
                        .all(|m| std::path::Path::new(&m.path).is_file())
                );
                completed.set(true);
                slint::quit_event_loop().unwrap();
                return;
            }
            assert_eq!(studio.timeline().clips.len(), 3);
            assert_eq!(studio.selection.len(), 3);
            let video = studio
                .timeline()
                .clips
                .iter()
                .find(|c| c.kind == model::ClipKind::Video)
                .unwrap();
            assert_eq!(video.masks.len(), 1);
            assert_eq!(video.opacity, 0.4);
            let imported = studio.project().media_by_id(&video.media_id).unwrap();
            assert_ne!(imported.id, unrelated);
            assert!(std::path::Path::new(&imported.path).starts_with(&b.path));
            assert_eq!(
                std::fs::read(&imported.path).unwrap(),
                std::fs::read(&source).unwrap()
            );
            let copied = studio.project().clone();
            studio.undo();
            assert_eq!(studio.project(), &before);
            studio.redo();
            assert_eq!(studio.project(), &copied);
            let captions: Vec<_> = studio
                .timeline()
                .clips
                .iter()
                .filter(|c| c.kind == model::ClipKind::Text)
                .map(|c| c.id.clone())
                .collect();
            let group = studio
                .clip(&captions[0])
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .caption_group
                .clone();
            assert_ne!(group.as_deref(), Some("source-group"));
            assert_eq!(
                group,
                studio
                    .clip(&captions[1])
                    .unwrap()
                    .text
                    .as_ref()
                    .unwrap()
                    .caption_group
            );
            studio.selection = captions;
            studio.duplicate_selected();
            assert_eq!(studio.selection.len(), 2);
            let first = studio
                .clip(&studio.selection[0])
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .caption_group
                .clone();
            let second = studio
                .clip(&studio.selection[1])
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .caption_group
                .clone();
            assert_eq!(first, second);
            assert_ne!(first, group);
            studio.undo();
            assert_eq!(studio.project(), &copied);
            let anchor = studio
                .timeline()
                .clips
                .iter()
                .find(|c| c.kind == model::ClipKind::Video)
                .unwrap()
                .id
                .clone();
            before_anchor_removal = Some(copied);
            studio.paste_clips(Some(&anchor));
            assert!(studio.paste_cancel.is_some());
            studio.apply(Command::RemoveClips {
                clip_ids: vec![anchor.clone()],
                ripple: false,
            });
            assert!(studio.clip(&anchor).is_none());
        },
    );
    app.run().unwrap();
    timer.stop();
    assert!(finished.get());
    println!("clipboard-ui: ok");
    // Like the other isolated native journeys, finish before process-global
    // graphics/audio destructors run. All project assertions and close ran above.
    crate::leave(0);
}
