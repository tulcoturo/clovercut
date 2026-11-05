// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors
use slint::ComponentHandle;
slint::include_modules!();
fn key(app: &CaptionsProbe, text: slint::SharedString) {
    app.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.clone() });
    app.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
}
fn main() {
    i_slint_backend_testing::init_no_event_loop();
    for dialog in [false, true] {
        for fixed in [false, true] {
            let app = CaptionsProbe::new().unwrap();
            app.set_dialog(dialog);
            app.set_fixed(fixed);
            app.window().set_size(slint::PhysicalSize::new(800, 900));
            let library = app.global::<CaptionsLibrary>();
            library.set_models(std::rc::Rc::new(slint::VecModel::from(vec!["base".into()])).into());
            let choices: Vec<slint::SharedString> = if fixed {
                vec!["English".into()]
            } else {
                vec![
                    "Auto-detect".into(),
                    "Bahasa Indonesia".into(),
                    "Português".into(),
                ]
            };
            library.set_languages(std::rc::Rc::new(slint::VecModel::from(choices)).into());
            library.set_language_fixed(fixed);
            library.set_ready(true);
            let weak = app.as_weak();
            library.on_language_changed(move |index| {
                let app = weak.unwrap();
                app.set_language(index);
                app.set_changes(app.get_changes() + 1);
                app.global::<CaptionsLibrary>().set_language(index);
            });
            app.show().unwrap();
            let control = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &app,
                "captions.language",
            )
            .find(|element| {
                element.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Combobox)
            })
            .expect("language selection is present in sheet and inline panel");
            control.mock_single_click(slint::platform::PointerEventButton::Left);
            key(&app, slint::platform::Key::DownArrow.into());
            key(&app, slint::platform::Key::Return.into());
            if fixed {
                assert_eq!(app.get_changes(), 0, "English-only list cannot change");
            } else {
                assert_eq!(app.get_language(), 1, "explicit language can be selected");
            }
            app.hide().unwrap();
        }
    }
    println!("captions UI: language selection in sheet and inline panel, English-only lock passed");
}
