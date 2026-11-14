// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors
use slint::{Model, ModelRc, SharedString, VecModel};
slint::include_modules!();
mod ui {
    pub use super::{
        CaptionsSheetData, ChainChoiceData, ExportData, RelinkData, SettingsData, StartData,
        TransitionChoiceData,
    };
}
include!("publish_helpers.rs");
fn main() {
    i_slint_backend_testing::init_no_event_loop();
    let choice = |label: &str, current| TransitionChoiceData {
        options: ModelRc::new(VecModel::from(vec![SharedString::from(label)])),
        values: ModelRc::new(VecModel::from(vec![1.0])),
        current,
        ..Default::default()
    };
    let model = VecModel::from(vec![choice("Soft", 0)]);
    let held = model.row_data(0).unwrap().options;
    sync_with(&model, vec![choice("Soft", 0)], same_transition_choice);
    assert!(
        held == model.row_data(0).unwrap().options,
        "equal nested rows retain model identity"
    );
    sync_with(&model, vec![choice("Hard", 0)], same_transition_choice);
    assert_eq!(
        model.row_data(0).unwrap().options.row_data(0).unwrap(),
        "Hard"
    );
    sync_with(&model, vec![choice("Hard", 1)], same_transition_choice);
    assert_eq!(
        model.row_data(0).unwrap().current,
        1,
        "non-model fields still update"
    );
    let chain = |label: &str| ChainChoiceData {
        options: ModelRc::new(VecModel::from(vec![SharedString::from(label)])),
        values: ModelRc::new(VecModel::from(vec![1.0])),
        ..Default::default()
    };
    let chain_model = VecModel::from(vec![chain("Matte")]);
    let held = chain_model.row_data(0).unwrap().options;
    sync_with(&chain_model, vec![chain("Matte")], same_chain_choice);
    assert!(held == chain_model.row_data(0).unwrap().options);
    sync_with(&chain_model, vec![chain("Result")], same_chain_choice);
    assert_eq!(
        chain_model
            .row_data(0)
            .unwrap()
            .options
            .row_data(0)
            .unwrap(),
        "Result"
    );
    let exporting = |codec| ExportData {
        codec,
        rates: ModelRc::new(VecModel::from(vec![SharedString::from("30 fps")])),
        ..Default::default()
    };
    let changes = std::cell::Cell::new(0);
    set_changed(exporting(0), exporting(0), same_export, |_| {
        changes.set(changes.get() + 1)
    });
    assert_eq!(
        changes.get(),
        0,
        "equal dialog data does not reset the form"
    );
    set_changed(exporting(0), exporting(1), same_export, |_| {
        changes.set(changes.get() + 1)
    });
    assert_eq!(changes.get(), 1, "real dialog edits are published");
    println!(
        "actual publish helpers: unchanged nested models/dialogs retained; real option/current/codec changes published"
    );
}
