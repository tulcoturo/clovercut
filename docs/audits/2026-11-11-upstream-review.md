# Upstream Review: Concat 9fcd218b..4e798b30 (v0.2.10+)

**Date:** 2026-11-11  
**Audited range:** `9fcd218b6fc8e658e370a2b8538c35bb56f6424e` .. `4e798b30a13efd85c4bf344c86aa2da3ff2f9a74`  
**Total commits:** 47  
**Upstream baseline:** Concat tag `v0.2.10` and following maintenance commits.  
**Clovercut baseline:** Clovercut 0.2.4 (`a1ebe92e`) on `main`.

---

## Executive Summary

Between commit `9fcd218b` (audited in Clovercut 0.2.4) and `4e798b30` (`upstream/main`), upstream Concat introduced substantial performance improvements, graphics overhauls, a motion animation system, mask stacks, and crash-resilient structured logging.

Key functional highlights:
1. **Title Rendering & Preview Performance:** Shadow blur restricted to bounding box (`blur_within`), multi-core title painting workers, background shader pipeline warm-up, and glance-decoding for filmstrips.
2. **WGSL Transitions Overhaul:** 17 transitions re-implemented in clean-room WGSL with motion blur, depth, easing, and soft edge SDFs (`c5fba06a`).
3. **Shake & Motion Effects:** 4 new GPU effects (`concat.shake`, `concat.beat-shake`, `concat.impact`, `concat.prism-shake`) with bundled preview cards and a dedicated `"Shake & Motion"` shelf.
4. **Motion Animations (In / Out / Loop):** Motion presets for video, images, titles, and shapes (`concat-core/src/motion.rs`), with an interactive card picker panel.
5. **Mask Stack:** Support for up to 8 combined masks per clip with boolean modes (`Add`, `Subtract`, `Intersect`, `None`), feather, expansion, and individual keyframing.
6. **Chroma Key GPU Driver Stability:** Complete elimination of early-return branches and dynamic cell pitch calculations in WGSL, resolving WARP and Intel/Mesa GPU device lost crashes.
7. **`concat-log` Crate:** Rewritten structured logging system with synchronous line flushing, deduplication of repeated lines, and crash detection from preceding runs.

---

## Complete Upstream Commit Inventory & Decisions

