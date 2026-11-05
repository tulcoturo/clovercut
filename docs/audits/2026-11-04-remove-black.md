# Remove Black — 4 November 2026

Adapt the independent `concat.remove-black` package and catalogue card from
[Concat 2b594e8e](https://github.com/jub0t/Concat/commit/2b594e8e241719ef2b5dd6b6d0566426e385e7ec).
The package appears in Cutout using the existing effect catalogue and shared
preview/export shader pipeline. Black level removes lifted black/noise; strength
controls the coverage of the remaining light. Existing chroma-key packages and
saved parameters remain unchanged by this adaptation.

Validation: 72 effects-library tests passed, strict effects Clippy (all targets)
and renderer Clippy (library) passed, and formatting passed. A local adapter-required
probe executed all 11 package shader fixtures and the committed composition
regression body against Vulkan llvmpipe (software), without skipping. Black,
opaque white, half-transparent white and dim red retain the intended alpha;
stacking the default effect preserves the foreground colour and coverage.
These are shader/compositor checks, not a complete desktop build or exported-file
qualification. The full native candidate remains separate from PR #17 while its
CI is running.

The larger upstream chroma-key replacement needs additional compatibility work:
Clovercut saves default links with no explicit parameters, so detecting legacy
links only by removed `similarity`/`softness` keys is insufficient to preserve
all existing projects. Do not silently replace that renderer while importing this
independent new package.

Native runs 41886591753 and 41888173961 compiled successfully but each failed
one ordinary regression: the imported square card was 360×360 rather than
Clovercut's required 480×270. The runs passed 1336/1337 and 1340/1341 ordinary
tests respectively; neither completed the later full qualification stages.

The card is regenerated at 480×270 using the current `concat_host::cards`
`Card::of` and `Painter::with_city`/`draw_to` implementation, bundled reference
images, actual Remove Black shader and Vulkan llvmpipe. The resulting JPEG was
visually inspected. This uses the standard card rendering path, rather than
relaxing the existing dimensions regression. Fresh native qualification is
required for the corrected candidate.
