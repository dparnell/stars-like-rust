//! The menu bar's four game menus — **Turn**, **Commands**, **Report** and
//! **Help** — resource `0x6d4`, with the original's own items and
//! accelerators. The tutorial names them by name ("select Generate from the
//! Turn menu", "Choose Research on the Commands menu"), so the shape
//! matters, not just the reachability; and each header and item is
//! recorded as a drawn widget under the scope `"menu"`, so a test can
//! press them and the tutor's ring can find them.
//!
//! The File and View menus stay with the frontend: they hold the file
//! dialogs and the window's own settings.

use crate::views::record;
use crate::{App, MenuItem, Screen};

/// One menu item: a button, enabled or not, recorded under its caption.
fn item(
    app: &mut App,
    ui: &mut egui::Ui,
    enabled: bool,
    caption: &str,
    key: &str,
) -> egui::Response {
    item_named(app, ui, enabled, caption, caption, key)
}

/// A menu item whose caption carries a mark — a check, or the space one
/// would take — and which is recorded under its plain name.
fn item_named(
    app: &mut App,
    ui: &mut egui::Ui,
    enabled: bool,
    caption: &str,
    name: &str,
    key: &str,
) -> egui::Response {
    let mut button = egui::Button::new(caption);
    if !key.is_empty() {
        button = button.shortcut_text(key);
    }
    let response = ui.add_enabled(enabled, button);
    record(app, ui, name, &response);
    response
}

