// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! CapCut / Jianying draft reader and project converter.
//!
//! Decodes plaintext JSON drafts as well as modern local AES-256-GCM carrier
//! envelopes (observed in CapCut / Jianying macOS 11+ `draft_info.json`), and maps
//! tracks, segments, media assets, timing, volume, transforms and captions
//! into Clovercut's [`Project`] document model.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use aes_gcm::aead::consts::U16;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::aes::Aes256;
use aes_gcm::{AesGcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

type Aes256Gcm16 = AesGcm<Aes256, U16>;

use crate::commands::{self, Command, IdMint};
use crate::doc::DocumentSettings;
use crate::model::ranges::{
    MAX_OFFSET, MAX_SCALE, MAX_SPEED, MAX_STRETCH, MIN_CLIP_DURATION, MIN_SCALE, MIN_SPEED,
    MIN_STRETCH, MIN_TRANSITION, wrap_rotation,
};
use crate::model::{
    Background, Clip, ClipKey, ClipKind, ColorSpace, Crop, KeyEase, KeyProperty, MediaItem,
    MediaKind, Project, SpeedPoint, TextAlign, TextStyle, Timeline, Track, TrackKind,
    VideoSettings,
};
use crate::placement::TOUCH;

/// The file a draft folder's edit is in, newest builds first.
pub const DRAFT_FILES: [&str; 4] = [
    "draft_content.json",
    "draft_info.json",
    "template_draft_content.json",
    "root_draft_meta_info.json",
];

/// What a draft folder starts every media path with when the file was
/// copied into the draft: `##_draftpath_placeholder_<id>_##/...`.
const PLACEHOLDER: &str = "##_draftpath_placeholder_";

/// CapCut's times are microseconds.
const MICROS: f64 = 1_000_000.0;

/// A CapCut title's size over Concat's: CapCut's default title, 15, as
/// Concat's default title, nine hundredths of the frame's height.
/// Approximate; a title can be resized after.
const TEXT_SIZE: f64 = 0.09 / 15.0;

/// One timeline of a draft: its name and its edit.
#[derive(Debug)]
pub struct DraftTimeline {
    /// What CapCut calls it, "Timeline 01"; empty for a draft of one.
    pub name: String,
    /// The `draft_info.json` (or `draft_content.json`) of it.
    pub edit: Value,
}

/// A draft as the host read it.
#[derive(Debug)]
pub struct Draft {
    /// What CapCut calls the draft (`draft_meta_info.json`'s
    /// `draft_name`), or the folder's name.
    pub name: String,
    /// The draft's folder, which a copied file's placeholder path stands
    /// for.
    pub folder: String,
    /// Its timelines, the one CapCut opens first first.
    pub timelines: Vec<DraftTimeline>,
}

/// A draft read into a project.
pub struct Imported {
    /// The project: the draft's media, and a timeline for each of its own.
    pub project: Project,
    /// The settings the document is written with: the first timeline's
    /// frame and rate, and the draft's name.
    pub settings: DocumentSettings,
    /// What did not come across, or came across changed, once per kind.
    pub notes: Vec<String>,
}

/// 20 MiB safety limit on raw input text.
pub const CAPCUT_MAX_BYTES: usize = 20 * 1024 * 1024;

/// Twelve absolute character offsets where 4-byte ASCII slices reside in modern drafts.
const EMBEDDED_OFFSETS: [usize; 12] = [0, 7, 20, 33, 40, 47, 59, 66, 76, 89, 99, 127];

/// Encoding detected for the CapCut draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapcutEncoding {
    /// Plain unencrypted JSON object.
    Json,
    /// Independently verified embedded AES-256-GCM carrier envelope.
    EmbeddedAesGcm,
}

/// Result of decoding raw CapCut draft text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapcutDecodedDraft {
    /// Decoded UTF-8 JSON text.
    pub text: String,
    /// Which carrier encoding was detected and decoded.
    pub encoding: CapcutEncoding,
}

/// Outcome of converting a CapCut draft into a Clovercut project.
#[derive(Clone, Debug)]
pub struct CapcutImportResult {
    /// The generated Clovercut project document.
    pub project: Project,
    /// Non-fatal diagnostic warnings noting unmapped or lossy elements.
    pub warnings: Vec<String>,
}

/// Errors occurring during CapCut draft decoding or conversion.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CapcutError {
    /// No timeline of the draft has a clip this can read.
    #[error("the CapCut draft has no clip Clovercut can read")]
    Empty,
    /// Draft text exceeds the 20 MiB safety threshold.
    #[error("CapCut draft exceeds 20 MiB limit")]
    TooLarge,
    /// Draft content is not a JSON object.
    #[error("CapCut draft requires JSON object")]
    NotAnObject,
    /// JSON parsing failed.
    #[error("CapCut draft JSON parse failed: {0}")]
    JsonParse(String),
    /// Carrier structure is unrecognized, invalid ASCII, or too short.
    #[error("CapCut draft is unrecognized opaque carrier or length too short")]
    OpaqueCarrier,
    /// GCM authentication tag verification failed.
    #[error("CapCut draft authentication failed: corrupt file or unsupported version")]
    AuthFailed,
    /// Decrypted payload is not valid UTF-8.
    #[error("CapCut draft decoded text is not valid UTF-8")]
    InvalidUtf8,
    /// Draft does not contain required timeline structures.
    #[error("Not a supported CapCut draft: requires tracks and materials or valid empty timeline")]
    UnsupportedStructure,
}

/// Decodes plain object JSON or the independently verified 12-split AES-256-GCM carrier.
pub fn decode_capcut_draft(input: &str) -> Result<CapcutDecodedDraft, CapcutError> {
    if input.len() > CAPCUT_MAX_BYTES {
        return Err(CapcutError::TooLarge);
    }
    let trimmed = input.trim().trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err(CapcutError::OpaqueCarrier);
    }

    // Try parsing as JSON first
    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        if parsed.is_object() {
            return Ok(CapcutDecodedDraft {
                text: trimmed.to_owned(),
                encoding: CapcutEncoding::Json,
            });
        }
        return Err(CapcutError::NotAnObject);
    }

    // Short malformed apparent JSON should be rejected with parse error
    let min_envelope_len = EMBEDDED_OFFSETS[EMBEDDED_OFFSETS.len() - 1] + 4;
    if (trimmed.starts_with('{') || trimmed.starts_with('[')) && trimmed.len() < min_envelope_len {
        return match serde_json::from_str::<Value>(trimmed) {
            Ok(_) => Err(CapcutError::NotAnObject),
            Err(e) => Err(CapcutError::JsonParse(e.to_string())),
        };
    }

    decode_embedded(trimmed)
}

fn decode_embedded(source: &str) -> Result<CapcutDecodedDraft, CapcutError> {
    let min_len = EMBEDDED_OFFSETS[EMBEDDED_OFFSETS.len() - 1] + 4;
    if source.len() < min_len {
        return Err(CapcutError::OpaqueCarrier);
    }

    let source_bytes = source.as_bytes();
    let mut material = [0u8; 48];
    let mut body = String::with_capacity(source.len() - 48);
    let mut cursor = 0;

    for (idx, &offset) in EMBEDDED_OFFSETS.iter().enumerate() {
        if offset > source.len() || offset + 4 > source.len() {
            return Err(CapcutError::OpaqueCarrier);
        }
        body.push_str(&source[cursor..offset]);
        for ch in 0..4 {
            let b = source_bytes[offset + ch];
            if b > 0x7f {
                return Err(CapcutError::OpaqueCarrier);
            }
            material[idx * 4 + ch] = b;
        }
        cursor = offset + 4;
    }
    body.push_str(&source[cursor..]);

    body.retain(|c| !c.is_whitespace());
    while !body.len().is_multiple_of(4) {
        body.push('=');
    }

    if body.is_empty() {
        return Err(CapcutError::OpaqueCarrier);
    }

    // Check characters are standard Base64
    for b in body.bytes() {
        if !(b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=') {
            return Err(CapcutError::OpaqueCarrier);
        }
    }

    let decoded_bytes = match STANDARD.decode(&body) {
        Ok(b) if b.len() >= 16 => b,
        _ => return Err(CapcutError::OpaqueCarrier),
    };

    let key = &material[..32];
    let iv = &material[32..48];

    let cipher = Aes256Gcm16::new_from_slice(key).map_err(|_| CapcutError::AuthFailed)?;
    let nonce = Nonce::<U16>::from_slice(iv);

    let plaintext = cipher
        .decrypt(nonce, decoded_bytes.as_slice())
        .map_err(|_| CapcutError::AuthFailed)?;

    let text_str = String::from_utf8(plaintext).map_err(|_| CapcutError::InvalidUtf8)?;
    let normalized = text_str.trim().trim_start_matches('\u{feff}').trim();

    let parsed: Value =
        serde_json::from_str(normalized).map_err(|e| CapcutError::JsonParse(e.to_string()))?;
    if !parsed.is_object() {
        return Err(CapcutError::NotAnObject);
    }

    Ok(CapcutDecodedDraft {
        text: normalized.to_owned(),
        encoding: CapcutEncoding::EmbeddedAesGcm,
    })
}

