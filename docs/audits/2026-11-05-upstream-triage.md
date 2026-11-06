# New upstream inventory

Fetched 5 November 2026: upstream moved from `8c86502d` to `483c9d6a`.
There are 24 new commits. This is an initial triage, not completed behavioral
qualification of every feature.

| Commits | Decision / next review |
| --- | --- |
| `4ab430e6`, `f0587664` | Shared device flags and memory thresholds adapted in the GPU candidate; macOS raster-window safe start remains separate. |
| `e938cfdf` | Review export pipelining/YUV conversion separately against alpha, colour/HDR, cancellation and atomic export guarantees. |
| `f500664b` | Workspace integrated with named preset identity and Reset semantics; full native CI passed 1349 ordinary, 30 gRPC and 14 virtual-desktop tests. See remember-workspace audit. |
| `a21dd4f6` | Integrated in #25 with all-timeline Undo, pending-search cancellation and stale-result protection; full native CI passed. |
| `b5e19da2` | Reviewed save-before-exit flow; launch failure, active recording/export and lock release need explicit handling. See workspace/restart review. |
| `25cf5073` | CapCut importer requires separate format, path handling, fixture and unsupported-feature review. |
| `d75331bc`, `2b29732e` | Compare enhancement controls/progress against existing cancellation and estimates. |
| `773efd12` | Clovercut already exposes caption generation in the inline library; inspect for remaining useful differences. |
| `0b1f125d`, `c009d037` | Editable shapes and MCP sizing form a larger coordinated feature; retain existing API/tool schema compatibility. |
| `80ed9457`, `0b94e48f` | JSON Schema API export is useful for automation; adapt only after inventorying Clovercut's extended contract. |
| `b27a3943` | Adapted in merged PR #24: external-link arrow on the empty-home guide; existing home UI suite and headless CI passed. |
| `761a0233`, `8aa44956`, `87017346`, `29814cd8`, `784021e7` | Test/lint fixes need comparison with Clovercut's codecs, tests and speech implementation rather than direct import. |
| `fcb52f72`, `9ab67d05`, `df53aa6c`, `483c9d6a` | Upstream audit/legal/repository/README presentation changes are not automatic Clovercut changes. Preserve our audit evidence and project presentation. |

Earlier feature backlog remains in the 4 November review. This inventory adds
new candidates; it does not supersede compatibility decisions there.

Detailed workspace/restart review: [source comparison](2026-11-05-workspace-restart-review.md).