/// Draw the four menus. Returns the message of anything that failed —
/// the tutorial's world not being made — for the frontend to show.
pub fn game_menus(app: &mut App, ui: &mut egui::Ui) -> Option<String> {
    let mut failed = None;
    let was = app.drawn_scope;
    app.drawn_scope = "menu";
    let playing = app.playing();

    let header = ui.menu_button("Turn", |ui| {
        // `InitializeMenu` greys each of these on its own condition; Wait
        // for New is dead in a single-player game because there is nobody
        // to wait for.
        if item(
            app,
            ui,
            app.menu_item_enabled(MenuItem::WaitForNew),
            "Wait for New",
            "",
        )
        .on_hover_text(
            "Watch for the other players' turns and generate the year. The \
             original makes this a mode of its own, entered by opening the \
             host file.",
        )
        .clicked()
        {
            ui.close_menu();
            app.open_host_mode();
        }
        if item(
            app,
            ui,
            app.menu_item_enabled(MenuItem::Generate),
            "Generate",
            "F9",
        )
        .on_hover_text(
            "Advance one year. The rolls will differ from the original \
             engine's: its generator is seeded from the clock and its state \
             is in no save file.",
        )
        .clicked()
        {
            ui.close_menu();
            app.generate_turn();
        }
    });
    record(app, ui, "Turn", &header.response);

    let header = ui.menu_button("Commands", |ui| {
        if item(app, ui, playing, "Ship Design…", "F4").clicked() {
            ui.close_menu();
            app.open_designer();
        }
        if item(app, ui, playing, "Research…", "F5").clicked() {
            ui.close_menu();
            app.open_research();
        }
        if item(app, ui, playing, "Battle Plans…", "F6").clicked() {
            ui.close_menu();
            app.open_battle_plans();
        }
        if item(
            app,
            ui,
            app.menu_item_enabled(MenuItem::PlayerRelations),
            "Player Relations…",
            "F7",
        )
        .clicked()
        {
            ui.close_menu();
            app.open_relations();
        }
        ui.separator();
        // Alive in a single-player game only while there is a password to
        // take off again.
        if item(
            app,
            ui,
            app.menu_item_enabled(MenuItem::ChangePassword),
            "Change Password…",
            "",
        )
        .clicked()
        {
            ui.close_menu();
            app.open_password_dialog();
        }
        ui.separator();
        // Not the original's — its production queue is reached from the
        // planet tile's Change button and the `q` key, both of which work.
        // This is a third way to the same dialog.
        let can = app.selected_planet().is_some() && app.setup.is_none();
        if item(app, ui, can, "Production…", "Q").clicked() {
            ui.close_menu();
            app.open_production();
        }
    });
    record(app, ui, "Commands", &header.response);

    let header = ui.menu_button("Report", |ui| {
        // The four report windows, in the resource's order and with its
        // separators: one between Others' Fleets and Battles, one after
        // Battles. Every item shows F3 after its caption, which is text and
        // not four accelerators — see `App::open_report`. The one that is
        // open carries a check mark, which `ReportDlg`'s WM_DESTROY takes
        // off again.
        let reports = [
            (Screen::Planets, "Planets…"),
            (Screen::Fleets, "Fleets…"),
            (Screen::EnemyFleets, "Others' Fleets…"),
            (Screen::Battles, "Battles…"),
        ];
        for (screen, caption) in reports {
            if screen == Screen::Battles {
                ui.separator();
            }
            let open = app.screen == screen;
            let mark = if open { "\u{2713} " } else { "    " };
            if item_named(app, ui, playing, &format!("{mark}{caption}"), caption, "F3").clicked() {
                ui.close_menu();
                app.choose_report(screen);
            }
        }
        ui.separator();
        if item(app, ui, playing, "Score…", "F10").clicked() {
            ui.close_menu();
            app.open_score_sheet();
        }
        // This project's own, like Production… under Commands: the original
        // has no such screen, and nothing else here would reach it.
        ui.separator();
        let open = app.screen == Screen::Players;
        let mark = if open { "\u{2713} " } else { "    " };
        if item_named(app, ui, playing, &format!("{mark}Players"), "Players", "")
            .on_hover_text("This project's own summary screen. Esc closes it.")
            .clicked()
        {
            ui.close_menu();
            app.show_screen(Screen::Players);
        }
    });
    record(app, ui, "Report", &header.response);

    let header = ui.menu_button("Help", |ui| {
        // Resource `0x6d4`'s Help menu: Introduction, Player's Guide (F1),
        // a rule, Technology Browser (F2), Tutorial, a rule, About.
        // Introduction is `WINHELP(HELP_CONTEXT, 0x1195)` and the guide
        // `WINHELP(HELP_INDEX)` (`CommandHandler`, `1020:47ab`, `1020:47c9`).
        if item(app, ui, true, "Introduction", "").clicked() {
            ui.close_menu();
            app.help_context(crate::help::context::INTRODUCTION);
        }
        if item(app, ui, true, "Player's Guide", "F1").clicked() {
            ui.close_menu();
            app.help_contents();
        }
        ui.separator();
        if item(app, ui, playing, "Technology Browser…", "F2").clicked() {
            ui.close_menu();
            app.open_browser();
        }
        ui.separator();
        let running = app.tutor.is_some();
        if item(app, ui, !running, "Tutorial", "")
            .on_hover_text(
                "Walk through the game a page at a time. The text is the \
                 game's own and is read from a copy of the original.",
            )
            .clicked()
        {
            ui.close_menu();
            // With no game up, the tutorial makes its own, as `StartTutor`
            // does; with one up it starts on that.
            if app.game.is_some() {
                app.start_tutor();
            } else {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let width = ui.ctx().input(|i| i.screen_rect().width()).max(0.0) as u32;
                if let Err(why) = app.create_tutor_world(width) {
                    failed = Some(why);
                }
            }
        }
        if item(app, ui, running, "Stop the tutorial", "").clicked() {
            ui.close_menu();
            app.end_tutor();
        }
        ui.separator();
        // `&About Stars!...`, `0x63`: `DIALOGBOX(..., About, hwnd, 0x5a)`.
        if item(app, ui, true, "About Stars!...", "").clicked() {
            ui.close_menu();
            let now = ui.input(|i| i.time);
            app.open_about(now);
        }
    });
    record(app, ui, "Help", &header.response);

    app.drawn_scope = was;
    failed
}
