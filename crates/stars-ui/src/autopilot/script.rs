//! The pages, one press at a time.

use super::Shell;
use crate::App;

// The worlds the pages send you to, pinned by `tutorial_seed.rs`.
const PRUNE: i16 = 0x0c;
const STOVE_TOP: i16 = 0x0d;
const ALEXANDER: i16 = 0x0f;
const PLANET_90210: i16 = 0x10;
const HIHO: i16 = 0x09;
const HACKER: i16 = 0x0a;
const NO_VACANCY: i16 = 0x03;
const SLIME: i16 = 0x08;
const WALLABY: i16 = 0x05;
const OXYGEN: i16 = 0x02;
const MOZART: i16 = 0x04;
const DWARTE: i16 = 0x15;
const MOBIUS: i16 = 0x13;
const CASTLE: i16 = 0x14;
const MOHOLDI: i16 = 0x07;
const SHAGGY_DOG: i16 = 0x0e;
const SEA_SQUARED: i16 = 0x11;
const RED_STORM: i16 = 0x12;
const BLOOP: i16 = 0x17;
const KALAMAZOO: i16 = 0x16;

/// A fresh tutorial game for the script to play.
#[must_use]
pub fn tutorial_app() -> App {
    let mut app = App::new();
    app.create_tutor_world(1400).expect("the tutorial's world");
    app
}

