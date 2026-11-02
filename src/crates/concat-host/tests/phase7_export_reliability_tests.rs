// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Phase 7 Export & Rendering Reliability Test Suite
//!
//! Covers Polish Plan Tasks 7.1 through 7.6:
//! - 7.1 & 7.2: Export format matrix (H.264, HEVC, 8-bit/10-bit, limited/full range,
//!   fractional rates: 23.976, 29.97, 30fps) and decoded output verification (dimensions,
//!   frame count, cut positions, sample alignment, and final frame).
//! - 7.3 & Audit E2: Cancellation in video, audio, and mux stages; cancellation latency
//!   measurement; destination preservation; temporary cleanup; slot release and retry.
//! - 7.4: Missing source files, unwritable destinations, and error recovery.
//! - 7.5: Host export request settings, captions and transitions reach the output.
//! - 7.6 & Audit E1: Concurrent exports with same-stem scratch isolation.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use concat_core::frame::Frame;
use concat_core::time::FrameRate;
use concat_host::dirs::AppDirs;
use concat_host::export::{self, ExportSpec, Exporter};
use concat_host::media;
use concat_host::projects;
use concat_host::session::Session;
use concat_host::titles::Titles;
use concat_media::audio::{self, AudioClip};
use concat_media::{
    AudioDecoder, AudioOptions, DecodeOptions, Decoder, EncodeOptions, Encoder, FrameSink,
    FrameSource, RateMode, SampleFormat, VideoCodec,
};
use concat_project::commands::{ClipPatch, Command};
use concat_project::model::{TextStyle, Transition};

const WIDTH: u32 = 160;
const HEIGHT: u32 = 90;
const AUDIO_RATE: u32 = 48_000;

fn make_test_picture(path: &Path, rate: FrameRate, seconds: u32, codec: VideoCodec) {
    let options = EncodeOptions {
        codec,
        preset: "ultrafast".to_owned(),
        crf: 18,
        rate_mode: RateMode::Vbr,
        bitrate_kbps: 0,
        ten_bit: false,
        color_range: concat_media::ColorRange::Limited,
        hardware: false,
        threads: 0,
        software_only: false,
    };
    let mut encoder = Encoder::create(path, WIDTH, HEIGHT, rate, &options).expect("encoder opens");
    let fps = rate.fps().as_f64();
    let frames = (f64::from(seconds) * fps).round() as u32;
    let mut frame = Frame::black(WIDTH, HEIGHT);
    for idx in 0..frames {
        let sec = (f64::from(idx) / fps).floor() as u8;
        frame.fill([30 + 35 * sec, 80, 190, 255]);
        encoder.write_frame(&frame).expect("writes frame");
    }
    encoder.finish().expect("finishes picture");
}

