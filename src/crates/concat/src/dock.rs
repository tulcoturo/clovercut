// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! The workspace's arrangement: a tree of splits and views, walked flat
//! for Slint, which has no recursion.

use std::collections::HashMap;

use crate::ui::{DockDivider, DockSide, PaneKind, SeatBox};
pub use crate::workspace_layout::{LayoutPreset, SavedDock};

pub const SEAT_MIN_W: f32 = 240.0;
pub const SEAT_MIN_H: f32 = 140.0;
/// The floor that holds where the one above cannot: enough of a seat left to
/// find its corner button with, which is the only way back from a layout that
/// has been squeezed too far.
pub const SEAT_MIN_GRAB: f32 = 64.0;
/// The gutter between the halves of a split, and the margin round the lot.
pub const SEAT_GAP: f32 = 8.0;

pub enum Dock {
    /// One view, filling its box.
    Leaf(PaneKind),
    /// Two nodes and the gutter between them. `columns` means side by side,
    /// which is a vertical gutter dragged left and right; `ratio` is the share
    /// of the usable extent — the box less the gutter — that the first takes.
    Split {
        columns: bool,
        ratio: f32,
        first: Box<Dock>,
        second: Box<Dock>,
    },
}

impl Dock {
    pub fn leaf(kind: PaneKind) -> Box<Dock> {
        Box::new(Dock::Leaf(kind))
    }

    /// The node a path names, following each step into the first branch or the
    /// second. A path that runs past a leaf stops there, which cannot happen
    /// for a path this module produced.
    pub fn at(&self, path: &[bool]) -> &Dock {
        let Some((step, rest)) = path.split_first() else {
            return self;
        };
        match self {
            Dock::Split { first, second, .. } => {
                if *step {
                    second.at(rest)
                } else {
                    first.at(rest)
                }
            }
            leaf => leaf,
        }
    }

    pub fn at_mut(&mut self, path: &[bool]) -> &mut Dock {
        let Some((step, rest)) = path.split_first() else {
            return self;
        };
        match self {
            Dock::Split { first, second, .. } => {
                if *step {
                    second.at_mut(rest)
                } else {
                    first.at_mut(rest)
                }
            }
            leaf => leaf,
        }
    }

    /// The path to the nth leaf, counted in the same walk `lay_out` uses — so
    /// the number a seat carries and the number a drop reports are the same
    /// number. Paths rather than indices for the editing below, because an
    /// index is invalidated by a change elsewhere in the tree and a path is
    /// only invalidated by a change on its own way down.
    pub fn leaf_path(&self, index: usize) -> Option<Vec<bool>> {
        fn walk(node: &Dock, want: usize, seen: &mut usize, path: &mut Vec<bool>) -> bool {
            match node {
                Dock::Leaf(_) => {
                    if *seen == want {
                        return true;
                    }
                    *seen += 1;
                    false
                }
                Dock::Split { first, second, .. } => {
                    path.push(false);
                    if walk(first, want, seen, path) {
                        return true;
                    }
                    path.pop();
                    path.push(true);
                    if walk(second, want, seen, path) {
                        return true;
                    }
                    path.pop();
                    false
                }
            }
        }
        let (mut seen, mut path) = (0, Vec::new());
        walk(self, index, &mut seen, &mut path).then_some(path)
    }

    /// The same for the nth split — parent before children, which is the order
    /// `lay_out` numbers the gutters in.
    pub fn split_path(&self, index: usize) -> Option<Vec<bool>> {
        fn walk(node: &Dock, want: usize, seen: &mut usize, path: &mut Vec<bool>) -> bool {
            let Dock::Split { first, second, .. } = node else {
                return false;
            };
            if *seen == want {
                return true;
            }
            *seen += 1;
            path.push(false);
            if walk(first, want, seen, path) {
                return true;
            }
            path.pop();
            path.push(true);
            if walk(second, want, seen, path) {
                return true;
            }
            path.pop();
            false
        }
        let (mut seen, mut path) = (0, Vec::new());
        walk(self, index, &mut seen, &mut path).then_some(path)
    }

