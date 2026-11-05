# Empty-home guide link

Adapt Concat `b27a3943`: the empty project's first-edit guide button now shows
an external-link arrow, consistent with the Learn cards. Its pill reserves
space for the icon, retaining the existing action, label, disabled state and
browser callback. Other pill buttons keep their current appearance.

Validation uses the existing isolated home UI fixture and its strict Clippy
check. No new strings, preferences or test fixtures are needed for this visual
adjustment. This does not qualify the full native desktop executable or the
external guide's contents.

The existing `check_home_ui.py --offline --suite home` run passed strict fixture
Clippy and simulated home interactions against the actual StartScreen source.
