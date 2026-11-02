# Changelog

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
