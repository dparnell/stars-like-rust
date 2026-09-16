//! The Score sheet — Reports (Score), or **F10**.
//!
//! `ScoreXDlg` (`1108:0f66`): one modeless window with three faces and a
//! single button that cycles them — the scoreboard (`DrawScoreReport`,
//! `1108:1e0c`), the victory conditions (`DrawVCReport`, `1108:168e`) and the
//! timeline (`DrawHistoryReport`, `1108:2494`).
//!
//! See `docs/ui/score-sheet.md`.

use crate::App;
use stars_core::score::Standing;
use stars_core::scoresheet::{self, Face, Stat};

/// The original highlights the leading figure, the first rank and the winner
/// in pure blue on a light grey dialog. Lightened here to stay legible on a
/// dark ground; nothing else about it changes.
const HIGHLIGHT: egui::Color32 = egui::Color32::from_rgb(0x8a, 0xb4, 0xff);

/// A player the game knows to be dead: grey, like the original's `0x7f7f7f`.
const DEAD: egui::Color32 = egui::Color32::from_rgb(0x9a, 0x9a, 0x9a);

/// Draw the sheet's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some(sheet) = app.score_sheet else {
        return;
    };
    ui.label(egui::RichText::new(sheet.face.title()).strong());
    ui.separator();

    match sheet.face {
        Face::Scores => scores(app, ui),
        Face::Victory => victory(app, ui),
        Face::Timeline => timeline(app, ui, sheet.graph),
    }

    ui.separator();
    ui.horizontal(|ui| {
        // One button, and it cycles: the original's `(face + 1) % 3`.
        if ui.button(sheet.face.next().title()).clicked() {
            app.score_next_face();
        }
        if ui.button("Close").clicked() {
            app.close_score_sheet();
        }
        // `ScoreXDlg`'s Help: `0x455` (`1108:12c1`), the Score sheet page.
        if ui.button("Help").clicked() {
            app.help_context(crate::help::context::SCORE);
        }
    });
}

/// The name to head a player's column with.
fn player_name(app: &App, player: usize) -> String {
    app.game
        .as_ref()
        .and_then(|game| game.players.get(player))
        .map_or_else(
            || format!("Player {}", player + 1),
            |p| {
                if p.name.is_empty() {
                    format!("Player {}", player + 1)
                } else {
                    p.name.clone()
                }
            },
        )
}

/// Whether the game knows this player to be dead.
fn is_dead(app: &App, player: usize) -> bool {
    app.game
        .as_ref()
        .and_then(|game| game.players.get(player))
        .is_some_and(|p| p.dead)
}

/// The colour a player's name is drawn in: grey when dead, blue when they have
/// won, ordinary otherwise.
fn name_colour(app: &App, standing: &Standing, ui: &egui::Ui) -> egui::Color32 {
    if is_dead(app, standing.player) {
        DEAD
    } else if standing.winner {
        HIGHLIGHT
    } else {
        ui.visuals().text_color()
    }
}

/// Reserve the band the rotated names live in and draw them into it.
fn header_band(
    ui: &mut egui::Ui,
    layout: &crate::score::Layout,
    names: &[(String, egui::Color32)],
) {
    // The band is as tall as the longest name is wide, which has to be known
    // before the space is allocated — so it is measured first.
    let font = egui::TextStyle::Small.resolve(ui.style());
    let tall = names
        .iter()
        .map(|(name, _)| {
            ui.fonts(|f| f.layout_no_wrap(name.clone(), font.clone(), egui::Color32::WHITE))
                .rect
                .width()
        })
        .fold(0.0_f32, f32::max)
        + 4.0;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), tall), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    rotated_headers(ui, &painter, rect.min, layout, names);
}

/// One player's name as a **rotated** column header.
///
/// The original draws these with a 90-degree Arial (`rghfontArial8[4]`),
/// because a column is only five digits wide on the scoreboard and a line and
/// a half on the victory report — nowhere near enough for a name across.
/// Returns how tall the band of headers has to be.
fn rotated_headers(
    ui: &egui::Ui,
    painter: &egui::Painter,
    at: egui::Pos2,
    layout: &crate::score::Layout,
    names: &[(String, egui::Color32)],
) -> f32 {
    let font = egui::TextStyle::Small.resolve(ui.style());
    let galleys: Vec<_> = names
        .iter()
        .map(|(name, colour)| {
            (
                ui.fonts(|f| f.layout_no_wrap(name.clone(), font.clone(), *colour)),
                *colour,
            )
        })
        .collect();
    // The band is as tall as the longest name is wide.
    let tall = galleys
        .iter()
        .map(|(galley, _)| galley.rect.width())
        .fold(0.0_f32, f32::max)
        + 4.0;
    for (index, (galley, colour)) in galleys.iter().enumerate() {
        // Turned a quarter turn anticlockwise, so the names read upwards from
        // the row of figures.
        let x = at.x + layout.column_at(index) + (layout.column - galley.rect.height()) / 2.0;
        let shape =
            egui::epaint::TextShape::new(egui::pos2(x, at.y + tall), galley.clone(), *colour)
                .with_angle(-std::f32::consts::FRAC_PI_2);
        painter.add(shape);
    }
    tall
}

