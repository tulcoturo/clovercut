// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Observational document benchmark. Run with --release; no media files are opened.
use concat_project::commands::{ClipPatch, NewMedia};
use concat_project::model::MediaKind;
use concat_project::{Command, DocumentSettings, Editor};
use serde_json::{Value, json};
use std::hint::black_box;
use std::time::Instant;

#[path = "support/large_document.rs"]
mod large_document;

fn fixture(count: usize) -> (Value, String, DocumentSettings) {
    large_document::build(
        count,
        NewMedia {
            path: "/synthetic/not-a-real-video.mp4".into(),
            name: "Synthetic video".into(),
            duration: Some(5.0),
            kind: MediaKind::Video,
            width: Some(1920),
            height: Some(1080),
            frame_rate: Some(30.0),
            frame_rate_fraction: Some("30/1".into()),
            video_codec: Some("h264".into()),
            audio_codec: None,
            has_audio: false,
            audio_tracks: vec![],
            origin: None,
            color_space: Default::default(),
        },
        "brightness",
    )
}

fn samples(name: &str, values: Vec<f64>) -> Value {
    let mut sorted = values.clone();
    sorted.sort_by(f64::total_cmp);
    json!({"scenario": name, "unit": "ms", "values": values,
        "mean": values.iter().sum::<f64>() / values.len() as f64,
        "p95": sorted[(values.len() * 95).div_ceil(100) - 1], "max": sorted.last().unwrap()})
}

fn baseline(count: usize) -> Value {
    let started = Instant::now();
    let (document, id, settings) = fixture(count);
    let fixture_ms = started.elapsed().as_secs_f64() * 1000.0;
    let bytes = serde_json::to_vec(&document).unwrap();
    let original = Editor::from_document(&document).unwrap();
    let before = original.project().clone();
    assert_eq!(before.active().clips.len(), count * 2);
    let mut parse = vec![];
    let mut save = vec![];
    let mut edit = vec![];
    for _ in 0..15 {
        let started = Instant::now();
        let value: Value = serde_json::from_slice(black_box(&bytes)).unwrap();
        let loaded = Editor::from_document(&value).unwrap();
        parse.push(started.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(loaded.project(), &before, "load must preserve the project");

        let started = Instant::now();
        let saved = serde_json::to_vec(&loaded.to_document(&settings)).unwrap();
        black_box(&saved);
        save.push(started.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(
            Editor::from_document(&serde_json::from_slice::<Value>(&saved).unwrap())
                .unwrap()
                .project(),
            &before
        );

        let mut editor = loaded;
        let started = Instant::now();
        editor
            .apply(Command::UpdateClip {
                clip_id: id.clone(),
                patch: ClipPatch {
                    opacity: Some(0.75),
                    ..Default::default()
                },
            })
            .unwrap();
        assert!(editor.undo());
        assert!(editor.redo());
        assert!(editor.undo());
        edit.push(started.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(
            editor.project(),
            &before,
            "edit history must restore the project"
        );
    }
    json!({"video_clips": count, "caption_cues": count, "filters_per_video_clip": 1,
        "total_clips": before.active().clips.len(), "json_bytes": bytes.len(), "fixture_ms": fixture_ms,
        "measurements": [samples("parse_json_and_load_document", parse), samples("serialize_document_to_json", save), samples("edit_undo_redo_undo", edit)]})
}

fn command(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("run this example with --release");
        std::process::exit(2);
    }
    let source = command("git", &["rev-parse", "HEAD"]);
    let dirty = command(
        "git",
        &["status", "--porcelain", "--untracked-files=normal"],
    )
    .map(|s| !s.is_empty());
    let workloads = [100, 500].map(baseline);
    println!("{}", serde_json::to_string_pretty(&json!({"schema_version": 1,
        "source_sha": source, "working_tree_dirty": dirty, "toolchain": command("rustc", &["--version"]),
        "os": std::env::consts::OS, "architecture": std::env::consts::ARCH,
        "machine_label": std::env::var("CLOVERCUT_BENCHMARK_MACHINE").ok(),
        "logical_parallelism": std::thread::available_parallelism().ok().map(std::num::NonZeroUsize::get),
        "recorded_at_unix_seconds": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        "optimized": !cfg!(debug_assertions), "samples_per_scenario": 15,
        "scope": "synthetic in-memory document operations; no file IO, media decoding, GPU, UI publication or presentation",
        "cache_state": "uncontrolled; repeated inputs", "performance_budget_qualified": false,
        "workloads": workloads})).unwrap());
}
