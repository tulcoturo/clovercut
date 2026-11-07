// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Render the actual creative packages, exercising their spatial/time behavior.
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use concat_core::frame::Frame;
use concat_core::time::Rational;
use concat_effects::{At, Catalogue, Package};
use concat_render::{Compositor, FramePlan, PlannedLayer, WgpuCompositor, detached_clip};

const IDS: &[&str] = &[
    "camera-shake",
    "digital-glitch",
    "vhs-tape",
    "scanlines",
    "kaleidoscope",
    "tilt-shift",
    "light-leak",
    "neon-edges",
    "old-film",
];
const W: u32 = 480;
const H: u32 = 270;

fn picture() -> Frame {
    let mut frame = Frame::transparent(W, H);
    if let Some(path) = std::env::var_os("CLOVERCUT_EFFECTS_QA_RGBA") {
        let bytes = std::fs::read(path).expect("input RGBA");
        assert_eq!(bytes.len(), (W * H * 4) as usize);
        frame.pixels_mut().copy_from_slice(&bytes);
        return frame;
    }
    for y in 0..H {
        for x in 0..W {
            let grid = if (x / 12 + y / 12) % 2 == 0 { 35 } else { 170 };
            frame.set_pixel(
                x,
                y,
                [
                    (x * 220 / W + 20) as u8,
                    (y * 200 / H + 25) as u8,
                    grid,
                    255,
                ],
            );
        }
    }
    frame
}

fn draw(
    gpu: &mut WgpuCompositor,
    image: &Frame,
    package: Option<&Package>,
    values: &BTreeMap<String, f64>,
    time: Rational,
    start: Rational,
) -> Frame {
    let mut layer = PlannedLayer::picture(detached_clip(), Arc::new(image.clone()));
    layer.clip_start = start;
    if let Some(package) = package {
        layer.effects = vec![package.pass(values, None).expect("shader")];
    }
    gpu.render(&FramePlan {
        time,
        canvas: [0, 0, 0, 0],
        layers: vec![layer],
        ..FramePlan::empty(W, H)
    })
}

fn difference(a: &Frame, b: &Frame) -> f64 {
    a.pixels()
        .iter()
        .zip(b.pixels())
        .map(|(a, b)| a.abs_diff(*b) as f64)
        .sum::<f64>()
        / a.pixels().len() as f64
}

fn ppm(path: &Path, frame: &Frame) {
    let mut data = format!("P6\n{} {}\n255\n", W, H).into_bytes();
    for px in frame.pixels().chunks_exact(4) {
        data.extend_from_slice(&px[..3]);
    }
    std::fs::write(path, data).expect("write preview");
}

