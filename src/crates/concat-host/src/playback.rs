// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Engine-owned audio playback.
//!
//! The engine decodes, mixes and clocks all audible clips itself, and the
//! window is a controller that follows.
//!
//! The shape of it:
//!
//! - Every audible clip's span is decoded **once** straight to raw PCM -
//!   the clip's filter chain and speed baked in with the same filters the
//!   exporter uses, so the preview and the export cannot disagree. The
//!   PCM lands as a file in the project's cache and is memory-mapped, so
//!   memory stays bounded however long the material is, and a reopened
//!   project pays nothing.
//!
//! - A span not decoded yet borrows the sound of one that is, from the same
//!   file under the same treatment: what a split, a trim inwards or an
//!   undo leaves is a stretch of a span already mapped, and it plays from
//!   that span's samples the moment the edit lands - see [`stand_in`]. A
//!   cut used to wait out a decode of everything after it, seconds to
//!   minutes of silence on a long recording. Where the borrowed samples
//!   are exactly the decode's - no speed change, no filter - the decode is
//!   a copy of them; otherwise FFmpeg decodes it, and the stand-in plays
//!   until it lands.
//! - Where a sound layer runs, the clips cannot each carry it: its chain is
//!   over their sum. The stretch is mixed once instead, through the
//!   exporter's own mix with the layer over it, and plays as one span in
//!   their place - see [`with_sound_layers`]. Until it lands the clips
//!   under it play as they are, so an edit there is never silence.
//!
//! - One cpal output stream mixes the mapped clips sample by sample, with
//!   volume and fades applied at mix time. Gain-only edits therefore never
//!   re-decode, and what a fade sounds like has exactly one definition: this
//!   file's `gain_at`.
//!
//! - The playback clock is the audio device's own sample counter. The window
//!   reads [`Playback::position_now`], which carries it forward between the
//!   device's buffers; there is no second clock to drift against.
//!
//! - The stream is *supervised*: pulled headphones, a changed default
//!   device, or a device that was missing at launch all rebuild the stream
//!   rather than leaving the session silent until restart. The supervisor
//!   re-seeds the new stream from the shared clock and the last clip set,
//!   so a rebuild sounds like a hiccup, not a reset.
//!
//! The mix callback owns its state and receives changes as messages; the
//! only lock it touches is an uncontended try-lock on the message channel
//! (contended solely during a rebuild, when the callback is being replaced
//! anyway). Decodes run on a small worker pool, newest request first - a
//! scrubbed trim queues many spans and the one under the playhead matters
//! most - and the clip set is re-sent to the callback as each one lands.
//!
//! Both caches are bounded. In memory, mappings not referenced by the
//! current clip set are dropped beyond a cap, oldest use first. On disk,
//! `cache/audio` keeps a byte budget: least-recently-modified files beyond
//! it are removed, never one the current clip set is using.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};

use concat_core::animate::{Ease, Key, Track};
use serde::Deserialize;

/// Every clip is decoded to this rate; the callback resamples to the device.
const PCM_RATE: u32 = 48_000;

/// Decoded spans kept mapped beyond the current clip set. Enough that
/// undo/redo across a few edits never re-decodes; small enough that a long
/// session cannot accumulate hundreds of mappings.
const MEMORY_CACHE_CAP: usize = 64;

/// Byte budget for `cache/audio` on disk. Stereo 16-bit 48k is ~11 MB a
/// minute, so this keeps hours of decoded material before anything is
/// evicted.
const DISK_CACHE_BUDGET: u64 = 2 * 1024 * 1024 * 1024;

/// Diagnostic record of an audio output callback buffer.
#[derive(Clone, Copy, Debug)]
pub struct CallbackMetric {
    /// Monotonic instant when the callback processed this buffer.
    pub instant: std::time::Instant,
    /// Timeline position at the start of the buffer (seconds).
    pub start_pos: f64,
    /// Timeline position at the end of the buffer (seconds).
    pub end_pos: f64,
    /// Peak amplitude observed across channels in this buffer.
    pub peak: f32,
    /// Number of sample frames in this buffer.
    pub frames: usize,
}

/// Concurrent FFmpeg decode processes. Two keeps a dual-span edit prompt
/// without letting a scrubbed trim fan out a dozen full-file decodes.
const DECODE_WORKERS: usize = 2;

/// One key of a clip's gain ride, as a spec carries it.
///
/// The engine's `animate::Key` in a shape serde can read: the same three
/// facts, with the timing function as its four control-point numbers.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GainKey {
    /// Where in the clip, `0..=1`.
    pub at: f64,
    /// Absolute linear gain there.
    pub gain: f64,
    /// The timing function into this key: a cubic bezier's two control
    /// points, `[x1, y1, x2, y2]`. Absent is a straight line.
    #[serde(default = "linear_ease")]
    pub ease: [f64; 4],
}

/// A straight line, for a spec that names no easing.
fn linear_ease() -> [f64; 4] {
    [0.0, 0.0, 1.0, 1.0]
}

/// One audible clip, as the window describes it.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipSpec {
    /// The media file.
    pub path: String,
    /// Which of the file's audio streams, by index; absent is the first.
    #[serde(default)]
    pub audio_stream: Option<u32>,
    /// Timeline seconds.
    pub start: f64,
    /// Timeline seconds the clip occupies.
    pub duration: f64,
    /// Seconds into the source file.
    pub source_start: f64,
    /// Linear gain. The whole clip's, unless `volume_curve` says otherwise.
    pub volume: f32,
    /// Gain as it changes over the clip, or empty for the constant above.
    /// A curve supersedes `volume` rather than multiplying it, matching the
    /// mixer - see `concat_media::audio::AudioClip`.
    #[serde(default)]
    pub volume_curve: Vec<GainKey>,
    /// Fade-in length in seconds.
    pub fade_in: f64,
    /// Fade-out length in seconds.
    pub fade_out: f64,
    /// Quadratic audio envelope, or the legacy linear ramp when absent.
    #[serde(default)]
    pub quadratic_fades: bool,
    /// Playback speed; 1.0 is the source's own.
    pub speed: f64,
    /// Whether a speed change keeps the pitch.
    pub preserve_pitch: bool,
    /// Internal continuous tempo map from the flattened project.
    #[serde(skip)]
    pub retime: Option<concat_media::audio::TempoCurve>,
    /// FFmpeg audio filter chain, or empty.
    #[serde(default)]
    pub chain: String,
    /// When set, this span is not a file but the mix of these clips under
    /// sound layers, over `start` to `start + duration` of the timeline,
    /// and it plays in their place there; see [`with_sound_layers`]. The
    /// fields above that describe a file are then unused.
    #[serde(default)]
    pub bus: Option<BusMix>,
}

/// The clips a stretch under sound layers mixes, and the layers.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusMix {
    /// Every audible clip with something inside the stretch, whole: the
    /// mix cuts each at the stretch's edges on its own clock.
    pub clips: Vec<ClipSpec>,
    /// The sound layers over the stretch, in the order they are heard.
    pub layers: Vec<BusSpec>,
}

/// One sound layer, as a spec carries it; see `concat_media::audio::AudioBus`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusSpec {
    /// Timeline seconds.
    pub start: f64,
    /// Timeline seconds it runs.
    pub duration: f64,
    /// The FFmpeg audio filters it runs over the mix.
    pub chain: String,
    /// How much of the treated sound replaces the untreated, `0..=1`.
    pub strength: f64,
    /// Optional keyframed mix strength, on the layer's own clock.
    #[serde(default)]
    pub strength_curve: Vec<GainKey>,
    /// Ramp in, seconds.
    pub fade_in: f64,
    /// Ramp out, seconds.
    pub fade_out: f64,
}

impl BusSpec {
    fn engine(&self) -> concat_media::audio::AudioBus {
        concat_media::audio::AudioBus {
            start: self.start,
            duration: self.duration,
            filter_chain: self.chain.clone(),
            strength: self.strength,
            strength_curve: track_of(&self.strength_curve),
            fade_in: self.fade_in,
            fade_out: self.fade_out,
        }
    }
}

impl ClipSpec {
    /// The engine's view of this clip, for a mix.
    fn engine(&self) -> concat_media::audio::AudioClip {
        concat_media::audio::AudioClip {
            path: PathBuf::from(&self.path),
            stream: self.audio_stream.map(|index| index as usize),
            start: self.start,
            duration: self.duration,
            source_start: self.source_start,
            speed: self.speed,
            preserve_pitch: self.preserve_pitch,
            retime: self.retime.clone(),
            volume: f64::from(self.volume),
            volume_curve: track_of(&self.volume_curve),
            fade_in: self.fade_in,
            fade_out: self.fade_out,
            quadratic_fades: self.quadratic_fades,
            filter_chain: self.chain.clone(),
        }
    }
}

/// What a flattened edit makes audible, as playback takes it: every clip
/// with sound through the exporter's own cut into pieces, so a curve or a
/// reverse sounds in the window as it will in the file, and the stretches
/// under sound layers mixed as the file mixes them. One definition for
/// every shell - the desktop window and the phone's alike.
pub fn audible(clips: &[concat_export::ExportClip]) -> Vec<ClipSpec> {
    let specs = clips
        .iter()
        .filter(|clip| {
            !clip.muted
                && (clip.kind == concat_export::ClipKind::Audio
                    || (clip.kind == concat_export::ClipKind::Video
                        && clip.has_audio.unwrap_or(true)))
        })
        .flat_map(concat_export::audio_pieces)
        .map(|piece| ClipSpec {
            path: piece.path.to_string_lossy().into_owned(),
            audio_stream: piece.stream.map(|index| index as u32),
            start: piece.start,
            duration: piece.duration,
            source_start: piece.source_start,
            volume: piece.volume as f32,
            volume_curve: piece
                .volume_curve
                .keys()
                .iter()
                .map(|key| GainKey {
                    at: key.at,
                    gain: key.value,
                    ease: [key.ease.x1, key.ease.y1, key.ease.x2, key.ease.y2],
                })
                .collect(),
            fade_in: piece.fade_in,
            fade_out: piece.fade_out,
            quadratic_fades: piece.quadratic_fades,
            speed: piece.speed,
            preserve_pitch: piece.preserve_pitch,
            retime: piece.retime,
            chain: piece.filter_chain,
            bus: None,
        })
        .collect();
    with_sound_layers(specs, &concat_export::audio_buses(clips))
}

/// `specs` with the stretches under sound layers mixed apart: one span per
/// run of layers that touch or overlap, carrying every clip with something
/// inside it and the layers over it. The clips themselves stay in the set,
/// whole - the player silences them under a span once that span is
/// decoded, and until then they are what is heard.
///
/// Layers are grouped so a span is the whole of what they cover together:
/// two that overlap are one sum treated twice, not two sums.
pub fn with_sound_layers(
    mut specs: Vec<ClipSpec>,
    layers: &[concat_media::audio::AudioBus],
) -> Vec<ClipSpec> {
    let layers: Vec<&concat_media::audio::AudioBus> =
        layers.iter().filter(|layer| layer.audible()).collect();
    if layers.is_empty() {
        return specs;
    }
    // In the order given, which is the order they are heard; grouped by
    // where they reach.
    let mut order: Vec<usize> = (0..layers.len()).collect();
    order.sort_by(|&a, &b| layers[a].start.total_cmp(&layers[b].start));
    let mut groups: Vec<(f64, f64, Vec<usize>)> = Vec::new();
    for index in order {
        let layer = layers[index];
        match groups.last_mut() {
            Some((_, end, members)) if layer.start <= *end => {
                *end = end.max(layer.end());
                members.push(index);
            }
            _ => groups.push((layer.start, layer.end(), vec![index])),
        }
    }
    let spans: Vec<ClipSpec> = groups
        .into_iter()
        .map(|(start, end, mut members)| {
            members.sort_unstable();
            let clips = specs
                .iter()
                .filter(|spec| spec.start < end && spec.start + spec.duration > start)
                .cloned()
                .collect();
            let over = members
                .into_iter()
                .map(|index| {
                    let layer = layers[index];
                    BusSpec {
                        start: layer.start,
                        duration: layer.duration,
                        chain: layer.filter_chain.clone(),
                        strength: layer.strength,
                        strength_curve: layer
                            .strength_curve
                            .keys()
                            .iter()
                            .map(|key| GainKey {
                                at: key.at,
                                gain: key.value,
                                ease: [key.ease.x1, key.ease.y1, key.ease.x2, key.ease.y2],
                            })
                            .collect(),
                        fade_in: layer.fade_in,
                        fade_out: layer.fade_out,
                    }
                })
                .collect();
            ClipSpec {
                retime: None,
                path: String::new(),
                audio_stream: None,
                start,
                duration: end - start,
                source_start: 0.0,
                volume: 1.0,
                volume_curve: Vec::new(),
                fade_in: 0.0,
                fade_out: 0.0,
                quadratic_fades: false,
                speed: 1.0,
                preserve_pitch: true,
                chain: String::new(),
                bus: Some(BusMix {
                    clips,
                    layers: over,
                }),
            }
        })
        .collect();
    specs.extend(spans);
    specs
}

