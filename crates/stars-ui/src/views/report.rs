//! The grid the four report windows draw.
//!
//! `DrawReport` (`1108:0bae`) lays it out: a row of raised header cells two
//! pixels in from the top left, then one row per object, each `dyArial8 + 4`
//! tall with a one-pixel shadow line down its left and along its top.
//! `DrawReportItem` (`1108:3398`) fills a cell, and `ReportDlg` (`1108:0018`)
//! turns a click in the header into a menu and a click in a row into a
//! selection.
//!
//! Widths come from `DxReportColHdr` (`1108:305e`), which measures the header
//! in the dialog font and then widens it by a rule of its own per column —
//! nothing here is a stored constant. See `docs/ui/reports.md`.

use egui::{Align2, Color32, Pos2, Rect, Sense, Vec2};

use crate::report::{Bar, Cell, ColumnMenu, Data, Entry, Report, Tint};
use crate::views::mineral_colour;
use crate::App;

/// `hbrButtonFace`, `hbrButtonHilite`, `hbrButtonShadow`.
fn face() -> Color32 {
    Color32::from_rgb(
        crate::toolbar::FACE[0],
        crate::toolbar::FACE[1],
        crate::toolbar::FACE[2],
    )
}

fn hilite() -> Color32 {
    Color32::from_rgb(
        crate::toolbar::HILITE[0],
        crate::toolbar::HILITE[1],
        crate::toolbar::HILITE[2],
    )
}

fn shadow() -> Color32 {
    Color32::from_rgb(
        crate::toolbar::SHADOW[0],
        crate::toolbar::SHADOW[1],
        crate::toolbar::SHADOW[2],
    )
}

/// The colours `SETTEXTCOLOR` is given, as `COLORREF`s of `0x00bbggrr`.
fn tint_colour(tint: Tint) -> Color32 {
    match tint {
        Tint::Plain => Color32::BLACK,
        Tint::Over => Color32::from_rgb(0xff, 0, 0),
        Tint::AtMax => Color32::from_rgb(0, 0x7f, 0),
        Tint::Poor => Color32::from_rgb(0x7f, 0x7f, 0),
    }
}

/// `0x00007f`, which the name column uses for the row the scanner has
/// selected.
const SELECTED: Color32 = Color32::from_rgb(0x7f, 0, 0);

/// A starbase bar's colour: `hbrYellow` or `hbrBlue` for the base itself,
/// `hbrPurple` for a mass driver, `hbrGreen` for a stargate.
fn bar_colour(bar: Bar) -> Color32 {
    match bar {
        Bar::Base { cargo: true } => Color32::from_rgb(0xff, 0xff, 0),
        Bar::Base { cargo: false } => Color32::from_rgb(0, 0, 0xff),
        Bar::Driver => Color32::from_rgb(0x80, 0, 0x80),
        Bar::Gate => Color32::from_rgb(0, 0x80, 0),
    }
}

/// `DxReportColHdr`: how wide a column is drawn, in pixels.
///
/// Every rule is the original's, written against two measurements — the
/// header's own width in the dialog font, and the width of a digit, which the
/// routine takes by measuring `"8"`.
fn column_width(
    report: Report,
    column: usize,
    text: f32,
    digit: f32,
    of: &dyn Fn(&str) -> f32,
) -> f32 {
    let least = |floor: f32| text.max(floor);
    let width = match report {
        Report::Planets => match column {
            0 => text * 2.0,
            1 | 13 | 14 => least(digit * 15.0),
            3 => least(digit * 4.0 + 2.0),
            4 => least(of("100% (100%)")),
            5 => text * 3.0 + 20.0,
            6 | 7 => least(digit * 5.0),
            9 => least(digit * 14.0),
            10 | 11 => least(digit * 11.0),
            12 => least(of("1000 /1000")),
            _ => text,
        },
        Report::Fleets => match column {
            0 | 1 | 3 | 8 | 10 | 11 => text * 2.0,
            2 => text * 2.5,
            4 => text * 2.0 - digit,
            5 => text * 4.5,
            6 => least(digit * 5.0),
            7 => least(digit * 19.0),
            _ => text,
        },
        Report::EnemyFleets => match column {
            0 => text * 3.0,
            1 | 5 => text * 2.0,
            2 => text * 2.5,
            _ => text,
        },
        Report::Battles => match column {
            0 => text * 2.5,
            _ => text,
        },
    };
    // `(dx + 6) / 2 << 1`: six pixels of margin, rounded down to even.
    (((width + 6.0) / 2.0).floor() * 2.0).max(8.0)
}

