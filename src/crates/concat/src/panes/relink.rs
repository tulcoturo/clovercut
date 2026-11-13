// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! The missing media dialog: what a project cannot find, and one folder
//! to look for it in.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::AtomicBool};

use concat_project::Command;
use concat_project::model::{MediaKind, MissingMedia};
use slint::VecModel;

use crate::i18n::{t, tf};
use crate::platform;
use crate::studio::Studio;
use crate::ui::{MissingMediaItem, RelinkData};

#[path = "../relink_search.rs"]
mod search;

/// Everything that can happen to the relink dialog.
#[derive(Clone, Debug)]
pub enum RelinkMsg {
    /// A project opened with files it cannot find.
    Show(Vec<MissingMedia>),
    /// The window came back to the front: what the project cannot find
    /// now, which may have changed while it was away.
    Recheck(Vec<MissingMedia>),
    /// Pick a folder and look for every missing file inside it.
    RelinkAll,
    /// The folder the picker answered with: look inside it.
    RelinkIn(PathBuf),
    /// Pick one missing source, including a renamed file.
    Locate(String),
    /// A still-current project-owned file picker answered.
    Located {
        token: Arc<AtomicBool>,
        id: String,
        old_path: String,
        path: PathBuf,
    },
    /// A project-owned worker replied; only the current request may apply it.
    SearchFinished {
        token: Arc<AtomicBool>,
        items: Vec<(String, String)>,
        index: HashMap<String, PathBuf>,
        chosen: Option<(String, PathBuf)>,
    },
    /// Remove a listed missing source and its clips as one undoable edit.
    Remove(String),
    Dismiss,
}

/// The relink dialog's state.
#[derive(Default)]
pub struct RelinkPane {
    pub open: bool,
    pub items: Vec<MissingMedia>,
    /// The media whose dialog the user closed: no dialog mentions them
    /// again until they come back and go missing anew.
    dismissed: HashSet<String>,
    job: Option<search::Job>,
    picker: Option<search::Job>,
}

impl RelinkPane {
    /// Applies one message. The studio is the rest of the window; while
    /// this runs the studio's copy of the pane is a blank it must not read.
    pub fn update(&mut self, msg: RelinkMsg, studio: &mut Studio) {
        match msg {
            RelinkMsg::Show(items) => self.show(items),
            // A return to the front: forget media that is back (losing it
            // again will be news), keep the silence the user asked for on
            // the rest, and open on whatever missing is new to them.
            RelinkMsg::Recheck(missing) => self.recheck(missing),
            // The picker runs with the studio unborrowed and answers as a
            // message of its own; see `host::after_dialog`.
            RelinkMsg::RelinkAll => {
                self.picker = None;
                let title = t("relink.selectFolderContainingMedia");
                crate::host::after_dialog_in_project(
                    move || platform::pick_folder(&title, ""),
                    |studio, _, _, folder| {
                        studio.handle(crate::panes::Msg::Relink(RelinkMsg::RelinkIn(folder)));
                    },
                );
            }
            RelinkMsg::RelinkIn(folder) => self.relink_in(studio, folder),
            RelinkMsg::SearchFinished {
                token,
                items,
                index,
                chosen,
            } => self.finish_relink(studio, token, items, index, chosen),
            RelinkMsg::Locate(id) => self.locate(studio, id),
            RelinkMsg::Located {
                token,
                id,
                old_path,
                path,
            } => self.located(studio, token, id, old_path, path),
            RelinkMsg::Remove(id) => self.remove(studio, &id),
            RelinkMsg::Dismiss => self.dismiss(),
        }
    }

