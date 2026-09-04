//! The galaxy map.
//!
//! Planet coordinates live only in the `.xy` universe file, so a game opened
//! without one has nothing to draw; the map says so rather than piling every
//! planet on the origin.
//!
//! The map distinguishes the two kinds of planet the loader keeps apart — the
//! ones the player owns and can command, and the ones they have merely scanned.
//! That distinction is the fog of war, and it comes out of `GameState` rather
//! than needing any bookkeeping here.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::views::{colonists, player_colour};
use crate::App;

/// Draw the galaxy.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some((min_x, min_y, max_x, max_y)) = app.extent() else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label("No planet positions.");
            ui.label(
                egui::RichText::new(
                    "They come from the game's .xy universe file, which was not found \
                     beside this save or one directory up.",
                )
                .weak(),
            );
        });
        return;
    };

    ui.horizontal(|ui| {
        ui.label(format!("{} planets known", app.visible_planets().len()));
        ui.separator();
        if let Some(planet) = app.selected_planet() {
            ui.label(format!(
                "selected: {} ({})",
                planet.name.unwrap_or("unnamed"),
                planet.id
            ));
        }
    });
    ui.separator();

    let available = ui.available_size();
    let (response, painter) = ui.allocate_painter(available, Sense::click());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 10, 18));

    // Fit the universe into the panel, keeping it square so distances read true.
    let span = (max_x - min_x).max(max_y - min_y).max(1.0);
    let margin = 16.0;
    let scale = (rect.width().min(rect.height()) - margin * 2.0) / span;
    let to_screen = |x: f32, y: f32| -> Pos2 {
        Pos2::new(
            rect.left() + margin + (x - min_x) * scale,
            rect.top() + margin + (y - min_y) * scale,
        )
    };

    let selected = app.selection.planet;
    let mut clicked: Option<i16> = None;
    let pointer = response.interact_pointer_pos();

    for (planet, owned) in app.visible_planets() {
        let Some(position) = planet.position else {
            continue;
        };
        let at = to_screen(f32::from(position.x), f32::from(position.y));

        let (colour, radius) = match planet.owner {
            Some(owner) => (player_colour(owner), if owned { 4.5 } else { 3.5 }),
            // Unowned but seen: a faint dot.
            None => (Color32::from_gray(110), 2.5),
        };
        painter.circle_filled(at, radius, colour);
        // A planet the player only knows about is drawn hollow, so the fog of
        // war is visible at a glance.
        if !owned {
            painter.circle_stroke(
                at,
                radius + 1.5,
                Stroke::new(1.0_f32, Color32::from_gray(70)),
            );
        }
        if Some(planet.id) == selected {
            painter.circle_stroke(at, radius + 4.0, Stroke::new(1.5_f32, Color32::WHITE));
        }
        if let Some(p) = pointer {
            if (p - at).length() <= radius + 6.0 {
                clicked = Some(planet.id);
            }
        }
    }

    // Fleets, as small marks offset from their planet so they do not hide it.
    if let Some(game) = app.game.as_ref() {
        for fleet in &game.fleets {
            let at = to_screen(f32::from(fleet.position.x), f32::from(fleet.position.y));
            let colour = player_colour(fleet.owner);
            let size = Vec2::splat(3.0);
            painter.rect_filled(
                Rect::from_center_size(at + Vec2::new(6.0, -6.0), size),
                0.0,
                colour,
            );
        }
    }

    if let Some(id) = clicked {
        app.selection.planet = Some(id);
    }

    // A short legend for the selected planet, drawn over the map.
    if let Some(planet) = app.selected_planet() {
        let text = match planet.owner {
            Some(owner) => format!(
                "{} — player {owner}, {} colonists, {} mines, {} factories",
                planet.name.unwrap_or("unnamed"),
                colonists(planet.pop),
                planet.mines,
                planet.factories
            ),
            None => format!("{} — unclaimed", planet.name.unwrap_or("unnamed")),
        };
        painter.text(
            rect.left_bottom() + Vec2::new(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            text,
            egui::FontId::proportional(13.0),
            Color32::from_gray(220),
        );
    }
}
