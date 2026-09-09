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
            if ui
                .add_enabled(
                    app.selected_planet().is_some(),
                    egui::Button::new(egui::RichText::new("Change").small()),
                )
                .clicked()
            {
                app.open_production();
            }
        }
        _ => grid(ui, "starbase", &app.planet_starbase_tile().1, false),
    }
}

/// A tile's rows: a label on the left and a value right-aligned against the
/// tile's inside edge.
///
/// `minerals` colours the first three labels the way `DrawPlanetMinSum` does —
/// the same `rgcrMin` blue, dark green and yellow the Selection Summary uses.
pub(crate) fn grid(ui: &mut egui::Ui, id: &str, rows: &[(String, String)], minerals: bool) {
    if rows.is_empty() {
        ui.label(egui::RichText::new("no data").weak().small());
        return;
    }
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([8.0, 1.0])
        .striped(false)
        .show(ui, |ui| {
            for (row, (label, value)) in rows.iter().enumerate() {
                let mut text = egui::RichText::new(label).small();
                if minerals && row < 3 {
                    let [r, g, b] = crate::survey::MINERAL_TEXT[row];
                    text = text.color(egui::Color32::from_rgb(r, g, b));
                }
                ui.label(text);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(value).small());
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

/// The tallest tile: the planet itself.
///
/// The original draws the planet as a picture here, sized to the tile. This
/// says in words what that picture says at a glance — whose it is, and whether
/// anybody has been.
fn summary(app: &mut App, ui: &mut egui::Ui) {
    // The planet's own face, when the game's pictures have been found. The
    // original draws it 64 pixels square in a sunken frame at the top of the
    // pane, and picks it from the planet's id, so a planet keeps the same face
    // all game.
    let picture = app
        .pane_planet()
        .map(|planet| planet.id)
        .and_then(|id| app.planet_picture(id));
    if let Some(cell) = picture {
        egui::Frame::none()
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .inner_margin(1.0)
            .show(ui, |ui| {
                crate::art::draw(app, ui, cell, 64.0);
            });
    }
    let Some(planet) = app.pane_planet() else {
        ui.label(egui::RichText::new("no planet selected").weak().small());
        return;
    };
    let owner = match planet.owner {
        Some(owner) => format!("player {}", owner + 1),
        None => "unowned".to_string(),
    };
    let detail = match planet.detail {
        stars_core::planet::Detail::Full => "yours",
        stars_core::planet::Detail::Scanned => "scanned",
        stars_core::planet::Detail::Minimal => "not surveyed",
    };
    ui.label(egui::RichText::new(format!("{owner} \u{2014} {detail}")).small());
    if planet.homeworld {
        ui.label(egui::RichText::new("homeworld").small());
    }
    if planet.starbase {
        ui.label(egui::RichText::new("starbase in orbit").small());
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

    // What is left for the body.
    egui::Rect::from_min_max(
        egui::pos2(
            rect.left() + 2.0,
            rect.top() + line + crate::tiles::BODY_EXTRA,
        ),
        egui::pos2(rect.right() - 2.0, rect.bottom() - 2.0),
    )
}
