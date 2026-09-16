//! The About box — Help (About Stars!...), menu `0x63`.
//!
//! Two boxes. The first is **this project's**: the original's About
//! template (resource 90, `crate::dialog::ABOUT_PROJECT`) with its captions
//! replaced, saying what this program is — a reverse-engineered,
//! cross-platform reimplementation of Stars! — with its own version, the
//! original's beside it, and a roll of credits that opens with how this
//! program was made and goes on to the original's own seventy-seven lines.
//! Its **About Stars!...** button opens the second box: the original's,
//! reproduced as `About` (`1018:1252`) draws it, with its **Order Info...**
//! and the original's roll.
//!
//! The roll is the original's mechanism in both: `WM_TIMER` every fifty
//! milliseconds moves the lines up two pixels through an empty static,
//! ten of them showing at a time in Arial 8 bold, from eleven lines below
//! the window (`iAbout1st = -11`) round again past the last line.
//!
//! The original's strings — its version format and its credits — are read
//! from the player's copy of the executable, like the tutorial's text;
//! without one the original's box shows the template's `Demo Version` and
//! an empty roll, and this project's names the two authors from its own
//! caption. See `docs/ui/about.md`.

use crate::App;

/// The button face the box is painted with (`hbrButtonFace`).
const BUTTON_FACE: egui::Color32 = egui::Color32::from_rgb(0xc0, 0xc0, 0xc0);
/// `crButtonText`.
const BUTTON_TEXT: egui::Color32 = egui::Color32::BLACK;

/// This project's name, as the box says it.
pub const PROJECT: &str = "Stars-re";

/// The state behind the boxes: when each opened, for its roll, and which
/// are up.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct About {
    /// egui's clock when this project's box opened.
    pub opened_at: f64,
    /// The original's box, while it is up: when it opened.
    pub original: Option<f64>,
    /// Whether the ordering information box is up over the original's.
    pub order_info: bool,
}

impl About {
    /// The timer's period: fifty milliseconds (`SETTIMER(..., 0x32, ...)`).
    pub const TICK: f64 = 0.05;
    /// How far each tick rolls, in pixels.
    pub const STEP: i32 = 2;
    /// Where the roll starts: eleven lines below the window.
    pub const FIRST: i32 = -11;
    /// The line past which the original's roll starts over (`0x4e`): one
    /// past its seventy-seven lines.
    pub const LAST: i32 = 0x4e;
    /// How many lines are drawn at once.
    pub const SHOWN: i32 = 10;

    /// The roll's position after `now` on a roll of `lines` lines: the
    /// first line drawn, and how many pixels of it have scrolled off the
    /// top.
    ///
    /// `WM_TIMER` (`1018:1320`) adds two to `iAboutPartial` each tick and,
    /// when that reaches `dyArial8`, zeroes it and steps `iAbout1st` on,
    /// wrapping to `-11` past one more than the last line — `0x4e` for the
    /// original's seventy-seven.
    #[must_use]
    pub fn position(&self, now: f64, line: i32, lines: i32) -> (i32, i32) {
        Self::roll(self.opened_at, now, line, lines)
    }

    /// [`Self::position`] for a roll that opened at `opened_at`.
    #[must_use]
    pub fn roll(opened_at: f64, now: f64, line: i32, lines: i32) -> (i32, i32) {
        let line = line.max(1);
        // A hair of slack, so a clock that lands on a tick counts it.
        let ticks = ((now - opened_at).max(0.0) / Self::TICK + 1e-6).floor();
        #[allow(clippy::cast_possible_truncation)]
        let ticks = ticks.min(f64::from(i32::MAX / 4)) as i32;
        // How many ticks one line takes: the partial reaches the line
        // height on the first tick that brings it there or past.
        let per_line = (line + Self::STEP - 1) / Self::STEP;
        let last = lines.max(0) + 1;
        let span = last - Self::FIRST + 1;
        let stepped = ticks / per_line;
        let first = Self::FIRST + stepped % span;
        let partial = (ticks % per_line) * Self::STEP;
        (first, partial)
    }
}

impl App {
    /// Help (About Stars!...): put this project's box up.
    pub fn open_about(&mut self, now: f64) {
        self.about = Some(About {
            opened_at: now,
            original: None,
            order_info: false,
        });
    }

    /// About Stars!...: the original's box over this project's.
    pub fn open_original_about(&mut self, now: f64) {
        if let Some(about) = self.about.as_mut() {
            about.original = Some(now);
            about.order_info = false;
        }
    }

    /// OK on the original's box: back to this project's.
    pub fn close_original_about(&mut self) {
        if let Some(about) = self.about.as_mut() {
            about.original = None;
            about.order_info = false;
        }
    }

    /// OK: take it all down, timers and all.
    pub fn close_about(&mut self) {
        self.about = None;
    }

    /// The original's version line: `SzVersion` from the executable, or
    /// the template's own text without one.
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

