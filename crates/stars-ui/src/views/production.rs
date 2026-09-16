//! The Production dialog.
//!
//! `ChangeProduction` (`10d0:0000`) opens it and `ProdCommandHandler`
//! (`10d0:1994`) runs it. Two lists side by side — the **inventory** of
//! everything this planet can build, and the planet's **queue** — with Add and
//! Remove between them, Item Up and Item Down beside the queue, and the cost of
//! whatever is selected underneath.
//!
//! See `docs/ui/production.md`.

use crate::dialog::Control;
use crate::App;
use stars_core::production::EtaMark;

/// Draw the dialog's contents, laid out from its own template.
///
/// Resource 93, `Planet Production`, 294 by 191 dialog units: the inventory
/// and the queue side by side, the buttons that move items between them in a
/// column down the middle, and a row along the foot. Everything between the
/// lists and that row is drawn rather than being controls — the cost panel
/// under each list, and the blue diamond. See [`crate::dialog::PRODUCTION`].
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    if app.production.is_none() {
        return;
    }
    app.drawn_scope = "production";
    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
    let step = App::production_step(ctrl, shift);
    let template = &crate::dialog::PRODUCTION;

    // The title bar's own text: `"Production Queue for %s"`.
    ui.label(egui::RichText::new(title(app)).strong());

    let want = template.pixels();
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(
            ui.available_width(),
            want.y
                * template.scale(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    ui.available_size(),
                )),
        ),
        egui::Sense::hover(),
    );
    let scale = template.scale(rect);
    let places = crate::dialog::unoverlapped(template);
    let at = |id: u16| -> egui::Rect {
        let (_, place) = places
            .iter()
            .find(|(other, _)| *other == id)
            .copied()
            .unwrap_or((id, (0, 0, 0, 0)));
        let (x, y, w, h) = place;
        egui::Rect::from_min_size(
            rect.min
                + egui::vec2(
                    f32::from(x) * crate::dialog::DLU_X * scale,
                    f32::from(y) * crate::dialog::DLU_Y * scale,
                ),
            egui::vec2(
                f32::from(w) * crate::dialog::DLU_X * scale,
                f32::from(h) * crate::dialog::DLU_Y * scale,
            ),
        )
    };
    let caption = |id: u16| -> String {
        template
            .control(id)
            .map_or_else(String::new, Control::label)
    };

    // The two lists. Their scroll positions are keyed by the opening, so a
    // list rolled down one time opens at the top the next, as a fresh list
    // box does.
    let opening = app.production.as_ref().map_or(0, |d| d.opening);
    list(ui, at(0x416), ("production-inventory", opening), |ui| {
        inventory(app, ui, step)
    });
    list(ui, at(0x417), ("production-queue", opening), |ui| {
        queue(app, ui, step)
    });

    // The column between them. The hints are this project's, not the game's,
    // and say what the modifiers do because the original says it in the manual
    // instead (p. 7-3).
    if button(app, ui, at(0x439), &caption(0x439)).clicked() {
        app.production_move(true);
    }
    if button(app, ui, at(0x418), &caption(0x418))
        .on_hover_text(
            "Shift for ten, Ctrl for a hundred, both for as many as possible. \
             The item goes under whichever queue row is selected.",
        )
        .clicked()
    {
        app.production_add(step);
    }
    if button(app, ui, at(0x419), &caption(0x419))
        .on_hover_text("Shift for ten, Ctrl for a hundred, both for all of them.")
        .clicked()
    {
        app.production_remove(step);
    }
    if button(app, ui, at(0x42d), &caption(0x42d))
        .on_hover_text("Empties the queue. Anything part-built loses what it has spent.")
        .clicked()
    {
        app.production_clear();
    }
    if button(app, ui, at(0x43a), &caption(0x43a)).clicked() {
        app.production_move(false);
    }

    // The cost panel under each list, and the completion line under the
    // queue's.
    let line = ui.text_style_height(&egui::TextStyle::Small);
    costs(app, ui, at(0x416), line, false);
    costs(app, ui, at(0x417), line, true);

    // The blue diamond, and the templates it reaches. Its `dyArial8` is
    // the same eight dialog units the template's controls are laid out
    // in, so it sits its seven pixels clear above the leftover checkbox
    // as in the original, rather than under it.
    templates(app, ui, rect, crate::dialog::DLU_Y * 8.0 * scale);

    // The row along the foot.
    {
        let where_ = at(0x8b);
        let mut on = app.production.as_ref().is_some_and(|d| d.no_research);
        let response = ui.put(
            where_,
            egui::Checkbox::new(&mut on, egui::RichText::new(caption(0x8b)).small()),
        );
        crate::views::record(app, ui, &caption(0x8b), &response);
        if response.changed() {
            if let Some(dialog) = app.production.as_mut() {
                dialog.no_research = on;
            }
        }
    }
    // Prev and Next write this planet's queue out and move on, which is what
    // `FinishProduction(1)` does before `SelectAdjPlanet`.
    let hint = "Shift jumps to the next planet with a starbase.";
    if button(app, ui, at(0x42e), &caption(0x42e))
        .on_hover_text(hint)
        .clicked()
    {
        app.production_step_planet(false, shift);
    }
    if button(app, ui, at(0x42f), &caption(0x42f))
        .on_hover_text(hint)
        .clicked()
    {
        app.production_step_planet(true, shift);
    }
    if button(app, ui, at(0x1), &caption(0x1)).clicked() {
        app.production_ok();
    }
    if button(app, ui, at(0x2), &caption(0x2)).clicked() {
        app.production_cancel();
    }
    // Help is `ProdCommandHandler`'s `WINHELP(HELP_CONTEXT, 0x423)`
    // (`10d0:3393`), the Production Dialog page.
    if button(app, ui, at(0x76), &caption(0x76)).clicked() {
        app.help_context(crate::help::context::PRODUCTION);
    }
}

