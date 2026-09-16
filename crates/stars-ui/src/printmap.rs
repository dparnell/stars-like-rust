//! Print Map — File (Print Map...), menu `0xd5`.
//!
//! `CommandHandler`'s `0xd5` arm (`1020:3288`) runs the **Print Map**
//! dialog (`PrintMapDlg`, `1108:a1c2`, resource 214), which asks how many
//! pages across and down to tile the map over — `vrgcPrintMapPage`, one
//! by one to start with — and then prints it through the Windows print
//! dialog: on each page, in the page's own pixels, a square map of the
//! whole universe bordered by a rectangle, with a block of text beside or
//! below it (the title, the game's name, the player's, the year), a
//! legend of five symbols, and every planet as a dot with its owner's
//! mark and its name.
//!
//! There is no printer behind an egui window, so **Print** here lays the
//! pages out exactly as the arm would have drawn them on the printer's
//! DC — the same geometry, on a Letter page at ninety-six dots an inch —
//! into a preview, from which the shell can save them as pictures
//! ([`crate::raster`]). See `docs/ui/print-map.md`.

use crate::App;
use stars_core::Planet;

/// `vrgcPrintMapPage` as the program starts: one page each way.
pub const DEFAULT_PAGES: [i16; 2] = [1, 1];

/// The dialog's two fields, as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintMapDialog {
    /// Pages across (`0x10c`) and down (`0x10d`), as text — each box
    /// takes one character (`EM_LIMITTEXT 1`).
    pub fields: [String; 2],
    /// The complaint the last **Print** raised, if any.
    pub error: Option<String>,
}

/// The preview: which page is on show of the ones **Print** laid out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintPreview {
    /// Pages across and down, as printed (after the orientation swap).
    pub across: i16,
    pub down: i16,
    /// The page on show, row by row from the top left.
    pub page: usize,
}

impl PrintPreview {
    /// How many pages there are.
    #[must_use]
    pub fn pages(&self) -> usize {
        usize::try_from(self.across).unwrap_or(1) * usize::try_from(self.down).unwrap_or(1)
    }

    /// The page on show as its column and row.
    #[must_use]
    pub fn at(&self) -> (i16, i16) {
        let across = i64::from(self.across.max(1));
        let page = i64::try_from(self.page).unwrap_or(0);
        (
            i16::try_from(page % across).unwrap_or(0),
            i16::try_from(page / across).unwrap_or(0),
        )
    }
}

/// The page the arm prints on, in its own pixels: US Letter at
/// ninety-six dots an inch, which is what `GetDeviceCaps` would say of
/// a display-resolution printer.
pub const PAGE: Page = Page {
    width: 816,
    height: 1056,
    dpi: 96,
};

/// A printer page as `GetDeviceCaps` describes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    /// `HORZRES`.
    pub width: i32,
    /// `VERTRES`.
    pub height: i32,
    /// `LOGPIXELSX` and `LOGPIXELSY`, taken as one.
    pub dpi: i32,
}

/// Where everything goes, worked out once for the whole print.
///
/// The arm's own arithmetic (`1020:32f4`–`1020:34f0`), variable for
/// variable. `margin` is `0x20` device pixels whatever the resolution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// Pages across and down, after the orientation swap.
    pub across: i16,
    pub down: i16,
    /// The map's outer square (`local_9a`/`local_9c`): the rectangle
    /// drawn round it, from the page origin.
    pub square: i32,
    /// The map's inner size (`local_8c`): the square less two margins,
    /// which the planet coordinates are scaled to.
    pub map: i32,
    /// The margin (`local_72`).
    pub margin: i32,
    /// Where the text block starts (`local_84`, `local_82`).
    pub text: (i32, i32),
    /// Where the legend starts (`local_80`, `local_7e`).
    pub legend: (i32, i32),
    /// The two fonts' line heights (`local_86`, `local_70`).
    pub line: i32,
    pub small_line: i32,
    /// The two fonts' sizes in pixels.
    pub font: f32,
    pub small_font: f32,
}

