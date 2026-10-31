// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Opt-in preview/API-call timings and project-close lifecycle assertions.
//!
//! Samples describe host calls, not device execution, presented-frame latency or
//! a proven memory plateau. OS caches are uncontrolled. Structured records retain
//! raw samples and distinguish unavailable process counters from device-wide VRAM.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use concat_host::projects;
use concat_project::Command;
use concat_project::commands::ClipPatch;
use concat_project::model::TextStyle;

use super::{Models, Studio};

#[path = "performance_metrics.rs"]
mod metrics;

fn read_process_memory_bytes() -> metrics::ProcessMemory {
    std::fs::read_to_string("/proc/self/status")
        .map(|status| metrics::process_memory(&status))
        .unwrap_or_default()
}

// One JSON object per line survives partial failures through the child stdout.
fn record(event: &str, data: serde_json::Value) {
    println!(
        "CLOVERCUT_BENCHMARK {}",
        serde_json::json!({"schema_version": 1, "event": event, "data": data})
    );
}

fn record_samples(scenario: &str, run: usize, samples: &[f64]) {
    let (mean, p95, max) = stats_summary(samples);
    record(
        "samples",
        serde_json::json!({
            "scenario": scenario, "run": run + 1, "unit": "ms",
            "values": samples, "mean": mean, "p95": p95, "max": max,
        }),
    );
}

fn record_memory(stage: &str) {
    let memory = read_process_memory_bytes();
    record(
        "memory",
        serde_json::json!({
            "stage": stage, "rss_bytes": memory.rss_bytes,
            "high_water_bytes": memory.high_water_bytes,
            "drm_device_wide_vram_bytes": metrics::drm_device_vram(Path::new("/sys/class/drm")),
        }),
    );
}

fn generate_fixtures(dir: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let hd_a = dir.join("hd_clip_a.mp4");
    let hd_b = dir.join("hd_clip_b.mp4");
    let uhd_4k = dir.join("uhd_4k_layer.mp4");
    let long_audio = dir.join("long_audio_180s.wav");

    if !hd_a.exists() {
        assert!(
            std::process::Command::new("ffmpeg")
                .args([
                    "-v",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=1920x1080:rate=30",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=440:sample_rate=48000",
                    "-t",
                    "4",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-g",
                    "15",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                ])
                .arg(&hd_a)
                .status()
                .unwrap()
                .success()
        );
    }

    if !hd_b.exists() {
        assert!(
            std::process::Command::new("ffmpeg")
                .args([
                    "-v",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=1920x1080:rate=30",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=660:sample_rate=48000",
                    "-t",
                    "4",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-g",
                    "15",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                ])
                .arg(&hd_b)
                .status()
                .unwrap()
                .success()
        );
    }

    if !uhd_4k.exists() {
        assert!(
            std::process::Command::new("ffmpeg")
                .args([
                    "-v",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=3840x2160:rate=30",
                    "-t",
                    "2",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-g",
                    "15",
                    "-pix_fmt",
                    "yuv420p",
                    "-an",
                ])
                .arg(&uhd_4k)
                .status()
                .unwrap()
                .success()
        );
    }

    if !long_audio.exists() {
        assert!(
            std::process::Command::new("ffmpeg")
                .args([
                    "-v",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=330:sample_rate=48000",
                    "-t",
                    "180",
                    "-ac",
                    "2",
                    "-c:a",
                    "pcm_s16le",
                ])
                .arg(&long_audio)
                .status()
                .unwrap()
                .success()
        );
    }

    (hd_a, hd_b, uhd_4k, long_audio)
}

fn stats_summary(samples: &[f64]) -> (f64, f64, f64) {
    metrics::stats_summary(samples).expect("benchmark samples must be nonempty and finite")
}