/// What the sheet measures its columns against: one digit and the widest
/// label, both in the bold face the original measures them in.
fn measure(ui: &egui::Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Small.resolve(ui.style());
    ui.fonts(|f| f.layout_no_wrap(text.to_string(), font, ui.visuals().text_color()))
        .rect
        .width()
        .ceil()
}

/// The scoreboard: a column per player, a row per figure, and Rank last.
fn scores(app: &mut App, ui: &mut egui::Ui) {
    let standings = app.score_standings();
    if standings.is_empty() {
        ui.label(egui::RichText::new("no scores in this file").weak().small());
        return;
    }
    // A player known to be dead keeps their name and loses their figures.
    let shows: Vec<bool> = standings
        .iter()
        .map(|standing| standing.known && !is_dead(app, standing.player))
        .collect();

    // The window sizes itself: a digit and the widest label, and a column per
    // player five digits wide (`InitScoreDlg`, `1108:13b6`).
    let line = ui.text_style_height(&egui::TextStyle::Small);
    let layout = crate::score::Layout::scores(
        measure(ui, crate::score::DIGIT_SAMPLE),
        line,
        measure(ui, crate::score::WIDEST_LABEL),
        standings.len(),
    );
    let names: Vec<(String, egui::Color32)> = standings
        .iter()
        .map(|standing| {
            (
                player_name(app, standing.player),
                name_colour(app, standing, ui),
            )
        })
        .collect();
    header_band(ui, &layout, &names);

    egui::Grid::new("score-report")
        .num_columns(standings.len() + 1)
        .min_col_width(layout.column)
        .spacing([2.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for stat in Stat::ALL {
                ui.label(egui::RichText::new(stat.label()).small());
                let best = scoresheet::best(&standings, stat);
                for (standing, shown) in standings.iter().zip(&shows) {
                    if !shown {
                        ui.label("");
                        continue;
                    }
                    let value = stat.of(standing);
                    let text = egui::RichText::new(comma(value)).small();
                    ui.label(if Some(value) == best {
                        text.color(HIGHLIGHT)
                    } else {
                        text
                    });
                }
                ui.end_row();
            }

            // Rank is not one of the eight: it has its own row, and the
            // highlight goes to first place rather than to the largest number.
            ui.label(egui::RichText::new("Rank:").small());
            for (standing, shown) in standings.iter().zip(&shows) {
                if !shown {
                    ui.label("");
                    continue;
                }
                let text = egui::RichText::new(standing.rank.to_string()).small();
                ui.label(if standing.rank == 1 {
                    text.color(HIGHLIGHT)
                } else {
                    text
                });
            }
            ui.end_row();
        });
}