/// Draw one report.
pub fn view(app: &mut App, ui: &mut egui::Ui, report: Report) {
    if app.game.is_none() {
        ui.label("No game open.");
        return;
    }
    app.reports.open = Some(report);

    let font = egui::TextStyle::Body.resolve(ui.style());
    let line = ui.text_style_height(&egui::TextStyle::Body);
    let row_height = line + 4.0;
    let of = |text: &str| {
        ui.fonts(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::BLACK))
            .rect
            .width()
            .ceil()
    };
    let digit = of("8");

    let columns = report.columns();
    let drawn = app.reports.state(report).drawn(report);
    let widths: Vec<f32> = drawn
        .iter()
        .map(|&c| column_width(report, c, of(columns[c].name), digit, &of))
        .collect();

    // The rows, in the order the sort leaves them.
    let battles = app.battles.clone();
    let player = app.local_player();
    let mut rows;
    let cells: Vec<Vec<(Cell, Tint)>>;
    let ids: Vec<RowId>;
    {
        let game = app.game.as_ref().expect("a game");
        let data = Data {
            game,
            player,
            battles: &battles,
        };
        rows = data.rows(report);
        data.sort(report, &app.reports, &mut rows);
        cells = rows
            .iter()
            .map(|&row| {
                drawn
                    .iter()
                    .map(|&c| {
                        let painted = data.cell(report, row, c);
                        (painted.cell, painted.tint)
                    })
                    .collect()
            })
            .collect();
        ids = rows.iter().map(|&row| row_id(&data, report, row)).collect();
    }

    ui.label(
        egui::RichText::new(report.title(rows.len()))
            .strong()
            .size(line),
    );
    ui.add_space(2.0);

    let total: f32 = widths.iter().sum::<f32>() + 4.0;
    let mut clicked_header: Option<(usize, Pos2)> = None;
    let mut clicked_row: Option<usize> = None;

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let height = row_height * (rows.len() + 1) as f32 + 4.0;
            let (response, painter) = ui.allocate_painter(
                Vec2::new(total.max(ui.available_width()), height),
                Sense::click(),
            );
            let origin = response.rect.min;
            painter.rect_filled(response.rect, 0.0, face());

            // The header row.
            let mut x = origin.x + 2.0;
            let top = origin.y + 2.0;
            for (index, &c) in drawn.iter().enumerate() {
                let w = widths[index];
                let cell = Rect::from_min_size(Pos2::new(x, top), Vec2::new(w - 1.0, row_height));
                raised(&painter, cell);
                if c == 0 {
                    painter.text(
                        Pos2::new(cell.left() + 3.0, cell.top() + 2.0),
                        Align2::LEFT_TOP,
                        columns[c].name,
                        font.clone(),
                        Color32::BLACK,
                    );
                } else {
                    painter.text(
                        Pos2::new(cell.center().x, cell.top() + 2.0),
                        Align2::CENTER_TOP,
                        columns[c].name,
                        font.clone(),
                        Color32::BLACK,
                    );
                }
                x += w;
            }

            // The rows.
            for (row, cells) in cells.iter().enumerate() {
                let y = top + row_height * (row + 1) as f32;
                let mut x = origin.x + 2.0;
                for (index, (cell, tint)) in cells.iter().enumerate() {
                    let w = widths[index];
                    let frame = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, row_height));
                    // `PATBLT` with the shadow brush: a line down the left and
                    // one along the top of every cell.
                    painter.rect_filled(
                        Rect::from_min_size(frame.min, Vec2::new(1.0, row_height)),
                        0.0,
                        shadow(),
                    );
                    painter.rect_filled(
                        Rect::from_min_size(frame.min, Vec2::new(w, 1.0)),
                        0.0,
                        shadow(),
                    );
                    let inner = Rect::from_min_max(
                        Pos2::new(frame.left() + 2.0, frame.top() + 2.0),
                        Pos2::new(frame.right() - 3.0, frame.bottom() - 1.0),
                    );
                    let selected = ids.get(row).is_some_and(|id| id.is_selected(app));
                    paint_cell(&painter, &font, inner, cell, *tint, selected);
                    x += w;
                }
            }

            // Where a click landed: the header row, or a row of the grid.
            if response.clicked() || response.secondary_clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let mut x = origin.x + 2.0;
                    let mut hit = None;
                    for (index, &c) in drawn.iter().enumerate() {
                        x += widths[index];
                        if pos.x < x {
                            hit = Some(c);
                            break;
                        }
                    }
                    if let Some(column) = hit {
                        let row = ((pos.y - top) / row_height).floor() as i64 - 1;
                        if row < 0 {
                            clicked_header = Some((column, pos));
                        } else if let Ok(row) = usize::try_from(row) {
                            if row < rows.len() {
                                clicked_row = Some(row);
                            }
                        }
                    }
                }
            }
        });

    if let Some((column, pos)) = clicked_header {
        app.report_menu = Some((report, column, pos));
    }
    if let Some(row) = clicked_row {
        if let Some(id) = ids.get(row) {
            id.select(app);
        }
    }

    menu(app, ui, report);
}

