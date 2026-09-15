//! The battle VCR.
//!
//! The board is drawn from [`crate::vcr::Vcr`], which **plays the recording**
//! rather than re-simulating it — see that module for why. This view adds only
//! the transport controls and the drawing.
//!
//! `BattleVCR` is a **window of its own** (`hwndVCRDlg`), which the Battle
//! Summary Report opens over itself when a row is clicked a second time. The
//! list of battles that used to live down the left of this view is that
//! report — see `crate::views::report`.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::vcr::Event;
use crate::views::player_colour;
use crate::App;

/// Draw the VCR, which shows whichever recording is open.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some(vcr) = app.vcr.as_ref() else {
        ui.label("Choose a battle.");
        return;
    };

    // The five transport buttons, with the template's own captions and its
    // own order. `EnableVCRButtons` (`10e8:48f6`) decides which are alive
    // from one number: the two backward ones once anything has played, the
    // three forward ones until the last frame.
    let back = vcr.can_rewind();
    let on = vcr.can_advance();
    let (position_now, last) = (vcr.position(), vcr.len());
    let caption = |id: u16| {
        crate::dialog::BATTLE_VCR
            .control(id)
            .map_or(String::new(), crate::dialog::Control::label)
    };
    // The buttons are recorded under the "vcr" scope, so the tutor can ring
    // Done and a test can press the transport.
    app.drawn_scope = "vcr";
    let mut action: Option<u16> = None;
    let mut seek: Option<usize> = None;
    ui.horizontal(|ui| {
        let mut button = |ui: &mut egui::Ui, id: u16, enabled: bool, tip: &str| {
            let label = caption(id);
            let response = ui
                .add_enabled(enabled, egui::Button::new(&label))
                .on_hover_text(tip);
            crate::views::note_widget(app, ui, &label, response.rect, enabled);
            if response.clicked() {
                action = Some(id);
            }
        };
        button(ui, 0xa1, back, "back to the start");
        button(ui, 0xa2, back, "step back");
        // `>/||` is one button that plays and pauses, which is what the
        // caption says.
        button(ui, 0xa3, on, "play, or pause");
        button(ui, 0xa4, on, "step");
        button(ui, 0xa5, on, "to the end");
        button(ui, 0x1, true, "close the VCR");
        ui.separator();
        let mut position = position_now;
        if ui
            .add(egui::Slider::new(&mut position, 0..=last).text("frame"))
            .changed()
        {
            seek = Some(position);
        }
    });
    let now = ui.input(|i| i.time);
    let Some(vcr) = app.vcr.as_mut() else {
        return;
    };
    let before = vcr.position();
    if let Some(position) = seek {
        vcr.seek(position);
        app.playing = false;
    }
    match action {
        Some(0xa1) => {
            vcr.rewind();
            app.playing = false;
        }
        Some(0xa2) => {
            vcr.back();
            app.playing = false;
        }
        Some(0xa3) => {
            app.playing = !app.playing;
            app.vcr_frame_entered = now;
        }
        Some(0xa4) => {
            vcr.step();
            app.playing = false;
        }
        Some(0xa5) => {
            vcr.end();
            app.playing = false;
        }
        Some(0x1) => {
            app.close_battle();
            return;
        }
        _ => {}
    }
    let Some(vcr) = app.vcr.as_mut() else {
        return;
    };
    // Playing to the end stops there, as the forward buttons dying says it
    // should.
    if !vcr.can_advance() {
        app.playing = false;
    }

    // Advance while playing, a frame every `VCR_FRAME` seconds — longer
    // when the frame's torpedoes are still flying; the shell keeps the
    // repaints coming whenever `playing` is set.
    let hold = crate::app::VCR_FRAME
        .max(flight_seconds(vcr.frame().map(|f| &f.event), vcr.tokens()) + 0.15);
    if app.playing && now - app.vcr_frame_entered >= hold {
        if vcr.step() {
            app.vcr_frame_entered = now;
        } else {
            app.playing = false;
        }
    }
    if vcr.position() != before {
        app.vcr_frame_entered = now;
    }
    if app.playing {
        ui.ctx().request_repaint();
    }

    // `DrawVCR` (`10e8:1c62`): "Phase %d/%d Round %d/%d" over the panel,
    // then what the frame did.
    ui.label(format!(
        "Phase {}/{}  Round {}/{}",
        vcr.position(),
        vcr.len(),
        u32::from(vcr.round()) + 1,
        vcr.frames().last().map_or(1, |f| u32::from(f.round) + 1)
    ));
    if let Some(frame) = vcr.frame() {
        ui.label(describe(&frame.event));
    } else {
        ui.label("before the first move");
    }
    ui.separator();

    // The board, `DrawVCR`'s way: ten squares of `dxyVCRSquare` three
    // apart from an origin of (10, 10), each framed in black; an empty one
    // filled black, an occupied one showing its first stack's ship as
    // `DrawFleetBitmap` draws it — the picture, the owner's emblem over its
    // bottom-left corner and a cross per further stack — and the focus
    // square framed in blue two pixels wide.
    // `dxyVCRSquare`: 64 when the window has room for the large board
    // beside the panel, else 32.
    let available = ui.available_width() - 240.0;
    let square: f32 = if available >= 10.0 * 67.0 + 20.0 {
        64.0
    } else {
        32.0
    };
    let pitch = square + 3.0;
    let board_side = 10.0 * pitch + 17.0;
    let mut clicked_square: Option<(u8, u8)> = None;
    let mut clicked_token: Option<usize> = None;
    // What the board is drawn from, taken out of the recording so the
    // pictures can be borrowed from the app while it is drawn.
    let (tokens, focus, event) = {
        let Some(vcr) = app.vcr.as_ref() else {
            return;
        };
        (
            vcr.tokens().to_vec(),
            vcr.focus,
            vcr.frame().map(|f| f.event.clone()),
        )
    };
    ui.horizontal_top(|ui| {
        let (response, painter) = ui.allocate_painter(
            Vec2::new(board_side, board_side.max(ui.available_height())),
            Sense::click(),
        );
        let rect = response.rect;
        crate::views::record(app, ui, "board", &response);
        // Whole pixels, so the one-pixel gaps between the squares stay.
        let origin = (rect.min + Vec2::new(10.0, 10.0)).round();
        let [r, g, b] = crate::toolbar::FACE;
        painter.rect_filled(rect, 0.0, Color32::from_rgb(r, g, b));
        // The board's own sunken frame: shadow along the top and left, a
        // highlight along the bottom and right.
        let board = Rect::from_min_size(
            rect.min + Vec2::new(8.0, 8.0),
            Vec2::splat(10.0 * pitch + 3.0),
        );
        let [sr, sg, sb] = crate::toolbar::SHADOW;
        let [hr, hg, hb] = crate::toolbar::HILITE;
        painter.rect_filled(board, 0.0, Color32::from_rgb(sr, sg, sb));
        painter.rect_filled(
            Rect::from_min_max(board.min + Vec2::splat(2.0), board.max + Vec2::splat(1.0)),
            0.0,
            Color32::from_rgb(hr, hg, hb),
        );
        painter.rect_filled(
            Rect::from_min_max(board.min + Vec2::splat(2.0), board.max - Vec2::splat(1.0)),
            0.0,
            Color32::from_rgb(r, g, b),
        );

        let square_rect = |x: u8, y: u8| {
            Rect::from_min_size(
                origin + Vec2::new(f32::from(x) * pitch, f32::from(y) * pitch),
                Vec2::splat(square + 2.0),
            )
        };
        let centre_of = |x: u8, y: u8| square_rect(x, y).min + Vec2::splat(1.0 + square / 2.0);

        let tokens = tokens.as_slice();
        let focus_square = focus
            .and_then(|i| tokens.get(i))
            .filter(|t| t.active && t.ships > 0)
            .and_then(|t| t.square);
        let ctx = ui.ctx().clone();
        for y in 0..10u8 {
            for x in 0..10u8 {
                let cell = square_rect(x, y);
                let here: Vec<&crate::vcr::Token> = tokens
                    .iter()
                    .filter(|t| t.active && t.ships > 0 && t.square == Some((x, y)))
                    .collect();
                painter.rect_stroke(cell, 0.0, Stroke::new(1.0_f32, Color32::BLACK));
                if here.is_empty() {
                    painter.rect_filled(cell, 0.0, Color32::BLACK);
                    continue;
                }
                // The focus token's stack when it stands here, else the first.
                let shown = focus
                    .and_then(|f| here.iter().find(|t| t.index == f))
                    .copied()
                    .unwrap_or(here[0]);
                let inner = cell.shrink(1.0);
                token_picture(app, ui, &ctx, inner, shown, here.len(), square < 64.0);
            }
        }
        if let Some((x, y)) = focus_square {
            let cell = square_rect(x, y);
            let blue = Color32::from_rgb(0x00, 0x00, 0x7f);
            painter.rect_filled(
                Rect::from_min_size(cell.min, Vec2::new(square + 1.0, 2.0)),
                0.0,
                blue,
            );
            painter.rect_filled(
                Rect::from_min_size(cell.min, Vec2::new(2.0, square + 1.0)),
                0.0,
                blue,
            );
            painter.rect_filled(
                Rect::from_min_size(
                    cell.min + Vec2::new(square, 0.0),
                    Vec2::new(2.0, square + 1.0),
                ),
                0.0,
                blue,
            );
            painter.rect_filled(
                Rect::from_min_size(
                    cell.min + Vec2::new(0.0, square),
                    Vec2::new(square + 1.0, 2.0),
                ),
                0.0,
                blue,
            );
        }

        // The frame's shots, as `AnimateAttack` (`10e8:3ac2`) draws them.
        if let Some(Event::Fire {
            attacker, shots, ..
        }) = &event
        {
            if let Some(from) = tokens.get(*attacker).and_then(|t| t.square) {
                let entered = app.vcr_frame_entered;
                animate_attack(
                    app,
                    ui,
                    &ctx,
                    &painter,
                    from,
                    shots,
                    tokens,
                    square,
                    centre_of,
                    now - entered,
                );
            }
        }

        if response.clicked() {
            if let Some(at) = response.interact_pointer_pos() {
                let x = ((at.x - origin.x) / pitch).floor();
                let y = ((at.y - origin.y) / pitch).floor();
                if (0.0..10.0).contains(&x) && (0.0..10.0).contains(&y) {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    {
                        clicked_square = Some((x as u8, y as u8));
                    }
                }
            }
        }

        // The panel beside it.
        ui.vertical(|ui| {
            ui.set_min_width(220.0);
            let Some(vcr) = app.vcr.as_ref() else {
                return;
            };
            if let Some(index) = vcr.focus {
                if let Some(token) = vcr.tokens().get(index) {
                    if let Some((x, y)) = token.square {
                        ui.label(format!("Selection {x},{y}"));
                    }
                    ui.colored_label(
                        player_colour(i16::from(token.player)),
                        app.player_name(usize::from(token.player)),
                    );
                    ui.label(format!("{} ({})", design_name(app, token), token.ships));
                    if !token.active || token.ships == 0 {
                        ui.label("Dead");
                    } else {
                        ui.label(format!("Shields: {}", token.shields * token.ships));
                    }
                    ui.separator();
                }
            }
            ui.heading("tokens");
            for token in vcr.tokens() {
                let colour = player_colour(i16::from(token.player));
                let state = if !token.active || token.ships == 0 {
                    "destroyed".to_string()
                } else {
                    format!("{} ships", token.ships)
                };
                let row = ui.selectable_label(
                    vcr.focus == Some(token.index),
                    egui::RichText::new(format!(
                        "{}: {} — {state}{}",
                        token.index,
                        design_name(app, token),
                        if token.armed { "" } else { ", unarmed" }
                    ))
                    .color(colour),
                );
                if row.clicked() {
                    clicked_token = Some(token.index);
                }
            }
            ui.separator();
            ui.heading("losses");
            let Some(vcr) = app.vcr.as_ref() else {
                return;
            };
            for (player, lost) in vcr.losses() {
                ui.colored_label(
                    player_colour(i16::from(player)),
                    format!("{}: {lost} ships", app.player_name(usize::from(player))),
                );
            }
        });
    });

    if let Some(index) = clicked_token {
        if let Some(vcr) = app.vcr.as_mut() {
            vcr.focus = Some(index);
        }
    }
    // A click on a square picks out the stack there — the next one along
    // when the square's stack is already the focus, as `VCRDlg`'s
    // `WM_LBUTTONDOWN` cycles them.
    if let Some(at) = clicked_square {
        if let Some(vcr) = app.vcr.as_mut() {
            let here: Vec<usize> = vcr
                .tokens()
                .iter()
                .filter(|t| t.active && t.ships > 0 && t.square == Some(at))
                .map(|t| t.index)
                .collect();
            vcr.focus = match vcr.focus.and_then(|f| here.iter().position(|&i| i == f)) {
                Some(n) => here.get((n + 1) % here.len()).copied(),
                None => here.first().copied(),
            };
        }
    }
}

