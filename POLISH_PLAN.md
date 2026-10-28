# Clovercut quality and polish plan

Updated 28 October 2026. Baseline: release 0.1.9 (`fb8b7f90`).
Detailed findings, test dispositions and dated execution records live in the
[quality audit](docs/audits/2026-10-28.md); commands and qualification requirements
live in [TESTING.md](docs/TESTING.md).

The objective is predictable editing, trustworthy validation and fast feedback.
There is no test-count reduction target. Preserve data, ownership, cancellation,
locks, undo and preview/export regressions. No new release is implied by this plan.

## Current status

| Area | Result | Remaining qualification |
|---|---|---|
| Test correctness | Seek timeouts cannot count as success; export matrix probes encoded output and final content; misleading parity name corrected | Ignored graphical/audio transport and parity suites need a dedicated session |
| CI and distribution | Connector, exact-source packaging gates, retained reports and native Linux lifecycle checks installed | Prior-version native upgrade needs a second native release; reinstall is already qualified |
| Development cost | Headless core/project/storage feedback; CI timing report; three controlled headless cold/warm pairs | Full desktop cold/warm comparison and any UI extraction prototype remain unqualified |
| Desktop feedback | Shared icon accessibility, translated recovery/cache feedback, documented cut policy | Keyboard, screen reader, scaling, RTL and complete mouse/keyboard journey |
| Responsiveness | Shared Enhance estimate policy; cancellable background relink; structured runtime probes | Runtime baselines, repeated lifecycle/memory plateau and large-file audio-copy latency |
| Data and services | Recents/caches/relink fixes; project/timeline/request guards; audio content identity; deterministic service fixtures | Real graphical recovery, provider credentials and interactive playback |
| Tooling and docs | Walkthrough opt-in; release/site/install metadata aligned; concise plan with linked evidence | State-driven screenshot driver and Windows/macOS interactive checklist execution |

Implemented behaviour is distinguished from interactive qualification below.
The newest desktop changes are undergoing full CI and Linux/Windows candidate
builds; the fast headless checks have passed. Release 0.1.9 binaries do not include
these later fixes. In particular, avoid **Clear project cache** in 0.1.9: its broad
cleanup can remove generated source media. The corrected development version
preserves that media.

## 1. Trustworthy tests — P0

- **Verified:** strengthen the existing seek assertion and five host export
  reliability cases rather than delete integration coverage. Probe codec, depth,
  range, exact rate and last-frame content.
- **Verified:** classify all 28 baseline ignored attributes by prerequisites.
  Runtime counts differ with platform/features; ordinary green CI does not imply
  execution of ignored tests.
- **Decision:** keep shader, output-preservation, ownership, cancellation and
  undo coverage. Keep 4K fixtures where resolution is part of the contract.
- **Pending environment:** execute the applicable ignored desktop transport,
  preview/export, clipboard and shutdown suites in an isolated graphical/audio
  session. Preserve evidence by source, platform and adapter.

## 2. CI and releases — P0

- **Verified:** Node 22 job executes all 20 bundled connector cases; exact-source
  x86_64/ARM64 installer/package tests gate publication; compact reports survive
  successful runs.
- **Verified:** `.deb`/`.rpm` install, dependencies, graphical startup, reinstall,
  removal and user-data preservation in isolated Debian 12/Fedora 43 on both
  architectures. AppImage checks remain mandatory. Never install QA packages on
  the developer's host.
- **Implemented:** native installer identifies a launcher earlier in `PATH`
  without deleting or replacing it; a regression reproduced the silent case.
- **Deferred with prerequisite:** an upgrade between different native releases
  cannot yet be qualified: 0.1.9 is the first native release. Same-version
  reinstall does not establish schema or prior-version upgrade compatibility.

## 3. Development cost — P1

- **Verified:** headless Clippy/tests for actual core/project/storage source,
  pure Enhance/search/audio/report helpers, without generated UI or media libs.
- **Measured:** CI report separates lint, test compilation, execution and
  feature-enabled work. Controlled local headless artifact-cold builds took
  17.337–17.443 s across three repetitions; warm commands are documented with
  their observation overhead. OS disk caches were uncontrolled.
- **Decision:** retain the gRPC feature-enabled second execution. Its observed
  runtime is seconds, while compilation dominates. Filtering shared cases gives
  little benefit and can lose feature-interaction coverage.
