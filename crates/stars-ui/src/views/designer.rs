//! The Ship and Starbase Designer.
//!
//! `ShipBuilder` (`10c8:008c`) opens dialog 0x5c and `SlotDlg` (`10c8:0550`)
//! runs it. One window with two faces:
//!
//! * a **browser** — two radio groups, a dropdown, and Copy / Edit / Delete —
//!   over the player's designs, the hulls they have researched, or the designs
//!   they have seen an opponent fly;
//! * an **editor**, reached by Copy or Edit, with the parts list on the left
//!   and the hull's schematic on the right, and OK / Cancel at the bottom.
//!
//! The schematic is not drawn by code: each hull carries a table of cells
//! (`HULDEF.rgbrc`) saying where its slots sit on a grid, and the designer
//! lays them out from that (`UpdateSlotGlobals`, `10c8:6528`). This view uses
//! the same table, so every hull comes out in its own shape.
//!
//! See `docs/ui/ship-design.md` for what is and is not reproduced.

use crate::{App, DesignView, DesignerDrag, PartRow};

/// One cell of the schematic grid, in points, where the grid has to fit a
/// pane that was not laid out for it — the browser and the popup. The
/// original's is 32 pixels and a slot is two cells square; the editor,
/// laid out from `ptslotGlob` like the original's, uses the original's
/// size scaled with the frame.
pub(crate) const CELL: f32 = 17.0;

/// Draw the designer's contents into whatever the shell gives it.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    if app.designer.is_none() {
        return;
    }
    app.drawn_scope = "designer";
    let editing = app.designer.as_ref().is_some_and(|d| d.editing.is_some());

    if editing {
        editor(app, ui);
    } else {
        browser(app, ui);
    }
}

// --- browsing ------------------------------------------------------------

/// The designer's client: 610 by 450 (`ShipBuilder`, `1020:4e14`), scaled
/// with the width the window has. The template's controls are placed at
/// their dialog-unit positions scaled the same way, and everything the
/// code places from `ptslotGlob` at its pixel offset.
struct Client {
    rect: egui::Rect,
    k: f32,
}

impl Client {
    fn allocate(ui: &mut egui::Ui) -> Self {
        let (client_w, client_h) = crate::dialog::SLOT_CLIENT;
        let k = ui.available_width() / client_w;
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(client_w * k, client_h * k), egui::Sense::hover());
        Client { rect, k }
    }

    /// A point of the client, in pixels of the original's.
    fn px(&self, x: f32, y: f32) -> egui::Pos2 {
        egui::pos2(self.rect.left() + x * self.k, self.rect.top() + y * self.k)
    }

    /// A rectangle of the client, in pixels of the original's.
    fn at(&self, x: f32, y: f32, w: f32, h: f32) -> egui::Rect {
        egui::Rect::from_min_size(self.px(x, y), egui::vec2(w * self.k, h * self.k))
    }

    /// One of the template's controls, where the template puts it.
    fn control(&self, id: u16) -> egui::Rect {
        crate::dialog::DESIGNER
            .control(id)
            .map_or(egui::Rect::NOTHING, |control| {
                crate::dialog::place(self.rect.min, self.k, control.at)
            })
    }

    /// A line and a half of Arial 8, the height of the foot's buttons.
    fn button_h(&self) -> f32 {
        (20.0 * self.k).clamp(16.0, 24.0)
    }

    /// One of the three buttons along the foot — OK, Cancel and Help at
    /// 610 − 226, − 148 and − 74, 68 wide (`SlotDlg`'s `WM_INITDIALOG`).
    fn foot_button(&self, x: f32) -> egui::Rect {
        let h = self.button_h();
        egui::Rect::from_min_size(
            self.px(x, crate::dialog::SLOT_CLIENT.1 - 6.0) - egui::vec2(0.0, h),
            egui::vec2(68.0 * self.k, h),
        )
    }
}

fn caption(id: u16) -> String {
    crate::dialog::DESIGNER
        .control(id)
        .map_or_else(String::new, crate::dialog::Control::label)
}

/// A 3-D frame round a group of controls with its caption cut into the top
/// edge, as `SlotDlg`'s `WM_PAINT` draws round the two radio groups:
/// `ExpandRc(dyArial8, dyArial8 / 2)` round the first and last control,
/// `Draw3dFrame`, and the caption at `left + 8`, half a line up.
fn group_frame(ui: &egui::Ui, first: egui::Rect, last: egui::Rect, k: f32, caption: &str) {
    let rect = first.union(last).expand2(egui::vec2(13.0 * k, 6.5 * k));
    let painter = ui.painter();
    let lit = egui::Color32::from_rgb(0xff, 0xff, 0xff);
    let dark = egui::Color32::from_rgb(0x80, 0x80, 0x80);
    painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0_f32, dark));
    painter.rect_stroke(
        rect.translate(egui::vec2(1.0, 1.0)),
        0.0,
        egui::Stroke::new(1.0_f32, lit),
    );
    let font = egui::FontId::proportional((11.0 * k).clamp(9.0, 13.0));
    let colour = ui.visuals().strong_text_color();
    let galley = painter.layout_no_wrap(caption.to_string(), font, colour);
    let at = egui::pos2(rect.left() + 8.0 * k, rect.top() - galley.size().y / 2.0);
    painter.rect_filled(
        egui::Rect::from_min_size(at, galley.size()).expand2(egui::vec2(2.0, 0.0)),
        0.0,
        ui.visuals().window_fill(),
    );
    painter.galley(at, galley, colour);
}

