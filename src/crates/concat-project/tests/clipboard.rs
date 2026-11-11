// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Full-state copying, typed placement and atomic clipboard regression tests.

use concat_project::model::*;
use concat_project::{
    Command, DocumentSettings, Editor, Project, clipboard::Clipboard, to_document,
};
use std::sync::Arc;

fn media(id: &str, path: &str) -> MediaItem {
    serde_json::from_value(serde_json::json!({"id": id, "path": path, "name": path, "kind": "video", "duration": 60.0, "hasAudio": true})).unwrap()
}

fn editor(project: &Project) -> Editor {
    Editor::from_document(&to_document(
        &DocumentSettings {
            canvas: [0, 0, 0, 255],
            name: "Copy".into(),
            width: 640,
            height: 360,
            rate_num: 30,
            rate_den: 1,
        },
        project,
    ))
    .unwrap()
}

fn source() -> Project {
    let mut p = Project::new();
    p.media = vec![
        media("m1", "/film.mp4"),
        media("m2", "/enhanced.mp4"),
        media("m3", "/background.png"),
    ];
    let t = p.active_mut();
    t.tracks = vec![
        Track {
            id: "a".into(),
            kind: TrackKind::Audio,
            ..Default::default()
        },
        Track {
            id: "v".into(),
            kind: TrackKind::Video,
            is_main: true,
            ..Default::default()
        },
        Track {
            id: "v2".into(),
            kind: TrackKind::Video,
            ..Default::default()
        },
        Track {
            id: "text".into(),
            kind: TrackKind::Text,
            ..Default::default()
        },
        Track {
            id: "captions".into(),
            kind: TrackKind::Captions,
            ..Default::default()
        },
        Track {
            id: "stickers".into(),
            kind: TrackKind::Sticker,
            ..Default::default()
        },
        Track {
            id: "filter".into(),
            kind: TrackKind::Filter,
            ..Default::default()
        },
    ];
    t.clips.clear();
    p
}

fn paste(editor: &mut Editor, clipboard: Clipboard, start: f64) -> String {
    editor
        .apply(Command::PasteClips {
            clipboard: Box::new(clipboard),
            start,
            shift: 0,
            locked_tracks: vec![],
        })
        .unwrap()
        .created_id
        .unwrap()
}

fn decorated(kind: ClipKind, lane: &str) -> Clip {
    let mut c = Clip::blank("source", lane, kind, "Styled", 2.0, 3.0);
    if matches!(kind, ClipKind::Video | ClipKind::Audio | ClipKind::Image) {
        c.media_id = "m1".into();
    }
    if kind == ClipKind::Text {
        c.text = Some(TextStyle {
            content: "Words".into(),
            runs: vec![TextRun {
                start: 1,
                end: 4,
                style: RunStyle {
                    color: Some("#ff0000".into()),
                    font_scale: Some(1.5),
                    ..Default::default()
                },
            }],
            text_case: concat_project::model::TextCase::Upper,
            strikethrough: true,
            underline: true,
            ..Default::default()
        });
    }
    if kind == ClipKind::Shape {
        c.shape = Some(ShapeStyle::default());
    }
    c.masks = vec![Mask::of("mask-1", "Mask 1", MaskShape::Circle)];
    c.cutout = Some(Cutout {
        mode: CutoutMode::Custom,
        subject: Subject::Person,
        feather: 0.02,
        strokes: vec![],
    });
    c.stabilize = Some(Stabilize::default());
    c.background = Some(Background::Image {
        media_id: "m3".into(),
    });
    c.enhanced = Some(Enhanced {
        offset: 4.0,
        original: "m1".into(),
        copy: "m2".into(),
        on: false,
    });
    c.source_start = 4.0;
    c.audio_stream = Some(2);
    c.scale = 1.4;
    c.offset_x = 0.2;
    c.offset_y = -0.1;
    c.rotation = 23.0;
    c.opacity = 0.37;
    c.visual_motion = CaptionMotion {
        entrance: 1,
        exit: 2,
        duration: 0.4,
    };
    c.speed = 1.5;
    c.volume = 0.6;
    c.fade_in = 0.2;
    c.fade_out = 0.4;
    c.flip_h = true;
    let mut effect = AppliedFilter::new("concat.glow");
    effect.enabled = false;
    effect.params.insert("amount".into(), 0.7);
    effect.set_key("amount", 0.5, 0.9, KeyEase::LINEAR);
    c.video_effects = vec![effect.clone()];
    c.filters = vec![effect];
    c.set_key(KeyProperty::Opacity, 0.0, 0.3, KeyEase::LINEAR);
    c.extra
        .insert("futureSetting".into(), serde_json::json!({"kept": true}));
    c
}

