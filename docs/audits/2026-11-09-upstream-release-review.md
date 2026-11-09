# Upstream review for Clovercut 0.2.4

Compared `9d7f61b3` to `9fcd218b`: four new commits, all reviewed.

| Commit | Clovercut decision |
| --- | --- |
| `b6fd874f` job wakefulness | Adapt host RAII wake holds on existing `SingleFlight` jobs, preserving cancellation ownership and job lifetime. Windows system power requests and macOS idle-sleep assertions are released after the final job ends. Linux behavior remains unchanged; no desktop or idle configuration is edited. Android integration remains outside this desktop release. |
| `77c4be81` look names | Adopt fourteen descriptive display labels while preserving package folders, stable IDs, shaders and parameters. Existing projects and bundled-card lookups continue to resolve. |
| `c63ae643` Chroma Key bisection | Upstream's seven-picture screen finder differs from Clovercut's retained chroma shader. Keep our existing per-package canaries and new non-empty facial-effect WARP regression; do not import a different effect implementation or parameters. |
| `9fcd218b` formatting | Upstream `chips.rs` formatting only; no runtime fix to transplant. |

The initial native effects runs exposed missing bundled cards and an aliased clock assertion. Four actual GPU-rendered 480×270 cards are now included. Animation is checked at constant strength and non-aliased clip instants. These assertions remain mandatory. The headless workflow reached its ten-minute limit while compiling the media fixture after the preceding fixtures passed; its budget is now twenty-five minutes, retaining every check.