/// Where playback tells the window what it cannot ask: the clock, and the
/// failures a user would otherwise experience as unexplained silence.
pub trait PlaybackEvents: Send + Sync + 'static {
    /// Timeline position in seconds, at ~30Hz while playing.
    fn position(&self, seconds: f64);
    /// A failure the user should see.
    fn error(&self, message: String);
}

/// Decoded audio: 16-bit little-endian stereo at [`PCM_RATE`] inside a WAV
/// container, memory-mapped from the cache file so the OS decides what stays
/// resident.
///
/// WAV rather than raw PCM so the cache files are ordinary audio files -
/// openable in anything, and self-describing if the format ever changes. The
/// header costs a one-time chunk walk here; the samples inside are the
/// identical bytes.
struct Pcm {
    map: memmap2::Mmap,
    /// Byte offset of the first sample - just past the `data` chunk header.
    start: usize,
    frames: u64,
}

impl Pcm {
    fn open(path: &Path) -> Result<Pcm, String> {
        let file = std::fs::File::open(path)
            .map_err(|error| format!("could not open {}: {error}", path.display()))?;
        // SAFETY: the cache file is written by our own decode, renamed into
        // place only when complete, and never modified afterwards; the sweep
        // deletes rather than truncates, and a deleted file's mapping stays
        // valid. Reads are bounds-checked against the mapped length.
        let map = unsafe { memmap2::Mmap::map(&file) }
            .map_err(|error| format!("could not map {}: {error}", path.display()))?;
        let (start, length) = wav_data_range(&map)
            .ok_or_else(|| format!("{} is not the wav this cache writes", path.display()))?;
        let frames = (length / 4) as u64;
        Ok(Pcm { map, start, frames })
    }

    /// One sample as a float in [-1, 1]. Out of range reads are silence,
    /// before the first frame as after the last.
    fn sample(&self, frame: impl TryInto<u64>, channel: usize) -> f32 {
        let Ok(frame) = frame.try_into() else {
            return 0.0;
        };
        if frame >= self.frames {
            return 0.0;
        }
        let at = self.start + (frame as usize * 2 + channel) * 2;
        let raw = i16::from_le_bytes([self.map[at], self.map[at + 1]]);
        f32::from(raw) / 32768.0
    }

    /// The bytes of `count` frames from `from`, as many as there are.
    fn frames_bytes(&self, from: u64, count: u64) -> &[u8] {
        let from = from.min(self.frames);
        let to = from.saturating_add(count).min(self.frames);
        &self.map[self.start + from as usize * 4..self.start + to as usize * 4]
    }
}

/// Finds the samples inside a WAV file: byte offset of the `data` chunk's
/// payload and its usable length.
///
/// A plain chunk walk, not a format library: the file was written by our own
/// [`WavWriter`], so the only variability is which bookkeeping chunks precede
/// `data`. A `u32::MAX` size - what a writer that could not seek back would
/// leave - means the payload is simply the rest of the file.
fn wav_data_range(bytes: &[u8]) -> Option<(usize, usize)> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }

    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]]);
        let payload = at + 8;

        if id == b"data" {
            let available = bytes.len() - payload;
            let length = if size == u32::MAX {
                available
            } else {
                (size as usize).min(available)
            };
            return Some((payload, length));
        }

        // A placeholder size on any other chunk means the walk cannot
        // continue - refuse rather than misread samples as headers.
        if size == u32::MAX {
            return None;
        }
        // Chunks are word-aligned; an odd size is followed by a pad byte.
        at = payload + size as usize + (size as usize & 1);
    }
    None
}

/// A clip the mix callback can play: spec fields it needs, plus the audio.
/// Clone is cheap - the samples are behind an `Arc` - and the supervisor
/// uses it to seed a rebuilt stream with the set the old one was playing.
#[derive(Clone)]
struct ActiveClip {
    start: f64,
    duration: f64,
    volume: f32,
    /// The gain ride, or empty for the constant in `volume`.
    volume_curve: Track,
    fade_in: f64,
    fade_out: f64,
    quadratic_fades: bool,
    pcm: Arc<Pcm>,
    /// The frame of `pcm` the clip's first instant is: zero for the clip's
    /// own decode, and where the clip falls inside a span that stands in
    /// for it - negative where the clip begins before that span does.
    lead: i64,
    /// Timeline stretches where a mixed span under sound layers plays in
    /// this clip's place, so it is silent there; empty for that span
    /// itself and wherever no layer is.
    covered: Arc<[(f64, f64)]>,
}

/// A spec's gain keys as the engine's track.
fn track_of(keys: &[GainKey]) -> Track {
    Track::new(
        keys.iter()
            .map(|key| {
                let [x1, y1, x2, y2] = key.ease;
                Key {
                    at: key.at,
                    value: key.gain,
                    ease: Ease::new(x1, y1, x2, y2),
                }
            })
            .collect(),
    )
}

/// The one definition of a clip's gain envelope.
fn gain_at(clip: &ActiveClip, local: f64) -> f32 {
    if local < 0.0 || local > clip.duration {
        return 0.0;
    }
    let at = clip.start + local;
    if clip.covered.iter().any(|&(from, to)| at >= from && at < to) {
        return 0.0;
    }
    // The ride, where there is one, replaces the constant - the same
    // precedence the mixer's filtergraph gives it, so the window and the
    // file agree about how loud the clip is at every instant.
    let mut gain = if clip.volume_curve.is_empty() {
        f64::from(clip.volume)
    } else {
        let x = if clip.duration > 0.0 {
            (local / clip.duration).clamp(0.0, 1.0)
        } else {
            0.0
        };
        clip.volume_curve.value_at(x, f64::from(clip.volume))
    };
    if clip.fade_in > 0.0 {
        gain *= concat_media::audio::fade_gain(clip.quadratic_fades, local / clip.fade_in);
    }
    if clip.fade_out > 0.0 {
        gain *= concat_media::audio::fade_gain(
            clip.quadratic_fades,
            (clip.duration - local) / clip.fade_out,
        );
    }
    gain.max(0.0) as f32
}

enum Msg {
    SetClips(Vec<ActiveClip>),
    Play(f64),
    Pause,
    Seek(f64),
}

struct Shared {
    /// Timeline position in microseconds, written by the mix callback.
    position_micros: AtomicU64,
    /// When the clock, run on from its last write, read zero: the time of
    /// that write less the position written, in microseconds since
    /// `epoch`. One word rather than the write's time beside the
    /// position, so a reader that catches the old one of these two with
    /// the new other still reads a clock that ran on smoothly.
    origin_micros: AtomicI64,
    epoch: std::time::Instant,
    playing: AtomicBool,
    /// Silent while set: the clock runs on, the speakers say nothing. A
    /// voiceover takes the picture without the timeline's sound, so the
    /// speakers do not bleed into the microphone.
    muted: AtomicBool,
    /// The loudest sample each side has sent the speakers since the meters
    /// last looked, before the hard limit, as f32 bits: what the VU meters
    /// show. Above 1.0 is a mix that clipped.
    peak_left: AtomicU32,
    peak_right: AtomicU32,
}

/// The longest the clock runs on past the callback's last word. A callback
/// comes every buffer, ten milliseconds or so; one that has not come in a
/// tenth of a second is a stalled device, and the clock waits for it.
const COAST: std::time::Duration = std::time::Duration::from_millis(100);

impl Shared {
    fn new() -> Self {
        Self {
            position_micros: AtomicU64::new(0),
            origin_micros: AtomicI64::new(0),
            epoch: std::time::Instant::now(),
            playing: AtomicBool::new(false),
            muted: AtomicBool::new(false),
            peak_left: AtomicU32::new(0),
            peak_right: AtomicU32::new(0),
        }
    }

    /// Lands the clock at `seconds`, now.
    fn stamp(&self, seconds: f64) {
        let position = (seconds.max(0.0) * 1_000_000.0) as u64;
        self.position_micros.store(position, Ordering::Relaxed);
        self.origin_micros
            .store(self.now_micros() - position as i64, Ordering::Relaxed);
    }

    fn now_micros(&self) -> i64 {
        self.epoch.elapsed().as_micros() as i64
    }

    fn position(&self) -> f64 {
        self.position_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// The clock between callbacks: where the last one left it plus the
    /// time since, while playing, so a reader polling faster than the
    /// device's buffers sees time move smoothly and not in buffer steps.
    fn position_now(&self) -> f64 {
        let position = self.position();
        if !self.playing.load(Ordering::Relaxed) {
            return position;
        }
        let run_on = self.now_micros() - self.origin_micros.load(Ordering::Relaxed);
        (run_on as f64 / 1_000_000.0).clamp(position, position + COAST.as_secs_f64())
    }
}

/// Asks Windows for a millisecond timer while something plays. Its default
/// tick is 15.6 ms, which turns the window's frame timer into an uneven
/// 16-or-31 ms beat the picture judders to; elsewhere timers are already
/// fine-grained.
fn fine_timer(on: bool) {
    #[cfg(windows)]
    // SAFETY: plain calls with a constant period; each begin is paired with
    // one end by the caller's flag.
    unsafe {
        if on {
            windows_sys::Win32::Media::timeBeginPeriod(1);
        } else {
            windows_sys::Win32::Media::timeEndPeriod(1);
        }
    }
    #[cfg(not(windows))]
    let _ = on;
}

/// One decode waiting for a worker.
struct DecodeJob {
    spec: ClipSpec,
    project: PathBuf,
    key: String,
    cancel: Arc<AtomicBool>,
}

/// The decode queue: newest job first, because a scrub over a trim handle
/// enqueues a span per pause and the latest is the one under the playhead.
struct DecodeQueue {
    jobs: Mutex<VecDeque<DecodeJob>>,
    available: Condvar,
}

/// A mapped span plus when it was last part of the clip set, for eviction.
struct CacheEntry {
    pcm: Arc<Pcm>,
    last_used: u64,
    /// Where in its file the span lies, for spans it can stand in for; none
    /// for a continuous speed ramp, whose source clock is not linear.
    reach: Option<Reach>,
}

/// Where in its file a decoded span lies, and under what treatment: what
/// [`stand_in`] matches a span still to decode against.
#[derive(Clone, Debug)]
struct Reach {
    /// Everything about the decode but where it starts and how long it
    /// runs - the file, the stream, the speed and the filters - so that two
    /// spans with the same one are the same sound at the same instant.
    source: String,
    /// Source seconds the span starts and ends at.
    from: f64,
    to: f64,
    /// The speed the span plays at: a second of it is this many of the
    /// file's.
    speed: f64,
    /// Whether the span's samples are the file's own, only resampled: at
    /// its own speed and through no filter. A stretch of such a span is
    /// then exactly what decoding that stretch would give, so it is copied
    /// rather than decoded.
    plain: bool,
}

impl Reach {
    /// The reach of the span `spec` decodes to, or none when a speed ramp makes its clock nonlinear.
    fn of(spec: &ClipSpec) -> Option<Reach> {
        if spec.retime.is_some() || spec.bus.is_some() {
            return None;
        }
        let speed = concat_media::audio::clamp_speed(spec.speed);
        Some(Reach {
            source: format!(
                "{}\0{:?}\0{:016x}\0{}\0{}",
                spec.path,
                spec.audio_stream,
                speed.to_bits(),
                spec.preserve_pitch,
                spec.chain
            ),
            from: spec.source_start,
            to: spec.source_start + spec.duration * speed,
            speed,
            plain: concat_media::audio::speed_filters(speed, spec.preserve_pitch).is_empty()
                && spec.chain.is_empty(),
        })
    }

    /// The frame of this span's samples where `other`'s first instant is.
    fn lead_of(&self, other: &Reach) -> i64 {
        ((other.from - self.from) / self.speed * f64::from(PCM_RATE)).round() as i64
    }
}

/// Half a sample, in seconds: how far apart two source positions may be
/// and still be the same instant, so the arithmetic of a split cannot
/// leave a span a hair outside the one it was cut from.
const HAIR: f64 = 0.5 / PCM_RATE as f64;

/// The decoded span that plays for `spec` while its own decode is still to
/// come, and the frame of it where `spec` begins.
///
/// Any span from the same file under the same treatment will do: what is
/// left of a clip after a split, a trim inwards, an undo, a second use of
/// the same recording - each is a stretch of a span already mapped. Of
/// those, the one that covers most of `spec`; past where it reaches, the
/// stand-in is silent until the decode lands. None for a span already
/// decoded, a continuously retimed span, or one nothing decoded overlaps.
fn stand_in<'a>(cache: &'a HashMap<String, CacheEntry>, spec: &ClipSpec) -> Option<(&'a str, i64)> {
    let wanted = Reach::of(spec)?;
    cache
        .iter()
        .filter_map(|(key, entry)| {
            let reach = entry.reach.as_ref()?;
            if reach.source != wanted.source {
                return None;
            }
            let covered = reach.to.min(wanted.to) - reach.from.max(wanted.from);
            (covered > HAIR).then_some((key, entry, reach, covered))
        })
        .max_by(|a, b| a.3.total_cmp(&b.3).then(a.1.last_used.cmp(&b.1.last_used)))
        .map(|(key, _, reach, _)| (key.as_str(), reach.lead_of(&wanted)))
}

