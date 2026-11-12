// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors
//! Exercise the inspector's real curve arithmetic without compiling the window.

/// Translation stub for arithmetic-only inspector tests.
pub mod i18n {
    /// Keep translation keys stable for the graph's preset catalogue.
    pub fn t(key: &str) -> String {
        key.to_owned()
    }
}

#[path = "../../concat/src/speed_ramp.rs"]
pub mod speed_ramp;
