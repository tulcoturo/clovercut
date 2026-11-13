# Advanced chroma key and source eyedropper

Imports the complete automatic screen finder, local plate/matte, edge grow and
softening, spill/light restoration and matte-view shader from upstream main
6584d349, including its later WARP loop/early-return corrections. The package
is concat.chroma-key-advanced. New removal controls choose this package.

Clovercut's original concat.chroma-key keeps its identifier, parameters,
defaults and exact shader. It is displayed as Chroma Key (Classic) under
Compatibility. This is intentional: old projects often saved a parameterless
link, so testing only for old similarity/softness keys would silently change
those projects' pixels. Both old green/blue aliases retain the original path.

The inspector offers a source eyedropper, automatic-colour reset and enum
choices including matte view. Sampling reads the clip's own source before
keying, through its crop/flips/rotation/stabilization; decode runs on a worker.
Escape cancels. Off-picture clicks leave the tool armed; a grey sample is
refused for the advanced key with a hint to Remove Black. Project epochs,
clip/media/package identity and locked tracks guard asynchronous delivery.
Returning to automatic clears both a held colour and its animation keys.

Colour sampling lives in the small host colour module and has local boundary,
finite-coordinate and screen-hue regressions. The exact old shader's 12 GPU
probes and the advanced shader's 24 probes pass on the local software adapter.
A real 480x270 catalogue card was rendered through the normal card painter and
visually inspected. All 72 effect-library tests pass. The app's actual Slint
sources compile with the frontend-only checker, including all callback/model
forwarding. All 26 locales pass strict checks; ordinary Auto abbreviations are
explicitly reviewed in the identical-term list. Native controller/window and
Windows adapter qualification remains required before publication.

The batch-edit portion of upstream 2b594e8e is tracked separately until the
Clovercut inspector's commands and per-peer effect matching are adapted.