impl Layout {
    /// Lay the print out for `pages` across and down on `page`.
    ///
    /// The larger page count goes along the page's longer side: `if
    /// (down < across) == (height < width) swap`. The map is a square as
    /// large as the pages allow, leaving room for the text — an inch
    /// below it when the sheet is taller than wide, an inch and a half
    /// beside it otherwise — and never more than 32000 pixels.
    #[must_use]
    pub fn new(pages: [i16; 2], page: Page) -> Layout {
        let (mut across, mut down) = (i32::from(pages[0].max(1)), i32::from(pages[1].max(1)));
        if (down < across) == (page.height < page.width) {
            std::mem::swap(&mut across, &mut down);
        }
        let margin = 0x20;
        let mut total_w = (page.width * across).min(32000);
        let mut total_h = (page.height * down).min(32000);
        // `HfontPrinterCreate(hdc, 8, ...)` and `(hdc, 5, ...)`: Arial at
        // eight and five points for the printer's resolution, and their
        // lines as `tmHeight + tmExternalLeading` — six and four fifths
        // of a point in pixels, which is what Arial measures.
        let font = 8.0 * page.dpi as f32 / 72.0;
        let small_font = 5.0 * page.dpi as f32 / 72.0;
        let line = (font * 1.15).round() as i32;
        let small_line = (small_font * 1.15).round() as i32;
        let (square, text, legend);
        if total_w < total_h {
            // Taller than wide: the map is as wide as the sheet and the
            // text goes under it, at least an inch of it.
            if total_h - page.dpi < total_w {
                total_w = total_h - page.dpi;
            }
            square = total_w;
            text = (margin, total_w + margin);
            legend = (total_w / 2, total_w + margin);
        } else {
            // Wider than tall: the map is as tall as the sheet and the
            // text goes beside it, an inch and a half of it.
            if total_w - page.dpi * 3 / 2 < total_h {
                total_h = total_w - page.dpi * 3 / 2;
            }
            square = total_h;
            text = (total_h + margin, margin);
            legend = (total_h + margin, total_h / 2);
        }
        Layout {
            across: i16::try_from(across).unwrap_or(1),
            down: i16::try_from(down).unwrap_or(1),
            square,
            map: square - margin * 2,
            margin,
            text,
            legend,
            line,
            small_line,
            font,
            small_font,
        }
    }

    /// Where a galaxy position lands on the whole print, from the
    /// page-grid origin: `(x − 1000) · map / dGal + margin`, and the
    /// y flipped about the universe as the scanner flips it.
    #[must_use]
    pub fn plot(&self, x: i32, y: i32, span: i32) -> (i32, i32) {
        let span = i64::from(span.max(1));
        let map = i64::from(self.map);
        let px = i64::from(x - 1000) * map / span + i64::from(self.margin);
        let py = (span + 1000 - i64::from(y)) * map / span + i64::from(self.margin);
        (
            i32::try_from(px).unwrap_or(0),
            i32::try_from(py).unwrap_or(0),
        )
    }
}

/// The string table's lines the print uses: the title (`0x520`), the
/// year (`0x521`, `Year: %d`) and the five legend entries (`0x522` to
/// `0x526`), read from the executable like the About box's credits.
pub const TITLE_STRING: u16 = 0x520;
/// See [`TITLE_STRING`].
pub const YEAR_STRING: u16 = 0x521;
/// See [`TITLE_STRING`].
pub const LEGEND_STRINGS: std::ops::RangeInclusive<u16> = 0x522..=0x526;

/// This project's own wording for the same lines, used when no copy of
/// the executable is at hand: the title, the year line with `{}` for the
/// number, and the five legend entries — each led by three spaces and an
/// equals sign, where the symbol goes.
pub const OWN_TITLE: &str = "Stars! Universe Map";
/// See [`OWN_TITLE`].
pub const OWN_YEAR: &str = "Year: {}";
/// See [`OWN_TITLE`].
pub const OWN_LEGEND: [&str; 5] = [
    "   = one of your planets",
    "   = with an orbital fort",
    "   = with a starbase",
    "   = an empty planet",
    "   = another player's, by number",
];

impl App {
    /// File (Print Map...): put the dialog up with the counts as they
    /// were last printed.
    pub fn open_print_map(&mut self) {
        self.print_map_dialog = Some(PrintMapDialog {
            fields: [
                self.print_pages[0].to_string(),
                self.print_pages[1].to_string(),
            ],
            error: None,
        });
    }

    /// Cancel: nothing changes.
    pub fn close_print_map(&mut self) {
        self.print_map_dialog = None;
    }

    /// A keystroke in one of the boxes (`EN_UPDATE`): anything but `1`
    /// to `9` beeps and is taken back out, which with a one-character
    /// box leaves it empty.
    pub fn print_map_type(&mut self, field: usize, text: &str) -> bool {
        let Some(dialog) = self.print_map_dialog.as_mut() else {
            return false;
        };
        let Some(slot) = dialog.fields.get_mut(field) else {
            return false;
        };
        let typed: String = text.chars().take(1).collect();
        let good = typed
            .chars()
            .next()
            .is_none_or(|c| c.is_ascii_digit() && c != '0');
        *slot = if good { typed } else { String::new() };
        good
    }

