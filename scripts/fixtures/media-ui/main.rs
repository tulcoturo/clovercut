// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};
slint::include_modules!();
fn click(app: &MediaProbe, x: f32, y: f32) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let position = slint::LogicalPosition::new(x, y);
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
fn held(app: &MediaProbe, key: slint::platform::Key, pressed: bool) {
    use slint::platform::WindowEvent;
    app.window().dispatch_event(if pressed {
        WindowEvent::KeyPressed { text: key.into() }
    } else {
        WindowEvent::KeyReleased { text: key.into() }
    });
}
fn press(app: &MediaProbe, text: slint::SharedString) {
    use slint::platform::WindowEvent;
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}
fn chord(app: &MediaProbe, key: &str) {
    held(app, slint::platform::Key::Control, true);
    press(app, key.into());
    held(app, slint::platform::Key::Control, false);
}
fn main() {
    i_slint_backend_testing::init_no_event_loop();
    let app = MediaProbe::new().unwrap();
    let items: Vec<MediaItemData> = (1..=4)
        .map(|id| MediaItemData {
            id,
            name: format!("File {id}").into(),
            ..Default::default()
        })
        .collect();
    app.set_items(Rc::new(slint::VecModel::from(items.clone())).into());
    let events = Rc::new(RefCell::new(Vec::new()));
    let binding = Rc::new(RefCell::new(String::from("a")));
    let capture = events.clone();
    let bound = binding.clone();
    app.on_bin_key(move |which, text, shift, alt, control, meta| {
        if !shift && !alt && !meta && control && text.as_str() == *bound.borrow() {
            capture.borrow_mut().push((which, "all"));
            true
        } else if text == slint::SharedString::from(slint::platform::Key::Delete) {
            capture.borrow_mut().push((which, "delete"));
            true
        } else {
            false
        }
    });
    app.show().unwrap();
    click(&app, 45.0, 45.0);
    assert_eq!(app.get_picked_row(), 1);
    assert!(!app.get_additive() && !app.get_range());
    chord(&app, "a");
    assert_eq!(
        &*events.borrow(),
        &[(1, "all")],
        "card click retains grid focus after host blur"
    );
    assert_eq!(app.get_window_keys(), 0);
    held(&app, slint::platform::Key::Shift, true);
    click(&app, 45.0, 135.0);
    held(&app, slint::platform::Key::Shift, false);
    assert_eq!(app.get_picked_row(), 4);
    assert!(app.get_range() && !app.get_additive());
    held(&app, slint::platform::Key::Control, true);
    click(&app, 45.0, 45.0);
    held(&app, slint::platform::Key::Control, false);
    assert!(app.get_additive() && !app.get_range());
    held(&app, slint::platform::Key::Shift, true);
    held(&app, slint::platform::Key::Control, true);
    click(&app, 45.0, 135.0);
    held(&app, slint::platform::Key::Control, false);
    held(&app, slint::platform::Key::Shift, false);
    assert!(app.get_additive() && app.get_range());
    held(&app, slint::platform::Key::Meta, true);
    click(&app, 45.0, 45.0);
    held(&app, slint::platform::Key::Meta, false);
    assert!(app.get_additive() && !app.get_range());
    // Background presses take focus as well; a second dock seat owns its own keys.
    click(&app, 650.0, 240.0);
    chord(&app, "a");
    assert_eq!(events.borrow().last(), Some(&(2, "all")));
    *binding.borrow_mut() = "j".into();
    chord(&app, "j");
    assert_eq!(events.borrow().last(), Some(&(2, "all")));
    let before = events.borrow().len();
    chord(&app, "a");
    assert_eq!(
        events.borrow().len(),
        before,
        "the old binding is not hard-coded in the grid"
    );
    press(&app, slint::platform::Key::Delete.into());
    assert_eq!(events.borrow().last(), Some(&(2, "delete")));
    app.set_items(Rc::new(slint::VecModel::<MediaItemData>::default()).into());
    chord(&app, "j");
    assert_eq!(
        events.borrow().len(),
        before + 2,
        "emptying a shelf preserves its keyboard owner"
    );
    let fallback = app.get_window_keys();
    press(&app, slint::platform::Key::Delete.into());
    assert_eq!(events.borrow().last(), Some(&(2, "delete")));
    assert_eq!(
        app.get_window_keys(),
        fallback,
        "Delete on an empty bin does not reach timeline shortcuts"
    );
    let before = events.borrow().len();
    click(&app, 45.0, 370.0);
    chord(&app, "a");
    assert_eq!(events.borrow().len(), before, "a text field keeps Ctrl+A");
    assert_eq!(
        (app.get_text_anchor() - app.get_text_cursor()).unsigned_abs() as usize,
        "ação 中文 🙂 text".len(),
        "the native text selection includes full UTF-8 words"
    );
    click(&app, 440.0, 390.0);
    let before_timeline = app.get_timeline_keys();
    chord(&app, "a");
    assert!(app.get_timeline_keys() > before_timeline);
    assert_eq!(
        events.borrow().len(),
        before,
        "timeline focus does not act on the bin"
    );
    app.set_items(Rc::new(slint::VecModel::from(items)).into());
    app.set_pane(true);
    let card = i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "MediaCard::card")
        .next()
        .expect("real pane card");
    card.mock_single_click(slint::platform::PointerEventButton::Left);
    chord(&app, "j");
    assert_eq!(
        events.borrow().last(),
        Some(&(3, "all")),
        "the real pane routes its focused grid"
    );
    let select =
        i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "Select::trigger")
            .next()
            .expect("sort dropdown");
    select.mock_single_click(slint::platform::PointerEventButton::Left);
    press(&app, slint::platform::Key::Escape.into());
    let before = events.borrow().len();
    chord(&app, "j");
    assert_eq!(
        events.borrow().len(),
        before + 1,
        "sort control focus remains in the media pane"
    );
    assert_eq!(events.borrow().last(), Some(&(3, "all")));
    let audio_tab =
        i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "LibraryTab::touch")
            .nth(1)
            .expect("Audio tab");
    audio_tab.mock_single_click(slint::platform::PointerEventButton::Left);
    let before = events.borrow().len();
    let fallback = app.get_window_keys();
    chord(&app, "j");
    assert_eq!(
        events.borrow().len(),
        before,
        "another library page does not select media"
    );
    assert_eq!(
        app.get_window_keys(),
        fallback + 1,
        "changing pages preserves window shortcuts"
    );
    i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "LibraryTab::touch")
        .next()
        .unwrap()
        .mock_single_click(slint::platform::PointerEventButton::Left);
    chord(&app, "j");
    assert_eq!(events.borrow().len(), before + 1);
    assert_eq!(events.borrow().last(), Some(&(3, "all")));
    app.hide().unwrap();
    println!(
        "media grid: focus after blur, modifier separation, two seats, rebound chords, empty shelves, text and timeline isolation passed"
    );
}
