//! The scanner: the galaxy map.
//!
//! The original's largest window, and the one the game is played on
//! (`ScannerWndProc`, `1058:0032`; `DrawScanner`, `1058:108a`). Its toolbar
//! carries **six views**, which are exclusive, and a row of **overlays and
//! filters**, which are not; both sets are named here exactly as the original
//! names them. See `docs/ui/scanner.md`.
//!
//! Two things are taken from the binary rather than invented. The **zoom** has
//! nine steps — 25, 38, 50, 75, 100, 125, 150, 200 and 400 per cent
//! (`vrgpctZoom`, `1068:0da4`) — and the scale is applied as the original's
//! integer shifts, not as a percentage. And the map is drawn **y-flipped**:
//! `LogicalToScan` (`1058:744e`) mirrors the galaxy's y about the universe's
//! height, so a planet stored near y=0 is drawn at the bottom.
//!
//! Planet coordinates live only in the `.xy` universe file, so a game opened
//! without one has nothing to draw; the map says so rather than piling every
//! planet on the origin.
//!
//! The map distinguishes the two kinds of planet the loader keeps apart — the
//! ones the player owns and can command, and the ones they have merely scanned.
//! That distinction is the fog of war, and it comes out of `GameState` rather
//! than needing any bookkeeping here.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::views::{colonists, player_colour};
use crate::{App, ScanView};