/// Imports and converts a CapCut draft (raw JSON or encrypted carrier) into a Clovercut [`Project`].
pub fn import_capcut_draft(input: &str) -> Result<CapcutImportResult, CapcutError> {
    let decoded = decode_capcut_draft(input)?;
    let root: Value =
        serde_json::from_str(&decoded.text).map_err(|e| CapcutError::JsonParse(e.to_string()))?;

    let draft = root.as_object().ok_or(CapcutError::NotAnObject)?;

    let mut warnings = vec![
        "Experimental import: converted basic CapCut timeline. Verify previews, geometry and font styles.".to_owned(),
    ];

    // Detect sparse empty timeline
    let sparse_empty = draft.get("tracks").is_none()
        && draft.get("id").and_then(Value::as_str).is_some()
        && draft.get("version").and_then(Value::as_i64).unwrap_or(0) > 0
        && draft
            .get("materials")
            .and_then(Value::as_object)
            .is_some_and(|m| m.is_empty());

    let tracks_val = match draft.get("tracks") {
        Some(Value::Array(arr)) => arr.as_slice(),
        _ if sparse_empty => &[],
        _ => return Err(CapcutError::UnsupportedStructure),
    };

    let materials = draft
        .get("materials")
        .and_then(Value::as_object)
        .ok_or(CapcutError::UnsupportedStructure)?;

    // Canvas configuration
    let canvas_cfg = draft.get("canvas_config").and_then(Value::as_object);
    let raw_w = canvas_cfg
        .and_then(|c| c.get("width"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let raw_h = canvas_cfg
        .and_then(|c| c.get("height"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;

    let has_explicit_canvas = raw_w > 0 && raw_h > 0;
    let mut canvas_w = if has_explicit_canvas {
        raw_w.clamp(16, 7680)
    } else {
        1920
    };
    let mut canvas_h = if has_explicit_canvas {
        raw_h.clamp(16, 7680)
    } else {
        1080
    };

    let fps_raw = draft.get("fps").and_then(Value::as_i64).unwrap_or(30);
    let fps = if [24, 25, 30, 50, 60].contains(&fps_raw) {
        fps_raw
    } else {
        warnings.push(format!(
            "Frame rate {fps_raw} not standard, defaulted to 30 fps."
        ));
        30
    };

    let mut material_by_id = HashMap::new();
    for (category, items) in materials {
        if let Some(arr) = items.as_array() {
            for item in arr {
                if let Some(obj) = item.as_object()
                    && let Some(id_str) = obj.get("id").and_then(Value::as_str)
                {
                    material_by_id.insert(id_str.to_owned(), (category.as_str(), obj));
                }
            }
        }
    }

    let mut project = Project::new();
    let mut media_items = Vec::new();
    let mut media_id_map = HashMap::<String, String>::new(); // capcut material id -> clovercut media id
    let mut next_m_idx = 1usize;
    let mut next_c_idx = 1usize;
    let mut next_t_idx = 1usize;

    // First pass: identify canvas dimension inference if missing
    if !has_explicit_canvas {
        for (_cat, obj) in material_by_id.values() {
            let mtype = obj.get("type").and_then(Value::as_str).unwrap_or("");
            if mtype == "video" || mtype.is_empty() {
                let w = obj.get("width").and_then(Value::as_u64).unwrap_or(0) as u32;
                let h = obj.get("height").and_then(Value::as_u64).unwrap_or(0) as u32;
                if w > 0 && h > 0 {
                    canvas_w = w.clamp(16, 7680);
                    canvas_h = h.clamp(16, 7680);
                    warnings.push(format!(
                        "Canvas dimensions inferred from video asset: {canvas_w}x{canvas_h}"
                    ));
                    break;
                }
            }
        }
    }

    let mut clovercut_tracks = Vec::new();
    let mut clovercut_clips = Vec::new();

    // Track layers mapping from track_render_index
    let mut track_render_indices = Vec::new();

    for track_val in tracks_val {
        let Some(track_obj) = track_val.as_object() else {
            continue;
        };
        let track_type = track_obj
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("video");
        let (t_kind, default_name) = match track_type {
            "video" => (TrackKind::Video, "Video"),
            "audio" => (TrackKind::Audio, "Audio"),
            "text" => (TrackKind::Text, "Text"),
            _ => {
                warnings.push(format!("Skipped unsupported track type: {track_type}"));
                continue;
            }
        };

        let track_id = format!("T{}", next_t_idx);
        next_t_idx += 1;

        let segments = track_obj
            .get("segments")
            .and_then(Value::as_array)
            .map(|a| a.as_slice())
            .unwrap_or(&[]);

        let render_index = segments
            .first()
            .and_then(|s| s.get("track_render_index"))
            .and_then(Value::as_i64)
            .unwrap_or(next_t_idx as i64);
        track_render_indices.push((track_id.clone(), render_index));

        clovercut_tracks.push(Track {
            id: track_id.clone(),
            visible: true,
            muted: false,
            kind: t_kind,
            is_main: clovercut_tracks.is_empty() && t_kind == TrackKind::Video,
            extra: serde_json::Map::new(),
        });

        for seg_val in segments {
            let Some(seg) = seg_val.as_object() else {
                continue;
            };
            let Some(mat_id) = seg.get("material_id").and_then(Value::as_str) else {
                continue;
            };
            let Some(&(cat, mat_obj)) = material_by_id.get(mat_id) else {
                warnings.push(format!(
                    "Clip references missing material {mat_id}, skipped"
                ));
                continue;
            };

            let target = seg.get("target_timerange").and_then(Value::as_object);
            let source = seg.get("source_timerange").and_then(Value::as_object);

            let start_us = target
                .and_then(|t| t.get("start"))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let dur_us = target
                .and_then(|t| t.get("duration"))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let src_in_us = source
                .and_then(|s| s.get("start"))
                .and_then(Value::as_i64)
                .unwrap_or(0);

            if dur_us <= 0 {
                continue;
            }

            let start = (start_us as f64) / 1_000_000.0;
            let duration = (dur_us as f64) / 1_000_000.0;
            let source_start = (src_in_us as f64) / 1_000_000.0;

            let mut clip_kind = match t_kind {
                TrackKind::Video => ClipKind::Video,
                TrackKind::Audio => ClipKind::Audio,
                TrackKind::Text => ClipKind::Text,
                _ => ClipKind::Video,
            };

            let mut clovercut_media_id = String::new();
            let mut clip_name: String;
            let mut text_style: Option<TextStyle> = None;

            if t_kind == TrackKind::Text {
                let content_raw = mat_obj.get("content").and_then(Value::as_str).unwrap_or("");
                let text_parsed = if content_raw.trim().starts_with('{') {
                    serde_json::from_str::<Value>(content_raw)
                        .ok()
                        .and_then(|v| v.get("text").and_then(Value::as_str).map(|s| s.to_owned()))
                        .unwrap_or_else(|| content_raw.to_owned())
                } else {
                    content_raw.to_owned()
                };

                clip_name = text_parsed.chars().take(30).collect();
                if clip_name.is_empty() {
                    clip_name = "Text".to_owned();
                }

                text_style = Some(TextStyle {
                    content: text_parsed,
                    ..Default::default()
                });
            } else {
                let path = mat_obj.get("path").and_then(Value::as_str).unwrap_or("");
                let mtype = mat_obj.get("type").and_then(Value::as_str).unwrap_or("");
                let m_kind = if cat == "audios" {
                    MediaKind::Audio
                } else if ["photo", "image"].contains(&mtype) {
                    MediaKind::Image
                } else {
                    MediaKind::Video
                };

                if m_kind == MediaKind::Image {
                    clip_kind = ClipKind::Image;
                }

                let mid = if let Some(existing) = media_id_map.get(mat_id) {
                    existing.clone()
                } else {
                    let new_mid = format!("m{}", next_m_idx);
                    next_m_idx += 1;
                    media_id_map.insert(mat_id.to_owned(), new_mid.clone());

                    let m_dur_us = mat_obj.get("duration").and_then(Value::as_i64).unwrap_or(0);
                    let m_dur = if m_dur_us > 0 {
                        Some((m_dur_us as f64) / 1_000_000.0)
                    } else {
                        None
                    };
                    let m_name = mat_obj
                        .get("material_name")
                        .and_then(Value::as_str)
                        .or_else(|| mat_obj.get("name").and_then(Value::as_str))
                        .unwrap_or("Media")
                        .to_owned();

                    media_items.push(MediaItem {
                        id: new_mid.clone(),
                        path: path.to_owned(),
                        name: m_name,
                        duration: m_dur,
                        kind: m_kind,
                        width: mat_obj
                            .get("width")
                            .and_then(Value::as_u64)
                            .map(|v| v as u32),
                        height: mat_obj
                            .get("height")
                            .and_then(Value::as_u64)
                            .map(|v| v as u32),
                        frame_rate: Some(fps as f64),
                        frame_rate_fraction: Some(format!("{fps}/1")),
                        video_codec: None,
                        audio_codec: None,
                        has_audio: m_kind == MediaKind::Video || m_kind == MediaKind::Audio,
                        audio_tracks: Vec::new(),
                        placeholder: false,
                        color_range: None,
                        color_space: ColorSpace::Sdr,
                        ..Default::default()
                    });
                    new_mid
                };

                clovercut_media_id = mid;
                clip_name = mat_obj
                    .get("material_name")
                    .and_then(Value::as_str)
                    .unwrap_or(default_name)
                    .to_owned();
            }

            // Speed calculation
            let speed_val = seg
                .get("extra_material_refs")
                .and_then(Value::as_array)
                .and_then(|refs| {
                    for r in refs {
                        if let Some(r_str) = r.as_str()
                            && let Some(&("speeds", s_obj)) = material_by_id.get(r_str)
                            && let Some(sp) = s_obj.get("speed").and_then(Value::as_f64)
                        {
                            return Some(sp);
                        }
                    }
                    None
                });

            let speed = speed_val.unwrap_or_else(|| {
                if clip_kind == ClipKind::Image || clip_kind == ClipKind::Text {
                    1.0
                } else {
                    let src_dur = source
                        .and_then(|s| s.get("duration"))
                        .and_then(Value::as_i64)
                        .unwrap_or(dur_us);
                    if src_dur > 0 && dur_us > 0 {
                        ((src_dur as f64) / (dur_us as f64)).clamp(0.1, 8.0)
                    } else {
                        1.0
                    }
                }
            });

            let volume = seg
                .get("volume")
                .and_then(Value::as_f64)
                .unwrap_or(1.0)
                .clamp(0.0, 4.0);

            // Transform mapping
            let clip_obj = seg.get("clip").and_then(Value::as_object);
            let transform = clip_obj
                .and_then(|c| c.get("transform"))
                .and_then(Value::as_object);
            let scale_obj = clip_obj
                .and_then(|c| c.get("scale"))
                .and_then(Value::as_object);
            let flip_obj = clip_obj
                .and_then(|c| c.get("flip"))
                .and_then(Value::as_object);

            let raw_x = transform
                .and_then(|t| t.get("x"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let raw_y = transform
                .and_then(|t| t.get("y"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);

            // CapCut raw.x in [-1, 1] relative to center -> Clovercut offset_x is [-0.5, 0.5] fraction of frame
            // CapCut raw.y is inverted (-y)
            let offset_x = raw_x * 0.5;
            let offset_y = -raw_y * 0.5;

            let scale = scale_obj
                .and_then(|s| s.get("x"))
                .and_then(Value::as_f64)
                .unwrap_or(1.0)
                .clamp(0.05, 10.0);
            let rotation = clip_obj
                .and_then(|c| c.get("rotation"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let opacity = clip_obj
                .and_then(|c| c.get("alpha"))
                .and_then(Value::as_f64)
                .unwrap_or(1.0)
                .clamp(0.0, 1.0);

            let flip_h = flip_obj
                .and_then(|f| f.get("horizontal"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let flip_v = flip_obj
                .and_then(|f| f.get("vertical"))
                .and_then(Value::as_bool)
                .unwrap_or(false);

            let clip_id = format!("c{}", next_c_idx);
            next_c_idx += 1;

            clovercut_clips.push(Arc::new(Clip {
                id: clip_id,
                track_id: track_id.clone(),
                media_id: clovercut_media_id,
                name: clip_name,
                kind: clip_kind,
                start,
                duration,
                source_start,
                volume,
                fade_in: 0.0,
                fade_out: 0.0,
                visual_motion: Default::default(),
                scale,
                offset_x,
                offset_y,
                rotation,
                stretch_x: 1.0,
                stretch_y: 1.0,
                opacity,
                speed,
                speed_curve: None,
                keys: Vec::new(),
                flip_h,
                flip_v,
                blend: String::new(),
                crop: None,
                cutout: None,
                masks: Vec::new(),
                motion: None,
                stabilize: None,
                preserve_pitch: true,
                filters: Vec::new(),
                video_effects: Vec::new(),
                audio_stream: None,
                text: text_style,
                shape: None,
                ..Default::default()
            }));
        }
    }

    if clovercut_tracks.is_empty() {
        clovercut_tracks.push(Track {
            id: "T1".to_owned(),
            visible: true,
            muted: false,
            kind: TrackKind::Video,
            is_main: true,
            extra: serde_json::Map::new(),
        });
    }

    // Sort tracks by CapCut track_render_index descending (higher drawn on top)
    let render_map: HashMap<String, i64> = track_render_indices.into_iter().collect();
    clovercut_tracks.sort_by(|a, b| {
        let idx_a = render_map.get(&a.id).copied().unwrap_or(0);
        let idx_b = render_map.get(&b.id).copied().unwrap_or(0);
        idx_b.cmp(&idx_a)
    });

    let timeline = Timeline {
        id: "TL1".to_owned(),
        name: draft
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Timeline 1")
            .to_owned(),
        video: VideoSettings {
            width: canvas_w,
            height: canvas_h,
            rate_num: fps,
            rate_den: 1,
            color_space: ColorSpace::Sdr,
            canvas: [0, 0, 0, 255],
        },
        background: crate::model::Background::None,
        tracks: clovercut_tracks,
        clips: clovercut_clips,
        linkage: false,
        extra: serde_json::Map::new(),
    };

    project.media = media_items;
    project.timelines = vec![Arc::new(timeline)];
    project.active_timeline_id = "TL1".to_owned();

    Ok(CapcutImportResult { project, warnings })
}

/// What did not come across, counted by what it was, said once each.
#[derive(Default)]
struct Notes(BTreeMap<String, usize>);

impl Notes {
    fn note(&mut self, what: impl Into<String>) {
        *self.0.entry(what.into()).or_insert(0) += 1;
    }

    fn lines(self) -> Vec<String> {
        self.0
            .into_iter()
            .map(|(what, count)| {
                if count == 1 {
                    what
                } else {
                    format!("{what} (×{count})")
                }
            })
            .collect()
    }
}

/// Reads `draft` into a project.
pub fn import(draft: &Draft) -> Result<Imported, CapcutError> {
    let mut mint = IdMint::default();
    let mut notes = Notes::default();
    // No starter timeline: the draft's are the project's.
    let mut project = Project {
        timelines: Vec::new(),
        ..Project::default()
    };
    let mut media = MediaBin::default();
    let mut first: Option<DocumentSettings> = None;

    for (index, part) in draft.timelines.iter().enumerate() {
        let mut timeline = read_timeline(part, draft, &mut media, &mut mint, &mut notes);
        if timeline.clips.is_empty() {
            continue;
        }
        timeline.id = mint.next("TL");
        if timeline.name.is_empty() {
            timeline.name = format!("Timeline {}", index + 1);
        }
        let video = &timeline.video;
        first.get_or_insert_with(|| DocumentSettings {
            name: draft.name.clone(),
            width: video.width,
            height: video.height,
            rate_num: video.rate_num,
            rate_den: video.rate_den,
            canvas: video.canvas,
        });
        let transitions = std::mem::take(&mut media.transitions);
        let id = timeline.id.clone();
        project.timelines.push(Arc::new(timeline));
        project.active_timeline_id = id;
        // Each through the editor's own command, so it lands on a lane the
        // way one dragged on would.
        for (start, duration, package, name) in transitions {
            let placed = commands::apply(
                &mut project,
                &mut mint,
                Command::AddTransitionClip {
                    track_id: None,
                    start,
                    duration: Some(duration),
                    transition_id: package.to_owned(),
                    name,
                },
            );
            if placed.is_err() {
                notes.note("a transition that would not fit on the timeline");
            }
        }
    }
    let Some(settings) = first else {
        return Err(CapcutError::Empty);
    };
    project.active_timeline_id = project.timelines[0].id.clone();
    project.media = media.items;
    Ok(Imported {
        project,
        settings,
        notes: notes.lines(),
    })
}

/// The project's media, one item a file across every timeline, and the
/// transitions the timeline being read has found.
#[derive(Default)]
struct MediaBin {
    items: Vec<MediaItem>,
    by_path: HashMap<String, String>,
    /// `(start, duration, package, name)` of each transition on the
    /// timeline being read.
    transitions: Vec<(f64, f64, &'static str, String)>,
}

impl MediaBin {
    /// The item for `material`'s file, made the first time it is met.
    fn item_for(
        &mut self,
        material: &Value,
        kind: MediaKind,
        folder: &str,
        mint: &mut IdMint,
    ) -> String {
        let path = resolve(&text(material, "path"), folder);
        if let Some(id) = self.by_path.get(&path) {
            return id.clone();
        }
        let id = mint.next("m");
        let name = [text(material, "material_name"), text(material, "name")]
            .into_iter()
            .find(|name| !name.is_empty())
            .unwrap_or_else(|| file_name(&path));
        let side = |key: &str| {
            material
                .get(key)
                .and_then(Value::as_u64)
                .filter(|side| *side > 0)
                .and_then(|side| u32::try_from(side).ok())
        };
        let duration = micros(material.get("duration"));
        self.items.push(MediaItem {
            id: id.clone(),
            path: path.clone(),
            name,
            kind,
            duration: (kind != MediaKind::Image && duration > 0.0).then_some(duration),
            width: (kind != MediaKind::Audio).then(|| side("width")).flatten(),
            height: (kind != MediaKind::Audio).then(|| side("height")).flatten(),
            has_audio: material
                .get("has_audio")
                .and_then(Value::as_bool)
                .unwrap_or(kind == MediaKind::Audio),
            ..MediaItem::default()
        });
        self.by_path.insert(path, id.clone());
        id
    }
}

/// A material's path with a copied file's placeholder replaced by the
/// draft's folder.
fn resolve(path: &str, folder: &str) -> String {
    match path.strip_prefix(PLACEHOLDER) {
        Some(rest) => match rest.split_once("_##") {
            Some((_, tail)) => format!("{}{tail}", folder.trim_end_matches(['/', '\\'])),
            None => path.to_owned(),
        },
        None => path.to_owned(),
    }
}

fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned()
}

/// A string field, empty when absent.
fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

/// A number field.
fn number(value: Option<&Value>, key: &str) -> Option<f64> {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_f64)
}

/// CapCut microseconds as seconds.
fn micros(value: Option<&Value>) -> f64 {
    value.and_then(Value::as_f64).unwrap_or(0.0) / MICROS
}

/// A frame rate as the exact fraction the engine works in.
fn rate_of(fps: f64) -> (i64, i64) {
    for num in [24_000_i64, 30_000, 60_000] {
        if (fps - num as f64 / 1001.0).abs() < 0.005 {
            return (num, 1001);
        }
    }
    if fps > 0.0 && (fps - fps.round()).abs() < 1e-3 {
        return (fps.round() as i64, 1);
    }
    if fps > 0.0 {
        return ((fps * 1000.0).round() as i64, 1000);
    }
    (30, 1)
}

/// One of the draft's timelines, its media into `bin`, its transitions
/// into `bin.transitions` for the caller to place.
fn read_timeline(
    part: &DraftTimeline,
    draft: &Draft,
    bin: &mut MediaBin,
    mint: &mut IdMint,
    notes: &mut Notes,
) -> Timeline {
    let edit = &part.edit;
    let canvas = edit.get("canvas_config");
    let side = |key: &str, fallback: u32| {
        number(canvas, key)
            .filter(|side| *side >= 2.0)
            .map_or(fallback, |side| (side as u32) & !1)
    };
    let (rate_num, rate_den) = rate_of(edit.get("fps").and_then(Value::as_f64).unwrap_or(30.0));
    let mut timeline = Timeline {
        name: part.name.clone(),
        video: VideoSettings {
            width: side("width", 1920),
            height: side("height", 1080),
            rate_num,
            rate_den,
            ..VideoSettings::default()
        },
        // As a new project's: no fill behind the picture, and a clip's
        // sound moving with its picture.
        background: Background::None,
        linkage: true,
        ..Timeline::default()
    };

    // Every material by its id, with the list it is in.
    let mut materials: HashMap<&str, (&str, &Value)> = HashMap::new();
    if let Some(lists) = edit.get("materials").and_then(Value::as_object) {
        for (list, entries) in lists {
            for entry in entries.as_array().into_iter().flatten() {
                if let Some(id) = entry.get("id").and_then(Value::as_str) {
                    materials.insert(id, (list.as_str(), entry));
                }
            }
        }
    }
    let reader = Reader {
        materials: &materials,
        folder: &draft.folder,
    };

    let tracks = edit.get("tracks").and_then(Value::as_array);
    for track in tracks.into_iter().flatten() {
        let kind = track.get("type").and_then(Value::as_str).unwrap_or("");
        let mut segments: Vec<&Value> = track
            .get("segments")
            .and_then(Value::as_array)
            .map(|segments| segments.iter().collect())
            .unwrap_or_default();
        if !matches!(kind, "video" | "audio" | "text") {
            if !segments.is_empty() {
                notes.note(format!("CapCut {kind} lane, not carried"));
            }
            continue;
        }
        segments.sort_by(|a, b| start_of(a).total_cmp(&start_of(b)));
        let mut clips: Vec<(Clip, Option<(f64, String)>)> = segments
            .into_iter()
            .filter_map(|segment| reader.clip(segment, kind, bin, mint, notes))
            .collect();
        bin.transitions.extend(cuts(&mut clips));
        for lane in lanes(clips.into_iter().map(|(clip, _)| clip).collect()) {
            let lane_kind = lane.first().map_or(TrackKind::Video, |clip| {
                TrackKind::from_clip_kind(clip.kind, false)
            });
            let track_id = mint.next("T");
            let muted = track.get("attribute").and_then(Value::as_u64).unwrap_or(0) & 1 != 0;
            timeline.tracks.push(Track {
                id: track_id.clone(),
                muted,
                kind: lane_kind,
                ..Track::default()
            });
            for mut clip in lane {
                clip.id = mint.next("c");
                clip.track_id = track_id.clone();
                timeline.clips.push(Arc::new(clip));
            }
        }
    }
    // Retain visual stacking while putting sound below Clovercut's fixed picture lane.
    timeline
        .tracks
        .sort_by_key(|track| track.kind != TrackKind::Audio);
    if !timeline
        .tracks
        .iter()
        .any(|track| track.kind == TrackKind::Video)
    {
        let at = timeline
            .tracks
            .iter()
            .take_while(|track| track.kind == TrackKind::Audio)
            .count();
        timeline.tracks.insert(
            at,
            Track {
                id: mint.next("T"),
                kind: TrackKind::Video,
                ..Track::default()
            },
        );
    }
    if let Some(track) = timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Video)
    {
        track.is_main = true;
    }
    timeline
}

fn start_of(segment: &Value) -> f64 {
    number(segment.get("target_timerange"), "start").unwrap_or(0.0)
}

/// What a segment's references point at.
struct Reader<'a> {
    materials: &'a HashMap<&'a str, (&'a str, &'a Value)>,
    folder: &'a str,
}

impl Reader<'_> {
    /// The materials `segment` refers to beside its own, by the list each
    /// is in.
    fn extras<'v>(&'v self, segment: &'v Value) -> impl Iterator<Item = (&'v str, &'v Value)> {
        segment
            .get("extra_material_refs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter_map(|id| self.materials.get(id).copied())
    }

    /// One segment as a clip, with the transition CapCut hung on it - the
    /// one into the next clip - as its length and name. `None` when it
    /// cannot be a clip.
    fn clip(
        &self,
        segment: &Value,
        track: &str,
        bin: &mut MediaBin,
        mint: &mut IdMint,
        notes: &mut Notes,
    ) -> Option<(Clip, Option<(f64, String)>)> {
        let id = segment.get("material_id").and_then(Value::as_str)?;
        let Some(&(list, material)) = self.materials.get(id) else {
            notes.note("a clip whose media is not in the draft");
            return None;
        };
        let target = segment.get("target_timerange");
        let (start, duration) = (
            micros(target.and_then(|range| range.get("start"))).max(0.0),
            micros(target.and_then(|range| range.get("duration"))),
        );
        if duration < MIN_CLIP_DURATION {
            return None;
        }
        if segment.get("visible").and_then(Value::as_bool) == Some(false) {
            notes.note("a clip hidden in CapCut, left out");
            return None;
        }
        let kind = match (track, list) {
            ("text", _) | (_, "texts") => ClipKind::Text,
            ("audio", _) | (_, "audios") => ClipKind::Audio,
            _ if material.get("type").and_then(Value::as_str) == Some("photo") => ClipKind::Image,
            _ => ClipKind::Video,
        };
        let name = [text(material, "material_name"), text(material, "name")]
            .into_iter()
            .find(|name| !name.is_empty())
            .unwrap_or_default();
        let mut clip = Clip::blank("", "", kind, name, start, duration);
        clip.source_start = micros(
            segment
                .get("source_timerange")
                .and_then(|range| range.get("start")),
        )
        .max(0.0);
        clip.volume = segment
            .get("volume")
            .and_then(Value::as_f64)
            .unwrap_or(1.0)
            .clamp(0.0, 10.0);
        self.speed(segment, &mut clip, notes);
        self.place(segment, &mut clip);
        self.keys(segment, &mut clip, notes);
        if segment.get("reverse").and_then(Value::as_bool) == Some(true) {
            notes.note("a clip played backwards, imported playing forwards");
        }

        let mut transition = None;
        for (list, extra) in self.extras(segment) {
            match list {
                "audio_fades" => {
                    clip.fade_in = micros(extra.get("fade_in_duration")).min(duration / 2.0);
                    clip.fade_out = micros(extra.get("fade_out_duration")).min(duration / 2.0);
                }
                "transitions" => {
                    let length = micros(extra.get("duration"));
                    transition =
                        Some((if length > 0.0 { length } else { 0.5 }, text(extra, "name")));
                }
                "speeds"
                | "canvases"
                | "placeholder_infos"
                | "sound_channel_mappings"
                | "material_colors"
                | "vocal_separations"
                | "beats"
                | "loudnesses" => {}
                "material_animations" => {
                    if extra
                        .get("animations")
                        .and_then(Value::as_array)
                        .is_some_and(|animations| !animations.is_empty())
                    {
                        notes.note("CapCut in/out animation, not carried");
                    }
                }
                other => notes.note(format!("CapCut {} on a clip, not carried", plain(other))),
            }
        }

        match kind {
            ClipKind::Video | ClipKind::Image | ClipKind::Audio => {
                let media = match kind {
                    ClipKind::Audio => MediaKind::Audio,
                    ClipKind::Image => MediaKind::Image,
                    _ => MediaKind::Video,
                };
                clip.media_id = bin.item_for(material, media, self.folder, mint);
                if kind == ClipKind::Image {
                    clip.source_start = 0.0;
                    clip.speed = 1.0;
                }
                if kind != ClipKind::Audio {
                    clip.crop = crop_of(material);
                }
            }
            ClipKind::Text => {
                let style = title_of(material)?;
                clip.name = style
                    .content
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(40)
                    .collect();
                clip.text = Some(style);
            }
            _ => return None,
        }
        Some((clip, transition))
    }

    /// The clip's speed: the segment's, or its speed material's - a curve
    /// when CapCut's is one, its points over the clip's length as
    /// Concat's are, joined by straight lines.
    fn speed(&self, segment: &Value, clip: &mut Clip, notes: &mut Notes) {
        let mut speed = segment.get("speed").and_then(Value::as_f64).unwrap_or(1.0);
        if let Some((_, material)) = self.extras(segment).find(|(list, _)| *list == "speeds") {
            speed = material
                .get("speed")
                .and_then(Value::as_f64)
                .unwrap_or(speed);
            if material.get("mode").and_then(Value::as_i64).unwrap_or(0) != 0 {
                let points: Vec<SpeedPoint> = material
                    .pointer("/curve_speed/speed_points")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|point| {
                        Some(SpeedPoint {
                            at: point.get("x")?.as_f64()?.clamp(0.0, 1.0),
                            speed: point.get("y")?.as_f64()?.clamp(MIN_SPEED, MAX_SPEED),
                            slope: None,
                        })
                    })
                    .collect();
                if let Some(points) = crate::speed::normalize(&points) {
                    clip.speed = crate::speed::mean_of(&points);
                    clip.speed_curve = Some(points);
                    return;
                }
                notes
                    .note("a CapCut speed curve Clovercut could not read, imported as its average");
            }
        }
        // What the clip covers of its source says the speed best when the
        // two disagree: it is what CapCut plays.
        let source = micros(
            segment
                .get("source_timerange")
                .and_then(|range| range.get("duration")),
        );
        if source > 0.0 && clip.duration > 0.0 && (source / clip.duration - speed).abs() > 0.01 {
            speed = source / clip.duration;
        }
        if !(MIN_SPEED..=MAX_SPEED).contains(&speed) {
            notes.note(format!(
                "a speed past Concat's {MIN_SPEED}-{MAX_SPEED}x, held to it"
            ));
        }
        clip.speed = speed.clamp(MIN_SPEED, MAX_SPEED);
    }

    /// Where the picture sits: CapCut's place is in half the canvas, y
    /// up; Concat's in the whole frame, y down. Both measure the size over
    /// the picture fitted inside the frame, and turn clockwise.
    fn place(&self, segment: &Value, clip: &mut Clip) {
        let placed = segment.get("clip");
        let scale = placed.and_then(|placed| placed.get("scale"));
        let moved = placed.and_then(|placed| placed.get("transform"));
        let flip = placed.and_then(|placed| placed.get("flip"));
        let x = number(scale, "x").unwrap_or(1.0).abs().max(MIN_SCALE);
        let y = number(scale, "y").unwrap_or(x).abs().max(MIN_SCALE);
        clip.scale = x.clamp(MIN_SCALE, MAX_SCALE);
        if (y - x).abs() > 1e-6 {
            clip.stretch_y = (y / x).clamp(MIN_STRETCH, MAX_STRETCH);
        }
        clip.offset_x = (number(moved, "x").unwrap_or(0.0) / 2.0).clamp(-MAX_OFFSET, MAX_OFFSET);
        clip.offset_y = (-number(moved, "y").unwrap_or(0.0) / 2.0).clamp(-MAX_OFFSET, MAX_OFFSET);
        clip.rotation = wrap_rotation(number(placed, "rotation").unwrap_or(0.0));
        clip.opacity = number(placed, "alpha").unwrap_or(1.0).clamp(0.0, 1.0);
        let flipped = |key: &str| {
            flip.and_then(|flip| flip.get(key))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        clip.flip_h = flipped("horizontal");
        clip.flip_v = flipped("vertical");
    }

    /// The keyframes Concat has a property for, eased straight from one
    /// to the next as CapCut's default is.
    fn keys(&self, segment: &Value, clip: &mut Clip, notes: &mut Notes) {
        let length = clip.duration * MICROS;
        let lists = segment.get("common_keyframes").and_then(Value::as_array);
        for list in lists.into_iter().flatten() {
            let kind = list
                .get("property_type")
                .and_then(Value::as_str)
                .unwrap_or("");
            let (property, value): (KeyProperty, fn(f64) -> f64) = match kind {
                "KFTypePositionX" => (KeyProperty::OffsetX, |x| x / 2.0),
                "KFTypePositionY" => (KeyProperty::OffsetY, |y| -y / 2.0),
                "KFTypeScaleX" | "UNIFORM_SCALE" => (KeyProperty::Scale, |s| s),
                "KFTypeRotation" => (KeyProperty::Rotation, |r| r),
                "KFTypeAlpha" => (KeyProperty::Opacity, |a| a.clamp(0.0, 1.0)),
                "KFTypeVolume" => (KeyProperty::Volume, |v| v.max(0.0)),
                "KFTypeScaleY" => continue,
                _ => {
                    notes.note("a CapCut keyframe Concat has no property for, not carried");
                    continue;
                }
            };
            for key in list
                .get("keyframe_list")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(raw) = key
                    .get("values")
                    .and_then(Value::as_array)
                    .and_then(|values| values.first())
                    .and_then(Value::as_f64)
                else {
                    continue;
                };
                let at = key
                    .get("time_offset")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                clip.keys.push(ClipKey {
                    property,
                    at: if length > 0.0 {
                        (at / length).clamp(0.0, 1.0)
                    } else {
                        0.0
                    },
                    value: value(raw),
                    ease: KeyEase::LINEAR,
                });
            }
        }
    }
}

/// What a material list holds, in words: "hsl" as "HSL adjustment".
fn plain(list: &str) -> String {
    match list {
        "effects" | "video_effects" => "effect".to_owned(),
        "hsl" => "HSL adjustment".to_owned(),
        "chromas" => "chroma key".to_owned(),
        "smart_relights" => "relighting".to_owned(),
        "material_colors" | "color_curves" | "adjusts" => "colour adjustment".to_owned(),
        other => other.trim_end_matches('s').replace('_', " "),
    }
}

/// The crop a video material carries, as its four corners; `None` for the
/// whole picture or a corner pin Concat's square crop cannot hold.
fn crop_of(material: &Value) -> Option<Crop> {
    let corners = material.get("crop")?;
    let at = |key: &str| number(Some(corners), key);
    let (left, top) = (at("upper_left_x")?, at("upper_left_y")?);
    let (right, bottom) = (at("lower_right_x")?, at("lower_right_y")?);
    let square = (at("upper_right_x")? - right).abs() < 1e-4
        && (at("upper_right_y")? - top).abs() < 1e-4
        && (at("lower_left_x")? - left).abs() < 1e-4
        && (at("lower_left_y")? - bottom).abs() < 1e-4;
    let crop = Crop {
        left: left.clamp(0.0, 1.0),
        top: top.clamp(0.0, 1.0),
        right: (1.0 - right).clamp(0.0, 1.0),
        bottom: (1.0 - bottom).clamp(0.0, 1.0),
    };
    (square && !crop.is_none()).then_some(crop)
}

/// A text material as a title: its words, and the look of its first run.
/// CapCut has written the words three ways - a JSON object whose `text`
/// and `styles` say it all, `<size><font>[words]</font></size>` markup,
/// and plain text - and every one is read. `None` for a title with no
/// words.
fn title_of(material: &Value) -> Option<TextStyle> {
    let content = text(material, "content");
    let parsed: Option<Value> = content
        .trim()
        .starts_with('{')
        .then(|| serde_json::from_str(content.trim()).ok())
        .flatten();
    let words = match parsed.as_ref().and_then(|parsed| parsed.get("text")) {
        Some(words) => words.as_str().unwrap_or("").to_owned(),
        None => unmarked(&content),
    };
    let words = words.trim().to_owned();
    if words.is_empty() {
        return None;
    }
    let mut style = TextStyle {
        content: words,
        ..TextStyle::default()
    };
    let run = parsed
        .as_ref()
        .and_then(|parsed| parsed.get("styles"))
        .and_then(Value::as_array)
        .and_then(|styles| styles.first());
    let colour = run
        .and_then(|run| run.pointer("/fill/content/solid/color"))
        .and_then(Value::as_array)
        .and_then(|rgb| hex(rgb))
        .or_else(|| Some(text(material, "text_color")).filter(|colour| colour.starts_with('#')));
    if let Some(colour) = colour {
        style.color = colour.to_lowercase();
    }
    let size = number(run, "size").or_else(|| number(Some(material), "font_size"));
    if let Some(size) = size.filter(|size| *size > 0.0) {
        style.font_size = size * TEXT_SIZE;
    }
    let flag = |key: &str| run.and_then(|run| run.get(key)).and_then(Value::as_bool);
    style.italic = flag("italic").unwrap_or(style.italic);
    style.underline = flag("underline").unwrap_or(style.underline);
    // CapCut's titles are regular unless made bold; Concat's start bold.
    if run.is_some() {
        style.font_weight = if flag("bold") == Some(true) {
            700.0
        } else {
            400.0
        };
    }
    style.align = match material.get("alignment").and_then(Value::as_u64) {
        Some(0) => TextAlign::Left,
        Some(2) => TextAlign::Right,
        _ => TextAlign::Center,
    };
    if let Some(alpha) = number(Some(material), "global_alpha") {
        style.opacity = alpha.clamp(0.0, 1.0);
    }
    Some(style)
}

/// Markup's words: what is outside the tags, and inside its brackets when
/// it has them.
fn unmarked(content: &str) -> String {
    let mut plain = String::new();
    let mut tag = false;
    for character in content.chars() {
        match character {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => plain.push(character),
            _ => {}
        }
    }
    match (plain.find('['), plain.rfind(']')) {
        (Some(open), Some(close)) if open < close => plain[open + 1..close].to_owned(),
        _ => plain,
    }
}

/// `[r, g, b]` in `0..1` as `#rrggbb`.
fn hex(rgb: &[Value]) -> Option<String> {
    let channel = |index: usize| {
        rgb.get(index)
            .and_then(Value::as_f64)
            .map(|level| (level.clamp(0.0, 1.0) * 255.0).round() as u8)
    };
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        channel(0)?,
        channel(1)?,
        channel(2)?
    ))
}

/// The cuts each clip's transition goes over, as `(start, length, package,
/// name)`. CapCut draws a transition by overlapping the two clips by its
/// length; Concat keeps a lane's clips end to end and lays the transition
/// over the cut. So where two overlap, the cut goes in the middle of the
/// overlap - the incoming clip starts later by half of it, and its in-point
/// moves by the source that covers - and the transition spans the overlap
/// it was drawn over. Where they only touch, it is centred on the cut.
fn cuts(clips: &mut [(Clip, Option<(f64, String)>)]) -> Vec<(f64, f64, &'static str, String)> {
    let mut placed = Vec::new();
    for index in 0..clips.len().saturating_sub(1) {
        let Some((length, name)) = clips[index].1.take() else {
            continue;
        };
        let (head, tail) = clips.split_at_mut(index + 1);
        let (left, right) = (&mut head[index].0, &mut tail[0].0);
        let end = left.start + left.duration;
        if right.start - end > TOUCH {
            // A gap between them: CapCut does not draw one there.
            continue;
        }
        let overlap = end - right.start;
        let cut = if overlap > TOUCH {
            let cut = right.start + overlap / 2.0;
            let shift = cut - right.start;
            left.duration = (cut - left.start).max(MIN_CLIP_DURATION);
            right.start = cut;
            right.duration = (right.duration - shift).max(MIN_CLIP_DURATION);
            right.source_start += shift * right.speed;
            cut
        } else {
            right.start
        };
        let span = if overlap > TOUCH { overlap } else { length };
        let longest = 2.0 * left.duration.min(right.duration);
        let span = span.min(longest).max(MIN_TRANSITION);
        let package = package_for(&name);
        let label = if name.is_empty() {
            "Transition".to_owned()
        } else {
            name
        };
        placed.push(((cut - span / 2.0).max(0.0), span, package, label));
    }
    placed
}

/// The Concat package nearest a CapCut transition, by the plain words in
/// its name; a dissolve for anything else, which keeps the cut's timing.
fn package_for(name: &str) -> &'static str {
    let name = name.to_lowercase();
    let has = |word: &str| name.contains(word);
    if has("black") {
        "concat.fade-black"
    } else if has("white") || has("flash") {
        "concat.fade-white"
    } else if has("blur") {
        "concat.blur-dissolve"
    } else if has("zoom") {
        "concat.zoom-transition"
    } else if has("spin") || has("rotate") {
        "concat.spin"
    } else if has("slide") {
        "concat.slide"
    } else if has("push") || has("pull") {
        "concat.push"
    } else if has("wipe") {
        "concat.linear-wipe"
    } else if has("glitch") {
        "concat.glitch-transition"
    } else {
        "concat.dissolve"
    }
}

/// A lane's clips laid so none covers another: one that would goes onto
/// a lane of its own above.
fn lanes(clips: Vec<Clip>) -> Vec<Vec<Clip>> {
    let mut lanes: Vec<Vec<Clip>> = Vec::new();
    for clip in clips {
        let fits = lanes.iter().position(|lane| {
            lane.last()
                .is_none_or(|last| clip.start >= last.start + last.duration - TOUCH)
        });
        match fits {
            Some(index) => lanes[index].push(clip),
            None => lanes.push(vec![clip]),
        }
    }
    lanes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A 9.x-shaped draft: a 9:16 timeline at 30, two cuts of one file
    /// overlapping for a transition, a photo, a title in the JSON form,
    /// music with fades and a keyframed volume, and a sticker lane.
    fn draft() -> Draft {
        let edit = json!({
            "fps": 30.0,
            "canvas_config": { "width": 1080, "height": 1920 },
            "materials": {
                "videos": [
                    { "id": "v", "type": "video", "path": "/m/a.mp4", "material_name": "a.mp4",
                      "duration": 14_000_000, "width": 1920, "height": 1080, "has_audio": true,
                      "crop": { "upper_left_x": 0.1, "upper_left_y": 0.0, "upper_right_x": 0.9,
                                "upper_right_y": 0.0, "lower_left_x": 0.1, "lower_left_y": 1.0,
                                "lower_right_x": 0.9, "lower_right_y": 1.0 } },
                    { "id": "p", "type": "photo", "path": "##_draftpath_placeholder_ABC_##/media/p.jpg",
                      "width": 800, "height": 600 }
                ],
                "audios": [{ "id": "au", "type": "music", "path": "/m/s.mp3", "name": "song",
                             "duration": 60_000_000 }],
                "texts": [{ "id": "t", "alignment": 1, "content": json!({
                    "text": "Hello there",
                    "styles": [{ "fill": { "content": { "solid": { "color": [1.0, 0.5, 0.0] } } },
                                 "range": [0, 11], "size": 15.0, "bold": true, "italic": true }]
                }).to_string() }],
                "transitions": [{ "id": "tr", "name": "Pull in", "duration": 600_000 }],
                "audio_fades": [{ "id": "f", "fade_in_duration": 1_000_000, "fade_out_duration": 2_000_000 }],
                "speeds": [
                    { "id": "s2", "type": "speed", "mode": 0, "speed": 2.0 },
                    { "id": "s3", "type": "speed", "mode": 1, "speed": 1.45, "curve_speed": {
                        "name": "Montage", "speed_points": [
                            { "x": 0.0, "y": 0.9 }, { "x": 0.5, "y": 6.8 }, { "x": 1.0, "y": 1.0 } ] } }
                ],
                "effects": [{ "id": "e", "type": "saturation" }]
            },
            "tracks": [
                { "type": "video", "segments": [
                    { "material_id": "v", "extra_material_refs": ["tr", "e", "s3"],
                      "target_timerange": { "start": 0, "duration": 4_000_000 },
                      "source_timerange": { "start": 1_000_000, "duration": 4_000_000 },
                      "clip": { "scale": { "x": 3.16, "y": 3.16 }, "transform": { "x": 0.5, "y": 0.25 },
                                "rotation": 90.0, "alpha": 0.5, "flip": { "horizontal": true, "vertical": false } } },
                    { "material_id": "v", "extra_material_refs": ["s2"],
                      "target_timerange": { "start": 3_400_000, "duration": 3_000_000 },
                      "source_timerange": { "start": 8_000_000, "duration": 6_000_000 } },
                    { "material_id": "p",
                      "target_timerange": { "start": 6_400_000, "duration": 2_000_000 } } ] },
                { "type": "text", "segments": [
                    { "material_id": "t", "target_timerange": { "start": 500_000, "duration": 2_000_000 } } ] },
                { "type": "audio", "segments": [
                    { "material_id": "au", "volume": 0.8, "extra_material_refs": ["f"],
                      "target_timerange": { "start": 0, "duration": 8_400_000 },
                      "source_timerange": { "start": 0, "duration": 8_400_000 },
                      "common_keyframes": [{ "property_type": "KFTypeVolume", "keyframe_list": [
                          { "time_offset": 0, "values": [0.0] },
                          { "time_offset": 4_200_000, "values": [1.0] } ] }] } ] },
                { "type": "sticker", "segments": [{ "material_id": "x" }] }
            ]
        });
        Draft {
            name: "Reel".into(),
            folder: "/drafts/0816".into(),
            timelines: vec![DraftTimeline {
                name: String::new(),
                edit,
            }],
        }
    }

    fn imported() -> Imported {
        import(&draft()).expect("imports")
    }

    fn clips_of(imported: &Imported, kind: ClipKind) -> Vec<Arc<Clip>> {
        imported.project.timelines[0]
            .clips
            .iter()
            .filter(|clip| clip.kind == kind)
            .cloned()
            .collect()
    }

    #[test]
    fn the_frame_rate_and_name_come_across() {
        let imported = imported();
        let settings = &imported.settings;
        assert_eq!((settings.width, settings.height), (1080, 1920));
        assert_eq!((settings.rate_num, settings.rate_den), (30, 1));
        assert_eq!(settings.name, "Reel");
        assert_eq!(rate_of(29.97), (30_000, 1001));
    }

    #[test]
    fn a_transition_overlap_becomes_a_cut_and_a_transition_clip() {
        let imported = imported();
        let videos = clips_of(&imported, ClipKind::Video);
        let (left, right) = (&videos[0], &videos[1]);
        // Overlapping 3.4s to 4.0s: the cut is at 3.7s.
        assert!((left.start + left.duration - 3.7).abs() < 1e-9);
        assert!((right.start - 3.7).abs() < 1e-9);
        // 0.3s more of the incoming clip's source is skipped, at 2x.
        assert!((right.source_start - (8.0 + 0.3 * 2.0)).abs() < 1e-9);
        assert_eq!(left.track_id, right.track_id, "end to end on one lane");
        let transitions = clips_of(&imported, ClipKind::Transition);
        assert_eq!(transitions.len(), 1);
        let transition = &transitions[0];
        assert!((transition.start - 3.4).abs() < 1e-9);
        assert!((transition.duration - 0.6).abs() < 1e-9);
        assert_ne!(
            transition.track_id, left.track_id,
            "over the cut, not on it"
        );
    }

    #[test]
    fn place_size_turn_crop_and_speed_come_across() {
        let imported = imported();
        let videos = clips_of(&imported, ClipKind::Video);
        let first = &videos[0];
        assert!((first.scale - 3.16).abs() < 1e-9);
        assert!((first.offset_x - 0.25).abs() < 1e-9);
        assert!((first.offset_y + 0.125).abs() < 1e-9, "CapCut's y is up");
        assert!((first.rotation - 90.0).abs() < 1e-9);
        assert!((first.opacity - 0.5).abs() < 1e-9);
        assert!(first.flip_h && !first.flip_v);
        let crop = first.crop.expect("a crop");
        assert!((crop.left - 0.1).abs() < 1e-9 && (crop.right - 0.1).abs() < 1e-9);
        assert!((videos[1].speed - 2.0).abs() < 1e-9);
        let curve = first.speed_curve.as_ref().expect("CapCut's curve");
        assert_eq!(curve.len(), 3);
        assert!((curve[1].speed - 6.8).abs() < 1e-9 && (curve[1].at - 0.5).abs() < 1e-9);
        assert!((first.speed - crate::speed::mean_of(curve)).abs() < 1e-9);
        assert_eq!(
            imported.project.media.len(),
            3,
            "one file, two cuts: one item"
        );
    }

    #[test]
    fn a_copied_file_is_found_in_the_draft() {
        let imported = imported();
        let photo = clips_of(&imported, ClipKind::Image).remove(0);
        let media = imported
            .project
            .media_by_id(&photo.media_id)
            .expect("media");
        assert_eq!(media.path, "/drafts/0816/media/p.jpg");
        assert_eq!(media.kind, MediaKind::Image);
    }

    #[test]
    fn titles_keep_their_words_and_look() {
        let imported = imported();
        let title = clips_of(&imported, ClipKind::Text).remove(0);
        let style = title.text.as_ref().expect("a title");
        assert_eq!(style.content, "Hello there");
        assert_eq!(style.color, "#ff8000");
        assert!(style.italic);
        assert!((style.font_weight - 700.0).abs() < 1e-9);
        assert!((style.font_size - 0.09).abs() < 1e-9);
        assert_eq!(
            unmarked("<size=15><font id=\"\">[hi you]</font></size>"),
            "hi you"
        );
    }

    #[test]
    fn sound_keeps_its_fades_volume_and_keys() {
        let imported = imported();
        let music = clips_of(&imported, ClipKind::Audio).remove(0);
        assert!((music.fade_in - 1.0).abs() < 1e-9 && (music.fade_out - 2.0).abs() < 1e-9);
        assert!((music.volume - 0.8).abs() < 1e-9);
        assert_eq!(music.keys.len(), 2);
        assert_eq!(music.keys[1].property, KeyProperty::Volume);
        assert!((music.keys[1].at - 0.5).abs() < 1e-9);
    }

    #[test]
    fn what_does_not_come_across_is_said() {
        let notes = imported().notes;
        assert!(
            notes.iter().any(|note| note.contains("sticker")),
            "{notes:?}"
        );
        assert!(
            notes.iter().any(|note| note.contains("effect")),
            "{notes:?}"
        );
    }

    #[test]
    fn the_project_saves_and_loads_whole() {
        let imported = imported();
        let document = crate::doc::to_document(&imported.settings, &imported.project);
        let read = crate::doc::from_document(&document).expect("loads");
        assert_eq!(
            read.active().clips.len(),
            imported.project.active().clips.len()
        );
        for timeline in &read.timelines {
            for track in &timeline.tracks {
                let mut lane: Vec<_> = timeline
                    .clips
                    .iter()
                    .filter(|clip| clip.track_id == track.id)
                    .collect();
                lane.sort_by(|a, b| a.start.total_cmp(&b.start));
                for pair in lane.windows(2) {
                    assert!(pair[1].start >= pair[0].start + pair[0].duration - TOUCH);
                }
            }
        }
    }

    #[test]
    fn a_draft_with_nothing_readable_is_refused() {
        let draft = Draft {
            name: String::new(),
            folder: String::new(),
            timelines: vec![DraftTimeline {
                name: String::new(),
                edit: json!({ "tracks": [{ "type": "sticker", "segments": [] }] }),
            }],
        };
        assert_eq!(import(&draft).err(), Some(CapcutError::Empty));
    }

    const KNOWN_PLAINTEXT: &str = "{\"tracks\":[],\"materials\":{},\"canvas_config\":{\"width\":640,\"height\":360},\"name\":\"synthetic known answer\"}";
    const KNOWN_CARRIER: &str = "0123IzU4567wv7PB7ea189abC6CoWBHy7cdefVJ60123MXE4567d+5y6i4k89ab0lycdefdHue9DabcdzpVgix4FTefghgXdTnoijklRGF8I+juVz/ImvyrNR6yRZG0mnop9K7b2dxceBMXzzDO6Xez5ExE37K0QXfSadgw97nrkaKdBMXsIqZzT42SY9G39Zqw6uAgTdt74UfU=";

    #[test]
    fn decodes_known_carrier_envelope() {
        let decoded = decode_capcut_draft(KNOWN_CARRIER).expect("decodes known carrier");
        assert_eq!(decoded.encoding, CapcutEncoding::EmbeddedAesGcm);
        assert_eq!(decoded.text, KNOWN_PLAINTEXT);
    }

    #[test]
    fn decodes_plain_json_draft() {
        let json_draft = r#"{"name": "test project", "tracks": [], "materials": {}}"#;
        let decoded = decode_capcut_draft(json_draft).expect("decodes plain json");
        assert_eq!(decoded.encoding, CapcutEncoding::Json);
        assert_eq!(decoded.text, json_draft);
    }

    #[test]
    fn rejects_corrupted_auth_tag() {
        let mut corrupted = KNOWN_CARRIER.to_owned();
        let len = corrupted.len();
        // Alter ciphertext byte
        corrupted.replace_range(len - 15..len - 14, "Z");
        assert!(decode_capcut_draft(&corrupted).is_err());
    }

    #[test]
    fn imports_classic_draft_into_clovercut_project() {
        let draft = r#"{
            "name": "CapCut Import Test",
            "fps": 30,
            "canvas_config": { "width": 1920, "height": 1080 },
            "materials": {
                "videos": [{
                    "id": "v1",
                    "type": "video",
                    "path": "/videos/clip.mp4",
                    "material_name": "Main Shot",
                    "duration": 10000000,
                    "width": 1920,
                    "height": 1080
                }],
                "speeds": [{ "id": "sp1", "speed": 1.5 }],
                "texts": [{
                    "id": "t1",
                    "type": "text",
                    "content": "{\"text\": \"Hello Clovercut\"}"
                }]
            },
            "tracks": [
                {
                    "type": "video",
                    "segments": [{
                        "material_id": "v1",
                        "target_timerange": { "start": 1000000, "duration": 2000000 },
                        "source_timerange": { "start": 2000000, "duration": 3000000 },
                        "extra_material_refs": ["sp1"],
                        "volume": 0.8,
                        "clip": {
                            "transform": { "x": 0.2, "y": -0.4 },
                            "scale": { "x": 1.2, "y": 1.2 },
                            "alpha": 0.9
                        }
                    }]
                },
                {
                    "type": "text",
                    "segments": [{
                        "material_id": "t1",
                        "target_timerange": { "start": 500000, "duration": 1500000 }
                    }]
                }
            ]
        }"#;

        let res = import_capcut_draft(draft).expect("imports draft successfully");
        let proj = res.project;
        assert_eq!(proj.media.len(), 1);
        assert_eq!(proj.media[0].name, "Main Shot");
        assert_eq!(proj.media[0].path, "/videos/clip.mp4");

        let tl = &proj.timelines[0];
        assert_eq!(tl.video.width, 1920);
        assert_eq!(tl.video.height, 1080);
        assert_eq!(tl.video.rate_num, 30);
        assert_eq!(tl.clips.len(), 2);

        // Check video clip properties
        let vid_clip = tl.clips.iter().find(|c| c.kind == ClipKind::Video).unwrap();
        assert_eq!(vid_clip.start, 1.0);
        assert_eq!(vid_clip.duration, 2.0);
        assert_eq!(vid_clip.source_start, 2.0);
        assert_eq!(vid_clip.speed, 1.5);
        assert_eq!(vid_clip.volume, 0.8);
        assert_eq!(vid_clip.offset_x, 0.1); // 0.2 * 0.5
        assert_eq!(vid_clip.offset_y, 0.2); // -(-0.4) * 0.5
        assert_eq!(vid_clip.scale, 1.2);
        assert_eq!(vid_clip.opacity, 0.9);

        // Check caption clip
        let txt_clip = tl.clips.iter().find(|c| c.kind == ClipKind::Text).unwrap();
        assert_eq!(txt_clip.start, 0.5);
        assert_eq!(txt_clip.duration, 1.5);
        assert_eq!(txt_clip.text.as_ref().unwrap().content, "Hello Clovercut");
    }
}
