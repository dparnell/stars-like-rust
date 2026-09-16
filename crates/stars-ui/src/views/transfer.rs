//! The Cargo Transfer dialog.
//!
//! `TransferDlg` (`1050:5686`), the **Xfer** button: dialog resource
//! `Cargo Transfer`, 280 by 163 dialog units with OK, Cancel and Help along
//! its foot (the template at `0x3461d5` in the executable), and everything
//! above them painted by `DrawXferDlg` (`1050:6908`):
//!
//! * two sides, each a framed square the width of its half
//!   (`GetXferLeftRightRcs`), with a title bar the height of a line and
//!   six rows under it a line and six apart — Fuel, Cargo, Ironium,
//!   Boranium, Germanium, Colonists — labels right-aligned 75 pixels in:
//!   the fleet in hand and the planet it orbits (Xfer), the fleet in hand
//!   and a fleet beside it (the fleets-here tile's Cargo), or the planet
//!   in hand and a fleet over it (the planet pane's Cargo);
//! * a fleet of the player's own has gauges (`DrawFleetGauge`) from 81
//!   pixels in to 4 from the edge; another player's fleet has figures in
//!   sunken frames and none on the cargo row; a planet has figures for
//!   the four holds and no fuel row (`DrawPlanetXferSide`);
//! * down the middle a pair of arrows a row, for fuel and the four holds
//!   but not the cargo total (`FSetupXferBtns`): the left arrow moves
//!   cargo **into the left side**, the right arrow into the right, one at
//!   a time, ten with Shift, a hundred with Ctrl, a thousand with both
//!   (`FTrackXfer`); an arrow is dead when its giver has none or its
//!   taker has no room (`UpdateXferBtns`), and the fuel pair is dead
//!   against a planet;
//! * a press or drag in one of the player's own gauges sets that hold to
//!   the pointer's share of the tank or hold.
//!
//! The arrows, the gauges and the three buttons are recorded as drawn
//! widgets under the scope `"xfer"` — the gauges by their row's label
//! (`Colonists gauge`, and `Colonists gauge (right)` on the right), the
//! arrows as `Colonists <` and `Colonists >` — so a test can press what the
//! tutorial's pages name. The dialog's ship mode, the **Split** button, is
//! `split.rs`.

use crate::dialog::Control;
use crate::App;

/// The rows, top to bottom, and which cargo kind each is
/// (`None` for the cargo total).
const ROWS: [(&str, Option<usize>); 6] = [
    ("Fuel", Some(stars_core::orders::FUEL)),
    ("Cargo", None),
    ("Ironium", Some(0)),
    ("Boranium", Some(1)),
    ("Germanium", Some(2)),
    ("Colonists", Some(stars_core::orders::COLONISTS)),
];

