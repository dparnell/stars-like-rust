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

use crate::report::{Bar, Cell, Click, ColumnMenu, Data, Entry, PopupKind, Report, Tint};
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

/// The caption the window carries, which counts what it is showing.
#[must_use]
pub fn window_title(app: &App, report: Report) -> String {
    let battles = app.battles.len();
    let player = app.local_player();
    let rows = app.game.as_ref().map_or(0, |game| {
        Data {
            game,
            player,
            battles: &app.battles[..battles],
        }
        .rows(report)
        .len()
    });
    report.title(rows)
}

/// Draw one report.
pub fn view(app: &mut App, ui: &mut egui::Ui, report: Report) {
    if app.game.is_none() {
        ui.label("No game open.");
        return;
    }
    app.drawn_scope = "report";

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
    // Every column's width, whether it is shown or not: the horizontal
    // scrollbar's range is worked out over all of them.
    let all_widths: Vec<f32> = (0..columns.len())
        .map(|c| column_width(report, c, of(columns[c].name), digit, &of))
        .collect();
    let drawn = app.reports.state(report).drawn(report);
    let widths: Vec<f32> = drawn.iter().map(|&c| all_widths[c]).collect();

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

    let mut clicked_header: Option<(usize, Pos2)> = None;
    // The row, the column, how far into the cell, how wide it is, and where.
    let mut clicked_row: Option<(usize, usize, f32, f32, Pos2)> = None;

    // `ReportDlg`'s `WM_SIZE`: the rows that fit are what is left of the
    // client once the header and the horizontal scrollbar have taken
    // `0x24` between them.
    let client = ui.available_rect_before_wrap();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a row count, from a height"
    )]
    let fits = ((client.height() - 36.0) / row_height).floor().max(0.0) as usize;
    let rows_vis = fits.min(rows.len());
    let state = *app.reports.state(report);
    let first_row = state.first_row.min(rows.len().saturating_sub(rows_vis));

    {
        let (response, painter) = ui.allocate_painter(client.size(), Sense::click());
        let origin = response.rect.min;
        painter.rect_filled(response.rect, 0.0, face());

        // The header row. Each heading is written down under its name,
        // so the tutor's ring and a test can find the column to click.
        let mut x = origin.x + 2.0;
        let top = origin.y + 2.0;
        let mut headings: Vec<(String, Rect)> = Vec::new();
        for (index, &c) in drawn.iter().enumerate() {
            let w = widths[index];
            let cell = Rect::from_min_size(Pos2::new(x, top), Vec2::new(w - 1.0, row_height));
            headings.push((columns[c].name.to_string(), cell));
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

        for (name, cell) in headings {
            crate::views::note_widget(app, ui, &name, cell, true);
        }

        // The rows that fit, from the one the scrollbar has scrolled to.
        for (seen, cells) in cells
            .iter()
            .enumerate()
            .skip(first_row)
            .take(rows_vis)
            .enumerate()
            .map(|(seen, (_, cells))| (seen, cells))
        {
            let row = first_row + seen;
            let y = top + row_height * (seen + 1) as f32;
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
                let mut left = origin.x + 2.0;
                let mut hit = None;
                for (index, &c) in drawn.iter().enumerate() {
                    if pos.x < left + widths[index] {
                        hit = Some((c, pos.x - left, widths[index]));
                        break;
                    }
                    left += widths[index];
                }
                if let Some((column, into, width)) = hit {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "a row number, from a pixel offset"
                    )]
                    let seen = ((pos.y - top) / row_height).floor() as i64 - 1;
                    if seen < 0 {
                        clicked_header = Some((column, pos));
                    } else if let Ok(seen) = usize::try_from(seen) {
                        let row = first_row + seen;
                        if seen < rows_vis && row < rows.len() {
                            clicked_row = Some((row, column, into, width, pos));
                        }
                    }
                }
            }
        }
    }

    scrollbars(
        app,
        ui,
        report,
        client,
        Geometry {
            line,
            row_height,
            rows: rows.len(),
            rows_vis,
            first_row,
            name_width: all_widths[0] + 2.0,
            widths: &all_widths,
        },
    );

    if let Some((column, pos)) = clicked_header {
        app.report_menu = Some((report, column, pos));
    }
    if let Some((row, column, into, width, pos)) = clicked_row {
        let production_open = app.production.is_some();
        let action = {
            let game = app.game.as_ref().expect("a game");
            let data = Data {
                game,
                player,
                battles: &battles,
            };
            data.click(report, rows[row], column, into, width, production_open)
        };
        act(app, action, ids.get(row).copied(), pos);
    }

    // The menu a header click has just opened is drawn from the next
    // frame: drawn now, the click that opened it would be the click
    // outside it that closes it.
    if clicked_header.is_none() {
        menu(app, ui, report);
    }
}

