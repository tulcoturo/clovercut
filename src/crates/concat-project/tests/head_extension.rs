// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors
//! Head extension follows content boundaries, neighboring clips and undo.
use concat_project::{
    Command, Editor,
    commands::{NewMedia, TrimEdge},
    model::{ClipKind, KeyProperty, MediaKind, TrackKind},
};

fn add(editor: &mut Editor, kind: ClipKind, track: &str, start: f64) -> String {
    let command = match kind {
        ClipKind::Text => Command::AddTextClip {
            track_id: Some(track.into()),
            above: true,
            start,
            style: None,
            duration: Some(4.0),
            offset_y: None,
        },
        ClipKind::Shape => Command::AddShapeClip {
            track_id: Some(track.into()),
            above: true,
            start,
            style: None,
            duration: Some(4.0),
            name: "Sticker".into(),
        },
        ClipKind::Transition => Command::AddTransitionClip {
            track_id: Some(track.into()),
            start,
            duration: Some(4.0),
            transition_id: "concat.push".into(),
            name: "Push".into(),
        },
        ClipKind::Layer | ClipKind::SoundLayer => Command::AddLayerClip {
            sound: kind == ClipKind::SoundLayer,
            track_id: Some(track.into()),
            start,
            duration: Some(4.0),
            effect_id: "concat.warm".into(),
            name: "Adjustment".into(),
        },
        ClipKind::Image | ClipKind::Video | ClipKind::Audio => {
            let media_kind = match kind {
                ClipKind::Image => MediaKind::Image,
                ClipKind::Audio => MediaKind::Audio,
                _ => MediaKind::Video,
            };
            let media = editor
                .apply(Command::AddMedia {
                    item: NewMedia {
                        path: format!("/fixture-{kind:?}-{start}.media"),
                        name: "Fixture".into(),
                        kind: media_kind,
                        duration: Some(4.0),
                        width: Some(320),
                        height: Some(320),
                        frame_rate: None,
                        frame_rate_fraction: None,
                        video_codec: None,
                        audio_codec: None,
                        has_audio: kind == ClipKind::Audio,
                        audio_tracks: vec![],
                        origin: None,
                        color_space: Default::default(),
                    },
                })
                .unwrap()
                .created_id
                .unwrap();
            Command::AddClip {
                media_id: media,
                track_id: track.into(),
                start,
                ripple: false,
            }
        }
    };
    let id = editor.apply(command).unwrap().created_id.unwrap();
    // Stills have an editorial five-second default, independent of media duration.
    if kind == ClipKind::Image {
        editor
            .apply(Command::TrimClip {
                clip_id: id.clone(),
                edge: TrimEdge::End,
                delta: -1.0,
                ripple: false,
            })
            .unwrap();
    }
    id
}
fn fixture(kind: ClipKind) -> (Editor, String, String) {
    let mut editor = Editor::new();
    let track_kind = match kind {
        ClipKind::Text => TrackKind::Text,
        ClipKind::Shape | ClipKind::Image => TrackKind::Sticker,
        ClipKind::Layer | ClipKind::SoundLayer | ClipKind::Transition => TrackKind::Filter,
        ClipKind::Audio => TrackKind::Audio,
        _ => TrackKind::Video,
    };
    let track = editor
        .apply(Command::AddTrack {
            bottom: kind == ClipKind::Audio,
            kind: Some(track_kind),
            insert_at: None,
        })
        .unwrap()
        .created_id
        .unwrap();
    let id = add(&mut editor, kind, &track, 8.0);
    (editor, track, id)
}
#[test]
fn still_and_generated_heads_extend_left_keep_existing_animation_and_undo() {
    for kind in [
        ClipKind::Image,
        ClipKind::Text,
        ClipKind::Shape,
        ClipKind::Layer,
        ClipKind::Transition,
        ClipKind::SoundLayer,
    ] {
        let (mut editor, _, id) = fixture(kind);
        editor
            .apply(Command::SetClipKey {
                clip_id: id.clone(),
                property: KeyProperty::Opacity,
                at: 0.5,
                value: 0.42,
                ease: Default::default(),
            })
            .unwrap();
        let before = serde_json::to_value(editor.project()).unwrap();
        assert!(
            editor
                .apply(Command::TrimClip {
                    clip_id: id.clone(),
                    edge: TrimEdge::Start,
                    delta: -3.0,
                    ripple: false
                })
                .unwrap()
                .applied,
            "{kind:?}"
        );
        let clip = editor.project().active().clip(&id).unwrap();
        assert_eq!(
            (clip.start, clip.duration, clip.source_start),
            (5.0, 7.0, 0.0),
            "{kind:?}"
        );
        let key = clip
            .keys_on(KeyProperty::Opacity)
            .find(|key| (key.value - 0.42).abs() < 1e-9)
            .unwrap();
        assert!(
            (clip.start + key.at * clip.duration - 10.0).abs() < 1e-9,
            "animation stays at the same timeline time"
        );
        let after = serde_json::to_value(editor.project()).unwrap();
        assert!(editor.undo());
        assert_eq!(serde_json::to_value(editor.project()).unwrap(), before);
        assert!(editor.redo());
        assert_eq!(serde_json::to_value(editor.project()).unwrap(), after);
    }
}
#[test]
fn free_heads_stop_at_zero_and_at_the_preceding_clip() {
    for kind in [
        ClipKind::Image,
        ClipKind::Text,
        ClipKind::Shape,
        ClipKind::Layer,
        ClipKind::Transition,
    ] {
        let (mut editor, _, id) = fixture(kind);
        editor
            .apply(Command::TrimClip {
                clip_id: id.clone(),
                edge: TrimEdge::Start,
                delta: -100.0,
                ripple: false,
            })
            .unwrap();
        let clip = editor.project().active().clip(&id).unwrap();
        assert_eq!(
            (clip.start, clip.duration, clip.source_start),
            (0.0, 12.0, 0.0)
        );
        let (mut editor, track, id) = fixture(kind);
        let previous = add(&mut editor, kind, &track, 1.0);
        editor
            .apply(Command::TrimClip {
                clip_id: id.clone(),
                edge: TrimEdge::Start,
                delta: -100.0,
                ripple: false,
            })
            .unwrap();
        let clip = editor.project().active().clip(&id).unwrap();
        assert_eq!((clip.start, clip.duration), (5.0, 7.0));
        let previous = editor.project().active().clip(&previous).unwrap();
        assert_eq!((previous.start, previous.duration), (1.0, 4.0));
    }
}
#[test]
fn recorded_video_and_audio_still_stop_at_their_in_point() {
    for kind in [ClipKind::Video, ClipKind::Audio] {
        let (mut editor, _, id) = fixture(kind);
        assert!(
            !editor
                .apply(Command::TrimClip {
                    clip_id: id.clone(),
                    edge: TrimEdge::Start,
                    delta: -3.0,
                    ripple: false
                })
                .unwrap()
                .applied
        );
        let clip = editor.project().active().clip(&id).unwrap();
        assert_eq!(
            (clip.start, clip.duration, clip.source_start),
            (8.0, 4.0, 0.0)
        );
    }
}
