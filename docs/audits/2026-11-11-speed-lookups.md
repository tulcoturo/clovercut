# Prepared speed-curve lookups

Selective adaptation of the lookup/precomputation idea in Concat `05012463` /
`fc716d28`, preserving Clovercut's public SpeedCurve API and project model.

A curve prepares the cumulative integral at every knot once. `consumed(x)` then
uses a binary search and one segment's closed-form integral, instead of summing
every preceding segment for every rendered frame. `speed_at` and `slope_at` use
the same binary lookup; `mean` uses the prepared total. `pieces` reuses each
preceding endpoint instead of looking it up twice.

This keeps the existing arriving-side behavior at duplicate knots. The upstream
project-command normalization and step-cut editing changes are separate work;
this patch does not claim to port that entire multi-crate change. Construction
adds one f64 per knot; the frame-time lookups allocate nothing. Persisted project
fields and public method names are unchanged.

Validation: all concat-core and concat-project tests, including integration and
documentation tests, passed locally with one low-priority build job. Strict
Clippy passed for both crates/all targets. Two new regressions compare prepared
areas with numerical integration for straight/smooth curves and pin source-time
continuity, boundary behavior, duplicate knots, cloning and non-finite queries.

An optimized local microbenchmark compared the actual baseline `843fbd1e` module
with the modified module: 200,000 consumed queries, five runs, median elapsed time,
zero slopes and knot speeds `0.5 + (index % 7)`, query order `((index * 7919) % 200000) / 200000`.
The accumulated answers agree within `1e-6`. Observed baseline→modified times:

| Segments | Baseline | Modified |
| --- | --- | --- |
| 8 | 5.014 ms | 2.889 ms |
| 64 | 30.959 ms | 3.726 ms |
| 256 | 117.800 ms | 4.521 ms |

These are isolated lookup timings on one machine, not an end-to-end playback or
export benchmark. Normal small curves have a smaller benefit than long curves.
