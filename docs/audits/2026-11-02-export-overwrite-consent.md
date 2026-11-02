# Export overwrite destination consent

## Source finding

`Confirm::ReplaceExport` stores the output path shown to the person, but `Studio::confirm_accepted` discarded it and sent an unscoped `Overwrite` message. `ExportPane::begin` then recomputed the output path and skipped its existence confirmation. A change to the form's name or folder while that answer was pending could therefore reuse consent for another destination.

This is a controller-state finding. It does not establish that an ordinary mouse/keyboard interaction can edit behind the modal or reproduce data loss on a user's machine.

## Correction

Carry the confirmed path through `ExportMsg::Overwrite`. Existing files require confirmation unless the current computed destination exactly matches that path. A changed destination that already exists opens its own confirmation, without starting a worker. New destinations retain the ordinary no-overwrite flow. Active-render guards and atomic export publication remain unchanged.

## Validation

Extend the existing native controller journey rather than add a duplicate integration test. It creates distinct existing files, requests consent for one, changes either the name or the folder, accepts the stale confirmation, and checks that the new target requires a new confirmation, no worker starts and both files retain their bytes. It also performs the actual final export over an existing sentinel file after accepting its matching confirmation; the parent independently checks the resulting encoded output.

Local formatting and diff checks passed. Native compilation, strict Clippy and the graphical controller journey remain pending CI. No new release is implied by this follow-up.
