// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Media-bin selection in the filtered, sorted order currently on screen.

use std::collections::HashSet;

/// Select a card by stable id. A stale/hidden card changes nothing. Shift
/// extends from an id anchor; Ctrl/Command toggles, or adds a Shift range.
pub fn click(
    selected: &mut HashSet<String>,
    anchor: &mut Option<String>,
    visible: &[&str],
    target: &str,
    additive: bool,
    range: bool,
) -> bool {
    let Some(to) = visible.iter().position(|id| *id == target) else {
        return false;
    };
    if range {
        let from = anchor
            .as_deref()
            .and_then(|held| visible.iter().position(|id| *id == held));
        if !additive {
            selected.clear();
        }
        let from = from.unwrap_or_else(|| {
            *anchor = Some(target.to_owned());
            to
        });
        selected.extend(
            visible[from.min(to)..=from.max(to)]
                .iter()
                .map(|id| (*id).to_owned()),
        );
    } else {
        if additive {
            if !selected.remove(target) {
                selected.insert(target.to_owned());
            }
        } else {
            selected.clear();
            selected.insert(target.to_owned());
        }
        *anchor = Some(target.to_owned());
    }
    true
}

/// Select only this shelf's visible cards, preserving a still-visible anchor.
pub fn all(selected: &mut HashSet<String>, anchor: &mut Option<String>, visible: &[&str]) {
    *selected = visible.iter().map(|id| (*id).to_owned()).collect();
    if !anchor
        .as_deref()
        .is_some_and(|held| visible.contains(&held))
    {
        *anchor = visible.first().map(|id| (*id).to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(values: &[&str]) -> HashSet<String> {
        values.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn plain_and_additive_clicks_keep_a_stable_id_anchor() {
        let mut selected = ids(&["hidden"]);
        let mut anchor = None;
        let visible = ["a", "b", "c"];
        click(&mut selected, &mut anchor, &visible, "b", false, false);
        assert_eq!(selected, ids(&["b"]));
        click(&mut selected, &mut anchor, &visible, "a", true, false);
        assert_eq!(selected, ids(&["a", "b"]));
        click(&mut selected, &mut anchor, &visible, "a", true, false);
        assert_eq!(selected, ids(&["b"]));
        assert_eq!(anchor.as_deref(), Some("a"));
    }

    #[test]
    fn shift_extends_shrinks_and_reverses_without_moving_the_anchor() {
        let mut selected = HashSet::new();
        let mut anchor = None;
        let visible = ["a", "b", "c", "d", "e"];
        click(&mut selected, &mut anchor, &visible, "c", false, false);
        click(&mut selected, &mut anchor, &visible, "e", false, true);
        assert_eq!(selected, ids(&["c", "d", "e"]));
        click(&mut selected, &mut anchor, &visible, "d", false, true);
        assert_eq!(selected, ids(&["c", "d"]));
        click(&mut selected, &mut anchor, &visible, "a", false, true);
        assert_eq!(selected, ids(&["a", "b", "c"]));
        assert_eq!(anchor.as_deref(), Some("c"));
    }

    #[test]
    fn additive_range_preserves_others_and_uses_the_current_sort() {
        let mut selected = ids(&["hidden", "a"]);
        let mut anchor = Some("a".to_owned());
        click(
            &mut selected,
            &mut anchor,
            &["c", "a", "b"],
            "b",
            true,
            true,
        );
        assert_eq!(selected, ids(&["hidden", "a", "b"]));
    }

    #[test]
    fn a_hidden_or_removed_anchor_starts_at_the_clicked_card() {
        let mut selected = ids(&["hidden"]);
        let mut anchor = Some("hidden".to_owned());
        click(&mut selected, &mut anchor, &["a", "b"], "b", false, true);
        assert_eq!(selected, ids(&["b"]));
        assert_eq!(anchor.as_deref(), Some("b"));
        anchor = None;
        click(&mut selected, &mut anchor, &["a", "b"], "b", true, true);
        assert_eq!(
            selected,
            ids(&["b"]),
            "an additive fallback inserts, never toggles"
        );
    }

    #[test]
    fn stale_card_events_do_not_change_the_selection_or_anchor() {
        let mut selected = ids(&["b"]);
        let mut anchor = Some("b".to_owned());
        assert!(!click(
            &mut selected,
            &mut anchor,
            &["a", "b"],
            "gone",
            false,
            true
        ));
        assert_eq!(selected, ids(&["b"]));
        assert_eq!(anchor.as_deref(), Some("b"));
    }

    #[test]
    fn select_all_replaces_hidden_selection_and_handles_an_empty_shelf() {
        let mut selected = ids(&["hidden"]);
        let mut anchor = Some("hidden".to_owned());
        all(&mut selected, &mut anchor, &["b", "a"]);
        assert_eq!(selected, ids(&["b", "a"]));
        assert_eq!(anchor.as_deref(), Some("b"));
        all(&mut selected, &mut anchor, &[]);
        assert!(selected.is_empty());
        assert!(anchor.is_none());
    }
}