fn browser(app: &mut App, ui: &mut egui::Ui) {
    // Components view is the odd one out: there is no design to show, so the
    // parts list stands where the schematic was and the dropdown becomes
    // the category filter (`DrawSlotDlg` returns early on `mdBuildComp`).
    let components = app
        .designer
        .as_ref()
        .is_some_and(|d| d.view == DesignView::Components);

    let client = Client::allocate(ui);
    let rect = client.rect;
    let k = client.k;

    // The left column: the two radio groups in their frames and the three
    // buttons, each where the template puts it.
    group_frame(
        ui,
        client.control(0x810),
        client.control(0x811),
        k,
        "Design",
    );
    group_frame(ui, client.control(0x812), client.control(0x815), k, "View");
    radios(app, ui, &|id| client.control(id), &caption);
    let can_copy = app.designer_can_copy();
    let can_edit = app.designer_can_edit();
    let can_delete = app.designer_can_delete();
    if crate::views::placed_button(app, ui, client.control(0x816), &caption(0x816), can_copy)
        .clicked()
    {
        app.designer_copy();
    }
    if crate::views::placed_button(app, ui, client.control(0x817), &caption(0x817), can_delete)
        .clicked()
    {
        match app.designer_delete_warning() {
            Some(question) => {
                if let Some(designer) = app.designer.as_mut() {
                    designer.confirm = Some(question);
                }
            }
            None => app.designer_delete(),
        }
    }
    if crate::views::placed_button(app, ui, client.control(0x818), &caption(0x818), can_edit)
        .clicked()
    {
        app.designer_edit();
    }

    // The dropdown at (610 − 264, 8), 240 wide — eight further right in
    // Components view, where it is the category filter over the parts
    // list at (610 − 256, 32), 240 by 266.
    let tall = client.button_h();
    let done_rect = client.foot_button(462.0);
    let foot = done_rect.top() - 6.0;
    if components {
        let dd = egui::Rect::from_min_size(client.px(354.0, 8.0), egui::vec2(240.0 * k, tall));
        filter_dropdown(app, ui, dd);
        let list = client.at(354.0, 32.0, 240.0, 266.0).intersect(rect);
        let mut child = ui.child_ui(list, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(list);
        egui::Frame::default()
            .inner_margin(4.0)
            .show(&mut child, |ui| {
                egui::ScrollArea::vertical()
                    .id_source("designer-components")
                    .show(ui, |ui| parts_list(app, ui, false));
            });
    } else {
        let dd = egui::Rect::from_min_size(client.px(346.0, 8.0), egui::vec2(240.0 * k, tall));
        dropdown(app, ui, dd);

        // The hull's picture at (610 − 338, 6), the slot grid from
        // (610 − 330, 32), and under the grid the plaque at its origin
        // plus (0x102, 0x111); the numbers panel fills the left half from
        // `yBuildInfoSum` (340) down.
        let picture_rect = client.at(272.0, 6.0, 66.0, 90.0);
        if picture_rect.height() > 8.0 {
            let mut child =
                ui.child_ui(picture_rect, egui::Layout::top_down(egui::Align::Min), None);
            child.set_clip_rect(picture_rect);
            picture(app, &mut child, false, false);
        }
        let grid =
            egui::Rect::from_min_max(client.px(280.0, 32.0), egui::pos2(rect.right() - 4.0, foot));
        if grid.width() > 40.0 && grid.height() > 8.0 {
            let mut child = ui.child_ui(grid, egui::Layout::top_down(egui::Align::Min), None);
            child.set_clip_rect(grid);
            schematic(app, &mut child, false, 32.0 * k);
        }
        if let Some((alive, built)) = app.designer_plaque() {
            let (dx, dy) = crate::dialog::PLAQUE_OFFSET;
            let plaque = client.at(280.0 + dx, 32.0 + dy, 60.0, 30.0);
            if plaque.bottom() <= foot {
                plaque_widget(app, ui, plaque, alive, built);
            }
        }
        let numbers =
            egui::Rect::from_min_max(client.px(16.0, 340.0), egui::pos2(rect.center().x, foot));
        if numbers.height() > 8.0 {
            let mut child = ui.child_ui(numbers, egui::Layout::top_down(egui::Align::Min), None);
            child.set_clip_rect(numbers);
            stats(app, &mut child);
        }
    }

    // `ShowMainControls` hides OK in the browser and calls the button beside
    // it `Done`; the browser's only way out is that one.
    if crate::views::placed_button(app, ui, done_rect, crate::dialog::DESIGNER_CLOSE.0, true)
        .clicked()
    {
        app.close_designer();
    }

    if let Some(question) = app.designer.as_ref().and_then(|d| d.confirm.clone()) {
        confirm(app, ui, rect, &question);
    }
}

/// The plaque under an existing design's schematic: the game's own
/// picture of it (`hdibPlaque`, 60 by 30) with `%ld of %ld` printed on
/// it — how many ships of the design exist, and how many were built.
fn plaque_widget(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect, alive: i64, built: i64) {
    let ctx = ui.ctx().clone();
    let cell = stars_formats::resources::art::Cell {
        resource: crate::dialog::PLAQUE_BITMAP,
        x: 0,
        y: 0,
        width: 60,
        height: 30,
    };
    let drawn = app
        .art
        .as_mut()
        .and_then(|art| art.sprite_at_size(&ctx, cell, rect.size()))
        .map(|image| image.paint_at(ui, rect))
        .is_some();
    let painter = ui.painter();
    if !drawn {
        painter.rect_filled(rect, 2.0, ui.visuals().faint_bg_color);
        painter.rect_stroke(rect, 2.0, ui.visuals().widgets.noninteractive.bg_stroke);
    }
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{alive} of {built}"),
        egui::FontId::proportional((rect.height() * 0.36).clamp(8.0, 12.0)),
        if drawn {
            egui::Color32::WHITE
        } else {
            ui.visuals().text_color()
        },
    );
}

