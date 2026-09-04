//! Players, their races, and how they regard each other.

use crate::views::player_colour;
use crate::App;

/// Draw the player screen.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some(game) = app.game.as_ref() else {
        return;
    };
    let mut research: Option<(usize, u8)> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (index, player) in game.players.iter().enumerate() {
            let owner = i16::try_from(index).unwrap_or(0);
            let planets = game
                .planets
                .iter()
                .filter(|p| p.owner == Some(owner))
                .count();
            ui.colored_label(
                player_colour(owner),
                egui::RichText::new(format!("player {index}")).heading(),
            );
            egui::Grid::new(format!("player_{index}"))
                .num_columns(2)
                .spacing([16.0, 2.0])
                .show(ui, |ui| {
                    ui.label("race");
                    ui.label(format!("{:?}", player.race.prt()));
                    ui.end_row();

                    ui.label("played by");
                    ui.label(match player.control {
                        stars_core::ai::Control::Human => "a person".to_string(),
                        stars_core::ai::Control::Computer { personality, .. } => {
                            format!("{personality:?}")
                        }
                    });
                    ui.end_row();

                    ui.label("planets held");
                    ui.label(planets.to_string());
                    ui.end_row();

                    ui.label("technology");
                    ui.label(format!("{:?}", player.research.levels));
                    ui.end_row();

                    ui.label("research");
                    let mut pct = player.research_pct;
                    if ui
                        .add(egui::Slider::new(&mut pct, 0..=100).suffix("%"))
                        .changed()
                    {
                        research = Some((index, pct));
                    }
                    ui.end_row();

                    if !player.relations.is_empty() {
                        ui.label("relations");
                        let text: Vec<String> = player
                            .relations
                            .iter()
                            .enumerate()
                            .filter(|(other, _)| *other != index)
                            .map(|(other, r)| {
                                let word = match r {
                                    0 => "neutral",
                                    1 => "friend",
                                    2 => "enemy",
                                    _ => "?",
                                };
                                format!("{other}: {word}")
                            })
                            .collect();
                        ui.label(text.join(", "));
                        ui.end_row();
                    }
                });
            ui.separator();
        }
    });

    if let Some((player, pct)) = research {
        app.set_research(player, pct);
    }
}
