# Voice isolation

Adapts 6e73902f and b184a55c, with 5e77293b's inspector commit correction.
DeepFilterNet 3 speech separation and Spleeter music separation run on CPU
workers sized to available cores and memory. The speech weights (8.6 MB) and
both licenses are bundled; music models download through the existing verified
model table, preserving Clovercut's existing speech providers and model entries.

The project records independent voice/background gains and engine choice.
SetClipVoice validates finite values, supports undo and survives save/reopen.
Inspector echo commits now emit that command rather than dropping the choice.
The existing audio panel exposes the mode, engine, individual levels and job
progress; clip menus provide Keep voice only and Remove voice. This adapts the
feature to Clovercut's inspector; later upstream audio-control presentation
commits remain separately recorded for review.

One background job splits each file/stream/engine into project-owned cache
parts shared by its clips. Closing the project cancels the job and stale UI
results are discarded by the existing project epoch mechanism. Failure is
reported and retains the original sound. Locked selected tracks are excluded.
Both playback and export use the same audio_pieces expansion; silent parts are
not decoded. Clovercut's continuous pitch-preserving retiming is preserved,
rather than replacing it with upstream's older constant-rate audio pieces.

Local checks passed the real bundled speech network on a public-domain voice
mixed with noise/tone, reconstruction of the original from both stems, both
engines' FFT round trips, strict engine Clippy, project save/reopen and undo,
and the shared mix's timing/gains/silent-parts test. The 256 project-library
and 64 pre-existing export tests also passed. Host/project/export/API/MCP
strict Clippy passed. All actual Slint sources compile with the frontend-only
checker, and all 26 locales pass strict validation. A native controller unit
regression checks echo commits for Keep, Remove and Off; native window checks
are still required before publishing. Music inference itself still needs a
reference run with the downloaded Spleeter networks; its DSP is locally tested.