/// The confirmation the Delete button raises, over the dialog's own foot.
fn confirm(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect, question: &str) {
    let where_ = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, rect.bottom() - 44.0),
        egui::pos2(rect.right() - 90.0, rect.bottom() - 4.0),
    );
    let mut child = ui.child_ui(where_, egui::Layout::top_down(egui::Align::Min), None);
    child.label(egui::RichText::new(question).color(egui::Color32::from_rgb(0xff, 0xc0, 0x60)));
    child.horizontal(|ui| {
        if ui.button("Yes, delete it").clicked() {
            app.designer_delete();
        }
        if ui.button("No").clicked() {
            if let Some(designer) = app.designer.as_mut() {
                designer.confirm = None;
            }
        }
    });
}

/// The two radio groups: **Design** (ship or starbase) and **View**.
fn radios(
    app: &mut App,
    ui: &mut egui::Ui,
    at: &impl Fn(u16) -> egui::Rect,
    caption: &impl Fn(u16) -> String,
) {
    let Some(designer) = app.designer.as_ref() else {
        return;
    };
    let mut starbase = designer.starbase;
    let mut view = designer.view;

    // A radio sits at the left of its rectangle, as a Windows radio does,
    // rather than centred in it.
    let radio_at = |ui: &mut egui::Ui, rect: egui::Rect, on: bool, text: &str| {
        let mut child = ui.child_ui(rect, egui::Layout::left_to_right(egui::Align::Center), None);
        child.add(egui::RadioButton::new(
            on,
            egui::RichText::new(text).small(),
        ))
    };
    // `Ships` and `Starbases` — the resource's captions are plural.
    let ships = radio_at(ui, at(0x810), !starbase, &caption(0x810));
    crate::views::record(app, ui, &caption(0x810), &ships);
    if ships.clicked() {
        starbase = false;
    }
    let bases = radio_at(ui, at(0x811), starbase, &caption(0x811));
    crate::views::record(app, ui, &caption(0x811), &bases);
    if bases.clicked() {
        starbase = true;
    }
    // `mdBuild = wParam - 0x812`, so the four View radios are in the enum's
    // own order.
    for (index, choice) in DesignView::ALL.into_iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let id = 0x812 + index as u16;
        let radio = radio_at(ui, at(id), view == choice, &caption(id));
        crate::views::record(app, ui, &caption(id), &radio);
        if radio.clicked() {
            view = choice;
        }
    }

    if let Some(designer) = app.designer.as_mut() {
        // Either radio resets the selection, as the original does by refilling
        // the dropdown and sending it back to the top.
        if designer.starbase != starbase || designer.view != view {
            designer.starbase = starbase;
            designer.view = view;
            designer.selected = 0;
            designer.filter = 0;
            designer.confirm = None;
        }
    }
}

fn dropdown(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect) {
    let list = app.designer_list();
    let Some(designer) = app.designer.as_mut() else {
        return;
    };
    if list.is_empty() {
        ui.put(
            rect,
            egui::Label::new(egui::RichText::new("No Designs").weak()),
        );
        return;
    }
    designer.selected = designer.selected.min(list.len() - 1);
    let mut selected = designer.selected;
    let mut child = ui.child_ui(rect, egui::Layout::top_down(egui::Align::Min), None);
    let box_ = egui::ComboBox::from_id_source("designer-dd")
        .width(rect.width())
        .selected_text(list[selected].clone())
        .show_ui(&mut child, |ui| {
            for (index, name) in list.iter().enumerate() {
                let entry = ui.selectable_value(&mut selected, index, name.clone());
                crate::views::record(app, ui, name, &entry);
            }
        });
    crate::views::record(app, ui, "Designs", &box_.response);
    let Some(designer) = app.designer.as_mut() else {
        return;
    };
    if selected != designer.selected {
        designer.selected = selected;
        designer.confirm = None;
    }
}

