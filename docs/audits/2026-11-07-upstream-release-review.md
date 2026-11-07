# Upstream review and desktop 0.2.3 candidate

Fetched Concat main on 7 November 2026: `483c9d6a` →
`443fb2a33648a86733c4360544cd8f291f5aeaae`, 15 new commits.
Commit divergence is not a count of missing Clovercut features: prior adaptations
have distinct commits and Clovercut retains typed tracks, caption groups,
generated-media ownership, exact export clocks and cancellable atomic publication.

## New commits

| Commit | Decision and Clovercut adaptation |
| --- | --- |
| `1ffd4f41` | Load dynamic ggml CPU kernels before Silero VAD, preserving selected audio stream, cancellation and speech-reel timing. A missing CPU kernel reports a Clovercut reinstall error rather than reaching ggml's abort. |
| `42dd9fc7`, `d65518b5` | Trial all built-in shaders, including software adapters. A canary names a package that leaves its device unusable, with driver errors logged. Add Windows WARP to the dedicated all-builtin pipeline and nine-effect behavior checks and make those checks a release gate. |
| `be75d58d` | Unsigned magnitudes prevent checked rational arithmetic from panicking on i128::MIN; preserve all existing rational timing contracts. |
| `ab8607dd` | Add 13 bundled-font language presets and bundled-glyph coverage checks. Keep Clovercut's existing vertical text and caption behavior. |
| `2b8690d1` | Platform-native affirmative/cancel order for confirmations and New Voice. Preserve destructive styling, translated labels and Clovercut's can-save checks. |
| `7b29c0f9` | Throttle replacement of a lost headless preview device to at most once every two seconds. Keep existing export fallback/cancellation/atomic output logic. |
| `f12b9b2a` | Progressive timeline edge-scroll speed and a larger wheel-enabled horizontal scrollbar. Convert absolute duration into Clovercut's local time-origin coordinates; left bound remains absolute zero. A real Lanes fixture covers a shifted origin. |
| `a8d3b31a` | A selected clip's transition wash lets group selection drags reach the clip; seam/resize controls retain their existing behavior and locks. |
| `d71dccbf` | Compare nested dropdown/dialog models by row contents; avoid replacing equal transition/export/settings/relink/start/captions data. Clovercut's key rows are plain ClipKeyData and need no upstream nested-model rewrite. Autosave publishes saved status/toasts only while retaining ordered save tickets, epoch guards, rename-recents handling and commit-warning behavior. Its translated saved/unsaved strings remain. Inline caption-source models receive the same equality protection. |
| `e7e1e03b` | Device-lost callback flags are shared by monitor/sibling compositors, so texture-only previews stop using a removed device. Preserve the macOS CPU-window/Metal-video policy and existing software fallback. A public-behavior regression checks the shared lost flag. |
| `6e73902f`, `b184a55c`, `5e77293b`, `443fb2a3` | Defer the coordinated voice-isolation feature rather than importing its UI alone. It adds three ONNX graphs, a new stem schema/command and multi-worker audio generation (about 300 MB per worker upstream). Clovercut needs project-owned stem reuse, cancellation, cache identity, global sound buses and continuous retiming integrated and qualified together. Adding that workload while investigating low-memory Windows preview performance is not a drop-in fix. Retain our current Audio inspector; no nonfunctional isolation controls are exposed. |

## Earlier pending inventory checked again

The 5 November inventory remains linked, with these decisions reconciled:

- Workspace persistence, missing-media removal/Undo, Enhance progress, appearance
  layout, playhead palette, home-guide affordance and macOS window safe start are
  already adapted. Span-aware Enhance estimates exist; keep 4096-pixel support.
- `773efd12`'s caption-library placement has a Clovercut equivalent, including our
  collection/selection handling; do not replace it with upstream's panel model.
- `e938cfdf` changes export pipelining and GPU YUV readback across six crates.
  It is not the reported window-preview bottleneck. Retain current alpha/HDR,
  exact-clock, recovery and atomic publication behavior until the whole pipeline
  has its own parity/throughput qualification; no unmeasured speedup is claimed.
