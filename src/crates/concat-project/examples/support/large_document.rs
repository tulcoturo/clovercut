// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Shared document shape for pure and native performance probes.
use concat_project::commands::{ClipPatch, NewMedia};
use concat_project::model::{AppliedFilter, TextStyle};
use concat_project::{Command, DocumentSettings, Editor};
use serde_json::Value;

pub fn build(count: usize, media: NewMedia, filter: &str) -> (Value, String, DocumentSettings) {
    let settings = DocumentSettings {
        name: "Synthetic document baseline".into(),
        width: 1920,
        height: 1080,
        rate_num: 30,
        rate_den: 1,
        canvas: [0, 0, 0, 255],
    };
    let mut editor = Editor::with_video(settings.video());
    let media_id = editor
        .apply(Command::AddMedia { item: media })
        .unwrap()
        .created_id
        .unwrap();
    let track_id = editor.project().active().tracks[0].id.clone();
    let mut first = None;
    let mut srt = String::new();
    let time = |s: usize| format!("{:02}:{:02}:{:02},000", s / 3600, s / 60 % 60, s % 60);
    for i in 0..count {
        let id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_id.clone(),
                start: i as f64 * 5.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();
        first.get_or_insert_with(|| id.clone());
        editor
            .apply(Command::UpdateClip {
                clip_id: id,
                patch: ClipPatch {
                    filters: Some(vec![AppliedFilter::new(filter)]),
                    ..Default::default()
                },
            })
            .unwrap();
        srt.push_str(&format!(
            "{}\n{} --> {}\nSynthetic caption {}\n\n",
            i + 1,
            time(i * 5),
            time(i * 5 + 2),
            i
        ));
    }
    editor
        .apply(Command::AddCaptionSource {
            name: "Synthetic captions.srt".into(),
            srt,
            style: TextStyle::default(),
            offset_y: 0.0,
            place_at: Some(0.0),
        })
        .unwrap();
    (editor.to_document(&settings), first.unwrap(), settings)
}