/// The tutorial, page by page, by pressing what the pages name where the
/// panes draw it — the fleet tile's Next button, which page 4 leans on,
/// has to be in view to be pressed. Every step asserts what the page
/// asked for, so a run that stalls says where.
pub fn tutorial(shell: &mut Shell) {
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

    // Page 4: Next three times, then Alexander. The ring sits on the
    // tile's Next each time, and the emboldened paragraph follows the
    // fleet in hand — "press Next once more" for the colony ship, "press
    // Next again" for the freighter — as `10f8:0fbc` writes it.
    for (bold, fleet) in [(25, 2), (27, 3), (29, 4)] {
        assert_eq!(shell.app.tutor_bold(), Some(bold));
        shell.assert_halo_on("fleet", "Next");
        shell.press("fleet", "Next");
        assert_eq!(shell.selected_fleet_id(), Some(fleet));
    }
    assert_eq!(shell.app.tutor_bold(), Some(31), "shift-click Alexander");
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
    // "Open Research from the Commands menu": the ring is on the menu, then
    // on its item.
    shell.assert_halo_on("menu", "Commands");
    shell.press("menu", "Commands");
    shell.assert_halo_on("menu", "Research…");
    shell.press("menu", "Research…");
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
    // The year's work is done, and the page stays up on "Press F9 to
    // generate the next one" until it is — `FTutorTaskDone`'s bit 3.
    assert_eq!(shell.page(), 5, "the page waits for the turn");
    assert_eq!(shell.app.tutor_bold(), Some(39));

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
    assert_eq!(shell.page(), 6, "2401 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
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
    assert_eq!(shell.page(), 12, "2402 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
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
    // The queue done, the bold moves on to "pick Teamster #4" and stays
    // there once it is picked — the "Goto 90210" above only decorated the
    // queue, as the original's nesting has it — rather than going back to
    // the top of the page because 90210 is no longer selected.
    assert_eq!(shell.app.tutor_bold(), Some(0x6d));
    shell.right_click_planet_and_pick(STOVE_TOP, "Teamster #4");
    assert_eq!(shell.selected_fleet_id(), Some(3));
    assert_eq!(shell.app.tutor_bold(), Some(0x6e), "the Xfer paragraph");
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
    assert_eq!(shell.page(), 16, "2403 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 17);

    // --- 2404 -------------------------------------------------------------
    // Page 17: the first message's Goto is Shaggy Dog, found by Stalwart
    // Defender #5; double-click the destroyer just above the planet, click
    // its waypoint at Shaggy Dog and Delete; the next message's Goto is
    // Dwarte; double-click Stove Top.
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(SHAGGY_DOG));
    let destroyer = {
        let game = shell.app.game.as_ref().expect("a game");
        game.fleets
            .iter()
            .find(|f| f.owner == 0 && f.id == 4)
            .expect("Stalwart Defender #5")
            .position
    };
    shell.frame();
    let map = shell.app.map_frame.expect("the map");
    let at = map.to_screen(destroyer.x, destroyer.y);
    shell.double_click_at(at);
    assert_eq!(shell.selected_fleet_id(), Some(4));
    let shaggy = shell.planet_on_screen(SHAGGY_DOG);
    shell.click_at(shaggy);
    assert_eq!(shell.app.selection.waypoint, Some(1));
    shell.app.delete_current_waypoint();
    shell.frame();
    shell.press("messages", "Next");
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(DWARTE));
    let home = shell.planet_on_screen(STOVE_TOP);
    shell.double_click_at(home);
    assert_eq!(shell.app.selection.planet, Some(STOVE_TOP));
    assert_eq!(shell.page(), 18);

    // Page 18: Change; double-click Santa Maria; OK.
    shell.press("planet", "Change");
    shell.double_click("production", "Santa Maria");
    shell.press("production", "OK");
    assert_eq!(shell.page(), 18, "2404 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 19);

    // --- 2405 -------------------------------------------------------------
    // Page 19: the first message's Goto is the new Santa Maria; a click in
    // the cargo gauge on the Fuel & Cargo tile is Xfer; fill with
    // colonists, OK; the % toolbar button; shift-click Shaggy Dog.
    shell.press("messages", "Goto");
    assert_eq!(
        shell.selected_fleet_id(),
        Some(2),
        "the new colony ship is fleet #3 again"
    );
    shell.press("fleet", "Cargo gauge");
    assert!(
        shell.app.xfer.is_some(),
        "the cargo gauge opens the Cargo Transfer dialog"
    );
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    shell.press("xfer", "OK");
    shell.press("toolbar", "%");
    assert_eq!(shell.app.scan_view, crate::ScanView::PlanetValue);
    shell.shift_click_planet(SHAGGY_DOG);
    assert_eq!(shell.page(), 20);

    // Page 20: Colonize; the leftmost toolbar button; the next two
    // messages' Goto is Teamster #4, sent home; 90210 from the "Orbiting
    // 90210" tile's Goto; the next message, then Armed Probe #1's waypoint
    // at No Vacancy deleted.
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Colonize");
    shell.press("toolbar", "Nml");
    assert_eq!(shell.app.scan_view, crate::ScanView::Normal);
    // The year's news here runs: the ship built, the Teamster's colonists
    // beamed down at 90210, then the two planets found. The page reads two
    // and Gotos the Teamster, so its message is read up to and followed.
    shell.next_message_until(stars_core::message::Goto::Fleet(3));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(3), "Teamster #4");
    shell.shift_click_planet(STOVE_TOP);
    shell.press("fleet", "Goto");
    assert_eq!(shell.app.selection.planet, Some(PLANET_90210));
    assert!(!shell.app.selection.on_fleet);
    // The next message is No Vacancy, found; the probe's waypoint there
    // wants deleting unless the probe has already reached it, in which
    // case the waypoint went on its own and the page is done.
    shell.press("messages", "Next");
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(NO_VACANCY));
    if shell.page() == 20 {
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
        shell.double_click_at(map.to_screen(probe.x, probe.y));
        assert_eq!(shell.selected_fleet_id(), Some(0), "Armed Probe #1");
        let no_vacancy = shell.planet_on_screen(NO_VACANCY);
        shell.click_at(no_vacancy);
        assert_eq!(shell.app.selection.waypoint, Some(1));
        shell.app.delete_current_waypoint();
        shell.frame();
    }
    assert_eq!(shell.page(), 20, "2405 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 21);

    // --- 2406 -------------------------------------------------------------
    // Page 21: the first message's Goto is Teamster #4, home; shift-click
    // Prune; Transport; QuikLoad from the blue diamond; shift-click Stove
    // Top again.
    shell.next_message_until(stars_core::message::Goto::Fleet(3));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(3), "Teamster #4");
    shell.shift_click_planet(PRUNE);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Transport");
    shell.right_click("fleet", "blue diamond");
    shell.press("fleet", "QuikLoad");
    shell.shift_click_planet(STOVE_TOP);
    assert_eq!(shell.page(), 22);

    // Page 22: QuikDrop for the leg home; tick Repeat Orders in the Fleet
    // Waypoints tile; select Stove Top; a Santa Maria in its queue.
    shell.right_click("fleet", "blue diamond");
    shell.press("fleet", "QuikDrop");
    shell.press("fleet", "Repeat Orders");
    {
        let game = shell.app.game.as_ref().expect("a game");
        let teamster = game
            .fleets
            .iter()
            .find(|f| f.owner == 0 && f.id == 3)
            .expect("Teamster");
        assert!(teamster.repeat_orders, "Repeat Orders is ticked");
    }
    let home = shell.planet_on_screen(STOVE_TOP);
    shell.click_at(home);
    assert_eq!(shell.app.selection.planet, Some(STOVE_TOP));
    assert!(!shell.app.selection.on_fleet);
    shell.press("planet", "Change");
    shell.double_click("production", "Santa Maria");
    shell.press("production", "OK");
    assert_eq!(shell.page(), 22, "2406 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 23);

    // --- 2407 -------------------------------------------------------------
    // Page 23: the first message's Goto is the new Santa Maria; load it;
    // the % button; Colonize at Red Storm; three more Santa Marias in
    // Stove Top's queue; the leftmost button.
    shell.next_message_until(stars_core::message::Goto::Fleet(6));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(6), "Santa Maria #7");
    shell.press("fleet", "Xfer");
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    shell.press("xfer", "OK");
    shell.press("toolbar", "%");
    shell.shift_click_planet(RED_STORM);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Colonize");
    let home = shell.planet_on_screen(STOVE_TOP);
    shell.click_at(home);
    shell.press("planet", "Change");
    for _ in 0..3 {
        shell.double_click("production", "Santa Maria");
    }
    shell.press("production", "OK");
    shell.press("toolbar", "Nml");
    assert_eq!(shell.page(), 24);

    // Page 24: the next message — mines built — switched off; the next
    // message's Goto is 90210; its queue: shift-double-click Factories
    // (Auto Build) and Mines (Auto Build); OK; the rest of the messages.
    shell.press("messages", "Next");
    shell.press("messages", "filter");
    shell.next_message_until(stars_core::message::Goto::Planet(PLANET_90210));
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(PLANET_90210));
    shell.press("planet", "Change");
    shell.scroll_to("production", "Factories", "Factory");
    shell.modifiers = egui::Modifiers::SHIFT;
    shell.double_click("production", "Factories");
    shell.modifiers = egui::Modifiers::NONE;
    shell.scroll_to("production", "Mines", "Factories");
    shell.modifiers = egui::Modifiers::SHIFT;
    shell.double_click("production", "Mines");
    shell.modifiers = egui::Modifiers::NONE;
    {
        let dialog = shell.app.production.as_ref().expect("open");
        assert_eq!(dialog.queue.len(), 2, "{:?}", dialog.queue);
        assert_eq!((dialog.queue[0].item, dialog.queue[0].count), (1, 10));
        assert_eq!((dialog.queue[1].item, dialog.queue[1].count), (0, 10));
    }
    shell.press("production", "OK");
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    assert_eq!(shell.page(), 25);

    // Page 25: the enemy scout between Slime and No Vacancy is the
    // Berserkers' to fly there, and it has not; the task is two Armed
    // Probes at the foot of Stove Top's queue.
    let home = shell.planet_on_screen(STOVE_TOP);
    shell.click_at(home);
    shell.press("planet", "Change");
    shell.double_click("production", "Armed Probe");
    shell.double_click("production", "Armed Probe");
    shell.press("production", "OK");
    assert_eq!(shell.page(), 25, "2407 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 26);

    // --- 2408 -------------------------------------------------------------
    // Page 26: the first message's Goto is the new colony ships; Split, one
    // Santa Maria across to Fleet #10, OK; the two left filled with
    // colonists and given Colonize at Slime; then Split All.
    shell.next_message_until(stars_core::message::Goto::Fleet(7));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(7), "the three Santa Marias");
    shell.press("fleet", "Split");
    assert!(shell.app.split.is_some(), "the Ship Transfer dialog is up");
    shell.frame();
    assert_eq!(shell.app.split.as_ref().expect("up").new_id, 9, "Fleet #10");
    shell.press("split", "Santa Maria >");
    assert_eq!(shell.app.split.as_ref().expect("up").right, vec![1]);
    shell.press("split", "OK");
    assert!(shell.app.split.is_none());
    assert_eq!(shell.app.own_fleets().len(), 10);
    shell.press("fleet", "Xfer");
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    assert_eq!(
        shell.app.xfer.as_ref().expect("up").aboard[3],
        50,
        "two holds"
    );
    shell.press("xfer", "OK");
    shell.shift_click_planet(SLIME);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Colonize");
    shell.press("fleet", "Split All");
    assert_eq!(shell.app.own_fleets().len(), 11);
    assert_eq!(shell.page(), 27);

    // Page 27: drag the waypoint at Slime to Sea Squared; the next
    // message's Goto is the new Armed Probes; shift-click Hiho.
    // Slime and Sea Squared are further apart than the map shows at this
    // zoom, so the map is taken down a step first — the toolbar's Zoom
    // menu — as a player would before dragging from one to the other.
    let zoom = shell.app.scan_zoom;
    shell.app.scan_zoom = zoom.saturating_sub(1);
    let (slime, sea_squared) = shell.two_planets_on_screen(SLIME, SEA_SQUARED);
    shell.drag(slime, sea_squared);
    shell.app.scan_zoom = zoom;
    shell.frame();
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        assert_eq!(
            fleet.waypoints[1].target,
            Some(SEA_SQUARED as u16),
            "the leg moved"
        );
    }
    shell.next_message_until(stars_core::message::Goto::Fleet(8));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(8), "Armed Probe #9");
    shell.shift_click_planet(HIHO);
    assert_eq!(shell.page(), 28);

    // Page 28: the next message — the Teamster unloading at Stove Top —
    // switched off; the last message's Goto is Wallaby; F5 is the shell's
    // key.
    shell.press("messages", "Next");
    assert_eq!(
        shell.app.current_message().map(|m| m.id),
        Some(stars_core::message::id::HAS_UNLOADED),
        "the Teamster has unloaded what it mined at Prune"
    );
    shell.press("messages", "filter");
    shell.next_message_until(stars_core::message::Goto::Planet(WALLABY));
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(WALLABY));
    shell.press("menu", "Commands");
    shell.press("menu", "Research…");
    assert_eq!(shell.page(), 29);

    // Page 29: research to 30% and Done. The percentage is a slider here
    // where the original has spin buttons; it is set directly.
    shell
        .app
        .research_dialog
        .as_mut()
        .expect("the dialog")
        .percent = 30;
    shell.press("research", "Done");
    assert_eq!(shell.page(), 29, "2408 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 30);

    // --- 2409 -------------------------------------------------------------
    // Page 30: the first message — a level of Weapons finished — has the
    // Research dialog for its Goto; 'Next field to research' to
    // Construction and Done (the dropdown is a plain egui widget and is
    // set the same way as the radio on page 5); then the next message,
    // the robots' haul, switched off.
    shell.next_message_until(stars_core::message::Goto::Research);
    assert_eq!(
        shell.app.current_message().map(|m| m.id),
        Some(stars_core::message::id::TECH_LEVEL_GAINED)
    );
    shell.press("messages", "Goto");
    assert!(shell.app.research_dialog.is_some(), "Goto opens the dialog");
    shell.app.research_dialog.as_mut().expect("the dialog").next =
        stars_core::research::NextField::Field(3);
    shell.press("research", "Done");
    shell.press("messages", "Next");
    assert_eq!(
        shell.app.current_message().map(|m| m.id),
        Some(stars_core::message::id::MINING_ROBOTS_LOADED),
        "the Teamster took what the robots dug at Prune"
    );
    shell.press("messages", "filter");
    assert_eq!(shell.page(), 31);

    // Page 31: the next message's Goto is Oxygen, found by Armed Probe #1;
    // Santa Maria #10 from Stove Top's menu, filled, and sent to settle
    // Oxygen; then the probe's waypoint dragged from Oxygen to Mozart.
    shell.next_message_until(stars_core::message::Goto::Planet(OXYGEN));
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(OXYGEN));
    shell.right_click_planet_and_pick(STOVE_TOP, "Santa Maria #10");
    assert_eq!(shell.selected_fleet_id(), Some(9));
    shell.press("fleet", "Xfer");
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    assert_eq!(shell.app.xfer.as_ref().expect("up").aboard[3], 25);
    shell.press("xfer", "OK");
    shell.shift_click_planet(OXYGEN);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Colonize");
    shell.frame();
    assert_eq!(shell.page(), 31, "the probe is still bound for Oxygen");
    // The probe is still a year short of Oxygen, in open space where
    // there is no planet to right-click: a click on it takes it in hand.
    let probe = shell.fleet_on_screen(0);
    shell.click_at(probe);
    assert_eq!(shell.selected_fleet_id(), Some(0));
    let zoom = shell.app.scan_zoom;
    shell.app.scan_zoom = zoom.saturating_sub(1);
    let (oxygen, mozart) = shell.two_planets_on_screen(OXYGEN, MOZART);
    shell.drag(oxygen, mozart);
    shell.app.scan_zoom = zoom;
    shell.frame();
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        assert_eq!(
            fleet.waypoints[1].target,
            Some(MOZART as u16),
            "the leg moved"
        );
    }
    assert_eq!(shell.page(), 31, "2409 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 32);

    // --- 2410 -------------------------------------------------------------
    // Page 32: the message that the colony ship was dismantled switched
    // off; the next message's Goto is Shaggy Dog; Change; three Factories
    // (Auto Build), three Mines (Auto Build), the leftover box, OK; Change
    // again and the blue diamond's Customize.
    shell.next_message_until_id(stars_core::message::id::FLEET_DISMANTLED);
    shell.press("messages", "filter");
    shell.next_message_until(stars_core::message::Goto::Planet(SHAGGY_DOG));
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(SHAGGY_DOG));
    shell.press("planet", "Change");
    shell.scroll_to("production", "Factories", "Factory");
    for _ in 0..3 {
        shell.double_click("production", "Factories");
    }
    shell.scroll_to("production", "Mines", "Factories");
    for _ in 0..3 {
        shell.double_click("production", "Mines");
    }
    shell.press(
        "production",
        "Contribute only leftover resources to research",
    );
    {
        let dialog = shell.app.production.as_ref().expect("open");
        assert_eq!(dialog.queue.len(), 2, "{:?}", dialog.queue);
        assert_eq!((dialog.queue[0].item, dialog.queue[0].count), (1, 3));
        assert_eq!((dialog.queue[1].item, dialog.queue[1].count), (0, 3));
        assert!(dialog.no_research);
    }
    shell.press("production", "OK");
    assert_eq!(shell.page(), 32, "the template is not set yet");
    shell.press("planet", "Change");
    shell.right_click("production", "blue diamond");
    shell.press("production", "Customize");
    // The page turns on the template itself (`FCheckTemplate`), so it is
    // still 32 with the Customize box open, and Import is what turns it.
    assert_eq!(shell.page(), 32);
    shell.press("customize", "Import");
    assert_eq!(shell.page(), 33);

    // Page 33: OK, and OK on the production dialog; the next message's
    // Goto is Bloop; a Santa Maria and a Teamster into Stove Top's queue.
    shell.press("customize", "OK");
    shell.press("production", "OK");
    shell.next_message_until(stars_core::message::Goto::Planet(BLOOP));
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(BLOOP));
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    assert_eq!(shell.app.selection.planet, Some(STOVE_TOP));
    shell.press("planet", "Change");
    shell.double_click("production", "Santa Maria");
    shell.double_click("production", "Teamster");
    shell.press("production", "OK");
    assert_eq!(shell.page(), 33, "2410 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 34);

    // --- 2411 -------------------------------------------------------------
    // Page 34: the first message's Goto is Armed Probe #1; shift-click on
    // Hacker.
    shell.next_message_until(stars_core::message::Goto::Fleet(0));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(0));
    shell.shift_click_planet(HACKER);
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(266));

    // Long Range Scout #2: shift-click Stove Top and set the task to Scrap
    // Fleet. In this world the scout still has a leg to fly (its route ran
    // a year longer than the original's), and no message points at it: it
    // is taken in hand from the fleet tile and its leg deleted first, as a
    // player would.
    shell.press("fleet", "Next");
    assert_eq!(shell.selected_fleet_id(), Some(1));
    {
        let leg = {
            let game = shell.app.game.as_ref().expect("a game");
            let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
            fleet.waypoints.get(1).map(|w| w.position)
        };
        if let Some(at) = leg {
            let pos = shell.point_on_screen(at, MOHOLDI);
            shell.click_at(pos);
            assert_eq!(shell.app.selection.waypoint, Some(1), "the leg in hand");
            assert!(shell.app.delete_current_waypoint());
            shell.frame();
            // The click on the leg's end took that planet in hand; the
            // scout again, before its new leg is laid.
            let scout = shell.fleet_on_screen(1);
            shell.click_at(scout);
            assert_eq!(shell.selected_fleet_id(), Some(1));
        }
    }
    shell.shift_click_planet(STOVE_TOP);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Scrap Fleet");
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(269));

    // Stalwart Defender #5 to Stove Top.
    shell.next_message_until(stars_core::message::Goto::Fleet(4));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(4));
    shell.shift_click_planet(STOVE_TOP);
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(271));

    // Armed Probe #9 after the enemy scout: shift-click the red triangle.
    shell.next_message_until(stars_core::message::Goto::Fleet(8));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(8));
    let enemy = {
        let game = shell.app.game.as_ref().expect("a game");
        game.fleets
            .iter()
            .find(|f| f.owner == 1 && f.id == 0)
            .map(|f| f.position)
            .expect("the enemy scout")
    };
    let pos = shell.point_on_screen(enemy, HIHO);
    shell.modifiers = egui::Modifiers::SHIFT;
    shell.click_at(pos);
    shell.modifiers = egui::Modifiers::NONE;
    shell.frame();
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        assert_eq!(fleet.waypoints[1].target, Some(0x200), "after the scout");
        assert_eq!(
            fleet.waypoints[1].target_class,
            stars_core::fleet::grobj::FLEET
        );
    }
    assert_eq!(shell.page(), 35);

    // Page 35: the new Santa Maria — its message's Goto — loads colonists
    // and settles Mozart, the best growth figure of the planets known.
    shell.next_message_until(stars_core::message::Goto::Fleet(2));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(2));
    shell.press("fleet", "Xfer");
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    assert_eq!(shell.app.xfer.as_ref().expect("up").aboard[3], 25);
    shell.press("xfer", "OK");
    shell.shift_click_planet(MOZART);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Colonize");
    shell.frame();
    assert_eq!(shell.page(), 35);
    assert_eq!(
        shell.app.tutor.as_ref().map(|t| t.bold),
        Some(275),
        "the Santa Maria is away; Teamster #12 is next"
    );

    // Here this world parts from the original's. Page 35 goes on to
    // Teamster #12 (fleet 11), which the original's Stove Top built in
    // 2410 alongside the Santa Maria; this engine's Stove Top had 33 kT of
    // ironium to the Teamster's 34 and the Santa Maria's 27, so the
    // Teamster is still on the ways — see `docs/ui/tutorial.md`, *Where
    // the run stops*. Everything the pages ask for through here has been
    // done through the panes, with a ring on every step.
    assert!(
        !shell
            .app
            .game
            .as_ref()
            .expect("a game")
            .fleets
            .iter()
            .any(|f| f.owner == 0 && f.id == 11),
        "when Teamster #12 exists, the run can go on: extend the test"
    );
}
