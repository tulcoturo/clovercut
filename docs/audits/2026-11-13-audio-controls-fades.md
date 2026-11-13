# Audio controls, stems and compatible fade curves

Adapts 8a402129, 444476f6, dd8bb0a3 and aa913fac. The Audio panel now uses the
calibrated gain fader, Speech/Song stem rows with individual mute/restore,
waveform fade panes and draggable corners. Speech is the default isolation
engine, using the bundled model. Clovercut keeps its full existing Audio Speed
page and curve editor. Peak waveforms and gain boosts gain the red clipping
zone above amber.

New clips use quadratic audio fades. A saved clip without quadraticFades reads
as linear, preserving its existing sound; the inspector can choose either and
undo/save the choice. Preview uses the shared fade_gain envelope and export
uses FFmpeg curve=qua. Sound-layer mix/visual-layer ramps remain linear. The
curve is carried through flattened clips, voice stems, retimed audio pieces,
live specs and active playback. Baked bus cache keys include the curve; ordinary
source decode caches do not, because their envelope is applied live.

Timeline ramp endpoints and Bezier controls preserve the original quadratic
when the visible clip is only a slice. Fade-pane waveforms sample the selected
stream over the actual source clock, including speed curves. The same source
clock fixes retimed eyedropper/mask/analysis lookups that previously used only
the constant mean rate. Voice stem levels now scale keyed gain tracks as well
as the constant volume, so a volume ride cannot bypass the stem mixer.

Local tests passed the actual exported PCM envelope sample by sample against
the square, live fade gain and cache keys, old-document linear defaults, source
clock integration/bounds and keyed stem gain. All 65 export and 260 project
library tests passed. Strict all-target Clippy for media/project/export/host/
API/MCP passed; full app Slint frontend compilation and all 26 locale checks
passed. Workspace Rust formatting passes. Native controller/window checks are
still required; a controller echo regression checks the new saved curve patch.
