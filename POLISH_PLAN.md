# Clovercut quality and polish plan

Status: implementation in progress, 28 October 2026. Baseline: `fb8b7f90` / release 0.1.9 source.
Evidence and test dispositions: [quality audit](docs/audits/2026-10-28.md).
The previous eight-phase programme remains in Git history. This plan does not
change existing release gates or authorize deletion of protective tests.

The objective is predictable editing, trustworthy validation and shorter
development cycles. There is no target test-count reduction. Prefer fixing
existing workflows before adding features.

## Execution record — first increment

Implemented and validated:

- P0: seek stress test requires all 50 seeks to publish the target picture and
  reach the playback position before the deadline; timeouts cannot count as success.
- P0: host export test renamed to describe its actual contract; the format
  matrix now inspects encoded codec, pixel format/depth, range, exact rate and
  final picture content. All five export reliability cases passed.
- P0: reusable x86_64/ARM64 package regressions gate release publication on the
  exact source SHA; the connector has a Node 22 CI job; reports survive success.
- P0: AUR regression fixtures follow the package version instead of assuming
  0.1.8; updating package metadata no longer breaks an unrelated stale-file test.
- Installation: Arch helpers are used only when the AUR reports the selected
  release; stale/unavailable AUR metadata falls back to the verified AppImage.
  Two regressions cover the actual 0.1.9-release/0.1.8-AUR mismatch and API failure.
- P1: the existing companion API/session fixtures also exercise the bundled
  connector: 20 cases cover licence/credit normalisation, pagination, expired
  URLs, offline browsing, request coalescing, explicit sign-in and reconnect.
- Test prerequisites and execution policies for all 28 ignored baseline cases
  are documented in [test suites and qualification](docs/TESTING.md).
- P1: common icon buttons expose their role, label, enabled state and default
  action. Slint generation passed; screen-reader qualification remains pending.
- P1/P2: Enhance request and estimate share a small pure planning module, tested
  without compiling the generated UI. Four cases passed (trimmed long source,
  boundaries, retiming/fractional rate, enhanced offset/still).
- P2: screenshot walkthrough is opt-in development tooling; fixture/capture
  failures no longer silently continue. State-driven steps remain future work.
- Distribution: 0.1.9 is published with all twelve packages. Website update is
  deployed as `03e59cff`; README/install/AUR metadata are aligned with the published release.