// --- editing -------------------------------------------------------------

fn editor(app: &mut App, ui: &mut egui::Ui) {
    // A drag that finished this frame, resolved after the lists are drawn so
    // both ends have been laid out.
    let dropped_on_list: Option<DesignerDrag>;
    let mut dropped_on_slot: Option<usize> = None;

    // The editor is laid out from `ptslotGlob`, the designer's client size
    // of 610 by 450 (`ShipBuilder`, `1020:4e14`), not from the template:
    // `SlotDlg`'s edit branch (`10c8:1f67`) moves the parts list to
    // (16, 32) with the category filter at (16, 8) above it — the column
    // the radios had — and the name field to (610 - 264, 8), 240 wide;
    // `DrawSlotDlg` draws the hull's picture at (610 - 338, 6) and
    // `UpdateSlotGlobals` starts the slot grid at (610 - 330, 32); OK,
    // Cancel and Help sit along the foot at 610 − 226, − 148 and − 74,
    // 68 wide and a line and a half tall (`SlotDlg`'s `WM_INITDIALOG`).
    // Everything scales with the width the frame has.
    let client = Client::allocate(ui);
    let rect = client.rect;
    let k = client.k;
    let px = |x: f32, y: f32| client.px(x, y);
    let tall = client.button_h();
    let ok_rect = client.foot_button(384.0);
    let cancel_rect = client.foot_button(462.0);
    name_field(
        app,
        ui,
        egui::Rect::from_min_size(px(346.0, 8.0), egui::vec2(240.0 * k, tall)),
    );
    filter_dropdown(
        app,
        ui,
        egui::Rect::from_min_size(px(16.0, 8.0), egui::vec2(240.0 * k, tall)),
    );
    let list = egui::Rect::from_min_size(
        px(16.0, 8.0 + tall / k + 4.0),
        egui::vec2(240.0 * k, 266.0 * k),
    )
    .intersect(rect);
    {
        let mut child = ui.child_ui(list, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(list);
        let inner = egui::Frame::default().inner_margin(4.0);
        let (_, payload) = child.dnd_drop_zone::<DesignerDrag, ()>(inner, |ui| {
            egui::ScrollArea::vertical()
                .id_source("designer-parts")
                .show(ui, |ui| parts_list(app, ui, true));
        });
        // `dnd_drop_zone` takes the payload itself, so this is the one chance
        // to read it.
        dropped_on_list = payload.map(|p| *p);
    }

    // The hull's picture at (610 − 338, 6) with its two arrows under it,
    // and the slot grid from (610 − 330, 32) — the same origin, so the
    // hull's own `rgbrc` table keeps its slots clear of the picture — down
    // to the row of buttons along the foot.
    let foot = ok_rect.top() - 6.0;
    let picture_rect = egui::Rect::from_min_max(px(272.0, 6.0), px(272.0 + 66.0, 96.0));
    if picture_rect.height() > 8.0 {
        let mut child = ui.child_ui(picture_rect, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(picture_rect);
        picture(app, &mut child, true, false);
    }
    let grid = egui::Rect::from_min_max(px(280.0, 32.0), egui::pos2(rect.right() - 4.0, foot));
    if grid.width() > 40.0 && grid.height() > 8.0 {
        let mut child = ui.child_ui(grid, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(grid);
        dropped_on_slot = schematic(app, &mut child, true, 32.0 * k);
    }
    // The cost and statistics (`DrawBuildSelHull`) fill the left half of
    // the client from `yBuildInfoSum` (340) down, under the parts list.
    let numbers = egui::Rect::from_min_max(px(16.0, 340.0), egui::pos2(rect.center().x, foot));
    if numbers.height() > 8.0 {
        let mut child = ui.child_ui(numbers, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(numbers);
        stats(app, &mut child);
        if let Some(complaint) = app.designer.as_ref().and_then(|d| d.complaint.clone()) {
            child.label(
                egui::RichText::new(complaint)
                    .small()
                    .color(egui::Color32::from_rgb(0xff, 0x8a, 0x8a)),
            );
        }
    }

    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
    if let Some(drag) = dropped_on_list {
        let drag = app.designer_drag_at_drop(drag, ctrl, shift);
        app.designer_drop_on_list(drag);
    }
    if let Some(target) = dropped_on_slot {
        if let Some(drag) = taken_payload(ui) {
            let drag = app.designer_drag_at_drop(drag, ctrl, shift);
            app.designer_drop_on_slot(drag, target);
        }
    }
    // `IDropPart`: a stack let go anywhere else comes off the design when
    // the pointer is on the left half of the dialog — the side the parts
    // list is on.
    let released = ui.input(|i| i.pointer.any_released());
    if released && egui::DragAndDrop::has_any_payload(ui.ctx()) {
        let on_left = ui
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|at| at.x < rect.center().x);
        if on_left {
            if let Some(drag) = taken_payload(ui) {
                if drag.from_slot.is_some() {
                    let drag = app.designer_drag_at_drop(drag, ctrl, shift);
                    app.designer_drop_on_list(drag);
                }
            }
        }
    }
    drag_preview(app, ui, 64.0 * k);
    // Delete or Backspace empties the slot selected. The original has no
    // key for it — a stack is dragged off — so this is an addition; it
    // stands aside for a text field, as the shell's own keys do.
    let selected_slot = app.designer.as_ref().and_then(|d| d.selected_slot);
    if let Some(index) = selected_slot {
        if !ui.ctx().wants_keyboard_input() {
            let pressed = ui.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
            });
            if pressed {
                app.designer_clear_slot(index);
            }
        }
    }

    // `ShowMainControls` shows OK for the editor and relabels the button
    // beside it `Cancel`.
    if crate::views::placed_button(app, ui, ok_rect, &caption(0x1), true).clicked() {
        app.designer_ok();
    }
    if crate::views::placed_button(app, ui, cancel_rect, crate::dialog::DESIGNER_CLOSE.1, true)
        .clicked()
    {
        app.designer_cancel();
    }
}

/// The payload of the drag that just ended, if it was one of ours.
fn taken_payload(ui: &egui::Ui) -> Option<DesignerDrag> {
    egui::DragAndDrop::take_payload::<DesignerDrag>(ui.ctx()).map(|p| *p)
}

/// What a drag carries, drawn under the pointer: the component's picture,
/// `size` square, centred on it. `FTrackSlot` carries the 64-pixel picture
/// offset by where it was grabbed; centring it keeps the pointer — which is
/// what the drop is judged by — visibly on the part.
fn drag_preview(app: &mut App, ui: &mut egui::Ui, size: f32) {
    let Some(drag) = egui::DragAndDrop::payload::<DesignerDrag>(ui.ctx()) else {
        return;
    };
    let Some(at) = ui.input(|i| i.pointer.interact_pos()) else {
        return;
    };
    let Some(part) = stars_core::parts::part(drag.category, drag.item) else {
        return;
    };
    let rect = egui::Rect::from_center_size(at, egui::vec2(size, size));
    let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("designer-drag"));
    let ctx = ui.ctx().clone();
    let cell = stars_core::parts::picture_cell(part.category, part.picture, 0);
    let image = cell.and_then(|cell| app.art.as_mut()?.sprite(&ctx, cell, size));
    match image {
        Some(image) => {
            // A `Ui` on the tooltip layer, so the picture rides over the
            // dialog and whatever else is under the pointer.
            let mut over = ui.child_ui(rect, egui::Layout::top_down(egui::Align::Min), None);
            over.set_clip_rect(egui::Rect::EVERYTHING);
            over.with_layer_id(layer, |ui| image.paint_at(ui, rect));
        }
        None => {
            let painter = ctx.layer_painter(layer);
            painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
            painter.rect_stroke(rect, 2.0, ui.visuals().widgets.active.bg_stroke);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                shorten(part.name),
                egui::FontId::proportional(9.0),
                ui.visuals().text_color(),
            );
        }
    }
}