/// The decoded span whose samples `spec`'s decode would be, stretch for
/// stretch, and the frame of it where `spec` begins: a plain span (see
/// [`Reach::plain`]) that covers all of `spec`. Copying from it is the
/// decode, at the speed of a copy.
fn copy_source(cache: &HashMap<String, CacheEntry>, spec: &ClipSpec) -> Option<(Arc<Pcm>, u64)> {
    let wanted = Reach::of(spec)?;
    if !wanted.plain {
        return None;
    }
    cache.values().find_map(|entry| {
        let reach = entry.reach.as_ref()?;
        let covers = reach.source == wanted.source
            && reach.from <= wanted.from + HAIR
            && reach.to >= wanted.to - HAIR;
        covers.then(|| (Arc::clone(&entry.pcm), reach.lead_of(&wanted).max(0) as u64))
    })
}

/// The audio engine: decode workers, the supervised output stream, and the
/// clock. One per app, for the app's lifetime.
pub struct Playback {
    tx: mpsc::Sender<Msg>,
    shared: Arc<Shared>,
    /// For surfacing failures; a mute app must never be a mystery.
    events: Arc<dyn PlaybackEvents>,
    /// Decoded clips by decode key. Bounded: see [`MEMORY_CACHE_CAP`].
    cache: Mutex<HashMap<String, CacheEntry>>,
    /// Monotonic use counter feeding `CacheEntry::last_used`.
    tick: AtomicU64,
    decoding: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// What the timeline currently wants audible.
    specs: Mutex<Vec<ClipSpec>>,
    /// The project whose cache the disk GC sweeps; set by `set_clips`.
    project: Mutex<Option<PathBuf>>,
    /// True while a disk sweep runs, so sweeps never pile up.
    sweeping: AtomicBool,
    queue: Arc<DecodeQueue>,
    /// The set most recently sent to the callback, for re-seeding a rebuilt
    /// stream. The supervisor reads it; `resync` writes it.
    last_active: Arc<Mutex<Vec<ActiveClip>>>,
    /// Recent callback telemetry for synchronization and latency measurements.
    callback_log: Arc<Mutex<VecDeque<CallbackMetric>>>,
    /// Injected stream failure flag for fault-injection testing.
    stream_failed: Arc<AtomicBool>,
    /// Lifetime stream build/rebuild count for supervisor recovery testing.
    rebuild_count: Arc<AtomicUsize>,
    /// Whether this holds the fine timer; see [`fine_timer`].
    fine: AtomicBool,
}

/// A playback failure the user would otherwise experience as unexplained
/// silence. Logged for the console, reported for the window's toast - the
/// difference between "audio is broken" and a bug report that names a file.
fn report(events: &dyn PlaybackEvents, message: String) {
    log::warn!("{message}");
    events.error(message);
}

/// A lock that outlives a panic on another thread. Every lock in this file
/// guards a cache, a queue or a list, and a poisoned one still holds a valid
/// value of that kind: recovering it costs at worst one stale entry, while
/// refusing it would end audio for the rest of the session.
fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Playback {
    /// Starts the audio threads: the supervised output stream, the clock
    /// publisher and the decode workers. An error is a thread that could not
    /// be spawned, which is the machine's condition and not the caller's.
    pub fn start(events: Arc<dyn PlaybackEvents>) -> Result<Arc<Playback>, String> {
        let (tx, rx) = mpsc::channel::<Msg>();
        let shared = Arc::new(Shared::new());
        let last_active: Arc<Mutex<Vec<ActiveClip>>> = Arc::new(Mutex::new(Vec::new()));
        let callback_log: Arc<Mutex<VecDeque<CallbackMetric>>> =
            Arc::new(Mutex::new(VecDeque::new()));
        let stream_failed = Arc::new(AtomicBool::new(false));
        let rebuild_count = Arc::new(AtomicUsize::new(0));

        {
            let shared = Arc::clone(&shared);
            let events = Arc::clone(&events);
            let last_active = Arc::clone(&last_active);
            let callback_log = Arc::clone(&callback_log);
            let stream_failed = Arc::clone(&stream_failed);
            let rebuild_count = Arc::clone(&rebuild_count);
            std::thread::Builder::new()
                .name("audio-output".into())
                .spawn(move || {
                    supervise_stream(
                        rx,
                        shared,
                        events,
                        last_active,
                        callback_log,
                        stream_failed,
                        rebuild_count,
                    )
                })
                .map_err(|error| format!("could not spawn the audio thread: {error}"))?;
        }

        // Position events at ~30Hz while playing, for listeners that only
        // want to know roughly where the transport is. The window does not
        // wait on these: it reads `position_now` on its own frame timer.
        {
            let shared = Arc::clone(&shared);
            let events = Arc::clone(&events);
            std::thread::Builder::new()
                .name("transport-events".into())
                .spawn(move || {
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(33));
                        if shared.playing.load(Ordering::Relaxed) {
                            events.position(shared.position());
                        }
                    }
                })
                .map_err(|error| format!("could not spawn the transport event thread: {error}"))?;
        }

        let playback = Arc::new(Playback {
            tx,
            shared,
            events,
            cache: Mutex::new(HashMap::new()),
            tick: AtomicU64::new(0),
            decoding: Mutex::new(HashMap::new()),
            specs: Mutex::new(Vec::new()),
            project: Mutex::new(None),
            sweeping: AtomicBool::new(false),
            queue: Arc::new(DecodeQueue {
                jobs: Mutex::new(VecDeque::new()),
                available: Condvar::new(),
            }),
            last_active,
            callback_log,
            stream_failed,
            rebuild_count,
            fine: AtomicBool::new(false),
        });

        for index in 0..DECODE_WORKERS {
            let this = Arc::clone(&playback);
            std::thread::Builder::new()
                .name(format!("audio-decode-{index}"))
                .spawn(move || decode_worker(&this))
                .map_err(|error| format!("could not spawn a decode worker: {error}"))?;
        }

        Ok(playback)
    }

    /// Starts playing from `position` seconds.
    pub fn play(&self, position: f64) {
        if !self.fine.swap(true, Ordering::Relaxed) {
            fine_timer(true);
        }
        self.shared.stamp(position);
        self.shared.playing.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Msg::Play(position.max(0.0)));
    }

    /// Stops playing; the position holds.
    pub fn pause(&self) {
        self.shared.playing.store(false, Ordering::Relaxed);
        if self.fine.swap(false, Ordering::Relaxed) {
            fine_timer(false);
        }
        let _ = self.tx.send(Msg::Pause);
    }

    /// Moves the playhead, playing or paused.
    pub fn seek(&self, position: f64) {
        let clamped = position.max(0.0);
        // The callback owns the clock while playing, but a paused callback
        // returns before storing - so a paused seek must land the shared
        // position itself, or the atomic keeps serving the pre-seek time.
        self.shared.stamp(clamped);
        let _ = self.tx.send(Msg::Seek(clamped));
    }

    /// The clock: timeline seconds, from the device's own sample counter.
    /// It moves a buffer at a time; see [`Playback::position_now`].
    pub fn position(&self) -> f64 {
        self.shared.position()
    }

    /// The clock as of now: [`Playback::position`] carried forward by the
    /// time since the device last reported, for drawing frames between its
    /// buffers. At most [`COAST`] ahead, so a stalled device stalls it.
    pub fn position_now(&self) -> f64 {
        self.shared.position_now()
    }

    /// Whether the transport is rolling.
    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Ordering::Relaxed)
    }

    /// The loudest sample each side has played since the last call, left
    /// and right, linear: 1.0 is full scale and above it a mix that
    /// clipped. Reading resets them: the meters' feed.
    pub fn take_levels(&self) -> (f32, f32) {
        (
            f32::from_bits(self.shared.peak_left.swap(0, Ordering::Relaxed)),
            f32::from_bits(self.shared.peak_right.swap(0, Ordering::Relaxed)),
        )
    }

    /// Silences the output, or lets it speak again. The clock is not
    /// touched: the picture plays on in time either way.
    pub fn set_muted(&self, muted: bool) {
        self.shared.muted.store(muted, Ordering::Relaxed);
    }

    /// Drains recent audio output callback records for latency/synchronization analysis.
    pub fn take_callback_metrics(&self) -> Vec<CallbackMetric> {
        locked(&self.callback_log).drain(..).collect()
    }

    /// Telemetry for clock drift analysis: returns `(epoch, origin_micros, position_micros)`.
    pub fn clock_diagnostics(&self) -> (std::time::Instant, i64, u64) {
        (
            self.shared.epoch,
            self.shared.origin_micros.load(Ordering::Relaxed),
            self.shared.position_micros.load(Ordering::Relaxed),
        )
    }

    /// For recovery testing: triggers a stream failure report to the supervisor.
    pub fn inject_stream_error(&self) {
        self.stream_failed.store(true, Ordering::Relaxed);
    }

    /// Number of times the supervisor has built or rebuilt the audio stream.
    pub fn rebuild_count(&self) -> usize {
        self.rebuild_count.load(Ordering::Relaxed)
    }

    /// Total bytes of memory-mapped WAV files currently held in the PCM cache.
    pub fn mapped_bytes(&self) -> usize {
        locked(&self.cache)
            .values()
            .map(|entry| entry.pcm.map.len())
            .sum()
    }

    /// Number of memory-mapped WAV spans currently held in the PCM cache.
    pub fn mapped_count(&self) -> usize {
        locked(&self.cache).len()
    }

    /// Replaces the audible clip set.
    ///
    /// Anything already decoded plays immediately; anything new joins the
    /// decode queue and joins the mix when it lands. `project` is the
    /// project folder, whose cache holds the PCM files.
    pub fn set_clips(self: &Arc<Self>, project: PathBuf, specs: Vec<ClipSpec>) {
        let project_changed = {
            let mut held = locked(&self.project);
            let changed = held.as_ref() != Some(&project);
            *held = Some(project.clone());
            changed
        };
        if project_changed || project.as_os_str().is_empty() {
            locked(&self.queue.jobs).clear();
            locked(&self.cache).clear();
        }
        let wanted: HashSet<String> = specs.iter().map(decode_key).collect();
        {
            let mut decoding = locked(&self.decoding);
            decoding.retain(|key, cancel| {
                let keep =
                    !project_changed && !project.as_os_str().is_empty() && wanted.contains(key);
                if !keep {
                    cancel.store(true, Ordering::Relaxed);
                }
                keep
            });
        }
        locked(&self.queue.jobs).retain(|job| !job.cancel.load(Ordering::Relaxed));
        *locked(&self.specs) = specs.clone();

        for spec in specs {
            let key = decode_key(&spec);
            if locked(&self.cache).contains_key(&key) {
                continue;
            }
            let cancel = Arc::new(AtomicBool::new(false));
            {
                let mut decoding = locked(&self.decoding);
                if decoding.contains_key(&key) {
                    continue;
                }
                decoding.insert(key.clone(), Arc::clone(&cancel));
            }
            let mut jobs = locked(&self.queue.jobs);
            jobs.push_back(DecodeJob {
                spec,
                project: project.clone(),
                key,
                cancel,
            });
            self.queue.available.notify_one();
        }

        self.resync();
        self.evict_memory();
        self.sweep_disk(project);
    }

    /// Sends the mix callback everything currently wanted *and* decoded, and
    /// remembers the set so a rebuilt stream can be seeded with it.
    fn resync(&self) {
        let specs = locked(&self.specs);
        let now = self.tick.fetch_add(1, Ordering::Relaxed) + 1;
        let mut cache = locked(&self.cache);
        // The stretches a mixed span under sound layers already covers:
        // the clips there fall silent only once it is ready to take over.
        let covered: Arc<[(f64, f64)]> = specs
            .iter()
            .filter(|spec| spec.bus.is_some() && cache.contains_key(&decode_key(spec)))
            .map(|spec| (spec.start, spec.start + spec.duration))
            .collect();
        let none: Arc<[(f64, f64)]> = Arc::from(Vec::new());
        let active: Vec<ActiveClip> = specs
            .iter()
            .filter_map(|spec| {
                // The span's own decode, or until it lands a span that
                // holds its sound already; see `stand_in`.
                let own = decode_key(spec);
                let (key, lead) = if cache.contains_key(&own) {
                    (own, 0)
                } else {
                    let (key, lead) = stand_in(&cache, spec)?;
                    (key.to_owned(), lead)
                };
                let entry = cache.get_mut(&key)?;
                entry.last_used = now;
                Some(ActiveClip {
                    start: spec.start,
                    duration: spec.duration,
                    volume: spec.volume,
                    volume_curve: track_of(&spec.volume_curve),
                    fade_in: spec.fade_in,
                    fade_out: spec.fade_out,
                    quadratic_fades: spec.quadratic_fades,
                    pcm: Arc::clone(&entry.pcm),
                    lead,
                    covered: if spec.bus.is_some() {
                        Arc::clone(&none)
                    } else {
                        Arc::clone(&covered)
                    },
                })
            })
            .collect();
        drop(cache);
        *locked(&self.last_active) = active.clone();
        let _ = self.tx.send(Msg::SetClips(active));
    }

    /// Drops mapped spans beyond [`MEMORY_CACHE_CAP`], oldest use first,
    /// never one the current clip set references. The mapping is address
    /// space and page cache, not copies - but a long session accumulates a
    /// mapping per edit of every trimmed span, and those add up.
    fn evict_memory(&self) {
        // The clip set first and the cache after, the order `resync` takes
        // them in, so the two can never hold one each and wait on the other.
        let specs = locked(&self.specs).clone();
        let mut cache = locked(&self.cache);
        // Live: every span the clip set names, and every span standing in
        // for one of them that is still to decode.
        let live: HashSet<String> = specs
            .iter()
            .flat_map(|spec| {
                let own = decode_key(spec);
                let standing = (!cache.contains_key(&own))
                    .then(|| stand_in(&cache, spec).map(|(key, _)| key.to_owned()))
                    .flatten();
                std::iter::once(own).chain(standing)
            })
            .collect();
        while cache.len() > MEMORY_CACHE_CAP {
            let doomed = cache
                .iter()
                .filter(|(key, _)| !live.contains(*key))
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone());
            match doomed {
                Some(key) => cache.remove(&key),
                // Everything over the cap is live: a genuinely enormous clip
                // set. Keeping it is correct; evicting live spans would just
                // re-decode them on the next resync.
                None => break,
            };
        }
    }

    /// Sweeps `cache/audio` down to [`DISK_CACHE_BUDGET`] on a worker
    /// thread: least-recently-modified first, never a file the current clip
    /// set is using. Old `.decoding` leftovers from crashed runs go too.
    fn sweep_disk(self: &Arc<Self>, project: PathBuf) {
        if self.sweeping.swap(true, Ordering::Relaxed) {
            return;
        }
        let this = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("audio-cache-sweep".into())
            .spawn(move || {
                let live: HashSet<String> = locked(&this.specs).iter().map(decode_key).collect();
                let directory = project.join("cache").join("audio");
                if let Some(doomed) = sweep_plan(&directory, &live, DISK_CACHE_BUDGET) {
                    for path in doomed {
                        let _ = std::fs::remove_file(path);
                    }
                }
                this.sweeping.store(false, Ordering::Relaxed);
            });
        // A machine that cannot spare a thread keeps its cache a while
        // longer; the next sweep asks again.
        if spawned.is_err() {
            self.sweeping.store(false, Ordering::Relaxed);
        }
    }
}