    fn remove(&mut self, studio: &mut Studio, id: &str) {
        if !self.open || studio.session.is_none() {
            return;
        }
        let Some(name) = self
            .items
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.name.clone())
        else {
            return;
        };
        // Cancel the old snapshot before applying an edit. Its queued reply
        // must never relink this source or the remaining dialog items.
        self.job = None;
        self.picker = None;
        studio.apply(Command::RemoveMedia {
            media_id: id.to_owned(),
        });
        // Locked clips or another rejected edit must leave the row available.
        if studio.project().media.iter().any(|item| item.id == id) {
            return;
        }
        self.items.retain(|item| item.id != id);
        self.dismissed.remove(id);
        self.open = !self.items.is_empty();
        studio.notify(&tf("relink.removed", &[&name]), false);
        studio.request_media_art();
        studio.request_preview();
    }

    /// Opens the dialog on a project's missing files, as opening a project
    /// does. A dismissal is about the project it was made in, so one from
    /// an earlier project is forgotten here.
    fn show(&mut self, items: Vec<MissingMedia>) {
        self.job = None;
        self.picker = None;
        self.open = !items.is_empty();
        self.items = items;
        self.dismissed.clear();
    }

    /// Applies a check made while the project is open - on a return to the
    /// front - to the dialog: media that is back is forgotten, dismissed
    /// media stays quiet, and the rest is the dialog's list.
    fn recheck(&mut self, missing: Vec<MissingMedia>) {
        let still: HashSet<&str> = missing.iter().map(|item| item.id.as_str()).collect();
        self.dismissed.retain(|id| still.contains(id.as_str()));
        let items: Vec<_> = missing
            .into_iter()
            .filter(|item| !self.dismissed.contains(&item.id))
            .collect();
        if !self
            .items
            .iter()
            .map(|item| (&item.id, &item.path))
            .eq(items.iter().map(|item| (&item.id, &item.path)))
        {
            self.job = None;
            self.picker = None;
        }
        self.items = items;
        self.open = !self.items.is_empty();
    }

    /// Closes the dialog, and keeps it closed for what it was listing
    /// until those files are back and go missing again.
    fn dismiss(&mut self) {
        self.job = None;
        self.picker = None;
        self.dismissed
            .extend(self.items.iter().map(|item| item.id.clone()));
        self.open = false;
    }

    /// Relinks missing media by searching `folder` (recursively) for files
    /// whose basename matches: each missing item looks for its own filename
    /// inside it. Successful relinks go through the editor as
    /// `UpdateMediaPath`, so undo covers the whole batch.
    fn relink_in(&mut self, studio: &mut Studio, folder: PathBuf) {
        self.picker = None;
        if !self.open || studio.session.is_none() {
            return;
        }
        // Snapshot the missing list now: as relinks land the list shrinks,
        // and we want a stable target for the toast count.
        let items: Vec<(String, String)> = self
            .items
            .iter()
            .map(|m| (m.id.clone(), m.path.clone()))
            .collect();
        let total = items.len();
        if total == 0 {
            self.open = false;
            return;
        }

        let wanted: HashSet<String> = items
            .iter()
            .filter_map(|(_, path)| {
                std::path::Path::new(path)
                    .file_name()?
                    .to_str()
                    .map(str::to_owned)
            })
            .collect();
        let job = search::Job::default();
        let token = job.token();
        let worker_cancel = Arc::clone(&token);
        self.job = Some(job);
        crate::host::spawn_in_project(
            move || search::find_named_files(&folder, &wanted, &worker_cancel),
            move |studio, _, _, index| {
                studio.handle(crate::panes::Msg::Relink(RelinkMsg::SearchFinished {
                    token,
                    items,
                    index,
                    chosen: None,
                }));
            },
        );
    }

    fn finish_relink(
        &mut self,
        studio: &mut Studio,
        token: Arc<AtomicBool>,
        items: Vec<(String, String)>,
        index: HashMap<String, PathBuf>,
        chosen: Option<(String, PathBuf)>,
    ) {
        if !self.job.as_ref().is_some_and(|job| job.accepts(&token)) {
            return;
        }
        self.job = None;
        self.picker = None;
        if studio.session.is_none() || !self.open {
            return;
        }
        let total = items.len();
        let missing: HashMap<_, _> = studio
            .project()
            .missing_media()
            .into_iter()
            .map(|item| (item.id, item.path))
            .collect();
        let proposed = match chosen {
            Some((id, path)) => search::located_replacements(&items, &missing, &index, &id, &path),
            None => search::replacements(&items, &missing, &index),
        };
        let commands: Vec<_> = proposed
            .iter()
            .map(|(id, target)| Command::UpdateMediaPath {
                media_id: id.clone(),
                new_path: target.clone(),
            })
            .collect();

        if !commands.is_empty() {
            studio.apply(Command::Batch { commands });
        }

        // Re-check what is still missing: the dialog updates to the
        // remainder (often empty, in which case it closes). A dismissal
        // only outlives its file while that file is missing - one that is
        // relinked here and lost again is news.
        let remaining = studio.project().missing_media();
        let still: HashSet<&str> = remaining.iter().map(|item| item.id.as_str()).collect();
        let relinked = proposed
            .iter()
            .filter(|(id, target)| {
                !still.contains(id.as_str())
                    && studio
                        .project()
                        .media_by_id(id)
                        .is_some_and(|item| &item.path == target)
            })
            .count();
        self.dismissed.retain(|id| still.contains(id.as_str()));
        if remaining.is_empty() {
            self.open = false;
            self.items.clear();
        } else {
            self.items = remaining;
        }

        studio.notify(&tf("relink.relinkedFiles", &[&relinked, &total]), false);
        studio.request_media_art();
        studio.request_preview();
    }

    /// The picker is project-owned and separately cancellable from a folder
    /// scan. Cancelling the native chooser never leaves a fake busy search.
    fn locate(&mut self, studio: &mut Studio, id: String) {
        if !self.open || studio.session.is_none() {
            return;
        }
        let Some(item) = self.items.iter().find(|item| item.id == id) else {
            return;
        };
        let old_path = item.path.clone();
        let title = tf("relink.locateFile", &[&item.name]);
        self.job = None;
        let job = search::Job::default();
        let token = job.token();
        self.picker = Some(job);
        crate::host::pick_files_in_project(title, None, move |studio, _, _, paths| {
            if let Some(path) = paths.into_iter().next() {
                studio.handle(crate::panes::Msg::Relink(RelinkMsg::Located {
                    token,
                    id,
                    old_path,
                    path,
                }));
            }
        });
    }

    fn located(
        &mut self,
        studio: &mut Studio,
        token: Arc<AtomicBool>,
        id: String,
        old_path: String,
        path: PathBuf,
    ) {
        if !self.open || !self.picker.as_ref().is_some_and(|job| job.accepts(&token)) {
            return;
        }
        self.picker = None;
        if !path.is_file()
            || !studio
                .project()
                .missing_media()
                .iter()
                .any(|item| item.id == id && item.path == old_path)
        {
            return;
        }
        let items: Vec<(String, String)> = self
            .items
            .iter()
            .map(|item| (item.id.clone(), item.path.clone()))
            .collect();
        let wanted: HashSet<String> = items
            .iter()
            .filter(|(held, _)| held != &id)
            .filter_map(|(_, path)| Path::new(path).file_name()?.to_str().map(str::to_owned))
            .collect();
        let folder = path.parent().unwrap_or_else(|| Path::new("")).to_owned();
        let job = search::Job::default();
        let token = job.token();
        let cancel = Arc::clone(&token);
        self.job = Some(job);
        crate::host::spawn_in_project(
            move || search::find_sibling_files(&folder, &wanted, &cancel),
            move |studio, _, _, index| {
                studio.handle(crate::panes::Msg::Relink(RelinkMsg::SearchFinished {
                    token,
                    items,
                    index,
                    chosen: Some((id, path)),
                }));
            },
        );
    }

    /// The dialog as Slint shows it.
    pub fn data(&self, studio: &Studio) -> RelinkData {
        let mut counts = HashMap::<&str, i32>::new();
        if self.open {
            for timeline in &studio.project().timelines {
                for clip in &timeline.clips {
                    let count = counts.entry(&clip.media_id).or_default();
                    *count = count.saturating_add(1);
                }
            }
        }
        RelinkData {
            open: self.open,
            searching: self.job.is_some(),
            items: slint::ModelRc::new(VecModel::from(
                self.items
                    .iter()
                    .map(|item| MissingMediaItem {
                        id: item.id.clone().into(),
                        name: item.name.clone().into(),
                        folder: shorten(
                            &Path::new(&item.path)
                                .parent()
                                .unwrap_or_else(|| Path::new(""))
                                .to_string_lossy(),
                        )
                        .into(),
                        kind: studio
                            .project()
                            .media_by_id(&item.id)
                            .map_or(0, |media| match media.kind {
                                MediaKind::Audio => 1,
                                MediaKind::Image => 2,
                                _ => 0,
                            }),
                        clips: counts.get(item.id.as_str()).copied().unwrap_or(0),
                    })
                    .collect::<Vec<_>>(),
            )),
        }
    }
}