/// `GetSystemMetrics(SM_CXVSCROLL)` and `SM_CYHSCROLL`, which Windows 3.1
/// gives as sixteen pixels at the usual resolution. The original asks the
/// system for both and places the two bars against the answer.
const SCROLLBAR: f32 = 16.0;

/// What the grid's shape works out to, for placing the two bars.
struct Geometry<'a> {
    /// `dyArial8`.
    line: f32,
    /// `dyArial8 + 4`.
    row_height: f32,
    /// How many rows the report holds.
    rows: usize,
    /// How many of them fit.
    rows_vis: usize,
    /// Which one is at the top.
    first_row: usize,
    /// `rgbdx[0] * 2 + 2`: the name column, which never scrolls.
    name_width: f32,
    /// Every column's width, shown or not.
    widths: &'a [f32],
}

/// The two scrollbars, placed where `ReportDlg` and `SetHScrollBar` put
/// them: the vertical one down the right beside the rows, the horizontal
/// one under them and starting past the name column, so that the column
/// that never scrolls has no bar under it either.
fn scrollbars(app: &mut App, ui: &mut egui::Ui, report: Report, client: Rect, at: Geometry<'_>) {
    let state = *app.reports.state(report);
    let rows_area = at.row_height * at.rows_vis as f32;

    // Down the right: one row a line, a screenful less one a page.
    let reach = at.rows.saturating_sub(at.rows_vis);
    if reach > 0 {
        let rect = Rect::from_min_size(
            Pos2::new(client.right() - SCROLLBAR, client.top() + at.line + 6.0),
            Vec2::new(SCROLLBAR, rows_area + 1.0),
        );
        let page = at.rows_vis.saturating_sub(1).max(1);
        if let Some(by) = scrollbar(ui, rect, at.first_row, reach, page, false) {
            app.reports.state_mut(report).first_row = by.apply(at.first_row, reach, page);
        }
    } else if app.reports.state(report).first_row != 0 {
        app.reports.state_mut(report).first_row = 0;
    }

    // Underneath: one column a line, three a page.
    let room = client.width() - at.name_width - SCROLLBAR;
    let scroll = crate::report::scroll_columns(&state, at.widths, room);
    if scroll.max == 0 {
        if state.first_field != 1 {
            app.reports.state_mut(report).first_field = 1;
        }
        return;
    }
    let rect = Rect::from_min_size(
        Pos2::new(
            client.left() + at.name_width,
            client.top() + at.line + rows_area + 7.0,
        ),
        Vec2::new(room.max(SCROLLBAR * 3.0), SCROLLBAR),
    );
    if let Some(by) = scrollbar(
        ui,
        rect,
        scroll.position,
        scroll.max,
        crate::report::COLUMN_PAGE,
        true,
    ) {
        let moved = by.apply(scroll.position, scroll.max, crate::report::COLUMN_PAGE);
        app.reports.state_mut(report).first_field =
            crate::report::first_field_at(&state, at.widths.len(), moved);
    }
}