fn filter_dropdown(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect) {
    let names: Vec<&'static str> = app
        .designer_filters()
        .iter()
        .map(|(_, name)| *name)
        .collect();
    let Some(designer) = app.designer.as_mut() else {
        return;
    };
    designer.filter = designer.filter.min(names.len().saturating_sub(1));
    let mut selected = designer.filter;
    let mut child = ui.child_ui(rect, egui::Layout::top_down(egui::Align::Min), None);
    let box_ = egui::ComboBox::from_id_source("designer-filter")
        .width(rect.width())
        .selected_text(names.get(selected).copied().unwrap_or(""))
        .show_ui(&mut child, |ui| {
            for (index, name) in names.iter().enumerate() {
                let entry = ui.selectable_value(&mut selected, index, *name);
                crate::views::record(app, ui, name, &entry);
            }
        });
    crate::views::record(app, ui, "Parts", &box_.response);
    if let Some(designer) = app.designer.as_mut() {
        designer.filter = selected;
    }
}

/// The parts list. Its rows can be picked up and dragged only in the
/// editor (`FTrackSlot`, `IDropPart`); the browser's Components view lists
/// the same parts as a plain list.
fn parts_list(app: &mut App, ui: &mut egui::Ui, draggable: bool) {
    let parts = app.designer_parts();
    for (index, part) in parts.iter().enumerate() {
        let row = ui
            .push_id(index, |ui| part_row(app, ui, part, index))
            .response;
        if !draggable {
            continue;
        }
        // The row is a drag source of its own rather than egui's
        // `dnd_drag_source`, which carries a picture of the whole row: the
        // original (`FTrackSlot`) carries the component's picture alone,
        // and `drag_preview` paints that one at the pointer.
        let id = egui::Id::new(("designer-part", index));
        let response = ui
            .interact(row.rect, id, egui::Sense::drag())
            .on_hover_cursor(egui::CursorIcon::Grab);
        if response.drag_started() {
            egui::DragAndDrop::set_payload(
                ui.ctx(),
                DesignerDrag {
                    category: part.category,
                    item: part.item,
                    count: 1,
                    from_slot: None,
                },
            );
        }
    }
    if parts.is_empty() {
        ui.label(
            egui::RichText::new("nothing you can build yet")
                .weak()
                .small(),
        );
    }
}