/// One decode worker: takes the newest queued job, skips jobs whose span the
/// timeline no longer wants, decodes, lands the result, resyncs.
fn decode_worker(playback: &Arc<Playback>) {
    loop {
        let job = {
            let mut jobs = locked(&playback.queue.jobs);
            loop {
                // Newest first: pop from the back where set_clips pushes.
                match jobs.pop_back() {
                    Some(job) => break job,
                    None => jobs = playback.queue.available.wait(jobs).unwrap(),
                }
            }
        };

        // A span queued by an edit that has since been edited again is dead
        // weight; skip it before paying for FFmpeg.
        let wanted = playback
            .specs
            .lock()
            .unwrap()
            .iter()
            .any(|spec| decode_key(spec) == job.key);
        if !wanted || job.cancel.load(Ordering::Relaxed) {
            finish_decode(playback, &job);
            continue;
        }

        let within = copy_source(&locked(&playback.cache), &job.spec);
        match decode_cancelling(&job.spec, &job.project, &job.key, within, Some(&job.cancel)) {
            Ok(pcm) => {
                let same_project = !job.project.as_os_str().is_empty()
                    && locked(&playback.project).as_ref() == Some(&job.project);
                if same_project && !job.cancel.load(Ordering::Relaxed) {
                    let now = playback.tick.fetch_add(1, Ordering::Relaxed) + 1;
                    locked(&playback.cache).insert(
                        job.key.clone(),
                        CacheEntry {
                            pcm: Arc::new(pcm),
                            last_used: now,
                            reach: Reach::of(&job.spec),
                        },
                    );
                }
            }
            Err(_) if job.cancel.load(Ordering::Relaxed) => {}
            Err(error) => report(
                playback.events.as_ref(),
                if job.spec.bus.is_some() {
                    format!("audio mix failed: {error}")
                } else {
                    format!("audio decode failed for {}: {error}", job.spec.path)
                },
            ),
        }
        finish_decode(playback, &job);
        playback.resync();
    }
}

// An obsolete job must never clear a newer job with the same key after a
// project switch; compare its token as well as its content key.
fn finish_decode(playback: &Playback, job: &DecodeJob) {
    let mut decoding = locked(&playback.decoding);
    if decoding
        .get(&job.key)
        .is_some_and(|token| Arc::ptr_eq(token, &job.cancel))
    {
        decoding.remove(&job.key);
    }
}

/// Which files a sweep of `directory` should delete to fit `budget` bytes:
/// least-recently-modified first, never a `.wav` whose key is in `live`,
/// plus any `.decoding` leftover older than a day. Returns `None` when the
/// directory cannot be read (not created yet - nothing to sweep).
fn sweep_plan(directory: &Path, live: &HashSet<String>, budget: u64) -> Option<Vec<PathBuf>> {
    let entries = std::fs::read_dir(directory).ok()?;
    let now = std::time::SystemTime::now();

    let mut doomed: Vec<PathBuf> = Vec::new();
    let mut candidates: Vec<(std::time::SystemTime, u64, PathBuf)> = Vec::new();
    let mut total: u64 = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = entry.metadata() else { continue };

        // A `.decoding` file is a crashed or torn run once it is old enough
        // that no live decode can still be writing it.
        if name.ends_with(".decoding") {
            let stale = meta
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age.as_secs() > 24 * 3600);
            if stale {
                doomed.push(path);
            }
            continue;
        }

        let Some(key) = name.strip_suffix(".wav") else {
            continue;
        };
        total += meta.len();
        if live.contains(key) {
            continue;
        }
        let modified = meta.modified().unwrap_or(now);
        candidates.push((modified, meta.len(), path));
    }

    if total > budget {
        candidates.sort_by_key(|(modified, ..)| *modified);
        for (_, size, path) in candidates {
            if total <= budget {
                break;
            }
            total = total.saturating_sub(size);
            doomed.push(path);
        }
    }
    Some(doomed)
}

/// Identity of one decoded span: everything that changes the samples.
/// Volume, fades and timeline position are deliberately absent - they apply
/// at mix time.
///
/// FNV-1a rather than the standard library's hasher, because these keys name
/// files that outlive the process: `DefaultHasher` is free to change between
/// Rust releases, and a changed hash would silently orphan every project's
/// audio cache on a toolchain upgrade.
fn decode_key(spec: &ClipSpec) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for &byte in bytes {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    // A mixed span is everything its mix is made of, gain and fades
    // included: they are baked into it, where a clip's are applied live.
    if let Some(bus) = &spec.bus {
        eat(b"bus");
        eat(&spec.start.to_bits().to_le_bytes());
        eat(&spec.duration.to_bits().to_le_bytes());
        for clip in &bus.clips {
            if clip.quadratic_fades {
                eat(b"quadratic-fades");
            }
            eat(decode_key(clip).as_bytes());
            for number in [
                clip.start,
                clip.fade_in,
                clip.fade_out,
                f64::from(clip.volume),
            ] {
                eat(&number.to_bits().to_le_bytes());
            }
            for key in &clip.volume_curve {
                for number in [key.at, key.gain].iter().chain(key.ease.iter()) {
                    eat(&number.to_bits().to_le_bytes());
                }
            }
        }
        for layer in &bus.layers {
            eat(b"layer");
            for number in [
                layer.start,
                layer.duration,
                layer.strength,
                layer.fade_in,
                layer.fade_out,
            ] {
                eat(&number.to_bits().to_le_bytes());
            }
            for key in &layer.strength_curve {
                for number in [key.at, key.gain].iter().chain(key.ease.iter()) {
                    eat(&number.to_bits().to_le_bytes());
                }
            }
            eat(layer.chain.as_bytes());
        }
        return format!("{hash:016x}");
    }
    eat(spec.path.as_bytes());
    eat(&spec.source_start.to_bits().to_le_bytes());
    eat(&spec.duration.to_bits().to_le_bytes());
    eat(&spec.speed.to_bits().to_le_bytes());
    eat(&[u8::from(spec.preserve_pitch)]);
    eat(spec.chain.as_bytes());
    if let Some(retime) = &spec.retime {
        eat(b"continuous-rubberband-v1");
        eat(&retime.duration.to_bits().to_le_bytes());
        for &(at, speed) in retime.curve.points() {
            eat(&at.to_bits().to_le_bytes());
            eat(&speed.to_bits().to_le_bytes());
        }
        if let Some(slopes) = retime.curve.slopes() {
            eat(b"smooth");
            for slope in slopes {
                eat(&slope.to_bits().to_le_bytes());
            }
        }
    }

    // Only when a stream is named, so every key from before streams were
    // named still finds its file.
    if let Some(stream) = spec.audio_stream {
        eat(b"stream");
        eat(&stream.to_le_bytes());
    }
    // Spans decoded before a late-starting stream was measured from its
    // first sample were cut in the wrong place on MPEG-TS and MTS files;
    // this retires every one of them (audit 2026-10-04, #3). The disk
    // budget sweeps the orphans. Decoder preroll also retires the short
    // MP3 spans cached before its bit reservoir was warmed at each seek.
    eat(b"named-channels-file-origin-v4");
    format!("{hash:016x}")
}

/// Decodes one clip span to the cache and maps it.
///
/// The decode mirrors the exporter's treatment of filters and speed, so
/// what the preview mixes is what the export renders. A file already in the
/// cache is reused as is - that is the point of the cache. `within` is a
/// mapped span whose samples from a frame on are this one's (see
/// [`copy_source`]): they are copied, which for the rest of a long
/// recording after a cut is a fraction of a second where the decode is
/// many.
#[cfg(test)]
fn decode(
    spec: &ClipSpec,
    project: &Path,
    key: &str,
    within: Option<(Arc<Pcm>, u64)>,
) -> Result<Pcm, String> {
    decode_cancelling(spec, project, key, within, None)
}

fn decode_cancelling(
    spec: &ClipSpec,
    project: &Path,
    key: &str,
    within: Option<(Arc<Pcm>, u64)>,
    cancel: Option<&AtomicBool>,
) -> Result<Pcm, String> {
    use concat_media::{AudioDecoder, AudioOptions, SampleFormat};
    let check_cancel = || {
        if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            Err("audio decode cancelled".to_owned())
        } else {
            Ok(())
        }
    };
    check_cancel()?;

    concat_media::audio::validate_chain(&spec.chain).map_err(|error| error.to_string())?;

    let directory = project.join("cache").join("audio");
    let destination = directory.join(format!("{key}.wav"));
    if destination.is_file() {
        return Pcm::open(&destination);
    }
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;

    if let Some(bus) = &spec.bus {
        return mix_span(spec, bus, &directory, key, &destination, cancel);
    }
    if let Some((pcm, from)) = within {
        let frames = (spec.duration * f64::from(PCM_RATE)).round() as u64;
        return write_span(&directory, key, &destination, |file, writer| {
            // In pieces, so the writer's buffer is never the whole span.
            let mut at = from;
            let end = from.saturating_add(frames).min(pcm.frames);
            while at < end {
                check_cancel()?;
                let count = (end - at).min(u64::from(PCM_RATE) * 10);
                writer.write(file, pcm.frames_bytes(at, count))?;
                at += count;
            }
            Ok(())
        });
    }

    // Speed first, then the clip's own filters - the same order the export
    // graph applies, because these are one definition, not two. The engine
    // owns what speed *means*; this function only decodes.
    let mut filters: Vec<String> = if spec.retime.is_some() {
        Vec::new()
    } else {
        concat_media::audio::speed_filters(spec.speed, spec.preserve_pitch)
    };
    if !spec.chain.is_empty() {
        filters.push(spec.chain.clone());
    }

    // A sped-up clip reads further into the file than it occupies on the
    // timeline: the source window is `duration * speed`, exactly the window
    // the exporter trims.
    let window = spec.duration * concat_media::audio::clamp_speed(spec.speed);

    let mut decoder = AudioDecoder::open(
        &spec.path,
        &AudioOptions {
            start: Some(spec.source_start),
            duration: Some(window),
            filters,
            retime: spec.retime.clone(),
            rate: PCM_RATE,
            channels: 2,
            format: SampleFormat::I16,
            stream: spec.audio_stream.map(|index| index as usize),
        },
    )
    .map_err(|error| error.to_string())?;

    write_span(&directory, key, &destination, |file, writer| {
        while let Some(frame) = decoder.next_frame().map_err(|error| error.to_string())? {
            check_cancel()?;
            writer.write(file, &decoder.bytes_of(&frame))?;
        }
        Ok(())
    })
}