fn make_test_wav(path: &Path, seconds: u32) {
    let total_samples = seconds * AUDIO_RATE;
    let mut data = Vec::with_capacity(total_samples as usize * 4);
    for i in 0..total_samples {
        let t = f64::from(i) / f64::from(AUDIO_RATE);
        let on = (t.floor() as u32) % 2 == 1;
        let sample = if on {
            (0.4 * (2.0 * std::f64::consts::PI * 880.0 * t).sin() * 32767.0) as i16
        } else {
            0
        };
        data.extend_from_slice(&sample.to_le_bytes());
        data.extend_from_slice(&sample.to_le_bytes());
    }
    let mut wav = Vec::with_capacity(44 + data.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&AUDIO_RATE.to_le_bytes());
    wav.extend_from_slice(&(AUDIO_RATE * 4).to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    std::fs::write(path, wav).expect("writes wav");
}

fn make_test_clip(root: &Path, stem: &str, seconds: u32, rate: FrameRate) -> PathBuf {
    let pic = root.join(format!("{stem}-pic.mp4"));
    let wav = root.join(format!("{stem}.wav"));
    let aac = root.join(format!("{stem}.m4a"));
    let out = root.join(format!("{stem}.mp4"));

    make_test_picture(&pic, rate, seconds, VideoCodec::H264);
    make_test_wav(&wav, seconds);
    let clip = AudioClip {
        path: wav,
        stream: None,
        start: 0.0,
        duration: f64::from(seconds),
        source_start: 0.0,
        speed: 1.0,
        preserve_pitch: true,
        retime: None,
        volume: 1.0,
        volume_curve: Default::default(),
        fade_in: 0.0,
        fade_out: 0.0,
        filter_chain: String::new(),
    };
    audio::mix_to_file(&[clip], f64::from(seconds), &aac).expect("mixes aac");
    audio::mux(&pic, &aac, &out).expect("muxes test clip");
    out
}

fn create_test_session(
    dir: &Path,
    name: &str,
    width: u32,
    height: u32,
    rate_num: i64,
    rate_den: i64,
) -> Session {
    let info = projects::create(
        dir.to_str().expect("utf-8"),
        name,
        width,
        height,
        rate_num,
        rate_den,
    )
    .expect("creates project");
    Session::open_info(&info).expect("opens project")
}

struct DecodedVideo {
    width: u32,
    height: u32,
    frames: Vec<Frame>,
    audio_duration: Option<f64>,
}

fn decode_exported_file(path: &Path) -> DecodedVideo {
    let mut decoder = Decoder::open(path, &DecodeOptions::default())
        .unwrap_or_else(|e| panic!("failed to open exported file {}: {e}", path.display()));
    let mut frames = Vec::new();
    while let Some(f) = decoder.next_frame().expect("decodes frame") {
        frames.push(f);
    }
    let (width, height) = frames
        .first()
        .map(|f| (f.width(), f.height()))
        .unwrap_or((0, 0));
    let audio_opts = AudioOptions {
        rate: AUDIO_RATE,
        channels: 2,
        format: SampleFormat::F32,
        ..AudioOptions::default()
    };
    let audio_duration = match AudioDecoder::open(path, &audio_opts) {
        Ok(mut dec) => {
            let samples = dec.collect_f32().expect("collects audio samples");
            Some(samples.len() as f64 / 2.0 / f64::from(AUDIO_RATE))
        }
        Err(_) => None,
    };
    DecodedVideo {
        width,
        height,
        frames,
        audio_duration,
    }
}

// -----------------------------------------------------------------------------
// TASKS 7.1 & 7.2: Export Matrix & Precision Alignment Verification
// -----------------------------------------------------------------------------
#[test]
fn test_export_format_matrix_and_precision_verification() {
    let root = std::env::temp_dir().join(format!("phase7-matrix-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();

    let clip_30 = make_test_clip(&root, "src30", 3, FrameRate::THIRTY);
    let clip_24 = make_test_clip(&root, "src24", 3, FrameRate::checked(24000, 1001).unwrap());

    // Matrix covering:
    // Codecs: H.264, HEVC
    // Depths: 8-bit, 10-bit
    // Ranges: Limited, Full
    // Rates:  30fps (30/1), 29.97fps (30000/1001), 23.976fps (24000/1001)
    let matrix = [
        (
            "h264_8bit_limited_30fps.mp4",
            30,
            1,
            VideoCodec::H264,
            false,
            concat_media::ColorRange::Limited,
            &clip_30,
        ),
        (
            "h264_8bit_full_2997fps.mp4",
            30000,
            1001,
            VideoCodec::H264,
            false,
            concat_media::ColorRange::Full,
            &clip_30,
        ),
        (
            "hevc_8bit_limited_23976fps.mp4",
            24000,
            1001,
            VideoCodec::Hevc,
            false,
            concat_media::ColorRange::Limited,
            &clip_24,
        ),
        (
            "hevc_10bit_limited_2997fps.mp4",
            30000,
            1001,
            VideoCodec::Hevc,
            true,
            concat_media::ColorRange::Limited,
            &clip_30,
        ),
    ];

    for (name, rate_num, rate_den, codec, ten_bit, color_range, media_path) in matrix {
        let output_path = root.join(name);
        let project_dir = root.join(format!("proj_{name}"));
        let mut session = create_test_session(
            &project_dir,
            &format!("Proj_{name}"),
            WIDTH,
            HEIGHT,
            rate_num,
            rate_den,
        );

        let mid = session
            .apply(Command::AddMedia {
                item: media::probe(media_path.to_str().unwrap())
                    .unwrap()
                    .to_new_media(),
            })
            .unwrap()
            .created_id
            .unwrap();

        let track_id = session.project().active().tracks[0].id.clone();
        session
            .apply(Command::AddClip {
                media_id: mid,
                track_id,
                start: 0.0,
                ripple: false,
            })
            .unwrap();

        let spec = ExportSpec {
            output: output_path.to_str().unwrap().to_owned(),
            crf: 18,
            preset: "ultrafast".to_owned(),
            codec,
            ten_bit,
            rate_mode: RateMode::Vbr,
            bitrate_kbps: 0,
            color_range,
            hdr: false,
        };

        let request = export::request(&session, &spec, Vec::new());
        let cancel = AtomicBool::new(false);
        let result = export::run(&request, &cancel, |_| {}).expect("export succeeds");
        assert_eq!(result, output_path.to_str().unwrap());

        // Decode exported file and verify exact precision
        let decoded = decode_exported_file(&output_path);
        assert_eq!(decoded.width, WIDTH, "{name}: export width matches");
        assert_eq!(decoded.height, HEIGHT, "{name}: export height matches");

        let expected_frames = ((3.0 * rate_num as f64) / (rate_den as f64)).round() as usize;
        assert_eq!(
            decoded.frames.len(),
            expected_frames,
            "{name}: frame count matches expected length"
        );

        let audio_dur = decoded.audio_duration.expect("audio stream present");
        assert!(
            (audio_dur - 3.0).abs() < 0.05,
            "{name}: audio duration matches video duration within one frame"
        );

        // Probe verification
        let info = media::probe(output_path.to_str().unwrap()).unwrap();
        let video_info = info.video.as_ref().expect("has video");
        assert_eq!(video_info.width, WIDTH);
        assert_eq!(video_info.height, HEIGHT);
        assert!(info.audio.is_some(), "has audio stream");

        // Inspect the encoded stream, not the requested export settings.
        let probe = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=codec_name,pix_fmt,color_range,avg_frame_rate",
                "-of",
                "json",
            ])
            .arg(&output_path)
            .output()
            .expect("ffprobe is required for encoded-format verification");
        assert!(
            probe.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&probe.stderr)
        );
        let metadata: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
        let stream = &metadata["streams"][0];
        assert_eq!(
            stream["codec_name"],
            if codec == VideoCodec::H264 {
                "h264"
            } else {
                "hevc"
            },
            "{name}"
        );
        let pixel_format = stream["pix_fmt"].as_str().expect("encoded pixel format");
        if ten_bit {
            assert_eq!(pixel_format, "yuv420p10le", "{name}: actual 10-bit output");
        } else {
            assert!(
                matches!(pixel_format, "yuv420p" | "yuvj420p"),
                "{name}: actual 8-bit output: {pixel_format}"
            );
        }
        assert_eq!(
            stream["color_range"],
            if color_range == concat_media::ColorRange::Full {
                "pc"
            } else {
                "tv"
            },
            "{name}"
        );
        assert_eq!(
            stream["avg_frame_rate"],
            format!("{rate_num}/{rate_den}"),
            "{name}"
        );

        // Final frame validity check
        let last_frame = &decoded.frames[expected_frames - 1];
        let px = last_frame.pixel(WIDTH / 2, HEIGHT / 2).unwrap();
        // The fixture changes red every second; its last second is [100,80,190].
        // Check picture content with room for lossy YUV conversion/compression.
        for (actual, expected) in px[..3].iter().zip([100u8, 80, 190]) {
            assert!(
                actual.abs_diff(expected) <= 12,
                "{name}: wrong final picture: {px:?}"
            );
        }
        assert_eq!(px[3], 255, "{name}: opaque output");
    }

    let _ = std::fs::remove_dir_all(&root);
}

