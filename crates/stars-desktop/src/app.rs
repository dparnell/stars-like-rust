//! The native eframe shell.
//!
//! This holds nothing but the window, the menu and the file dialog: every
//! screen is drawn by `stars_ui::views`, so the desktop and web frontends can
//! differ only in how they are hosted.

use std::path::{Path, PathBuf};

use stars_ui::App;

/// The eframe application.
pub struct StarsApp {
    app: App,
    /// The clock reading the Host Mode dialog measures "time since last
    /// change" from (`ctickLast`).
    host_since: f64,
    /// The files the last "save a new game" wrote, to report back.
    written: Vec<String>,
    /// The File menu's tail: the games opened most recently.
    recent: stars_ui::recent::Recent,
    /// Where the frame window was while it was neither maximised nor
    /// minimised, which is what `GetWindowPlacement`'s `rcNormalPosition`
    /// holds and what `SetWindowIniString` stores.
    restored: Option<(i16, i16, i16, i16)>,
    /// Which letter that rectangle goes out with.
    frame_state: stars_ui::settings::WindowState,
}

impl StarsApp {
    /// Create the application, optionally opening a file straight away.
    #[must_use]
    pub fn new(open: Option<PathBuf>) -> Self {
        let mut app = App::new();
        // There is somebody here to ask, so a guarded turn asks.
        app.prompt_for_password = true;
        let ini = read_ini();
        let recent = stars_ui::recent::Recent::read_ini(&ini);
        // Which columns each report shows and what it sorts on, from
        // `[Misc]`. The window rectangles beside them belong to windows
        // this project does not have; `Ini` carries them through untouched.
        app.reports.read_ini(&ini);
        // The scanner's view, overlays, filters, zoom, the toolbar and the
        // window layout, all of which live in `[Windows]`.
        app.read_scanner_ini(&ini);
        // The four cargo orders the blue diamond offers and the five
        // production templates, which share `[ZipOrders]`.
        app.read_zip_ini(&ini);
        // Which of the planet pane's tiles stand open.
        app.read_tiles_ini(&ini);
        // `ReadIniSettings` copies `[Files] File1` into `szBase` and sets the
        // startup-file bit, so a launch with nothing to go on reopens the
        // game last played.
        let open = open.or_else(|| recent.startup_file().map(PathBuf::from));
        let mut opened = None;
        if let Some(path) = open {
            if let Err(e) = app.open(&path) {
                app.error = Some(e);
            } else {
                opened = Some(path);
            }
        }
        // The selection and the message pane, which only come back for the
        // same game, the same player and — for the message — the same year.
        app.read_selection_ini(&ini);
        let mut this = Self {
            app,
            written: Vec::new(),
            host_since: 0.0,
            recent,
            restored: None,
            frame_state: stars_ui::settings::WindowState::Normal,
        };
        if let Some(path) = opened.as_deref() {
            this.note_opened(path);
        }
        this.find_art(opened.as_deref());
        this
    }

    /// Put a game at the head of the recently-opened list and write the
    /// settings out again, if anything actually moved.
    fn note_opened(&mut self, path: &Path) {
        if self.recent.opened(&path.display().to_string()) {
            self.write_settings();
        }
    }

    /// Write the settings file: the recently-opened list, each report's
    /// columns and sort, the scanner, and the zip orders and templates.
    fn write_settings(&self) {
        let mut ini = read_ini();
        self.recent.write_ini(&mut ini);
        self.app.reports.write_ini(&mut ini);
        self.app.write_scanner_ini(&mut ini);
        self.app.write_zip_ini(&mut ini);
        self.app.write_selection_ini(&mut ini);
        self.app.write_tiles_ini(&mut ini);
        if let Some((left, top, width, height)) = self.restored {
            stars_ui::settings::set_frame_window(
                &mut ini,
                stars_ui::settings::WindowRect::frame(
                    self.frame_state,
                    (left, top),
                    (width, height),
                ),
            );
        }
        write_ini(&ini);
    }

