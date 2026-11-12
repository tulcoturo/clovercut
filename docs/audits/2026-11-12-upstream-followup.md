# Concat follow-up through 0ef1e125

This batch follows the integration through Concat `344e7d08`. It adapts the
following sixteen upstream commits to Clovercut rather than replacing fork
implementations or changing saved project identifiers.

| Commit | Integrated behavior |
| --- | --- |
| `5343a34d` | Whisper uses Metal only on supported Apple GPU families; logs the chosen path. |
| `c491fc75` | Recent-project actions remain under the card's touch area and accept pointer clicks. |
| `9c8afc06` | Modal sheets leave the custom window controls reachable. |
| `4e988705` | Static images and titles are held instead of decoded and uploaded on every frame. |
| `6c4bb32b` | Export timing records the adapter, encoder, decode paths and stage costs, including failures and cancellation. |
| `7a0ca6fc` | Android shares current/previous logs and identifies abnormal previous process exits. |
| `d27addbd` | Android includes native-crash signal, thread, abort reason and stack details when available. |
| `7d71d2be` | Export logs record the request, stages, progress milestones and final outcome. |
| `8ca80f4a` | iOS startup checks strip terminal colors before checking the success message. |
| `91729332` | Clips cut between frames can be consolidated using the new endpoint quantisation. |
| `f353a71b` | Emoji tests distinguish joined pictures from fallback letters using glyph counts and actual fill behavior. |
| `0053e4b6` | Failure annotations use UTF-8 even with a Windows cp1252 console. |
| `4e76edc2` | GPU-loss diagnostics include adapter/driver/API reasons and the frame's composition. |
| `4c83a44c` | The CPU compositor is available in production and renders independent row bands in parallel. |
| `c26d466c` | Preview recovery releases dead devices, bounds retries and eventually falls back to CPU. |
| `0ef1e125` | The monitor reports CPU fallback once per run, with translated text. |

## Required adaptations

The export timing changes depended on the previously missing `e938cfdf`
pipeline: bounded asynchronous GPU readback, encoder-native I420/NV12 conversion,
and a bounded encoder worker queue. That prerequisite is included. Animated
Clovercut title sources still evaluate each frame; only actual stills are held.
Queued treatments preserve Clovercut's premultiplied canvas composition.

Clovercut retains cancellation-aware muxing, complete final frame counts,
source-relative effect clocks, masks and subject/face processing, range-export
origins and its macOS window-only software-rendering setting. CPU preview
fallback does not silently substitute a degraded export or API frame: strict
requests still fail when effects/HDR cannot be reproduced.

Android confirmation handling is adapted to the existing monolithic controller
and four-field confirmation model. Whisper capability selection is adapted to
Clovercut's model cache. Constructors and preview calls in imported tests use
the fork's explicit canvas APIs. The new messages are translated across all
26 registered locales.

## One animation inspector

The three-preset VideoMotion and CaptionMotion panels are removed. Video,
images, titles and captions use the existing Animate panel with fourteen
entrance/exit motions and nine loops. Caption collection scope remains
available in that panel.

Saved legacy animation fields and rendering remain supported. Replacing an
entrance or exit clears only that corresponding legacy phase; the other phase
is preserved. The new motion and legacy-field change are committed together,
so undo restores both. Untouched projects are not automatically migrated.

## Validation

Local checks cover core/project/text tests, strict renderer/media/export linting,
the actual renderer's GPU/CPU regressions, recent-project pointer interaction,
the complete Slint source tree, locale/model/card inventories and UTF-8
annotations. The full native application is compiled and tested on CI.

The effects workflow additionally exercises queued RGBA/YUV readback, loss
reporting, repeated software-adapter frames and the recovery ladder on Vulkan
and Windows WARP. Platform-specific behavior still requires its corresponding
platform; Linux checks do not establish Intel Mac or Android device results.

The manual CI `tests_only` mode resumes workspace tests, documentation examples,
gRPC checks and optional desktop journeys after a separately recorded lint pass.
It does not replace release validation: the default and workflow-call paths
still lint all production crates. The still-image regression identifies its
fixture sources by media identity because Clovercut reserves internal lanes
for transitions.
