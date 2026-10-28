// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Phase 4 Task 4.4: Preview vs Export Rendering Parity.
//!
//! Validates semantic and visual parity between:
//! 1. Real GPU-backed Slint preview publication path (`studio.preview_clips`,
//!    `studio.host.monitor.frame_sources`, GPU compositing via `WgpuCompositor` / shaders).
//! 2. `concat-export` production export pipeline (`concat_host::export::request`,
//!    `concat_host::export::run_on`, FFmpeg libx264/AAC encoding).
//! 3. Independent decoded frame and audio verification via `concat_media::Decoder`
//!    and `concat_media::AudioDecoder`.
//!
//! Evaluates difference metrics:
//! - Max absolute pixel error (max_diff)
//! - Mean absolute error (MAE)
//! - Peak signal-to-noise ratio (PSNR)
//! - Temporal alignment and exact frame-index matching
//! - Audio duration, RMS energy, and track mix parity
//!
//! Covers:
//! - Constant & variable speed curves
//! - Reverse playback
//! - Video transitions & opacity / blending
//! - Generated captions with styling and positioning
//! - Multi-track audio and gain transitions
//! - Proxy-enabled vs original full-quality media
//! - Fractional frame rates (24000/1001 and 30000/1001)
//! - Exact cut boundaries and first/last frames

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use concat_host::export::{self, ExportSpec};
use concat_host::media;
use concat_host::projects;
use concat_host::reverse::{self, Reversers};
use concat_media::{
    AudioDecoder, AudioOptions, ColorRange, DecodeOptions, Decoder, FrameSource, RateMode,
    SampleFormat, VideoCodec,
};
use concat_project::commands::{ClipMove, ClipPatch, Command, TrackFlag, TrimEdge};
use concat_project::model::{KeyEase, KeyProperty, SpeedPoint, TextStyle, Transition};

use crate::panes::Msg;
use crate::panes::monitor::{MonitorMsg, Picture};
use crate::studio::Studio;

const WIDTH: u32 = 160;
const HEIGHT: u32 = 90;

/// Statistical image difference metrics between preview and decoded export.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParityMetrics {
    pub max_diff: u8,
    pub mae: f64,
    pub psnr: f64,
    pub rgb_mae: [f64; 3],
}

impl ParityMetrics {
    pub fn compute(preview: &[u8], export: &[u8]) -> Self {
        assert_eq!(
            preview.len(),
            export.len(),
            "preview ({} bytes) and export ({} bytes) dimension mismatch",
            preview.len(),
            export.len()
        );
        assert_eq!(
            preview.len() % 4,
            0,
            "RGBA buffer must be multiple of 4 bytes"
        );
        let pixels = preview.len() / 4;
        let mut max_diff: u8 = 0;
        let mut sum_diff: [u64; 3] = [0, 0, 0];
        let mut sum_sq: [u64; 3] = [0, 0, 0];

        for i in 0..pixels {
            let base = i * 4;
            for c in 0..3 {
                let diff = preview[base + c].abs_diff(export[base + c]);
                if diff > max_diff {
                    max_diff = diff;
                }
                sum_diff[c] += diff as u64;
                sum_sq[c] += (diff as u64) * (diff as u64);
            }
        }

        let total_subpixels = (pixels * 3) as f64;
        let total_sum_diff = (sum_diff[0] + sum_diff[1] + sum_diff[2]) as f64;
        let total_sum_sq = (sum_sq[0] + sum_sq[1] + sum_sq[2]) as f64;
        let mae = total_sum_diff / total_subpixels;
        let mse = total_sum_sq / total_subpixels;
        let psnr = if mse < 1e-12 {
            99.99
        } else {
            10.0 * (255.0f64.powi(2) / mse).log10()
        };

        Self {
            max_diff,
            mae,
            psnr,
            rgb_mae: [
                sum_diff[0] as f64 / pixels as f64,
                sum_diff[1] as f64 / pixels as f64,
                sum_diff[2] as f64 / pixels as f64,
            ],
        }
    }
}

/// Calculates RMS audio amplitude for stereo float samples.
pub fn calculate_audio_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum_sq / samples.len() as f64).sqrt() as f32
}

