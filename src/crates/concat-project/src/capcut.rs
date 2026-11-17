// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Independent CapCut / Jianying draft decoder and project converter.
//!
//! Decodes plaintext JSON drafts as well as modern local AES-256-GCM carrier
//! envelopes (observed in CapCut / Jianying macOS 11+ `draft_info.json`), and maps
//! tracks, segments, media assets, timing, volume, transforms and captions
//! into Clovercut's [`Project`] document model.
//!
//! No proprietary code, assets, accounts or remote services are used.

use std::collections::HashMap;
use std::sync::Arc;

use aes_gcm::aead::consts::U16;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::aes::Aes256;
use aes_gcm::{AesGcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

type Aes256Gcm16 = AesGcm<Aes256, U16>;

use crate::model::{
    Clip, ClipKind, ColorSpace, MediaItem, MediaKind, Project, TextStyle, Timeline, Track,
    TrackKind, VideoSettings,
};

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

    if body.is_empty() || body.len() % 4 != 0 {
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

    // Canonical Base64 check (reject non-canonical padding/spaces)
    if STANDARD.encode(&decoded_bytes) != body {
        return Err(CapcutError::OpaqueCarrier);
    }

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
            .map_or(false, |m| m.is_empty());

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
                if let Some(obj) = item.as_object() {
                    if let Some(id_str) = obj.get("id").and_then(Value::as_str) {
                        material_by_id.insert(id_str.to_owned(), (category.as_str(), obj));
                    }
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
            if mtype == "video" || mtype == "" {
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

                let mut style = TextStyle::default();
                style.content = text_parsed;
                text_style = Some(style);
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
                        if let Some(r_str) = r.as_str() {
                            if let Some(&("speeds", s_obj)) = material_by_id.get(r_str) {
                                if let Some(sp) = s_obj.get("speed").and_then(Value::as_f64) {
                                    return Some(sp);
                                }
                            }
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

#[cfg(test)]
mod tests {
    use super::*;

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
