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
    /// The files the last "save a new game" wrote, to report back.
    written: Vec<String>,
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
        Self {
            app,
            written: Vec::new(),
        }
    }

    /// Write a generated game out as a complete set of files.
    fn save_new_game(&mut self) {
        let suggestion = self
            .app
            .universe
            .as_ref()
            .and_then(|u| u.game().ok())
            .map(|g| format!("{}.hst", sanitise(&g.name)))
            .unwrap_or_else(|| "game.hst".into());
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save the new game")
            .set_file_name(suggestion)
            .add_filter("Stars! host file", &["hst"])
            .save_file()
        else {
            return;
        };
        match self.app.save_new_game(&path) {
            Ok(written) => {
                self.app.error = None;
                self.written = written
                    .iter()
                    .filter_map(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .collect();
            }
            Err(e) => self.app.error = Some(e),
        }
    }

    fn save(&mut self, ask: bool) {
        if !self.app.can_save_game() {
            self.save_new_game();
            return;
        }
        let target = if ask || self.app.path.is_none() {
            rfd::FileDialog::new()
                .set_title("Save the game")
                .set_file_name(
                    self.app
                        .path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "game.m1".into()),
                )
                .save_file()
        } else {
            self.app.path.clone()
        };
        let Some(path) = target else { return };
        match self.app.save(&path) {
            Ok(()) => self.app.error = None,
            Err(e) => self.app.error = Some(e),
        }
    }

    /// Write the game's universe as a `.xy`.
    fn save_universe(&mut self) {
        let suggestion = self
            .app
            .universe
            .as_ref()
            .and_then(|u| u.game().ok())
            .map(|g| format!("{}.xy", sanitise(&g.name)))
            .unwrap_or_else(|| "game.xy".into());
        let Some(path) = rfd::FileDialog::new()
            .set_title("Write the universe file")
            .set_file_name(suggestion)
            .add_filter("Stars! universe", &["xy"])
            .save_file()
        else {
            return;
        };
        match self.app.save_universe(&path) {
            Ok(()) => self.app.error = None,
            Err(e) => self.app.error = Some(e),
        }
    }

    /// Act on what the New Game wizard asked for.
    fn wizard(&mut self, action: stars_ui::views::newgame::Action) {
        use stars_ui::views::newgame::Action;
        match action {
            Action::Cancel => self.app.setup = None,
            Action::Create => {
                let Some(config) = self.app.setup.clone() else {
                    return;
                };
                match self.app.new_game(&config) {
                    Ok(()) => self.app.error = None,
                    Err(e) => self.app.error = Some(e),
                }
            }
            Action::LoadRace(index) => {
                let Some(path) = rfd::FileDialog::new()
                    .set_title("Open a race file")
                    .add_filter(
                        "Stars! race",
                        &["r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "hst", "m1"],
                    )
                    .pick_file()
                else {
                    return;
                };
                match stars_ui::views::newgame::race_from_file(&path) {
                    Ok(race) => {
                        if let Some(player) = self
                            .app
                            .setup
                            .as_mut()
                            .and_then(|c| c.players.get_mut(index))
                        {
                            player.race = race;
                        }
                        self.app.error = None;
                    }
                    Err(e) => self.app.error = Some(e),
                }
            }
        }
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
                    if ui.button("New game…").clicked() {
                        ui.close_menu();
                        self.app.setup = Some(stars_core::newgame::NewGame::default());
                    }
                    if ui.button("Open…").clicked() {
                        ui.close_menu();
                        self.pick_file();
                    }
                    let open = self.app.game.is_some();
                    if ui
                        .add_enabled(open, egui::Button::new("Save"))
                        .on_hover_text(
                            "A game opened from a file is written back by replacing only \
                             what you changed. A new game is written out whole: a .xy, a \
                             .hst and one .mN per player.",
                        )
                        .clicked()
                    {
                        ui.close_menu();
                        self.save(false);
                    }
                    if ui
                        .add_enabled(open, egui::Button::new("Save as…"))
                        .clicked()
                    {
                        ui.close_menu();
                        self.save(true);
                    }
                    if ui
                        .add_enabled(
                            self.app.universe.is_some(),
                            egui::Button::new("Write universe (.xy)…"),
                        )
                        .clicked()
                    {
                        ui.close_menu();
                        self.save_universe();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.separator();
                if ui
                    .add_enabled(
                        self.app.game.is_some() && self.app.setup.is_none(),
                        egui::Button::new("Generate turn"),
                    )
                    .on_hover_text(
                        "Advance one year. The rolls will differ from the original \
                         engine's: its generator is seeded from the clock and its state \
                         is in no save file.",
                    )
                    .clicked()
                {
                    self.app.generate_turn();
                }
                ui.separator();
                for screen in Screen::ALL {
                    let enabled = self.app.game.is_some() && self.app.setup.is_none();
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
                    let mut status = self.app.status_line();
                    if self.app.dirty {
                        status.push_str(" · unsaved changes");
                    }
                    ui.label(status);
                });
            });
        });

        if !self.written.is_empty() {
            let written = self.written.join(", ");
            egui::TopBottomPanel::top("written").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Wrote {written}"));
                    if ui.button("dismiss").clicked() {
                        self.written.clear();
                    }
                });
            });
        }

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

        if let Some(turn) = self.app.last_turn.clone() {
            egui::TopBottomPanel::bottom("turn").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Year {} — {} planets mined, population {:+}, {} ships built",
                        turn.year, turn.mined, turn.population, turn.ships
                    ));
                    for (player, orders) in &turn.replayed {
                        ui.label(format!("· replayed {orders} orders from player {player}"));
                    }
                    for (player, fields) in &turn.breakthroughs {
                        ui.label(format!("· player {player} gained {fields} levels"));
                    }
                    if !turn.skipped.is_empty() {
                        ui.label(
                            egui::RichText::new(format!(
                                "· not simulated: {}",
                                turn.skipped.join(", ")
                            ))
                            .weak(),
                        );
                    }
                    if ui.button("dismiss").clicked() {
                        self.app.last_turn = None;
                    }
                });
            });
        }

        let action = egui::CentralPanel::default()
            .show(ctx, |ui| stars_ui::views::central(&mut self.app, ui))
            .inner;
        if let Some(action) = action {
            self.wizard(action);
        }
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

/// Turn a game name into something safe to suggest as a file name.
fn sanitise(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "game".to_string()
    } else {
        trimmed
    }
}