/// The victory report: what the game is playing for, and who has done it.
fn victory(app: &mut App, ui: &mut egui::Ui) {
    let conditions = app.score_conditions();
    let standings = app.score_standings();

    // The victory report's columns are only a line and a half wide, which is
    // what makes the rotated names necessary.
    let line = ui.text_style_height(&egui::TextStyle::Small);
    let layout = crate::score::Layout::victory(
        measure(ui, crate::score::DIGIT_SAMPLE),
        line,
        measure(ui, crate::score::WIDEST_SENTENCE),
        standings.len(),
    );
    let names: Vec<(String, egui::Color32)> = standings
        .iter()
        .map(|standing| {
            (
                player_name(app, standing.player),
                name_colour(app, standing, ui),
            )
        })
        .collect();
    header_band(ui, &layout, &names);

    egui::Grid::new("score-victory")
        .num_columns(standings.len() + 1)
        .min_col_width(layout.column)
        .spacing([2.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for condition in &conditions {
                // A condition the game is not playing for is still listed,
                // greyed, with its setting.
                let text = egui::RichText::new(&condition.text).small();
                ui.label(if condition.active || !condition.scoreable {
                    text
                } else {
                    text.color(DEAD)
                });
                for standing in &standings {
                    if !condition.scoreable || !standing.known {
                        ui.label("");
                        continue;
                    }
                    let ticked = scoresheet::met(standing, condition);
                    let colour = if is_dead(app, standing.player) || !condition.active {
                        DEAD
                    } else if standing.winner {
                        HIGHLIGHT
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.label(
                        egui::RichText::new(if ticked { "✔" } else { "" })
                            .small()
                            .color(colour),
                    );
                }
                ui.end_row();
            }
        });
}

/// The timeline: one figure, drawn year by year, a line per player.
fn timeline(app: &mut App, ui: &mut egui::Ui, graph: Stat) {
    // The original puts this menu on the graph's title, which takes a hand
    // cursor to say so.
    let mut chosen = Stat::ALL.iter().position(|s| *s == graph).unwrap_or(0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("History of").small());
        egui::ComboBox::from_id_source("score-graph")
            .width(130.0)
            .show_index(ui, &mut chosen, Stat::ALL.len(), |i| {
                Stat::ALL[i].name().to_string()
            });
    });
    if Stat::ALL[chosen] != graph {
        app.score_set_graph(Stat::ALL[chosen]);
    }

    let Some(game) = app.game.as_ref() else {
        return;
    };
    let (first, span) = scoresheet::span(game.turn);
    let (top, step) = scoresheet::value_scale(scoresheet::peak(game, graph).unwrap_or(0));
    let me = app.local_player();

    let width = ui.available_width().max(260.0);
    let (response, painter) = ui.allocate_painter(egui::vec2(width, 220.0), egui::Sense::hover());
    let rect = response.rect;
    let plot = egui::Rect::from_min_max(
        rect.min + egui::vec2(46.0, 6.0),
        rect.max - egui::vec2(6.0, 18.0),
    );
    let visuals = ui.visuals();
    painter.rect_filled(plot, 0.0, visuals.extreme_bg_color);

    let grid = visuals.weak_text_color().gamma_multiply(0.4);
    let label = visuals.weak_text_color();
    let font = egui::FontId::proportional(9.0);

    // The years along the bottom, five or ten apart.
    let year_step = scoresheet::year_step(span);
    if span > 0 {
        let mut turn = 0;
        while turn <= span {
            let x = plot.left() + plot.width() * f32::from(turn) / f32::from(span);
            painter.line_segment(
                [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
                egui::Stroke::new(1.0_f32, grid),
            );
            painter.text(
                egui::pos2(x, plot.bottom() + 2.0),
                egui::Align2::CENTER_TOP,
                (2400 + first + turn).to_string(),
                font.clone(),
                label,
            );
            turn += year_step;
        }
    }

    // The values up the side, at the interval the ladder chose.
    if step > 0 {
        let mut value = step;
        while value < top {
            let y = plot.bottom() - plot.height() * value as f32 / top as f32;
            painter.line_segment(
                [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
                egui::Stroke::new(1.0_f32, grid),
            );
            painter.text(
                egui::pos2(plot.left() - 4.0, y),
                egui::Align2::RIGHT_CENTER,
                comma(value),
                font.clone(),
                label,
            );
            value += step;
        }
    }

    // A line per player, in that player's colour — except the player looking
    // at it, who is white, as their own things are everywhere else.
    for player in 0..game.players.len() {
        let points = scoresheet::line(game, player, graph);
        if points.is_empty() {
            continue;
        }
        let colour = if player == me {
            egui::Color32::WHITE
        } else {
            crate::views::player_colour(i16::try_from(player).unwrap_or(0))
        };
        let at = |(turn, value): (u16, i32)| {
            let x = if span == 0 {
                plot.left()
            } else {
                plot.left() + plot.width() * f32::from(turn - first) / f32::from(span)
            };
            let y = if top == 0 {
                plot.bottom()
            } else {
                plot.bottom() - plot.height() * value as f32 / top as f32
            };
            egui::pos2(x, y)
        };
        if points.len() == 1 {
            // A single year is a dot: the original calls `SetPixel`.
            painter.circle_filled(at(points[0]), 1.5, colour);
        } else {
            painter.add(egui::Shape::line(
                points.iter().map(|p| at(*p)).collect(),
                egui::Stroke::new(1.0_f32, colour),
            ));
        }
    }

    // The legend the original writes inside the top-left of the graph.
    ui.horizontal_wrapped(|ui| {
        for player in 0..game.players.len() {
            if scoresheet::line(game, player, graph).is_empty() {
                continue;
            }
            let colour = if player == me {
                egui::Color32::WHITE
            } else {
                crate::views::player_colour(i16::try_from(player).unwrap_or(0))
            };
            ui.label(
                egui::RichText::new(player_name(app, player))
                    .small()
                    .color(colour),
            );
        }
    });
}

/// A figure with thousands separators, as the sheet writes them.
fn comma(value: i32) -> String {
    let digits = value.abs().to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if value < 0 {
        format!("-{out}")
    } else {
        out
    }
}
