//! The tutorial, played through the interface itself.
//!
//! `tutorial_walkthrough.rs` drives the tutorial through the calls the panes
//! make. This drives it through the panes: every frame is laid out by egui
//! as the shell lays it out, and each step is a press on a button where the
//! pane drew it, or a shift-click on the map where the scanner drew the
//! planet. So it fails when a button the page names is not there to be
//! pressed — cut off by its tile, disabled, or drawn somewhere else — which
//! no amount of calling `select_adjacent_fleet` would notice.
//!
//! The buttons are found through [`stars_ui::app::DrawnWidget`], which every
//! pane button records itself in as it is drawn.

use stars_ui::{App, Screen};

// The worlds the pages send you to, pinned by `tutorial_seed.rs`.
const PRUNE: i16 = 0x0c;
const STOVE_TOP: i16 = 0x0d;
const ALEXANDER: i16 = 0x0f;
const PLANET_90210: i16 = 0x10;
const HIHO: i16 = 0x09;
const NO_VACANCY: i16 = 0x03;
const SLIME: i16 = 0x08;
const WALLABY: i16 = 0x05;
const OXYGEN: i16 = 0x02;
const DWARTE: i16 = 0x15;
const MOBIUS: i16 = 0x13;
const CASTLE: i16 = 0x14;
const MOHOLDI: i16 = 0x07;
const SHAGGY_DOG: i16 = 0x0e;
const SEA_SQUARED: i16 = 0x11;
const RED_STORM: i16 = 0x12;
const BLOOP: i16 = 0x17;
const KALAMAZOO: i16 = 0x16;

/// A headless shell: the app, an egui context, and the input for the next
/// frame.
struct Shell {
    app: App,
    ctx: egui::Context,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
    /// The clock, in seconds: a frame is a sixtieth, and every click is a
    /// second after the last, or egui would take each one for the third of
    /// a triple.
    time: f64,
    /// Where the halo was painted this frame, if anywhere.
    halo: Option<egui::Rect>,
}

impl Shell {
    fn new(app: App) -> Self {
        Shell {
            app,
            ctx: egui::Context::default(),
            events: Vec::new(),
            modifiers: egui::Modifiers::NONE,
            time: 0.0,
            halo: None,
        }
    }

