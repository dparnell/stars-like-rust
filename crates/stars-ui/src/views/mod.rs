//! The egui screens, written once and drawn by every frontend.
//!
//! Each view takes `&mut App` and an `&mut egui::Ui` and draws into it. They
//! hold no state of their own: everything the player has chosen lives in
//! [`crate::App`], so a frontend can save and restore it, and so the views stay
//! testable through that state rather than through the widgets.

pub mod battleplans;
pub mod battles;
pub mod browser;
pub mod designer;
pub mod fleet;
pub mod galaxy;
pub mod host;
pub mod messages;
pub mod newgame;
pub mod parameters;
pub mod password;
pub mod planet;
pub mod players;
pub mod popup;
pub mod production;
pub mod race;
pub mod race_wizard;
pub mod relations;
pub mod report;
pub mod research;
pub mod score;
pub mod statusbar;
pub mod survey;
pub mod toolbar;
pub mod tutorial;

use crate::{App, Screen};

/// The name of a planetary production item.
pub(crate) fn item_name(item: u16) -> String {
    stars_core::production::item_name(item).to_string()
}

/// Draw whichever screen is selected.
///
/// Returns what the New Game wizard asked for, when it is open and the player
/// clicked something the shell has to act on (creating the game needs no
/// shell, but opening a race file does).
pub fn central(app: &mut App, ui: &mut egui::Ui) -> Option<newgame::Action> {
    if app.setup.is_some() {
        return newgame::view(app, ui);
    }
    if app.game.is_none() {
        empty(app, ui);
        return None;
    }
    match app.screen {
        Screen::Galaxy => galaxy::view(app, ui),
        Screen::Planets => report::view(app, ui, crate::report::Report::Planets),
        Screen::Fleets => report::view(app, ui, crate::report::Report::Fleets),
        Screen::EnemyFleets => report::view(app, ui, crate::report::Report::EnemyFleets),
        Screen::Battles => battles::view(app, ui),
        Screen::Players => players::view(app, ui),
    }
    None
}

/// The title screen, shown before a game is opened.
fn empty(app: &mut App, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(80.0);
        ui.heading("Stars!");
        ui.add_space(8.0);
        ui.label("Open a save file to begin — a player file (.m1 …) or a host file (.hst).");
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Planet positions come from the .xy universe file, which is looked for \
                 beside the save and one directory up.",
            )
            .weak(),
        );
        ui.add_space(16.0);
        if ui.button("Start a new game…").clicked() {
            app.setup = Some(stars_core::newgame::NewGame::default());
        }
    });
}

/// The colour a player's things are drawn in.
///
/// The game's own sixteen (`rgcrPlrHistory`) — see
/// [`stars_core::scoresheet::PLAYER_COLOURS`]. `MANUAL.PDF` p. 5-16 sends a
/// player to the Score sheet's history graph to find out which colour is
/// theirs, so these are the colours that answer has to match.
#[must_use]
pub fn player_colour(player: i16) -> egui::Color32 {
    let index = usize::try_from(player).unwrap_or(0);
    let [r, g, b] = stars_core::scoresheet::player_colour(index);
    egui::Color32::from_rgb(r, g, b)
}

/// Each mineral's own colour, as the game colours them: ironium blue,
/// boranium green, germanium yellow. Colonists are drawn in white.
#[must_use]
pub fn mineral_colour(mineral: usize) -> egui::Color32 {
    match mineral {
        0 => egui::Color32::from_rgb(90, 130, 230),
        1 => egui::Color32::from_rgb(90, 200, 110),
        2 => egui::Color32::from_rgb(220, 200, 80),
        _ => egui::Color32::from_rgb(230, 230, 230),
    }
}

/// The tile both panes end with: the fleets at this place, and the fuel and
/// cargo of whichever is chosen.
///
/// `DrawPlanetShipList` (`1048:377e`), which the planet pane draws as its
/// fourth tile and the fleet pane as its seventh — the same routine in the same
/// corner, so whichever is selected, the pane's bottom right answers "what else
/// is here?". A dropdown of the fleets, then a **Fuel** gauge and a **Cargo**
/// gauge for the one chosen, their labels aligned on the wider of the two.
///
/// The three buttons the original puts along the bottom are not here: they open
/// the Xfer and Merge dialogs, which this project does not have.
pub fn fleets_here(app: &mut App, ui: &mut egui::Ui) {
    let title = app.pane_fleets_title();
    tile(ui, title, |ui| fleets_here_body(app, ui));
}

