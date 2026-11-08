# Concat review, 8 November 2026

Reviewed all 22 commits after `443fb2a3` through
`f1d59f3dbc759c6a7a31204c3c247f5273630762` in the local upstream checkout.
This supplements the [issue inventory](2026-11-07-upstream-issues-review.md).
An ancestry count includes selectively adapted commits with different hashes;
it does not measure missing features.

| Commit | Decision for Clovercut |
| --- | --- |
| `3c6fc266` relink | Adapt the missing Modal close-to-dismiss bridge. Preserve asynchronous searches, cancellation and stale-result guards. Per-file locating and automatic dialog sizing remain separate work. |
| `40403032` full-page settings | Retain the existing settings modal; a page replacement needs its own responsive and focus qualification. |
| `bf9ba9f0` caption controls | Keep caption collections and per-word styling. A searchable shared style picker needs integration with that richer model. |
| `78b11014` tray grouping | Cosmetic regrouping deferred; no functional delete regression established. |
| `8a402129` audio controls | Defer the coordinated stem mixer and inspector rewrite; do not expose controls without the model and audio-bus integration. |
| `27626edd` choice fields | Retain current controls; a global visual replacement needs keyboard, phone and inspector checks. |
| `444476f6` stem controls | Part of the separate voice-isolation integration, with generated-media ownership and cancellation requirements. |
| `dd8bb0a3` speech default | Depends on voice isolation above; do not import an unused default. |
| `e9c0f6ad` settings width | Fixes the new upstream settings page, which Clovercut has not adopted. |
| `b97a3242` remove filename | Retain the existing inspector context; cosmetic change deferred. |
| `aa913fac` quadratic fades | Preserve existing projects' linear fade sound. A selectable curve and project migration need explicit qualification; do not silently reinterpret saved fades. |
| `34cdf4ab` long text | Adapt scrolling and caret reveal to title, caption, speech and voiceover fields. Inspector title height can be dragged; fixed dialog fields retain their bounded height. Use Clovercut's direct wheel handling, clamp stale offsets and resize only while pressed. Preserve Unicode selection and recording guards. |
| `fc60f4eb` GPU budget override | Keep resource defaults; a new escape hatch needs parser and device-pressure tests before becoming supported configuration. |
| `ec5538b4` curve grid and multiselect | Defer the larger speed editor interaction change; preserve exact retiming and existing gestures. |
| `1e1051ca` meter and outline styling | Cosmetic inspector changes deferred with the associated control redesign. |
| `6b22a49b` continuous speed audio | Already implemented through Clovercut's continuous RubberBand/sample-clock path, including cut windows and audio buses. Do not replace it with upstream's simpler atempo path. |
| `ba532ebf` speed graph height | Keep current layout pending the coordinated graph review. |
| `c6be3e05` speed icons | Cosmetic change deferred with the graph review. |
| `a9b6db97` CI and formatting | Clovercut already has its own bounded native/headless workflows; no wholesale workflow replacement. |
| `ff907196` colours and tab layout | Retain typed-track and inspector behavior; evaluate any silent-video tab change with actual audio metadata rather than importing the complete cosmetic rewrite. |
| `1459eee2` chroma device-loss suspect | Not applicable: Clovercut retains the small colour/similarity/softness/spill shader, without upstream's automatic screen-finder loop or its `continue`. Importing that shader would change effect parameters and saved-project output. No Windows driver fix is claimed. |
| `f1d59f3d` version | Upstream release metadata only; do not change Clovercut's version or published 0.2.3 artifacts. |

## Qualification

The long-text fixture renders the real components with the software renderer,
including direct wheel input, caret reveal, text shrink, resize gestures,
Unicode word selection and the voiceover recording guard. The relink fixture
checks both the header close button and clicking outside the dialog. Native
and headless CI qualification is recorded on the candidate pull request.
