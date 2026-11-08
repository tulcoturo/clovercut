// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors
use i_slint_backend_testing::ElementHandle;
use slint::ComponentHandle;
slint::include_modules!();
fn paint(app: &TextProbe) {
    app.window().take_snapshot().unwrap();
}
fn find(app: &TextProbe, label: &str) -> ElementHandle {
    ElementHandle::find_by_element_type_name(app, "TextInput")
        .find(|element| {
            element
                .accessible_label()
                .is_some_and(|value| value == label)
        })
        .unwrap_or_else(|| panic!("missing {label}"))
}
fn id(app: &TextProbe, name: &str) -> ElementHandle {
    ElementHandle::find_by_element_id(app, name)
        .next()
        .unwrap_or_else(|| panic!("missing id {name}"))
}
fn click(app: &TextProbe, at: slint::LogicalPosition) {
    use slint::platform::{PointerEventButton, WindowEvent};
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: at });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: at,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: at,
        button: PointerEventButton::Left,
    });
}
fn key(app: &TextProbe, text: slint::SharedString) {
    use slint::platform::WindowEvent;
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}
fn wheel(app: &TextProbe, at: slint::LogicalPosition, delta_y: f32) {
    app.window()
        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
            position: at,
            delta_x: 0.0,
            delta_y,
        });
    paint(app);
}
fn long_words() -> String {
    (0..80)
        .map(|i| format!("Line {i}: ação — português, 中文, 🙂 and editable words\n"))
        .collect()
}
fn app(mode: i32) -> TextProbe {
    let app = TextProbe::new().unwrap();
    app.set_words(long_words().into());
    app.set_mode(mode);
    app.show().unwrap();
    paint(&app);
    app
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
    let probe = app(0);
    wheel(&probe, slint::LogicalPosition::new(40.0, 40.0), -240.0);
    assert!(probe.get_editor_scroll() < -100.0);
    click(&probe, slint::LogicalPosition::new(40.0, 40.0));
    key(&probe, slint::platform::Key::End.into());
    paint(&probe);
    probe.invoke_reveal_bottom();
    paint(&probe);
    assert!(probe.get_editor_scroll() < -500.0);
    probe.set_words("short".into());
    paint(&probe);
    assert_eq!(
        probe.get_editor_scroll(),
        0.0,
        "shrinking text clamps the old scroll"
    );
    let grip = id(&probe, "ResizeBox::drag");
    let p = grip.absolute_position();
    let s = grip.size();
    let at = slint::LogicalPosition::new(p.x + s.width / 2.0, p.y + s.height / 2.0);
    probe
        .window()
        .dispatch_event(slint::platform::WindowEvent::PointerMoved { position: at });
    paint(&probe);
    assert_eq!(
        probe.get_editor_height(),
        104.0,
        "hover alone must not resize the box"
    );
    grip.mock_drag(
        slint::LogicalPosition::new(at.x, at.y + 80.0),
        slint::platform::PointerEventButton::Left,
    );
    paint(&probe);
    assert!(probe.get_editor_height() > 160.0);
    probe.hide().unwrap();
    println!(
        "text primitive: direct wheel, caret reveal, scroll clamp and pressed-only resize passed"
    );
    for mode in [1, 3, 4, 5] {
        let probe = app(mode);
        let label = if mode == 1 {
            "textPanel.content"
        } else {
            "common.script"
        };
        let input = find(&probe, label);
        let at = input.absolute_position();
        let position = slint::LogicalPosition::new(at.x + 20.0, at.y + 10.0);
        click(&probe, position);
        let before = input.absolute_position().y;
        wheel(&probe, position, -180.0);
        assert!(
            input.absolute_position().y < before - 40.0,
            "mode {mode}: actual editor scrolls: before={before}, after={}",
            input.absolute_position().y
        );
        let value = input.accessible_value().expect("text value");
        assert_eq!(value.as_str(), long_words());
        if mode == 1 {
            input.set_accessible_value("ação 中文 🙂");
            paint(&probe);
            assert_eq!(probe.get_words().as_str(), "ação 中文 🙂");
            assert!(probe.get_edits() > 0);
            key(&probe, slint::platform::Key::End.into());
            probe
                .window()
                .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                    text: slint::platform::Key::Shift.into(),
                });
            key(&probe, slint::platform::Key::LeftArrow.into());
            probe
                .window()
                .dispatch_event(slint::platform::WindowEvent::KeyReleased {
                    text: slint::platform::Key::Shift.into(),
                });
            let anchor = probe.get_anchor() as usize;
            let cursor = probe.get_selection_cursor() as usize;
            let text = probe.get_words();
            assert!(text.is_char_boundary(anchor) && text.is_char_boundary(cursor));
            assert_eq!(
                anchor.abs_diff(cursor),
                4,
                "word styling still selects a complete UTF-8 emoji"
            );
            key(&probe, slint::platform::Key::Tab.into());
            paint(&probe);
            assert!(
                probe.get_commits() > 0,
                "title blur still commits its own draft"
            );
        }
        if mode == 5 {
            probe.set_taking(true);
            paint(&probe);
            assert_eq!(input.accessible_enabled(), Some(false));
            assert!(
                input.absolute_position().y >= before - 1.0,
                "recording resets prompt scroll"
            );
        }
        probe.hide().unwrap();
    }
    println!(
        "actual title/caption/speech/voiceover editors: long Unicode drafts, wheel, edit and recording guard passed"
    );
    for scrim in [false, true] {
        let probe = app(2);
        if scrim {
            click(&probe, slint::LogicalPosition::new(2.0, 2.0));
        } else {
            id(&probe, "Modal::shut").mock_single_click(slint::platform::PointerEventButton::Left);
        }
        assert_eq!(probe.get_dismisses(), 1);
        assert!(!probe.get_opened());
        probe.hide().unwrap();
    }
    println!("relink UI: header close and outside click reach the existing dismiss callback");
}
