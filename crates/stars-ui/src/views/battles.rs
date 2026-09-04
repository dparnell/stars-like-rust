//! The battle VCR.
//!
//! The board is drawn from [`crate::vcr::Vcr`], which **plays the recording**
//! rather than re-simulating it — see that module for why. This view adds only
//! the transport controls and the drawing.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::vcr::{Event, BOARD};
use crate::views::player_colour;
use crate::App;

/// Draw the battle screen.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    if app.battles.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label("This file holds no battle recordings.");
            ui.label(
                egui::RichText::new(
                    "Recordings live in a player's own file for the year the battle \
                     happened; a host file carries none.",
                )
                .weak(),
            );
        });
        return;
    }

    let labels: Vec<String> = app
        .battles
        .iter()
        .map(|b| {
            format!(
                "{:#06x} — {} tokens, {} ships lost",
                b.id,
                b.tokens.len(),
                b.ships_destroyed()
            )
        })
        .collect();

    egui::SidePanel::left("battle_list")
        .resizable(true)
        .default_width(240.0)
        .show_inside(ui, |ui| {
            ui.heading(format!("{} battles", labels.len()));
            ui.separator();
            let current = app.vcr.as_ref().map(|v| v.id);
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (index, label) in labels.iter().enumerate() {
                    let selected = current == Some(app.battles[index].id);
                    if ui.selectable_label(selected, label).clicked() {
                        app.open_battle(index);
                    }
                }
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        let Some(vcr) = app.vcr.as_mut() else {
            ui.label("Choose a battle.");
            return;
        };

        ui.horizontal(|ui| {
            if ui.button("⏮").on_hover_text("rewind").clicked() {
                vcr.rewind();
                app.playing = false;
            }
            if ui.button("◀").on_hover_text("step back").clicked() {
                vcr.back();
                app.playing = false;
            }
            let play = if app.playing { "⏸" } else { "▶" };
            if ui.button(play).on_hover_text("play").clicked() {
                app.playing = !app.playing;
            }
            if ui.button("▶|").on_hover_text("step").clicked() {
                vcr.step();
                app.playing = false;
            }
            if ui.button("⏭").on_hover_text("to the end").clicked() {
                vcr.end();
                app.playing = false;
            }
            ui.separator();
            let mut position = vcr.position();
            let last = vcr.len();
            if ui
                .add(egui::Slider::new(&mut position, 0..=last).text("frame"))
                .changed()
            {
                vcr.seek(position);
                app.playing = false;
            }
        });

        // Advance while playing. egui redraws continuously because the shell
        // requests a repaint whenever `playing` is set.
        if app.playing && !vcr.step() {
            app.playing = false;
        }

        ui.label(format!(
            "battle {:#06x} · round {} · frame {} of {}",
            vcr.id,
            vcr.round(),
            vcr.position(),
            vcr.len()
        ));
        if let Some(frame) = vcr.frame() {
            ui.label(describe(&frame.event));
        } else {
            ui.label("before the first move");
        }
        ui.separator();

        // The board.
        let side = ui
            .available_height()
            .min(ui.available_width() - 220.0)
            .max(120.0);
        ui.horizontal(|ui| {
            let (response, painter) = ui.allocate_painter(Vec2::splat(side), Sense::hover());
            let rect = response.rect;
            painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 10, 18));
            let cell = rect.width() / f32::from(BOARD);

            for i in 0..=u32::from(BOARD) {
                let offset = cell * i as f32;
                let grey = Stroke::new(1.0_f32, Color32::from_gray(40));
                painter.line_segment(
                    [
                        Pos2::new(rect.left() + offset, rect.top()),
                        Pos2::new(rect.left() + offset, rect.bottom()),
                    ],
                    grey,
                );
                painter.line_segment(
                    [
                        Pos2::new(rect.left(), rect.top() + offset),
                        Pos2::new(rect.right(), rect.top() + offset),
                    ],
                    grey,
                );
            }

            let tokens = vcr.tokens();
            for (y, row) in vcr.board().iter().enumerate() {
                for (x, here) in row.iter().enumerate() {
                    if here.is_empty() {
                        continue;
                    }
                    let centre = Pos2::new(
                        rect.left() + (x as f32 + 0.5) * cell,
                        rect.top() + (y as f32 + 0.5) * cell,
                    );
                    for (n, index) in here.iter().enumerate() {
                        let token = &tokens[*index];
                        let nudge = Vec2::new(
                            (n as f32 - (here.len() as f32 - 1.0) / 2.0) * cell * 0.22,
                            0.0,
                        );
                        let colour = player_colour(i16::from(token.player));
                        let spot = Rect::from_center_size(centre + nudge, Vec2::splat(cell * 0.34));
                        painter.rect_filled(spot, 2.0, colour);
                        // An unarmed token is drawn hollow: the recording says
                        // whether a token has weapons, and nothing else does.
                        if !token.armed {
                            painter.rect_filled(
                                spot.shrink(2.0),
                                1.0,
                                Color32::from_rgb(8, 10, 18),
                            );
                        }
                        painter.text(
                            centre + nudge,
                            egui::Align2::CENTER_CENTER,
                            token.index.to_string(),
                            egui::FontId::proportional(cell * 0.3),
                            Color32::WHITE,
                        );
                    }
                }
            }

            // The roster beside it.
            ui.vertical(|ui| {
                ui.heading("tokens");
                for token in vcr.tokens() {
                    let colour = player_colour(i16::from(token.player));
                    let state = if !token.active || token.ships == 0 {
                        "destroyed".to_string()
                    } else {
                        format!("{} ships", token.ships)
                    };
                    ui.colored_label(
                        colour,
                        format!(
                            "{}: player {} — {state}{}",
                            token.index,
                            token.player,
                            if token.armed { "" } else { ", unarmed" }
                        ),
                    );
                }
                ui.separator();
                ui.heading("losses");
                for (player, lost) in vcr.losses() {
                    ui.colored_label(
                        player_colour(i16::from(player)),
                        format!("player {player}: {lost} ships"),
                    );
                }
            });
        });
    });
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
