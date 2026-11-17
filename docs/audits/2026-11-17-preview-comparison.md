# Preview responsiveness: Clovercut and ReaCut

Compared Clovercut `a00e1cab` with ReaCut
[`7ba4c708`](https://github.com/Xiangyu-CAS/rea-capcut/tree/7ba4c708657717168928b7c815274517b951366e).
The reported problem is slow selection/playback in both the library and timeline
on Windows ARM. No measurements from that physical machine are available yet.

## Source comparison

| Area | ReaCut implementation | Clovercut implementation and decision |
| --- | --- | --- |
| Source preview | `src/components/SourcePreview.tsx` uses a dedicated media element, original/preview URL and `preload="auto"`; cleanup pauses and releases it. | `source.rs::preview_media` builds a source timeline and uses the same monitor/decoder pool as the timeline. Keep shared compositing semantics while improving its scheduling. |
| Playback reuse | `src/components/Player.tsx` keeps sources in a map and observes decoded video frames with `requestVideoFrameCallback`. | `concat-media::ReaderPool` already retains decoded frames and warm readers. Replacing it with browser media elements is not justified by the comparison. |
| Derived media | `electron/media.ts` keys cache entries by path, size and modification time; compatible preview proxies are separate from original media. | Existing proxy/cache paths should be measured for first and repeated selection, source changes and decoder misses before changing retention policies. |
| Foreground contention | Browser media decoding is separate from FFmpeg subprocess work used by the application. This is an architectural observation, not proof of superior latency. | `host.rs` previously submitted monitor frames, thumbnails, prefetch and analysis to one FIFO with three workers. All workers can be occupied ahead of a click. Reserve a separate frame worker. |
| Hardware | ReaCut does not establish performance on Windows ARM. | `hardware_wins` deliberately excludes 8-bit H.264 from desktop hardware decoding. Measure CPU versus D3D11VA on the affected Windows ARM machine before changing that policy. |

This is an independent implementation of the scheduling improvement, not copied
ReaCut source. Its HTML/Chromium rendering path is not CapCut's internal engine.

## Implemented first step

`PreviewWorkers` retains three background workers and adds one reusable frame
worker with its own FIFO. Only `spawn_frame` uses it; prefetch, thumbnail work and
other jobs remain on the background queue. No thread is created per frame.
Existing project-epoch checks and result publication remain in `deliver_then`.

Debug logging separates **frame worker queue time** from **frame worker execution
time**. These numbers do not include time spent waiting for the monitor's previous
request, UI event delivery or the GPU draw, and must not be called complete
click-to-first-frame measurements.

Two deterministic synchronization tests verify that a frame completes while all
background workers are blocked, and that background work still completes while
the frame worker is blocked. The test also verifies that queued background work
drains once the blockers are released. The headless workflow runs these tests
through strict Clippy without compiling a native window.

## Limits and next measurements

This removes background FIFO contention; it does not preempt a running decode,
bypass locks held on the same reader/GPU, or cancel an obsolete foreground frame.
`MonitorPane::busy` still coalesces new requests until the current one completes.
Frame work remains serial, and background CPU contention still exists. No claim
of measured Windows ARM speedup or superiority over either other editor is made.

Next: measure click-to-first-frame and play-to-first-moving-frame for cold/repeated
selection, no-effects/effects, H.264/HEVC and proxy/original, recording decoder
backend, queue delay, decode/composition and UI delivery separately. Then choose
between better stale-request handling, prewarming/reuse and hardware policy from
those measurements. Encrypted draft interoperability is a separate work item.
