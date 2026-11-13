# Locate individual missing sources

Completes upstream 3c6fc266 while retaining Clovercut's asynchronous recursive
search, project epochs, cancellation and stale-source guards. Each missing row
now offers Locate for renamed files. Its source folder is searched at that
level for other missing names; a per-file choice never starts an unbounded
recursive scan. The chosen file and valid sibling matches apply as one undo.

Native file pickers have their own cancellation token. Dismissal, changed
missing lists, removals and later requests invalidate old answers; choosing a
file in an earlier project cannot affect the current one. Cancelling a chooser
does not leave the dialog falsely marked as searching. The worker rechecks the
original missing paths and every target before applying its proposal.

The dialog grows to its rows up to five, then scrolls. Rows show media-kind
marks, source folders with home shortened to ~, clip counts and Remove. Search
progress/cancellation and the header/outside dismissal bridge are retained.
Desktop scrolling uses Clovercut's existing direct wheel component.

Nine isolated filesystem/job regressions pass with strict Clippy, including
renamed sources plus sibling matching, no recursive sibling scan, cancellation,
symlink cycles, stable duplicates and stale replies. The actual Slint text/UI
fixture passes, including a real Locate-button callback, header close, outside
click and existing Unicode editor/resize behavior. Full app Slint frontend and
all 26 strict locale checks pass. Workspace formatting passes. Native dialog/
controller qualification remains required before publishing.