| Commit | Category | Upstream Summary | Clovercut Decision & Adaptation Strategy |
| :--- | :--- | :--- | :--- |
| `4e798b30` | CI / Windows | Windows builds window tests sequentially to avoid parallel UI compiler locks | **Adopt**: Improves CI stability under GitHub Actions runners. |
| `a42e6963` | Logging | Rewritten structured logging into dedicated `concat-log` crate (#594, #330) | **Adapt**: Valuable for crash diagnostics. Retain Clovercut naming, sanitize user paths (`~`), ensure no UI lock on flush. |
| `6df09f4e` | Docs / Policy | Code signing documentation clarification | **Not Applicable**: Clovercut uses its own distribution & signing workflow. |
| `d3a1b876` | Settings UI | Phone settings tab row hides desktop-only tabs without gaps (#718) | **Adopt**: Harmless responsive UI fix for mobile/narrow views. |
| `9ce3c25b` | CI / Release | Extend macOS release runner timeout to 100 minutes | **Adopt**: Useful for preventing Apple Silicon build timeouts on GitHub Actions. |
| `71efe1f4` | CI / Memory | Slint UI read once in `--profile test` mode to keep memory within runner limits | **Adopt**: Critical for CI memory management (Slint generated Rust UI exceeds 85MB). |
| `817631a2` | GPU / Effects | Chroma Key matte computed without early returns to avoid WARP device loss (#673) | **Adopt**: Critical GPU driver stability fix for Windows WARP and Linux Mesa drivers. |
| `6926a27a` | CI / Memory | Lint window crate independently with `-j 1` to stay under 16GB runner limit | **Adopt**: Necessary for CI runner reliability. |
| `0272125d` | GPU / Tests | Chroma Key bisect reads shader with LF line endings on Windows CRLF | **Adopt**: Fixes cross-platform GPU testing. |
| `6be5f256` | Start / Welcome | Welcome screen visual refresh with warm light card styling | **Adapt selectively**: Adapt layout improvements without breaking Clovercut branding and project cards. |
| `997c6ff9` | Performance | Studio never paints titles directly on UI thread; uses background worker (Refs #261) | **Adopt**: Massive UI responsiveness improvement during playback and scrubbing. |
| `f54666aa` | GPU / Tests | Chroma Key matte unit tests across available GPU adapters (#673) | **Adopt**: Ensures regression prevention on software & hardware adapters. |
| `bb0e7c44` | MCP / Tools | Expose clip mask stack and mask keyframing to MCP agent tools | **Adopt**: Keeps Clovercut MCP tools in sync with project capabilities. |
| `1f0a7494` | Performance | Filmstrip import skips deblocking and unreferenced frames | **Adopt**: Noticeably accelerates video import and thumbnail generation. |
| `56cd69a5` | Performance | Background effect pipeline compilation on window open | **Adopt**: Eliminates first-frame stutter when applying or scrubbing effects. |
| `8cacaa04` | Tests / Perf | Playback cut and import performance benchmarks | **Adopt**: Useful regression tests for CI. |
| `7acf1cfa` | Performance | Multi-core title painting using thread pool; keep titles painted on drag (Refs #261) | **Adopt**: Drops title dragging latency to near zero. |
| `31193bc0` | Tests | Mask keyframe test cleanup for clippy compliance | **Adopt**: Code hygiene. |
| `0496f612` | Performance | Title painting optimized (77ms -> 8ms); shadow blur bounded to glyph ink | **Adopt**: High-priority rendering optimization. |
| `da2fd371` | Export / Clean | Export decoder imports restricted to test scope | **Adopt**: Cleaner build dependency graph. |
| `565ea9a2` | Export / Perf | Ahead queue: clips open separate decoder threads 0.5s prior to cut | **Adapt**: Verify integration with Clovercut's atomic export and cancellation tokens. |
| `ab16bf42` | Style | Format mask stack source files | **Adopt**: Formatting. |
| `aef7ef66` | Features / Core | Multi-mask stack per clip with blend modes and independent keyframes (#489) | **Adapt**: Integrates into `concat-core`, `concat-project`, and inspector UI. |
| `96c079d2` | Chore | Mark `src/patches` as vendored | **Adopt**: Repository hygiene. |
| `d22699f2` | Motion / Fix | Title, caption, and shape support for In/Out/Loop motion (#369) | **Adopt**: Extends motion system across all timeline item types. |
| `b487dfe1` | Docs | README copy adjustments | **Not Applicable**: Clovercut maintains its own branding and README. |
| `0a3ff41d` | Library UI | Media pane sidebar scrolling so Audio Effects is visible under Video Effects | **Adopt**: Fixes UI reachability on smaller displays. |
| `085bcca6` | Schema / API | Restore ClipMask doc comment; clarify MotionMove in schema | **Adopt**: API consistency. |
| `6fea490d` | i18n | Remove unused `motion.move` translation key | **Adopt**: Keeps localization clean. |
| `2a2fd238` | GPU / Effects | Remove dynamic loops in Chroma Key plate and matte passes (#673) | **Adopt**: GPU shader stability on Direct3D 12 and Vulkan. |
| `39ef4c70` | Motion / UI | Fix motion selection persistence in Animate panel (#369) | **Adopt**: Ensures chosen motion doesn't reset on inspector click. |
| `8be07bd3` | Effects / Cards | Bundle thumbnail cards for Shake, Impact, Beat Shake, Prism Shake | **Adopt**: Add `.jpg` preview cards to `src/crates/concat/cards/`. |
| `ccb07c0e` | Style | Rustfmt awake.rs and GPU tests | **Adopt**: Formatting. |
| `ea03fc04` | Motion / UI | Animate panel with interactive card grid (In, Out, Loop pages) | **Adapt**: Adapt into Clovercut clip inspector while keeping Clovercut tab styling. |
| `7dbd7512` | Motion / Engine | In, Out, and Loop animation engine for clips, pictures, titles, shapes (#369) | **Adapt**: Core motion module with curve interpolation. |
| `7f32c970` | Effects | Shake, Impact, Beat Shake, Prism Shake packages & "Shake & Motion" shelf | **Adopt**: Adds 4 high-demand visual effects and category. |
| `c1aca495` | CLI | `concat-cli preview` draws effect at specific moment with `--moment` | **Adopt**: Extremely valuable for headless verification and automated tests. |
| `fa8771ff` | Render / Fix | Effect layer calculates `clip_time` relative to layer start, not timeline | **Adopt**: Corrects timing alignment for effects applied to adjustment layers. |
| `d2ff6ec4` | Clean-room | Clean-room line audit recount | **Not Applicable**: Upstream provenance accounting. |
| `c5fba06a` | Transitions | Re-implemented 17 WGSL transitions with motion blur and depth | **Adopt**: Significant visual quality upgrade across all native transitions. |
| `4a024283` | CLI | `concat-cli preview` draws transition at arbitrary cut progress (`--progress`) | **Adopt**: Essential tool for headless testing of transitions. |
| `80227d78` | Clean-room | Clean-room documentation update | **Not Applicable**: Upstream provenance accounting. |
| `af775b02` | Style | Rustfmt over awake.rs | **Adopt**: Formatting. |
| `60494749` | Start UI | New Project modal scrolling, shape wrap, and hover fix (#688, #689, #693) | **Adopt**: Improves modal behavior on small screens. |
| `ae32e16c` | GPU / Effects | Chroma Key plate cell pitch precomputed without texture queries (#673) | **Adopt**: Prevents FXC/Direct3D 12 shader compiler crashes. |
| `4ee168f8` | Looks / Fix | Renamed looks carry new IDs and names inside effect.toml | **Adopt**: Consistency in preset naming. |
| `880449ed` | Clean-room | Clean-room documentation recount | **Not Applicable**: Upstream provenance accounting. |

---

## Detailed Architectural Analysis of Major Features

### 1. Performance Optimizations (Titles, Thumbnails, Pipelines)
- **Bounded Title Shadow Blur (`0496f612`):** Previously, shadow blurs in `concat-text` processed the entire text texture area, resulting in ~77ms render time per frame. The new `blur_within` function computes the bounding box of glyph ink plus shadow offset and bounds the horizontal and vertical passes to that sub-region. Frame time dropped from 77ms to 8ms.
- **Worker-Thread Title Painting (`7acf1cfa`, `997c6ff9`):** Title rasterization is fully decoupled from the UI thread. The studio stage requests titles from a dedicated worker thread pool, rendering existing cached textures immediately and updating asynchronously when the new raster is ready.
- **Fast Glance Thumbnail Import (`1f0a7494`):** When generating timeline filmstrips, the media reader specifies `DecodeOptions::glance`, directing FFmpeg to skip in-loop deblocking filters and unreferenced B-frames. This speeds up timeline asset loading by 2.5x–4x.
- **Shader Pipeline Warm-up (`56cd69a5`):** During studio startup, `warm_shaders` walks all registered effects and transition packages in a background task, compiling their WGSL pipelines ahead of time. This prevents GPU pipeline compilation spikes during initial playback or user interaction.

### 2. Native WGSL Transitions Overhaul (`c5fba06a`)
The 17 native transitions were re-written in clean-room WGSL:
- **`Dissolve` & `Blur Dissolve`:** Linear light mixing with disk bokeh defocus that expands and swaps at cut midpoint.
- **`Push` & `Slide`:** Eased motion with dynamic motion smear along the trajectory and soft directional drop shadows.
- **`Spin` & `Swirl`:** Rotational transition with angular motion blur and mirrored edge sampling (no black border artifacts).
- **`Zoom` & `Zoom Blur`:** Logarithmic scale zooms with radial streak sampling.
- **`Whip Pan`:** Eased horizontal snap with velocity-proportional motion blur.
- **`RGB Split` & `Glitch`:** Lateral chromatic aberration and pseudo-random scanline tearing with 18 distinct bands per cut.
- **`Light Leak`:** Ray-marched warm additive exposure flare with amber edges.
- **`Cube`:** Ray-traced 3D rotating box with surface shading and depth perspective.
- **`Iris`, `Linear Wipe`, `Clock Wipe`, `Shape Wipe`:** SDF-based wipes with smooth pixel-feathered borders, including heart and star signed distance functions.

### 3. Shake & Motion Effects (`7f32c970`, `8be07bd3`)
- **Packages:** Added `concat.shake`, `concat.beat-shake`, `concat.impact`, `concat.prism-shake` under `src/crates/concat-effects/packages/`.
- **UI:** Registered the `"Shake & Motion"` category shelf in `media-pane.slint`.
- **Thumbnails:** Packaged bundled preview cards (`concat.shake.jpg`, `concat.beat-shake.jpg`, etc.) in `src/crates/concat/cards/`.
- **Timing:** Fixed `fa8771ff` so adjustment effect layers evaluate clip time relative to layer entry instead of global timeline origin.

### 4. Motion Animations Engine (`7dbd7512`, `ea03fc04`, `39ef4c70`, `d22699f2`)
- **Core Module:** Defined in `src/crates/concat-core/src/motion.rs`. Supports `Move` (In/Out animations) and `Cycle` (Loop animations).
- **Supported Types:** Video clips, image stills, text titles, captions, and solid/shape overlays.
- **Motion Catalog:**
  - *In / Out:* `Fade`, `SlideLeft`, `SlideRight`, `SlideUp`, `SlideDown`, `ZoomIn`, `ZoomOut`, `Pop`, `Drop`, `Rise`, `Spin`, `Flip`, `Swing`, `Squash`.
  - *Loop:* `Pulse`, `Heartbeat`, `Float`, `Breathe`, `Jitter`, `Wobble`, `Pendulum`, `SpinLoop`.
- **Inspector UI:** Added `motion-picker.slint` integrated into `clip-inspector.slint` with three tabs: **In**, **Out**, and **Loop**, with a **None** reset card.

### 5. Multi-Mask Stack Architecture (`aef7ef66`, `bb0e7c44`)
- **Data Model:** `MaskStack` in `concat-core/src/mask.rs` supporting up to 8 combined masks (`MaskShape`: Rectangle, Circle, Linear, Polygon, Brush).
- **Boolean Blend Modes:** `Add`, `Subtract`, `Intersect`, `Invert`.
- **Parameters:** Independent feathering, expansion/contraction, opacity, and transform keyframes.
- **UI:** Dedicated masks subpanel in `clip-inspector.slint` allowing users to add, reorder, delete, and individually keyframe masks.
- **MCP Tool Integration:** Tool schemas in `concat-mcp` updated with full mask stack inspection and editing.

### 6. Chroma Key Driver Stability Fixes (`817631a2`, `2a2fd238`, `ae32e16c`)
- **Problem:** On Windows WARP (software Direct3D 12 adapter) and some Intel/Mesa Linux drivers, Chroma Key caused `DeviceLost` errors.
- **Root Cause:** Direct3D 12 compiler (FXC) crashes on WGSL early-return instructions (`return`) placed before texture sampling operations, and dynamic texture queries inside nested loops.
- **Resolution:**
  - All early returns removed; shader uses ternary selects at the pixel exit.
  - 3x3 plate and matte filter loops flattened to constant loops `0..3`.
  - Cell pitch precomputed on CPU based on texture dimensions (`ceil(size / 16.0)`) instead of query inside WGSL.

### 7. Structured Crash-Resilient Logging (`a42e6963`)
- **Crate:** `src/crates/concat-log/`.
- **Crash Recovery:** Synchronous write + flush on every message. If the previous process crashed without clean shutdown, the new run logs the crash event along with the final error recorded.
- **Deduplication:** Identical recurring log lines are written up to 10 times, followed by a repeat counter, preventing log spam from saturating disk.
- **Sanitization:** User home directory paths are replaced with `~`.
- **Environment:** Controlled via `CONCAT_LOG` (or `CLOVERCUT_LOG`) filter syntax.

---

## Phased Implementation Roadmap for Clovercut

To preserve system stability and keep user machine resources free (no heavy local compilations):

### Phase 1: High-Performance & Crash-Proof Fixes (Completed - PR #49)
- [x] Adopt bounded shadow blur in `concat-text/src/lib.rs` (`0496f612`).
- [x] Adopt glance decoding in `concat-media/src/decode.rs` (`1f0a7494`).
- [x] Retain Clovercut simplified display-level chroma key (immune to WARP/Mesa device loss).
- [x] Adopt shader warm-up on startup in `concat-host/src/preview.rs` (`56cd69a5`).
- [x] Adopt settings tab separation per shell for mobile/compact views (`d3a1b876`).

### Phase 2: Effects & Transitions Modernization (Completed - PR #49)
- [x] Import the 4 Shake & Motion effect packages into `src/crates/concat-effects/packages/` and bundle cards in `cards/` (`7f32c970`, `8be07bd3`, `fa8771ff`).
- [x] Add `"Shake & Motion"` shelf to `src/crates/concat/ui/workspace/media-pane.slint`.
- [x] Update the 17 WGSL transition packages with the motion blur, SDF edge, and clean-room shaders (`c5fba06a`).
- [x] Adopt transition & effect CLI testing flags (`--progress`, `--moment`) in `concat-cli` (`4a024283`, `c1aca495`).

### Phase 3: Advanced Capabilities
#### Motion System (Completed - PR #50)
- [x] Import motion core models and presets (`concat-core/src/motion.rs`).
- [x] Integrate `Clip::motion`, `Clip::pose_at`, `transform_at` and `opacity_at` compositing in `concat-core/src/timeline.rs`.
- [x] Update project model (`ClipMotion`, `ClipMove`, `ClipLoop`) and command dispatch (`ClipPatch::motion`).
- [x] Support motion in export flattening (`concat-export`) and title rendering (`concat-host/src/titles.rs`).
- [x] Integrate interactive motion picker with animated cards (`motion_tiles.rs`, `motion-picker.slint`, `AnimatePanel`).
- [x] Add phone UI support and complete 25-locale translations.

#### Title Painting & Rendering Performance (Completed - PR #51)
- [x] Fast PNG deflate compression using `png::Compression::Fast` in `concat-text` (`0496f612`).
- [x] Multi-threaded title painting on every core via `std::thread::available_parallelism()` and `std::thread::scope` in `paint_ahead` (`7acf1cfa`).
- [x] Dynamic RAM-based cache ceiling allocation via `live_share` based on physical system memory (`7acf1cfa`).
- [x] Scalability benchmark suite `edit_scale.rs`.

#### Export Multi-Threaded Read-Ahead (Completed - PR #52)
- [x] Decoupled clip decoding onto dedicated background threads with bounded queues (`concat-export/src/ahead.rs` - `565ea9a2`).
- [x] `Feeds::look_ahead` opens decoder 0.5s prior to cut on desktop to eliminate cut seek stalls (`565ea9a2`).
- [x] Android `read_lead` set to 0 to preserve MediaCodec hardware codec seats.
- [x] Clean test-scoped `Decoder` and `FrameSource` imports (`da2fd371`).
- [x] Resilient IPv4 and DNS fallback configuration in GitHub Actions CI workflows.

#### Multi-Mask Stack Architecture (Completed - PR #53)
- [x] Integrate mask stack multi-layer architecture and keyframing into `concat-core`, `concat-project`, and UI (`aef7ef66`, `ab16bf42`, `31193bc0`, `bb0e7c44`):
  - `concat-core`: `mask.rs` implements `MaskStack`, `MaskMode` (Add, Subtract, Intersect, None), `ShapeMask` with `expand` and `opacity`; `animate.rs` and `timeline.rs` keyframe each mask individually.
  - `concat-render`: WGSL mask shader `MASK_STACK_WGSL` compiles uniform buffers supporting up to 8 combined masks; reference CPU evaluation and GPU integration tests.
  - `concat-project`: `model.rs` represents `Clip.masks: Vec<Mask>` with `MaskSource`, `MaskMode`, individual parameter tracks `key_runs`, `Command::SetMaskKey`, `ClearMaskKey`, `ClearMaskKeys`.
  - `concat-export` & `concat-host`: Pipeline resolves and renders mask stacks per clip across export and preview pipelines.
  - `concat-mcp`: Updated `clip_masks` and `mask_key_set` tools with matching JSON schemas.
  - `concat` UI (Slint & Rust): Added `studio/masks.rs`, updated `masking.rs` (`MaskAt`), inspector UI with mask list reordering, mode pickers, expand, opacity, and full keyframe cluster support. All 26 locales updated.
  - CI verification: 100% green across visual effect, GPU render, and headless quality checks. Merged into `main`.

#### Creative CapCut-Style Face Distortion & Full-Body Effects (Completed - PR #54 & Batch 2)
- [x] **Big Head (`concat.big-head`)**:
  - Exaggerated head/face enlargement with spherical radial bulge centered on detected facial landmarks (`needs_face: true`).
  - Configurable `amount`, `radius`, and edge `feather` with zero artifacts on the torso or background.
- [x] **Alien Face (`concat.face-alien`)**:
  - Exaggerated manga/alien eyes with localized bilateral eye bulge and a tapered slim jaw/chin pinch.
  - Configurable `amount`, `eye_size`, and `chin_pinch`.
- [x] **Cyber Aura (`concat.subject-cyber-aura`)**:
  - Radiant animated cyber aura and directional ray bursts hugging the full-body silhouette (`needs_subject: true`).
  - Multi-harmonic energy expansion waves with two-tone cyberpunk gradient lighting.
- [x] **Clone Trail (`concat.subject-clone-trail`)**:
  - Chromatic RGB ghost clones oscillating behind the moving subject, CapCut viral phantom dance style.
  - Configurable `amount`, `spread`, `speed`, and `glow`.
- [x] **Subject Disintegration (`concat.subject-disintegration`)**:
  - Dramatic particle dissolution ("Thanos snap" style) with glowing incandescent ember boundaries and floating sparks flying in the wind.
  - Configurable `progress`, `direction`, `spread`, and `hue`.
- [x] **Laser Eyes (`concat.face-laser-eyes`)**:
  - Blazing laser beams projected from detected eyes with starburst iris flares and customizable beam hue.
  - Configurable `amount`, `beam_length`, `flare_size`, and `hue`.
- [x] **Edge Runner (`concat.subject-edge-runner`)**:
  - High-speed electric circuit pulses racing around the person silhouette with chromatic decay trail.
  - Configurable `amount`, `width`, `speed`, and `hue`.
- [x] **Face Funhouse (`concat.face-funhouse`)**:
  - Fluid wave warp distorting facial expressions with rippling carnival mirror optics.
  - Configurable `amount`, `frequency`, `speed`, and `radius`.
- [x] Verified zero-overhead pass-through when subject or face is absent (fully compliant with probe tests and FXC/WARP drivers).

#### Crash-Resilient Structured Logging (`concat-log` - PR feat/creative-fx-and-log)
- [x] Integrated `concat-log` crate into workspace (`a42e6963`, #594, #330):
  - Synchronous unbuffered file writes guaranteeing zero lost log lines ahead of crashes or process aborts.
  - Crash detection on subsequent run launch with previous error attribution.
  - Deduplication of repeating log entries (collapsing identical messages after 10 repetitions).
  - Automatic `$HOME` sanitization (`~`) in paths for privacy.
  - Clean run shutdown hook (`concat_host::logs::end()`).
  - Timed operation tracing (`Op`) and colored console output.
  - Documentation in `docs/LOGGING.md`.