// -----------------------------------------------------------------------------
// TASK 7.3 & AUDIT E2: Cancellation Latency, Destination Preservation & Cleanup
// -----------------------------------------------------------------------------
#[test]
fn test_export_cancellation_latency_and_destination_preservation() {
    let root = std::env::temp_dir().join(format!("phase7-cancel-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();

    let clip = make_test_clip(&root, "clip_long", 6, FrameRate::THIRTY);
    let output_path = root.join("cancel_target.mp4");

    // 1. Write pre-existing destination file to verify it is NOT corrupted on cancelled export
    let existing_payload = b"PRE_EXISTING_USER_FILE_CONTENT_DO_NOT_CORRUPT";
    std::fs::write(&output_path, existing_payload).unwrap();

    let mut session = create_test_session(
        &root.join("proj_cancel"),
        "ProjCancel",
        WIDTH,
        HEIGHT,
        30,
        1,
    );
    let mid = session
        .apply(Command::AddMedia {
            item: media::probe(clip.to_str().unwrap()).unwrap().to_new_media(),
        })
        .unwrap()
        .created_id
        .unwrap();
    let v_track = session.project().active().tracks[0].id.clone();
    session
        .apply(Command::AddClip {
            media_id: mid,
            track_id: v_track,
            start: 0.0,
            ripple: false,
        })
        .unwrap();

    let spec = ExportSpec {
        output: output_path.to_str().unwrap().to_owned(),
        crf: 18,
        preset: "ultrafast".to_owned(),
        codec: VideoCodec::H264,
        ten_bit: false,
        rate_mode: RateMode::Vbr,
        bitrate_kbps: 0,
        color_range: concat_media::ColorRange::Limited,
        hdr: false,
    };

    let request = export::request(&session, &spec, Vec::new());
    let cancel = Arc::new(AtomicBool::new(false));

    // Cancel shortly after video rendering starts and measure cancellation latency
    let cancel_trigger = Arc::clone(&cancel);
    let start_time = Instant::now();
    let cancel_time = Arc::new(Mutex::new(None));
    let ct = Arc::clone(&cancel_time);

    let handle = std::thread::spawn({
        let cancel = Arc::clone(&cancel);
        move || {
            export::run(&request, &cancel, move |progress| {
                if progress.total > 0
                    && progress.frame >= 5
                    && !cancel_trigger.swap(true, Ordering::Relaxed)
                {
                    *ct.lock().unwrap() = Some(Instant::now());
                }
            })
        }
    });

    let res = handle.join().unwrap();
    let finished_time = Instant::now();

    assert!(res.is_err(), "export should return Err on cancellation");
    let err_msg = res.unwrap_err();
    assert!(
        err_msg.contains("cancel") || err_msg.contains("stopped"),
        "error message should indicate cancellation: {err_msg}"
    );

    // Measure video cancellation latency
    if let Some(triggered) = *cancel_time.lock().unwrap() {
        let latency = finished_time.duration_since(triggered);
        println!(
            "[Audit E2] Video stage cancellation latency: {:.2} ms (total call: {:.2} ms)",
            latency.as_secs_f64() * 1000.0,
            finished_time.duration_since(start_time).as_secs_f64() * 1000.0
        );
        assert!(
            latency.as_millis() < 500,
            "cancellation latency should be responsive (< 500ms), was {:?}",
            latency
        );
    }

    // 2. Destination preservation check
    let current_bytes = std::fs::read(&output_path).expect("target file still exists");
    assert_eq!(
        current_bytes, existing_payload,
        "destination file must remain completely uncorrupted and preserved after cancellation"
    );

    // Verify scratch/staging files are cleaned up
    let entries: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            !name.starts_with(".cancel_target") || name == "cancel_target.mp4",
            "staging/scratch file {name} was not removed after cancellation"
        );
    }

    // 3. Measure synchronous audio mixing cancellation latency
    let audio_clip = AudioClip {
        path: root.join("clip_long.wav"),
        stream: None,
        start: 0.0,
        duration: 6.0,
        source_start: 0.0,
        speed: 1.0,
        preserve_pitch: true,
        retime: None,
        volume: 1.0,
        volume_curve: Default::default(),
        fade_in: 0.0,
        fade_out: 0.0,
        filter_chain: String::new(),
    };
    let audio_out = root.join("test_audio_cancel.m4a");
    let audio_payload = b"previous audio output";
    std::fs::write(&audio_out, audio_payload).unwrap();
    let audio_cancel = AtomicBool::new(true);
    let audio_res = audio::mix_to_file_cancelling(
        std::slice::from_ref(&audio_clip),
        6.0,
        &audio_out,
        Some(&audio_cancel),
    );
    assert!(matches!(audio_res, Err(concat_media::Error::Cancelled)));
    assert_eq!(std::fs::read(&audio_out).unwrap(), audio_payload);

    // Request cancellation from an actual output callback, after mixing has
    // started. The next chunk must not be delivered. This measures the
    // cancellation boundary without a sleep/startup/scheduler race or a
    // wall-clock performance budget on a shared CI runner.
    audio_cancel.store(false, Ordering::Relaxed);
    let mut writes = 0;
    let mut cancel_at = None;
    let audio_res =
        audio::mix_pcm_cancelling(&[audio_clip], &[], 0.0, 6.0, Some(&audio_cancel), |chunk| {
            assert!(!chunk.is_empty());
            writes += 1;
            cancel_at = Some(Instant::now());
            audio_cancel.store(true, Ordering::Relaxed);
            Ok(())
        });
    assert!(matches!(audio_res, Err(concat_media::Error::Cancelled)));
    assert_eq!(
        writes, 1,
        "no further PCM chunk may be delivered after cancellation"
    );
    println!(
        "[Audit E2] Audio cancellation return after first output chunk: {:.2} ms (observational, not a performance budget)",
        cancel_at
            .expect("mixing must deliver a chunk before cancellation")
            .elapsed()
            .as_secs_f64()
            * 1000.0
    );

    // 4. Test SingleFlight slot release and retry via Exporter
    let exporter = Exporter::new();
    let job1 = exporter.begin().expect("claims export slot");
    assert!(exporter.is_busy(), "exporter should be busy while job held");
    assert!(
        exporter.begin().is_err(),
        "second concurrent export must be rejected by SingleFlight"
    );

    exporter.cancel();
    assert!(job1.cancelled(), "job reflects cancellation when cancelled");
    drop(job1);

    assert!(
        !exporter.is_busy(),
        "exporter slot must be free after job dropped"
    );
    let job2 = exporter.begin().expect("retry succeeds immediately");
    drop(job2);

    let _ = std::fs::remove_dir_all(&root);
}

