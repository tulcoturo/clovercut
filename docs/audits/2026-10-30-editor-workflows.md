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

Complete engine, virtual-desktop journey and runtime qualification are pending
for source `1d67eecffb90ed902b2a4d13fac928f519acaf1c` in
[CI](https://github.com/tulcoturo/clovercut/actions/runs/41101539243).

## Limits

The controller journey does not inject mouse/keyboard events, exercise a screen
reader or visually compare caption rendering. Software-adapter/null-audio results
are not physical GPU/audio qualifications. Six short resource cycles do not
establish a memory plateau or long-run leak behaviour. Synchronous adoption still
needs complete-call and event-loop observations before a background refactor.
Real provider accounts, cross-version native upgrades, actual Monterey execution,
long playback drift and the full cross-platform desktop checklist remain open.
