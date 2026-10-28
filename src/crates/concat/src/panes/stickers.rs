// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Session-only GIPHY browsing and animated originals adopted by a project.

use crate::{i18n::t, panes::Msg, studio::Studio};
use concat_host::{AppDirs, giphy};
use concat_project::Command;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// Messages deliberately print no arguments: keys and signed media URLs must
/// never enter the activity log, even with debug logging enabled.
pub enum StickersMsg {
    Key(String),
    Search(String),
    Mode(i32),
    Favourite(String, bool),
    Page(bool),
    Source(String),
    Preview(String),
    Place(String, Option<String>, f64),
    Loaded(u64, Result<giphy::Page, String>),
    Thumbnail(u64, String, concat_core::frame::Frame),
    Fetched(
        u64,
        u64,
        String,
        Option<u64>,
        Option<String>,
        f64,
        Result<PathBuf, String>,
    ),
    Adopted(
        u64,
        u64,
        String,
        String,
        Option<String>,
        f64,
        Box<Result<concat_host::media::MediaSummary, String>>,
    ),
}
impl std::fmt::Debug for StickersMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Key(_) => "Key",
            Self::Search(_) => "Search",
            Self::Mode(_) => "Mode",
            Self::Favourite(..) => "Favourite",
            Self::Page(_) => "Page",
            Self::Source(_) => "Source",
            Self::Preview(_) => "Preview",
            Self::Place(..) => "Place",
            Self::Loaded(..) => "Loaded",
            Self::Thumbnail(..) => "Thumbnail",
            Self::Fetched(..) => "Fetched",
            Self::Adopted(..) => "Adopted",
        })
    }
}