/// How long a torpedo's flight across `dx`, `dy` squares takes: eight
/// steps a square at fifteen milliseconds, and never under a fifth of a
/// second so a shot at the next square can be seen.
fn flight_of(dx: i32, dy: i32) -> f64 {
    (f64::from(dx.abs().max(dy.abs()) * 8) * 0.015).max(0.2)
}

/// The longest torpedo flight a frame draws, in seconds — nothing for a
/// frame with no torpedoes in it.
fn flight_seconds(event: Option<&Event>, tokens: &[crate::vcr::Token]) -> f64 {
    let Some(Event::Fire {
        attacker, shots, ..
    }) = event
    else {
        return 0.0;
    };
    let Some(from) = tokens.get(*attacker).and_then(|t| t.square) else {
        return 0.0;
    };
    shots
        .iter()
        .filter(|shot| shot.torpedo())
        .filter_map(|shot| tokens.get(shot.target).and_then(|t| t.square))
        .map(|to| {
            flight_of(
                i32::from(to.0) - i32::from(from.0),
                i32::from(to.1) - i32::from(from.1),
            )
        })
        .fold(0.0, f64::max)
}

/// The name of the design a token was built to.
fn design_name(app: &App, token: &crate::vcr::Token) -> String {
    app.game
        .as_ref()
        .and_then(|g| g.designs.get(usize::from(token.player)))
        .and_then(|d| d.get(usize::from(token.design)))
        .map_or_else(|| format!("design {}", token.design), |d| d.name.clone())
}

