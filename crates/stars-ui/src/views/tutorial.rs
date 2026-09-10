//! The tutorial's own window.
//!
//! `TutorDlg` (`10f8:0000`) over dialog resource 2502 — three buttons along
//! the foot and, above them, a panel `DrawTutorText` (`10f8:03c0`) paints by
//! hand. See `docs/ui/tutorial.md`.

use crate::App;

/// Draw the tutor window's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some(page) = app.tutor.as_ref().map(crate::tutorial::Tutor::page) else {
        return;
    };
    let template = &crate::dialog::TUTOR;
    let (rect, at, caption) = crate::views::dialog_frame(ui, template);
    let line = at(0x2).height() / 12.0 * 8.0;

    // The panel, and the sunken frame round it: shadow along the top and
    // left, highlight along the bottom and right — the raised groove of the
    // rest of the game turned inside out, which is what makes it a well for
    // text rather than a box round a group.
    let hide = at(0x2);
    let area = crate::dialog::tutor_text_area(rect, hide.top(), line);
    let outer = area.expand((line / 2.0).floor());
    sunken(ui.painter(), outer);
    ui.painter().rect_filled(area, 0.0, face());

    match app.tutor_page() {
        Some(paragraphs) => {
            let bold = app
                .tutor
                .as_ref()
                .and_then(crate::tutorial::Tutor::bold_line);
            paint(ui, area, line, &paragraphs, bold);
        }
        None => {
            let mut child = ui.child_ui(
                area.shrink(4.0),
                egui::Layout::top_down(egui::Align::Min),
                None,
            );
            child.set_clip_rect(area);
            child.label(
                egui::RichText::new(
                    "The tutorial's words are the game's own and are read from a copy of \
                     the original. File (Use the original's pictures…) is where to point \
                     this at one.",
                )
                .small()
                .weak(),
            );
        }
    }

    // `Hide`, `Hint` and `Panic!`.
    if crate::views::dialog_button(ui, at(0x2), &caption(0x2), true).clicked() {
        app.tutor_notice = app.hide_tutor().map(str::to_string);
    }
    // Hint opens the page's help topic, which this project has no help file
    // for, so it says which topic it would have opened.
    let help = app.tutor.as_ref().map_or(0, |t| t.help);
    crate::views::dialog_button(ui, at(0x76), &caption(0x76), false).on_disabled_hover_text(
        format!("Help topic {help:#x}, which this project has no file for."),
    );
    if crate::views::dialog_button(ui, at(0x9c7), &caption(0x9c7), true).clicked() {
        app.tutor_panic = true;
    }

    // The caption carries the page number: string `0x1e2`, which the
    // original fills in with `wsprintf`.
    ui.painter().text(
        rect.min + egui::vec2(6.0, 2.0),
        egui::Align2::LEFT_TOP,
        format!(
            "Stars! Tutor - Page {page} of {}",
            stars_formats::tutorial::PAGES
        ),
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().text_color(),
    );

    if app.tutor_panic {
        panic_dialog(app, ui);
    }
}

/// Lay the page's paragraphs into the panel.
///
/// `DrawTutorText` flows them: a paragraph whose first character is an
/// **upper-case letter** starts a new one, with half a line of air above it,
/// and anything else — the ones beginning with a space — runs straight on
/// from the last. It stops at the first paragraph one character long, which
/// is how a short page ends.
///
/// The emphasised paragraph is not drawn bold. The original swaps the text
/// and background colours for it, so it comes out in **reverse video**.
fn paint(
    ui: &mut egui::Ui,
    area: egui::Rect,
    line: f32,
    paragraphs: &[String],
    bold: Option<usize>,
) {
    let font = egui::FontId::proportional(line * 0.8);
    let painter = ui.painter().with_clip_rect(area);
    let (mut x, mut y) = (area.left(), area.top());

    for (index, text) in paragraphs.iter().enumerate() {
        // A one-character paragraph ends the page.
        if text.chars().count() <= 1 {
            break;
        }
        let fresh = text.chars().next().is_some_and(|c| c.is_ascii_uppercase());
        if fresh && index != 0 {
            y += (line / 2.0).floor() + line;
            x = area.left();
        }
        let inverted = bold == Some(index);

        // Word wrap, continuing from wherever the last paragraph left off.
        for word in text.split_inclusive(' ') {
            let galley =
                painter.layout_no_wrap(word.to_string(), font.clone(), egui::Color32::PLACEHOLDER);
            let width = galley.rect.width();
            if x + width > area.right() && x > area.left() {
                x = area.left();
                y += line;
            }
            if y + line > area.bottom() {
                return;
            }
            let at = egui::pos2(x, y);
            if inverted {
                painter.rect_filled(
                    egui::Rect::from_min_size(at, egui::vec2(width, line)),
                    0.0,
                    text_colour(),
                );
            }
            painter.galley(
                at,
                painter.layout_no_wrap(
                    word.to_string(),
                    font.clone(),
                    if inverted { face() } else { text_colour() },
                ),
                text_colour(),
            );
            x += width;
        }
    }
}

