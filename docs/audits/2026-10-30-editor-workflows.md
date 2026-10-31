# Editor workflow and responsiveness — 30 October 2026

Candidate changes after 0.2.1. No new release is implied.

## Changes

- The home-page grid/list selector has a shared 16 logical-pixel gap below it,
  separating the controls from both thumbnails and project rows.

- The optional screenshot walkthrough waits up to 60 seconds for all three
  imported fixture paths and reports import failures. Timeline edits bind media
  by path rather than document order. Other layout captures remain timed; staged
  warning dialogs remain illustrations.
- A separate ignored editor journey uses the real event loop and asynchronous
  import, relink and export workers. It creates captions, checks split/undo/redo,
  renames and saves/reopens, moves the original source, relinks with undo/redo and
  saves/reopens again. The export pane produces a real file. The parent process
  independently checks codec, dimensions, frame rate/count, audio stream and the
  decoded last-frame colour. Asynchronous stages have individual deadlines.
- The opt-in runtime probe times actual library-audio adoption/placement and adds
  six layered-project open/seek/edit/undo/redo/close cycles. Owned resource counters
  must return to zero after every close; process RSS remains a raw observation.
  Manual CI can execute this with `runtime_probe=true`, retaining partial logs and
  structured records even on failure.
- Header File/Edit/View menus, Settings, Shortcuts and Export now expose their
  existing translated names, button roles and default accessible actions. Their
  custom controls previously only exposed pointer interactions. This does not
  establish complete Tab order or screen-reader qualification.
- Project-owned audio identity comparisons use two bounded 64 KiB buffers. Files
  still need identical bytes; equal length, links and occupied names are handled
  as before. No concurrency or ownership policy changed.

## Audio scan experiment

The [raw experiment](data/2026-10-30-audio-buffer-experiment.json) retains fifteen
samples for each candidate, including outliers. An optimised build of the actual
`owned_audio::destination` helper compared identical 345,600,078-byte stereo WAV
files on one Linux host. Three blocks interleaved the four buffer variants,
reversing their order in the middle block. OS caches were uncontrolled and files
were repeatedly read; this is not a cold-storage measurement.

| Buffer per reader | Median |
|---|---:|
| Existing 8 KiB | 81.88 ms |
| Selected 64 KiB | 46.69 ms |
| Experimental 128 KiB | 43.05 ms |
| Experimental 512 KiB | 42.96 ms |

The selected size reduced the observed median by 43% with 128 KiB total buffered
storage. Larger candidates brought little additional median benefit and retained
116.71/183.15 ms outliers. A follow-up run of the committed standalone probe at
64 KiB observed 50.04–51.70 ms across five samples, illustrating run-to-run noise.
These measurements exclude file copying, media probing, document commands and UI
event-loop/presentation delay. They do not establish total adoption latency.

## Validation

