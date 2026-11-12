# Colour glyph caching and layered emoji

Adapted Concat `9ca542cb` and `38d31b92` onto Clovercut `843fbd1e`.

The text renderer keeps decoded bitmap strikes and painted COLR gradient glyphs
in a byte-bounded, 32 MiB cache shared by titles using the same Fonts instance,
including concurrent painting threads. COLR v1 gradients, transforms, clipping
and composite layers can render in their own palette. Presentation selectors
U+FE0E/U+FE0F choose an outline or emoji picture where the font supports it.

Clovercut's mixed vertical writing, baseline placement, underline/strike,
vertical alignment, shape renderer, bundled font catalogues and fast PNG encoder
are retained. Cached marks carry an additional placement transform: rotated
Latin runs turn their bitmap or gradient picture; upright emoji remain upright.
The PNG title-cache version advances from 9 to 10 so existing cached titles are
repainted with the new colour renderer.

The upstream machine-font sequence test assumed every colour-font emoji had
saturated pixels. The installed Noto face renders the family sequence as a
neutral pictogram. The adapted test checks visibility, one-picture width, and
exact invariance under changed fill/outline/shadow instead; grayscale palettes
remain valid, and an outline substituted for the font picture fails that test.
Deterministic synthetic-font tests cover bitmap decoding, fallback, gradients,
alpha, cache bounds, concurrent sharing and presentation selectors independently
of installed emoji fonts. Two additional tests cover Clovercut's rotated Latin
and upright emoji bitmaps, rotated gradients and cache reuse across orientations.

Validation:
- 84 concat-text tests passed, including existing typography and vertical tests.
- Strict Clippy/all targets and rustfmt for concat-text passed.
- Public TitleStyle, TitleRun, RenderedFrame, Rendered and ShapeStyle fields were
  compared with the baseline and are unchanged.
- Actual PNG probes were generated and visually inspected for gradient/bitmap
  orientation. The new isolated Linux/Windows CI workflow keeps those probes.
- Builds were limited to pure Rust text dependencies, one low-priority compiler
  job, and two test threads; no full native application build ran locally.

No end-to-end latency benchmark is claimed. The repeated-emoji regression checks
one decode across twelve repeated glyphs and five paints, and the concurrency
regression verifies reuse across painting threads.