    /// One frame, laid out as the desktop shell lays it out: the message
    /// pane along the foot, the three panes down the left, the map in the
    /// middle and the tutor window over it — and then, as the shell does,
    /// the question of whether the page's task is done.
    fn frame(&mut self) {
        self.time += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1920.0, 1080.0),
            )),
            time: Some(self.time),
            events: std::mem::take(&mut self.events),
            modifiers: self.modifiers,
            ..Default::default()
        };
        let app = &mut self.app;
        app.start_frame();
        app.screen = Screen::Galaxy;
        let mut halo = None;
        let _ = self.ctx.run(input, |ctx| {
            egui::TopBottomPanel::bottom("messages")
                .show(ctx, |ui| stars_ui::views::messages::view(app, ui));
            egui::SidePanel::left("planet")
                .exact_width(420.0)
                .show(ctx, |ui| stars_ui::views::planet::view(app, ui));
            egui::SidePanel::left("fleet")
                .exact_width(420.0)
                .show(ctx, |ui| stars_ui::views::fleet::view(app, ui));
            egui::SidePanel::left("survey")
                .exact_width(200.0)
                .show(ctx, |ui| stars_ui::views::survey::view(app, ui));
            egui::CentralPanel::default().show(ctx, |ui| stars_ui::views::central(app, ui));
            // The tutor window sits where the shell first puts it, in the
            // corner the map does not use.
            if app.tutor.as_ref().is_some_and(|t| !t.hidden) {
                egui::Window::new("Stars! Tutor")
                    .default_pos(egui::pos2(1700.0, 40.0))
                    .show(ctx, |ui| stars_ui::views::tutorial::view(app, ui));
            }
            // The Research and Production dialogs, where the shell puts
            // them: over the map.
            if app.research_dialog.is_some() {
                egui::Window::new("Research")
                    .current_pos(egui::pos2(900.0, 100.0))
                    .show(ctx, |ui| stars_ui::views::research::view(app, ui));
            }
            if app.production.is_some() {
                egui::Window::new("Production")
                    .current_pos(egui::pos2(900.0, 100.0))
                    .default_width(700.0)
                    .show(ctx, |ui| stars_ui::views::production::view(app, ui));
            }
            if app.xfer.is_some() {
                egui::Window::new("Cargo Transfer")
                    .current_pos(egui::pos2(900.0, 100.0))
                    .default_width(stars_ui::dialog::TRANSFER.pixels().x)
                    .show(ctx, |ui| stars_ui::views::transfer::view(app, ui));
            }
            halo = stars_ui::views::tutorial::halo(app, ctx);
        });
        self.halo = halo;
        app.advance_tutor();
    }

    /// A left click at a point: the pointer arrives, presses on the next
    /// frame and lets go on the one after, which is the shape egui reads as
    /// a click.
    fn click_at(&mut self, at: egui::Pos2) {
        self.time += 1.0;
        self.events.push(egui::Event::PointerMoved(at));
        self.frame();
        self.events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: self.modifiers,
        });
        self.frame();
        self.events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: self.modifiers,
        });
        self.frame();
        self.events.push(egui::Event::PointerGone);
        self.frame();
    }

    /// Press the button a pane drew under this caption. It has to be there,
    /// enabled, and wholly in view.
    fn press(&mut self, scope: &str, label: &str) {
        self.frame();
        let button = self
            .app
            .drawn_button(scope, label)
            .unwrap_or_else(|| {
                panic!(
                    "no {label:?} button in the {scope} pane; drawn: {:?}",
                    self.app
                        .drawn
                        .iter()
                        .map(|w| format!("{}/{}", w.scope, w.label))
                        .collect::<Vec<_>>()
                )
            })
            .clone();
        assert!(button.enabled, "{scope}'s {label:?} is greyed out");
        assert!(
            button.visible,
            "{scope}'s {label:?} is drawn at {:?} but cut off by its tile",
            button.rect
        );
        self.click_at(button.rect.center());
    }

    /// Where a planet is on the screen this frame.
    fn planet_on_screen(&mut self, planet: i16) -> egui::Pos2 {
        self.frame();
        let at = {
            let game = self.app.game.as_ref().expect("a game");
            game.planets
                .iter()
                .chain(game.known_planets.iter())
                .find(|p| p.id == planet)
                .and_then(|p| p.position)
                .expect("a placed planet")
        };
        let map = self.app.map_frame.expect("the scanner drew the map");
        let mut pos = map.to_screen(at.x, at.y);
        if !map.rect.contains(pos) {
            // Off the edge of the map: the wheel would bring it into view,
            // and this is where the wheel would leave it.
            self.app.scan_center = Some(at);
            self.frame();
            let map = self.app.map_frame.expect("the scanner drew the map");
            pos = map.to_screen(at.x, at.y);
        }
        assert!(map.rect.contains(pos), "{planet:#x} is on the map");
        // Under the tutor window? Then drag the window to whichever corner
        // of the map is furthest away, as a player would.
        if let Some(window) = self.tutor_window() {
            if window.expand(8.0).contains(pos) {
                let corners = [
                    map.rect.left_top(),
                    map.rect.right_top() - egui::vec2(window.width(), 0.0),
                    map.rect.left_bottom() - egui::vec2(0.0, window.height()),
                    map.rect.right_bottom() - window.size(),
                ];
                let far = corners
                    .into_iter()
                    .max_by(|a, b| {
                        a.distance(pos)
                            .partial_cmp(&b.distance(pos))
                            .expect("finite")
                    })
                    .expect("four corners");
                self.drag(
                    window.min + egui::vec2(20.0, 8.0),
                    far + egui::vec2(20.0, 8.0),
                );
                self.frame();
                let window = self.tutor_window().expect("still up");
                assert!(!window.contains(pos), "the window moved off {planet:#x}");
            }
        }
        pos
    }

    /// Where the tutor window is this frame.
    fn tutor_window(&self) -> Option<egui::Rect> {
        self.ctx
            .memory(|m| m.area_rect(egui::Id::new("Stars! Tutor")))
    }

    /// A left-drag from one point to another.
    fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
        self.events.push(egui::Event::PointerMoved(from));
        self.frame();
        self.events.push(egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: self.modifiers,
        });
        self.frame();
        // In steps, so egui sees a drag rather than a jump.
        for step in 1..=4 {
            let t = step as f32 / 4.0;
            self.events
                .push(egui::Event::PointerMoved(from + (to - from) * t));
            self.frame();
        }
        self.events.push(egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: self.modifiers,
        });
        self.frame();
    }

    /// A right-click on a planet, which raises the menu of what is there,
    /// then a click on one of its entries.
    fn right_click_planet_and_pick(&mut self, planet: i16, entry: &str) {
        let pos = self.planet_on_screen(planet);
        self.events.push(egui::Event::PointerMoved(pos));
        self.frame();
        self.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: self.modifiers,
        });
        self.frame();
        self.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: self.modifiers,
        });
        self.frame();
        assert!(self.app.scan_menu_at.is_some(), "the menu is up");
        self.press("scanner", entry);
    }

    /// Shift-click a planet on the map, which is how a waypoint is laid.
    fn shift_click_planet(&mut self, planet: i16) {
        let pos = self.planet_on_screen(planet);
        self.modifiers = egui::Modifiers::SHIFT;
        self.click_at(pos);
        self.modifiers = egui::Modifiers::NONE;
    }

    /// Roll the wheel over a list until one of its rows is in view — a row
    /// below the fold of a list box, as the original's own list boxes fold
    /// after ten rows.
    fn scroll_to(&mut self, scope: &str, label: &str, over: &str) {
        for _ in 0..40 {
            self.frame();
            let row = self
                .app
                .drawn_button(scope, label)
                .unwrap_or_else(|| panic!("no {label:?} in the {scope} pane"));
            if row.visible {
                return;
            }
            let down = row.rect.top()
                > self
                    .app
                    .drawn_button(scope, over)
                    .expect("a row in view to roll over")
                    .rect
                    .top();
            let at = self
                .app
                .drawn_button(scope, over)
                .expect("a row in view")
                .rect
                .center();
            self.events.push(egui::Event::PointerMoved(at));
            self.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, if down { -3.0 } else { 3.0 }),
                modifiers: self.modifiers,
            });
            self.frame();
        }
        panic!("{label:?} never came into view in the {scope} pane");
    }

    /// A right-click on a drawn widget, which raises whatever menu it has.
    fn right_click(&mut self, scope: &str, label: &str) {
        self.frame();
        let at = self
            .app
            .drawn_button(scope, label)
            .unwrap_or_else(|| panic!("no {label:?} in the {scope} pane"))
            .rect
            .center();
        self.events.push(egui::Event::PointerMoved(at));
        self.frame();
        for pressed in [true, false] {
            self.events.push(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: self.modifiers,
            });
            self.frame();
        }
    }

    /// A double-click on a drawn widget: two clicks, close together.
    fn double_click(&mut self, scope: &str, label: &str) {
        self.frame();
        let at = self
            .app
            .drawn_button(scope, label)
            .unwrap_or_else(|| panic!("no {label:?} in the {scope} pane"))
            .rect
            .center();
        self.double_click_at(at);
    }

    /// A double-click at a point on the screen.
    fn double_click_at(&mut self, at: egui::Pos2) {
        self.time += 1.0;
        self.events.push(egui::Event::PointerMoved(at));
        self.frame();
        for _ in 0..2 {
            for pressed in [true, false] {
                self.events.push(egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: self.modifiers,
                });
                self.frame();
            }
        }
        self.frame();
    }

    /// A click with a modifier held, as shift-Add is.
    fn press_with(&mut self, modifiers: egui::Modifiers, scope: &str, label: &str) {
        self.modifiers = modifiers;
        self.press(scope, label);
        self.modifiers = egui::Modifiers::NONE;
    }

    /// F9: the shell's key, so the year is generated directly.
    fn generate(&mut self) {
        self.app.generate_turn();
        self.frame();
    }

    fn page(&self) -> usize {
        self.app.tutor.as_ref().expect("running").page()
    }

    /// The halo rings the widget named — the thing the page wants pressed.
    fn assert_halo_on(&mut self, scope: &str, label: &str) {
        self.frame();
        let widget = self
            .app
            .drawn_button(scope, label)
            .unwrap_or_else(|| panic!("no {label:?} in the {scope} pane"))
            .rect;
        let halo = self.halo.unwrap_or_else(|| {
            panic!(
                "no halo, but {scope}'s {label:?} is what the page wants; target {:?}",
                self.app.tutor_target()
            )
        });
        assert!(
            halo.contains_rect(widget),
            "the halo {halo:?} is not around {scope}'s {label:?} at {widget:?}"
        );
    }

    /// The halo rings a planet on the map.
    fn assert_halo_on_planet(&mut self, planet: i16) {
        let at = self.planet_on_screen(planet);
        let halo = self.halo.expect("a halo on the map");
        assert!(
            halo.contains(at),
            "the halo {halo:?} is not around {planet:#x} at {at:?}"
        );
    }

    fn selected_fleet_id(&self) -> Option<u16> {
        self.app
            .selection
            .fleet
            .map(|f| self.app.game.as_ref().expect("a game").fleets[f].id)
    }
}

