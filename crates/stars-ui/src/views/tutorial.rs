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
    let mut area = crate::dialog::tutor_text_area(rect, hide.top(), line);

    // The game's own words when a copy of the original is to hand, and this
    // project's retelling of the page when it is not.
    let paragraphs = app.tutor_page();
    let bold = app
        .tutor
        .as_ref()
        .and_then(crate::tutorial::Tutor::bold_line);

    // The original's panel was cut to fit its own Arial 8, and a page that
    // was written to fill it overruns in any other face. Rather than lose
    // the end of the page, measure it first and let the panel — and the
    // buttons under it — come down by the difference.
    let needed = paragraphs.as_deref().map_or(0.0, |paragraphs| {
        flow(ui, None, area, line, paragraphs, bold)
    });
    let extra = (needed - area.height()).max(0.0).ceil();
    if extra > 0.0 {
        ui.allocate_exact_size(egui::vec2(rect.width(), extra), egui::Sense::hover());
        area.max.y += extra;
    }
    let down = egui::vec2(0.0, extra);

    let outer = area.expand((line / 2.0).floor());
    sunken(ui.painter(), outer);
    ui.painter().rect_filled(area, 0.0, face());
    if let Some(paragraphs) = paragraphs.as_deref() {
        let painter = ui.painter().with_clip_rect(area);
        flow(ui, Some(&painter), area, line, paragraphs, bold);
    }

    // `Hide`, `Hint` and `Panic!`.
    if crate::views::dialog_button(ui, at(0x2).translate(down), &caption(0x2), true).clicked() {
        app.tutor_notice = app.hide_tutor().map(str::to_string);
    }
    // Hint opens the page's own help topic — `TutorDlg` hands `WINHELP`
    // `tutor.idh` (`10f8:01bd`), which the page checks set as they work
    // out what is wanted. The checks here do not set one yet, so a page
    // with none opens the guide's contents rather than nothing.
    let help = app.tutor.as_ref().map_or(0, |t| t.help);
    if crate::views::dialog_button(ui, at(0x76).translate(down), &caption(0x76), true).clicked() {
        if help == 0 {
            app.help_contents();
        } else {
            app.help_context(u32::from(help));
        }
    }
    if crate::views::dialog_button(ui, at(0x9c7).translate(down), &caption(0x9c7), true).clicked() {
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

/// Lay the page's paragraphs into the panel, and say how tall they came to.
///
/// `DrawTutorText` flows them: a paragraph whose first character is an
/// **upper-case letter** starts a new one, with half a line of air above it,
/// and anything else — the ones beginning with a space — runs straight on
/// from the last. It stops at the first paragraph one character long, which
/// is how a short page ends.
///
/// The emphasised paragraph is not drawn bold. The original swaps the text
/// and background colours for it, so it comes out in **reverse video**.
///
/// With no painter this only measures, which is how the panel learns how
/// much room the page wants before it is drawn.
fn flow(
    ui: &egui::Ui,
    painter: Option<&egui::Painter>,
    area: egui::Rect,
    line: f32,
    paragraphs: &[String],
    bold: Option<usize>,
) -> f32 {
    let font = egui::FontId::proportional(line * 0.8);
    let (mut x, mut y) = (area.left(), area.top());
    let mut bottom = area.top();

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
            let width = ui.fonts(|fonts| {
                fonts
                    .layout_no_wrap(word.to_string(), font.clone(), egui::Color32::PLACEHOLDER)
                    .rect
                    .width()
            });
            if x + width > area.right() && x > area.left() {
                x = area.left();
                y += line;
            }
            bottom = bottom.max(y + line);
            if let Some(painter) = painter {
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
            }
            x += width;
        }
    }
    bottom - area.top()
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

/// The **halo**: a pulsing ring around whatever the tutorial would like
/// pressed next, painted over everything once the panes have drawn.
///
/// This project's own aid, not the original's, whose only pointer is the
/// emboldened paragraph. [`App::tutor_target`] says what to ring — a button
/// or row by the name a pane recorded it under, or a point on the map — and
/// this finds where that was drawn this frame and rings it. Nothing is
/// drawn while the tutor window is hidden, or when there is no target.
///
/// Returns where the halo went, for a test to check.
pub fn halo(app: &App, ctx: &egui::Context) -> Option<egui::Rect> {
    if app.tutor.as_ref().is_none_or(|t| t.hidden || t.finished) {
        return None;
    }
    let target = app.tutor_target()?;
    let rect = match target {
        crate::app::TutorTarget::Widget { scope, label } => {
            let widget = app.drawn_button(scope, &label)?;
            if !widget.visible {
                return None;
            }
            widget.rect
        }
        crate::app::TutorTarget::Map(at) => {
            let map = app.map_frame?;
            let centre = map.to_screen(at.x, at.y);
            if !map.rect.contains(centre) {
                return None;
            }
            egui::Rect::from_center_size(centre, egui::vec2(18.0, 18.0))
        }
    };

    // A breath every second and a half: the ring swells and fades, and the
    // frame after this one is asked for so it keeps breathing.
    let t = ctx.input(|i| i.time);
    #[allow(clippy::cast_possible_truncation)]
    let pulse = ((t * std::f64::consts::TAU / 1.5).sin() * 0.5 + 0.5) as f32;
    let grow = 3.0 + pulse * 5.0;
    let alpha = (110.0 + pulse * 130.0) as u8;
    let colour = egui::Color32::from_rgba_unmultiplied(0xff, 0xb0, 0x00, alpha);
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("tutor-halo"),
    ));
    let ring = rect.expand(grow);
    painter.rect_stroke(ring, 4.0, egui::Stroke::new(2.5_f32, colour));
    painter.rect_stroke(
        ring.expand(3.0),
        6.0,
        egui::Stroke::new(1.0_f32, colour.gamma_multiply(0.5)),
    );
    ctx.request_repaint_after(std::time::Duration::from_millis(33));
    Some(ring)
}