fn part_row(app: &mut App, ui: &mut egui::Ui, part: &PartRow, index: usize) {
    let selected = app
        .designer
        .as_ref()
        .is_some_and(|d| d.selected_part == Some(index));
    let label = format!("{}  {}kT", part.name, part.mass);
    let cell = stars_core::parts::picture_cell(part.category, part.picture, 0);
    let mut clicked = false;
    ui.horizontal(|ui| {
        // The original lists a part with its picture beside it, at half the
        // size the browser shows.
        if let Some(cell) = cell {
            crate::art::draw(app, ui, cell, 32.0);
        }
        let row = ui.selectable_label(selected, egui::RichText::new(&label).small());
        crate::views::record(app, ui, &part.name, &row);
        clicked = row.clicked();
    });
    if clicked {
        if let Some(designer) = app.designer.as_mut() {
            designer.selected_part = Some(index);
        }
    }
}

fn name_field(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect) {
    let Some(mut name) = app
        .designer
        .as_ref()
        .and_then(|d| d.editing.as_ref())
        .map(|e| e.design.name.clone())
    else {
        return;
    };
    // The template's own field, `0x81b`, limited to the same 31 characters
    // the original limits it to.
    if ui
        .put(
            rect,
            egui::TextEdit::singleline(&mut name).char_limit(stars_core::design::MAX_NAME),
        )
        .changed()
    {
        app.designer_rename(&name);
    }
}

// --- the picture and the schematic ---------------------------------------

/// The hull picture, with the two arrows under it while editing.
///
/// `DrawFleetBitmap` blits the ship 64 pixels square in a sunken frame, with
/// the owner's race emblem over its bottom-left corner, and the arrows spin
/// between the four pictures the hull owns. Without a copy of the original to
/// read the bitmaps out of, the frame holds the picture's number instead —
/// which is still what the arrows change.
///
/// The browser names the hull beside the picture; the editor, laid out as
/// the original's, has the name in its own field and puts the two arrows
/// under the picture (`rgrcBuildSpin`, at (610 − 317, 75) and 14 to the
/// right), so `beside` is false there.
fn picture(app: &mut App, ui: &mut egui::Ui, editing: bool, beside: bool) {
    let Some(design) = app.designer_subject() else {
        return;
    };
    let hull = App::designer_hull(&design);
    let ship = stars_formats::resources::art::ship(
        u16::from(design.picture),
        stars_formats::resources::art::ShipSize::Large,
    );
    let emblem = app.emblem_of(
        app.local_player(),
        stars_formats::resources::art::EmblemSize::Medium,
    );
    ui.horizontal(|ui| {
        if app.has_art() {
            // The emblem sits over the ship's bottom-left corner, as the
            // original overlays it: sixteen pixels on the sixty-four.
            let corner = ui.cursor().min;
            crate::art::draw(app, ui, ship, 64.0);
            if let Some(emblem) = emblem {
                let ctx = ui.ctx().clone();
                if let Some(art) = app.art.as_mut() {
                    if let Some(image) = art.sprite(&ctx, emblem, 16.0) {
                        let at = egui::Rect::from_min_size(
                            corner + egui::vec2(0.0, 48.0),
                            egui::vec2(16.0, 16.0),
                        );
                        image.paint_at(ui, at);
                    }
                }
            }
        } else {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(72.0, 48.0), egui::Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(rect, 2.0, ui.visuals().faint_bg_color);
            painter.rect_stroke(rect, 2.0, ui.visuals().widgets.noninteractive.bg_stroke);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("#{}", design.picture),
                egui::FontId::proportional(14.0),
                ui.visuals().weak_text_color(),
            );
        }
        if beside {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(hull.map_or("", |h| h.name))
                        .small()
                        .strong(),
                );
                if editing {
                    picture_arrows(app, ui);
                }
            });
        }
    });
    if editing && !beside {
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.add_space(21.0);
            picture_arrows(app, ui);
        });
    }
}

/// The two arrows that spin between a hull's four pictures.
fn picture_arrows(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let left = ui.small_button("◀");
        crate::views::record(app, ui, "picture left", &left);
        if left.clicked() {
            app.designer_next_picture(false);
        }
        let right = ui.small_button("▶");
        crate::views::record(app, ui, "picture right", &right);
        if right.clicked() {
            app.designer_next_picture(true);
        }
    });
}

