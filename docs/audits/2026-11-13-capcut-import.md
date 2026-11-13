# CapCut draft import

Adapts upstream 25cf5073 with the parser's subsequent fixes through 6584d349.
Desktop users can choose drafts from any folder, including copies from other
machines, without requiring a local CapCut installation. Opening or dropping a
draft also imports it. Parsing, probing and writing run off the UI thread; a
superseded result cannot replace a newer open project.

Import creates a separately named Clovercut project. Destination checks resolve
symlink parents before any write and reject paths inside the original draft.
Timeline identifiers cannot traverse outside the chosen folder. Malformed or
encrypted JSON is refused. Unsupported content is reported in the completion
notice and detailed in the log. Source drafts are never modified.

The adapter preserves typed lanes and an empty Video anchor for title/audio-only
drafts. Existing media is probed to restore stream metadata, particularly audio
which CapCut often omits from its video materials. Existing Clovercut speed
normalization and integral helpers replace upstream-specific equivalents.

Local parser and host tests exercise timing, styles, transforms, audio fades,
multiple timelines, stable project round trips, unsupported content, untouched
source bytes, separate repeated imports, symlink destinations and invalid IDs.
A tiny generated video/audio fixture checks that omitted metadata cannot mute
an imported video. Strict project/host Clippy and locale validation run locally;
all app Slint sources compile with the frontend-only checker. The actual native
window/controller remains subject to GitHub validation before publication.