- **Deferred pending measurement:** desktop cold/warm comparison, pure/UI crate
  extraction and profile changes. The generated UI is about 94 MB of Rust;
  do not repeat resource-heavy local builds on the 16 GiB host or infer a speedup
  from headless timings. Use isolated CI and the documented resource limits.
- **Decision:** consolidate fixtures only when there is real duplication and a
  named replacement assertion. No broad rewrite or deletion for its own sake.

## 4. Desktop workflow — P1

- **Implemented:** shared `IconButton` exposes role, label, enabled state and
  action; recovery loading/cancel/result and cache feedback translated across
  all 25 locales. Slint generation and strict locale inventory passed.
- **Documented:** toolbar/menu/shortcut/quick split preserve continuous playhead
  time; razor and frame stepping use the rational frame grid. Keep intentional
  subframe audio/move/trim semantics and existing characterisation coverage.
- **Pending environment:** fresh-profile import/edit/captions/library/save/reopen/
  recovery/export journey; Tab/Shift+Tab, Enter/Space, Escape and typing isolation;
  light/dark, 100–200% scale, long translations/RTL and screen reader. Record
  actual screenshots and failures; no speculative layout changes without review.

## 5. Responsiveness and resources — P1

- **Verified:** Enhance estimate/request share source-span/margin planning; four
  cases cover trimmed, retimed, still and previously enhanced inputs.
- **Implemented:** relink searches run off the UI thread, cancel when dismissed
  or superseded and reject stale results before an undoable batch is applied.
  Eight pure search/ownership/eligibility cases passed.
- **Verified reporting:** runtime probe preserves raw samples, p95/max and
  provenance; unavailable counters stay null. Host rendering-call timing is not
  device execution or presentation; DRM counters are device-wide.
- **Pending environment:** deterministic graphical lifecycle split, repeated
  seek/play/switch/undo resource cycles, startup/first preview/seek/playback and
  inspector timings on representative projects. Do not claim a memory plateau
  from one close or change cache/thread budgets without repeated baselines.
- **Open measurement:** audio adoption still copies/probes synchronously. Content
  comparison uses bounded memory, but large-file UI latency needs measurement
  before a concurrency/ownership refactor.

## 6. Projects, media and services — P1

- **Verified:** Unix recent paths preserve case and literal backslashes; 19
  storage cases protect legacy manifests, ordered/concurrent saves, failure
  recovery and cache boundaries.
- **Verified:** cleanup preserves generated project media and unknown cache
  entries outside reserved preview storage; refuses symlinked cache roots and
  non-project folders. Reproduced data-loss regressions now pass.
- **Verified:** relink chooses nearest exact basenames deterministically, avoids
  link cycles/broken targets, and treats directories as missing media. A match
  by name does not establish recording identity.
- **Implemented:** project-scoped picker results and project/timeline-owned
  sound/sticker actions reject obsolete replies. Preview request revisions
  preserve newer choices. Owned audio reuse compares content, preserving distinct
  equal-sized recordings and symbolic-link targets.
- **Verified fixtures:** 20 offline catalogue/API/session cases cover pagination,
  attribution/licences, expired URLs, coalescing, explicit login and reconnect.
- **Pending environment:** live providers, authenticated Studio/browser lifecycle,
  actual relink/undo and delayed preview/download interactions. Keep credentials
  and personal media outside source and artifacts. Existing VFR/PTS/reverse,
  caption, alpha and sound-layer coverage remains in the release gates.

## 7. Maintainability and documentation — P2

- **Verified:** release 0.1.9 has twelve desktop assets; site and native links are
  deployed; README remains project-focused. AUR metadata follows the selected
  release and stale AUR installation falls back to the verified AppImage.
- **Implemented:** cohesive pure helpers extracted while fixing their callers;
  no large `studio.rs` rewrite. Historical suite renames are deferred until the
  relevant behaviour changes; names alone do not justify churn.
- **Implemented, limited:** Hyprland/grim walkthrough is excluded from ordinary
  builds and fails on fixture/capture failure. Its timed steps and staged dialogs
  are not evidence of complete recovery/export. State-driven waits and real
  failure scenarios remain open; do not advertise screenshot completion as QA.

## Completion evidence

Integrate coherent changes after applicable checks pass. Record exact tested
sources and candidate artifacts separately from published releases. The remaining
interactive/hardware/live-provider work requires the environment listed above;
it is unqualified until executed, rather than implicitly green or abandoned.
