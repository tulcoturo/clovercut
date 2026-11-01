# Export lifecycle follow-up

## Confirmed problems and changes

Reopening the export sheet unconditionally reset its phase to idle, even while its worker was rendering. Open now reveals an active render without changing its phase, progress, stage or choices. Again, Start and Overwrite cannot reset or replace a running render.

Progress, completion and gallery-publication callbacks previously carried no render identity. Each accepted render now captures a process-wide request identity using the existing request ownership helper. Cancel invalidates it immediately; obsolete progress, failures, successes and publication callbacks cannot change a replacement render or publish its result through the pane. Completion is accepted only while that same request is running; gallery completion is accepted only for its matching completed render, once.

Cancel uses the job's own cancellation handle. A worker can release the shared exporter slot before its queued UI completion arrives; cancelling the pane in that interval must not cancel another export that has since acquired the slot. The shared single-flight slot still prevents concurrent workers and remains occupied until the cancelled worker exits.

Exports remain window-wide: switching the active project does not discard a valid render of the captured project snapshot.

## Validation

The existing native import/edit/reopen/relink/export journey now also closes and reopens a running export, exercises repeated start/retry/overwrite requests, cancels its real worker and waits for the slot to be released. It starts a replacement render, injects the previous request's progress/success/failure/publication callbacks and checks that the replacement state remains intact, then waits for its actual export and independently inspects the encoded output.

Local formatting, diff checks and the three actual-source request ownership tests pass. Native compilation, strict Clippy and desktop regression validation are pending CI. These controller tests do not qualify physical mouse/keyboard interaction or mobile gallery publishing.