fn shorten(path: &str) -> String {
    let Some(home) = std::env::home_dir() else {
        return path.to_owned();
    };
    let home = home.to_string_lossy();
    match path.strip_prefix(home.as_ref()) {
        Some(rest) if rest.is_empty() || rest.starts_with(std::path::MAIN_SEPARATOR) => {
            format!("~{rest}")
        }
        _ => path.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn missing(id: &str, path: &str) -> MissingMedia {
        MissingMedia {
            id: id.to_owned(),
            name: format!("{id}.mp4"),
            path: path.to_owned(),
        }
    }

    #[test]
    fn a_recheck_opens_the_dialog_on_what_is_missing() {
        let mut pane = RelinkPane::default();
        pane.recheck(vec![missing("m1", "/gone/a.mp4")]);
        assert!(pane.open);
        assert_eq!(pane.items.len(), 1);
        assert_eq!(pane.items[0].id, "m1");
    }

    #[test]
    fn a_dismissed_file_stays_quiet_while_it_is_missing() {
        let mut pane = RelinkPane::default();
        pane.recheck(vec![missing("m1", "/gone/a.mp4")]);
        pane.job = Some(search::Job::default());
        let token = pane.job.as_ref().unwrap().token();
        pane.recheck(vec![missing("m1", "/gone/a.mp4")]);
        assert!(
            pane.job.as_ref().unwrap().accepts(&token),
            "unchanged focus checks must not cancel work"
        );
        pane.dismiss();
        assert!(token.load(std::sync::atomic::Ordering::Relaxed));
        assert!(!pane.open, "dismissing closes the dialog");

        pane.recheck(vec![missing("m1", "/gone/a.mp4")]);
        assert!(!pane.open, "and the same missing file does not reopen it");
        assert!(pane.items.is_empty());
    }

    #[test]
    fn a_file_that_comes_back_and_goes_missing_again_is_news() {
        let mut pane = RelinkPane::default();
        pane.recheck(vec![missing("m1", "/gone/a.mp4")]);
        pane.dismiss();
        // Back on disk: the check finds nothing and forgets the dismissal.
        pane.recheck(Vec::new());
        assert!(!pane.open);
        // Gone a second time: the dialog should be back.
        pane.recheck(vec![missing("m1", "/gone/a.mp4")]);
        assert!(pane.open);
        assert_eq!(pane.items.len(), 1);
    }

    #[test]
    fn a_new_missing_file_opens_the_dialog_beside_a_dismissed_one() {
        let mut pane = RelinkPane::default();
        pane.recheck(vec![missing("m1", "/gone/a.mp4")]);
        pane.dismiss();
        pane.recheck(vec![
            missing("m1", "/gone/a.mp4"),
            missing("m2", "/gone/b.mp4"),
        ]);
        assert!(pane.open);
        assert_eq!(pane.items.len(), 1);
        assert_eq!(pane.items[0].id, "m2", "only the news is listed");
    }

    #[test]
    fn a_recheck_that_finds_nothing_closes_the_dialog() {
        let mut pane = RelinkPane::default();
        pane.recheck(vec![missing("m1", "/gone/a.mp4")]);
        pane.recheck(Vec::new());
        assert!(!pane.open);
        assert!(pane.items.is_empty());
    }

    #[test]
    fn opening_a_project_forgets_an_earlier_projects_dismissals() {
        let mut pane = RelinkPane::default();
        pane.recheck(vec![missing("m1", "/gone/a.mp4")]);
        pane.dismiss();
        // Another project opens with the same id missing: Show is the
        // project-open path, and it starts the dialog afresh.
        pane.show(vec![missing("m1", "/gone/a.mp4")]);
        assert!(pane.open);
        assert_eq!(pane.items.len(), 1);
    }
}
