//! The planet pane.
//!
//! The original's top-left pane: the selected planet as **six tiles in two
//! columns**, each with its own little title bar (`PlanetWndProc`,
//! `1048:0000`, over the `rgtilePlanet` table at `1120:07fc`).
//!
//! ```text
//! ┌───────────────────────┬───────────────────────┐
//! │ the planet, drawn     │ the fleets in orbit   │
//! ├───────────────────────┼───────────────────────┤
//! │ Minerals On Hand      │ Production            │
//! ├───────────────────────┼───────────────────────┤
//! │ Status                │ Starbase              │
//! └───────────────────────┴───────────────────────┘
//! ```
//!
//! The column each tile belongs to, its order and its height are read from that
//! table rather than guessed; see `docs/ui/planet-pane.md`. Every label and
//! every number format here is the original's.
//!
//! What is not the original: it draws the planet itself as a picture and its
//! tiles can be collapsed by clicking their title bars, and neither is
//! reproduced.

use crate::App;

/// Draw the planet pane.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    app.drawn_scope = "planet";
    let open = app.open_tiles;
    let mut open: Vec<bool> = open.to_vec();
    crate::views::tile_pane(
        app,
        ui,
        &crate::tiles::PLANET_TILES,
        &mut open,
        tile_title,
        tile_body,
    );
    for (index, value) in open.iter().enumerate() {
        app.open_tiles[index] = *value;
    }
}

/// What a tile's title bar says. Three of the six take their title from the
/// game rather than the table.
pub(crate) fn tile_title(app: &mut App, index: usize) -> String {
    match index {
        0 => app.planet_pane_title(),
        3 => app.pane_fleets_title().to_string(),
        5 => app.planet_starbase_tile().0,
        other => crate::tiles::PLANET_TILES[other].title.to_string(),
    }
}

/// What goes inside one tile.
pub(crate) fn tile_body(app: &mut App, ui: &mut egui::Ui, index: usize) {
    match index {
        0 => summary(app, ui),
        1 => grid(ui, "minerals", &app.planet_minerals_tile(), true),
        2 => grid(ui, "status", &app.planet_status_tile(), false),
        3 => crate::views::fleets_here_body(app, ui),
        4 => {
            production(ui, &app.planet_production_rows());
            let ours = app.selected_planet().is_some();
            if crate::views::flow_button(app, ui, "Change", ours).clicked() {
                app.open_production();
            }
        }
        _ => starbase(app, ui),
    }
}

/// The **Starbase** tile: four rows, a rule, the two the mass driver fills,
/// and then the **Set Dest** button with the warp gauge beside it.
///
/// `DrawPlanetStarbase` (`1048:22cc`) puts the button in the left **third**
/// of that last row and gives the gauge what is left of it, and it draws the
/// gauge only when there is a driver — the button is always there, with its
/// disabled style when there is not.
fn starbase(app: &mut App, ui: &mut egui::Ui) {
    let rows = app.planet_starbase_tile().1;
    let Some((driver_rows, above)) = rows.split_at_checked(RULE_AFTER).map(|(a, b)| (b, a)) else {
        grid(ui, "starbase", &rows, false);
        return;
    };
    // `DrawPlanetStarbase` draws the Damage figure in dark red when there is
    // any, and puts the text colour back afterwards, so it is the one row of
    // the four with a colour of its own.
    let damaged = app
        .pane_planet()
        .and_then(|planet| crate::popup::starbase_damage_pct(planet.starbase_damage))
        .is_some();
    rows_grid(ui, "starbase", above, &|_| None, &|row| {
        (damaged && row == DAMAGE_ROW).then(|| colour(DAMAGE_TEXT))
    });
    ui.separator();
    grid(ui, "starbase-driver", driver_rows, false);

    let Some(driver) = app.planet_mass_driver() else {
        return;
    };
    let line = ui.text_style_height(&egui::TextStyle::Small);
    let font = egui::TextStyle::Small.resolve(ui.style());
    let full = ui.available_width();
    ui.horizontal(|ui| {
        let button = egui::Button::new(egui::RichText::new("Set Dest").small())
            .min_size(egui::vec2(full / 3.0, line))
            .selected(app.set_packet_dest);
        if ui.add_enabled(driver.present(), button).clicked() {
            app.set_packet_dest = !app.set_packet_dest;
        }
        if !driver.present() {
            return;
        }
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), line), egui::Sense::hover());
        let gauge = crate::survey::Gauge {
            segments: vec![(driver.fill, risk_colour(driver.risk))],
            total: driver.total,
            label: driver.label.clone(),
        };
        crate::views::survey::gauge_bar(ui, ui.painter(), &gauge, rect, &font);
    });
}

