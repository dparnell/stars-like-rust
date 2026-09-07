//! The native eframe shell.
//!
//! This holds nothing but the window, the menu and the file dialog: every
//! screen is drawn by `stars_ui::views`, so the desktop and web frontends can
//! differ only in how they are hosted.

use std::path::{Path, PathBuf};

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
        let mut this = Self {
            app,
            written: Vec::new(),
        };
        this.find_art(None);
        this
    }

    /// Look for a copy of the original executable and read its pictures.
    ///
    /// Failing costs nothing: every screen that draws one of these pictures
    /// draws perfectly well without it. The order is the one that will find
    /// the right file with the least surprise — an explicit `STARS_EXE` first,
    /// then beside whatever save was just opened, then the working directory
    /// and a `binary` under it, which is where a checkout of this project keeps
    /// its own copy.
    fn find_art(&mut self, beside: Option<&Path>) {
        if self.app.has_art() {
            return;
        }
        let mut roots: Vec<PathBuf> = Vec::new();
        if let Ok(explicit) = std::env::var("STARS_EXE") {
            let path = PathBuf::from(explicit);
            if let Some(art) = read_art(&path) {
                self.load_art(&path, art);
                return;
            }
            if let Some(dir) = path.parent() {
                roots.push(dir.to_path_buf());
            }
        }
        if let Some(dir) = beside.and_then(Path::parent) {
            roots.push(dir.to_path_buf());
            if let Some(up) = dir.parent() {
                roots.push(up.to_path_buf());
            }
        }
        roots.push(PathBuf::from("."));
        roots.push(PathBuf::from("binary"));

        for root in roots {
            let Ok(entries) = std::fs::read_dir(&root) else {
                continue;
            };
            let mut found: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
                })
                .filter(|path| {
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.to_ascii_lowercase().starts_with("stars"))
                })
                .collect();
            // A deterministic order, so a directory holding several copies
            // always gives the same one.
            found.sort();
            for path in found {
                if let Some(bytes) = read_art(&path) {
                    self.load_art(&path, bytes);
                    return;
                }
            }
        }
    }

    /// Hand a candidate to the app, keeping the failure quiet.
    fn load_art(&mut self, path: &Path, bytes: Vec<u8>) {
        let name = path.display().to_string();
        let _ = self.app.load_art(bytes, &name);
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
        self.write_templates(&path);
    }

    /// Read the player's production templates out of `stars.ini`.
    ///
    /// The original keeps them there rather than in a save — see
    /// `stars_formats::ProductionTemplate` — so they follow the installation
    /// rather than the game. This looks for the file beside the save, which is
    /// where a copy of Stars! run from its own directory would have put it.
    fn read_templates(&mut self, beside: &Path) {
        let Some(ini) = beside.parent().map(|dir| dir.join("stars.ini")) else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&ini) else {
            return;
        };
        let values = ini_section(&text, stars_formats::TEMPLATE_INI_SECTION);
        self.app.load_production_templates(&values);
    }

    /// Write them back, leaving every other section of the file alone.
    fn write_templates(&mut self, beside: &Path) {
        let Some(ini) = beside.parent().map(|dir| dir.join("stars.ini")) else {
            return;
        };
        let existing = std::fs::read_to_string(&ini).unwrap_or_default();
        let updated = with_ini_values(
            &existing,
            stars_formats::TEMPLATE_INI_SECTION,
            &self.app.production_templates_ini(),
        );
        if updated != existing {
            let _ = std::fs::write(&ini, updated);
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
            } else {
                self.read_templates(&path);
                // A game opened from its own directory may have the original
                // sitting beside it, which is the likeliest place of all.
                self.find_art(Some(&path));
            }
        }
    }
}

impl StarsApp {
    /// Ask for a copy of the original to read the pictures out of.
    fn pick_art(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("Find a copy of the original Stars!")
            .add_filter("Programs", &["exe"])
            .pick_file();
        let Some(path) = picked else { return };
        match read_art(&path) {
            Some(bytes) => {
                let name = path.display().to_string();
                if let Err(e) = self.app.load_art(bytes, &name) {
                    self.app.error = Some(e);
                }
            }
            None => {
                self.app.error = Some(format!("cannot read {}", path.display()));
            }
        }
    }
}

/// Read a candidate executable, refusing anything implausible before the whole
/// file is pulled into memory.
fn read_art(path: &Path) -> Option<Vec<u8>> {
    let size = std::fs::metadata(path).ok()?.len();
    // The real thing is about four megabytes, nearly all of it pictures.
    if !(64 * 1024..64 * 1024 * 1024).contains(&size) {
        return None;
    }
    std::fs::read(path).ok()
}

/// The `key = value` pairs of one section of a Windows profile file.
fn ini_section(text: &str, section: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            inside = name.eq_ignore_ascii_case(section);
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            out.push((key.trim().to_string(), value.trim().to_string()));
        }
    }
    out
}

