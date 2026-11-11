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
    let mut data = format!("P6\n{} {}\n255\n", frame.width(), frame.height()).into_bytes();
    for px in frame.pixels().chunks_exact(4) {
        data.extend_from_slice(&px[..3]);
    }
    std::fs::write(path, data).expect("write preview");
}

/// Exercise dissolution with an analysed matte, not just the missing-mask probes.
fn check_disintegration(gpu: &mut WgpuCompositor, catalogue: &Catalogue) {
    use concat_core::subject::SubjectMask;

    let (w, h) = (96, 64);
    let mut image = Frame::transparent(w, h);
    image.fill([100, 80, 60, 192]);
    let matte = |left: u32, coverage: u8| {
        let mut gray = vec![0; (w * h) as usize];
        for y in 12..52 {
            for x in left..left + 20 {
                gray[(y * w + x) as usize] = coverage;
            }
        }
        Arc::new(SubjectMask::new(w, h, gray).expect("subject matte"))
    };
    let package = catalogue
        .get("concat.subject-disintegration")
        .expect("package");
    let mut draw = |progress: f64, direction: f64, mask, canvas, source: &Frame| {
        let mut values = package.params_at(At::Default);
        values.insert("progress".into(), progress);
        values.insert("direction".into(), direction);
        let mut pass = package.pass(&values, None).expect("shader");
        pass.subject_mask = mask;
        let mut layer = PlannedLayer::picture(detached_clip(), Arc::new(source.clone()));
        layer.effects = vec![pass];
        gpu.render(&FramePlan {
            canvas,
            layers: vec![layer],
            ..FramePlan::empty(w, h)
        })
    };
    let clear = [0; 4];
    let original = draw(0.0, 0.0, Some(matte(20, 255)), clear, &image);
    for direction in [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0, 360.0] {
        let untouched = draw(100.0, direction, None, clear, &image);
        assert_eq!(
            untouched.pixels(),
            original.pixels(),
            "missing matte is idle"
        );
        let finished = draw(100.0, direction, Some(matte(20, 255)), clear, &image);
        assert_eq!(
            finished.pixel(30, 32).unwrap()[3],
            0,
            "subject dissolves at {direction} degrees"
        );
        assert_eq!(
            finished.pixel(2, 2),
            original.pixel(2, 2),
            "background stays whole"
        );
        assert_eq!(
            finished.pixel(60, 32),
            original.pixel(60, 32),
            "no sparks remain at completion"
        );
    }
    let partial = draw(100.0, 45.0, Some(matte(20, 128)), clear, &image);
    assert!(
        partial.pixel(30, 32).unwrap()[3].abs_diff(96) <= 2,
        "soft matte keeps fractional coverage"
    );
    let moved = draw(100.0, 45.0, Some(matte(60, 255)), clear, &image);
    assert_eq!(
        moved.pixel(30, 32),
        original.pixel(30, 32),
        "old position is restored"
    );
    assert_eq!(
        moved.pixel(70, 32).unwrap()[3],
        0,
        "dissolution follows the matte"
    );
    let canvas = [20, 120, 180, 255];
    let composited = draw(100.0, 45.0, Some(matte(20, 255)), canvas, &image);
    let center = composited.pixel(30, 32).unwrap();
    assert!(
        center.iter().zip(canvas).all(|(a, b)| a.abs_diff(b) <= 2),
        "dissolved pixels reveal the canvas: {center:?}"
    );
    let mut transparent = image.clone();
    transparent.fill([0; 4]);
    let empty = draw(50.0, 45.0, Some(matte(20, 255)), clear, &transparent);
    assert!(
        empty.pixels().chunks_exact(4).all(|p| p[3] == 0),
        "an empty source cannot emit particles"
    );
    let halfway = draw(50.0, 45.0, Some(matte(20, 255)), clear, &image);
    assert_ne!(
        halfway.pixels(),
        original.pixels(),
        "intermediate dissolve is visible"
    );
    let interior_alpha: Vec<_> = (14..50)
        .flat_map(|y| (22..38).map(move |x| (x, y)))
        .map(|(x, y)| halfway.pixel(x, y).unwrap()[3])
        .collect();
    assert!(interior_alpha.contains(&0), "halfway has dissolved pixels");
    assert!(
        interior_alpha.iter().any(|a| *a > 150),
        "halfway still has subject pixels"
    );
    if let Some(dir) = std::env::var_os("CLOVERCUT_EFFECTS_QA_OUTPUT") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("probe folder");
        for progress in [0, 50, 100] {
            let frame = draw(progress as f64, 45.0, Some(matte(20, 255)), canvas, &image);
            ppm(
                &dir.join(format!("subject-disintegration-{progress}.ppm")),
                &frame,
            );
            let alpha = draw(progress as f64, 45.0, Some(matte(20, 255)), clear, &image);
            let mut coverage = Frame::transparent(w, h);
            for y in 0..h {
                for x in 0..w {
                    let a = alpha.pixel(x, y).unwrap()[3];
                    coverage.set_pixel(x, y, [a, a, a, 255]);
                }
            }
            ppm(
                &dir.join(format!("subject-disintegration-alpha-{progress}.ppm")),
                &coverage,
            );
        }
    }
    println!(
        "Subject disintegration: endpoints, directions, soft/moved matte, canvas and transparency passed"
    );
}

fn main() {
    let catalogue = Catalogue::builtin();
    let mut gpu = WgpuCompositor::new().expect("a real GPU or software adapter is required");
    println!("Adapter: {:?}", gpu.adapter_info());
    check_disintegration(&mut gpu, catalogue);
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
    // Exercise non-empty facial uniforms on both Vulkan and Windows WARP,
    // not only the empty-face package trials above.
    let face = Arc::new(
        concat_core::face::FaceFrame::new(
            vec![concat_core::face::Face {
                bounds: [0.25, 0.1, 0.5, 0.7],
                points: [
                    [0.38, 0.35],
                    [0.62, 0.35],
                    [0.5, 0.5],
                    [0.42, 0.62],
                    [0.58, 0.62],
                ],
                confidence: 0.95,
            }],
            W as f32 / H as f32,
        )
        .unwrap(),
    );
    for id in ["concat.face-sunglasses", "concat.face-dog"] {
        let package = catalogue.get(id).expect("face package");
        let render = |gpu: &mut WgpuCompositor, face: Option<Arc<concat_core::face::FaceFrame>>| {
            let mut pass = package.pass(&empty, None).expect("face shader");
            pass.face_frame = face;
            let mut layer = PlannedLayer::picture(detached_clip(), Arc::new(image.clone()));
            layer.effects = vec![pass];
            gpu.render(&FramePlan {
                time: seconds,
                canvas: [0; 4],
                layers: vec![layer],
                ..FramePlan::empty(W, H)
            })
        };
        let absent = render(&mut gpu, None);
        assert!(
            difference(&baseline, &absent) < 0.05,
            "{id}: no face must preserve the picture"
        );
        let decorated = render(&mut gpu, Some(Arc::clone(&face)));
        assert!(
            difference(&baseline, &decorated) > 0.1,
            "{id}: detected face has no visible accessory"
        );
        gpu.trial(&canary, std::time::Duration::from_secs(60))
            .unwrap_or_else(|error| panic!("{id}: device unusable after tracked face: {error}"));
        if let Some(output) = &output {
            ppm(&output.join(format!("{id}.ppm")), &decorated);
        }
        println!("{id}: missing face, tracked accessory and live-device canary passed");
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
