//! The Selection Summary pane.
//!
//! The third pane down the left of the original's frame, and the one that
//! answers "what is that?": whatever is selected in the scanner, this
//! summarises it (`DrawMineSurvey`, `1028:065a`).
//!
//! The planet case is the one with a shape of its own. Four lines of text run
//! across the top — value, population, owner, and how old the report is — and
//! whatever is left is split into **six equal rows**: gravity, temperature and
//! radiation against the race's habitable band, then ironium, boranium and
//! germanium against a shared kiloton scale. Everything below the text sits in
//! two sunken frames, and every colour in it is the game's own.
//!
//! The title is `"<name> Summary"` (`SetMineralTitleBar`, `1028:47dc`).
//!
//! See `docs/ui/mine-survey-pane.md` for what is and is not reproduced.

use crate::survey::{self as sv, Gauge};
use crate::toolbar;
use crate::{App, ScanThing, SurveyBar, SurveySubject};

fn colour([r, g, b]: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

/// Draw the survey pane.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    app.drawn_scope = "survey";
    ui.label(egui::RichText::new(app.survey_title()).small().strong());
    ui.separator();

    match app.survey_subject() {
        SurveySubject::DeepSpace => {
            ui.label(egui::RichText::new("nothing selected").weak().small());
        }
        SurveySubject::Fleet(_) => fleet(app, ui),
        SurveySubject::Thing(thing) => object(app, ui, thing),
        SurveySubject::Planet(_) => planet(app, ui),
    }
}

/// The planet case, laid out the way `DrawMineSurvey` lays it out.
fn planet(app: &mut App, ui: &mut egui::Ui) {
    let font = egui::TextStyle::Small.resolve(ui.style());
    let line = ui.text_style_height(&egui::TextStyle::Small);
    let text = ui.visuals().text_color();
    let width = |ui: &egui::Ui, s: &str| {
        ui.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), text))
            .rect
            .width()
            .ceil()
    };

    // The label column is the widest label plus six, and the pane goes narrow
    // when four of those would not fit across it.
    let pane_width = ui.available_width();
    let widest = |ui: &egui::Ui, narrow: bool| {
        let mut w: f32 = 0.0;
        for (long, short) in sv::ENV_LABELS {
            w = w.max(width(ui, if narrow { short } else { long }));
        }
        for label in sv::MINERAL_LABELS {
            let label = if narrow {
                &label[..sv::MINERAL_LABEL_NARROW]
            } else {
                label
            };
            w = w.max(width(ui, label));
        }
        w + sv::COLUMN_PAD
    };
    let mut labels = widest(ui, false);
    let narrow = sv::narrow(labels, pane_width);
    if narrow {
        labels = widest(ui, true);
    }
    // The value column on the right is sized from `999mR`, not from the
    // readings that go in it.
    let values = width(ui, sv::VALUE_SAMPLE) + sv::COLUMN_PAD;

    for (label, value) in app.survey_planet_rows(narrow) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.label(egui::RichText::new(label).small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(value).small());
            });
        });
    }

    let environment = app.survey_environment();
    let minerals = app.survey_minerals();
    if environment.is_empty() {
        ui.label(
            egui::RichText::new("this planet has not been surveyed")
                .weak()
                .small(),
        );
        return;
    }

    // Six rows in whatever is left, each the same height and forced even.
    let remaining = ui.available_height();
    let row = sv::row_height(remaining + line * 4.0 + 2.0, line).max(line + 2.0);
    let block = row * 3.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(pane_width, block * 2.0 + 6.0 + line),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    let bars_left = rect.left() + labels;
    let bars_right = rect.right() - values;

    // The two sunken frames the bars sit in: a shadow along the top and left,
    // a highlight along the bottom and right, and black behind the bars.
    let frame = |top: f32| {
        let r = egui::Rect::from_min_max(
            egui::pos2(bars_left, top),
            egui::pos2(bars_right, top + block),
        );
        let fill = |x: f32, y: f32, w: f32, h: f32, c: egui::Color32| {
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h)),
                0.0,
                c,
            );
        };
        let shadow = colour(toolbar::SHADOW);
        let hilite = colour(toolbar::HILITE);
        fill(r.left(), r.top(), r.width(), 1.0, shadow);
        fill(r.left(), r.top(), 1.0, r.height(), shadow);
        fill(
            r.left() + 1.0,
            r.top() + 1.0,
            r.width() - 2.0,
            r.height() - 1.0,
            egui::Color32::BLACK,
        );
        fill(r.right() - 1.0, r.top() + 1.0, 1.0, r.height(), hilite);
        fill(r.left(), r.bottom(), r.width(), 1.0, hilite);
        r
    };

    let env_rect = frame(rect.top());
    for (index, bar) in environment.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let top = env_rect.top() + index as f32 * row;
        habitability(
            ui, &painter, bar, index, top, row, bars_left, bars_right, &font, line,
        );
    }

    let min_top = rect.top() + block + 6.0;
    let scale = app.mineral_scale;
    let min_rect = frame(min_top);
    for (index, bar) in minerals.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let top = min_rect.top() + index as f32 * row;
        mineral(
            ui, &painter, bar, index, top, row, bars_left, bars_right, &font, line, scale, narrow,
        );
    }

    // The kiloton scale under the three bars. The original sizes the ticks
    // from the widest figure — each label plus half its own width again — and
    // then rounds the step up to a round number for the size of the scale.
    let bar_width = (bars_right - bars_left) - 3.0;
    let label_width = width(ui, &scale.to_string()).max(1.0);
    #[allow(clippy::cast_possible_truncation)]
    let fits = (bar_width / (label_width * 1.5)).floor().max(1.0) as i32;
    let step = sv::scale_step(scale, fits);
    let ticks = sv::scale_ticks(scale, step);
    let tick_y = min_rect.bottom() + 2.0;
    let text = ui.visuals().text_color();
    for tick in 0..=ticks {
        let value = tick * step;
        #[allow(clippy::cast_precision_loss)]
        let x = bars_left + bar_width * (value as f32) / (scale as f32);
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, min_rect.bottom()), egui::vec2(1.0, 2.0)),
            0.0,
            text,
        );
        painter.text(
            egui::pos2(x, tick_y),
            egui::Align2::CENTER_TOP,
            value.to_string(),
            font.clone(),
            text,
        );
    }
    // And its unit, to the left of the row of figures.
    painter.text(
        egui::pos2(bars_left - 4.0, tick_y),
        egui::Align2::RIGHT_TOP,
        sv::KT,
        font.clone(),
        text,
    );

    // `MineClick`'s `htMineScale`: the scale itself is a hit area, and
    // either button on it offers the nine the graph can be drawn against.
    let strip = egui::Rect::from_min_max(
        egui::pos2(rect.left(), min_rect.bottom()),
        egui::pos2(rect.right(), tick_y + line),
    );
    let response = ui.interact(strip, ui.id().with("mineral scale"), egui::Sense::click());
    if response.clicked() || response.secondary_clicked() {
        app.mineral_menu = response.interact_pointer_pos();
    }
    mineral_scale_menu(app, ui);
}

