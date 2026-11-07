// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors
use slint::ComponentHandle;
slint::include_modules!();
fn event(app: &ScrollProbe, x: f32, pressed: bool) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let position = slint::LogicalPosition::new(x, 10.0);
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    if pressed {
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        });
    }
}
fn ticks(n: usize) {
    for _ in 0..n {
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(16));
    }
}
fn release(app: &ScrollProbe, x: f32) {
    app.window()
        .dispatch_event(slint::platform::WindowEvent::PointerReleased {
            position: slint::LogicalPosition::new(x, 10.0),
            button: slint::platform::PointerEventButton::Left,
        });
}
fn main() {
    i_slint_backend_testing::init_no_event_loop();
    let app = ScrollProbe::new().unwrap();
    app.show().unwrap();
    event(&app, 200.0, true);
    event(&app, 461.0, false);
    ticks(1);
    let slow = app.get_left();
    assert!(slow > 0.0 && slow < 0.3, "inner edge creeps: {slow}");
    event(&app, 600.0, false);
    ticks(1);
    let fast = app.get_left() - slow;
    assert!(
        fast > slow * 5.0,
        "far edge runs faster: slow {slow}, fast {fast}"
    );
    ticks(400);
    assert!(
        (app.get_left() - 562.5).abs() < 0.01,
        "bound uses absolute duration minus origin: {}",
        app.get_left()
    );
    ticks(20);
    assert!((app.get_left() - 562.5).abs() < 0.01, "stops past content");
    release(&app, 600.0);
    app.set_left(-5399.9);
    event(&app, 200.0, true);
    event(&app, -100.0, false);
    ticks(10);
    assert_eq!(
        app.get_left(),
        -5400.0,
        "left bound is absolute zero, not local zero"
    );
    release(&app, -100.0);
    app.window()
        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
            position: slint::LogicalPosition::new(250.0, 185.0),
            delta_x: 0.0,
            delta_y: -30.0,
        });
    assert_eq!(app.get_bar_offset(), 30.0);
    app.window()
        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
            position: slint::LogicalPosition::new(250.0, 185.0),
            delta_x: -2000.0,
            delta_y: 0.0,
        });
    assert_eq!(app.get_bar_offset(), 1000.0, "wheel clamps overflow");
    println!(
        "actual timeline UI: gradual edge speed, absolute bounds at shifted origin, and horizontal scrollbar wheel passed"
    );
}
