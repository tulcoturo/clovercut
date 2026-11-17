# Changelog

## [0.2.7] - 2026-11-17

Modern CapCut Desktop draft decryption and import, atomic multi-clip audio detach and reattach, curated typography preset templates, export selected time range selection, and visual/typography bug fixes.

### Added
- **Modern CapCut Desktop Draft Decryption & Import:** Direct decryption and import of modern CapCut Desktop projects (`draft_content.json`). Supports AES-256-GCM 12-split carrier decryption, folder and direct file selection, timeline tracks, media clips, text styles, and supported effects without requiring external conversion tools.
- **Atomic Multi-clip Detach & Reattach Audio:** Batch detach audio from multiple selected video clips into synchronized sound clips on dedicated audio tracks in a single atomic transaction with unified undo/redo. Reattach audio restores audio tracks back to their parent video clips cleanly.
- **Curated Title Preset Library:** Bundled 8 production title styles into the preset library: Cinema, Classic, Chapter, Minimalist Editorial, Quote, Breaking News, Minimal Lower Third, and Neon Glow.
- **Export Selected Time Range:** New range selection in the export dialog allowing exports of either the entire timeline or the selected clips / In-Out range, with dynamic real-time recalculation of duration and estimated output file size. Full localization support across all 25 locales.
- **3-Point Editing and Slip Editing:** Added monitor Insert and Overwrite 3-point editing commands, as well as Slip Clip editing with detached audio sync awareness.

### Fixed
- **Pure White Color Range (#61):** Ensured full color range (0-255) when rasterizing painted shapes and title overlays on the timeline, preventing clipped highlights and crushed white levels.
- **Captions Editor Text Padding and Word Wrapping (#63):** Fixed text truncation and clipping at horizontal boundaries within the subtitle/captions editor with proper word-wrapping and boundary padding.

See [complete release notes](docs/releases/0.2.7.md).

## 0.2.6

Titles paint in background workers; colour emoji are cached and layered colour fonts retain their gradients.
The Animate inspector provides fourteen entrance/exit motions and nine loops for video, pictures, titles and captions.
Shapes have editable colours, gradients, outlines, corners, dimensions, polygon/star settings, line heads, trim and shadows.
Full-screen preview opens from the monitor button or F11 and closes with Esc.
Clips support multiple masks with independent keyframes. Shake, Beat Shake, Impact and Prism Shake join the effects catalogue.
Voice isolation separates speech from noise or singing from accompaniment. Advanced Chroma Key detects green/blue screens automatically.
See [release notes](docs/releases/0.2.6.md).

## 0.2.4

Person-aware neon/electric effects, tracked sunglasses and dog accessories, animated
reference previews and bundled fallback cards. Media selection, long-text editing,
export progress, preview cadence metrics and desktop interaction fixes; task-scoped
wakefulness on Windows/macOS and compatible look label updates.
See [release notes](docs/releases/0.2.4.md).

## 0.2.3

Nine creative effects and animated effect/filter thumbnails; preview recovery,
autosave/dropdown stability, timeline scroll fixes and captions CPU initialization.
Workspace persistence, playhead colours, missing-media Undo, language presets and
macOS window safe start. See [release notes](docs/releases/0.2.3.md).

All notable changes to **Clovercut** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.2.2] - 2026-11-02

### Fixed
- Preserve running exports when reopening the panel; reject obsolete callbacks and cancel only the panel's own job.
- Restore search/sorting for small project libraries and add sorting in narrow desktop windows.

### Changed
- Remember the project view, use compact grid/list icons, improve spacing and keyboard/accessibility behavior.
- Reduce owned-audio comparison overhead with bounded 64 KiB buffers.
- Document macOS Gatekeeper first launch and report signing/notarization status explicitly.

See [complete release notes](docs/releases/0.2.2.md). Earlier 0.1.1–0.2.1 notes remain in `docs/releases/`.

---

## [0.1.0] - 2026-10-22

### Added
- **Android Deployment:** Added official `aarch64-linux-android` package target and APK packaging pipeline (`Clovercut-0.1.0-android-arm64.apk`).
- **WebM Alpha Support:** Upstream WebM VP8/VP9 alpha layer video decoding natively composited on the GPU.
- **SVG Import:** Direct vector graphic import support rendered crisply into timeline stills.
- **Whisper Subtitle Studio:** Embedded Whisper speech-to-text auto-caption generator with dedicated subtitle styling inspector.
- **3D LUT Grading:** Real-time `.cube` 3D LUT look-up table support and interactive Lift/Gamma/Gain color wheels.
- **Asset Integrations:** Integrated media panels for GIPHY animated stickers, OpenGameArt sound FX, and vector graphics.
- **16:9 Showcase Assets:** High-resolution widescreen UI walkthroughs and documentation evidence (`2508 x 1416`).

### Changed
- **Visual Identity & Polish:** Completely refined typography, vertical centering, and squircle clover mark alignment across TitleBar, application header, and the Export dialog.
- **Brand Consistency:** Modernized brand assets and application icons across all dimensions (`16px` to `512px`).
- **GPU Seeking:** Optimized frame seeking routines for instant response during timeline scrubbing.
- **Timeline Diamond Keyframes:** Replaced placeholder keyframe markers with sleek diamond indicators.
- **Documentation Overhaul:** Rewrote `README.md` and documentation highlighting local-first architecture and CapCut-style workflows.

### Fixed
- **Export Header Alignment:** Fixed icon and title vertical alignment and scaling in the Export configuration modal.
- **TitleBar Typography Metrics:** Corrected baseline and vertical offsets ensuring the clover logo and title text are perfectly centered.
- **VLC MP4 Seek Tolerance:** Fixed seeking and playback glitches on MP4 files with non-standard keyframe index structures.
- **Multi-Monitor Windows Fix:** Resolved display identification bugs on multi-monitor setups under Windows.
- **macOS Cmd+Q Handling:** Ensured immediate, safe project persistence when quitting via keyboard shortcuts.
- **Repo Hygiene:** Cleaned up ~5.6MB of obsolete Concat marks, temporary audit files, and internal agent handoff notes.

---

## [0.0.2] - 2026-10-21

### Added
- Windows x64 installer and portable ZIP distribution packages.
- Visual speed curves with interactive control points, smooth/linear interpolation, and presets.
- Pitch-preserving continuous audio speed ramp processor.

### Fixed
- MP3 audio continuity during speed ramps and transitions.
- Linux modifier key reconciliation preventing stuck multi-selection states.
- Direct3D shader compilation on Windows using FXC by default.

---

## [0.0.1] - 2026-10-19

### Added
- Initial standalone Clovercut distribution fork based on Concat.
- Independent branding, identity, and application IDs.
- Linux x86_64 and aarch64 AppImage release packaging.
