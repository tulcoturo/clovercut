// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! 3-point source editing: Insert (ripple) and Overwrite placement.

use concat_project::{
    Command, DocumentSettings, Editor,
    commands::NewMedia,
    model::{MediaKind, Project},
    to_document,
};

fn setup_studio() -> (Editor, String, String) {
    let settings = DocumentSettings {
        name: "3-point studio".into(),
        width: 1920,
        height: 1080,
        rate_num: 30,
        rate_den: 1,
        canvas: [0, 0, 0, 255],
    };
    let mut editor = Editor::from_document(&to_document(&settings, &Project::new())).unwrap();
    let track_id = editor.project().active().tracks[0].id.clone();
    let media = editor
        .apply(Command::AddMedia {
            item: NewMedia {
                path: "/footage/take1.mp4".into(),
                name: "take1.mp4".into(),
                duration: Some(20.0),
                kind: MediaKind::Video,
                width: Some(1920),
                height: Some(1080),
                frame_rate: Some(30.0),
                frame_rate_fraction: Some("30/1".into()),
                video_codec: None,
                audio_codec: None,
                has_audio: true,
                audio_tracks: Vec::new(),
                origin: None,
                color_space: Default::default(),
            },
        })
        .unwrap()
        .created_id
        .unwrap();

    (editor, track_id, media)
}

#[test]
fn insert_mode_ripples_later_clips() {
    let (mut editor, track_id, media_id) = setup_studio();

    // Place an initial clip at 0.0 with 5.0 duration
    editor
        .apply(Command::PlaceMediaRange {
            media_id: media_id.clone(),
            track_id: track_id.clone(),
            start: 0.0,
            source_start: Some(0.0),
            duration: Some(5.0),
            overwrite: false,
        })
        .unwrap();

    // Place second clip at 5.0 with 5.0 duration (from 5.0 to 10.0)
    let c2 = editor
        .apply(Command::PlaceMediaRange {
            media_id: media_id.clone(),
            track_id: track_id.clone(),
            start: 5.0,
            source_start: Some(5.0),
            duration: Some(5.0),
            overwrite: false,
        })
        .unwrap()
        .created_id
        .unwrap();

    // Now insert a 3.0s cut at 5.0s (between c1 and c2)
    let inserted = editor
        .apply(Command::PlaceMediaRange {
            media_id: media_id.clone(),
            track_id: track_id.clone(),
            start: 5.0,
            source_start: Some(10.0),
            duration: Some(3.0),
            overwrite: false,
        })
        .unwrap()
        .created_id
        .unwrap();

    let tl = editor.project().active();
    assert_eq!(tl.clips.len(), 3);

    let ins_clip = tl.clip(&inserted).unwrap();
    assert_eq!(ins_clip.start, 5.0);
    assert_eq!(ins_clip.duration, 3.0);
    assert_eq!(ins_clip.source_start, 10.0);

    // c2 should have rippled from 5.0 to 8.0!
    let c2_clip = tl.clip(&c2).unwrap();
    assert_eq!(c2_clip.start, 8.0);
    assert_eq!(c2_clip.duration, 5.0);

    // Undo should restore c2 back to 5.0
    assert!(editor.undo());
    let tl_after = editor.project().active();
    assert_eq!(tl_after.clips.len(), 2);
    assert_eq!(tl_after.clip(&c2).unwrap().start, 5.0);

    assert!(editor.redo());
    assert_eq!(editor.project().active().clips.len(), 3);
}

