//! The fleet pane.
//!
//! The **same window as the planet pane**, with a different tile table when a
//! fleet is selected rather than a planet (`rgtileShip`, `1120:090e`). Seven
//! tiles in two columns:
//!
//! ```text
//! ┌───────────────────────┬───────────────────────┐
//! │ the fleet, drawn      │ Fuel & Cargo          │
//! ├───────────────────────┼───────────────────────┤
//! │ <planet> / In Deep    │ Fleet Composition     │
//! │ Space                 │                       │
//! ├───────────────────────┼───────────────────────┤
//! │ Fleet Waypoints       │ the other fleets here │
//! ├───────────────────────┤                       │
//! │ Waypoint Task         │                       │
//! └───────────────────────┴───────────────────────┘
//! ```
//!
//! See `docs/ui/fleet-pane.md`.

use crate::App;

/// Draw the fleet pane.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let mut open: Vec<bool> = app.open_ship_tiles.to_vec();
    crate::views::tile_pane(
        app,
        ui,
        &crate::tiles::SHIP_TILES,
        &mut open,
        tile_title,
        tile_body,
    );
    for (index, value) in open.iter().enumerate() {
        app.open_ship_tiles[index] = *value;
    }
}

/// What a tile's title bar says. Three of the seven take their title from the
/// game rather than the table: the fleet's own name, the planet it is at, and
/// the fleets-here tile, which titles itself by which pane it is in.
fn tile_title(app: &mut App, index: usize) -> String {
    match index {
        0 => app
            .survey_subject()
            .fleet_index()
            .map_or_else(|| "Fleet".to_string(), |i| app.fleet_display_name(i)),
        1 => app.fleet_location_title(),
        6 => app.pane_fleets_title().to_string(),
        other => crate::tiles::SHIP_TILES[other].title.to_string(),
    }
}

/// What goes inside one tile.
fn tile_body(app: &mut App, ui: &mut egui::Ui, index: usize) {
    match index {
        0 => {
            summary(app, ui);
            walk_buttons(app, ui);
        }
        1 => {
            location(app, ui);
            // The location tile's Goto goes to the planet the fleet orbits
            // (`rghwndBtn[3]`, `SelectAdjPlanet(0, sel.fl.idPlanet)`).
            let orbiting = app
                .survey_subject()
                .fleet_index()
                .and_then(|i| app.game.as_ref()?.fleets.get(i))
                .and_then(|f| f.orbiting)
                .is_some();
            if ui
                .add_enabled(
                    orbiting,
                    egui::Button::new(egui::RichText::new("Goto").small()),
                )
                .clicked()
            {
                app.goto_orbited_planet();
            }
        }
        2 => crate::views::planet::grid(ui, "waypoints", &app.fleet_waypoints_tile(), false),
        3 => waypoint_task(app, ui),
        4 => crate::views::planet::grid(ui, "fuel-cargo", &app.fleet_cargo_tile(), false),
        5 => crate::views::planet::grid(ui, "composition", &app.fleet_composition_tile(), false),
        _ => crate::views::fleets_here_body(app, ui),
    }
}

/// The tallest tile: the fleet itself, which the original draws as a picture.
///
/// `DrawFleetBitmap` blits the primary design's ship 64 pixels square with the
/// owner's race emblem over its bottom-left corner. Without the game's own
/// pictures the tile is the same words without the ship.
fn summary(app: &mut App, ui: &mut egui::Ui) {
    let Some(fleet) = app.pane_fleet() else {
        ui.label(egui::RichText::new("no fleet selected").weak().small());
        return;
    };
    let ships: i32 = fleet.stacks.iter().map(|s| s.count).sum();
    let owner = fleet.owner;
    let position = fleet.position;
    let picture = app.fleet_picture();
    let emblem = app.fleet_emblem(stars_formats::resources::art::EmblemSize::Medium);
    {
        if let Some((cell, distinct)) = picture {
            ui.horizontal_top(|ui| {
                let corner = ui.cursor().min;
                if crate::art::draw(app, ui, cell, 64.0) {
                    if let Some(emblem) = emblem {
                        let ctx = ui.ctx().clone();
                        if let Some(art) = app.art.as_mut() {
                            if let Some(image) = art.sprite(&ctx, emblem, 16.0) {
                                image.paint_at(
                                    ui,
                                    egui::Rect::from_min_size(
                                        corner + egui::vec2(0.0, 48.0),
                                        egui::vec2(16.0, 16.0),
                                    ),
                                );
                            }
                        }
                    }
                    // The original marks a mixed fleet beside the picture
                    // rather than drawing every design in it.
                    if distinct > 1 {
                        ui.label(egui::RichText::new(format!("+{}", distinct - 1)).small());
                    }
                }
            });
        }
        ui.label(egui::RichText::new(format!("player {}", owner + 1)).small());
        ui.label(egui::RichText::new(format!("{ships} ships")).small());
        ui.label(egui::RichText::new(format!("({}, {})", position.x, position.y)).small());
    }
}