    /// Whether a view of this kind is anywhere in the tree.
    pub fn contains(&self, kind: PaneKind) -> bool {
        match self {
            Dock::Leaf(held) => *held == kind,
            Dock::Split { first, second, .. } => first.contains(kind) || second.contains(kind),
        }
    }

    pub fn kind_at(&self, index: usize) -> Option<PaneKind> {
        match self.at(&self.leaf_path(index)?) {
            Dock::Leaf(kind) => Some(*kind),
            _ => None,
        }
    }

    /// Minimum width required so every leaf in this subtree keeps at least `SEAT_MIN_W`.
    pub fn min_width(&self) -> f32 {
        match self {
            Dock::Leaf(_) => SEAT_MIN_W,
            Dock::Split {
                columns: true,
                first,
                second,
                ..
            } => first.min_width() + SEAT_GAP + second.min_width(),
            Dock::Split {
                columns: false,
                first,
                second,
                ..
            } => first.min_width().max(second.min_width()),
        }
    }

    fn leaf_count(&self) -> usize {
        match self {
            Dock::Leaf(_) => 1,
            Dock::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }

    fn min_width_with(&self, start: usize, minimums: &HashMap<usize, f32>) -> f32 {
        if minimums.is_empty() {
            return self.min_width();
        }
        match self {
            Dock::Leaf(PaneKind::Inspector) => minimums
                .get(&start)
                .copied()
                .filter(|width| width.is_finite())
                .unwrap_or(SEAT_MIN_W)
                .clamp(SEAT_MIN_W, 600.0),
            Dock::Leaf(_) => SEAT_MIN_W,
            Dock::Split {
                columns,
                first,
                second,
                ..
            } => {
                let a = first.min_width_with(start, minimums);
                let b = second.min_width_with(start + first.leaf_count(), minimums);
                if *columns { a + SEAT_GAP + b } else { a.max(b) }
            }
        }
    }

    /// Minimum height required so every leaf in this subtree keeps at least `SEAT_MIN_H`.
    pub fn min_height(&self) -> f32 {
        match self {
            Dock::Leaf(_) => SEAT_MIN_H,
            Dock::Split {
                columns: false,
                first,
                second,
                ..
            } => first.min_height() + SEAT_GAP + second.min_height(),
            Dock::Split {
                columns: true,
                first,
                second,
                ..
            } => first.min_height().max(second.min_height()),
        }
    }

    /// Turn one leaf into a split, with a new view on the named side of it.
    /// Half and half: the seat being split is the only thing that knows how
    /// much room there is, and half of it is the answer that needs no rule.
    pub fn split_leaf(&mut self, path: &[bool], kind: PaneKind, side: DockSide) {
        let node = self.at_mut(path);
        let existing = Box::new(std::mem::replace(node, Dock::Leaf(kind)));
        let (columns, first, second) = match side {
            DockSide::Left => (true, Dock::leaf(kind), existing),
            DockSide::Right => (true, existing, Dock::leaf(kind)),
            DockSide::Top => (false, Dock::leaf(kind), existing),
            _ => (false, existing, Dock::leaf(kind)),
        };
        *node = Dock::Split {
            columns,
            ratio: 0.5,
            first,
            second,
        };
    }

    /// Take a leaf out, and let its sibling take the space — the split it was
    /// half of stops existing. A path with nothing above it is the whole tree,
    /// which cannot be removed: a window with no seats has nowhere to put
    /// anything back.
    pub fn remove_leaf(&mut self, path: &[bool]) {
        let Some((last, above)) = path.split_last() else {
            return;
        };
        let parent = self.at_mut(above);
        let survivor = match parent {
            Dock::Split { first, second, .. } => {
                let keep = if *last { first } else { second };
                std::mem::replace(keep.as_mut(), Dock::Leaf(PaneKind::Media))
            }
            _ => return,
        };
        *parent = survivor;
    }
}

/// The tree walked flat: a box per view, a box per gutter.
#[derive(Default)]
pub struct DockLayout {
    pub seats: Vec<SeatBox>,
    pub dividers: Vec<DockDivider>,
    /// The usable extent of each split — its box less the gutter — in the same
    /// order as `dividers`. A divider drag is a delta in pixels and a ratio is
    /// a fraction, and this is what converts between them.
    pub extents: Vec<f32>,
}

#[cfg(test)]
pub fn lay_out(node: &Dock, bounds: (f32, f32, f32, f32), out: &mut DockLayout) {
    lay_out_with_minimums(node, bounds, out, &HashMap::new());
}

/// Content minimums follow leaf indices, matching the live Slint seats.
/// Values on non-inspector seats are ignored after a pane changes kind.
pub fn lay_out_with_minimums(
    node: &Dock,
    (x, y, w, h): (f32, f32, f32, f32),
    out: &mut DockLayout,
    minimums: &HashMap<usize, f32>,
) {
    match node {
        Dock::Leaf(kind) => out.seats.push(SeatBox {
            index: out.seats.len() as i32,
            kind: *kind,
            x,
            y,
            width: w.max(0.0),
            height: h.max(0.0),
        }),
        Dock::Split {
            columns,
            ratio,
            first,
            second,
        } => {
            // The slot is taken before the children are walked, so a gutter's
            // number is its split's position in a parent-first walk — which is
            // what `split_path` counts and what a drag reports back.
            let index = out.dividers.len();
            out.dividers.push(DockDivider::default());
            out.extents.push(0.0);

            let along = if *columns { w } else { h };
            let usable = (along - SEAT_GAP).max(0.0);
            let (min_first, min_second) = if *columns {
                (
                    first.min_width_with(out.seats.len(), minimums),
                    second.min_width_with(out.seats.len() + first.leaf_count(), minimums),
                )
            } else {
                (first.min_height(), second.min_height())
            };
            let head = if usable >= min_first + min_second {
                (usable * ratio).clamp(min_first, usable - min_second)
            } else if min_first + min_second > 0.0 {
                (usable * (min_first / (min_first + min_second))).clamp(0.0, usable)
            } else {
                (usable * ratio).clamp(0.0, usable)
            };
            out.extents[index] = usable;
            out.dividers[index] = if *columns {
                DockDivider {
                    index: index as i32,
                    columns: true,
                    x: x + head,
                    y,
                    width: SEAT_GAP,
                    height: h.max(0.0),
                }
            } else {
                DockDivider {
                    index: index as i32,
                    columns: false,
                    x,
                    y: y + head,
                    width: w.max(0.0),
                    height: SEAT_GAP,
                }
            };

            if *columns {
                lay_out_with_minimums(first, (x, y, head, h), out, minimums);
                lay_out_with_minimums(
                    second,
                    (x + head + SEAT_GAP, y, usable - head, h),
                    out,
                    minimums,
                );
            } else {
                lay_out_with_minimums(first, (x, y, w, head), out, minimums);
                lay_out_with_minimums(
                    second,
                    (x, y + head + SEAT_GAP, w, usable - head),
                    out,
                    minimums,
                );
            }
        }
    }
}

impl LayoutPreset {
    pub fn from_index(index: i32) -> Self {
        match index {
            0 => Self::Default,
            1 => Self::Audio,
            2 => Self::VerticalRight,
            3 => Self::VerticalLeft,
            4 => Self::InspectorPriority,
            5 => Self::MediaPriority,
            _ => Self::Default,
        }
    }

