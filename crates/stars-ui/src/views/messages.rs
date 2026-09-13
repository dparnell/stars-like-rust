//! The message pane.
//!
//! The original shows the year's news **one message at a time** in a pane of
//! its own, below the planet pane: a title bar saying which message of how
//! many, the message itself, and three buttons — Prev, Goto and Next
//! (`MessageWndProc`, `1030:5c92`).
//!
//! Two of the title bar's decorations are controls (`HtMsgBox`, `1030:7d8c`):
//! the square at the **left** silences the kind of message being shown, and the
//! square at the **right** shows the silenced ones anyway. The right one is
//! drawn only when something is actually hidden, which is the same condition
//! the original draws its own under.
//!
//! The keys are the original's: Up and Down step, Home and End jump to the
//! ends, Enter is Goto, `+` filters the message being shown and `-` toggles
//! showing the filtered ones.
//!
//! What is not the original: its title bar carries little bitmaps where this
//! has short labelled buttons, and its "FILTERED" watermark is drawn diagonally
//! across the message text where this puts it above.

use crate::App;

/// One of the title bar's two controls: the game's own little bitmap when
/// a copy of the game is there to read it from, and a short label when
/// there is not.
///
/// `DecorateMsgTitleBar` (`1030:799c`) blits each as a pair — the one-bit
/// mask with `SRCAND` and the colour with `SRCPAINT` — out of two strips
/// packed separately, which is why the glyph carries a row in each.
fn decoration(
    app: &mut App,
    ui: &mut egui::Ui,
    glyph: crate::message::Glyph,
    label: &str,
) -> egui::Response {
    use stars_formats::resources::Name;

    let Some(mask_y) = glyph.mask_y else {
        return ui.small_button(label);
    };
    let size = egui::vec2(glyph.size.0 as f32, glyph.size.1 as f32);
    let ctx = ui.ctx().clone();
    let picture = app.art.as_mut().and_then(|art| {
        art.sprite_masked_between(
            &ctx,
            &Name::Id(crate::message::COLOUR_SHEET),
            &Name::Id(crate::message::MASK_SHEET),
            (0, glyph.colour_y),
            (0, mask_y),
            glyph.size,
            size,
        )
        .map(|image| image.sense(egui::Sense::click()))
    });
    let response = match picture {
        Some(image) => ui.add(image),
        None => ui.small_button(label),
    };
    // Recorded under its plain name — `filter`, `show`, `view filtered`,
    // `hide filtered` — whether it came out as the game's glyph or as a
    // button, so a test can press the blue check mark page 13 names.
    crate::views::record(app, ui, label, &response);
    response
}

/// Draw the message pane.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    app.drawn_scope = "messages";
    keys(app, ui);

    let filtered_here = app
        .current_message()
        .is_some_and(|m| app.message_filter().hidden(m.id));
    let has_filtered = app.has_filtered_messages();

    // --- the title bar, and the two controls in it
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if app.current_message().is_some() {
                    // `+` both ways, as in the original.
                    let (label, hint) = if filtered_here {
                        ("show", "stop filtering messages like this one  (+)")
                    } else {
                        ("filter", "stop showing messages like this one  (+)")
                    };
                    let glyph = if filtered_here {
                        crate::message::FILTER_ON
                    } else {
                        crate::message::FILTER
                    };
                    if decoration(app, ui, glyph, label)
                        .on_hover_text(hint)
                        .clicked()
                    {
                        app.toggle_message_filter();
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if has_filtered {
                        let label = if app.view_filtered {
                            "hide filtered"
                        } else {
                            "view filtered"
                        };
                        let glyph = if app.view_filtered {
                            crate::message::REVEAL_ON
                        } else {
                            crate::message::REVEAL
                        };
                        if decoration(app, ui, glyph, label)
                            .on_hover_text("show the messages you have filtered  (-)")
                            .clicked()
                        {
                            app.toggle_view_filtered();
                        }
                    }
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.label(app.message_title());
                    });
                });
            });
        });

    // --- the message itself
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.set_min_height(48.0);
            let body = ui.vertical(|ui| {
                egui::ScrollArea::vertical()
                    .max_height(72.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.label(app.message_body());
                    });
            });
            // A filtered message on screen because the player asked to see the
            // filtered ones carries **FILTERED** written corner to corner
            // across it.
            if filtered_here && app.view_filtered {
                watermark(ui, body.response.rect);
            }
        });

    // --- Prev, Goto, Next
    // Recorded under the pane's own scope, so a test can tell the message
    // pane's Prev and Next from the fleet tile's.
    ui.horizontal(|ui| {
        let has_previous = app.message_previous(false).is_some();
        let has_next = app.message_next(false).is_some();
        if crate::views::flow_button(app, ui, "Prev", has_previous).clicked() {
            app.show_previous_message();
        }
        let goto = app.message_goto() != stars_core::message::Goto::None;
        let label = app.message_goto_label();
        if crate::views::flow_button(app, ui, label, goto).clicked() {
            app.message_goto_follow();
        }
        if crate::views::flow_button(app, ui, "Next", has_next).clicked() {
            app.show_next_message();
        }
    });
}

