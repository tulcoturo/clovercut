// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! The source preview: a file from the library seen and heard on its own,
//! before it is anywhere near the timeline (#197).
//!
//! A click on a video or a still in the bin shows it on the monitor,
//! paused at its start, with a chip that names it; Play and Space play it,
//! the monitor's timecode moves through it. A click on a sound plays it at
//! once, with a line moving across its card; a second click pauses it.
//! The timeline is not touched: the monitor and the speakers are lent a
//! scratch copy of the project holding the one clip, and given back the
//! moment the person clicks the timeline or the chip's cross.

use std::sync::Arc;

use concat_project::Command;
use concat_project::commands::{self, IdMint};
use concat_project::model::{MediaKind, Project, Timeline};

use crate::studio::Studio;
use crate::ui::SourcePreview;
use slint::{ComponentHandle, SharedString};

/// The file lent to the monitor and the speakers.
pub struct Source {
    pub media_id: String,
    pub name: String,
    pub kind: MediaKind,
    /// The bin's row for the file, for its card to show the playing line;
    /// -1 for a sound from the Audio page.
    pub row: i32,
    /// The Audio page's card for the file, or empty for one in the bin.
    pub card: String,
    /// The scratch project flattened, as the monitor draws it.
    pub clips: Arc<Vec<concat_export::ExportClip>>,
    pub duration: f64,
    pub time: f64,
    pub playing: bool,
    /// Follows the speakers' clock while it plays.
    timer: slint::Timer,
}

/// The project as it is, with its active timeline holding only `media_id`
/// from zero: what the source preview draws and plays.
fn scratch(project: &Project, media_id: &str) -> Option<Project> {
    let mut scratch = project.clone();
    let timeline = scratch.active_mut();
    timeline.clips.clear();
    // The library borrows a lane, never its mute/visibility switches.
    let track = timeline.tracks.first_mut()?;
    track.visible = true;
    track.muted = false;
    let track_id = track.id.clone();
    commands::apply(
        &mut scratch,
        &mut IdMint::default(),
        Command::AddClip {
            media_id: media_id.to_owned(),
            track_id,
            start: 0.0,
            ripple: false,
        },
    )
    .ok()?;
    Some(scratch)
}

/// The picture actually requested by the monitor. A library source owns
/// it independently of the timeline, and the preview axis can show a
/// different instant from the playhead.
pub(crate) fn picture_name<'a>(
    timeline: &'a Timeline,
    time: f64,
    source: Option<(&'a str, MediaKind)>,
) -> Option<&'a str> {
    if let Some((name, kind)) = source {
        return (kind != MediaKind::Audio).then_some(name);
    }
    timeline
        .tracks
        .iter()
        .rev()
        .filter(|track| track.visible)
        .find_map(|track| {
            timeline.clips.iter().find(|clip| {
                clip.track_id == track.id
                    && (clip.kind.is_visual()
                        || matches!(
                            clip.kind,
                            concat_project::model::ClipKind::Text
                                | concat_project::model::ClipKind::Shape
                        ))
                    && clip.start <= time
                    && time < clip.start + clip.duration
            })
        })
        .map(|clip| clip.name.as_str())
}

impl Studio {
    /// Invalidate pending previews when a newer preview or timeline action wins.
    pub fn next_preview_request(&mut self) -> u64 {
        self.source_revision = self.source_revision.wrapping_add(1);
        self.source_revision
    }

    /// A card was clicked. A sound plays, or pauses when it is the one
    /// playing; a picture goes on the monitor, paused at its start.
    pub fn preview_media(&mut self, media_id: &str, row: i32) {
        self.next_preview_request();
        if let Some(source) = self.source.as_ref()
            && source.media_id == media_id
            && source.card.is_empty()
        {
            if source.kind == MediaKind::Audio {
                self.source_toggle();
            }
            return;
        }
        let Some(media) = self.project().media_by_id(media_id).cloned() else {
            return;
        };
        let Some(project_dir) = self
            .session
            .as_ref()
            .map(|session| std::path::PathBuf::from(session.path()))
        else {
            return;
        };
        let Some(scratch) = scratch(self.project(), media_id) else {
            return;
        };
        // The timeline stops: the speakers and the monitor are the
        // source's now.
        self.pause();
        if let Some(old) = self.source.take() {
            old.timer.stop();
        }
        let clips = Arc::new(concat_export::flatten::flatten_timeline_in(
            &scratch,
            None,
            Some(&project_dir),
        ));
        let duration = scratch
            .active()
            .clips
            .first()
            .map_or(0.0, |clip| clip.duration);
        self.host
            .playback
            .set_clips(project_dir, crate::studio::audio_specs(&clips));
        self.host.playback.seek(0.0);
        self.source = Some(Source {
            media_id: media_id.to_owned(),
            name: media.name.clone(),
            kind: media.kind,
            row,
            card: String::new(),
            clips,
            duration,
            time: 0.0,
            playing: false,
            timer: slint::Timer::default(),
        });
        if media.kind == MediaKind::Audio {
            self.source_toggle();
        }
        self.request_preview();
    }