/// The planet the fleet is at, or deep space.
fn location(app: &App, ui: &mut egui::Ui) {
    let at_planet = app.pane_fleet().is_some_and(|f| f.orbiting.is_some());
    ui.label(
        egui::RichText::new(if at_planet { "in orbit" } else { "under way" })
            .weak()
            .small(),
    );
}

/// The **Waypoint Task** tile: what the fleet does when it gets there.
///
/// `DrawShipWayPtOrders` (`1050:0912`) with `UpdateOrdersDDs` (`1050:93ee`)
/// behind it. The tile is always about `sel.iwpAct`, the waypoint the scanner
/// has in hand, and it is three controls deep: the task, then whatever second
/// choice that task needs, then — for Transport only — an action and a
/// quantity per cargo.
fn waypoint_task(app: &mut App, ui: &mut egui::Ui) {
    use stars_formats::task;

    let Some(waypoint) = app.task_waypoint() else {
        ui.label(egui::RichText::new(app.fleet_task_tile()).small());
        return;
    };
    let Some(leg) = app.task_leg() else {
        return;
    };
    let current = leg.task;

    // The task dropdown, the full width of the tile.
    ui.label(
        egui::RichText::new(format!("Waypoint {waypoint}"))
            .small()
            .weak(),
    );
    let mut chosen = current;
    egui::ComboBox::from_id_source("waypoint-task")
        .width(ui.available_width() - 8.0)
        .selected_text(egui::RichText::new(task::caption(current)).small())
        .show_ui(ui, |ui| {
            for id in task::ALL {
                ui.selectable_value(
                    &mut chosen,
                    id,
                    egui::RichText::new(task::caption(id)).small(),
                );
            }
        });
    if chosen != current {
        app.set_waypoint_task(chosen);
        return;
    }

    // The second choice, where the task has one.
    match current {
        task::LAY_MINES => years(app, ui),
        task::PATROL => patrol(app, ui),
        task::TRANSFER => transfer(app, ui),
        task::TRANSPORT => transport(app, ui),
        _ => {}
    }

    if let Some((note, warning)) = app.waypoint_task_note() {
        ui.add_space(2.0);
        let text = egui::RichText::new(note).small();
        ui.label(if warning {
            text.color(egui::Color32::from_rgb(0xff, 0x60, 0x60))
        } else {
            text.weak()
        });
    }
}

/// Lay Mine Field's duration: one to five years, then indefinitely.
///
/// The list is built from string `0x385` (` for %d year%c`, the `%c` being a
/// space or an `s`) and then string `0x386` — which reads `iindefinitely` in
/// the shipped table, a doubled letter where the other entries have a leading
/// space. The typo is the game's; the wording here is not corrected because
/// the table is transcribed, not rewritten.
fn years(app: &mut App, ui: &mut egui::Ui) {
    let caption = |index: u16| -> String {
        match index {
            0..=4 => format!(
                " for {} year{}",
                index + 1,
                if index == 0 { " " } else { "s" }
            ),
            _ => "iindefinitely".to_string(),
        }
    };
    let now = app.waypoint_task_word(0);
    let mut chosen = now.min(5);
    egui::ComboBox::from_id_source("waypoint-mine-years")
        .selected_text(egui::RichText::new(caption(chosen)).small())
        .show_ui(ui, |ui| {
            for index in 0..=5u16 {
                ui.selectable_value(
                    &mut chosen,
                    index,
                    egui::RichText::new(caption(index)).small(),
                );
            }
        });
    if chosen != now {
        app.set_waypoint_task_word(0, chosen);
    }
}

