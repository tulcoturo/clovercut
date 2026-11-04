# Timeline fade controls — 4 November 2026

Adapt [Concat a4838588](https://github.com/jub0t/Concat/commit/a4838588c3b1c71aea54ca1d7e268db16937701e)
on top of the desktop responsiveness candidate `0f71c132` (PR #17).

Desktop audio clips and video sound bands show fade handles while hovered or
selected. Drag inward to set a fade, up to half the clip; returning to the edge
removes it. The ramp and duration label follow the pointer. Locked tracks, touch
input and the split tool do not expose these handles. Keep the existing inspector
and phone fade controls. Separate the two hit targets at the middle so maximum
fade-in and fade-out remain individually draggable.

Clovercut publishes bounded slices of very long clips. A slice's displayed
length and fade are not the original clip's duration or complete envelope.
Handles are therefore shown only when the published geometry contains the whole
clip; windowed clips retain the inspector controls and their existing partial
ramp rendering. Do not interpret a publication boundary as a clip boundary.

Unlike the upstream debounce-only gesture, start/update/finish are explicit.
Updates remain in the echo until release, even if the pointer pauses. One release
creates one undo step; another drag starts a separate step. Cancel restores the
original state. Pending edits to another clip land before the new fade starts;
unmatched target/side callbacks and non-finite updates are ignored. Existing
commands, persistence and preview/export fade behavior remain the source of truth.

Validation:

- Simulated pointer regression against the actual Clip component: fade-in/out,
  half-duration cap, reset at edge, lock, touch, bounded geometry and split tool.
- Full Slint application code generation with a 4 GiB address-space ceiling.
- Formatting and source diff checks.
- A selected native phase-3 regression is added for audio/video controllers,
  commit/undo separation, cancel, click without movement, pending unrelated edits,
  locks, malformed updates and save/reopen. It has not run locally; full native
  Rust compilation and execution remain CI qualification, not implied by Slint
  generation. The desktop gate now requires at least 14 selected cases.

No published release or physical pointer/audio qualification is implied.

## Subsequent native qualification

Full CI passed on `a6a9a352ca3575cab82ada82272e5b6e6bde8f4a`: workspace
Clippy/tests, doctests, gRPC and all 14 selected desktop journeys. The runtime
probe completed with exit zero. See the [qualification record](2026-11-04-desktop-upstream-qualification.md)
for raw observations and limits; earlier pending notes above describe the
implementation-time state. Physical Monterey interaction remains unqualified.