#[test]
fn every_clip_kind_keeps_its_complete_state_and_auxiliary_media() {
    for (kind, lane) in [
        (ClipKind::Video, "v"),
        (ClipKind::Audio, "a"),
        (ClipKind::Image, "stickers"),
        (ClipKind::Video, "stickers"),
        (ClipKind::Text, "text"),
        (ClipKind::Shape, "stickers"),
        (ClipKind::Layer, "filter"),
        (ClipKind::SoundLayer, "filter"),
    ] {
        let mut p = source();
        p.active_mut().clips.push(Arc::new(decorated(kind, lane)));
        let original = editor(&p);
        let held = Clipboard::capture(original.project(), &["source".into()]);
        let source = held.clips[0].clip.clone();
        let mut target = Project::new();
        // Same ID, different file: this must not be mistaken for the source.
        target.media.push(media("m1", "/unrelated.mp4"));
        let mut target = editor(&target);
        let before = target.project().clone();
        let id = paste(&mut target, held, 12.0);
        let copy = target.project().active().clip(&id).unwrap();
        if lane == "stickers" {
            assert_eq!(
                target
                    .project()
                    .active()
                    .track(&copy.track_id)
                    .unwrap()
                    .kind,
                TrackKind::Sticker,
                "animated/video stickers must remain on sticker lanes"
            );
        }
        let mut expected = source.clone();
        expected.id = copy.id.clone();
        expected.track_id = copy.track_id.clone();
        expected.start = 12.0;
        if expected.text.is_none() {
            expected.name.push_str(" copy");
        }
        let mapped = |id: &str| {
            target
                .project()
                .media
                .iter()
                .find(|m| m.path == original.project().media_by_id(id).unwrap().path)
                .unwrap()
                .id
                .clone()
        };
        if !expected.media_id.is_empty() {
            expected.media_id = mapped(&expected.media_id);
        }
        if let Some(e) = &mut expected.enhanced {
            e.original = mapped(&e.original);
            e.copy = mapped(&e.copy);
        }
        if let Some(Background::Image { media_id }) = &mut expected.background {
            *media_id = mapped(media_id);
        }
        assert_eq!(copy, &expected, "kind {kind:?}");
        assert!(target.undo());
        assert_eq!(
            target.project(),
            &before,
            "imports, tracks and clip undo together"
        );
        assert!(target.redo());
        let reopened = editor(target.project());
        assert_eq!(
            reopened.project().active().clip(&id),
            target.project().active().clip(&id)
        );
    }
}

#[test]
fn group_placement_preserves_timing_and_distinct_typed_unlocked_lanes() {
    let mut p = source();
    for (id, lane, start, kind) in [
        ("v1", "v", 1.0, ClipKind::Video),
        ("v2", "v2", 2.5, ClipKind::Video),
        ("caption", "captions", 3.0, ClipKind::Text),
        ("layer", "filter", 1.0, ClipKind::Layer),
    ] {
        let mut c = Clip::blank(id, lane, kind, id, start, 2.0);
        if kind == ClipKind::Video {
            c.media_id = "m1".into();
        }
        if kind == ClipKind::Text {
            c.text = Some(TextStyle {
                caption_group: Some("cg1".into()),
                ..Default::default()
            });
        }
        p.active_mut().clips.push(Arc::new(c));
    }
    let held = Clipboard::capture(
        &p,
        &["v1".into(), "v2".into(), "caption".into(), "layer".into()],
    );
    let mut target = source();
    target
        .active_mut()
        .tracks
        .retain(|t| t.id == "v" || t.id == "a");
    let mut target = editor(&target);
    target
        .apply(Command::PasteClips {
            clipboard: Box::new(held),
            start: 10.0,
            shift: 100,
            locked_tracks: vec!["v".into()],
        })
        .unwrap();
    let clips = &target.project().active().clips;
    let lanes: std::collections::HashSet<_> = clips.iter().map(|c| &c.track_id).collect();
    assert_eq!(lanes.len(), 4);
    for c in clips {
        assert_ne!(c.track_id, "v");
        let kind = target.project().active().track(&c.track_id).unwrap().kind;
        assert_eq!(
            kind,
            TrackKind::from_clip_kind(c.kind, concat_project::captions::is_caption(c))
        );
        assert_eq!(
            c.start,
            if c.name.starts_with("v2") {
                11.5
            } else if c.kind == ClipKind::Text {
                12.0
            } else {
                10.0
            }
        );
    }
}

