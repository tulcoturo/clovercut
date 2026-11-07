# Concat commits and issue review after Clovercut 0.2.3

Source: [Concat](https://github.com/jub0t/Concat), fetched 7 November 2026.
Upstream main remains `443fb2a33648a86733c4360544cd8f291f5aeaae`: **zero new
commits** after the source reviewed for 0.2.3. An upstream-only Git log counts
selectively adapted commits as missing because our adaptations have different
identities. It is not a feature-deficit count. At Clovercut baseline
`bb0b732d`, `git rev-list --left-right --count origin/main...upstream/main`
reports 376 Clovercut-only and 269 Concat-only commits. Those are ancestry
counts, including already-adapted code, not 269 absent fixes.

The [previous commit-by-commit review](2026-11-07-upstream-release-review.md)
and [earlier inventory](2026-11-05-upstream-triage.md) remain applicable. This
review revisits the remaining decisions and checks reports against our actual
source, rather than importing upstream wholesale.

## Issue inventory and selected changes

Read the titles/labels of all **210 open issues**, plus **40 recently closed
issues**. Open labels include 66 bugs, 108 feature requests, 10 performance,
20 Windows, 9 macOS, 21 Android and 6 iOS reports; labels overlap. This is an
inventory, not reproduction of all reports. Detailed investigation focused on
the desktop/API reports below. No issues or comments were changed upstream.

| Concat report | Clovercut finding and decision |
| --- | --- |
| [#595](https://github.com/jub0t/Concat/issues/595) | Confirmed: periodic render progress can stop before the final frame, particularly on short/silent cuts without later mux updates. Count written frames rather than zero-based indices, and emit the full frame count only after the encoder finishes successfully; normalize successful job state to the known total. Keep actual stage names, hardware fallback, cancellation and atomic output publication. Extend the real silent-export regression. |
| [#596](https://github.com/jub0t/Concat/issues/596) | Confirmed: MCP promises a completed output size but JobStatus has none. Add optional `bytes`, observed once from the published file outside the job-book mutex. Failed metadata reads do not turn completed exports into failures; replies without the new optional field remain readable. All transports share the same status. |
| [#689](https://github.com/jub0t/Concat/issues/689) | Confirmed structural risk: Modal caps the card height and clips a non-scrolling new-project form. Give this form a bounded scrolling body, retain its header and phone touch behavior, and reset scroll when opened. Test small/scaled windows, custom dimensions, wrapped errors and a real Create click. |
| [#688](https://github.com/jub0t/Concat/issues/688) | Confirmed structural risk: six natural-width shape chips and a preview exceed the body of the 640px card; the old breakpoint measured the background window. Use card width, equal multi-row shape cells and a reserved preview column. Test geometry and option activation in the real component. |
| [#693](https://github.com/jub0t/Concat/issues/693), closed [#548](https://github.com/jub0t/Concat/issues/548) | Confirmed with actual software-rendered frames: entering a poster button transfers hover away from the card and destroys the conditional action row. Keep the row instantiated and visible while the card or either button is hovered; inherit the home busy guard for pointer and accessible actions. The regression paints repeated pointer moves, checks that button pixels remain visible, and clicks Reveal/Remove without opening the project. Physical Windows driver behavior remains a separate check. |
| [#692](https://github.com/jub0t/Concat/issues/692) | Native picker parent/focus ownership needs a coordinated platform/window-handle change and a Windows interaction test. A headless UI check cannot prove native modal behavior. Keep separate. |
| [#598](https://github.com/jub0t/Concat/issues/598) | Our creation code already includes the failing operation/path and original OS error. A sandbox returning ENOENT does not prove PermissionDenied; do not invent that classification by parsing a localized message. Typed storage errors can be reviewed separately without changing every file failure's API contract blindly. |
| [#430](https://github.com/jub0t/Concat/issues/430) | Shorter template fills need an explicit fit/clamp policy: changing duration can change following typed-track clips, transitions, retiming and audio buses. Preserve ownership and the template clock until coordinated behavior/fixtures are specified. |
| [#680](https://github.com/jub0t/Concat/issues/680) | Long-text editing deserves its own actual-caret/selection/scroll test, preserving Clovercut's Unicode per-word styling and caption controls; not a reason to replace the text inspector with upstream's smaller model. |
| [#627](https://github.com/jub0t/Concat/issues/627) | Named export presets are a separate preference/UI feature. Do not conflate remembering destination settings with fixing job completion reporting. |

## Existing equivalents and larger pending work

- [#590](https://github.com/jub0t/Concat/issues/590): Clovercut already supports
  ProRes 4444/MOV and VP9 alpha/WebM. Correct stale MCP/API descriptions that
  listed only MP4 and three codecs; this does not add or remove a codec.

- [#555](https://github.com/jub0t/Concat/issues/555): Camera Shake is included in
  Clovercut 0.2.3; effects/filter hover previews and eight other new visual
  effects are also present. No duplicate Shake package is needed.
- [#319](https://github.com/jub0t/Concat/issues/319) and
  [#205](https://github.com/jub0t/Concat/issues/205): workspace/window persistence
  is already adapted, including named preset identity and Reset behavior.
- [#406](https://github.com/jub0t/Concat/issues/406),
  [#470](https://github.com/jub0t/Concat/issues/470),
  [#200](https://github.com/jub0t/Concat/issues/200) and
  [#481](https://github.com/jub0t/Concat/issues/481): retain our typed multi-clip
  clipboard/batch editing and direct fades rather than duplicating their UI.
- [#622](https://github.com/jub0t/Concat/issues/622),
  [#634](https://github.com/jub0t/Concat/issues/634),
  [#633](https://github.com/jub0t/Concat/issues/633),
  [#635](https://github.com/jub0t/Concat/issues/635) and
  [#682](https://github.com/jub0t/Concat/issues/682): model publication,
  horizontal wheel/scroll bounds, transition group dragging and safe rational
  arithmetic were adapted for 0.2.3. Physical Mac wheel performance remains a
  distinct open Clovercut report, not proven fixed by these tests.
- [#476](https://github.com/jub0t/Concat/issues/476),
  [#546](https://github.com/jub0t/Concat/issues/546),
  [#472](https://github.com/jub0t/Concat/issues/472),
  [#673](https://github.com/jub0t/Concat/issues/673) and
  [#667](https://github.com/jub0t/Concat/issues/667): retain qualified shared-GPU
  initialization, bounded preview work, approximate scrubbing/exact rest,
  CPU-VAD initialization and device-loss guards. Target/delivery FPS are now
  distinguished in main. Hosted software rendering does not qualify a physical
  low-memory laptop or establish the cause of a report without logs/media.
- Voice isolation (`6e73902f` through `443fb2a3`,
  [#120](https://github.com/jub0t/Concat/issues/120)) still requires three model
  downloads, project-owned generated stems, cancellation/reuse and audio-bus/
  continuous-retiming qualification. Do not add incomplete controls or a new
  multi-worker memory load to low-memory preview diagnosis.
- GPU YUV/pipelined export (`e938cfdf`) still needs alpha/HDR/clock/output parity
  and throughput measurements across supported backends. Correct progress does
  not claim a rendering speedup.
- CapCut ingress (`25cf5073`,
  [#477](https://github.com/jub0t/Concat/issues/477)), editable shapes
  (`0b1f125d`/`c009d037`, [#288](https://github.com/jub0t/Concat/issues/288)), full
  API schema (`80ed9457`/`0b94e48f`) and restart (`b5e19da2`) retain the previously
  documented format/ownership/extended-contract/lifecycle requirements. These
  are larger features, not safe independent cherry-picks.
- Mobile-only distribution, layouts and crashes are outside the desktop scope.
  Apple notarization and Windows trust/signing need real credentials/platform
  checks; disabling security or changing renderer backends blindly is not a fix.

## Qualification

Local qualification includes six tests of the actual API job book/wire types
and strict Clippy; the real home/new-project fixture with the software renderer
and bundled fonts; compact/scaled windows, custom fields, wrapped errors,
non-overlapping controls, pointer scrolling, real Create clicks and busy guards;
painted poster-button regions through repeated hover movements and Reveal/Remove
clicks; the existing home keyboard/search/view/theme/wheel regressions; workspace
formatting and diff checks. Wide and compact captured frames were inspected.

Integration requires full native Clippy/workspace/documentation/gRPC tests,
selected virtual-desktop journeys and the headless PR checks. The exact candidate,
CI run and final results are recorded in the accompanying pull request. These
checks do not qualify physical Windows/Mac performance or native file dialogs.
The 0.2.3 tag/packages are not modified by this follow-up.
