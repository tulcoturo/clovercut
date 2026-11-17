// SPDX-License-Identifier: AGPL-3.0-or-later
//! Voice choices are document state, with undo and finite validation.
use concat_project::{
    Command, Editor,
    model::{VoiceKind, VoiceMix},
};

#[test]
fn voice_choices_round_trip_and_undo_without_touching_other_clip_settings() {
    let mut project = concat_project::Project::new();
    project.media.push(serde_json::from_value(serde_json::json!({"id":"media", "path":"/voice-fixture.mov", "kind":"video", "duration":4.0, "hasAudio":true})).unwrap());
    let track = project.active().tracks[0].id.clone();
    project.active_mut().clips.push(
        concat_project::model::Clip::blank(
            "clip",
            track,
            concat_project::model::ClipKind::Video,
            "Voice",
            0.0,
            4.0,
        )
        .into(),
    );
    std::sync::Arc::make_mut(&mut project.active_mut().clips[0]).media_id = "media".into();
    let settings = concat_project::DocumentSettings {
        name: "Voice".into(),
        width: 320,
        height: 180,
        rate_num: 30,
        rate_den: 1,
        canvas: [0, 0, 0, 255],
    };
    let document = concat_project::to_document(&settings, &project);
    let mut editor = Editor::from_document(&document).expect("video fixture");
    let clip_id = editor.project().active().clips[0].id.clone();
    let before = editor.project().active().clips[0].clone();
    let mix = VoiceMix {
        voice: 0.7,
        background: 0.2,
        kind: VoiceKind::Speech,
    };
    editor
        .apply(Command::SetClipVoice {
            clip_id: clip_id.clone(),
            voice: Some(mix),
        })
        .unwrap();
    assert_eq!(editor.project().active().clips[0].voice, Some(mix));
    assert!(editor.undo());
    assert_eq!(editor.project().active().clips[0], before);
    assert!(editor.redo());
    let saved = editor.to_document(&settings);
    let reopened = Editor::from_document(&saved).unwrap();
    assert_eq!(reopened.project().active().clips[0].voice, Some(mix));
    assert_eq!(editor.project().active().clips[0].voice, Some(mix));
    assert!(
        editor
            .apply(Command::SetClipVoice {
                clip_id,
                voice: Some(VoiceMix {
                    voice: f64::NAN,
                    ..mix
                })
            })
            .is_err()
    );
    assert_eq!(editor.project().active().clips[0].voice, Some(mix));
}
