# Concat main comparison — 16 November 2026

Compared current Clovercut integration source `a00e1cab` (the published 0.2.6 application code plus CI changes) with upstream `a7cda105`, dated 16 November. This compares development commits, not release tags. Previous reviewed upstream tip: `6584d349`.

## Result

26 new commits: 12 functional changes pending or partially present, one group-drag fix with an equivalent approach already present, one dependent translation update, eight CI commits and four repository/docs/branding commits. Detailed source observations and all 26 hashes are in [the JSON inventory](data/2026-11-16-upstream-delta.json).

The new pending functional changes cover project unknown-data preservation, decimal-comma FFmpeg filters, VAD dropping speech, export setup/title resolution, missing-media preview messaging, text inset/wrapping, notifications, startup/previous-run logging and Android jobs/encoding.

Priority: project data preservation (`942194dc`), numeric locale (`577e8532`), transcription (`ff9f7e74`), export setup (`8b497918`), then UI/log fixes. Preserve the fork's typed tracks, masks, stable IDs, cancellation and encoder fallbacks when adapting.

`40f13a6a` must not be counted blindly as missing: Clovercut already records drag starts as f64 and applies a shared clamped f64 shift. Source-level equivalence does not substitute for running the upstream regression scenario.

## Older backlog and branch distinction

Git ancestry reports 414 upstream-only commits (388 previously inventoried plus 26 new). These include changes already ported with different hashes, merges and repository-only changes. This is not a count of missing features. The original 388-entry ledger still contains 258 behavior-verification entries and 23 adapted-candidate entries; those statuses were not reconciled after all subsequent ports. Do not infer 258 missing features or full parity from that ledger.

Previously identified native AAC/backend changes and the settings page remain review items; this delta audit does not declare them implemented or establish their complete scope.

Remote `origin/main` still points at `a00f6926`; the latest tested changes live on `integration/upstream-nov12` and release 0.2.6. Using only origin/main would overstate the application's backlog. No branches or release tags were changed by this audit.
