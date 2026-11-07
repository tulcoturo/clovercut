# Named playhead colours

Adapt the playhead palette portion of Concat `2fe7e68b`. Appearance offers the
same six named colour swatches as the accent plus a custom picker. Named colours
follow the active light/dark palette; a custom colour remains fixed. The first
swatch restores the default and clears the saved override.

Preferences retain the existing playhead field. Names are stored in lowercase;
matching ignores case and surrounding whitespace, and unknown names fall back
to the default. Existing hex values remain accepted by the unchanged colour
parser, including unprefixed forms and the supported short/alpha variants.
The accent's existing brand/default logic is unchanged. The now-unused
settings.default locale key is removed; no new translated strings are needed.

Validation: the real storage/preference suite passed 52 tests and strict isolated
Clippy. The existing home UI fixture now exports the real Theme and verifies all
six named choices, custom colour and reset in both palettes, alongside existing
home interactions; strict fixture Clippy and the run passed. The existing colour
parser tests already cover prefixed/unprefixed, short and alpha hex variants.
Full Slint source generation and strict locale inventory are checked separately.
Native controller compilation and physical-display visual qualification remain
separate gates; the fixture does not simulate settings swatch clicks.
Full application Slint source generation passed under a 4 GiB ceiling, and strict
locale inventory passed for all 25 locales (1115 messages on this branch).

After merging the qualified workspace-persistence main, the combined storage
suite passed 54 tests plus strict isolated Clippy; strict 25-locale inventory and
full Slint source generation also passed. The current native run still targets
the original palette implementation commit, not this combined merge.

## Completed native qualification — 6 November

[CI 42277505050](https://github.com/tulcoturo/clovercut/actions/runs/42277505050)
completed successfully on `4305da9bab0caceb083329f039daed923459d63e`: strict
workspace/all-target and gRPC Clippy, 1345 ordinary tests, 30 gRPC tests and all
14 selected virtual-desktop tests passed with no skipped cases in these reports.
The downloaded JUnit reports were inspected independently. This run predates the
workspace merge; the combined branch checks above remain the evidence for that
combination. PR #30 was merged as `6b77c29231fd1d0e88b061530a39bb851cb45571`.
Physical settings-swatch pointer/display qualification remains open.
