//! The Host Mode dialog.
//!
//! `HostModeDialog`, `IDD_HOST_MODE` (115). In the original this **is** host
//! mode: `BringUpHostDlg` hides the map and runs this dialog in a loop, so the
//! host sees nothing else. Here it is a window over the same game, which is the
//! one departure worth knowing about — see `docs/ui/host-mode.md`.
//!
//! ```text
//!   Game: Kestrel                        Next year is:
//!   File: Kestrel                                 2401
//!
//!    * 1: Humanoids     turned in        [ Generate Now  ]
//!    * 2: Rabbitoids    still out        [ Auto Generate ]
//!    * 3: Turindrones   turned in        [ Password...   ]
//!                                        [ Close         ]
//!                                        [ Help          ]
//!
//!                                            Time since
//!                                          last change:
//!                                              1:04
//! ```
//!
//! The player list is painted rather than laid out — the template has nothing
//! in that whole area — so what it says came from `DrawHostDialog2`.

use crate::app::TurnStatus;
use crate::App;

/// What the dialog was asked to do.
pub enum Action {
    /// Generate this many turns in a row.
    Generate(u16),
    /// Set the host's password.
    Password,
    /// Leave host mode.
    Close,
}

/// Draw the dialog, and say what was asked of it.
///
/// `elapsed` is how long it has been since a player's status last changed, in
/// seconds; the original keeps that clock itself (`ctickLast`).
pub fn view(app: &mut App, ui: &mut egui::Ui, elapsed: f64) -> Option<Action> {
    app.game.as_ref()?;
    let mut action = None;

    egui::Grid::new("host-heading")
        .num_columns(2)
        .spacing([16.0, 2.0])
        .show(ui, |ui| {
            ui.label("Game:");
            ui.label(app.game_name());
            ui.end_row();
            ui.label("File:");
            ui.label(app.host_file_name());
            ui.end_row();
            ui.label("Next year is:");
            ui.label(egui::RichText::new(app.next_year().to_string()).strong());
            ui.end_row();
        });
    ui.separator();

    let statuses = app.turn_statuses();
    let names: Vec<String> = (0..statuses.len())
        .map(|player| app.player_name(player))
        .collect();

    ui.horizontal_top(|ui| {
        // The list, one row per player: the blue diamond the original draws
        // beside every one of them, the number, the name and the status.
        ui.vertical(|ui| {
            for (player, status) in statuses.iter().enumerate() {
                ui.horizontal(|ui| {
                    diamond(ui);
                    ui.label(format!("{}:", player + 1));
                    ui.colored_label(
                        colour(*status),
                        format!("{} {}", names[player], status.name()),
                    );
                });
            }
        });

        ui.add_space(12.0);
        ui.vertical(|ui| {
            let outstanding = app.turns_outstanding();
            // Shift and Ctrl turn one generation into a run of them, which is
            // what the original reads as the button goes down.
            let (shift, control) = ui.input(|i| (i.modifiers.shift, i.modifiers.command));
            let passes = App::generate_passes(shift, control);
            let generate = ui.button("Generate Now");
            let generate = if passes > 1 {
                generate.on_hover_text(format!(
                    "Force {passes} turns in a row. Let go of the modifiers for one."
                ))
            } else if outstanding > 0 {
                generate.on_hover_text(format!(
                    "{outstanding} of {} turns are still out.",
                    statuses.len()
                ))
            } else {
                generate
            };
            if generate.clicked() {
                action = Some(Action::Generate(passes));
            }
            // Auto Generate is enabled by the options this project has not
            // recovered, so it is where the original leaves it with none set.
            ui.add_enabled(false, egui::Button::new("Auto Generate"))
                .on_disabled_hover_text(
                    "Generating on a timer needs the Auto Generate Options dialog, \
                     which is painted rather than laid out and is not recovered.",
                );
            // The host's own password, which is not the local player's: it
            // goes into the host file rather than a player block.
            if ui
                .button("Password…")
                .on_hover_text("The password that guards host mode, kept in the host file.")
                .clicked()
            {
                action = Some(Action::Password);
            }
            if ui.button("Close").clicked() {
                action = Some(Action::Close);
            }
        });
    });

    ui.separator();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Time since last change:")
                .small()
                .weak(),
        );
        ui.label(egui::RichText::new(elapsed_text(elapsed)).small());
    });

    action
}

/// The colour a status is drawn in: the original's dark green for the two that
/// are not being waited for, dark red for the rest.
fn colour(status: TurnStatus) -> egui::Color32 {
    if status.outstanding() {
        egui::Color32::from_rgb(127, 0, 0)
    } else {
        egui::Color32::from_rgb(0, 127, 0)
    }
}

/// The blue diamond the original draws beside each player.
fn diamond(ui: &mut egui::Ui) {
    let size = ui.text_style_height(&egui::TextStyle::Body) * 0.6;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let centre = rect.center();
    let half = size / 2.0;
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(centre.x, centre.y - half),
            egui::pos2(centre.x + half, centre.y),
            egui::pos2(centre.x, centre.y + half),
            egui::pos2(centre.x - half, centre.y),
        ],
        egui::Color32::from_rgb(0, 0, 200),
        egui::Stroke::NONE,
    ));
}

/// How long since anything changed, in the original's four formats: seconds,
/// then `m:ss`, then `h:mm:ss`, then `d days h:mm:ss`.
#[must_use]
pub fn elapsed_text(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    let (s, m, h) = (total % 60, (total / 60) % 60, (total / 3600) % 24);
    let days = total / 86_400;
    if total < 60 {
        format!("{total} seconds")
    } else if total < 3_600 {
        format!("{m}:{s:02}")
    } else if days == 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{days} days {h}:{m:02}:{s:02}")
    }
}