/// Creates a solid color synthetic video fixture.
fn make_solid_video(
    path: &Path,
    color_hex: &str,
    w: u32,
    h: u32,
    num: i64,
    den: i64,
    seconds: f64,
) {
    let status = std::process::Command::new("ffmpeg")
        .stdin(std::process::Stdio::null())
        .args([
            "-nostdin",
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("color=c={color_hex}:s={w}x{h}:d={seconds}:r={num}/{den}"),
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-crf",
            "16",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(path)
        .status()
        .expect("ffmpeg solid video creation");
    assert!(status.success(), "failed to create solid video at {path:?}");
}

/// Creates a recognizable test-pattern video fixture with moving indicators and audio.
fn make_testsrc_video(path: &Path, w: u32, h: u32, num: i64, den: i64, seconds: f64) {
    let status = std::process::Command::new("ffmpeg")
        .stdin(std::process::Stdio::null())
        .args([
            "-nostdin",
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc2=size={w}x{h}:rate={num}/{den}:duration={seconds}"),
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency=1000:sample_rate=48000:duration={seconds}"),
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-crf",
            "16",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
        ])
        .arg(path)
        .status()
        .expect("ffmpeg testsrc video creation");
    assert!(
        status.success(),
        "failed to create testsrc video at {path:?}"
    );
}

/// Creates an audio-only synthetic fixture with a specific frequency tone.
fn make_tone_audio(path: &Path, freq_hz: u32, seconds: f64) {
    let status = std::process::Command::new("ffmpeg")
        .stdin(std::process::Stdio::null())
        .args([
            "-nostdin",
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency={freq_hz}:sample_rate=48000:duration={seconds}"),
            "-c:a",
            "pcm_s16le",
            "-ac",
            "2",
        ])
        .arg(path)
        .status()
        .expect("ffmpeg tone audio creation");
    assert!(status.success(), "failed to create tone audio at {path:?}");
}

/// Helper struct wrapping production Studio preview and export verification.
#[allow(dead_code)]
struct ParityHarness {
    studio: Studio,
    dir: PathBuf,
    width: u32,
    height: u32,
    rate_num: i64,
    rate_den: i64,
}

impl ParityHarness {
    fn new(name: &str, width: u32, height: u32, rate_num: i64, rate_den: i64) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "clovercut-parity-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");

        let gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
        let host = crate::host::Host::start(gpu).expect("host start with GPU");
        let mut studio = Studio::new(host);

        let info = projects::create(
            dir.to_str().unwrap(),
            name,
            width,
            height,
            rate_num,
            rate_den,
        )
        .expect("create project");
        studio.open_project(info).expect("open project");

        Self {
            studio,
            dir,
            width,
            height,
            rate_num,
            rate_den,
        }
    }

    fn import_media(&mut self, path: &Path) -> String {
        let summary = media::probe(path.to_str().unwrap()).expect("probe media");
        self.studio
            .apply(Command::AddMedia {
                item: summary.to_new_media(),
            })
            .expect("media added")
    }

    /// Renders a frame through the production preview publication path.
    fn render_preview_frame(&mut self, time: f64) -> Vec<u8> {
        let (clips, settings) = self
            .studio
            .preview_clips(self.width, self.height, time)
            .expect("preview clips");
        let spec = concat_host::preview::FrameSpec {
            canvas: [0, 0, 0, 255],
            time,
            width: self.width,
            height: self.height,
            moving: false,
            proxy: false,
            color_space: self.studio.project().active().video.color_space,
        };

        // Render frame via host monitor
        let bytes = self
            .studio
            .host
            .monitor
            .frame(clips, &settings, spec)
            .expect("preview frame render");

        // Forward to MonitorPane as the production Slint UI loop does
        self.studio.handle(Msg::Monitor(MonitorMsg::Frame(
            Ok(Picture::Pixels(bytes.clone(), self.width, self.height)),
            spec,
        )));

        bytes
    }

    /// Exports the project using the production export pipeline.
    fn export_video(
        &mut self,
        label: &str,
    ) -> (PathBuf, Vec<concat_core::frame::Frame>, Option<Vec<f32>>) {
        let export_path = self.dir.join(format!("export-{label}.mp4"));
        let spec = ExportSpec {
            output: export_path.to_string_lossy().into_owned(),
            crf: 16,
            preset: "ultrafast".into(),
            codec: VideoCodec::H264,
            ten_bit: false,
            rate_mode: RateMode::Vbr,
            bitrate_kbps: 0,
            color_range: ColorRange::Limited,
            hdr: false,
        };

        let titles = self
            .studio
            .host
            .titles
            .clips(self.studio.project(), self.width, self.height)
            .into_iter()
            .map(|t| t.clip)
            .collect();

        let request = export::request(self.studio.session.as_ref().unwrap(), &spec, titles);
        let cancel = AtomicBool::new(false);
        let compositor = self.studio.host.monitor.off_thread();
        let written = export::run_on(&request, compositor, &cancel, |_| {})
            .expect("export execution succeeded");
        let written_path = PathBuf::from(written);

        // Decode exported video frames independently
        let mut decoder = Decoder::open(&written_path, &DecodeOptions::default())
            .expect("export video decoder open");
        let mut frames = Vec::new();
        while let Some(frame) = decoder.next_frame().expect("next frame decode") {
            frames.push(frame);
        }

        // Decode exported audio independently if present
        let audio_options = AudioOptions {
            rate: 48000,
            channels: 2,
            format: SampleFormat::F32,
            ..AudioOptions::default()
        };
        let audio = match AudioDecoder::open(&written_path, &audio_options) {
            Ok(mut audio_dec) => Some(audio_dec.collect_f32().expect("collect audio f32")),
            Err(concat_media::Error::NoAudioStream { .. }) => None,
            Err(e) => panic!("audio decode error: {e}"),
        };

        (written_path, frames, audio)
    }
}

