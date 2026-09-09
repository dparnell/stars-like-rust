//! The Game Parameters window.
//!
//! View (Game Parameters), menu id `0x9e`. It is the **Advanced Game** wizard
//! shown read-only: three dialogs, 390, 391 and 392, all 261 by 210 dialog
//! units, carrying the same five-button footer the race wizard's pages carry
//! and at the same `y = 190`.
//!
//! Page 3 is where `MANUAL.PDF` p. 2-3 sends a player: *"To view the winning
//! conditions once the game has begun, choose the View (Race) menu item, then
//! turn to page 3 of the View Game Parameters dialog that appears."*
//!
//! Page 2 holds nothing but the footer and page 3 only its seven checkboxes,
//! because the player list and each condition's own control are built at run
//! time — the same trap the race wizard's middle pages set.
//!
//! See `docs/ui/view-menu.md`.

use crate::App;

/// Draw the window's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let page = app.parameters_page.min(2);
    let template = crate::dialog::GAME_PARAMS[page];
    let (rect, at, caption) = crate::views::dialog_frame(ui, template);

    let painter = ui.painter_at(rect);
    let font = egui::TextStyle::Small.resolve(ui.style());
    let text = ui.visuals().text_color();
    painter.text(
        rect.min + egui::vec2(6.0, 2.0),
        egui::Align2::LEFT_TOP,
        format!("Game Parameters — {}", template.caption),
        font.clone(),
        text,
    );

    let body = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 4.0, rect.top() + 16.0),
        egui::pos2(rect.right() - 4.0, at(0x76).top() - 4.0),
    );
    {
        let mut child = ui.child_ui(body, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(body);
        match page {
            0 => universe(app, &mut child),
            1 => players(app, &mut child),
            _ => victory(app, &mut child, rect, &at),
        }
    }

    // The footer. This is a viewer, so the three buttons that would change
    // something are dead and `Cancel` is the way out.
    crate::views::dialog_button(ui, at(0x76), &caption(0x76), false);
    if crate::views::dialog_button(ui, at(0x2), "Close", true).clicked() {
        app.screen = crate::Screen::Galaxy;
    }
    if crate::views::dialog_button(ui, at(0x42e), &caption(0x42e), page > 0).clicked() {
        app.parameters_page = page.saturating_sub(1);
    }
    if crate::views::dialog_button(ui, at(0x42f), &caption(0x42f), page + 1 < 3).clicked() {
        app.parameters_page = page + 1;
    }
    crate::views::dialog_button(ui, at(0x430), &caption(0x430), false);
}

/// Page 1: what the universe was made with.
fn universe(app: &mut App, ui: &mut egui::Ui) {
    let rows = app.game_parameters_rows();
    if rows.is_empty() {
        ui.label(
            egui::RichText::new(
                "The game's settings live in its .xy universe file, which was not \
                 found beside this save or one directory up.",
            )
            .weak()
            .small(),
        );
        return;
    }
    egui::Grid::new("game-parameters")
        .num_columns(2)
        .spacing([14.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for (label, value) in &rows {
                ui.label(egui::RichText::new(label).small());
                ui.label(egui::RichText::new(value).small());
                ui.end_row();
            }
        });
}

/// Page 2: who is playing. The template holds nothing but the footer, because
/// the original builds this list at run time.
fn players(app: &mut App, ui: &mut egui::Ui) {
    let names: Vec<String> = app
        .game
        .as_ref()
        .map(|game| {
            (0..game.players.len())
                .map(|index| app.player_name(index))
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() {
        ui.label(egui::RichText::new("no players recorded").weak().small());
        return;
    }
    for (index, name) in names.iter().enumerate() {
        ui.label(egui::RichText::new(format!("{}.  {name}", index + 1)).small());
    }
}

/// Page 3: the winning conditions, each against the checkbox the template
/// gives it.
fn victory(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect, at: &impl Fn(u16) -> egui::Rect) {
    ui.label(
        egui::RichText::new(crate::dialog::VICTORY_HEADING)
            .small()
            .strong(),
    );
    let conditions = app.game_parameters_conditions();
    if conditions.is_empty() {
        ui.label(egui::RichText::new("none recorded").weak().small());
        return;
    }
    // The seven checkboxes are `0x123`..`0x129`, twelve units square, and each
    // condition's own words go beside its box.
    let painter = ui.painter_at(rect);
    let font = egui::TextStyle::Small.resolve(ui.style());
    for (index, condition) in conditions.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let id = 0x123 + index as u16;
        let Some(control) = crate::dialog::GAME_PARAMS[2].control(id) else {
            // More conditions than the page has boxes: the Score sheet lists
            // ten and the template has seven, so the rest run on underneath.
            ui.label(row_text(condition, ui));
            continue;
        };
        let _ = control;
        let box_ = at(id);
        painter.rect_stroke(
            box_,
            0.0,
            egui::Stroke::new(1.0_f32, ui.visuals().text_color()),
        );
        if condition.active {
            painter.rect_filled(box_.shrink(3.0), 0.0, ui.visuals().text_color());
        }
        painter.text(
            egui::pos2(box_.right() + 4.0, box_.top()),
            egui::Align2::LEFT_TOP,
            &condition.text,
            font.clone(),
            colour(condition, ui),
        );
    }
}

/// A condition the game is not playing for is listed greyed, with its setting,
/// exactly as the Score sheet lists it.
fn colour(condition: &stars_core::scoresheet::Condition, ui: &egui::Ui) -> egui::Color32 {
    if condition.active || !condition.scoreable {
        ui.visuals().text_color()
    } else {
        egui::Color32::from_rgb(0x9a, 0x9a, 0x9a)
    }
}

/// The same, as a `RichText` for the rows that have no box of their own.
fn row_text(condition: &stars_core::scoresheet::Condition, ui: &egui::Ui) -> egui::RichText {
    egui::RichText::new(&condition.text)
        .small()
        .color(colour(condition, ui))
}
