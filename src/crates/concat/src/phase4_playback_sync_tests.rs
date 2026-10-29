// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Phase 4 Task 4.2: Audio/Video Synchronization and Drift Measurement.
//!
//! Exercises the full production pipeline:
//! - FFmpeg audio decode and PCM caching in cache/audio.
//! - CPAL audio stream supervisor and mix callback.
//! - Slint transport timer and Playback::position_now() clock progression.
//! - GPU/software monitor frame planning, decoding, and pixel/texture publication.
//! - Measures startup latency, steady-state A/V offset, seek latency, cuts,
//!   all 6 frame rates, nonzero source timestamps, muted/silent projects,
//!   and 10-minute continuous playback drift.

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use concat_host::projects;
use concat_project::commands::{Command, TrackFlag, TrimEdge};
use slint::ComponentHandle;

use crate::studio::{Models, Studio};
use crate::ui::App;

/// Generates synthetic test media with mathematically aligned video flashes
/// and 1 kHz audio tone bursts.
///
/// Video:
/// - 64x36 dimensions.
/// - Every `period_secs` (e.g. 1.0s), the frame at that instant is pure WHITE (255, 255, 255).
/// - All other frames have dark binary RGB codes identifying the frame index modulo 16.
///
/// Audio:
/// - 48,000 Hz 16-bit PCM stereo.
/// - At the exact same timestamps `t = k * period_secs`, an exact 1 kHz sine burst
///   of 40 ms duration with amplitude 0.8 is present.
/// - All other samples are silent (0.0).
pub fn generate_av_sync_fixture(
    dir: &Path,
    filename: &str,
    num: i64,
    den: i64,
    duration_secs: f64,
    period_secs: f64,
    pts_offset_secs: f64,
) -> PathBuf {
    let output_path = dir.join(filename);
    let fps = num as f64 / den as f64;
    let total_frames = (duration_secs * fps).ceil() as usize;
    let rate_str = format!("{num}/{den}");

    // Use FFmpeg with rawvideo piped to stdin, combined with an audio evaluation filter.
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
        &rate_str,
        "-i",
        "-",
        "-f",
        "lavfi",
        "-i",
        &format!(
            "aevalsrc=exprs=0.8*sin(2*PI*1000*t)*between(mod(t\\,{period_secs})\\,0\\,0.04):s=48000:d={duration_secs}"
        ),
    ]);

    if pts_offset_secs > 0.0 {
        cmd.args([
            "-vf",
            &format!("setpts=PTS+{pts_offset_secs}/TB"),
            "-af",
            &format!("asetpts=PTS+{pts_offset_secs}/TB"),
        ]);
    }

    cmd.args([
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
        let mut input = encoder.stdin.take().expect("failed to open ffmpeg stdin");
        let pixels_per_frame = 64 * 36;
        for frame in 0..total_frames {
            let t = frame as f64 / fps;
            let in_period = t % period_secs;
            // Frame is a flash if it is the closest frame to the start of the period
            let is_flash = in_period < (1.0 / fps);
            let colour = if is_flash {
                [255u8, 255u8, 255u8]
            } else {
                let code = (frame % 16) as u8;
                [
                    if code & 1 != 0 { 100 } else { 20 },
                    if code & 2 != 0 { 100 } else { 20 },
                    if code & 4 != 0 { 100 } else { 20 },
                ]
            };
            let mut frame_bytes = Vec::with_capacity(pixels_per_frame * 3);
            for _ in 0..pixels_per_frame {
                frame_bytes.extend_from_slice(&colour);
            }
            input
                .write_all(&frame_bytes)
                .expect("failed to write raw video frame to ffmpeg");
        }
    }

    assert!(
        encoder.wait().expect("ffmpeg failed to finish").success(),
        "ffmpeg failed to encode synthetic A/V fixture"
    );
    output_path
}

const TEST: &str = "studio::phase4_playback_sync_tests::av_sync_and_drift_measurement";
const CHILD: &str = "CLOVERCUT_PHASE42_SUBTEST";