/// Patrol's range, which the original labels `Intercept` and keeps in **word
/// one** — word zero is the warp it patrols at.
///
/// The entries come from `stars_core::patrol::patrol_range`, so what the tile
/// offers is exactly what the engine honours. See `docs/ui/fleet-pane.md` for
/// where that parts company with the original's own list.
fn patrol(app: &mut App, ui: &mut egui::Ui) {
    let caption = |index: u16| -> String {
        let range = stars_core::patrol::patrol_range(index);
        if range >= 10_000 {
            " any enemy".to_string()
        } else {
            format!(" within {range} l.y.")
        }
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Intercept").small());
        let now = app.waypoint_task_word(1);
        let mut chosen = now.min(10);
        egui::ComboBox::from_id_source("waypoint-patrol-range")
            .selected_text(egui::RichText::new(caption(chosen)).small())
            .show_ui(ui, |ui| {
                for index in 0..=10u16 {
                    ui.selectable_value(
                        &mut chosen,
                        index,
                        egui::RichText::new(caption(index)).small(),
                    );
                }
            });
        if chosen != now {
            app.set_waypoint_task_word(1, chosen);
        }
    });
}

/// Transfer Fleet's recipient: every player but you, named as the original
/// names them — `PszPlayerName` with capital, plural and "the" all set.
fn transfer(app: &mut App, ui: &mut egui::Ui) {
    let others = app.relations_others();
    let name = |app: &App, player: usize| -> String { app.psz_player_name(player) };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("To").small());
        let now = app.waypoint_task_word(0);
        let mut chosen = now;
        let showing = name(app, usize::from(now));
        egui::ComboBox::from_id_source("waypoint-transfer-to")
            .selected_text(egui::RichText::new(showing).small())
            .show_ui(ui, |ui| {
                for other in &others {
                    let label = name(app, *other);
                    ui.selectable_value(
                        &mut chosen,
                        u16::try_from(*other).unwrap_or(0),
                        egui::RichText::new(label).small(),
                    );
                }
            });
        if chosen != now {
            app.set_waypoint_task_word(0, chosen);
        }
    });
}

/// Transport's cargo table: an action and a quantity for each of the five
/// kinds, listed fuel first as the original's dropdown lists them.
///
/// The quantity box is greyed for the four actions that need no figure, and
/// its unit follows the cargo — kilotons, hundreds of colonists, milligrams —
/// except that a percentage action overrides all three.
fn transport(app: &mut App, ui: &mut egui::Ui) {
    for slot in stars_formats::CARGO_ORDER {
        let (action, quantity) = app.waypoint_transport(slot);
        let fuel = slot == 4;
        let mut chosen = action;
        let mut amount = quantity;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{}:", stars_formats::cargo_name(slot))).small());
            egui::ComboBox::from_id_source(("waypoint-xfer", slot))
                .width(130.0)
                .selected_text(egui::RichText::new(action.caption(fuel)).small())
                .show_ui(ui, |ui| {
                    for option in stars_formats::XferAction::ALL {
                        ui.selectable_value(
                            &mut chosen,
                            option,
                            egui::RichText::new(option.caption(fuel)).small(),
                        );
                    }
                });
            if chosen.needs_quantity() {
                ui.add(egui::DragValue::new(&mut amount).range(0..=0x0fff));
                ui.label(egui::RichText::new(stars_formats::cargo_unit(slot, chosen)).small());
            }
        });
        if chosen != action || amount != quantity {
            app.set_waypoint_transport(slot, chosen, amount);
        }
    }
}

/// **Prev**, **Next** and **Rename** across the foot of the fleet's own tile.
///
/// `rghwndBtn[4]`, `[5]` and `[6]` in `ShipCommandProc` (`1050:2640`). Prev
/// and Next are `SelectAdjFleet(-1, 0)` and `SelectAdjFleet(1, 0)`, which walk
/// **your own** fleets and wrap round; the tutorial leans on them, and on the
/// `n` key that does the same thing.
fn walk_buttons(app: &mut App, ui: &mut egui::Ui) {
    let mine = !app.own_fleets().is_empty();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(mine, egui::Button::new(egui::RichText::new("Prev").small()))
            .clicked()
        {
            app.select_adjacent_fleet(-1);
        }
        if ui
            .add_enabled(mine, egui::Button::new(egui::RichText::new("Next").small()))
            .clicked()
        {
            app.select_adjacent_fleet(1);
        }
        // Rename opens a dialog of its own in the original; the fleet's name
        // is edited from the Fleets screen here.
        ui.add_enabled(
            false,
            egui::Button::new(egui::RichText::new("Rename").small()),
        )
        .on_disabled_hover_text("Rename a fleet from the Fleets screen.");
    });
}