    pub fn to_index(self) -> i32 {
        self as i32
    }

    pub fn build_dock(self) -> Dock {
        match self {
            Self::Default => default_dock(),
            Self::Audio => audio_dock(),
            Self::VerticalRight => vertical_right_dock(),
            Self::VerticalLeft => vertical_left_dock(),
            Self::InspectorPriority => inspector_priority_dock(),
            Self::MediaPriority => media_priority_dock(),
        }
    }
}

pub fn audio_dock() -> Dock {
    Dock::Split {
        columns: true,
        ratio: 0.78,
        first: Box::new(Dock::Split {
            columns: true,
            ratio: 0.32,
            first: Box::new(Dock::Split {
                columns: false,
                ratio: 0.55,
                first: Dock::leaf(PaneKind::Media),
                second: Dock::leaf(PaneKind::Preview),
            }),
            second: Dock::leaf(PaneKind::Timeline),
        }),
        second: Dock::leaf(PaneKind::Inspector),
    }
}

pub fn vertical_right_dock() -> Dock {
    Dock::Split {
        columns: true,
        ratio: 0.70,
        first: Box::new(Dock::Split {
            columns: false,
            ratio: 0.56,
            first: Box::new(Dock::Split {
                columns: true,
                ratio: 0.52,
                first: Dock::leaf(PaneKind::Media),
                second: Dock::leaf(PaneKind::Inspector),
            }),
            second: Dock::leaf(PaneKind::Timeline),
        }),
        second: Dock::leaf(PaneKind::Preview),
    }
}

pub fn vertical_left_dock() -> Dock {
    Dock::Split {
        columns: true,
        ratio: 0.30,
        first: Dock::leaf(PaneKind::Preview),
        second: Box::new(Dock::Split {
            columns: false,
            ratio: 0.56,
            first: Box::new(Dock::Split {
                columns: true,
                ratio: 0.52,
                first: Dock::leaf(PaneKind::Media),
                second: Dock::leaf(PaneKind::Inspector),
            }),
            second: Dock::leaf(PaneKind::Timeline),
        }),
    }
}

pub fn inspector_priority_dock() -> Dock {
    Dock::Split {
        columns: false,
        ratio: 0.56,
        first: Box::new(Dock::Split {
            columns: true,
            ratio: 0.52,
            first: Box::new(Dock::Split {
                columns: true,
                ratio: 0.46,
                first: Dock::leaf(PaneKind::Media),
                second: Dock::leaf(PaneKind::Preview),
            }),
            second: Dock::leaf(PaneKind::Inspector),
        }),
        second: Dock::leaf(PaneKind::Timeline),
    }
}

pub fn media_priority_dock() -> Dock {
    Dock::Split {
        columns: false,
        ratio: 0.56,
        first: Box::new(Dock::Split {
            columns: true,
            ratio: 0.48,
            first: Dock::leaf(PaneKind::Media),
            second: Box::new(Dock::Split {
                columns: true,
                ratio: 0.54,
                first: Dock::leaf(PaneKind::Preview),
                second: Dock::leaf(PaneKind::Inspector),
            }),
        }),
        second: Dock::leaf(PaneKind::Timeline),
    }
}

/// The arrangement the editor opens with: a three-panel upper workspace
/// (Media Library, Preview Monitor, and Inspector side by side across the top)
/// and the Timeline spanning the full width of the lower portion of the window.
/// About 0.28 of the width goes to the library, 0.44 to the monitor, and 0.28
/// to the inspector; the timeline takes the lower 0.44 of the height across
/// the entire window width.
pub fn default_dock() -> Dock {
    Dock::Split {
        columns: false,
        ratio: 0.56,
        first: Box::new(Dock::Split {
            columns: true,
            ratio: 0.72,
            first: Box::new(Dock::Split {
                columns: true,
                ratio: 0.389,
                first: Dock::leaf(PaneKind::Media),
                second: Dock::leaf(PaneKind::Preview),
            }),
            second: Dock::leaf(PaneKind::Inspector),
        }),
        // The timeline spans the full width of the lower workspace, with its
        // VU meters at its own right edge (see vu-meter.slint).
        second: Dock::leaf(PaneKind::Timeline),
    }
}

/// The top edge of a row, measured down the stack from the first lane.
/// The dock a narrow window shows: the monitor over the timeline, and
/// nothing beside either. The library and the inspector are a switch away
/// on each seat - see `PaneSwitcher` - which on a phone is how they are
/// reached in every editor: one at a time, in the space the picture had.
pub fn compact_dock() -> Dock {
    Dock::Split {
        columns: false,
        ratio: 0.5,
        first: Dock::leaf(PaneKind::Preview),
        second: Dock::leaf(PaneKind::Timeline),
    }
}

pub fn row_top(heights: &[f32], row: i32) -> f32 {
    heights.iter().take(row.max(0) as usize).sum()
}

/// Deeper than any arrangement a person builds by hand; a file claiming
/// more is not one Concat wrote.
const SAVED_DEPTH: usize = 24;

fn pane_name(kind: PaneKind) -> &'static str {
    match kind {
        PaneKind::Media => "media",
        PaneKind::Preview => "preview",
        PaneKind::Inspector => "inspector",
        PaneKind::Timeline => "timeline",
        PaneKind::Keyframes => "keyframes",
    }
}

