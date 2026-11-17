// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Caption membership, independent of ordinary titles and timeline placement.

use crate::model::{Clip, ClipKind, Timeline};

/// The default caption collection within a timeline.
pub const DEFAULT_GROUP: &str = "captions";

/// Whether a clip belongs to a caption collection.
pub fn is_caption(clip: &Clip) -> bool {
    clip.kind == ClipKind::Text && group(clip).is_some()
}

fn group(clip: &Clip) -> Option<&str> {
    clip.text
        .as_ref()?
        .caption_group
        .as_deref()
        .filter(|id| !id.is_empty())
}

/// Apply a caption inspector edit to the collection or just the selected clips.
pub fn scoped_targets(timeline: &Timeline, selected: &[String], all: bool) -> Vec<String> {
    if all {
        style_targets(timeline, selected)
    } else {
        selected.to_vec()
    }
}

/// Expand styling targets to their caption collections, preserving selection
/// order. Text content, timing, and ordinary titles are never grouped by this.
pub fn style_targets(timeline: &Timeline, selected: &[String]) -> Vec<String> {
    let groups: Vec<&str> = selected
        .iter()
        .filter_map(|id| timeline.clip(id).and_then(group))
        .collect();
    let mut ids = selected.to_vec();
    for clip in &timeline.clips {
        if group(clip).is_some_and(|id| groups.contains(&id)) && !ids.contains(&clip.id) {
            ids.push(clip.id.clone());
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Command, DocumentSettings, Editor,
        commands::ClipPatch,
        model::{Project, TextStyle},
        to_document,
    };

    #[test]
    fn subtitle_library_survives_reopen_and_copies_have_independent_style_scopes() {
        let settings = DocumentSettings {
            canvas: [0, 0, 0, 255],
            name: "Library".into(),
            width: 640,
            height: 360,
            rate_num: 30,
            rate_den: 1,
        };
        let mut editor = Editor::from_document(&to_document(&settings, &Project::new())).unwrap();
        let source = editor.apply(Command::AddCaptionSource {
            name: "translation.srt".into(),
            srt: "1\n00:00:01,000 --> 00:00:01,300\n中文 Hello\n\n2\n00:00:02,000 --> 00:00:03,000\nSecond\n".into(),
            style: TextStyle::default(), offset_y: 0.35, place_at: None,
        }).unwrap().created_id.unwrap();
        assert!(
            editor.project().active().clips.is_empty(),
            "import only adds a library file"
        );
        let mut editor = Editor::from_document(&to_document(&settings, editor.project())).unwrap();
        assert_eq!(editor.project().caption_sources[0].name, "translation.srt");
        editor
            .apply(Command::PlaceCaptionSource {
                source_id: source.clone(),
                start: 5.0,
                track_id: Some("T1".into()),
            })
            .unwrap();
        let first = editor.project().active().clips[0].id.clone();
        assert_eq!(editor.project().active().clip(&first).unwrap().start, 6.0);
        assert!((editor.project().active().clip(&first).unwrap().duration - 0.3).abs() < 1e-6);
        let t2 = editor
            .apply(Command::AddTrack {
                bottom: false,
                kind: Some(crate::model::TrackKind::Captions),
                insert_at: None,
            })
            .unwrap()
            .created_id
            .unwrap();
        editor
            .apply(Command::PlaceCaptionSource {
                source_id: source.clone(),
                start: 10.0,
                track_id: Some(t2),
            })
            .unwrap();
        let scope = style_targets(editor.project().active(), &[first]);
        assert_eq!(
            scope.len(),
            2,
            "a copied collection has an independent scope"
        );
        assert_eq!(editor.project().active().clips.len(), 4);
        assert!(editor.undo());
        assert_eq!(editor.project().active().clips.len(), 2);
        assert!(editor.redo());
        editor
            .apply(Command::RemoveCaptionSource { source_id: source })
            .unwrap();
        assert_eq!(
            editor.project().active().clips.len(),
            4,
            "removing an asset keeps timeline captions"
        );
        assert!(editor.project().caption_sources.is_empty());
        assert!(editor.undo());
        assert_eq!(editor.project().caption_sources.len(), 1);
    }

    #[test]
    fn short_caption_motion_has_a_neutral_middle_and_independent_phases() {
        use crate::model::CaptionMotion;
        for kind in 1..=3 {
            let motion = CaptionMotion {
                entrance: kind,
                exit: kind,
                duration: 2.0,
            };
            assert_eq!(motion.sample(0.0, 0.2).0, 0.0);
            assert_eq!(motion.sample(0.1, 0.2), (1.0, 1.0, 0.0));
            assert_eq!(motion.sample(0.2, 0.2).0, 0.0);
            assert!(motion.sample(0.05, 0.2).0 > 0.0);
        }
        assert_eq!(CaptionMotion::default().sample(0.0, 1.0), (1.0, 1.0, 0.0));
    }

    #[test]
    fn caption_scope_and_animated_paint_survive_save_and_undo() {
        use crate::model::{KeyEase, KeyProperty};
        let mut project = Project::new();
        for id in ["a", "b"] {
            let mut clip = Clip::blank(id, "T1", ClipKind::Text, id, 0.0, 2.0);
            clip.text = Some(TextStyle {
                content: id.into(),
                caption_group: Some(DEFAULT_GROUP.into()),
                underline: true,
                vertical: true,
                vertical_align: 2,
                color: "#ff0000".into(),
                ..Default::default()
            });
            project.active_mut().clips.push(clip.into());
        }
        let selected = vec!["a".to_owned()];
        assert_eq!(scoped_targets(project.active(), &selected, false), selected);
        assert_eq!(
            scoped_targets(project.active(), &selected, true),
            vec!["a", "b"]
        );
        let settings = DocumentSettings {
            canvas: [0, 0, 0, 255],
            name: "Caption keys".into(),
            width: 640,
            height: 360,
            rate_num: 30,
            rate_den: 1,
        };
        let mut editor = Editor::from_document(&to_document(&settings, &project)).unwrap();
        let mut commands = Vec::new();
        for id in scoped_targets(editor.project().active(), &selected, true) {
            for (property, first, last) in [
                (KeyProperty::TextRed, 1.0, 0.0),
                (KeyProperty::TextBlue, 0.0, 1.0),
                (KeyProperty::StrokeWidth, 0.0, 0.01),
                (KeyProperty::StrokeOpacity, 0.0, 1.0),
            ] {
                for (at, value) in [(0.0, first), (1.0, last)] {
                    commands.push(Command::SetClipKey {
                        clip_id: id.clone(),
                        property,
                        at,
                        value,
                        ease: KeyEase::LINEAR,
                    });
                }
            }
        }
        editor.apply(Command::Batch { commands }).unwrap();
        let clip = editor.project().active().clip("a").unwrap();
        let middle = clip.text_at(0.5);
        assert_eq!(middle.color, "#800080");
        assert!((middle.stroke_width - 0.005).abs() < 1e-6);
        assert!(middle.stroke_color.ends_with("80"));
        assert!(middle.underline);
        assert!(middle.vertical);
        assert_eq!(middle.vertical_align, 2);
        assert_eq!(middle.content, "a");
        let reopened = Editor::from_document(&to_document(&settings, editor.project())).unwrap();
        assert_eq!(
            reopened
                .project()
                .active()
                .clip("b")
                .unwrap()
                .text_at(0.5)
                .color,
            "#800080"
        );
        assert!(editor.undo());
        assert!(editor.project().active().clip("a").unwrap().keys.is_empty());
        assert!(editor.project().active().clip("b").unwrap().keys.is_empty());
    }

    #[test]
    fn caption_styles_round_trip_and_undo_together_without_touching_titles_or_words() {
        let mut project = Project::new();
        for (id, group) in [
            ("a", Some("captions")),
            ("b", Some("captions")),
            ("title", None),
            ("translation", Some("translation")),
        ] {
            let mut clip = Clip::blank(id, "T1", ClipKind::Text, id, 0.0, 2.0);
            clip.text = Some(TextStyle {
                content: id.into(),
                caption_group: group.map(str::to_owned),
                ..Default::default()
            });
            project.active_mut().clips.push(clip.into());
        }
        let settings = DocumentSettings {
            canvas: [0, 0, 0, 255],
            name: "captions".into(),
            width: 1920,
            height: 1080,
            rate_num: 25,
            rate_den: 1,
        };
        let mut editor = Editor::from_document(&to_document(&settings, &project)).unwrap();
        let ids = style_targets(editor.project().active(), &["a".into()]);
        assert_eq!(ids, ["a", "b"]);
        let commands = ids
            .iter()
            .map(|id| {
                let mut style = editor
                    .project()
                    .active()
                    .clip(id)
                    .unwrap()
                    .text
                    .clone()
                    .unwrap();
                style.font_size = 0.08;
                Command::UpdateClip {
                    clip_id: id.clone(),
                    patch: ClipPatch {
                        text: Some(Some(style)),
                        ..Default::default()
                    },
                }
            })
            .collect();
        editor.apply(Command::Batch { commands }).unwrap();
        for id in ["a", "b"] {
            let clip = editor.project().active().clip(id).unwrap();
            assert_eq!(clip.text.as_ref().unwrap().font_size, 0.08);
            assert_eq!(clip.text.as_ref().unwrap().content, id);
            assert_eq!((clip.start, clip.duration), (0.0, 2.0));
        }
        for id in ["title", "translation"] {
            assert_eq!(
                editor
                    .project()
                    .active()
                    .clip(id)
                    .unwrap()
                    .text
                    .as_ref()
                    .unwrap()
                    .font_size,
                TextStyle::default().font_size
            );
        }
        let reopened = Editor::from_document(&to_document(&settings, editor.project())).unwrap();
        assert_eq!(
            style_targets(reopened.project().active(), &["a".into()]),
            ids
        );
        assert!(editor.undo());
        assert!(!editor.can_undo());
        assert_eq!(
            editor
                .project()
                .active()
                .clip("b")
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .font_size,
            TextStyle::default().font_size
        );
        assert!(editor.redo());
    }

    #[test]
    fn legacy_titles_are_not_implicitly_captions() {
        let style: crate::model::TextStyle =
            serde_json::from_str(r#"{"content":"Title"}"#).unwrap();
        assert!(style.caption_group.is_none());
        assert!(!style.vertical, "existing text stays horizontal");
        assert!(
            !serde_json::to_value(style)
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key("captionGroup")
        );
    }

    #[test]
    fn text_clips_can_be_merged_combining_content() {
        let mut project = Project::new();
        let mut clip1 = Clip::blank("c1", "T1", ClipKind::Text, "Hello", 0.0, 2.0);
        clip1.text = Some(TextStyle {
            content: "Hello".into(),
            ..Default::default()
        });
        let mut clip2 = Clip::blank("c2", "T1", ClipKind::Text, "World", 2.0, 3.0);
        clip2.text = Some(TextStyle {
            content: "World".into(),
            ..Default::default()
        });
        project.active_mut().clips.push(clip1.into());
        project.active_mut().clips.push(clip2.into());

        let settings = DocumentSettings {
            canvas: [0, 0, 0, 255],
            name: "Text Merge".into(),
            width: 1920,
            height: 1080,
            rate_num: 30,
            rate_den: 1,
        };
        let mut editor = Editor::from_document(&to_document(&settings, &project)).unwrap();

        let outcome = editor
            .apply(Command::MergeClips {
                clip_ids: vec!["c1".into(), "c2".into()],
            })
            .unwrap();
        assert!(outcome.applied);
        assert_eq!(editor.project().active().clips.len(), 1);

        let merged = &editor.project().active().clips[0];
        assert_eq!(merged.id, "c1");
        assert_eq!(merged.start, 0.0);
        assert_eq!(merged.duration, 5.0);
        assert_eq!(merged.text.as_ref().unwrap().content, "Hello World");

        assert!(editor.undo());
        assert_eq!(editor.project().active().clips.len(), 2);
        assert_eq!(
            editor
                .project()
                .active()
                .clip("c1")
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .content,
            "Hello"
        );
        assert_eq!(
            editor
                .project()
                .active()
                .clip("c2")
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .content,
            "World"
        );

        assert!(editor.redo());
        assert_eq!(editor.project().active().clips.len(), 1);
        assert_eq!(
            editor.project().active().clips[0]
                .text
                .as_ref()
                .unwrap()
                .content,
            "Hello World"
        );
    }

    #[test]
    fn text_clip_can_be_split_at_cursor_index() {
        let mut project = Project::new();
        let mut clip = Clip::blank("c1", "T1", ClipKind::Text, "Hello World", 0.0, 4.0);
        clip.text = Some(TextStyle {
            content: "Hello World".into(),
            ..Default::default()
        });
        project.active_mut().clips.push(clip.into());

        let settings = DocumentSettings {
            canvas: [0, 0, 0, 255],
            name: "Text Split".into(),
            width: 1920,
            height: 1080,
            rate_num: 30,
            rate_den: 1,
        };
        let mut editor = Editor::from_document(&to_document(&settings, &project)).unwrap();

        // Split at timeline time 2.0 with cursor between "Hello" and "World" (char 5)
        let outcome = editor
            .apply(Command::SplitTextClip {
                clip_id: "c1".into(),
                time: 2.0,
                cursor_char_index: Some(5),
            })
            .unwrap();
        assert!(outcome.applied);
        assert_eq!(editor.project().active().clips.len(), 2);

        let head = editor.project().active().clip("c1").unwrap();
        assert_eq!(head.start, 0.0);
        assert_eq!(head.duration, 2.0);
        assert_eq!(head.text.as_ref().unwrap().content, "Hello");

        let tail_id = outcome.created_id.unwrap();
        let tail = editor.project().active().clip(&tail_id).unwrap();
        assert_eq!(tail.start, 2.0);
        assert_eq!(tail.duration, 2.0);
        assert_eq!(tail.text.as_ref().unwrap().content, "World");

        assert!(editor.undo());
        assert_eq!(editor.project().active().clips.len(), 1);
        assert_eq!(
            editor
                .project()
                .active()
                .clip("c1")
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .content,
            "Hello World"
        );

        assert!(editor.redo());
        assert_eq!(editor.project().active().clips.len(), 2);
    }

    #[test]
    fn batch_replace_captions_across_timeline() {
        let mut project = Project::new();
        let mut clip1 = Clip::blank("c1", "T1", ClipKind::Text, "Apple Pie", 0.0, 2.0);
        clip1.text = Some(TextStyle {
            content: "I like apple pie".into(),
            ..Default::default()
        });
        let mut clip2 = Clip::blank("c2", "T1", ClipKind::Text, "Apple Cider", 2.0, 2.0);
        clip2.text = Some(TextStyle {
            content: "Warm APPLE cider is great".into(),
            ..Default::default()
        });
        let mut clip3 = Clip::blank("c3", "T1", ClipKind::Text, "Orange Juice", 4.0, 2.0);
        clip3.text = Some(TextStyle {
            content: "Orange juice only".into(),
            ..Default::default()
        });
        project.active_mut().clips.push(clip1.into());
        project.active_mut().clips.push(clip2.into());
        project.active_mut().clips.push(clip3.into());

        let settings = DocumentSettings {
            canvas: [0, 0, 0, 255],
            name: "Batch Replace".into(),
            width: 1920,
            height: 1080,
            rate_num: 30,
            rate_den: 1,
        };
        let mut editor = Editor::from_document(&to_document(&settings, &project)).unwrap();

        // Case-insensitive search for "apple" -> "cherry"
        let outcome = editor
            .apply(Command::BatchReplaceCaptions {
                search: "apple".into(),
                replacement: "cherry".into(),
                match_case: false,
            })
            .unwrap();
        assert!(outcome.applied);

        assert_eq!(
            editor
                .project()
                .active()
                .clip("c1")
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .content,
            "I like cherry pie"
        );
        assert_eq!(
            editor
                .project()
                .active()
                .clip("c2")
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .content,
            "Warm cherry cider is great"
        );
        assert_eq!(
            editor
                .project()
                .active()
                .clip("c3")
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .content,
            "Orange juice only"
        );

        assert!(editor.undo());
        assert_eq!(
            editor
                .project()
                .active()
                .clip("c1")
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .content,
            "I like apple pie"
        );
        assert_eq!(
            editor
                .project()
                .active()
                .clip("c2")
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .content,
            "Warm APPLE cider is great"
        );

        assert!(editor.redo());
        assert_eq!(
            editor
                .project()
                .active()
                .clip("c1")
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .content,
            "I like cherry pie"
        );
    }
}
