# Managing projects and templates

The home screen can display projects as a grid or compact list. Search matches
project names and folder paths; sorting and filtering apply to the whole library
before splitting results into pages of 24. The saved history no longer evicts
projects after 40 entries. Previously evicted entries must be opened again to
return to the history. Removing an entry from this screen does not delete its
project folder.

On the desktop, click the project title in the top bar to rename it. Enter
commits the title; Escape or leaving the field cancels. Empty names are ignored.
The usual save mechanism persists the title and refreshes its recent-project
entry after a successful write. Renaming does not move the project folder or
change media paths.

The editor’s **Templates** library displays saved bundles with their poster,
output format and placeholder count:

- **Edit a copy** asks for a destination folder and opens an independent project.
  Placeholder slots remain editable, and bundled media and fonts are copied.
  A numbered name is chosen if the destination already contains that project.
- **Update from project** replaces the selected template with the current
  project after confirmation, preserving the template’s name.
- **Delete** removes the template bundle after confirmation. Projects previously
  created from it keep their own assets and continue to work.

To reuse a design, edit a copy and replace its slots with the desired media.
To revise a saved design, edit a copy, make the changes, then update the original
entry from the Templates library. Saving a new template still refuses an
existing name; replacement is always an explicit action.

Bundle creation stages all media before publishing the template. Updates stage
new assets and atomically replace the manifest before reclaiming the previous
assets. A library lock coordinates readers and writers across app processes.
Storage tests cover failed saves, updates, independent copies, deletion and
libraries larger than 100 projects. The isolated storage check intentionally
omits native poster extraction; full CI uses the real media backend.
