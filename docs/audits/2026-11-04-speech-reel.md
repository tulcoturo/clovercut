# Speech-only caption input

Selective adaptation of Concat e2b20c27, building on PR #19's language selection.

Whisper now receives the detected speech with 1.5 seconds of leading context
and 0.5 seconds of trailing context. Overlapping padded intervals are merged.
Caption times are mapped back to the original decoded clip window before the
existing speech-boundary refinement and controller timeline conversion.
Progress uses the audio actually sent to Whisper.

At an exact concatenation boundary, starts map to the following source piece
and ends to the preceding piece. This prevents a new caption from starting
at the end of the previous piece, across removed silence. A caption crossing
multiple pieces spans their original timeline interval.

Empty or unusable detection retains the original audio, preserving the existing
fallback for quiet speech. Explicit language selection, English-only models,
thread limits and cancellation remain in place. No repeated-dialogue deletion
or new hallucination heuristics were imported.

Validation: three standalone Rust regression tests pass under Clippy with
warnings denied. They verify copied sample contents, unsorted intervals,
overlap merging, fallback borrowing, empty input, seam endpoint bias and
mapping bounds. The headless workflow runs these tests without native speech
libraries. Full native compilation and recognition quality with a downloaded
Whisper model remain pending; these tests do not establish transcription
accuracy or a performance gain.

Headless GitHub validation passed on `b267365f` (run 41887445211).
The first native dispatch (41887446290) failed during checkout because the
supplied ref was abbreviated; no compilation or tests ran. It was relaunched
with the full 40-character source SHA in run 41888041844. A subsequent native
regression also covers source mapping followed by speech-boundary refinement.