/// One Windows 3.1 scrollbar: an arrow at each end, a thumb between them,
/// and a trough that pages when it is clicked.
///
/// Returns what the player asked for, if anything. `horizontal` only says
/// which way round to draw it.
fn scrollbar(
    ui: &mut egui::Ui,
    rect: Rect,
    position: usize,
    max: usize,
    page: usize,
    horizontal: bool,
) -> Option<crate::report::ScrollBy> {
    use crate::report::ScrollBy;

    let painter = ui.painter_at(rect);
    // The trough is the one part not in button face: `COLOR_SCROLLBAR` is a
    // half-tone of face and white, which at this size reads as a light grey.
    painter.rect_filled(rect, 0.0, Color32::from_rgb(0xe0, 0xe0, 0xe0));

    let along = if horizontal {
        rect.width()
    } else {
        rect.height()
    };
    let button = SCROLLBAR.min(along / 2.0);
    let slice = |from: f32, size: f32| {
        if horizontal {
            Rect::from_min_size(
                Pos2::new(rect.left() + from, rect.top()),
                Vec2::new(size, rect.height()),
            )
        } else {
            Rect::from_min_size(
                Pos2::new(rect.left(), rect.top() + from),
                Vec2::new(rect.width(), size),
            )
        }
    };
    let less = slice(0.0, button);
    let more = slice(along - button, button);
    let trough = along - button * 2.0;

    // The thumb takes the share of the trough that a page is of the whole.
    #[allow(clippy::cast_precision_loss)]
    let span = (max + page) as f32;
    #[allow(clippy::cast_precision_loss)]
    let thumb_size = (trough * page as f32 / span).clamp(SCROLLBAR / 2.0, trough.max(1.0));
    #[allow(clippy::cast_precision_loss)]
    let thumb_at = button + (trough - thumb_size) * position as f32 / max.max(1) as f32;
    let thumb = slice(thumb_at, thumb_size);

    for (face_rect, arrow) in [(less, false), (more, true)] {
        painter.rect_filled(face_rect, 0.0, face());
        raised(&painter, face_rect);
        arrowhead(&painter, face_rect, horizontal, arrow);
    }
    painter.rect_filled(thumb, 0.0, face());
    raised(&painter, thumb);

    let response = ui.interact(
        rect,
        ui.id().with(("report scrollbar", horizontal)),
        Sense::click_and_drag(),
    );
    let pointer = response.interact_pointer_pos()?;
    let hit = if horizontal {
        pointer.x - rect.left()
    } else {
        pointer.y - rect.top()
    };

    if response.dragged() || response.is_pointer_button_down_on() {
        // Dragging anywhere but the two arrows tracks the thumb.
        if hit > button && hit < along - button {
            let travel = (trough - thumb_size).max(1.0);
            let from = (hit - button - thumb_size / 2.0).clamp(0.0, travel);
            #[allow(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss
            )]
            let to = (from * max as f32 / travel).round() as usize;
            return Some(ScrollBy::To(to));
        }
    }
    if !response.clicked() {
        return None;
    }
    if hit < button {
        Some(ScrollBy::Line(false))
    } else if hit > along - button {
        Some(ScrollBy::Line(true))
    } else if hit < thumb_at {
        Some(ScrollBy::Page(false))
    } else if hit > thumb_at + thumb_size {
        Some(ScrollBy::Page(true))
    } else {
        None
    }
}

/// The little black triangle on a scrollbar's arrow button.
fn arrowhead(painter: &egui::Painter, rect: Rect, horizontal: bool, forward: bool) {
    let middle = rect.center();
    let reach = rect.width().min(rect.height()) / 4.0;
    let points = match (horizontal, forward) {
        (true, true) => [
            Pos2::new(middle.x - reach / 2.0, middle.y - reach),
            Pos2::new(middle.x - reach / 2.0, middle.y + reach),
            Pos2::new(middle.x + reach, middle.y),
        ],
        (true, false) => [
            Pos2::new(middle.x + reach / 2.0, middle.y - reach),
            Pos2::new(middle.x + reach / 2.0, middle.y + reach),
            Pos2::new(middle.x - reach, middle.y),
        ],
        (false, true) => [
            Pos2::new(middle.x - reach, middle.y - reach / 2.0),
            Pos2::new(middle.x + reach, middle.y - reach / 2.0),
            Pos2::new(middle.x, middle.y + reach),
        ],
        (false, false) => [
            Pos2::new(middle.x - reach, middle.y + reach / 2.0),
            Pos2::new(middle.x + reach, middle.y + reach / 2.0),
            Pos2::new(middle.x, middle.y - reach),
        ],
    };
    painter.add(egui::Shape::convex_polygon(
        points.to_vec(),
        Color32::BLACK,
        egui::Stroke::NONE,
    ));
}

/// Do what a click asked for.
///
/// `ExecuteReportClick` selects the row's object first and then acts, and
/// the two are not alternatives: opening the production queue also leaves
/// the planet selected.
fn act(app: &mut App, action: Click, id: Option<RowId>, pos: Pos2) {
    if action == Click::Refused {
        return;
    }
    if let Some(id) = id {
        id.select(app);
    }
    match action {
        Click::Refused | Click::Select | Click::Vcr => {}
        Click::Production => app.open_production(),
        // `SetScanWp(1)`: take hold of the first waypoint the fleet is
        // actually going to.
        Click::Waypoint => app.selection.waypoint = Some(1),
        // `TransferStuff` opens a dialog this project does not have; the
        // fleet is selected, which is as far as it goes.
        Click::Transfer => {}
        Click::Popup(kind) => {
            if let Some(popup) = popup_for(app, kind, id) {
                app.popup = Some((popup, (pos.x, pos.y)));
            }
        }
    }
}