#[test]
fn caption_groups_are_independent_per_paste_but_keep_their_internal_members() {
    let mut p = source();
    for (id, group, at) in [
        ("one", "cg1", 1.0),
        ("two", "cg1", 2.0),
        ("three", "cg2", 3.0),
    ] {
        let mut c = Clip::blank(id, "captions", ClipKind::Text, id, at, 0.5);
        c.text = Some(TextStyle {
            caption_group: Some(group.into()),
            content: id.into(),
            ..Default::default()
        });
        p.active_mut().clips.push(Arc::new(c));
    }
    let held = Clipboard::capture(&p, &["one".into(), "two".into(), "three".into()]);
    let mut target = editor(&p);
    paste(&mut target, held.clone(), 10.0);
    let t = target.project().active();
    let copies: Vec<_> = t.clips.iter().filter(|c| c.start >= 10.0).collect();
    let group = |c: &Clip| c.text.as_ref().unwrap().caption_group.clone().unwrap();
    assert_eq!(group(copies[0]), group(copies[1]));
    assert_ne!(group(copies[0]), group(copies[2]));
    assert_ne!(group(copies[0]), "cg1");
    let scope = concat_project::captions::style_targets(t, &[copies[0].id.clone()]);
    assert_eq!(scope.len(), 2);
    let first_group = group(copies[0]);
    paste(&mut target, held, 20.0);
    assert!(
        target
            .project()
            .active()
            .clips
            .iter()
            .filter(|c| c.start >= 20.0)
            .all(|c| group(c) != first_group)
    );
}

#[test]
fn missing_auxiliary_media_and_font_conflicts_leave_no_partial_edit() {
    let mut p = source();
    p.active_mut()
        .clips
        .push(Arc::new(decorated(ClipKind::Video, "v")));
    let mut held = Clipboard::capture(&p, &["source".into()]);
    held.media.retain(|m| m.id != "m3");
    let mut target = editor(&Project::new());
    let before = target.project().clone();
    assert!(
        target
            .apply(Command::PasteClips {
                clipboard: Box::new(held),
                start: 0.0,
                shift: 0,
                locked_tracks: vec![]
            })
            .is_err()
    );
    assert_eq!(target.project(), &before);
    assert!(!target.undo());
    let mut held = Clipboard::capture(&p, &["source".into()]);
    held.fonts.push(CustomFont {
        family: "Custom".into(),
        path: "/other.ttf".into(),
    });
    let mut p = Project::new();
    p.fonts.push(CustomFont {
        family: "Custom".into(),
        path: "/existing.ttf".into(),
    });
    let mut target = editor(&p);
    let before = target.project().clone();
    assert!(
        target
            .apply(Command::PasteClips {
                clipboard: Box::new(held),
                start: 0.0,
                shift: 0,
                locked_tracks: vec![]
            })
            .is_err()
    );
    assert_eq!(target.project(), &before);
}

#[test]
fn neighbours_remap_only_when_they_are_part_of_the_copied_group() {
    let mut p = source();
    let mut v = Clip::blank("video", "v", ClipKind::Video, "video", 1.0, 2.0);
    v.media_id = "m1".into();
    v.muted = Some(true);
    let mut next = v.clone();
    next.id = "next".into();
    next.name = "next".into();
    next.start = 3.0;
    next.transition_in = Some(Transition {
        id: "cross-fade".into(),
        duration: 0.5,
        params: Default::default(),
    });
    let mut a = Clip::blank("audio", "a", ClipKind::Audio, "audio", 1.0, 2.0);
    a.media_id = "m1".into();
    a.detached_from = Some("video".into());
    p.active_mut().clips = vec![Arc::new(v), Arc::new(next), Arc::new(a)];
    let mut target = editor(&Project::new());
    paste(
        &mut target,
        Clipboard::capture(&p, &["video".into(), "next".into(), "audio".into()]),
        10.0,
    );
    let t = target.project().active();
    let video = t.clips.iter().find(|c| c.name == "video copy").unwrap();
    let audio = t.clips.iter().find(|c| c.name == "audio copy").unwrap();
    assert_eq!(audio.detached_from.as_deref(), Some(video.id.as_str()));
    assert!(
        t.clips
            .iter()
            .find(|c| c.name == "next copy")
            .unwrap()
            .transition_in
            .is_some()
    );
    let mut separate = editor(&Project::new());
    let id = paste(
        &mut separate,
        Clipboard::capture(&p, &["next".into()]),
        10.0,
    );
    assert!(
        separate
            .project()
            .active()
            .clip(&id)
            .unwrap()
            .transition_in
            .is_none()
    );
    let id = paste(
        &mut separate,
        Clipboard::capture(&p, &["audio".into()]),
        20.0,
    );
    assert!(
        separate
            .project()
            .active()
            .clip(&id)
            .unwrap()
            .detached_from
            .is_none()
    );
}