/// How many rows come before the rule `DrawPlanetStarbase` draws across the
/// tile: Dock Capacity, Armor, Shields and Damage.
const RULE_AFTER: usize = 4;

/// Which of those four rows the damage figure is.
const DAMAGE_ROW: usize = 3;

/// `SetTextColor(0x00007f)` — the dark red a damaged base's figure is drawn
/// in. A COLORREF, so it reads blue-green-red.
const DAMAGE_TEXT: [u8; 3] = [0x7f, 0, 0];

/// A `[u8; 3]` as egui sees it.
fn colour([r, g, b]: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

/// The gauge's three brushes, as `FCreateStuff` (`1000:0160`) makes them:
/// `hbrPurple` `0x7f007f`, `hbrYellow` `0x00ffff` and `hbrRed` `0x0000ff`,
/// which are COLORREFs and so read blue-green-red.
fn risk_colour(risk: crate::app::Risk) -> [u8; 3] {
    match risk {
        crate::app::Risk::Safe => [0x7f, 0, 0x7f],
        crate::app::Risk::Risky => [0xff, 0xff, 0],
        crate::app::Risk::Dangerous => [0xff, 0, 0],
    }
}

/// A tile's rows: a label on the left and a value right-aligned against the
/// tile's inside edge.
///
/// `minerals` colours the first three labels the way `DrawPlanetMinSum` does —
/// the same `rgcrMin` blue, dark green and yellow the Selection Summary uses.
pub(crate) fn grid(ui: &mut egui::Ui, id: &str, rows: &[(String, String)], minerals: bool) {
    rows_grid(
        ui,
        id,
        rows,
        &|row| (minerals && row < 3).then(|| colour(crate::survey::MINERAL_TEXT[row])),
        &|_| None,
    );
}

/// The same grid, with a colour for any label and any value that wants one.
fn rows_grid(
    ui: &mut egui::Ui,
    id: &str,
    rows: &[(String, String)],
    label_colour: &dyn Fn(usize) -> Option<egui::Color32>,
    value_colour: &dyn Fn(usize) -> Option<egui::Color32>,
) {
    if rows.is_empty() {
        ui.label(egui::RichText::new("no data").weak().small());
        return;
    }
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([8.0, 0.0])
        .striped(false)
        .show(ui, |ui| {
            for (row, (label, value)) in rows.iter().enumerate() {
                let mut text = egui::RichText::new(label).small();
                if let Some(colour) = label_colour(row) {
                    text = text.color(colour);
                }
                ui.label(text);
                let mut shown = egui::RichText::new(value).small();
                if let Some(colour) = value_colour(row) {
                    shown = shown.color(colour);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(shown);
                });
                ui.end_row();
            }
        });
}

/// The production tile, whose rows are coloured by when they will be built —
/// red for one that practically never will be, which is the manual's warning
/// on p. 7-7.
fn production(ui: &mut egui::Ui, rows: &[(String, stars_core::production::EtaMark)]) {
    use stars_core::production::EtaMark;
    if rows.is_empty() {
        ui.label(egui::RichText::new("--- Queue is Empty ---").weak().small());
        return;
    }
    for (line, mark) in rows {
        let text = egui::RichText::new(line).small();
        ui.label(match mark {
            EtaMark::Never => text.color(egui::Color32::from_rgb(0xff, 0x6b, 0x6b)),
            EtaMark::AllNextYear => text.color(egui::Color32::from_rgb(0x5a, 0xd6, 0x8a)),
            EtaMark::FirstNextYear => text.color(egui::Color32::from_rgb(0xa3, 0xbf, 0x5a)),
            EtaMark::Idle => text.weak(),
            EtaMark::Ordinary => text,
        });
    }
}

