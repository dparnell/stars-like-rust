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
pub mod fleets;
pub mod galaxy;
pub mod host;
pub mod messages;
pub mod newgame;
pub mod parameters;
pub mod password;
pub mod planet;
pub mod planets;
pub mod players;
pub mod production;
pub mod race;
pub mod race_wizard;
pub mod relations;
pub mod research;
pub mod score;
pub mod statusbar;
pub mod survey;
pub mod toolbar;

use crate::{App, Screen};

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
        Screen::Planets => planets::view(app, ui),
        Screen::Fleets => fleets::view(app, ui),
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
    let list = app.pane_fleet_list();
    let chosen = app.pane_fleet_choice();
    let gauges = app.pane_fleet_gauges();
    let mut choose = None;

    tile(ui, title, |ui| {
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
                gauge(
                    ui,
                    "Fuel",
                    width,
                    gauges.fuel_capacity,
                    &[(gauges.fuel, egui::Color32::from_rgb(210, 170, 60))],
                    format!("{}mg", gauges.fuel),
                );
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
                );
            }
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
) {
    ui.horizontal(|ui| {
        ui.add_sized(
            egui::vec2(label_width, ui.spacing().interact_size.y * 0.6),
            egui::Label::new(egui::RichText::new(label).small()),
        );
        let height = ui.text_style_height(&egui::TextStyle::Small);
        let bar = (ui.available_width() - 44.0).max(24.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(bar, height), egui::Sense::hover());
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
