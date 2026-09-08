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
use crate::{App, ScanView, DIGIT_HEIGHT, DIGIT_SHEET, DIGIT_WIDTH};

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
    // The point the selection is on, which the fleet marks compare against
    // (`ptSelMain`).
    let selected_at = app
        .selected_object()
        .and_then(|object| app.object_position(object));
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

    // Scanner coverage, before anything else: `DrawScanner` paints the discs
    // straight after it clears the map, so they lie under the minefields, the
    // planets and the fleets alike. Each is a **filled** ellipse — dark blue
    // for a normal range and olive for a penetrating one — rather than the
    // outline this project drew before, and every range this player's planets,
    // fleets and (for a Packet Physics race) packets reach is one disc.
    for disc in app.scanner_coverage() {
        let at = to_screen(f32::from(disc.position.x), f32::from(disc.position.y));
        let [r, g, b] = if disc.penetrating {
            stars_ui_arrow::COVERAGE_PENETRATING
        } else {
            stars_ui_arrow::COVERAGE_NORMAL
        };
        #[allow(clippy::cast_precision_loss)]
        painter.circle_filled(at, disc.radius as f32 * scale, Color32::from_rgb(r, g, b));
    }

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
    // The Ship Paths overlay: a pass of its own, before the planets, so the
    // lines lie under the planet dots and the fleet marks. Each is the fleet's
    // waypoints joined up from the first, in the same solid red whoever owns
    // it — not the owner's colour, and not faded.
    {
        let [r, g, b] = stars_ui_arrow::PATH_COLOUR;
        let stroke = Stroke::new(1.0_f32, Color32::from_rgb(r, g, b));
        for path in app.fleet_paths() {
            let points: Vec<Pos2> = path
                .iter()
                .map(|at| to_screen(f32::from(at.x), f32::from(at.y)))
                .collect();
            for leg in points.windows(2) {
                painter.line_segment([leg[0], leg[1]], stroke);
            }
        }
    }

    // What the selection draws for itself — `DrawShipScanPath`. The scale line
    // through a scanned object goes under the planets with the paths; the
    // selected fleet's own path and a planet's route line go over them, so
    // they are drawn later.
    scale_line(app, ui, &to_screen, scale);

    let names_visible = app.planet_names_visible();
    let name_style = app.planet_name_style();
    // Which planets have fleets in orbit, and whose. Collected here and drawn
    // after the loop, because painting the game's own ring sprite needs the
    // app mutably and the loop is holding it.
    let rings = app.orbit_rings();
    let mut to_ring: Vec<(egui::Pos2, OrbitRing, bool)> = Vec::new();
    // The Normal view's planet marks and their starbase flags, likewise: they
    // are blits out of the scanner's sheet, which needs the app mutably.
    let mut to_planet: Vec<(egui::Pos2, stars_ui_arrow::PlanetMark)> = Vec::new();
    let mut to_starbase: Vec<(egui::Pos2, Color32, bool)> = Vec::new();

    // Every planet **position** gets a dot, explored or not: everybody knows
    // where the planets are, only not what is on them. `DrawScanner` walks
    // `rgptPlan`, which is the whole universe, before it walks what is known.
    {
        if let Some(universe) = app.universe.as_ref() {
            for planet in universe.planets_resolved() {
                #[allow(clippy::cast_precision_loss)]
                let at = to_screen(planet.x as f32, planet.y as f32);
                to_planet.push((at, stars_ui_arrow::PLANET_UNEXPLORED));
            }
        }
    }
    for (planet, owned) in app.visible_planets() {
        let Some(position) = planet.position else {
            continue;
        };
        let at = to_screen(f32::from(position.x), f32::from(position.y));

        let (colour, radius) = match view {
            // "Just a thousand dim points of light": the planet loop is skipped
            // altogether, so only the base dots the first loop drew are left.
            ScanView::NoPlayerInfo => (Color32::TRANSPARENT, 0.0),
            // Two concentric discs sized and coloured by what the planet is
            // worth, with a flag over an inhabited one.
            ScanView::PlanetValue => {
                if let Some(discs) = app.planet_value_discs(planet) {
                    for (radius, [r, g, b]) in discs {
                        painter.circle_filled(at, radius, Color32::from_rgb(r, g, b));
                    }
                }
                if let Some([r, g, b]) = app.planet_value_flag(planet) {
                    let colour = Color32::from_rgb(r, g, b);
                    // A pole 21 tall with a 7x6 banner at the top of it.
                    painter.rect_filled(
                        Rect::from_min_size(at + Vec2::new(0.0, -19.0), egui::vec2(1.0, 21.0)),
                        0.0,
                        colour,
                    );
                    painter.rect_filled(
                        Rect::from_min_size(at + Vec2::new(0.0, -19.0), egui::vec2(7.0, 6.0)),
                        0.0,
                        colour,
                    );
                }
                (Color32::TRANSPARENT, 0.0)
            }
            // A circle whose width is a step up the population ladder.
            ScanView::Population => {
                match app.planet_population_disc(planet) {
                    Some((radius, [r, g, b])) => (Color32::from_rgb(r, g, b), radius),
                    // Nobody lives there: small and grey, as the manual has it.
                    None => (Color32::from_gray(140), 2.0),
                }
            }
            // Three bars beside the planet, in the mineral colours.
            ScanView::SurfaceMineral | ScanView::MineralConcentration => {
                let layout = app.mineral_bar_layout();
                let bars = app.planet_mineral_bars(planet, view == ScanView::MineralConcentration);
                if !bars.is_empty() {
                    #[allow(clippy::cast_precision_loss)]
                    let origin = at + Vec2::new(-(layout[0] as f32), -(layout[1] as f32));
                    #[allow(clippy::cast_precision_loss)]
                    let axis = layout[2] as f32;
                    // The axis: a corner, along the bottom and up the left.
                    let frame = Color32::from_gray(110);
                    painter.rect_filled(
                        Rect::from_min_size(origin + Vec2::new(-2.0, 0.0), egui::vec2(axis, 1.0)),
                        0.0,
                        frame,
                    );
                    painter.rect_filled(
                        Rect::from_min_size(
                            origin + Vec2::new(-2.0, -axis + 1.0),
                            egui::vec2(1.0, axis),
                        ),
                        0.0,
                        frame,
                    );
                    for bar in &bars {
                        #[allow(clippy::cast_precision_loss)]
                        let (width, step) = (layout[3] as f32, layout[4] as f32);
                        #[allow(clippy::cast_precision_loss)]
                        let height = bar.height as f32;
                        #[allow(clippy::cast_precision_loss)]
                        let x = origin.x + bar.mineral as f32 * step;
                        if height > 0.0 {
                            painter.rect_filled(
                                Rect::from_min_size(
                                    egui::pos2(x, origin.y - height),
                                    egui::vec2(width, height),
                                ),
                                0.0,
                                crate::views::mineral_colour(bar.mineral),
                            );
                        }
                    }
                }
                (Color32::TRANSPARENT, 0.0)
            }
            // The Normal view is the game's own marks, gathered here and drawn
            // after the loop because they come out of the scanner's sheet.
            ScanView::Normal => {
                to_planet.push((at, app.planet_mark(planet, Some(planet.id) == selected)));
                if let Some([r, g, b]) = app.planet_starbase_mark(planet) {
                    to_starbase.push((at, Color32::from_rgb(r, g, b), Some(planet.id) == selected));
                }
                (Color32::TRANSPARENT, 0.0)
            }
        };
        if radius > 0.0 {
            painter.circle_filled(at, radius, colour);
            // A planet the player only knows about is drawn hollow, so the fog
            // of war is visible at a glance.
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
        }
        if let Ok(id) = u16::try_from(planet.id) {
            if let Some(ring) = rings.get(&id) {
                to_ring.push((at, *ring, Some(planet.id) == selected));
            }
        }
        if names_visible {
            if let Some(name) = planet.name {
                // Player Colors, when it is on, writes an owned planet's name
                // in its owner's colour and this player's own in white. Every
                // other name is white too: that is the colour the loop sets
                // before it starts and puts back after each coloured one.
                let colour = match app.planet_name_colour(planet.owner) {
                    None | Some(None) => Color32::WHITE,
                    Some(Some(owner)) => player_colour(i16::try_from(owner).unwrap_or(0)),
                };
                // Centred on the planet and five pixels under it — eleven more
                // in the Population view, where the ladder is in the way.
                let where_ = at + Vec2::new(0.0, f32::from(name_style.below));
                let font = egui::FontId::proportional(name_style.pixels());
                painter.text(where_, egui::Align2::CENTER_TOP, name, font.clone(), colour);
                if name_style.bold {
                    // The original asks Windows for the Arial Bold face at the
                    // two zoomed-in sizes. egui has no bold family loaded, so
                    // the weight is faked by writing the name again half a
                    // pixel across, which thickens the stems the same way.
                    painter.text(
                        where_ + Vec2::new(0.5, 0.0),
                        egui::Align2::CENTER_TOP,
                        name,
                        font,
                        colour,
                    );
                }
            }
        }
        if let Some(p) = pointer {
            if (p - at).length() <= radius + 6.0 {
                clicked = Some((position.x, position.y, Some(planet.id), None));
            }
        }
    }

    planet_marks(app, ui, &to_planet, &to_starbase);

    // The orbit rings, over the planets. The original blits one of three
    // rings out of the scanner's own sheet — grey for this player's fleets,
    // red for anybody else's, magenta for both — and uses the **larger** ring
    // when the planet is the selected object rather than at any particular
    // zoom.
    if app.orbit_rings_visible() {
        orbit_rings(app, ui, &to_ring);
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

    // Fleets. A fleet **in orbit** has no mark of its own — it adds to its
    // planet's orbit ring — and one in deep space is an arrow pointing the way
    // it is going, in the colour its owner earns. A fleet on the selected point
    // is drawn as an 11x11 glyph instead of the arrow.
    let mut to_arrow: Vec<(egui::Pos2, u8, Color32)> = Vec::new();
    let mut to_fleet_glyph: Vec<(egui::Pos2, (u32, u32))> = Vec::new();
    if let Some(game) = app.game.as_ref() {
        for (index, fleet) in game.fleets.iter().enumerate() {
            if fleet.stacks.is_empty() {
                continue;
            }
            // The idle filter shows only the fleets with nothing to do, and
            // the two ship filters hide a fleet none of whose ships they count.
            if app.scan_overlays.idle_fleets && fleet.waypoints.len() > 1 {
                continue;
            }
            if !app.fleet_scan_visible(index, fleet) {
                continue;
            }
            let at = to_screen(f32::from(fleet.position.x), f32::from(fleet.position.y));
            // A fleet's own mark is clickable, but a planet under the pointer
            // wins: the planet comes first in the cycle too.
            if clicked.is_none() {
                if let Some(p) = pointer {
                    if (p - at).length() <= 6.0 {
                        clicked = Some((fleet.position.x, fleet.position.y, None, Some(index)));
                    }
                }
            }
            let [r, g, b] = app.fleet_arrow_colour(fleet);
            let colour = Color32::from_rgb(r, g, b);
            // The mark, gathered here and drawn after the loop because tinting
            // the game's own stencil needs the app mutably. A fleet in orbit
            // has none: its planet's ring is its mark.
            if App::fleet_draws_arrow(fleet) {
                if Some(fleet.position) == selected_at {
                    to_fleet_glyph.push((at, app.fleet_selected_cell(fleet)));
                } else {
                    to_arrow.push((at, app.fleet_arrow_of(fleet), colour));
                }
            }
        }
    }
    fleet_arrows(app, ui, &to_arrow);
    fleet_glyphs(app, ui, &to_fleet_glyph);

    // The selected fleet's own path, over the marks: green, with a leg it
    // travels twice in yellow and drawn once. Every waypoint takes an 11x11
    // bite out of the line — the game draws no marker of its own at a
    // waypoint, and that hole is what points one out.
    for leg in app.selected_fleet_path() {
        let from = to_screen(f32::from(leg.from.x), f32::from(leg.from.y));
        let to = to_screen(f32::from(leg.to.x), f32::from(leg.to.y));
        let Some((start, end)) = stars_ui_arrow::leg_outside_waypoints(
            (from.x, from.y),
            (to.x, to.y),
            stars_ui_arrow::WAYPOINT_HOLE,
        ) else {
            continue;
        };
        let [r, g, b] = if leg.doubled {
            if app.scan_overlays.fleet_paths {
                stars_ui_arrow::DOUBLED_LEG_COLOUR
            } else {
                stars_ui_arrow::DOUBLED_LEG_PLAIN
            }
        } else {
            stars_ui_arrow::SHIP_PATH_COLOUR
        };
        painter.line_segment(
            [Pos2::new(start.0, start.1), Pos2::new(end.0, end.1)],
            Stroke::new(1.0_f32, Color32::from_rgb(r, g, b)),
        );
    }

    // And a selected planet's own two lines: purple to wherever its mass
    // driver is aimed, then green to wherever it routes new fleets. A planet
    // with both draws both.
    for (line, colour) in [
        (app.planet_driver_line(), stars_ui_arrow::DRIVER_COLOUR),
        (app.planet_route_line(), stars_ui_arrow::ROUTE_COLOUR),
    ] {
        let Some((from, to)) = line else {
            continue;
        };
        let [r, g, b] = colour;
        painter.line_segment(
            [
                to_screen(f32::from(from.x), f32::from(from.y)),
                to_screen(f32::from(to.x), f32::from(to.y)),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(r, g, b)),
        );
    }

    // The ship counts, one per **location** rather than per fleet, which is
    // what the original writes. The two ship filters have already narrowed
    // what each fleet contributes, and each number sits above whichever mark
    // the fleet under it was given.
    if app.scan_overlays.ship_counts {
        let counts = app.ship_counts();
        let mut to_digit: Vec<(egui::Pos2, u8, Color32)> = Vec::new();
        for count in &counts {
            let at = to_screen(f32::from(count.position.x), f32::from(count.position.y));
            let colour = match app.ship_count_colour(count) {
                Some(owner) => player_colour(i16::try_from(owner).unwrap_or(0)),
                None => Color32::WHITE,
            };
            // The top-left of the first digit: seven pixels above the y the
            // count was handed, and left of the point by the layout's offset.
            let top = at.y - f32::from(count.above) - DIGIT_HEIGHT as f32;
            for (dx, digit) in App::ship_count_digits(count.ships) {
                to_digit.push((Pos2::new(at.x + f32::from(dx), top), digit, colour));
            }
        }
        ship_count_digits(app, ui, &to_digit);
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

/// Draw the orbit rings gathered while the planets were drawn.
///
/// The game's own sprite when a copy of the original has been found — the
/// scanner's sheet holds the three colours at 11 pixels from `x = 0x10` and
/// again at 19 from `x = 0x1d`, with one mask for each column at `y = 0x45` —
/// and a stroked circle in the same colour when it has not.
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
                // The two blits the game makes: the mask at `(x, 0x45)` and
                // then the ring itself, `(n - 1)` rows down its column.
                let x = if side == 19 { 0x1d } else { 0x10 };
                art.sprite_masked_at(
                    &ctx,
                    &stars_formats::resources::Name::Text("ScannerBmp".to_string()),
                    (x, ring.row() * side),
                    (x, 0x45),
                    (side, side),
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

/// Draw the planet marks gathered while the map was drawn.
///
/// Each is a blit out of `ScannerBmp`: a 3x3 dot for a position nobody has
/// explored, a 3x3 for one that is known and unowned, a 5x5 in the owner's
/// colour, and an 11x11 blob over a mask for whichever is selected. Without the
/// game's own sheet each falls back to a disc of the same size and colour.
///
/// The starbase flag goes on top: a small filled square up and to the right,
/// blue for a starbase and yellow for an orbital fort.
fn planet_marks(
    app: &mut App,
    ui: &mut egui::Ui,
    planets: &[(egui::Pos2, stars_ui_arrow::PlanetMark)],
    starbases: &[(egui::Pos2, Color32, bool)],
) {
    let ctx = ui.ctx().clone();
    let painter = ui.painter().clone();
    let sheet = stars_formats::resources::Name::Text("ScannerBmp".to_string());
    for (at, mark) in planets {
        let side = mark.side as f32;
        let half = side / 2.0;
        let rect = Rect::from_min_size(*at - Vec2::new(half, half), egui::vec2(side, side));
        let drawn = app
            .art
            .as_mut()
            .and_then(|art| match mark.mask {
                Some(mask) => art.sprite_masked_at(
                    &ctx,
                    &sheet,
                    mark.cell,
                    mask,
                    (mark.side, mark.side),
                    egui::vec2(side, side),
                ),
                None => art.sprite_at(
                    &ctx,
                    &sheet,
                    mark.cell.0,
                    mark.cell.1,
                    mark.side,
                    mark.side,
                    egui::vec2(side, side),
                ),
            })
            .map(|image| image.paint_at(ui, rect))
            .is_some();
        if !drawn {
            let [r, g, b] = mark.colour;
            painter.circle_filled(*at, half, Color32::from_rgb(r, g, b));
        }
    }
    for (at, colour, selected) in starbases {
        // `pt + (3, -4)` and three pixels across, or `pt + (4, -6)` and five
        // when the planet is the selected one.
        let (offset, side) = if *selected {
            (Vec2::new(4.0, -6.0), 5.0)
        } else {
            (Vec2::new(3.0, -4.0), 3.0)
        };
        painter.rect_filled(
            Rect::from_min_size(*at + offset, egui::vec2(side, side)),
            0.0,
            *colour,
        );
    }
}

/// Draw the scale line through whatever the scanner has selected.
///
/// `DrawShipScanPath` (`1058:540c`) runs it through the object along its
/// heading, `warp² × 5` galaxy units each way, and marks every year: a
/// **tick** — a short perpendicular — for each of the five years behind, and
/// an **arrow head** of two five-pixel barbs for each of the five ahead. The
/// ticks and the barbs are fixed pixel sizes; only the line itself scales with
/// the map. The pen is `hpenStarbase`, the same blue the fleet paths use.
fn scale_line(app: &App, ui: &mut egui::Ui, to_screen: &impl Fn(f32, f32) -> Pos2, scale: f32) {
    let Some(line) = app.scan_scale_line() else {
        return;
    };
    let painter = ui.painter().clone();
    let [r, g, b] = stars_ui_arrow::PATH_COLOUR;
    let stroke = Stroke::new(1.0_f32, Color32::from_rgb(r, g, b));
    let at = to_screen(f32::from(line.at.x), f32::from(line.at.y));
    // The heading in screen terms: the map draws the galaxy upside down, so
    // the y component turns over.
    #[allow(clippy::cast_precision_loss)]
    let along = Vec2::new(line.heading.0 as f32, -line.heading.1 as f32);
    let length = along.length();
    if length <= 0.0 {
        return;
    }
    let unit = along / length;
    #[allow(clippy::cast_precision_loss)]
    let reach = line.reach() as f32 * scale;
    painter.line_segment([at - unit * reach, at + unit * reach], stroke);

    // A mark at each year, five back and five forward.
    let tick = Vec2::new(-unit.y, unit.x) * 4.9;
    #[allow(clippy::cast_precision_loss)]
    let year = line.year() as f32 * scale;
    for step in -stars_ui_arrow::SCALE_YEARS..=stars_ui_arrow::SCALE_YEARS {
        if step == 0 {
            continue;
        }
        #[allow(clippy::cast_precision_loss)]
        let mark = at + unit * (year * step as f32);
        if step < 0 {
            painter.line_segment([mark + tick, mark - tick], stroke);
        } else {
            // The two barbs of an arrow head, five pixels back from the mark
            // at forty-five degrees either side.
            for turn in [-std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_4] {
                let (sin, cos) = turn.sin_cos();
                let barb =
                    Vec2::new(-unit.x * cos - -unit.y * sin, -unit.x * sin + -unit.y * cos) * 5.0;
                painter.line_segment([mark + barb, mark], stroke);
            }
        }
    }
}

/// Draw the digits of the ship counts.
///
/// `DrawScanFleetCount` blits them out of `hbmpNumbers` — bitmap 249, eleven
/// 4x7 cells — through a mask, so the colour comes from the pen: a stencil
/// tinted, exactly as the fleet arrows are. Without the game's own sheet the
/// number is written as text instead, which is what this project drew before.
fn ship_count_digits(app: &mut App, ui: &mut egui::Ui, digits: &[(egui::Pos2, u8, Color32)]) {
    if digits.is_empty() {
        return;
    }
    let ctx = ui.ctx().clone();
    let painter = ui.painter().clone();
    let size = egui::vec2(DIGIT_WIDTH as f32, DIGIT_HEIGHT as f32);
    for (at, digit, colour) in digits {
        let drawn = app
            .art
            .as_mut()
            .and_then(|art| {
                art.stencil_at(
                    &ctx,
                    &stars_formats::resources::Name::Id(DIGIT_SHEET),
                    u32::from(*digit) * DIGIT_WIDTH,
                    0,
                    DIGIT_WIDTH,
                    DIGIT_HEIGHT,
                    size,
                )
            })
            .map(|image| {
                image
                    .tint(*colour)
                    .paint_at(ui, Rect::from_min_size(*at, size));
            })
            .is_some();
        if !drawn {
            painter.text(
                *at,
                egui::Align2::LEFT_TOP,
                digit.to_string(),
                egui::FontId::monospace(9.0),
                *colour,
            );
        }
    }
}

/// Draw the fleets that are sitting on the selected point.
///
/// `DrawScanner` puts an 11x11 glyph there rather than the arrow — blue for one
/// of this player's, red for anybody else's — so the fleet under the cursor is
/// picked out. Without the game's sheet it is a ring in the same colours.
fn fleet_glyphs(app: &mut App, ui: &mut egui::Ui, fleets: &[(egui::Pos2, (u32, u32))]) {
    if fleets.is_empty() {
        return;
    }
    let ctx = ui.ctx().clone();
    let painter = ui.painter().clone();
    let sheet = stars_formats::resources::Name::Text("ScannerBmp".to_string());
    for (at, cell) in fleets {
        let drawn = app
            .art
            .as_mut()
            .and_then(|art| {
                art.sprite_at(&ctx, &sheet, cell.0, cell.1, 11, 11, egui::vec2(11.0, 11.0))
            })
            .map(|image| {
                image.paint_at(
                    ui,
                    Rect::from_min_size(*at - Vec2::new(5.5, 5.5), egui::vec2(11.0, 11.0)),
                );
            })
            .is_some();
        if !drawn {
            let colour = if cell.1 == 0x24 {
                crate::SCAN_YOURS
            } else {
                crate::SCAN_OTHER
            };
            let colour = Color32::from_rgb(colour[0], colour[1], colour[2]);
            painter.circle_stroke(*at, 5.0, Stroke::new(1.5_f32, colour));
        }
    }
}
