// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Built-in typographic styling recipes and title presets.
//!
//! Inspired by creative NLE workflows (such as CapCut and ReaCut title recipes),
//! these presets give creators instantaneous, professional-grade styling for
//! chapters, movie titles, lower-thirds, quotes, and broadcast captions.

use crate::model::{CaptionMotion, TextAlign, TextCase, TextStyle};

/// A designed typographic preset ready to apply to text clips.
#[derive(Clone, Debug, PartialEq)]
pub struct TitlePreset {
    /// Machine identifier (e.g. "classic", "cinema", "chapter").
    pub id: &'static str,
    /// Display name shown in titles panel.
    pub name: &'static str,
    /// Preset category ("Title", "Chapter", "LowerThird", "Quote").
    pub category: &'static str,
    /// Typographic and visual styling.
    pub style: TextStyle,
    /// Vertical placement as a frame-height fraction (positive moves down).
    pub offset_y: f64,
}

/// Returns the designed title preset matching `id`, if known.
pub fn title_preset(id: &str) -> Option<TitlePreset> {
    all_title_presets().into_iter().find(|p| p.id == id)
}

/// Returns the standard catalogue of creative title presets.
pub fn all_title_presets() -> Vec<TitlePreset> {
    vec![
        TitlePreset {
            id: "classic",
            name: "Classic Title",
            category: "Title",
            style: TextStyle {
                content: "Bold Title".into(),
                font_family: "Hanken Grotesk".into(),
                font_size: 0.08,
                font_weight: 800.0,
                color: "#ffffff".into(),
                align: TextAlign::Center,
                shadow: true,
                ..Default::default()
            },
            offset_y: 0.0,
        },
        TitlePreset {
            id: "cinema",
            name: "Cinema Title",
            category: "Title",
            style: TextStyle {
                content: "A JOURNEY\nINTO LIGHT".into(),
                font_family: "Hanken Grotesk".into(),
                font_size: 0.075,
                font_weight: 600.0,
                color: "#fff0d5".into(),
                tracking: 0.02,
                text_case: TextCase::Upper,
                align: TextAlign::Center,
                shadow: true,
                caption_motion: CaptionMotion {
                    entrance: 1, // fade
                    exit: 1,
                    duration: 0.5,
                },
                ..Default::default()
            },
            offset_y: 0.0,
        },
        TitlePreset {
            id: "chapter",
            name: "Chapter Heading",
            category: "Chapter",
            style: TextStyle {
                content: "CHAPTER 01\nTHE BEGINNING".into(),
                font_family: "Hanken Grotesk".into(),
                font_size: 0.065,
                font_weight: 700.0,
                color: "#ffd69a".into(),
                tracking: 0.015,
                align: TextAlign::Center,
                shadow: true,
                ..Default::default()
            },
            offset_y: -0.1,
        },
        TitlePreset {
            id: "editorial",
            name: "Minimalist Editorial",
            category: "Title",
            style: TextStyle {
                content: "Observation\nVol. 01".into(),
                font_family: "Hanken Grotesk".into(),
                font_size: 0.07,
                font_weight: 400.0,
                color: "#ffffff".into(),
                align: TextAlign::Left,
                shadow: false,
                stroke_width: 0.0,
                ..Default::default()
            },
            offset_y: 0.0,
        },
        TitlePreset {
            id: "lower_third",
            name: "Lower Third Banner",
            category: "LowerThird",
            style: TextStyle {
                content: "Speaker Name\nGuest Host".into(),
                font_family: "Hanken Grotesk".into(),
                font_size: 0.045,
                font_weight: 600.0,
                color: "#ffffff".into(),
                align: TextAlign::Left,
                background: "#11151cd9".into(),
                background_radius: 0.008,
                background_padding_x: 0.02,
                background_padding_y: 0.012,
                shadow: false,
                ..Default::default()
            },
            offset_y: 0.35,
        },
        TitlePreset {
            id: "quote",
            name: "Interview Quote",
            category: "Quote",
            style: TextStyle {
                content: "“Every frame is an opportunity\nto tell a meaningful story.”".into(),
                font_family: "Hanken Grotesk".into(),
                font_size: 0.055,
                font_weight: 500.0,
                italic: true,
                color: "#f5f5f7".into(),
                align: TextAlign::Center,
                shadow: true,
                ..Default::default()
            },
            offset_y: 0.1,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_presets_catalogue_resolves_all_categories() {
        let presets = all_title_presets();
        assert!(presets.len() >= 6);

        let classic = title_preset("classic").expect("classic preset exists");
        assert_eq!(classic.category, "Title");
        assert_eq!(classic.style.font_weight, 800.0);

        let lower_third = title_preset("lower_third").expect("lower third exists");
        assert_eq!(lower_third.category, "LowerThird");
        assert_eq!(lower_third.offset_y, 0.35);
        assert!(!lower_third.style.background.is_empty());

        let cinema = title_preset("cinema").expect("cinema exists");
        assert_eq!(cinema.style.caption_motion.entrance, 1);
        assert_eq!(cinema.style.color, "#fff0d5");

        assert!(title_preset("unknown_id").is_none());
    }
}
