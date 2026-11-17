# Transition clips

Adapts Concat `f2a26081`, `9ce13fff` and the document-version prerequisite
`0ddc8214`, while retaining Clovercut's legacy transition implementation.

Transition cards now create independent clips, on click or drag. A nearby cut
centres the transition; otherwise it acts over the shot. Their parameters occupy
a dedicated inspector heading, and selecting a replacement changes the single
transition package rather than appending an ignored effect. Transitions move,
trim, duplicate and paste using the existing clip commands and typed Filter
lanes. Default placement creates a Filter lane above the affected picture rather
than occupying an empty text/sticker lane. Explicit locked destinations remain
unchanged.

The new rendering path is isolated in `concat-export/src/transition_layers.rs`.
Without an enabled transition layer, the old per-cut resolver and its tests are
unchanged. In a project using layers, each side is carried through the span with
a silent stand-in, using source handles where available and held frames where
not. The GPU's direct preview texture and the ordinary composite use the same
side selection. CPU dissolve fallback blends premultiplied colour, preserving
transparent edges in eight- and sixteen-bit frames.

Clovercut-specific generated title sources hold their original endpoint instead
of restarting. Stand-ins also hold motion, caption treatments, keyed masks and
effect parameters at that boundary. Their shader clock is retained separately
from the decoder clock, including continued footage and held frames. Subject/
face processing, cancellation, range origins and the asynchronous encoder path
are retained.

Documents are now version 2. Version-1 projects remain readable; older builds
refuse the new document version rather than silently discarding independent
transitions they cannot represent. Original per-cut transitions remain editable
and render through the existing path when no new layer is enabled.

Local checks cover 64 export tests, 247 project-library tests and the project
integration suites, including all-kind clipboard and head-extension coverage.
New cases cover both sides, missing handles, a single shot, hidden/empty/tiny
layers, document flattening, generated/masked/moving stand-ins and transparent
dissolve output. All actual app Slint sources compile without locally compiling
the native GUI. The existing remote editor journey additionally checks the
controller's create, replace, duplicate, move, trim and undo behavior. Native
window validation is still required before publication.
