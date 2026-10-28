# Test suites and qualification

Run Rust commands from `src/` with the native dependencies configured as described
in [building](RELEASING.md). Run Python/Node commands from the repository root.
Use full commit SHAs for workflow `ref` inputs; checkout does not resolve an
abbreviated SHA as a commit.

## Fast editing/document feedback

Changes to `concat-core` and `concat-project` run **Headless quality checks** on
pushes and pull requests. It uses the pinned Rust toolchain and checks these two
crates without Slint, FFmpeg, GPU or audio dependencies. The full release CI stays
mandatory; this narrow check does not qualify the desktop application.

```sh
cd src
cargo clippy --locked -p concat-core -p concat-project --all-targets -- -D warnings
cargo test --locked -p concat-core -p concat-project
```

Project-storage changes also run in the fast job. The isolated check compiles the
actual `concat-host/src/projects.rs` source with its existing tests, including
recents, legacy manifests, ordered/concurrent saves and failure recovery:

```sh
python3 scripts/check_project_io.py
```

Run this command from the repository root. `--offline` uses already cached
packages. The script selects the pinned toolchain through `src/`, uses a temporary
manifest with the workspace version, and reuses exact dependency versions and a
copy of the workspace lockfile. The workspace lockfile stays unchanged. The check
needs no FFmpeg/Slint/GPU/audio libraries and adds no workspace crate. It qualifies
this storage module, not the host's full media/service integration. Windows-only
cases still require Windows qualification; Linux runs do not execute them.

## Ordinary validation

| Suite | Command | Requirements |
|---|---|---|
| Formatting | `cargo fmt --all --check` | Pinned toolchain |
| Workspace lint | `cargo clippy --workspace --all-targets -- -D warnings` | Native libraries; generated UI compilation |
| Workspace tests | `cargo nextest run --locked --workspace --profile ci --build-jobs 1` | Native libraries, FFmpeg/ffprobe; CI requires a usable GPU adapter |
| Locale rules | `python3 -m unittest discover -s scripts -p test_locales.py` | Python |
| Translations | `python3 scripts/locales.py --check --strict` | Python |
| Models | `python3 scripts/models.py --check` | Python; no model inference |
| Connector | `for file in src/crates/concat-host/audio-service/test/*.test.mjs; do node "$file" || exit; done` | Node 22+; loopback HTTP fixtures, no browser/account/external service |
| Linux packages | `python3 -m unittest discover -s scripts -p test_linux_packaging.py -v` | Linux, dpkg-deb, RPM tools, makepkg/pacman; does not install on the host |

The Node suites report 20 cases at this increment: 11 catalogue/API, 6 preview
language cases and 3 session-recovery cases. The API/session fixtures are shared
with the companion Audio API and now also test the copy shipped with the editor.
The Linux suite checks
real package payloads and mocked installer failure paths; it does not qualify
installation on every distribution. Release workflows require checks against
the selected source commit, not a previous successful branch run.

For connector-only changes, the manual **CI** dispatch offers `connector_only`.
That run is labelled **Audio connector checks** and skips Rust compilation.
Reusable release calls do not expose this selector and always run the engine.