fn pane_of(name: &str) -> Option<PaneKind> {
    Some(match name {
        "media" => PaneKind::Media,
        "preview" => PaneKind::Preview,
        "inspector" => PaneKind::Inspector,
        "timeline" => PaneKind::Timeline,
        "keyframes" => PaneKind::Keyframes,
        _ => return None,
    })
}

impl Dock {
    pub fn to_saved(&self) -> SavedDock {
        match self {
            Dock::Leaf(kind) => SavedDock::Pane(pane_name(*kind).to_owned()),
            Dock::Split {
                columns,
                ratio,
                first,
                second,
            } => SavedDock::Split {
                columns: *columns,
                ratio: *ratio,
                first: Box::new(first.to_saved()),
                second: Box::new(second.to_saved()),
            },
        }
    }

    /// The dock a saved one describes, or None when any of it does not read:
    /// a view this build does not know, a ratio that is not a number, a tree
    /// deeper than a hand could make. A ratio is pulled back inside the
    /// gutter's reach, so a seat cannot come back squeezed to nothing.
    pub fn from_saved(saved: &SavedDock) -> Option<Dock> {
        fn build(saved: &SavedDock, depth: usize) -> Option<Dock> {
            if depth > SAVED_DEPTH {
                return None;
            }
            Some(match saved {
                SavedDock::Pane(name) => Dock::Leaf(pane_of(name)?),
                SavedDock::Split {
                    columns,
                    ratio,
                    first,
                    second,
                } => {
                    if !ratio.is_finite() {
                        return None;
                    }
                    Dock::Split {
                        columns: *columns,
                        ratio: ratio.clamp(0.05, 0.95),
                        first: Box::new(build(first, depth + 1)?),
                        second: Box::new(build(second, depth + 1)?),
                    }
                }
            })
        }
        build(saved, 0)
    }
}