/// One square's stack, as `DrawFleetBitmap` (`1050:490e`) draws it for the
/// VCR: the ship's picture at 32 or 64 pixels, the owner's emblem over its
/// bottom-left corner (eight pixels on the small picture, sixteen on the
/// large) and, for every further stack on the square up to three, a small
/// cross in the next corner. Without the pictures the square is painted
/// in the owner's colour with the stack's number on it.
fn token_picture(
    app: &mut App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    rect: Rect,
    token: &crate::vcr::Token,
    stacks: usize,
    small: bool,
) {
    let picture = app
        .game
        .as_ref()
        .and_then(|g| g.designs.get(usize::from(token.player)))
        .and_then(|d| d.get(usize::from(token.design)))
        .map(|d| u16::from(d.picture));
    let size = if small {
        stars_formats::resources::art::ShipSize::Small
    } else {
        stars_formats::resources::art::ShipSize::Large
    };
    let emblem_size = if small {
        stars_formats::resources::art::EmblemSize::Small
    } else {
        stars_formats::resources::art::EmblemSize::Medium
    };
    let emblem = app.emblem_of(usize::from(token.player), emblem_size);
    let mut drawn = false;
    if let (Some(picture), Some(art)) = (picture, app.art.as_mut()) {
        let cell = stars_formats::resources::art::ship(picture, size);
        if let Some(image) = art.sprite_at_size(ctx, cell, rect.size()) {
            image.paint_at(ui, rect);
            drawn = true;
        }
        if drawn {
            if let Some(emblem) = emblem {
                let side =
                    if small { 8.0 } else { 16.0 } * rect.width() / if small { 32.0 } else { 64.0 };
                let at = Rect::from_min_size(
                    egui::pos2(rect.left(), rect.bottom() - side),
                    Vec2::splat(side),
                );
                if let Some(image) = art.sprite_at_size(ctx, emblem, at.size()) {
                    image.paint_at(ui, at);
                }
            }
        }
    }
    let painter = ui.painter();
    if !drawn {
        let colour = player_colour(i16::from(token.player));
        painter.rect_filled(rect, 0.0, colour);
        if !token.armed {
            painter.rect_filled(rect.shrink(3.0), 0.0, Color32::from_rgb(8, 10, 18));
        }
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            token.index.to_string(),
            egui::FontId::proportional(rect.width() * 0.4),
            Color32::WHITE,
        );
    }
    // The crosses: `DrawFleetBitmap` lays one per extra stack, arms of
    // five pixels on the small picture and eight on the large, in the
    // top-left, bottom-right, top-right and bottom-left corners in turn.
    let arm = if small { 5.0 } else { 8.0 };
    let thick = if small { 1.0 } else { 2.0 };
    for n in 0..stacks.saturating_sub(1).min(3) {
        let bottom = (n & 1 == 1) != ((n & 2) == 2);
        let right = n & 2 != 0;
        let x = if right {
            rect.right() - 2.0 - arm
        } else {
            rect.left() + 2.0
        };
        let y = if bottom {
            rect.bottom() - 2.0 - arm
        } else {
            rect.top() + 2.0
        };
        let mid = (arm - 1.0) / 2.0;
        painter.rect_filled(
            Rect::from_min_size(egui::pos2(x + mid, y), Vec2::new(thick, arm)),
            0.0,
            Color32::WHITE,
        );
        painter.rect_filled(
            Rect::from_min_size(egui::pos2(x, y + mid), Vec2::new(arm, thick)),
            0.0,
            Color32::WHITE,
        );
    }
}

