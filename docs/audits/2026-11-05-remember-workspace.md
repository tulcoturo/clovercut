# Remember desktop workspace

Adapt Concat `f500664b`, retaining Clovercut's six layout presets and Reset
semantics. Settings > General offers Remember layout, enabled for legacy
preferences by default. Turning it off clears stored geometry and docks; future
launches use the default arrangement. It does not rearrange the current session.

Saved data holds named preset identity, wide/compact dock trees, normal logical
window dimensions and maximized state. A custom arrangement restores independently
of its preset, so Reset still rebuilds the selected preset. Unknown views,
nonfinite ratios and overly deep dock trees are rejected; ratios are clamped.
Invalid layout preferences go through existing per-field recovery, preserving
unrelated settings. Missing or unreadable docks fall back to the chosen preset
and compact default. Size is bounded on restoration; the existing platform screen
fit can maximize a window too large for the current monitor.

Dock edits and window resizing schedule a 600 ms debounce, including the launch
screen. Resize borrows are deferred to avoid reentrant publication. Writes only
occur when the snapshot changes. Normal window-close paths and version switching
flush layout. A maximized/fullscreen/tiny window does not replace the last normal
size. Phone shells neither expose the option nor save desktop layout.

Serialization data is separated from generated UI types in workspace_layout.rs.
The existing storage-only checker compiles this actual module and preferences;
there is no copied layout implementation or fake UI fixture.

Validation: storage/preferences suite passed 53 tests plus strict isolated Clippy;
new preference tests cover legacy defaults, named preset round trip, disabled
state and invalid-layout recovery preserving unrelated fields. Full application
Slint source generation passed under a 4 GiB address-space ceiling. Strict locale
inventory passed all 25 locales (1115 messages on this branch); nine locale-tool
tests and formatting passed.

Native dock tests additionally cover all six presets, compact round trip,
unknown views, nonfinite ratios, depth rejection and bounded ratios. The existing
editor journey now exercises startup restoration, preset Reset, saving while
compact and disabling persistence. These native checks remain pending full CI;
physical window placement/maximization across displays is not qualified locally.