/// The nine scales, with a tick on the one in use.
fn mineral_scale_menu(app: &mut App, ui: &mut egui::Ui) {
    let Some(at) = app.mineral_menu else {
        return;
    };
    let (captions, checked) = app.mineral_scale_menu();
    let mut chose = None;
    let area = egui::Area::new(ui.id().with("mineral scale menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                for (index, caption) in captions.iter().enumerate() {
                    let mark = if checked == Some(index) {
                        "\u{2713} "
                    } else {
                        "    "
                    };
                    if ui.button(format!("{mark}{caption}")).clicked() {
                        chose = Some(index);
                    }
                }
            });
        });
    if let Some(index) = chose {
        app.set_mineral_scale(index);
        app.mineral_menu = None;
    } else if ui.ctx().input(|i| i.pointer.any_click()) && !area.response.hovered() {
        app.mineral_menu = None;
    }
}

/// One environment bar: the race's habitable band, the planet's diamond on it,
/// and the reach terraforming would give.
#[allow(clippy::too_many_arguments)]
fn habitability(
    ui: &egui::Ui,
    painter: &egui::Painter,
    bar: &SurveyBar,
    index: usize,
    top: f32,
    row: f32,
    left: f32,
    right: f32,
    font: &egui::FontId,
    line: f32,
) {
    let text = ui.visuals().text_color();
    let mid = top + (row - line) / 2.0;
    painter.text(
        egui::pos2(left - 2.0, mid),
        egui::Align2::RIGHT_TOP,
        &bar.label,
        font.clone(),
        text,
    );
    if !bar.value.is_empty() {
        painter.text(
            egui::pos2(right + 4.0, mid),
            egui::Align2::LEFT_TOP,
            &bar.value,
            font.clone(),
            text,
        );
    }
    let inner = (right - left) - 4.0;
    #[allow(clippy::cast_precision_loss)]
    let along = |v: i32| left + 2.0 + inner * (v.clamp(0, 100) as f32) / 100.0;

    // The band the race can live in, in the dark shade of the variable's own
    // colour; an immune race lives anywhere, which is the whole bar.
    let (low, high) = if bar.immune {
        (along(0), along(100))
    } else {
        (along(bar.low), along(bar.high))
    };
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(low, top + 2.0),
            egui::pos2(high, top + row - 1.0),
        ),
        0.0,
        colour(sv::ENV_BAND[index]),
    );

    // Where terraforming could take it: a one-pixel run from the planet's own
    // value to the reach.
    let mark = colour(sv::ENV_MARK[index]);
    if let Some(reach) = bar.terraform {
        let (a, b) = (along(bar.at), along(reach));
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(a.min(b), top + row - 4.0),
                egui::pos2(a.max(b) + 1.0, top + row - 3.0),
            ),
            0.0,
            mark,
        );
    }

    // And the planet itself, as the original's diamond: half as tall as a
    // quarter of the row, and never smaller than two pixels.
    let at = along(bar.at);
    let half = (row / 4.0 - 1.0).max(2.0);
    let centre = top + row / 2.0;
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(at, centre - half),
            egui::pos2(at + half, centre),
            egui::pos2(at, centre + half),
            egui::pos2(at - half, centre),
        ],
        mark,
        egui::Stroke::NONE,
    ));
}

