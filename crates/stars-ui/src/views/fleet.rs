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
    app.drawn_scope = "fleet";
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
        0 => picture_tile(app, ui),
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
        5 => {
            crate::views::planet::grid(ui, "composition", &app.fleet_composition_tile(), false);
            split_buttons(app, ui);
        }
        _ => crate::views::fleets_here_body(app, ui),
    }
}

/// The tallest tile: the fleet itself, as a picture, with Prev, Next and
/// Rename in a column beside it.
///
/// `DrawPlanShipBitmap` (`1048:3336`) puts the picture twelve pixels in and
/// six down (two, in the small layout) and blits it 64 square —
/// `DrawFleetBitmap`, the primary design's ship with the owner's emblem over
/// its bottom-left corner — and then stands the three buttons
/// (`rghwndBtn[4..=6]`) in a column to its right: each `dyArial8 * 3 / 2`
/// tall, three pixels apart, starting two pixels above the picture, and as
/// wide as the tile's inside less 95. Without the game's own pictures the
/// tile says in words what the picture would have shown.
fn picture_tile(app: &mut App, ui: &mut egui::Ui) {
    let Some(fleet) = app.pane_fleet() else {
        ui.label(egui::RichText::new("no fleet selected").weak().small());
        return;
    };
    let ships: i32 = fleet.stacks.iter().map(|s| s.count).sum();
    let owner = fleet.owner;
    let body = ui.max_rect();
    let small = app.window_layout == crate::WindowLayout::Small;
    let line = ui.text_style_height(&egui::TextStyle::Small);
    // The tile's inside edge is two in from its frame; the body here starts
    // there, so the twelve becomes ten.
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;
    let top = body.top() + if small { 0.0 } else { 4.0 };

    let picture = app.fleet_picture();
    let emblem = app.fleet_emblem(stars_formats::resources::art::EmblemSize::Medium);
    let square = egui::Rect::from_min_size(egui::pos2(left, top), egui::vec2(64.0, 64.0));
    let mut drawn = false;
    if let Some((cell, distinct)) = picture {
        let mut child = ui.child_ui(square, egui::Layout::top_down(egui::Align::Min), None);
        if crate::art::draw(app, &mut child, cell, 64.0) {
            drawn = true;
            if let Some(emblem) = emblem {
                let ctx = ui.ctx().clone();
                if let Some(art) = app.art.as_mut() {
                    if let Some(image) = art.sprite(&ctx, emblem, 16.0) {
                        image.paint_at(
                            ui,
                            egui::Rect::from_min_size(
                                square.min + egui::vec2(0.0, 48.0),
                                egui::vec2(16.0, 16.0),
                            ),
                        );
                    }
                }
            }
            // The original marks a mixed fleet beside the picture rather
            // than drawing every design in it.
            if distinct > 1 {
                ui.painter().text(
                    square.right_top() + egui::vec2(2.0, 0.0),
                    egui::Align2::LEFT_TOP,
                    format!("+{}", distinct - 1),
                    egui::TextStyle::Small.resolve(ui.style()),
                    ui.visuals().text_color(),
                );
            }
        }
    }
    if !drawn {
        let mut child = ui.child_ui(
            egui::Rect::from_min_size(egui::pos2(left, top), egui::vec2(85.0, 64.0)),
            egui::Layout::top_down(egui::Align::Min),
            None,
        );
        child.label(egui::RichText::new(format!("player {}", owner + 1)).small());
        child.label(egui::RichText::new(format!("{ships} ships")).small());
    }

    // The column of buttons.
    let width = (right - left - 95.0).max(40.0);
    let height = (line * 3.0 / 2.0 - if small { 2.0 } else { 0.0 }).floor();
    let gap = if small { 2.0 } else { 3.0 };
    let mut y = top - if small { 2.0 } else { 4.0 };
    let mine = !app.own_fleets().is_empty();
    for (label, delta) in [("Prev", -1), ("Next", 1)] {
        let rect =
            egui::Rect::from_min_size(egui::pos2(right - width, y), egui::vec2(width, height));
        if crate::views::placed_button(app, ui, rect, label, mine).clicked() {
            app.select_adjacent_fleet(delta);
        }
        y += height + gap;
    }
    // Rename opens a dialog of its own in the original; the fleet's name is
    // edited from the Fleets screen here.
    let rect = egui::Rect::from_min_size(egui::pos2(right - width, y), egui::vec2(width, height));
    crate::views::placed_button(app, ui, rect, "Rename", false)
        .on_disabled_hover_text("Rename a fleet from the Fleets screen.");
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

    // Which waypoint the tile is about. The original keeps this in the
    // Fleet Waypoints tile's orders listbox (`FillOrdersLB`, `1050:928c`,
    // with `SetOrdersLbSel` moving the selection); here it is a row of
    // numbers, which does the same job in the room a tile has.
    //
    // **Zero is on it.** Waypoint zero is where the fleet already is, and
    // Lay Mine Field is a task you give there rather than somewhere you are
    // going — page 61 of the tutorial is exactly that. Zero cannot be
    // dragged or deleted, but its task is yours to set.
    let count = app
        .survey_subject()
        .fleet_index()
        .and_then(|i| app.game.as_ref()?.fleets.get(i))
        .map_or(0, |f| f.waypoints.len());
    if count > 1 {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Waypoint").small().weak());
            for index in 0..count {
                if ui
                    .add(egui::SelectableLabel::new(
                        index == waypoint,
                        egui::RichText::new(index.to_string()).small(),
                    ))
                    .clicked()
                {
                    app.selection.waypoint = Some(index);
                }
            }
        });
    } else {
        ui.label(
            egui::RichText::new(format!("Waypoint {waypoint}"))
                .small()
                .weak(),
        );
    }
    let mut chosen = current;
    // The dropdown and, while it is open, its choices are recorded like the
    // buttons, under the caption each shows, so a test can open it and pick
    // a task the way the tutorial's pages say to.
    let dropdown = egui::ComboBox::from_id_source("waypoint-task")
        .width(ui.available_width() - 8.0)
        .selected_text(egui::RichText::new(task::caption(current)).small())
        .show_ui(ui, |ui| {
            for id in task::ALL {
                let response = ui.selectable_value(
                    &mut chosen,
                    id,
                    egui::RichText::new(task::caption(id)).small(),
                );
                crate::views::record(app, ui, task::caption(id), &response);
            }
        });
    crate::views::record(app, ui, "Waypoint Task", &dropdown.response);
    if chosen != current {
        app.set_waypoint_task(chosen);
        return;
    }

    // The second choice, where the task has one.
    match current {
        task::LAY_MINES => years(app, ui),
        task::PATROL => patrol(app, ui),
        task::TRANSFER => transfer(app, ui),
        task::TRANSPORT => {
            transport(app, ui);
            zip_diamond(app, ui);
        }
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

/// **Split** and **Split All** across the foot of the Fleet Composition tile.
///
/// `rghwndBtn[9]` is Split All — `FFleetSplitAll` (`1038:3a00`), which puts
/// every ship after the first into a fleet of its own and shares the cargo
/// out as it goes. The tutorial uses it to stop two colony ships going to the
/// same planet: *"We don't want both colonizers to go to Slime so hit the
/// Split All button in the Fleet Composition tile."*
///
/// Split itself opens a dialog for moving ships one at a time, which this
/// project does from the Fleets screen instead.
fn split_buttons(app: &mut App, ui: &mut egui::Ui) {
    let mine = app
        .survey_subject()
        .fleet_index()
        .and_then(|i| app.game.as_ref()?.fleets.get(i))
        .is_some_and(|f| usize::try_from(f.owner).is_ok_and(|owner| owner == app.local_player()));
    let several = app.pane_fleet_ships() > 1;
    ui.horizontal(|ui| {
        ui.add_enabled(
            false,
            egui::Button::new(egui::RichText::new("Split").small()),
        )
        .on_disabled_hover_text("Split a fleet ship by ship from the Fleets screen.");
        if ui
            .add_enabled(
                mine && several,
                egui::Button::new(egui::RichText::new("Split All").small()),
            )
            .on_hover_text("Put every ship into a fleet of its own.")
            .clicked()
        {
            if let Some(index) = app.survey_subject().fleet_index() {
                app.split_all(index);
            }
        }
    });
}

/// The **blue diamond** beside the Transport cargo table, and the menu it
/// raises.
///
/// `DrawShipWayPtOrders` (`1050:0912`) draws it with `DrawDiamond(hdc, rc,
/// hbrBBlue)` and remembers where it put it in `rgrcRef[5]`, which is what
/// makes it a click target. Right-clicking it offers the saved cargo orders:
/// the two built-in ones, the four custom slots (`vrgZip`), and
/// `<Customize>`, which opens the dialog that fills them in.
///
/// The tutorial leans on it repeatedly — *"right click on the blue diamond
/// in the Waypoint Task tile and select QuikDrop to empty the freighter's
/// hold at 90210"*.
fn zip_diamond(app: &mut App, ui: &mut egui::Ui) {
    let line = ui.text_style_height(&egui::TextStyle::Small);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(line + 4.0, line + 2.0), egui::Sense::click());
    let centre = rect.center();
    let half = line / 2.0;
    let blue = egui::Color32::from_rgb(
        crate::dialog::DIAMOND[0],
        crate::dialog::DIAMOND[1],
        crate::dialog::DIAMOND[2],
    );
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(centre.x, centre.y - half),
            egui::pos2(centre.x + half, centre.y),
            egui::pos2(centre.x, centre.y + half),
            egui::pos2(centre.x - half, centre.y),
        ],
        blue,
        egui::Stroke::NONE,
    ));
    let response = response.on_hover_text(
        "Right-click for the saved cargo orders, or to define one from this waypoint.",
    );

    let mut chosen: Option<usize> = None;
    // The original raises it on the **right** button; a left click is
    // offered too, since a menu you cannot find is no menu at all.
    response.context_menu(|ui| {
        for (index, label) in app.zip_menu().iter().enumerate() {
            let usable = index < 2
                || index == App::ZIP_ORDERS + 2
                || app
                    .zip_orders
                    .get(index - 2)
                    .is_some_and(|slot| !slot.name.is_empty());
            if ui
                .add_enabled(
                    usable,
                    egui::Button::new(egui::RichText::new(label).small()),
                )
                .clicked()
            {
                chosen = Some(index);
                ui.close_menu();
            }
        }
    });

    match chosen {
        Some(0) => {
            app.zip_quik(true);
        }
        Some(1) => {
            app.zip_quik(false);
        }
        Some(index) if index == App::ZIP_ORDERS + 2 => app.zip_dialog = Some(0),
        Some(index) => {
            app.zip_apply(index - 2);
        }
        None => {}
    }

    if let Some(slot) = app.zip_dialog {
        customize_zip(app, ui, slot);
    }
}