#[test]
fn overwrite_mode_splits_middle_of_clip() {
    let (mut editor, track_id, media_id) = setup_studio();

    // Base clip spanning 0.0 to 10.0
    let base_id = editor
        .apply(Command::PlaceMediaRange {
            media_id: media_id.clone(),
            track_id: track_id.clone(),
            start: 0.0,
            source_start: Some(0.0),
            duration: Some(10.0),
            overwrite: false,
        })
        .unwrap()
        .created_id
        .unwrap();

    // Overwrite range [4.0, 7.0] (3s duration)
    let cut_id = editor
        .apply(Command::PlaceMediaRange {
            media_id: media_id.clone(),
            track_id: track_id.clone(),
            start: 4.0,
            source_start: Some(12.0),
            duration: Some(3.0),
            overwrite: true,
        })
        .unwrap()
        .created_id
        .unwrap();

    let tl = editor.project().active();
    // Should now have 3 clips: head [0..4], new [4..7], tail [7..10]
    assert_eq!(tl.clips.len(), 3);

    let head = tl.clip(&base_id).unwrap();
    assert_eq!(head.start, 0.0);
    assert_eq!(head.duration, 4.0);
    assert_eq!(head.source_start, 0.0);

    let new_clip = tl.clip(&cut_id).unwrap();
    assert_eq!(new_clip.start, 4.0);
    assert_eq!(new_clip.duration, 3.0);
    assert_eq!(new_clip.source_start, 12.0);

    let tail = tl
        .clips
        .iter()
        .find(|c| c.id != base_id && c.id != cut_id)
        .unwrap();
    assert_eq!(tail.start, 7.0);
    assert_eq!(tail.duration, 3.0);
    assert_eq!(tail.source_start, 7.0);

    assert!(editor.undo());
    assert_eq!(editor.project().active().clips.len(), 1);
    assert_eq!(editor.project().active().clips[0].duration, 10.0);

    assert!(editor.redo());
    assert_eq!(editor.project().active().clips.len(), 3);
}

#[test]
fn overwrite_mode_replaces_entire_covered_clip() {
    let (mut editor, track_id, media_id) = setup_studio();

    // Small clip in the middle [3.0, 5.0]
    editor
        .apply(Command::PlaceMediaRange {
            media_id: media_id.clone(),
            track_id: track_id.clone(),
            start: 3.0,
            source_start: Some(0.0),
            duration: Some(2.0),
            overwrite: false,
        })
        .unwrap();

    // Overwrite covering [2.0, 7.0]
    let big_id = editor
        .apply(Command::PlaceMediaRange {
            media_id: media_id.clone(),
            track_id: track_id.clone(),
            start: 2.0,
            source_start: Some(5.0),
            duration: Some(5.0),
            overwrite: true,
        })
        .unwrap()
        .created_id
        .unwrap();

    let tl = editor.project().active();
    assert_eq!(tl.clips.len(), 1);
    assert_eq!(tl.clips[0].id, big_id);
    assert_eq!(tl.clips[0].start, 2.0);
    assert_eq!(tl.clips[0].duration, 5.0);

    assert!(editor.undo());
    assert_eq!(editor.project().active().clips.len(), 1);
    assert_eq!(editor.project().active().clips[0].start, 3.0);
    assert_eq!(editor.project().active().clips[0].duration, 2.0);
}

#[test]
fn slip_clip_shifts_source_window_preserving_timeline_placement() {
    let (mut editor, track_id, media_id) = setup_studio();

    let clip_id = editor
        .apply(Command::PlaceMediaRange {
            media_id: media_id.clone(),
            track_id,
            start: 2.0,
            source_start: Some(3.0),
            duration: Some(5.0),
            overwrite: false,
        })
        .unwrap()
        .created_id
        .unwrap();

    // Slip forward by +4.0s
    editor
        .apply(Command::SlipClip {
            clip_id: clip_id.clone(),
            delta: 4.0,
        })
        .unwrap();

    let clip = editor.project().active().clip(&clip_id).unwrap();
    assert_eq!(clip.start, 2.0, "timeline start must not move");
    assert_eq!(clip.duration, 5.0, "timeline duration must not change");
    assert_eq!(clip.source_start, 7.0, "source start slips forward");

    // Slip forward beyond media duration (total media is 20s, clip length 5s, max source start is 15s)
    editor
        .apply(Command::SlipClip {
            clip_id: clip_id.clone(),
            delta: 50.0,
        })
        .unwrap();

    let clip = editor.project().active().clip(&clip_id).unwrap();
    assert_eq!(
        clip.source_start, 15.0,
        "source start clamped to media limit"
    );
    assert_eq!(clip.start, 2.0);

    // Slip backward below zero
    editor
        .apply(Command::SlipClip {
            clip_id: clip_id.clone(),
            delta: -100.0,
        })
        .unwrap();

    let clip = editor.project().active().clip(&clip_id).unwrap();
    assert_eq!(clip.source_start, 0.0, "source start clamped to zero");

    // Undo restores previous position
    assert!(editor.undo());
    assert_eq!(
        editor
            .project()
            .active()
            .clip(&clip_id)
            .unwrap()
            .source_start,
        15.0
    );
}
