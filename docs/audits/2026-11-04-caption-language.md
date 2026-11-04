# Spoken language for captions — 4 November 2026

Adapt the language-choice portion of
[Concat e2b20c27](https://github.com/jub0t/Concat/commit/e2b20c2712be708eedb4bc86d146f1301ee3d3ba).
The Silero speech concatenation, timestamp remapping and hallucination filtering
from that commit remain separate work. Existing greedy decoding, playback thread
headroom and VAD timing behavior are unchanged.

Both the caption sheet and Clovercut's inline desktop panel offer Auto-detect
and Whisper's languages by their own names. English-only models show English
alone. The selected code is saved in preferences and restored at startup before
the sheet opens. Regional tags such as `pt-BR` normalize to `pt`; unknown or absent
codes use automatic detection. An English-only context always receives `en`.
Changing model does not discard the remembered multilingual choice.

Capture the language when starting either a selected-clip transcription or the
whole-timeline batch, so every job uses that run's choice. Running controls are
disabled and the controller ignores language changes while running. Existing
request/timeline ownership and cancellation remain intact. Old JSON requests omit
`language` and deserialize with automatic detection.

Validation:

- The pure language policy passes Clippy with warnings denied, including missing
  documentation, and tests normalization, automatic/explicit/English-only choices
  and unique language identifiers without native speech libraries.
- Simulated pointer/keyboard checks use the actual caption sheet and inline panel
  and confirm selection plus the English-only lock.
- Full Slint application code generation passed under a 4 GiB address-space cap.
- All 1,113 messages in 25 locales pass the strict inventory; nine locale-tool
  tests and the model inventory pass.
- Native tests check the identifiers against linked Whisper, compatibility of old
  requests and row mapping. The existing complete editor journey now checks
  startup restoration and persistence from the inline controller. These native
  checks remain CI qualification; no downloaded-model transcription or accuracy
  improvement is claimed from the local UI and policy checks.

## Native compilation follow-up

The first native CI attempt (`41880797565`) found an E0451 privacy error in the
startup struct-update expression. Startup now uses `CaptionsPane::with_saved_language`,
which constructs private pane state inside its owning module. Fields remain
private; the constructor normalizes the saved code using the tested policy.
The branch incorporates main after PR #17 merged (`b9587300`), and requires a
fresh native CI result before integration. The initial failed run does not
qualify the caption controller or the new effect's complete desktop integration.