/// `Customize Zip Orders` (string `0x231`), `ZipOrderDlg` (`1080:0175`).
///
/// Four slots to choose between, then **Import**, which copies the waypoint's
/// own cargo table into the chosen slot and asks for a name, and **Delete**,
/// which empties it. The original's list of what a slot holds is drawn in the
/// dialog; here each slot's name carries it.
fn customize_zip(app: &mut App, ui: &mut egui::Ui, slot: usize) {
    let mut open = true;
    let mut picked = slot;
    egui::Window::new("Customize Zip Orders")
        .open(&mut open)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            for index in 0..App::ZIP_ORDERS {
                let held = app.zip_orders[index].name.clone();
                let name = if held.is_empty() {
                    format!("<Unused {}>", index + 1)
                } else {
                    held
                };
                ui.radio_value(&mut picked, index, egui::RichText::new(name).small());
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    let name = format!("Custom {}", picked + 1);
                    app.zip_import(picked, &name);
                }
                let filled = !app.zip_orders[picked].name.is_empty();
                if ui
                    .add_enabled(filled, egui::Button::new("Delete"))
                    .clicked()
                {
                    app.zip_delete(picked);
                }
            });
            if !app.zip_orders[picked].name.is_empty() {
                ui.add_space(2.0);
                ui.text_edit_singleline(&mut app.zip_orders[picked].name);
            }
        });
    app.zip_dialog = open.then_some(picked);
}