/// The tile's contents, without the frame: the planet pane draws its own.
pub fn fleets_here_body(app: &mut App, ui: &mut egui::Ui) {
    let list = app.pane_fleet_list();
    let chosen = app.pane_fleet_choice();
    let gauges = app.pane_fleet_gauges();
    let mut choose = None;

    {
        if list.is_empty() {
            ui.label(egui::RichText::new("none").weak().small());
            return;
        }
        let showing = chosen
            .and_then(|index| list.iter().find(|entry| entry.index == index))
            .map(|entry| format!("{} ({})", entry.name, entry.ships))
            .unwrap_or_default();
        egui::ComboBox::from_id_source("pane-fleets-here")
            .width(ui.available_width() - 8.0)
            .selected_text(egui::RichText::new(showing).small())
            .show_ui(ui, |ui| {
                for entry in &list {
                    let label = format!("{} ({})", entry.name, entry.ships);
                    if ui
                        .selectable_label(Some(entry.index) == chosen, label)
                        .clicked()
                    {
                        choose = Some(entry.key);
                    }
                }
            });

        match gauges {
            // Somebody else's fleet is seen, not known: the original draws no
            // gauges for anything it does not have in full detail.
            None => {
                ui.label(
                    egui::RichText::new("Fuel and cargo are known only for your own fleets.")
                        .small()
                        .weak(),
                );
            }
            Some(gauges) => {
                let width = ui.fonts(|f| {
                    ["Fuel ", "Cargo "]
                        .iter()
                        .map(|label| {
                            f.layout_no_wrap(
                                (*label).to_string(),
                                egui::TextStyle::Small.resolve(ui.style()),
                                egui::Color32::PLACEHOLDER,
                            )
                            .size()
                            .x
                        })
                        .fold(0.0_f32, f32::max)
                });
                // The fuel gauge is **draggable**: dragging it moves fuel
                // between this fleet and the one the dropdown is showing.
                // The tutorial's page 56 is the one that needs it — "Click
                // and drag in the fuel gauge in the Other Fleets Here tile
                // until Teamster #4 has 383mg of fuel."
                let dragged = gauge(
                    ui,
                    "Fuel",
                    width,
                    gauges.fuel_capacity,
                    &[(gauges.fuel, egui::Color32::from_rgb(210, 170, 60))],
                    format!("{}mg", gauges.fuel),
                    true,
                );
                if let (Some(fraction), Some(other)) = (dragged, app.pane_fleet_choice()) {
                    #[allow(clippy::cast_possible_truncation)]
                    let wanted = (f32::from(i16::try_from(gauges.fuel_capacity).unwrap_or(0))
                        * fraction) as i32;
                    app.drag_fleet_fuel(other, wanted);
                }
                let mut bars: Vec<(i32, egui::Color32)> = (0..3)
                    .map(|m| (gauges.minerals[m], mineral_colour(m)))
                    .collect();
                bars.push((gauges.colonists, mineral_colour(3)));
                gauge(
                    ui,
                    "Cargo",
                    width,
                    gauges.cargo_capacity,
                    &bars,
                    format!("{}kT", gauges.cargo()),
                    false,
                );
            }
        }
    }

    // The three buttons across the tile's foot. `ShipCommandProc`
    // (`1050:2640`) wires them as `rghwndBtn[0..2]`: Xfer with whatever is
    // chosen, **Goto** it, and load everything off it. Goto is the one the
    // tutorial leans on — it is how you take command of a fleet in orbit
    // beside the one you are looking at.
    let chosen_fleet = app
        .pane_fleet_choice()
        .and_then(|index| app.game.as_ref()?.fleets.get(index))
        .filter(|f| usize::try_from(f.owner).is_ok_and(|owner| owner == app.local_player()))
        .map(|f| f.id);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                chosen_fleet.is_some(),
                egui::Button::new(egui::RichText::new("Goto").small()),
            )
            .clicked()
        {
            if let Some(id) = chosen_fleet {
                app.goto_fleet(id);
            }
        }
        // Xfer and the load-everything button open the transfer dialog, which
        // this project reaches from the Fleets screen instead.
        for label in ["Xfer", "Load All"] {
            ui.add_enabled(false, egui::Button::new(egui::RichText::new(label).small()))
                .on_disabled_hover_text("Transfer cargo from the Fleets screen.");
        }
    });

    if let Some(key) = choose {
        app.choose_pane_fleet(key);
    }
}

