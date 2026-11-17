# Batch inspector edits

Completes the batch portion of upstream 2b594e8e. Compatible selected clips now
share the inspector: text, pictures (video/still), shapes, adjustment layers,
ordinary audio, sound layers or transition clips. Clovercut separates ordinary
audio gain from sound-layer bus strength, so those controls are not falsely
shared; the existing mixed-audio volume tool remains available. Locked tracks
are excluded even for a single selection.

Controls read the first eligible clip. Numeric/shape edits retain Clovercut's
relative transforms and caption-scoped styling. Effect additions, bypass,
ordering, removal, scalar/colour/curve edits and grading apply to the compatible
peers in one undo gesture. An effect is matched by package and occurrence:
the second blur edits the second blur in each peer, skipping clips without it.
Clovercut's subject/face source ordering and transition-clip replacement are
retained. Existing animation and mask features are preserved.

Background edits cover all eligible main-lane pictures; Fit/Fill handles each
clip's own media geometry. Automatic/custom cutout choices and subject/clear
commands are batched; brush painting itself requires a single source picture.
The eyedropper samples the first shown source and writes matching captured
peers together; a changed selection does not retarget a pending read. Resetting
to automatic clears both held colours and colour keys across the same peers.
Another clip's keyed grading ride is not moved to the first clip's local time.

Pure target/matching regressions pass locally, covering order, locked tracks,
incompatible kinds, missing IDs and duplicate effect occurrences. The existing
native editor journey now checks actual two-shape width edits, effect add/knob/
bypass/remove and one-step undo restoring the exact project. That native journey
must pass remotely before publication. Earlier full app Slint frontend checks
remain valid because this block changes controller/model behavior only.
