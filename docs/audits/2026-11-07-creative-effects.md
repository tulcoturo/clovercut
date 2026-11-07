# Creative visual effects and animated catalogue thumbnails

Adds nine new visual effects to the 24 existing effects (33 total). Existing
filter IDs, including concat.vhs, retain their original implementations and
parameters. These are built-in format-2 WGSL packages, carried by the same
project chain, parameter/keyframe resolution and compositor used for preview
and export. There is no downloaded plug-in, native dependency or API change.

| Package | Controls |
| --- | --- |
| Camera Shake | Amount, speed, rotation; bounded automatic overscan |
| Digital Glitch | Amount, update speed, horizontal blocks |
| VHS Tape | Amount, tracking distortion, speed |
| CRT Scanlines | Amount, spacing, softness |
| Old Film | Amount, grain, film wear, speed |
| Kaleidoscope | Amount, segment count, rotation, zoom |
| Tilt Shift | Amount, focus position/width, rotation |
| Light Leak | Amount, position, width, speed |
| Neon Edges | Amount, line width, background detail |

All new controls support animation. Distortion and blur sample with alpha-aware
accumulation. Pointwise treatments retain coverage; no new effect turns a fully
transparent clip opaque. Geometry/blur may move or spread existing coverage.
Zero Amount is the original picture, including extended-light colour and alpha.
Creative treatments may deliberately change the colour or luminance; they do
not claim a neutral HDR grade. No limiter clips their shader result at SDR white.

Catalogue cards extend the existing transition worker to effects and filters.
The 180 ms hover debounce, 24 frames of 192x108, one worker, cancellation by
request generation, and four-card memory cache remain. Static effects demonstrate
before/after strength; temporal shaders also advance local time over 1.5 seconds.
The cloned preview does not mutate saved settings or the actual monitor. No
hover animation starts on a software adapter. Bundled stills cover the new
packages before local card rendering. Retargeted/recycled cards cancel their
previous request. The existing global name remains for GL transition compatibility.

## Validation

- 72 catalogue/parser/manifest tests pass. Strict isolated effect and GPU-fixture
  Clippy pass. Shader parsing and Naga validation use the actual built-in loader.
- On Vulkan llvmpipe, all nine effects pass 53 pinned scene-linear float probes,
  spatial-change checks on a chart and the existing city reference photo, zero
  Amount, min/max settings, opaque/transparent coverage, temporal change and
  clip-local timing. Stacking in opposite orders produces different frames.
  Software rendering is used deliberately for these checks; it is not evidence
  of physical-GPU performance or the application's software-hover behavior.
- Actual rendered photographs were reviewed for each new effect. Preview stills
  are generated from the shader output, not an illustration of its intended look.
- The actual Slint catalogue-card fixture passed strict Clippy and execution for
  effects, filters and transitions: hover debounce, one request per hover, card
  retarget cancellation, leave cancellation, quick crossing, disabled cards and
  no accidental application.
- Full application Slint source generation passed with a 4 GiB address-space cap.
  Native Rust/UI compilation, host Painter tests, and desktop CI remain separate
  gates. This document will be updated with their exact tested commit/results.

New GPU checks run in Visual effect quality checks with a required adapter;
missing adapters fail rather than silently skip. Actual catalogue-card UI checks
exercise hover, debounce, retarget/cancellation and disabled states separately
from native compositor behavior. They do not establish mouse testing in the
installed app, a new published release or physical macOS qualification.