    /// Keep note of where the frame is, so that the settings file gets the
    /// **restored** rectangle rather than whatever a maximised window
    /// happens to fill — which is what `GetWindowPlacement` gives the
    /// original for nothing.
    fn note_frame(&mut self, ctx: &egui::Context) {
        let (maximised, minimised, outer) = ctx.input(|i| {
            let viewport = i.viewport();
            (
                viewport.maximized.unwrap_or(false),
                viewport.minimized.unwrap_or(false),
                viewport.outer_rect,
            )
        });
        self.frame_state = if maximised {
            stars_ui::settings::WindowState::Maximised
        } else if minimised {
            stars_ui::settings::WindowState::Iconised
        } else {
            stars_ui::settings::WindowState::Normal
        };
        if maximised || minimised {
            return;
        }
        if let Some(rect) = outer {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a window's corner, in whole pixels"
            )]
            let round = |v: f32| v.round() as i16;
            self.restored = Some((
                round(rect.left()),
                round(rect.top()),
                round(rect.width()),
                round(rect.height()),
            ));
        }
    }

    /// Open a game by path, from the menu's recently-used list.
    fn open_path(&mut self, path: &Path) {
        if let Err(e) = self.app.open(path) {
            self.app.error = Some(e);
        } else {
            self.read_templates(path);
            self.note_opened(path);
            self.app.read_selection_ini(&read_ini());
            self.find_art(Some(path));
        }
    }

    /// Look for a copy of the original executable and read its pictures.
    ///
    /// Failing costs nothing: every screen that draws one of these pictures
    /// draws perfectly well without it. The order is the one that will find
    /// the right file with the least surprise — an explicit `STARS_EXE` first,
    /// then beside whatever save was just opened, then the working directory
    /// and a `binary` under it, which is where a checkout of this project keeps
    /// its own copy — and then the same two under each directory above this
    /// program's own, so a build run from `target/debug` inside a checkout
    /// finds the checkout's copy however it was launched.
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
        if let Some(here) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
        {
            for above in here.ancestors() {
                roots.push(above.to_path_buf());
                roots.push(above.join("binary"));
            }
        }

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
        // A `stars.ini` sitting beside the save **wins** over the settings
        // file: it is the one that travels with the game, so a copied game
        // directory brings its own templates and orders with it.
        let beside = stars_ui::settings::Ini::parse(&text);
        self.app.read_zip_ini(&beside);

        // `[Misc] DefaultPassword`, which `FCheckPassword` consults before it
        // puts the password prompt up.
        self.app.default_password = beside
            .get(
                stars_formats::DEFAULT_PASSWORD_INI_SECTION,
                stars_formats::DEFAULT_PASSWORD_INI_KEY,
            )
            .unwrap_or_default()
            .to_string();
    }

    /// Write them back, leaving every other section of the file alone.
    fn write_templates(&mut self, beside: &Path) {
        let Some(ini) = beside.parent().map(|dir| dir.join("stars.ini")) else {
            return;
        };
        let existing = std::fs::read_to_string(&ini).unwrap_or_default();
        let mut parsed = stars_ui::settings::Ini::parse(&existing);
        self.app.write_zip_ini(&mut parsed);
        let updated = parsed.to_string();
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

    /// Write the race the wizard is holding, and close it.
    ///
    /// The original's own file filter is `"Stars! Race Files|*.R*|"` (string
    /// `0x0531`) and it names the file after the race.
    fn save_race(&mut self) {
        let Some(built) = self.app.race_wizard_file() else {
            return;
        };
        let bytes = match built {
            Ok(bytes) => bytes,
            Err(e) => {
                self.app.error = Some(e.to_string());
                return;
            }
        };
        let suggestion = self
            .app
            .race_wizard
            .as_ref()
            .map(|w| format!("{}.r1", sanitise(&w.name)))
            .unwrap_or_else(|| "race.r1".into());
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save the race")
            .set_file_name(suggestion)
            .add_filter("Stars! race files", &["r1"])
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, bytes) {
            Ok(()) => {
                self.app.error = None;
                self.app.close_race_wizard();
            }
            // The original's own words for this, string `0x010b`.
            Err(e) => {
                self.app.error = Some(format!(
                    "Stars! was unable to save your race data file. Please try again. ({e})"
                ));
            }
        }
    }

    /// Act on what the Host Mode dialog asked for.
    ///
    /// Generating writes the new files for everybody, which is what a host
    /// generation is: the year runs, and every player gets a turn file. The
    /// original asks before generating with turns outstanding and before a
    /// forced run of them, and so does this.
    fn host(&mut self, action: stars_ui::views::host::Action, now: f64) {
        use stars_ui::views::host::Action;
        match action {
            Action::Generate(passes) => {
                let outstanding = self.app.turns_outstanding();
                let question = if passes > 1 {
                    format!("Force generate {passes} turns in a row?")
                } else if outstanding > 0 {
                    format!(
                        "{outstanding} of {} turns are still out. Generate anyway?",
                        self.app.turn_statuses().len()
                    )
                } else {
                    String::new()
                };
                if !question.is_empty()
                    && rfd::MessageDialog::new()
                        .set_title("Stars!")
                        .set_description(&question)
                        .set_buttons(rfd::MessageButtons::YesNo)
                        .show()
                        != rfd::MessageDialogResult::Yes
                {
                    return;
                }
                self.app.generate_turns(passes);
                self.host_since = now;
                // A generated year is only a host's when everyone can read it.
                let Some(path) = self.app.path.clone() else {
                    return;
                };
                match self.app.save_new_game(&path) {
                    Ok(written) => {
                        self.written = written
                            .iter()
                            .filter_map(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string())
                            .collect();
                        self.app.error = None;
                    }
                    Err(e) => self.app.error = Some(e),
                }
            }
            Action::Password => self.app.open_host_password_dialog(),
            Action::Close => self.app.close_host_mode(),
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
                self.note_opened(&path);
                self.app.read_selection_ini(&read_ini());
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

impl eframe::App for StarsApp {
    /// `WriteIniSettings` runs on the way out, and so does this: each
    /// report's columns and sort, and the recently-opened list, go back to
    /// `stars.ini`.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.write_settings();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.app.start_frame();
        self.note_frame(ctx);
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
                self.app.research_ok();
            } else {
                self.app.open_research();
            }
        }

        // Commands (Battle Plans...) is F6.
        if self.app.game.is_some()
            && self.app.setup.is_none()
            && ctx.input(|i| i.key_pressed(egui::Key::F6))
        {
            if self.app.battle_plans.is_some() {
                self.app.close_battle_plans();
            } else {
                self.app.open_battle_plans();
            }
        }

        // The letter keys and F9 the tutorial leans on. They stand aside for
        // whatever has the focus, as `FHandleKey` (`1018:165a`) does for the
        // toolbar, the lists and the message editor.
        if self.app.game.is_some() && self.app.setup.is_none() && !ctx.wants_keyboard_input() {
            // `n` walks your own fleets, wrapping round (`SelectAdjFleet`,
            // `1050:3d32`) — "Hit the n key to look at your next fleet."
            if ctx.input(|i| i.key_pressed(egui::Key::N)) {
                self.app.select_adjacent_fleet(1);
            }
            // `q` opens the production queue of the planet selected, which is
            // the Change button's dialog.
            if ctx.input(|i| i.key_pressed(egui::Key::Q)) {
                self.app.open_production();
            }
            // F9 generates the next year — the tutorial ends most of its
            // pages with it.
            if ctx.input(|i| i.key_pressed(egui::Key::F9)) {
                self.app.generate_turn();
            }
            // F3 walks round the four reports and back to the map, and Esc
            // closes whichever is up. The four menu items only *show* F3;
            // the key itself has an id of its own. See `App::open_report`.
            if ctx.input(|i| i.key_pressed(egui::Key::F3)) {
                self.app.open_report();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.app.close_report();
            }
        }

        // Backspace and Delete drop the waypoint the map has in hand.
        // `FHandleKey` (`1018:165a`) treats the two as one key and asks no
        // question, and it refuses when the selection is not a fleet. It also
        // stands aside for whatever has the focus — a text field, the
        // toolbar, a list — which here means only doing it when nothing else
        // wants the key.
        if self.app.game.is_some()
            && self.app.setup.is_none()
            && self.app.selection.fleet.is_some()
            && !ctx.wants_keyboard_input()
            && ctx
                .input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        {
            self.app.delete_current_waypoint();
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

        // The modeless windows, which draw every frame. They belong out
        // here rather than inside a key handler: a window nested in one
        // is drawn only on the frame that key is pressed.
        // The password prompt holds a save that is not open yet, so it is put
        // up on its own, with no close button — Cancel gives up on the file,
        // as the original's loader does.
        if self.app.password_prompt.is_some() {
            let now = ctx.input(|i| i.time);
            egui::Window::new("Stars!")
                .id(egui::Id::new("password-prompt"))
                .collapsible(false)
                .resizable(false)
                .default_width(260.0)
                .show(ctx, |ui| {
                    stars_ui::views::password::prompt(&mut self.app, ui, now);
                });
        }

        if self.app.host_mode {
            let mut open = true;
            // The original hides everything else and runs this modally; here it
            // is a window, so the map is still there behind it.
            let elapsed = ctx.input(|i| i.time) - self.host_since;
            let mut action = None;
            egui::Window::new("Stars! Host Mode")
                .open(&mut open)
                .resizable(false)
                .default_width(440.0)
                .show(ctx, |ui| {
                    action = stars_ui::views::host::view(&mut self.app, ui, elapsed);
                });
            if !open {
                self.app.close_host_mode();
            }
            if let Some(action) = action {
                self.host(action, ctx.input(|i| i.time));
            }
        }

        if self.app.password_dialog.is_some() {
            let mut open = true;
            egui::Window::new(self.app.password_title())
                .id(egui::Id::new("change-password"))
                .open(&mut open)
                .resizable(false)
                .default_width(320.0)
                .show(ctx, |ui| {
                    stars_ui::views::password::view(&mut self.app, ui);
                });
            if !open {
                self.app.close_password_dialog();
            }
        }

        if self.app.game.is_some()
            && self.app.setup.is_none()
            && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F))
        {
            self.app.find_open = true;
        }

        // File's own three: `&New...\tCtrl+N`, `&Open...\tCtrl+O` and
        // `&Save\tCtrl+S`. The fourth, Ctrl+A for Save And Submit, has
        // nothing behind it here.
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::N)) {
            self.app.setup = Some(stars_core::newgame::NewGame::default());
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            self.pick_file();
        }
        if self.app.game.is_some()
            && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S))
        {
            self.save(false);
        }

        // View (Race) is F8 in the original.
        if self.app.game.is_some()
            && self.app.setup.is_none()
            && ctx.input(|i| i.key_pressed(egui::Key::F8))
        {
            if self.app.race_viewer.is_some() {
                self.app.close_race_viewer();
            } else {
                let me = self.app.local_player();
                self.app.open_race_viewer(me);
            }
        }

        if self.app.find_open {
            let mut open = true;
            egui::Window::new("Find")
                .open(&mut open)
                .resizable(false)
                .default_width(260.0)
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new("A planet or fleet by name, or a fleet by number.")
                            .small()
                            .weak(),
                    );
                    let mut text = std::mem::take(&mut self.app.find_text);
                    let field = ui.add(egui::TextEdit::singleline(&mut text).desired_width(220.0));
                    // Only when nothing else has it: the box draws every
                    // frame, and asking for focus unconditionally would take
                    // it back off whatever the user clicked next.
                    if ui.memory(|m| m.focused().is_none()) {
                        field.request_focus();
                    }
                    let entered =
                        field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    self.app.find_text = text;
                    let pressed = ui.button("Find").clicked();
                    if entered || pressed {
                        let typed = self.app.find_text.clone();
                        self.app.find(&typed);
                    }
                });
            if !open {
                self.app.find_open = false;
            }
        }

        if self.app.battle_plans.is_some() {
            let mut open = true;
            egui::Window::new("Battle Plans")
                .open(&mut open)
                .resizable(false)
                .default_width(400.0)
                .show(ctx, |ui| {
                    stars_ui::views::battleplans::view(&mut self.app, ui);
                });
            if !open {
                self.app.close_battle_plans();
            }
        }

        if self.app.race_wizard.is_some() {
            let mut open = true;
            let mut finish = false;
            egui::Window::new(self.app.race_wizard_title())
                .id(egui::Id::new("race-wizard"))
                .open(&mut open)
                .resizable(true)
                .default_width(420.0)
                .show(ctx, |ui| {
                    finish = stars_ui::views::race_wizard::view(&mut self.app, ui);
                });
            if !open {
                self.app.close_race_wizard();
            } else if finish {
                self.save_race();
            }
        }

        if self.app.race_viewer.is_some() {
            let mut open = true;
            egui::Window::new("Race")
                .open(&mut open)
                .resizable(true)
                .default_width(400.0)
                .show(ctx, |ui| stars_ui::views::race::view(&mut self.app, ui));
            if !open {
                self.app.close_race_viewer();
            }
        }

        if self.app.game_parameters {
            let mut open = true;
            egui::Window::new("Game Parameters")
                .open(&mut open)
                .resizable(true)
                .default_width(420.0)
                .show(ctx, |ui| {
                    stars_ui::views::parameters::view(&mut self.app, ui);
                });
            if !open {
                self.app.game_parameters = false;
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

        // The report that is open, which is a window over the map rather
        // than a screen: `ReportDlg` creates one, sizes it from `ptSize`
        // and places it with `StickyDlgPos`, and the scanner goes on
        // drawing behind it.
        if let Some(report) = self.app.open_report_kind() {
            let state = *self.app.reports.state(report);
            let screen = ctx.screen_rect();
            let size = egui::vec2(f32::from(state.size.0), f32::from(state.size.1));
            let at = if state.centred() {
                screen.center() - size / 2.0
            } else {
                egui::pos2(f32::from(state.pos.0), f32::from(state.pos.1))
            };
            let min = stars_ui::report::ReportState::MIN_SIZE;
            let mut open = true;
            let shown = egui::Window::new(stars_ui::views::report::window_title(&self.app, report))
                .id(egui::Id::new(("report", report.irpt())))
                .open(&mut open)
                .resizable(true)
                .min_size([f32::from(min.0), f32::from(min.1)])
                .default_pos(at)
                .default_size(size)
                .show(ctx, |ui| {
                    stars_ui::views::report::view(&mut self.app, ui, report);
                });
            // `StickyDlgPos` on the way out keeps the top-left, and
            // `WM_DESTROY` the size.
            if let Some(shown) = shown {
                let rect = shown.response.rect;
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a window's corner, in whole pixels"
                )]
                let round = |v: f32| v.round() as i16;
                let state = self.app.reports.state_mut(report);
                state.pos = (round(rect.left()), round(rect.top()));
                state.size = (round(rect.width()), round(rect.height()));
            }
            if !open {
                self.app.close_report();
            }
        }

        // `BattleVCR` (`hwndVCRDlg`) is a window over the Battle Summary
        // Report, not a screen. Closing it is what lets another recording be
        // opened — the original will not swap one for another either.
        if self.app.vcr.is_some() {
            let mut open = true;
            egui::Window::new(stars_ui::dialog::BATTLE_VCR.caption)
                .open(&mut open)
                .resizable(true)
                .default_width(420.0)
                .show(ctx, |ui| stars_ui::views::battles::view(&mut self.app, ui));
            if !open {
                self.app.vcr = None;
                self.app.playing = false;
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
                    // Resource `0x6d4`, in its order: New, Custom Race
                    // Wizard, Open, Close, Save, Save And Submit, a rule,
                    // Print Map, a rule, Exit. The three with nothing behind
                    // them are left out and listed in `menus.md`; what this
                    // project adds of its own comes after the originals.
                    if ui
                        .add(egui::Button::new("New…").shortcut_text("Ctrl+N"))
                        .clicked()
                    {
                        ui.close_menu();
                        self.app.setup = Some(stars_core::newgame::NewGame::default());
                    }
                    if ui
                        .button("Custom Race Wizard…")
                        .on_hover_text(
                            "Design a race and write it out as a .r file, which a \
                             new game can then start a player from.",
                        )
                        .clicked()
                    {
                        ui.close_menu();
                        self.app.open_race_wizard();
                    }
                    if ui
                        .add(egui::Button::new("Open…").shortcut_text("Ctrl+O"))
                        .clicked()
                    {
                        ui.close_menu();
                        self.pick_file();
                    }
                    let open = self.app.game.is_some();
                    if ui
                        .add_enabled(open, egui::Button::new("Save").shortcut_text("Ctrl+S"))
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
                    // `InitializeMenu` rebuilds this every time the menu
                    // drops: one item per remembered game, captioned with
                    // its number and its whole path, inserted by position
                    // just above Exit.
                    if !self.recent.is_empty() {
                        ui.separator();
                        let mut reopen = None;
                        for index in 0..self.recent.paths().len() {
                            let Some(caption) = self.recent.caption(index) else {
                                continue;
                            };
                            // The `&` marks the accelerator in Windows; egui
                            // draws the text as it is given, so it goes.
                            if ui.button(caption.replacen('&', "", 1)).clicked() {
                                reopen = Some(index);
                            }
                        }
                        if let Some(index) = reopen {
                            ui.close_menu();
                            if let Some(path) =
                                self.recent.paths().get(index).map(PathBuf::from)
                            {
                                self.open_path(&path);
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                // The original's View menu. Only its one checkable item so
                // far — the rest of what it holds (Toolbar, Zoom, Window
                // Layout, Race, Game Parameters) has no home here yet.
                // The View menu, in the original's own order: Toolbar, a
                // rule, Find, the two submenus, Player Colors, a rule, then
                // Race and Game Parameters.
                ui.menu_button("View", |ui| {
                    let playing = self.app.game.is_some() && self.app.setup.is_none();

                    let mut shown = self.app.toolbar_visible();
                    if ui
                        .checkbox(&mut shown, "Toolbar")
                        .on_hover_text(
                            "Hide the scanner's toolbar to make room. Most of what \
                             it does is on these menus too.",
                        )
                        .changed()
                    {
                        self.app.toolbar_hidden = !shown;
                    }
                    ui.separator();

                    if ui
                        .add_enabled(playing, egui::Button::new("Find…").shortcut_text("Ctrl+F"))
                        .clicked()
                    {
                        ui.close_menu();
                        self.app.find_open = true;
                    }

                    // Both submenus carry a check mark on the current
                    // choice, which `InitializeMenu` sets by **position** —
                    // `CheckMenuItem(zoom, iScanZoom + 4, MF_BYPOSITION)`
                    // and the same for `iWindowLayout`.
                    ui.menu_button("Zoom", |ui| {
                        for (step, percent) in stars_ui::App::ZOOM_PERCENT.iter().enumerate() {
                            let zoom = i8::try_from(step).unwrap_or(4) - 4;
                            let mark = if self.app.scan_zoom == zoom { "\u{2713} " } else { "    " };
                            if ui.button(format!("{mark}{percent}%")).clicked() {
                                self.app.scan_zoom = zoom;
                                ui.close_menu();
                            }
                        }
                    });

                    ui.menu_button("Window Layout", |ui| {
                        for layout in stars_ui::WindowLayout::ALL {
                            let mark = if self.app.window_layout == layout {
                                "\u{2713} "
                            } else {
                                "    "
                            };
                            if ui.button(format!("{mark}{}", layout.name())).clicked() {
                                self.app.window_layout = layout;
                                ui.close_menu();
                            }
                        }
                    });

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
                    ui.separator();

                    if ui
                        .add_enabled(playing, egui::Button::new("Race…").shortcut_text("F8"))
                        .clicked()
                    {
                        ui.close_menu();
                        let me = self.app.local_player();
                        self.app.open_race_viewer(me);
                    }
                    if ui
                        .add_enabled(playing, egui::Button::new("Game Parameters…"))
                        .clicked()
                    {
                        ui.close_menu();
                        self.app.game_parameters = true;
                    }
                });
                // The original's menu bar has six menus, and the four after
                // File and View — Turn, Commands, Report and Help — are
                // shared with the tutorial's test harness, which needs to
                // press what the pages name; `views::menubar`.
                if let Some(why) = stars_ui::views::menubar::game_menus(&mut self.app, ui) {
                    self.written = vec![why];
                }

                // Find lives in the original's View menu, not on the
                // scanner's toolbar, so it sits in this frontend's own menu
                // bar rather than cluttering the toolbar with a control the
                // original does not have there.
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

        // The game screen — the three panes down the left, the dialogs over
        // the map, the map, and the tutor's ring last of all — is
        // `views::frame`, shared with the tutorial's test harness so that
        // what the test presses is what the player sees.
        let (action, _) = stars_ui::views::frame::game_screen(&mut self.app, ctx);
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
    // `InitInstance` (`1000:0d70`) hands the stored rectangle straight to
    // `CreateWindow` as x, y, width and height, and asks for a maximised
    // window when the file said so — or when it said nothing it could read,
    // which is the only key that defaults that way.
    let frame = stars_ui::settings::frame_window(&read_ini());
    let mut viewport = egui::ViewportBuilder::default()
        .with_min_inner_size([720.0, 480.0])
        .with_title("Stars!");
    match frame.size() {
        Some((width, height)) => {
            viewport = viewport.with_inner_size([f32::from(width), f32::from(height)]);
        }
        None => viewport = viewport.with_inner_size([1200.0, 800.0]),
    }
    if let Some((left, top)) = frame.position() {
        viewport = viewport.with_position([f32::from(left), f32::from(top)]);
    }
    if stars_ui::settings::frame_starts_maximised(frame) {
        viewport = viewport.with_maximized(true);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Stars!",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            // `[Fonts]`, which the original reads and never writes.
            install_font(
                &cc.egui_ctx,
                &stars_ui::settings::Fonts::read_ini(&read_ini()),
            );
            Ok(Box::new(StarsApp::new(open)))
        }),
    )
}

/// Where this project keeps what the original keeps in `stars.ini`.
///
/// The original writes `stars.ini` in the Windows directory, which has no
/// equivalent here; this is the same file, in the same format, in the place
/// each system keeps a program's settings — `%APPDATA%` on Windows,
/// `$XDG_CONFIG_HOME` or `~/.config` elsewhere.
fn ini_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("stars-like-rust").join("stars.ini"))
}