/// Draw the dialog's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    if app.xfer.is_none() {
        return;
    }
    app.drawn_scope = "xfer";
    let template = &crate::dialog::TRANSFER;
    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
    // `FTrackXfer`: 1, 10 with Shift, 100 with Ctrl, 1000 with both.
    let step = match (ctrl, shift) {
        (false, false) => 1,
        (false, true) => 10,
        (true, false) => 100,
        (true, true) => 1000,
    };

    let want = template.pixels();
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(
            ui.available_width(),
            want.y
                * template.scale(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    ui.available_size(),
                )),
        ),
        egui::Sense::hover(),
    );
    let scale = template.scale(rect);
    let line = ui.text_style_height(&egui::TextStyle::Small).ceil();
    let font = egui::TextStyle::Small.resolve(ui.style());
    let painter = ui.painter_at(rect);

    // The two halves, above the button row.
    let foot = rect.top()
        + f32::from(template.control(1).map_or(145, |c| c.at.1)) * crate::dialog::DLU_Y * scale;
    let whole = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), foot));
    let mid = whole.center().x;
    let margin = 4.0 + line + 3.0;
    let left = egui::Rect::from_min_max(
        egui::pos2(whole.left() + margin - (line + 1.0), whole.top() + 4.0),
        egui::pos2(mid - margin, whole.bottom() - 4.0),
    );
    let right = egui::Rect::from_min_max(
        egui::pos2(mid + margin, whole.top() + 4.0),
        egui::pos2(whole.right() - margin + line + 1.0, whole.bottom() - 4.0),
    );

    let Some(dialog) = app.xfer.clone() else {
        return;
    };
    let name_of = |app: &App, object: crate::app::XferObject| match object {
        crate::app::XferObject::Fleet(index) => app.fleet_display_name(index),
        crate::app::XferObject::Planet(id) => app.planet_name(id),
        crate::app::XferObject::Packet(index) => app.thing_name(crate::ScanThing::Packet(index)),
        // `idsDeepSpace`, the title `DrawPlanetXferSide` gives a side
        // whose planet id is -1.
        crate::app::XferObject::Space => "Deep Space".to_string(),
    };
    let names = [
        name_of(app, dialog.objects[0]),
        name_of(app, dialog.objects[1]),
    ];

    // Each side is a square as wide as its half, framed, with a title bar.
    let row_step = line + 6.0;
    let side = |ui: &egui::Ui, painter: &egui::Painter, rect: egui::Rect, title: &str| {
        let square = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), rect.width()));
        frame_3d(painter, square);
        let bar = egui::Rect::from_min_size(
            square.min + egui::vec2(1.0, 1.0),
            egui::vec2(square.width() - 2.0, line + 2.0),
        );
        frame_3d(painter, bar);
        painter.text(
            bar.center(),
            egui::Align2::CENTER_CENTER,
            title,
            font.clone(),
            ui.visuals().text_color(),
        );
        // The rows start three under the title bar.
        bar.bottom() + 3.0
    };
    let halves = [left, right];
    let tops = [
        side(ui, &painter, left, &names[0]),
        side(ui, &painter, right, &names[1]),
    ];

    let label_x = 75.0;
    let mut moves: Vec<(usize, i32)> = Vec::new();
    let mut sets: Vec<(usize, usize, i32)> = Vec::new();

    for (row, (label, kind)) in ROWS.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let dy = row as f32 * row_step;

        for which in 0..2 {
            let half = halves[which];
            let y = tops[which] + dy;
            let planet = dialog.is_planet(which);
            let packet = dialog.is_packet(which);
            // A planet has no fuel row and no cargo total
            // (`DrawPlanetXferSide`); a fleet has all six labels, and a
            // fleet not the player's own shows figures where the gauges
            // would be, with none on the cargo row
            // (`DrawFleetCargoXferSide`); a packet starts a row down with
            // `Packet Shell` over the three minerals, and has no
            // colonists row (`DrawThingXferSide`).
            if planet && row < 2 {
                continue;
            }
            if packet && (row == 0 || row == 5) {
                continue;
            }
            let label = if packet && row == 1 {
                "Packet Shell"
            } else {
                label
            };
            painter.text(
                egui::pos2(half.left() + 4.0 + label_x, y),
                egui::Align2::RIGHT_TOP,
                label,
                font.clone(),
                ui.visuals().text_color(),
            );
            if (dialog.own[which] && !planet) || packet {
                // --- a gauge, which is a control: a press or drag along
                // it sets the hold.
                let gauge_rect = egui::Rect::from_min_max(
                    egui::pos2(half.left() + 4.0 + label_x + 6.0, y),
                    egui::pos2(half.right() - 4.0, y + line),
                );
                let has = dialog.has[which];
                let (amount, total, colour, unit) = match kind {
                    Some(k) if *k == stars_core::orders::FUEL => (
                        has[*k],
                        dialog.fuel_capacity[which],
                        crate::survey::CARGO_COLOURS[4],
                        "mg",
                    ),
                    Some(k) => (
                        has[*k],
                        dialog.cargo_capacity[which],
                        crate::survey::CARGO_COLOURS[*k],
                        "kT",
                    ),
                    None => (
                        dialog.cargo(which),
                        dialog.cargo_capacity[which],
                        crate::survey::CARGO_COLOURS[0],
                        "kT",
                    ),
                };
                let gauge = crate::survey::Gauge {
                    segments: if kind.is_none() {
                        let mut all: Vec<(i32, [u8; 3])> = (0..3)
                            .map(|i| (has[i], crate::survey::CARGO_COLOURS[i]))
                            .collect();
                        if !packet {
                            all.push((
                                has[stars_core::orders::COLONISTS],
                                crate::survey::CARGO_COLOURS[3],
                            ));
                        }
                        all
                    } else {
                        vec![(amount, colour)]
                    },
                    total,
                    label: format!("{amount} of {total}{unit}"),
                };
                crate::views::survey::gauge_bar(ui, &painter, &gauge, gauge_rect, &font);
                // `FTrackXfer` drags a gauge of the player's own fleet, or
                // a packet's mineral gauges, never its fuel against a
                // planet.
                let can_drag = kind.is_some_and(|k| {
                    if packet {
                        k < 3
                    } else {
                        k != stars_core::orders::FUEL || dialog.fuel_moves()
                    }
                });
                let response = ui.interact(
                    gauge_rect,
                    ui.id().with(("xfer-gauge", which, row)),
                    if can_drag {
                        egui::Sense::click_and_drag()
                    } else {
                        egui::Sense::hover()
                    },
                );
                let name = if which == 0 {
                    format!("{label} gauge")
                } else {
                    format!("{label} gauge (right)")
                };
                crate::views::record(app, ui, &name, &response);
                if can_drag && (response.clicked() || response.dragged()) {
                    if let (Some(k), Some(at)) = (kind, response.interact_pointer_pos()) {
                        // `FTrackXfer`: the pointer's place along the bar,
                        // less the two pixels of frame, as a share of the
                        // capacity.
                        let width = (gauge_rect.width() - 2.0).max(1.0);
                        let share = ((at.x - gauge_rect.left()) / width).clamp(0.0, 1.0);
                        #[allow(clippy::cast_possible_truncation)]
                        let want = (f64::from(share) * f64::from(total)).round() as i32;
                        sets.push((which, *k, want));
                    }
                }
            } else if let Some(k) = kind {
                // --- a figure in a sunken frame: `%ld kT`, or `%ld mg`
                // for another player's fuel.
                let value_rect = egui::Rect::from_min_max(
                    egui::pos2(half.left() + 4.0 + label_x + 4.0, y - 1.0),
                    egui::pos2(half.right() - 4.0, y + line + 1.0),
                );
                frame_3d(&painter, value_rect);
                let unit = if *k == stars_core::orders::FUEL {
                    "mg"
                } else {
                    "kT"
                };
                painter.text(
                    egui::pos2(value_rect.right() - 3.0, y),
                    egui::Align2::RIGHT_TOP,
                    format!("{} {unit}", dialog.has[which][*k]),
                    font.clone(),
                    ui.visuals().text_color(),
                );
            }
        }

        // --- the arrows between, for fuel and the four holds: the left
        // arrow moves into the left side, the right into the right. Each
        // is dead when its giver has none or its taker has no room
        // (`UpdateXferBtns`); the fuel pair is dead against a planet and
        // left out altogether against a space object (`FSetupXferBtns`).
        let fuel_pair_shown =
            *kind != Some(stars_core::orders::FUEL) || (0..2).all(|side| !dialog.is_packet(side));
        if let (Some(k), true) = (kind, fuel_pair_shown) {
            let size = line + 3.0;
            let y = tops[0] + dy - 2.0;
            let into_left =
                egui::Rect::from_min_size(egui::pos2(mid - size + 1.0, y), egui::vec2(size, size));
            let into_right =
                egui::Rect::from_min_size(egui::pos2(mid + 3.0, y), egui::vec2(size, size));
            let colonists_stay = *k == stars_core::orders::COLONISTS && !dialog.colonists_move();
            let live = |taker: usize, giver: usize| {
                (*k != stars_core::orders::FUEL || dialog.fuel_moves())
                    && !colonists_stay
                    && dialog.has[giver][*k] > 0
                    && dialog.room(taker, *k) > 0
            };
            let name = format!("{label} <");
            if crate::views::placed_button_named(app, ui, into_left, "<", &name, live(0, 1))
                .clicked()
            {
                moves.push((*k, step));
            }
            let name = format!("{label} >");
            if crate::views::placed_button_named(app, ui, into_right, ">", &name, live(1, 0))
                .clicked()
            {
                moves.push((*k, -step));
            }
        }
    }

    for (side, kind, want) in sets {
        app.xfer_set(side, kind, want);
    }
    for (kind, delta) in moves {
        app.xfer_move(kind, delta);
    }

    // The row along the foot, from the template.
    let at = |id: u16| -> egui::Rect {
        let control = template.control(id);
        let (x, y, w, h) = control.map_or((0, 0, 0, 0), |c| c.at);
        egui::Rect::from_min_size(
            rect.min
                + egui::vec2(
                    f32::from(x) * crate::dialog::DLU_X * scale,
                    f32::from(y) * crate::dialog::DLU_Y * scale,
                ),
            egui::vec2(
                f32::from(w) * crate::dialog::DLU_X * scale,
                f32::from(h) * crate::dialog::DLU_Y * scale,
            ),
        )
    };
    let caption = |id: u16| -> String {
        template
            .control(id)
            .map_or_else(String::new, Control::label)
    };
    if crate::views::placed_button(app, ui, at(0x1), &caption(0x1), true).clicked() {
        app.xfer_ok();
    }
    if crate::views::placed_button(app, ui, at(0x2), &caption(0x2), true).clicked() {
        app.xfer_cancel();
    }
    // Help is `WINHELP(HELP_CONTEXT, 0x433)` (`1050:59be`): the cargo
    // transfer page of the player's guide.
    if crate::views::placed_button(app, ui, at(0x76), &caption(0x76), true).clicked() {
        app.help_context(crate::help::context::CARGO_TRANSFER);
    }
}

/// `_Draw3dFrame(hdc, rc, 0)`: one ring, lit along the top and left and
/// shadowed along the bottom and right.
pub(crate) fn frame_3d(painter: &egui::Painter, rect: egui::Rect) {
    let colour = |[r, g, b]: [u8; 3]| egui::Color32::from_rgb(r, g, b);
    let hilite = colour(crate::toolbar::HILITE);
    let shadow = colour(crate::toolbar::SHADOW);
    let fill = |r: egui::Rect, c: egui::Color32| painter.rect_filled(r, 0.0, c);
    let at = |x: f32, y: f32, w: f32, h: f32| {
        egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
    };
    fill(at(rect.left(), rect.top(), rect.width(), 1.0), hilite);
    fill(at(rect.left(), rect.top(), 1.0, rect.height()), hilite);
    fill(
        at(rect.left(), rect.bottom() - 1.0, rect.width(), 1.0),
        shadow,
    );
    fill(
        at(rect.right() - 1.0, rect.top(), 1.0, rect.height()),
        shadow,
    );
}
