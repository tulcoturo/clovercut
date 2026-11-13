// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Enhance: an enhanced copy of a media file, written once.
//!
//! Restoration is not real time - a frame takes the model a good part of
//! a second on an accelerator and many seconds without one - so a clip is
//! not enhanced as it plays. Instead the job here reads the file frame by
//! frame, runs each through [`concat_vision::enhance`], and writes the
//! result into the project's cache as a file of its own: a PNG for a
//! still, an MP4 with the original's sound for footage. The window then
//! points the clip at the copy, the way a freeze frame points at its
//! still, and the monitor and the export read it like any other file.
//!
//! One job at a time through a [`SingleFlight`], like every long job the
//! host runs, with the model fetched on first use and kept loaded. The
//! copy is named after the file's path, size and modification time, the
//! factor it was enlarged by and the strength it was blended at, so a file
//! replaced on disk gets a fresh one, the same file asked for twice gets
//! the one already there, and asking again with other settings writes a
//! copy of its own beside the first rather than over it.
//!
//! Before a person spends minutes of their machine on a whole file,
//! [`Enhancers::preview`] runs the model on the middle of one frame - a
//! few tiles, seconds - and says how long a tile took, which is what the
//! window's estimate for the whole run is made of.
//!
//! Enhance only ever runs because the person asked, so a crash inside it
//! does not come back on the next open and needs no tripwire; see
//! [`crate::jobs::Tripwire`].

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use concat_core::Frame;
use concat_core::time::Rational;
use concat_media::{
    DecodeOptions, Decoder, EncodeOptions, Encoder, FrameSink, FrameSource, RateMode,
    SeekableSource, VideoCodec,
};
use concat_vision::enhance::{self as vision, Enhancer};
use concat_vision::{ModelId, models};

pub use crate::cutout::Progress;
use crate::jobs::SingleFlight;

/// The most a preview shows, in output pixels: about what an inspector a
/// few hundred points wide, on a screen of two pixels a point, shows one
/// to one. Its source is a handful of tiles at most.
pub const PREVIEW_WIDTH: u32 = 640;
/// See [`PREVIEW_WIDTH`]: the same at sixteen by nine.
pub const PREVIEW_HEIGHT: u32 = 360;

/// The strength the inspector starts at: most of the model's restoration,
/// with a quarter of the original's texture left in so faces do not come
/// out waxen - the "plastic" look a full-strength enhancer is known for.
pub const DEFAULT_STRENGTH: f32 = 0.75;

/// What one enhancement covers.
#[derive(Clone, Debug)]
pub struct EnhanceRequest {
    /// The media file to read.
    pub media_path: String,
    /// A still: one frame in, one PNG out.
    pub still: bool,
    /// How many times larger the copy is; see
    /// [`concat_vision::enhance::factor_of`].
    pub factor: u32,
    /// How much of the model's answer is kept, 0 to 1; the rest is the
    /// original, enlarged plainly. See [`concat_vision::enhance::mix`].
    pub strength: f32,
    /// Where the copy goes; see [`target_for`].
    pub target: PathBuf,
    /// Seconds into the file to start at and how many to cover: the span
    /// a clip shows, with a little either side, rather than the whole
    /// file - an hour's recording with ten seconds of it on the timeline
    /// is ten seconds' work. `None` for the whole file, and for a still.
    pub span: Option<(f64, f64)>,
}

/// What a preview covers: one frame of a file, at the size it would be
/// enhanced to.
#[derive(Clone, Debug)]
pub struct PreviewRequest {
    /// The media file to read.
    pub media_path: String,
    /// Seconds into the file: the frame under the playhead. 0 for a still.
    pub seconds: f64,
    /// How many times larger, as for [`EnhanceRequest::factor`].
    pub factor: u32,
}

/// One frame's middle, before and after: both at the enhanced size, so
/// the window can blend them at any strength without the model.
#[derive(Clone, Debug)]
pub struct Preview {
    /// The middle of the frame enlarged by an ordinary bicubic filter -
    /// what the clip would be without Enhance, and what Strength blends
    /// towards.
    pub plain: Frame,
    /// The same middle as the model answers it, at full strength.
    pub enhanced: Frame,
    /// Seconds the model took over one tile, warm; see
    /// [`steady_tile_seconds`].
    pub tile_seconds: f64,
    /// Tiles in one whole frame of the file.
    pub tiles_per_frame: usize,
}