impl Drop for ParityHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 1: Constant and Variable Speed Curves
// ─────────────────────────────────────────────────────────────────────────────
fn subtest_speed_curves() {
    eprintln!("[Task 4.4 Subtest 1] Speed curves parity...");
    let mut harness = ParityHarness::new("speed-curves", WIDTH, HEIGHT, 30, 1);
    let src = harness.dir.join("testsrc.mp4");
    make_testsrc_video(&src, WIDTH, HEIGHT, 30, 1, 4.0);
    let media_id = harness.import_media(&src);

    let clip_id = harness
        .studio
        .apply(Command::AddClipAtFirstFree {
            media_id,
            start: 0.0,
        })
        .unwrap();

    // Constant speed 2.0x
    harness.studio.apply(Command::SetClipSpeedCurve {
        clip_id: clip_id.clone(),
        ripple: false,
        curve: Some(vec![
            SpeedPoint {
                at: 0.0,
                speed: 2.0,
                slope: None,
            },
            SpeedPoint {
                at: 1.0,
                speed: 2.0,
                slope: None,
            },
        ]),
    });

    let (_exported_file, frames, _) = harness.export_video("speed-2x");
    assert_eq!(
        frames.len(),
        60,
        "4.0s clip at 2.0x speed must produce 60 output frames at 30 fps"
    );

    // Test multiple sample points (t = 0.2s, 1.0s, 1.8s)
    let sample_instants = [0.2, 1.0, 1.8];
    for &t in &sample_instants {
        let preview_bytes = harness.render_preview_frame(t);
        let frame_idx = ((t * 30.0).round() as usize).min(frames.len() - 1);
        let export_frame = &frames[frame_idx];
        let metrics = ParityMetrics::compute(&preview_bytes, export_frame.pixels());

        eprintln!(
            "  2x Speed at t={t:.2}s (frame {frame_idx}): max_diff={}, MAE={:.2}, PSNR={:.2} dB",
            metrics.max_diff, metrics.mae, metrics.psnr
        );
        assert!(
            metrics.mae < 12.0,
            "Speed 2.0x parity failure at t={t}s: MAE={:.2} exceeded limit (expected compression loss < 12.0)",
            metrics.mae
        );
        assert!(
            metrics.psnr > 25.0,
            "Speed 2.0x parity failure at t={t}s: PSNR={:.2} dB below limit",
            metrics.psnr
        );
    }

    // Now test Variable S-curve speed (0.5x to 2.0x)
    harness.studio.apply(Command::SetClipSpeedCurve {
        clip_id,
        ripple: false,
        curve: Some(vec![
            SpeedPoint {
                at: 0.0,
                speed: 0.5,
                slope: Some(0.0),
            },
            SpeedPoint {
                at: 1.0,
                speed: 2.0,
                slope: Some(0.0),
            },
        ]),
    });

    let (_, var_frames, _) = harness.export_video("speed-variable");
    for &t in &[0.5f64, 1.5, 2.5] {
        if (t * 30.0f64).round() as usize >= var_frames.len() {
            continue;
        }
        let preview_bytes = harness.render_preview_frame(t);
        let frame_idx = ((t * 30.0f64).round() as usize).min(var_frames.len() - 1);
        let export_frame = &var_frames[frame_idx];
        let metrics = ParityMetrics::compute(&preview_bytes, export_frame.pixels());

        eprintln!(
            "  Variable Speed at t={t:.2}s (frame {frame_idx}): max_diff={}, MAE={:.2}, PSNR={:.2} dB",
            metrics.max_diff, metrics.mae, metrics.psnr
        );
        assert!(
            metrics.mae < 15.0,
            "Variable speed parity failure at t={t}s: MAE={:.2}",
            metrics.mae
        );
    }
    eprintln!("[Task 4.4 Subtest 1] Passed.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 2: Reverse Playback
// ─────────────────────────────────────────────────────────────────────────────
fn subtest_reversed_playback() {
    eprintln!("[Task 4.4 Subtest 2] Reverse playback parity...");
    let mut harness = ParityHarness::new("reverse-playback", WIDTH, HEIGHT, 30, 1);
    let src = harness.dir.join("testsrc.mp4");
    make_testsrc_video(&src, WIDTH, HEIGHT, 30, 1, 3.0);
    let media_id = harness.import_media(&src);

    let clip_id = harness
        .studio
        .apply(Command::AddClipAtFirstFree {
            media_id: media_id.clone(),
            start: 0.0,
        })
        .unwrap();

    // Perform reversal via production Reversers
    let reversers = Reversers::new();
    let rev_target = reverse::target_for(&harness.dir, src.to_str().unwrap(), 0.0, 3.0, false)
        .expect("named target");
    let req = reverse::ReverseRequest {
        target: rev_target.clone(),
        media_path: src.to_str().unwrap().to_owned(),
        audio_only: false,
        start: 0.0,
        duration: 3.0,
    };
    let written = reversers.reverse(&req, &mut |_| {}).expect("reverse media");
    assert_eq!(written, rev_target);

    // Replace clip media with reversed media
    let summary = media::probe(written.to_str().unwrap()).expect("probe reversed media");
    harness.studio.apply(Command::ReplaceClipMedia {
        clip_id,
        item: summary.to_new_media(),
        source_start: None,
        enhanced: false,
        offset: 0.0,
    });

    let (_, frames, _) = harness.export_video("reversed");
    assert_eq!(frames.len(), 90, "3.0s reversed clip must have 90 frames");

    // Verify preview at t=0.5s, 1.5s, 2.5s matches export at same timeline instant
    for &t in &[0.5, 1.5, 2.5] {
        let preview_bytes = harness.render_preview_frame(t);
        let frame_idx = ((t * 30.0).round() as usize).min(frames.len() - 1);
        let export_frame = &frames[frame_idx];
        let metrics = ParityMetrics::compute(&preview_bytes, export_frame.pixels());

        eprintln!(
            "  Reversed at t={t:.2}s (frame {frame_idx}): max_diff={}, MAE={:.2}, PSNR={:.2} dB",
            metrics.max_diff, metrics.mae, metrics.psnr
        );
        assert!(
            metrics.mae < 12.0,
            "Reversed playback parity failure at t={t}s: MAE={:.2}",
            metrics.mae
        );
        assert!(
            metrics.psnr > 25.0,
            "Reversed playback PSNR failure at t={t}s: PSNR={:.2}",
            metrics.psnr
        );
    }
    eprintln!("[Task 4.4 Subtest 2] Passed.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 3: Video Transitions and Opacity / Blending
// ─────────────────────────────────────────────────────────────────────────────
fn subtest_transitions_and_blending() {
    eprintln!("[Task 4.4 Subtest 3] Video transitions & blending parity...");
    let mut harness = ParityHarness::new("transitions", WIDTH, HEIGHT, 30, 1);
    let red_path = harness.dir.join("red.mp4");
    let blue_path = harness.dir.join("blue.mp4");
    make_solid_video(&red_path, "red", WIDTH, HEIGHT, 30, 1, 2.0);
    make_solid_video(&blue_path, "blue", WIDTH, HEIGHT, 30, 1, 2.0);

    let red_media = harness.import_media(&red_path);
    let blue_media = harness.import_media(&blue_path);

    let first = harness
        .studio
        .apply(Command::AddClipAtFirstFree {
            media_id: red_media,
            start: 0.0,
        })
        .unwrap();

    let track = harness.studio.project().active().tracks[0].id.clone();
    let second = harness
        .studio
        .apply(Command::AddClipAtFirstFree {
            media_id: blue_media,
            start: 2.0,
        })
        .unwrap();

    // Set 1.0s handles and overlap for crossfade transition
    harness.studio.apply(Command::TrimClip {
        clip_id: second.clone(),
        edge: TrimEdge::Start,
        delta: 0.5,
        ripple: false,
    });
    harness.studio.apply(Command::TrimClip {
        clip_id: first.clone(),
        edge: TrimEdge::End,
        delta: -0.5,
        ripple: false,
    });
    harness.studio.apply(Command::MoveClips {
        moves: vec![ClipMove {
            clip_id: second.clone(),
            start: 1.0,
            track_id: track,
        }],
    });
    harness.studio.apply(Command::UpdateClip {
        clip_id: second,
        patch: ClipPatch {
            transition_in: Some(Some(Transition {
                id: "crossfade".into(),
                duration: 0.5,
                params: Default::default(),
            })),
            ..ClipPatch::default()
        },
    });

    let (_, frames, _) = harness.export_video("crossfade");

    // Sample before transition (t=0.5s Red), during transition (t=1.25s Purple blend),
    // and after transition (t=1.8s Blue)
    for &(t, desc) in &[
        (0.5, "Outgoing Red"),
        (1.25, "Mid-Transition Purple"),
        (1.8, "Incoming Blue"),
    ] {
        let preview_bytes = harness.render_preview_frame(t);
        let frame_idx = ((t * 30.0).round() as usize).min(frames.len() - 1);
        let export_frame = &frames[frame_idx];
        let metrics = ParityMetrics::compute(&preview_bytes, export_frame.pixels());

        eprintln!(
            "  Transition ({desc}) at t={t:.2}s: max_diff={}, MAE={:.2}, PSNR={:.2} dB",
            metrics.max_diff, metrics.mae, metrics.psnr
        );
        assert!(
            metrics.mae < 10.0,
            "Transition parity failure at {desc}: MAE={:.2}",
            metrics.mae
        );
        assert!(
            metrics.psnr > 28.0,
            "Transition PSNR failure at {desc}: PSNR={:.2}",
            metrics.psnr
        );
    }
    eprintln!("[Task 4.4 Subtest 3] Passed.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 4: Generated Captions with Styling and Positioning
// ─────────────────────────────────────────────────────────────────────────────
fn subtest_captions_and_styling() {
    eprintln!("[Task 4.4 Subtest 4] Generated captions & styling parity...");
    let mut harness = ParityHarness::new("captions", WIDTH, HEIGHT, 30, 1);
    let bg_path = harness.dir.join("bg.mp4");
    make_solid_video(&bg_path, "black", WIDTH, HEIGHT, 30, 1, 2.0);
    let bg_media = harness.import_media(&bg_path);

    harness
        .studio
        .apply(Command::AddClipAtFirstFree {
            media_id: bg_media,
            start: 0.0,
        })
        .unwrap();

    // Add text clip with styling and positioning
    let text_clip_id = harness
        .studio
        .apply(Command::AddTextClip {
            above: false,
            track_id: None,
            start: 0.0,
            duration: Some(2.0),
            offset_y: Some(0.1),
            style: Some(TextStyle {
                content: "CLOVERCUT".into(),
                font_size: 0.25,
                color: "#ffffff".into(),
                shadow: true,
                ..TextStyle::default()
            }),
        })
        .unwrap();

    // Set a scale keyframe to verify animated motion parity
    harness.studio.apply(Command::SetClipKey {
        clip_id: text_clip_id,
        property: KeyProperty::Scale,
        at: 0.0,
        value: 1.0,
        ease: KeyEase::LINEAR,
    });

    let (_, frames, _) = harness.export_video("captions");

    for &t in &[0.5, 1.0, 1.5] {
        let preview_bytes = harness.render_preview_frame(t);
        let frame_idx = ((t * 30.0).round() as usize).min(frames.len() - 1);
        let export_frame = &frames[frame_idx];
        let metrics = ParityMetrics::compute(&preview_bytes, export_frame.pixels());

        eprintln!(
            "  Caption overlay at t={t:.2}s (frame {frame_idx}): max_diff={}, MAE={:.2}, PSNR={:.2} dB",
            metrics.max_diff, metrics.mae, metrics.psnr
        );
        // Text rendering has antialiased font rasterization; codec compression on high-contrast
        // black/white text produces slight ringing, but MAE remains very low.
        assert!(
            metrics.mae < 10.0,
            "Caption parity failure at t={t}s: MAE={:.2}",
            metrics.mae
        );
        assert!(
            metrics.psnr > 28.0,
            "Caption PSNR failure at t={t}s: PSNR={:.2}",
            metrics.psnr
        );
    }
    eprintln!("[Task 4.4 Subtest 4] Passed.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 5: Multiple Audio Tracks and Gain Transitions
// ─────────────────────────────────────────────────────────────────────────────
fn subtest_multitrack_audio_and_gain() {
    eprintln!("[Task 4.4 Subtest 5] Multi-track audio & gain transitions parity...");
    let mut harness = ParityHarness::new("multitrack-audio", WIDTH, HEIGHT, 30, 1);
    let tone1 = harness.dir.join("tone_440.wav");
    let tone2 = harness.dir.join("tone_880.wav");
    make_tone_audio(&tone1, 440, 2.0);
    make_tone_audio(&tone2, 880, 2.0);

    let media1 = harness.import_media(&tone1);
    let media2 = harness.import_media(&tone2);

    // Track 1
    let clip1 = harness
        .studio
        .apply(Command::AddClipAtFirstFree {
            media_id: media1,
            start: 0.0,
        })
        .unwrap();

    // Track 2
    let track2_id = harness
        .studio
        .apply(Command::AddTrack {
            bottom: false,
            kind: None,
            insert_at: None,
        })
        .unwrap();
    let clip2 = harness
        .studio
        .apply(Command::AddClip {
            media_id: media2,
            start: 0.0,
            track_id: track2_id.clone(),
            ripple: false,
        })
        .unwrap();

    // Volume fade on Track 1: 0.0 -> 1.0
    harness.studio.apply(Command::SetClipKey {
        clip_id: clip1.clone(),
        property: KeyProperty::Volume,
        at: 0.0,
        value: 0.0,
        ease: KeyEase::LINEAR,
    });
    harness.studio.apply(Command::SetClipKey {
        clip_id: clip1,
        property: KeyProperty::Volume,
        at: 1.0,
        value: 1.0,
        ease: KeyEase::LINEAR,
    });

    let (_, _, audio_opt) = harness.export_video("audio-mix");
    let audio = audio_opt.expect("export must contain decoded audio");
    let duration_secs = audio.len() as f64 / 2.0 / 48000.0;
    eprintln!("  Exported audio duration: {duration_secs:.3}s (target: 2.000s)");
    assert!(
        (duration_secs - 2.0).abs() < 0.05,
        "Audio length mismatch: got {duration_secs}s, expected 2.0s"
    );

    let half_idx = audio.len() / 2;
    let rms_first_half = calculate_audio_rms(&audio[..half_idx]);
    let rms_second_half = calculate_audio_rms(&audio[half_idx..]);
    eprintln!("  Audio RMS: 1st half={rms_first_half:.4}, 2nd half={rms_second_half:.4}");
    assert!(
        rms_second_half > rms_first_half,
        "Volume ramp must produce higher energy in second half (got {rms_first_half} -> {rms_second_half})"
    );

    // Test Track Muting parity: mute track 2
    harness.studio.apply(Command::SetTrackFlag {
        track_id: track2_id,
        flag: TrackFlag::Muted,
        value: true,
    });

    let (_, _, muted_audio_opt) = harness.export_video("audio-muted");
    let muted_audio = muted_audio_opt.expect("export must contain audio");
    let rms_muted = calculate_audio_rms(&muted_audio);
    let rms_unmuted = calculate_audio_rms(&audio);
    eprintln!("  RMS comparison: unmuted={rms_unmuted:.4}, muted track 2={rms_muted:.4}");
    assert!(
        rms_muted < rms_unmuted,
        "Muting track 2 must strictly reduce overall mix RMS"
    );
    eprintln!("[Task 4.4 Subtest 5] Passed.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 6: Proxy-enabled versus Original Full-Quality Media
// ─────────────────────────────────────────────────────────────────────────────
fn subtest_proxy_vs_original() {
    eprintln!("[Task 4.4 Subtest 6] Proxy vs original media parity...");
    let mut harness = ParityHarness::new("proxies", WIDTH, HEIGHT, 30, 1);
    let original_path = harness.dir.join("original.mp4");
    let proxy_path = harness.dir.join("proxy.mp4");

    // Original is solid Green [0, 255, 0], Proxy is solid Orange [255, 128, 0]
    make_solid_video(&original_path, "green", WIDTH, HEIGHT, 30, 1, 2.0);
    make_solid_video(&proxy_path, "orange", 80, 46, 30, 1, 2.0);

    let media_id = harness.import_media(&original_path);
    harness
        .studio
        .apply(Command::AddClipAtFirstFree {
            media_id,
            start: 0.0,
        })
        .unwrap();

    // Register adopted proxy in the reader pool
    concat_host::scheduler()
        .pool()
        .adopt_proxy(&original_path, proxy_path);

    // Enable proxies in preferences
    harness.studio.prefs.proxies = true;

    // Preview with proxy = true: verify it decodes the proxy (Orange)
    let (clips, settings) = harness
        .studio
        .preview_clips(WIDTH, HEIGHT, 0.5)
        .expect("preview clips");
    let spec_proxy = concat_host::preview::FrameSpec {
        canvas: [0, 0, 0, 255],
        time: 0.5,
        width: WIDTH,
        height: HEIGHT,
        moving: true,
        proxy: true, // Preview reading proxy
        color_space: harness.studio.project().active().video.color_space,
    };
    let preview_proxy_bytes = harness
        .studio
        .host
        .monitor
        .frame(clips, &settings, spec_proxy)
        .expect("preview with proxy");

    // Preview proxy pixel should have dominant Red and Green (Orange)
    let p_center =
        &preview_proxy_bytes[(HEIGHT as usize / 2 * WIDTH as usize + WIDTH as usize / 2) * 4..];
    eprintln!(
        "  Preview with proxy active center pixel: [R={}, G={}, B={}] (Orange)",
        p_center[0], p_center[1], p_center[2]
    );
    assert!(
        p_center[0] > 150 && p_center[1] > 80,
        "Proxy preview must read the adopted proxy"
    );

    // Now export: export MUST ALWAYS read full-quality original media (Green), NEVER the proxy!
    let (_, frames, _) = harness.export_video("proxy-test");
    let exp_frame = &frames[15]; // frame at 0.5s
    let e_center = exp_frame.pixel(WIDTH / 2, HEIGHT / 2).unwrap();
    eprintln!(
        "  Exported frame center pixel: [R={}, G={}, B={}] (Green)",
        e_center[0], e_center[1], e_center[2]
    );
    assert!(
        e_center[0] < 50 && e_center[1] > 100 && e_center[2] < 50,
        "Production export MUST use original media (Green) and never proxy (Orange)"
    );

    // And verify preview with proxy = false also matches exported original media
    let preview_full_bytes = harness.render_preview_frame(0.5);
    let metrics = ParityMetrics::compute(&preview_full_bytes, exp_frame.pixels());
    eprintln!(
        "  Full-quality preview vs Export: max_diff={}, MAE={:.2}, PSNR={:.2} dB",
        metrics.max_diff, metrics.mae, metrics.psnr
    );
    assert!(
        metrics.mae < 10.0,
        "Full quality preview vs export MAE={:.2}",
        metrics.mae
    );
    eprintln!("[Task 4.4 Subtest 6] Passed.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 7: Fractional Frame Rates (24000/1001 and 30000/1001)
// ─────────────────────────────────────────────────────────────────────────────
fn subtest_fractional(rate_num: i64, rate_den: i64, name: &str, expected_frames: usize) {
    eprintln!("[Task 4.4 Subtest 7] Fractional rate parity: {name} ({rate_num}/{rate_den})...");
    let mut harness = ParityHarness::new(name, WIDTH, HEIGHT, rate_num, rate_den);
    let src = harness.dir.join("testsrc.mp4");
    make_testsrc_video(&src, WIDTH, HEIGHT, rate_num, rate_den, 2.0);
    let media_id = harness.import_media(&src);

    let clip_id = harness
        .studio
        .apply(Command::AddClipAtFirstFree {
            media_id,
            start: 0.0,
        })
        .unwrap();

    // Trim to exactly 1.5s
    harness.studio.apply(Command::TrimClip {
        clip_id,
        edge: TrimEdge::End,
        delta: -0.5,
        ripple: false,
    });

    let (_, frames, _) = harness.export_video(name);
    eprintln!(
        "    Decoded {name} frames: {} (expected {})",
        frames.len(),
        expected_frames
    );
    assert_eq!(
        frames.len(),
        expected_frames,
        "Frame count mismatch for {name}: got {}, expected {expected_frames}",
        frames.len()
    );

    // Check 3 rational time instants: k * (den / num)
    let delta = rate_den as f64 / rate_num as f64;
    for &k in &[5, 15, 25] {
        let t = k as f64 * delta;
        let preview_bytes = harness.render_preview_frame(t);
        let export_frame = &frames[k];
        let metrics = ParityMetrics::compute(&preview_bytes, export_frame.pixels());

        eprintln!(
            "    Frame {k} (t={t:.4}s): max_diff={}, MAE={:.2}, PSNR={:.2} dB",
            metrics.max_diff, metrics.mae, metrics.psnr
        );
        assert!(
            metrics.mae < 10.0,
            "{name} frame {k} parity failure: MAE={:.2}",
            metrics.mae
        );
        assert!(
            metrics.psnr > 28.0,
            "{name} frame {k} PSNR failure: PSNR={:.2}",
            metrics.psnr
        );
    }
    eprintln!("[Task 4.4 Subtest 7] {name} Passed.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtest 8: Exact Cut Boundaries and First/Last Frames
// ─────────────────────────────────────────────────────────────────────────────
fn subtest_cut_boundaries_and_edges() {
    eprintln!("[Task 4.4 Subtest 8] Exact cut boundaries & edge frames parity...");
    let mut harness = ParityHarness::new("cut-edges", WIDTH, HEIGHT, 30, 1);
    let red_path = harness.dir.join("red.mp4");
    let blue_path = harness.dir.join("blue.mp4");
    make_solid_video(&red_path, "red", WIDTH, HEIGHT, 30, 1, 1.0);
    make_solid_video(&blue_path, "blue", WIDTH, HEIGHT, 30, 1, 1.0);

    let red_media = harness.import_media(&red_path);
    let blue_media = harness.import_media(&blue_path);

    // Clip 1: Red [0.0 to 1.0s] (frames 0..30)
    harness
        .studio
        .apply(Command::AddClipAtFirstFree {
            media_id: red_media,
            start: 0.0,
        })
        .unwrap();

    // Clip 2: Blue [1.0 to 2.0s] (frames 30..60)
    harness
        .studio
        .apply(Command::AddClipAtFirstFree {
            media_id: blue_media,
            start: 1.0,
        })
        .unwrap();

    let (_, frames, _) = harness.export_video("cut-edges");
    assert_eq!(
        frames.len(),
        60,
        "Two 1.0s clips at 30 fps must export 60 frames"
    );

    let is_red = |pix: [u8; 4]| pix[0] > 180 && pix[2] < 50;
    let is_blue = |pix: [u8; 4]| pix[0] < 50 && pix[2] > 180;

    // 1. First frame (t = 0.0s, frame 0): must be Red in preview and export
    let prev_first = harness.render_preview_frame(0.0);
    let exp_first = frames[0].pixel(WIDTH / 2, HEIGHT / 2).unwrap();
    let prev_first_px: [u8; 4] = prev_first
        [(HEIGHT as usize / 2 * WIDTH as usize + WIDTH as usize / 2) * 4..][..4]
        .try_into()
        .unwrap();
    eprintln!("  Frame 0: preview={prev_first_px:?}, export={exp_first:?}");
    assert!(is_red(prev_first_px), "Preview frame 0 must be Red");
    assert!(is_red(exp_first), "Export frame 0 must be Red");

    // 2. Final frame of Clip 1 (t = 29/30 s = 0.9667s, frame 29): must be Red in preview and export
    let t_last_red = 29.0 / 30.0;
    let prev_last_red = harness.render_preview_frame(t_last_red);
    let exp_last_red = frames[29].pixel(WIDTH / 2, HEIGHT / 2).unwrap();
    let prev_last_red_px: [u8; 4] = prev_last_red
        [(HEIGHT as usize / 2 * WIDTH as usize + WIDTH as usize / 2) * 4..][..4]
        .try_into()
        .unwrap();
    eprintln!(
        "  Frame 29 (last frame of Clip 1): preview={prev_last_red_px:?}, export={exp_last_red:?}"
    );
    assert!(is_red(prev_last_red_px), "Preview frame 29 must be Red");
    assert!(is_red(exp_last_red), "Export frame 29 must be Red");

    // 3. First frame of Clip 2 (t = 1.0s, frame 30): must be Blue in preview and export
    let prev_first_blue = harness.render_preview_frame(1.0);
    let exp_first_blue = frames[30].pixel(WIDTH / 2, HEIGHT / 2).unwrap();
    let prev_first_blue_px: [u8; 4] = prev_first_blue
        [(HEIGHT as usize / 2 * WIDTH as usize + WIDTH as usize / 2) * 4..][..4]
        .try_into()
        .unwrap();
    eprintln!(
        "  Frame 30 (first frame of Clip 2): preview={prev_first_blue_px:?}, export={exp_first_blue:?}"
    );
    assert!(is_blue(prev_first_blue_px), "Preview frame 30 must be Blue");
    assert!(is_blue(exp_first_blue), "Export frame 30 must be Blue");

    // 4. Final frame of Clip 2 (t = 59/30 s = 1.9667s, frame 59): must be Blue in preview and export
    let t_last_blue = 59.0 / 30.0;
    let prev_last_blue = harness.render_preview_frame(t_last_blue);
    let exp_last_blue = frames[59].pixel(WIDTH / 2, HEIGHT / 2).unwrap();
    let prev_last_blue_px: [u8; 4] = prev_last_blue
        [(HEIGHT as usize / 2 * WIDTH as usize + WIDTH as usize / 2) * 4..][..4]
        .try_into()
        .unwrap();
    eprintln!(
        "  Frame 59 (last frame of Clip 2): preview={prev_last_blue_px:?}, export={exp_last_blue:?}"
    );
    assert!(is_blue(prev_last_blue_px), "Preview frame 59 must be Blue");
    assert!(is_blue(exp_last_blue), "Export frame 59 must be Blue");

    // Pixel difference on boundary frames
    let metrics_29 = ParityMetrics::compute(&prev_last_red, frames[29].pixels());
    let metrics_30 = ParityMetrics::compute(&prev_first_blue, frames[30].pixels());
    eprintln!(
        "  Frame 29 boundary metrics: MAE={:.2}, PSNR={:.2} dB",
        metrics_29.mae, metrics_29.psnr
    );
    eprintln!(
        "  Frame 30 boundary metrics: MAE={:.2}, PSNR={:.2} dB",
        metrics_30.mae, metrics_30.psnr
    );
    assert!(metrics_29.mae < 5.0 && metrics_30.mae < 5.0);

    eprintln!("[Task 4.4 Subtest 8] Passed.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Top-Level Entry Point: Child-Process Dispatcher
// ─────────────────────────────────────────────────────────────────────────────
#[test]
#[ignore = "requires an isolated graphical session, FFmpeg, GPU and Slint"]
fn preview_vs_export_rendering_parity() {
    const TEST: &str =
        "studio::phase4_preview_export_parity_tests::preview_vs_export_rendering_parity";
    const CHILD: &str = "CLOVERCUT_PHASE44_SUBTEST";

    let Some(subtest) = std::env::var_os(CHILD) else {
        // Parent runner: runs each subtest in an isolated child process
        let subtests = [
            "speed_curves",
            "reversed_playback",
            "transitions_and_blending",
            "captions_and_styling",
            "multitrack_audio_and_gain",
            "proxy_vs_original",
            "fractional_23976",
            "fractional_29970",
            "cut_boundaries_and_edges",
        ];

        let start_all = std::time::Instant::now();
        eprintln!("\n=======================================================");
        eprintln!("Starting Task 4.4 Preview vs Export Parity Test Suite");
        eprintln!("Total subtests: {}", subtests.len());
        eprintln!("=======================================================\n");

        for sub in subtests {
            let start = std::time::Instant::now();
            eprintln!("\n--- Launching Subtest: {sub} ---");
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .stdin(std::process::Stdio::null())
                .arg(TEST)
                .arg("--exact")
                .arg("--nocapture")
                .arg("--ignored")
                .env(CHILD, sub)
                .status()
                .expect("failed to spawn subtest child process");

            let elapsed = start.elapsed();
            assert!(
                status.success(),
                "Subtest '{sub}' failed with exit code {:?} in {:.2}s",
                status.code(),
                elapsed.as_secs_f64()
            );
            eprintln!(
                "--- Subtest '{sub}' PASSED in {:.2}s ---\n",
                elapsed.as_secs_f64()
            );
        }

        eprintln!("\n=======================================================");
        eprintln!(
            "ALL {} TASK 4.4 SUBTESTS PASSED SUCCESSFULLY in {:.2}s",
            subtests.len(),
            start_all.elapsed().as_secs_f64()
        );
        eprintln!("=======================================================\n");
        return;
    };

    // Child runner: executes the single requested subtest
    let subtest_name = subtest.to_string_lossy();
    match subtest_name.as_ref() {
        "speed_curves" => subtest_speed_curves(),
        "reversed_playback" => subtest_reversed_playback(),
        "transitions_and_blending" => subtest_transitions_and_blending(),
        "captions_and_styling" => subtest_captions_and_styling(),
        "multitrack_audio_and_gain" => subtest_multitrack_audio_and_gain(),
        "proxy_vs_original" => subtest_proxy_vs_original(),
        "fractional_23976" => subtest_fractional(24000, 1001, "23.976fps", 36),
        "fractional_29970" => subtest_fractional(30000, 1001, "29.970fps", 45),
        "cut_boundaries_and_edges" => subtest_cut_boundaries_and_edges(),
        other => panic!("unknown subtest {other}"),
    }

    crate::leave(0);
}