/// One mineral bar: what is on the surface, what this year's mining will add,
/// and a marker at the concentration.
#[allow(clippy::too_many_arguments)]
fn mineral(
    ui: &egui::Ui,
    painter: &egui::Painter,
    bar: &SurveyBar,
    index: usize,
    top: f32,
    row: f32,
    left: f32,
    right: f32,
    font: &egui::FontId,
    line: f32,
    scale: i32,
    narrow: bool,
) {
    let mid = top + (row - line) / 2.0;
    let label = if narrow {
        &bar.label[..sv::MINERAL_LABEL_NARROW.min(bar.label.len())]
    } else {
        bar.label.as_str()
    };
    // Each mineral's label is written in its own colour, and they are not the
    // same three the bars use.
    painter.text(
        egui::pos2(left - 2.0, mid),
        egui::Align2::RIGHT_TOP,
        label,
        font.clone(),
        colour(sv::MINERAL_TEXT[index]),
    );
    painter.text(
        egui::pos2(right + 4.0, mid),
        egui::Align2::LEFT_TOP,
        &bar.value,
        font.clone(),
        ui.visuals().text_color(),
    );

    let inner = (right - left) - 4.0;
    #[allow(clippy::cast_precision_loss)]
    let along = |v: i32| inner * (v.clamp(0, scale) as f32) / (scale.max(1) as f32);
    let run = |amount: i32, fill: egui::Color32| {
        let w = along(amount);
        if w > 0.0 {
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(left + 1.0, top + 3.0),
                    egui::vec2(w, row - 5.0),
                ),
                0.0,
                fill,
            );
        }
    };
    // The sum first, in the dark shade, then the surface stock over it in the
    // bright one — so the tail that shows is this year's mining.
    run(bar.sum, colour(sv::MINERAL_SUM[index]));
    run(bar.at, colour(sv::MINERAL_BAR[index]));

    // A bar that runs past the end of the scale says so.
    if bar.sum > scale {
        painter.text(
            egui::pos2(right - 1.0, mid),
            egui::Align2::RIGHT_TOP,
            sv::OVERFLOW,
            font.clone(),
            ui.visuals().text_color(),
        );
    }

    // The concentration, as a marker along its own percentage scale rather
    // than the kiloton one.
    #[allow(clippy::cast_precision_loss)]
    let at = left + 1.0 + (inner - 11.0).max(0.0) * (bar.high.clamp(0, 100) as f32) / 100.0;
    let half = ((row - 6.0) / 2.0).max(2.0);
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(at, top + 3.0),
            egui::pos2(at, top + 3.0 + half * 2.0),
            egui::pos2(at - half, top + 3.0 + half),
        ],
        egui::Color32::WHITE,
        egui::Stroke::NONE,
    ));
}