/// `strength` as the whole percent it is keyed by: what the inspector
/// shows, and what tells two copies apart, so float noise in a slider
/// never makes a copy of its own.
pub fn strength_percent(strength: f32) -> u32 {
    (strength.clamp(0.0, 1.0) * 100.0).round() as u32
}

/// How long the model takes over one tile once it is warm: the fastest of
/// `times`. The first run of a model in a session compiles it and can take
/// many times longer than the rest, and a preview is often that first run;
/// every tile is one shape, so the rest take as long as each other.
pub fn steady_tile_seconds(times: &[std::time::Duration]) -> f64 {
    times
        .iter()
        .min()
        .map(std::time::Duration::as_secs_f64)
        .unwrap_or(0.0)
}

/// About how long enhancing `frames` frames of `tiles_per_frame` tiles
/// each takes at `tile_seconds` a tile, in seconds: the model's share,
/// which is nearly all of it - decoding and encoding are a rounding error
/// beside it.
pub fn estimate_seconds(tile_seconds: f64, tiles_per_frame: usize, frames: f64) -> f64 {
    tile_seconds * tiles_per_frame as f64 * frames.max(1.0)
}

/// The enhancement service: the model, loaded once, and the one-job slot.
pub struct Enhancers {
    gate: Arc<SingleFlight>,
    /// The app's data directory, where the downloaded model lives.
    data: PathBuf,
    model: Mutex<Option<Arc<Enhancer>>>,
}

/// Where the enhanced copy of `media` at `factor` and `strength` lives
/// under `project`: in the cache, named by the file's path, size and
/// modification time and by the settings. Full strength - all Enhance
/// made before it had a strength - keeps the old name, so a copy made
/// then is found again rather than made twice. None for a file that
/// cannot be stat'ed, which cannot be read either.
///
/// A copy of a span is named by the exact span too, so a
/// clip's copy is not mistaken for another clip's of the same file.
pub fn target_for(
    project: &Path,
    media: &str,
    factor: u32,
    strength: f32,
    still: bool,
    span: Option<(f64, f64)>,
) -> Option<PathBuf> {
    let meta = std::fs::metadata(media).ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    media.hash(&mut hasher);
    meta.len().hash(&mut hasher);
    if let Some((start, duration)) = span {
        (start.to_bits(), duration.to_bits()).hash(&mut hasher);
    }
    if let Ok(modified) = meta.modified()
        && let Ok(since) = modified.duration_since(std::time::UNIX_EPOCH)
    {
        since.as_secs().hash(&mut hasher);
    }
    let extension = if still { "png" } else { "mp4" };
    let percent = strength_percent(strength);
    let strength = if percent == 100 {
        String::new()
    } else {
        format!("-{percent}pc")
    };
    Some(project.join("cache").join(format!(
        "enhance-{:016x}-{factor}x{strength}.{extension}",
        hasher.finish()
    )))
}

/// The frame of `path` at `seconds`, at its own size, or the first frame
/// when `seconds` is past the end.
fn frame_at(path: &str, seconds: f64) -> Result<Frame, String> {
    let mut options = DecodeOptions::default().in_software();
    if seconds > 0.0 {
        options = options.starting_at(Rational::approximate(seconds).unwrap_or(Rational::ZERO));
    }
    let mut decoder = Decoder::open(path, &options).map_err(|error| error.to_string())?;
    if let Some(frame) = decoder.next_frame().map_err(|error| error.to_string())? {
        return Ok(frame);
    }
    decoder
        .seek(Rational::ZERO)
        .map_err(|error| error.to_string())?;
    decoder
        .next_frame()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("{path} holds no picture"))
}

impl Enhancers {
    /// A service with nothing loaded yet, keeping its model under `data`.
    pub fn new(data: &Path) -> Enhancers {
        Enhancers {
            gate: Arc::new(SingleFlight::new()),
            data: data.to_path_buf(),
            model: Mutex::new(None),
        }
    }

    /// Whether an enhancement is running.
    pub fn is_busy(&self) -> bool {
        self.gate.is_busy()
    }

    /// Asks the running enhancement to stop after the frame in hand.
    pub fn cancel(&self) {
        self.gate.cancel();
    }

    /// Whether the model is on disk.
    pub fn installed(&self) -> bool {
        models::installed(&self.data, ModelId::Enhance)
    }