    /// A card on the Audio page was clicked: its sound plays through the
    /// speakers as a sound from the bin does, without being in the bin, or
    /// pauses when it is the one playing. `card` names it for its card's
    /// playing line.
    pub fn preview_file(&mut self, card: &str, name: &str, path: &std::path::Path) {
        self.next_preview_request();
        if let Some(source) = self.source.as_ref()
            && source.card == card
        {
            self.source_toggle();
            return;
        }
        let Some(project_dir) = self
            .session
            .as_ref()
            .map(|session| std::path::PathBuf::from(session.path()))
        else {
            return;
        };
        let summary = match concat_host::media::probe(&path.to_string_lossy()) {
            Ok(summary) => summary,
            Err(error) => {
                self.notify(&crate::host::probe_error(&error), true);
                return;
            }
        };
        // The scratch project with the file in its bin, under an id no
        // real item has, and the one clip cut from it.
        let mut scratch = self.project().clone();
        scratch.media.retain(|item| item.path != summary.path);
        let mut mint = IdMint::default();
        mint.adopt_project(&scratch);
        let Ok(Some(media_id)) = commands::apply(
            &mut scratch,
            &mut mint,
            Command::AddMedia {
                item: summary.to_new_media(),
            },
        )
        .map(|outcome| outcome.created_id) else {
            return;
        };
        let Some(scratch) = self::scratch(&scratch, &media_id) else {
            return;
        };
        self.pause();
        if let Some(old) = self.source.take() {
            old.timer.stop();
        }
        let clips = Arc::new(concat_export::flatten::flatten_timeline_in(
            &scratch,
            None,
            Some(&project_dir),
        ));
        let duration = scratch
            .active()
            .clips
            .first()
            .map_or(0.0, |clip| clip.duration);
        self.host
            .playback
            .set_clips(project_dir, crate::studio::audio_specs(&clips));
        self.host.playback.seek(0.0);
        self.source = Some(Source {
            media_id,
            name: name.to_owned(),
            kind: MediaKind::Audio,
            row: -1,
            card: card.to_owned(),
            clips,
            duration,
            time: 0.0,
            playing: false,
            timer: slint::Timer::default(),
        });
        self.source_toggle();
        self.request_preview();
    }

    /// Back to the timeline: its picture on the monitor, its sound in the
    /// speakers, the transport where the playhead was.
    pub fn close_source(&mut self) {
        self.next_preview_request();
        let Some(source) = self.source.take() else {
            return;
        };
        source.timer.stop();
        self.host.playback.pause();
        self.sync_audio();
        self.host.playback.seek(self.playhead);
        self.request_preview();
    }