/// One of the template's buttons, at the size the template gives it, and
/// recorded under its caption so a test can press it.
fn button(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect, text: &str) -> egui::Response {
    let caption = text.replace('&', "");
    crate::views::placed_button(app, ui, rect, &caption, true)
}

/// One of the two list boxes: a sunken frame with a scrolling list inside it.
fn list(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    id: impl std::hash::Hash,
    body: impl FnOnce(&mut egui::Ui),
) {
    ui.painter()
        .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    ui.painter().rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
    );
    let inner = rect.shrink(2.0);
    let mut child = ui.child_ui(inner, egui::Layout::top_down(egui::Align::Min), None);
    child.set_clip_rect(inner);
    // A list box's rows are a line of text each, nothing added: the
    // template's 84 dialog units are ten rows of 8-point text, and the
    // tutorial's first year needs Factory, the seventh row, in view without
    // scrolling. egui's selectable rows would pad that out to six.
    child.spacing_mut().item_spacing.y = 0.0;
    child.spacing_mut().button_padding.y = 0.0;
    child.spacing_mut().interact_size.y = child.text_style_height(&egui::TextStyle::Small);
    egui::ScrollArea::vertical()
        .id_source(id)
        .show(&mut child, body);
}

/// The panel `DrawProductionDlg` (`10d0:35dc`) draws under a list.
///
/// `Required Minerals:` in bold, then four rows a line apart — ironium,
/// boranium, germanium and then **resources**, which is `rgpszMin`'s sixth
/// entry and is drawn in black. Each label is in its own colour and its figure
/// is right-aligned, with `kT` after the three minerals and nothing after
/// resources. Under the queue's panel a line and a half further down goes
/// `"%d%% Done,   Completion "` and the item's own estimate.
fn costs(app: &mut App, ui: &mut egui::Ui, list: egui::Rect, line: f32, queue: bool) {
    let Some(cost) = app.production_costs(queue) else {
        return;
    };
    let painter = ui.painter();
    let font = egui::TextStyle::Small.resolve(ui.style());
    let text = ui.visuals().text_color();
    let mut y = list.bottom() + 4.0;
    painter.text(
        egui::pos2(list.left(), y),
        egui::Align2::LEFT_TOP,
        crate::dialog::REQUIRED_MINERALS,
        font.clone(),
        text,
    );
    // The rows are inset twenty pixels on each side of the list's own edges.
    let left = list.left() + 20.0;
    let right = list.right() - 20.0;
    let kt = ui
        .fonts(|f| f.layout_no_wrap(crate::dialog::KT.to_string(), font.clone(), text))
        .rect
        .width();
    for (index, (label, colour)) in crate::dialog::COST_ROWS.iter().enumerate() {
        y += line;
        let [r, g, b] = *colour;
        painter.text(
            egui::pos2(left, y),
            egui::Align2::LEFT_TOP,
            *label,
            font.clone(),
            egui::Color32::from_rgb(r, g, b),
        );
        painter.text(
            egui::pos2(right - kt - 2.0, y),
            egui::Align2::RIGHT_TOP,
            cost[index].to_string(),
            font.clone(),
            text,
        );
        if index < 3 {
            painter.text(
                egui::pos2(right - kt, y),
                egui::Align2::LEFT_TOP,
                crate::dialog::KT,
                font.clone(),
                text,
            );
        }
    }
    if queue {
        if let Some((done, eta)) = app.production_completion() {
            y += line * 1.5;
            painter.text(
                egui::pos2(list.left(), y),
                egui::Align2::LEFT_TOP,
                format!("{done}{}{eta}", crate::dialog::COMPLETION),
                font,
                text,
            );
        }
    }
}