/// Draw the galaxy.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some((min_x, min_y, max_x, max_y)) = app.extent() else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label("No planet positions.");
            ui.label(
                egui::RichText::new(
                    "They come from the game's .xy universe file, which was not found \
                     beside this save or one directory up.",
                )
                .weak(),
            );
        });
        return;
    };

    toolbar(app, ui);
    ui.separator();

    let available = ui.available_size();
    let (response, painter) = ui.allocate_painter(available, Sense::click());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 10, 18));

    // Fit the universe into the panel at 100%, keeping it square so distances
    // read true, then apply the scanner's own zoom on top. The y axis is
    // mirrored, as `LogicalToScan` mirrors it.
    let span = (max_x - min_x).max(max_y - min_y).max(1.0);
    let margin = 16.0;
    let fit = (rect.width().min(rect.height()) - margin * 2.0) / span;
    #[allow(clippy::cast_precision_loss)]
    let zoom = app.scan_scale(1024) as f32 / 1024.0;
    let scale = fit * zoom;
    let to_screen = |x: f32, y: f32| -> Pos2 {
        Pos2::new(
            rect.left() + margin + (x - min_x) * scale,
            // `dGalInv - y`: the scanner shows the galaxy upside down.
            rect.top() + margin + (max_y - y) * scale,
        )
    };

    let selected = app.selection.planet;
    let mut clicked: Option<i16> = None;
    let pointer = response.interact_pointer_pos();

    // Minefields first, so they lie under the planets rather than over them. A
    // field is a circle whose radius is the square root of its mine count,
    // which is exactly how the game draws one. It is an overlay of its own.
    if let Some(game) = app.game.as_ref().filter(|_| app.scan_overlays.minefields) {
        for field in &game.minefields {
            let at = to_screen(f32::from(field.position.x), f32::from(field.position.y));
            #[allow(clippy::cast_possible_truncation)]
            let radius = field.radius() as f32 * scale;
            let colour = player_colour(field.owner);
            painter.circle_filled(
                at,
                radius,
                Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), 24),
            );
            painter.circle_stroke(
                at,
                radius,
                Stroke::new(
                    1.0_f32,
                    Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), 90),
                ),
            );
        }
    }

    let view = app.scan_view;
    let race = app
        .game
        .as_ref()
        .and_then(|g| g.players.get(app.local_player()))
        .map(|p| p.race.clone());
    for (planet, owned) in app.visible_planets() {
        let Some(position) = planet.position else {
            continue;
        };
        let at = to_screen(f32::from(position.x), f32::from(position.y));

        let (colour, radius) = match view {
            // Everybody's colours taken off: the map as geography.
            ScanView::NoPlayerInfo => (Color32::from_gray(140), 3.0),
            // How good the planet is for this race, green through red.
            ScanView::PlanetValue => {
                let value = race.as_ref().map_or(0, |race| {
                    stars_core::hab::pct_planet_desirability(planet, race)
                });
                (value_colour(value), 4.0)
            }
            // How many people live there.
            ScanView::Population => {
                let radius = if planet.pop > 0 {
                    #[allow(clippy::cast_precision_loss)]
                    (2.0 + (planet.pop as f32).sqrt() / 25.0).min(9.0)
                } else {
                    2.0
                };
                (
                    planet.owner.map_or(Color32::from_gray(110), player_colour),
                    radius,
                )
            }
            // What is on the surface, or in the ground: drawn as the mineral
            // whose reading is highest, in that mineral's colour.
            ScanView::SurfaceMineral | ScanView::MineralConcentration => {
                let readings: [i32; 3] = if view == ScanView::SurfaceMineral {
                    planet.surface_min
                } else {
                    [
                        i32::from(planet.min_conc[0]),
                        i32::from(planet.min_conc[1]),
                        i32::from(planet.min_conc[2]),
                    ]
                };
                let (best, amount) = readings
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, v)| **v)
                    .map_or((0, 0), |(i, v)| (i, *v));
                #[allow(clippy::cast_precision_loss)]
                let radius = (2.0 + (amount as f32).sqrt() / 6.0).min(9.0);
                (mineral_colour(best), radius)
            }
            ScanView::Normal => match planet.owner {
                Some(owner) => (player_colour(owner), if owned { 4.5 } else { 3.5 }),
                // Unowned but seen: a faint dot.
                None => (Color32::from_gray(110), 2.5),
            },
        };
        painter.circle_filled(at, radius, colour);
        // A planet the player only knows about is drawn hollow, so the fog of
        // war is visible at a glance.
        if !owned {
            painter.circle_stroke(
                at,
                radius + 1.5,
                Stroke::new(1.0_f32, Color32::from_gray(70)),
            );
        }
        if Some(planet.id) == selected {
            painter.circle_stroke(at, radius + 4.0, Stroke::new(1.5_f32, Color32::WHITE));
        }
        if app.scan_overlays.names {
            if let Some(name) = planet.name {
                painter.text(
                    at + Vec2::new(0.0, radius + 2.0),
                    egui::Align2::CENTER_TOP,
                    name,
                    egui::FontId::proportional(9.0),
                    Color32::from_gray(170),
                );
            }
        }
        if let Some(p) = pointer {
            if (p - at).length() <= radius + 6.0 {
                clicked = Some(planet.id);
            }
        }
    }

    // Scanner coverage: a ring round each of the player's planets, showing how
    // far it sees. An overlay of its own.
    if app.scan_overlays.scanner_coverage {
        let me = app.local_player();
        if let (Some(game), Some(race)) = (app.game.as_ref(), race.as_ref()) {
            let levels = game.players.get(me).map_or([0u8; 6], |p| p.research.levels);
            for planet in &game.planets {
                let (Some(owner), Some(position)) = (planet.owner, planet.position) else {
                    continue;
                };
                if usize::try_from(owner).is_ok_and(|o| o != me) {
                    continue;
                }
                let range = stars_core::scanning::planet_scanner_range_for_tech(
                    planet, race, &levels, true,
                );
                if range.normal <= 0 {
                    continue;
                }
                let at = to_screen(f32::from(position.x), f32::from(position.y));
                #[allow(clippy::cast_precision_loss)]
                painter.circle_stroke(
                    at,
                    range.normal as f32 * scale,
                    Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(90, 150, 220, 60)),
                );
            }
        }
    }

    // Fleets, as small marks offset from their planet so they do not hide it.
    if let Some(game) = app.game.as_ref() {
        for fleet in &game.fleets {
            if fleet.stacks.is_empty() {
                continue;
            }
            // The idle filter shows only the fleets with nothing to do.
            if app.scan_overlays.idle_fleets && fleet.waypoints.len() > 1 {
                continue;
            }
            let at = to_screen(f32::from(fleet.position.x), f32::from(fleet.position.y));
            let colour = player_colour(fleet.owner);
            let size = Vec2::splat(3.0);
            painter.rect_filled(
                Rect::from_center_size(at + Vec2::new(6.0, -6.0), size),
                0.0,
                colour,
            );
            // Where it is going, leg by leg.
            if app.scan_overlays.fleet_paths && fleet.waypoints.len() > 1 {
                let mut from = at;
                for waypoint in fleet.waypoints.iter().skip(1) {
                    let to = to_screen(
                        f32::from(waypoint.position.x),
                        f32::from(waypoint.position.y),
                    );
                    painter.line_segment(
                        [from, to],
                        Stroke::new(
                            1.0_f32,
                            Color32::from_rgba_unmultiplied(
                                colour.r(),
                                colour.g(),
                                colour.b(),
                                140,
                            ),
                        ),
                    );
                    from = to;
                }
            }
            if app.scan_overlays.ship_counts {
                let ships: i32 = fleet.stacks.iter().map(|s| s.count).sum();
                painter.text(
                    at + Vec2::new(9.0, -9.0),
                    egui::Align2::LEFT_BOTTOM,
                    ships.to_string(),
                    egui::FontId::proportional(9.0),
                    colour,
                );
            }
        }
    }

    if let Some(id) = clicked {
        app.selection.planet = Some(id);
    }

    // A short legend for the selected planet, drawn over the map.
    if let Some(planet) = app.selected_planet() {
        let text = match planet.owner {
            Some(owner) => format!(
                "{} — player {owner}, {} colonists, {} mines, {} factories",
                planet.name.unwrap_or("unnamed"),
                colonists(planet.pop),
                planet.mines,
                planet.factories
            ),
            None => format!("{} — unclaimed", planet.name.unwrap_or("unnamed")),
        };
        painter.text(
            rect.left_bottom() + Vec2::new(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            text,
            egui::FontId::proportional(13.0),
            Color32::from_gray(220),
        );
    }
}

