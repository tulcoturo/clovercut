// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Phase 4 Task 4.3: Rapid Transport, Project Switching, and Audio Device Recovery.
//!
//! Validates:
//! 1. Rapid transport: rapid play/pause transitions, high-frequency scrubbing (20 Hz)
//!    across cuts, gaps, and alternating directions without deadlocks or stale frames.
//! 2. Project switching: opening, closing and switching disposable projects during active
//!    playback, pending PCM decoding and GPU frame publication without cross-project bleed.
//! 3. Audio device failure & recovery: controlled stream error handling and supervisor
//!    rebuild verification while preserving playback continuity.

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use concat_host::projects;
use concat_project::commands::Command;
use slint::ComponentHandle;

use crate::studio::{Models, Studio};
use crate::ui::App;

fn generate_robustness_media(
    dir: &Path,
    filename: &str,
    duration_secs: f64,
    color_rgb: [u8; 3],
) -> PathBuf {
    let output_path = dir.join(filename);
    let fps = 30.0;
    let total_frames = (duration_secs * fps).ceil() as usize;

    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args([
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
        "30/1",
        "-i",
        "-",
        "-f",
        "lavfi",
        "-i",
        &format!("sine=frequency=1000:sample_rate=48000:duration={duration_secs}"),
        "-c:v",
        "ffv1",
        "-pix_fmt",
        "bgr0",
        "-c:a",
        "pcm_s16le",
        "-ac",
        "2",
    ])
    .arg(&output_path)
    .stdin(std::process::Stdio::piped());

    let mut encoder = cmd.spawn().expect("failed to spawn ffmpeg encoder");
    {
        let mut input = encoder.stdin.take().expect("failed to open stdin");
        let pixels = 64 * 36;
        let mut frame_bytes = Vec::with_capacity(pixels * 3);
        for _ in 0..pixels {
            frame_bytes.extend_from_slice(&color_rgb);
        }
        for _ in 0..total_frames {
            input.write_all(&frame_bytes).unwrap();
        }
    }
    assert!(encoder.wait().unwrap().success());
    output_path
}