/// The planet's name, as the dialog's own title bar has it.
fn title(app: &App) -> String {
    let name = app
        .production
        .as_ref()
        .map(|d| d.planet)
        .and_then(|id| {
            app.game
                .as_ref()?
                .planets
                .iter()
                .find(|p| p.id == id)
                .map(|p| {
                    p.name
                        .map_or_else(|| format!("Planet {id}"), str::to_string)
                })
        })
        .unwrap_or_default();
    format!("Production Queue for {name}")
}

fn inventory(app: &mut App, ui: &mut egui::Ui, step: i32) {
    let rows = app.production_inventory();
    if rows.is_empty() {
        ui.label(egui::RichText::new("nothing to build").weak().small());
        return;
    }
    let selected = app.production.as_ref().map_or(0, |d| d.inventory_index);
    let mut clicked = None;
    let mut added = false;
    for (index, row) in rows.iter().enumerate() {
        // An auto-build item is italic and labelled, as `FillProdSrcLB` draws
        // it; a unique item shows how many are left.
        let mut text = egui::RichText::new(if row.auto {
            format!("{} (Auto Build)", row.name)
        } else if row.unlimited() {
            row.name.clone()
        } else {
            format!("{}  ({})", row.name, row.count)
        })
        .small();
        if row.auto {
            text = text.italics();
        }
        let response = ui.selectable_label(index == selected, text);
        crate::views::record(app, ui, &row.name, &response);
        if response.clicked() {
            clicked = Some(index);
        }
        if response.double_clicked() {
            clicked = Some(index);
            added = true;
        }
    }
    if let Some(index) = clicked {
        if let Some(dialog) = app.production.as_mut() {
            dialog.inventory_index = index;
        }
    }
    if added {
        app.production_add(step);
    }
}

fn queue(app: &mut App, ui: &mut egui::Ui, step: i32) {
    let rows = app.production_queue_rows();
    let schedule = app.production_schedule();
    let selected = app.production.as_ref().and_then(|d| d.queue_index);

    // The original's first line, which is where "add to the front" lives.
    let mut clicked: Option<Option<usize>> = None;
    let mut removed = false;
    let top = ui.selectable_label(
        selected.is_none(),
        egui::RichText::new("— Top of the Queue —").small().weak(),
    );
    crate::views::record(app, ui, "Top of the Queue", &top);
    if top.clicked() {
        clicked = Some(None);
    }
    for (index, (count, name)) in rows.iter().enumerate() {
        let auto = app
            .production
            .as_ref()
            .and_then(|d| d.queue.get(index))
            .is_some_and(|e| e.is_auto());
        // An auto-build row reads "up to N"; auto alchemy reads "as needed",
        // because its count is only a placeholder.
        let label = if auto {
            if app
                .production
                .as_ref()
                .and_then(|d| d.queue.get(index))
                .is_some_and(|e| e.item == stars_core::production::item::AUTO_ALCHEMY)
            {
                format!("{name} as needed")
            } else {
                format!("{name} up to {count}")
            }
        } else {
            format!("{count}  {name}")
        };
        // The year it will be finished, and the colour that goes with it.
        // `!` is the manual's red row: the item will practically never be
        // built (p. 7-7).
        let (when, mark) = schedule
            .get(index)
            .cloned()
            .unwrap_or_else(|| (String::new(), EtaMark::Ordinary));
        let mut text = egui::RichText::new(format!("{label}   {when}")).small();
        if auto {
            text = text.italics();
        }
        text = match mark {
            EtaMark::Never => text.color(egui::Color32::from_rgb(0xff, 0x6b, 0x6b)),
            EtaMark::AllNextYear => text.color(egui::Color32::from_rgb(0x5a, 0xd6, 0x8a)),
            EtaMark::FirstNextYear => text.color(egui::Color32::from_rgb(0xa3, 0xbf, 0x5a)),
            EtaMark::Idle => text.weak(),
            EtaMark::Ordinary => text,
        };
        let response = ui.selectable_label(selected == Some(index), text);
        crate::views::record(app, ui, &label, &response);
        if response.clicked() {
            clicked = Some(Some(index));
        }
        if response.double_clicked() {
            clicked = Some(Some(index));
            removed = true;
        }
    }
    if rows.is_empty() {
        ui.label(egui::RichText::new("nothing queued").weak().small());
    }
    if let Some(index) = clicked {
        if let Some(dialog) = app.production.as_mut() {
            dialog.queue_index = index;
        }
    }
    if removed {
        app.production_remove(step);
    }
}

