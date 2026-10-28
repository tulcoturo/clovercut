# Desktop qualification

This is an execution checklist, not a passing result. Run it in a dedicated QA
session with synthetic media and a fresh application profile. Preserve the exact
source/package SHA, OS/architecture, GPU/driver, audio device, scale and locale
with the result. Use [TESTING.md](TESTING.md) for automated suites and
[EDITING.md](EDITING.md) for editing contracts.

Do not use a timed screenshot driver's completion as qualification. Its staged
warnings do not reproduce failed saves, and configuring export does not create
an output file. Keep credentials and personal media out of captures/logs.

## Setup and evidence

Record a separate result for Linux, Windows and macOS; a package startup check
is not interactive editing coverage. Verify the running executable's source or
candidate artifact, especially if a portable and native install coexist.

Use a disposable user/profile so the application cannot reuse personal projects,
accounts or settings. Linux supports fresh XDG directories; on Windows/macOS use
a dedicated QA user unless profile isolation has been independently verified.
Prepare two short videos, a still, a transparent image, a title/caption fixture,
a short audio file and a longer audio file. Keep originals outside the project.

For each row, record **pass**, **fail** or **not run**, with a screenshot/log and
observed behaviour when relevant. An unavailable screen reader or service means
not run. Do not fill in the whole checklist from one successful export.

## Editing and recovery

| Action | Required observation |
|---|---|
| Start with a fresh profile; create and reopen a project | No stale project/settings; correct name, dimensions, rate and background |
| Import video, still, audio and captions | Finished imports show correct metadata; failures identify the affected file |
| Select clips; seek via ruler; use empty space/selection box | Selection and seeking follow the documented separate actions |
| Split via shortcut, menu, playhead button and razor at fractional rates | Targeting and locks preserved; timing follows the documented cut policy; undo/redo restores state |
| Trim/move audio with snapping off; edit several linked tracks | Subframe timing remains possible; locks and linkage semantics remain intact |
| Edit a title and selected words; apply caption-collection styling | Only the intended text/captions change; save/reopen and undo preserve styling |
| Copy generated media/captions across projects | Destination owns required media; caption groups are independent; cancelling/switching rejects stale delivery |
| Save/reopen after several edits; retain an older valid saved version | Reopened project reflects the completed save and undoable editing remains coherent |
| Remove/move a synthetic source, then search a folder with duplicate names | Search stays responsive; nearest deterministic filename chosen; remaining missing items stay visible |
| Cancel a long relink search, switch projects or restore a source before completion | Delayed results do not change restored paths or another project; successful relink is one undoable batch |
| Save to an intentionally unwritable disposable destination | A real save failure is visible; previous valid saved data remains intact; recovery has a usable next action |
| Clear preview cache after Enhance/Reverse/freeze-frame edits and undo | Generated source media remains playable and survives save/reopen; do not run on published 0.1.9 |
| Preview/play/seek while editing captions and audio effects | No stale picture/preview takeover, doubled mix or continued playback after stop/close |
| Export a short edited project; inspect the actual output | Correct dimensions/rate/codec/audio/final picture; caption and transition appearance match intended output |
| Cancel export and provoke a disposable output failure | Existing destination remains intact; cancellation/failure is reported and retry works |

Use a disposable file/permission boundary to provoke a real failed save. Disk-full
qualification belongs in an isolated size-limited filesystem; never exhaust the
host's system disk. These are separate scenarios and should have separate results.

## Libraries and delayed work

Start a sound/sticker download, then select another preview, change timeline,
change project and disable its provider in separate runs. Newer preview choices
and the active project's media/selection must remain intact. Revisit the catalogue
to confirm reusable cached downloads still work. Place two different recordings
with the same filename and byte length: neither may silently reuse the other's
owned copy. Time longer-file adoption to detect synchronous UI stalls.

Repeat delayed-work checks for automatic captions and generated speech: cancel
and start a new request, switch timeline before completion, and reopen the panel.
Old progress/results must not end the new job or add captions/voice to another
timeline. A successful speech result must retain its requested start and style.

Exercise offline browsing, expired URLs, explicit login/reconnect, unavailable
browser/runtime, port conflict and startup timeout where the environment permits.
Record which cases use local fixtures and which use an actual provider. Check
visible licence/credit details for downloaded content; authenticated cases need
a dedicated QA account outside committed evidence.

## Layout, keyboard and accessibility

Repeat the core selection, inspector, modal and export tasks in both themes at
100%, 150% and 200% scale, including a narrow window and a maximized window. Include
a long translation and an RTL locale. Check clipped labels, overlapping controls,
selection visibility, drag/drop feedback and tooltip placement.

Use only the keyboard for one complete create/import/edit/save/export journey.
Verify Tab/Shift+Tab order, visible focus, Enter/Space activation, Escape dismissal
and shortcut isolation while typing. With the platform screen reader, inspect
icon labels, roles, enabled states and actions. Confirm loading/error status is
understandable without relying on colour alone.

## Result record

Attach the completed rows to a dated audit with exact source/package provenance,
environment and failures. Link applicable ignored-test output, real exported
fixture inspection and screenshots. Record unexecuted rows explicitly. Candidate
packages and development fixes are separate from a published release; passing
this checklist does not publish or tag a release.