    /// The model, fetched first when it is not on disk.
    fn loaded(
        &self,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Arc<Enhancer>, String> {
        if let Some(loaded) = self
            .model
            .lock()
            .map_err(|_| "model slot poisoned")?
            .as_ref()
        {
            return Ok(Arc::clone(loaded));
        }
        let file = models::model_file(&self.data, ModelId::Enhance);
        if !self.installed() {
            crate::cutout::fetch(ModelId::Enhance, &file, cancel, progress)?;
        }
        let enhancer = Arc::new(fastest(&file, cancel)?);
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("enhance cancelled".to_owned());
        }
        *self.model.lock().map_err(|_| "model slot poisoned")? = Some(Arc::clone(&enhancer));
        Ok(enhancer)
    }

    /// Writes the copy `request` asks for, or finds it already written.
    /// Blocks until done, so run it on its own thread; `progress` is
    /// called as it goes. Returns where the copy is.
    pub fn enhance(
        &self,
        request: &EnhanceRequest,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<PathBuf, String> {
        if let Some((start, duration)) = request.span
            && (!start.is_finite() || !duration.is_finite() || start < 0.0 || duration <= 0.0)
        {
            return Err("invalid Enhance span".to_owned());
        }
        if request.target.is_file() {
            return Ok(request.target.clone());
        }
        let job = self.gate.begin("enhance")?;
        let cancel = job.cancel_handle();
        let enhancer = self.loaded(&cancel, progress)?;
        if let Some(folder) = request.target.parent() {
            std::fs::create_dir_all(folder)
                .map_err(|error| format!("{}: {error}", folder.display()))?;
        }
        progress(Progress::Analysing(0.0));
        let result = if request.still {
            still(&enhancer, request)
        } else {
            footage(&enhancer, request, &job, progress)
        };
        result.map(|()| request.target.clone())
    }

    /// The middle of one frame through the model, at most
    /// [`PREVIEW_WIDTH`] by [`PREVIEW_HEIGHT`] once enlarged, beside the
    /// same middle enlarged plainly. Takes the one job slot like a whole
    /// run - the model is one model - and blocks until done; `progress`
    /// reports the model's download, the one slow part a preview can
    /// have. Nothing is written: the clip is not touched.
    pub fn preview(
        &self,
        request: &PreviewRequest,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Preview, String> {
        let job = self.gate.begin("enhance")?;
        let cancel = job.cancel_handle();
        let enhancer = self.loaded(&cancel, progress)?;
        if job.cancelled() {
            return Err("enhance cancelled".to_owned());
        }
        let frame = frame_at(&request.media_path, request.seconds)?;
        let (width, height) = (frame.width(), frame.height());
        let region =
            vision::centre_region(width, height, request.factor, PREVIEW_WIDTH, PREVIEW_HEIGHT);
        let (enhanced, times) = enhancer.enhance_region(&frame, region, request.factor)?;
        let plain = vision::resize(
            &vision::cut(&frame, region),
            enhanced.width(),
            enhanced.height(),
        );
        Ok(Preview {
            plain,
            enhanced,
            tile_seconds: steady_tile_seconds(&times),
            tiles_per_frame: vision::tiles(width, height).len(),
        })
    }
}

/// One frame through the model, as a PNG: lossless, since the point was
/// the detail.
fn still(enhancer: &Enhancer, request: &EnhanceRequest) -> Result<(), String> {
    let mut decoder = Decoder::open(&request.media_path, &DecodeOptions::default().in_software())
        .map_err(|error| error.to_string())?;
    let frame = decoder
        .next_frame()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("{} holds no picture", request.media_path))?;
    let enhanced = vision::mix(
        enhancer.enhance(&frame, request.factor)?,
        &frame,
        request.strength,
    );
    let partial = request.target.with_extension("part.png");
    let written = (|| {
        let file = std::fs::File::create(&partial).map_err(|error| error.to_string())?;
        let mut encoder = png::Encoder::new(
            std::io::BufWriter::new(file),
            enhanced.width(),
            enhanced.height(),
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
        writer
            .write_image_data(enhanced.pixels())
            .map_err(|error| error.to_string())
    })();
    match written {
        Ok(()) => std::fs::rename(&partial, &request.target)
            .map_err(|error| format!("{}: {error}", request.target.display())),
        Err(error) => {
            let _ = std::fs::remove_file(&partial);
            Err(error)
        }
    }
}

/// Every frame through the model, encoded at the enlarged size, then the
/// original's sound copied in beside the picture. Written under partial
/// names and moved into place only when whole, so a copy that is there is
/// a copy that is complete.
fn footage(
    enhancer: &Enhancer,
    request: &EnhanceRequest,
    job: &crate::jobs::Job,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), String> {
    footage_with(request, job, progress, &mut |frame| {
        enhancer.enhance(frame, request.factor)
    })
}

/// Shared media pipeline; tests use a small deterministic processor instead of a downloaded model.
fn footage_with(
    request: &EnhanceRequest,
    job: &crate::jobs::Job,
    progress: &mut dyn FnMut(Progress),
    process: &mut dyn FnMut(&Frame) -> Result<Frame, String>,
) -> Result<(), String> {
    let info = concat_media::probe(&request.media_path).map_err(|error| error.to_string())?;
    let video = info
        .video
        .as_ref()
        .ok_or_else(|| "no video stream".to_owned())?;
    let rate = video.frame_rate;
    // About how many frames there are, for the readout; the count is not
    // known until the last one is read.
    let seconds = match request.span {
        Some((_, duration)) => duration,
        None => info.duration.map_or(0.0, |duration| duration.as_f64()),
    };
    let expected = (seconds * rate.fps().as_f64()).max(1.0);
    // Even sides, which the encoder wants: a stray pixel at the right or
    // bottom is cropped rather than the whole copy refused.
    let width = (video.width * request.factor) & !1;
    let height = (video.height * request.factor) & !1;
    if width == 0 || height == 0 {
        return Err("the picture is too small to enhance".to_owned());
    }

    let partial_video = request.target.with_extension("part.mp4");
    let partial_muxed = request.target.with_extension("muxed.part.mp4");
    let mut options = DecodeOptions::default().in_software().threaded(2);
    if let Some((start, _)) = request.span {
        options = options.starting_at(Rational::approximate(start).unwrap_or(Rational::ZERO));
    }
    let end = request.span.map(|(start, duration)| start + duration);
    let mut decoder =
        Decoder::open(&request.media_path, &options).map_err(|error| error.to_string())?;
    let mut encoder = Encoder::create(
        &partial_video,
        width,
        height,
        rate,
        &EncodeOptions {
            codec: VideoCodec::H264,
            // A copy the timeline reads for as long as the project lives:
            // near-transparent quality, and the time it takes is spent in
            // the model anyway.
            preset: "medium".to_owned(),
            crf: 16,
            rate_mode: RateMode::Vbr,
            bitrate_kbps: 0,
            ten_bit: false,
            color_range: concat_media::ColorRange::Limited,
            hardware: false,
            threads: 2,
            software_only: false,
        },
    )
    .map_err(|error| error.to_string())?;
    let encoded = (|| {
        let mut count = 0u64;
        let mut last: Option<(Frame, Frame)> = None;
        while let Some(frame) = decoder.next_frame().map_err(|error| error.to_string())? {
            if job.cancelled() {
                return Err("enhance cancelled".to_owned());
            }
            if let Some(end) = end {
                let past = decoder
                    .position()
                    .map_or(count as f64 >= expected, |at| at.as_f64() >= end);
                if past {
                    break;
                }
            }
            // Exact repeated pictures share the already mixed answer; keep only one pair.
            let enhanced = match &last {
                Some((before, answer))
                    if before.width() == frame.width()
                        && before.height() == frame.height()
                        && before.pixels() == frame.pixels() =>
                {
                    answer.clone()
                }
                _ => {
                    let enhanced = vision::mix(process(&frame)?, &frame, request.strength);
                    let enhanced = crop(&enhanced, width, height);
                    last = Some((frame, enhanced.clone()));
                    enhanced
                }
            };
            encoder
                .write_frame(&enhanced)
                .map_err(|error| error.to_string())?;
            count += 1;
            progress(Progress::Analysing(
                (count as f64 / expected).min(0.99) as f32
            ));
        }
        encoder.finish().map_err(|error| error.to_string())
    })();
    // Windows will not rename or delete a file something still holds open,
    // and a finished encoder keeps its output open until it is dropped.
    drop(encoder);
    drop(decoder);
    let encoded = encoded.and_then(|()| {
        if job.cancelled() {
            Err("enhance cancelled".to_owned())
        } else {
            Ok(())
        }
    });
    if let Err(error) = encoded {
        let _ = std::fs::remove_file(&partial_video);
        return Err(error);
    }
    let finished = if info.audio.is_some() {
        // A span's sound is mixed out for the same seconds, so the copy's
        // sound starts where its picture does; a whole file's is the
        // file's own, muxed in as it is.
        let muxed = match request.span {
            Some((start, duration)) => {
                let partial_audio = request.target.with_extension("part.m4a");
                let clip = concat_media::audio::AudioClip {
                    path: PathBuf::from(&request.media_path),
                    stream: None,
                    start: 0.0,
                    duration,
                    source_start: start,
                    speed: 1.0,
                    preserve_pitch: true,
                    retime: None,
                    volume: 1.0,
                    volume_curve: concat_core::animate::Track::new(Vec::new()),
                    fade_in: 0.0,
                    fade_out: 0.0,
                    quadratic_fades: false,
                    filter_chain: String::new(),
                };
                let muxed = concat_media::audio::mix_to_file_cancelling(
                    &[clip],
                    duration,
                    &partial_audio,
                    Some(job.cancel_flag()),
                )
                .and_then(|()| {
                    concat_media::audio::mux_cancelling(
                        &partial_video,
                        &partial_audio,
                        &partial_muxed,
                        Some(job.cancel_flag()),
                    )
                });
                let _ = std::fs::remove_file(&partial_audio);
                muxed
            }
            None => concat_media::audio::mux(
                &partial_video,
                Path::new(&request.media_path),
                &partial_muxed,
            ),
        }
        .map_err(|error| error.to_string());
        let _ = std::fs::remove_file(&partial_video);
        muxed.and_then(|()| {
            std::fs::rename(&partial_muxed, &request.target)
                .map_err(|error| format!("{}: {error}", request.target.display()))
        })
    } else {
        std::fs::rename(&partial_video, &request.target)
            .map_err(|error| format!("{}: {error}", request.target.display()))
    };
    if finished.is_err() {
        let _ = std::fs::remove_file(&partial_muxed);
    }
    finished
}

/// `frame` cut to `width` by `height` from its top-left, or the frame
/// itself when it is that size already.
pub(crate) fn crop(frame: &concat_core::Frame, width: u32, height: u32) -> concat_core::Frame {
    if frame.width() == width && frame.height() == height {
        return frame.clone();
    }
    let mut out = concat_core::Frame::black(width, height);
    let bytes = width as usize * concat_core::frame::BYTES_PER_PIXEL;
    let stride = frame.width() as usize * concat_core::frame::BYTES_PER_PIXEL;
    let source = frame.pixels();
    let pixels = out.pixels_mut();
    for y in 0..height as usize {
        pixels[y * bytes..(y + 1) * bytes].copy_from_slice(&source[y * stride..y * stride + bytes]);
    }
    out
}

/// The model on whichever engine is fastest here, found by running both:
/// our own shaders on the GPU, and ONNX Runtime with the platform's
/// accelerator (CoreML on Apple's machines). Neither wins everywhere. On
/// an M5, CoreML beat our shaders 45 ms to 105 ms a window in a test of
/// its own, yet in the running app it took about 0.2 s a window, seven
/// times our shaders, so a choice made from a benchmark was the wrong one
/// where it mattered. Each candidate is warmed on one window and timed on
/// the next two, in the app as it is, and both times are logged. ONNX
/// Runtime on the CPU is what is left when no GPU opens.
fn fastest(file: &Path, cancel: &AtomicBool) -> Result<Enhancer, String> {
    let gpu = on_gpu(file).and_then(|gpu| {
        let seconds = window_seconds(&gpu, cancel);
        if seconds.is_finite() {
            log::info!("enhance: GPU warm window {:.0} ms", seconds * 1000.0);
            Some((gpu, seconds))
        } else {
            log::warn!("enhance: GPU warm-up failed; trying ONNX Runtime");
            None
        }
    });
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("enhance cancelled".to_owned());
    }
    // Measure on this machine, including CPU-only runtimes: an adapter's
    // existence is not evidence that it is faster than every CPU.
    let runtime = match Enhancer::load(file) {
        Ok(enhancer) => Some(enhancer),
        Err(error) => {
            log::warn!("enhance: ONNX Runtime would not load it: {error}");
            None
        }
    };
    let selected = match (gpu, runtime) {
        (Some((gpu, ours)), Some(runtime)) => {
            let theirs = window_seconds(&runtime, cancel);
            log::info!(
                "enhance: a window takes {:.0} ms on our shaders, {:.0} ms through ONNX Runtime",
                ours * 1000.0,
                theirs * 1000.0
            );
            if theirs < ours {
                log::info!("enhance: through ONNX Runtime");
                Ok(runtime)
            } else {
                log::info!("enhance: on our shaders");
                Ok(gpu)
            }
        }
        (Some((gpu, _)), None) => Ok(gpu),
        (None, Some(runtime)) => Ok(runtime),
        (None, None) => Err("enhance: nothing here could run the model".to_owned()),
    };
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        Err("enhance cancelled".to_owned())
    } else {
        selected
    }
}

