# Enhance progress presentation

Selective adaptation of Concat `2b29732e`: Enhance now shows the existing shared
Progress bar beneath its translated stage/percentage label. Both the percentage
and bar read the same clamped `shown` value, so the label cannot claim progress
outside the bar's range. Existing fetching/enhancing messages, progress source,
cancellation and preview/run controls are retained. No new strings are required.

The frame-count correction in that upstream commit is already equivalent to
Clovercut's shared enhance_plan module: requests and estimates use the actual
source span and margins, including retiming and enhanced-copy offsets. Its four
existing standalone tests passed under strict Clippy; no duplicate estimate
implementation or new tests were introduced.

The 2K output cap from `d75331bc` is not adopted: Clovercut currently supports
up to a 4096-pixel longest side. Fractional scaling, comparison-slider and
estimate-row presentation remain separate potential adaptations.

Validation: full Slint application source generation passed under a 4 GiB
address-space ceiling, strict inventory passed for all 25 locales, and diff
whitespace checks passed. This is a UI presentation change; no inference-quality,
model throughput or physical-display visual qualification is claimed.
