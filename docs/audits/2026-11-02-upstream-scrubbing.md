# Upstream scrubbing adaptation — 2 November 2026

Source: [Concat 75919f5e](https://github.com/jub0t/Concat/commit/75919f5e).
Parent Clovercut source: `5df3ec2d` on main, after the
[native large-project baseline](2026-11-02-native-large-project-baseline.md).

Fast interactive seeks can use cached or nearby keyframes, then request an exact
frame after rest. Exact playback, export and saved-frame paths remain explicit.
The adaptation preserves Clovercut's f64 timeline clock, generated media,
canvas handling and preview observations. Time-dependent cutout and stabilization
sources stay exact. Refinement carries a request generation so a late result
cannot overwrite a newer seek, edit, quality change or playback.

The pool caches approximate frames under their actual decoded source index.
Local FFmpeg 9 B-frame footage exposed keyframe-only decoding that could return
backward timestamps without reaching the indexed target. Such failure falls back
to exact decoding and disables approximation for that path until pool clear.
The regression covers both a controlled keyframe fixture and default B-frame
footage, plus exact cache reuse and reset of the fallback state.

Local validation with pinned Rust 1.93.1 and two build jobs:

- `cargo clippy --offline --locked -p concat-media --all-targets -- -D warnings`: passed.
- `cargo test --offline --locked -p concat-media --lib -- --test-threads=1`: 155 passed, 3 ignored.
- `cargo fmt --all --check` and `git diff --check`: passed.

These checks cover the media library, not the full desktop integration. Full CI,
selected virtual desktop regressions and the runtime probe are requested for the
candidate commit. Native application qualification remains pending their results.
The phase-6 probe records separate nearest/refinement timings and exactness flags;
no controlled speedup or physical interaction latency is claimed here. Memory
policy from upstream `f19fda34` is deliberately a separate follow-up.
