# Settings layout polish

Selective layout adaptation of Concat `2fe7e68b`: every settings page has a
10 px top inset, including Clovercut's additional libraries page. Interface
scale leads desktop Appearance, using the existing shared DialogSection header
choice control. Existing scale values, theme binding, callback, persistence and
phone visibility are retained. Existing custom playhead colour handling remains.

Validation: full application Slint source generation completed under a 4 GiB
address-space ceiling. Strict locale inventory passed for all 25 locales.
No new strings, preference fields or callbacks are introduced. This does not
claim native executable compilation or visual qualification on a physical display.
The existing headless UI workflow runs on the UI change; no duplicative test
fixture is added for a layout-only adjustment.