/// The frame's shots, as `AnimateAttack` (`10e8:3ac2`) draws them. Every
/// target the shot reached gets: for a beam, two lines from the near edge
/// of the attacker's square to the target's centre — red, or blue when the
/// weapon's second flag is set — and the small burst on the target; for a
/// torpedo, one of the four torpedo frames flying from the attacker to the
/// target while the VCR plays (`fAnimate`), then the middle burst on the
/// target unless the torpedoes were deflected; and last, on every target,
/// the large burst where ships were destroyed and the small one where not.
#[allow(clippy::too_many_arguments)]
fn animate_attack(
    app: &mut App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    painter: &egui::Painter,
    from: (u8, u8),
    shots: &[crate::vcr::Shot],
    tokens: &[crate::vcr::Token],
    square: f32,
    centre_of: impl Fn(u8, u8) -> Pos2,
    elapsed: f64,
) {
    use stars_formats::resources::art::VCR_ICONS;
    // The icons are 32 pixels, the small square's size; they grow with it.
    let icon_size = square;
    let icon = |app: &mut App, ui: &mut egui::Ui, which: usize, at: Pos2| {
        let rect = Rect::from_center_size(at, Vec2::splat(icon_size));
        let drawn = app
            .art
            .as_mut()
            .and_then(|art| art.icon(ctx, VCR_ICONS[which], icon_size))
            .map(|image| image.paint_at(ui, rect))
            .is_some();
        if !drawn {
            // Without the icons: a red burst of the icon's size.
            let radius = match which {
                0 => 3.0,
                1 => 6.0,
                2 => 10.0,
                _ => 2.0,
            } * icon_size
                / 32.0;
            painter.circle_filled(at, radius, Color32::from_rgb(0xff, 0x00, 0x00));
        }
    };
    let here = centre_of(from.0, from.1);
    let third = square / 3.0;
    let mut in_flight = false;
    for shot in shots {
        let Some(to) = tokens.get(shot.target).and_then(|t| t.square) else {
            continue;
        };
        if to == from {
            continue;
        }
        let there = centre_of(to.0, to.1);
        let dx = i32::from(to.0) - i32::from(from.0);
        let dy = i32::from(to.1) - i32::from(from.1);
        // The two points the beams leave from: a third of a square either
        // side of the centre, along the edge that faces the target.
        let (a, b) = if dx.abs() > dy.abs() {
            let edge = here.x + third * dx.signum() as f32;
            (
                Pos2::new(edge, here.y - third),
                Pos2::new(edge, here.y + third),
            )
        } else {
            let edge = here.y + third * dy.signum() as f32;
            (
                Pos2::new(here.x - third, edge),
                Pos2::new(here.x + third, edge),
            )
        };
        if shot.beam() {
            let colour = if shot.weapon & 0x02 != 0 {
                Color32::from_rgb(0x00, 0x00, 0xff)
            } else {
                Color32::from_rgb(0xff, 0x00, 0x00)
            };
            let pen = Stroke::new(1.0_f32, colour);
            painter.line_segment([a, there], pen);
            painter.line_segment([b, there], pen);
            icon(app, ui, 0, there);
        }
        if shot.torpedo() {
            // The flight: eight steps a square, each held `0x23 − 10 ×
            // viSpeedVCR` ticks in the original — a middling fifteen
            // milliseconds here — from the moment the frame was entered,
            // whether it was stepped to or played.
            let steps = f64::from(dx.abs().max(dy.abs()) * 8);
            let flight = flight_of(dx, dy);
            let t = if elapsed < flight {
                (elapsed / flight).clamp(0.0, 1.0)
            } else {
                1.0
            };
            if t < 1.0 {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let step = (t * steps) as usize;
                let at = a + (there - a) * t as f32;
                icon(app, ui, 3 + (step & 3), at);
                in_flight = true;
                ctx.request_repaint();
            } else if !shot.deflected() {
                icon(app, ui, 1, there);
            }
        }
    }
    // The bursts land once everything has arrived.
    if in_flight {
        return;
    }
    for shot in shots {
        let Some(to) = tokens.get(shot.target).and_then(|t| t.square) else {
            continue;
        };
        let there = centre_of(to.0, to.1);
        icon(app, ui, if shot.ships_killed > 0 { 2 } else { 0 }, there);
    }
}

/// One line describing what a frame did.
fn describe(event: &Event) -> String {
    match event {
        Event::Move { token, from, to } => format!(
            "token {token} moves ({}, {}) → ({}, {})",
            from.0, from.1, to.0, to.1
        ),
        Event::Fire {
            attacker,
            target,
            range,
            ships_killed,
            ..
        } => {
            if *ships_killed > 0 {
                format!(
                    "token {attacker} fires on {target} at range {range} — {ships_killed} destroyed"
                )
            } else {
                format!("token {attacker} fires on {target} at range {range}")
            }
        }
        Event::Disengage { token } => format!("token {token} leaves the battle"),
    }
}
