# Concat adaptations

Clovercut selectively adapts fixes from [Concat](https://github.com/jub0t/Concat).
A commit appearing in the upstream-only log may already have an equivalent
Clovercut implementation. Preserve typed tracks, caption collections, generated
media ownership, cancellation, atomic exports and Clovercut's existing UI.

## Current review

A 5 November fetch found 24 additional commits through `483c9d6a`.
The [new inventory](audits/2026-11-05-upstream-triage.md) is an initial triage;
[GPU initialization](audits/2026-11-05-gpu-initialization.md) passed full native
CI with 1341 ordinary, 30 gRPC and 14 virtual-desktop tests. Physical Mac startup
and VRAM-pressure behavior remain unqualified. The earlier detailed review below remains applicable.

Reviewed through `8c86502d` (4 November 2026). The
[full inventory and decisions](audits/2026-11-04-upstream-review.md) covers all 51
commits after the previous `f2a26081` review and the earlier feature backlog.

The current candidate adapts scrubbing (`75919f5e`), VideoToolbox quality
(`5ab7fd14`), shared-device export recovery (`359839d7`), selected dependency
updates (`d64e7f25`) and the idle-cache portion of `f19fda34`. Full CI passed on
`a6a9a352`, including 14 desktop journeys and the complete runtime probe
([qualification](audits/2026-11-04-desktop-upstream-qualification.md)). Decoder thread counts and allocator policy retain their current values.
The timeline-tab scroll fix already has a Clovercut equivalent (`9ca0134b`).
Upstream codec removals, speech replacement, licensing changes and the transition
rewrite are separate decisions, not automatic imports.

The next candidate adapts direct timeline fades (`a4838588`) with explicit
commit/cancel gestures and bounded-geometry protection. See the
[adaptation record](audits/2026-11-04-timeline-fades.md). Native CI passed as part
of the same `a6a9a352` qualification.

The independent [Remove Black package](audits/2026-11-04-remove-black.md),
[spoken-language selection](audits/2026-11-04-caption-language.md) and
[speech-reel timing](audits/2026-11-04-speech-reel.md) are now integrated.
The [combined native qualification](audits/2026-11-05-caption-effects-qualification.md)
passed 1341 ordinary, 30 gRPC and 14 virtual-desktop tests. Physical Mac and
recognition-quality checks remain separate. The existing chroma-key renderer
is retained and the larger inspector rewrite remains separate.

The qualified [AAC tail adaptation](audits/2026-11-04-aac-tail.md) selectively
adapts packet-end trimming from `536a416d`, preserving encoder selection and
Opus/PCM handling. Full native CI passed on `b79af76f` with 1343 ordinary, 30 gRPC and 14
virtual-desktop tests. Platform encoders are not yet adapted or qualified.

### Earlier quick-split review

Reviewed through `bfb39ca4` (28 October 2026). The new desktop quick-split button
is adapted through Clovercut's existing split callback, retaining selected/active
and typed-main-track targeting, locked-track protection and undo. It uses the
shared keyboard-operable IconButton and existing translated tooltip, stays inside
the lane viewport at its edges, and hides during hand-tool panning and touch use.
The Urdu update `e03ae41d` needs no import: all 32 affected messages are already
translated, including Clovercut-specific wording.

Validation: full Slint UI code generation passed with two CPU cores and a 4 GiB
address-space ceiling; the strict locale inventory passed for all 25 languages
(1,095 messages). This follow-up has not rebuilt the desktop executable.

The six commits after `9e20c562` through `3d549940` change only the upstream README and donation/support presentation; no additional application code is present. The earlier comparison adds eleven
commits after `94f4255f`; the earlier adaptation batch starts at `2429c0ca`. Adaptations below are newer than the source used by the published
0.1.6 release and are included in desktop release 0.1.7.

### Desktop 0.1.8 follow-up

A fresh fetch on 26 October still points to `9e20c562`; there are no newer
upstream main commits beyond the reviewed batch. This follow-up stays outside
the published `v0.1.7` tag.

These follow-up adaptations are included in desktop 0.1.8. Their original
validation records below describe checks at implementation time. A subsequent
full Linux desktop build of `10a53e84` completed in 38 minutes with the established
resource limits; its native smoke check passed on the RX 6600 XT, including
Whisper and the required audio filters. Desktop packaging and release CI validate
the versioned source separately.

The independent ColourField correction from `870290a0` is adapted: the alpha
swatch alternates by row plus column, so transparency is shown over a checker
rather than horizontal stripes. This does not introduce project transparency or
change export formats by itself. The coordinated canvas/alpha adaptation below
is a subsequent unreleased feature.

The next adaptation combines `643bbe1f`, `870290a0`, `23bc6899` and `470391e7`:
per-timeline RGBA canvas, shared background picker and checker previews, ProRes
4444/MOV/AAC and VP9-alpha/WebM/Opus. Preserves typed main tracks, independent
main-lane/clip backdrops, cancellable audio mixing, unique per-run scratch names
and atomic output publication. The existing ProRes matrix fallback is retained
once. The old six-argument project creation helper remains available alongside
explicit canvas creation. Active-document mirrors also carry the canvas.

Additional adaptation fixes premultiplied filter-stack reuse, fractional-strength
interpolation and repeated canvas clearing. Prepared pictures and treatment
kernels receive straight colour, then return to premultiplied composition.
GPU-kept transitions use the same straight-colour contract as readback transitions.
CPU-fallback preview upload uses a
clear ground because its frame already contains the canvas. Uncovered coloured
and transparent frames stay visible even without a clip under the playhead.

Validation: 807 engine regressions passed across core, project, media, render,
export, host and API, followed by all 61 GPU regressions with a real adapter
required. The latter includes three further regressions (810 distinct passing
tests overall): transparent transition endpoints/readback parity, prepared
transparent pictures and straight-colour filter processing. The alpha export
with sound also passed again after those changes. Engine Clippy runs with
warnings denied. Slint code generation, the strict 1,089-message inventory in
all 25 locales and nine locale-tool tests pass. Optional live-provider,
model-download and foreign-platform tests retain their prerequisites.
A full desktop Rust build and native Windows/macOS runtime validation have not
been performed for this follow-up; these results do not produce a new executable.

The inspector fixes `cdbdf074`/`b613ba62` were checked again against Clovercut's
cached kind/batch selection properties. Clovercut's tab labels already depend
on these properties rather than animated clip values, while retaining its extra
caption and animation tabs. No replacement of its tab model is warranted from
this inspection; playback interaction still needs runtime verification.

### Per-word text follow-up

Adapted `64b25e90` for desktop titles and captions: select characters in the
Text inspector to change their colour, weight, italic, underline, strike or
relative size. The title's base style stays independent. Runs follow Unicode
characters through edits and casing, participate in undo/redo, project persistence,
typed clipboard copying, presets and the MCP tools.

Clovercut retains vertical CJK/rotated Latin shaping, transformed colour glyphs,
caption collection scopes, locked-track protection, vertical alignment, expanded
ink canvases and bounded shadow rendering. Per-word changes target only the
selected caption. Generated captions and defaults discard runs from unrelated
source words. The title cache version advances from 8 to 9.

Validation: 490 distinct passing regressions across project, text, host and MCP,
including typed clipboard copying, caption-local formatting, undo/reopen, Unicode
case remapping, vertical mixed-script runs and preview/export title caching.
Eight optional provider/model/platform cases retain their prerequisites. Strict
engine Clippy, Slint code generation, all 1,093 messages in 25 locales and nine
locale-tool tests pass. No new full desktop executable or native Windows/macOS
runtime validation was produced by this follow-up.

### Track-linkage follow-up

Adapted `1385c36b` to the fixed typed main video track, with opt-in timeline
state rather than silently enabling it in existing projects. The desktop chain
button, View menu, API and MCP use the same undoable command. New timelines
inherit the active timeline's choice. The MCP summary explicitly reports it.

Removed spans from selected tracks and the main track are unioned in original
coordinates. Linked trim/speed batches resolve once, independently of selection
order; groups of ripple-delete commands coalesce before applying. This corrects
the upstream double-ripple defect. Transient window locks reach the project
engine through scoped edit methods and do not enter project serialization.
Locked followers are neither shifted nor deleted. The fixed main remains the
anchor even when empty or when audio lies below it. Crossing beds remain intact;
following clips cannot be pulled over their tail on the same typed lane.
Linked trim previews run the same command engine from the saved gesture state.

Validation: 462 passing regressions across project, host and MCP include 14
new linked-edit scenarios: multi-track removals, trim/speed batches, insertion,
freeze frames, speed curves, crossing beds, locks, undo/reopen and preview/commit
parity. 8 optional provider/model/platform tests retain their prerequisites.
Strict engine Clippy, Slint code generation, all 1,095 messages in 25 locales
and nine locale-tool regressions pass. No new full desktop executable or native
Windows/macOS runtime verification was produced in this follow-up.

### Global audio-bus follow-up

Adapted `2606aff1` to Clovercut's typed adjustment tracks, retaining audio below
the fixed main track and the existing click-to-apply behavior. Dragging an audio
effect creates a generated `soundLayer`; visual effect/category behavior and
compact track heights stay intact. Existing typed clipboard copying handles the
complete sound-layer state instead of importing upstream's partial reconstruction.
Old commands omit `sound` and retain visual-layer behavior.

The bus treats the summed soundtrack for its span, with dry/wet strength, fades,
ordered overlap and opacity-keyframe curves. Curves use one sample clock on both
wet and dry branches so filter frame sizes cannot misalign their gain. Preview
and export share the same mixer. Window cuts happen after source retiming,
effects, gain and fades, retaining continuous Rubber Band ramps and their
original clock; bus windows retain filter and fade history from their start.
Clovercut's stream-specific preroll, sample-accurate delay, PCM WAV and Opus/AAC
encoder policy, cancellation and atomic export publication are preserved.

Preview jobs carry cancellation tokens across edits and project changes; an old
job cannot remove a newer job's token for the same content key. Bus cache identity
includes the underlying gain/fades/tempo and layer chain/strength/keys/order.
Unfinished mixes leave ordinary clips audible and only replace them when ready.
Generated bus spans cannot borrow or copy plain clip samples. Unique scratch
files, checked flushes and atomic cache publication protect concurrent windows.

Validation: 671 distinct passing regressions across project, media, export,
host and MCP cover whole-mix treatment, ordered buses, source gain/tempo/fades,
preview/export windows, keyed mix strength, cancellation, stale jobs and
concurrent cache publication. Ten optional provider/model/platform tests retain
their prerequisites. After stereo normalization, 26 audio-mixer regressions,
the sound-layer export integration and 18 playback regressions pass again,
including mono sources and effects. Strict engine Clippy, Slint code generation,
all 1,096 messages in 25 locales and nine locale-tool regressions pass.
The native packaging smoke check now verifies the required global-mix filters.
No new desktop executable or native Windows/macOS runtime validation was produced.

### Latest adaptations and pending work

The eleven commits below were compared with Clovercut. Status is explicit: the
first batch adapts independent correctness fixes; the timeline redesigns remain
separate. These selected adaptations are included in release 0.1.7; they are absent from the older 0.1.6 executable.

| Commit | Assessment |
|---|---|
| `153cce29` | Adapted: both Kokoro archive sizes and digests updated in the manifest and compiled speech table, independently checked against the official sherpa-onnx release metadata. Existing installed models and speaker IDs are retained. |
| `e0111c01` | Adapted to Clovercut: compatible PCM spans play immediately after a cut and plain spans copy from the mapped recording. Preserves project-change cache isolation, disk-backed PCM, gain curves, stream selection and the existing continuous-speed-ramp engine; nonlinear retimes are deliberately excluded from linear source reuse. Audio-stream seeks and post-seek stream discard include MPG, TS and Matroska regression fixtures. Deferred upstream sound-layer features are not imported incidentally. |
| `c2847c16` | Adapted: environment/SOCKS routing, Windows per-user proxy/PAC and fixed macOS proxy settings; useful failure classification; resumable downloads retain range headers; Freesound retains header authentication; all loopback Audio API traffic bypasses proxies; signed query strings and proxy credentials are excluded from new diagnostics. Explicit proxies are respected without an automatic direct retry. Updates still use shipped trust roots. Model download settings now show translated failures and offer browser download/manual import. Chosen files use unique temporary copies and checksum verification before installation; failed or cancelled imports preserve the original and any partial download. Automatic browser downloads prefer the official source over the unpublished release mirror. |
| `83a237e8` | Adapted and extended: per-call SVG temporary files avoid concurrent decoder races; relative images resolve beside the SVG. Cache identity also includes linked-image metadata, including missing images that subsequently appear, with namespace-aware SVG/SVGZ parsing. Concurrent-render and linked-bitmap-update regressions pass. |
| `268cd601` | Adapted across text, title caches, render planning and export. Expanded canvases preserve overflow; caption reveal coordinates, bounded shadow rendering, transformed colour glyphs and byte-limited live-title caching are retained. The title cache version is bumped so old clipped pictures are not reused. The independent per-word rich-text model is adapted in the later follow-up above. |
| `a6e5ab77` | Adapted for macOS: save/stop-export handling precedes leaving past whisper/Metal static destructors. Native macOS runtime verification remains required; Linux engine tests cannot validate that quit path. |
| `23897dd8`, `3564d450` | Adapted: Up/Down navigate previous/next cuts across all tracks through the shared menu handler and customizable shortcut profiles. Uses the actual fractional project rate, rather than upstream's rounded rate. Added 4:5/3:4 presets to project creation, settings and monitor without changing existing preset indices. |
| `9e20c562` | Adapted to typed tracks: first opening centers the actual main video row, including audio below it. Explicit vertical scroll is retained per timeline during the session; resizing clamps it to the available stack. The expanded ruler/scrub band remains intact. Compatible tray tooltips are shortened in all 25 locales. |
| `1385c36b` | Adapted in the track-linkage follow-up above. Original upstream review: optional linkage should respect the fixed typed main track, grouped captions, multi-lane operations, locks and undo. Upstream defaults linkage on and identifies the main lane dynamically. A mechanical reproduction using its exact `close_gaps`, `removed_before` and `follow_main` helpers confirms a double ripple when both main/text lanes lose the same two-second interval: remaining text at 5 s lands at 1 s instead of 3 s. Do not import this implementation unchanged. |
| `1e683882` | Phone-specific interface: do not bring into the desktop-focused editor. Ordered multi-selection may be considered separately if a desktop use case is missing; preserve typed placement and existing multi-file import. |

Validation of this adaptation batch: 493 ordinary engine regressions across
text, export, host, media and render; 65 core and 15 speech tests; and two opt-in
live-provider tests covering OpenGameArt, YouTube and Freesound search, downloads,
media probing and cache reuse. The model manifest matches all 23 compiled models.
Native macOS quit behavior and Windows/macOS proxy integration require their own
platform validation. A subsequent full desktop build and startup check are recorded below.

The desktop follow-up adds checksum-verified manual model import, cut navigation,
4:5/3:4 presets and typed-main-track opening. Its targeted checks cover 164 host
and 15 speech regressions, three standalone desktop-navigation regressions,
engine Clippy with warnings denied, Slint code generation and the strict
1,083-message inventory in all 25 locales. Nine locale-tool regressions pass.
The manual-import regression covers valid, wrong and cancelled copies, retaining
the original and previous partial and leaving no staging files.
The initial Rust window check stopped at its memory guard. The full Linux desktop
application subsequently compiled successfully from `637c15df`, using the existing
`quick` profile and temporary disk swap. A fresh-profile startup check passed on
the AMD Radeon RX 6600 XT Vulkan adapter and initialized Whisper successfully.
The resulting local executable includes these adaptations, which subsequently
shipped in desktop release 0.1.7.

### Previously adapted/reviewed batch

| Commit | Change | Clovercut assessment |
|---|---|---|
| `c63c7ecf` | Direct3D effect sampling | Adapted: effect and transition helpers use explicit mip level zero, avoiding implicit derivatives in parameter-controlled loops. Keeps the existing shader contracts and single-mip pictures. |
| `23bc6899` (decoder only) | ProRes colour matrix | Adapted independently of alpha export: untagged ProRes frames inherit the stream's matrix before colour conversion. Other codecs and explicitly tagged frames retain their behaviour. |
| `e6bfd3fc` | New-project notice | Adapted in all locales, referring only to Clovercut's editable frame size and frame rate; does not promise the upstream canvas control. |
| `7db37f2a` | Enhance only the used source span; reuse identical frames | Adapted with a one-second margin, span-specific cache identity, original/copy offsets, preview alignment and rejection when a clip changes source while the job runs. Keeps cancellation-aware audio mixing/muxing, bounded decoder/encoder threads and atomic publication. Old enhanced links default to offset zero; clipboard retains the offset. |
| `f55451e6` | Default title font | Already equivalent: Clovercut's model and UI use bundled Hanken Grotesk, with regression coverage. |
| `cdbdf074`, `b613ba62` | Stable inspector labels during playback | Clovercut already derives tab choice through cached kind/batch properties and has additional caption/animation tabs. Reproduce any remaining model churn before adapting a held label model; never copy upstream's smaller tab list. |
| `aa38b9d5`, `84c0e625`, `94f4255f` | GPU Enhance and runtime engine selection | Adapted together: compact-network compute shaders, fixed-shape CoreML and runtime timing of both engines, including CPU on Linux/Windows. Every GPU candidate must execute successfully; initialization errors are captured, and a later GPU failure switches lazily to ONNX Runtime. GPU weights must match the pinned model digest. Keeps the span/cancellation/atomic-copy adaptations above. |
| `fa2c5168`, `13e67ea2` | Import saved voices with metadata and automatic sample placement | Adapted into the existing saved-voice shelf: import audio/video recordings, retain a local sample with name/picture/description and suggest its start from streamed audio levels. Closing or replacing the recording cancels analysis; request identity prevents stale replies overwriting a reopened dialog. All eight new messages are translated in the 25 locales. Speech engine feature gates are unchanged. |
| `870290a0`, `23bc6899`, `470391e7` | Transparent canvas and ProRes 4444/VP9 alpha export | Adapted together in the unreleased follow-up above, preserving Clovercut audio and atomic export behavior. |
| `b237687c` | Concat application identifier migration | Not applicable: retain Clovercut's identifier, portable mode and storage paths. |

A protected local GPU/ONNX comparison used a 272×64 source crossing two tiles,
2× output and a two-core CPU quota: GPU 0.089 s versus ONNX Runtime 1.742 s,
mean difference 0.101 and maximum 2 levels out of 255. These are host-pipeline
test measurements, not a full-video benchmark or a promise for other machines.
The real automatic selector and its output were exercised separately in the same
test. Ordinary tests need no model download; the real-model comparison is opt-in.

The span regression exercises the real decode/encode/audio pipeline with a small
deterministic picture processor, so it needs no Enhance model download. It checks
picture and sound start times, output duration, exact-frame reuse and cancellation
cleanup. Separate tests cover ProRes colour, old enhanced links, toggling copies
and preservation through Clovercut's typed clipboard.

### Previous batch

| Commit | Change | Clovercut assessment |
|---|---|---|
| `638b0432` | Write Save Audio As WAV as PCM | Adapted: `.wav` (case-insensitive) now uses packed 16-bit PCM, with 1024-sample blocks and an exact final sample count. AAC remains unchanged for other destinations. Keeps Clovercut’s cancellation-aware mixer, fades, volume keys, tempo and stream selection; does not transplant upstream’s audio-bus mixer. |
| `b5042b5c` | Seek trimmed audio backwards on its own stream | Our existing one-second preroll already passes all four new MPEG-PS/TS regression cases, both in preview and saved M4A: tone onset 0.000125 seconds after the cut. Keep the current solution for now. Stream-specific seeking may still improve robustness/performance in other containers, but these fixtures do not demonstrate a remaining Clovercut bug. |
| `24f05885` | Bottom-aligned waveform bars | Adapted: bottom-aligned waveform bars and one upper hot-level band across the shared Wave component. Keeps existing peak caches, zoom/trim sampling, gain-only layout updates, compact track sizes and the existing bottom baseline. |
| `20dcd9aa`, `bc05ea3c`, `758da5a8` | Typography case, decoration and hover labels | Adapted only missing case and strikethrough controls. Reuses Clovercut’s accessible B/I/U controls and translated hover labels; preserves vertical text, caption-group/batch targeting and existing underline rendering. Case changes are display-only and reversible. The five new labels are translated in all 25 locales; no upstream inspector replacement. |
| `64b25e90` | Per-word rich text | Adapted in the unreleased per-word text follow-up above, preserving vertical text, caption scopes, typed copying and bounded overflow rendering. |
| `643bbe1f` | Project-wide frame background colour | Adapted in the unreleased canvas/alpha follow-up above; independent of existing main-lane and per-clip backdrops. |
| `4e207179` | Put wheels/curves after scalar adjustments | Conflicts with our deliberately compact Color/Curves/Wheels navigation and collapsible groups. Preserve our layout. |
| `b44bbf5e` | Remove the one-card Titles shelf | Adapted selectively: Styles opens first and includes the existing default-title click/drag card above its presets; Languages remains separate. Removed the redundant Titles category. Unlike the fixed Clean preset, default insertion retains the saved title style and localized content. |
| `2429c0ca` | Formatting and canvas callback wiring | Relevant wiring belongs to the new canvas feature above; the unreleased canvas follow-up includes the needed desktop wiring. No standalone change needed. |

## Integration details

WAV uses packed PCM S16LE in 1024-sample blocks with an exact final sample count;
M4A keeps AAC. The existing mix graph retains trim, speed, gain, fades and
cancellation. Clovercut's one-second decoder preroll already passes upstream's
four new MPEG-PS/TS trimmed-audio examples.

Waveforms rise from the floor with one upper hot-level band. Peak caches,
zoom/trim sampling and gain-only layout updates are unchanged. Text casing is
applied at the shared preview/export boundary without rewriting the source or
caption transcript. Strike and case participate in caption-group/batch edits,
copy/paste, presets and the serialized title cache key. Old projects default to
Original/no strike; unknown case values fall back to Original. The existing
vertical text and underline implementation is preserved.

Regression coverage lives in `concat-media` audio tests, `concat-project` model
and clipboard tests, `concat-text` rendering tests and `concat-host` title tests.
UI formatting, strict Clippy and all 25 locale inventories are checked separately.
Hardware-specific and live-provider tests retain their explicit prerequisites.

## Earlier adaptations

| Area | Concat commits | Result |
| --- | --- | --- |
| Reader pool | `e8c546ac`, `2b6eb230` | Separate rolling readers for two cuts from one file, without opening extra readers for neighbouring prefetch requests. |
| Transition stacking | `76988c49` | A chain of transitions stays beneath logos and tracks above it, regardless of clip list order. |
| Export frame grid | `fe2be741` | Export and audio mix end on the timeline's quantised frame grid. |
| Frame plan borrowing | `167a7331` | Avoid cloning complete plans when treatments do not need them. |
| Camera/container seeking | `d02b7565`, `39ebb75e` | Recover from overshooting MPEG-TS seeks and account for coarse container timestamp precision. |
| Non-square pixels | `b8337561` | Display HDV/AVCHD/DV at their aspect-correct dimensions before rotation. |
| Variable frame rate | `1dac19b7`, `bac31a3f` | Hold the preceding picture across pauses and index moving frames by capture cadence. |
| Damaged packets | `7c113230` | Tolerate a bounded sequence of corrupt packets; still fail on a broken stream. |
| Common media origin | `93c18b72` | Preview, transcription and export preserve offsets between picture and sound. |
| Import artwork | `7b2d30ea`, `ccc6131c`, `7dc91a92` | Prioritise posters, then waveforms and strips; use suitable keyframes with an exact fallback. |
| Captions | `f1a5f4d3`, `2922614c` | Fix consecutive Whisper timestamps and tighten caption spans to detected speech using the bundled Silero VAD model. |
| Library default | `6466b9f7` | Open on All while retaining Clovercut's favourites, most-used and recent modes. |
| Search focus | `0a45c5a5` (functional part) | Enter still submits online searches, then releases focus; Escape releases focus. |
| Filter preview | `599157e7` | Label the audition in the monitor and allow dismissal by its close button, Escape or selecting a clip. |
| Text effects | `0acb767b` | Add the generic effects chain beside Clovercut's caption, style and animation tabs. |
| Encoder stalls | `0c86f62b` | A hardware encoder stall restarts the render once in software, resets progress/ETA and reports the new stage. |

Other selectively adapted changes include unnamed WAV channel layouts
(`e353fe57`), updater TLS roots/MCP annotations/VAD cache publication (`41ca63a4`),
feature-gated Chatterbox configuration (`e854f821`), font-picker Escape handling
(`3e651f4f`) and the visual-layer copying correction from `2606aff1`.
Cross-project paste adapts `1d04d147` through Clovercut's own typed-track and
project-owned resource implementation. The SoundLayer/audio-bus schema is adapted in the current global audio follow-up above.
Android renderer changes remain excluded.

Detailed investigation logs and superseded audit reports remain available in
Git history; they are not current setup or release instructions.
