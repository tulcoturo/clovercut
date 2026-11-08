# Media-bin selection and focus, 8 November 2026

[Concat #696](https://github.com/jub0t/Concat/issues/696) also applies to
Clovercut: Select All falls through to the timeline, and Shift-click is treated
as an additive toggle rather than a visible range. The existing marquee also
walks document order even when the cards are sorted by name or kind.

## Adaptation

- Give each media grid its own keyboard focus. Card/background clicks commit
  the previous field before the grid takes focus. There is no shared focus flag
  for multiple dock seats. A text field keeps its own selection keys, and the
  timeline keeps its keyboard behavior when it has focus.
- Resolve Select All and Delete through the existing user keymap, including
  platform Command mapping. Select All replaces the selection with the current
  shelf's visible files; Delete uses the existing undoable media removal, rather
  than operating on an unrelated timeline selection. Modal, text-input and
  shortcut-capture guards remain in force.
- Keep the grid alive when a shelf becomes empty, including after removing its
  last file. Keyboard ownership does not disappear with the cards.
- Store the range anchor by media id. Shift replaces the range, Ctrl/Command
  toggles, and Ctrl/Command+Shift adds a range. Resolve endpoints against the
  current filtered/sorted order. A hidden/deleted anchor starts a new range at
  the target; stale hidden/deleted targets change nothing. Selection does not
  change the project schema or timeline selection.
- Walk that same visible order for marquee geometry. Preserve stable row ids,
  generated-media shelves, card dragging and the phone's tap-to-add behavior.

## Qualification

Six standalone strict-Clippy tests cover selection policy. The real Slint grid
fixture covers focus after the host blur, pointer modifier separation, two dock
seats, rebound chord delivery, empty shelves and text/timeline focus isolation, the real pane's sorting control and page changes.
Native media-bin tests check sorting, stale rows and import/generated filtering.
The existing full controller journey additionally exercises the real keymap,
modal/capture guards, timeline isolation and removal with Undo/Redo.

These checks run in headless/native CI; final run links and counts are recorded
on the pull request. Simulated input does not qualify physical Windows/Mac
latency or driver rendering. No upstream issue was changed.

## Additional upstream commit

Fetched one further Concat commit after `f1d59f3d`, through
`9d7f61b3`: brighter coral/fuchsia/lime layer colours. Retain Clovercut's existing
typed-track colours and theme/accent treatment; this is a cosmetic alternative,
not a missing functional fix. The previous 22-commit review remains applicable.