    /// Print: take the counts, complain about the first that is not a
    /// digit from 1 to 9 — and then print anyway, as the original does.
    ///
    /// `PrintMapDlg`'s OK arm (`1108:a2f2`) walks the two boxes storing
    /// each into `vrgcPrintMapPage` and, at the first bad one, puts up
    /// *You must specify a number between 1 and 9*, focuses it and
    /// `break`s — out of the loop, not out of the arm, so `EndDialog(1)`
    /// follows and the frame prints with whatever the array held. The
    /// complaint is returned for the shell to show.
    pub fn print_map_ok(&mut self) -> Option<String> {
        let dialog = self.print_map_dialog.take()?;
        let mut complaint = None;
        for (index, field) in dialog.fields.iter().enumerate() {
            let mut chars = field.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if ('1'..='9').contains(&c) => {
                    self.print_pages[index] = i16::from(c as u8 - b'0');
                }
                _ => {
                    complaint =
                        Some("Each count has to be a single digit from 1 to 9.".to_string());
                    break;
                }
            }
        }
        let layout = Layout::new(self.print_pages, PAGE);
        self.print_preview = Some(PrintPreview {
            across: layout.across,
            down: layout.down,
            page: 0,
        });
        complaint
    }

    /// Close the preview.
    pub fn close_print_preview(&mut self) {
        self.print_preview = None;
        self.print_save_requested = false;
    }

    /// The print's layout, for the counts last printed.
    #[must_use]
    pub fn print_layout(&self) -> Layout {
        Layout::new(self.print_pages, PAGE)
    }

    /// The print's own strings — the title, the year line and the five
    /// legend entries — from the executable's string table when a copy
    /// is at hand, else this project's wording.
    #[must_use]
    pub fn print_strings(&self) -> (String, String, Vec<String>) {
        use stars_formats::resources::text;
        let exe = self.art.as_ref().map(crate::art::Art::executable);
        let read = |ids: u16| exe.and_then(|exe| text::string(exe, ids));
        let title = read(TITLE_STRING).unwrap_or_else(|| OWN_TITLE.to_string());
        let year = read(YEAR_STRING)
            .map(|s| s.replace("%d", "{}"))
            .unwrap_or_else(|| OWN_YEAR.to_string());
        let legend: Vec<String> = LEGEND_STRINGS
            .map(read)
            .collect::<Option<Vec<_>>>()
            .unwrap_or_else(|| OWN_LEGEND.iter().map(|s| (*s).to_string()).collect());
        (title, year, legend)
    }

    /// `dGal`: the universe's width in light years.
    #[must_use]
    pub fn galaxy_span(&self) -> i32 {
        let size = self.game.as_ref().map_or(0, |g| g.galaxy_size);
        i32::from(size.clamp(0, 4)) * 400 + 400
    }

    /// Draw one page of the print — column `column`, row `row` of the
    /// grid — into `rect`, which stands for the page at `scale` pixels
    /// per printer pixel.
    ///
    /// This is the arm's page loop (`1020:3560`–`1020:3a8c`) drawn with
    /// egui's painter: the border, the text block, the legend with its
    /// symbols, the owners' marks in the small face, then every planet's
    /// dot and, with the names overlay on, its name.
    #[allow(clippy::too_many_lines)]
    pub fn paint_print_page(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        scale: f32,
        column: i16,
        row: i16,
    ) {
        use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke};
        let layout = self.print_layout();
        let page = PAGE;
        let painter = painter.with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        // The page's offset into the whole print (`local_a4`, `local_a2`).
        let ox = -page.width * i32::from(column);
        let oy = -page.height * i32::from(row);
        let at = |x: i32, y: i32| -> Pos2 {
            rect.min + egui::vec2((x + ox) as f32 * scale, (y + oy) as f32 * scale)
        };
        let big = FontId::proportional(layout.font * scale);
        let small = FontId::proportional(layout.small_font * scale);
        let black = Color32::BLACK;
        let line = layout.line;

        // The rectangle round the map.
        painter.rect_stroke(
            Rect::from_min_max(at(0, 0), at(layout.square, layout.square)),
            0.0,
            Stroke::new((1.0 * scale).max(1.0), black),
        );

        // The text block: title, game, player, year.
        let (tx, ty) = layout.text;
        let mut y = ty;
        let text_out = |painter: &egui::Painter, x: i32, y: i32, s: &str, font: &FontId| {
            painter.text(at(x, y), Align2::LEFT_TOP, s, font.clone(), black);
        };
        let ctr_text = |painter: &egui::Painter, x: i32, y: i32, s: &str, font: &FontId| {
            painter.text(at(x, y), Align2::CENTER_TOP, s, font.clone(), black);
        };
        let (title, year_line, legend) = self.print_strings();
        let game_name = self.game_name();
        let year = self
            .game
            .as_ref()
            .map_or(2400, |g| i32::from(g.turn) + 2400);
        text_out(&painter, tx, y, &title, &big);
        y += line;
        text_out(&painter, tx, y, &game_name, &big);
        y += line;
        text_out(
            &painter,
            tx,
            y,
            &self.psz_player_name(self.local_player()),
            &big,
        );
        y += line;
        text_out(
            &painter,
            tx,
            y,
            &year_line.replace("{}", &year.to_string()),
            &big,
        );

        // Nothing but the frame and the text in the No Player Info view.
        if self.scan_view as u8 == 5 {
            return;
        }

        // The legend: five lines, then the symbols over their leading
        // spaces — the fort and starbase marks and the other player's
        // number in the small face, the dots in the big.
        let (lx, ly) = layout.legend;
        let mut y = ly;
        for entry in &legend {
            text_out(&painter, lx, y, entry, &big);
            y += line;
        }
        let small_line = layout.small_line;
        ctr_text(
            &painter,
            lx,
            ly + line * 3 / 2 - small_line / 2,
            "|",
            &small,
        );
        ctr_text(
            &painter,
            lx,
            ly + line * 5 / 2 - small_line / 2,
            "+",
            &small,
        );
        ctr_text(
            &painter,
            lx,
            ly + line * 9 / 2 + 8 - small_line,
            "2",
            &small,
        );
        let dot = |painter: &egui::Painter, x: i32, y: i32, own: bool| {
            dot_at(painter, at(x, y), scale, own);
        };
        dot(&painter, lx, ly - 4 + line / 2, true);
        dot(&painter, lx, ly - 4 + line * 7 / 2, false);
        dot(&painter, lx, ly + line * 9 / 2 + 8, false);

        // The planets the player has a record of: their owners' marks.
        let span = self.galaxy_span();
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return;
        };
        let known: Vec<&Planet> = game
            .planets
            .iter()
            .chain(game.known_planets.iter())
            .collect();
        for planet in &known {
            let Some(p) = planet.position else { continue };
            let (x, y) = layout.plot(i32::from(p.x), i32::from(p.y), span);
            match planet.owner {
                Some(owner) if usize::try_from(owner).is_ok_and(|o| o == me) => {
                    if planet.starbase {
                        let fort = self.starbase_hull(planet) == Some(32);
                        ctr_text(
                            &painter,
                            x,
                            y + 4 - small_line,
                            if fort { "|" } else { "+" },
                            &small,
                        );
                    }
                    dot(&painter, x, y, true);
                }
                Some(owner) => {
                    ctr_text(
                        &painter,
                        x,
                        y - small_line,
                        &(owner + 1).to_string(),
                        &small,
                    );
                }
                None => {}
            }
        }
        // Every planet: a dot, and its name fourteen under when the
        // names overlay is on (`grbitScan & 0x400`).
        let names = self.grbit_scan() & 0x400 != 0;
        for planet in &known {
            let Some(p) = planet.position else { continue };
            let (x, y) = layout.plot(i32::from(p.x), i32::from(p.y), span);
            dot(&painter, x, y, false);
            if names {
                ctr_text(&painter, x, y + 14, &self.planet_name(planet.id), &big);
            }
        }
    }

    /// One page of the print as a picture, at the page's own size.
    #[must_use]
    pub fn print_page_image(&self, page: usize) -> crate::raster::Image {
        let layout = self.print_layout();
        let across = i64::from(layout.across.max(1));
        let page = i64::try_from(page).unwrap_or(0);
        let (column, row) = (
            i16::try_from(page % across).unwrap_or(0),
            i16::try_from(page / across).unwrap_or(0),
        );
        let size = (PAGE.width as usize, PAGE.height as usize);
        crate::raster::render(size, &mut |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(egui::Color32::WHITE))
                .show(ctx, |ui| {
                    let rect = ui.max_rect();
                    self.paint_print_page(ui.painter(), rect, 1.0, column, row);
                });
        })
    }
}

/// `DrawPlanetPrintDot` (`1038:7e44`): a small round dot for any planet,
/// a larger one for the player's own, built from `PatBlt` rectangles —
/// 7 across for the small (three strips), 11 for the large (four).
pub fn dot_at(painter: &egui::Painter, centre: egui::Pos2, scale: f32, own: bool) {
    let black = egui::Color32::BLACK;
    let strips: &[(i32, i32, i32, i32)] = if own {
        &[
            (-5, -2, 11, 5),
            (-2, -5, 5, 11),
            (-4, -3, 9, 7),
            (-3, -4, 7, 9),
        ]
    } else {
        &[(-3, -1, 7, 3), (-1, -3, 3, 7), (-2, -2, 5, 5)]
    };
    for &(dx, dy, w, h) in strips {
        let min = centre + egui::vec2(dx as f32 * scale, dy as f32 * scale);
        let size = egui::vec2(w as f32 * scale, h as f32 * scale);
        painter.rect_filled(egui::Rect::from_min_size(min, size), 0.0, black);
    }
}
