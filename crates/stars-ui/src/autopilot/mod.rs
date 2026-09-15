//! The tutorial, played through the interface itself — the driver.
//!
//! `tests/tutorial_ui.rs` drives the tutorial through the panes: every
//! frame is laid out by egui as the shell lays it out, and each step is a
//! press on a button where the pane drew it, or a shift-click on the map
//! where the scanner drew the planet. So it fails when a button the page
//! names is not there to be pressed — cut off by its tile, disabled, or
//! drawn somewhere else.
//!
//! The buttons are found through [`crate::app::DrawnWidget`], which every
//! pane button records itself in as it is drawn. [`Shell`] is the headless
//! shell, [`script::tutorial`] the pages, and a [`Sink`] is where the
//! frames go — a film on disk ([`Recorder`]), or a window watching the run
//! at a human pace (`stars-desktop`'s `autoplay_tutorial` example).

use crate::App;

pub mod script;

/// Where the frames of a run go.
pub trait Sink {
    /// One frame, tessellated: its texture changes and its meshes, and
    /// whether the tutor turned a page this frame.
    fn frame(
        &mut self,
        delta: &egui::TexturesDelta,
        primitives: &[egui::ClippedPrimitive],
        page_turned: bool,
    );
    /// A step of the script is about to happen — a press, a click, a
    /// generate. A live watcher waits here so the run reads at a human
    /// pace; a film does not.
    fn step(&mut self, _what: &str) {}
}

/// A headless shell: the app, an egui context, and the input for the next
/// frame.
pub struct Shell {
    /// The app being driven.
    pub app: App,
    ctx: egui::Context,
    events: Vec<egui::Event>,
    /// The modifiers held for the next click.
    pub modifiers: egui::Modifiers,
    /// The clock, in seconds: a frame is a sixtieth, and every click is a
    /// second after the last, or egui would take each one for the third of
    /// a triple.
    time: f64,
    /// Where the halo was painted this frame, if anywhere.
    halo: Option<egui::Rect>,
    /// Whether a press may send the page's bold back up: off by default,
    /// and on while a page's own task cannot be finished in this world
    /// and the rest of the year is being played around it.
    pub off_the_page: bool,
    /// Where each frame goes — a film, or a window watching the run.
    sink: Option<Box<dyn Sink>>,
}

impl Shell {
    pub fn new(app: App) -> Self {
        // `STARS_TUTORIAL_VIDEO=path` films the run: every frame's shapes,
        // rasterised, as raw RGBA at 1920 by 1080 for ffmpeg to encode.
        let sink = std::env::var("STARS_TUTORIAL_VIDEO")
            .ok()
            .map(|path| Box::new(Recorder::new(&path)) as Box<dyn Sink>);
        Self::with_sink(app, sink)
    }

    /// A shell whose frames go to `sink` — a window watching the run, say.
    /// The game's own pictures and text are used when a copy of the
    /// original is beside the sources, as they would be on the desktop.
    pub fn with_sink(mut app: App, sink: Option<Box<dyn Sink>>) -> Self {
        if sink.is_some() {
            let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
            for name in ["stars.2.7j.exe", "stars.exe", "STARS!.EXE"] {
                if let Ok(bytes) = std::fs::read(root.join(name)) {
                    let _ = app.load_art(bytes, "the film's copy");
                    break;
                }
            }
        }
        Shell {
            app,
            ctx: egui::Context::default(),
            events: Vec::new(),
            modifiers: egui::Modifiers::NONE,
            time: 0.0,
            halo: None,
            off_the_page: false,
            sink,
        }
    }

    /// Tell the sink a step of the script is about to happen, so a live
    /// watcher can pace the run.
    pub fn step(&mut self, what: &str) {
        if let Some(sink) = self.sink.as_mut() {
            sink.step(what);
        }
    }

