# Desktop upstream qualification — 4 November 2026

Tested source: `a6a9a352ca3575cab82ada82272e5b6e6bde8f4a`.
[Full CI 41867492557](https://github.com/tulcoturo/clovercut/actions/runs/41867492557)
passed. This qualifies the code in PR #17; the following evidence-only commit
changes documentation and the retained runtime report, not application code.

Passed workspace Clippy with warnings denied (all targets and walkthrough),
1,332 ordinary workspace tests, doctests, gRPC Clippy and 30 gRPC tests. All 14
selected virtual desktop journeys passed, including the fade gesture controller,
clipboard isolation, exact preview/export parity and full editor workflow.
The runtime report is complete with process exit code zero.

[Raw runtime report](data/2026-11-04-desktop-upstream-runtime.json) retains all
samples, source metadata and software-adapter limitations. Of 18 explicit scrub
requests, 13 returned approximate frames and five were already exact; every
following refinement was required to be exact. These are frame-source call
observations, not physical presentation latency or a controlled before/after
speedup. The report explicitly does not qualify a performance budget.

The explicit cache trim observed 70,502,400 CPU-cache bytes before and after:
the pool was already below the 96 MiB idle cap. This sample does not demonstrate
a memory saving, a plateau or timed-idleness behavior. Keep decoder thread and
allocator policy changes separate from this result.

The Linux virtual display/audio/software adapter does not establish physical
Monterey wheel smoothness, native Apple encoder quality or Windows/macOS runtime
qualification. Issue #18 remains open for reporter validation of a built candidate.
No release or Gatekeeper notarization follows from merging this source.