    /// Play or pause the source, from where it stands; from its start
    /// again once it has played to the end.
    pub fn source_toggle(&mut self) {
        let Some(source) = self.source.as_mut() else {
            return;
        };
        if source.playing {
            source.playing = false;
            source.timer.stop();
            self.host.playback.pause();
            return;
        }
        if source.time >= source.duration - 1e-3 {
            source.time = 0.0;
        }
        source.playing = true;
        self.host.playback.play(source.time);
        source.timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(16),
            || {
                crate::host::Shell::with(|shell, app| {
                    let mut studio = shell.studio.borrow_mut();
                    let position = studio.host.playback.position_now();
                    let Some(source) = studio.source.as_mut() else {
                        return;
                    };
                    source.time = position.min(source.duration);
                    let ended = source.time >= source.duration;
                    if ended {
                        source.playing = false;
                        source.timer.stop();
                    }
                    if ended {
                        studio.host.playback.pause();
                    }
                    studio.request_preview();
                    studio.publish_source(&app);
                });
            },
        );
        self.wake_meters();
    }

    /// The monitor's timecode moved through the source.
    pub fn source_seek(&mut self, seconds: f64) {
        let Some(source) = self.source.as_mut() else {
            return;
        };
        source.time = seconds.clamp(0.0, source.duration);
        let time = source.time;
        if source.playing {
            self.host.playback.play(time);
        } else {
            self.host.playback.seek(time);
        }
        self.request_preview();
    }

    /// What the window shows of the source: the chip, the card's line, the
    /// monitor's readout.
    pub fn publish_source(&self, app: &crate::ui::App) {
        let preview = app.global::<SourcePreview>();
        match self.source.as_ref() {
            Some(source) => {
                preview.set_open(true);
                preview.set_name(source.name.as_str().into());
                preview.set_row(source.row);
                preview.set_card(source.card.as_str().into());
                preview.set_time(source.time as f32);
                preview.set_duration(source.duration as f32);
                preview.set_playing(source.playing);
                preview.set_progress(if source.duration > 0.0 {
                    (source.time / source.duration) as f32
                } else {
                    0.0
                });
            }
            None => {
                preview.set_open(false);
                preview.set_row(-1);
                preview.set_card(SharedString::new());
                preview.set_playing(false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use concat_project::model::{Clip, ClipKind, Track};

    #[test]
    fn library_preview_ignores_hidden_and_muted_timeline_tracks() {
        let mut project = Project::default();
        project.active_mut().tracks = vec![Track {
            id: "t".into(),
            visible: false,
            muted: true,
            ..Default::default()
        }];
        project.media.push(concat_project::model::MediaItem {
            id: "m".into(),
            path: "/source.mp4".into(),
            name: "source".into(),
            duration: Some(3.0),
            kind: MediaKind::Video,
            has_audio: true,
            ..Default::default()
        });
        let preview = scratch(&project, "m").expect("source project");
        let clips = concat_export::flatten::flatten_timeline(&preview, None);
        assert_eq!(clips.len(), 1);
        assert!(
            !clips[0].hidden,
            "a hidden timeline lane must not hide the library"
        );
        assert!(
            !clips[0].muted,
            "a muted timeline lane must not silence the library"
        );
        assert!(
            project.active().clips.is_empty(),
            "preview must not place a clip"
        );
        assert!(!project.active().tracks[0].visible);
        assert!(project.active().tracks[0].muted);
    }

    /// Run explicitly with isolated XDG directories and a graphical session.
    #[test]
    #[ignore = "requires a graphical session, FFmpeg and audio output"]
    fn new_project_library_and_timeline_playback() {
        // Slint's backend and the audio/GPU workers own process-lifetime
        // state. Keep their shutdown out of libtest's worker-thread TLS
        // destructors, just as the real app does through `leave`.
        const CHILD: &str = "CLOVERCUT_PLAYBACK_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "source::tests::new_project_library_and_timeline_playback",
                    "--ignored",
                    "--exact",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("playback regression: ok"));
            return;
        }
        use crate::host::Host;
        use crate::panes::monitor::{MonitorMsg, Picture};
        use crate::studio::Models;
        use crate::ui::{App, Editor};
        use concat_host::{media, projects};

        let dir = std::env::temp_dir().join(format!("clovercut-playback-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let video = dir.join("source.mkv");
        let encoded = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=64x36:rate=10:duration=3",
                "-c:v",
                "ffv1",
            ])
            .arg(&video)
            .status()
            .unwrap();
        assert!(encoded.success());
        let gpu = crate::platform::select_backend(|_| {}, || {}, || {}, || {}).unwrap();
        let host = Host::start(gpu).unwrap();
        let mut studio = Studio::new(host);
        let app = App::new().unwrap();
        let models = Models::new();
        let info = projects::create(dir.to_str().unwrap(), "New project", 64, 36, 10, 1).unwrap();
        studio.open_project(info).unwrap();
        let item = media::probe(video.to_str().unwrap())
            .unwrap()
            .to_new_media();
        let id = studio.apply(Command::AddMedia { item }).unwrap();
        studio.preview_media(&id, 0);
        assert!(studio.timeline().clips.is_empty());
        studio.publish(&app, &models);
        assert!(
            app.global::<Editor>().get_has_picture(),
            "an imported source must be visible on an empty timeline"
        );

        // Decode and draw actual frames at separate playback instants.
        let render = |studio: &mut Studio, time: f64| {
            let (clips, settings) = studio.preview_clips(64, 36, time).unwrap();
            let spec = concat_host::preview::FrameSpec {
                canvas: [0, 0, 0, 255],
                time,
                width: 64,
                height: 36,
                moving: true,
                proxy: false,
                color_space: Default::default(),
            };
            let bytes = studio.host.monitor.frame(clips, &settings, spec).unwrap();
            assert!(
                bytes.chunks_exact(4).any(|p| p[0].abs_diff(p[1]) > 40),
                "the video must contain colour rather than black"
            );
            studio.handle(crate::panes::Msg::Monitor(MonitorMsg::Frame(
                Ok(Picture::Pixels(bytes.clone(), 64, 36)),
                spec,
            )));
            bytes
        };
        let first = render(&mut studio, 0.0);
        studio.play_toggle();
        assert!(studio.source.as_ref().unwrap().playing);
        assert!(!studio.playing, "source transport owns playback");
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(studio.host.playback.position_now() > 0.0);
        let later = render(&mut studio, 1.0);
        assert_ne!(first, later, "playback must show changing video frames");
        studio.publish(&app, &models);
        assert!(app.global::<Editor>().get_preview_frame().size().width > 0);

        studio.close_source();
        let track_id = studio.timeline().tracks[0].id.clone();
        studio.apply(Command::AddClip {
            media_id: id.clone(),
            track_id,
            start: 0.0,
            ripple: false,
        });
        studio.play_toggle();
        assert!(studio.playing);
        render(&mut studio, 0.5);
        studio.publish(&app, &models);
        assert!(app.global::<Editor>().get_has_picture());
        studio.pause();
        studio.seek(4.0);
        studio.publish(&app, &models);
        assert!(
            !app.global::<Editor>().get_has_picture(),
            "timeline gaps remain empty"
        );
        studio.preview_media(&id, 0);
        studio.publish(&app, &models);
        assert!(
            app.global::<Editor>().get_has_picture(),
            "a library source stays visible when the timeline playhead is in a gap"
        );
        studio.close_source();
        drop(studio);
        std::fs::remove_dir_all(dir).unwrap();
        println!("playback regression: ok");
        crate::leave(0);
    }

    #[test]
    fn library_pictures_do_not_depend_on_timeline_content() {
        let mut timeline = Timeline::default();
        for time in [0.0, 3.0, 100.0] {
            for kind in [MediaKind::Video, MediaKind::Image] {
                assert_eq!(
                    picture_name(&timeline, time, Some(("source", kind))),
                    Some("source")
                );
            }
        }
        timeline.tracks.push(Track {
            id: "t".into(),
            ..Default::default()
        });
        timeline.clips.push(Arc::new(Clip::blank(
            "c",
            "t",
            ClipKind::Video,
            "timeline",
            0.0,
            10.0,
        )));
        assert_eq!(
            picture_name(&timeline, 1.0, Some(("sound", MediaKind::Audio))),
            None
        );
        assert_eq!(picture_name(&timeline, 1.0, None), Some("timeline"));
        assert_eq!(picture_name(&timeline, 10.0, None), None);
        timeline.tracks[0].visible = false;
        assert_eq!(picture_name(&timeline, 1.0, None), None);
    }

    #[test]
    fn preview_axis_uses_its_own_instant_and_topmost_visible_picture() {
        let timeline = Timeline {
            tracks: vec![
                Track {
                    id: "bottom".into(),
                    ..Default::default()
                },
                Track {
                    id: "top".into(),
                    ..Default::default()
                },
            ],
            clips: vec![
                Arc::new(Clip::blank(
                    "a",
                    "bottom",
                    ClipKind::Video,
                    "bottom",
                    2.0,
                    5.0,
                )),
                Arc::new(Clip::blank("b", "top", ClipKind::Text, "title", 3.0, 1.0)),
            ],
            ..Default::default()
        };
        assert_eq!(picture_name(&timeline, 0.0, None), None);
        assert_eq!(picture_name(&timeline, 2.0, None), Some("bottom"));
        assert_eq!(picture_name(&timeline, 3.0, None), Some("title"));
        assert_eq!(picture_name(&timeline, 4.0, None), Some("bottom"));
    }
}
