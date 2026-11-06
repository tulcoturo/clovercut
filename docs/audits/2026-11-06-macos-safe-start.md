# macOS raster-window safe start

Adapt the remaining macOS safe-start portion of Concat `4ab430e6`, following the
already integrated shared instance-flags fix. macOS now participates in the
existing startup tripwire, persistent --software choice and Settings GPU switch.

On a macOS safe start, select_backend skips opening a shared window GPU device
and explicitly selects Skia's software renderer. The video compositor continues
to open its own Metal device: Metal has no software adapter. An explicit
WgpuCompositor::software() still remains software-only and can return None on
macOS; it is not silently converted into a hardware recovery request. Windows,
Linux and existing mobile selection policies are retained.

The macOS-specific settings hint, translated in all 25 locales, explains that
only the window moves to software and video still uses the GPU. Normal settings
retain their existing help. No deployment minimum or package format changes.
This does not address Gatekeeper signing/notarization.

Validation: strict renderer-library Clippy passed. With CONCAT_GPU=software, the
adapter-required Linux Vulkan helper opened llvmpipe and passed 11 shader probes
plus transparent/stacked composition, verifying the existing software path still
works. The 53-test real storage/preferences suite (including existing startup
recovery/preference tests) and strict isolated Clippy passed. Full Slint source
generation passed under a 4 GiB ceiling; strict inventory passed all 25 locales
and nine locale-tool tests passed.

These are Linux/software checks. Native full CI remains pending; macOS compiler,
CPU-window/Metal-preview behavior and physical Monterey hardware are unqualified.
The implementation does not guarantee recovery from a Metal compositor failure:
only the window is moved away from Metal. Keep issue-specific hardware validation
and release/package qualification separate.

## Compatibility with integrated playhead palette

Merged main `6b77c29231fd1d0e88b061530a39bb851cb45571` into the candidate.
Only the upstream documentation had a conflict; both adaptation records are
retained. Code, UI and locale changes merged automatically. The actual combined
storage/preference suite passed all 54 tests and strict isolated Clippy. Strict
locale inventory passed all 25 locales (1118 messages) and nine locale-tool tests
passed. Native CI 42283728156 still targets the original macOS candidate
`951bdee7`, so it does not qualify this combined tree.
The combined home/Theme fixture also passed strict Clippy and execution: all six
named playhead colours, custom/reset behavior in both themes, wheel scrolling,
wide/narrow home controls and keyboard/accessibility checks. This fixture does
not exercise macOS renderer selection or physical settings pointer interaction.