fn main() {
    let catalogue = Catalogue::builtin();
    let mut gpu = WgpuCompositor::new().expect("a real GPU or software adapter is required");
    println!("Adapter: {:?}", gpu.adapter_info());
    // Build every builtin pipeline on this adapter, including WARP/FXC. A
    // canary after each package attributes a device loss to its actual cause.
    let canary = catalogue
        .get("concat.exposure")
        .expect("exposure canary")
        .trial_pass()
        .expect("canary shader");
    let mut trials = 0;
    for package in catalogue.packages() {
        if let Some(pass) = package.trial_pass() {
            gpu.trial(&pass, std::time::Duration::from_secs(60))
                .unwrap_or_else(|error| panic!("{}: pipeline trial: {error}", package.id()));
            gpu.trial(&canary, std::time::Duration::from_secs(60))
                .unwrap_or_else(|error| {
                    panic!("{}: device unusable after shader: {error}", package.id())
                });
            trials += 1;
        } else if package.transition().is_some() {
            let cut = catalogue
                .transition_pass(package.id(), &BTreeMap::new(), 0.4)
                .expect("transition");
            gpu.probe_transition(&cut, [0.2, 0.3, 0.1, 1.0], [0.5, 0.1, 0.7, 1.0], 8, 0.0)
                .unwrap_or_else(|| panic!("{}: transition pipeline refused", package.id()));
            gpu.trial(&canary, std::time::Duration::from_secs(60))
                .unwrap_or_else(|error| {
                    panic!(
                        "{}: device unusable after transition: {error}",
                        package.id()
                    )
                });
            trials += 1;
        }
    }
    println!("{trials} builtin visual pipelines and device-health canaries passed");
    let output = std::env::var_os("CLOVERCUT_EFFECTS_QA_OUTPUT").map(std::path::PathBuf::from);
    if let Some(output) = &output {
        std::fs::create_dir_all(output).expect("output folder");
    }
    let image = picture();
    let empty = BTreeMap::new();
    let seconds = Rational::new(7, 10);
    let baseline = draw(&mut gpu, &image, None, &empty, seconds, Rational::ZERO);
    if let Some(output) = &output {
        ppm(&output.join("original.ppm"), &baseline);
    }
    let mut total_probes = 0;
    for id in IDS {
        let package = catalogue
            .get(&format!("concat.{id}"))
            .expect("built-in creative effect");
        for probe in &package.probes {
            let pass = package.probe_pass(probe).expect("probe pass");
            let colour = gpu
                .probe(&[pass], probe.input, 8, 0.0)
                .expect("float readback");
            probe
                .check(colour)
                .unwrap_or_else(|e| panic!("{id}: {}: {e}", probe.name));
            assert!(colour.iter().all(|v| v.is_finite()));
            total_probes += 1;
        }
        // Spatial checks catch a shader silently refused by a driver, something
        // uniform colour probes alone cannot distinguish from a valid no-op.
        let began = Instant::now();
        let rendered = draw(
            &mut gpu,
            &image,
            Some(package),
            &empty,
            seconds,
            Rational::ZERO,
        );
        assert!(!gpu.is_dead(), "{id}: device lost");
        let changed = difference(&baseline, &rendered);
        assert!(changed > 0.05, "{id}: no visible effect ({changed})");
        assert!(
            rendered.pixels().chunks_exact(4).all(|p| p[3] == 255),
            "{id}: opaque input gained holes"
        );
        if let Some(output) = &output {
            ppm(&output.join(format!("{id}.ppm")), &rendered);
        }
        if std::env::var_os("CLOVERCUT_EFFECTS_QA_ANIMATE").is_some()
            && let Some(output) = &output
        {
            for index in 0..24 {
                let frame = draw(
                    &mut gpu,
                    &image,
                    Some(package),
                    &empty,
                    Rational::new(index, 15),
                    Rational::ZERO,
                );
                ppm(&output.join(format!("{id}-{index:02}.ppm")), &frame);
            }
        }
        let off = [("amount".to_owned(), 0.0)].into_iter().collect();
        let disabled = draw(
            &mut gpu,
            &image,
            Some(package),
            &off,
            seconds,
            Rational::ZERO,
        );
        assert!(
            difference(&baseline, &disabled) < 0.1,
            "{id}: zero must be identity"
        );
        // Every legal slider extreme must compile, complete and keep coverage.
        for at in [At::Min, At::Max] {
            let values = package.params_at(at);
            let edge = draw(
                &mut gpu,
                &image,
                Some(package),
                &values,
                seconds,
                Rational::ZERO,
            );
            assert!(
                edge.pixels().chunks_exact(4).all(|p| p[3] == 255),
                "{id}: {at:?} made holes"
            );
        }
        // Effects with motion use clip-local time: moving the clip on the
        // timeline preserves the same picture at the same elapsed clip time.
        if [
            "camera-shake",
            "digital-glitch",
            "vhs-tape",
            "light-leak",
            "old-film",
        ]
        .contains(id)
        {
            let later = draw(
                &mut gpu,
                &image,
                Some(package),
                &empty,
                seconds + Rational::from_int(100),
                Rational::from_int(100),
            );
            assert!(
                difference(&rendered, &later) < 0.1,
                "{id}: absolute timeline phase leaked into clip"
            );
            let next = draw(
                &mut gpu,
                &image,
                Some(package),
                &empty,
                Rational::new(17, 10),
                Rational::ZERO,
            );
            assert!(
                difference(&rendered, &next) > 0.01,
                "{id}: motion is frozen"
            );
        }
        // A composited title/cutout is not turned into an opaque rectangle.
        let mut transparent = image.clone();
        for pixel in transparent.pixels_mut().chunks_exact_mut(4) {
            pixel[3] = 0;
        }
        let clear = draw(
            &mut gpu,
            &transparent,
            Some(package),
            &empty,
            seconds,
            Rational::ZERO,
        );
        assert!(
            clear.pixels().chunks_exact(4).all(|p| p[3] == 0),
            "{id}: transparent input gained coverage"
        );
        println!(
            "{id}: colour probes, spatial response {changed:.3}, zero, bounds, alpha and clip timing passed ({:?})",
            began.elapsed()
        );
    }
    // Actual stacking/order, as stored by the editor and consumed by both the
    // preview and export compositor. Different orders must produce different frames.
    let make_stack = |ids: &[&str]| {
        let mut layer = PlannedLayer::picture(detached_clip(), Arc::new(image.clone()));
        layer.effects = ids
            .iter()
            .map(|id| {
                catalogue
                    .get(id)
                    .expect("package")
                    .pass(&empty, None)
                    .expect("pass")
            })
            .collect();
        FramePlan {
            time: seconds,
            layers: vec![layer],
            ..FramePlan::empty(W, H)
        }
    };
    let a = gpu.render(&make_stack(&["concat.light-leak", "concat.neon-edges"]));
    let b = gpu.render(&make_stack(&["concat.neon-edges", "concat.light-leak"]));
    assert!(difference(&a, &b) > 0.01, "effect order was lost");
    println!(
        "{} effects; {total_probes} pinned float probes; stack ordering passed",
        IDS.len()
    );
}
