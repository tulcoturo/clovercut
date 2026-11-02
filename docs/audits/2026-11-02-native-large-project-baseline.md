# Native large-project measurement follow-up

The document-only baseline is integrated through PR #15. Extend the existing opt-in native performance probe instead of creating another graphical harness or adding timing thresholds to ordinary tests.

The pure example and native probe share the actual command-based fixture builder. The pure example retains its synthetic media metadata and stored-only filter. The native probe supplies a real probed HD source and the renderable `concat.adjust` filter. Both build 100/500 video clips and matching caption cues with the same timeline spacing, but media durations and filter data differ; these are not interchangeable before/after timing inputs.

For each size, five cycles separately record session open, synchronous Slint model publication, seek/preview planning, frame-source acquisition, host texture composition calls, a validated edit/undo/redo/undo sequence and save/close. The first/middle/last video clips use different media offsets; one generated source is reused, so per-file IO scaling is outside the measurement. Fixture build/probe/save is recorded separately.

After every close the probe records process memory and owned-cache counters, requiring empty CPU/GPU/title/PCM caches. This confirms the tracked cache lifecycle only, not an RSS plateau or absence of all leaks. UI event queues are not drained and no physical frame presentation or audio-device behavior is measured. Software-adapter host-call timings are not GPU execution timings.

Local validation: strict project Clippy for all targets passed; the optimized shared pure fixture completed both workloads with semantic round-trip/history checks and 15 samples per scenario. The four report-collector regression cases, formatting and diff checks passed. Native compilation/execution and the resulting measurements remain pending the focused desktop/runtime CI run. No production editing behavior or release version changes.
