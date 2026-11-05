# GPU initialization candidate

Selective adaptations of Concat `4ab430e6` and `f0587664`.

The window and standalone compositor use one shared instance-flags function,
which removes only indirect-call validation from wgpu defaults and then applies
wgpu environment overrides. Clovercut uses no indirect draw/dispatch calls
(source search across renderer and window GPU code). This avoids building the
unused startup validation pipeline reported to fail on older Metal compilers.
`WGPU_VALIDATION_INDIRECT_CALL=1` restores that validation for diagnosis.

Both devices also share resource-creation/device-loss budget thresholds of
95%/99%. Vulkan and Direct3D 12 can reject allocations through existing error
handling and export recovery; Metal ignores these limits. No new recovery
implementation or allocation-pressure performance claim is made.

The broader macOS raster-window safe-start changes from `4ab430e6` are deferred:
they require separate backend, tripwire and actual macOS launch qualification.
No deployment target, package minimum or encoder choice changes here.

Validation: strict renderer-library Clippy and formatting passed. On Vulkan
llvmpipe, an adapter-required helper opened the compositor and passed all 11
Remove Black shader fixtures plus transparent/stacked composition, both with
default flags and with indirect-call validation explicitly enabled. These are
software Vulkan checks, not Metal/physical GPU, VRAM-pressure or native window
qualification. Full native CI remains pending.
