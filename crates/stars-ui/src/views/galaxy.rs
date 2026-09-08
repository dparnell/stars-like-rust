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

use crate::app as stars_ui_arrow;
use crate::app::ScanObject;
use crate::app::ScanThing;
use crate::views::{colonists, player_colour};
use crate::OrbitRing;
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

    // View (Toolbar) hides it, which the manual offers as a way to make room.
    if app.toolbar_visible() {
        crate::views::toolbar::view(app, ui);
        ui.separator();
    }

    let available = ui.available_size();
    let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
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
    // What the pointer landed on: where it is in galaxy units, and which
    // planet or fleet was hit. The scanner needs the point rather than the
    // object, because clicking the same point again cycles through everything
    // on it.
    let mut clicked: Option<(i16, i16, Option<i16>, Option<usize>)> = None;
    // **Only on the frame of a click.** `interact_pointer_pos` is `Some` for
    // every frame the button is held down, so hit-testing on it alone would
    // act again and again while the button is down — which, with the
    // click-again-to-cycle rule below, spins through everything on the spot.
    let pointer = (response.clicked() || response.secondary_clicked())
        .then(|| response.interact_pointer_pos())
        .flatten();
    // A space object under the pointer, which planets and fleets outrank.
    let mut thing_hit: Option<ScanThing> = None;

    // Minefields first, so they lie under the planets rather than over them.
    // A field is a circle whose radius is the square root of its mine count,
    // filled with one of the game's three pattern brushes — the hatch says
    // which kind of field it is — in the colour its owner earns. It is an
    // overlay of its own.
    let mut to_minefield: Vec<(egui::Pos2, f32, u16, Color32, bool)> = Vec::new();
    if let Some(game) = app.game.as_ref().filter(|_| app.scan_overlays.minefields) {
        for (field_index, field) in game.minefields.iter().enumerate() {
            // The menu behind the Mine Fields button chooses whose are drawn.
            if !app.shows_minefield(field.owner) {
                continue;
            }
            let at = to_screen(f32::from(field.position.x), f32::from(field.position.y));
            #[allow(clippy::cast_possible_truncation)]
            let radius = field.radius() as f32 * scale;
            // A field's own centre is clickable, under the planets and fleets.
            if let Some(p) = pointer {
                if (p - at).length() <= 6.0 {
                    thing_hit.get_or_insert(ScanThing::Minefield(field_index));
                }
            }
            let [r, g, b] = app.minefield_colour(field);
            to_minefield.push((
                at,
                radius,
                App::minefield_pattern(field),
                Color32::from_rgb(r, g, b),
                app.minefield_centre_marked(field),
            ));
        }
    }
    minefields(app, ui, &to_minefield, rect.min);

    let view = app.scan_view;
    let race = app
        .game
        .as_ref()
        .and_then(|g| g.players.get(app.local_player()))
        .map(|p| p.race.clone());
    let names_visible = app.planet_names_visible();
    // Which planets have fleets in orbit, and whose. Collected here and drawn
    // after the loop, because painting the game's own ring sprite needs the
    // app mutably and the loop is holding it.
    let rings = app.orbit_rings();
    let mut to_ring: Vec<(egui::Pos2, OrbitRing, bool)> = Vec::new();
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
                (crate::views::mineral_colour(best), radius)
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
        if let Ok(id) = u16::try_from(planet.id) {
            if let Some(ring) = rings.get(&id) {
                to_ring.push((at, *ring, Some(planet.id) == selected));
            }
        }
        if names_visible {
            if let Some(name) = planet.name {
                // Player Colors, when it is on, writes an owned planet's name
                // in its owner's colour and this player's own in white. An
                // unowned planet keeps the ordinary colour either way.
                let colour = match app.planet_name_colour(planet.owner) {
                    None => Color32::from_gray(170),
                    Some(None) => Color32::WHITE,
                    Some(Some(owner)) => player_colour(i16::try_from(owner).unwrap_or(0)),
                };
                painter.text(
                    at + Vec2::new(0.0, radius + 2.0),
                    egui::Align2::CENTER_TOP,
                    name,
                    egui::FontId::proportional(9.0),
                    colour,
                );
            }
        }
        if let Some(p) = pointer {
            if (p - at).length() <= radius + 6.0 {
                clicked = Some((position.x, position.y, Some(planet.id), None));
            }
        }
    }

    // The orbit rings, over the planets. The original blits one of three
    // rings out of the scanner's own sheet — grey for this player's fleets,
    // red for anybody else's, magenta for both — and uses the **larger** ring
    // when the planet is the selected object rather than at any particular
    // zoom.
    orbit_rings(app, ui, &to_ring);

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

    // Mineral packets, wormholes and the Mystery Trader, as `DrawScanner`
    // draws them: a packet is an outline whose shape and size the game picks,
    // a wormhole is a masked blit out of the scanner's own sheet with a line to
    // its far end, and the Trader is a fleet arrow tinted yellow.
    let mut to_wormhole: Vec<egui::Pos2> = Vec::new();
    let mut to_trader: Vec<(egui::Pos2, u8)> = Vec::new();
    if let Some(game) = app.game.as_ref() {
        // The line between a pair of wormholes goes under the glyphs.
        for (index, hole) in game.wormholes.iter().enumerate() {
            if !hole.include {
                continue;
            }
            if let Some(far) = app.wormhole_line(index) {
                let from = to_screen(f32::from(hole.position.x), f32::from(hole.position.y));
                let to = to_screen(
                    f32::from(game.wormholes[far].position.x),
                    f32::from(game.wormholes[far].position.y),
                );
                painter.line_segment(
                    [from, to],
                    Stroke::new(1.0_f32, Color32::from_rgb(90, 0, 110)),
                );
            }
        }

        let radius = app.packet_mark_radius();
        for (index, packet) in game.packets.iter().enumerate() {
            if !packet.include {
                continue;
            }
            let at = to_screen(f32::from(packet.position.x), f32::from(packet.position.y));
            if let Some(p) = pointer {
                if (p - at).length() <= radius + 3.0 {
                    thing_hit.get_or_insert(ScanThing::Packet(index));
                }
            }
            if App::packet_is_diamond(packet) {
                // A packet with no speed: a yellow diamond, a pixel outside the
                // radius on every side.
                let r = radius + 1.0;
                painter.add(egui::Shape::closed_line(
                    vec![
                        at + Vec2::new(0.0, -r),
                        at + Vec2::new(-r, 0.0),
                        at + Vec2::new(0.0, r),
                        at + Vec2::new(r, 0.0),
                    ],
                    Stroke::new(1.0_f32, Color32::from_rgb(255, 255, 0)),
                ));
            } else {
                // One under way: a square, red unless it is ours.
                let mine = usize::try_from(packet.owner).is_ok_and(|o| o == app.local_player());
                let colour = if mine {
                    Color32::from_gray(200)
                } else {
                    Color32::from_rgb(255, 0, 0)
                };
                painter.rect_stroke(
                    egui::Rect::from_center_size(at, egui::vec2(radius * 2.0, radius * 2.0)),
                    0.0,
                    Stroke::new(1.0_f32, colour),
                );
            }
        }
        for (index, hole) in game.wormholes.iter().enumerate() {
            if !hole.include {
                continue;
            }
            let at = to_screen(f32::from(hole.position.x), f32::from(hole.position.y));
            if let Some(p) = pointer {
                if (p - at).length() <= 6.0 {
                    thing_hit.get_or_insert(ScanThing::Wormhole(index));
                }
            }
            to_wormhole.push(at);
        }
        for (index, trader) in game.traders.iter().enumerate() {
            if !trader.include {
                continue;
            }
            let at = to_screen(f32::from(trader.position.x), f32::from(trader.position.y));
            if let Some(p) = pointer {
                if (p - at).length() <= 6.0 {
                    thing_hit.get_or_insert(ScanThing::Trader(index));
                }
            }
            if let Some(arrow) = app.trader_arrow(index) {
                to_trader.push((at, arrow));
            }
        }
    }
    wormholes(app, ui, &to_wormhole);
    // The Trader's arrow is the fleets' own sheet, tinted yellow.
    fleet_arrows(
        app,
        ui,
        &to_trader
            .iter()
            .map(|(at, arrow)| (*at, *arrow, Color32::from_rgb(255, 255, 0)))
            .collect::<Vec<_>>(),
    );

    // Fleets, as small marks offset from their planet so they do not hide it.
    let mut to_arrow: Vec<(egui::Pos2, u8, Color32)> = Vec::new();
    if let Some(game) = app.game.as_ref() {
        for (index, fleet) in game.fleets.iter().enumerate() {
            if fleet.stacks.is_empty() {
                continue;
            }
            // The idle filter shows only the fleets with nothing to do.
            if app.scan_overlays.idle_fleets && fleet.waypoints.len() > 1 {
                continue;
            }
            let at = to_screen(f32::from(fleet.position.x), f32::from(fleet.position.y));
            // A fleet's own mark is clickable, but a planet under the pointer
            // wins: the planet comes first in the cycle too.
            if clicked.is_none() {
                if let Some(p) = pointer {
                    if (p - (at + Vec2::new(6.0, -6.0))).length() <= 5.0 {
                        clicked = Some((fleet.position.x, fleet.position.y, None, Some(index)));
                    }
                }
            }
            let colour = player_colour(fleet.owner);
            // The mark, which the original draws as an arrow pointing the way
            // the fleet is going. Gathered here and drawn after the loop,
            // because tinting the game's own stencil needs the app mutably.
            to_arrow.push((at + Vec2::new(6.0, -6.0), app.fleet_arrow_of(fleet), colour));
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
        }
    }
    fleet_arrows(app, ui, &to_arrow);

    // The ship counts, one per **location** rather than per fleet, which is
    // what the original writes. The two ship filters have already narrowed
    // what each fleet contributes.
    if app.scan_overlays.ship_counts {
        for count in app.ship_counts() {
            let at = to_screen(f32::from(count.position.x), f32::from(count.position.y));
            let colour = match app.ship_count_colour(&count) {
                Some(owner) => player_colour(i16::try_from(owner).unwrap_or(0)),
                None => Color32::WHITE,
            };
            painter.text(
                at + Vec2::new(9.0, -9.0),
                egui::Align2::LEFT_BOTTOM,
                count.ships.to_string(),
                egui::FontId::proportional(9.0),
                colour,
            );
        }
    }

    // The measuring tape: a right-drag from anywhere to anywhere, snapping to
    // whatever it passes over (`FHandleMeasuringTape`, `1058:9974`). Shift
    // widens what it snaps to.
    {
        let to_galaxy = |p: Pos2| -> (i16, i16) {
            #[allow(clippy::cast_possible_truncation)]
            let x = (min_x + (p.x - rect.left() - margin) / scale) as i16;
            #[allow(clippy::cast_possible_truncation)]
            let y = (max_y - (p.y - rect.top() - margin) / scale) as i16;
            (x, y)
        };
        let wide = ui.input(|i| i.modifiers.shift);
        if response.drag_started_by(egui::PointerButton::Secondary) {
            if let Some(p) = response.interact_pointer_pos() {
                let (x, y) = to_galaxy(p);
                app.measure_from(x, y);
            }
        }
        if response.dragged_by(egui::PointerButton::Secondary) {
            if let Some(p) = response.interact_pointer_pos() {
                let (x, y) = to_galaxy(p);
                app.measure_to(x, y, wide);
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Secondary) {
            app.measure_end();
        }
    }
    if let Some((from, to)) = app.measuring {
        let a = to_screen(f32::from(from.x), f32::from(from.y));
        let b = to_screen(f32::from(to.x), f32::from(to.y));
        painter.line_segment([a, b], Stroke::new(1.0_f32, Color32::from_gray(230)));
        painter.circle_stroke(b, 4.0, Stroke::new(1.0_f32, Color32::from_gray(230)));
    }

    // A space object counts as a click of its own when no planet or fleet was
    // under the pointer: the original's `FFindNearestObject` mask takes things
    // as well, and the panes above simply outrank them.
    let clicked = clicked.or_else(|| {
        thing_hit
            .and_then(|thing| app.object_position(ScanObject::Thing(thing)))
            .map(|at| (at.x, at.y, None, None))
    });

    // Add Way Points Mode: the map gives orders instead of selecting. A drag
    // that starts on one of the selected fleet's waypoints moves it; anything
    // else appends a leg (`FHandleWayPointDrag`, `1058:8176`).
    if app.add_waypoints && app.selection.fleet.is_some() {
        let to_galaxy = |p: Pos2| -> (i16, i16) {
            #[allow(clippy::cast_possible_truncation)]
            let x = (min_x + (p.x - rect.left() - margin) / scale) as i16;
            #[allow(clippy::cast_possible_truncation)]
            let y = (max_y - (p.y - rect.top() - margin) / scale) as i16;
            (x, y)
        };
        // A waypoint under the pointer, in galaxy units — the tolerance is the
        // grab radius in pixels converted back.
        if response.drag_started() {
            app.dragging_waypoint = response
                .interact_pointer_pos()
                .map(to_galaxy)
                .and_then(|(x, y)| app.waypoint_at(x, y, f64::from(8.0 / scale)));
        }
        if response.dragged() {
            if let (Some(waypoint), Some(p)) =
                (app.dragging_waypoint, response.interact_pointer_pos())
            {
                let (x, y) = to_galaxy(p);
                app.move_waypoint(waypoint, x, y);
            }
        }
        if response.drag_stopped() {
            app.dragging_waypoint = None;
        }
        if response.clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                let (x, y) = to_galaxy(p);
                app.add_waypoint(x, y);
            }
        }
    } else if let Some((x, y, planet, fleet)) = clicked {
        let hit = match (planet, fleet) {
            (Some(id), _) => Some(ScanObject::Planet(id)),
            (None, Some(index)) => Some(ScanObject::Fleet(index)),
            _ => thing_hit.map(ScanObject::Thing),
        };
        if let Some(hit) = hit {
            if response.secondary_clicked() {
                // The right-click menu, which lists everything at that point.
                app.scan_menu_at = Some((x, y));
            } else {
                // A click selects what is under the pointer; the same spot
                // again steps to the next thing there.
                app.scan_click_on(hit);
            }
        }
    }

    // The menu itself, while it is up. It is placed on the object rather than
    // on the pointer, so it stays with what it is about if the map moves.
    if let Some((x, y)) = app.scan_menu_at {
        let items = app.scan_menu(x, y);
        let at = to_screen(f32::from(x), f32::from(y)) + Vec2::new(10.0, 10.0);
        let mut chosen = None;
        let area = egui::Area::new(egui::Id::new("scanner-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(180.0);
                    if items.is_empty() {
                        ui.label(egui::RichText::new("nothing here").weak().small());
                    }
                    for item in &items {
                        // The original's list puts a separator between the
                        // planet and the fleets, and ticks the selection.
                        if item.first_of_group {
                            ui.separator();
                        }
                        if ui
                            .selectable_label(
                                item.checked,
                                egui::RichText::new(&item.label).small(),
                            )
                            .clicked()
                        {
                            chosen = Some(item.object);
                        }
                    }
                });
            });
        if let Some(object) = chosen {
            app.select_object(object);
            app.scan_menu_at = None;
        } else if ui.input(|i| i.key_pressed(egui::Key::Escape))
            || (response.clicked() && !area.response.contains_pointer())
        {
            // Anywhere else, or Escape, puts it away.
            app.scan_menu_at = None;
        }
    }

    // The status bar the original keeps along the bottom of the scanner: what
    // is there, where it is, and how far the tape is stretched.
    {
        let bar = app.status_bar();
        let mut text = format!("{}   x: {}   y: {}", bar.name, bar.x, bar.y);
        if let Some(distance) = bar.distance {
            text.push_str(&format!("   {distance}"));
        }
        painter.text(
            rect.left_bottom() + Vec2::new(8.0, -24.0),
            egui::Align2::LEFT_BOTTOM,
            text,
            egui::FontId::proportional(11.0),
            Color32::from_gray(190),
        );
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

/// Draw the orbit rings gathered while the planets were drawn.
///
/// The game's own sprite when a copy of the original has been found — the
/// scanner's sheet holds the three colours at 11 pixels and again at 19 — and
/// a stroked circle in the same colour when it has not.
fn orbit_rings(app: &mut App, ui: &mut egui::Ui, rings: &[(egui::Pos2, OrbitRing, bool)]) {
    if rings.is_empty() {
        return;
    }
    let ctx = ui.ctx().clone();
    let painter = ui.painter().clone();
    for (at, ring, selected) in rings {
        let side: u32 = if *selected { 19 } else { 11 };
        let drawn = app
            .art
            .as_mut()
            .and_then(|art| {
                art.sprite_at(
                    &ctx,
                    &stars_formats::resources::Name::Text("ScannerBmp".to_string()),
                    if side == 19 { 29 } else { 16 },
                    ring.row() * side,
                    side,
                    side,
                    egui::vec2(side as f32, side as f32),
                )
            })
            .map(|image| {
                let half = side as f32 / 2.0;
                image.paint_at(
                    ui,
                    egui::Rect::from_min_size(
                        *at - Vec2::new(half, half),
                        egui::vec2(side as f32, side as f32),
                    ),
                );
            })
            .is_some();
        if !drawn {
            let [r, g, b] = ring.colour();
            painter.circle_stroke(
                *at,
                side as f32 / 2.0,
                Stroke::new(1.0_f32, Color32::from_rgb(r, g, b)),
            );
        }
    }
}

/// Draw the fleet marks gathered while the fleets were drawn.
///
/// `GetScanFleetOrientation` picks one of eight arrows for the way the fleet
/// is heading, and the original blits it **through a mask** so the colour
/// comes from the pen rather than the bitmap — which is why this tints a
/// stencil rather than drawing the picture. Without the game's own sheet it
/// falls back to the small square this project drew before.
fn fleet_arrows(app: &mut App, ui: &mut egui::Ui, arrows: &[(egui::Pos2, u8, Color32)]) {
    if arrows.is_empty() {
        return;
    }
    let ctx = ui.ctx().clone();
    let painter = ui.painter().clone();
    for (at, arrow, colour) in arrows {
        let (x, y, side) = app.fleet_arrow_cell(*arrow);
        let drawn = app
            .art
            .as_mut()
            .and_then(|art| {
                art.stencil_at(
                    &ctx,
                    &stars_formats::resources::Name::Id(stars_ui_arrow::ARROW_SHEET),
                    x,
                    y,
                    side,
                    side,
                    egui::vec2(side as f32, side as f32),
                )
            })
            .map(|image| {
                let half = side as f32 / 2.0;
                image.tint(*colour).paint_at(
                    ui,
                    Rect::from_min_size(
                        *at - Vec2::new(half, half),
                        egui::vec2(side as f32, side as f32),
                    ),
                );
            })
            .is_some();
        if !drawn {
            painter.rect_filled(Rect::from_center_size(*at, Vec2::splat(3.0)), 0.0, *colour);
        }
    }
}

/// Draw the wormholes gathered while the map was drawn.
///
/// `DrawScanner` blits a nine-pixel cell out of `ScannerBmp` with the mask
/// beside it, centred on the wormhole. Without the game's own sheet it is two
/// rings, which is as close to a swirl as a circle gets.
fn wormholes(app: &mut App, ui: &mut egui::Ui, holes: &[egui::Pos2]) {
    if holes.is_empty() {
        return;
    }
    let ctx = ui.ctx().clone();
    let painter = ui.painter().clone();
    let side = stars_ui_arrow::WORMHOLE_SIDE;
    for at in holes {
        let drawn = app
            .art
            .as_mut()
            .and_then(|art| {
                art.sprite_masked_at(
                    &ctx,
                    &stars_formats::resources::Name::Text("ScannerBmp".to_string()),
                    stars_ui_arrow::WORMHOLE_CELL,
                    stars_ui_arrow::WORMHOLE_MASK,
                    (side, side),
                    egui::vec2(side as f32, side as f32),
                )
            })
            .map(|image| {
                let half = side as f32 / 2.0;
                image.paint_at(
                    ui,
                    Rect::from_min_size(
                        *at - Vec2::new(half, half),
                        egui::vec2(side as f32, side as f32),
                    ),
                );
            })
            .is_some();
        if !drawn {
            let colour = Color32::from_rgb(180, 140, 220);
            painter.circle_stroke(*at, 4.0, Stroke::new(1.0_f32, colour));
            painter.circle_stroke(*at, 2.0, Stroke::new(1.0_f32, colour));
        }
    }
}

/// Draw the minefields gathered while the map was drawn.
///
/// `DrawScanner` fills each with one of three 8x8 pattern brushes
/// (`rghbrPat`, resources 460 to 462, one per kind) in the field's own colour,
/// anchored to the map's origin with `SetBrushOrg` so the dots hold still when
/// the map moves — which is why the pattern is tiled from `origin` here rather
/// than from each circle.
///
/// Without the game's own brushes it is the translucent disc this project drew
/// before.
///
/// Not reproduced: the original hollows each circle out of the ones already
/// drawn (`fHollowOut`), which keeps a pattern brush from painting an overlap
/// twice. Drawing the same pattern twice at the same anchor comes to the same
/// thing here, since the dots land in the same places.
fn minefields(
    app: &mut App,
    ui: &mut egui::Ui,
    fields: &[(egui::Pos2, f32, u16, Color32, bool)],
    origin: egui::Pos2,
) {
    if fields.is_empty() {
        return;
    }
    let ctx = ui.ctx().clone();
    let painter = ui.painter().clone();
    for (at, radius, pattern, colour, mark_centre) in fields {
        let name = stars_formats::resources::Name::Id(*pattern);
        let tile = app
            .art
            .as_mut()
            .and_then(|art| Some((art.pattern(&ctx, &name)?, art.sheet_size(&name)?)));
        match tile {
            Some((texture, (width, height))) => {
                painter.add(pattern_disc(
                    *at,
                    *radius,
                    texture,
                    egui::vec2(width as f32, height as f32),
                    origin,
                    *colour,
                ));
            }
            None => {
                painter.circle_filled(
                    *at,
                    *radius,
                    Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), 24),
                );
                painter.circle_stroke(
                    *at,
                    *radius,
                    Stroke::new(
                        1.0_f32,
                        Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), 90),
                    ),
                );
            }
        }
        // The centre gets a mark of its own when no planet is sitting on it.
        if *mark_centre {
            let half = 2.0;
            painter.rect_filled(
                Rect::from_center_size(*at, egui::vec2(half * 2.0, half * 2.0)),
                0.0,
                *colour,
            );
        }
    }
}

/// A disc filled with a tiling pattern, anchored to `origin`.
fn pattern_disc(
    at: egui::Pos2,
    radius: f32,
    texture: egui::TextureId,
    tile: Vec2,
    origin: egui::Pos2,
    colour: Color32,
) -> egui::Shape {
    let mut mesh = egui::Mesh::with_texture(texture);
    let uv = |p: egui::Pos2| egui::pos2((p.x - origin.x) / tile.x, (p.y - origin.y) / tile.y);
    mesh.colored_vertex(at, colour);
    mesh.vertices[0].uv = uv(at);
    // A fan, fine enough that the edge reads as a circle at any zoom.
    let steps = ((radius * 1.5) as usize).clamp(16, 96);
    for step in 0..=steps {
        #[allow(clippy::cast_precision_loss)]
        let angle = step as f32 / steps as f32 * std::f32::consts::TAU;
        let p = at + Vec2::new(angle.cos(), angle.sin()) * radius;
        mesh.colored_vertex(p, colour);
        let last = mesh.vertices.len() - 1;
        mesh.vertices[last].uv = uv(p);
        if step > 0 {
            mesh.add_triangle(0, last as u32 - 1, last as u32);
        }
    }
    egui::Shape::mesh(mesh)
}