/// The keys the original's pane answers to.
///
/// In the original the arrows belong to the message window, while `+`, `-` and
/// Enter are forwarded to it from the frame wherever the player is
/// (`mdi.c:1880`), so they work without clicking on the pane first. Here they
/// all work whenever no widget has taken the keyboard — which is what stops a
/// `-` typed into a text box from hiding messages.
fn keys(app: &mut App, ui: &egui::Ui) {
    if ui.memory(|m| m.focused()).is_some() {
        return;
    }
    ui.input(|i| {
        if i.key_pressed(egui::Key::ArrowDown) {
            app.show_next_message();
        }
        if i.key_pressed(egui::Key::ArrowUp) {
            app.show_previous_message();
        }
        if i.key_pressed(egui::Key::Home) {
            app.show_first_message();
        }
        if i.key_pressed(egui::Key::End) {
            app.show_last_message();
        }
        if i.key_pressed(egui::Key::Enter) {
            app.message_goto_follow();
        }
        if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
            app.toggle_message_filter();
        }
        if i.key_pressed(egui::Key::Minus) {
            app.toggle_view_filtered();
        }
    });
}

/// The **FILTERED** watermark, written corner to corner across the message.
///
/// `DiaganolTextOut` builds a `LOGFONT` at the heaviest weight there is,
/// points its escapement down the rectangle's own diagonal, and shrinks the
/// size until the text fits with eight pixels to spare each way — giving up
/// entirely on a rectangle under ten pixels. See [`crate::message`].
fn watermark(ui: &egui::Ui, rect: egui::Rect) {
    use crate::message;

    if rect.width() < message::WATERMARK_MIN || rect.height() < message::WATERMARK_MIN {
        return;
    }
    let font = egui::TextStyle::Body.resolve(ui.style());
    let colour = ui.visuals().weak_text_color();
    let galley = ui.fonts(|f| f.layout_no_wrap("FILTERED".to_string(), font.clone(), colour));
    let scale = message::watermark_scale(rect.size(), galley.rect.size());
    if scale <= 0.0 {
        return;
    }
    // The original shrinks the font itself; laying the text out once at the
    // size the fit allows comes to the same place.
    let sized = egui::FontId::new((font.size * scale).max(4.0), font.family.clone());
    let galley = ui.fonts(|f| f.layout_no_wrap("FILTERED".to_string(), sized, colour));
    let angle = message::watermark_angle(rect.size());
    // Turned about its own middle and set down in the middle of the message.
    let half = galley.rect.size() / 2.0;
    let (sin, cos) = angle.sin_cos();
    let offset = egui::vec2(half.x * cos - half.y * sin, half.x * sin + half.y * cos);
    let shape =
        egui::epaint::TextShape::new(rect.center() - offset, galley, colour).with_angle(angle);
    ui.painter().with_clip_rect(rect).add(shape);
}