/// Year zero, played by pressing what the pages name where the panes draw
/// it — and the fleet tile's Next button, which page 4 leans on, has to be
/// in view to be pressed.
#[test]
fn year_zero_is_played_through_the_panes() {
    let mut app = App::new();
    app.create_tutor_world(1400).expect("the tutorial's world");
    let mut shell = Shell::new(app);
    shell.frame();
    assert_eq!(shell.page(), 1);

    // Page 1: "click on the Next button" in the Messages pane, four times —
    // and the halo says so.
    shell.assert_halo_on("messages", "Next");
    for _ in 0..4 {
        shell.press("messages", "Next");
    }
    assert_eq!(shell.page(), 2, "all five messages read");

    // Page 2: the Fleets in Orbit tile's Goto, then shift-click Prune. The
    // halo moves from the button to the planet once the probe is in hand.
    shell.assert_halo_on("planet", "Goto");
    shell.press("planet", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(0), "Armed Probe #1 in hand");
    shell.assert_halo_on_planet(PRUNE);
    shell.shift_click_planet(PRUNE);
    {
        let fleet = &shell.app.game.as_ref().expect("a game").fleets[0];
        assert_eq!(
            fleet.waypoints.iter().map(|w| w.target).collect::<Vec<_>>(),
            vec![Some(0x0d), Some(0x0c)],
            "a leg to Prune"
        );
    }
    assert_eq!(shell.page(), 3, "the leg to Prune turned the page");

    // Page 3 says n; page 4 says "press the Next button in the tile showing
    // Long Range Scout #2". Both are the fleet tile's Next, which has to be
    // visible to be pressed.
    shell.press("fleet", "Next");
    assert_eq!(shell.selected_fleet_id(), Some(1), "Long Range Scout #2");
    shell.shift_click_planet(PLANET_90210);
    assert_eq!(shell.page(), 4);

    // Page 4: Next three times, then Alexander.
    for _ in 0..3 {
        shell.press("fleet", "Next");
    }
    assert_eq!(shell.selected_fleet_id(), Some(4), "Stalwart Defender #5");
    shell.shift_click_planet(ALEXANDER);
    assert_eq!(shell.page(), 5);

    // Page 5: round to Armed Probe #1 again, then Weapons in the Research
    // dialog and its **Done** button — the button the page names, which
    // the dialog must therefore have. (F5 is the shell's key, so the dialog
    // is opened directly; the radio button is a plain egui widget and is
    // set the same way.)
    shell.press("fleet", "Next");
    shell.press("fleet", "Next");
    assert_eq!(shell.selected_fleet_id(), Some(0));
    shell.app.open_research();
    shell
        .app
        .research_dialog
        .as_mut()
        .expect("the dialog")
        .field = 1;
    // A window takes a frame or two to grow to its contents — and then it
    // must stop: the dialog once grew thirty pixels every frame until Done
    // had gone off the bottom of the screen.
    let mut done_at = Vec::new();
    for _ in 0..6 {
        shell.frame();
        done_at.push(
            shell
                .app
                .drawn_button("research", "Done")
                .expect("Done is drawn")
                .rect
                .min
                .y,
        );
    }
    assert_eq!(done_at[3], done_at[5], "the window settles: {done_at:?}");
    assert!(
        shell.app.drawn_button("research", "Help").is_some(),
        "Help is drawn beside it"
    );
    shell.press("research", "Done");
    assert!(
        shell.app.research_dialog.is_none(),
        "Done closes the dialog"
    );
    assert_eq!(shell.page(), 6, "the year is done");

    // Prev walks the other way, and is there too.
    shell.press("fleet", "Prev");
    assert_eq!(
        shell.selected_fleet_id(),
        Some(5),
        "Cotton Picker #6, wrapping round"
    );
    shell.generate();
    assert_eq!(shell.app.game.as_ref().expect("a game").turn, 1);
    assert_eq!(shell.page(), 6);

    // --- 2401 -------------------------------------------------------------
    // Page 6: "Press Change on the Production tile. Pick Factory in the
    // list on the left, hold down shift and press Add twice, giving 20
    // factories in all, then press OK."
    // "Read the message in the Messages pane" — the empty queue at Stove
    // Top, whose Goto puts the planet in front of the fleet the pane was
    // left on; the halo points there first, then at Change.
    shell.assert_halo_on("messages", "Goto");
    shell.press("messages", "Goto");
    shell.assert_halo_on("planet", "Change");
    shell.press("planet", "Change");
    assert!(
        shell.app.production.is_some(),
        "the Production dialog is up"
    );
    shell.assert_halo_on("production", "Factory");
    shell.press("production", "Factory");
    shell.assert_halo_on("production", "Add ->");
    shell.press_with(egui::Modifiers::SHIFT, "production", "Add ->");
    shell.press_with(egui::Modifiers::SHIFT, "production", "Add ->");
    shell.assert_halo_on("production", "OK");
    {
        let dialog = shell.app.production.as_ref().expect("still up");
        assert_eq!(dialog.queue.len(), 1, "{:?}", dialog.queue);
        assert_eq!(dialog.queue[0].count, 20, "twenty factories in one row");
    }
    shell.press("production", "OK");
    assert!(shell.app.production.is_none(), "OK closes the dialog");
    assert_eq!(shell.page(), 7, "2401 is done");
    shell.generate();
    assert_eq!(shell.page(), 7);

    // --- 2402 -------------------------------------------------------------
    // Page 7: "Start with your first message and press Goto": Armed Probe
    // #1, arrived at Prune; five stops for it; then the next message's Goto
    // is Long Range Scout #2.
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(0), "Armed Probe #1");
    for planet in [HIHO, NO_VACANCY, SLIME, WALLABY, OXYGEN] {
        shell.shift_click_planet(planet);
    }
    shell.press("messages", "Next");
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(1), "Long Range Scout #2");
    assert_eq!(shell.page(), 8);

    // Page 8: four stops, then the next message's Goto: Stalwart Defender.
    for planet in [DWARTE, MOBIUS, CASTLE, MOHOLDI] {
        shell.shift_click_planet(planet);
    }
    shell.press("messages", "Next");
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(4), "Stalwart Defender #5");
    assert_eq!(shell.page(), 9);

    // Page 9: five stops; "read the next two messages and press Goto so
    // that the Summary pane shows Prune". The fourth message is the
    // factories built at Stove Top; the fifth is the client's own "you have
    // found a planet" for Prune, the first of three this year, and its Goto
    // is the planet.
    for planet in [SHAGGY_DOG, SEA_SQUARED, RED_STORM, BLOOP, KALAMAZOO] {
        shell.shift_click_planet(planet);
    }
    shell.press("messages", "Next");
    shell.press("messages", "Next");
    shell.press("messages", "Goto");
    assert_eq!(
        shell.app.selection.planet,
        Some(PRUNE),
        "Prune in the Summary pane"
    );
    assert_eq!(shell.page(), 10);

    // Page 10: right-click Stove Top and pick Cotton Picker #6; shift-click
    // Prune; then the Waypoint Task dropdown, set to Remote Mining.
    shell.right_click_planet_and_pick(STOVE_TOP, "Cotton Picker #6");
    assert_eq!(shell.selected_fleet_id(), Some(5));
    shell.shift_click_planet(PRUNE);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Remote Mining");
    assert_eq!(shell.page(), 11);

    // Page 11: the next message's Goto is Alexander, found by Stalwart
    // Defender #5; the one after is 90210, found by Long Range Scout #2.
    shell.press("messages", "Next");
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(ALEXANDER));
    shell.press("messages", "Next");
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(PLANET_90210));
    assert_eq!(shell.page(), 12);

    // Page 12: right-click Stove Top and pick Santa Maria #3; press Xfer on
    // the "Orbiting Stove Top" tile; "drag in the Colonists gauge until the
    // hold carries 25kT of colonists" — the far end of the gauge is the
    // whole hold, and the colony ship's hold is 25 — then OK; shift-click
    // 90210; Colonize from the Waypoint Task dropdown.
    shell.right_click_planet_and_pick(STOVE_TOP, "Santa Maria #3");
    assert_eq!(shell.selected_fleet_id(), Some(2));
    shell.assert_halo_on("fleet", "Xfer");
    shell.press("fleet", "Xfer");
    assert!(shell.app.xfer.is_some(), "the Cargo Transfer dialog is up");
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    assert_eq!(
        shell.app.xfer.as_ref().expect("still up").aboard[3],
        25,
        "a full hold of colonists"
    );
    shell.press("xfer", "OK");
    assert!(shell.app.xfer.is_none(), "OK closes it");
    {
        let game = shell.app.game.as_ref().expect("a game");
        let ship = game
            .fleets
            .iter()
            .find(|f| f.owner == 0 && f.id == 2)
            .expect("the ship");
        assert_eq!(ship.cargo.colonists, 25, "and the colonists are aboard");
    }
    shell.shift_click_planet(PLANET_90210);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Colonize");
    assert_eq!(shell.page(), 13, "2402 is done");
    shell.generate();
    assert_eq!(shell.page(), 13);

    // --- 2403 -------------------------------------------------------------
    // Page 13: the first message — factories built — switched off with the
    // check mark at the top left of the Messages pane; the next message's
    // Goto is Stove Top; Change, three shift-Adds of Factories (Auto Build),
    // OK.
    shell.press("messages", "filter");
    shell.press("messages", "Next");
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(STOVE_TOP));
    shell.press("planet", "Change");
    shell.scroll_to("production", "Factories", "Factory");
    shell.press("production", "Factories");
    for _ in 0..3 {
        shell.press_with(egui::Modifiers::SHIFT, "production", "Add ->");
    }
    shell.press("production", "OK");
    assert_eq!(shell.page(), 14);

    // Page 14: the next two messages' Goto is 90210; "press the q key" —
    // the shell's key for Change Production; Factory and Mine double-
    // clicked three times each; the leftover box ticked; OK. Then Teamster
    // #4 from Stove Top's menu, Xfer, the hold filled with colonists, OK.
    shell.press("messages", "Next");
    shell.press("messages", "Next");
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(PLANET_90210));
    shell.app.open_production();
    shell.frame();
    for _ in 0..3 {
        shell.double_click("production", "Factory");
    }
    for _ in 0..3 {
        shell.double_click("production", "Mine");
    }
    shell.press(
        "production",
        "Contribute only leftover resources to research",
    );
    {
        let dialog = shell.app.production.as_ref().expect("open");
        assert_eq!(dialog.queue.len(), 2, "{:?}", dialog.queue);
        assert_eq!((dialog.queue[0].item, dialog.queue[0].count), (7, 3));
        assert_eq!((dialog.queue[1].item, dialog.queue[1].count), (8, 3));
        assert!(dialog.no_research);
    }
    shell.press("production", "OK");
    shell.right_click_planet_and_pick(STOVE_TOP, "Teamster #4");
    assert_eq!(shell.selected_fleet_id(), Some(3));
    shell.press("fleet", "Xfer");
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    assert_eq!(shell.app.xfer.as_ref().expect("up").aboard[3], 210);
    shell.press("xfer", "OK");
    assert_eq!(shell.page(), 15);

    // Page 15: shift-click 90210; Transport; the blue diamond's QuikDrop;
    // the next message's Goto is Hiho; double-click Armed Probe #1 on the
    // map.
    shell.shift_click_planet(PLANET_90210);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Transport");
    shell.right_click("fleet", "blue diamond");
    shell.press("fleet", "QuikDrop");
    shell.press("messages", "Next");
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(HIHO));
    // "Double-click on Armed Probe #1" — the blue triangle just right of
    // Hiho, still on its way.
    let probe = {
        let game = shell.app.game.as_ref().expect("a game");
        game.fleets
            .iter()
            .find(|f| f.owner == 0 && f.id == 0)
            .expect("Armed Probe #1")
            .position
    };
    shell.frame();
    let map = shell.app.map_frame.expect("the map");
    let at = map.to_screen(probe.x, probe.y);
    assert!(map.rect.contains(at), "the probe is on the map");
    shell.double_click_at(at);
    assert_eq!(shell.selected_fleet_id(), Some(0), "Armed Probe #1 in hand");
    assert_eq!(shell.page(), 16);

    // Page 16: "Click Hiho and press the Delete key" — the probe's waypoint
    // at Hiho comes into the map's hand with a click on it, and Delete is
    // the shell's key, so the waypoint in hand is dropped directly.
    let hiho = shell.planet_on_screen(HIHO);
    shell.click_at(hiho);
    assert_eq!(
        shell.app.selection.waypoint,
        Some(1),
        "Hiho is the waypoint in hand"
    );
    shell.app.delete_current_waypoint();
    shell.frame();
    {
        let game = shell.app.game.as_ref().expect("a game");
        let probe = game
            .fleets
            .iter()
            .find(|f| f.owner == 0 && f.id == 0)
            .expect("the probe");
        assert_eq!(
            probe.waypoints[1].target,
            Some(NO_VACANCY as u16),
            "straight on to No Vacancy"
        );
    }
    assert_eq!(shell.page(), 17, "2403 is done");
}

/// The planet pane's own tile has Prev and Next as well — `SelectAdjPlanet`
/// walks the player's planets the way the fleet tile walks the fleets.
#[test]
fn the_planet_tile_walks_the_players_planets() {
    let mut app = App::new();
    app.create_tutor_world(1400).expect("the tutorial's world");
    let mut shell = Shell::new(app);
    shell.frame();
    let home = shell
        .app
        .selection
        .planet
        .expect("the home world is selected");
    // With one planet, Next comes back round to it.
    shell.press("planet", "Next");
    assert_eq!(shell.app.selection.planet, Some(home));
    assert!(!shell.app.selection.on_fleet, "and the planet is in front");
}