- Native `.deb` and `.rpm` installation, dependency resolution, graphical startup,
  reinstallation, removal and user-data preservation passed on x86_64 and ARM64
  in [isolated CI](https://github.com/tulcoturo/clovercut/actions/runs/40667742945).
  The harness also runs in future Linux builds before artifact upload.

Validation: the [full Rust CI](https://github.com/tulcoturo/clovercut/actions/runs/40666635410)
passed Clippy, documentation examples, 1300 tests (27 skipped) and 30 gRPC tests.
The [Linux candidate build](https://github.com/tulcoturo/clovercut/actions/runs/40671290101)
passed compilation, AppImage checks and native package installation/startup checks.
The [latest packaging matrix](https://github.com/tulcoturo/clovercut/actions/runs/40674173356)
passed on x86_64 and ARM64. The production Rust/UI source is unchanged since the
full CI commit `c23a64da`; later changes affect connector tests, CI, packaging and docs.

Also passed: 21 package regressions, 9 locale-tool regressions, 20 connector
cases, 4 Enhance-planning cases and 5 export reliability cases passed. Strict
locale inventory, rustfmt, actionlint and Slint generation passed.

Limits: full desktop Rust checking exceeded the deliberate 6 GiB address-space
limit (rustc allocation failure/SIGABRT), so it is not recorded as passing. The corresponding
full desktop checks subsequently passed in isolated CI, as recorded above.
Local Docker access is denied and passwordless sudo is unavailable; remote native
container checks passed instead. No packages were installed on the host.
Interactive graphical/audio and screen-reader qualification are still pending.

The remaining sections are planned work, not a completed-programme claim. Keep
the existing gRPC second pass for now: its measured cost is only 5.644 seconds.

## Execution record — second increment

- Measured build cost: a versioned [CI timing report](docs/audits/data/2026-10-28-ci-timings.json)
  separates workspace lint, test compilation, execution and feature-enabled checks.
  A reusable offline summariser preserves failed work and unavailable intervals.
  Cache state is unknown; controlled cold/warm comparisons remain pending.
- Fast feedback: `headless-quality.yml` checks `concat-core` and `concat-project`
  on relevant pushes/PRs with no graphical compilation or native media libraries.
  Full desktop/release gates remain in place.
- Lifecycle contract: strengthened the existing undo-depth test to verify the
  oldest retained state and exact final state after all redos, instead of only
  counting 200 successful undo operations. No test was removed.
- Local validation: 289 headless tests plus 2 documentation examples passed with
  Rust 1.93.1; focused Clippy, formatting, 4 timing-report regressions and
  actionlint passed under a 4 GiB per-process limit and two build jobs.
  The [headless GitHub CI](https://github.com/tulcoturo/clovercut/actions/runs/40680397227)
  also passed all checks on source `8fe42d88`.
- Phase-6 review: its printed GPU composition duration measures a host API call,
  not proven device execution time; DRM VRAM is device-wide rather than per-process,
  and missing process counters currently become zero. Keep this suite opt-in and
  do not derive budgets from those values. Structured runtime sampling and the
  graphical lifecycle split remain pending.

## Execution record — third increment

- Runtime probe reporting now preserves raw samples and summaries as versioned
  JSON records, with separate fixture, initialisation, preview, audio, captions,
  history and 4K host-call measurements. No performance budget was invented.
- Unsupported/malformed process counters remain unavailable; device-wide DRM
  counters retain device identity and actual zeros. Empty/invalid timing samples
  cannot become successful zero-duration measurements.
- Cache/presentation/GPU labels now describe observed calls. Structured close
  counters and all existing lifecycle assertions remain in place.
- The collector retains partial records and requires the test command's exit
  status as well as a completion marker. Missing provenance remains explicit.
- Validation: four pure Rust measurement cases and four collector cases passed;
  direct Clippy, formatting and actionlint passed. A synthetic protocol fixture
  compiled the actual Rust reporting helpers and checked their JSON with the
  collector; it is not recorded as a graphical benchmark.
- [Full CI](https://github.com/tulcoturo/clovercut/actions/runs/40682285824)
  and [headless CI](https://github.com/tulcoturo/clovercut/actions/runs/40682285873)
  passed on `2562025e`. Subsequent changes strengthen collector exit-status checks
  and document usage; Rust integration is unchanged from that qualified source.
- The ignored graphical benchmark has **not** been executed. Controlled runtime
  baselines, the separate graphical lifecycle suite and interactive qualification
  remain pending. See [runtime report instructions](docs/TESTING.md#opt-in-runtime-performance-report).

## Execution record — fourth increment

- Fixed a confirmed recent-project bug: Unix paths differing by case or a literal
  trailing backslash were collapsed by Windows-style comparison. Remember/forget
  now follows the same platform policy as media-bin paths; Windows case/slash
  handling is preserved. Forgetting a row leaves project folders intact.
- A Linux persistence regression failed before the fix (one recent entry instead
  of two), then passed after it. The existing path-comparison test now expresses
  the platform contract instead of assuming Windows rules everywhere.
- `check_project_io.py` compiles the actual host storage source in a disposable
  crate using the checkout's dependency versions and lockfile. It does not copy
  storage logic or alter the workspace lockfile. This extends fast CI to recents,
  legacy manifests, ordered/concurrent saves and durability failures without
  native media/UI compilation.
- Local validation: all 16 storage cases, focused Clippy, formatting and workflow
  lint passed. The [expanded headless CI](https://github.com/tulcoturo/clovercut/actions/runs/40699236574)
  also passed on `ab827c3c`. This narrow check does not replace full host/release
  qualification.

## Execution record — fifth increment

- Fixed a data-preservation bug: clearing the entire project cache also deleted
  generated Enhance/Reverse media and freeze frames that clips can reference.
  Cleanup now removes only reserved artwork, the poster and hash-named waveform
  files, preserving generated media, unknown content and audio/proxy directories.
  It does not infer ownership from the last saved document, which misses unsaved
  edits and undo history.
- Cleanup requires a project manifest and refuses a symlinked cache root. Directory
  counting no longer follows links; removing the reserved artwork tree leaves
  external link targets untouched. No recursive sweep of unknown directories.
- Two regressions failed before the fix: generated media was removed and a linked
  cache root was accepted. Three cases now cover preservation, symlink boundaries
  and refusing non-project folders. All 19 storage cases, focused Clippy and
  formatting passed locally. The [headless CI](https://github.com/tulcoturo/clovercut/actions/runs/40701166898)
  also passed on `3d58d24a`. This is storage qualification, not an interactive
  graphical walkthrough.

## Execution record — sixth increment

- Extracted missing-media search from the dialog into a pure filesystem helper.
  Three regressions reproduced the previous depth-first algorithm's deeper-match
  selection, filesystem-order-dependent ties and acceptance of broken links.
- Search now uses breadth-first traversal and sorted folder entries, preserving
  the documented nearest-root preference with deterministic equal-depth ties.
  It indexes only requested names and stops when they are all found.
- Canonical directory identities prevent symlink cycles and repeated alias scans;
  working file/folder links remain usable and non-file targets are excluded.
  Exact-basename matching and the existing undoable relink batch are preserved.
- Five focused cases, direct Clippy, formatting and workflow lint passed locally.
  The [expanded headless CI](https://github.com/tulcoturo/clovercut/actions/runs/40702927565)
  also passed on `c657bfbc`. Symlink cases are Unix-specific.
  The search remains synchronous; this increment
  does not qualify large-tree UI responsiveness or candidate recording identity.
  Graphical relink/undo qualification remains pending.

## Priorities

- **P0:** validation that can incorrectly report success, or omit required release checks.
- **P1:** editing consistency, accessibility, useful feedback and measured performance.
- **P2:** maintainability, documentation and broader platform qualification.

## 1. Make the test results trustworthy — P0

1. Separate completed seeks from timed-out seeks in the rapid-transport suite.
2. Rename the host export-shape test currently called GUI/API parity, or implement
   independent entry-point comparison. Keep title/transition export coverage.
3. Probe actual encoded codec, bit depth, colour tags and rate in the format
   matrix; verify final-frame content against the source fixture.
4. Classify ignored cases as hardware, graphical, live-provider or benchmark,
   with prerequisites and a last-qualified source/platform record.

Acceptance: a never-landing seek fails; changed output format is detected; test
names match their assertions; ordinary green CI does not claim graphical/live
coverage it did not execute. Preserve atomic-save/export, cancellation, locks,
clipboard and resource-ownership regressions.

## 2. Close CI and release gaps — P0

1. Run connector tests in a fast job using the supported Node runtime. Confirm
   all six language cases execute; avoid a file-only false-success report.
2. Require Linux packaging/installer regressions on the exact release SHA, in
   x86_64 and ARM64 jobs, before publishing. Reuse the existing suite.
3. Retain compact successful JUnit reports/timing summaries, distinguishing
   executed, ignored and environment-unavailable checks.
4. Add native-package install qualification in disposable Debian/Ubuntu and
   RPM-family environments, including launch, upgrade and removal. Keep the
   existing AppImage and architecture/startup checks.

Acceptance: broken connector fixtures/package regressions block publication;
the release identifies the tested source; packages launch without build-machine
paths. Use isolated environments; never install test packages onto the user's desktop.

## 3. Reduce development cost using measurements — P1

1. Record cold and warm timings separately for dependency setup, Slint generation,
   Rust checking/Clippy, test compilation, linking and execution.
2. Investigate the desktop crate's checking/compilation cost. Prototype separating
   pure editing/layout logic from generated UI only if measurements show a
   benefit. Do not change profiles speculatively.
3. Consolidate fixture builders where this removes real duplication. Use small
   media for functional cases; retain 4K where resolution affects behaviour.
4. Evaluate filtering the second server execution to gRPC and feature-sensitive
   cases. Preserve feature-enabled compilation/lints; expect seconds, not minutes,
   from this small consolidation.
5. Preserve process isolation for global settings and GPU/audio state. Do not
   increase heavy-build concurrency on the 16 GiB development machine.

Acceptance: record comparable before/after cold and warm timings, with the same
machine/toolchain/settings and no loss of failure detection. Follow the
[documented resource limits](docs/RELEASING.md#local-development), including memory
guard and swap requirements for a full graphical build.

## 4. Review the desktop workflow interactively — P1

1. Walk through fresh launch, import, edit, captions, libraries, save/reopen,
   missing-media recovery and export. Record actual failures and screenshots.
2. Review workspace presets at desktop sizes and 100–200% scaling, both themes,
   longer translations and RTL text. Check clipping, density, selection visibility,
   drag/drop feedback and tooltip placement.
3. Audit Tab/Shift+Tab, Enter/Space, Escape, shortcut isolation while typing,
   accessible labels/actions and non-colour status cues. Start with the shared
   `IconButton` and playhead cut button.
4. Decide/document the cut-grid policy across shortcut, menu, quick split and
   razor. Check rational frame rates and preserve intentional continuous audio
   operations before changing characterisation tests.
5. Make loading, disabled, empty and error states actionable and translated.
   Preserve selection and timeline position when background work finishes.

Acceptance: an isolated profile can complete the edit/export journey with mouse
and keyboard; evidence identifies source, scale and platform. Scale validation to
risk; do not create tests for every reversible visual adjustment.

## 5. Improve responsiveness and resource feedback — P1

1. Make Enhance estimates use the same source span/margins as the job. Cover
   trimmed, still, retimed and already-enhanced clips.
2. Split phase-6 into deterministic lifecycle regressions and an opt-in benchmark
   producing structured samples with OS/GPU/driver and project metadata.
3. Measure startup, first useful preview, seek latency, playback, thumbnails,
   waveforms and inspector publication on small, caption-heavy, long-audio and
   layered-4K projects.
4. Measure RSS/cache growth through repeated seek/play/project-switch/undo cycles.
   Mark unavailable platform/GPU counters as unavailable rather than zero.
5. Set budgets after repeated stable baseline runs. Investigate p95 spikes and
   growth trends before changing cache sizes or threading.

Acceptance: ETA corresponds to requested work; functional cleanup checks are
deterministic; benchmark reports separate decode, composition, UI publication and
fixture-generation time. Avoid unsupported cross-machine timing promises.

## 6. Validate projects, media and online services — P1

1. Recheck crash recovery, missing/replaced media, failed saves, full disks,
   destination preservation, clipboard-owned files and undo across linked edits.
2. Qualify trim/speed/reverse/VFR/nonzero-PTS playback and export, caption styling,
   transparent transitions and global sound effects against existing coverage.
3. Exercise provider-disabled/offline/auth-expired/rate-limited states, stale
   replies, cancelled downloads, cache reuse and licence/credit display.
4. Review Studio session lifecycle, missing browser/runtime, port conflicts,
   startup timeouts, reconnect and log retention. Keep credentials/browser profiles
   outside source and test artifacts.
5. Review cache size visibility and safe cleanup of disposable downloads without
   removing project-owned media, installed transitions or needed AI models.

Acceptance: failed/obsolete work cannot overwrite the active project, output or
selection; recovery gives a next action; offline reuse and attribution remain
correct. Live-service checks stay separate from deterministic ordinary CI.

## 7. Maintain test tooling and release documentation — P2

1. Move the Hyprland/grim walkthrough behind explicit development tooling; wait
   for observable states and fail on missing fixtures/screenshots. Maintain a
   portable Windows/macOS checklist.
2. Rename historical phase suites by behaviour and share small isolated fixtures.
   Remove a case only after identifying its replacement contract/negative case.
3. Extract cohesive modules from `studio.rs` incrementally while fixing those
   areas. Avoid a large rewrite alongside behavioural changes.
4. Align published links, native-package availability, AUR status, supported
   platforms and requirements with verified artifacts. Replace obsolete compile-time
   claims in comments and keep the README concise.

Acceptance: maintained checks have a purpose and runnable command; production
builds do not carry an unnecessary walkthrough driver; documentation reflects
published capabilities.

## Order and completion evidence

Start with 1–2, then establish measurements in 3 before performance changes. Run
4–6 as focused reviews, fixing confirmed issues in small changes; finish tooling
and documentation in 7. Track each item as proposed, in progress, verified or
deferred, with a reason for deferral.

Each change records the problem, affected behaviour, validation and material
limitations. Evidence scales with risk: functional regressions for timing/data
fixes, actual installs for package changes, visual review for layout changes.
Completion depends on observed results, not tests added or removed.
