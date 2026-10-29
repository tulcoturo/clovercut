// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Characterizes the existing edit policy before any timing change.
use super::*;

// JSON's default float parser may change the last bit. Keep exact equality
// for undo; persistence here must preserve every field and each frame, with
// only sub-picosecond tolerance for the three serialized clip times.
fn reopened_matches(info: &projects::ProjectInfo, expected: &model::Project) {
    let session = concat_host::Session::open_info(info).unwrap();
    let actual = session.project();
    let mut comparable = expected.clone();
    let fps = actual.active().video.rate();
    for clip in &actual.active().clips {
        let before = comparable.active_mut().clip_mut(&clip.id).unwrap();
        for (old, new) in [
            (&mut before.start, clip.start),
            (&mut before.duration, clip.duration),
            (&mut before.source_start, clip.source_start),
        ] {
            assert!((*old - new).abs() < 1e-12);
            assert_eq!((*old * fps).round(), (new * fps).round());
            *old = new;
        }
    }
    assert_eq!(actual, &comparable);
}

#[test]
#[ignore = "requires an isolated graphical session and GPU"]
fn cut_and_gesture_rounding_characterization() {
    const CHILD: &str = "CLOVERCUT_PHASE3_CUT_CHILD";
    const TEST: &str = "studio::phase3_timeline_tests::cut_and_gesture_rounding_characterization";
    if std::env::var_os(CHILD).is_none() {
        let root =
            std::env::temp_dir().join(format!("clovercut-phase3-cut-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                TEST,
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
        std::fs::remove_dir_all(&root).unwrap();
        print!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("phase3 cuts: ok"));
        return;
    }
    let gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(gpu).unwrap());
    let _app = crate::ui::App::new().unwrap();
    let mut cases = 0;
    let mut differences = 0;
    let mut half_down = 0;
    let mut exact_menu_off_grid = 0;
    for (num, den) in [
        (24, 1),
        (25, 1),
        (30, 1),
        (60, 1),
        (24000, 1001),
        (30000, 1001),
    ] {
        let fps = num as f64 / den as f64;
        for offset in [0.0, 0.49, 0.5, 0.51] {
            let info = projects::create(
                std::env::temp_dir().to_str().unwrap(),
                &format!("Disposable cut {num}-{den}-{offset}"),
                64,
                36,
                num,
                den,
            )
            .unwrap();
            studio.open_project(info.clone()).unwrap();
            // This fixture observes editing and persistence, not asynchronous monitor publication.
            studio.on_start = true;
            let track = studio.timeline().tracks[0].id.clone();
            let id = studio
                .apply(Command::AddTextClip {
                    track_id: Some(track),
                    above: false,
                    start: 0.0,
                    style: None,
                    duration: Some(8.0),
                    offset_y: None,
                })
                .unwrap();
            let before = studio.project().clone();
            let wanted = (50.0 + offset) / fps;
            let input = wanted as f32;
            studio.selection = vec![id.clone()];
            studio.split_at(input, true);
            assert_eq!(studio.timeline().clips.len(), 2);
            let menu = studio.clip(&id).unwrap().duration;
            assert_eq!(menu, f64::from(input));
            let menu_state = studio.project().clone();
            studio.session.as_mut().unwrap().save(None).unwrap();
            reopened_matches(&info, &menu_state);
            studio.undo();
            assert_eq!(studio.project(), &before);
            studio.split_clip_at(&id, input);
            assert_eq!(studio.timeline().clips.len(), 2);
            let razor = studio.clip(&id).unwrap().duration;
            let razor_frame = (razor * fps).round() as i64;
            let computed = ((f64::from(input) * fps).round() * den as f64) / num as f64;
            assert_eq!(razor, computed);
            if menu != razor {
                differences += 1;
            }
            if offset == 0.5 && razor_frame == 50 {
                half_down += 1;
            }
            if offset == 0.0 && (menu * fps - 50.0).abs() > 1e-12 {
                exact_menu_off_grid += 1;
            }
            println!(
                "cut rate={num}/{den} offset={offset} input={input:.12} menu={menu:.12} razor={razor:.12} razor_frame={razor_frame}"
            );
            let razor_state = studio.project().clone();
            studio.session.as_mut().unwrap().save(None).unwrap();
            reopened_matches(&info, &razor_state);
            studio.undo();
            assert_eq!(studio.project(), &before);
            // Exact endpoints are no-ops for both split entry points.
            for end in [0.0, 8.0] {
                studio.split_at(end, true);
                studio.split_clip_at(&id, end);
                assert_eq!(studio.project(), &before);
            }
            // Frame stepping is explicitly on the project's rational grid.
            studio.playhead = 50.0 * den as f64 / num as f64;
            studio.step_frames(1);
            assert_eq!(studio.playhead, 51.0 * den as f64 / num as f64);
            studio.step_frames(-1);
            assert_eq!(studio.playhead, 50.0 * den as f64 / num as f64);
            // Existing move and trim semantics are continuous time, even with snap disabled.
            studio.lanes.snap = false;
            studio.prefs.magnetic = false;
            studio.prefs.trim_follow = false;
            let subframe = (0.25 / fps) as f32;
            studio.selection = vec![id.clone()];
            studio.clip_pressed(&id, false, -1);
            studio.clip_dragged(subframe, 0.0);
            let echo = studio
                .echo
                .as_ref()
                .unwrap()
                .active()
                .clip(&id)
                .unwrap()
                .clone();
            assert_eq!(echo.start, f64::from(subframe));
            studio.clip_released();
            assert_eq!(studio.clip(&id).unwrap(), &echo);
            assert!((studio.clip(&id).unwrap().start * fps - 0.25).abs() < 1e-5);
            studio.undo();
            assert_eq!(studio.project(), &before);
            studio.clip_pressed(&id, false, 1);
            studio.clip_dragged(subframe, 0.0);
            studio.clip_released();
            assert!((studio.clip(&id).unwrap().duration * fps - (8.0 * fps + 0.25)).abs() < 1e-3);
            studio.undo();
            assert_eq!(studio.project(), &before);
            // Existing snap policy: nearest target strictly inside an 8-pixel threshold.
            studio.lanes.snap = true;
            studio.playhead = 4.0;
            assert_eq!(studio.snapped(4.03125, 0.0625, &id), 4.0);
            assert_eq!(studio.snapped(4.0625, 0.0625, &id), 4.0625);
            assert_eq!(studio.snap_shift(&[4.03125], 0.0625, &[&id]), -0.03125);
            cases += 1;
        }
    }
    assert_eq!(cases, 24);
    assert!(differences > 0);
    assert!(
        half_down > 0,
        "ideal half frame can narrow below the midpoint"
    );
    assert!(exact_menu_off_grid > 0);
    println!(
        "phase3 cuts: ok; cases={cases}; menu/razor differences={differences}; ideal half rounded down={half_down}; exact menu frame off-grid={exact_menu_off_grid}"
    );
    crate::leave(0);
}

