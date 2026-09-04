//! The native eframe shell.
//!
//! This holds nothing but the window, the menu and the file dialog: every
//! screen is drawn by `stars_ui::views`, so the desktop and web frontends can
//! differ only in how they are hosted.

use std::path::PathBuf;

use stars_ui::{App, Screen};

/// The eframe application.
pub struct StarsApp {
    app: App,
}

impl StarsApp {
    /// Create the application, optionally opening a file straight away.
    #[must_use]
    pub fn new(open: Option<PathBuf>) -> Self {
        let mut app = App::new();
        if let Some(path) = open {
            if let Err(e) = app.open(&path) {
                app.error = Some(e);
            }
        }
        Self { app }
    }

    fn pick_file(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("Open a Stars! save")
            .add_filter(
                "Stars! files",
                &["hst", "m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8", "xy"],
            )
            .pick_file();
        if let Some(path) = picked {
            if let Err(e) = self.app.open(&path) {
                self.app.error = Some(e);
            }
        }
    }
}

impl eframe::App for StarsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Playback needs a steady stream of frames; everything else is happy to
        // redraw only on input.
        if self.app.playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open…").clicked() {
                        ui.close_menu();
                        self.pick_file();
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.separator();
                for screen in Screen::ALL {
                    let enabled = self.app.game.is_some();
                    if ui
                        .add_enabled(
                            enabled,
                            egui::SelectableLabel::new(self.app.screen == screen, screen.title()),
                        )
                        .clicked()
                    {
                        self.app.screen = screen;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(self.app.status_line());
                });
            });
        });

        if let Some(error) = self.app.error.clone() {
            egui::TopBottomPanel::top("error").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(0xff, 0x8a, 0x8a), &error);
                    if ui.button("dismiss").clicked() {
                        self.app.error = None;
                    }
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::central(&mut self.app, ui);
        });
    }
}

/// Run the native shell.
///
/// # Errors
/// Returns whatever eframe could not do — usually a missing display.
pub fn run(open: Option<PathBuf>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("Stars!"),
        ..Default::default()
    };
    eframe::run_native(
        "Stars!",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(StarsApp::new(open)))
        }),
    )
}
