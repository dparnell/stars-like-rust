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
            // The location tile is its title — the planet, or `In Deep
            // Space` — and two buttons, `DrawShipPlanet` (`1050:17b6`): three
            // columns of `(width - 16) / 3`, a line and a half tall, four
            // under the title bar, with **Goto** in the first (`rghwndBtn[3]`,
            // `SelectAdjPlanet(0, sel.fl.idPlanet)`) and, in the third
            // (`rghwndBtn[7]`), **Xfer** at a planet or **Jettison** in deep
            // space.
            let (orbiting, mine) = app
                .survey_subject()
                .fleet_index()
                .and_then(|i| app.game.as_ref()?.fleets.get(i))
                .map_or((false, false), |f| {
                    (
                        f.orbiting.is_some(),
                        usize::try_from(f.owner).is_ok_and(|o| o == app.local_player()),
                    )
                });
            let line = ui.text_style_height(&egui::TextStyle::Small);
            // `dyArial8 * 3 >> 1`: whole pixels, as the original's are.
            let tall = (line * 3.0 / 2.0).floor();
            // `((right - 4) - (left + 4) - 16) / 3`: three columns between
            // four-pixel margins, with sixteen for the two gaps.
            let width = ((ui.available_width() - 24.0) / 3.0).floor().max(1.0);
            let top = ui.cursor().top() + 4.0;
            let left = ui.max_rect().left();
            let column = move |n: f32| {
                egui::Rect::from_min_size(
                    egui::pos2(left + 4.0 + n * (width + 8.0), top),
                    egui::vec2(width, tall),
                )
            };
            if crate::views::placed_button(app, ui, column(0.0), "Goto", orbiting).clicked() {
                app.goto_orbited_planet();
            }
            if orbiting {
                if crate::views::placed_button(app, ui, column(2.0), "Xfer", mine).clicked() {
                    app.open_xfer();
                }
            } else {
                // `idsJettison2` in deep space, dead while salvage lies at
                // the spot (`DrawShipPlanet`, `1050:17b6`); it raises the
                // same dialog with deep space on the right.
                let can = mine && app.can_jettison();
                if crate::views::placed_button(app, ui, column(2.0), "Jettison", can).clicked() {
                    app.open_xfer();
                }
            }
            ui.allocate_space(egui::vec2(ui.available_width(), tall + 4.0));
        }
        2 => waypoints(app, ui),
        3 => waypoint_task(app, ui),
        4 => fuel_and_cargo(app, ui),
        5 => {
            crate::views::planet::grid(ui, "composition", &app.fleet_composition_tile(), false);
            battle_plan_row(app, ui);
            crate::views::planet::grid(
                ui,
                "composition-figures",
                &app.fleet_composition_figures(),
                false,
            );
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
    // Remote Mining over a planet the robots can dig: `Mining Rate per
    // Year:` and the three figures in the mineral colours, each followed
    // by `kT` (`DrawShipWayPtOrders`, `EstMineralsMined`).
    if let Some(rate) = app.waypoint_mining_rate() {
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new("Mining Rate per Year:")
                .small()
                .strong(),
        );
        ui.horizontal(|ui| {
            for (mineral, figure) in rate.iter().enumerate() {
                ui.label(
                    egui::RichText::new(format!("{figure}kT"))
                        .small()
                        .color(crate::views::mineral_colour(mineral)),
                );
            }
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
    // Under the dropdown, `Warp Factor` and a gauge (`rgrcRef[18]`,
    // `DrawFleetGauge` with no fleet): the warp the fleet patrols at, task
    // word 0, 0 to 10, set by a press or drag along the bar
    // (`ClickInShipOrders`, its `0x12` case).
    let line = ui.text_style_height(&egui::TextStyle::Small);
    let mine = app
        .pane_fleet()
        .is_some_and(|f| usize::try_from(f.owner).is_ok_and(|owner| owner == app.local_player()));
    let warp = app.waypoint_task_word(0).min(10);
    let top = ui.cursor().min;
    let label_width = 64.0;
    ui.painter().text(
        top,
        egui::Align2::LEFT_TOP,
        "Warp Factor",
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().text_color(),
    );
    let bar = egui::Rect::from_min_size(
        top + egui::vec2(label_width + 4.0, 0.0),
        egui::vec2((ui.available_width() - label_width - 6.0).max(20.0), line),
    );
    let response = ui.interact(
        bar,
        ui.id().with("patrol-warp-gauge"),
        if mine {
            egui::Sense::click_and_drag()
        } else {
            egui::Sense::hover()
        },
    );
    crate::views::record(app, ui, "Patrol warp gauge", &response);
    let painter = ui.painter();
    painter.rect_filled(bar, 0.0, ui.visuals().extreme_bg_color);
    let filled = bar.width() * f32::from(warp) / 10.0;
    painter.rect_filled(
        egui::Rect::from_min_size(bar.min, egui::vec2(filled, bar.height())),
        0.0,
        egui::Color32::from_rgb(0x30, 0x80, 0xd0),
    );
    painter.rect_stroke(
        bar,
        0.0,
        egui::Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
    );
    painter.text(
        bar.center(),
        egui::Align2::CENTER_CENTER,
        format!("Warp {warp}"),
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().text_color(),
    );
    if mine && (response.dragged() || response.clicked()) {
        if let Some(p) = response.interact_pointer_pos() {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let chosen = (((p.x - bar.left()) / bar.width()) * 11.0)
                .floor()
                .clamp(0.0, 10.0) as u16;
            if chosen != warp {
                app.set_waypoint_task_word(0, chosen);
            }
        }
    }
    ui.allocate_space(egui::vec2(ui.available_width(), line));
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
/// The Transport task's cargo table, three controls deep as the original
/// draws it (`DrawShipWayPtOrders`, `UpdateOrdersDDs` `1050:93ee`): a
/// **cargo** dropdown — the tile's second — listing fuel and then the four
/// holds, an **action** dropdown for the cargo chosen — the third — and a
/// quantity box beside it when the action wants one. Page 35 of the
/// tutorial names them so: "make the tile's second dropdown read Colonists
/// and its third Unload All". The blue diamond sits beside the cargo row.
fn transport(app: &mut App, ui: &mut egui::Ui) {
    let shown = app
        .transport_cargo_shown
        .min(stars_formats::CARGO_ORDER.len() - 1);
    let slot = stars_formats::CARGO_ORDER[shown];
    let (action, quantity) = app.waypoint_transport(slot);
    let fuel = slot == 4;

    // The cargo dropdown, and the diamond beside it.
    let mut pick = shown;
    ui.horizontal(|ui| {
        let cargo = egui::ComboBox::from_id_source("waypoint-xfer-cargo")
            .width(110.0)
            .selected_text(egui::RichText::new(stars_formats::cargo_name(slot)).small())
            .show_ui(ui, |ui| {
                for (index, slot) in stars_formats::CARGO_ORDER.iter().enumerate() {
                    let response = ui.selectable_value(
                        &mut pick,
                        index,
                        egui::RichText::new(stars_formats::cargo_name(*slot)).small(),
                    );
                    crate::views::record(app, ui, stars_formats::cargo_name(*slot), &response);
                }
            });
        crate::views::record(app, ui, "Cargo", &cargo.response);
        zip_diamond(app, ui);
    });
    if pick != shown {
        app.transport_cargo_shown = pick;
        return;
    }

    // The action for it, and the quantity where the action takes one.
    let mut chosen = action;
    let mut amount = quantity;
    ui.horizontal(|ui| {
        let actions = egui::ComboBox::from_id_source("waypoint-xfer-action")
            .width(130.0)
            .selected_text(egui::RichText::new(action.caption(fuel)).small())
            .show_ui(ui, |ui| {
                for option in stars_formats::XferAction::ALL {
                    let response = ui.selectable_value(
                        &mut chosen,
                        option,
                        egui::RichText::new(option.caption(fuel)).small(),
                    );
                    crate::views::record(app, ui, option.caption(fuel), &response);
                }
            });
        crate::views::record(app, ui, "Action", &actions.response);
        if chosen.needs_quantity() {
            ui.add(egui::DragValue::new(&mut amount).range(0..=0x0fff));
            ui.label(egui::RichText::new(stars_formats::cargo_unit(slot, chosen)).small());
        }
    });
    if chosen != action || amount != quantity {
        app.set_waypoint_transport(slot, chosen, amount);
    }
}

/// The **Fuel & Cargo** tile, `DrawShipCargo` (`1050:1a54`): `Fuel` and a
/// gauge, `Cargo` and a gauge — each gauge from the wider of the two labels
/// to four from the edge, a line tall, the rows a line and four apart —
/// then the three minerals and the colonists as figures, the name in its
/// own colour on the left and `%ld kT` right-aligned. The two gauges are
/// the tile's click targets (`rgrcRef[2]` and `[3]`): a press in either is
/// the Xfer button, which page 19 says in so many words.
fn fuel_and_cargo(app: &mut App, ui: &mut egui::Ui) {
    let line = ui.text_style_height(&egui::TextStyle::Small);
    let font = egui::TextStyle::Small.resolve(ui.style());
    let Some(gauges) = app.fleet_in_hand_gauges() else {
        // Somebody else's fleet: figures only, as `fleet_cargo_tile` gives.
        crate::views::planet::grid(ui, "fuel-cargo", &app.fleet_cargo_tile(), false);
        return;
    };
    let mine = true;
    let rect = ui.max_rect();
    let painter = ui.painter().clone();
    let colour = |[r, g, b]: [u8; 3]| egui::Color32::from_rgb(r, g, b);
    let text = ui.visuals().text_color();
    let label_width = ["Fuel", "Cargo"]
        .iter()
        .map(|s| {
            ui.fonts(|f| f.layout_no_wrap((*s).to_string(), font.clone(), text))
                .rect
                .width()
        })
        .fold(0.0_f32, f32::max)
        .ceil();
    let left = rect.left() + 4.0;
    let right = rect.right() - 4.0;
    let mut y = rect.top() + 1.0;
    let mut open = false;

    for (label, gauge) in [
        (
            "Fuel",
            crate::survey::Gauge {
                segments: vec![(gauges.fuel, crate::survey::CARGO_COLOURS[4])],
                total: gauges.fuel_capacity,
                label: format!("{} of {}mg", gauges.fuel, gauges.fuel_capacity),
            },
        ),
        (
            "Cargo",
            crate::survey::Gauge {
                segments: (0..3)
                    .map(|i| (gauges.minerals[i], crate::survey::CARGO_COLOURS[i]))
                    .chain(std::iter::once((
                        gauges.colonists,
                        crate::survey::CARGO_COLOURS[3],
                    )))
                    .collect(),
                total: gauges.cargo_capacity,
                label: format!("{} of {}kT", gauges.cargo(), gauges.cargo_capacity),
            },
        ),
    ] {
        painter.text(
            egui::pos2(left, y),
            egui::Align2::LEFT_TOP,
            label,
            font.clone(),
            text,
        );
        let bar = egui::Rect::from_min_max(
            egui::pos2(left + label_width + 4.0, y),
            egui::pos2(right, y + line),
        );
        crate::views::survey::gauge_bar(ui, &painter, &gauge, bar, &font);
        let response = ui.interact(
            bar,
            ui.id().with(("cargo-gauge", label)),
            if mine {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            },
        );
        crate::views::record(app, ui, &format!("{label} gauge"), &response);
        if response.clicked() {
            open = true;
        }
        y += line + 4.0;
    }
    if open {
        app.open_xfer();
    }

    let names = ["Ironium", "Boranium", "Germanium", "Colonists"];
    let amounts = [
        gauges.minerals[0],
        gauges.minerals[1],
        gauges.minerals[2],
        gauges.colonists,
    ];
    for (index, (name, amount)) in names.iter().zip(amounts).enumerate() {
        painter.text(
            egui::pos2(left, y),
            egui::Align2::LEFT_TOP,
            *name,
            font.clone(),
            colour(crate::survey::CARGO_COLOURS[index]),
        );
        painter.text(
            egui::pos2(right, y),
            egui::Align2::RIGHT_TOP,
            format!("{amount} kT"),
            font.clone(),
            text,
        );
        y += line;
    }
    ui.allocate_space(egui::vec2(ui.available_width(), (y - rect.top()).max(0.0)));
}

/// The **Fleet Waypoints** tile, `DrawShipOrders` (`1050:0000`): a list
/// box four rows tall of the fleet's waypoints, then the figures for the
/// leg in hand — where it is coming from or going, the distance, the warp,
/// the travel time and the fuel — and along the foot the **Repeat Orders**
/// checkbox (`BM_SETCHECK` from `fRepOrders`) with a blue diamond at the
/// right. The list is the other way to take a waypoint in hand: page 17
/// says "click its waypoint at Shaggy Dog", and either the map or this
/// list will do.
fn waypoints(app: &mut App, ui: &mut egui::Ui) {
    let line = ui.text_style_height(&egui::TextStyle::Small);
    let Some(index) = app.survey_subject().fleet_index() else {
        return;
    };
    let (legs, repeat, mine) = {
        let Some(game) = app.game.as_ref() else {
            return;
        };
        let Some(fleet) = game.fleets.get(index) else {
            return;
        };
        let legs: Vec<(usize, String)> = fleet
            .waypoints
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, w)| (i, app.location_name(w.target_class, w.target, w.position)))
            .collect();
        (
            legs,
            fleet.repeat_orders,
            usize::try_from(fleet.owner).is_ok_and(|o| o == app.local_player()),
        )
    };

    // The list box: four rows, framed — `(dyArial8 + 2) * 4` in the
    // original, a pixel a row less here so the checkbox at the foot fits
    // inside the tile rather than over its frame.
    let list = egui::Rect::from_min_size(
        ui.cursor().min + egui::vec2(2.0, 2.0),
        egui::vec2(ui.available_width() - 4.0, (line + 1.0) * 4.0),
    );
    ui.painter()
        .rect_filled(list, 0.0, ui.visuals().extreme_bg_color);
    ui.painter().rect_stroke(
        list,
        0.0,
        egui::Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
    );
    let mut picked = None;
    {
        let inner = list.shrink(2.0);
        let mut child = ui.child_ui(inner, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(inner);
        child.spacing_mut().item_spacing.y = 0.0;
        child.spacing_mut().button_padding.y = 0.0;
        child.spacing_mut().interact_size.y = line;
        egui::ScrollArea::vertical()
            .id_source("fleet-waypoints")
            .show(&mut child, |ui| {
                for (i, name) in &legs {
                    let response = ui.selectable_label(
                        app.selection.waypoint == Some(*i),
                        egui::RichText::new(name).small(),
                    );
                    crate::views::record(app, ui, name, &response);
                    if response.clicked() {
                        picked = Some(*i);
                    }
                }
            });
    }
    if let Some(i) = picked {
        app.selection.waypoint = Some(i);
    }
    ui.allocate_space(egui::vec2(ui.available_width(), list.height() + 4.0));

    // The figures, with the warp drawn as the gauge it is in the original
    // (`rgrcRef[0]`) rather than as a number: a bar of eleven steps, warp
    // 0 to 10, that a drag or a click sets the leg in hand to.
    let rows: Vec<(String, String)> = app
        .fleet_waypoints_tile()
        .into_iter()
        .filter(|(label, _)| label != "Warp Factor")
        .collect();
    let (first, rest) = rows.split_at(rows.len().min(1));
    crate::views::planet::grid(ui, "waypoints-to", first, false);
    if let Some((waypoint, warp)) = app.task_waypoint().and_then(|w| {
        let leg = app.task_leg()?;
        Some((w, leg.warp))
    }) {
        let top = ui.cursor().min;
        let label_width = 64.0;
        ui.painter().text(
            top + egui::vec2(0.0, 0.0),
            egui::Align2::LEFT_TOP,
            "Warp Factor",
            egui::TextStyle::Small.resolve(ui.style()),
            ui.visuals().text_color(),
        );
        let bar = egui::Rect::from_min_size(
            top + egui::vec2(label_width + 4.0, 0.0),
            egui::vec2((ui.available_width() - label_width - 6.0).max(20.0), line),
        );
        let response = ui.interact(
            bar,
            ui.id().with("warp-gauge"),
            if mine {
                egui::Sense::click_and_drag()
            } else {
                egui::Sense::hover()
            },
        );
        crate::views::record(app, ui, "Warp gauge", &response);
        let painter = ui.painter();
        painter.rect_filled(bar, 0.0, ui.visuals().extreme_bg_color);
        let filled = bar.width() * f32::from(warp) / 10.0;
        painter.rect_filled(
            egui::Rect::from_min_size(bar.min, egui::vec2(filled, bar.height())),
            0.0,
            egui::Color32::from_rgb(0x30, 0x80, 0xd0),
        );
        painter.rect_stroke(
            bar,
            0.0,
            egui::Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
        );
        painter.text(
            bar.center(),
            egui::Align2::CENTER_CENTER,
            format!("Warp {warp}"),
            egui::TextStyle::Small.resolve(ui.style()),
            ui.visuals().text_color(),
        );
        if mine && (response.dragged() || response.clicked()) {
            if let Some(p) = response.interact_pointer_pos() {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let chosen = (((p.x - bar.left()) / bar.width()) * 11.0)
                    .floor()
                    .clamp(0.0, 10.0) as u8;
                app.set_waypoint_warp(waypoint, chosen);
            }
        }
        ui.allocate_space(egui::vec2(ui.available_width(), line));
    }
    crate::views::planet::grid(ui, "waypoints", rest, false);

    // Repeat Orders, along the foot: a line tall, as a Windows checkbox is.
    ui.spacing_mut().interact_size.y = line;
    ui.spacing_mut().button_padding.y = 0.0;
    let mut on = repeat;
    let response = ui.add_enabled(
        mine,
        egui::Checkbox::new(&mut on, egui::RichText::new("Repeat Orders").small()),
    );
    crate::views::record(app, ui, "Repeat Orders", &response);
    if response.changed() {
        app.set_repeat_orders(index, on);
    }
}

/// The **Battle Plan:** row under the Fleet Composition tile's ship list
/// (`DrawFleetComp`, `1050:1e72`): the label in bold and `hwndBattleDD`
/// beside it, filled by `FillBattleDD` (`1050:9a36`) with `Battle Plans...`
/// first and then the player's plans by name, the fleet's plan selected
/// one down. `ShipCommandProc` answers a pick: the first entry raises the
/// Battle Plans dialog, any other sets the fleet's `iplan` to its index
/// less one. Recorded as `Battle Plan`, with the entries by their names.
fn battle_plan_row(app: &mut App, ui: &mut egui::Ui) {
    let Some(fleet) = app.pane_fleet() else {
        return;
    };
    let mine = usize::try_from(fleet.owner).is_ok_and(|o| o == app.local_player());
    let current = usize::from(fleet.battle_plan);
    let names: Vec<String> = app
        .battle_plan_list()
        .iter()
        .map(|plan| plan.name.clone())
        .collect();
    let showing = names
        .get(current)
        .cloned()
        .unwrap_or_else(|| "Battle Plans...".to_string());
    let mut pick: Option<usize> = None;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Battle Plan:").small().strong());
        ui.add_enabled_ui(mine, |ui| {
            let dropdown = egui::ComboBox::from_id_source("fleet-battle-plan")
                .width(ui.available_width() - 8.0)
                .selected_text(egui::RichText::new(showing).small())
                .show_ui(ui, |ui| {
                    let response =
                        ui.selectable_label(false, egui::RichText::new("Battle Plans...").small());
                    crate::views::record(app, ui, "Battle Plans...", &response);
                    if response.clicked() {
                        pick = Some(0);
                    }
                    for (slot, name) in names.iter().enumerate() {
                        let response =
                            ui.selectable_label(slot == current, egui::RichText::new(name).small());
                        crate::views::record(app, ui, name, &response);
                        if response.clicked() {
                            pick = Some(slot + 1);
                        }
                    }
                });
            crate::views::record(app, ui, "Battle Plan", &dropdown.response);
        });
    });
    match pick {
        Some(0) => app.open_battle_plans(),
        Some(entry) => {
            if let Some(index) = app.survey_subject().fleet_index() {
                if let Ok(slot) = u8::try_from(entry - 1) {
                    if usize::from(slot) != current {
                        app.set_battle_plan(index, slot);
                    }
                }
            }
        }
        None => {}
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
/// Split itself opens the Ship Transfer dialog (`split.rs`).
fn split_buttons(app: &mut App, ui: &mut egui::Ui) {
    let mine = app
        .survey_subject()
        .fleet_index()
        .and_then(|i| app.game.as_ref()?.fleets.get(i))
        .is_some_and(|f| usize::try_from(f.owner).is_ok_and(|owner| owner == app.local_player()));
    let several = app.pane_fleet_ships() > 1;
    ui.horizontal(|ui| {
        if crate::views::flow_button(app, ui, "Split", mine && several)
            .on_hover_text("Move ships to a fleet of their own, one at a time.")
            .clicked()
        {
            app.open_split();
        }
        if crate::views::flow_button(app, ui, "Split All", mine && several)
            .on_hover_text("Put every ship into a fleet of its own.")
            .clicked()
        {
            if let Some(index) = app.survey_subject().fleet_index() {
                app.split_all(index);
            }
        }
        // `rghwndBtn[10]`, Merge: the Merge Fleets dialog over the fleets
        // sharing this spot.
        let can_merge = app.can_merge();
        if crate::views::flow_button(app, ui, "Merge", mine && can_merge)
            .on_hover_text("Join other fleets here into this one.")
            .clicked()
        {
            app.open_merge();
        }
    });
    if app.merge.is_some() {
        merge_dialog(app, ui);
    }
}

/// `Merge Fleets` (`MergeFleetsDlg`, `1080:3376`): the fleets at the spot
/// as a list to tick, Select All and Select None, OK and Cancel. A fleet
/// with orders beyond where it stands is marked.
fn merge_dialog(app: &mut App, ui: &mut egui::Ui) {
    let Some(dialog) = app.merge.clone() else {
        return;
    };
    let was = app.drawn_scope;
    app.drawn_scope = "merge";
    let mut ticked = dialog.ticked.clone();
    let mut done: Option<bool> = None;
    let mut open = true;
    egui::Window::new("Merge Fleets")
        .open(&mut open)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            for (row, index) in dialog.fleets.iter().enumerate() {
                let label = app.merge_row(*index);
                let on = ticked.get(row).copied().unwrap_or(false);
                let response = ui.selectable_label(on, egui::RichText::new(&label).small());
                crate::views::record(app, ui, &label, &response);
                if response.clicked() {
                    if let Some(slot) = ticked.get_mut(row) {
                        *slot = !*slot;
                    }
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if crate::views::flow_button(app, ui, "Select All", true).clicked() {
                    ticked.iter_mut().for_each(|t| *t = true);
                }
                if crate::views::flow_button(app, ui, "Select None", true).clicked() {
                    ticked.iter_mut().for_each(|t| *t = false);
                }
            });
            ui.horizontal(|ui| {
                if crate::views::flow_button(app, ui, "OK", true).clicked() {
                    done = Some(true);
                }
                if crate::views::flow_button(app, ui, "Cancel", true).clicked() {
                    done = Some(false);
                }
            });
        });
    app.drawn_scope = was;
    if let Some(dialog) = app.merge.as_mut() {
        dialog.ticked = ticked;
    }
    match done {
        Some(true) => {
            app.merge_ok();
        }
        Some(false) => app.merge_cancel(),
        None => {
            if !open {
                app.merge_cancel();
            }
        }
    }
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
    crate::views::record(app, ui, "blue diamond", &response);

    let mut chosen: Option<usize> = None;
    // The original raises it on the **right** button; a left click is
    // offered too, since a menu you cannot find is no menu at all. The
    // menu's entries are recorded as they are drawn, so a test can pick
    // QuikDrop the way page 15 says.
    let labels = app.zip_menu();
    response.context_menu(|ui| {
        for (index, label) in labels.iter().enumerate() {
            let usable = index < 2
                || index == App::ZIP_ORDERS + 2
                || app
                    .zip_orders
                    .get(index - 2)
                    .is_some_and(|slot| !slot.name.is_empty());
            let entry = ui.add_enabled(
                usable,
                egui::Button::new(egui::RichText::new(label).small()),
            );
            // Recorded against the menu's own clip, not the tile's.
            crate::views::record(app, ui, label, &entry);
            if entry.clicked() {
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
    let mut done = false;
    let mut picked = slot;
    // Its widgets are recorded under a scope of their own, the fleet tile
    // behind it having buttons of the same names.
    let was = app.drawn_scope;
    app.drawn_scope = "zip";
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
                let radio = ui.radio_value(&mut picked, index, egui::RichText::new(&name).small());
                crate::views::record(app, ui, &name, &radio);
            }
            ui.separator();
            ui.horizontal(|ui| {
                let import = ui.button("Import");
                crate::views::record(app, ui, "Import", &import);
                if import.clicked() {
                    let name = format!("Custom {}", picked + 1);
                    app.zip_import(picked, &name);
                }
                let filled = !app.zip_orders[picked].name.is_empty();
                let delete = ui.add_enabled(filled, egui::Button::new("Delete"));
                crate::views::record(app, ui, "Delete", &delete);
                if delete.clicked() {
                    app.zip_delete(picked);
                }
            });
            if !app.zip_orders[picked].name.is_empty() {
                ui.add_space(2.0);
                ui.text_edit_singleline(&mut app.zip_orders[picked].name);
            }
            ui.add_space(4.0);
            let ok = ui.button("OK");
            crate::views::record(app, ui, "OK", &ok);
            if ok.clicked() {
                done = true;
            }
        });
    app.drawn_scope = was;
    app.zip_dialog = (open && !done).then_some(picked);
}
