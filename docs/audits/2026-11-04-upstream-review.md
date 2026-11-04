# Upstream review — 4 November 2026

Reviewed Concat main through `8c86502d` after fetching on 4 November: 51 commits
since the previous `f2a26081` review. This is a change inventory, not a claim that
every commit represents a missing feature. Clovercut retains its desktop focus,
typed tracks, caption collections, generated media, exact export and atomic
cancellation contracts.

## Current implementation batch

- Issue #18: desktop home wheel deltas update directly instead of scheduling
  Flickable's 180 ms wheel animations. Phone input retains Flickable behavior.
  Vertical timeline scroll updates only its viewport property and saved position,
  without refreshing artwork or publishing every window model. High-frequency
  vertical scroll no longer writes an activity-log entry per event.
- `5ab7fd14`: VideoToolbox quality goes on the encoder context on Apple Silicon;
  Intel uses the quality-derived bitrate. Software encoder priority is retained.
- `359839d7`: an export lent the window's device first recovers on a sibling of
  that device, then uses software after another loss. Avoid opening a second
  GPU device while the window is using its own.
- `d64e7f25`: selectively update rustls 0.23.46, rustls-webpki 0.103.16,
  webpki roots/certificates 1.0.10, tokio 1.53.3 and pulldown-cmark 0.13.6.
  Preserve unrelated lockfile packages and dependency edges.
- Earlier `f19fda34`, partial: once preview request counts stay unchanged across
  five-second sweeps, trim CPU frame caches to a combined 96 MiB cap and shrink
  the monitor GPU pool using its existing method. Active budgets refill normally;
  in-flight frames and generated-media ownership survive eviction. Keep decoder
  thread counts and allocator policy unchanged until comparative measurements.

## Remaining earlier candidates

The scrub change `75919f5e` is implemented on the same candidate; full desktop
qualification is pending. Timeline tab scrolling is already adapted. Chinese
messages are already translated. Expanded chroma-key controls, caption-language
and VAD changes, direct audio fade handles and settings grouping remain feature
work requiring their own Clovercut adaptations. GPU-first encoder selection is
not adopted automatically: preserve reliable software encoding and fallback.

## New commit inventory