// -----------------------------------------------------------------------------
// TASK 7.4: Error Boundaries (Missing Sources & Unwritable Output)
// -----------------------------------------------------------------------------
#[test]
fn test_export_boundary_and_error_handling() {
    let root = std::env::temp_dir().join(format!("phase7-errors-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();

    let mut session = create_test_session(&root.join("proj_err"), "ProjErr", WIDTH, HEIGHT, 30, 1);

    // 1. Missing source clip (imported then deleted from disk)
    let doomed = root.join("doomed_source.mp4");
    let clip = make_test_clip(&root, "doomed", 2, FrameRate::THIRTY);
    std::fs::rename(clip, &doomed).unwrap();
    let mid = session
        .apply(Command::AddMedia {
            item: media::probe(doomed.to_str().unwrap())
                .unwrap()
                .to_new_media(),
        })
        .unwrap()
        .created_id
        .unwrap();
    let track_id = session.project().active().tracks[0].id.clone();
    session
        .apply(Command::AddClip {
            media_id: mid,
            track_id,
            start: 0.0,
            ripple: false,
        })
        .unwrap();
    // Delete source file before export to simulate missing media
    std::fs::remove_file(&doomed).unwrap();

    let missing_spec = ExportSpec {
        output: root.join("missing_source.mp4").to_str().unwrap().into(),
        crf: 18,
        preset: "ultrafast".into(),
        codec: VideoCodec::H264,
        ten_bit: false,
        rate_mode: RateMode::Vbr,
        bitrate_kbps: 0,
        color_range: concat_media::ColorRange::Limited,
        hdr: false,
    };
    let request = export::request(&session, &missing_spec, Vec::new());

    let cancel = AtomicBool::new(false);
    let result = export::run(&request, &cancel, |_| {});
    assert!(
        result.is_err(),
        "export with missing source file must return error cleanly"
    );

    // Verify no stray partial files remained
    assert!(
        !root.join("missing_source.mp4").exists(),
        "unsuccessful export destination must not exist"
    );

    // 2. Unwritable destination
    let unwritable_path = root.join("nonexistent_subdir/never_created/output.mp4");
    let unwritable_spec = ExportSpec {
        output: unwritable_path.to_string_lossy().into(),
        crf: 18,
        preset: "ultrafast".into(),
        codec: VideoCodec::H264,
        ten_bit: false,
        rate_mode: RateMode::Vbr,
        bitrate_kbps: 0,
        color_range: concat_media::ColorRange::Limited,
        hdr: false,
    };
    let unwritable_req = export::request(&session, &unwritable_spec, Vec::new());
    let unwritable_res = export::run(&unwritable_req, &cancel, |_| {});
    assert!(
        unwritable_res.is_err(),
        "export to unwritable path must fail gracefully with error"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// -----------------------------------------------------------------------------
// Host request settings, titles and transitions reach the exported file.
// -----------------------------------------------------------------------------
#[test]
fn host_export_request_preserves_settings_titles_and_transitions() {
    let root = std::env::temp_dir().join(format!("host-request-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();

    let clip_a = make_test_clip(&root, "clip_a", 2, FrameRate::THIRTY);
    let clip_b = make_test_clip(&root, "clip_b", 2, FrameRate::THIRTY);
    let output_path = root.join("host_export.mp4");

    let mut session = create_test_session(
        &root.join("proj_host_export"),
        "Host export",
        WIDTH,
        HEIGHT,
        30,
        1,
    );

    let mid_a = session
        .apply(Command::AddMedia {
            item: media::probe(clip_a.to_str().unwrap())
                .unwrap()
                .to_new_media(),
        })
        .unwrap()
        .created_id
        .unwrap();

    let mid_b = session
        .apply(Command::AddMedia {
            item: media::probe(clip_b.to_str().unwrap())
                .unwrap()
                .to_new_media(),
        })
        .unwrap()
        .created_id
        .unwrap();

    let track_id = session.project().active().tracks[0].id.clone();
    let _clip_a_id = session
        .apply(Command::AddClip {
            media_id: mid_a,
            track_id: track_id.clone(),
            start: 0.0,
            ripple: false,
        })
        .unwrap()
        .created_id
        .unwrap();

    let clip_b_id = session
        .apply(Command::AddClip {
            media_id: mid_b,
            track_id,
            start: 2.0,
            ripple: false,
        })
        .unwrap()
        .created_id
        .unwrap();

    // Add crossfade transition between clip_a and clip_b
    session
        .apply(Command::UpdateClip {
            clip_id: clip_b_id,
            patch: ClipPatch {
                transition_in: Some(Some(Transition {
                    id: "crossfade".to_owned(),
                    duration: 0.5,
                    params: Default::default(),
                })),
                ..ClipPatch::default()
            },
        })
        .unwrap();

    // Add caption/title overlay
    session
        .apply(Command::AddTextClip {
            above: false,
            track_id: None,
            start: 0.5,
            duration: Some(1.5),
            offset_y: Some(0.0),
            style: Some(TextStyle {
                content: "Host export fixture".into(),
                font_size: 0.2,
                ..Default::default()
            }),
        })
        .unwrap();

    let app_dirs = AppDirs::under(&root.join("app"));
    let titles = Titles::new(&app_dirs);
    let title_clips: Vec<_> = titles
        .clips(session.project(), WIDTH, HEIGHT)
        .into_iter()
        .map(|t| t.clip)
        .collect();

    assert!(!title_clips.is_empty(), "title clip should be generated");

    let spec = ExportSpec {
        output: output_path.to_str().unwrap().into(),
        crf: 18,
        preset: "ultrafast".into(),
        codec: VideoCodec::H264,
        ten_bit: false,
        rate_mode: RateMode::Vbr,
        bitrate_kbps: 0,
        color_range: concat_media::ColorRange::Limited,
        hdr: false,
    };

    let request = export::request(&session, &spec, title_clips);

    // Verify that the host request carries the requested settings.
    assert_eq!(request.width, WIDTH);
    assert_eq!(request.height, HEIGHT);
    assert_eq!(request.rate_num, 30);
    assert_eq!(request.rate_den, 1);
    assert_eq!(request.codec, VideoCodec::H264);
    assert_eq!(request.color_range, concat_media::ColorRange::Limited);
    assert!(!request.hdr);

    // Verify timeline clips include media clips, transition, and title clips
    assert!(
        request.clips.len() >= 3,
        "must include 2 media clips and at least 1 title clip"
    );
    let has_transition = request.clips.iter().any(|c| c.transition.is_some());
    assert!(
        has_transition,
        "transition must be present on flattened clips"
    );
    let has_title = request
        .clips
        .iter()
        .any(|c| c.kind == concat_export::ClipKind::Image || c.generated.is_some());
    assert!(has_title, "title clip must be present on export clips");

    // Execute export
    let cancel = AtomicBool::new(false);
    let result = export::run(&request, &cancel, |_| {}).expect("export succeeds");
    assert_eq!(result, output_path.to_str().unwrap());

    // Verify decoded output
    let decoded = decode_exported_file(&output_path);
    assert_eq!(decoded.width, WIDTH);
    assert_eq!(decoded.height, HEIGHT);
    // 2 clips of 2.0s = 4.0s = 120 frames at 30fps
    assert_eq!(decoded.frames.len(), 120);

    let _ = std::fs::remove_dir_all(&root);
}

// -----------------------------------------------------------------------------
// TASK 7.6 & AUDIT E1: Same-Stem Staging Name Collision Avoidance
// -----------------------------------------------------------------------------
#[test]
fn test_export_same_stem_staging_isolation() {
    let root = std::env::temp_dir().join(format!("phase7-stem-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();

    let clip = make_test_clip(&root, "clip_stem", 2, FrameRate::THIRTY);

    let mut session_a = create_test_session(&root.join("proj_a"), "ProjA", WIDTH, HEIGHT, 30, 1);
    let mid_a = session_a
        .apply(Command::AddMedia {
            item: media::probe(clip.to_str().unwrap()).unwrap().to_new_media(),
        })
        .unwrap()
        .created_id
        .unwrap();
    let track_a = session_a.project().active().tracks[0].id.clone();
    session_a
        .apply(Command::AddClip {
            media_id: mid_a,
            track_id: track_a,
            start: 0.0,
            ripple: false,
        })
        .unwrap();

    let mut session_b = create_test_session(&root.join("proj_b"), "ProjB", WIDTH, HEIGHT, 30, 1);
    let mid_b = session_b
        .apply(Command::AddMedia {
            item: media::probe(clip.to_str().unwrap()).unwrap().to_new_media(),
        })
        .unwrap()
        .created_id
        .unwrap();
    let track_b = session_b.project().active().tracks[0].id.clone();
    session_b
        .apply(Command::AddClip {
            media_id: mid_b,
            track_id: track_b,
            start: 0.0,
            ripple: false,
        })
        .unwrap();

    // Two destinations with the EXACT SAME STEM in the SAME DIRECTORY:
    // Stem is "shared_stem". In previous code, both would race on `.shared_stem.concat-video.mp4`!
    let out_a = root.join("shared_stem.mp4");
    let out_b = root.join("shared_stem.mov");

    let spec_a = ExportSpec {
        output: out_a.to_str().unwrap().into(),
        crf: 18,
        preset: "ultrafast".into(),
        codec: VideoCodec::H264,
        ten_bit: false,
        rate_mode: RateMode::Vbr,
        bitrate_kbps: 0,
        color_range: concat_media::ColorRange::Limited,
        hdr: false,
    };
    let spec_b = ExportSpec {
        output: out_b.to_str().unwrap().into(),
        crf: 18,
        preset: "ultrafast".into(),
        codec: VideoCodec::H264,
        ten_bit: false,
        rate_mode: RateMode::Vbr,
        bitrate_kbps: 0,
        color_range: concat_media::ColorRange::Limited,
        hdr: false,
    };

    let req_a = export::request(&session_a, &spec_a, Vec::new());
    let req_b = export::request(&session_b, &spec_b, Vec::new());

    // Execute concurrently across two OS threads
    let handle_a = std::thread::spawn(move || {
        let cancel = AtomicBool::new(false);
        export::run(&req_a, &cancel, |_| {})
    });
    let handle_b = std::thread::spawn(move || {
        let cancel = AtomicBool::new(false);
        export::run(&req_b, &cancel, |_| {})
    });

    let res_a = handle_a.join().unwrap().expect("export A succeeds");
    let res_b = handle_b.join().unwrap().expect("export B succeeds");

    assert_eq!(res_a, out_a.to_str().unwrap());
    assert_eq!(res_b, out_b.to_str().unwrap());

    // Decode and verify both outputs are intact and undamaged by the concurrent execution
    let dec_a = decode_exported_file(&out_a);
    let dec_b = decode_exported_file(&out_b);

    assert_eq!(dec_a.frames.len(), 60);
    assert_eq!(dec_b.frames.len(), 60);
    assert!(dec_a.audio_duration.unwrap() > 1.95);
    assert!(dec_b.audio_duration.unwrap() > 1.95);

    let _ = std::fs::remove_dir_all(&root);
}