/// The row a point down the stack falls in, clamped to the stack.
pub fn row_at(heights: &[f32], y: f32) -> i32 {
    let mut top = 0.0;
    for (row, height) in heights.iter().enumerate() {
        top += height;
        if y < top {
            return row as i32;
        }
    }
    (heights.len() as i32 - 1).max(0)
}

/// The row whose top edge is nearest a point down the stack.
///
/// What a *move* wants, and not the same question as `row_at`: dragging a clip
/// halfway into the lane below should put it there, which is the rounding the
/// old `delta / lane-height` did for free back when every lane was one height.
#[allow(dead_code)]
pub fn nearest_row(heights: &[f32], y: f32) -> i32 {
    let (mut top, mut best, mut best_distance) = (0.0_f32, 0_i32, f32::MAX);
    for (row, height) in heights.iter().enumerate() {
        let distance = (top - y).abs();
        if distance < best_distance {
            best_distance = distance;
            best = row as i32;
        }
        top += height;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspector_minimums_follow_content_and_ignore_other_pane_kinds() {
        let dock = Dock::Split {
            columns: true,
            ratio: 0.95,
            first: Dock::leaf(PaneKind::Media),
            second: Dock::leaf(PaneKind::Inspector),
        };
        let mut minimums = HashMap::from([(0, 500.0), (1, 336.0)]);
        let mut out = DockLayout::default();
        lay_out_with_minimums(&dock, (0.0, 0.0, 900.0, 600.0), &mut out, &minimums);
        assert_eq!(out.seats[1].width, 336.0);
        minimums.insert(1, 240.0);
        let mut smaller = DockLayout::default();
        lay_out_with_minimums(&dock, (0.0, 0.0, 900.0, 600.0), &mut smaller, &minimums);
        assert_eq!(smaller.seats[1].width, 240.0);
        assert!(smaller.seats[0].width > out.seats[0].width);
        let mut narrow = DockLayout::default();
        lay_out_with_minimums(&dock, (0.0, 0.0, 300.0, 600.0), &mut narrow, &minimums);
        assert!(narrow.seats.iter().all(|seat| seat.width > 0.0));
        assert_eq!(
            narrow.seats[0].width + narrow.seats[1].width + SEAT_GAP,
            300.0
        );
    }

    #[test]
    fn nested_inspectors_have_independent_content_minimums() {
        let dock = Dock::Split {
            columns: false,
            ratio: 0.5,
            first: Box::new(Dock::Split {
                columns: true,
                ratio: 0.99,
                first: Dock::leaf(PaneKind::Media),
                second: Dock::leaf(PaneKind::Inspector),
            }),
            second: Box::new(Dock::Split {
                columns: true,
                ratio: 0.99,
                first: Dock::leaf(PaneKind::Preview),
                second: Dock::leaf(PaneKind::Inspector),
            }),
        };
        let mut out = DockLayout::default();
        lay_out_with_minimums(
            &dock,
            (0.0, 0.0, 1000.0, 800.0),
            &mut out,
            &HashMap::from([(1, 336.0), (3, 280.0)]),
        );
        assert_eq!(out.seats[1].width, 336.0);
        assert_eq!(out.seats[3].width, 280.0);
    }

    #[test]
    fn default_dock_lays_out_three_upper_panels_and_full_width_lower_timeline() {
        let dock = default_dock();
        let mut out = DockLayout::default();
        lay_out(&dock, (0.0, 0.0, 1920.0, 1080.0), &mut out);
        assert_eq!(out.seats.len(), 4);

        let media = out
            .seats
            .iter()
            .find(|s| s.kind == PaneKind::Media)
            .expect("media seat");
        let preview = out
            .seats
            .iter()
            .find(|s| s.kind == PaneKind::Preview)
            .expect("preview seat");
        let inspector = out
            .seats
            .iter()
            .find(|s| s.kind == PaneKind::Inspector)
            .expect("inspector seat");
        let timeline = out
            .seats
            .iter()
            .find(|s| s.kind == PaneKind::Timeline)
            .expect("timeline seat");

        // All three upper panels sit in the top row (same y and height)
        assert_eq!(media.y, preview.y);
        assert_eq!(preview.y, inspector.y);
        assert_eq!(media.height, preview.height);
        assert_eq!(preview.height, inspector.height);

        // Ordered left to right: Media -> Preview -> Inspector
        assert!(media.x < preview.x);
        assert!(preview.x < inspector.x);

        // Timeline sits below the upper workspace and spans 100% of the usable width
        assert!(timeline.y > media.y + media.height);
        assert_eq!(timeline.x, media.x);
        assert!((timeline.width - 1920.0).abs() < 0.01);
        assert!(media.width >= SEAT_MIN_W);
        assert!(preview.width >= SEAT_MIN_W);
        assert!(inspector.width >= SEAT_MIN_W);
    }

    #[test]
    fn default_dock_enforces_minimum_seat_widths_down_to_narrow_desktop() {
        let dock = default_dock();
        for &(w, h) in &[
            (911.0, 684.0),
            (1264.0, 704.0),
            (1584.0, 884.0),
            (1904.0, 1064.0),
        ] {
            let mut out = DockLayout::default();
            lay_out(&dock, (8.0, 8.0, w, h), &mut out);
            for seat in &out.seats {
                assert!(
                    seat.width >= SEAT_MIN_W - 0.01,
                    "seat {:?} width {} < SEAT_MIN_W at usable {}x{}",
                    seat.kind,
                    seat.width,
                    w,
                    h
                );
                assert!(
                    seat.height >= SEAT_MIN_H - 0.01,
                    "seat {:?} height {} < SEAT_MIN_H at usable {}x{}",
                    seat.kind,
                    seat.height,
                    w,
                    h
                );
            }
        }
    }

    #[test]
    fn all_layout_presets_produce_four_seats() {
        for preset in [
            LayoutPreset::Default,
            LayoutPreset::Audio,
            LayoutPreset::VerticalRight,
            LayoutPreset::VerticalLeft,
            LayoutPreset::InspectorPriority,
            LayoutPreset::MediaPriority,
        ] {
            let dock = preset.build_dock();
            let mut out = DockLayout::default();
            lay_out(&dock, (0.0, 0.0, 1920.0, 1080.0), &mut out);
            assert_eq!(out.seats.len(), 4, "preset {:?} seat count", preset);
            assert!(out.seats.iter().any(|s| s.kind == PaneKind::Media));
            assert!(out.seats.iter().any(|s| s.kind == PaneKind::Preview));
            assert!(out.seats.iter().any(|s| s.kind == PaneKind::Inspector));
            assert!(out.seats.iter().any(|s| s.kind == PaneKind::Timeline));
        }
    }
}

#[cfg(test)]
mod saved_tests {
    use super::*;

    #[test]
    fn presets_and_compact_layout_round_trip_without_changing_reset_target() {
        for index in 0..6 {
            let preset = LayoutPreset::from_index(index);
            let dock = preset.build_dock().to_saved();
            let read: SavedDock =
                serde_json::from_str(&serde_json::to_string(&dock).unwrap()).unwrap();
            assert_eq!(Dock::from_saved(&read).unwrap().to_saved(), dock);
            let named: LayoutPreset =
                serde_json::from_str(&serde_json::to_string(&preset).unwrap()).unwrap();
            assert_eq!(named, preset);
        }
        let dock = compact_dock().to_saved();
        assert_eq!(Dock::from_saved(&dock).unwrap().to_saved(), dock);
    }

    #[test]
    fn saved_layout_rejects_unknown_views_nonfinite_ratios_and_excessive_depth() {
        assert!(Dock::from_saved(&SavedDock::Pane("future-view".into())).is_none());
        let split = |ratio, first| SavedDock::Split {
            columns: true,
            ratio,
            first: Box::new(first),
            second: Box::new(SavedDock::Pane("timeline".into())),
        };
        assert!(Dock::from_saved(&split(f32::NAN, SavedDock::Pane("preview".into()))).is_none());
        let mut deep = SavedDock::Pane("preview".into());
        for _ in 0..26 {
            deep = split(0.5, deep);
        }
        assert!(Dock::from_saved(&deep).is_none());
        let Dock::Split { ratio, .. } =
            Dock::from_saved(&split(0.0, SavedDock::Pane("media".into()))).unwrap()
        else {
            panic!("expected split")
        };
        assert_eq!(ratio, 0.05);
    }
}
