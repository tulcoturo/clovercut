// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

use i_slint_backend_testing::ElementHandle;
use slint::ComponentHandle;
slint::include_modules!();
fn key(app: &HomeProbe, text: slint::SharedString) {
    app.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.clone() });
    app.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
}
fn app_with_count(count: usize) -> HomeProbe {
    let app = HomeProbe::new().unwrap();
    app.set_projects(
        std::rc::Rc::new(slint::VecModel::from(
            (0..count)
                .map(|i| RecentProjectData {
                    name: format!("Project {i}").into(),
                    path: format!("/synthetic/{i}").into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        ))
        .into(),
    );
    app.show().unwrap();
    app
}
fn app() -> HomeProbe {
    app_with_count(9)
}
fn find(app: &HomeProbe, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(app, label)
        .next()
        .unwrap_or_else(|| panic!("missing {label}"))
}

fn snapshot(app: &HomeProbe, name: &str) -> slint::SharedPixelBuffer<slint::Rgba8Pixel> {
    let frame = app.window().take_snapshot().unwrap();
    if let Some(directory) = std::env::var_os("CLOVERCUT_UI_SNAPSHOT_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let mut image = format!("P6\n{} {}\n255\n", frame.width(), frame.height()).into_bytes();
        for pixel in frame.as_bytes().chunks_exact(4) {
            image.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(directory.join(format!("{name}.ppm")), image).unwrap();
    }
    frame
}

fn main() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("software".into()),
        },
    )))
    .unwrap();
    for (width, height, scale) in [
        (1100, 900, 1.0),
        (640, 360, 1.0),
        (800, 450, 1.25),
        (360, 600, 1.0),
    ] {
        for custom in [false, true] {
            let app = HomeProbe::new().unwrap();
            app.set_project_test(true);
            app.set_custom_project(custom);
            app.set_probe_width(width as f32 / scale);
            app.set_probe_height(height as f32 / scale);
            app.show().unwrap();
            app.window()
                .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
                    scale_factor: scale,
                });
            app.window()
                .set_size(slint::PhysicalSize::new(width, height));
            let logical_w = width as f32 / scale;
            let logical_h = height as f32 / scale;
            app.window()
                .dispatch_event(slint::platform::WindowEvent::Resized {
                    size: slint::LogicalSize::new(logical_w, logical_h),
                });
            // Force a real layout/paint before inspecting conditional controls.
            snapshot(
                &app,
                &format!("project-{width}-{height}-{scale}-{custom}-top"),
            );
            // The shape controls have separate non-overlapping rectangles.
            let shapes: Vec<_> = ["16:9", "9:16", "1:1", "4:3", "4:5", "3:4"]
                .iter()
                .map(|label| find(&app, label))
                .collect();
            for (i, shape) in shapes.iter().enumerate() {
                let at = shape.absolute_position();
                let size = shape.size();
                let card_width = 640.0_f32.min(logical_w - 32.0);
                let card_left = (logical_w - card_width) / 2.0;
                assert!(
                    at.x >= card_left && at.x + size.width <= card_left + card_width - 10.0,
                    "shape must fit the card at {width}x{height}/{scale}: {at:?} {size:?}"
                );
                for other in &shapes[i + 1..] {
                    let b = other.absolute_position();
                    let bs = other.size();
                    assert!(
                        at.x + size.width <= b.x + 0.1
                            || b.x + bs.width <= at.x + 0.1
                            || at.y + size.height <= b.y + 0.1
                            || b.y + bs.height <= at.y + 0.1
                    );
                }
            }
            let previews: Vec<_> = ElementHandle::find_by_element_type_name(&app, "FrameShape")
                .filter(|preview| preview.size().width >= 30.0)
                .collect();
            assert!(previews.len() <= 1);
            if width == 1100 {
                assert_eq!(previews.len(), 1);
            }
            for preview in previews {
                let b = preview.absolute_position();
                let bs = preview.size();
                for shape in &shapes {
                    let at = shape.absolute_position();
                    let size = shape.size();
                    assert!(
                        at.x + size.width <= b.x + 0.1
                            || b.x + bs.width <= at.x + 0.1
                            || at.y + size.height <= b.y + 0.1
                            || b.y + bs.height <= at.y + 0.1,
                        "the frame preview must not cover a shape control"
                    );
                }
            }
            if !custom {
                shapes[4].invoke_accessible_default_action();
                assert_eq!(app.get_aspect(), 4);
            }
            // Scroll at the form's padding, away from sliders/selects.
            let x = (logical_w - 640.0_f32.min(logical_w - 32.0)) / 2.0 + 8.0;
            let scroll = |delta_y| {
                app.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(x, logical_h / 2.0),
                        delta_x: 0.0,
                        delta_y,
                    })
            };
            scroll(-200.0);
            snapshot(
                &app,
                &format!("project-{width}-{height}-{scale}-{custom}-middle"),
            );
            // Compact forms intentionally put Size below the first viewport.
            // Inspect the controls after bringing that section into view.
            let sizes: Vec<_> = ["720p", "1080p", "2K", "4K", "common.custom"]
                .iter()
                .map(|label| find(&app, label))
                .collect();
            for (i, size) in sizes.iter().enumerate() {
                let at = size.absolute_position();
                let extent = size.size();
                let card_width = 640.0_f32.min(logical_w - 32.0);
                let card_left = (logical_w - card_width) / 2.0;
                assert!(at.x >= card_left && at.x + extent.width <= card_left + card_width - 10.0);
                for other in &sizes[i + 1..] {
                    let b = other.absolute_position();
                    let bs = other.size();
                    assert!(
                        at.x + extent.width <= b.x + 0.1
                            || b.x + bs.width <= at.x + 0.1
                            || at.y + extent.height <= b.y + 0.1
                            || b.y + bs.height <= at.y + 0.1
                    );
                }
            }
            scroll(-10000.0);
            snapshot(
                &app,
                &format!("project-{width}-{height}-{scale}-{custom}-bottom"),
            );
            let create = find(&app, "start.create");
            let at = create.absolute_position();
            let size = create.size();
            assert!(
                at.y >= 42.0 && at.y + size.height <= logical_h,
                "Create must be reachable at {width}x{height}/{scale}, custom={custom}: {at:?} {size:?}"
            );
            create.mock_single_click(slint::platform::PointerEventButton::Left);
            assert_eq!(
                app.get_creates(),
                1,
                "the visible Create button accepts a real click"
            );
            app.set_creating_project(true);
            let busy = find(&app, "start.creating");
            assert_eq!(busy.accessible_enabled(), Some(false));
            busy.invoke_accessible_default_action();
            assert_eq!(app.get_creates(), 1, "a busy form cannot create twice");
            app.hide().unwrap();
        }
    }
    println!(
        "new project UI: compact/scaled windows, shape cells, custom fields/errors and reachable Create passed"
    );
    let hover = self::app();
    hover.window().set_size(slint::PhysicalSize::new(1100, 900));
    let before_hover = snapshot(&hover, "recent-before-hover");
    let card = find(&hover, "Project 0");
    let at = card.absolute_position();
    hover
        .window()
        .dispatch_event(slint::platform::WindowEvent::PointerMoved {
            position: slint::LogicalPosition::new(at.x + 20.0, at.y + 20.0),
        });
    snapshot(&hover, "recent-card-hover");
    for (label, reveal) in [
        ("start.showInFolder", true),
        ("start.removeFromList", false),
    ] {
        println!("home UI: checking painted poster action {label}");
        let action = find(&hover, label);
        let at = action.absolute_position();
        let size = action.size();
        let position =
            slint::LogicalPosition::new(at.x + size.width / 2.0, at.y + size.height / 2.0);
        for _ in 0..20 {
            hover
                .window()
                .dispatch_event(slint::platform::WindowEvent::PointerMoved { position });
            i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(16));
            let frame = snapshot(&hover, &format!("recent-action-{label}"));
            let mut changed = 0;
            let stride = frame.width() as usize;
            for y in at.y as usize..(at.y + size.height) as usize {
                for x in at.x as usize..(at.x + size.width) as usize {
                    let index = y * stride + x;
                    changed +=
                        usize::from(frame.as_slice()[index] != before_hover.as_slice()[index]);
                }
            }
            assert!(
                changed > 16,
                "the action must remain visibly painted under the pointer"
            );
            find(&hover, label);
        }
        find(&hover, label).mock_single_click(slint::platform::PointerEventButton::Left);
        assert_eq!(
            hover.get_opens(),
            0,
            "a poster action must not open its project"
        );
        assert_eq!(
            if reveal {
                hover.get_reveals()
            } else {
                hover.get_forgets()
            },
            1
        );
    }
    let reveal = find(&hover, "start.showInFolder");
    let forget = find(&hover, "start.removeFromList");
    hover.set_busy(true);
    for action in [reveal, forget] {
        assert_eq!(action.accessible_enabled(), Some(false));
        action.invoke_accessible_default_action();
    }
    assert_eq!(
        (hover.get_opens(), hover.get_reveals(), hover.get_forgets()),
        (0, 1, 1),
        "persistent action controls must respect the home's busy guard"
    );
    hover.hide().unwrap();
    println!("home UI: painted poster actions remain present on hover and accept clicks");
    let themed = HomeProbe::new().unwrap();
    let theme = themed.global::<Theme>();
    for dark in [false, true] {
        theme.set_dark(dark);
        for index in 0..6 {
            theme.set_accent_choice(index);
            theme.set_playhead_choice(index);
            assert!(!theme.get_playhead_is_custom());
            let expected = if index == 0 {
                theme.get_playhead_default()
            } else {
                theme.get_accent()
            };
            assert_eq!(theme.get_playhead(), expected);
        }
        let custom = slint::Color::from_rgb_u8(23, 45, 67);
        theme.set_playhead_custom(custom);
        theme.set_playhead_choice(6);
        assert!(theme.get_playhead_is_custom());
        assert_eq!(theme.get_playhead(), custom);
        theme.set_playhead_choice(0);
        assert_eq!(theme.get_playhead(), theme.get_playhead_default());
    }
    println!("theme UI: named playhead colours, custom colour and reset passed in both palettes");

    let wheel = HomeProbe::new().unwrap();
    wheel.set_wheel_test(true);
    wheel.window().set_size(slint::PhysicalSize::new(1100, 900));
    wheel.show().unwrap();
    let area = find(&wheel, "wheel-test");
    let at = area.absolute_position();
    let scroll = |delta_y| {
        wheel
            .window()
            .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(at.x + 50.0, at.y + 50.0),
                delta_x: 0.0,
                delta_y,
            });
    };
    for expected in [-40.0, -80.0, -120.0] {
        scroll(-40.0);
        assert_eq!(
            wheel.get_wheel_offset(),
            expected,
            "wheel updates immediately"
        );
    }
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(250));
    assert_eq!(
        wheel.get_wheel_offset(),
        -120.0,
        "no delayed wheel animation"
    );
    scroll(-2000.0);
    assert_eq!(wheel.get_wheel_offset(), -800.0, "clamped at bottom");
    scroll(30.0);
    assert_eq!(
        wheel.get_wheel_offset(),
        -770.0,
        "reverses without dead travel"
    );
    scroll(2000.0);
    assert_eq!(wheel.get_wheel_offset(), 0.0, "clamped at top");
    wheel.hide().unwrap();
    println!("wheel UI: immediate repeated ticks, rest, limits and reversal passed");
    for width in [1100, 600] {
        for count in [1, 8, 9] {
            let app = app_with_count(count);
            app.set_query("".into());
            app.set_probe_width(width as f32);
            app.set_probe_height(900.0);
            app.window().set_size(slint::PhysicalSize::new(width, 900));
            find(&app, "start.searchProjects");
            find(&app, "start.sortName").invoke_accessible_default_action();
            assert_eq!(
                app.get_sort(),
                1,
                "name sorting is available at {width}px with {count} projects"
            );
            find(&app, "start.sortRecent").invoke_accessible_default_action();
            assert_eq!(app.get_sort(), 0);
            if width == 1100 {
                find(&app, "start.listView").invoke_accessible_default_action();
                assert_eq!(app.get_view(), 1);
                find(&app, "start.gridView").invoke_accessible_default_action();
                assert_eq!(app.get_view(), 0);
            }
            app.hide().unwrap();
        }
    }
    let empty = app_with_count(0);
    empty.set_query("".into());
    assert!(
        ElementHandle::find_by_accessible_label(&empty, "start.searchProjects")
            .next()
            .is_none()
    );
    empty.hide().unwrap();
    println!(
        "home UI: small-library search, sorting and icon view controls passed at wide/narrow widths"
    );
    let app = app();
    let clear = find(&app, "start.clearSearch");
    clear.invoke_accessible_default_action();
    assert_eq!(app.get_query().as_str(), "");
    key(&app, "x".into());
    assert_eq!(
        app.get_query().as_str(),
        "x",
        "clearing restores input focus"
    );
    key(&app, slint::platform::Key::Escape.into());
    assert_eq!(app.get_query().as_str(), "");
    app.hide().unwrap();
    println!("home UI: accessible clear, focus restoration and Escape passed");
    for activation in [slint::platform::Key::Return.into(), " ".into()] {
        let app = self::app();
        find(&app, "start.searchProjects")
            .mock_single_click(slint::platform::PointerEventButton::Left);
        key(&app, slint::platform::Key::Tab.into());
        key(&app, activation);
        assert_eq!(
            app.get_query().as_str(),
            "",
            "Tab reaches clear; Enter/Space activates it"
        );
        app.hide().unwrap();
    }
    println!("home UI: Tab/Enter/Space clear passed");
    let app = self::app();
    let project = find(&app, "Project 0");
    project.invoke_accessible_default_action();
    assert_eq!(app.get_opens(), 1);
    app.set_busy(true);
    assert_eq!(project.accessible_enabled(), Some(false));
    project.invoke_accessible_default_action();
    assert_eq!(
        app.get_opens(),
        1,
        "disabled cards refuse accessible activation"
    );
    println!("home UI: disabled card activation passed");
}