/// The hull schematic. Returns the slot a drag was dropped on, if any.
/// How big the schematic grid is, for a caller that has to size itself
/// before it draws — the design pop-up, which is the designer's own panel
/// over whatever raised it.
pub(crate) fn schematic_size(app: &App) -> egui::Vec2 {
    let slots = app.designer_schematic();
    let Some(design) = app.designer_subject() else {
        return egui::Vec2::ZERO;
    };
    let Some(hull) = App::designer_hull(&design) else {
        return egui::Vec2::ZERO;
    };
    let mut cols = 0;
    let mut rows = 0;
    for s in &slots {
        cols = cols.max(s.cell.0 + 2);
        rows = rows.max(s.cell.1 + 2);
    }
    if let Some((_, (right, bottom))) = hull.cargo_cells() {
        cols = cols.max(right);
        rows = rows.max(bottom);
    }
    #[allow(clippy::cast_precision_loss)]
    egui::vec2(cols as f32 * CELL, rows as f32 * CELL)
}

pub(crate) fn schematic(
    app: &mut App,
    ui: &mut egui::Ui,
    editing: bool,
    cell: f32,
) -> Option<usize> {
    let slots = app.designer_schematic();
    let design = app.designer_subject()?;
    let hull = App::designer_hull(&design)?;
    if slots.is_empty() {
        return None;
    }

    // The grid is as wide and tall as the furthest cell any slot or the hold
    // reaches, plus the two cells a slot occupies.
    let mut cols = 0;
    let mut rows = 0;
    for s in &slots {
        cols = cols.max(s.cell.0 + 2);
        rows = rows.max(s.cell.1 + 2);
    }
    if let Some((_, (right, bottom))) = hull.cargo_cells() {
        cols = cols.max(right);
        rows = rows.max(bottom);
    }

    let size = egui::vec2(cols as f32 * cell, rows as f32 * cell);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let origin = rect.min;
    let cell_rect = |(col, row): (i32, i32)| {
        egui::Rect::from_min_size(
            origin + egui::vec2(col as f32 * cell, row as f32 * cell),
            egui::vec2(cell * 2.0, cell * 2.0),
        )
    };

    cargo_box(app, ui, hull, &design, origin, cell);

    let mut dropped = None;
    for (index, slot) in slots.iter().enumerate() {
        let target = cell_rect(slot.cell);
        let response = slot_widget(app, ui, target, slot, index, editing);
        if response {
            dropped = Some(index);
        }
    }
    dropped
}

/// The cargo space: three lines in a box, or a circle on the two starbases the
/// original draws round.
fn cargo_box(
    app: &App,
    ui: &mut egui::Ui,
    hull: &'static stars_core::components::Hull,
    design: &stars_core::design::ShipDesign,
    origin: egui::Pos2,
    cell: f32,
) {
    let Some(((left, top), (right, bottom))) = hull.cargo_cells() else {
        return;
    };
    let rect = egui::Rect::from_min_max(
        origin + egui::vec2(left as f32 * cell, top as f32 * cell),
        origin + egui::vec2(right as f32 * cell, bottom as f32 * cell),
    );
    let painter = ui.painter();
    let stroke = ui.visuals().widgets.noninteractive.fg_stroke;

    let starbase = design.is_starbase();
    // `DrawSlotDlg` draws the dock round for the Space Dock and the Death Star
    // and square for everything else — including the Space Station and the
    // Ultra Station, which also have docks. That is what the binary compares
    // (`ihuldef == 33 || ihuldef == 36`), so it is what happens here.
    let round = starbase && (design.hull_id == 33 || design.hull_id == 36);
    if round {
        painter.circle_stroke(rect.center(), rect.width().min(rect.height()) / 2.0, stroke);
    } else {
        painter.rect_stroke(rect, 0.0, stroke);
    }

    let capacity = if hull.unlimited_cargo() {
        "Unlimited".to_string()
    } else if starbase {
        format!("{}kT", hull.cargo_max)
    } else {
        // A ship's hold grows with cargo pods, so the figure is the design's.
        format!("{}kT", design.cargo_capacity().unwrap_or(0))
    };
    // A ship reads Cargo / <n>kT / max; a starbase reads <n>kT / Space / Dock.
    let lines = if starbase {
        [capacity, "Space".into(), "Dock".into()]
    } else {
        ["Cargo".into(), capacity, "max".into()]
    };
    let colour = ui.visuals().text_color();
    let font = egui::FontId::proportional(slot_font(cell));
    for (i, line) in lines.iter().enumerate() {
        let y = rect.top() + rect.height() * (i as f32 + 0.5) / 3.0;
        painter.text(
            egui::pos2(rect.center().x, y),
            egui::Align2::CENTER_CENTER,
            line,
            font.clone(),
            colour,
        );
    }
    let _ = app;
}