#[test]
#[ignore = "requires an isolated graphical session, FFmpeg, GPU and audio device"]
fn phase6_performance_and_memory_suite() {
    const CHILD: &str = "CLOVERCUT_PHASE6_PERF_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let root = std::env::temp_dir().join(format!("clovercut-phase6-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "studio::phase6_performance_tests::phase6_performance_and_memory_suite",
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
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        print!("{stdout}");
        eprint!("{stderr}");
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            output.status.success(),
            "benchmark child failed: {} (diagnostics printed above)",
            output.status
        );
        return;
    }

    let root = PathBuf::from(std::env::var("TMPDIR").unwrap());
    let media_dir = root.join("media");
    std::fs::create_dir_all(&media_dir).unwrap();
    let fixture_start = Instant::now();
    let (hd_a, hd_b, uhd_4k, long_audio) = generate_fixtures(&media_dir);
    record_samples(
        "fixture_generation",
        0,
        &[fixture_start.elapsed().as_secs_f64() * 1000.0],
    );
    record_memory("before_initialisation");

    // -------------------------------------------------------------------------
    // TASK 6.1: Initialisation, seek and sequential preview API calls
    // -------------------------------------------------------------------------
    let t_cold_start = Instant::now();
    let gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
    // The software window deliberately does not share its device with the
    // monitor. This texture-call probe needs a dedicated compositor device;
    // it does not measure the production CPU-window presentation path.
    let shared_window_device = gpu.is_some();
    let monitor_gpu = gpu
        .or_else(crate::gpu::Gpu::acquire)
        .expect("texture-call benchmark requires a compositor adapter");
    let mut studio = Studio::new(crate::host::Host::start(Some(monitor_gpu)).unwrap());
    let app = crate::ui::App::new().unwrap();
    let models = Models::new();
    studio.workspace = (1920.0, 1080.0);
    studio.lanes.width = 1880.0;
    studio.publish(&app, &models);
    let cold_startup_ms = t_cold_start.elapsed().as_secs_f64() * 1000.0;

    record(
        "context",
        serde_json::json!({
            "os": std::env::consts::OS, "architecture": std::env::consts::ARCH,
            "source_sha": std::env::var("CLOVERCUT_BENCHMARK_SOURCE").ok(),
            "machine_label": std::env::var("CLOVERCUT_BENCHMARK_MACHINE").ok(),
            "toolchain": std::env::var("CLOVERCUT_BENCHMARK_TOOLCHAIN").ok(),
            "window_adapter": crate::gpu::adapter_description(),
            "monitor_device": if shared_window_device { "shared with window" } else { "dedicated; texture-call probe only" },
            "physical_ram_bytes": concat_host::memory::total(),
            "reader_pool_budget_bytes": concat_media::ReaderPool::frame_budget(concat_host::memory::total()),
            "preview_pixels": [960, 540], "layered_pixels": [3840, 2160],
            "audio_seconds": 180, "caption_cues": 240, "cache_state": "uncontrolled",
            "gpu_timing": "host texture_of call; not device execution or presentation latency",
            "drm_scope": "device-wide; not per-process or tied to the selected adapter",
        }),
    );
    record_samples("initialisation_and_publish", 0, &[cold_startup_ms]);
    let mut warm_open_runs = Vec::new();
    let proj_6_1 =
        projects::create(root.to_str().unwrap(), "Bench_6_1", 1920, 1080, 30, 1).unwrap();
    for run in 0..3 {
        let t0 = Instant::now();
        studio.open_project(proj_6_1.clone()).unwrap();
        studio.publish(&app, &models);
        warm_open_runs.push(t0.elapsed().as_secs_f64() * 1000.0);
        if run < 2 {
            studio.close_project().unwrap();
        }
    }
    println!(
        "[6.1 Startup] Init+publish: {:.2} ms | Empty project open (3 runs): {:.2} ms, {:.2} ms, {:.2} ms",
        cold_startup_ms, warm_open_runs[0], warm_open_runs[1], warm_open_runs[2]
    );

    record_samples("empty_project_open_and_publish", 0, &warm_open_runs);

    // Populate Bench_6_1 with 2 HD clips
    let m_hd_a = studio
        .apply(Command::AddMedia {
            item: concat_host::media::probe(hd_a.to_str().unwrap())
                .unwrap()
                .to_new_media(),
        })
        .unwrap();
    let m_hd_b = studio
        .apply(Command::AddMedia {
            item: concat_host::media::probe(hd_b.to_str().unwrap())
                .unwrap()
                .to_new_media(),
        })
        .unwrap();
    let v_track = studio.timeline().tracks[0].id.clone();
    studio
        .apply(Command::AddClip {
            media_id: m_hd_a.clone(),
            track_id: v_track.clone(),
            start: 0.0,
            ripple: false,
        })
        .unwrap();
    studio
        .apply(Command::AddClip {
            media_id: m_hd_b.clone(),
            track_id: v_track.clone(),
            start: 4.0,
            ripple: false,
        })
        .unwrap();

    // Two passes over 20 timestamps; only monitor state is explicitly cleared.
    // Reader, driver and OS caches are not qualified as cold or warm.
    for run in 0..3 {
        studio.host.monitor.clear();
        let mut cold_seeks = Vec::new();
        let mut warm_seeks = Vec::new();
        for idx in 0..20 {
            let t_sec = (idx as f64 * 0.38).min(7.5);
            let t0 = Instant::now();
            let (clips, settings) = studio.preview_clips(960, 540, t_sec).unwrap();
            let spec = concat_host::preview::FrameSpec {
                canvas: [0, 0, 0, 255],
                time: t_sec,
                width: 960,
                height: 540,
                moving: false,
                proxy: false,
                color_space: concat_project::model::ColorSpace::default(),
            };
            let sources = studio
                .host
                .monitor
                .frame_sources(clips, &settings, spec)
                .unwrap();
            let _tex = studio.host.monitor.texture_of(&sources, spec).unwrap();
            cold_seeks.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        for idx in 0..20 {
            let t_sec = (idx as f64 * 0.38).min(7.5);
            let t0 = Instant::now();
            let (clips, settings) = studio.preview_clips(960, 540, t_sec).unwrap();
            let spec = concat_host::preview::FrameSpec {
                canvas: [0, 0, 0, 255],
                time: t_sec,
                width: 960,
                height: 540,
                moving: false,
                proxy: false,
                color_space: concat_project::model::ColorSpace::default(),
            };
            let sources = studio
                .host
                .monitor
                .frame_sources(clips, &settings, spec)
                .unwrap();
            let _tex = studio.host.monitor.texture_of(&sources, spec).unwrap();
            warm_seeks.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        record_samples("seek_first_pass_after_monitor_clear", run, &cold_seeks);
        record_samples("seek_repeated_timestamps", run, &warm_seeks);
        let (c_mean, c_p95, c_max) = stats_summary(&cold_seeks);
        let (w_mean, w_p95, w_max) = stats_summary(&warm_seeks);
        println!(
            "[6.1 Seek Run {}] First pass after monitor clear (20 pts): mean={:.2}ms p95={:.2}ms max={:.2}ms | Repeated timestamps: mean={:.3}ms p95={:.3}ms max={:.3}ms",
            run + 1,
            c_mean,
            c_p95,
            c_max,
            w_mean,
            w_p95,
            w_max
        );
    }

    // Sequential preview calls, not paced presentation or audio/video sync.
    for run in 0..3 {
        studio.host.monitor.clear();
        let mut frame_times = Vec::new();
        let mut over_33ms = 0usize;
        for f in 0..90 {
            let t_sec = f as f64 / 30.0;
            let t0 = Instant::now();
            let (clips, settings) = studio.preview_clips(960, 540, t_sec).unwrap();
            let spec = concat_host::preview::FrameSpec {
                canvas: [0, 0, 0, 255],
                time: t_sec,
                width: 960,
                height: 540,
                moving: true,
                proxy: false,
                color_space: concat_project::model::ColorSpace::default(),
            };
            let sources = studio
                .host
                .monitor
                .frame_sources(clips, &settings, spec)
                .unwrap();
            let _tex = studio.host.monitor.texture_of(&sources, spec).unwrap();
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            if ms > 33.333 {
                over_33ms += 1;
            }
            frame_times.push(ms);
        }
        record_samples("sequential_preview_call", run, &frame_times);
        let (mean, p95, max) = stats_summary(&frame_times);
        println!(
            "[6.1 Playback 90f Run {}] mean={:.2}ms p95={:.2}ms max={:.2}ms (>33.3ms frames: {}/90)",
            run + 1,
            mean,
            p95,
            max,
            over_33ms
        );
    }

    studio.close_project().unwrap();

    // -------------------------------------------------------------------------
    // TASK 6.2A: Long Audio Project (180s stereo 48kHz WAV + 12 spans)
    // -------------------------------------------------------------------------
    let proj_audio =
        projects::create(root.to_str().unwrap(), "Bench_6_2_Audio", 1920, 1080, 30, 1).unwrap();
    studio.open_project(proj_audio).unwrap();
    // Exercise the actual library adoption path, including content comparison,
    // copy, probe, document commands and selection. Repeated uses reuse the copy.
    // These synchronous call timings do not measure event-loop/presentation delay.
    for run in 0..3 {
        let started = Instant::now();
        studio.place_sound("Long audio", &long_audio, None, 0.0);
        record_samples(
            "library_audio_adoption_and_place",
            run,
            &[started.elapsed().as_secs_f64() * 1000.0],
        );
        assert!(!studio.selection.is_empty(), "library audio must be placed");
        studio.undo();
    }
    let m_audio = studio
        .apply(Command::AddMedia {
            item: concat_host::media::probe(long_audio.to_str().unwrap())
                .unwrap()
                .to_new_media(),
        })
        .unwrap();
    let a_track = studio.timeline().tracks[1].id.clone();
    for i in 0..12 {
        let cid = studio
            .apply(Command::AddClip {
                media_id: m_audio.clone(),
                track_id: a_track.clone(),
                start: i as f64 * 15.0,
                ripple: false,
            })
            .unwrap();
        let in_offset = i as f64 * 5.0;
        studio.apply(Command::TrimClip {
            clip_id: cid.clone(),
            edge: concat_project::commands::TrimEdge::End,
            delta: -(180.0 - (in_offset + 14.0)),
            ripple: false,
        });
        if in_offset > 0.0 {
            studio.apply(Command::TrimClip {
                clip_id: cid.clone(),
                edge: concat_project::commands::TrimEdge::Start,
                delta: in_offset,
                ripple: false,
            });
            studio.apply(Command::MoveClips {
                moves: vec![concat_project::commands::ClipMove {
                    clip_id: cid,
                    track_id: a_track.clone(),
                    start: i as f64 * 15.0,
                }],
            });
        }
    }

    for run in 0..3 {
        let t_peaks = Instant::now();
        let pyramid = concat_media::Pyramid::of(
            concat_media::peaks::extract(&long_audio, 200, None).unwrap(),
        );
        let peaks_ms = t_peaks.elapsed().as_secs_f64() * 1000.0;
        studio.peaks.insert(m_audio.clone(), Arc::new(pyramid));

        record_samples("audio_peaks_extraction", run, &[peaks_ms]);

        // Clear cached wave SVG strings to measure cold vs warm wave_path generation
        studio.waves.borrow_mut().clear();
        let t_pub_cold = Instant::now();
        studio.publish_lanes(&app, &models);
        let pub_cold_ms = t_pub_cold.elapsed().as_secs_f64() * 1000.0;

        let t_pub_warm = Instant::now();
        studio.publish_lanes(&app, &models);
        let pub_warm_ms = t_pub_warm.elapsed().as_secs_f64() * 1000.0;
        record_samples("audio_publish_cleared_waves", run, &[pub_cold_ms]);
        record_samples("audio_publish_repeated", run, &[pub_warm_ms]);

        println!(
            "[6.2A Long Audio 180s Run {}] peaks_extract={:.2}ms | publish_lanes(cold_waves)={:.3}ms | publish_lanes(warm_waves)={:.3}ms | mapped_pcm={:.2}MB ({})",
            run + 1,
            peaks_ms,
            pub_cold_ms,
            pub_warm_ms,
            studio.host.playback.mapped_bytes() as f64 / 1048576.0,
            studio.host.playback.mapped_count()
        );
    }
    // Wait briefly so at least some audio decode workers finish mapping spans
    std::thread::sleep(Duration::from_millis(500));
    println!(
        "[6.2A Long Audio After Workers] mapped_pcm={:.2}MB ({} spans)",
        studio.host.playback.mapped_bytes() as f64 / 1048576.0,
        studio.host.playback.mapped_count()
    );
    studio.close_project().unwrap();

    // -------------------------------------------------------------------------
    // TASK 6.2B: Caption-Heavy Project (240 caption cues + video & audio clips)
    // -------------------------------------------------------------------------
    let proj_caps = projects::create(
        root.to_str().unwrap(),
        "Bench_6_2_Captions",
        1920,
        1080,
        30,
        1,
    )
    .unwrap();
    studio.open_project(proj_caps).unwrap();
    let m_vid = studio
        .apply(Command::AddMedia {
            item: concat_host::media::probe(hd_a.to_str().unwrap())
                .unwrap()
                .to_new_media(),
        })
        .unwrap();
    let v_track = studio.timeline().tracks[0].id.clone();
    for i in 0..30 {
        studio.apply(Command::AddClip {
            media_id: m_vid.clone(),
            track_id: v_track.clone(),
            start: i as f64 * 4.0,
            ripple: false,
        });
    }
    // Build 240 caption cues (0.5s each across 120s)
    let mut srt = String::new();
    for i in 0..240 {
        let s_ms = i * 500;
        let e_ms = s_ms + 450;
        let fmt_srt = |ms: usize| {
            let h = ms / 3_600_000;
            let m = (ms / 60_000) % 60;
            let s = (ms / 1_000) % 60;
            let rem = ms % 1_000;
            format!("{h:02}:{m:02}:{s:02},{rem:03}")
        };
        srt.push_str(&format!(
            "{}\n{} --> {}\nCaption cue number {} with shadow and stroke\n\n",
            i + 1,
            fmt_srt(s_ms),
            fmt_srt(e_ms),
            i + 1
        ));
    }
    studio
        .apply(Command::AddCaptionSource {
            name: "240_cues.srt".into(),
            srt,
            style: TextStyle {
                shadow: true,
                stroke_width: 0.004,
                background: "#00000080".into(),
                ..TextStyle::default()
            },
            offset_y: 0.35,
            place_at: Some(0.0),
        })
        .unwrap();

    println!(
        "[6.2B Caption-Heavy Setup] Total timeline clips: {}",
        studio.timeline().clips.len()
    );

    for run in 0..3 {
        // 1. Measure flatten_timeline_in directly
        let project_dir = PathBuf::from(studio.session.as_ref().unwrap().path());
        let t_flat = Instant::now();
        for _ in 0..60 {
            let _ = concat_export::flatten::flatten_timeline_in(
                studio.project(),
                None,
                Some(&project_dir),
            );
        }
        let flat_60_ms = t_flat.elapsed().as_secs_f64() * 1000.0;

        // 2. Measure preview_clips across 60 caption cue transitions (t = 0.1, 0.6, 1.1, ...)
        let t_preview_clips = Instant::now();
        for cue in 0..60 {
            let time = cue as f64 * 0.5 + 0.1;
            let _ = studio.preview_clips(960, 540, time).unwrap();
        }
        let preview_60_ms = t_preview_clips.elapsed().as_secs_f64() * 1000.0;

        // 3. Measure UI publish_lanes on caption-heavy timeline
        let t_pub = Instant::now();
        for cue in 0..20 {
            studio.playhead = cue as f64 * 0.5 + 0.1;
            studio.publish_lanes(&app, &models);
        }
        let pub_20_ms = t_pub.elapsed().as_secs_f64() * 1000.0;
        record_samples("caption_flatten_60_calls_total", run, &[flat_60_ms]);
        record_samples("caption_preview_60_calls_total", run, &[preview_60_ms]);
        record_samples("caption_publish_20_calls_total", run, &[pub_20_ms]);

        println!(
            "[6.2B Caption-Heavy 240-cues Run {}] 60x flatten_timeline_in={:.2}ms ({:.3}ms/call) | 60x preview_clips(cue transitions)={:.2}ms ({:.3}ms/cue) | 20x publish_lanes={:.2}ms ({:.3}ms/pub) | titles_live={:.1}MB ({})",
            run + 1,
            flat_60_ms,
            flat_60_ms / 60.0,
            preview_60_ms,
            preview_60_ms / 60.0,
            pub_20_ms,
            pub_20_ms / 20.0,
            studio.host.titles.live_bytes() as f64 / 1048576.0,
            studio.host.titles.live_count()
        );
    }

    // -------------------------------------------------------------------------
    // TASK 6.3 (Part 1): 200 Undo / Redo Steps on the Caption Project
    // -------------------------------------------------------------------------
    let first_clip_id = studio.timeline().clips[0].id.clone();
    for run in 0..3 {
        let t_apply = Instant::now();
        for step in 0..210 {
            let op = 0.2 + ((step % 80) as f64) * 0.01;
            studio.apply(Command::UpdateClip {
                clip_id: first_clip_id.clone(),
                patch: ClipPatch {
                    opacity: Some(op),
                    ..Default::default()
                },
            });
        }
        let apply_ms = t_apply.elapsed().as_secs_f64() * 1000.0;

        let t_undo = Instant::now();
        let mut undone = 0usize;
        while studio.session.as_ref().unwrap().can_undo() {
            studio.session.as_mut().unwrap().undo();
            undone += 1;
        }
        let undo_ms = t_undo.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(undone, 200, "Editor undo depth must cap at UNDO_DEPTH=200");

        let t_redo = Instant::now();
        let mut redone = 0usize;
        while studio.session.as_ref().unwrap().can_redo() {
            studio.session.as_mut().unwrap().redo();
            redone += 1;
        }
        let redo_ms = t_redo.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(redone, 200);

        record_samples("history_apply_210_calls_total", run, &[apply_ms]);
        record_samples("history_undo_200_calls_total", run, &[undo_ms]);
        record_samples("history_redo_200_calls_total", run, &[redo_ms]);
        record_memory("after_history_cycle");
        let rss_now = read_process_memory_bytes().rss_bytes;
        println!(
            "[6.3 200-Undo/Redo Run {}] 210 applies={:.2}ms | 200 undos={:.2}ms ({:.4}ms/step) | 200 redos={:.2}ms ({:.4}ms/step) | RSS bytes={:?}",
            run + 1,
            apply_ms,
            undo_ms,
            undo_ms / 200.0,
            redo_ms,
            redo_ms / 200.0,
            rss_now
        );
    }

    studio.close_project().unwrap();

    // -------------------------------------------------------------------------
    // TASK 6.2C: Layered 4K Project (3840x2160 two video layers + 4K shadow title)
    // -------------------------------------------------------------------------
    let proj_4k =
        projects::create(root.to_str().unwrap(), "Bench_6_2_4K", 3840, 2160, 30, 1).unwrap();
    studio.open_project(proj_4k).unwrap();
    let m_4k = studio
        .apply(Command::AddMedia {
            item: concat_host::media::probe(uhd_4k.to_str().unwrap())
                .unwrap()
                .to_new_media(),
        })
        .unwrap();
    let m_hd = studio
        .apply(Command::AddMedia {
            item: concat_host::media::probe(hd_b.to_str().unwrap())
                .unwrap()
                .to_new_media(),
        })
        .unwrap();
    let t0_id = studio.timeline().tracks[0].id.clone();
    studio
        .apply(Command::AddClip {
            media_id: m_4k,
            track_id: t0_id,
            start: 0.0,
            ripple: false,
        })
        .unwrap();
    let t_pip_id = studio
        .apply(Command::AddTrack {
            bottom: false,
            kind: None,
            insert_at: None,
        })
        .unwrap();
    let pip_cid = studio
        .apply(Command::AddClip {
            media_id: m_hd,
            track_id: t_pip_id,
            start: 0.0,
            ripple: false,
        })
        .unwrap();
    studio.apply(Command::SetClipTransform {
        clip_id: pip_cid,
        scale: Some(0.5),
        offset_x: Some(0.2),
        offset_y: Some(-0.2),
        rotation: Some(5.0),
        stretch_x: None,
        stretch_y: None,
    });
    studio
        .apply(Command::AddTextClip {
            track_id: None,
            above: true,
            start: 0.0,
            style: Some(TextStyle {
                content: "4K Layered Compositing & Shadow Title Benchmark".into(),
                font_size: 0.06,
                shadow: true,
                stroke_width: 0.004,
                background: "#101820cc".into(),
                ..TextStyle::default()
            }),
            duration: Some(2.0),
            offset_y: Some(0.32),
        })
        .unwrap();

    // Measure 4K concat_text::render_frame directly (3 runs of 5 renders)
    let fonts = concat_text::Fonts::new();
    for run in 0..3 {
        let mut paint_times = Vec::new();
        for i in 0..5 {
            let t_paint = Instant::now();
            let rendered = concat_text::render_frame(
                &fonts,
                &concat_text::TitleStyle {
                    content: format!("4K Shadow Title Frame Benchmark #{i}"),
                    font_family: "Hanken Grotesk".into(),
                    font_size: 0.06,
                    font_weight: 700.0,
                    italic: false,
                    underline: false,
                    strikethrough: false,
                    vertical_align: 1,
                    vertical: false,
                    color: "#ffffff".into(),
                    align: concat_text::Align::Center,
                    stroke_width: 0.004,
                    stroke_color: "#000000".into(),
                    shadow: true,
                    background: "#101820cc".into(),
                    background_radius: 0.015,
                    background_padding_x: 0.02,
                    background_padding_y: 0.01,
                    line_height: 1.2,
                    tracking: 0.0,
                    max_width: 0.8,
                    max_height: 0.0,
                    runs: Vec::new(),
                },
                3840,
                2160,
            )
            .unwrap();
            assert_eq!(rendered.width, 3840);
            paint_times.push(t_paint.elapsed().as_secs_f64() * 1000.0);
        }
        record_samples("4k_title_render_call", run, &paint_times);
        let (mean, p95, max) = stats_summary(&paint_times);
        println!(
            "[6.2C 4K Title Paint (3840x2160 shadow+stroke+plate) Run {}] mean={:.2}ms p95={:.2}ms max={:.2}ms",
            run + 1,
            mean,
            p95,
            max
        );
    }

    // Host call timings, without device timestamp queries or presentation waits.
    for run in 0..3 {
        studio.host.monitor.clear();
        let mut decode_times = Vec::new();
        let mut gpu_times = Vec::new();
        for f in 0..10 {
            let t_sec = f as f64 / 30.0;
            let (clips, settings) = studio.preview_clips(3840, 2160, t_sec).unwrap();
            let spec = concat_host::preview::FrameSpec {
                canvas: [0, 0, 0, 255],
                time: t_sec,
                width: 3840,
                height: 2160,
                moving: false,
                proxy: false,
                color_space: concat_project::model::ColorSpace::default(),
            };
            let t_dec = Instant::now();
            let sources = studio
                .host
                .monitor
                .frame_sources(clips, &settings, spec)
                .unwrap();
            decode_times.push(t_dec.elapsed().as_secs_f64() * 1000.0);

            let t_gpu = Instant::now();
            let _tex = studio.host.monitor.texture_of(&sources, spec).unwrap();
            gpu_times.push(t_gpu.elapsed().as_secs_f64() * 1000.0);
        }
        record_samples("4k_frame_sources_call", run, &decode_times);
        record_samples("4k_texture_of_host_call", run, &gpu_times);
        let (d_mean, d_p95, d_max) = stats_summary(&decode_times);
        let (g_mean, g_p95, g_max) = stats_summary(&gpu_times);
        println!(
            "[6.2C 4K Layered 3840x2160 Run {}] decode(frame_sources): mean={:.2}ms p95={:.2}ms max={:.2}ms | texture_of(host call): mean={:.2}ms p95={:.2}ms max={:.2}ms | pool={:.1}MB gpu_pool={:.1}MB",
            run + 1,
            d_mean,
            d_p95,
            d_max,
            g_mean,
            g_p95,
            g_max,
            studio.host.monitor.pool_cached_bytes() as f64 / 1048576.0,
            studio.host.monitor.gpu_pool_bytes() as f64 / 1048576.0,
        );
    }

    // -------------------------------------------------------------------------
    // TASK 6.3 (Part 2): Project-Switch & Close Resource Release Verification
    // -------------------------------------------------------------------------
    record_memory("before_project_close");
    let before_close_pool = studio.host.monitor.pool_cached_bytes();
    let before_close_gpu = studio.host.monitor.gpu_pool_bytes();
    let before_close_titles = studio.host.titles.live_bytes();
    let before_close_blocks = studio.title_blocks.len();
    let before_close_pcm = studio.host.playback.mapped_bytes();

    studio.close_project().unwrap();

    let after_close_pool = studio.host.monitor.pool_cached_bytes();
    let after_close_gpu = studio.host.monitor.gpu_pool_bytes();
    let after_close_titles = studio.host.titles.live_bytes();
    let after_close_blocks = studio.title_blocks.len();
    let after_close_pcm = studio.host.playback.mapped_bytes();
    record_memory("after_project_close");
    record(
        "lifecycle",
        serde_json::json!({
            "cpu_pool_bytes": [before_close_pool, after_close_pool],
            "gpu_pool_bytes": [before_close_gpu, after_close_gpu],
            "title_cache_bytes": [before_close_titles, after_close_titles],
            "title_blocks": [before_close_blocks, after_close_blocks],
            "mapped_pcm_bytes": [before_close_pcm, after_close_pcm],
        }),
    );

    println!(
        "[6.3 Lifecycle Close] Before close -> After close:\n  CPU Frame Pool: {:.2} MB -> {:.2} MB\n  GPU Layer Pool: {:.2} MB -> {:.2} MB\n  Live Titles Cache: {:.2} MB -> {:.2} MB\n  Title Blocks Map: {} -> {}\n  PCM Mapped Audio: {:.2} MB -> {:.2} MB",
        before_close_pool as f64 / 1048576.0,
        after_close_pool as f64 / 1048576.0,
        before_close_gpu as f64 / 1048576.0,
        after_close_gpu as f64 / 1048576.0,
        before_close_titles as f64 / 1048576.0,
        after_close_titles as f64 / 1048576.0,
        before_close_blocks,
        after_close_blocks,
        before_close_pcm as f64 / 1048576.0,
        after_close_pcm as f64 / 1048576.0,
    );

    assert_eq!(after_close_pool, 0, "CPU frame pool must be empty on close");
    assert_eq!(after_close_gpu, 0, "GPU layer pool must be shrunk on close");
    assert_eq!(
        after_close_titles, 0,
        "Live titles cache must be cleared on close"
    );
    assert_eq!(
        after_close_blocks, 0,
        "Studio title_blocks must be cleared on close"
    );
    assert_eq!(
        after_close_pcm, 0,
        "PCM memory-mapped audio cache must be cleared on close"
    );
    // Reopen the saved layered project repeatedly, exercise seek and edit history,
    // then observe counters after every close. Preserve raw RSS; allocator caching
    // and this short sample do not establish a leak or a memory plateau.
    let saved_4k = projects::open(root.join("Bench_6_2_4K").to_str().unwrap()).unwrap();
    for cycle in 0..6 {
        let started = Instant::now();
        studio.open_project(saved_4k.clone()).unwrap();
        for time in [0.0, 0.5, 1.0, 0.2] {
            studio.seek(time);
            let (clips, settings) = studio.preview_clips(960, 540, time).unwrap();
            let spec = concat_host::preview::FrameSpec {
                canvas: [0, 0, 0, 255],
                time,
                width: 960,
                height: 540,
                moving: false,
                proxy: false,
                color_space: Default::default(),
            };
            studio.host.monitor.frame(clips, &settings, spec).unwrap();
        }
        let clip_id = studio.timeline().clips[0].id.clone();
        let before_opacity = studio.clip(&clip_id).unwrap().opacity;
        studio.apply(Command::UpdateClip {
            clip_id: clip_id.clone(),
            patch: ClipPatch {
                opacity: Some(0.75),
                ..Default::default()
            },
        });
        assert_eq!(studio.clip(&clip_id).unwrap().opacity, 0.75);
        studio.undo();
        assert_eq!(studio.clip(&clip_id).unwrap().opacity, before_opacity);
        studio.redo();
        assert_eq!(studio.clip(&clip_id).unwrap().opacity, 0.75);
        studio.undo();
        assert_eq!(studio.clip(&clip_id).unwrap().opacity, before_opacity);
        studio.publish(&app, &models);
        studio.close_project().unwrap();
        record_samples(
            "layered_project_open_seek_edit_close",
            cycle,
            &[started.elapsed().as_secs_f64() * 1000.0],
        );
        record_memory(&format!("after_repeated_close_{}", cycle + 1));
        let counters = [
            studio.host.monitor.pool_cached_bytes() as u64,
            studio.host.monitor.gpu_pool_bytes(),
            studio.host.titles.live_bytes() as u64,
            studio.title_blocks.len() as u64,
            studio.host.playback.mapped_bytes() as u64,
        ];
        record(
            "lifecycle",
            serde_json::json!({"cycle": cycle + 1, "after_close_counters": counters}),
        );
        assert_eq!(
            counters, [0; 5],
            "owned caches must be empty after every close"
        );
    }
    record(
        "complete",
        serde_json::json!({"lifecycle_assertions": "passed"}),
    );
    // Match production shutdown: Slint/native worker TLS is process-lifetime.
    crate::leave(0);
}
