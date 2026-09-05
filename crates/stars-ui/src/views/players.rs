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

/// Draw the local player's battle plans, and let them be edited.
///
/// A plan's tactic and target fields are shown as the numbers the file holds:
/// the original bounds them (tactic `0..=6`, targets `0..=8`, from the replay
/// arm at `1048:c287`) but what each value means is not decoded — see
/// `docs/formats/battleplan.md`. "Attack who" is decoded, so it is named.
fn battle_plans(
    app: &crate::App,
    ui: &mut egui::Ui,
    player: &stars_core::Player,
    edit: &mut Option<(usize, stars_formats::BattlePlanRecord)>,
    delete: &mut Option<usize>,
) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new("battle plans").strong());
    for (slot, plan) in player.battle_plans.iter().enumerate() {
        let mut edited = plan.clone();
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(format!("{slot}:"));
            if ui
                .add(egui::TextEdit::singleline(&mut edited.name).desired_width(120.0))
                .lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
            {
                changed = true;
            }
            ui.label("tactic");
            changed |= ui
                .add(egui::DragValue::new(&mut edited.tactic).range(0..=6))
                .changed();
            ui.label("targets");
            changed |= ui
                .add(egui::DragValue::new(&mut edited.primary_target).range(0..=8))
                .changed();
            changed |= ui
                .add(egui::DragValue::new(&mut edited.secondary_target).range(0..=8))
                .changed();
            egui::ComboBox::from_id_source(("attack_who", slot))
                .selected_text(attack_who(edited.attack_who))
                .show_ui(ui, |ui| {
                    for value in 0..=3u8 {
                        if ui
                            .selectable_label(edited.attack_who == value, attack_who(value))
                            .clicked()
                        {
                            edited.attack_who = value;
                            changed = true;
                        }
                    }
                });
            // The original moves every fleet at or past a deleted plan down a
            // slot, so deleting is not a free action.
            if player.battle_plans.len() > 1 && ui.small_button("delete").clicked() {
                *delete = Some(slot);
            }
        });
        if changed {
            *edit = Some((slot, edited));
        }
    }
    // A new plan is a definition written one past the end, which is exactly
    // what the game's dialog logs when the player adds one.
    if player.battle_plans.len() < 16 && ui.button("add a plan").clicked() {
        let mut plan = player
            .battle_plans
            .last()
            .cloned()
            .unwrap_or_else(|| stars_core::default_battle_plans(app.local_player())[0].clone());
        plan.name = "New plan".to_string();
        *edit = Some((player.battle_plans.len(), plan));
    }
}

/// The name of an "attack who" selector value.
fn attack_who(value: u8) -> String {
    match value {
        0 => "nobody".to_string(),
        1 => "enemies".to_string(),
        2 => "neutrals and enemies".to_string(),
        3 => "everyone".to_string(),
        race => format!("race {}", race - 4),
    }
}

/// Draw the player screen.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some(game) = app.game.as_ref() else {
        return;
    };
    let mut research: Option<(usize, u8)> = None;
    let mut relations: Option<(usize, u8)> = None;
    let mut default_queue: Option<stars_formats::DefaultQueue> = None;
    let mut plan_edit: Option<(usize, stars_formats::BattlePlanRecord)> = None;
    let mut plan_delete: Option<usize> = None;
    let me = app.local_player();

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

                        // The player whose orders this session records is the
                        // only one whose relations it may change, because the
                        // order carries that player's own table.
                        for other in 0..game.players.len() {
                            if other == me {
                                continue;
                            }
                            let now = player.relations.get(other).copied().unwrap_or(0);
                            ui.label(format!("regards player {other}"));
                            ui.horizontal(|ui| {
                                for (value, word) in [(0u8, "neutral"), (1, "friend"), (2, "enemy")]
                                {
                                    if ui.selectable_label(now == value, word).clicked() {
                                        relations = Some((other, value));
                                    }
                                }
                            });
                            ui.end_row();
                        }
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
                battle_plans(app, ui, player, &mut plan_edit, &mut plan_delete);
            }
            ui.separator();
        }
    });

    if let Some((slot, plan)) = plan_edit {
        app.set_battle_plan_definition(slot, &plan);
    }
    if let Some(slot) = plan_delete {
        app.delete_battle_plan(slot);
    }
    if let Some((player, pct)) = research {
        app.set_research(player, pct);
    }
    if let Some((other, value)) = relations {
        app.set_relations(other, value);
    }
    if let Some(queue) = default_queue {
        app.set_default_queue(queue);
    }
}