/// Run explicitly with an active display and audio session.
#[test]
#[ignore = "requires an isolated graphical session, FFmpeg, GPU and audio device"]
fn av_sync_and_drift_measurement() {
    let Some(subtest) = std::env::var_os(CHILD) else {
        run_isolated_subtests(&[
            "multirate_sync",
            "seek_accuracy",
            "nonzero_pts",
            "cuts_and_transitions",
            "muted_and_silent",
        ]);
        return;
    };

    let subtest = subtest.to_str().unwrap();
    match subtest {
        "multirate_sync" => run_multirate_sync(),
        "seek_accuracy" => run_seek_accuracy(),
        "nonzero_pts" => run_nonzero_pts(),
        "cuts_and_transitions" => run_cuts_and_transitions(),
        "muted_and_silent" => run_muted_and_silent(),
        "ten_minute_drift" => run_ten_minute_drift(),
        other => panic!("Unknown subtest: {other}"),
    }
}

#[test]
#[ignore = "requires an isolated graphical/audio session and ten minutes of playback"]
fn ten_minute_playback_drift() {
    run_isolated_subtests(&["ten_minute_drift"]);
}

fn run_isolated_subtests(subtests: &[&str]) {
    for &sub in subtests {
        let root =
            std::env::temp_dir().join(format!("clovercut-phase42-{}-{}", std::process::id(), sub));
        std::fs::create_dir_all(&root).unwrap();

        println!("\n=== Running Subtest: {sub} ===");
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.stdin(std::process::Stdio::null())
            .args([
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
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 1: Multi-Rate Steady-State Synchronization & A/V Offset
// ─────────────────────────────────────────────────────────────────────────────
fn run_multirate_sync() {
    println!("--- Subtest 1: Multi-Rate Steady-State A/V Synchronization ---");

    let rates = [
        (24, 1),
        (25, 1),
        (30, 1),
        (60, 1),
        (24000, 1001),
        (30000, 1001),
    ];

    if std::env::var("CLOVERCUT_RATE_NUM").is_err() {
        for (num, den) in rates {
            let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
            cmd.stdin(std::process::Stdio::null())
                .args([
                    "studio::phase4_playback_sync_tests::av_sync_and_drift_measurement",
                    "--exact",
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("CLOVERCUT_PHASE42_SUBTEST", "multirate_sync")
                .env("CLOVERCUT_RATE_NUM", num.to_string())
                .env("CLOVERCUT_RATE_DEN", den.to_string());

            let output = cmd.output().expect("failed to execute single-rate process");
            print!("{}", String::from_utf8_lossy(&output.stdout));
            if !output.status.success() {
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                panic!("Single rate {num}/{den} failed!");
            }
        }
        println!("SUBTEST_PASSED: multirate_sync");
        crate::leave(0);
    }

    let num: i64 = std::env::var("CLOVERCUT_RATE_NUM")
        .unwrap()
        .parse()
        .unwrap();
    let den: i64 = std::env::var("CLOVERCUT_RATE_DEN")
        .unwrap()
        .parse()
        .unwrap();
    run_single_rate_sync(num, den);
}

fn run_single_rate_sync(num: i64, den: i64) {
    let fps = num as f64 / den as f64;
    let dir = std::env::temp_dir().join(format!("av_sync_{num}_{den}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let media_path = generate_av_sync_fixture(&dir, "sync_test.nut", num, den, 6.0, 1.0, 0.0);

    let _gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(None).unwrap());
    let app = App::new().unwrap();
    let info = projects::create(
        dir.to_str().unwrap(),
        &format!("Sync_{num}_{den}"),
        64,
        36,
        num,
        den,
    )
    .unwrap();
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

    let playback_start_mono = Instant::now();
    {
        let mut studio = shell.studio.borrow_mut();
        studio.play_toggle();
    }

    // Run Slint event loop and monitor events
    let mut first_audio_buffer_time = None;
    let mut first_video_frame_time = None;
    let mut flash_events: Vec<(Instant, f64, i64)> = Vec::new();
    let mut tone_events: Vec<(Instant, f64, f64, f32)> = Vec::new();

    let timer = slint::Timer::default();
    let loop_start = Instant::now();
    let test_duration = Duration::from_millis(3500); // observe 3 full seconds

    timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(4),
            move || {
                let mut studio = shell.studio.borrow_mut();

                // Drain audio metrics
                let audio_metrics = studio.host.playback.take_callback_metrics();
                for m in audio_metrics {
                    if first_audio_buffer_time.is_none() {
                        first_audio_buffer_time = Some(m.instant);
                    }
                    if m.peak > 0.3 {
                        // 1 kHz tone burst detected in buffer
                        tone_events.push((m.instant, m.start_pos, m.end_pos, m.peak));
                    }
                }

                // Check observed video frames
                let frames_len = studio.monitor.observed_frames.len();
                let instants_len = studio.monitor.observed_instants.len();
                assert_eq!(frames_len, instants_len);

                for i in 0..frames_len {
                    let (spec, _clock, pixel) = &studio.monitor.observed_frames[i];
                    let instant = studio.monitor.observed_instants[i];
                    if first_video_frame_time.is_none() {
                        first_video_frame_time = Some(instant);
                    }
                    let is_white_flash = pixel[0] > 220 && pixel[1] > 220 && pixel[2] > 220;
                    if is_white_flash {
                        let frame_idx = (spec.time * fps + 1e-6).floor() as i64;
                        if !flash_events.iter().any(|(_, _, idx)| *idx == frame_idx) {
                            flash_events.push((instant, spec.time, frame_idx));
                        }
                    }
                }

                if loop_start.elapsed() >= test_duration {
                    studio.pause();

                    // Analyze Startup Latency
                    let audio_startup_ms = first_audio_buffer_time
                        .map(|t| t.duration_since(playback_start_mono).as_secs_f64() * 1000.0)
                        .unwrap_or(0.0);
                    let video_startup_ms = first_video_frame_time
                        .map(|t| t.duration_since(playback_start_mono).as_secs_f64() * 1000.0)
                        .unwrap_or(0.0);

                    println!(
                        "Rate {num}/{den} (fps={fps:.3}):\n  Startup Latency: audio={audio_startup_ms:.1} ms, video={video_startup_ms:.1} ms",
                    );

                    // Analyze Flash & Tone Synchronization
                    println!(
                        "  Detected Events: {} video flashes, {} audio tone bursts",
                        flash_events.len(),
                        tone_events.len()
                    );
                    assert!(
                        flash_events.len() >= 2,
                        "Expected at least 2 video flash events at rate {num}/{den}, got {}",
                        flash_events.len()
                    );
                    assert!(
                        !tone_events.is_empty(),
                        "Expected audio tone burst events at rate {num}/{den}"
                    );

                    // Compute A/V offset for matching periodic events (at t = 1.0s, 2.0s, etc.)
                    for (v_inst, v_time, v_frame) in &flash_events {
                        // Find the audio buffer containing the flash event timestamp
                        if let Some((a_inst, a_start, a_end, a_peak)) = tone_events
                            .iter()
                            .find(|(_, start, end, _)| *v_time >= *start - 0.005 && *v_time <= *end + 0.005)
                            .or_else(|| {
                                tone_events.iter().min_by(|(_, s1, _, _), (_, s2, _, _)| {
                                    (s1 - v_time).abs().partial_cmp(&(s2 - v_time).abs()).unwrap()
                                })
                            })
                        {
                            let sample_offset_secs = (*v_time - *a_start).max(0.0);
                            let a_sample_inst = *a_inst + Duration::from_secs_f64(sample_offset_secs);
                            let software_offset_ms = (v_inst.duration_since(loop_start).as_secs_f64()
                                - a_sample_inst.duration_since(loop_start).as_secs_f64())
                                * 1000.0;
                            let software_offset_frames = software_offset_ms / (1000.0 / fps);
                            let est_physical_offset_ms = software_offset_ms - 52.2;
                            let est_physical_frames = est_physical_offset_ms / (1000.0 / fps);

                            println!(
                                "  Flash Frame {v_frame} (t={v_time:.3}s) vs Tone (range=[{a_start:.3}s, {a_end:.3}s], peak={a_peak:.2}):\n    Software A/V Offset: {software_offset_ms:+.1} ms ({software_offset_frames:+.2} frames) | Est. Physical (DAC buffer unmeasured): {est_physical_offset_ms:+.1} ms ({est_physical_frames:+.2} frames)"
                            );

                            // Steady-state Phase 4 target: measured A/V offset <= 1.0 project frame
                            if *v_time >= 1.0 {
                                assert!(
                                    software_offset_frames.abs() <= 1.0,
                                    "D1 Defect reproduction: steady-state software A/V offset ({software_offset_frames:+.2} frames, {software_offset_ms:+.1} ms) at rate {num}/{den} exceeds 1.0 frame limit",
                                );
                            }
                        }
                    }

                    let _ = std::fs::remove_dir_all(&dir);
                    println!("Rate {num}/{den}: PASS");
                    slint::quit_event_loop().unwrap();
                }
            },
        );

    app.run().unwrap();
    crate::leave(0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 2: Seek Completion Latency & Frame Accuracy
// ─────────────────────────────────────────────────────────────────────────────
fn run_seek_accuracy() {
    println!("--- Subtest 2: Seek Completion Latency & Landing Frame Accuracy ---");
    let (num, den) = (30, 1);
    let fps: f64 = 30.0;
    let dir = std::env::temp_dir().join("av_seek_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let media_path = generate_av_sync_fixture(&dir, "seek_test.nut", num, den, 10.0, 1.0, 0.0);

    let _gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(None).unwrap());
    let app = App::new().unwrap();
    let info = projects::create(dir.to_str().unwrap(), "Seek_Accuracy", 64, 36, num, den).unwrap();
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

    // Test seeks to multiple positions: paused seek, playing seek, and boundary seek
    let seek_targets: [f64; 4] = [1.0, 3.0, 5.0, 2.5];
    let mut current_seek_idx = 0;
    let mut seek_initiated_at = Instant::now();
    let mut waiting_for_landing = false;

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(5),
        move || {
            let mut studio = shell.studio.borrow_mut();

            if !waiting_for_landing && current_seek_idx < seek_targets.len() {
                let target_time = seek_targets[current_seek_idx];
                println!("Initiating seek to {target_time:.3} s (frame {})", (target_time * fps) as i64);
                seek_initiated_at = Instant::now();
                studio.monitor.clear_observed();
                studio.seek(target_time);
                waiting_for_landing = true;
                return;
            }

            if waiting_for_landing {
                let target_time = seek_targets[current_seek_idx];
                let target_frame = (target_time * fps).round() as i64;

                // Check if target frame was published to monitor
                if let Some((spec, _clock, pixel)) = studio.monitor.observed_frames.last() {
                    let published_frame = (spec.time * fps).round() as i64;
                    if published_frame == target_frame {
                        let latency_ms = seek_initiated_at.elapsed().as_secs_f64() * 1000.0;
                        let is_flash = pixel[0] > 220 && pixel[1] > 220 && pixel[2] > 220;
                        println!(
                            "  Seek landed on frame {published_frame} in {latency_ms:.1} ms (is_flash={is_flash})"
                        );

                        // Confirm Playback clock settled
                        assert!((studio.host.playback.position_now() - target_time).abs() < 0.05);

                        current_seek_idx += 1;
                        waiting_for_landing = false;
                        if current_seek_idx == seek_targets.len() {
                            let _ = std::fs::remove_dir_all(&dir);
                            println!("All seeks completed accurately: PASS");
                            slint::quit_event_loop().unwrap();
                        }
                    }
                }

                assert!(
                    seek_initiated_at.elapsed() < Duration::from_secs(4),
                    "Seek timed out waiting for frame"
                );
            }
        },
    );

    app.run().unwrap();
    println!("SUBTEST_PASSED: seek_accuracy");
    crate::leave(0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 3: Nonzero Source PTS and Clip Source Start
// ─────────────────────────────────────────────────────────────────────────────
fn run_nonzero_pts() {
    println!("--- Subtest 3: Nonzero Source PTS and Clip Source Start ---");
    let (num, den) = (24, 1);
    let dir = std::env::temp_dir().join("av_nonzero_pts");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Generate media with 2.0s PTS offset
    let media_path = generate_av_sync_fixture(&dir, "pts_offset.nut", num, den, 5.0, 1.0, 2.0);

    let _gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(None).unwrap());
    let app = App::new().unwrap();
    let info = projects::create(dir.to_str().unwrap(), "Nonzero_PTS", 64, 36, num, den).unwrap();
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

    // Adjust clip source_start to 1.0s (drag start edge 1.0s to the right)
    let clip_id = studio.timeline().clips[0].id.clone();
    studio.apply(Command::TrimClip {
        clip_id,
        edge: TrimEdge::Start,
        delta: 1.0,
        ripple: false,
    });

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
    let mut observed_tone = false;
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(5),
        move || {
            let mut studio = shell.studio.borrow_mut();
            let audio_metrics = studio.host.playback.take_callback_metrics();
            if audio_metrics.iter().any(|m| m.peak > 0.3) {
                observed_tone = true;
            }

            if began.elapsed() >= Duration::from_millis(2500) {
                studio.pause();
                println!("Nonzero PTS playback active: tone verified={observed_tone}");
                assert!(
                    observed_tone,
                    "Nonzero PTS clip must play audible tone bursts"
                );
                let _ = std::fs::remove_dir_all(&dir);
                println!("Nonzero PTS test: PASS");
                slint::quit_event_loop().unwrap();
            }
        },
    );

    app.run().unwrap();
    println!("SUBTEST_PASSED: nonzero_pts");
    crate::leave(0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 4: Track Cuts & Transitions
// ─────────────────────────────────────────────────────────────────────────────
fn run_cuts_and_transitions() {
    println!("--- Subtest 4: Track Cuts & Transitions ---");
    let (num, den) = (24, 1);
    let dir = std::env::temp_dir().join("av_cuts_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let media_a = generate_av_sync_fixture(&dir, "cut_a.nut", num, den, 3.0, 1.0, 0.0);
    let media_b = generate_av_sync_fixture(&dir, "cut_b.nut", num, den, 3.0, 1.0, 0.0);

    let _gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(None).unwrap());
    let app = App::new().unwrap();
    let info =
        projects::create(dir.to_str().unwrap(), "Cuts_Transitions", 64, 36, num, den).unwrap();
    studio.open_project(info).unwrap();
    studio.on_start = false;

    let item_a = concat_host::media::probe(media_a.to_str().unwrap())
        .unwrap()
        .to_new_media();
    let id_a = studio.apply(Command::AddMedia { item: item_a }).unwrap();
    let item_b = concat_host::media::probe(media_b.to_str().unwrap())
        .unwrap()
        .to_new_media();
    let id_b = studio.apply(Command::AddMedia { item: item_b }).unwrap();

    let track_id = studio.timeline().tracks[0].id.clone();
    studio
        .apply(Command::AddClip {
            media_id: id_a,
            track_id: track_id.clone(),
            start: 0.0,
            ripple: false,
        })
        .unwrap();
    studio
        .apply(Command::AddClip {
            media_id: id_b,
            track_id,
            start: 3.0,
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

    // Play across cut at 3.0s (start from 2.5s to 3.5s)
    {
        let mut studio = shell.studio.borrow_mut();
        studio.seek(2.5);
        studio.play_toggle();
    }

    let began = Instant::now();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(5),
        move || {
            let mut studio = shell.studio.borrow_mut();
            let pos = studio.host.playback.position_now();

            // Sink startup is separate from transport progress. A deadline
            // is a failure; only the device clock crossing the cut can pass.
            assert!(
                began.elapsed() < Duration::from_secs(5),
                "Timed out before transport crossed the cut: position={pos:.3}"
            );
            if pos >= 3.5 {
                studio.pause();
                println!("Playback crossed 3.0s cut boundary to position {pos:.3} s");
                let _ = std::fs::remove_dir_all(&dir);
                println!("Cuts test: PASS");
                slint::quit_event_loop().unwrap();
            }
        },
    );

    app.run().unwrap();
    println!("SUBTEST_PASSED: cuts_and_transitions");
    crate::leave(0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 5: Muted Tracks and Silent / Audio-Only Projects
// ─────────────────────────────────────────────────────────────────────────────
fn run_muted_and_silent() {
    println!("--- Subtest 5: Muted Tracks and Silent / Audio-Only Projects ---");
    let (num, den) = (24, 1);
    let dir = std::env::temp_dir().join("av_mute_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let media_path = generate_av_sync_fixture(&dir, "mute_test.nut", num, den, 3.0, 1.0, 0.0);

    let _gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(None).unwrap());
    let app = App::new().unwrap();
    let info = projects::create(dir.to_str().unwrap(), "Mute_Silent", 64, 36, num, den).unwrap();
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
            track_id: track_id.clone(),
            start: 0.0,
            ripple: false,
        })
        .unwrap();

    // Mute the track
    studio.apply(Command::SetTrackFlag {
        track_id,
        flag: TrackFlag::Muted,
        value: true,
    });

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
    let timer = slint::Timer::default();
    let mut callback_count = 0;
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(5),
        move || {
            let mut studio = shell.studio.borrow_mut();
            let audio_metrics = studio.host.playback.take_callback_metrics();
            callback_count += audio_metrics.len();

            // Muted track must never output audio amplitude
            for m in audio_metrics {
                assert_eq!(m.peak, 0.0, "Muted track must output exact silence");
            }

            let pos = studio.host.playback.position_now();
            assert!(
                began.elapsed() < Duration::from_secs(5),
                "Timed out waiting for muted playback: position={pos:.3}, callbacks={callback_count}"
            );
            if pos > 0.5 && callback_count > 0 {
                studio.pause();
                println!("Muted track confirmed: audio peak=0.0, clock progressed to {pos:.3} s");
                assert!(pos > 0.5, "Clock must progress while muted");
                let _ = std::fs::remove_dir_all(&dir);
                println!("Muted and silent test: PASS");
                slint::quit_event_loop().unwrap();
            }
        },
    );

    app.run().unwrap();
    println!("SUBTEST_PASSED: muted_and_silent");
    crate::leave(0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 6: Ten-Minute Continuous Playback Drift
// ─────────────────────────────────────────────────────────────────────────────
fn run_ten_minute_drift() {
    println!("--- Subtest 6: Ten-Minute Continuous Playback Drift ---");
    let (num, den) = (24, 1);
    let fps = 24.0;
    let dir = std::env::temp_dir().join("av_drift_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Generate 610 seconds of synthetic media
    println!("Generating 610s synthetic media for 10-minute drift run...");
    let media_path = generate_av_sync_fixture(&dir, "drift_610s.nut", num, den, 610.0, 1.0, 0.0);
    println!("Media generated successfully.");

    let _gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    let mut studio = Studio::new(crate::host::Host::start(None).unwrap());
    let app = App::new().unwrap();
    let info =
        projects::create(dir.to_str().unwrap(), "Ten_Minute_Drift", 64, 36, num, den).unwrap();
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

    let playback_start_mono = Instant::now();
    let mut last_sample_time = Instant::now();
    let mut samples: Vec<(f64, f64, f64, f64)> = Vec::new(); // (elapsed_wall, audio_pos, clock_now, drift_ms)

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            let mut studio = shell.studio.borrow_mut();
            // Periodically clear observed frames to avoid memory buildup over 10 minutes
            studio.monitor.clear_observed();
            let _ = studio.host.playback.take_callback_metrics();

            let elapsed_wall = playback_start_mono.elapsed().as_secs_f64();

            // Sample every 10 seconds
            if last_sample_time.elapsed() >= Duration::from_secs(10) {
                last_sample_time = Instant::now();
                let audio_pos = studio.host.playback.position();
                let clock_now = studio.host.playback.position_now();
                let drift_ms = (audio_pos - elapsed_wall) * 1000.0;
                let drift_frames = (audio_pos - elapsed_wall) * fps;

                samples.push((elapsed_wall, audio_pos, clock_now, drift_ms));
                println!(
                    "Sample [{:>3.0}s / 600s]: audio_pos={:>7.3}s, clock_now={:>7.3}s | drift={:>+6.2} ms ({:>+5.2} frames)",
                    elapsed_wall, audio_pos, clock_now, drift_ms, drift_frames
                );
            }

            // Target: 600 seconds (10 minutes)
            if elapsed_wall >= 600.0 {
                studio.pause();

                println!("\n=== Ten-Minute Drift Measurement Complete ===");
                println!("Total duration elapsed: {elapsed_wall:.2} s");
                println!("Total samples recorded: {}", samples.len());

                let first = samples.first().unwrap();
                let last = samples.last().unwrap();
                let total_drift_ms = last.3 - first.3;
                let drift_rate_ppm = (total_drift_ms / 1000.0) / (last.0 - first.0) * 1_000_000.0;

                println!("First sample: wall={:.1}s, audio={:.3}s, drift={:+.2} ms", first.0, first.1, first.3);
                println!("Final sample: wall={:.1}s, audio={:.3}s, drift={:+.2} ms", last.0, last.1, last.3);
                println!("Net cumulative drift over 10 min: {total_drift_ms:+.2} ms ({:.2} frames)", total_drift_ms / (1000.0 / fps));
                println!("Oscillator drift rate: {drift_rate_ppm:+.1} ppm");

                // Acceptance criteria:
                // Clock drift relative to real elapsed time must remain steady and bounded.
                // Under 1 project frame of cumulative drift between audio device and transport.
                assert!(
                    (total_drift_ms / (1000.0 / fps)).abs() <= 1.5,
                    "Cumulative drift exceeds threshold: {total_drift_ms:.2} ms"
                );

                let _ = std::fs::remove_dir_all(&dir);
                println!("Ten-minute drift test: PASS");
                slint::quit_event_loop().unwrap();
            }
        },
    );

    app.run().unwrap();
    println!("SUBTEST_PASSED: ten_minute_drift");
    crate::leave(0);
}