    /// One frame, laid out as the desktop shell lays it out: the message
    /// pane along the foot, the three panes down the left, the map in the
    /// middle and the tutor window over it — and then, as the shell does,
    /// the question of whether the page's task is done.
    pub fn frame(&mut self) {
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
        let mut halo = None;
        let page_before = app.tutor.as_ref().map(|t| t.page());
        let output = self.ctx.run(input, |ctx| {
            // The menu bar's game menus, then the game screen exactly as
            // the desktop draws it — `views::frame` — so the panes, the
            // dialogs and the tutor's window are where a player has them.
            egui::TopBottomPanel::top("menubar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    crate::views::menubar::game_menus(app, ui);
                });
            });
            let (_, ring) = crate::views::frame::game_screen(app, ctx);
            halo = ring;
        });
        self.halo = halo;
        if let Some(sink) = self.sink.as_mut() {
            let page = app.tutor.as_ref().map(|t| t.page());
            let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
            sink.frame(&output.textures_delta, &primitives, page != page_before);
        }
    }

    /// A left click at a point: the pointer arrives, presses on the next
    /// frame and lets go on the one after, which is the shape egui reads as
    /// a click.
    pub fn click_at(&mut self, at: egui::Pos2) {
        self.step("click");
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
    pub fn press(&mut self, scope: &str, label: &str) {
        self.step(&format!("{scope}: {label}"));
        self.frame();
        self.assert_halo_somewhere(scope, label);
        let before = (self.page(), self.app.tutor_bold());
        self.press_unchecked(scope, label);
        // Following the pages, the bold never goes back up the page: a
        // step done stays done. Page 14 once sent it back to "Goto 90210"
        // when Teamster #4 was picked.
        let after = (self.page(), self.app.tutor_bold());
        if after.0 == before.0 && !self.off_the_page {
            assert!(
                after.1 >= before.1,
                "the bold went back from {:?} to {:?} on page {} after {scope}/{label}",
                before.1,
                after.1,
                after.0
            );
        }
    }

    /// The press itself, without the checks around it.
    pub fn press_unchecked(&mut self, scope: &str, label: &str) {
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

    /// Where two planets are on the screen at once: the map is centred
    /// between them first, so bringing one in does not push the other out.
    pub fn two_planets_on_screen(&mut self, a: i16, b: i16) -> (egui::Pos2, egui::Pos2) {
        let at = |app: &App, id: i16| {
            let game = app.game.as_ref().expect("a game");
            game.planets
                .iter()
                .chain(game.known_planets.iter())
                .find(|p| p.id == id)
                .and_then(|p| p.position)
                .expect("a placed planet")
        };
        let (pa, pb) = (at(&self.app, a), at(&self.app, b));
        self.app.scan_center = Some(stars_core::movement::Point::new(
            i16::midpoint(pa.x, pb.x),
            i16::midpoint(pa.y, pb.y),
        ));
        self.frame();
        // Too far apart for the map at this zoom: zoom out until both fit,
        // as a player would with the toolbar's Zoom.
        loop {
            let map = self.app.map_frame.expect("the scanner drew the map");
            let fits = map.rect.contains(map.to_screen(pa.x, pa.y))
                && map.rect.contains(map.to_screen(pb.x, pb.y));
            if fits || self.app.scan_zoom <= -4 {
                break;
            }
            self.app.scan_zoom -= 1;
            self.frame();
        }
        let first = self.planet_on_screen(a);
        let second = self.planet_on_screen(b);
        // Bringing the second in may have moved the map; the first is
        // asked again, where it now is.
        let first_again = self.planet_on_screen(a);
        if first_again != first {
            let second = self.planet_on_screen(b);
            return (self.planet_on_screen(a), second);
        }
        (first, second)
    }

    /// Where a planet is on the screen this frame.
    pub fn planet_on_screen(&mut self, planet: i16) -> egui::Pos2 {
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
        self.point_on_screen(at, planet)
    }

    /// Where one of our fleets is on the screen this frame — in deep space,
    /// where there is no planet to right-click.
    pub fn fleet_on_screen(&mut self, id: u16) -> egui::Pos2 {
        self.frame();
        let at = {
            let game = self.app.game.as_ref().expect("a game");
            game.fleets
                .iter()
                .find(|f| f.owner == 0 && f.id == id)
                .map(|f| f.position)
                .expect("our fleet")
        };
        self.point_on_screen(at, i16::try_from(id).unwrap_or(0))
    }

    /// Where a point of the map is on the screen, brought into view and
    /// out from under the tutor window as a player would.
    pub fn point_on_screen(&mut self, at: stars_core::movement::Point, planet: i16) -> egui::Pos2 {
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
    pub fn tutor_window(&self) -> Option<egui::Rect> {
        self.ctx
            .memory(|m| m.area_rect(egui::Id::new("Stars! Tutor")))
    }

    /// Queue a raw input event for the next frame.
    pub fn push_event(&mut self, event: egui::Event) {
        self.events.push(event);
    }

    /// A left-drag from one point to another.
    pub fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
        self.events.push(egui::Event::PointerMoved(from));
        self.frame();
        self.events.push(egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: self.modifiers,
        });
        self.frame();
        // In steps, so egui sees a drag rather than a jump — and the first
        // step a few pixels only, as a hand's is, so what is grabbed is
        // still under the pointer when the drag is seen to begin.
        // Eight pixels: past egui's six, where a press becomes a drag, and
        // inside the twenty a waypoint is grabbed from.
        let first = from + (to - from).normalized() * 8.0;
        self.events.push(egui::Event::PointerMoved(first));
        self.frame();
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
    pub fn right_click_planet_and_pick(&mut self, planet: i16, entry: &str) {
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
    pub fn shift_click_planet(&mut self, planet: i16) {
        let pos = self.planet_on_screen(planet);
        self.modifiers = egui::Modifiers::SHIFT;
        self.click_at(pos);
        self.modifiers = egui::Modifiers::NONE;
    }

    /// Roll the wheel over a list until one of its rows is in view — a row
    /// below the fold of a list box, as the original's own list boxes fold
    /// after ten rows.
    pub fn scroll_to(&mut self, scope: &str, label: &str, over: &str) {
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
    pub fn right_click(&mut self, scope: &str, label: &str) {
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
    pub fn double_click(&mut self, scope: &str, label: &str) {
        self.frame();
        let widget = self
            .app
            .drawn_button(scope, label)
            .unwrap_or_else(|| panic!("no {label:?} in the {scope} pane"))
            .clone();
        assert!(
            widget.visible,
            "{scope}'s {label:?} is drawn at {:?} but cut off by its list",
            widget.rect
        );
        self.double_click_at(widget.rect.center());
    }

    /// A double-click at a point on the screen.
    pub fn double_click_at(&mut self, at: egui::Pos2) {
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
    pub fn press_with(&mut self, modifiers: egui::Modifiers, scope: &str, label: &str) {
        self.modifiers = modifiers;
        self.press(scope, label);
        self.modifiers = egui::Modifiers::NONE;
    }

    /// Generate from the Turn menu, which is where the ring sends a
    /// player whose year is done ("Press F9" being a key): the page must
    /// be waiting on the turn, with the ring on the menu, and it turns
    /// once the year has been generated.
    pub fn generate(&mut self) {
        self.frame();
        assert!(
            self.app.tutor_waiting(),
            "page {} is waiting on the turn",
            self.page()
        );
        let page = self.page();
        self.assert_halo_on("menu", "Turn");
        self.press("menu", "Turn");
        self.assert_halo_on("menu", "Generate");
        self.press("menu", "Generate");
        assert_ne!(self.page(), page, "the year turned the page");
    }

    /// Generate the year without waiting for the page to be done — what a
    /// player does when a page asks for something this world has not got
    /// (`docs/ui/tutorial.md`, *The run, to the end*). The tutor counts
    /// the year's pages done and goes on to the next year's.
    pub fn generate_anyway(&mut self, why: &str) {
        self.frame();
        self.step(&format!("generate: {why}"));
        self.press_unchecked("menu", "Turn");
        self.press_unchecked("menu", "Generate");
    }

    pub fn page(&self) -> usize {
        self.app.tutor.as_ref().expect("running").page()
    }

    /// Press Next until the message in front points where the page says
    /// to Goto — and Prev, when the year's messages came in another order
    /// than the original's and the one wanted is behind.
    pub fn next_message_until(&mut self, target: stars_core::message::Goto) {
        for _ in 0..40 {
            self.frame();
            if self.app.message_goto() == target {
                return;
            }
            if self.app.message_next(false).is_none() {
                break;
            }
            self.press("messages", "Next");
        }
        for _ in 0..40 {
            self.frame();
            if self.app.message_goto() == target {
                return;
            }
            if self.app.message_previous(false).is_none() {
                break;
            }
            self.press_unchecked("messages", "Prev");
        }
        panic!(
            "no message pointing at {target:?}: {:?}",
            self.app.messages()
        );
    }

    /// Take a fleet of ours in hand from the message that points at it —
    /// or, when no message this year does (a world that has drifted from
    /// the original's by a year), by its number, as View/Find would.
    pub fn goto_fleet_by_message_or_number(&mut self, id: u16) {
        let pointed = self.app.game.as_ref().is_some_and(|game| {
            game.messages
                .iter()
                .any(|m| m.player == 0 && m.goto(&[1]) == stars_core::message::Goto::Fleet(id))
        });
        if pointed {
            self.next_message_until(stars_core::message::Goto::Fleet(id));
            self.press("messages", "Goto");
        } else {
            self.step(&format!("find fleet {id} by number"));
            assert!(self.app.goto_fleet(id), "fleet {id} is ours");
            self.frame();
        }
    }

    /// Press Next until the message in front is of the kind named.
    pub fn next_message_until_id(&mut self, id: u16) {
        for _ in 0..12 {
            self.frame();
            if self.app.current_message().map(|m| m.id) == Some(id) {
                return;
            }
            self.press("messages", "Next");
        }
        panic!("no message of kind {id}: {:?}", self.app.messages());
    }

    /// Whatever the page is waiting on, the ring is on **something** —
    /// or the thing wanted is one the ring cannot reach: a fleet of
    /// another player's that is not in view, or the salvage of a battle
    /// this world fought in orbit rather than in space (page 38, where
    /// the minerals went onto Hiho). A reader following the pages must
    /// never be left with a bold paragraph and no ring, which is what
    /// page 4 looked like from the desktop.
    pub fn assert_halo_somewhere(&self, scope: &str, label: &str) {
        use crate::tutorial::Check;
        let Some(check) = self.app.tutor_pending() else {
            return;
        };
        if self.app.tutor_target().is_some() {
            return;
        }
        let no_salvage = self
            .app
            .game
            .as_ref()
            .is_some_and(|g| !g.packets.iter().any(|p| p.warp == 0));
        // A fleet of the player's own that this world has not built —
        // page 35's Teamster #12, which the original's Stove Top could
        // afford and this one could not.
        let unbuilt = |id: i16| {
            let me = i16::try_from(self.app.local_player()).unwrap_or(-1);
            self.app.game.as_ref().is_some_and(|g| {
                !g.fleets
                    .iter()
                    .any(|f| f.owner == me && i16::try_from(f.id) == Ok(id))
            })
        };
        let excused = match check {
            Check::Summary { class: 2, id } | Check::Selection { class: 2, id } => {
                *id >= 0x200 || unbuilt(*id)
            }
            Check::Cargo { fleet, .. }
            | Check::TransportWaypoint { fleet, .. }
            | Check::FleetWaypoint { fleet, .. }
            | Check::ColonizeWaypoint { fleet, .. }
            | Check::RepeatOrders { fleet }
            | Check::Fuel { fleet, .. }
            | Check::FleetOrders { fleet, .. } => unbuilt(i16::try_from(*fleet).unwrap_or(-1)),
            Check::Summary { class: 8, id: -1 } => no_salvage,
            // "Not ten fleets any more" is how page 65 sees Split All
            // done; a world that happens to have ten after the split
            // cannot show it.
            Check::FleetCount {
                count,
                cmp: crate::tutorial::Cmp::NotExactly,
            } => self
                .app
                .game
                .as_ref()
                .is_some_and(|g| g.fleets.iter().filter(|f| f.owner == 0).count() == *count),
            _ => false,
        };
        assert!(
            excused,
            "page {} is waiting on {check:?} and nothing is ringed, before {scope}/{label}",
            self.page()
        );
    }

    /// The halo rings the widget named — the thing the page wants pressed.
    pub fn assert_halo_on(&mut self, scope: &str, label: &str) {
        // A pane scrolls a ringed widget into view on the frame after it
        // finds it out of view, so give it a couple.
        for _ in 0..3 {
            self.frame();
            if self
                .app
                .drawn_button(scope, label)
                .is_some_and(|w| w.visible)
            {
                break;
            }
        }
        let widget = self
            .app
            .drawn_button(scope, label)
            .unwrap_or_else(|| panic!("no {label:?} in the {scope} pane"))
            .clone();
        let halo = self.halo.unwrap_or_else(|| {
            panic!(
                "no halo, but {scope}'s {label:?} is what the page wants; target {:?}; widget {widget:?}",
                self.app.tutor_target()
            )
        });
        let widget = widget.rect;
        assert!(
            halo.contains_rect(widget),
            "the halo {halo:?} is not around {scope}'s {label:?} at {widget:?}"
        );
    }

    /// The halo rings a planet on the map.
    pub fn assert_halo_on_planet(&mut self, planet: i16) {
        let at = self.planet_on_screen(planet);
        let halo = self.halo.expect("a halo on the map");
        assert!(
            halo.contains(at),
            "the halo {halo:?} is not around {planet:#x} at {at:?}"
        );
    }

    /// The fleet in hand, by number — and it must be the player's own: a
    /// Berserker fleet with the same number once slipped through here.
    pub fn selected_fleet_id(&self) -> Option<u16> {
        self.app.selection.fleet.map(|f| {
            let fleet = &self.app.game.as_ref().expect("a game").fleets[f];
            assert_eq!(
                fleet.owner, 0,
                "fleet {} in hand is somebody else's",
                fleet.id
            );
            fleet.id
        })
    }
}

/// A software rasteriser for egui's output, so a run can be filmed
/// without a window: each frame's meshes are drawn into an RGBA buffer
/// and appended raw to a file, which `ffmpeg -f rawvideo` turns into a
/// video. Vertex colours and textures are egui's premultiplied sRGBA,
/// blended as its shader blends them; the font atlas is sampled nearest.
pub struct Raster {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// The frame, as RGBA rows.
    pub pixels: Vec<u8>,
    textures: std::collections::HashMap<egui::TextureId, (usize, usize, Vec<[u8; 4]>)>,
}

/// A film on disk: every frame of a [`Raster`] appended raw to a file,
/// for `ffmpeg -f rawvideo -pix_fmt rgba` to encode.
pub struct Recorder {
    raster: Raster,
    out: std::io::BufWriter<std::fs::File>,
    frames: usize,
}

impl Sink for Recorder {
    fn frame(
        &mut self,
        delta: &egui::TexturesDelta,
        primitives: &[egui::ClippedPrimitive],
        page_turned: bool,
    ) {
        use std::io::Write;
        self.raster.draw(delta, primitives);
        // A page turn is held for two seconds of film.
        let copies = if page_turned { 24 } else { 1 };
        for _ in 0..copies {
            self.out
                .write_all(&self.raster.pixels)
                .expect("the film's file");
            self.frames += 1;
        }
    }
}

impl Recorder {
    /// A film at 1920 by 1080, written to `path`.
    #[must_use]
    pub fn new(path: &str) -> Self {
        let file = std::fs::File::create(path).expect("the film's file");
        Recorder {
            raster: Raster::new(1920, 1080),
            out: std::io::BufWriter::new(file),
            frames: 0,
        }
    }
}

impl Raster {
    /// A blank frame of the given size.
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Raster {
            width,
            height,
            pixels: vec![0; width * height * 4],
            textures: std::collections::HashMap::new(),
        }
    }

    /// Take the frame's textures on board — whole or as a patch.
    pub fn textures(&mut self, delta: &egui::TexturesDelta) {
        for (id, image) in &delta.set {
            let (size, pixels): ([usize; 2], Vec<[u8; 4]>) = match &image.image {
                egui::ImageData::Color(image) => (
                    image.size,
                    image.pixels.iter().map(|c| c.to_array()).collect(),
                ),
                egui::ImageData::Font(image) => (
                    image.size,
                    image.srgba_pixels(None).map(|c| c.to_array()).collect(),
                ),
            };
            match image.pos {
                None => {
                    self.textures.insert(*id, (size[0], size[1], pixels));
                }
                Some([x, y]) => {
                    if let Some((w, _, existing)) = self.textures.get_mut(id) {
                        for row in 0..size[1] {
                            for col in 0..size[0] {
                                let at = (y + row) * *w + (x + col);
                                if let Some(slot) = existing.get_mut(at) {
                                    *slot = pixels[row * size[0] + col];
                                }
                            }
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.textures.remove(id);
        }
    }

    /// Draw one frame and write it out — held for a moment longer when the
    /// page has just turned, so a viewer can read it.
    /// Draw one frame over the window's ground.
    pub fn draw(&mut self, delta: &egui::TexturesDelta, primitives: &[egui::ClippedPrimitive]) {
        self.textures(delta);
        // The window's ground, as the desktop paints it.
        for px in self.pixels.as_chunks_mut::<4>().0 {
            *px = [0x1b, 0x1b, 0x1b, 0xff];
        }
        for primitive in primitives {
            let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive else {
                continue;
            };
            let clip = primitive.clip_rect;
            let texture = self.textures.get(&mesh.texture_id).cloned();
            for triangle in mesh.indices.as_chunks::<3>().0 {
                let v = [
                    &mesh.vertices[triangle[0] as usize],
                    &mesh.vertices[triangle[1] as usize],
                    &mesh.vertices[triangle[2] as usize],
                ];
                self.triangle(v, clip, texture.as_ref());
            }
        }
    }

    /// One triangle, with barycentric colour and texture coordinates.
    #[allow(clippy::many_single_char_names)]
    pub fn triangle(
        &mut self,
        v: [&egui::epaint::Vertex; 3],
        clip: egui::Rect,
        texture: Option<&(usize, usize, Vec<[u8; 4]>)>,
    ) {
        let (x0, y0) = (v[0].pos.x, v[0].pos.y);
        let (x1, y1) = (v[1].pos.x, v[1].pos.y);
        let (x2, y2) = (v[2].pos.x, v[2].pos.y);
        let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
        if area.abs() < 1e-6 {
            return;
        }
        let left = x0.min(x1).min(x2).max(clip.left()).max(0.0).floor() as usize;
        let right = (x0.max(x1).max(x2).min(clip.right()).min(self.width as f32)).ceil() as usize;
        let top = y0.min(y1).min(y2).max(clip.top()).max(0.0).floor() as usize;
        let bottom = (y0
            .max(y1)
            .max(y2)
            .min(clip.bottom())
            .min(self.height as f32))
        .ceil() as usize;
        if left >= right || top >= bottom {
            return;
        }
        let colour = |vertex: &egui::epaint::Vertex| {
            let [r, g, b, a] = vertex.color.to_array();
            [f32::from(r), f32::from(g), f32::from(b), f32::from(a)]
        };
        let (c0, c1, c2) = (colour(v[0]), colour(v[1]), colour(v[2]));
        for y in top..bottom {
            let py = y as f32 + 0.5;
            for x in left..right {
                let px = x as f32 + 0.5;
                let w0 = ((x1 - px) * (y2 - py) - (x2 - px) * (y1 - py)) / area;
                let w1 = ((x2 - px) * (y0 - py) - (x0 - px) * (y2 - py)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let mut c = [0.0_f32; 4];
                for k in 0..4 {
                    c[k] = c0[k] * w0 + c1[k] * w1 + c2[k] * w2;
                }
                if let Some((tw, th, pixels)) = texture {
                    let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
                    let t = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
                    let tx = ((u * *tw as f32) as usize).min(tw.saturating_sub(1));
                    let ty = ((t * *th as f32) as usize).min(th.saturating_sub(1));
                    let texel = pixels[ty * tw + tx];
                    for k in 0..4 {
                        c[k] = c[k] * f32::from(texel[k]) / 255.0;
                    }
                }
                if c[3] <= 0.0 {
                    continue;
                }
                // Premultiplied over: out = src + dst * (1 - src.a).
                let at = (y * self.width + x) * 4;
                let keep = 1.0 - c[3] / 255.0;
                for (channel, src) in self.pixels[at..at + 3].iter_mut().zip(&c) {
                    let d = f32::from(*channel);
                    *channel = (src + d * keep).round().clamp(0.0, 255.0) as u8;
                }
                self.pixels[at + 3] = 255;
            }
        }
    }
}