Linux builds separately run `scripts/ci/smoke-native-linux.sh` in clean Debian 12
and Fedora 43 containers: install, dependency resolution, version, graphical
startup, reinstall, remove and user-data preservation. These checks require Docker
and are never run against the developer's host package manager. The optional
`release_tag` input of **Linux packaging checks** qualifies an existing published
version without rebuilding the application; see [release validation](RELEASING.md#package-validation).

The small pure Enhance planning module can also be checked without the generated
UI or native libraries, from the repository root:

```sh
rustc --edition 2024 --test src/crates/concat/src/enhance_plan.rs -o /tmp/clovercut-enhance-plan-tests
/tmp/clovercut-enhance-plan-tests
```

This is a narrow policy check, not a substitute for compiling the application.

## Ignored tests: complete source inventory at 0.1.9

There are 28 ignored Rust test attributes in this baseline. Ignored tests are
compiled when their platform/features are enabled but are not executed by the
ordinary default test run. Each group below needs an explicit qualification run.

| Group | Cases | Prerequisites and execution policy |
|---|---:|---|
| `concat` phase-3 timeline | 5 | Isolated graphical session and GPU; transport case also needs FFmpeg. Run when editing cut timing, placement or geometry. |
| `concat` phase-4 sync, robustness, preview/export parity | 3 | Isolated Slint/GPU/audio session and FFmpeg. Run for transport, timing, rendering or audio changes and before qualifying a desktop release. |
| `concat` phase-6 performance/memory | 1 | Graphical/GPU/audio session and FFmpeg; labelled hardware baseline. Report samples separately from functional assertions. |
| `concat` source preview | 1 | Graphical session, FFmpeg and audio output. Run when changing library/timeline preview routing. |
| `concat` durability warning and running-work shutdown | 2 | Isolated graphical/audio session; shutdown case also needs FFmpeg/GPU. Run for lifecycle and recovery changes. |
| `concat` cross-project clipboard | 1 | Isolated graphical/audio session. Run when changing copying, ownership or destination track placement. |
| `concat-vision` CoreML Enhance probe | 1 | Downloaded model and supported CoreML environment. Performance probe, not generic CI coverage. |
| `concat-vision` stabilisation timing | 1 | Synthetic frame analysis benchmark; no stable cross-machine timing budget. Run when profiling tracking. |
| `concat-speech` file transcription | 2 | Downloaded speech model and explicit audio input. Run when changing speech/model integration. |
| `concat-render` upscale speed | 1 | Downloaded model and supported adapter; benchmark only. |
| `concat-media` MediaCodec comparison | 1 | Android device and `CONCAT_DEVICE_SAMPLE`; retain outside desktop qualification. |
| `concat-media` audio seek/decode timing | 1 | `CONCAT_AUDIO_PROBE` files; diagnostic measurement, not ordinary pass coverage. |
| `concat-host` split timing | 1 | `CONCAT_SPLIT_PROBE` long recording and optional cut position; diagnostic measurement. |
| `concat-host` real GPU Enhance comparison | 1 | `CLOVERCUT_ENHANCE_MODEL` and actual GPU. Verify parity and record timing. |
| `concat-host` GL catalogue/fixtures/rendering | 3 | One online download/cache case, one fixture-driven adaptation/validation case, one hardware rendering case. See [GL Transitions](GL_TRANSITIONS.md). |
| `concat-host` OpenGameArt | 1 | Internet; searches, downloads and reuses real provider content. |
| `concat-host` YouTube music/effects | 1 | Local Audio API and authenticated dedicated Studio session. |
| `concat-host` combined audio providers | 1 | Network, connected YouTube API and a Freesound key provided through the environment. |

Never run every ignored test indiscriminately. Some intentionally need private
inputs, several open graphical/audio sessions, and others measure performance
rather than assert a product contract.

For example, a specific graphical regression can be selected explicitly:

```sh
cargo test -p concat --lib studio::phase4_playback_robustness_tests::playback_robustness_and_recovery -- --exact --ignored --nocapture --test-threads=1
```

The separate screenshot driver is not an ignored test; it is opt-in development
tooling described in [the build guide](RELEASING.md#optional-graphical-walkthrough).

## Record results honestly

For each qualification, record the source SHA, platform/architecture, GPU/driver,
toolchain, suite/filter, environment prerequisites, result and relevant metrics.
Distinguish passed, failed, ignored and unavailable. No result recorded means
unqualified, not passed. Keep credentials, personal media and browser profiles
out of committed evidence and uploaded logs.

For performance comparisons, separate fixture setup, cold/warm compilation,
execution, decode, rendering and UI publication. Use repeated runs and report
p95/growth trends; do not compare arbitrary hardware as a regression budget.

Keep tests that protect saved data, existing output files, undo, ownership,
locks and preview/export agreement. Consolidation requires a named replacement
assertion and evidence that the same failure is still detected. The
[quality audit](audits/2026-10-28.md) records the current disposition decisions.

## Structured CI timing reports

Export the job metadata for a completed run and summarise it from the repository
root. Supply the full SHA of the source actually tested: a reusable workflow's
own branch/SHA may differ from its checkout `ref`.

```sh
gh run view 40666635410 -R tulcoturo/clovercut --json jobs > /tmp/clovercut-ci-jobs.json
python3 scripts/ci_timings.py /tmp/clovercut-ci-jobs.json \
  --output /tmp/clovercut-ci-timings.json \
  --source c23a64da016a78e0ec13cacd1999f560d94e155f \
  --run-id 40666635410 --runner-label ubuntu-24.04
python3 -m unittest discover -s scripts -p test_ci_timings.py -v
```

The versioned JSON contains every step, its conclusion, elapsed wall seconds and
observed totals by category. Incomplete/skipped intervals are `null`; failure
costs are retained. The gRPC and documentation steps include compilation, so their
full durations must not be labelled test execution. Parallel job durations must
not be summed into workflow wall time. API timestamps have second resolution.

Cache state defaults to `unknown`. Set `--cache-state cold` or `warm` only with
recorded evidence; a restored cache does not prove every crate was warm. Compare
the same runner/toolchain/profiles/dependencies and retain the matching command,
source and cache evidence. This tool reports observations and sets no performance
budgets. The [first increment report](audits/data/2026-10-28-ci-timings.json) is a
baseline with unknown cache state, not a controlled cold/warm comparison.

## Opt-in runtime performance report

The phase-6 probe requires an isolated graphical/GPU/audio session and FFmpeg.
Its child uses fresh XDG directories on Linux. Use a clean checkout and a dedicated
QA session; other platforms' profile isolation has not been qualified. Compilation
of this ignored test does not mean the graphical probe executed.

From `src/`, in Bash, supply provenance and retain both output and exit status:

```sh
export CLOVERCUT_BENCHMARK_SOURCE="$(git rev-parse HEAD)"
export CLOVERCUT_BENCHMARK_TOOLCHAIN="$(rustc --version)"
export CLOVERCUT_BENCHMARK_MACHINE="reference-machine-label"
set -o pipefail
probe_status=0
cargo test -p concat --lib studio::phase6_performance_tests::phase6_performance_and_memory_suite \
  -- --exact --ignored --nocapture --test-threads=1 2>&1 \
  | tee /tmp/clovercut-performance.log || probe_status=$?
python3 ../scripts/performance_report.py /tmp/clovercut-performance.log \
  --exit-code "$probe_status" --output /tmp/clovercut-performance.json
```

The collector retains partial records and exits nonzero for an incomplete or
failed probe. A completion marker alone cannot override a failed test command.
Metadata completeness is separate from execution status; absent provenance stays
unknown. `performance_budget_qualified` remains false: collecting samples does
not establish a performance budget.

Records preserve raw millisecond samples, mean, nearest-rank p95 and maximum.
Fixture generation is separate from initialisation, preview calls, waveform
extraction/publication, caption calls, undo/redo and 4K rendering calls. Names
ending in `calls_total` report a batch duration, not individual-call samples.
Preview calls do not measure paced display or audio/video synchronisation.
`texture_of` measures the host call, without device timestamp queries or a
presentation wait. Monitor clearing does not prove cold OS/driver/reader caches.

Process RSS/high-water counters are `null` when unavailable. DRM values identify
each device and are device-wide, not per-process or necessarily the selected
adapter; an empty list means no available DRM counters. Actual zeros stay zero.
The before/after close counters and existing assertions remain; one close does
not demonstrate a memory plateau or exercise a resource that was already empty.

The pure counter/statistics helpers can be checked without Slint or native media
libraries, from `src/`:

```sh
rustc --edition 2024 --test -D warnings crates/concat/src/performance_metrics.rs -o /tmp/clovercut-performance-metrics-tests
/tmp/clovercut-performance-metrics-tests
```

These four cases and the four Python collector cases run in **Headless quality
checks**. They qualify reporting logic, not graphical runtime performance.

## Missing-media search without the graphical application

From `src/`, test the same search helper used by the relink dialog:

```sh
clippy-driver --edition 2024 --test -D warnings crates/concat/src/relink_search.rs -o /tmp/clovercut-relink-search-tests
/tmp/clovercut-relink-search-tests
```

Eight cases cover nearest duplicate selection, stable equal-depth selection,
broken links, valid links with directory cycles, empty/missing searches,
cancellation, request ownership and delayed replacement eligibility.
The two symlink cases run on Unix; other targets compile the applicable cases.
The headless CI runs these checks on relevant changes without Slint/native media
libraries. It does not execute the graphical dialog, decode candidate files or
prove that identical filenames refer to the same recording.

## Project-owned audio copies

From `src/`, check the same destination-selection helper used by the sound library:

```sh
clippy-driver --edition 2024 --test -D warnings crates/concat/src/owned_audio.rs -o /tmp/clovercut-owned-audio-tests
/tmp/clovercut-owned-audio-tests
```

Two cases verify reuse of identical content, separation of equally sized but
different recordings, and preservation of valid/broken symbolic-link targets.
Comparison streams fixed-size blocks rather than loading a recording into memory.
These checks run in headless CI. They do not qualify interactive download,
playback or large-file copy latency.

## Controlled headless build baseline

The [three-pair local baseline](audits/data/2026-10-28-headless-build-baseline.json)
uses Rust 1.93.1, two CPU cores/build jobs and the same debug/codegen settings for
each repetition. Each pair starts with a new Cargo target directory and cached
offline dependencies, then repeats compilation and runs the tests with the same
target. OS disk caches are uncontrolled. Wall times include process setup and up
to 0.1 seconds of observation delay; short warm timings are approximate.

Artifact-cold test compilation took 17.337–17.443 seconds, warm compilation about
0.2 seconds, and the warm test command about 0.7 seconds, including Cargo and
documentation-test overhead. Reported child RSS high-water marks were below
520 MiB; they are not aggregate concurrent-process memory. All commands passed.
This establishes a repeatable headless development baseline, not a before/after
desktop speedup or a runtime performance budget.