/// One gauge: a label, a bar of one or more coloured segments, and the figure.
fn gauge(
    ui: &mut egui::Ui,
    label: &str,
    label_width: f32,
    capacity: i32,
    segments: &[(i32, egui::Color32)],
    text: String,
    draggable: bool,
) -> Option<f32> {
    let mut dragged = None;
    ui.horizontal(|ui| {
        ui.add_sized(
            egui::vec2(label_width, ui.spacing().interact_size.y * 0.6),
            egui::Label::new(egui::RichText::new(label).small()),
        );
        let height = ui.text_style_height(&egui::TextStyle::Small);
        let bar = (ui.available_width() - 44.0).max(24.0);
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(bar, height),
            if draggable {
                egui::Sense::click_and_drag()
            } else {
                egui::Sense::hover()
            },
        );
        if draggable {
            if let Some(at) = response.interact_pointer_pos() {
                if response.dragged() || response.clicked() {
                    dragged = Some(((at.x - rect.left()) / rect.width()).clamp(0.0, 1.0));
                }
            }
        }
        let painter = ui.painter();
        painter.rect_filled(rect, 1.0, egui::Color32::from_gray(40));
        if capacity > 0 {
            let mut x = rect.left();
            for (amount, colour) in segments {
                if *amount <= 0 {
                    continue;
                }
                #[allow(clippy::cast_precision_loss)]
                let width = rect.width() * (*amount as f32) / (capacity as f32);
                let width = width.min(rect.right() - x);
                if width <= 0.0 {
                    break;
                }
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(x, rect.top()),
                        egui::vec2(width, rect.height()),
                    ),
                    1.0,
                    *colour,
                );
                x += width;
            }
        }
        ui.label(egui::RichText::new(text).small());
    });
    dragged
}

/// The frame every tile shares, as both panes draw it.
fn tile(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(title).small().strong());
            body(ui);
        });
}

/// A population figure, in the units players expect.
///
/// The simulation stores population in units of 100 colonists, which is a
/// detail of the file format and not something to put in front of anyone.
#[must_use]
pub fn colonists(pop: i32) -> String {
    let people = i64::from(pop) * 100;
    if people >= 1_000_000 {
        format!("{:.2}M", people as f64 / 1_000_000.0)
    } else if people >= 1_000 {
        format!("{}k", people / 1_000)
    } else {
        people.to_string()
    }
}

/// Lay a pane's tiles out from its table, the way `ReflowColumn` lays them
/// out, and draw each one in the frame `FDrawTileNC` (`1048:1086`) gives it.
///
/// The planet pane and the fleet pane are the **same window** with different
/// tables, so they share this: two fixed-width columns, tiles stacked from
/// four pixels down with four pixels between, each as tall as its line count
/// makes it or shrunk to its title bar when it is closed, and a click on a
/// title bar opening or closing it.
pub(crate) fn tile_pane(
    app: &mut App,
    ui: &mut egui::Ui,
    tiles: &[crate::tiles::Tile],
    open: &mut [bool],
    title: fn(&mut App, usize) -> String,
    body: fn(&mut App, &mut egui::Ui, usize),
) {
    let line = ui.text_style_height(&egui::TextStyle::Small);
    // The original's `fSmallTiles`, which the frame sets from the screen it
    // finds itself on; there is no such thing here, so the tiles are always
    // the full size.
    let small = false;
    let columns = [
        crate::tiles::column_tops(tiles, line, small, open, 0),
        crate::tiles::column_tops(tiles, line, small, open, 1),
    ];
    let height = columns
        .iter()
        .flatten()
        .map(|(_, top, tall)| top + tall)
        .fold(0.0_f32, f32::max)
        + crate::tiles::TILE_GAP;

    // The pane is drawn at the table's own width. With less room than that the
    // columns are scaled down together rather than clipped, so the proportions
    // stay the original's.
    let full = crate::tiles::COLUMN_PITCH + crate::tiles::TILE_WIDTH + crate::tiles::COLUMN_LEFT;
    let scale = (ui.available_width() / full).clamp(0.25, 1.0);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height * scale),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    let mut toggled: Option<usize> = None;

    for (column, here) in columns.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let left = rect.left()
            + (crate::tiles::COLUMN_LEFT + column as f32 * crate::tiles::COLUMN_PITCH) * scale;
        for (index, top, tall) in here {
            let frame = egui::Rect::from_min_size(
                egui::pos2(left, rect.top() + top * scale),
                egui::vec2(crate::tiles::TILE_WIDTH * scale, tall * scale),
            );
            let name = title(app, *index);
            let inside =
                crate::views::planet::tile_frame(ui, &painter, frame, &name, open[*index], line);
            // Clicking the title bar opens or closes the tile, and the column
            // reflows around it.
            let bar = egui::Rect::from_min_max(
                frame.min,
                egui::pos2(
                    frame.right(),
                    frame.top() + line + crate::tiles::TITLE_EXTRA + 2.0,
                ),
            );
            if ui
                .interact(bar, ui.id().with(("tile", index)), egui::Sense::click())
                .clicked()
            {
                toggled = Some(*index);
            }
            if open[*index] && inside.height() > 4.0 {
                let mut child = ui.child_ui(inside, egui::Layout::top_down(egui::Align::Min), None);
                child.set_clip_rect(inside);
                child.spacing_mut().item_spacing.y = 0.0;
                body(app, &mut child, *index);
            }
        }
    }

    if let Some(index) = toggled {
        open[index] = !open[index];
    }
}