/// The pop-up a column raises, where this project has one to raise.
fn popup_for(app: &App, kind: PopupKind, id: Option<RowId>) -> Option<crate::popup::Popup> {
    match kind {
        PopupKind::Fleet => match id {
            Some(RowId::Fleet(index)) => Some(app.fleet_popup(index)),
            _ => None,
        },
        PopupKind::Defense => app
            .best_defense_part()
            .map(|(category, item)| crate::popup::Popup::Component((category, item))),
        PopupKind::Mineral(mineral) => match id {
            Some(RowId::Planet(planet)) => app.mineral_popup(planet, mineral),
            _ => None,
        },
        PopupKind::Industry { factories } => match id {
            Some(RowId::Planet(planet)) => app.industry_popup(planet, factories),
            _ => None,
        },
        PopupKind::Resources => match id {
            Some(RowId::Planet(planet)) => app.resources_popup(planet),
            _ => None,
        },
        PopupKind::Population => match id {
            Some(RowId::Planet(planet)) => app.population_popup(planet),
            _ => None,
        },
        PopupKind::Starbase => match id {
            Some(RowId::Planet(planet)) => app.starbase_popup(planet),
            _ => None,
        },
    }
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
    Battle {
        /// Which recording.
        index: usize,
        /// The planet it happened at, or `None` for deep space.
        planet: Option<i16>,
    },
}

fn row_id(data: &Data<'_>, report: Report, row: usize) -> RowId {
    match report {
        Report::Planets => RowId::Planet(data.game.planets.get(row).map_or(-1, |p| p.id)),
        Report::Fleets | Report::EnemyFleets => RowId::Fleet(row),
        Report::Battles => RowId::Battle {
            index: row,
            planet: data
                .battles
                .get(row)
                .filter(|b| b.planet != u16::MAX)
                .and_then(|b| i16::try_from(b.planet).ok()),
        },
    }
}

impl RowId {
    fn is_selected(self, app: &App) -> bool {
        match self {
            RowId::Planet(id) => app.selection.planet == Some(id),
            RowId::Fleet(index) => app.selection.fleet == Some(index),
            // `DrawReportItem` colours the Location cell when the battle's
            // point is the one the scanner has.
            RowId::Battle { planet, .. } => {
                planet.is_some_and(|id| app.selection.planet == Some(id))
            }
        }
    }

    /// `ExecuteReportClick` (`1108:7cd6`) starts by selecting what the row is
    /// about, whichever column was clicked.
    fn select(self, app: &mut App) {
        match self {
            // `SelectAdjPlanet(0, id)` and `SelectAdjFleet(0, id)`: the same
            // selection a click on the map makes, so the panes follow.
            RowId::Planet(id) => app.select_object(crate::app::ScanObject::Planet(id)),
            RowId::Fleet(index) => app.select_object(crate::app::ScanObject::Fleet(index)),
            // `ExecuteReportClick` moves the scanner to where the battle
            // was and returns; only a click that finds it already there
            // opens the recording. A battle in deep space has no planet to
            // select, so it opens at once.
            RowId::Battle { index, planet } => {
                let opens =
                    crate::report::battle_opens(planet, app.selection.planet, app.vcr.is_some());
                if let Some(id) = planet {
                    app.select_object(crate::app::ScanObject::Planet(id));
                }
                if opens {
                    app.open_battle(index);
                }
            }
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
                            let entry = ui.button(text);
                            crate::views::record(app, ui, text, &entry);
                            if entry.clicked() {
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
                            let sub = ui.menu_button(&title, |ui| {
                                while inner < built.entries.len() {
                                    match &built.entries[inner] {
                                        Entry::Submenu => break,
                                        Entry::Separator => {
                                            ui.separator();
                                        }
                                        Entry::Item(text) => {
                                            let entry = ui.button(text);
                                            crate::views::record(app, ui, text, &entry);
                                            if entry.clicked() {
                                                chose = Some(inner);
                                                ui.close_menu();
                                            }
                                        }
                                    }
                                    inner += 1;
                                }
                            });
                            crate::views::record(app, ui, &title, &sub.response);
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

    // A click outside the menu closes it. The menu's own rectangle is
    // what counts, not whether it is hovered: a submenu opening under
    // the pointer takes the hover for itself on the very frame its title
    // is clicked.
    let outside = ui.ctx().input(|i| {
        i.pointer.any_click()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| !area.response.rect.contains(p))
    });
    if let Some(index) = chose {
        app.reports.choose(report, column, index);
        dismiss = true;
    } else if outside {
        dismiss = true;
    }
    if dismiss {
        app.report_menu = None;
    }
}
