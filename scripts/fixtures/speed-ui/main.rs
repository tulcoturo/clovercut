// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors
use i_slint_backend_testing::ElementHandle;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
slint::include_modules!();
fn click(app: &SpeedProbe, position: slint::LogicalPosition) {
    use slint::platform::{PointerEventButton, WindowEvent};
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}
fn main() {
    i_slint_backend_testing::init_no_event_loop();
    let app = SpeedProbe::new().unwrap();
    let ramp = app.global::<SpeedRamp>();
    ramp.set_usable(true);
    ramp.set_curved(true);
    ramp.set_points(ModelRc::new(VecModel::from(vec![
        SpeedPointData {
            index: 0,
            x: 0.0,
            y: 0.5,
            end: true,
        },
        SpeedPointData {
            index: 1,
            x: 0.5,
            y: 0.5,
            end: false,
        },
        SpeedPointData {
            index: 2,
            x: 1.0,
            y: 0.5,
            end: true,
        },
    ])));
    ramp.set_selected(ModelRc::new(VecModel::from(vec![false; 3])));
    let removed = Rc::new(RefCell::new(Vec::new()));
    let captured = removed.clone();
    ramp.on_remove(move |index| captured.borrow_mut().push(index));
    app.show().unwrap();
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(1));
    let mut handles: Vec<_> =
        ElementHandle::find_by_element_type_name(&app, "SpeedHandle").collect();
    assert_eq!(
        handles.len(),
        3,
        "the real inspector shows the curve's points"
    );
    handles.sort_by(|a, b| a.absolute_position().x.total_cmp(&b.absolute_position().x));
    let at = handles[1].absolute_position();
    let size = handles[1].size();
    let middle = slint::LogicalPosition::new(at.x + size.width / 2.0, at.y + size.height / 2.0);
    click(&app, middle);
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(80));
    click(&app, middle);
    assert_eq!(
        &*removed.borrow(),
        &[1],
        "double-click reaches the inspector's remove callback"
    );
    assert_eq!(app.get_commits(), 1, "removing a point is committed once");
    println!("Speed inspector graph-to-host removal and commit passed");
}
