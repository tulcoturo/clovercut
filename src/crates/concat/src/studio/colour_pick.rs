// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Clovercut contributors
use super::*;

/// A colour knob waiting for its colour off the picture: the clip it is on,
/// the link in that clip's picture chain, and the knob's key.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ColourPick {
    pub clip: String,
    pub entry: usize,
    pub key: String,
    pub effect: String,
    pub media: String,
    pub expected: AppliedFilter,
    pub targets: Vec<String>,
    pub shown: Vec<AppliedFilter>,
}

/// Whether a clip of this kind has a picture of its own to take a colour
/// off: a video or a still, decoded from its file. A title or a shape is
/// drawn, and has no file to read the pixel from.
pub(super) fn picks_colour(kind: model::ClipKind) -> bool {
    matches!(kind, model::ClipKind::Video | model::ClipKind::Image)
}

use concat_host::colour::{about as colour_about, has_hue};

impl Studio {
    /// A colour knob's eyedropper, pressed: armed for the selected clip's
    /// link `index` and knob `key`, so the next press on the stage takes
    /// the colour there off the clip's own picture (see `pick_colour_at`),
    /// or put down again when it already was. Only a clip with a file's
    /// picture of its own offers one.
    pub fn chain_pick_colour(&mut self, index: i32, key: &str) {
        let Ok(entry) = usize::try_from(index) else {
            return;
        };
        let Some(clip) = self
            .edit_targets()
            .first()
            .and_then(|id| self.clip(id))
            .cloned()
        else {
            return;
        };
        let Some(link) = clip.video_effects.get(entry) else {
            return;
        };
        if !Catalogue::builtin().get(&link.id).is_some_and(|package| {
            package
                .manifest
                .param(key)
                .is_some_and(|param| param.kind == ParamType::Color)
        }) {
            return;
        }
        let pick = ColourPick {
            clip: clip.id.clone(),
            entry,
            key: key.to_owned(),
            effect: clip
                .video_effects
                .get(entry)
                .map(|link| link.id.clone())
                .unwrap_or_default(),
            media: clip.media_id.clone(),
            expected: link.clone(),
            targets: self.edit_targets(),
            shown: clip.video_effects.clone(),
        };
        if self.picking_colour.as_ref() == Some(&pick) {
            self.picking_colour = None;
            return;
        }
        if self.locked(&clip.track_id) {
            return;
        }
        if !picks_colour(clip.kind) {
            self.notify(&t("studio.pickColourNoPicture"), true);
            return;
        }
        self.flush_commit();
        self.picking_colour = Some(pick);
        self.notify(&t("studio.pickColourHint"), false);
    }

    /// The eyedropper put down without a pick: Escape, or the project
    /// going. Whether one was armed.
    pub fn cancel_colour_pick(&mut self) -> bool {
        self.picking_colour.take().is_some()
    }

    /// A colour knob of the selected clip's link `index` back to its
    /// default - the colour its package finds by itself - as one edit.
    pub fn chain_auto_colour(&mut self, index: i32, key: &str) {
        self.flush_commit();
        self.edit_links(false, index, |link, _| {
            link.params.remove(key);
            link.keys.remove(key);
        });
        self.clip_commit();
    }

    /// A press on the stage while an eyedropper is armed: the colour of the
    /// clip's own picture under it - before its effects, through its crop,
    /// flips, turn and stabilization, the way a brush stroke finds its
    /// pixel - read off its file at the playhead on a worker, and laid on
    /// the knob as one edit. A press off the picture says so and keeps the
    /// eyedropper armed; a pick disarms it.
    pub(super) fn pick_colour_at(&mut self, pick: ColourPick, x: f64, y: f64) {
        let Some(clip) = self
            .stage_clips()
            .into_iter()
            .find(|shown| shown.id == pick.clip)
            .cloned()
        else {
            self.picking_colour = None;
            self.notify(&t("studio.pickColourGone"), true);
            return;
        };
        let [u, v] = self.stage_to_source(&clip, x, y);
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            self.notify(&t("studio.pickColourOffPicture"), false);
            return;
        }
        let Some(media) = self.project().media_by_id(&clip.media_id) else {
            self.picking_colour = None;
            return;
        };
        let path = media.path.clone();
        let seconds = self.source_at_playhead(&clip);
        self.picking_colour = None;
        spawn_in_project(
            move || {
                concat_host::media::still_at(&path, seconds, 960)
                    .and_then(|frame| colour_about(&frame, u, v).ok_or_else(String::new))
            },
            move |studio, _, _, picked| match picked {
                Ok(colour) => studio.lay_picked_colour(&pick, colour),
                Err(error) if error.is_empty() => {}
                Err(error) => studio.notify(&error, true),
            },
        );
    }

    /// A colour taken off the picture laid on the knob it was picked for,
    /// wherever the selection has gone since, as one edit. A key told a
    /// grey - nothing to key - says so, and where a black backdrop goes.
    fn lay_picked_colour(&mut self, pick: &ColourPick, [r, g, b]: [u8; 3]) {
        let Some(clip) = self.clip(&pick.clip).cloned() else {
            return;
        };
        if self.locked(&clip.track_id) || clip.media_id != pick.media {
            return;
        }
        let mut effects = clip.video_effects.clone();
        let Some(link) = effects.get_mut(pick.entry) else {
            return;
        };
        if link.id != pick.effect || *link != pick.expected {
            return;
        }
        let package = Catalogue::builtin()
            .packages()
            .find(|package| package.answers_to(&link.id));
        let Some(package) = package.filter(|package| {
            package
                .manifest
                .param(&pick.key)
                .is_some_and(|param| param.kind == ParamType::Color)
        }) else {
            return;
        };
        let keys = package.category().eq_ignore_ascii_case("Cutout");
        if keys && !has_hue([r, g, b]) {
            self.notify(&t("studio.pickColourGrey"), false);
            return;
        }
        let colour = packed(slint::Color::from_argb_u8(255, r, g, b));
        let commands = pick
            .targets
            .iter()
            .filter_map(|id| {
                let clip = self.clip(id)?;
                if self.locked(&clip.track_id) {
                    return None;
                }
                let mut effects = clip.video_effects.clone();
                let row = peer_link(&pick.shown, pick.entry, &effects)?;
                effects[row].params.insert(pick.key.clone(), colour);
                Some(Command::UpdateClip {
                    clip_id: id.clone(),
                    patch: ClipPatch {
                        video_effects: Some(effects),
                        ..ClipPatch::default()
                    },
                })
            })
            .collect();
        self.flush_commit();
        self.apply(Command::Batch { commands });
    }
}