/// Lay a dialog template into the room the shell gives it.
///
/// Returns the area the dialog occupies and two helpers: one that places a
/// control by id, and one that reads its caption with the accelerator marker
/// taken out. The whole template is scaled by the smaller of the two ratios so
/// its proportions survive being squeezed. See [`crate::dialog`].
pub(crate) fn dialog_frame<'a>(
    ui: &mut egui::Ui,
    template: &'a crate::dialog::Template,
) -> (
    egui::Rect,
    impl Fn(u16) -> egui::Rect + 'a,
    impl Fn(u16) -> String + 'a,
) {
    let scale = template.scale(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        ui.available_size(),
    ));
    let want = template.pixels() * scale;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), want.y),
        egui::Sense::hover(),
    );
    let origin = rect.min;
    let place = move |id: u16| -> egui::Rect {
        template.control(id).map_or(egui::Rect::NOTHING, |control| {
            crate::dialog::place(origin, scale, control.at)
        })
    };
    let caption = move |id: u16| -> String {
        template
            .control(id)
            .map_or_else(String::new, crate::dialog::Control::label)
    };
    (rect, place, caption)
}

/// One of a template's push buttons, drawn where the template puts it.
pub(crate) fn dialog_button(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    text: &str,
    enabled: bool,
) -> egui::Response {
    ui.put(
        rect,
        egui::Button::new(egui::RichText::new(text).small()).sense(if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        }),
    )
}

/// The etched frame the original draws around a hand-made group box.
///
/// `_Draw3dFrame` (`1040:336a`) with `fErase = -1`: an outer ring in shadow
/// along the top and left and in highlight along the bottom and right, then
/// the same ring one pixel in with the two swapped. That is the Windows 3.1
/// groove — the dialogs that want a group box without a `BUTTON` control draw
/// it themselves and write the caption over the top edge afterwards.
pub(crate) fn draw_3d_frame(painter: &egui::Painter, rect: egui::Rect) {
    let hilite = egui::Color32::from_rgb(
        crate::toolbar::HILITE[0],
        crate::toolbar::HILITE[1],
        crate::toolbar::HILITE[2],
    );
    let shadow = egui::Color32::from_rgb(
        crate::toolbar::SHADOW[0],
        crate::toolbar::SHADOW[1],
        crate::toolbar::SHADOW[2],
    );
    let line = |x: f32, y: f32, w: f32, h: f32, colour: egui::Color32| {
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h)),
            0.0,
            colour,
        );
    };
    for (outer, top_left, bottom_right) in
        [(rect, shadow, hilite), (rect.shrink(1.0), hilite, shadow)]
    {
        let (w, h) = (outer.width(), outer.height());
        line(outer.left(), outer.top(), w, 1.0, top_left);
        line(outer.left(), outer.top(), 1.0, h, top_left);
        line(outer.left(), outer.bottom(), w + 1.0, 1.0, bottom_right);
        line(outer.right(), outer.top(), 1.0, h, bottom_right);
    }
}