/// The production templates, reached from the **blue diamond**.
///
/// `DrawProductionDlg` puts a small raised blue diamond at the bottom left of
/// the dialog with `Apply or define a production template` beside it, remembers
/// its rectangle in `rcProdDiamond`, and `ProductionDlg` hit-tests that:
/// hovering shows the help cursor, a **left** click explains what to do, and a
/// **right** click brings up the menu.
fn templates(app: &mut App, ui: &mut egui::Ui, dialog: egui::Rect, line: f32) {
    /// What the original says when the diamond is left-clicked.
    const HELP: &str = "Right click on the blue diamond to apply a production template to \
                        this queue, or choose <Customize> to define a template based on the \
                        auto build items in the current queue.";

    let names: Vec<String> = (0..stars_formats::TEMPLATE_SLOTS)
        .map(|slot| app.production_template_name(slot))
        .collect();
    let usable: Vec<bool> = app
        .production_templates()
        .iter()
        .map(|t| t.queue.is_some())
        .collect();

    let mut apply = None;
    let mut customize = false;

    {
        // `DrawProductionDlg` puts it `dyArial8 * 5 / 2 + 12` up from the
        // bottom of the client area, six pixels in, `dyArial8` wide and
        // `dyArial8 | 1` tall.
        let rect = crate::dialog::diamond(dialog, line);
        let response = ui.interact(
            rect,
            ui.id().with("production-diamond"),
            egui::Sense::click(),
        );
        diamond(ui, rect);
        crate::views::record(app, ui, "blue diamond", &response);
        // And its caption, four pixels past its right edge.
        ui.painter().text(
            egui::pos2(rect.right() + 4.0, rect.top()),
            egui::Align2::LEFT_TOP,
            crate::dialog::TEMPLATE_HINT,
            egui::TextStyle::Small.resolve(ui.style()),
            ui.visuals().text_color(),
        );
        // The original swaps in the arrow-and-question-mark cursor over it.
        let response = response.on_hover_cursor(egui::CursorIcon::Help);

        // A left click only tells you to use the right button.
        if response.clicked() {
            ui.memory_mut(|m| m.open_popup(egui::Id::new("diamond-help")));
        }
        egui::popup_below_widget(
            ui,
            egui::Id::new("diamond-help"),
            &response,
            egui::PopupCloseBehavior::CloseOnClick,
            |ui| {
                ui.set_max_width(320.0);
                ui.label(egui::RichText::new(HELP).small());
            },
        );

        // The right button brings up the menu: every template that has
        // something in it, then <Customize>.
        // The entries are recorded as they are drawn, so a test can pick
        // one the way page 32 says to.
        response.context_menu(|ui| {
            for (slot, name) in names.iter().enumerate() {
                if !usable.get(slot).copied().unwrap_or(false) {
                    continue;
                }
                let entry = ui.button(name);
                crate::views::record(app, ui, name, &entry);
                if entry.clicked() {
                    apply = Some(slot);
                    ui.close_menu();
                }
            }
            ui.separator();
            let entry = ui.button("<Customize>");
            crate::views::record(app, ui, "Customize", &entry);
            if entry.clicked() {
                customize = true;
                ui.close_menu();
            }
        });
    }

    if let Some(slot) = apply {
        app.production_apply_template(slot);
    }
    if customize {
        app.production_customize_open(0);
    }
    if app.production_customize_slot().is_some() {
        customize_panel(app, ui);
    }
}

/// The diamond itself: a small raised blue lozenge.
///
/// `DrawDiamond` (`1028:4b60`) walks it scanline by scanline, laying a
/// highlight along the upper-left edges and a shadow along the lower-right
/// before filling the middle. This paints the same three pieces as polygons.
fn diamond(ui: &egui::Ui, rect: egui::Rect) {
    let painter = ui.painter();
    let c = rect.center();
    let w = rect.width() / 2.0;
    let top = egui::pos2(c.x, rect.top());
    let bottom = egui::pos2(c.x, rect.bottom());
    let left = egui::pos2(c.x - w, c.y);
    let right = egui::pos2(c.x + w, c.y);

    painter.add(egui::Shape::convex_polygon(
        vec![top, right, bottom, left],
        egui::Color32::from_rgb(0x30, 0x60, 0xff),
        egui::Stroke::NONE,
    ));
    // The highlight runs up the left side, the shadow down the right.
    painter.line_segment(
        [left, top],
        egui::Stroke::new(1.0_f32, ui.visuals().widgets.inactive.fg_stroke.color),
    );
    painter.line_segment(
        [top, right],
        egui::Stroke::new(1.0_f32, ui.visuals().widgets.inactive.fg_stroke.color),
    );
    painter.line_segment(
        [right, bottom],
        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(0x40)),
    );
    painter.line_segment(
        [bottom, left],
        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(0x40)),
    );
}

