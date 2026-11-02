# Release 0.2.2 cancellation test follow-up

Release run 41415424564 built all six desktop targets successfully, but publication was correctly blocked by the native test job. Of 1,328 executed workspace cases, 1,327 passed; `test_export_cancellation_latency_and_destination_preservation` failed because its audio cancellation measurement was 107.74 ms against a strict 100 ms bound.

The measurement included worker startup, file/encoder initialization, scheduling and joining a thread after a 1 ms sleep. It did not synchronize with the start of audio mixing and does not establish a calibrated cancellation performance budget on a shared runner.

Replace that audio timing assertion with two functional checks: an already-cancelled encoded-file request must return the specific cancellation error and preserve an existing destination; cancellation requested by the first real PCM output callback must return the specific cancellation error without delivering another chunk. The latter exercises the shared mixing loop after work has started. Return time remains printed as an observation. Existing video cancellation, destination/scratch cleanup, export-slot release and retry assertions remain in place.

Only test code and this record change; production rendering code is unchanged. Native CI must validate the new source before publication. The earlier successful packages do not by themselves validate the corrected test source.
