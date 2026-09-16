//! The About box — Help (About Stars!...), menu `0x63`.
//!
//! `About` (`1018:1252`) runs dialog resource 90 ([`crate::dialog::ABOUT`]):
//! the game's icon and its titles, the version line filled in from
//! `SzVersion`, the copyright and publisher, and under them the
//! **credits** — seventy-seven lines of the string table
//! (`idsDesignProgramming` on, `stars_formats::resources::text::CREDITS`)
//! rolled up through an empty static by a fifty-millisecond timer, two
//! pixels a tick, one line every `dyArial8`. The lines are drawn centred
//! in Arial 8 bold, button-face behind, ten at a time; the first pass
//! starts eleven lines below the window (`iAbout1st = -11`) so the roll
//! begins empty, and past the last line (`0x4e`) it starts over.
//!
//! **Order Info...** (`0x76`) puts up dialog 97 over it, the publisher's
//! ordering details; OK closes either.
//!
//! The credits and the version format are the game's own strings, read
//! from the player's copy of the executable like the tutorial's text;
//! without one the roll is empty and the version line is the template's
//! `Demo Version`, which is what the resource holds before `SzVersion`
//! overwrites it. See `docs/ui/about.md`.

use crate::App;

/// The button face the box is painted with (`hbrButtonFace`).
const BUTTON_FACE: egui::Color32 = egui::Color32::from_rgb(0xc0, 0xc0, 0xc0);
/// `crButtonText`.
const BUTTON_TEXT: egui::Color32 = egui::Color32::BLACK;

/// The state behind the box: when it opened, for the roll, and whether
/// the ordering information is up over it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct About {
    /// egui's clock when the box opened.
    pub opened_at: f64,
    /// Whether the ordering information box is up.
    pub order_info: bool,
}

impl About {
    /// The timer's period: fifty milliseconds (`SETTIMER(..., 0x32, ...)`).
    pub const TICK: f64 = 0.05;
    /// How far each tick rolls, in pixels.
    pub const STEP: i32 = 2;
    /// Where the roll starts: eleven lines below the window.
    pub const FIRST: i32 = -11;
    /// The line past which the roll starts over.
    pub const LAST: i32 = 0x4e;
    /// How many lines are drawn at once.
    pub const SHOWN: i32 = 10;

    /// The roll's position after `now`: the first line drawn, and how many
    /// pixels of it have scrolled off the top.
    ///
    /// `WM_TIMER` (`1018:1320`) adds two to `iAboutPartial` each tick and,
    /// when that reaches `dyArial8`, zeroes it and steps `iAbout1st` on,
    /// wrapping to `-11` past `0x4e`.
    #[must_use]
    pub fn position(&self, now: f64, line: i32) -> (i32, i32) {
        let line = line.max(1);
        // A hair of slack, so a clock that lands on a tick counts it.
        let ticks = ((now - self.opened_at).max(0.0) / Self::TICK + 1e-6).floor();
        #[allow(clippy::cast_possible_truncation)]
        let ticks = ticks.min(f64::from(i32::MAX / 4)) as i32;
        // How many ticks one line takes: the partial reaches the line
        // height on the first tick that brings it there or past.
        let per_line = (line + Self::STEP - 1) / Self::STEP;
        let span = Self::LAST - Self::FIRST + 1;
        let lines = ticks / per_line;
        let first = Self::FIRST + lines % span;
        let partial = (ticks % per_line) * Self::STEP;
        (first, partial)
    }
}

impl App {
    /// Help (About Stars!...): put the box up.
    pub fn open_about(&mut self, now: f64) {
        self.about = Some(About {
            opened_at: now,
            order_info: false,
        });
    }

    /// OK: take it down, timer and all.
    pub fn close_about(&mut self) {
        self.about = None;
    }

    /// The version line: `SzVersion` from the executable, or the
    /// template's own text without one.
    #[must_use]
    pub fn about_version(&self) -> String {
        self.art
            .as_ref()
            .and_then(|art| stars_formats::resources::text::version(art.executable()))
            .unwrap_or_else(|| {
                crate::dialog::ABOUT
                    .control(0x401)
                    .map_or_else(String::new, crate::dialog::Control::label)
            })
    }

    /// The credits, line by line, or nothing without the executable.
    #[must_use]
    pub fn about_credits(&self) -> Vec<String> {
        self.art
            .as_ref()
            .and_then(|art| stars_formats::resources::text::credits(art.executable()))
            .unwrap_or_default()
    }
}