Local checks passed: formatting, standalone benchmark Clippy, both owned-audio
identity/link regressions, all 26 actual-source project/persistence/template
checks and eight structured performance/build-report regressions. Strict inventory
checks across all 25 locales and nine locale-tool tests passed. Workflow YAML
and embedded Bash syntax also passed. Complete UI generation passed from source
`b70905d1` using the cached Slint build script: 17.38 seconds and 95,007,862 bytes
of generated Rust ([record](data/2026-10-30-editor-ui-generation.json)). This checks
the Slint tree; it does not compile the generated Rust or run the interface.
The [headless CI](https://github.com/tulcoturo/clovercut/actions/runs/41100249182)
also passed for the audio-change source `9680ca5c`; the subsequent source change
adds header accessibility metadata/actions and the shared home-page spacing.

The first full candidate run (`1d67eecf`, workflow `41101539243`) failed during
Clippy: the repeated-cycle counter array mixed `usize` and `u64`. Tests and the
runtime probe did not execute. The counter is now explicitly converted to `u64`.
Full engine, virtual-desktop journey and runtime qualification remain pending
for the updated source; earlier headless/UI-generation results do not qualify them.

Additional home-page polish clears a nonempty search with Escape and returns
focus after the clear button, keeps empty/no-match feedback below the relevant
controls and hides the view selector when there are no projects. Grid/list choice
is now stored as `projectListView` in preferences, defaulting to thumbnails for
existing profiles. The selected durability case verifies both published modes
and preferences reload. Those controller assertions await the full desktop run.

## Limits

The controller journey does not inject mouse/keyboard events, exercise a screen
reader or visually compare caption rendering. Software-adapter/null-audio results
are not physical GPU/audio qualifications. Six short resource cycles do not
establish a memory plateau or long-run leak behaviour. Synchronous adoption still
needs complete-call and event-loop observations before a background refactor.
Real provider accounts, cross-version native upgrades, actual Monterey execution,
long playback drift and the full cross-platform desktop checklist remain open.

## Follow-up: project browsing and fast feedback

The home view now persists grid/list choice through preferences and republishes
it from application state, including after restart or a narrow/wide layout change.
Legacy profiles default to thumbnails. Escape clears an active search and the
clear button restores focus. Empty libraries omit the unused view selector;
no-match messages remain beneath the view controls and their 16 px gap.

UI generation passed for `e9ded934` (17.49 s, 95,043,774 generated Rust bytes).
The headless project check now includes actual preferences, directory-policy and
library-history source, with no copied implementation. All 51 cases and strict
Clippy passed locally. The existing concurrent preference-write case now also
checks the persisted project-view flag. Full graphical checks remain pending in
[the updated CI](https://github.com/tulcoturo/clovercut/actions/runs/41107753988).

## Follow-up: keyboard and accessible actions

The project-search clear control now uses the existing focusable `Pressable`
component: Tab reaches it, Enter/Space and the default accessible action clear the
query, and its translated label/tooltip exists in all 25 locales. Disabled home
cards now expose their disabled state and reject the default accessible action.
Pagination has a 12 logical-pixel inset above its controls on both home layouts.
Empty queries skip per-project case-folding before normal ordering/pagination.

For source `e3f2a670`, all 51 headless storage/preferences/history cases and strict
Clippy passed; the nine locale-tool cases and strict locale inventory passed.
Complete UI generation passed in 17.63 s (95,084,748 generated Rust bytes),
[record](data/2026-10-30-home-navigation-ui-generation.json). Keyboard gestures,
screen-reader behaviour and visual layout remain unqualified by these static checks.

## Simulated home-screen interaction qualification

`python3 scripts/check_home_ui.py --offline` passed locally, including strict
Clippy and actual Slint event delivery against the imported `ui/start.slint`.
It verified accessible clear, typing after focus restoration, Escape, Tab with
both Enter and Space, enabled-card activation and rejection while busy. The
fixture contains synthetic projects and callback state; it does not copy the
home screen's implementation. It generates about 5.68 MB of Rust, keeping this
check separate from the full editor's roughly 95 MB generated UI. No desktop
build speedup is inferred from this size difference.

Headless CI now runs the fixture for UI changes. No physical window, GPU, audio,
provider account or personal profile is used. The selected simulated interactions
are qualified locally; real screen readers, physical keyboard/window integration,
visual rendering and the complete editor journey remain separate. The first
focused desktop dispatch stopped at checkout because its ref was abbreviated;
it was relaunched with full source `1c017cf4` in workflow `41109516254`.

The initial headless-CI fixture run stopped at the Fontconfig build prerequisite;
it did not execute the UI assertions. The runner now explicitly installs
Fontconfig/FreeType development libraries. Review also corrected the repeated
resource cycle to assert clip opacity and undo/redo state, rather than require a
new object ID from an update command (updates intentionally create no ID).

### Native qualification follow-up (31 October)

At `9c93fb71ebcdb11042c6aa387f35159d883bea2f`, full native Clippy, ordinary tests, documentation examples, gRPC checks and all 13 selected virtual desktop regressions passed in run 41110924160. The optional runtime probe failed before seek measurements: software window selection intentionally returns no shared GPU, while the probe called the texture-only API. The probe now explicitly acquires a dedicated compositor device in that case and records this distinction; its timings do not qualify the production CPU-window presentation path. Child failure diagnostics are printed once rather than duplicated into the assertion, allowing the collector to retain partial records. Runtime qualification remains pending a new run.