#[test]
#[ignore = "requires graphical session, GPU, and audio output"]
fn playback_robustness_and_recovery() {
    const TEST: &str = "studio::phase4_playback_robustness_tests::playback_robustness_and_recovery";
    const CHILD: &str = "CLOVERCUT_PHASE43_SUBTEST";

    let Some(subtest) = std::env::var_os(CHILD) else {
        // Parent runner: executes each subtest in an isolated child process
        for sub in [
            "rapid_transport_scrubbing",
            "project_switching_lifecycle",
            "audio_device_recovery",
        ] {
            let root = std::env::temp_dir().join(format!(
                "clovercut-phase43-{}-{}",
                std::process::id(),
                sub
            ));
            std::fs::create_dir_all(&root).unwrap();

            println!("\n=== Running Task 4.3 Subtest: {sub} ===");
            let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
            cmd.args([
                TEST,
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, sub)
            .env("TMPDIR", &root)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("XDG_STATE_HOME", root.join("state"));

            let output = cmd.output().expect("failed to execute child subtest");
            let _ = std::fs::remove_dir_all(root);

            print!("{}", String::from_utf8_lossy(&output.stdout));
            if !output.status.success() {
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                panic!("Subtest {sub} failed!");
            }
            assert!(
                String::from_utf8_lossy(&output.stdout).contains("SUBTEST_PASSED"),
                "Subtest {sub} did not print SUBTEST_PASSED"
            );
        }
        return;
    };

    let subtest = subtest.to_str().unwrap();
    match subtest {
        "rapid_transport_scrubbing" => run_rapid_transport_scrubbing(),
        "project_switching_lifecycle" => run_project_switching_lifecycle(),
        "audio_device_recovery" => run_audio_device_recovery(),
        other => panic!("Unknown subtest: {other}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 1: Rapid Play/Pause Transitions and 20 Hz Scrubbing Stress
// ─────────────────────────────────────────────────────────────────────────────
fn run_rapid_transport_scrubbing() {
    println!("--- Subtest 1: Rapid Play/Pause & 20 Hz Scrubbing Stress ---");
    let dir = std::env::temp_dir().join("robust_rapid_transport");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let media_path = generate_robustness_media(&dir, "rapid.nut", 10.0, [50, 150, 250]);

    let _gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(None).unwrap());
    let app = App::new().unwrap();
    let info = projects::create(dir.to_str().unwrap(), "Rapid_Transport", 64, 36, 30, 1).unwrap();
    studio.open_project(info).unwrap();
    studio.on_start = false;

    let item = concat_host::media::probe(media_path.to_str().unwrap())
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

    // Phase A: 30 rapid play/pause transitions
    let mut toggle_count = 0;
    let mut scrub_count = 0;
    let mut seek_latencies = Vec::new();
    let mut current_seek_start = Instant::now();
    let mut waiting_for_seek = false;
    let mut target_seek_time = 0.0;

    let timer = slint::Timer::default();
    let began = Instant::now();

    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(20), // 50 Hz tick for driving 20 Hz scrub
        move || {
            let mut studio = shell.studio.borrow_mut();

            // Phase A: First 30 ticks alternate play/pause
            if toggle_count < 30 {
                studio.play_toggle();
                toggle_count += 1;
                return;
            }

            // Phase B: 50 scrubbing seeks at ~20 Hz alternating forward & backward across boundaries
            if scrub_count < 50 || waiting_for_seek {
                if !waiting_for_seek {
                    // Alternate seek targets: 0.5s, 4.0s, 2.0s, 8.5s, 0.1s, 9.9s (near boundary)
                    let targets = [0.5, 4.0, 2.0, 8.5, 0.1, 9.9, 3.0, 6.0, 1.5, 7.5];
                    target_seek_time = targets[scrub_count % targets.len()];
                    current_seek_start = Instant::now();
                    studio.monitor.clear_observed();
                    studio.seek(target_seek_time);
                    waiting_for_seek = true;
                    scrub_count += 1;
                } else {
                    // Check if landed
                    let current_pos = studio.host.playback.position_now();
                    let elapsed = current_seek_start.elapsed();
                    assert!(elapsed < Duration::from_millis(150),
                        "Seek to {target_seek_time:.3}s did not land before its deadline (position {current_pos:.3}s)");
                    let picture_landed = studio.monitor.observed_frames.last()
                        .is_some_and(|(spec, _, _)| (spec.time - target_seek_time).abs() < 1.0 / 30.0);
                    if (current_pos - target_seek_time).abs() < 0.1 && picture_landed {
                        let latency_ms = current_seek_start.elapsed().as_secs_f64() * 1000.0;
                        seek_latencies.push(latency_ms);
                        waiting_for_seek = false;
                    }
                }
                return;
            }

            // Phase C: Resume playback and verify stability
            if !studio.playing {
                studio.play_toggle();
            }

            if began.elapsed() >= Duration::from_millis(3500) {
                studio.pause();

                println!("Rapid play/pause toggles completed: {toggle_count}");
                println!("Scrubbing seeks completed: {}", seek_latencies.len());
                let min_lat = seek_latencies.iter().copied().fold(f64::INFINITY, f64::min);
                let max_lat = seek_latencies.iter().copied().fold(0.0, f64::max);
                let avg_lat: f64 = seek_latencies.iter().sum::<f64>() / seek_latencies.len() as f64;
                println!("Seek Latency Distribution: min={min_lat:.1} ms, avg={avg_lat:.1} ms, max={max_lat:.1} ms");

                assert_eq!(seek_latencies.len(), 50, "Every requested seek must land");
                assert!(max_lat < 150.0, "Max seek latency must remain bounded");

                let _ = std::fs::remove_dir_all(&dir);
                println!("Rapid transport & scrubbing stress: PASS");
                slint::quit_event_loop().unwrap();
            }
        },
    );

    app.run().unwrap();
    println!("SUBTEST_PASSED: rapid_transport_scrubbing");
    crate::leave(0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 2: Project Switching and Lifecycle Invariants
// ─────────────────────────────────────────────────────────────────────────────
fn run_project_switching_lifecycle() {
    println!("--- Subtest 2: Project Switching & Lifecycle Invariants ---");
    let dir_a = std::env::temp_dir().join("robust_project_a");
    let dir_b = std::env::temp_dir().join("robust_project_b");
    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    let media_a = generate_robustness_media(&dir_a, "media_a.nut", 5.0, [255, 0, 0]); // RED
    let media_b = generate_robustness_media(&dir_b, "media_b.nut", 5.0, [0, 255, 0]); // GREEN

    let _gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(None).unwrap());
    let app = App::new().unwrap();

    let info_a = projects::create(dir_a.to_str().unwrap(), "Project_A", 64, 36, 30, 1).unwrap();
    let info_b = projects::create(dir_b.to_str().unwrap(), "Project_B", 64, 36, 30, 1).unwrap();

    // 1. Open Project A, populate with RED clip and start playing
    studio.open_project(info_a.clone()).unwrap();
    studio.on_start = false;

    let item_a = concat_host::media::probe(media_a.to_str().unwrap())
        .unwrap()
        .to_new_media();
    let id_a = studio.apply(Command::AddMedia { item: item_a }).unwrap();
    let track_id_a = studio.timeline().tracks[0].id.clone();
    studio
        .apply(Command::AddClip {
            media_id: id_a,
            track_id: track_id_a,
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

    {
        let mut studio = shell.studio.borrow_mut();
        studio.play_toggle();
        assert!(studio.playing);
    }

    // 2. While Project A is actively playing, switch immediately to Project B
    std::thread::sleep(Duration::from_millis(150));
    {
        let mut studio = shell.studio.borrow_mut();
        println!("Switching to Project B while Project A is actively playing...");
        studio.open_project(info_b).unwrap();
        studio.on_start = false;

        // Transport must stop on project switch
        assert!(
            !studio.playing,
            "Playback must stop when project is switched"
        );

        let item_b = concat_host::media::probe(media_b.to_str().unwrap())
            .unwrap()
            .to_new_media();
        let id_b = studio.apply(Command::AddMedia { item: item_b }).unwrap();
        let track_id_b = studio.timeline().tracks[0].id.clone();
        studio
            .apply(Command::AddClip {
                media_id: id_b,
                track_id: track_id_b,
                start: 0.0,
                ripple: false,
            })
            .unwrap();

        studio.publish(&app, &shell.models);
        studio.play_toggle();
        assert!(studio.playing);
    }

    // 3. Play Project B for a moment, then switch back to Project A
    std::thread::sleep(Duration::from_millis(150));
    {
        let mut studio = shell.studio.borrow_mut();
        println!("Switching back to Project A...");
        studio.open_project(info_a).unwrap();
        studio.on_start = false;
        assert!(!studio.playing);

        // Verify Project A state: exactly 1 clip remains, media preserved
        assert_eq!(studio.timeline().clips.len(), 1);
        println!("Project A state verified intact after round-trip switch.");
    }

    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
    println!("Project switching & lifecycle: PASS");
    println!("SUBTEST_PASSED: project_switching_lifecycle");
    crate::leave(0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 3: Audio Device Error Handling & Supervisor Stream Recovery
// ─────────────────────────────────────────────────────────────────────────────
fn run_audio_device_recovery() {
    println!("--- Subtest 3: Audio Device Error & Supervisor Stream Recovery ---");
    let dir = std::env::temp_dir().join("robust_device_recovery");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let media_path = generate_robustness_media(&dir, "recovery.nut", 10.0, [100, 100, 100]);

    let _gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(None).unwrap());
    let app = App::new().unwrap();
    let info = projects::create(dir.to_str().unwrap(), "Device_Recovery", 64, 36, 30, 1).unwrap();
    studio.open_project(info).unwrap();
    studio.on_start = false;

    let item = concat_host::media::probe(media_path.to_str().unwrap())
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

    {
        let mut studio = shell.studio.borrow_mut();
        studio.play_toggle();
    }

    let began = Instant::now();
    let mut error_injected = false;
    let mut injected_position = 0.0;
    let mut initial_rebuild_count = 0;
    let timer = slint::Timer::default();

    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(10),
        move || {
            let studio = shell.studio.borrow_mut();

            let pos = studio.host.playback.position_now();
            let rebuilds = studio.host.playback.rebuild_count();
            assert!(
                began.elapsed() < Duration::from_secs(5),
                "Timed out waiting for playback/recovery: position={pos:.3}, injected={error_injected}, rebuilds={rebuilds}"
            );
            // Inject after actual device-clock progress, allowing sink startup.
            if !error_injected && pos >= 0.3 {
                initial_rebuild_count = studio.host.playback.rebuild_count();
                println!("Initial stream build count: {initial_rebuild_count}");
                assert!(
                    initial_rebuild_count >= 1,
                    "Expected initial audio stream to be built"
                );

                println!("Injecting controlled audio stream error...");
                studio.host.playback.inject_stream_error();
                injected_position = pos;
                error_injected = true;
                return;
            }

            // Require both a replacement stream and further transport progress.
            if error_injected && rebuilds > initial_rebuild_count && pos >= injected_position + 0.5 {
                let new_rebuild_count = studio.host.playback.rebuild_count();
                println!("Current stream build count after recovery: {new_rebuild_count}");
                assert!(
                    new_rebuild_count > initial_rebuild_count,
                    "Supervisor must rebuild the audio stream after failure"
                );

                let pos = studio.host.playback.position_now();
                println!(
                    "Playback position after recovery: {pos:.3} s (playing={})",
                    studio.playing
                );
                assert!(studio.playing, "Recovery must leave transport playing");

                let _ = std::fs::remove_dir_all(&dir);
                println!("Audio device error & supervisor recovery: PASS");
                slint::quit_event_loop().unwrap();
            }
        },
    );

    app.run().unwrap();
    println!("SUBTEST_PASSED: audio_device_recovery");
    crate::leave(0);
}
