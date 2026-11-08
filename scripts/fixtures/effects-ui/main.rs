// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
slint::include_modules!();
fn point(app: &EffectsProbe, x: f32, y: f32) {
    app.window()
        .dispatch_event(slint::platform::WindowEvent::PointerMoved {
            position: slint::LogicalPosition::new(x, y),
        });
}
fn elapsed(ms: u64) {
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(ms));
}
fn main() {
    i_slint_backend_testing::init_no_event_loop();
    for glyph in [Glyph::Sparkle, Glyph::Filter, Glyph::Transition] {
        let app = EffectsProbe::new().unwrap();
        let requests = Rc::new(RefCell::new(Vec::<String>::new()));
        let captured = requests.clone();
        app.global::<TransitionPreview>()
            .on_request(move |id| captured.borrow_mut().push(id.to_string()));
        let item = CatalogueItemData {
            id: "concat.vhs-tape".into(),
            label: "VHS Tape".into(),
            enabled: true,
            glyph,
            ..Default::default()
        };
        app.set_item(item.clone());
        app.show().unwrap();
        point(&app, 60.0, 60.0);
        elapsed(120);
        assert!(requests.borrow().is_empty(), "no render during quick hover");
        elapsed(80);
        assert_eq!(&*requests.borrow(), &["concat.vhs-tape"]);
        elapsed(400);
        assert_eq!(requests.borrow().len(), 1, "one request per hover");
        app.set_item(CatalogueItemData {
            id: "concat.old-film".into(),
            ..item.clone()
        });
        elapsed(1);
        assert_eq!(
            requests.borrow().last().map(String::as_str),
            Some(""),
            "retarget cancels old thumbnail"
        );
        elapsed(200);
        assert_eq!(
            requests.borrow().last().map(String::as_str),
            Some("concat.old-film")
        );
        point(&app, 280.0, 220.0);
        assert_eq!(
            requests.borrow().last().map(String::as_str),
            Some(""),
            "leaving cancels rendering"
        );
        let count = requests.borrow().len();
        point(&app, 60.0, 60.0);
        elapsed(40);
        point(&app, 280.0, 220.0);
        elapsed(220);
        assert_eq!(
            requests.borrow().len(),
            count,
            "quick crossing queues no work"
        );
        app.set_item(CatalogueItemData {
            enabled: false,
            ..item
        });
        point(&app, 60.0, 60.0);
        elapsed(250);
        assert_eq!(
            requests.borrow().len(),
            count,
            "disabled cards queue no work"
        );
        assert_eq!(app.get_applied(), 0, "hover does not apply an effect");
        app.hide().unwrap();
    }
    let app = SubjectShelfProbe::new().unwrap();
    app.set_entries(
        Rc::new(slint::VecModel::from(vec![
            CatalogueEntryData {
                id: "concat.subject-neon".into(),
                name: "Subject Neon".into(),
                source_only: true,
                ..Default::default()
            },
            CatalogueEntryData {
                id: "concat.warm".into(),
                name: "Warm".into(),
                ..Default::default()
            },
        ]))
        .into(),
    );
    app.show().unwrap();
    i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "LibraryTab::touch")
        .nth(6)
        .expect("Effects page")
        .mock_single_click(slint::platform::PointerEventButton::Left);
    i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "Switch::touch")
        .next()
        .expect("effect layer toggle")
        .mock_single_click(slint::platform::PointerEventButton::Left);
    i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "CatalogueCard::card")
        .nth(1)
        .expect("ordinary effect")
        .mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(
        app.get_layer_applied(),
        1,
        "ordinary effects retain adjustment-layer mode"
    );
    i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "CatalogueCard::card")
        .next()
        .expect("subject effect")
        .mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(
        app.get_clip_applied(),
        1,
        "subject effect applies to source despite layer mode"
    );
    assert_eq!(app.get_layer_applied(), 1);
    i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "CatalogueCard::card")
        .nth(1)
        .unwrap()
        .mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(
        app.get_clip_applied(),
        2,
        "source-only selection resets layer mode"
    );
    app.hide().unwrap();
    println!(
        "actual catalogue card: effect/filter/transition hover debounce, retarget, cancel, quick crossing, disabled states and source-only subject shelf routing passed"
    );
}
