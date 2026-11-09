// SPDX-License-Identifier: AGPL-3.0-or-later
//! Subject stages, source ownership and atomic rejection regressions.
use concat_project::commands::{self, ClipPatch, Command, CommandError, IdMint};
use concat_project::model::{AppliedFilter, Clip, ClipKind, Project, order_subject_effects};

#[test]
fn subject_stage_keeps_relative_order_and_original_picture_chain() {
    let mut effects: Vec<_> = [
        "concat.vhs-tape",
        "concat.subject-neon",
        "concat.adjust",
        "concat.subject-electric",
    ]
    .into_iter()
    .map(AppliedFilter::new)
    .collect();
    order_subject_effects(&mut effects);
    assert_eq!(
        effects.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        [
            "concat.subject-neon",
            "concat.subject-electric",
            "concat.vhs-tape",
            "concat.adjust"
        ]
    );
    let once = effects.clone();
    order_subject_effects(&mut effects);
    assert_eq!(effects, once);
}

#[test]
fn subject_effect_rejects_non_source_clips_before_mutating_other_properties() {
    for kind in [ClipKind::Layer, ClipKind::Text, ClipKind::Shape] {
        let mut project = Project::new();
        project.active_mut().clips.push(
            Clip {
                id: "c1".into(),
                name: "Keep".into(),
                kind,
                ..Default::default()
            }
            .into(),
        );
        let before = serde_json::to_value(&project).unwrap();
        let result = commands::apply(
            &mut project,
            &mut IdMint::default(),
            Command::UpdateClip {
                clip_id: "c1".into(),
                patch: ClipPatch {
                    name: Some("Changed".into()),
                    video_effects: Some(vec![AppliedFilter::new("concat.subject-neon")]),
                    ..Default::default()
                },
            },
        );
        assert!(matches!(result, Err(CommandError::SubjectNeedsMedia)));
        assert_eq!(serde_json::to_value(project).unwrap(), before);
    }
}

#[test]
fn source_effect_keeps_the_background_and_does_not_add_cutout_state() {
    let mut project = Project::new();
    project.active_mut().clips.push(
        Clip {
            id: "c1".into(),
            kind: ClipKind::Video,
            ..Default::default()
        }
        .into(),
    );
    commands::apply(
        &mut project,
        &mut IdMint::default(),
        Command::UpdateClip {
            clip_id: "c1".into(),
            patch: ClipPatch {
                video_effects: Some(vec![
                    AppliedFilter::new("concat.vhs-tape"),
                    AppliedFilter::new("concat.subject-neon"),
                ]),
                ..Default::default()
            },
        },
    )
    .unwrap();
    let clip = project.active().clip("c1").unwrap();
    assert!(clip.cutout.is_none());
    assert_eq!(clip.video_effects[0].id, "concat.subject-neon");
    assert_eq!(clip.video_effects[1].id, "concat.vhs-tape");
}

#[test]
fn subject_effect_cannot_be_created_as_a_timeline_layer() {
    let mut project = Project::new();
    let before = serde_json::to_value(&project).unwrap();
    let result = commands::apply(
        &mut project,
        &mut IdMint::default(),
        Command::AddLayerClip {
            sound: false,
            track_id: None,
            start: 0.0,
            duration: Some(3.0),
            effect_id: "concat.face-dog".into(),
            name: "Electric".into(),
        },
    );
    assert!(matches!(result, Err(CommandError::SubjectNeedsMedia)));
    assert_eq!(serde_json::to_value(project).unwrap(), before);
}
