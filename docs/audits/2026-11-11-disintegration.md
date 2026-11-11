# Subject disintegration coverage correction

Clovercut baseline: `843fbd1e` (PR #55).

The previous shader set an internal alpha to zero when a subject pixel dissolved,
but used that alpha to mix its RGB back toward the original source and returned
`base.a`. A real packaged-shader render at 100% therefore retained alpha 192 for
a source with alpha 192 instead of removing the subject.

The corrected shader removes the matte-covered share of source alpha, retains
coverage outside the subject, and composites ember particles in premultiplied
space before returning straight RGBA. It normalises its sweep for wind direction
and image aspect, and fades embers and particles out at both progress endpoints.
At 100% the subject reveals the lower layer/canvas. This does not reconstruct the
background hidden behind the person in the original footage.

The regression runs in the existing creative-effects harness on Linux Vulkan
and Windows WARP CI, with a supplied subject matte. It checks nine wind angles,
0/50/100% progress, missing and moved mattes, fractional mask/source coverage,
transparent input, preserved distant background and composition over a canvas.
Optional CI artifacts include colour and alpha renders at all three progress
values. This supplements the existing missing-matte package probes.

Local validation used one low-priority compiler job and Vulkan llvmpipe, without
building or launching the full application. The regression failed on the original
shader (expected alpha 0, actual 192) and passed after the correction. The harness
also validates every builtin pipeline with device-health canaries and existing
creative-effect probes. Windows validation is reported by the PR's CI separately.