/// Seconds `enhancer` takes over one window once warm: the first run
/// compiles and is not counted, the better of the next two is.
fn window_seconds(enhancer: &Enhancer, cancel: &AtomicBool) -> f64 {
    let side = vision::WINDOW;
    let frame = Frame::black(side - 2 * vision::PAD, side - 2 * vision::PAD);
    let region = vision::Tile {
        x: 0,
        y: 0,
        width: frame.width(),
        height: frame.height(),
    };
    let mut best = f64::INFINITY;
    for run in 0..3 {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return f64::INFINITY;
        }
        let started = std::time::Instant::now();
        if enhancer.enhance_region(&frame, region, 1).is_err() {
            return f64::INFINITY;
        }
        if run > 0 {
            best = best.min(started.elapsed().as_secs_f64());
        }
    }
    best
}

/// The model on the GPU, in compute shaders of our own
/// (`concat_render::upscale`), when its file reads as the compact network
/// and a GPU opens: about six times what ONNX Runtime does on this
/// machine's CPU, and the CPU is where ONNX Runtime ran nearly everywhere -
/// Windows' runtime has no GPU provider at all. `None` sends Enhance the
/// slow way rather than not at all.
fn on_gpu(file: &Path) -> Option<Enhancer> {
    if let Err(error) = crate::models::verify(file, ModelId::Enhance.spec().sha256) {
        log::warn!("enhance: model is not the pinned compact network: {error}; using ONNX Runtime");
        return None;
    }
    let bytes = std::fs::read(file).ok()?;
    let net = match concat_vision::compact::CompactNet::read(&bytes) {
        Ok(net) => net,
        Err(error) => {
            log::warn!("enhance: {error}; on the CPU instead");
            return None;
        }
    };
    let layers: Vec<concat_render::upscale::ConvLayer> = net
        .layers
        .into_iter()
        .map(|layer| concat_render::upscale::ConvLayer {
            in_channels: layer.in_channels,
            out_channels: layer.out_channels,
            weight: layer.weight,
            bias: layer.bias,
            prelu: layer.prelu,
        })
        .collect();
    let upscaler =
        concat_render::upscale::Upscaler::new(&layers, net.scale as u32, vision::WINDOW)?;
    log::info!(
        "enhance: on the GPU{}",
        if upscaler.half() {
            ", in half floats"
        } else {
            ""
        }
    );
    Some(Enhancer::with(Box::new(GpuWindows {
        gpu: Some(upscaler),
        file: file.to_path_buf(),
        fallback: None,
    })))
}

