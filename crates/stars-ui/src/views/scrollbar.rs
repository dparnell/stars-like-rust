//! A Windows 3.1 scroll bar, for the scanner: the original's map is a
//! scrolled window (`WS_HSCROLL | WS_VSCROLL`), and past 100% the system
//! puts a bar along its right edge and another along its foot, each an
//! arrow button at either end, a thumb the size of what is in view, and a
//! track that pages when clicked.

use egui::{Color32, Pos2, Rect, Sense, Vec2};

/// How wide a bar is, and how long its arrow buttons are: the system's
/// `SM_CXVSCROLL` and `SM_CYHSCROLL`, sixteen pixels in Windows 3.1.
pub const WIDTH: f32 = 16.0;

/// One bar. `pos` is the scroll position in pixels of the scrolled content,
/// `page` how much of it is in view and `total` how much there is; the bar
/// answers with a new position when the player moved it.
pub fn scroll_bar(
    ui: &mut egui::Ui,
    id: egui::Id,
    rect: Rect,
    horizontal: bool,
    pos: f32,
    page: f32,
    total: f32,
) -> Option<f32> {
    let colour = |[r, g, b]: [u8; 3]| Color32::from_rgb(r, g, b);
    let face = colour(crate::toolbar::FACE);
    let lit = colour(crate::toolbar::HILITE);
    let dark = colour(crate::toolbar::SHADOW);
    let painter = ui.painter().clone();
    let length = if horizontal {
        rect.width()
    } else {
        rect.height()
    };
    let arrow = WIDTH.min(length / 2.0);
    let track_len = (length - 2.0 * arrow).max(0.0);
    let range = (total - page).max(0.0);
    let pos = pos.clamp(0.0, range);
    let thumb_len = if total > 0.0 {
        (track_len * page / total).clamp(WIDTH.min(track_len), track_len)
    } else {
        track_len
    };
    let travel = (track_len - thumb_len).max(0.0);
    let thumb_at = if range > 0.0 {
        travel * pos / range
    } else {
        0.0
    };

    // The track: the system's dithered grey, a solid mid-grey here.
    painter.rect_filled(rect, 0.0, colour([0xd8, 0xd8, 0xd8]));

    let along = |from: f32, len: f32| -> Rect {
        if horizontal {
            Rect::from_min_size(
                Pos2::new(rect.left() + from, rect.top()),
                Vec2::new(len, rect.height()),
            )
        } else {
            Rect::from_min_size(
                Pos2::new(rect.left(), rect.top() + from),
                Vec2::new(rect.width(), len),
            )
        }
    };
    let bevel = |r: Rect, down: bool| {
        painter.rect_filled(r, 0.0, face);
        let (tl, br) = if down { (dark, lit) } else { (lit, dark) };
        painter.rect_filled(
            Rect::from_min_size(r.min, Vec2::new(r.width(), 1.0)),
            0.0,
            tl,
        );
        painter.rect_filled(
            Rect::from_min_size(r.min, Vec2::new(1.0, r.height())),
            0.0,
            tl,
        );
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(r.left(), r.bottom() - 1.0),
                Vec2::new(r.width(), 1.0),
            ),
            0.0,
            br,
        );
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(r.right() - 1.0, r.top()),
                Vec2::new(1.0, r.height()),
            ),
            0.0,
            br,
        );
        painter.rect_stroke(r, 0.0, egui::Stroke::new(1.0_f32, Color32::BLACK));
    };
    let triangle = |r: Rect, towards_start: bool| {
        let c = r.center();
        let h = 3.0;
        let points = match (horizontal, towards_start) {
            (true, true) => vec![
                Pos2::new(c.x - h, c.y),
                Pos2::new(c.x + h, c.y - h),
                Pos2::new(c.x + h, c.y + h),
            ],
            (true, false) => vec![
                Pos2::new(c.x + h, c.y),
                Pos2::new(c.x - h, c.y - h),
                Pos2::new(c.x - h, c.y + h),
            ],
            (false, true) => vec![
                Pos2::new(c.x, c.y - h),
                Pos2::new(c.x - h, c.y + h),
                Pos2::new(c.x + h, c.y + h),
            ],
            (false, false) => vec![
                Pos2::new(c.x, c.y + h),
                Pos2::new(c.x - h, c.y - h),
                Pos2::new(c.x + h, c.y - h),
            ],
        };
        painter.add(egui::Shape::convex_polygon(
            points,
            Color32::BLACK,
            egui::Stroke::NONE,
        ));
    };

    let mut moved: Option<f32> = None;
    let line = (page / 10.0).max(1.0);

    // The two arrow buttons: a line each way.
    let first = along(0.0, arrow);
    let last = along(length - arrow, arrow);
    let first_response = ui.interact(first, id.with("first"), Sense::click_and_drag());
    let last_response = ui.interact(last, id.with("last"), Sense::click_and_drag());
    bevel(first, first_response.is_pointer_button_down_on());
    bevel(last, last_response.is_pointer_button_down_on());
    triangle(first, true);
    triangle(last, false);
    if first_response.clicked() {
        moved = Some((pos - line).max(0.0));
    }
    if last_response.clicked() {
        moved = Some((pos + line).min(range));
    }

    // The thumb, which drags; the track either side of it, which pages.
    let thumb = along(arrow + thumb_at, thumb_len);
    let thumb_response = ui.interact(thumb, id.with("thumb"), Sense::click_and_drag());
    if thumb_response.dragged() && travel > 0.0 {
        let delta = thumb_response.drag_delta();
        let step = if horizontal { delta.x } else { delta.y };
        moved = Some((pos + step * range / travel).clamp(0.0, range));
    }
    let track = along(arrow, track_len);
    let track_response = ui.interact(track, id.with("track"), Sense::click());
    if track_response.clicked() && !thumb_response.hovered() {
        if let Some(at) = track_response.interact_pointer_pos() {
            let before = if horizontal {
                at.x < thumb.left()
            } else {
                at.y < thumb.top()
            };
            moved = Some(if before {
                (pos - page).max(0.0)
            } else {
                (pos + page).min(range)
            });
        }
    }
    bevel(thumb, thumb_response.dragged());
    moved
}
