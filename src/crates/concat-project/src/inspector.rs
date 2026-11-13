// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Clovercut contributors
//! Common inspector targets and effect occurrence matching for batch edits.
use crate::model::{AppliedFilter, ClipKind, Project};

fn family(kind: ClipKind) -> u8 {
    match kind {
        ClipKind::Text => 0,
        ClipKind::Video | ClipKind::Image => 1,
        ClipKind::Shape => 2,
        ClipKind::Layer => 3,
        ClipKind::Audio => 4,
        // Sound layers edit bus strength; ordinary audio edits clip gain.
        ClipKind::SoundLayer => 5,
        ClipKind::Transition => 6,
    }
}

/// Selected clips with compatible controls, preserving order and excluding
/// locked tracks. Incompatible or missing selected clips have no common panel.
pub fn targets(
    project: &Project,
    selected: &[String],
    locked: impl Fn(&str) -> bool,
) -> Vec<String> {
    let timeline = project.active();
    let Some(first) = selected.first().and_then(|id| timeline.clip(id)) else {
        return Vec::new();
    };
    if !selected.iter().all(|id| {
        timeline
            .clip(id)
            .is_some_and(|clip| family(clip.kind) == family(first.kind))
    }) {
        return Vec::new();
    }
    selected
        .iter()
        .filter(|id| {
            timeline
                .clip(id)
                .is_some_and(|clip| !locked(&clip.track_id))
        })
        .cloned()
        .collect()
}

/// The same occurrence of the same package in a peer chain; a missing second
/// blur is skipped rather than applying the edit to its first blur or glow.
pub fn peer_link(shown: &[AppliedFilter], index: usize, peer: &[AppliedFilter]) -> Option<usize> {
    let id = &shown.get(index)?.id;
    let turn = shown[..index]
        .iter()
        .filter(|entry| &entry.id == id)
        .count();
    peer.iter()
        .enumerate()
        .filter(|(_, entry)| &entry.id == id)
        .nth(turn)
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Clip;
    #[test]
    fn common_controls_keep_order_skip_locked_tracks_and_refuse_mixed_families() {
        let mut project = Project::new();
        for (id, kind, lane) in [
            ("v", ClipKind::Video, "unlocked"),
            ("i", ClipKind::Image, "locked"),
            ("s", ClipKind::Shape, "unlocked"),
            ("a", ClipKind::Audio, "unlocked"),
            ("bus", ClipKind::SoundLayer, "unlocked"),
        ] {
            project
                .active_mut()
                .clips
                .push(Clip::blank(id, lane, kind, id, 0.0, 4.0).into());
        }
        let select = |ids: &[&str]| ids.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        assert_eq!(
            targets(&project, &select(&["i", "v"]), |_| false),
            select(&["i", "v"])
        );
        assert_eq!(
            targets(&project, &select(&["i", "v"]), |lane| lane == "locked"),
            select(&["v"])
        );
        for ids in [
            &["i"][..],
            &["v", "s"][..],
            &["a", "bus"][..],
            &["missing", "v"][..],
        ] {
            assert!(targets(&project, &select(ids), |lane| lane == "locked").is_empty());
        }
    }
    #[test]
    fn peers_match_the_same_occurrence_even_when_other_effects_differ() {
        let chain = |ids: &[&str]| {
            ids.iter()
                .map(|id| AppliedFilter::new(*id))
                .collect::<Vec<_>>()
        };
        let shown = chain(&["blur", "glow", "blur"]);
        assert_eq!(
            peer_link(&shown, 2, &chain(&["glow", "blur", "warm", "blur"])),
            Some(3)
        );
        assert_eq!(peer_link(&shown, 2, &chain(&["blur", "glow"])), None);
        assert_eq!(peer_link(&shown, 99, &shown), None);
    }
}