/// `Something's Really Gone Wrong!` — what **Panic!** opens.
fn panic_dialog(app: &mut App, ui: &mut egui::Ui) {
    let template = &crate::dialog::TUTOR_PANIC;
    let mut open = true;
    egui::Window::new(template.caption)
        .open(&mut open)
        .resizable(false)
        .default_width(template.pixels().x)
        .show(ui.ctx(), |ui| {
            let (_, at, caption) = crate::views::dialog_frame(ui, template);
            // The three explanations are this project's wording, not the
            // original's.
            for (button, static_, note) in [
                (
                    0x9c9u16,
                    0xffffu16,
                    "Start the year again, throwing away everything you have changed.",
                ),
                (
                    0x9ca,
                    0xfffe,
                    "Start the year again and do the page's work for you, so you can see \
                     what should have happened.",
                ),
                (
                    0x76,
                    0xfffd,
                    "Open help on whatever the page is waiting for — the same as the \
                     Hint button.",
                ),
            ] {
                // Neither of the two restarts is wired up: this project has
                // no way to replay a year it has already generated.
                crate::views::dialog_button(ui, at(button), &caption(button), false)
                    .on_disabled_hover_text("Not built: a generated year cannot be replayed yet.");
                let where_ = at(static_);
                let mut child = child_at(ui, where_);
                child.label(egui::RichText::new(note).small().weak());
            }
            if crate::views::dialog_button(ui, at(0x2), &caption(0x2), true).clicked() {
                app.tutor_panic = false;
            }
        });
    if !open {
        app.tutor_panic = false;
    }
}

/// A clipped child at a placed control's rectangle.
fn child_at(ui: &mut egui::Ui, rect: egui::Rect) -> egui::Ui {
    let mut child = ui.child_ui(rect, egui::Layout::top_down(egui::Align::Min), None);
    child.set_clip_rect(rect);
    child
}

/// The sunken frame `DrawTutorText` draws round its panel: shadow above and
/// left, highlight below and right.
fn sunken(painter: &egui::Painter, rect: egui::Rect) {
    let shadow = egui::Color32::from_rgb(
        crate::toolbar::SHADOW[0],
        crate::toolbar::SHADOW[1],
        crate::toolbar::SHADOW[2],
    );
    let hilite = egui::Color32::from_rgb(
        crate::toolbar::HILITE[0],
        crate::toolbar::HILITE[1],
        crate::toolbar::HILITE[2],
    );
    let bar = |x: f32, y: f32, w: f32, h: f32, c: egui::Color32| {
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h)),
            0.0,
            c,
        );
    };
    bar(rect.left(), rect.top(), rect.width(), 1.0, shadow);
    bar(rect.left(), rect.top(), 1.0, rect.height(), shadow);
    bar(rect.left(), rect.bottom() - 1.0, rect.width(), 1.0, hilite);
    bar(rect.right() - 1.0, rect.top(), 1.0, rect.height(), hilite);
}

/// `COLOR_BTNFACE`.
fn face() -> egui::Color32 {
    egui::Color32::from_rgb(
        crate::toolbar::FACE[0],
        crate::toolbar::FACE[1],
        crate::toolbar::FACE[2],
    )
}

/// `COLOR_BTNTEXT`.
fn text_colour() -> egui::Color32 {
    egui::Color32::BLACK
}