/// The tallest tile: the planet itself, as a picture, with Prev and Next
/// beside it.
///
/// `DrawPlanShipBitmap` (`1048:3336`) draws it twelve pixels in and six down
/// (two, in the small layout): a 70-pixel sunken frame — shadow above and
/// left, highlight below and right, a black line inside — with the 64-pixel
/// face inside it, picked from the planet's id so a planet keeps the same
/// face all game. Then two buttons (`rghwndBtn[4]`, `[5]`) in a column to
/// its right, each `dyArial8 * 3 / 2` tall and three apart, starting two
/// pixels above the frame, as wide as the tile's inside less 95: **Prev** and
/// **Next**, which are `SelectAdjPlanet(±1)` and walk the player's own
/// planets. Without the game's own pictures the frame holds words instead.
fn summary(app: &mut App, ui: &mut egui::Ui) {
    let body = ui.max_rect();
    let small = app.window_layout == crate::WindowLayout::Small;
    let line = ui.text_style_height(&egui::TextStyle::Small);
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;
    let top = body.top() + if small { 0.0 } else { 4.0 };

    // The sunken frame, 70 square, and the face 3 pixels inside it.
    let frame = egui::Rect::from_min_size(egui::pos2(left, top), egui::vec2(70.0, 70.0));
    {
        let colour = |[r, g, b]: [u8; 3]| egui::Color32::from_rgb(r, g, b);
        let painter = ui.painter();
        let bar = |x: f32, y: f32, w: f32, h: f32, c: egui::Color32| {
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h)),
                0.0,
                c,
            );
        };
        bar(
            frame.left(),
            frame.top(),
            70.0,
            2.0,
            colour(crate::toolbar::SHADOW),
        );
        bar(
            frame.left(),
            frame.top(),
            2.0,
            70.0,
            colour(crate::toolbar::SHADOW),
        );
        bar(
            frame.left() + 2.0,
            frame.bottom() - 2.0,
            68.0,
            2.0,
            colour(crate::toolbar::HILITE),
        );
        bar(
            frame.right() - 2.0,
            frame.top() + 2.0,
            2.0,
            68.0,
            colour(crate::toolbar::HILITE),
        );
        bar(
            frame.left() + 2.0,
            frame.top() + 2.0,
            66.0,
            1.0,
            egui::Color32::BLACK,
        );
        bar(
            frame.left() + 2.0,
            frame.top() + 2.0,
            1.0,
            66.0,
            egui::Color32::BLACK,
        );
    }
    let face = egui::Rect::from_min_size(frame.min + egui::vec2(3.0, 3.0), egui::vec2(64.0, 64.0));
    let picture = app
        .pane_planet()
        .map(|planet| planet.id)
        .and_then(|id| app.planet_picture(id));
    let mut drawn = false;
    if let Some(cell) = picture {
        let mut child = ui.child_ui(face, egui::Layout::top_down(egui::Align::Min), None);
        drawn = crate::art::draw(app, &mut child, cell, 64.0);
    }
    if !drawn {
        // Without the game's pictures: whose it is, in words, where the
        // face would be.
        let mut child = ui.child_ui(face, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(face);
        match app.pane_planet() {
            None => {
                child.label(egui::RichText::new("no planet").weak().small());
            }
            Some(planet) => {
                let owner = match planet.owner {
                    Some(owner) => format!("player {}", owner + 1),
                    None => "unowned".to_string(),
                };
                child.label(egui::RichText::new(owner).small());
                let detail = match planet.detail {
                    stars_core::planet::Detail::Full => "yours",
                    stars_core::planet::Detail::Scanned => "scanned",
                    stars_core::planet::Detail::Minimal => "not surveyed",
                };
                child.label(egui::RichText::new(detail).small());
                if planet.homeworld {
                    child.label(egui::RichText::new("homeworld").small());
                }
                if planet.starbase {
                    child.label(egui::RichText::new("starbase").small());
                }
            }
        }
    }

    // Prev and Next, in a column beside the picture.
    let width = (right - left - 95.0).max(40.0);
    let height = (line * 3.0 / 2.0 - if small { 2.0 } else { 0.0 }).floor();
    let gap = if small { 2.0 } else { 3.0 };
    let mut y = top - if small { 2.0 } else { 4.0 };
    let me = i16::try_from(app.local_player()).unwrap_or(-1);
    let mine = app
        .game
        .as_ref()
        .is_some_and(|game| game.planets.iter().any(|p| p.owner == Some(me)));
    for (label, delta) in [("Prev", -1), ("Next", 1)] {
        let rect =
            egui::Rect::from_min_size(egui::pos2(right - width, y), egui::vec2(width, height));
        if crate::views::placed_button(app, ui, rect, label, mine).clicked() {
            app.select_adjacent_planet(delta);
        }
        y += height + gap;
    }
}
/// The frame every tile shares — `FDrawTileNC` (`1048:1086`).
///
/// A 3-D frame around the whole tile, a title bar one pixel inside it and
/// `dyArial8 + 2` tall with a 3-D frame of its own, the title **centred** in
/// Arial 8 bold, and a seventeen-pixel button at the title bar's right end
/// with a shadow line down its left. Clicking the title bar opens or closes
/// the tile, which is what `fPopped` records and what makes the column reflow.
///
/// Returns whether the tile was clicked.
pub fn tile_frame(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    title: &str,
    open: bool,
    line: f32,
) -> egui::Rect {
    let colour = |[r, g, b]: [u8; 3]| egui::Color32::from_rgb(r, g, b);
    let face = colour(crate::toolbar::FACE);
    let hilite = colour(crate::toolbar::HILITE);
    let shadow = colour(crate::toolbar::SHADOW);
    let fill = |r: egui::Rect, c: egui::Color32| painter.rect_filled(r, 0.0, c);
    let at = |x: f32, y: f32, w: f32, h: f32| {
        egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
    };
    // `_Draw3dFrame`: lit along the top and left, shadowed along the bottom
    // and right.
    let frame = |r: egui::Rect| {
        fill(at(r.left(), r.top(), r.width(), 1.0), hilite);
        fill(at(r.left(), r.top(), 1.0, r.height()), hilite);
        fill(at(r.left(), r.bottom() - 1.0, r.width(), 1.0), shadow);
        fill(at(r.right() - 1.0, r.top(), 1.0, r.height()), shadow);
    };

    fill(rect, face);
    frame(rect);

    let bar = egui::Rect::from_min_max(
        rect.min + egui::vec2(1.0, 1.0),
        egui::pos2(
            rect.right() - 1.0,
            rect.top() + 1.0 + line + crate::tiles::TITLE_EXTRA,
        ),
    );
    frame(bar);
    painter.text(
        bar.center(),
        egui::Align2::CENTER_CENTER,
        title,
        egui::TextStyle::Small.resolve(ui.style()),
        colour(crate::statusbar::TEXT),
    );

    // The button that opens and closes it, and the shadow line beside it.
    let button = egui::Rect::from_min_max(
        egui::pos2(
            rect.right() - 1.0 - crate::tiles::BUTTON_WIDTH,
            rect.top() + 1.0,
        ),
        egui::pos2(rect.right() - 1.0, bar.bottom() + 1.0),
    );
    fill(button, face);
    frame(button);
    // Open or closed is which way the mark points.
    let mid = button.center();
    let arm = 3.0;
    let points = if open {
        vec![
            egui::pos2(mid.x - arm, mid.y - arm / 2.0),
            egui::pos2(mid.x + arm, mid.y - arm / 2.0),
            egui::pos2(mid.x, mid.y + arm),
        ]
    } else {
        vec![
            egui::pos2(mid.x - arm, mid.y + arm / 2.0),
            egui::pos2(mid.x + arm, mid.y + arm / 2.0),
            egui::pos2(mid.x, mid.y - arm),
        ]
    };
    painter.add(egui::Shape::convex_polygon(
        points,
        colour(crate::statusbar::TEXT),
        egui::Stroke::NONE,
    ));
    fill(
        at(button.left() - 1.0, rect.top() + 1.0, 1.0, bar.height()),
        shadow,
    );

    // What is left for the body: from the line under the title bar to a
    // pixel above the tile's foot. The rows are laid a `line` apart with
    // no leading of their own, where the original's `dyArial8` carries
    // two pixels of it, so the body keeps every pixel the bar and the
    // frame can spare — the last row's descenders were being cut off.
    egui::Rect::from_min_max(
        egui::pos2(rect.left() + 2.0, bar.bottom() + 1.0),
        egui::pos2(rect.right() - 2.0, rect.bottom() - 1.0),
    )
}
