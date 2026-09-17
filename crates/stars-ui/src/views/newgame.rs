//! The New Game wizard: universe settings, options, and who is playing.
//!
//! The original spreads this over four dialogs (`NewGameDlg`, `NewGameDlg2`,
//! `NewGameDlg3` and `SimpleNewGameDlg`) plus the race wizard. This is one
//! page, because there is no race wizard yet: a human player picks one of the
//! ten primary racial traits and gets the stock Humanoid economy with it, or
//! loads a race someone else designed from a `.rN` file.

use stars_core::newgame::{stock_race, Density, NewPlayer, Size, StartDistance};
use stars_core::opponents::{self, LEVEL_NAMES, PERSONALITY_NAMES};
use stars_core::race::Prt;
use stars_core::{ai::Control, Race};

use crate::App;

/// What the wizard is asking the shell to do, once the player has clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Create the game as configured.
    Create,
    /// Abandon the wizard.
    Cancel,
    /// Open a race file for this player slot.
    LoadRace(usize),
}

/// Draw the wizard, returning what the player asked for.
///
/// The shell owns the file dialog, so [`Action::LoadRace`] comes back rather
/// than being handled here.
#[must_use]
pub fn view(app: &mut App, ui: &mut egui::Ui) -> Option<Action> {
    let fallback_ms = crate::views::egui_clock_ms(ui);
    let fresh = app.fresh_game_id(fallback_ms);
    let config = app.setup.as_mut()?;
    let mut action = None;
    // The seed as typed, kept while it does not parse so a half-typed
    // number is not snapped back.
    let mut seed_text = ui.memory_mut(|m| {
        m.data
            .get_temp::<String>(egui::Id::new("new-game-seed-text"))
            .filter(|t| t.trim().parse::<u32>().ok() == Some(config.id))
            .unwrap_or_else(|| config.id.to_string())
    });
    let mut new_seed = false;

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("New game");
        ui.add_space(8.0);

        egui::Grid::new("new_game_universe")
            .num_columns(2)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut config.name).desired_width(220.0));
                ui.end_row();

                ui.label("Universe");
                egui::ComboBox::from_id_source("size")
                    .selected_text(config.size.name())
                    .show_ui(ui, |ui| {
                        for size in Size::ALL {
                            ui.selectable_value(&mut config.size, size, size.name());
                        }
                    });
                ui.end_row();

                ui.label("Density");
                egui::ComboBox::from_id_source("density")
                    .selected_text(config.density.name())
                    .show_ui(ui, |ui| {
                        for density in Density::ALL {
                            ui.selectable_value(&mut config.density, density, density.name());
                        }
                    });
                ui.end_row();

                ui.label("Players start");
                egui::ComboBox::from_id_source("distance")
                    .selected_text(config.start_distance.name())
                    .show_ui(ui, |ui| {
                        for distance in StartDistance::ALL {
                            ui.selectable_value(
                                &mut config.start_distance,
                                distance,
                                distance.name(),
                            );
                        }
                    });
                ui.end_row();

                ui.label("Planets");
                ui.label(
                    stars_core::newgame::planet_count(config.size, config.density).to_string(),
                );
                ui.end_row();

                // The game's id, which seeds its universe: the clock's
                // milliseconds when the wizard opened, as `GenerateWorld`
                // takes `GetTickCount()`, and any number typed in its
                // place. The same settings and seed give the same universe.
                ui.label("Seed");
                ui.horizontal(|ui| {
                    let mut text = seed_text.clone();
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .desired_width(110.0)
                            .char_limit(10),
                    );
                    if field.changed() {
                        seed_text = text;
                        if let Ok(id) = seed_text.trim().parse::<u32>() {
                            config.id = id;
                        }
                    }
                    let bad = seed_text.trim().parse::<u32>().is_err();
                    if bad {
                        ui.colored_label(
                            egui::Color32::from_rgb(0xff, 0x8a, 0x8a),
                            "a whole number up to 4294967295",
                        );
                    }
                    if ui
                        .button("New seed")
                        .on_hover_text("Another from the clock.")
                        .clicked()
                    {
                        new_seed = true;
                    }
                });
                ui.end_row();
            });

        ui.add_space(6.0);
        ui.checkbox(&mut config.clumping, "Clump the planets into clusters");
        ui.checkbox(
            &mut config.random_events,
            "Random events (wormholes, artifacts, the Mystery Trader)",
        );
        ui.checkbox(&mut config.slow_tech, "Slower tech advances");
        ui.checkbox(
            &mut config.unlimited_minerals,
            "Maximum mineral concentration on every planet",
        );
        ui.checkbox(&mut config.public_scores, "Scores are public");

        ui.add_space(10.0);
        ui.separator();
        ui.heading("Players");
        ui.add_space(4.0);

        let mut remove: Option<usize> = None;
        for index in 0..config.players.len() {
            ui.horizontal(|ui| {
                ui.label(format!("{}.", index + 1));
                let human = matches!(config.players[index].control, Control::Human);
                if human {
                    human_row(ui, index, &mut config.players[index], &mut action);
                } else {
                    computer_row(ui, index, &mut config.players[index]);
                }
                if config.players.len() > 1 && ui.button("remove").clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            config.players.remove(index);
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let full = config.players.len() >= stars_core::newgame::MAX_PLAYERS;
            if ui
                .add_enabled(!full, egui::Button::new("add a person"))
                .clicked()
            {
                config.players.push(NewPlayer::human(stock_race(Prt::Joat)));
            }
            if ui
                .add_enabled(!full, egui::Button::new("add a computer player"))
                .clicked()
            {
                if let Some(opponent) = opponents::opponent(0, 1) {
                    config.players.push(opponent.as_player());
                }
            }
            if full {
                ui.label(
                    egui::RichText::new(format!(
                        "{} players is the most a game holds",
                        stars_core::newgame::MAX_PLAYERS
                    ))
                    .weak(),
                );
            }
        });
        // The original's simple New Game dialog fills the roster itself
        // from a difficulty and the universe's size (`InitNewGamePlr`);
        // here that is a button beside the difficulty it would use.
        ui.horizontal(|ui| {
            ui.label("Or let the game choose the opponents, as its simple dialog does, at");
            for (level, name) in LEVEL_NAMES.iter().enumerate() {
                if ui.button(*name).clicked() {
                    let mut rng = stars_core::rng::Rng::randomize(fresh);
                    let roster =
                        stars_core::newgame::simple_game_opponents(config.size, level, &mut rng);
                    config
                        .players
                        .retain(|p| matches!(p.control, Control::Human));
                    for (personality, level) in roster {
                        let personality = personality
                            .unwrap_or_else(|| usize::try_from(rng.random(6)).unwrap_or(0));
                        let level =
                            level.unwrap_or_else(|| usize::try_from(rng.random(4)).unwrap_or(0));
                        if let Some(opponent) = opponents::opponent(personality, level) {
                            if config.players.len() < stars_core::newgame::MAX_PLAYERS {
                                config.players.push(opponent.as_player());
                            }
                        }
                    }
                }
            }
        });

        ui.add_space(12.0);
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Create game").clicked() {
                action = Some(Action::Create);
            }
            if ui.button("Cancel").clicked() {
                action = Some(Action::Cancel);
            }
        });
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Saving a new game writes the whole set: a .xy universe, a .hst host file \
                 and one .mN per player. Space objects and messages are not written, \
                 because the simulation does not carry them.",
            )
            .weak(),
        );
    });

    if new_seed {
        config.id = fresh;
        seed_text = fresh.to_string();
    }
    ui.memory_mut(|m| {
        m.data
            .insert_temp(egui::Id::new("new-game-seed-text"), seed_text);
    });
    action
}

