# Remove a missing source from the project

Selective adaptation of Concat `a21dd4f6`. Missing-media rows show the total
clip usage across timelines and offer Remove from project. The existing
`RemoveMedia` command removes the source and its clips as one undoable edit.
The file on disk is not deleted. The dialog closes when its final row is removed.

Clovercut-specific safeguards: only listed rows in an open project can act;
removal cancels the pending search snapshot before applying the edit; a rejected
edit leaves the row available; stale IDs are ignored. Usage counting only runs
while the dialog is open. Existing cancel/dismiss/search behavior is retained.

Validation: full application Slint source generation passed under a 4 GiB
address-space ceiling. Strict locale inventory passed for all 25 locales
(1116 messages); three strings were adapted per locale, including a distinct
Dutch usage label. The existing multi-timeline RemoveMedia project regression
now verifies one Undo restores the complete project and Redo removes again;
it passed, as did strict project Clippy for all targets.

The existing native import/edit/reopen/relink/export journey is extended with
removal while a search is pending, Undo/Redo restoration, stale-row rejection
and a fresh successful relink followed by export. This controller journey still
requires native CI; no local full GUI compile or simulated trash-button click
is claimed.