/// The GPU upscaler as the model `Enhancer` runs window by window.
struct GpuWindows {
    gpu: Option<concat_render::upscale::Upscaler>,
    file: PathBuf,
    fallback: Option<Box<dyn vision::WindowRunner>>,
}

impl vision::WindowRunner for GpuWindows {
    fn run(&mut self, planes: Vec<f32>) -> Result<Vec<f32>, String> {
        if let Some(gpu) = &self.gpu {
            match gpu.run(&planes) {
                Ok(answer) => return Ok(answer),
                Err(error) => log::warn!("enhance: {error}; switching to ONNX Runtime"),
            }
            self.gpu = None;
        }
        if self.fallback.is_none() {
            self.fallback = Some(vision::runtime_runner(&self.file)?);
        }
        self.fallback
            .as_mut()
            .expect("runtime initialized")
            .run(planes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Explicit real-model comparison; no download or inference in ordinary tests.
    #[test]
    #[ignore = "requires CLOVERCUT_ENHANCE_MODEL and a real GPU"]
    fn real_gpu_enhance_matches_runtime_and_reports_time() {
        let path = PathBuf::from(std::env::var_os("CLOVERCUT_ENHANCE_MODEL").expect("model path"));
        let gpu = on_gpu(&path).expect("real GPU");
        let runtime = Enhancer::load(&path).expect("runtime");
        let frame = Frame::from_rgba(
            272,
            64,
            (0..272 * 64)
                .flat_map(|n| {
                    let x = n % 272;
                    let y = n / 272;
                    [(x % 256) as u8, (y * 3) as u8, ((x + y) % 256) as u8, 255]
                })
                .collect(),
        )
        .unwrap();
        let cancel = AtomicBool::new(false);
        let gpu_warm = window_seconds(&gpu, &cancel);
        let cpu_warm = window_seconds(&runtime, &cancel);
        assert!(gpu_warm.is_finite() && cpu_warm.is_finite());
        let start = std::time::Instant::now();
        let answer = gpu.enhance(&frame, 2).unwrap();
        let gpu_seconds = start.elapsed().as_secs_f64();
        let start = std::time::Instant::now();
        let reference = runtime.enhance(&frame, 2).unwrap();
        let cpu_seconds = start.elapsed().as_secs_f64();
        assert_eq!((answer.width(), answer.height()), (544, 128));
        let errors: Vec<f64> = answer
            .pixels()
            .iter()
            .zip(reference.pixels())
            .map(|(a, b)| f64::from(a.abs_diff(*b)))
            .collect();
        let mean = errors.iter().sum::<f64>() / errors.len() as f64;
        let worst = errors.iter().copied().fold(0.0, f64::max);
        eprintln!(
            "Enhance host pipeline: two tiles GPU {gpu_seconds:.3}s, runtime {cpu_seconds:.3}s; warm window GPU {gpu_warm:.3}s runtime {cpu_warm:.3}s; mean error {mean:.3}/255, max {worst}/255"
        );
        let selected = fastest(&path, &cancel).expect("selects a measured engine");
        let selected_answer = selected.enhance(&frame, 2).expect("selected engine runs");
        assert_eq!(
            (selected_answer.width(), selected_answer.height()),
            (544, 128)
        );
        assert!(
            selected_answer
                .pixels()
                .iter()
                .zip(reference.pixels())
                .all(|(a, b)| a.abs_diff(*b) <= 8)
        );
        assert!(
            mean < 0.5 && worst <= 8.0,
            "GPU differs from runtime: mean {mean}, max {worst}"
        );
    }

    #[test]
    fn the_copy_is_named_after_the_file_and_the_factor() {
        let dir = std::env::temp_dir().join(format!("concat-enhance-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let media = dir.join("clip.mp4");
        std::fs::write(&media, b"not really").expect("write");
        let media = media.to_string_lossy().into_owned();
        let project = dir.join("project");
        // Full strength keeps the name Enhance gave copies before it had
        // one, so those are found again.
        let two = target_for(&project, &media, 2, 1.0, false, None).expect("named");
        assert!(two.starts_with(project.join("cache")));
        assert!(two.to_string_lossy().ends_with("-2x.mp4"), "{two:?}");
        assert_eq!(
            target_for(&project, &media, 2, 1.0, false, None),
            Some(two.clone())
        );
        assert_ne!(
            target_for(&project, &media, 1, 1.0, false, None),
            Some(two.clone())
        );
        let still = target_for(&project, &media, 2, 1.0, true, None).expect("named");
        assert!(still.to_string_lossy().ends_with("-2x.png"), "{still:?}");
        assert_eq!(
            target_for(
                &project,
                &dir.join("gone.mp4").to_string_lossy(),
                2,
                1.0,
                false,
                None
            ),
            None
        );
        // Another strength is another copy, keyed by its whole percent so
        // a slider's float noise is not.
        let blended = target_for(&project, &media, 2, 0.75, false, None).expect("named");
        assert!(
            blended.to_string_lossy().ends_with("-2x-75pc.mp4"),
            "{blended:?}"
        );
        assert_ne!(blended, two);
        assert_eq!(
            target_for(&project, &media, 2, 0.750_001, false, None),
            Some(blended.clone())
        );
        assert_ne!(
            target_for(&project, &media, 4, 0.75, false, None),
            Some(blended)
        );
        assert_ne!(
            target_for(&project, &media, 2, 1.0, false, Some((1.0, 1.5))),
            target_for(&project, &media, 2, 1.0, false, Some((1.0001, 1.5))),
        );
        // A file with different bytes is a different copy.
        std::fs::write(&media, b"not really, but longer").expect("write");
        assert_ne!(target_for(&project, &media, 2, 1.0, false, None), Some(two));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_span_keeps_its_picture_sound_and_reuses_identical_frames() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.mkv");
        // FFV1 keeps exact repeated frames; the second colour and the tone start at one second.
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-threads",
                "1",
                "-f",
                "lavfi",
                "-i",
                "color=red:s=64x48:r=10:d=1",
                "-f",
                "lavfi",
                "-i",
                "color=blue:s=64x48:r=10:d=2",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=880:duration=3",
                "-filter_complex",
                "[0:v][1:v]concat=n=2:v=1:a=0[v];[2:a]volume=enable='lt(t,1)':volume=0[a]",
                "-map",
                "[v]",
                "-map",
                "[a]",
                "-c:v",
                "ffv1",
                "-c:a",
                "pcm_s16le",
                "-threads",
                "1",
            ])
            .arg(&source)
            .status()
            .expect("ffmpeg fixture tool");
        assert!(status.success());
        let target = dir.path().join("span.mp4");
        let request = EnhanceRequest {
            media_path: source.to_string_lossy().into_owned(),
            still: false,
            factor: 2,
            strength: 1.0,
            target: target.clone(),
            span: Some((1.0, 1.5)),
        };
        let gate = Arc::new(SingleFlight::new());
        let job = gate.begin("test").unwrap();
        let mut calls = 0;
        footage_with(&request, &job, &mut |_| {}, &mut |frame| {
            calls += 1;
            Ok(vision::resize(frame, frame.width() * 2, frame.height() * 2))
        })
        .expect("processes only the requested span");
        assert_eq!(calls, 1, "identical decoded pictures reuse the answer");
        let info = concat_media::probe(&target).unwrap();
        assert!((info.duration.unwrap().as_f64() - 1.5).abs() < 0.1);
        let video = info.video.unwrap();
        assert_eq!((video.width, video.height), (128, 96));
        let first = frame_at(&target.to_string_lossy(), 0.0).unwrap();
        let pixel = first.pixel(64, 48).unwrap();
        assert!(
            pixel[2] > 180 && pixel[0] < 40,
            "starts at the blue frame: {pixel:?}"
        );
        let sound = concat_media::AudioDecoder::open(
            &target,
            &concat_media::AudioOptions {
                duration: Some(0.25),
                channels: 1,
                format: concat_media::SampleFormat::F32,
                ..Default::default()
            },
        )
        .unwrap()
        .collect_f32()
        .unwrap();
        let rms =
            (sound.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / sound.len() as f64).sqrt();
        assert!(
            rms > 0.03,
            "audio starts at the tone, not the silent original beginning: {rms}"
        );
        assert!(!target.with_extension("part.mp4").exists());
        assert!(!target.with_extension("part.m4a").exists());
        assert!(!target.with_extension("muxed.part.mp4").exists());
        // Cancelling from the progress callback must never publish a partial copy.
        let cancelled = EnhanceRequest {
            target: dir.path().join("cancelled.mp4"),
            ..request
        };
        let error = footage_with(&cancelled, &job, &mut |_| gate.cancel(), &mut |frame| {
            Ok(vision::resize(frame, frame.width() * 2, frame.height() * 2))
        })
        .unwrap_err();
        assert!(error.contains("cancelled"));
        assert!(!cancelled.target.exists());
        assert!(!cancelled.target.with_extension("part.mp4").exists());
    }

    #[test]
    fn the_estimate_is_the_warm_tile_time_over_every_tile_of_every_frame() {
        use std::time::Duration;
        // The first run compiled the model; the warm ones say what a tile
        // costs.
        let times = [
            Duration::from_millis(2400),
            Duration::from_millis(300),
            Duration::from_millis(320),
        ];
        assert!((steady_tile_seconds(&times) - 0.3).abs() < 1e-9);
        assert_eq!(steady_tile_seconds(&[]), 0.0);
        // 1080p is 8 by 5 tiles; ten seconds at 30 is 300 frames.
        let tiles = vision::tiles(1920, 1080).len();
        assert_eq!(tiles, 40);
        assert!((estimate_seconds(0.3, tiles, 300.0) - 3600.0).abs() < 1e-6);
        // A still is one frame, however short its duration says it is.
        assert!((estimate_seconds(0.3, tiles, 0.0) - 12.0).abs() < 1e-6);
        assert_eq!(strength_percent(0.754), 75);
        assert_eq!(strength_percent(1.2), 100);
        assert_eq!(strength_percent(-1.0), 0);
    }

    #[test]
    fn a_crop_takes_the_top_left_and_leaves_a_fitting_frame_alone() {
        let mut frame = concat_core::Frame::black(4, 3);
        frame.set_pixel(3, 2, [1, 2, 3, 255]);
        frame.set_pixel(0, 0, [9, 9, 9, 255]);
        let same = crop(&frame, 4, 3);
        assert_eq!(same.pixels(), frame.pixels());
        let cut = crop(&frame, 2, 2);
        assert_eq!((cut.width(), cut.height()), (2, 2));
        assert_eq!(cut.pixel(0, 0), Some([9, 9, 9, 255]));
        assert_eq!(cut.pixel(1, 1), Some([0, 0, 0, 255]));
    }
}
