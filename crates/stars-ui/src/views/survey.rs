//! The mine survey pane.
//!
//! The third pane down the left of the original's frame, and the one that
//! answers "what is that?": whatever is selected in the scanner, this
//! summarises it (`DrawMineSurvey`, `1028:065a`). A planet gets its value, its
//! population, and the six **bars** the pane is really for — gravity,
//! temperature and radiation against the race's habitable band, then ironium,
//! boranium and germanium against what is on the surface and what is in the
//! ground. A fleet gets its ships, mass, cargo and orders. Nothing selected
//! reads `Deep Space`.
//!
//! The title is `"<name> Summary"` (`SetMineralTitleBar`, `1028:47dc`).
//!
//! See `docs/ui/mine-survey-pane.md` for what is and is not reproduced.

use crate::{App, SurveyBar, SurveySubject};

/// Draw the survey pane.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(app.survey_title()).small().strong());
    ui.separator();

    match app.survey_subject() {
        SurveySubject::DeepSpace => {
            ui.label(egui::RichText::new("nothing selected").weak().small());
        }
        SurveySubject::Fleet(_) => {
            for line in app.survey_fleet_rows() {
                ui.label(egui::RichText::new(line).small());
            }
        }
        SurveySubject::Planet(_) => {
            egui::Grid::new("survey-planet")
                .num_columns(2)
                .spacing([8.0, 1.0])
                .show(ui, |ui| {
                    for (label, value) in app.survey_planet_rows() {
                        ui.label(egui::RichText::new(label).small());
                        ui.label(egui::RichText::new(value).small());
                        ui.end_row();
                    }
                });
            let environment = app.survey_environment();
            let minerals = app.survey_minerals();
            if environment.is_empty() {
                ui.label(
                    egui::RichText::new("this planet has not been surveyed")
                        .weak()
                        .small(),
                );
                return;
            }
            ui.add_space(2.0);
            for bar in &environment {
                habitability(ui, bar);
            }
            ui.add_space(4.0);
            // The mineral bars share a scale, as the original's do: it draws
            // one axis under all three.
            let scale = minerals
                .iter()
                .map(|m| m.at.max(m.high * 10))
                .max()
                .unwrap_or(1)
                .max(1);
            for bar in &minerals {
                mineral(ui, bar, scale);
            }
        }
    }
}

/// One environment bar: the race's habitable band, with the planet on it.
fn habitability(ui: &mut egui::Ui, bar: &SurveyBar) {
    ui.horizontal(|ui| {
        ui.set_height(14.0);
        ui.label(egui::RichText::new(&bar.label).small());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(&bar.value).small());
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width().max(24.0), 10.0),
                egui::Sense::hover(),
            );
            let painter = ui.painter();
            painter.rect_filled(rect, 1.0, ui.visuals().extreme_bg_color);
            // The band the race can live in, drawn along the bar; an immune
            // race lives anywhere, which the original shows as the whole bar.
            let span = |v: i32| rect.left() + rect.width() * (v.clamp(0, 100) as f32) / 100.0;
            let (low, high) = if bar.immune {
                (rect.left(), rect.right())
            } else {
                (span(bar.low), span(bar.high))
            };
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(low, rect.top()),
                    egui::pos2(high, rect.bottom()),
                ),
                1.0,
                ui.visuals().selection.bg_fill,
            );
            // And the planet itself, as the original's little diamond.
            let at = span(bar.at);
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(at - 1.0, rect.top() - 1.0),
                    egui::pos2(at + 1.0, rect.bottom() + 1.0),
                ),
                0.0,
                ui.visuals().strong_text_color(),
            );
        });
    });
}

/// One mineral bar: what is on the surface, over how rich the ground is.
fn mineral(ui: &mut egui::Ui, bar: &SurveyBar, scale: i32) {
    ui.horizontal(|ui| {
        ui.set_height(14.0);
        ui.label(egui::RichText::new(&bar.label).small());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(&bar.value).small());
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width().max(24.0), 10.0),
                egui::Sense::hover(),
            );
            let painter = ui.painter();
            painter.rect_filled(rect, 1.0, ui.visuals().extreme_bg_color);
            let width = |v: i32| rect.width() * (v.min(scale) as f32) / (scale as f32);
            // The concentration behind, the surface stock in front.
            painter.rect_filled(
                egui::Rect::from_min_size(
                    rect.left_top(),
                    egui::vec2(width(bar.high * 10), rect.height()),
                ),
                1.0,
                ui.visuals().widgets.inactive.bg_fill,
            );
            painter.rect_filled(
                egui::Rect::from_min_size(
                    rect.left_top(),
                    egui::vec2(width(bar.at), rect.height()),
                ),
                1.0,
                ui.visuals().selection.bg_fill,
            );
        });
    });
}
