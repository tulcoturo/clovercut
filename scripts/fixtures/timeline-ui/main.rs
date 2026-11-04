// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors
use slint::ComponentHandle;
slint::include_modules!();
fn drag(app: &TimelineProbe, from: (f32, f32), to: (f32, f32)) {
    use slint::platform::{PointerEventButton, WindowEvent};
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(from.0, from.1),
    });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: slint::LogicalPosition::new(from.0, from.1),
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(to.0, to.1),
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: slint::LogicalPosition::new(to.0, to.1),
        button: PointerEventButton::Left,
    });
}
fn main() {
    i_slint_backend_testing::init_no_event_loop();
    let app = TimelineProbe::new().unwrap();
    app.window().set_size(slint::PhysicalSize::new(500, 200));
    app.show().unwrap();
    drag(&app, (26.5, 42.5), (126.5, 42.5));
    assert!(
        (app.get_fade_in() - 1.0).abs() < 0.01,
        "fade-in follows inward drag"
    );
    assert_eq!((app.get_starts(), app.get_finishes()), (1, 1));
    drag(&app, (413.5, 42.5), (313.5, 42.5));
    assert!(
        (app.get_fade_out() - 1.0).abs() < 0.01,
        "fade-out follows inward drag"
    );
    assert_eq!((app.get_starts(), app.get_finishes()), (2, 2));
    drag(&app, (120.0, 42.5), (420.0, 42.5));
    assert_eq!(app.get_fade_in(), 2.0, "fade is capped at half the clip");
    drag(&app, (212.0, 42.5), (12.0, 42.5));
    assert_eq!(
        app.get_fade_in(),
        0.0,
        "dragging back to the edge removes the fade"
    );
    app.hide().unwrap();
    for mode in 0..4 {
        let app = TimelineProbe::new().unwrap();
        app.set_locked(mode == 0);
        app.set_finger(mode == 1);
        app.set_windowed(mode == 2);
        if mode == 3 {
            app.set_tool(TimelineTool::Split);
        }
        app.window().set_size(slint::PhysicalSize::new(500, 200));
        app.show().unwrap();
        drag(&app, (26.5, 42.5), (126.5, 42.5));
        assert_eq!(app.get_changes(), 0, "mode {mode} must not edit fades");
        assert_eq!(app.get_starts(), 0);
        app.hide().unwrap();
    }
    println!("timeline UI: fade-in/out drag, lock, touch, windowing and split tool passed");
}