    /// This project's version line: the crate's version, and the
    /// original's it is rebuilt from.
    #[must_use]
    pub fn about_project_version(&self) -> String {
        let (major, minor, letter) = stars_formats::resources::text::VERSION;
        format!(
            "Version {} — rebuilt from Stars! {major}.{minor}{letter}",
            env!("CARGO_PKG_VERSION")
        )
    }

    /// The original's credits, line by line, or nothing without the
    /// executable.
    #[must_use]
    pub fn about_credits(&self) -> Vec<String> {
        self.art
            .as_ref()
            .and_then(|art| stars_formats::resources::text::credits(art.executable()))
            .unwrap_or_default()
    }

    /// This project's roll: how the program was made, then the original's
    /// credits after it — or the two names its caption carries, without
    /// the executable to read the rest from.
    #[must_use]
    pub fn about_project_credits(&self) -> Vec<String> {
        let mut lines: Vec<String> = [
            PROJECT,
            "",
            "A cross-platform reimplementation of Stars!",
            "",
            "",
            "Reverse Engineering",
            "",
            "The original Windows 3.1 executable, read in Ghidra",
            "with the NB09 debug symbols recovered by",
            "sirgwain/stars-asm and stars-decompile",
            "",
            "",
            "Programming",
            "",
            "Rewritten in Rust and drawn with egui",
            "Windows, macOS and Linux",
            "",
            "",
            "Assets",
            "",
            "The artwork, strings and help are the original's,",
            "read at run time from the player's own copy",
            "",
            "",
            "The Original",
            "",
            "Stars! 2.60j, by",
            "",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        let original = self.about_credits();
        if original.is_empty() {
            lines.push("Jeff Johnson".to_string());
            lines.push("Jeff McBride".to_string());
        } else {
            lines.extend(original);
        }
        lines
    }
}

/// Draw this project's box.
pub fn view(app: &mut App, ui: &mut egui::Ui, now: f64) {
    let Some(about) = app.about.clone() else {
        return;
    };
    let template = &crate::dialog::ABOUT_PROJECT;
    let version = app.about_project_version();
    let lines = app.about_project_credits();
    let pressed = draw_box(app, ui, template, &version, &lines, about.opened_at, now);
    match pressed {
        Some(0x76) => app.open_original_about(now),
        Some(0x1) => app.close_about(),
        _ => {}
    }
}

/// Draw the original's box.
pub fn original(app: &mut App, ui: &mut egui::Ui, now: f64) {
    let Some(opened_at) = app.about.as_ref().and_then(|a| a.original) else {
        return;
    };
    let template = &crate::dialog::ABOUT;
    let version = app.about_version();
    let lines = app.about_credits();
    let pressed = draw_box(app, ui, template, &version, &lines, opened_at, now);
    match pressed {
        Some(0x76) => {
            if let Some(about) = app.about.as_mut() {
                about.order_info = true;
            }
        }
        Some(0x1) => app.close_original_about(),
        _ => {}
    }
}

/// One About box from its template: the icon, the statics centred on
/// their top line, the version in `0x401`, the roll through `0x41f`, and
/// the two buttons — returning the id of the one pressed.
fn draw_box(
    app: &mut App,
    ui: &mut egui::Ui,
    template: &crate::dialog::Template,
    version: &str,
    lines: &[String],
    opened_at: f64,
    now: f64,
) -> Option<u16> {
    let (rect, at, caption) = crate::views::dialog_frame(ui, template);
    let scale = template.scale(rect);
    // `WM_ERASEBKGND` fills the client with the button face, and
    // `WM_CTLCOLOR` hands every static the same brush.
    ui.painter().rect_filled(rect, 0.0, BUTTON_FACE);
    let line_px = crate::dialog::DLU_Y * 8.0 * scale;
    let font = egui::FontId::proportional(line_px * 0.8);
    let mut pressed = None;

    for control in template.controls {
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
                credits(ui, where_, scale, opened_at, now, lines);
            }
            (crate::dialog::Class::Static, id, text) => {
                let text = if id == 0x401 {
                    version.to_string()
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
            (crate::dialog::Class::Button, id, _)
                if crate::views::placed_button(app, ui, at(id), &caption(id), true).clicked() =>
            {
                pressed = Some(id);
            }
            _ => {}
        }
    }
    // The timer keeps the roll moving.
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(50));
    pressed
}

/// The credits rolling through their static.
fn credits(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    scale: f32,
    opened_at: f64,
    now: f64,
    lines: &[String],
) {
    // `dyArial8`: Arial 8's line, thirteen pixels at the dialog's own size.
    let line_px = 13.0 * scale;
    #[allow(clippy::cast_possible_truncation)]
    let line = line_px.round() as i32;
    let count = i32::try_from(lines.len()).unwrap_or(i32::MAX);
    let (first, partial) = About::roll(opened_at, now, line, count);
    let painter = ui.painter().with_clip_rect(rect);
    let font = egui::FontId::proportional(line_px * 0.8);
    let mut top = rect.top() - partial as f32;
    for index in first..first + About::SHOWN {
        // Past the last line there is nothing to draw.
        if index >= count {
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

/// The ordering information box over the original's About box.
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
