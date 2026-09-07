//! The race viewer.
//!
//! View (Race), `IDM_RACE_EDIT1` (`0x9c`), F8. The original opens the **race
//! wizard** on the player's own race, read-only: six pages, `IDD_RACE_WIZARD_1`
//! through `_6`, walked with Back and Next.
//!
//! Those dialogs are largely **owner-drawn** — the habitability sliders and the
//! economy bars are painted rather than laid out as controls — so what is
//! reproduced here is what each page *says* about the race, page by page and
//! in the wizard's own order, rather than how it looks.
//!
//! See `docs/ui/race-wizard.md`.

use crate::App;

/// Draw the viewer's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some((player, page)) = app.race_viewer else {
        return;
    };
    let pages = app.race_pages(player);
    let Some(showing) = pages.get(page) else {
        ui.label(egui::RichText::new("no race to show").weak().small());
        return;
    };

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(showing.title).strong());
        ui.label(
            egui::RichText::new(format!("({} of {})", page + 1, pages.len()))
                .small()
                .weak(),
        );
    });
    ui.separator();

    egui::Grid::new("race-page")
        .num_columns(2)
        .spacing([14.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for (label, value) in &showing.rows {
                ui.label(egui::RichText::new(label).small());
                ui.label(egui::RichText::new(value).small());
                ui.end_row();
            }
        });

    ui.separator();
    ui.horizontal(|ui| {
        // The wizard's own buttons, less the ones that would change something.
        if ui
            .add_enabled(page > 0, egui::Button::new("< Back"))
            .clicked()
        {
            app.race_viewer_page(false);
        }
        if ui
            .add_enabled(page + 1 < pages.len(), egui::Button::new("Next >"))
            .clicked()
        {
            app.race_viewer_page(true);
        }
        if ui.button("Close").clicked() {
            app.close_race_viewer();
        }
    });
}
