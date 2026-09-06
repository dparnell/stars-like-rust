//! The Player Relations dialog.
//!
//! `RelationsDlg` (`10f0:0088`), reached from Commands (Player Relations) or
//! **F7**. Modal, and the whole of it is a list of the other players beside
//! three radio buttons saying how this player regards whichever is selected.
//!
//! See `docs/ui/player-relations.md`.

use crate::App;
use stars_core::relations::Relation;

/// Draw the dialog's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some(selected) = app.relations_dialog else {
        return;
    };
    let others = app.relations_others();

    ui.horizontal_top(|ui| {
        // `&Player:` over the listbox (control 0x7d3).
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Player:").small());
            egui::ScrollArea::vertical()
                .max_height(120.0)
                .show(ui, |ui| {
                    for other in &others {
                        let name = player_name(app, *other);
                        let colour =
                            crate::views::player_colour(i16::try_from(*other).unwrap_or(0));
                        let label = egui::SelectableLabel::new(
                            *other == selected,
                            egui::RichText::new(name).color(colour),
                        );
                        if ui.add(label).clicked() {
                            app.relations_select(*other);
                        }
                    }
                });
        });

        ui.add_space(12.0);

        // The `Relation` group: the three radios, in the order the dialog
        // stacks them — Friend, Neutral, Enemy — which is not the order of
        // their values.
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Relation").small());
                let now = app.regard(selected);
                for relation in Relation::ALL {
                    if ui
                        .add(egui::SelectableLabel::new(now == relation, relation.name()))
                        .clicked()
                    {
                        app.set_regard(selected, relation);
                    }
                }
            });
        });
    });

    ui.separator();
    if ui.button("Close").clicked() {
        app.close_relations();
    }
}

/// The name to list a player under.
fn player_name(app: &App, player: usize) -> String {
    app.game
        .as_ref()
        .and_then(|game| game.players.get(player))
        .filter(|p| !p.name.is_empty())
        .map_or_else(
            || format!("Player {}", player + 1),
            |p| format!("{} ({})", p.name, player + 1),
        )
}