| Commit | Decision | Reason |
|---|---|---|
| `09f1a5de` | Keep project-specific | Documentation, attribution, licensing or upstream audit; preserve Clovercut facts and contributor attribution. |
| `00da96cb` | Keep project-specific | Documentation, attribution, licensing or upstream audit; preserve Clovercut facts and contributor attribution. |
| `5ab7fd14` | Adapted, CI pending | VideoToolbox quality ignored in current encoder options. |
| `fa79f78d` | Do not import as a batch | Upstream removed x264/x265 and adds replacement fallbacks; Clovercut retains its existing software encoders. |
| `6c58ab7e` | Keep project-specific | Documentation, attribution, licensing or upstream audit; preserve Clovercut facts and contributor attribution. |
| `32945bca` | Do not import as a batch | Upstream removed x264/x265 and adds replacement fallbacks; Clovercut retains its existing software encoders. |
| `3c5ffe8d` | Keep project-specific | Documentation, attribution, licensing or upstream audit; preserve Clovercut facts and contributor attribution. |
| `536a416d` | Separate audio change | Native AAC encoder selection and packet-end trimming need platform opening/fallback and sample-alignment coverage. |
| `c1b38d66` | Keep project-specific | Documentation, attribution, licensing or upstream audit; preserve Clovercut facts and contributor attribution. |
| `21af6971` | Defer platform codec replacement | Changes decoder/encoder backend; mobile portion is outside current desktop priorities. |
| `a465eb06` | Keep project-specific | Documentation, attribution, licensing or upstream audit; preserve Clovercut facts and contributor attribution. |
| `98f04de5` | Keep project-specific | Documentation, attribution, licensing or upstream audit; preserve Clovercut facts and contributor attribution. |
| `c415037f` | Defer platform codec replacement | Changes decoder/encoder backend; mobile portion is outside current desktop priorities. |
| `b8b65287` | Do not import as a batch | Upstream removed x264/x265 and adds replacement fallbacks; Clovercut retains its existing software encoders. |
| `e63756df` | Separate speech decision | Would replace existing speech engines and packaging; not a drop-in improvement. |
| `eda5e0af` | Equivalent | Formatting already matches Clovercut frame_pixels wrapper. |
| `75a04719` | Keep project-specific | Documentation, attribution, licensing or upstream audit; preserve Clovercut facts and contributor attribution. |
| `733bc94b` | Keep project-specific | Documentation, attribution, licensing or upstream audit; preserve Clovercut facts and contributor attribution. |
| `3a0318d3` | Separate speech decision | Would replace existing speech engines and packaging; not a drop-in improvement. |
| `2c3fd3e2` | Keep project-specific | Documentation, attribution, licensing or upstream audit; preserve Clovercut facts and contributor attribution. |
| `0ddc8214` | Defer schema change | Clovercut already preserves unknown fields and rejects newer versions; Concat v2 includes transition-clip semantics we have not adopted. |
| `d64e7f25` | Adapted, CI pending | Targeted dependency maintenance without replacing our lockfile. |
| `cb895fe8` | Separate speech decision | Would replace existing speech engines and packaging; not a drop-in improvement. |
| `c4854685` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `8b690cdc` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `cd702c8c` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `aba54f2b` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `4a93effc` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `87c9c056` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `6c89c24e` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `b59669df` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `205dfd38` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `b27c7d81` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `87e31832` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `adf51a1c` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `d9116df0` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `ee63b1c2` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `d3e96188` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `0c62eef6` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `ab192d41` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `dd9a9a0d` | Defer mechanical split | Sixteen-file upstream studio split would mix unrelated Clovercut behavior; extract modules when changing their responsibilities. |
| `051cc672` | Not applicable | Upstream identity, version or merge metadata does not define Clovercut behavior. |
| `8a49ad5c` | Keep project-specific | Documentation, attribution, licensing or upstream audit; preserve Clovercut facts and contributor attribution. |
| `5e47b721` | Candidate tooling | CI diagnostic annotations are useful but independent of application fixes. |
| `3116de29` | Not applicable | Upstream identity, version or merge metadata does not define Clovercut behavior. |
| `5284cbb3` | Do not import as a batch | Upstream removed x264/x265 and adds replacement fallbacks; Clovercut retains its existing software encoders. |
| `359839d7` | Adapted, CI pending | Preserve the window device during first export recovery. |
| `9ce13fff` | Separate feature | Completes the earlier transition rewrite; adapt typed tracks, persistence, trim/undo and exact rendering together. |
| `7279a2d4` | Review selectively | Our FrameSpec and redundant-cast fixes already exist; Android and tests for upstream encoder removal do not apply wholesale. |
| `4c5581cb` | Not applicable | Upstream identity, version or merge metadata does not define Clovercut behavior. |
| `8c86502d` | Review selectively | Our FrameSpec and redundant-cast fixes already exist; Android and tests for upstream encoder removal do not apply wholesale. |

## Validation and limits

The previous scrub candidate passed full workspace Clippy, ordinary tests,
doctests and gRPC checks. Its desktop suite passed 12 of 13 journeys; the clipboard
child completed all assertions but exited unsuccessfully afterward. The isolated
journey now uses the application's existing termination path after assertions and
project close, as the other native playback journeys do. Parent failures include
the child status for diagnosis. This does not waive functional assertions.

The revised batch requires full CI, all selected desktop journeys and the runtime
probe. Media-library tests and direct-wheel simulated UI checks run locally.
The scrub probe keeps the HD project open until its samples finish and requires
nonempty sources. It also records explicit idle-trim counters and duration; this
is not a timed-idleness or RSS-plateau test. Local checks passed: 156 media tests
(3 optional ignored), strict media Clippy and simulated home/wheel checks.

Physical Monterey wheel smoothness, Apple VideoToolbox output behavior and an
idle-memory plateau remain unqualified. Issue #18 must stay open until the Mac
reporter can validate a built candidate. No release or Gatekeeper fix is implied.

## Subsequent native qualification

Full CI passed on `a6a9a352ca3575cab82ada82272e5b6e6bde8f4a`: workspace
Clippy/tests, doctests, gRPC and all 14 selected desktop journeys. The runtime
probe completed with exit zero. See the [qualification record](2026-11-04-desktop-upstream-qualification.md)
for raw observations and limits; earlier pending notes above describe the
implementation-time state. Physical Monterey interaction remains unqualified.