/// The `<Customize>` dialog (`ZipProdDlg`, `10d0:5490`), drawn inline.
fn customize_panel(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut slot) = app.production_customize_slot() else {
        return;
    };
    // Its buttons are recorded under a scope of their own, since the
    // production dialog behind it has an OK too.
    let was = app.drawn_scope;
    app.drawn_scope = "customize";
    let names: Vec<String> = (0..stars_formats::TEMPLATE_SLOTS)
        .map(|s| app.production_template_name(s))
        .collect();

    // `ZipProdDlg` is a dialog of its own over the production dialog, so
    // this goes on a layer above it: nothing underneath — the leftover
    // checkbox, the dialog's own OK — can take a click meant for it.
    let over = ui.min_rect().left_bottom();
    egui::Area::new(egui::Id::new("production-customize"))
        .order(egui::Order::Foreground)
        .fixed_pos(over)
        .show(ui.ctx(), |ui| {
            egui::Frame::group(ui.style())
                .fill(ui.visuals().window_fill)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("Customize Production Templates")
                            .small()
                            .strong(),
                    );
                    ui.horizontal(|ui| {
                        for (index, name) in names.iter().enumerate() {
                            let radio =
                                ui.radio_value(&mut slot, index, egui::RichText::new(name).small());
                            crate::views::record(app, ui, name, &radio);
                        }
                    });
                    app.production_customize_select(slot);

                    // What the chosen template holds, and its research setting.
                    let templates = app.production_templates();
                    let queue = templates.get(slot).and_then(|t| t.queue.as_ref());
                    for line in stars_core::production::template_lines(queue) {
                        ui.label(egui::RichText::new(line).small());
                    }
                    ui.label(
                        egui::RichText::new(if queue.is_some_and(|q| q.no_research) {
                            "Don't contribute to research"
                        } else {
                            "Contribute to research"
                        })
                        .small()
                        .weak(),
                    );

                    let editable = app.production_template_editable(slot);
                    ui.horizontal(|ui| {
                        let import = ui
                            .button(egui::RichText::new("Import").small())
                            .on_hover_text(
                                "Takes the auto-build items out of this planet's queue, in order, \
                     and makes them this template.",
                            );
                        crate::views::record(app, ui, "Import", &import);
                        if import.clicked() {
                            let name = app.production_template_name(slot);
                            app.production_import_template(slot, &name);
                        }
                        let delete = ui
                            .add_enabled(
                                editable,
                                egui::Button::new(egui::RichText::new("Delete").small()),
                            )
                            .on_disabled_hover_text(
                                "The default template cannot be deleted. To empty it, import an \
                     empty queue over it.",
                            );
                        crate::views::record(app, ui, "Delete", &delete);
                        if delete.clicked() {
                            app.production_delete_template(slot);
                        }
                        let ok = ui.button(egui::RichText::new("OK").small());
                        crate::views::record(app, ui, "OK", &ok);
                        if ok.clicked() {
                            app.production_customize_close(true);
                        }
                        let cancel = ui
                            .button(egui::RichText::new("Cancel").small())
                            .on_hover_text("Puts every template back as it was.");
                        crate::views::record(app, ui, "Cancel", &cancel);
                        if cancel.clicked() {
                            app.production_customize_close(false);
                        }
                        // `ZipProdDlg`'s Help: `0x452` (`10d0:5d8e`).
                        let help = ui.button(egui::RichText::new("Help").small());
                        crate::views::record(app, ui, "Help", &help);
                        if help.clicked() {
                            app.help_context(crate::help::context::PRODUCTION_TEMPLATES);
                        }
                    });

                    // Renaming, for a slot that has one to rename.
                    if editable {
                        let mut name = app.production_template_name(slot);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Name").small());
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut name)
                                        .desired_width(160.0)
                                        .char_limit(stars_formats::TEMPLATE_NAME_MAX),
                                )
                                .changed()
                            {
                                app.production_rename_template(slot, &name);
                            }
                        });
                    }
                });
        });
    app.drawn_scope = was;
}
