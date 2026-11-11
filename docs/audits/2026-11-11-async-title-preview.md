# Background visible-title preparation

Adaptation of Concat `997c6ff9` for Clovercut's visible-only, in-memory preview.

The interactive monitor prepares an uncached set of visible titles/shapes on a
project-scoped worker. It shows the footage meanwhile and requests the frame
again on completion. Only one job runs at a time; the completed set is keyed by
timeline, revision, preview dimensions and visible cue IDs. Incomplete flattened
lists are not cached as complete. A failed title is attempted once for that set.
Animated titles retain their generated image source, evaluated at frame time by
the rendering worker. Direct manipulation retains the live rendering path.

`preview_clips` still provides complete preparation for saved-frame and parity
callers. The monitor uses `preview_clips_ready` instead. This distinction avoids
capturing a saved frame from an intermediate preview with its titles missing.

Completion identity uses the existing project epoch and request ownership rules.
Project close clears prepared results and advances the live cache generation;
a painter finishing late cannot repopulate that cleared cache. No all-timeline
prepainting or PNG encoding is introduced into the interactive path.

Validation:
- Strict standalone Clippy and six ownership/cache tests passed locally.
- Native host regression covers a late painter after cache clear.
- A disposable desktop regression checks non-painting interactive requests,
  actual off-thread painting, animated paint keys, flattening reuse and complete
  saved-frame preparation; added to the existing virtual desktop CI selection.
- Native build and graphical validation run on GitHub Actions. No full native
  app compilation or desktop launch is performed on the user's machine.

No end-to-end latency benchmark has been measured for this adaptation.
