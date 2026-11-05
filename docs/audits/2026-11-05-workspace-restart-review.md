# Workspace persistence and restart review

Reviewed upstream `f500664b` and `b5e19da2` against Clovercut's existing source.
These are implementation decisions for subsequent changes, not shipped features.

## Remember workspace

Clovercut has `LayoutPreset`, `current_layout_preset`, `set_layout_preset` and
`reset_current_layout` beyond upstream's default arrangement. A direct copy of
upstream SavedDock does not preserve the selected preset. Store preset identity
with the wide/compact dock snapshots so Reset continues to rebuild the user's
chosen preset after reopening. Preserve all current PaneKind variants.

Use stable named views, finite ratios, depth limits and bounded ratios. Reuse
Clovercut's existing per-field preferences recovery (`prefs::load`/repair), so a
bad or future layout cannot discard unrelated settings. Exercise unknown view,
invalid saved layout, missing legacy fields and reset-after-reopen with existing
preference/dock tests rather than a parallel fake layout implementation.

Save settled geometry with debounce and flush on normal close. Store logical
unmaximized dimensions and maximized state separately; clamp restored dimensions
for the current display. Keep compact and wide layouts distinct. Turning Remember
off must clear saved state and restore default startup behavior without repeated
settings writes from each publish/drag tick. No monitor-position persistence is
assumed from upstream: its SavedLayout stores size/maximized state, not x/y.

## Restart for startup-only settings

Reuse Clovercut's save-before-exit flow and its failed-save relaunch regression.
Offer Restart now/Later after a startup-only preference change. Starting another
process must follow a successful close/save; a save failure keeps the current
window. Preserve project path arguments as OsString and platform launch rules
(bundle on macOS, AppImage when applicable, executable otherwise).

Review recording, active export, jobs and project lock release before adopting
upstream's delayed shell launch. Check launch target before hiding the window,
and report failure rather than silently losing the session. Actual process
restart needs a dedicated opt-in journey; ordinary tests should inspect launch
plans or use an injected launcher, without starting a second editor instance.

These changes are separate from the merged GPU initialization fix: Metal startup
on the affected hardware remains unqualified, and offering a restart cannot
establish that the hardware failure is fixed.