/// Read the settings file. A missing or unreadable one is an empty file —
/// the original treats every missing key as its default too.
fn read_ini() -> stars_ui::settings::Ini {
    ini_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| stars_ui::settings::Ini::parse(&text))
        .unwrap_or_default()
}

/// Write it back, whole. Failing costs nothing but the settings.
///
/// Everything the file holds that this project has no use for is carried
/// through by [`stars_ui::settings::Ini`], so a `stars.ini` the original
/// wrote is not damaged by passing through here.
fn write_ini(ini: &stars_ui::settings::Ini) {
    let Some(path) = ini_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, ini.to_string());
}

/// Find the file a `[Fonts]` name refers to, if it is somewhere fonts
/// usually live.
///
/// The original hands the name to `CreateFontIndirect` and lets Windows
/// match it. egui has no font matcher: it wants the bytes. So a name is
/// honoured when it names a file that can be found — an outright path, or
/// `<name>.ttf` (with and without its spaces) in the usual directories —
/// and otherwise the built-in face stands, which is what a Windows box
/// without Arial installed would have done too.
fn find_font(name: &str) -> Option<Vec<u8>> {
    let direct = Path::new(name);
    if direct.is_absolute() && direct.is_file() {
        return std::fs::read(direct).ok();
    }
    let bare = name.replace(' ', "");
    let mut roots: Vec<PathBuf> = vec![
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
        PathBuf::from("C:\\Windows\\Fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(&home).join("Library/Fonts"));
        roots.push(PathBuf::from(&home).join(".fonts"));
        roots.push(PathBuf::from(home).join(".local/share/fonts"));
    }
    for root in roots {
        for stem in [name, bare.as_str()] {
            for extension in ["ttf", "ttc", "otf", "TTF"] {
                let path = root.join(format!("{stem}.{extension}"));
                if path.is_file() {
                    return std::fs::read(path).ok();
                }
            }
        }
    }
    None
}

/// Install the `[Fonts]` regular face, when its file can be found.
///
/// Only the first of the four: this project draws no bold or italic
/// proportional text — egui's bundled set has no such face, which
/// `crate::popup` already notes — so the other three are read, kept and
/// reported, and nothing yet asks for them.
fn install_font(ctx: &egui::Context, fonts: &stars_ui::settings::Fonts) {
    let Some(bytes) = find_font(fonts.regular()) else {
        return;
    };
    let mut set = egui::FontDefinitions::default();
    set.font_data
        .insert("stars-ini".to_string(), egui::FontData::from_owned(bytes));
    set.families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "stars-ini".to_string());
    ctx.set_fonts(set);
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

#[cfg(test)]
mod tests {
    /// A window drawn inside a key handler is drawn only on the frame that key
    /// is pressed, so it flashes and vanishes.
    ///
    /// This has happened twice — first with Find, Race and Game Parameters
    /// inside the F7 handler, then with the password prompt, Host Mode and
    /// Change Password inside F6 — because a new window is easy to anchor on
    /// the wrong neighbour. The modeless windows all belong at the top level of
    /// `update`, and this checks that they are there.
    #[test]
    fn no_window_is_drawn_inside_a_key_handler() {
        let source = include_str!("app.rs");
        let mut depth: i32 = 0;
        // The depth a key handler's body sits at, while one is open.
        let mut handler: Option<i32> = None;
        let mut offenders = Vec::new();

        for (number, line) in source.lines().enumerate() {
            // Braces inside strings and comments are not structure.
            let code = line.split("//").next().unwrap_or("");
            let code: String = code.split('"').step_by(2).collect::<Vec<_>>().join("");

            if code.contains("key_pressed(") {
                handler = Some(depth);
            }
            if handler.is_some_and(|at| depth > at) && code.contains("egui::Window::new") {
                offenders.push(format!("line {}: {}", number + 1, line.trim()));
            }
            depth += i32::try_from(code.matches('{').count()).unwrap_or(0);
            depth -= i32::try_from(code.matches('}').count()).unwrap_or(0);
            if handler.is_some_and(|at| depth <= at) && !code.contains("key_pressed(") {
                handler = None;
            }
        }

        assert!(
            offenders.is_empty(),
            "these windows only draw on the frame their key is pressed:\n{}",
            offenders.join("\n")
        );
    }
}
