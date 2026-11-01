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
fn app() -> HomeProbe {
    let app = HomeProbe::new().unwrap();
    app.set_projects(
        std::rc::Rc::new(slint::VecModel::from(
            (0..9)
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
fn find(app: &HomeProbe, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(app, label)
        .next()
        .unwrap_or_else(|| panic!("missing {label}"))
}
fn main() {
    i_slint_backend_testing::init_no_event_loop();
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
