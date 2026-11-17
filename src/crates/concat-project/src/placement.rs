// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Where a clip may go: no clip ever covers another on its lane.
//!
//! Lanes exist so two pieces of media never share a span of one lane. A
//! clip can sit beside another, between two, or anywhere a lane is empty;
//! it never lands on top of one. Every command that places, moves or
//! lengthens a clip asks these, and so does the window's drag, so what the
//! drag shows is what the drop does.

use std::collections::HashSet;

use crate::commands::ClipMove;
use crate::model::Timeline;

/// How far two clips may run into each other and still count as touching:
/// float seconds that should meet exactly sometimes miss by a hair.
pub const TOUCH: f64 = 1e-6;

/// Whether `[a_start, a_end)` and `[b_start, b_end)` share more than an edge.
fn covers(a_start: f64, a_end: f64, b_start: f64, b_end: f64) -> bool {
    a_start < b_end - TOUCH && b_start < a_end - TOUCH
}

/// Where a dragged group begins once it has moved by `shift` seconds: every
/// start moves by the one shift, and the group stops at zero as a whole, so
/// it keeps its shape instead of piling its early clips on the edge.
///
/// `starts` and the sum are f64 on purpose. The shift comes from the pointer
/// in f32, and rounding each start to f32 before adding it moves clips that
/// butt by up to ~1e-5 s late in a long timeline - more than [`TOUCH`] - so a
/// run laid end to end would look as if it covered itself and the drop would
/// be refused. Adding one shift to exact starts keeps the spacing exact.
pub fn shifted_starts(starts: &[f64], shift: f64) -> Vec<f64> {
    let earliest = starts.iter().copied().fold(f64::INFINITY, f64::min);
    let shift = if earliest.is_finite() {
        shift.max(-earliest)
    } else {
        shift
    };
    starts
        .iter()
        .map(|start| (start + shift).max(0.0))
        .collect()
}

impl Timeline {
    /// Whether `[start, start + duration)` on `track_id` is clear of every
    /// clip but the ones in `ignore`.
    pub fn is_free(&self, track_id: &str, start: f64, duration: f64, ignore: &[&str]) -> bool {
        let end = start + duration;
        !self.clips.iter().any(|clip| {
            clip.track_id == track_id
                && !ignore.contains(&clip.id.as_str())
                && covers(start, end, clip.start, clip.start + clip.duration)
        })
    }

    /// The start nearest `start` where `duration` fits on `track_id`
    /// without covering anything but `ignore`: `start` itself when it is
    /// clear, otherwise the closest gap before or after, never before zero.
    /// There is always one, after the lane's last clip.
    pub fn nearest_free_start(
        &self,
        track_id: &str,
        start: f64,
        duration: f64,
        ignore: &[&str],
    ) -> f64 {
        let start = start.max(0.0);
        let others = self
            .clips
            .iter()
            .filter(|clip| clip.track_id == track_id && !ignore.contains(&clip.id.as_str()));
        // Every place a gap can begin: the wanted start, the lane's start,
        // just after a clip, or just before one.
        let mut candidates = vec![start, 0.0];
        for clip in others {
            candidates.push(clip.start + clip.duration);
            candidates.push(clip.start - duration);
        }
        candidates
            .into_iter()
            .filter(|&at| at >= 0.0 && self.is_free(track_id, at, duration, ignore))
            .min_by(|a, b| (a - start).abs().total_cmp(&(b - start).abs()))
            .unwrap_or(start)
    }

