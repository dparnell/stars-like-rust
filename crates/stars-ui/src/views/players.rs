//! Players, their races, and how they regard each other.

use crate::views::player_colour;
use crate::App;

/// The planetary items a default production queue can hold.
const ITEMS: [(u8, &str); 5] = [
    (0, "mines"),
    (1, "factories"),
    (2, "defences"),
    (3, "alchemy"),
    (5, "terraforming"),
];

/// The name of one planetary item.
fn item_name(id: u8) -> &'static str {
    ITEMS
        .iter()
        .find(|(item, _)| *item == id)
        .map_or("something", |(_, name)| *name)
}

/// What an "attack who" byte says, in the dialog's own words (strings `0x78`
/// to `0x7b`); `4` and up name a player.
fn attack_who(value: u8) -> String {
    match value {
        0 => "Nobody".to_string(),
        1 => "Enemies".to_string(),
        2 => "Neutrals & Enemies".to_string(),
        3 => "Everyone".to_string(),
        player => format!("player {}", player - 4),
    }
}

/// Name the local player's battle plans, and offer the dialog that edits them.
///
/// The plans are not on this screen in the original — they have a dialog of
/// their own, Commands (Battle Plans...), F6 — so this only lists them.
fn battle_plans(ui: &mut egui::Ui, player: &stars_core::Player, open: &mut bool) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("battle plans").strong());
        if ui.small_button("Battle Plans…").clicked() {
            *open = true;
        }
    });
    for (slot, plan) in player.battle_plans.iter().enumerate() {
        let tactic = stars_core::battle::Tactic::from_raw(plan.tactic_nibble());
        let primary = stars_core::battle::TargetClass::from_raw(plan.primary_target);
        let secondary = stars_core::battle::TargetClass::from_raw(plan.secondary_target);
        ui.label(
            egui::RichText::new(format!(
                "{slot}: {} — {} then {}, {}, attacking {}{}",
                plan.name,
                primary.name(),
                secondary.name(),
                tactic.map_or("?", |t| t.name()),
                attack_who(plan.attack_who),
                if plan.dump_cargo() {
                    ", dumping cargo"
                } else {
                    ""
                }
            ))
            .small(),
        );
    }
}

/// Draw the player screen.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    if app.game.is_none() {
        return;
    }
    // The emblems are drawn out of the game's own pictures, which needs the
    // app mutably; the rest of this reads the game. Taking them out for the
    // duration keeps the two apart.
    let mut art = app.art.take();
    let emblems: Vec<Option<stars_formats::resources::art::Cell>> =
        (0..app.game.as_ref().map_or(0, |game| game.players.len()))
            .map(|player| app.emblem_of(player, stars_formats::resources::art::EmblemSize::Large))
            .collect();
    let game = app.game.as_ref().expect("checked just above");
    let mut research: Option<(usize, u8)> = None;
    let mut open_relations = false;
    let mut default_queue: Option<stars_formats::DefaultQueue> = None;
    let mut open_battle_plans = false;
    let mut password: Option<String> = None;
    let mut password_box = app.password_box.clone();
    let me = app.local_player();

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (index, player) in game.players.iter().enumerate() {
            let owner = i16::try_from(index).unwrap_or(0);
            let planets = game
                .planets
                .iter()
                .filter(|p| p.owner == Some(owner))
                .count();
            ui.horizontal(|ui| {
                if let Some(Some(cell)) = emblems.get(index) {
                    crate::art::draw_with(art.as_mut(), ui, *cell, 32.0);
                }
                ui.colored_label(
                    player_colour(owner),
                    egui::RichText::new(format!("player {index}")).heading(),
                );
            });
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

                    if index == me {
                        // The queue a planet this player settles starts with.
                        ui.label("new colonies build");
                        ui.vertical(|ui| {
                            let queue = &player.default_queue;
                            if queue.items.is_empty() {
                                ui.label(egui::RichText::new("nothing — they start empty").weak());
                            } else {
                                for entry in &queue.items {
                                    ui.label(format!(
                                        "{} x {}",
                                        entry.count,
                                        item_name(entry.item)
                                    ));
                                }
                            }
                            ui.horizontal(|ui| {
                                for (id, name) in ITEMS {
                                    if ui.small_button(format!("+100 {name}")).clicked() {
                                        let mut next = queue.clone();
                                        next.items.push(stars_formats::DefaultQueueItem {
                                            item: id,
                                            count: 100,
                                        });
                                        next.items.truncate(stars_formats::DEFAULT_QUEUE_MAX);
                                        default_queue = Some(next);
                                    }
                                }
                                if !queue.items.is_empty() && ui.small_button("clear").clicked() {
                                    default_queue = Some(stars_formats::DefaultQueue {
                                        no_research: queue.no_research,
                                        items: Vec::new(),
                                    });
                                }
                            });
                            let mut exempt = queue.no_research;
                            if ui
                                .checkbox(&mut exempt, "and take no research from them")
                                .changed()
                            {
                                default_queue = Some(stars_formats::DefaultQueue {
                                    no_research: exempt,
                                    items: queue.items.clone(),
                                });
                            }
                        });
                        ui.end_row();

                        // The turn password. What is kept is a checksum of
                        // the typed text, which is all the game ever kept —
                        // enough to stop another player in a play-by-mail game
                        // opening this turn by accident, and no more than that.
                        ui.label("turn password");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut password_box)
                                    .password(true)
                                    .hint_text(if player.password == 0 {
                                        "none set"
                                    } else {
                                        "one is set"
                                    })
                                    .desired_width(120.0),
                            );
                            if ui.button("set").clicked() {
                                password = Some(password_box.clone());
                            }
                            if player.password != 0 && ui.button("clear").clicked() {
                                password = Some(String::new());
                            }
                        });
                        ui.end_row();

                        // Relations are set in their own dialog, which is
                        // where the original sets them; this row says how they
                        // stand and opens it.
                        ui.label("relations");
                        ui.horizontal(|ui| {
                            ui.label(regards(game, me));
                            if ui.button("Player Relations…").clicked() {
                                open_relations = true;
                            }
                        });
                        ui.end_row();
                    } else if !player.relations.is_empty() {
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
            if index == me {
                battle_plans(ui, player, &mut open_battle_plans);
            }
            ui.separator();
        }
    });

    app.art = art;
    if let Some(text) = password {
        app.set_password(&text);
        password_box.clear();
    }
    app.password_box = password_box;
    if open_battle_plans {
        app.open_battle_plans();
    }
    if let Some((player, pct)) = research {
        app.set_research(player, pct);
    }
    if open_relations {
        app.open_relations();
    }
    if let Some(queue) = default_queue {
        app.set_default_queue(queue);
    }
}

/// How the local player regards everybody else, in a line.
fn regards(game: &stars_core::GameState, me: usize) -> String {
    let text: Vec<String> = stars_core::relations::others(game, me)
        .into_iter()
        .map(|other| {
            format!(
                "{}: {}",
                other,
                stars_core::relations::regard(game, me, other).name()
            )
        })
        .collect();
    if text.is_empty() {
        "nobody else".to_string()
    } else {
        text.join(", ")
    }
}
