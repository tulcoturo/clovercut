// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Serializable desktop layout data, independent of generated UI types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutPreset {
    #[default]
    Default = 0,
    Audio = 1,
    VerticalRight = 2,
    VerticalLeft = 3,
    InspectorPriority = 4,
    MediaPriority = 5,
}

/// A dock as the settings file keeps it between runs: the same tree, with
/// each view by name, so a view renamed or added later fails one layout
/// and never the whole file.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SavedDock {
    Pane(String),
    Split {
        columns: bool,
        ratio: f32,
        first: Box<SavedDock>,
        second: Box<SavedDock>,
    },
}

/// Workspace arrangement retained between desktop sessions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedLayout {
    /// Wide-window panel arrangement.
    pub wide: Option<SavedDock>,
    /// Compact-window panel arrangement.
    pub compact: Option<SavedDock>,
    /// Preset used by Reset layout, independently of custom panel edits.
    pub preset: LayoutPreset,
    /// Last normal window width in logical pixels.
    pub width: Option<f32>,
    /// Last normal window height in logical pixels.
    pub height: Option<f32>,
    /// Whether the window was maximized.
    pub maximized: bool,
}