/// Writes a span's cache file through `fill`, under a temporary name until
/// it is whole, and maps it: a crash or a failure mid-way leaves no file a
/// later run would take for a finished one.
fn write_span(
    directory: &Path,
    key: &str,
    destination: &Path,
    fill: impl FnOnce(&mut std::io::BufWriter<std::fs::File>, &mut WavWriter) -> Result<(), String>,
) -> Result<Pcm, String> {
    let temporary = scratch_path(directory, key);
    let result = (|| -> Result<(), String> {
        let mut file = std::io::BufWriter::new(
            std::fs::File::create(&temporary)
                .map_err(|error| format!("could not create {}: {error}", temporary.display()))?,
        );
        let mut writer = WavWriter::start(&mut file)?;
        fill(&mut file, &mut writer)?;
        writer.finish(&mut file)?;
        std::io::Write::flush(&mut file)
            .map_err(|error| format!("could not flush audio cache: {error}"))
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }

    commit_span(&temporary, destination)
}

/// Mixes a stretch under sound layers to the cache and maps it: the
/// exporter's own mix of the clips with the layers over the sum, so what
/// plays here is what the file will hold.
fn mix_span(
    spec: &ClipSpec,
    bus: &BusMix,
    directory: &Path,
    key: &str,
    destination: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<Pcm, String> {
    let clips: Vec<concat_media::audio::AudioClip> =
        bus.clips.iter().map(ClipSpec::engine).collect();
    let layers: Vec<concat_media::audio::AudioBus> =
        bus.layers.iter().map(BusSpec::engine).collect();
    let temporary = scratch_path(directory, key);
    let result = (|| -> Result<(), String> {
        let mut file = std::io::BufWriter::new(
            std::fs::File::create(&temporary)
                .map_err(|error| format!("could not create {}: {error}", temporary.display()))?,
        );
        let mut writer = WavWriter::start(&mut file)?;
        concat_media::audio::mix_pcm_cancelling(
            &clips,
            &layers,
            spec.start,
            spec.start + spec.duration,
            cancel,
            |bytes| {
                writer
                    .write(&mut file, bytes)
                    .map_err(|detail| concat_media::Error::Ffi {
                        operation: "write audio cache",
                        path: temporary.clone(),
                        detail,
                    })
            },
        )
        .map_err(|error| error.to_string())?;
        writer.finish(&mut file)?;
        std::io::Write::flush(&mut file)
            .map_err(|error| format!("could not flush audio cache: {error}"))
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    commit_span(&temporary, destination)
}

// Two windows may finish the same content key together. A completed cache
// entry is reusable, never something to overwrite while Windows maps it.
fn commit_span(temporary: &Path, destination: &Path) -> Result<Pcm, String> {
    if destination.is_file() {
        let _ = std::fs::remove_file(temporary);
        return Pcm::open(destination);
    }
    if let Err(error) = std::fs::rename(temporary, destination) {
        let _ = std::fs::remove_file(temporary);
        if !destination.is_file() {
            return Err(format!(
                "could not commit {}: {error}",
                destination.display()
            ));
        }
    }
    Pcm::open(destination)
}

fn scratch_path(directory: &Path, key: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    directory.join(format!(
        "{key}.{}-{}.wav.decoding",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Writes the RIFF/WAVE framing around 16-bit stereo samples at
/// [`PCM_RATE`]: a header with placeholder sizes first, patched once the
/// sample count is known.
struct WavWriter {
    data_bytes: u64,
}

impl WavWriter {
    fn start<W: std::io::Write + std::io::Seek>(out: &mut W) -> Result<Self, String> {
        let io = |error: std::io::Error| format!("could not write the audio cache: {error}");
        let channels: u16 = 2;
        let bits: u16 = 16;
        let block = channels * bits / 8;
        out.write_all(b"RIFF").map_err(io)?;
        out.write_all(&0u32.to_le_bytes()).map_err(io)?;
        out.write_all(b"WAVE").map_err(io)?;
        out.write_all(b"fmt ").map_err(io)?;
        out.write_all(&16u32.to_le_bytes()).map_err(io)?;
        out.write_all(&1u16.to_le_bytes()).map_err(io)?; // PCM
        out.write_all(&channels.to_le_bytes()).map_err(io)?;
        out.write_all(&PCM_RATE.to_le_bytes()).map_err(io)?;
        out.write_all(&(PCM_RATE * u32::from(block)).to_le_bytes())
            .map_err(io)?;
        out.write_all(&block.to_le_bytes()).map_err(io)?;
        out.write_all(&bits.to_le_bytes()).map_err(io)?;
        out.write_all(b"data").map_err(io)?;
        out.write_all(&0u32.to_le_bytes()).map_err(io)?;
        Ok(Self { data_bytes: 0 })
    }

    fn write<W: std::io::Write>(&mut self, out: &mut W, bytes: &[u8]) -> Result<(), String> {
        out.write_all(bytes)
            .map_err(|error| format!("could not write the audio cache: {error}"))?;
        self.data_bytes += bytes.len() as u64;
        Ok(())
    }

    fn finish<W: std::io::Write + std::io::Seek>(self, out: &mut W) -> Result<(), String> {
        use std::io::SeekFrom;
        let io = |error: std::io::Error| format!("could not finish the audio cache: {error}");
        let data = u32::try_from(self.data_bytes).unwrap_or(u32::MAX);
        out.seek(SeekFrom::Start(4)).map_err(io)?;
        out.write_all(&data.saturating_add(36).to_le_bytes())
            .map_err(io)?;
        out.seek(SeekFrom::Start(40)).map_err(io)?;
        out.write_all(&data.to_le_bytes()).map_err(io)?;
        out.flush().map_err(io)
    }
}

/// Owns the output stream for the life of the app, rebuilding it whenever
/// it dies or the default device changes.
///
/// Setup failure is a retry with backoff, not a permanent return: a machine
/// that boots Concat before its audio device is ready, or loses its only
/// device mid-session, gets sound back when the device does. A rebuilt
/// stream is seeded from the shared clock and the last clip set, so playback
/// carries straight on.
fn supervise_stream(
    rx: mpsc::Receiver<Msg>,
    shared: Arc<Shared>,
    events: Arc<dyn PlaybackEvents>,
    last_active: Arc<Mutex<Vec<ActiveClip>>>,
    callback_log: Arc<Mutex<VecDeque<CallbackMetric>>>,
    stream_failed: Arc<AtomicBool>,
    rebuild_count: Arc<AtomicUsize>,
) {
    use cpal::traits::{DeviceTrait, HostTrait};

    // The callback drains this through a try-lock: uncontended in steady
    // state (the supervisor only takes it between streams), and a missed
    // drain on a contended tick is caught on the next callback.
    let rx = Arc::new(Mutex::new(rx));
    let mut backoff = std::time::Duration::from_secs(1);
    let mut reported_missing = false;

    loop {
        let host = cpal::default_host();
        let built = host
            .default_output_device()
            .ok_or_else(|| "no audio output device".to_owned())
            .and_then(|device| {
                let name = device.name().unwrap_or_default();
                build_stream(&device, &rx, &shared, &events, &last_active, &callback_log)
                    .map(|s| (s, name))
            });

        match built {
            Ok((stream, device_name)) => {
                backoff = std::time::Duration::from_secs(1);
                reported_missing = false;
                rebuild_count.fetch_add(1, Ordering::Relaxed);
                stream_failed.store(false, Ordering::Relaxed);

                // Watch for the two reasons to rebuild: the stream reported
                // an error, or the default device is no longer the one the
                // stream was built on (headphones unplugged, output switched
                // in system settings).
                let failed = stream.failed;
                let _stream = stream.stream;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    if failed.load(Ordering::Relaxed) || stream_failed.load(Ordering::Relaxed) {
                        report(
                            events.as_ref(),
                            "audio stream failed; rebuilding".to_owned(),
                        );
                        break;
                    }
                    let current = cpal::default_host()
                        .default_output_device()
                        .and_then(|d| d.name().ok());
                    if let Some(current) = current
                        && current != device_name
                    {
                        // Informational only - switching outputs is a
                        // normal act, not an error toast.
                        log::info!("audio device changed to {current}; rebuilding");
                        break;
                    }
                }
                // `_stream` drops here, releasing the device before rebuild.
            }
            Err(error) => {
                // Once, not every retry: a machine with no sound card would
                // otherwise toast every backoff tick forever.
                if !reported_missing {
                    report(
                        events.as_ref(),
                        format!("audio output unavailable: {error}; will keep trying"),
                    );
                    reported_missing = true;
                }
                // No stream means no callback to drain the channel, and
                // every message left in it pins the PCM it names. Fold them
                // into the seed a rebuilt stream starts from instead, so
                // the next stream picks up where the window is and the
                // channel stays empty.
                if let Ok(receiver) = rx.try_lock() {
                    while let Ok(message) = receiver.try_recv() {
                        match message {
                            Msg::SetClips(next) => *locked(&last_active) = next,
                            Msg::Play(at) => {
                                shared.stamp(at);
                                shared.playing.store(true, Ordering::Relaxed);
                            }
                            Msg::Pause => shared.playing.store(false, Ordering::Relaxed),
                            Msg::Seek(at) => shared.stamp(at),
                        }
                    }
                }
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(std::time::Duration::from_secs(30));
            }
        }
    }
}

/// A built, playing stream plus its error flag.
struct BuiltStream {
    stream: cpal::Stream,
    failed: Arc<AtomicBool>,
}

/// Builds and starts one output stream on `device`, seeded with the current
/// clip set and clock so a rebuild resumes where the last stream stopped.
fn build_stream(
    device: &cpal::Device,
    rx: &Arc<Mutex<mpsc::Receiver<Msg>>>,
    shared: &Arc<Shared>,
    events: &Arc<dyn PlaybackEvents>,
    last_active: &Arc<Mutex<Vec<ActiveClip>>>,
    callback_log: &Arc<Mutex<VecDeque<CallbackMetric>>>,
) -> Result<BuiltStream, String> {
    use cpal::traits::DeviceTrait;

    let config = device
        .default_output_config()
        .map_err(|error| format!("no default output config: {error}"))?;
    let sample_format = config.sample_format();
    let config: cpal::StreamConfig = config.into();

    match sample_format {
        cpal::SampleFormat::F32 => stream_for::<f32>(
            device,
            &config,
            rx,
            shared,
            events,
            last_active,
            callback_log,
        ),
        cpal::SampleFormat::I16 => stream_for::<i16>(
            device,
            &config,
            rx,
            shared,
            events,
            last_active,
            callback_log,
        ),
        cpal::SampleFormat::U16 => stream_for::<u16>(
            device,
            &config,
            rx,
            shared,
            events,
            last_active,
            callback_log,
        ),
        other => Err(format!("unsupported sample format {other:?}")),
    }
}

fn stream_for<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    rx: &Arc<Mutex<mpsc::Receiver<Msg>>>,
    shared: &Arc<Shared>,
    events: &Arc<dyn PlaybackEvents>,
    last_active: &Arc<Mutex<Vec<ActiveClip>>>,
    callback_log: &Arc<Mutex<VecDeque<CallbackMetric>>>,
) -> Result<BuiltStream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    use cpal::traits::{DeviceTrait, StreamTrait};

    let channels = config.channels as usize;
    let step = 1.0 / f64::from(config.sample_rate.0);

    // Seed from the world as it is, not from zero: a stream rebuilt mid-
    // playback resumes the same clips at the shared clock's position.
    let mut clips: Vec<ActiveClip> = locked(last_active).clone();
    let mut playing = shared.playing.load(Ordering::Relaxed);
    let mut position = shared.position();

    let failed = Arc::new(AtomicBool::new(false));
    let rx = Arc::clone(rx);
    let shared = Arc::clone(shared);
    let callback_log = Arc::clone(callback_log);

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let mut sought = false;
                // try_lock, not lock: the audio callback must never block.
                // Contention only exists while the supervisor swaps streams,
                // and then this callback is on its way out anyway.
                if let Ok(receiver) = rx.try_lock() {
                    while let Ok(message) = receiver.try_recv() {
                        match message {
                            Msg::SetClips(next) => clips = next,
                            Msg::Play(at) => {
                                position = at;
                                playing = true;
                            }
                            Msg::Pause => playing = false,
                            Msg::Seek(at) => {
                                position = at;
                                sought = true;
                            }
                        }
                    }
                }

                if !playing {
                    // A paused seek must reach the shared clock - the playing
                    // store below never runs. Only on an actual seek, so this
                    // silent branch cannot overwrite a fresher play() store
                    // with its own stale idea of the position.
                    if sought {
                        shared.stamp(position);
                    }
                    data.fill(T::from_sample(0.0));
                    return;
                }

                let start_pos = position;
                let silent = shared.muted.load(Ordering::Relaxed);
                let (mut loudest_left, mut loudest_right) = (0.0f32, 0.0f32);
                for frame in data.chunks_mut(channels) {
                    let mut left = 0.0f32;
                    let mut right = 0.0f32;

                    for clip in &clips {
                        let local = position - clip.start;
                        if local < 0.0 || local >= clip.duration {
                            continue;
                        }
                        let gain = gain_at(clip, local);
                        if gain <= 0.0 {
                            continue;
                        }

                        // Linear interpolation across the 48k source grid -
                        // exact when the device runs at 48k, inaudibly close
                        // when it does not.
                        let read = local * f64::from(PCM_RATE);
                        let index = read.floor() as i64 + clip.lead;
                        let fraction = (read - read.floor()) as f32;
                        let pcm = &clip.pcm;
                        left += gain
                            * (pcm.sample(index, 0) * (1.0 - fraction)
                                + pcm.sample(index + 1, 0) * fraction);
                        right += gain
                            * (pcm.sample(index, 1) * (1.0 - fraction)
                                + pcm.sample(index + 1, 1) * fraction);
                    }

                    if silent {
                        left = 0.0;
                        right = 0.0;
                    }
                    loudest_left = loudest_left.max(left.abs());
                    loudest_right = loudest_right.max(right.abs());
                    // Hard limit. A mix of boosted clips can exceed full
                    // scale; wrapping would be far worse than flattening.
                    frame[0] = T::from_sample(left.clamp(-1.0, 1.0));
                    if channels > 1 {
                        frame[1] = T::from_sample(right.clamp(-1.0, 1.0));
                    }
                    for extra in frame.iter_mut().skip(2) {
                        *extra = T::from_sample(0.0);
                    }

                    position += step;
                }

                // One writer, so a load and a store keep the larger without
                // a compare loop; the meters' swap may land between them and
                // lose one buffer's peak, which no eye can see.
                for (cell, loudest) in [
                    (&shared.peak_left, loudest_left),
                    (&shared.peak_right, loudest_right),
                ] {
                    if loudest > f32::from_bits(cell.load(Ordering::Relaxed)) {
                        cell.store(loudest.to_bits(), Ordering::Relaxed);
                    }
                }

                if let Ok(mut log) = callback_log.try_lock() {
                    log.push_back(CallbackMetric {
                        instant: std::time::Instant::now(),
                        start_pos,
                        end_pos: position,
                        peak: loudest_left.max(loudest_right),
                        frames: data.len() / channels,
                    });
                    if log.len() > 4096 {
                        log.drain(0..2048);
                    }
                }

                // Land the shared clock at start_pos (the timeline position of the
                // audio buffer currently being delivered to the device). Stamping
                // the buffer-end (position) would advance the software playback
                // clock ahead of the audible stream by a full buffer duration.
                shared.stamp(start_pos);
            },
            {
                let events = Arc::clone(events);
                let failed = Arc::clone(&failed);
                move |error| {
                    // The supervisor rebuilds on this flag; the report says
                    // why the sound hiccuped.
                    report(events.as_ref(), format!("audio stream error: {error}"));
                    failed.store(true, Ordering::Relaxed);
                }
            },
            None,
        )
        .map_err(|error| format!("could not build the output stream: {error}"))?;

    stream
        .play()
        .map_err(|error| format!("could not start the output stream: {error}"))?;

    Ok(BuiltStream { stream, failed })
}

#[cfg(test)]
mod tests {
    #[test]
    fn live_fade_curve_matches_export_and_changes_only_baked_mix_cache_keys() {
        let map = memmap2::MmapMut::map_anon(4)
            .unwrap()
            .make_read_only()
            .unwrap();
        let mut clip = super::ActiveClip {
            start: 0.0,
            duration: 4.0,
            volume: 1.0,
            volume_curve: super::Track::default(),
            fade_in: 2.0,
            fade_out: 2.0,
            quadratic_fades: false,
            pcm: std::sync::Arc::new(super::Pcm {
                map,
                start: 0,
                frames: 1,
            }),
            lead: 0,
            covered: std::sync::Arc::from([]),
        };
        assert_eq!(super::gain_at(&clip, 1.0), 0.5);
        assert_eq!(super::gain_at(&clip, 3.0), 0.5);
        clip.quadratic_fades = true;
        assert_eq!(super::gain_at(&clip, 1.0), 0.25);
        assert_eq!(super::gain_at(&clip, 3.0), 0.25);
        let plain = tone_spec(std::path::Path::new("/curve-fixture.wav"), 0.0, 4.0);
        let mut curved = plain.clone();
        curved.quadratic_fades = true;
        assert_eq!(
            super::decode_key(&plain),
            super::decode_key(&curved),
            "ordinary decode excludes its live envelope"
        );
        let mix = |spec| {
            super::with_sound_layers(vec![spec], &[layer(0.0, 4.0)])
                .into_iter()
                .find(|spec| spec.bus.is_some())
                .unwrap()
        };
        assert_ne!(
            super::decode_key(&mix(plain)),
            super::decode_key(&mix(curved)),
            "a baked bus mix includes its curve"
        );
    }

    use super::{COAST, Shared, sweep_plan, wav_data_range};
    use std::sync::atomic::Ordering;

    fn assert_clean_tone_tail(played: &[f32], reference: &[f32], clip: &concat_export::ExportClip) {
        let rate = f64::from(super::PCM_RATE);
        let curve = clip.retime().unwrap();
        let begin = (2.0 * rate) as usize;
        let end = ((clip.duration - 0.1) * rate) as usize;
        let amplitude = |samples: &[f32], frequency: f64| {
            let mut real = 0.0;
            let mut imaginary = 0.0;
            for (n, &sample) in samples.iter().enumerate() {
                let angle = std::f64::consts::TAU * frequency * n as f64 / rate;
                real += f64::from(sample) * angle.cos();
                imaginary += f64::from(sample) * angle.sin();
            }
            2.0 * real.hypot(imaginary) / samples.len() as f64
        };
        // Pitch preservation may change phase. Test what is audible:
        // no periodic loss of either tone's level, and no click impulses.
        for at in (begin..end - 4800).step_by(4800) {
            let t = at as f64 / rate;
            let source =
                clip.source_start + clip.duration * curve.consumed(t / clip.duration) - 2.0;
            let source = (source * rate).round() as usize;
            for frequency in [439.0, 997.0] {
                let expected = amplitude(&reference[source..source + 4800], frequency);
                let actual = amplitude(&played[at..at + 4800], frequency);
                assert!(
                    (actual - expected).abs() < expected * 0.1,
                    "tone {frequency}Hz at {t}s: amplitude {actual}, expected {expected}"
                );
            }
        }
        let jump = played[begin..end]
            .windows(2)
            .map(|p| (p[1] - p[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(jump < 0.065, "click impulse in normal-speed tail: {jump}");
    }

    #[test]
    fn a_speed_ramp_returning_to_unity_keeps_the_original_samples() {
        use super::{ClipSpec, PCM_RATE, decode, decode_key};
        use concat_export::{ClipKind, ExportClip, audio_pieces};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("music.mp3");
        std::fs::write(&path, include_bytes!("../tests/data/speed-ramp-tone.mp3")).unwrap();
        let reference = decode(
            &ClipSpec {
                bus: None,
                path: path.to_string_lossy().into_owned(),
                audio_stream: None,
                start: 0.0,
                duration: 5.0,
                source_start: 2.0,
                speed: 1.0,
                preserve_pitch: true,
                retime: None,
                volume: 1.0,
                volume_curve: vec![],
                fade_in: 0.0,
                fade_out: 0.0,
                quadratic_fades: false,
                chain: String::new(),
            },
            dir.path(),
            "reference",
            None,
        )
        .unwrap();
        let reference: Vec<f32> = (0..reference.frames)
            .map(|n| reference.sample(n, 0))
            .collect();
        for (preserve_pitch, returned_speed) in [
            (true, 1.0),
            (false, 1.0),
            (true, 1.000168),
            (false, 1.000168),
        ] {
            let mut clip = ExportClip::blank(ClipKind::Audio, 0.0, 6.0, 0);
            clip.path = path.to_string_lossy().into_owned();
            clip.preserve_pitch = preserve_pitch;
            clip.speed_curve = vec![(0.0, 1.0), (0.15, 2.0), (0.3, returned_speed), (1.0, 1.0)];
            clip.speed_slopes = vec![0.0; 4];
            let pieces = audio_pieces(&clip);
            if preserve_pitch {
                assert_eq!(
                    pieces.len(),
                    1,
                    "a pitch-preserving curve must keep one continuous filter"
                );
                assert!(pieces[0].retime.is_some());
            }
            let mut played = vec![0.0; (clip.duration * f64::from(PCM_RATE)) as usize];
            for piece in pieces {
                let spec = ClipSpec {
                    bus: None,
                    path: clip.path.clone(),
                    audio_stream: None,
                    start: piece.start,
                    duration: piece.duration,
                    source_start: piece.source_start,
                    speed: piece.speed,
                    preserve_pitch,
                    retime: piece.retime.clone(),
                    volume: 1.0,
                    volume_curve: vec![],
                    fade_in: 0.0,
                    fade_out: 0.0,
                    quadratic_fades: false,
                    chain: String::new(),
                };
                let pcm = decode(&spec, dir.path(), &decode_key(&spec), None).unwrap();
                let start = (piece.start * f64::from(PCM_RATE)).round() as usize;
                let count = (piece.duration * f64::from(PCM_RATE)).round() as usize;
                for frame in 0..count.min(played.len() - start) {
                    played[start + frame] = pcm.sample(frame as u64, 0);
                }
            }
            assert_clean_tone_tail(&played, &reference, &clip);
        }
    }

    #[test]
    fn a_continuous_tempo_curve_keeps_the_source_clock() {
        use super::{ClipSpec, PCM_RATE, WavWriter, decode, decode_key};
        use concat_export::{ClipKind, ExportClip, audio_pieces};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clock.wav");
        let mut file = std::fs::File::create(&path).unwrap();
        let mut writer = WavWriter::start(&mut file).unwrap();
        let bytes: Vec<u8> = (0..PCM_RATE * 8)
            .flat_map(|frame| {
                let t = f64::from(frame) / f64::from(PCM_RATE);
                let sample = if (t as u32) % 2 == 1 {
                    (10000.0 * (std::f64::consts::TAU * 440.0 * t).sin()) as i16
                } else {
                    0
                };
                sample.to_le_bytes().repeat(2)
            })
            .collect();
        writer.write(&mut file, &bytes).unwrap();
        writer.finish(&mut file).unwrap();
        for speed in [2.0, 3.3, 0.25] {
            let mut clip = ExportClip::blank(ClipKind::Audio, 0.0, 4.0, 0);
            clip.path = path.to_string_lossy().into_owned();
            clip.speed_curve = vec![(0.0, speed), (0.25, 1.0), (1.0, 1.0)];
            clip.speed_slopes = vec![0.0; 3];
            let piece = audio_pieces(&clip).remove(0);
            let spec = ClipSpec {
                bus: None,
                path: clip.path.clone(),
                audio_stream: None,
                start: 0.0,
                duration: 4.0,
                source_start: 0.0,
                speed: piece.speed,
                preserve_pitch: true,
                retime: piece.retime,
                volume: 1.0,
                volume_curve: vec![],
                fade_in: 0.0,
                fade_out: 0.0,
                quadratic_fades: false,
                chain: String::new(),
            };
            let pcm = decode(&spec, dir.path(), &decode_key(&spec), None).unwrap();
            if speed == 2.0 {
                let onset = (2200..2800)
                    .step_by(5)
                    .find(|ms| {
                        let start = ms * 48;
                        let power: f64 = (start..start + 480)
                            .map(|n| f64::from(pcm.sample(n, 0)).powi(2))
                            .sum();
                        (power / 480.0).sqrt() > 0.1
                    })
                    .expect("the source's three-second tone must arrive");
                assert!(
                    (onset as i64 - 2500).abs() <= 50,
                    "tone arrived at {onset} ms"
                );
            }
            assert!(
                (pcm.frames as f64 / f64::from(PCM_RATE) - 4.0).abs() < 0.15,
                "speed {speed}: decoded {} frames",
                pcm.frames
            );
            let curve = clip.retime().unwrap();
            for t in [1.65, 2.65, 3.65] {
                let source = 4.0 * curve.consumed(t / 4.0);
                let sounding = source.floor() as u32 % 2 == 1;
                let start = ((t - 0.025) * f64::from(PCM_RATE)) as u64;
                let power: f64 = (start..start + 2400)
                    .map(|n| f64::from(pcm.sample(n, 0)).powi(2))
                    .sum();
                let rms = (power / 2400.0).sqrt();
                assert!(
                    if sounding { rms > 0.1 } else { rms < 0.02 },
                    "speed {speed}, timeline {t}, source {source}: RMS {rms}"
                );
            }
        }
    }

    #[test]
    fn a_flat_unity_curve_decodes_like_an_ordinary_clip() {
        use super::{ClipSpec, decode, decode_key};
        use concat_export::{ClipKind, ExportClip, audio_pieces};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("music.mp3");
        std::fs::write(&path, include_bytes!("../tests/data/speed-ramp-tone.mp3")).unwrap();
        let mut clip = ExportClip::blank(ClipKind::Audio, 0.0, 2.0, 0);
        clip.path = path.to_string_lossy().into_owned();
        clip.source_start = 1.0;
        clip.speed_curve = vec![(0.0, 1.0), (1.0, 1.0)];
        clip.speed_slopes = vec![0.0; 2];
        let piece = audio_pieces(&clip).remove(0);
        let mut spec = ClipSpec {
            bus: None,
            path: clip.path,
            audio_stream: None,
            start: 0.0,
            duration: 2.0,
            source_start: 1.0,
            speed: piece.speed,
            preserve_pitch: true,
            retime: piece.retime,
            volume: 1.0,
            volume_curve: vec![],
            fade_in: 0.0,
            fade_out: 0.0,
            quadratic_fades: false,
            chain: String::new(),
        };
        let curved = decode(&spec, dir.path(), &decode_key(&spec), None).unwrap();
        spec.retime = None;
        let ordinary = decode(&spec, dir.path(), &decode_key(&spec), None).unwrap();
        assert_eq!(curved.frames, ordinary.frames);
        for frame in 0..curved.frames {
            assert_eq!(curved.sample(frame, 0), ordinary.sample(frame, 0));
        }
    }

    #[test]
    fn an_exported_mp3_ramp_returns_to_clean_normal_speed() {
        use concat_export::{ClipKind, ExportClip, audio_pieces};
        use concat_media::{AudioDecoder, AudioOptions, SampleFormat};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("music.mp3");
        std::fs::write(&path, include_bytes!("../tests/data/speed-ramp-tone.mp3")).unwrap();
        let reference = AudioDecoder::open(
            &path,
            &AudioOptions {
                start: Some(2.0),
                duration: Some(5.0),
                format: SampleFormat::F32,
                ..Default::default()
            },
        )
        .unwrap()
        .collect_f32()
        .unwrap();
        let reference: Vec<f32> = reference.chunks_exact(2).map(|p| p[0]).collect();
        let mut clip = ExportClip::blank(ClipKind::Audio, 0.0, 4.137, 0);
        clip.path = path.to_string_lossy().into_owned();
        clip.source_start = 2.0;
        clip.speed_curve = vec![(0.0, 2.0), (0.25, 1.0), (1.0, 1.0)];
        clip.speed_slopes = vec![0.0; 3];
        let output = dir.path().join("export.m4a");
        concat_media::audio::mix_to_file(&audio_pieces(&clip), clip.duration, &output).unwrap();
        let rendered = AudioDecoder::open(
            &output,
            &AudioOptions {
                format: SampleFormat::F32,
                ..Default::default()
            },
        )
        .unwrap()
        .collect_f32()
        .unwrap();
        let rendered: Vec<f32> = rendered.chunks_exact(2).map(|p| p[0]).collect();
        assert_clean_tone_tail(&rendered, &reference, &clip);
    }

    #[test]
    fn the_clock_runs_on_between_buffers_but_not_past_a_stall() {
        let shared = Shared::new();
        shared.stamp(5.0);
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(shared.position_now(), 5.0, "a paused clock holds");

        shared.playing.store(true, Ordering::Relaxed);
        shared.stamp(5.0);
        std::thread::sleep(std::time::Duration::from_millis(20));
        let now = shared.position_now();
        assert!(now > 5.015 && now < 5.0 + COAST.as_secs_f64(), "{now}");
        assert_eq!(shared.position(), 5.0, "the device's own word is unchanged");

        std::thread::sleep(COAST + std::time::Duration::from_millis(20));
        let stalled = shared.position_now();
        assert!(
            (stalled - (5.0 + COAST.as_secs_f64())).abs() < 1e-3,
            "{stalled}"
        );

        // A reader between a callback's two stores - the new position, the
        // old origin - reads no earlier than the device's word, not back.
        shared.stamp(5.0);
        shared.position_micros.store(5_010_000, Ordering::Relaxed);
        let between = shared.position_now();
        assert!(
            (5.01..=5.01 + COAST.as_secs_f64()).contains(&between),
            "{between}"
        );
    }

    /// A minimal RIFF/WAVE: fmt chunk, then a data chunk with `samples`.
    fn wav(data_size: u32, samples: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // pipe placeholder
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes.extend_from_slice(samples);
        bytes
    }

    fn layer(start: f64, duration: f64) -> concat_media::audio::AudioBus {
        concat_media::audio::AudioBus {
            start,
            duration,
            filter_chain: "lowpass=f=60,lowpass=f=60".to_owned(),
            strength: 1.0,
            strength_curve: super::Track::default(),
            fade_in: 0.0,
            fade_out: 0.0,
        }
    }

    fn tone_spec(path: &std::path::Path, start: f64, duration: f64) -> super::ClipSpec {
        super::ClipSpec {
            path: path.to_string_lossy().into_owned(),
            audio_stream: None,
            start,
            duration,
            source_start: 0.0,
            volume: 1.0,
            volume_curve: Vec::new(),
            fade_in: 0.0,
            fade_out: 0.0,
            quadratic_fades: false,
            speed: 1.0,
            retime: None,
            bus: None,
            preserve_pitch: true,
            chain: String::new(),
        }
    }

    fn cached(
        spec: &super::ClipSpec,
        project: &std::path::Path,
    ) -> std::collections::HashMap<String, super::CacheEntry> {
        let key = super::decode_key(spec);
        let pcm = super::decode(spec, project, &key, None).expect("decodes");
        std::collections::HashMap::from([(
            key,
            super::CacheEntry {
                pcm: std::sync::Arc::new(pcm),
                last_used: 0,
                reach: super::Reach::of(spec),
            },
        )])
    }

    /// Three seconds of a rising tone, so that every instant of it sounds
    /// unlike every other and a stretch played from the wrong place shows.
    fn rising_tone(dir: &std::path::Path) -> std::path::PathBuf {
        let rate = super::PCM_RATE;
        let mut data = Vec::new();
        for index in 0..rate * 3 {
            let t = f64::from(index) / f64::from(rate);
            let phase = 2.0 * std::f64::consts::PI * (200.0 * t + 100.0 * t * t);
            let value = (0.5 * phase.sin() * 32767.0) as i16;
            data.extend_from_slice(&value.to_le_bytes());
            data.extend_from_slice(&value.to_le_bytes());
        }
        let source = dir.join("rising.wav");
        let mut file = std::fs::File::create(&source).expect("creates");
        let mut writer = super::WavWriter::start(&mut file).expect("starts");
        writer.write(&mut file, &data).expect("writes");
        writer.finish(&mut file).expect("finishes");
        source
    }

    /// Both halves of a split clip play the moment the cut is made, from
    /// the span the whole clip had decoded, at the instant each half
    /// begins - and what each half's own decode then writes, a copy of that
    /// stretch, is what decoding it afresh gives. The half after the cut
    /// used to be silent until a decode of all of it had run: seconds to
    /// minutes on a long recording.
    #[test]
    fn a_split_clip_plays_at_once_from_the_span_it_was_cut_from() {
        let dir = std::env::temp_dir().join(format!("concat-split-{}", std::process::id()));
        let fresh = dir.join("fresh");
        std::fs::create_dir_all(&fresh).expect("a folder");
        let source = rising_tone(&dir);
        let whole = tone_spec(&source, 0.0, 3.0);
        let cache = cached(&whole, &dir);

        let mut after = tone_spec(&source, 1.25, 1.75);
        after.source_start = 1.25;
        let before = tone_spec(&source, 0.0, 1.25);
        let (key, lead) = super::stand_in(&cache, &after).expect("the whole stands in");
        assert_eq!(key, super::decode_key(&whole));
        assert_eq!(lead, 60_000, "the half after the cut, from the cut on");
        assert_eq!(
            super::stand_in(&cache, &before).map(|(_, lead)| lead),
            Some(0)
        );

        for half in [&before, &after] {
            let key = super::decode_key(half);
            let within = super::copy_source(&cache, half);
            assert!(within.is_some(), "a plain stretch is copied, not decoded");
            let copied = super::decode(half, &dir, &key, within).expect("copies");
            let decoded = super::decode(half, &fresh, &key, None).expect("decodes");
            assert!(
                copied.frames.abs_diff(decoded.frames) <= 1,
                "{} frames copied, {} decoded",
                copied.frames,
                decoded.frames
            );
            let worst = (0..copied.frames.min(decoded.frames) as i64)
                .map(|frame| (copied.sample(frame, 0) - decoded.sample(frame, 0)).abs())
                .fold(0.0_f32, f32::max);
            assert!(worst < 0.001, "the copy differs from the decode by {worst}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    struct Quiet;

    impl super::PlaybackEvents for Quiet {
        fn position(&self, _: f64) {}
        fn error(&self, message: String) {
            panic!("{message}");
        }
    }

    /// The engine with no decode workers and no output: what `set_clips`
    /// hands the mix stays put to be looked at, and what it queues waits.
    fn idle(cache: std::collections::HashMap<String, super::CacheEntry>) -> super::Playback {
        let (tx, _) = std::sync::mpsc::channel();
        super::Playback {
            tx,
            shared: std::sync::Arc::new(super::Shared::new()),
            events: std::sync::Arc::new(Quiet),
            cache: std::sync::Mutex::new(cache),
            tick: std::sync::atomic::AtomicU64::new(0),
            decoding: std::sync::Mutex::new(std::collections::HashMap::new()),
            specs: std::sync::Mutex::new(Vec::new()),
            project: std::sync::Mutex::new(None),
            sweeping: std::sync::atomic::AtomicBool::new(true),
            queue: std::sync::Arc::new(super::DecodeQueue {
                jobs: std::sync::Mutex::new(std::collections::VecDeque::new()),
                available: std::sync::Condvar::new(),
            }),
            last_active: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            callback_log: Default::default(),
            stream_failed: Default::default(),
            rebuild_count: Default::default(),
            fine: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The engine itself, as the window drives it: a clip decoded, then
    /// split. Both halves are in what the mix plays the moment the split is
    /// handed over, before any decode of theirs has run, each from where it
    /// begins in the whole; the decodes are queued all the same, so each
    /// half comes to have a span of its own.
    #[test]
    fn the_engine_plays_both_halves_of_a_split_the_moment_it_is_made() {
        let dir = std::env::temp_dir().join(format!("concat-engine-split-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let source = rising_tone(&dir);
        let whole = tone_spec(&source, 0.0, 3.0);
        let playback = std::sync::Arc::new(idle(cached(&whole, &dir)));
        *super::locked(&playback.project) = Some(dir.clone());
        playback.set_clips(dir.clone(), vec![whole.clone()]);
        assert_eq!(super::locked(&playback.last_active).len(), 1);

        let before = tone_spec(&source, 0.0, 1.25);
        let mut after = tone_spec(&source, 1.25, 1.75);
        after.source_start = 1.25;
        playback.set_clips(dir.clone(), vec![before, after]);
        let playing: Vec<(f64, i64)> = super::locked(&playback.last_active)
            .iter()
            .map(|clip| (clip.start, clip.lead))
            .collect();
        assert_eq!(playing, vec![(0.0, 0), (1.25, 60_000)]);
        assert_eq!(
            super::locked(&playback.queue.jobs).len(),
            2,
            "both decodes queued"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What may stand in, and what may be copied. The same file under a
    /// filter is the same sound under that filter, so it stands in, but a
    /// filter carries what came before the cut into the stretch after it,
    /// so it is decoded all the same; another filter, another speed or a
    /// stretch the span never reached is another sound. A span reaching
    /// only part of the way plays where it reaches.
    #[test]
    fn only_the_same_sound_stands_in_and_only_a_plain_one_is_copied() {
        let dir = std::env::temp_dir().join(format!("concat-stand-in-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let source = rising_tone(&dir);
        let mut filtered = tone_spec(&source, 0.0, 2.0);
        filtered.chain = "lowpass=f=2000".to_owned();
        let cache = cached(&filtered, &dir);

        let mut half = filtered.clone();
        half.duration = 1.0;
        half.source_start = 1.0;
        assert_eq!(
            super::stand_in(&cache, &half).map(|(_, lead)| lead),
            Some(48_000)
        );
        assert!(
            super::copy_source(&cache, &half).is_none(),
            "filtered: decoded"
        );

        let mut ramp = half.clone();
        ramp.retime = Some(concat_media::audio::TempoCurve {
            curve: concat_core::SpeedCurve::new(&[(0.0, 1.0), (1.0, 2.0)]).unwrap(),
            duration: ramp.duration,
        });
        assert!(
            super::stand_in(&cache, &ramp).is_none(),
            "speed ramps must decode their own source clock"
        );
        let mut stream = half.clone();
        stream.audio_stream = Some(2);
        assert!(
            super::stand_in(&cache, &stream).is_none(),
            "another stream is another sound"
        );

        let mut other = half.clone();
        other.chain = "highpass=f=2000".to_owned();
        assert!(super::stand_in(&cache, &other).is_none(), "another filter");
        let mut faster = half.clone();
        faster.speed = 2.0;
        assert!(super::stand_in(&cache, &faster).is_none(), "another speed");
        let mut later = half.clone();
        later.source_start = 2.5;
        later.duration = 0.5;
        assert!(super::stand_in(&cache, &later).is_none(), "never reached");

        // Out past the span: it plays the second it has, then waits.
        let mut longer = filtered.clone();
        longer.source_start = 1.0;
        longer.duration = 2.0;
        assert_eq!(
            super::stand_in(&cache, &longer).map(|(_, lead)| lead),
            Some(48_000)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn edited_bus_jobs_are_cancelled_and_old_completion_cannot_clear_a_new_project_job() {
        let dir = std::env::temp_dir().join(format!("clovercut-bus-jobs-{}", std::process::id()));
        let player = std::sync::Arc::new(idle(Default::default()));
        let specs = super::with_sound_layers(
            vec![tone_spec(std::path::Path::new("tone.wav"), 0.0, 3.0)],
            &[layer(1.0, 1.0)],
        );
        player.set_clips(dir.clone(), specs.clone());
        let old = super::locked(&player.queue.jobs).pop_back().unwrap();
        player.set_clips(dir.join("another-project"), specs);
        assert!(old.cancel.load(std::sync::atomic::Ordering::Relaxed));
        let current = super::locked(&player.decoding)
            .get(&old.key)
            .cloned()
            .unwrap();
        assert!(!std::sync::Arc::ptr_eq(&current, &old.cancel));
        super::finish_decode(&player, &old);
        assert!(super::locked(&player.decoding).contains_key(&old.key));
        player.set_clips(dir, vec![]);
        assert!(current.load(std::sync::atomic::Ordering::Relaxed));
        assert!(super::locked(&player.queue.jobs).is_empty());
    }

    #[test]
    fn bus_cache_keys_include_underlying_mix_and_do_not_reuse_plain_clip_spans() {
        let mut specs = super::with_sound_layers(
            vec![tone_spec(std::path::Path::new("tone.wav"), 0.0, 3.0)],
            &[layer(1.0, 1.0)],
        );
        let span = specs.iter_mut().find(|spec| spec.bus.is_some()).unwrap();
        assert!(super::Reach::of(span).is_none());
        let key = super::decode_key(span);
        span.bus.as_mut().unwrap().clips[0].volume = 0.2;
        assert_ne!(key, super::decode_key(span));
        let key = super::decode_key(span);
        span.bus.as_mut().unwrap().layers[0].strength = 0.2;
        assert_ne!(key, super::decode_key(span));
        assert_ne!(
            super::scratch_path(std::path::Path::new("cache"), &key),
            super::scratch_path(std::path::Path::new("cache"), &key)
        );
    }

    #[test]
    fn simultaneous_cache_writers_keep_a_complete_file_and_leave_no_scratch_files() {
        let dir =
            std::env::temp_dir().join(format!("clovercut-bus-writers-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads = (0..2)
            .map(|_| {
                let dir = dir.clone();
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    super::write_span(&dir, "shared", &dir.join("shared.wav"), |file, writer| {
                        barrier.wait();
                        writer.write(file, &[0; 480 * 4])
                    })
                    .map(|pcm| pcm.frames)
                })
            })
            .collect::<Vec<_>>();
        for worker in threads {
            assert_eq!(worker.join().unwrap().unwrap(), 480);
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Times what a cut costs the half after it before it sounds, by hand:
    /// its own decode as before, against the copy and the stand-in now.
    /// `CONCAT_SPLIT_PROBE` names a long recording; `CONCAT_SPLIT_AT` the
    /// cut, in seconds (120 by default).
    #[test]
    #[ignore = "a probe"]
    fn split_timing() {
        let Some(path) = std::env::var_os("CONCAT_SPLIT_PROBE") else {
            return;
        };
        let cut: f64 = std::env::var("CONCAT_SPLIT_AT")
            .ok()
            .and_then(|at| at.parse().ok())
            .unwrap_or(120.0);
        let path = std::path::PathBuf::from(path);
        let length = concat_media::probe(&path)
            .expect("probes")
            .duration
            .expect("a length")
            .as_f64();
        let dir = std::env::temp_dir().join(format!("concat-split-timing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("fresh")).expect("a folder");
        let whole = tone_spec(&path, 0.0, length);
        let started = std::time::Instant::now();
        let cache = cached(&whole, &dir);
        eprintln!(
            "the whole {length:.0} s clip decoded in {:?}",
            started.elapsed()
        );

        let mut after = tone_spec(&path, cut, length - cut);
        after.source_start = cut;
        let key = super::decode_key(&after);
        let started = std::time::Instant::now();
        let standing = super::stand_in(&cache, &after);
        eprintln!(
            "after the cut at {cut} s: a stand-in found in {:?}",
            started.elapsed()
        );
        assert!(standing.is_some());
        let started = std::time::Instant::now();
        super::decode(&after, &dir.join("fresh"), &key, None).expect("decodes");
        eprintln!(
            "  its own decode, as every cut ran it: {:?}",
            started.elapsed()
        );
        let started = std::time::Instant::now();
        super::decode(&after, &dir, &key, super::copy_source(&cache, &after)).expect("copies");
        eprintln!("  its copy now: {:?}", started.elapsed());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Layers that touch or overlap are mixed as one stretch, apart from
    /// one further on; each stretch takes the clips with something inside
    /// it, and the clips stay in the set.
    #[test]
    fn sound_layers_are_mixed_in_stretches_of_the_clips_beneath_them() {
        let path = std::path::Path::new("a.wav");
        let specs = vec![tone_spec(path, 0.0, 2.0), tone_spec(path, 5.0, 3.0)];
        let mixed =
            super::with_sound_layers(specs, &[layer(1.0, 1.0), layer(6.0, 1.0), layer(1.5, 1.0)]);
        assert_eq!(mixed.len(), 4, "two clips and two stretches");
        let stretches: Vec<&super::ClipSpec> =
            mixed.iter().filter(|spec| spec.bus.is_some()).collect();
        assert_eq!(
            (stretches[0].start, stretches[0].duration),
            (1.0, 1.5),
            "the first two layers are one stretch"
        );
        let first = stretches[0].bus.as_ref().expect("a mix");
        assert_eq!(first.layers.len(), 2);
        assert_eq!(
            first
                .layers
                .iter()
                .map(|layer| layer.start)
                .collect::<Vec<_>>(),
            vec![1.0, 1.5],
            "heard in the order given"
        );
        assert_eq!(first.clips.len(), 1, "only the clip under it");
        assert_eq!((stretches[1].start, stretches[1].duration), (6.0, 1.0));
        assert_eq!(
            stretches[1].bus.as_ref().expect("a mix").clips[0].start,
            5.0
        );
        // No layer that does anything, nothing added.
        let mut idle = layer(0.0, 1.0);
        idle.strength = 0.0;
        assert_eq!(
            super::with_sound_layers(vec![tone_spec(path, 0.0, 2.0)], &[idle]).len(),
            1
        );
    }

    /// A stretch under a sound layer decodes to the mix with the layer
    /// over it - the tone taken away by the low-pass - for exactly its
    /// length, and silences a clip under it only where it covers.
    #[test]
    fn a_stretch_under_a_sound_layer_decodes_to_the_treated_mix() {
        let dir = std::env::temp_dir().join(format!("concat-stretch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let rate = super::PCM_RATE;
        let mut data = Vec::new();
        for index in 0..rate * 3 {
            let t = f64::from(index) / f64::from(rate);
            let value = (0.5 * (2.0 * std::f64::consts::PI * 440.0 * t).sin() * 32767.0) as i16;
            data.extend_from_slice(&value.to_le_bytes());
            data.extend_from_slice(&value.to_le_bytes());
        }
        let source = dir.join("tone.wav");
        {
            let mut file = std::fs::File::create(&source).expect("creates");
            let mut writer = super::WavWriter::start(&mut file).expect("starts");
            writer.write(&mut file, &data).expect("writes");
            writer.finish(&mut file).expect("finishes");
        }
        let specs =
            super::with_sound_layers(vec![tone_spec(&source, 0.0, 3.0)], &[layer(1.0, 1.0)]);
        let stretch = specs
            .iter()
            .find(|spec| spec.bus.is_some())
            .expect("a stretch");
        let key = super::decode_key(stretch);
        let pcm = super::decode(stretch, &dir, &key, None).expect("mixes");
        let peak = (rate / 10..rate * 9 / 10)
            .map(|frame| pcm.sample(u64::from(frame), 0).abs())
            .fold(0.0_f32, f32::max);
        let frames = pcm.frames;
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(frames, u64::from(rate), "a second, the stretch's length");
        assert!(peak < 0.05, "the tone came through at {peak}");

        let clip = super::ActiveClip {
            start: 0.0,
            duration: 3.0,
            volume: 1.0,
            volume_curve: super::Track::default(),
            fade_in: 0.0,
            fade_out: 0.0,
            quadratic_fades: false,
            pcm: std::sync::Arc::new(pcm),
            lead: 0,
            covered: std::sync::Arc::from(vec![(1.0, 2.0)]),
        };
        assert_eq!(super::gain_at(&clip, 0.5), 1.0);
        assert_eq!(super::gain_at(&clip, 1.5), 0.0, "the stretch plays here");
        assert_eq!(super::gain_at(&clip, 2.0), 1.0);
    }

    #[test]
    fn finds_the_data_chunk_past_the_headers() {
        let bytes = wav(8, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let (start, length) = wav_data_range(&bytes).expect("valid wav");
        assert_eq!(length, 8);
        assert_eq!(&bytes[start..start + length], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn a_placeholder_data_size_means_the_rest_of_the_file() {
        // What the wav muxer writes to a pipe: it cannot seek back to patch.
        let bytes = wav(u32::MAX, &[9, 9, 9, 9]);
        let (start, length) = wav_data_range(&bytes).expect("valid wav");
        assert_eq!(length, 4);
        assert_eq!(start, bytes.len() - 4);
    }

    #[test]
    fn a_stated_size_is_clamped_to_the_file() {
        // A truncated decode must read short, not out of bounds.
        let bytes = wav(1000, &[1, 2]);
        assert_eq!(wav_data_range(&bytes).map(|(_, length)| length), Some(2));
    }

    #[test]
    fn refuses_files_that_are_not_wav() {
        assert_eq!(wav_data_range(b""), None);
        assert_eq!(wav_data_range(b"RIFFxxxxJUNK"), None);
        assert_eq!(wav_data_range(&[0u8; 64]), None);
    }

    #[test]
    fn the_sweep_removes_the_oldest_dead_files_first_and_never_live_ones() {
        use std::collections::HashSet;

        let directory = std::env::temp_dir().join(format!(
            "concat-sweep-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0),
        ));
        std::fs::create_dir_all(&directory).expect("scratch dir");

        // Three 4-byte caches, written oldest-first so mtimes order them.
        for name in ["old", "live", "new"] {
            std::fs::write(directory.join(format!("{name}.wav")), [0u8; 4]).expect("writes");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        let live: HashSet<String> = ["live".to_owned()].into();

        // Budget for two files: only the oldest dead file goes; the live one
        // survives despite being older than "new".
        let doomed = sweep_plan(&directory, &live, 8).expect("plans");
        let names: Vec<_> = doomed
            .iter()
            .filter_map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .collect();
        assert_eq!(names, vec!["old.wav"]);

        // Inside budget: nothing goes.
        assert_eq!(
            sweep_plan(&directory, &live, 1024).expect("plans"),
            Vec::<std::path::PathBuf>::new()
        );

        // A directory that does not exist yet is nothing to sweep, not a
        // panic - the first decode has simply not happened.
        assert!(sweep_plan(&directory.join("missing"), &live, 8).is_none());

        let _ = std::fs::remove_dir_all(&directory);
    }
}