#[test]
fn split_tail_with_speed_curve_keeps_source_coverage_and_group_gaps_shift_together() {
    let mut p = source();
    let mut a = Clip::blank("tail", "v", ClipKind::Video, "tail", 1.0, 2.0);
    a.media_id = "m1".into();
    a.source_start = 7.0;
    a.speed_curve = Some(vec![
        SpeedPoint {
            at: 0.0,
            speed: 0.5,
            slope: None,
        },
        SpeedPoint {
            at: 1.0,
            speed: 2.0,
            slope: None,
        },
    ]);
    let mut b = a.clone();
    b.id = "later".into();
    b.name = "later".into();
    b.start = 5.0;
    p.active_mut().clips = vec![Arc::new(a), Arc::new(b)];
    let source = editor(&p);
    let held = Clipboard::capture(source.project(), &["tail".into(), "later".into()]);
    let mut target = source.project().clone();
    target.active_mut().clips = vec![Arc::new(Clip {
        id: "occupied".into(),
        start: 10.0,
        duration: 3.0,
        ..held.clips[0].clip.clone()
    })];
    let mut target = editor(&target);
    paste(&mut target, held.clone(), 10.0);
    let clips = &target.project().active().clips;
    for (source, name) in held.clips.iter().zip(["tail copy", "later copy"]) {
        let copy = clips.iter().find(|c| c.name == name).unwrap();
        assert_eq!(
            (
                copy.duration,
                copy.source_start,
                copy.speed,
                &copy.speed_curve
            ),
            (
                source.clip.duration,
                source.clip.source_start,
                source.clip.speed,
                &source.clip.speed_curve
            )
        );
        assert_eq!(copy.start, 13.0 + source.clip.start - 1.0);
    }
}

#[test]
fn a_clipboard_command_cannot_be_injected_through_json() {
    let command = Command::PasteClips {
        clipboard: Box::default(),
        start: 0.0,
        shift: 0,
        locked_tracks: vec![],
    };
    assert!(serde_json::to_value(command).is_err());
    assert!(
        serde_json::from_value::<Command>(serde_json::json!({"op":"pasteClips", "start":0}))
            .is_err()
    );
}

#[test]
fn template_slots_with_no_file_keep_distinct_media_and_remain_placeholders() {
    let mut p = source();
    p.media[0].path.clear();
    p.media[0].placeholder = true;
    let mut c = Clip::blank("slot", "v", ClipKind::Video, "Slot", 0.0, 2.0);
    c.media_id = "m1".into();
    p.active_mut().clips.push(Arc::new(c));
    let held = Clipboard::capture(&p, &["slot".into()]);
    let mut target = editor(&Project::new());
    paste(&mut target, held.clone(), 0.0);
    paste(&mut target, held, 3.0);
    assert_eq!(target.project().media.len(), 2);
    assert!(
        target
            .project()
            .media
            .iter()
            .all(|m| m.placeholder && m.path.is_empty())
    );
    assert_ne!(
        target.project().active().clips[0].media_id,
        target.project().active().clips[1].media_id
    );
}

#[test]
fn an_empty_caption_group_does_not_turn_an_ordinary_title_into_captions() {
    let mut p = source();
    let mut c = Clip::blank("title", "text", ClipKind::Text, "Title", 0.0, 2.0);
    c.text = Some(TextStyle {
        caption_group: Some(String::new()),
        ..Default::default()
    });
    p.active_mut().clips.push(Arc::new(c));
    let mut target = editor(&Project::new());
    let id = paste(&mut target, Clipboard::capture(&p, &["title".into()]), 0.0);
    let copy = target.project().active().clip(&id).unwrap();
    assert!(!concat_project::captions::is_caption(copy));
    assert_eq!(
        target
            .project()
            .active()
            .track(&copy.track_id)
            .unwrap()
            .kind,
        TrackKind::Text
    );
}