/// `_Draw3dFrame` with `fErase = 0`: one raised ring, no fill.
fn raised(painter: &egui::Painter, rect: Rect) {
    let (w, h) = (rect.width(), rect.height());
    let fill = |x: f32, y: f32, w: f32, h: f32, colour: Color32| {
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h)),
            0.0,
            colour,
        );
    };
    fill(rect.left(), rect.top(), w, 1.0, hilite());
    fill(rect.left(), rect.top(), 1.0, h, hilite());
    fill(rect.left(), rect.bottom(), w + 1.0, 1.0, shadow());
    fill(rect.right(), rect.top(), 1.0, h, shadow());
}

fn paint_cell(
    painter: &egui::Painter,
    font: &egui::FontId,
    rect: Rect,
    cell: &Cell,
    tint: Tint,
    selected: bool,
) {
    let colour = if selected {
        SELECTED
    } else {
        tint_colour(tint)
    };
    match cell {
        Cell::Empty => {}
        Cell::Left(text) => {
            painter.text(
                rect.left_top(),
                Align2::LEFT_TOP,
                text,
                font.clone(),
                colour,
            );
        }
        Cell::Right(text) => {
            painter.text(
                rect.right_top(),
                Align2::RIGHT_TOP,
                text,
                font.clone(),
                colour,
            );
        }
        Cell::Centre(text) => {
            painter.text(
                Pos2::new(rect.center().x, rect.top()),
                Align2::CENTER_TOP,
                text,
                font.clone(),
                colour,
            );
        }
        Cell::Name(text, bars) => {
            painter.text(
                rect.left_top(),
                Align2::LEFT_TOP,
                text,
                font.clone(),
                colour,
            );
            // Three stacked bars down the right of the cell, each a third of
            // its height and as wide as it is tall.
            let third = (rect.height() / 3.0).floor().max(1.0) - 1.0;
            for (index, bar) in bars.iter().enumerate() {
                let y = rect.top() + (third + 1.0) * index as f32;
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(rect.right() - third, y),
                        Vec2::new(third, third),
                    ),
                    0.0,
                    bar_colour(*bar),
                );
            }
        }
        Cell::Pair(first, second) => {
            painter.text(
                Pos2::new(rect.center().x, rect.top()),
                Align2::RIGHT_TOP,
                first,
                font.clone(),
                colour,
            );
            painter.text(
                rect.right_top(),
                Align2::RIGHT_TOP,
                second,
                font.clone(),
                Color32::BLACK,
            );
        }
        Cell::Minerals(values) => {
            let step = rect.width() / values.len() as f32;
            for (index, value) in values.iter().enumerate() {
                painter.text(
                    Pos2::new(rect.left() + step * (index + 1) as f32 - 2.0, rect.top()),
                    Align2::RIGHT_TOP,
                    crate::report::commas(*value),
                    font.clone(),
                    mineral_colour(index),
                );
            }
        }
    }
}