/// One human player's row: which primary racial trait, or a loaded race.
fn human_row(ui: &mut egui::Ui, index: usize, player: &mut NewPlayer, action: &mut Option<Action>) {
    ui.label("a person playing");
    let current = player.race.prt();
    let stock = current.is_some_and(|prt| player.race == stock_race(prt));
    let label = match (stock, current) {
        (true, Some(prt)) => prt.name().to_string(),
        (false, Some(prt)) => format!("a custom {} race", prt.abbrev()),
        _ => "an unrecognised race".to_string(),
    };
    egui::ComboBox::from_id_source(("prt", index))
        .selected_text(label)
        .width(180.0)
        .show_ui(ui, |ui| {
            for prt in Prt::ALL {
                if ui
                    .selectable_label(stock && current == Some(prt), prt.name())
                    .clicked()
                {
                    player.race = stock_race(prt);
                }
            }
        });
    if ui.button("load a race…").clicked() {
        *action = Some(Action::LoadRace(index));
    }
}

/// One computer player's row: which of the twenty-four built-in opponents.
fn computer_row(ui: &mut egui::Ui, index: usize, player: &mut NewPlayer) {
    ui.label("the computer playing");
    let current = opponents::ALL.iter().find(|o| o.race == player.race);
    let label = current.map_or_else(|| "a custom race".to_string(), opponents::Opponent::name);
    egui::ComboBox::from_id_source(("ai", index))
        .selected_text(label)
        .width(220.0)
        .show_ui(ui, |ui| {
            for personality in 0..PERSONALITY_NAMES.len() {
                for level in 0..LEVEL_NAMES.len() {
                    let Some(opponent) = opponents::opponent(personality, level) else {
                        continue;
                    };
                    let selected = current.is_some_and(|c| std::ptr::eq(c, opponent));
                    if ui.selectable_label(selected, opponent.name()).clicked() {
                        *player = opponent.as_player();
                    }
                }
            }
        });
}

/// Read a race out of a `.rN` race file (or any file whose first player block
/// carries a full race).
///
/// A race file's type-6 block is a bare race definition; a `.mN` or `.hst`
/// puts a player header in front of the very same struct, and
/// [`stars_formats::PlayerRecord`] decodes both.
///
/// # Errors
/// Returns a message suitable for showing to the player.
pub fn race_from_file(path: &std::path::Path) -> Result<Race, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let file = stars_formats::StarsFile::decode(&bytes)
        .map_err(|e| format!("cannot decode {}: {e}", path.display()))?;
    let records = stars_formats::player_records(&file)
        .map_err(|e| format!("cannot read the players in {}: {e}", path.display()))?;
    let record = records
        .iter()
        .find_map(|r| r.race.as_ref())
        .ok_or_else(|| format!("{} holds no race definition", path.display()))?;
    Ok(stars_core::race_from_record(record))
}