/// State belongs to this window; no search history or media catalogue is persisted.
pub struct StickersPane {
    dirs: AppDirs,
    key: String,
    scratch: Option<Arc<giphy::Scratch>>,
    generation: Arc<AtomicU64>,
    results: Vec<giphy::Sticker>,
    known: HashMap<String, giphy::Sticker>,
    pub mode: i32,
    thumbnails: HashMap<String, slint::Image>,
    pub query: String,
    pub page: usize,
    pub more: bool,
    pub loading: bool,
    pub pending: String,
    pending_request: u64,
    pending_epoch: u64,
    pending_timeline: String,
    pub error: String,
}
impl Default for StickersPane {
    fn default() -> Self {
        Self {
            dirs: AppDirs {
                config: PathBuf::new(),
                data: PathBuf::new(),
            },
            key: String::new(),
            scratch: None,
            generation: Arc::new(AtomicU64::new(0)),
            results: Vec::new(),
            known: HashMap::new(),
            mode: 0,
            thumbnails: HashMap::new(),
            query: String::new(),
            page: 0,
            more: false,
            loading: false,
            pending: String::new(),
            pending_request: 0,
            pending_epoch: 0,
            pending_timeline: String::new(),
            error: String::new(),
        }
    }
}
impl StickersPane {
    /// Read only local configuration. Nothing goes online at startup.
    pub fn open(dirs: &AppDirs) -> Self {
        Self {
            dirs: dirs.clone(),
            key: giphy::load_key(dirs),
            ..Self::default()
        }
    }
    /// Whether a key has been configured; its value is never published to Slint.
    pub fn configured(&self) -> bool {
        !self.key.is_empty()
    }
    /// Find a public sticker by its opaque provider identifier.
    pub fn get(&self, id: &str) -> Option<&giphy::Sticker> {
        self.results
            .iter()
            .find(|item| item.id == id)
            .or_else(|| self.known.get(id))
    }
    /// Stop browsing when this optional integration is disabled.
    pub fn suspend(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.loading = false;
        self.pending.clear();
        self.pending_request = self.pending_request.wrapping_add(1);
        self.results.clear();
        self.known.clear();
        self.thumbnails.clear();
        self.error.clear();
    }
    /// Cards reuse decoded images rather than doing disk or network work at paint time.
    pub fn rows(&self, prefs: &crate::prefs::Preferences) -> Vec<crate::ui::StickerCardData> {
        let query = self.query.to_lowercase();
        self.results
            .iter()
            .filter(|item| {
                self.mode == 0
                    || query.is_empty()
                    || item.title.to_lowercase().contains(&query)
                    || item.author.to_lowercase().contains(&query)
            })
            .filter(|item| {
                self.mode != 1 || prefs.favourites.contains(&format!("giphy:{}", item.id))
            })
            .map(|item| crate::ui::StickerCardData {
                id: item.id.as_str().into(),
                title: item.title.as_str().into(),
                author: item.author.as_str().into(),
                favourite: prefs.favourites.contains(&format!("giphy:{}", item.id)),
                thumbnail: self.thumbnails.get(&item.id).cloned().unwrap_or_default(),
            })
            .collect()
    }
    fn search(&mut self, page: usize, prefs: &crate::prefs::Preferences) {
        if self.loading || !self.configured() {
            return;
        }
        self.loading = true;
        self.results.clear();
        self.thumbnails.clear();
        self.more = false;
        self.error.clear();
        self.page = page;
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let key = self.key.clone();
        let query = self.query.clone();
        let personal = self.mode > 0;
        let mut saved: Vec<String> = if self.mode == 1 {
            prefs.favourites.clone()
        } else {
            prefs
                .library_history
                .items
                .keys()
                .filter(|id| prefs.library_history.used(id))
                .cloned()
                .collect()
        };
        saved.retain(|id| id.starts_with("giphy:"));
        if self.mode >= 2 {
            saved.sort_by(|a, b| prefs.library_history.compare(self.mode, a, b));
        }
        if self.mode == 3 {
            saved.truncate(50);
        }
        let searching_saved = personal && !query.trim().is_empty();
        let more = saved.len() > (page + 1).saturating_mul(24);
        let ids: Vec<String> = if searching_saved {
            saved
        } else {
            saved
                .into_iter()
                .skip(page.saturating_mul(24))
                .take(24)
                .collect()
        }
        .into_iter()
        .map(|id| id.trim_start_matches("giphy:").to_owned())
        .collect();
        let token = self.generation.clone();
        crate::host::spawn(
            move || {
                if personal {
                    let mut items = Vec::new();
                    for chunk in ids.chunks(50) {
                        if token.load(Ordering::Relaxed) != generation {
                            return Ok(giphy::Page::default());
                        }
                        items.extend(giphy::by_ids(&key, chunk)?.stickers);
                    }
                    let more = if searching_saved {
                        let query = query.to_lowercase();
                        items.retain(|item| {
                            item.title.to_lowercase().contains(&query)
                                || item.author.to_lowercase().contains(&query)
                        });
                        let more = items.len() > (page + 1).saturating_mul(24);
                        items = items
                            .into_iter()
                            .skip(page.saturating_mul(24))
                            .take(24)
                            .collect();
                        more
                    } else {
                        more
                    };
                    Ok(giphy::Page {
                        stickers: items,
                        more,
                    })
                } else {
                    giphy::search(&key, &query, page)
                }
            },
            move |studio, _, _, result| {
                studio.handle(Msg::Stickers(StickersMsg::Loaded(generation, result)))
            },
        );
    }
    fn fetch(
        &mut self,
        id: &str,
        preview: Option<u64>,
        track: Option<String>,
        start: f64,
        timeline: String,
    ) {
        let epoch = crate::host::project_epoch();
        if !self.pending.is_empty() && self.pending_epoch == epoch {
            return;
        }
        let Some(sticker) = self.get(id).cloned() else {
            return;
        };
        if self.scratch.is_none() {
            match giphy::Scratch::new() {
                Ok(scratch) => self.scratch = Some(Arc::new(scratch)),
                Err(error) => {
                    self.error = error;
                    return;
                }
            }
        }
        let scratch = self.scratch.as_ref().unwrap().clone();
        self.pending = id.to_owned();
        self.error.clear();
        let id = id.to_owned();
        self.pending_epoch = epoch;
        self.pending_timeline = timeline;
        self.pending_request = self.pending_request.wrapping_add(1);
        let request = self.pending_request;
        crate::host::spawn(
            move || giphy::original(&sticker, &scratch),
            move |studio, _, _, result| {
                studio.handle(Msg::Stickers(StickersMsg::Fetched(
                    epoch, request, id, preview, track, start, result,
                )))
            },
        );
    }
    /// Apply a browser action or a worker result.
    pub fn update(&mut self, msg: StickersMsg, studio: &mut Studio) {
        if !studio.prefs.giphy_library
            && matches!(
                &msg,
                StickersMsg::Search(_)
                    | StickersMsg::Mode(_)
                    | StickersMsg::Favourite(..)
                    | StickersMsg::Page(_)
                    | StickersMsg::Preview(_)
                    | StickersMsg::Place(..)
                    | StickersMsg::Fetched(..)
                    | StickersMsg::Adopted(..)
            )
        {
            self.pending.clear();
            return;
        }
        match msg {
            StickersMsg::Key(key) if key.trim().is_empty() => {
                self.error = t("stickers.keyPlaceholder")
            }
            StickersMsg::Key(key) => match giphy::save_key(&self.dirs, &key) {
                Ok(()) => {
                    self.key = key.trim().to_owned();
                    self.error.clear();
                    self.more = false;
                    self.page = 0;
                    self.loading = false;
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    self.results.clear();
                    self.thumbnails.clear();
                }
                Err(error) => self.error = error,
            },
            StickersMsg::Mode(mode) => {
                self.mode = mode.clamp(0, 3);
                self.query.clear();
                self.loading = false;
                self.search(0, &studio.prefs);
            }
            StickersMsg::Favourite(id, on) => {
                studio.library_favourite(&format!("giphy:{id}"), on);
            }
            StickersMsg::Search(query) => {
                if !self.loading {
                    self.query = query.chars().take(50).collect();
                    self.search(0, &studio.prefs);
                }
            }
            StickersMsg::Page(next) => {
                if !self.loading && (!next || self.more) {
                    self.search(
                        if next {
                            self.page + 1
                        } else {
                            self.page.saturating_sub(1)
                        },
                        &studio.prefs,
                    );
                }
            }
            StickersMsg::Source(id) => {
                if let Some(sticker) = self.get(&id) {
                    let _ = crate::platform::open_link(&sticker.source);
                }
            }
            StickersMsg::Preview(id) => {
                if self.pending.is_empty() || self.pending_epoch != crate::host::project_epoch() {
                    let request = studio.next_preview_request();
                    self.fetch(
                        &id,
                        Some(request),
                        None,
                        0.0,
                        studio.project().active_timeline_id.clone(),
                    );
                }
            }
            StickersMsg::Place(id, track, start) => self.fetch(
                &id,
                None,
                track,
                start,
                studio.project().active_timeline_id.clone(),
            ),
            StickersMsg::Loaded(generation, result) => {
                if generation != self.generation.load(Ordering::Relaxed) {
                    return;
                }
                self.loading = false;
                match result {
                    Err(error) => {
                        self.error = error;
                        self.more = false;
                    }
                    Ok(page) => {
                        self.more = page.more;
                        for item in &page.stickers {
                            self.known.insert(item.id.clone(), item.clone());
                        }
                        self.results = page.stickers;
                        self.thumbnails.clear();
                        let items = self.results.clone();
                        let token = self.generation.clone();
                        crate::host::spawn(
                            move || {
                                for item in items {
                                    if generation != token.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    if let Ok(frame) = giphy::thumbnail(&item) {
                                        crate::host::on_ui(move |studio, _, _| {
                                            studio.handle(Msg::Stickers(StickersMsg::Thumbnail(
                                                generation, item.id, frame,
                                            )))
                                        });
                                    }
                                }
                            },
                            |_, _, _, ()| {},
                        );
                    }
                }
            }
            StickersMsg::Thumbnail(generation, id, frame) => {
                if generation == self.generation.load(Ordering::Relaxed) {
                    self.thumbnails.insert(id, crate::host::image_of(&frame));
                }
            }
            StickersMsg::Fetched(epoch, request, id, preview, track, start, result) => {
                if request != self.pending_request {
                    return;
                }
                self.pending.clear();
                if epoch != crate::host::project_epoch()
                    || self.pending_timeline != studio.project().active_timeline_id
                {
                    return;
                }
                match result {
                    Err(error) => {
                        self.error = error;
                    }
                    Ok(path) => {
                        let Some(sticker) = self.get(&id).cloned() else {
                            return;
                        };
                        if let Some(request) = preview {
                            if request == studio.source_revision {
                                studio.preview_file(&format!("giphy:{id}"), &sticker.title, &path);
                            }
                        } else {
                            let Some(project) =
                                studio.session.as_ref().map(|s| PathBuf::from(s.path()))
                            else {
                                self.error = t("sounds.openProjectFirst");
                                return;
                            };
                            self.pending = id.clone();
                            let name = sticker.title.clone();
                            crate::host::spawn(
                                move || giphy::adopt(&sticker, &path, &project),
                                move |studio, _, _, result| {
                                    studio.handle(Msg::Stickers(StickersMsg::Adopted(
                                        epoch,
                                        request,
                                        id,
                                        name,
                                        track,
                                        start,
                                        Box::new(result),
                                    )))
                                },
                            );
                        }
                    }
                }
            }
            StickersMsg::Adopted(epoch, request, sticker_id, name, track, start, result) => {
                if request != self.pending_request {
                    return;
                }
                self.pending.clear();
                if epoch != crate::host::project_epoch()
                    || self.pending_timeline != studio.project().active_timeline_id
                {
                    return;
                }
                match *result {
                    Err(error) => self.error = error,
                    Ok(summary) => {
                        studio.close_source();
                        let mut item = summary.to_new_media();
                        item.name = name;
                        let Some(media_id) = studio.apply(Command::AddMedia { item }) else {
                            return;
                        };
                        let Some(track_id) = track.or_else(|| {
                            studio.apply(Command::AddTrack {
                                bottom: false,
                                kind: Some(concat_project::model::TrackKind::Sticker),
                                insert_at: None,
                            })
                        }) else {
                            return;
                        };
                        if let Some(id) = studio.apply(Command::AddClip {
                            media_id,
                            track_id,
                            start,
                            ripple: false,
                        }) {
                            studio.selection = vec![id];
                            studio.record_library_use(&format!("giphy:{sticker_id}"));
                            studio.clip_set(crate::ui::ClipField::Scale, 0.35);
                            studio.flush_commit();
                        }
                    }
                }
            }
        }
    }
}