/// The scanner's toolbar: the six views, the overlays and the zoom.
///
/// The original lays these out as bitmap buttons across the top of the map;
/// the names here are its own (`idsNormalView` and the strings after it).
fn toolbar(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        // The views are exclusive: one is showing at a time.
        for view in ScanView::ALL {
            let on = app.scan_view == view;
            if ui
                .selectable_label(on, short_name(view))
                .on_hover_text(view.name())
                .clicked()
            {
                app.scan_view = view;
            }
        }
        ui.separator();
        let overlays = &mut app.scan_overlays;
        ui.toggle_value(&mut overlays.names, "names")
            .on_hover_text("Planet Names Overlay");
        ui.toggle_value(&mut overlays.scanner_coverage, "range")
            .on_hover_text("Scanner Coverage Overlay");
        ui.toggle_value(&mut overlays.minefields, "mines")
            .on_hover_text("Mine Fields Overlay");
        ui.toggle_value(&mut overlays.fleet_paths, "paths")
            .on_hover_text("Fleet Paths Overlay");
        ui.toggle_value(&mut overlays.ship_counts, "counts")
            .on_hover_text("Ship Counts Overlay");
        ui.toggle_value(&mut overlays.idle_fleets, "idle")
            .on_hover_text("Idle Fleets Filter");
        ui.separator();
        // Nine steps, from a quarter size to four times.
        if ui.small_button("−").on_hover_text("Zoom Menu").clicked() {
            app.scan_zoom_by(-1);
        }
        ui.label(format!("{}%", app.scan_zoom_percent()));
        if ui.small_button("+").on_hover_text("Zoom Menu").clicked() {
            app.scan_zoom_by(1);
        }
    });
}

/// A short label for a view, since the original's buttons are icons.
fn short_name(view: ScanView) -> &'static str {
    match view {
        ScanView::Normal => "normal",
        ScanView::SurfaceMineral => "surface",
        ScanView::MineralConcentration => "concentration",
        ScanView::PlanetValue => "value",
        ScanView::Population => "population",
        ScanView::NoPlayerInfo => "no players",
    }
}

/// The colour of a planet in the value view: green for a good world down
/// through yellow to red for a hostile one.
fn value_colour(value: i16) -> Color32 {
    if value <= 0 {
        return Color32::from_rgb(150, 40, 40);
    }
    let t = f32::from(value.clamp(0, 100)) / 100.0;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Color32::from_rgb(
        (220.0 * (1.0 - t) + 60.0 * t) as u8,
        (60.0 * (1.0 - t) + 200.0 * t) as u8,
        60,
    )
}

/// Each mineral's own colour, as the game colours them: ironium blue,
/// boranium green, germanium yellow.
fn mineral_colour(mineral: usize) -> Color32 {
    match mineral {
        0 => Color32::from_rgb(90, 130, 230),
        1 => Color32::from_rgb(90, 200, 110),
        _ => Color32::from_rgb(220, 200, 80),
    }
}
