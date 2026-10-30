// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Standalone observation of the actual owned-audio destination/content scan.
//! Compile with rustc --edition 2024 -O scripts/bench_owned_audio.rs -o /tmp/audio-bench
//! Run /tmp/audio-bench SOURCE IDENTICAL_OWNED_COPY [REPETITIONS]. Neither is modified.
//! Output is JSON-lines. This excludes copy, media probe and event-loop latency.
#[path = "../src/crates/concat/src/owned_audio.rs"]
mod owned_audio;

fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    assert!(
        (3..=4).contains(&args.len()),
        "expected SOURCE OWNED_COPY [REPETITIONS]"
    );
    let source = std::path::Path::new(&args[1]);
    let target = std::path::Path::new(&args[2]);
    let repetitions: usize = args
        .get(3)
        .map(|arg| arg.to_str().unwrap().parse().unwrap())
        .unwrap_or(5);
    assert!((1..=100).contains(&repetitions));
    assert!(
        owned_audio::matches_copy(source, target).unwrap(),
        "copies must have identical contents"
    );
    let stem = target.file_stem().unwrap().to_str().unwrap();
    let extension = target
        .extension()
        .map(|value| format!(".{}", value.to_str().unwrap()))
        .unwrap_or_default();
    let folder = target.parent().unwrap();
    println!(
        "{{\"event\":\"context\",\"file_bytes\":{},\"warmup_comparisons\":1,\"cache_state\":\"uncontrolled repeated reads\"}}",
        source.metadata().unwrap().len()
    );
    for run in 0..repetitions {
        let started = std::time::Instant::now();
        let chosen = owned_audio::destination(source, folder, stem, &extension);
        assert_eq!(chosen, target);
        println!(
            "{{\"event\":\"sample\",\"run\":{},\"ms\":{:.6}}}",
            run + 1,
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
}