/// The fleet case: the picture, then a column of text with two gauges in it.
fn fleet(app: &mut App, ui: &mut egui::Ui) {
    let font = egui::TextStyle::Small.resolve(ui.style());
    let line = ui.text_style_height(&egui::TextStyle::Small);
    let text = ui.visuals().text_color();
    let summary = app.survey_fleet(false);

    // The picture sits at the top left in a black square, with the owner's
    // emblem tucked under its right corner, and the text runs down a column
    // 0x56 in from the pane's edge.
    let rows = 2 + summary.orders.len() + usize::from(summary.sweeping.is_some());
    #[allow(clippy::cast_precision_loss)]
    let text_height = (rows as f32 + 2.0) * (line + sv::ROW_GAP);
    let height = text_height.max(sv::PICTURE_AT.1 + sv::EMBLEM_AT.1 + sv::EMBLEM_SIDE + 4.0);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);

    let plinth = egui::pos2(
        rect.left() + sv::PICTURE_AT.0,
        rect.top() + sv::PICTURE_AT.1,
    );
    let black = |at: egui::Pos2, size: egui::Vec2| {
        painter.rect_filled(
            egui::Rect::from_min_size(at, size),
            0.0,
            egui::Color32::BLACK,
        );
    };
    // The two black squares the original blits its bitmaps into: 0x42 for the
    // fleet and 0x22 by 0x24 for the emblem. Without a copy of the game to
    // read them out of, the squares are all there is.
    black(
        plinth + egui::vec2(2.0, 2.0),
        egui::vec2(sv::PICTURE_SIDE + 2.0, sv::PICTURE_SIDE + 2.0),
    );
    black(
        plinth + egui::vec2(sv::EMBLEM_AT.0 - 1.0, sv::EMBLEM_AT.1 - 3.0),
        egui::vec2(sv::EMBLEM_SIDE + 2.0, sv::EMBLEM_SIDE + 4.0),
    );

    let left = rect.left() + sv::TEXT_LEFT;
    let mut y = rect.top() + sv::PICTURE_AT.1;
    let put = |y: f32, s: &str| {
        painter.text(
            egui::pos2(left, y),
            egui::Align2::LEFT_TOP,
            s,
            font.clone(),
            text,
        );
    };
    put(y, &summary.ships);
    y += line + sv::ROW_GAP;

    // The two gauges, each labelled on the left and drawn from `0x5a` plus the
    // wider of the two labels to four short of the pane's right edge.
    if let (Some(fuel), Some(cargo)) = (summary.fuel.as_ref(), summary.cargo.as_ref()) {
        let width = |s: &str| {
            ui.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), text))
                .rect
                .width()
                .ceil()
        };
        let label = width(sv::FUEL_LABEL).max(width(sv::CARGO_LABEL));
        let bar_left = rect.left() + sv::GAUGE_LEFT + label;
        let bar_right = rect.right() - 4.0;
        if bar_left < bar_right {
            for (gauge, label) in [(fuel, sv::FUEL_LABEL), (cargo, sv::CARGO_LABEL)] {
                put(y, label);
                gauge_bar(
                    ui,
                    &painter,
                    gauge,
                    egui::Rect::from_min_max(
                        egui::pos2(bar_left, y),
                        egui::pos2(bar_right, y + line),
                    ),
                    &font,
                );
                y += line + sv::ROW_GAP;
            }
        }
    }

    put(y, &summary.mass);
    y += line + sv::ROW_GAP;
    for row in &summary.orders {
        put(y, row);
        y += line + sv::ROW_GAP;
    }
    if let Some(sweeping) = &summary.sweeping {
        put(y, sweeping);
    }
}

/// One gauge: a one-pixel frame, the segments stacked left to right inside it,
/// the button face for the rest, and the label centred on it when it fits.
///
/// `LDrawGauge` (`1040:31a2`) draws every gauge in the game, so the planet
/// pane's mass-driver gauge comes through here too.
pub(crate) fn gauge_bar(
    ui: &egui::Ui,
    painter: &egui::Painter,
    gauge: &Gauge,
    rect: egui::Rect,
    font: &egui::FontId,
) {
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0_f32, ui.visuals().text_color()),
    );
    let inner = rect.shrink(1.0);
    painter.rect_filled(inner, 0.0, colour(toolbar::FACE));
    if gauge.total > 0 {
        let mut running = 0_i32;
        let mut x = inner.left();
        for (amount, fill) in &gauge.segments {
            running += *amount;
            #[allow(clippy::cast_precision_loss)]
            let to = inner.left()
                + inner.width() * (running.clamp(0, gauge.total) as f32) / (gauge.total as f32);
            if to > x {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x, inner.top()),
                        egui::pos2(to, inner.bottom()),
                    ),
                    0.0,
                    colour(*fill),
                );
            }
            x = to;
        }
    }
    // The original measures the label against the bar less three and simply
    // leaves it out when it will not fit.
    let galley = ui
        .fonts(|f| f.layout_no_wrap(gauge.label.clone(), font.clone(), ui.visuals().text_color()));
    if galley.rect.width() < rect.width() - 3.0 {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &gauge.label,
            font.clone(),
            egui::Color32::BLACK,
        );
    }
}

