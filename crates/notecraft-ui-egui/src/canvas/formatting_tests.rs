//! Home tab formatting through the egui_kittest harness: ribbon buttons and shortcuts run the
//! format commands and the page text is read back from the session.

use egui_kittest::kittest::Queryable;

use crate::app_shell::AppShell;

type Harness = egui_kittest::Harness<'static, AppShell>;

const PAGE: &str = notecraft_engine::mock::WELCOME_ID;

fn harness(text: &str) -> Harness {
    let mut shell = AppShell::new();
    shell.set_page_source(PAGE, text);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, shell: &mut AppShell| {
                shell.logic(ui.ctx());
                shell.ui(ui);
            },
            shell,
        );
    harness.run();
    harness
}

fn text(harness: &Harness) -> String {
    harness
        .state()
        .session()
        .page_text(PAGE)
        .unwrap_or_default()
        .to_owned()
}

fn select_all_in_text(harness: &mut Harness) {
    let origin = harness.get_by_label("Canvas").rect().min + egui::vec2(120.0, 135.0);
    harness.hover_at(origin);
    harness.step();
    harness.drag_at(origin);
    harness.step();
    harness.drop_at(origin);
    harness.step();
    harness.step();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
}

#[test]
fn the_bold_button_wraps_the_selection_and_undo_restores_it() {
    let mut harness = harness("Hello\n");
    select_all_in_text(&mut harness);
    harness.get_by_label("Bold").click();
    harness.run();
    assert!(text(&harness).contains("**"), "got {:?}", text(&harness));
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();
    assert_eq!(text(&harness), "Hello\n");
}

#[test]
fn ctrl_i_italicizes_the_selection() {
    let mut harness = harness("Hello\n");
    select_all_in_text(&mut harness);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::I);
    harness.run();
    assert!(text(&harness).contains('*'), "got {:?}", text(&harness));
}
