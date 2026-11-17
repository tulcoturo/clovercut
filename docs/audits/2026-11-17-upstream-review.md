# Upstream Review: Concat 0ef1e125..a7cda105 (v0.2.11+)

**Date:** 2026-11-17  
**Audited range:** `0ef1e12521f5727b9c9703c39bc0f607831fbb9c` .. `a7cda105c55cf4a5413dd747e16b0980ff2555c1`  
**Total commits:** 31  
**Upstream baseline:** Concat tag `v0.2.11` and subsequent maintenance commits up to `upstream/main`.  
**Clovercut baseline:** Clovercut (`feat/capcut-draft-import` / `main`).

---

## 1. Executive Summary

Since the last sync point (`0ef1e125`), upstream Concat tagged `v0.2.11` and added 31 commits. Because **Clovercut has diverged substantially from Concat's architecture** (dedicated subtitles track separate from text titles, local visual AI effects, native CapCut draft decoder, template slot system, three-point editing, and slip editing), the vast majority of upstream commits are either inapplicable or already superseded by Clovercut's implementations.

We identified **4 high-impact technical fixes** that directly improve Clovercut's robustness and quality:

1. **Multi-Clip Drag Precision in `f64` (`40f13a6a`):**
   - Previously, dragging contiguous multi-clip selections converted clip positions to `f32` before summing deltas. At the tail end of long timelines, floating point rounding errors caused spurious micro-overlaps (~2e-5 s > `TOUCH`), causing the engine to silently reject the drag.
   - Solution: pure `f64` arithmetic via `shifted_starts` in [`placement.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-project/src/placement.rs).
2. **FFmpeg Filter Decimal Comma Locale Guard (`577e8532`):**
   - FFmpeg parses filter arguments (e.g. `atempo=1.5`) using `strtod`, which respects `LC_NUMERIC`. On systems configured with Portuguese (pt-PT / pt-BR) or European locales where comma is the decimal separator, `atempo=1.5` failed with `"Invalid argument"`, crashing audio playback rate adjustments.
   - Solution: explicit `LC_NUMERIC` guard (`keep_c_numbers()`) in [`concat-media`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-media/src/audio.rs) before invoking FFmpeg filter graphs.
3. **Voice Activity Detector (VAD) Energy Resilience for Whisper (`ff9f7e74`):**
   - The Silero VAD speech detector on clips with background music or noise prematurely discarded audio, in some cases losing over 90% of speech to transcribe (#763).
   - Solution: energy-based validation (`Doubt`); if discarded audio power is comparable to speech, bypass VAD trimming and transcribe the full chunk.
4. **Export-Resolution Title Rasterization & Async Pipeline Prep (`8b497918`):**
   - Subtitles and titles are rasterized at the actual export output resolution (e.g. 4K UHD) rather than the preview canvas size, preventing blurry or pixelated text in final exports.

---

## 2. Commit Inventory & Triage Decisions

| Commit | Category | Upstream Summary | Clovercut Decision & Adaptation Strategy |
| :--- | :--- | :--- | :--- |
| `a7cda105` | CI / Workflows | Manual dispatch for desktop packaging without requiring release tags | **Adapt**: Allows manual desktop bundle packaging in CI via `workflow_dispatch`. |
| `9d26aac8` | CI / Smoke | Export smoke tests across Linux, Windows, and macOS | **Adopt**: Automated export validation in CI. |
| `218247fc` | CI / Android | Strip debug line tables on Android builds to conserve runner RAM | **Adopt for Android**: Prevents OOM crashes during APK builds. |
| `76585480` | CLI / Smoke | `--smoke-export` CLI flags for headless export verification | **Adopt**: Useful for headless integration test harnesses. |
| `552517cb` | Media / Android | Tune MediaCodec header parameters in first audio packet (#747, #786) | **Adopt for Android**: Fixes MP4 video render lockup on Android 15 (API 35). |
| `3a10dc47` | CI / Android | Strip symbol tables in release APK | **Adopt for Android**: Faster packaging time. |
| `6f04670d` | CI / Android | Adjust swap space for Android build runner | **Adopt for Android**: Runner stability. |
| `020d8cad` | CI / Dependencies | Bump GitHub actions versions | **Adopt**: Standard CI dependency hygiene. |
| `517ac579` | i18n | Warning strings across 25 locales | **Adopt**: Keep translation tables up to date. |
| `f22453d8` | Android / Debug | Preserve unstripped native libraries as CI artifact for backtraces | **Adopt for Android**: Crash diagnostics. |
| `e72afe4c` | Logging | Last run alert cites newest root error | **Adopt**: Clearer error reporting on startup crash recovery. |
| `8b497918` | Host / Export | Rasterize titles at export resolution; async pipeline initialization (#747) | **Adopt (HIGH IMPACT)**: Crisp titles at 4K export and non-blocking export startup. |
| `ac843caa` | Android / UI | Async export start on Android to prevent UI freeze | **Adopt for Android**: Mobile UI responsiveness. |
| `6d4b6710` | Branding | Concat cat SVG & PNG branding assets | **NOT APPLICABLE**: Clovercut has its own 4-leaf clover identity. |
| `9d77f43b` | Logging | Clarify reason for fallback to software renderer | **Adopt**: Helpful GPU diagnostics. |
| `942194dc` | Project / Core | Preserve unrecognized project fields (`extra`) on load | **Adopt**: Clovercut already features flexible `extra` metadata preservation. |
| `de9a6192` | CI / Governance | Upstream PR close/reopen bot policies | **NOT APPLICABLE**: Concat-specific repo governance. |
| `ff9f7e74` | Speech / AI | VAD resilience: prevent dropping speech comparable to background noise (#763) | **Adopt (HIGH IMPACT)**: Crucial Whisper accuracy fix on clips with music/noise. |
| `577e8532` | Media / Audio | Decimal comma locale guard for FFmpeg filters (#762) | **Adopt (CRITICAL)**: Prevents filter graph failure on European/comma locale systems. |
| `73ef32c1` | Docs / MCP | Documentation for MCP tools | **Adopt**: MCP documentation sync. |
| `25e300e0` | Crash Reporter | Fix false positive crash detector from 0.2.8 | **NOT APPLICABLE**: Concat migration artifact. |
| `e6e85769` | GitHub | Concat issue templates | **NOT APPLICABLE**: Upstream repo configuration. |
| `40f13a6a` | Project / Timeline | Pure `f64` multi-clip drag resolution | **Adopt (HIGH IMPACT)**: Eliminates silent drag collision bug on long timelines. |
| `b8d66e36` | UI / Slint | Slint text box first-character padding adjustment | **Adopt**: Slint styling tweak. |
| `6c9558aa` | UI / Monitor | Explicit missing file notice on video monitor canvas (#455) | **Adopt**: Better UX when media files are missing. |
| `30b33fc1` | UI / Notifications | Multi-line notification toast card with copy & close buttons (#455) | **Adapt Selectively**: Incorporate multi-line toast without regressing Clovercut layout. |
| `6584d349` | Release | Version 0.2.11 tag and release docs | **NOT APPLICABLE**: Clovercut has independent versioning. |
| `de4f94bf` | i18n | 5 translated strings | **Adopt**: Translation catalogs. |
| `798a57b1` | Style | rustfmt formatting pass | **Adopt**: Code hygiene. |
| `3f9798c6` | Tests / Text | Unit test for emoji grapheme cluster segmentation on Windows | **Adopt**: Unit test coverage. |
| `51699741` | Tests / Export | Numeric frame comparison in export test to tolerate encoder jitter (#756) | **Adopt**: Test harness stability. |

---

## 3. Recommended Action Plan for Clovercut

1. **Phase 1 (Engine & Timeline - `concat-project`):** [DONE]
   - Integrated `shifted_starts` and `f64` precision from `40f13a6a` in [`placement.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-project/src/placement.rs) and [`studio.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat/src/studio.rs).
2. **Phase 2 (Media & Audio - `concat-media`):** [DONE]
   - Integrated `keep_c_numbers()` from `577e8532` in [`ffi.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-media/src/ffi.rs), invoked via `ffi::begin()` in [`audio.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-media/src/audio.rs) and [`treat.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-media/src/treat.rs).
3. **Phase 3 (High-Resolution Title Rendering - `concat-host` & `concat`):** [DONE]
   - Passed output export resolution to `titles.clips()` during export in [`panes/export.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat/src/panes/export.rs) and added `request_for()` in [`export.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-host/src/export.rs).
4. **Phase 4 (VAD / Whisper Speech Resilience - `concat-speech`):** [IN PROGRESS]
   - Integrate `Doubt` energy-based VAD validation from `ff9f7e74` in [`speech_reel.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-speech/src/speech_reel.rs) and [`transcribe.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-speech/src/transcribe.rs).