/// A space object: the picture plinth, then a shape of its own.
///
/// A minefield and a packet run plain lines down a column `0x28` past the
/// picture; a wormhole runs a right-aligned label against a value, a line and
/// a half apart and starting `0x2f` past it; and the Mystery Trader wraps its
/// notice across the pane before its one line.
fn object(app: &mut App, ui: &mut egui::Ui, thing: ScanThing) {
    let font = egui::TextStyle::Small.resolve(ui.style());
    let line = ui.text_style_height(&egui::TextStyle::Small);
    let text = ui.visuals().text_color();
    let summary = app.survey_thing();
    let wormhole = matches!(thing, ScanThing::Wormhole(_));

    let plinth_height = if summary.emblem {
        sv::PICTURE_AT.1 + sv::EMBLEM_AT.1 + sv::EMBLEM_SIDE + 4.0
    } else {
        sv::PICTURE_AT.1 + sv::PICTURE_SIDE + 8.0
    };
    let gap = if wormhole {
        line * sv::WIDE_ROW
    } else {
        line + sv::ROW_GAP
    };
    #[allow(clippy::cast_precision_loss)]
    let rows = (summary.rows.len() + summary.table.len()) as f32;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), (rows * gap + 12.0).max(plinth_height)),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);

    // The plinth. Only an object with an owner gets the second black square
    // the emblem goes in.
    let plinth = egui::pos2(
        rect.left() + sv::PICTURE_AT.0,
        rect.top() + sv::PICTURE_AT.1,
    );
    let black = |at: egui::Pos2, size: egui::Vec2| {
        painter.rect_filled(
            egui::Rect::from_min_size(at, size),
            0.0,
            egui::Color32::BLACK,
        );
    };
    black(
        plinth + egui::vec2(1.0, 2.0),
        egui::vec2(sv::PICTURE_SIDE + 2.0, sv::PICTURE_SIDE + 2.0),
    );
    if summary.emblem {
        black(
            plinth + egui::vec2(sv::EMBLEM_AT.0 - 1.0, sv::EMBLEM_AT.1 - 3.0),
            egui::vec2(sv::EMBLEM_SIDE + 2.0, sv::EMBLEM_SIDE + 4.0),
        );
    }

    let left = rect.left()
        + if wormhole {
            sv::WORMHOLE_TEXT_LEFT
        } else {
            sv::THING_TEXT_LEFT
        };
    let mut y = rect.top() + sv::PICTURE_AT.1;

    // The Mystery Trader's notice is word-wrapped across what is left of the
    // pane, and everything else starts below it.
    if let Some(notice) = &summary.notice {
        let galley = ui.fonts(|f| {
            f.layout(
                notice.clone(),
                font.clone(),
                text,
                (rect.right() - left - 4.0).max(16.0),
            )
        });
        let height = galley.rect.height();
        painter.galley(egui::pos2(left, y), galley, text);
        y += height + 8.0;
    }

    for row in &summary.rows {
        painter.text(
            egui::pos2(left, y),
            egui::Align2::LEFT_TOP,
            row,
            font.clone(),
            text,
        );
        y += gap;
    }

    // The two-column part: the labels right-aligned in a column of their own,
    // the values from the same x.
    if !summary.table.is_empty() {
        let widest = summary
            .table
            .iter()
            .map(|(label, _)| {
                ui.fonts(|f| f.layout_no_wrap(label.clone(), font.clone(), text))
                    .rect
                    .width()
                    .ceil()
            })
            .fold(0.0_f32, f32::max);
        let column = left
            + widest
            + if wormhole {
                sv::WORMHOLE_LABEL_GAP
            } else {
                0.0
            };
        for (label, value) in &summary.table {
            painter.text(
                egui::pos2(column - 2.0, y),
                egui::Align2::RIGHT_TOP,
                label,
                font.clone(),
                text,
            );
            painter.text(
                egui::pos2(column, y),
                egui::Align2::LEFT_TOP,
                value,
                font.clone(),
                text,
            );
            y += gap;
        }
    }
}