fn isolated_fixture(test: &str, run: impl FnOnce(&mut Studio, &projects::ProjectInfo)) {
    const CHILD: &str = "CLOVERCUT_PHASE3_INTERACTION_CHILD";
    if std::env::var(CHILD).ok().as_deref() != Some(test) {
        let root = std::env::temp_dir().join(format!(
            "clovercut-phase3-interaction-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                test,
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, test)
            .env("TMPDIR", &root)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("XDG_STATE_HOME", root.join("state"))
            .output()
            .unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        print!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(gpu).unwrap());
    let _app = crate::ui::App::new().unwrap();
    let info = projects::create(
        std::env::temp_dir().to_str().unwrap(),
        "Disposable interactions",
        64,
        36,
        60,
        1,
    )
    .unwrap();
    studio.open_project(info.clone()).unwrap();
    studio.on_start = true;
    studio.lanes.snap = false;
    studio.prefs.magnetic = false;
    studio.prefs.trim_follow = false;
    run(&mut studio, &info);
    crate::leave(0);
}

fn title(studio: &mut Studio, track: usize, start: f64, duration: f64) -> String {
    studio
        .apply(Command::AddTextClip {
            track_id: Some(studio.timeline().tracks[track].id.clone()),
            above: false,
            start,
            style: None,
            duration: Some(duration),
            offset_y: None,
        })
        .unwrap()
}

#[test]
#[ignore = "requires an isolated graphical session and GPU"]
fn mixed_selection_locks_placement_and_origin_boundary() {
    isolated_fixture(
        "studio::phase3_timeline_tests::mixed_selection_locks_placement_and_origin_boundary",
        |studio, info| {
            // This case needs three independent lanes; a new project starts
            // with one, so create the other two explicitly.
            for _ in 0..2 {
                studio.apply(Command::AddTrack {
                    bottom: true,
                    kind: None,
                    insert_at: None,
                });
            }
            let a = title(studio, 0, 0.5, 1.0);
            let b = title(studio, 1, 2.5, 1.0);
            let c = title(studio, 2, 5.0, 1.0);
            // A real audio item and video item, represented through ordinary commands.
            for (kind, track) in [(model::MediaKind::Audio, 1), (model::MediaKind::Video, 2)] {
                let item: concat_project::commands::NewMedia =
                    serde_json::from_value(serde_json::json!({
                        "path":format!("{}/disposable-{kind:?}.mkv",info.path),"name":"fixture",
                        "kind":kind,"duration":2.0,"hasAudio":kind==model::MediaKind::Audio
                    }))
                    .unwrap();
                let media = studio.apply(Command::AddMedia { item }).unwrap();
                studio
                    .apply(Command::AddClip {
                        media_id: media,
                        track_id: studio.timeline().tracks[track].id.clone(),
                        start: 10.0,
                        ripple: false,
                    })
                    .unwrap();
            }
            studio.select_all();
            assert_eq!(studio.selection.len(), 5);
            let track = studio.clip(&c).unwrap().track_id.clone();
            studio.track_flags(studio.row_of(&track), true, false, true);
            assert!(!studio.selection.contains(&c));
            assert!(
                !studio
                    .selection
                    .iter()
                    .any(|id| studio.clip(id).unwrap().track_id == track)
            );
            let before = studio.project().clone();
            studio.clip_pressed(&c, false, -1);
            assert!(matches!(studio.gesture, Gesture::None));
            studio.split_clip_at(&c, 5.5);
            assert_eq!(studio.project(), &before);
            studio.selection = vec![a.clone(), b.clone()];
            studio.clip_pressed(&a, false, -1);
            studio.clip_dragged(0.25, 0.0);
            studio.clip_released();
            assert_eq!(
                studio.clip(&b).unwrap().start - studio.clip(&a).unwrap().start,
                2.0
            );
            studio.undo();
            assert_eq!(studio.project(), &before);
            // Selected clips must keep their spacing when the first reaches zero.
            studio.selection = vec![a.clone(), b.clone()];
            studio.clip_pressed(&a, false, -1);
            studio.clip_dragged(-3.0, 0.0);
            studio.clip_released();
            assert_eq!(studio.clip(&a).unwrap().start, 0.0);
            assert_eq!(
                studio.clip(&b).unwrap().start,
                2.0,
                "origin clamp must shift the group rigidly"
            );
        },
    );
}

#[test]
#[ignore = "requires an isolated graphical session and GPU"]
fn drag_history_restores_exact_state_without_roundtrip_drift() {
    isolated_fixture(
        "studio::phase3_timeline_tests::drag_history_restores_exact_state_without_roundtrip_drift",
        |studio, info| {
            let start = 262144.0 + 1.0 / 60.0;
            let id = title(studio, 0, start, 1.0);
            let before = studio.project().clone();
            studio.selection = vec![id.clone()];
            studio.clip_pressed(&id, false, -1);
            studio.clip_dragged(0.25, 0.0);
            studio.clip_cancelled();
            assert_eq!(studio.project(), &before);
            assert!(studio.echo.is_none());
            studio.undo();
            assert!(
                studio.timeline().clips.is_empty(),
                "cancel added no history entry"
            );
            studio.redo();
            assert_eq!(studio.project(), &before);
            studio.selection = vec![id.clone()];
            studio.clip_pressed(&id, false, 1);
            studio.clip_dragged(0.125, 0.0);
            studio.clip_cancelled();
            assert_eq!(studio.project(), &before);
            studio.clip_pressed(&id, false, -1);
            studio.clip_released();
            assert_eq!(studio.project(), &before, "click is not an edit");
            studio.clip_pressed(&id, false, -1);
            for delta in [0.0625, 0.125, 0.25] {
                studio.clip_dragged(delta, 0.0);
            }
            studio.clip_released();
            let moved = studio.project().clone();
            studio.undo();
            assert_eq!(
                studio.project(),
                &before,
                "whole gesture is exactly one undo"
            );
            studio.redo();
            assert_eq!(
                studio.project(),
                &moved,
                "redo restores the exact committed clip"
            );
            studio.undo();
            for _ in 0..100 {
                for delta in [0.125, -0.125] {
                    studio.selection = vec![id.clone()];
                    studio.clip_pressed(&id, false, -1);
                    studio.clip_dragged(delta, 0.0);
                    studio.clip_released();
                }
            }
            assert_eq!(
                studio.clip(&id).unwrap().start,
                start,
                "repeated opposite drags must not narrow the origin"
            );
            studio.session.as_mut().unwrap().save(None).unwrap();
            reopened_matches(info, studio.project());
        },
    );
}

#[test]
#[ignore = "requires an isolated graphical session and GPU"]
fn published_geometry_keeps_a_long_clip_tail_in_the_correct_frame() {
    isolated_fixture(
        "studio::phase3_timeline_tests::published_geometry_keeps_a_long_clip_tail_in_the_correct_frame",
        |studio, _| {
            let duration = 262144.0 + 1.0 / 60.0;
            let id = title(studio, 0, 0.0, duration);
            studio.lanes.width = 1000.0;
            studio.lanes.seconds_per_pixel = 0.0005;
            studio.lanes.scroll_left = 262144.0;
            let app = crate::ui::App::new().unwrap();
            let models = Models::new();
            studio.publish_lanes(&app, &models);
            let shown = models.clips.clone();
            let row = (0..shown.row_count())
                .filter_map(|i| shown.row_data(i))
                .find(|c| c.id.as_str() == id)
                .unwrap();
            assert!((f64::from(row.start + row.duration) - 1.0 / 60.0).abs() < 1e-6);
            assert!(
                row.duration < 2.0,
                "geometry is bounded to visible overscan"
            );
            assert_eq!(
                studio.clip(&id).unwrap().duration,
                duration,
                "view slicing must not edit the project"
            );
        },
    );
}

#[test]
#[ignore = "requires an isolated graphical session, FFmpeg and GPU"]
fn transport_clock_matches_decoded_and_published_frame() {
    const TEST: &str =
        "studio::phase3_timeline_tests::transport_clock_matches_decoded_and_published_frame";
    const CHILD: &str = "CLOVERCUT_PHASE36_RATE";
    let Some(rate) = std::env::var_os(CHILD) else {
        let mut failures = Vec::new();
        for (num, den) in [
            (24, 1),
            (25, 1),
            (30, 1),
            (60, 1),
            (24000, 1001),
            (30000, 1001),
        ] {
            let root = std::env::temp_dir()
                .join(format!("clovercut-phase36-{}-{num}", std::process::id()));
            std::fs::create_dir(&root).unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    TEST,
                    "--exact",
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD, format!("{num}/{den}"))
                .env("TMPDIR", &root)
                .env("XDG_CONFIG_HOME", root.join("config"))
                .env("XDG_DATA_HOME", root.join("data"))
                .env("XDG_CACHE_HOME", root.join("cache"))
                .env("XDG_STATE_HOME", root.join("state"))
                .output()
                .unwrap();
            std::fs::remove_dir_all(root).unwrap();
            print!("{}", String::from_utf8_lossy(&output.stdout));
            if !output.status.success() {
                failures.push(format!(
                    "{num}/{den}: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
                continue;
            }
            assert!(String::from_utf8_lossy(&output.stdout).contains("phase36 playback: ok"));
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        return;
    };
    use std::cell::RefCell;
    use std::io::Write;
    use std::rc::Rc;
    use std::time::{Duration, Instant};
    let rate = rate.to_str().unwrap();
    let (num, den) = rate.split_once('/').unwrap();
    let (num, den): (i64, i64) = (num.parse().unwrap(), den.parse().unwrap());
    let fps = num as f64 / den as f64;
    let dir = std::env::temp_dir();
    let video = dir.join("numbered.nut");
    // Lossless binary RGB codes identify each decoded source frame modulo eight.
    // NUT retains the exact rational frame timestamps, including NTSC rates.
    let mut encoder = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgb24",
            "-video_size",
            "64x36",
            "-framerate",
            rate,
            "-i",
            "-",
            "-an",
            "-c:v",
            "ffv1",
            "-pix_fmt",
            "bgr0",
        ])
        .arg(&video)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = encoder.stdin.take().unwrap();
        for frame in 0..96 {
            let colour = [
                if frame & 1 != 0 { 255 } else { 0 },
                if frame & 2 != 0 { 255 } else { 0 },
                if frame & 4 != 0 { 255 } else { 0 },
            ];
            input.write_all(&colour.repeat(64 * 36)).unwrap();
        }
    }
    assert!(encoder.wait().unwrap().success());
    let _gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    // Use the production pixel publication path so the final Slint image is readable.
    let mut studio = Studio::new(crate::host::Host::start(None).unwrap());
    let app = crate::ui::App::new().unwrap();
    let info = projects::create(
        dir.to_str().unwrap(),
        "Disposable playback",
        64,
        36,
        num,
        den,
    )
    .unwrap();
    studio.open_project(info).unwrap();
    studio.on_start = true;
    let item = concat_host::media::probe(video.to_str().unwrap())
        .unwrap()
        .to_new_media();
    let media_id = studio.apply(Command::AddMedia { item }).unwrap();
    let track_id = studio.timeline().tracks[0].id.clone();
    studio
        .apply(Command::AddClip {
            media_id,
            track_id,
            start: 0.0,
            ripple: false,
        })
        .unwrap();
    studio.prefs.proxies = false;
    let models = Models::new();
    studio.publish(&app, &models);
    let shell = Rc::new(crate::host::Shell {
        app: app.as_weak(),
        studio: RefCell::new(studio),
        models,
    });
    crate::host::Shell::install(shell.clone());
    let editor = app.global::<Editor>();
    editor.set_seats(slint::ModelRc::from(shell.models.seats.clone()));
    editor.set_tracks(slint::ModelRc::from(shell.models.tracks.clone()));
    editor.set_clips(slint::ModelRc::from(shell.models.clips.clone()));
    editor.set_stage_items(slint::ModelRc::from(shell.models.stage.clone()));
    // Publish the audit's exact frame 50 while paused before starting the
    // real clock in its late half; subsequent frames come from transport ticks.
    {
        let mut studio = shell.studio.borrow_mut();
        studio.on_start = false;
        studio.seek(50.0 * den as f64 / num as f64);
        studio.publish(&app, &shell.models);
    }
    let began = Instant::now();
    let timer = slint::Timer::default();
    let mut seen = 0;
    let mut checked = 0;
    let mut fractional = 0;
    let mut mismatches = 0;
    let mut paused_frame_seen = false;
    let observed_app = app.clone_strong();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(2),
        move || {
            let mut studio = shell.studio.borrow_mut();
            for (spec, clock, pixel) in studio.monitor.observed_frames.iter().skip(seen) {
                let actual = i64::from(pixel[0] > 127)
                    + 2 * i64::from(pixel[1] > 127)
                    + 4 * i64::from(pixel[2] > 127);
                if !spec.moving {
                    // Opening queues an initial frame at zero before the
                    // explicit seek. Wait for its coalesced frame-50 result.
                    if *clock != 50.0 * den as f64 / num as f64 {
                        continue;
                    }
                    assert_eq!(*clock, 50.0 * den as f64 / num as f64);
                    assert_eq!(actual, 50 % 8, "paused exact frame 50 must be decoded");
                    paused_frame_seen = true;
                    continue;
                }
                let expected = (clock * fps + 1e-6).floor() as i64;
                assert_eq!((spec.time * fps).round() as i64, expected);
                println!(
                    "frame {num}/{den} clock={:.12} expected={expected} decoded_mod8={actual}",
                    clock
                );
                if actual != expected % 8 {
                    mismatches += 1;
                }
                if (clock * fps).fract() > 0.5 {
                    fractional += 1;
                }
                checked += 1;
            }
            seen = studio.monitor.observed_frames.len();
            if paused_frame_seen && !studio.playing {
                let published = observed_app
                    .global::<Editor>()
                    .get_preview_frame()
                    .to_rgba8()
                    .unwrap();
                let current = studio.monitor.image.to_rgba8().unwrap();
                assert_eq!(published.as_bytes(), current.as_bytes());
                studio.playhead = 50.75 * den as f64 / num as f64;
                studio.play_toggle();
                studio.request_preview();
            }
            if checked >= 16 {
                let published = observed_app
                    .global::<Editor>()
                    .get_preview_frame()
                    .to_rgba8()
                    .unwrap();
                let current = studio.monitor.image.to_rgba8().unwrap();
                assert_eq!(
                    published.as_bytes(),
                    current.as_bytes(),
                    "worker result must reach the real Editor image"
                );
                assert!(studio.host.playback.position_now() > 50.75 / fps);
                assert!(
                    fractional > 0,
                    "exercise the late half of a real playback interval"
                );
                studio.pause();
                assert_eq!(
                    mismatches, 0,
                    "running-clock interval and published decoded picture disagree"
                );
                println!(
                    "phase36 playback: ok rate={num}/{den} frames={checked} late-half={fractional}"
                );
                crate::leave(0);
            }
            assert!(
                began.elapsed() < Duration::from_secs(15),
                "timed out waiting for actual frame publication: {checked}"
            );
        },
    );
    app.run().unwrap();
}
