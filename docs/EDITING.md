# Desktop editing

These notes describe current development behaviour. See the
[release notes](releases/) for the features included in a downloaded package.

## Timeline

Click a clip to select it and open its inspector without moving the playhead.
Click or drag the time ruler or the reserved strip beneath its labels to seek.
Clicking empty track space or making a selection box changes selection only.
The default playhead is white; an explicit custom colour remains respected.

A split button follows the playhead just below the ruler on desktop. Click it
to use the same cut command as the toolbar and keyboard shortcut, respecting
the current selection, track targeting and locked tracks. Its hover label follows
the interface language. The button hides while using the hand tool or touch.

The timeline toolbar's plus button adds a timeline. Tabs appear when there is
more than one timeline; existing rename, close and reorder actions remain.
Automatic track heights use 80 px for the main video (the manual Medium height),
64 px for other video and audio, 36 px for stickers and 28 px for text,
captions and adjustment tracks. Manual Small, Medium and Large sizes remain
available. Inspector width is adjustable, with its minimum derived from the
active controls. A transition indicator spans both clips at the cut.

Track colours identify content consistently in the lane icons, clip headers and
drag previews: blue for video, green for audio, pink for text, gold for captions,
orange for stickers and violet for adjustments. Global audio effects keep a teal
clip colour. Downloaded animated stickers retain their sticker colour even when
their media file is a video; light and dark themes use matching palettes.

## Text and colour

Colour adjustment names, groups, wheels and curve channels follow the selected
language without changing project parameter IDs or keyframes. Groups can be
folded and reset independently. Text supports bold, italic, underline and
vertical writing. The development branch also adds strikethrough and display-only
uppercase, lowercase and sentence case; selecting the active case again restores
the original spelling. Captions retain their source text and group editing.

## Frame background and transparent export (unreleased)

Choose Background when creating a project, or change it in Project settings.
The choice belongs to each timeline: a new timeline inherits it, then can use a
different colour or Transparent. This fills gaps and space around titles as well
as uncovered picture edges. Existing main-lane and per-clip image/blur backgrounds
remain independent and render above the frame background.

Transparent areas use a checker in the monitor and export preview. ProRes 4444
exports to `.mov` with AAC sound; VP9 alpha exports to `.webm` with Opus sound,
when the linked encoder is available. Both keep transparency and write SDR;
HDR timelines are tone-mapped. Other formats composite transparency over black.
The export dialog explains this before writing and selects an available alpha
codec for a transparent timeline. ProRes quality comes from its profile, so it
does not show a bitrate control.

Old projects keep their opaque black background. Canvas changes participate in
undo/redo and survive save/reopen, including projects with several timelines.
These features are not part of the published 0.1.7 packages.

## Copy, paste and duplicate

Copy a selection, open another project in the same window and paste at the
playhead. This is an in-app clipboard, shared across projects in that window.
The complete clip settings, effect/keyframe state, caption membership, selected
audio streams and generated-media references are retained. Pasted caption groups
are independent of the source; neighbour transitions and detached-audio links
are retained when their companion clips are copied too.

External media remain references. Source-project-owned files are copied into
the destination before paste is accepted; verified resources and fonts are
reused. Conflicting font families do not silently replace the destination font.
Resource copying is cancellable. Closing/switching projects or deleting the
paste anchor prevents stale worker deliveries from changing another timeline.
Paste respects current destination track types and locks. Duplicate uses the
same group operation after the selected span, as one undo step.

## Content libraries

See [audio sources](AUDIO_LIBRARY.md), [GL Transitions](GL_TRANSITIONS.md) and
[favourites and usage lists](LIBRARY_ORGANIZATION.md). Category arrows page
through groups of library icons; wheel scrolling remains available.

### Styling selected words

In the Text inspector, select part of a title or caption, then change its fill,
bold/italic/underline/strike or size. Those controls apply to the selection;
**Whole title** returns them to the base style. **Reset** removes the selected
words' overrides. Selected-word formatting affects that caption alone, even when
whole-caption styling is configured to apply to its collection.

Word sizes remain relative to the title size. Their formatting follows inserted
and removed characters and survives undo, save/reopen and copying between
projects. Changing display casing keeps the original editable words. New titles
and generated captions use the default whole-title style without carrying over
formatting ranges from another title's words.

### Linking tracks during ripple edits

The chain button beside Magnetic enables **Linkage** for the active timeline.
It starts off so existing projects keep their previous behavior. The choice is
saved with the timeline and participates in undo/redo; new timelines inherit it.

When enabled, a ripple edit on Clovercut's fixed main video track moves the
following captions, titles, video overlays, audio, stickers and adjustment layers.
It covers magnetic trims, ripple delete, insertions that make room, freeze frames
and ripple speed changes. An edit on another track only ripples that track.
Linkage does not turn an ordinary move or non-ripple delete into a ripple edit.

Locked tracks remain unchanged. Text completely contained in footage removed by
a ripple edit is removed with it; undo restores both. Clips crossing a cut, such
as a music bed, keep their start and duration. A following clip on that same
track stops at the bed's end rather than overlapping it. Multi-track removals,
trims and speed changes count shared removed/inserted time once.

### Global audio effects

Drag an effect from the audio-effects library onto the timeline to create a
sound layer. It uses an adjustment track above the main video track; ordinary
audio tracks remain below video. Clicking a library effect still applies it to
the selected clip.

A sound layer treats the sum of all audible clips during its interval. This is
useful for one limiter, compressor, room or telephone treatment over the mix.
Select the layer and use **Audio → Layer** for strength and fades, or
**Audio effects** for its chain. Its stored opacity keyframes also control mix
strength, including easing. Source clips keep their independent gain, fades,
filters and speed ramps.

Overlapping sound layers run in track order from bottom to top, then start time.
Their order matters: a limiter after a gain boost differs from a limiter before
it. Muting the layer's track bypasses its effect while keeping the source audio.
The layer can be moved, trimmed backwards, copied between projects, saved and
undone like other generated clips, and follows main-track ripple edits when
Linkage is enabled. Locked tracks retain their existing protection.

Preview prepares the mixed interval in the background. Until it is ready, the
source clips remain audible untreated; once ready, that mixed interval replaces
them, avoiding doubled audio. Editing the treatment or changing projects cancels
obsolete work. Exports use the same mixer and retain cancellation and atomic
output publication.

## Project cache

Clear project cache removes the reserved thumbnail artwork directory, the project
poster and hash-named waveform entries. Other cache entries remain, including
enhanced copies, reversed clips and freeze frames needed by unsaved edits or undo. Audio/proxy
working directories also remain. The cache folder therefore travels with the
project: it contains generated source media as well as disposable previews.

## Relinking missing media

Choose a search folder in the missing-media dialog. Relinking matches exact
filenames and prefers the candidate closest to that folder; equal-depth matches
use folder-name order. It searches valid linked folders without revisiting cycles
and ignores broken links and non-file entries. The search stops once all requested
names are found. Filename matching does not establish that two copies contain the
same recording; review the result when folders contain duplicate names.

Successful replacements remain one undoable batch. Files that cannot be found
stay in the missing-media dialog. Interactive qualification of this workflow
remains separate from the filesystem-search checks.