    /// `moves` as they may land: every clip shifted in time by the one
    /// amount nearest zero that leaves none of them covering a clip that
    /// stays put, so a group keeps its shape and lands in the nearest gap
    /// that holds all of it. An unknown clip is dropped, and an unknown
    /// track leaves its clip on the lane it is on. None when the moved
    /// clips would cover each other, which no shift in time can fix.
    pub fn resolve_moves(&self, moves: &[ClipMove]) -> Option<Vec<ClipMove>> {
        let wanted: Vec<(ClipMove, f64)> = moves
            .iter()
            .filter_map(|wanted| {
                let clip = self.clip(&wanted.clip_id)?;
                let track_id = if self.track(&wanted.track_id).is_some() {
                    wanted.track_id.clone()
                } else {
                    clip.track_id.clone()
                };
                Some((
                    ClipMove {
                        clip_id: wanted.clip_id.clone(),
                        start: wanted.start.max(0.0),
                        track_id,
                    },
                    clip.duration,
                ))
            })
            .collect();
        if wanted.is_empty() {
            return Some(Vec::new());
        }
        for (i, (a, a_len)) in wanted.iter().enumerate() {
            for (b, b_len) in &wanted[i + 1..] {
                if a.track_id == b.track_id
                    && covers(a.start, a.start + a_len, b.start, b.start + b_len)
                {
                    return None;
                }
            }
        }

        let moving: HashSet<&str> = wanted.iter().map(|(m, _)| m.clip_id.as_str()).collect();
        let staying: Vec<_> = self
            .clips
            .iter()
            .filter(|clip| !moving.contains(clip.id.as_str()))
            .collect();
        let earliest = wanted
            .iter()
            .map(|(m, _)| m.start)
            .fold(f64::INFINITY, f64::min);
        let fits = |shift: f64| {
            earliest + shift >= -TOUCH
                && wanted.iter().all(|(m, len)| {
                    let start = m.start + shift;
                    !staying.iter().any(|clip| {
                        clip.track_id == m.track_id
                            && covers(start, start + len, clip.start, clip.start + clip.duration)
                    })
                })
        };
        // Every shift that butts one moved clip against one that stays,
        // either side, plus none and the one that lands the group at zero.
        let mut shifts = vec![0.0, -earliest];
        for (m, len) in &wanted {
            for clip in staying.iter().filter(|clip| clip.track_id == m.track_id) {
                shifts.push(clip.start + clip.duration - m.start);
                shifts.push(clip.start - len - m.start);
            }
        }
        let shift = shifts
            .into_iter()
            .filter(|&shift| fits(shift))
            .min_by(|a, b| a.abs().total_cmp(&b.abs()))?;
        Some(
            wanted
                .into_iter()
                .map(|(m, _)| ClipMove {
                    start: (m.start + shift).max(0.0),
                    ..m
                })
                .collect(),
        )
    }

    /// The gap a clip sits in on its lane: where the clip before it ends
    /// (zero when there is none) and where the clip after it starts
    /// (infinity when there is none). A trim stays inside it.
    pub fn room_around(&self, clip_id: &str) -> Option<(f64, f64)> {
        let clip = self.clip(clip_id)?;
        let end = clip.start + clip.duration;
        let neighbours = self
            .clips
            .iter()
            .filter(|other| other.track_id == clip.track_id && other.id != clip.id);
        let mut before = 0.0_f64;
        let mut after = f64::INFINITY;
        for other in neighbours {
            let other_end = other.start + other.duration;
            if other_end <= clip.start + TOUCH {
                before = before.max(other_end);
            }
            if other.start >= end - TOUCH {
                after = after.min(other.start);
            }
        }
        Some((before, after))
    }
}

impl Timeline {
    /// The incoming clip at the nearest valid visual cut on this lane. A
    /// transition is offered only where clips actually touch, within the
    /// timeline's frame tolerance, and the pointer is close to that cut.
    pub fn transition_drop_target(
        &self,
        track_id: &str,
        seconds: f64,
        tolerance: f64,
    ) -> Option<&crate::model::Clip> {
        let frame = self.video.rate_den as f64 / self.video.rate_num.max(1) as f64;
        self.clips
            .iter()
            .filter(|clip| {
                clip.track_id == track_id
                    && clip.kind.is_visual()
                    && (clip.start - seconds).abs() <= tolerance
                    && clip.duration >= frame
                    && self.clips.iter().any(|other| {
                        other.id != clip.id
                            && other.track_id == track_id
                            && other.kind.is_visual()
                            && other.duration >= frame
                            && (other.start + other.duration - clip.start).abs() < frame / 2.0
                    })
            })
            .min_by(|a, b| {
                (a.start - seconds)
                    .abs()
                    .total_cmp(&(b.start - seconds).abs())
            })
            .map(|clip| clip.as_ref())
    }

    /// Visual effect drops address the clip under the pointer, never the selection.
    /// Half-open bounds give a cut to the incoming clip and reject empty space.
    pub fn visual_effect_drop_target(
        &self,
        track_id: &str,
        seconds: f64,
    ) -> Option<&crate::model::Clip> {
        self.clips
            .iter()
            .find(|clip| {
                clip.track_id == track_id
                    && !matches!(clip.kind, crate::model::ClipKind::Audio)
                    && clip.start <= seconds
                    && seconds < clip.start + clip.duration
            })
            .map(|clip| clip.as_ref())
    }

    /// The actual audio-capable clip under an effect drop, independent of
    /// selection. The end is exclusive, so a cut belongs to its incoming clip.
    pub fn audio_effect_drop_target(
        &self,
        track_id: &str,
        seconds: f64,
    ) -> Option<&crate::model::Clip> {
        self.clips
            .iter()
            .find(|clip| {
                clip.track_id == track_id
                    && matches!(
                        clip.kind,
                        crate::model::ClipKind::Audio | crate::model::ClipKind::Video
                    )
                    && clip.start <= seconds
                    && seconds < clip.start + clip.duration
            })
            .map(|clip| clip.as_ref())
    }
}