/// Replace some keys in one section, leaving everything else in the file
/// exactly as it was — the same courtesy the save code extends to a game file.
fn with_ini_values(text: &str, section: &str, values: &[(String, String)]) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let mut inside = false;
    let mut seen_section = false;
    let mut written = false;

    let write_values = |out: &mut String| {
        for (key, value) in values {
            out.push_str(key);
            out.push('=');
            out.push_str(value);
            out.push('\n');
        }
    };

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            // Leaving the section: put our keys in before the next one starts.
            if inside && !written {
                write_values(&mut out);
                written = true;
            }
            inside = name.eq_ignore_ascii_case(section);
            seen_section |= inside;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        // Drop the keys we are replacing; keep the rest of the section.
        if inside {
            if let Some((key, _)) = trimmed.split_once('=') {
                if values.iter().any(|(k, _)| k == key.trim()) {
                    continue;
                }
            }
        }
        out.push_str(line);
        out.push('\n');
    }

    if !seen_section {
        out.push('[');
        out.push_str(section);
        out.push_str("]\n");
    }
    if !written {
        write_values(&mut out);
    }
    out
}

impl eframe::App for StarsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Playback needs a steady stream of frames; everything else is happy to
        // redraw only on input.
        if self.app.playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }

        if self.app.game.is_some()
            && self.app.setup.is_none()
            && ctx.input(|i| i.key_pressed(egui::Key::F4))
        {
            if self.app.designer.is_some() {
                self.app.close_designer();
            } else {
                self.app.open_designer();
            }
        }

        if self.app.game.is_some()
            && self.app.setup.is_none()
            && ctx.input(|i| i.key_pressed(egui::Key::F2))
        {
            if self.app.browser.is_some() {
                self.app.close_browser();
            } else {
                self.app.open_browser();
            }
        }

        if self.app.game.is_some()
            && self.app.setup.is_none()
            && ctx.input(|i| i.key_pressed(egui::Key::F5))
        {
            if self.app.research_dialog.is_some() {
                self.app.research_cancel();
            } else {
                self.app.open_research();
            }
        }

        if self.app.game.is_some()
            && self.app.setup.is_none()
            && ctx.input(|i| i.key_pressed(egui::Key::F7))
        {
            if self.app.relations_dialog.is_some() {
                self.app.close_relations();
            } else {
                // Refused outright in a single-player game, as the original
                // refuses it: the menu item is there and does nothing.
                self.app.open_relations();
            }
        }

        if self.app.game.is_some()
            && self.app.setup.is_none()
            && ctx.input(|i| i.key_pressed(egui::Key::F10))
        {
            if self.app.score_sheet.is_some() {
                self.app.close_score_sheet();
            } else {
                self.app.open_score_sheet();
            }
        }

        // Both of these are modeless in the original, so they sit alongside
        // whatever else is open rather than blocking it — and they are drawn
        // every frame, not only on the one their key was pressed.
        if self.app.browser.is_some() {
            let mut open = true;
            egui::Window::new("Technology Browser")
                .open(&mut open)
                .resizable(true)
                .default_width(420.0)
                .show(ctx, |ui| stars_ui::views::browser::view(&mut self.app, ui));
            if !open {
                self.app.close_browser();
            }
        }

        if self.app.score_sheet.is_some() {
            let mut open = true;
            egui::Window::new("Score")
                .open(&mut open)
                .resizable(true)
                .default_width(520.0)
                .show(ctx, |ui| stars_ui::views::score::view(&mut self.app, ui));
            if !open {
                self.app.close_score_sheet();
            }
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
                    // The game's own pictures are read out of a copy of the
                    // original executable. They are never required — every
                    // screen draws without them — so this only ever says where
                    // they came from, or offers to be pointed at one.
                    match self.app.art.as_ref() {
                        Some(art) => {
                            ui.label(
                                egui::RichText::new(format!("Pictures: {}", art.source))
                                    .small()
                                    .weak(),
                            );
                        }
                        None => {
                            if ui
                                .button("Use the original's pictures…")
                                .on_hover_text(
                                    "Point this at a copy of the original stars.exe and                                      the planets, race emblems and other artwork are                                      read out of it. Nothing is copied anywhere.",
                                )
                                .clicked()
                            {
                                ui.close_menu();
                                self.pick_art();
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                // The original's View menu. Only its one checkable item so
                // far — the rest of what it holds (Toolbar, Zoom, Window
                // Layout, Race, Game Parameters) has no home here yet.
                ui.menu_button("View", |ui| {
                    let mut on = self.app.scan_overlays.player_colours;
                    if ui
                        .checkbox(&mut on, "Player Colors")
                        .on_hover_text(
                            "Write planet names and ship counts in each player's \
                             own colour. Yours stay white.",
                        )
                        .changed()
                    {
                        self.app.scan_overlays.player_colours = on;
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
                // The original's Commands menu opens the designer with F4.
                if ui
                    .add_enabled(
                        self.app.game.is_some() && self.app.setup.is_none(),
                        egui::Button::new("Ship Design…").shortcut_text("F4"),
                    )
                    .clicked()
                {
                    self.app.open_designer();
                }
                if ui
                    .add_enabled(
                        self.app.selected_planet().is_some() && self.app.setup.is_none(),
                        egui::Button::new("Production…"),
                    )
                    .clicked()
                {
                    self.app.open_production();
                }
                if ui
                    .add_enabled(
                        self.app.game.is_some() && self.app.setup.is_none(),
                        egui::Button::new("Research…").shortcut_text("F5"),
                    )
                    .clicked()
                {
                    self.app.open_research();
                }
                if ui
                    .add_enabled(
                        self.app.game.is_some() && self.app.setup.is_none(),
                        egui::Button::new("Technology Browser…").shortcut_text("F2"),
                    )
                    .clicked()
                {
                    self.app.open_browser();
                }
                if ui
                    .add_enabled(
                        self.app.game.is_some() && self.app.setup.is_none(),
                        egui::Button::new("Score…").shortcut_text("F10"),
                    )
                    .clicked()
                {
                    self.app.open_score_sheet();
                }
                if ui
                    .add_enabled(
                        self.app.game.is_some() && self.app.setup.is_none(),
                        egui::Button::new("Player Relations…").shortcut_text("F7"),
                    )
                    .clicked()
                {
                    self.app.open_relations();
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
                // Find lives in the original's View menu, not on the
                // scanner's toolbar, so it sits in this frontend's own menu
                // bar rather than cluttering the toolbar with a control the
                // original does not have there.
                if self.app.game.is_some() && self.app.setup.is_none() {
                    ui.separator();
                    let mut text = std::mem::take(&mut self.app.find_text);
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .desired_width(110.0)
                            .hint_text("Find…  (Ctrl+F)"),
                    );
                    let entered =
                        field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    self.app.find_text = text;
                    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
                        field.request_focus();
                    }
                    if entered {
                        let typed = self.app.find_text.clone();
                        self.app.find(&typed);
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

        // The three panes the original keeps down the left of its frame, in
        // its own order: the planet at the top, the messages under it, and the
        // survey at the bottom (`RefitFrameChildren`, `mdi.c`).
        if self.app.game.is_some() {
            egui::SidePanel::left("planet")
                .resizable(true)
                .default_width(380.0)
                .show(ctx, |ui| {
                    egui::TopBottomPanel::bottom("messages")
                        .resizable(true)
                        .default_height(160.0)
                        .show_inside(ui, |ui| stars_ui::views::messages::view(&mut self.app, ui));
                    // Below the messages, the survey pane: whatever is
                    // selected, summarised.
                    egui::TopBottomPanel::bottom("survey")
                        .resizable(true)
                        .default_height(190.0)
                        .show_inside(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .show(ui, |ui| stars_ui::views::survey::view(&mut self.app, ui));
                        });
                    // One pane, two tile tables: the original swaps the
                    // planet's tiles for the fleet's when a fleet is selected.
                    let fleet =
                        matches!(self.app.survey_subject(), stars_ui::SurveySubject::Fleet(_));
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            if fleet {
                                stars_ui::views::fleet::view(&mut self.app, ui);
                            } else {
                                stars_ui::views::planet::view(&mut self.app, ui);
                            }
                        });
                    });
                });
        }

        // The Ship and Starbase Designer is a dialog in the original, so it is
        // a window here rather than one of the screens.
        if self.app.designer.is_some() {
            let mut open = true;
            egui::Window::new("Ship and Starbase Designer")
                .open(&mut open)
                .resizable(true)
                .default_width(660.0)
                .show(ctx, |ui| stars_ui::views::designer::view(&mut self.app, ui));
            if !open {
                self.app.close_designer();
            }
        }

        if self.app.research_dialog.is_some() {
            let mut open = true;
            egui::Window::new("Research")
                .open(&mut open)
                .resizable(true)
                .default_width(700.0)
                .show(ctx, |ui| stars_ui::views::research::view(&mut self.app, ui));
            if !open {
                self.app.research_cancel();
            }
        }

        if self.app.relations_dialog.is_some() {
            let mut open = true;
            egui::Window::new("Player Relations")
                .open(&mut open)
                .resizable(false)
                .default_width(320.0)
                .show(ctx, |ui| {
                    stars_ui::views::relations::view(&mut self.app, ui)
                });
            if !open {
                self.app.close_relations();
            }
        }

        if self.app.production.is_some() {
            let mut open = true;
            egui::Window::new("Production")
                .open(&mut open)
                .resizable(true)
                .default_width(700.0)
                .show(ctx, |ui| {
                    stars_ui::views::production::view(&mut self.app, ui)
                });
            if !open {
                self.app.production_cancel();
            }
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