/// One slot. Returns whether a drag was dropped on it.
fn slot_widget(
    app: &mut App,
    ui: &mut egui::Ui,
    rect: egui::Rect,
    slot: &crate::SchematicSlot,
    index: usize,
    editing: bool,
) -> bool {
    let id = egui::Id::new(("designer-slot", index));
    let response = ui.interact(rect, id, egui::Sense::click_and_drag());
    crate::views::record(app, ui, &format!("slot {index}"), &response);
    let selected = app
        .designer
        .as_ref()
        .is_some_and(|d| d.selected_slot == Some(index));

    ui.painter()
        .rect_filled(rect, 1.0, ui.visuals().extreme_bg_color);
    // `DrawSlotDlg` blits the picture — the component's, or the
    // category's off the empty-slot sheet — into the slot and prints the
    // line over its foot. Without the game's pictures the slot names what
    // it holds or takes instead.
    let font = slot_font(rect.width() / 2.0);
    let ctx = ui.ctx().clone();
    let drawn = slot
        .picture
        .and_then(|cell| app.art.as_mut()?.sprite(&ctx, cell, rect.width()))
        .map(|image| image.paint_at(ui, rect))
        .is_some();
    let painter = ui.painter();
    let stroke = if selected {
        egui::Stroke::new(2.0_f32, ui.visuals().selection.bg_fill)
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    painter.rect_stroke(rect, 1.0, stroke);

    if !drawn {
        let title = match &slot.fitted {
            Some((name, _)) => name.clone(),
            None => App::designer_slot_kinds(slot.allowed),
        };
        painter.text(
            egui::pos2(rect.center().x, rect.top() + rect.height() * 0.35),
            egui::Align2::CENTER_CENTER,
            shorten(&title),
            egui::FontId::proportional(font),
            ui.visuals().text_color(),
        );
    }
    // The line sits `dyArial6 + 4` above the slot's foot, over the picture.
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - font * 0.5 - 4.0),
        egui::Align2::CENTER_CENTER,
        &slot.label,
        egui::FontId::proportional(font),
        if drawn {
            egui::Color32::BLACK
        } else {
            ui.visuals().weak_text_color()
        },
    );

    if response.clicked() {
        if let Some(designer) = app.designer.as_mut() {
            designer.selected_slot = Some(index);
        }
    }

    if !editing {
        return false;
    }

    // Dragging a fitted component off its slot, and dropping one on. The
    // count is read at the drop (`designer_drag_at_drop`), as the original
    // reads the keys then.
    if slot.fitted.is_some() && response.drag_started() {
        if let Some(part) = app
            .designer_subject()
            .and_then(|d| d.slots.get(index).copied())
            .and_then(|s| stars_core::design::slot_part(&s))
        {
            let drag = DesignerDrag {
                category: part.category,
                item: part.item,
                count: 1,
                from_slot: Some(index),
            };
            egui::DragAndDrop::set_payload(ui.ctx(), drag);
        }
    }
    // The no-way cursor over a slot that would refuse what is carried
    // (`FTrackSlot` sets `hcurNoWay` from `IDropPart`'s answer).
    if response.contains_pointer() {
        if let Some(drag) = egui::DragAndDrop::payload::<DesignerDrag>(ui.ctx()) {
            if !app.designer_accepts_drop(*drag, index) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::NotAllowed);
            }
        }
    }

    // `contains_pointer` rather than `hovered`: egui reports nothing as
    // hovered while something is being dragged, which is exactly when a
    // drop has to be seen.
    let released = ui.input(|i| i.pointer.any_released());
    response.contains_pointer() && released && egui::DragAndDrop::has_any_payload(ui.ctx())
}

/// The size of the lettering on a slot, from the size of its cells: Arial 6
/// on the original's 32-pixel cells.
fn slot_font(cell: f32) -> f32 {
    (cell * 0.3).clamp(8.0, 11.0)
}

/// Slot pictures are small; a long list of categories will not fit.
fn shorten(text: &str) -> String {
    if text.chars().count() <= 18 {
        return text.to_string();
    }
    text.chars().take(17).chain(std::iter::once('…')).collect()
}

// --- the numbers ---------------------------------------------------------

pub(crate) fn stats(app: &mut App, ui: &mut egui::Ui) {
    let Some(design) = app.designer_subject() else {
        return;
    };
    let hulls = app
        .designer
        .as_ref()
        .is_some_and(|d| d.view == DesignView::Hulls && d.editing.is_none());
    // `Cost of one <name>`, with " Hull" appended while browsing bare hulls.
    let suffix = if hulls { " Hull" } else { "" };
    ui.label(
        egui::RichText::new(format!("Cost of one {}{suffix}", design.name))
            .small()
            .strong(),
    );

    let cost = app.designer_cost_rows();
    let stats = app.designer_stat_rows();
    ui.horizontal_top(|ui| {
        egui::Grid::new("designer-cost")
            .num_columns(2)
            .spacing([10.0, 1.0])
            .show(ui, |ui| {
                for (label, value) in &cost {
                    ui.label(egui::RichText::new(label).small());
                    ui.label(egui::RichText::new(value).small());
                    ui.end_row();
                }
            });
        ui.add_space(20.0);
        egui::Grid::new("designer-stats")
            .num_columns(2)
            .spacing([10.0, 1.0])
            .show(ui, |ui| {
                for (label, value) in &stats {
                    ui.label(egui::RichText::new(label).small());
                    ui.label(egui::RichText::new(value).small());
                    ui.end_row();
                }
            });
    });
}
