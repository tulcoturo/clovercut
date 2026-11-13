# Concat review for Clovercut 0.2.5

Frozen upstream: `6584d34971ae5e168f12cb475bdf58055b67d780`
(Concat 0.2.11). This review extends the previous reviews through
`9fcd218b`, `344e7d08` and `0ef1e125`; it does not use ancestry alone to
judge whether a change was adapted into the fork.

## New upstream commits

| Commit | Decision and evidence |
| --- | --- |
| `51699741` | Adopt the consolidation fixture's binary frame numbers. Compare every decoded source-frame number and retain checks for gaps, duration, following-clip position and multiple frame rates. Clovercut's explicit software encoder option is retained. |
| `3f9798c6` | Already covered by `22349a88`: the font's actual shaped colour glyphs determine the fallback expectation. This additionally verifies that outline fallback responds to styling and colour pictures retain their own colours. Linux and Windows passed. |
| `798a57b1` | Formatting only. The fork's pinned formatter already accepts the corresponding Android, log and recovery-ladder sources. |
| `de4f94bf` | All five messages already translated in all 26 Clovercut locales, including the fork's additional locale. Keep Clovercut branding and existing translations. |
| `6584d349` | Upstream version marker. Clovercut independently advances from 0.2.4 to 0.2.5. |

## Earlier implementation review

Rechecked the earlier inventory against current code rather than treating the
old audit's adoption intentions as proof. Speed-curve normalization, step-cut
behavior, grouped inspector selection, single-step duplicate undo, mask stacks
and MCP schemas, animated clip-relative effects, transition shaders, asynchronous
title painting, colour emoji caching, glance decoding, structured activity logs,
frame-grid consolidation and the final-frame/export-progress fixes are present.
The sixteen-commit follow-up also supplies the asynchronous readback/YUV encoder
prerequisite that the earlier implementation had missed; see the November 12
audit for adaptations and platform limits.

Differences intentionally retained include Clovercut's typed lanes and linkage
defaults, stable saved look identifiers, subject/face effects, title/caption
features, audio implementation, atomic cancellation-aware export, branding and
distribution. Upstream contribution policy, sponsor copy and upstream release
numbering are not product backlog. The smaller duplicate animation pickers are
removed; legacy saved fields still render and undo restores both representations.

No additional product feature or behavior fix was found missing in the reviewed
range. The new consolidation fixture is included in the full release checks.
Release publication requires native CI and desktop regressions, headless UI and
document checks, Linux packaging checks, GPU effects on Linux and Windows, and
all desktop package builds to succeed. Physical Android/Intel Mac verification
is not implied by hosted Linux checks.