- `25cf5073` CapCut ingress needs a separate path/fixture review and Clovercut
  ownership/typed-track conversion. `0b1f125d`/`c009d037` shapes require a matching
  project, inspector and MCP schema change. These are not safe partial imports.
- `80ed9457`/`0b94e48f` full JSON Schema output must describe Clovercut's extended
  replies/commands and owned media rather than upstream's smaller contract.
  Existing MCP request schemas remain authoritative; no incomplete new contract
  is exposed in this release.
- `b5e19da2` restart needs the previously documented recording/export/lock policy;
  do not substitute a naive save-and-exec path for Clovercut's lifecycle.
- `761a0233`, `8aa44956`, `87017346`, `29814cd8`, `784021e7` were inspected as
  upstream codec/test/lint maintenance. Retain our x264/x265 codec policy, current
  VFR/PTS tolerances, document version and speech implementation; no test-bound
  relaxation is imported to make CI pass. Current Clippy/test gates validate our
  source independently. Upstream branding, licensing and donation presentation
  remain project-specific.

## Windows preview discussion

[Discussion #28](https://github.com/tulcoturo/clovercut/discussions/28) reports an
unusable preview on a Windows laptop without a dedicated GPU, including at lower
quality. No adapter/driver, media codec, project or timings are provided. An
integrated GPU is not the same as the WARP software renderer; the report does
not establish which is active or the root cause.

The actual published Clovercut v0.2.2 source was fetched into the separate
`origin/tags/v0.2.2` ref (`b358a99deb8a8d31fb510bda310022e5b55cef6a`), avoiding the
same-named inherited Concat local tag. That source predates current approximate
scrubbing/exact rest refinement. Those fixes, lower UI publication churn and the
new lost-device guards are included in this candidate. Proxies remain opt-in;
no background re-encode is enabled behind the user's back. Windows WARP shader
checks establish pipeline correctness, not usable playback on the reporter's
hardware. Keep the discussion unresolved until a built candidate can be measured
on the affected machine; no reply was posted on the user's behalf.

## Effect integration corrections

The first complete native run on `a0aae21d` compiled successfully and ran 1352
ordinary tests: 1350 passed; two integration tests failed. New bundled cards
were 320x180 while the established card contract is 480x270. They are now freshly
rendered at native card size. Two Light Leak probe expectations had been derived
for a 16x16 target, while the package CLI and general renderer suite use 8x8.
Their independent analytic expectations and the focused GPU fixture now use
that same canonical target; no tolerance was widened or assertion removed.

## Release gates

Release 0.2.3 requires strict workspace/native tests, selected desktop journeys,
Linux and Windows software-adapter effect checks, all six desktop target builds,
native startup/codec smoke checks and twelve nonempty packages with manifest and
checksums. Physical Windows laptop performance and physical Monterey operation
remain separate from hosted-runner qualification. Publication and links are not
reported as available until verified.

Local candidate checks passed: 406 core/project/effect/text unit, integration and
documentation cases; strict Clippy for those crates; actual publication-model
identity checks; origin-aware timeline/scrollbar interaction; actual hover checks;
full Slint source generation; strict 25-locale inventory and nine locale-tool
tests; native-size JPEG metadata; and the corrected canonical 53 GPU probes.
Full native workspace and multi-platform package results remain release gates.

## Hosted candidate findings

The e6205568 dedicated effect workflow passed on Ubuntu and Windows, including
all 125 built-in visual pipelines and health canaries and all nine-effect/53-probe
checks. The Windows report's adapter is retained with the downloaded evidence;
this qualifies its software-renderer pipeline, not the discussion reporter's FPS.

The release's Windows ARM64 job failed before compiling the app: BtbN's old
24 October FFmpeg SDK URL returned 404 after retries. Replace only ffmpeg-winarm64
with the official dated 7 November SDK, still FFmpeg 8.1.3/GPL/shared. The digest
`e6654a726f7c43c149bd3c958ac052aba80434e59a79e282b0339fbb83bc087c` matches the official
GitHub asset digest and an independent local download. ZIP SDK headers and every
DLL's ARM64 PE machine type were checked. Retain checksum enforcement; do not
substitute an unchecked latest URL or disable required ARM64 packages.