/// Draw the box.
pub fn view(app: &mut App, ui: &mut egui::Ui, now: f64) {
    let Some(about) = app.about.clone() else {
        return;
    };
    let template = &crate::dialog::ABOUT;
    let (rect, at, caption) = crate::views::dialog_frame(ui, template);
    let scale = template.scale(rect);
    // `WM_ERASEBKGND` fills the client with the button face, and
    // `WM_CTLCOLOR` hands every static the same brush.
    ui.painter().rect_filled(rect, 0.0, BUTTON_FACE);
    let line_px = crate::dialog::DLU_Y * 8.0 * scale;
    let font = egui::FontId::proportional(line_px * 0.8);

    let controls = template.controls;
    for control in controls {
        let where_ = crate::dialog::place(rect.min, scale, control.at);
        match (control.class, control.id, control.text) {
            (crate::dialog::Class::Static, _, "StarsIco") => {
                // `SS_ICON`: the game's own icon, at the icon size.
                let size = where_.height().min(where_.width());
                if let Some(image) = app
                    .art
                    .as_mut()
                    .and_then(|art| art.icon(ui.ctx(), "StarsIco", size))
                {
                    ui.put(
                        egui::Rect::from_min_size(where_.min, egui::vec2(size, size)),
                        image,
                    );
                }
            }
            (crate::dialog::Class::Static, 0x41f, _) => {
                credits(app, ui, where_, scale, now, &about);
            }
            (crate::dialog::Class::Static, id, text) => {
                let text = if id == 0x401 {
                    app.about_version()
                } else {
                    text.replace("&&", "&")
                };
                // `SS_CENTER`: the text centred on the control's top line.
                let top = egui::Rect::from_min_size(
                    where_.min,
                    egui::vec2(where_.width(), line_px.max(where_.height().min(line_px))),
                );
                ui.painter().text(
                    top.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    font.clone(),
                    BUTTON_TEXT,
                );
            }
            (crate::dialog::Class::Button, 0x76, _) => {
                if crate::views::placed_button(app, ui, at(0x76), &caption(0x76), true).clicked() {
                    if let Some(about) = app.about.as_mut() {
                        about.order_info = true;
                    }
                }
            }
            (crate::dialog::Class::Button, 0x1, _)
                if crate::views::placed_button(app, ui, at(0x1), &caption(0x1), true).clicked() =>
            {
                app.close_about();
            }
            _ => {}
        }
    }
    // The timer keeps the roll moving.
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(50));
}

/// The credits rolling through their static.
fn credits(app: &App, ui: &mut egui::Ui, rect: egui::Rect, scale: f32, now: f64, about: &About) {
    let lines = app.about_credits();
    // `dyArial8`: Arial 8's line, thirteen pixels at the dialog's own size.
    let line_px = 13.0 * scale;
    #[allow(clippy::cast_possible_truncation)]
    let line = line_px.round() as i32;
    let (first, partial) = about.position(now, line);
    let painter = ui.painter().with_clip_rect(rect);
    let font = egui::FontId::proportional(line_px * 0.8);
    let mut top = rect.top() - partial as f32;
    for index in first..first + About::SHOWN {
        // Lines run `0` to `0x4c`; past the last there is nothing to draw.
        if index > About::LAST - 2 {
            break;
        }
        if index >= 0 {
            if let Some(text) = usize::try_from(index).ok().and_then(|i| lines.get(i)) {
                let row = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), top),
                    egui::vec2(rect.width(), line_px),
                );
                // Arial 8 **bold**: egui's set has no bold face, so the
                // line is painted twice, a hair apart.
                for dx in [0.0, 0.7] {
                    painter.text(
                        row.center() + egui::vec2(dx, 0.0),
                        egui::Align2::CENTER_CENTER,
                        text,
                        font.clone(),
                        BUTTON_TEXT,
                    );
                }
            }
        }
        top += line_px;
    }
}

/// The ordering information box over the About box.
pub fn order_info(app: &mut App, ui: &mut egui::Ui) {
    let template = &crate::dialog::ORDER_INFO;
    let (rect, at, caption) = crate::views::dialog_frame(ui, template);
    let scale = template.scale(rect);
    ui.painter().rect_filled(rect, 0.0, BUTTON_FACE);
    let line_px = crate::dialog::DLU_Y * 8.0 * scale;
    let font = egui::FontId::proportional(line_px * 0.8);
    for control in template.controls {
        let where_ = crate::dialog::place(rect.min, scale, control.at);
        match control.class {
            crate::dialog::Class::Static => {
                ui.painter().text(
                    where_.center(),
                    egui::Align2::CENTER_CENTER,
                    control.label(),
                    font.clone(),
                    BUTTON_TEXT,
                );
            }
            crate::dialog::Class::Button
                if crate::views::placed_button(app, ui, at(0x1), &caption(0x1), true).clicked() =>
            {
                if let Some(about) = app.about.as_mut() {
                    about.order_info = false;
                }
            }
            _ => {}
        }
    }
}
