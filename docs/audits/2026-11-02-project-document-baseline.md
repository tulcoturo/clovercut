# Large-document baseline

Measured the optimized actual `concat-project` example at clean source `d31862fdb0ae23b5c1859304d3a5c222c0cfe681`, using pinned Rust 1.93.1 on the local Linux x86_64 host. Build plus first execution compiled only the lightweight project/core dependencies, not the desktop UI. Compilation is outside the samples.

Fixtures contain 100/500 synthetic video clips, one stored filter per clip and 100/500 caption cues. Every load/save and edit/undo/redo/undo sample checks semantic preservation outside the timed sections, except undo/redo success assertions within the history sequence. Fixture construction is outside the three measured scenarios. Each scenario has fifteen repeated-input samples.

| Workload | JSON bytes | Parse JSON + load median / p95 | Serialize median / p95 | Edit + undo/redo/undo median / p95 |
|---|---:|---:|---:|---:|
| 100 video clips + 100 captions | 251,176 | 4.084 / 6.020 ms | 1.292 / 1.519 ms | 0.011 / 0.012 ms |
| 500 video clips + 500 captions | 1,255,186 | 21.620 / 32.315 ms | 6.282 / 7.873 ms | 0.048 / 0.075 ms |

[Raw samples and provenance](data/2026-11-02-project-document-baseline.json) retain timings, exact source, toolchain, dirty-tree state, workload sizes and logical parallelism. Reproduce using the example command in [TESTING.md](../TESTING.md#large-document-baseline-without-the-desktop-build). Wall-clock caches and scheduler load were uncontrolled; these are observations, not thresholds or a controlled before/after comparison.

The fixture uses actual commands and document code but synthetic media metadata. No input video is decoded, filter rendered, title rasterized, project file read/written or desktop frame published/presented. These numbers do not establish app startup, autosave durability, preview responsiveness, memory plateau or physical-device performance. Generated IDs vary between independently constructed fixtures; comparisons must retain the same workload shape and settings.

No production engine optimization is justified by this single document-only observation. The next useful measurement is a native workload separating session open, UI publication, preview/decode and resource cleanup on the same source and machine, rather than inferring editor-wide speed from document timings.