#[cfg(test)]
mod library_drop_tests {
    use super::*;
    use crate::model::{Clip, ClipKind};
    use std::sync::Arc;
    #[test]
    fn transitions_snap_only_to_adjacent_visual_cuts_on_the_target_lane() {
        let timeline = Timeline {
            clips: vec![
                Arc::new(Clip::blank("a", "v", ClipKind::Video, "A", 0.0, 2.0)),
                Arc::new(Clip::blank("b", "v", ClipKind::Image, "B", 2.0, 3.0)),
                Arc::new(Clip::blank("gap", "v", ClipKind::Video, "Gap", 6.0, 2.0)),
                Arc::new(Clip::blank(
                    "audio",
                    "a",
                    ClipKind::Audio,
                    "Audio",
                    2.0,
                    2.0,
                )),
            ],
            ..Timeline::default()
        };
        assert_eq!(
            timeline.transition_drop_target("v", 2.1, 0.25).unwrap().id,
            "b"
        );
        assert!(timeline.transition_drop_target("v", 0.0, 0.25).is_none());
        assert!(timeline.transition_drop_target("v", 3.0, 0.25).is_none());
        assert!(timeline.transition_drop_target("v", 6.0, 0.25).is_none());
        assert!(timeline.transition_drop_target("a", 2.0, 0.25).is_none());
    }
    #[test]
    fn visual_effects_target_pictures_and_layers_but_not_audio_or_gaps() {
        let timeline = Timeline {
            clips: vec![
                Arc::new(Clip::blank("a", "v", ClipKind::Video, "A", 0.0, 2.0)),
                Arc::new(Clip::blank("b", "v", ClipKind::Image, "B", 2.0, 2.0)),
                Arc::new(Clip::blank("text", "t", ClipKind::Text, "Text", 0.0, 2.0)),
                Arc::new(Clip::blank(
                    "layer",
                    "l",
                    ClipKind::Layer,
                    "Layer",
                    0.0,
                    2.0,
                )),
                Arc::new(Clip::blank(
                    "audio",
                    "a",
                    ClipKind::Audio,
                    "Audio",
                    0.0,
                    2.0,
                )),
            ],
            ..Timeline::default()
        };
        assert_eq!(
            timeline.visual_effect_drop_target("v", 1.0).unwrap().id,
            "a"
        );
        assert_eq!(
            timeline.visual_effect_drop_target("v", 2.0).unwrap().id,
            "b"
        );
        assert!(timeline.visual_effect_drop_target("v", 4.0).is_none());
        assert!(timeline.visual_effect_drop_target("v", -0.1).is_none());
        assert!(timeline.visual_effect_drop_target("a", 1.0).is_none());
        assert!(timeline.visual_effect_drop_target("t", 1.0).is_some());
        assert!(timeline.visual_effect_drop_target("l", 1.0).is_some());
    }
    #[test]
    fn audio_effects_target_the_pointer_clip_and_reject_gaps_and_stills() {
        let timeline = Timeline {
            clips: vec![
                Arc::new(Clip::blank("a", "v", ClipKind::Video, "A", 0.0, 2.0)),
                Arc::new(Clip::blank("b", "v", ClipKind::Video, "B", 2.0, 2.0)),
                Arc::new(Clip::blank(
                    "still",
                    "s",
                    ClipKind::Image,
                    "Still",
                    0.0,
                    5.0,
                )),
            ],
            ..Timeline::default()
        };
        assert_eq!(timeline.audio_effect_drop_target("v", 1.0).unwrap().id, "a");
        assert_eq!(timeline.audio_effect_drop_target("v", 2.0).unwrap().id, "b");
        assert!(timeline.audio_effect_drop_target("v", 4.0).is_none());
        assert!(timeline.audio_effect_drop_target("s", 1.0).is_none());
    }

    #[test]
    fn a_dragged_group_stops_at_zero_as_a_whole() {
        // Pulled left past the start, the early clip would stop at zero and
        // the rest would go on: the group keeps its shape instead.
        assert_eq!(shifted_starts(&[3.0, 13.0, 23.0], -5.0), [0.0, 10.0, 20.0]);
        assert_eq!(shifted_starts(&[3.0, 13.0], -1.0), [2.0, 12.0]);
        assert_eq!(shifted_starts(&[3.0, 13.0], 4.0), [7.0, 17.0]);
        assert!(shifted_starts(&[], 4.0).is_empty());
    }
}