/// What a row is about, so that clicking it can select the thing.
#[derive(Debug, Clone, Copy)]
enum RowId {
    Planet(i16),
    Fleet(usize),
    Battle(usize),
}

fn row_id(data: &Data<'_>, report: Report, row: usize) -> RowId {
    match report {
        Report::Planets => RowId::Planet(data.game.planets.get(row).map_or(-1, |p| p.id)),
        Report::Fleets | Report::EnemyFleets => RowId::Fleet(row),
        Report::Battles => RowId::Battle(row),
    }
}

impl RowId {
    fn is_selected(self, app: &App) -> bool {
        match self {
            RowId::Planet(id) => app.selection.planet == Some(id),
            RowId::Fleet(index) => app.selection.fleet == Some(index),
            RowId::Battle(index) => app
                .vcr
                .as_ref()
                .is_some_and(|v| app.battles.get(index).is_some_and(|b| b.id == v.id)),
        }
    }

    /// `ExecuteReportClick` (`1108:7cd6`) starts by selecting what the row is
    /// about, whichever column was clicked.
    fn select(self, app: &mut App) {
        match self {
            RowId::Planet(id) => app.selection.planet = Some(id),
            RowId::Fleet(index) => app.selection.fleet = Some(index),
            RowId::Battle(index) => app.open_battle(index),
        }
    }
}

/// The menu a header click opens, and what picking an item does.
fn menu(app: &mut App, ui: &mut egui::Ui, report: Report) {
    let Some((menu_report, column, pos)) = app.report_menu else {
        return;
    };
    if menu_report != report {
        return;
    }
    let built = ColumnMenu::build(report, column, app.reports.state(report));
    let mut chose: Option<usize> = None;
    let mut dismiss = false;

    let area = egui::Area::new(egui::Id::new(("report menu", report.irpt())))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(160.0);
                let mut index = 0;
                while index < built.entries.len() {
                    match &built.entries[index] {
                        Entry::Separator => {
                            ui.separator();
                            index += 1;
                        }
                        Entry::Item(text) => {
                            if ui.button(text).clicked() {
                                chose = Some(index);
                            }
                            index += 1;
                        }
                        // A null opens a submenu: the next entry titles it and
                        // the ones after it fill it, up to the next null.
                        Entry::Submenu => {
                            let title = match built.entries.get(index + 1) {
                                Some(Entry::Item(text)) => text.clone(),
                                _ => String::new(),
                            };
                            let mut inner = index + 2;
                            ui.menu_button(title, |ui| {
                                while inner < built.entries.len() {
                                    match &built.entries[inner] {
                                        Entry::Submenu => break,
                                        Entry::Separator => {
                                            ui.separator();
                                        }
                                        Entry::Item(text) => {
                                            if ui.button(text).clicked() {
                                                chose = Some(inner);
                                                ui.close_menu();
                                            }
                                        }
                                    }
                                    inner += 1;
                                }
                            });
                            while inner < built.entries.len()
                                && built.entries[inner] != Entry::Submenu
                            {
                                inner += 1;
                            }
                            index = inner + 1;
                        }
                    }
                }
            });
        });

    if let Some(index) = chose {
        app.reports.choose(report, column, index);
        dismiss = true;
    } else if ui.ctx().input(|i| i.pointer.any_click()) && !area.response.hovered() {
        dismiss = true;
    }
    if dismiss {
        app.report_menu = None;
    }
}
