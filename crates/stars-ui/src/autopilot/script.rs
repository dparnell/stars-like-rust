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
const NEIL: i16 = 0x0b;
const LA_TE_DA: i16 = 0x06;
const SPEED_BUMP: i16 = 0x01;
const LEVER: i16 = 0x00;
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
        shell.app.xfer.as_ref().expect("still up").aboard()[3],
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
    assert_eq!(shell.app.xfer.as_ref().expect("up").aboard()[3], 210);
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
        shell.app.xfer.as_ref().expect("up").aboard()[3],
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
    assert_eq!(shell.app.xfer.as_ref().expect("up").aboard()[3], 25);
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
    // The Berserkers are Super Stealth, their scout 75% cloaked bare, so
    // the probe sees it only within a quarter of its range — and where the
    // two stand this year is not the original's to the light year. When
    // the triangle is not on the map the probe is sent to where the scout
    // was seen last instead, which is what a player could do.
    let (enemy, seen) = {
        let game = shell.app.game.as_ref().expect("a game");
        let (index, scout) = game
            .fleets
            .iter()
            .enumerate()
            .find(|(_, f)| f.owner == 1 && f.id == 0)
            .expect("the enemy scout");
        (scout.position, shell.app.in_view.fleets.contains(&index))
    };
    let pos = shell.point_on_screen(enemy, HIHO);
    shell.modifiers = egui::Modifiers::SHIFT;
    shell.click_at(pos);
    shell.modifiers = egui::Modifiers::NONE;
    shell.frame();
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        if seen {
            assert_eq!(fleet.waypoints[1].target, Some(0x200), "after the scout");
            assert_eq!(
                fleet.waypoints[1].target_class,
                stars_core::fleet::grobj::FLEET
            );
        } else {
            assert_eq!(fleet.waypoints[1].position, enemy, "to where it was");
        }
    }
    if seen {
        assert_eq!(shell.page(), 35);
    } else {
        shell.off_the_page = true;
    }
    let scout_seen = seen;

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
    assert_eq!(shell.app.xfer.as_ref().expect("up").aboard()[3], 25);
    shell.press("xfer", "OK");
    shell.shift_click_planet(MOZART);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Colonize");
    shell.frame();
    if scout_seen {
        assert_eq!(shell.page(), 35);
        assert_eq!(
            shell.app.tutor.as_ref().map(|t| t.bold),
            Some(275),
            "the Santa Maria is away; Teamster #12 is next"
        );
    }

    // Here this world parts from the original's. Page 35 goes on to
    // Teamster #12 (fleet 11), which the original's Stove Top built in
    // 2410 alongside the Santa Maria; this engine's Stove Top had 33 kT of
    // ironium to the Teamster's 34 and the Santa Maria's 27, so the
    // Teamster is still on the ways — see `docs/ui/tutorial.md`, *Where
    // the run stops*. A player does what the pages of 2411 can still be
    // done with and generates; the tutor counts the year's pages done.
    let teamster_12 = shell
        .app
        .game
        .as_ref()
        .expect("a game")
        .fleets
        .iter()
        .any(|f| f.owner == 0 && f.id == 11);
    assert!(
        !teamster_12,
        "Teamster #12 exists: the pages of 2411 can be played in full — extend the script"
    );
    // Page 36's work is done all the same, since the world needs it as
    // much as the original's did: seventy mines at the top of Stove Top's
    // queue, and the next field of research set to Biotechnology. The
    // tutor's page does not move — it is still on the Teamster — so the
    // presses go unchecked, as a player's would.
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    assert_eq!(shell.app.selection.planet, Some(STOVE_TOP));
    shell.press("planet", "Change");
    shell.scroll_to("production", "Mine", "Factory");
    shell.press("production", "Mine");
    shell.press("production", "Top of the Queue");
    for _ in 0..7 {
        shell.press_with(egui::Modifiers::SHIFT, "production", "Add ->");
    }
    {
        let dialog = shell.app.production.as_ref().expect("open");
        assert_eq!(
            (dialog.queue[0].item, dialog.queue[0].count),
            (stars_core::production::item::MINE, 70),
            "{:?}",
            dialog.queue
        );
    }
    shell.press("production", "OK");
    shell.press("menu", "Commands");
    shell.press("menu", "Research…");
    shell.app.research_dialog.as_mut().expect("the dialog").next =
        stars_core::research::NextField::Field(5);
    shell.press("research", "Done");
    shell.generate_anyway("Teamster #12 was not built; on to 2412");
    assert_eq!(shell.page(), 38, "2412: the year's pages are counted done");

    // --- 2412 -------------------------------------------------------------
    // Page 38: the first message's Goto is dead (Armed Probe #1 finished
    // its orders at Hacker and died there); the next message's Goto is
    // Armed Probe #9, which caught the scout at Hiho. Then Teamster #4 —
    // the page says View/Find; the planet's own menu finds it as well.
    shell.next_message_until(stars_core::message::Goto::Fleet(8));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(8));
    shell.right_click_planet_and_pick(STOVE_TOP, "Teamster #4");
    assert_eq!(shell.selected_fleet_id(), Some(3));
    shell.frame();
    assert_eq!(shell.page(), 39, "Teamster #4 in hand turns the page");

    // Page 39: the battle messages (a look at each is a hint), then Goto
    // Red Storm.
    shell.next_message_until(stars_core::message::Goto::Planet(RED_STORM));
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(RED_STORM));
    shell.frame();
    assert_eq!(shell.page(), 40);

    // Page 40: Goto Slime, two Terraform Environments at the head of its
    // queue; two Teamsters into Stove Top's.
    shell.next_message_until(stars_core::message::Goto::Planet(SLIME));
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(SLIME));
    shell.press("planet", "Change");
    shell.scroll_to("production", "Terraform Environment", "Factory");
    shell.double_click("production", "Terraform Environment");
    shell.double_click("production", "Terraform Environment");
    // Leftover-only research is already ticked: Slime was settled with
    // the default queue page 33 set up.
    shell.press("production", "OK");
    shell.frame();
    assert_eq!(shell.page(), 40);
    assert_eq!(
        shell.app.tutor.as_ref().map(|t| t.bold),
        Some(319),
        "Slime's terraforming is queued; the Teamsters are next"
    );
    {
        let game = shell.app.game.as_ref().expect("a game");
        let slime = game.planets.iter().find(|p| p.id == SLIME).expect("Slime");
        assert!(slime.no_research, "leftover-only research ticked");
        assert_eq!(
            (slime.queue[0].item, slime.queue[0].count),
            (stars_core::production::item::TERRAFORM, 2)
        );
    }
    // Two Teamsters into Stove Top's queue. The page asks for them at the
    // second slot, after the seventy mines of page 36; in this world the
    // Teamster of 2410 is still building at the head of the queue and the
    // two join it there, so the tutor does not see the rung done — the
    // year is generated regardless, as the page's last line says.
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    assert_eq!(shell.app.selection.planet, Some(STOVE_TOP));
    shell.press("planet", "Change");
    shell.double_click("production", "Teamster");
    shell.double_click("production", "Teamster");
    shell.press("production", "OK");
    shell.frame();
    {
        let game = shell.app.game.as_ref().expect("a game");
        let home = game
            .planets
            .iter()
            .find(|p| p.id == STOVE_TOP)
            .expect("home");
        let teamsters: i32 = home
            .queue
            .iter()
            .filter(|q| q.ship && q.item == 3)
            .map(|q| q.count)
            .sum();
        assert!(teamsters >= 2, "{:?}", home.queue);
    }
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("the Teamsters joined the one still building; on to 2413");
    }
    assert_eq!(shell.page(), 41, "2413: the Teamster page");

    // --- 2413 -------------------------------------------------------------
    // Page 41: the first message is the new Teamster — Teamster #1, in
    // the number Armed Probe #1 left free — its Goto; Xfer, the hold
    // filled with colonists; a waypoint at Slime with Transport and
    // Colonists set to Unload All from the tile's two dropdowns.
    shell.next_message_until(stars_core::message::Goto::Fleet(0));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(0), "Teamster #1");
    shell.press("fleet", "Xfer");
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    assert_eq!(shell.app.xfer.as_ref().expect("up").aboard()[3], 210);
    shell.press("xfer", "OK");
    shell.shift_click_planet(SLIME);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Transport");
    shell.press("fleet", "Cargo");
    shell.press("fleet", "Colonists");
    shell.press("fleet", "Action");
    shell.press("fleet", "Unload All");
    shell.frame();
    assert_eq!(
        shell.app.tutor.as_ref().map(|t| t.bold),
        Some(322),
        "the Teamster is bound for Slime; the next message is next"
    );
    // The next message is the level of Construction: its Goto is the
    // Research dialog; the next field to Propulsion.
    shell.next_message_until(stars_core::message::Goto::Research);
    shell.press("messages", "Goto");
    shell.app.research_dialog.as_mut().expect("the dialog").next =
        stars_core::research::NextField::Field(2);
    shell.press("research", "Done");
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(325));
    // The next message is what the level brought — the Robo-Miner — and
    // its Goto is the Technology Browser, opened on it.
    shell.press("messages", "Next");
    assert_eq!(
        shell.app.current_message().map(|m| m.id),
        Some(stars_core::message::id::BREAKTHROUGH_PART),
        "the Robo-Miner is the next message"
    );
    shell.press("messages", "Goto");
    assert!(
        shell.app.browser.is_some(),
        "the browser is open on the part"
    );
    shell.press("browser", "Close");
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(327));
    // F4 is the shell's key; the menu's Ship Design is the same command.
    shell.press("menu", "Commands");
    shell.press("menu", "Ship Design…");
    shell.frame();
    assert_eq!(shell.page(), 42, "the designer open turns the page");

    // Page 42: Available Hull Types, Mini-Miner from the dropdown, Copy
    // Selected Design; then the parts dragged onto the slots — a Long
    // Hump 6 on the engine, a Rhino Scanner on the scanner slot, and a
    // Robo-Miner on each of the two mining slots under the Mining Robots
    // filter.
    shell.press("designer", "Available Hull Types");
    shell.press("designer", "Designs");
    shell.press("designer", "Mini-Miner");
    shell.press("designer", "Copy Selected Design");
    shell.frame();
    assert!(
        shell
            .app
            .designer
            .as_ref()
            .is_some_and(|d| d.editing.is_some()),
        "the editor is open on the copy"
    );
    let fit = |shell: &mut Shell, part: &str, slot: &str| {
        shell.frame();
        let from = shell
            .app
            .drawn_button("designer", part)
            .unwrap_or_else(|| panic!("{part} in the parts list"))
            .rect
            .center();
        let to = shell
            .app
            .drawn_button("designer", slot)
            .unwrap_or_else(|| {
                panic!(
                    "{slot} on the schematic; drawn: {:?}",
                    shell
                        .app
                        .drawn
                        .iter()
                        .filter(|w| w.scope == "designer")
                        .map(|w| w.label.clone())
                        .collect::<Vec<_>>()
                )
            })
            .rect
            .center();
        shell.step(&format!("designer: {part} onto {slot}"));
        shell.drag(from, to);
    };
    fit(shell, "Long Hump 6", "slot 0");
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(333));
    fit(shell, "Rhino Scanner", "slot 1");
    shell.frame();
    // The page's arm asks after the engine and the scanner only, so the
    // page turns here, with the robots still to fit — as the original's
    // does.
    assert_eq!(shell.page(), 43, "engine and scanner fitted: page 43");
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(337));
    shell.press("designer", "Parts");
    shell.press("designer", "Mining Robots");
    fit(shell, "Robo-Miner", "slot 2");
    fit(shell, "Robo-Miner", "slot 3");
    {
        let editing = shell
            .app
            .designer
            .as_ref()
            .and_then(|d| d.editing.as_ref())
            .expect("the editor");
        let fitted: Vec<Option<(u16, usize)>> =
            (0..4)
                .map(|i| {
                    editing.design.slots.get(i).copied().and_then(|s| {
                        stars_core::design::slot_part(&s).map(|p| (p.category, p.item))
                    })
                })
                .collect();
        assert_eq!(
            fitted,
            vec![
                Some((0x1, 3)),
                Some((0x2, 1)),
                Some((0x80, 2)),
                Some((0x80, 2))
            ],
            "the Mini-Miner fitted out"
        );
    }

    // Page 43: OK finishes the design, Done closes the designer; the new
    // Mini-Miner into Stove Top's queue, then a hundred mines at its top
    // with Ctrl held.
    shell.press("designer", "OK");
    shell.frame();
    assert!(
        shell
            .app
            .designer
            .as_ref()
            .is_some_and(|d| d.editing.is_none()),
        "back in the browser"
    );
    shell.press("designer", "Done");
    shell.frame();
    assert_eq!(shell.page(), 43);
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(339));
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    shell.press("planet", "Change");
    shell.double_click("production", "Mini-Miner");
    shell.press("production", "OK");
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(342));
    shell.press("planet", "Change");
    shell.scroll_to("production", "Mine", "Factory");
    shell.press("production", "Mine");
    shell.press("production", "Top of the Queue");
    shell.press_with(egui::Modifiers::COMMAND, "production", "Add ->");
    {
        let dialog = shell.app.production.as_ref().expect("open");
        assert_eq!(
            (dialog.queue[0].item, dialog.queue[0].count),
            (stars_core::production::item::MINE, 100),
            "{:?}",
            dialog.queue
        );
    }
    // The page wants the Mini-Miner right behind the mines. The original's
    // Stove Top had nothing else queued; this one still has a Teamster of
    // page 40's on the ways, so the Mini-Miner is moved up past it with
    // Item Up — which a player following the page here would do too.
    shell.press("production", "1  Mini-Miner");
    shell.press("production", "Item Up");
    {
        let dialog = shell.app.production.as_ref().expect("open");
        assert_eq!(
            (dialog.queue[1].ship, dialog.queue[1].item),
            (true, 6),
            "{:?}",
            dialog.queue
        );
    }
    shell.press("production", "OK");
    shell.frame();
    assert_eq!(shell.page(), 44, "the mines turn the page");

    // Page 44: the rest of the messages — the Privateer hull among them —
    // then Armed Probe #9 home to Stove Top.
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(348));
    // The probe caught the scout over Hiho and orbits it now, so it is
    // taken from the planet's menu — unless the cloaked scout slipped it,
    // in which case it is wherever the chase left it and is picked by
    // number.
    let probe_at_hiho = shell.app.game.as_ref().is_some_and(|game| {
        game.fleets
            .iter()
            .any(|f| f.owner == 0 && f.id == 8 && f.orbiting == Some(HIHO as u16))
    });
    if probe_at_hiho {
        shell.right_click_planet_and_pick(HIHO, "Armed Probe #9");
    } else {
        assert!(shell.app.goto_fleet(8), "Armed Probe #9 is ours");
        shell.frame();
    }
    assert_eq!(shell.selected_fleet_id(), Some(8), "Armed Probe #9");
    shell.shift_click_planet(STOVE_TOP);
    shell.frame();
    assert_eq!(shell.page(), 44, "2413 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 45);

    // --- 2414 -------------------------------------------------------------
    // Page 45: the first message's Goto is Sea Squared, the last's Oxygen;
    // Min Terraform twice into Oxygen's queue, and the default template
    // imported from it through the blue diamond's Customize.
    shell.next_message_until(stars_core::message::Goto::Planet(SEA_SQUARED));
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(SEA_SQUARED));
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(354));
    shell.next_message_until(stars_core::message::Goto::Planet(OXYGEN));
    shell.press("messages", "Goto");
    assert_eq!(shell.app.selection.planet, Some(OXYGEN));
    shell.press("planet", "Change");
    shell.scroll_to("production", "Min Terraform", "Factory");
    shell.double_click("production", "Min Terraform");
    shell.double_click("production", "Min Terraform");
    {
        let dialog = shell.app.production.as_ref().expect("open");
        assert_eq!(
            (dialog.queue[0].item, dialog.queue[0].count),
            (stars_core::production::item::AUTO_MIN_TERRAFORM, 2),
            "{:?}",
            dialog.queue
        );
        assert!(
            dialog.no_research,
            "the settled planet's leftover-only research"
        );
    }
    shell.right_click("production", "blue diamond");
    shell.press("production", "Customize");
    // Import into the default, which is the slot the box opens on: the
    // page's check reads the player's default queue.
    shell.press("customize", "Import");
    shell.press("customize", "OK");
    // The page's queue checks read the planet's own queue, which the
    // dialog's OK writes; the year is done with it.
    shell.press("production", "OK");
    assert_eq!(shell.page(), 45, "2414 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 46);

    // --- 2415 -------------------------------------------------------------
    // Page 46: Armed Probe #9 to Neil. Here the probe is still on its way
    // home from Hiho, so it is taken from the map, its Stove Top leg
    // deleted and Neil shift-clicked in its place.
    let probe = shell.fleet_on_screen(8);
    shell.click_at(probe);
    assert_eq!(shell.selected_fleet_id(), Some(8), "Armed Probe #9");
    let home = shell.planet_on_screen(STOVE_TOP);
    shell.click_at(home);
    assert_eq!(shell.app.selection.waypoint, Some(1), "its leg in hand");
    assert!(shell.app.delete_current_waypoint());
    shell.frame();
    let probe = shell.fleet_on_screen(8);
    shell.click_at(probe);
    assert_eq!(shell.selected_fleet_id(), Some(8));
    shell.shift_click_planet(NEIL);
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(361));

    // "Goto the new Teamster": none was built this year — Stove Top's
    // ironium went into the Mini-Miner — so the page's freighter rungs
    // cannot be reached; the Planet Summary Report is looked at as the
    // page says, sorted by Value, and closed (Esc is the shell's key).
    let new_teamster = shell
        .app
        .game
        .as_ref()
        .expect("a game")
        .fleets
        .iter()
        .any(|f| f.owner == 0 && f.id == 6);
    assert!(
        !new_teamster,
        "Teamster #7 exists: page 46 can be played in full — extend the script"
    );
    shell.press("menu", "Report");
    shell.press("menu", "Planets…");
    shell.frame();
    assert!(shell.app.open_report_kind().is_some(), "the report is up");
    shell.press("report", "Value");
    shell.press("report", "Sort by Value");
    shell.frame();
    assert_eq!(
        shell.app.reports.state(crate::report::Report::Planets).sort,
        4,
        "sorted on Value"
    );
    shell.app.close_report();
    shell.frame();

    // Page 47's Research dialog, opened and shut as the page says; the
    // remaining messages; Teamster #12 was never built.
    shell.press("menu", "Commands");
    shell.press("menu", "Research…");
    shell.press("research", "Done");
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.generate_anyway("no new Teamster this year; on to 2416");
    assert_eq!(shell.page(), 48, "2416: the year's pages are counted done");

    // --- 2416 -------------------------------------------------------------
    // Page 48: Stalwart Defender #5, home with its orders done, to
    // Wallaby; the new Mini-Miner to Prune to merge with the Cotton
    // Picker; Stove Top's auto factories raised to sixty with sixty auto
    // mines behind them; Construction next.
    shell.next_message_until(stars_core::message::Goto::Fleet(4));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(4), "Stalwart Defender #5");
    shell.shift_click_planet(WALLABY);
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(377));
    // The Mini-Miner is fleet 1 here — Mini-Miner #2, in the number Long
    // Range Scout #2 left — where the original's was fleet 7, so the
    // page's merge rung is not seen done; the order is given all the
    // same. Stove Top may have finished it a year before the original's
    // did (the freighter's loads at Prune come out a little ahead), in
    // which case there is no message this year to Goto from and it is
    // picked out by hand, as the page's *Goto* would have.
    let (miner, built_this_year) = {
        let game = shell.app.game.as_ref().expect("a game");
        let miner = game
            .fleets
            .iter()
            .find(|f| f.owner == 0 && f.stacks.iter().any(|s| s.design == 6))
            .map(|f| f.id)
            .expect("the Mini-Miner");
        let built_this_year = game
            .messages
            .iter()
            .any(|m| m.player == 0 && m.goto(&[1]) == stars_core::message::Goto::Fleet(miner));
        (miner, built_this_year)
    };
    if built_this_year {
        shell.next_message_until(stars_core::message::Goto::Fleet(miner));
        shell.press("messages", "Goto");
    } else {
        assert!(shell.app.goto_fleet(miner), "the Mini-Miner is ours");
        shell.frame();
    }
    assert_eq!(shell.selected_fleet_id(), Some(miner), "the Mini-Miner");
    shell.shift_click_planet(PRUNE);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Merge with Fleet");
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        assert_eq!(fleet.waypoints[1].task, stars_formats::task::MERGE);
        assert_eq!(
            fleet.waypoints[1].target_class,
            stars_core::fleet::grobj::FLEET,
            "the waypoint reads Cotton Picker #6"
        );
        assert_eq!(fleet.waypoints[1].target, Some(5));
    }
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    shell.press("planet", "Change");
    shell.scroll_to("production", "Factories", "Factory");
    shell.press("production", "Factories");
    shell.press("production", "Factories up to 30");
    for _ in 0..3 {
        shell.press_with(egui::Modifiers::SHIFT, "production", "Add ->");
    }
    shell.scroll_to("production", "Mines", "Factories");
    shell.press("production", "Mines");
    shell.press("production", "Factories up to 60");
    for _ in 0..6 {
        shell.press_with(egui::Modifiers::SHIFT, "production", "Add ->");
    }
    {
        let dialog = shell.app.production.as_ref().expect("open");
        let tail: Vec<(u16, i32)> = dialog
            .queue
            .iter()
            .filter(|q| q.is_auto())
            .map(|q| (q.item, q.count))
            .collect();
        assert_eq!(
            tail,
            vec![
                (stars_core::production::item::AUTO_FACTORY, 60),
                (stars_core::production::item::AUTO_MINE, 60)
            ],
            "{:?}",
            dialog.queue
        );
    }
    shell.press("production", "OK");
    shell.next_message_until(stars_core::message::Goto::Research);
    shell.press("messages", "Goto");
    shell.app.research_dialog.as_mut().expect("the dialog").next =
        stars_core::research::NextField::Field(3);
    shell.press("research", "Done");
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("the Teamster still building heads Stove Top's queue; on to 2417");
    }
    assert_eq!(shell.page(), 49, "2417");

    // --- 2417 -------------------------------------------------------------
    // Page 49: every message, and Teamster #1 — its colonists put down at
    // Slime, in a message the filter of page 28 hides — home to Stove Top,
    // taken from Slime's menu.
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.right_click_planet_and_pick(SLIME, "Teamster #1");
    assert_eq!(shell.selected_fleet_id(), Some(0), "Teamster #1");
    shell.shift_click_planet(STOVE_TOP);
    shell.frame();
    assert_eq!(shell.page(), 49, "2417 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 50);

    // --- 2418 -------------------------------------------------------------
    // Page 50: a Teamster into Stove Top's queue; Goto 90210; the
    // Research dialog opened and shut; the rest of the messages.
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    shell.press("planet", "Change");
    shell.press("production", "Top of the Queue");
    shell.double_click("production", "Teamster");
    {
        let dialog = shell.app.production.as_ref().expect("open");
        assert_eq!((dialog.queue[0].ship, dialog.queue[0].item), (true, 3));
    }
    shell.press("production", "OK");
    shell.frame();
    // The page's other rungs are hints and a dialog-shut check, so the
    // Teamster alone sees the page done; the rest is read all the same.
    assert!(
        shell.app.tutor_waiting(),
        "page 50 is done with the Teamster"
    );
    // No message of 2418 points at 90210 here, so it is taken from the map.
    let planet = shell.planet_on_screen(PLANET_90210);
    shell.click_at(planet);
    assert_eq!(shell.app.selection.planet, Some(PLANET_90210));
    // The dialog open takes the page's last rung back until it is shut,
    // so the opening press goes unchecked.
    shell.press("menu", "Commands");
    shell.press_unchecked("menu", "Research…");
    shell.press("research", "Done");
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.frame();
    assert_eq!(shell.page(), 50, "2418 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 51);

    // --- 2419 -------------------------------------------------------------
    // Pages 51 and 52: two freighters set shuttling colonists — Teamster
    // #12 to Wallaby and the new Teamster to Oxygen, each loading at home
    // with Repeat Orders on. Teamster #12 was never built here; the new
    // Teamster of the year is fleet 1 when Stove Top has built it, and
    // failing that Teamster #7, idle at home since 2416, is given the
    // Oxygen run.
    let new_teamster = {
        let game = shell.app.game.as_ref().expect("a game");
        game.messages
            .iter()
            .filter(|m| m.player == 0 && m.id == stars_core::message::id::SHIP_BUILT)
            .find_map(|m| match m.goto(&[1]) {
                stars_core::message::Goto::Fleet(1) => Some(1),
                _ => None,
            })
    };
    match new_teamster {
        Some(id) => {
            shell.next_message_until(stars_core::message::Goto::Fleet(id));
            shell.press("messages", "Goto");
        }
        None => shell.right_click_planet_and_pick(STOVE_TOP, "Teamster #7"),
    }
    let shuttle = shell.selected_fleet_id().expect("a Teamster in hand");
    shell.press("fleet", "Xfer");
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    assert_eq!(shell.app.xfer.as_ref().expect("up").aboard()[3], 210);
    shell.press("xfer", "OK");
    shell.shift_click_planet(OXYGEN);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Transport");
    shell.press("fleet", "Cargo");
    shell.press("fleet", "Colonists");
    shell.press("fleet", "Action");
    shell.press("fleet", "Unload All");
    // "Load All Available" on the colonists: the new leg keeps the last
    // one's Transport with Colonists showing, so the action alone changes.
    shell.shift_click_planet(STOVE_TOP);
    shell.press("fleet", "Action");
    shell.press("fleet", "Load All Available");
    shell.press("fleet", "Repeat Orders");
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = game
            .fleets
            .iter()
            .find(|f| f.owner == 0 && f.id == shuttle)
            .expect("the shuttle");
        assert!(fleet.repeat_orders, "Repeat Orders ticked");
        assert_eq!(fleet.waypoints.len(), 3, "{:?}", fleet.waypoints);
        assert_eq!(fleet.waypoints[1].target, Some(OXYGEN as u16));
        assert_eq!(fleet.waypoints[2].target, Some(STOVE_TOP as u16));
        let actions: Vec<stars_formats::XferAction> = fleet.waypoints[2]
            .transport
            .as_ref()
            .map(|t| t.items.iter().map(|i| i.action).collect())
            .unwrap_or_default();
        assert_eq!(
            actions,
            vec![
                stars_formats::XferAction::None,
                stars_formats::XferAction::None,
                stars_formats::XferAction::None,
                stars_formats::XferAction::LoadAll,
                stars_formats::XferAction::None
            ],
            "the colonists alone are loaded"
        );
    }
    // Max Terraform at the foot of 90210's queue.
    let planet = shell.planet_on_screen(PLANET_90210);
    shell.click_at(planet);
    assert_eq!(shell.app.selection.planet, Some(PLANET_90210));
    shell.press("planet", "Change");
    {
        let last = shell
            .app
            .production_queue_rows()
            .last()
            .map(|(count, name)| format!("{name} up to {count}"))
            .expect("a queue");
        shell.press("production", &last);
    }
    shell.scroll_to("production", "Max Terraform", "Factory");
    shell.double_click("production", "Max Terraform");
    {
        let dialog = shell.app.production.as_ref().expect("open");
        let last = dialog.queue.last().expect("the terraforming");
        assert_eq!(
            (last.item, last.count),
            (stars_core::production::item::AUTO_MAX_TERRAFORM, 1),
            "{:?}",
            dialog.queue
        );
    }
    shell.press("production", "OK");
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("Teamster #12 was never built; on to 2420");
    }
    assert_eq!(shell.page(), 53, "2420");

    // --- 2420 -------------------------------------------------------------
    // Page 53: Armed Probe #9, its orders done at Neil, on to La Te Da;
    // Weapons next; a starbase design — the station copied, a Stargate
    // 100/250 on its left Orbital slot, named Gater with the next picture
    // — and one queued at Stove Top.
    shell.goto_fleet_by_message_or_number(8);
    assert_eq!(shell.selected_fleet_id(), Some(8), "Armed Probe #9");
    shell.shift_click_planet(LA_TE_DA);
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(417));
    shell.next_message_until(stars_core::message::Goto::Research);
    shell.press("messages", "Goto");
    shell.app.research_dialog.as_mut().expect("the dialog").next =
        stars_core::research::NextField::Field(1);
    shell.press("research", "Done");
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(419));
    // The Stargate 100/250 wants Construction 5, which the original's
    // player had by now and this one — Propulsion having been researched
    // since 2413 — has not: the designer is opened as the page says, and
    // the design left for a year the part is there.
    let stargate = {
        let game = shell.app.game.as_ref().expect("a game");
        let builder = stars_core::parts::Builder::player(&game.players[0]).designing_starbase(true);
        stars_core::parts::availability(&builder, stars_core::components::slot::SPECIAL_SB, 1)
            .is_available()
    };
    shell.press("menu", "Commands");
    shell.press("menu", "Ship Design…");
    shell.press("designer", "Starbases");
    if stargate {
        shell.press("designer", "Copy Selected Design");
        shell.frame();
        assert!(
            shell
                .app
                .designer
                .as_ref()
                .is_some_and(|d| d.editing.is_some()),
            "the editor is open on the copy"
        );
        shell.press("designer", "Parts");
        shell.press("designer", "Orbital");
        fit(shell, "Stargate 100/250", "slot 0");
        shell.app.designer_rename("Gater");
        shell.press("designer", "picture right");
        {
            let editing = shell
                .app
                .designer
                .as_ref()
                .and_then(|d| d.editing.as_ref())
                .expect("the editor");
            assert_eq!(editing.design.name, "Gater");
            assert_eq!(
                editing
                    .design
                    .slots
                    .first()
                    .and_then(stars_core::design::slot_part)
                    .map(|p| p.name),
                Some("Stargate 100/250")
            );
        }
        shell.press("designer", "OK");
        shell.press("designer", "Done");
        shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
        shell.press("planet", "Change");
        shell.scroll_to("production", "Gater", "Factory");
        shell.double_click("production", "Gater");
        {
            let dialog = shell.app.production.as_ref().expect("open");
            assert!(
                dialog.queue.iter().any(|q| q.ship && q.item == 0x11),
                "{:?}",
                dialog.queue
            );
        }
        shell.press("production", "OK");
    } else {
        // Closing takes the page's designer rung back; unchecked.
        shell.press_unchecked("designer", "Done");
    }
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("no Stargate to fit yet; on to 2421");
    }
    assert_eq!(shell.page(), 54, "2421");

    // --- 2421 -------------------------------------------------------------
    // Page 54: Teamster #1, home with its orders done, filled with
    // colonists and set to unload them at Wallaby; that order saved as
    // the zip order DropCol through the diamond's Customize; then Stove
    // Top with Load All Available and Repeat Orders.
    shell.next_message_until(stars_core::message::Goto::Fleet(0));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(0), "Teamster #1");
    shell.press("fleet", "Xfer");
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    assert_eq!(shell.app.xfer.as_ref().expect("up").aboard()[3], 210);
    shell.press("xfer", "OK");
    shell.shift_click_planet(WALLABY);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Transport");
    shell.press("fleet", "Cargo");
    shell.press("fleet", "Colonists");
    shell.press("fleet", "Action");
    shell.press("fleet", "Unload All");
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(428));
    shell.right_click("fleet", "blue diamond");
    shell.press("fleet", "<Customize>");
    shell.press("zip", "Import");
    shell.app.zip_orders[0].name = "DropCol".to_string();
    shell.press("zip", "OK");
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(430));
    shell.shift_click_planet(STOVE_TOP);
    shell.press("fleet", "Action");
    shell.press("fleet", "Load All Available");
    shell.press("fleet", "Repeat Orders");
    shell.frame();
    assert_eq!(shell.page(), 55, "the shuttle set turns the page");

    // Page 55: the rest of the messages; the Planet Summary Report, its
    // Min Conc column reverse-sorted on the weighted average.
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.press("menu", "Report");
    shell.press("menu", "Planets…");
    // Min Conc lies off the right of the window: scrolled to, as the
    // report's scrollbar would.
    shell
        .app
        .reports
        .state_mut(crate::report::Report::Planets)
        .first_field = 8;
    shell.frame();
    shell.press("report", "Min Conc");
    shell.press("report", "Reverse Sort by Min Conc");
    shell.press("report", "Weighted Average");
    shell.frame();
    {
        let state = shell.app.reports.state(crate::report::Report::Planets);
        assert_eq!(
            (state.sort, state.ascending, state.subsort),
            (0x0b, false, 3)
        );
    }
    // The sort check reads the open report, so the year is generated
    // with the report still up — it is modeless — and closed after.
    assert_eq!(shell.page(), 55, "2421 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 56, "2422");
    shell.app.close_report();
    shell.frame();

    // --- 2422 -------------------------------------------------------------
    // Page 56: Teamster #4, which ran dry a light year short of home with
    // the bigger loads the Mini-Miner digs — as the page says it does,
    // though the original's had not yet left Prune, where the Cotton
    // Picker's fuel was to be had. Out here there is no fleet to take
    // fuel from, so the page's gauge is passed over.
    shell.next_message_until(stars_core::message::Goto::Fleet(3));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(3), "Teamster #4");
    let alone = {
        let game = shell.app.game.as_ref().expect("a game");
        let at = game
            .fleets
            .iter()
            .find(|f| f.owner == 0 && f.id == 3)
            .map(|f| f.position);
        game.fleets
            .iter()
            .filter(|f| f.owner == 0 && f.id != 3)
            .all(|f| Some(f.position) != at)
    };
    assert!(
        alone,
        "Teamster #4 has company: page 56's fuel can be taken — extend the script"
    );
    // The tutor stays on the fuel for the rest of the year, its bold
    // following what is in hand; the pages after it are played anyway.
    shell.off_the_page = true;

    // Page 57: a Teamster queued; Propulsion next; the messages; the
    // Frigate mine layer, when the Frigate hull is there to copy — it
    // wants Construction 6, and the Mine Dispenser 50 Energy 2 and
    // Biotechnology 4, which this player has not yet reached.
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    shell.press("planet", "Change");
    shell.double_click("production", "Teamster");
    shell.press("production", "OK");
    shell.next_message_until(stars_core::message::Goto::Research);
    shell.press("messages", "Goto");
    shell.app.research_dialog.as_mut().expect("the dialog").next =
        stars_core::research::NextField::Field(2);
    shell.press("research", "Done");
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    let frigate = {
        let game = shell.app.game.as_ref().expect("a game");
        let builder = stars_core::parts::Builder::player(&game.players[0]);
        stars_core::parts::availability(&builder, stars_core::components::slot::HULL, 8)
            .is_available()
            && stars_core::parts::availability(&builder, stars_core::components::slot::MINES, 1)
                .is_available()
    };
    assert!(
        !frigate,
        "the Frigate and the Mine Dispenser 50 are there: page 57's design can be drawn — extend the script"
    );

    // Page 58: the Berserker fleet by Wallaby, and Stalwart Defender #5
    // sent after it — where there is one to send it after.
    let quarry = {
        let game = shell.app.game.as_ref().expect("a game");
        let wallaby = game
            .planets
            .iter()
            .find(|p| p.id == WALLABY)
            .and_then(|p| p.position)
            .expect("Wallaby");
        game.fleets
            .iter()
            .enumerate()
            .filter(|(i, f)| f.owner != 0 && shell.app.fleet_in_view(*i))
            .map(|(_, f)| {
                #[allow(clippy::cast_possible_truncation)]
                let d = stars_core::movement::distance(f.position, wallaby) as i64;
                (f, d)
            })
            .filter(|(_, d)| *d <= 60)
            .min_by_key(|(_, d)| *d)
            .map(|(f, _)| (f.owner, f.id, f.position))
    };
    match quarry {
        Some((owner, id, at)) => {
            let pos = shell.point_on_screen(at, WALLABY);
            shell.click_at(pos);
            shell.right_click_planet_and_pick(WALLABY, "Stalwart Defender #5");
            assert_eq!(shell.selected_fleet_id(), Some(4));
            let pos = shell.point_on_screen(at, WALLABY);
            shell.modifiers = egui::Modifiers::SHIFT;
            shell.click_at(pos);
            shell.modifiers = egui::Modifiers::NONE;
            shell.frame();
            let game = shell.app.game.as_ref().expect("a game");
            let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
            assert_eq!(
                fleet.waypoints[1].target,
                Some((u16::try_from(owner).unwrap_or(0) << 9) | id),
                "after the Berserker fleet"
            );
        }
        None => {
            shell.right_click_planet_and_pick(WALLABY, "Stalwart Defender #5");
            assert_eq!(shell.selected_fleet_id(), Some(4));
        }
    }
    shell.off_the_page = false;
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("no Frigate to design yet; on to 2423");
    }
    assert_eq!(shell.page(), 59, "2423");

    // --- 2423 -------------------------------------------------------------
    // Page 59: the first message; Teamster #7, idle at home since 2416;
    // the report sorted by Population.
    shell.press("messages", "Next");
    shell.right_click_planet_and_pick(STOVE_TOP, "Teamster #7");
    assert_eq!(shell.selected_fleet_id(), Some(6), "Teamster #7");
    shell.press("menu", "Report");
    shell.press("menu", "Planets…");
    // The report keeps the scroll 2421 left it with; back to the left.
    shell
        .app
        .reports
        .state_mut(crate::report::Report::Planets)
        .first_field = 0;
    shell.frame();
    shell.press("report", "Population");
    shell.press("report", "Sort by Population");
    shell.frame();
    assert_eq!(
        shell.app.reports.state(crate::report::Report::Planets).sort,
        2,
        "sorted on Population"
    );
    shell.app.close_report();
    shell.frame();

    // Page 60: Teamster #7 filled and sent to Sea Squared with DropCol;
    // then Teamster #4, home from Prune, merged with Teamster #3 through
    // the Merge Fleets dialog. The page's "new Teamster" is Teamster #3
    // itself in the original; here Teamster #3 has waited at home since
    // 2416, and the merge is the same.
    shell.off_the_page = true;
    shell.right_click_planet_and_pick(STOVE_TOP, "Teamster #7");
    shell.press("fleet", "Xfer");
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    // Filled to the hold: a Teamster's 210, twice that when Stove Top
    // built the pair together.
    let hold = {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        fleet
            .stacks
            .iter()
            .map(|s| {
                game.designs[0]
                    .get(usize::from(s.design))
                    .and_then(stars_core::design::ShipDesign::cargo_capacity)
                    .unwrap_or(0)
                    * s.count
            })
            .sum::<i32>()
    };
    assert_eq!(hold % 210, 0, "Teamsters");
    assert_eq!(shell.app.xfer.as_ref().expect("up").aboard()[3], hold);
    shell.press("xfer", "OK");
    shell.frame();
    assert_eq!(shell.page(), 60, "the loaded freighter turns the page");
    shell.shift_click_planet(SEA_SQUARED);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Transport");
    shell.right_click("fleet", "blue diamond");
    shell.press("fleet", "DropCol");
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        assert_eq!(
            fleet.waypoints[1]
                .transport
                .as_ref()
                .map(|t| t.items[3].action),
            Some(stars_formats::XferAction::UnloadAll),
            "DropCol unloads the colonists"
        );
    }
    // Which Teamster waits at home to be merged into #4 depends on how
    // Stove Top's queue fell: Teamster #3 when the freighters came one a
    // year, or none at all when the year's pair came out as one fleet and
    // has just been sent off. The merge pane lists whoever is here.
    shell.right_click_planet_and_pick(STOVE_TOP, "Teamster #4");
    assert_eq!(shell.selected_fleet_id(), Some(3), "Teamster #4");
    let sent = shell.app.game.as_ref().and_then(|game| {
        game.fleets
            .iter()
            .find(|f| f.owner == 0 && f.waypoints.len() > 1 && f.cargo.colonists > 0)
            .map(|f| f.id)
    });
    shell.press("fleet", "Merge");
    let rows: Vec<String> = shell
        .app
        .drawn
        .iter()
        .filter(|w| w.scope == "merge" && w.label.starts_with("Teamster #"))
        .map(|w| w.label.clone())
        .collect();
    let partner = rows
        .iter()
        .find(|label| {
            !label.starts_with("Teamster #4")
                && sent.is_none_or(|id| !label.starts_with(&format!("Teamster #{} ", id + 1)))
        })
        .cloned();
    match partner {
        Some(label) => {
            // With two fleets at the spot both start ticked; with more,
            // only the one in hand, and the partner is ticked by hand.
            if rows.len() > 2 {
                shell.press("merge", &label);
            }
            shell.press("merge", "OK");
            shell.frame();
            let game = shell.app.game.as_ref().expect("a game");
            let merged = game
                .fleets
                .iter()
                .find(|f| f.owner == 0 && f.id == 3)
                .expect("Teamster #4");
            assert!(
                merged.stacks.iter().map(|s| s.count).sum::<i32>() >= 2,
                "two Teamsters in the one fleet"
            );
        }
        None => {
            shell.press("merge", "Cancel");
            shell.frame();
        }
    }

    // Page 61: a Mini-Miner into Stove Top's queue; Armed Probe #9's
    // waypoint dragged from La Te Da to Speed Bump, then Lever. The Mine
    // Layer and the Berserker colony ship the page has are not here.
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    shell.press("planet", "Change");
    shell.press("production", "Top of the Queue");
    shell.scroll_to("production", "Mini-Miner", "Factory");
    shell.double_click("production", "Mini-Miner");
    shell.press("production", "OK");
    let probe = shell.fleet_on_screen(8);
    shell.click_at(probe);
    assert_eq!(shell.selected_fleet_id(), Some(8), "Armed Probe #9");
    let (from, to) = shell.two_planets_on_screen(LA_TE_DA, SPEED_BUMP);
    shell.drag(from, to);
    shell.frame();
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        assert_eq!(
            fleet.waypoints.get(1).and_then(|w| w.target),
            Some(SPEED_BUMP as u16),
            "{:?}",
            fleet.waypoints
        );
    }
    shell.shift_click_planet(LEVER);
    shell.frame();
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        assert_eq!(
            fleet.waypoints.get(2).and_then(|w| w.target),
            Some(LEVER as u16)
        );
    }
    shell.off_the_page = false;
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("no Mine Layer, no Berserker colony ship; on to 2424");
    }
    assert_eq!(shell.page(), 62, "2424");

    // --- 2424 -------------------------------------------------------------
    // Page 62: Stalwart Defender #5 back to Wallaby — it never left here,
    // having fought the Berserker over Wallaby itself, so that rung is
    // passed over; Mini-Miner #3, new this year, to Prune to merge; the
    // messages.
    shell.off_the_page = true;
    shell.next_message_until(stars_core::message::Goto::Fleet(2));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(2), "Mini-Miner #3");
    shell.shift_click_planet(PRUNE);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Merge with Fleet");
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        assert_eq!(fleet.waypoints[1].target, Some(5), "the Cotton Picker");
    }
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.off_the_page = false;
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("the destroyer is at Wallaby already; on to 2425");
    }
    assert_eq!(shell.page(), 63, "2425");

    // --- 2425 -------------------------------------------------------------
    // Page 63: the % view; the Santa Maria edited in place — the Long
    // Hump 6 dragged off into the parts list and a Daddy Long Legs 7
    // dragged on.
    for _ in 0..2 {
        if shell.app.message_next(false).is_some() {
            shell.press("messages", "Next");
        }
    }
    shell.press("toolbar", "%");
    shell.press("menu", "Commands");
    shell.press("menu", "Ship Design…");
    shell.press("designer", "Designs");
    shell.press("designer", "Santa Maria");
    shell.press("designer", "Edit Selected Design");
    shell.frame();
    assert!(
        shell
            .app
            .designer
            .as_ref()
            .is_some_and(|d| d.editing.is_some()),
        "the editor is open on the Santa Maria"
    );
    {
        // Off the slot and onto the list: any row of the list is inside
        // its drop zone.
        shell.frame();
        let from = shell
            .app
            .drawn_button("designer", "slot 0")
            .expect("the engine slot")
            .rect
            .center();
        let to = shell
            .app
            .drawn_button("designer", "Quick Jump 5")
            .expect("a row of the parts list")
            .rect
            .center();
        shell.step("designer: the Long Hump 6 off the engine slot");
        shell.drag(from, to);
    }
    fit(shell, "Daddy Long Legs 7", "slot 0");
    {
        let editing = shell
            .app
            .designer
            .as_ref()
            .and_then(|d| d.editing.as_ref())
            .expect("the editor");
        assert_eq!(
            editing
                .design
                .slots
                .first()
                .and_then(stars_core::design::slot_part)
                .map(|p| p.name),
            Some("Daddy Long Legs 7")
        );
    }
    shell.press("designer", "OK");
    shell.press("designer", "Done");
    shell.frame();
    assert_eq!(shell.page(), 64, "the saved design turns the page");

    // Page 64: three of the new Santa Marias at the head of Stove Top's
    // queue; Max Terraform at the end of Sea Squared's; Construction next;
    // the rest of the messages.
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    shell.press("planet", "Change");
    shell.press("production", "Top of the Queue");
    for _ in 0..3 {
        shell.double_click("production", "Santa Maria");
    }
    {
        let dialog = shell.app.production.as_ref().expect("open");
        assert_eq!(
            (
                dialog.queue[0].ship,
                dialog.queue[0].item,
                dialog.queue[0].count
            ),
            (true, 2, 3),
            "{:?}",
            dialog.queue
        );
    }
    shell.press("production", "OK");
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(507));
    let sea = shell.planet_on_screen(SEA_SQUARED);
    shell.click_at(sea);
    assert_eq!(shell.app.selection.planet, Some(SEA_SQUARED));
    shell.press("planet", "Change");
    {
        let last = shell
            .app
            .production_queue_rows()
            .last()
            .map(|(count, name)| format!("{name} up to {count}"))
            .expect("a queue");
        shell.press("production", &last);
    }
    shell.scroll_to("production", "Max Terraform", "Factory");
    shell.double_click("production", "Max Terraform");
    shell.double_click("production", "Max Terraform");
    {
        let dialog = shell.app.production.as_ref().expect("open");
        let last = dialog.queue.last().expect("the terraforming");
        assert_eq!(
            (dialog.queue.len(), last.item, last.count),
            (3, stars_core::production::item::AUTO_MAX_TERRAFORM, 2),
            "{:?}",
            dialog.queue
        );
    }
    shell.press("production", "OK");
    shell.next_message_until(stars_core::message::Goto::Research);
    shell.press("messages", "Goto");
    shell.app.research_dialog.as_mut().expect("the dialog").next =
        stars_core::research::NextField::Field(3);
    shell.press("research", "Done");
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.frame();
    assert_eq!(shell.page(), 64, "2425 is done, waiting on the turn");
    assert!(shell.app.tutor_waiting());
    shell.generate();
    assert_eq!(shell.page(), 65, "2426");

    // --- 2426 -------------------------------------------------------------
    // Page 65: the three new Santa Marias — one fleet, fleet 7 here where
    // the original's was 9 — filled with colonists and sent to settle
    // Lever, then Split All, and the second and third given Speed Bump
    // and Bloop instead by dragging their waypoints.
    shell.off_the_page = true;
    for _ in 0..4 {
        if shell.app.message_next(false).is_some() {
            shell.press("messages", "Next");
        }
    }
    let colonizers = {
        let game = shell.app.game.as_ref().expect("a game");
        game.fleets
            .iter()
            .find(|f| {
                f.owner == 0
                    && f.orbiting == Some(STOVE_TOP as u16)
                    && f.stacks.iter().any(|s| s.design == 2 && s.count == 3)
            })
            .map(|f| f.id)
            .expect("the three Santa Marias")
    };
    shell.next_message_until(stars_core::message::Goto::Fleet(colonizers));
    shell.press("messages", "Goto");
    assert_eq!(shell.selected_fleet_id(), Some(colonizers));
    shell.press("fleet", "Xfer");
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("xfer", "Colonists gauge")
        .expect("the colonists gauge")
        .rect;
    shell.click_at(egui::pos2(gauge.right() - 1.0, gauge.center().y));
    assert_eq!(shell.app.xfer.as_ref().expect("up").aboard()[3], 75);
    shell.press("xfer", "OK");
    shell.shift_click_planet(LEVER);
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Colonize");
    shell.press("fleet", "Split All");
    shell.frame();
    let halves: Vec<u16> = {
        let game = shell.app.game.as_ref().expect("a game");
        let mut ids: Vec<u16> = game
            .fleets
            .iter()
            .filter(|f| {
                f.owner == 0
                    && f.orbiting == Some(STOVE_TOP as u16)
                    && f.stacks.iter().any(|s| s.design == 2 && s.count == 1)
            })
            .map(|f| f.id)
            .collect();
        ids.sort_unstable();
        ids
    };
    assert_eq!(halves.len(), 3, "three fleets of one: {halves:?}");
    for (half, planet) in [(halves[1], SPEED_BUMP), (halves[2], BLOOP)] {
        let name = format!("Santa Maria #{}", half + 1);
        shell.right_click_planet_and_pick(STOVE_TOP, &name);
        assert_eq!(shell.selected_fleet_id(), Some(half), "{name}");
        let (from, to) = shell.two_planets_on_screen(LEVER, planet);
        shell.drag(from, to);
        shell.frame();
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        assert_eq!(
            (
                fleet.waypoints.get(1).and_then(|w| w.target),
                fleet.waypoints.get(1).map(|w| w.task)
            ),
            (Some(planet as u16), Some(stars_formats::task::COLONIZE)),
            "{name} bound for {planet:#x} to settle it"
        );
    }
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }

    // Page 66: three Teamsters into Stove Top's queue; Stalwart Defender
    // #5 after the Berserker ship by Wallaby, where there is one.
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    shell.press("planet", "Change");
    for _ in 0..3 {
        shell.double_click("production", "Teamster");
    }
    shell.press("production", "OK");
    let quarry = {
        let game = shell.app.game.as_ref().expect("a game");
        let wallaby = game
            .planets
            .iter()
            .find(|p| p.id == WALLABY)
            .and_then(|p| p.position)
            .expect("Wallaby");
        game.fleets
            .iter()
            .enumerate()
            .filter(|(i, f)| f.owner != 0 && shell.app.fleet_in_view(*i))
            .map(|(_, f)| {
                #[allow(clippy::cast_possible_truncation)]
                let d = stars_core::movement::distance(f.position, wallaby) as i64;
                (f, d)
            })
            .filter(|(_, d)| *d <= 60)
            .min_by_key(|(_, d)| *d)
            .map(|(f, _)| f.position)
    };
    if let Some(at) = quarry {
        let pos = shell.point_on_screen(at, WALLABY);
        shell.click_at(pos);
        shell.right_click_planet_and_pick(WALLABY, "Stalwart Defender #5");
        assert_eq!(shell.selected_fleet_id(), Some(4));
        let pos = shell.point_on_screen(at, WALLABY);
        shell.modifiers = egui::Modifiers::SHIFT;
        shell.click_at(pos);
        shell.modifiers = egui::Modifiers::NONE;
    }
    shell.off_the_page = false;
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("the colony ships have other numbers here; on to 2427");
    }
    assert_eq!(shell.page(), 67, "2427");

    // --- 2427 -------------------------------------------------------------
    // Pages 67 to 69: Stalwart Defender #5 in hand — the Berserker colony
    // ship it is to intercept died over Wallaby in 2426 here, so there is
    // nothing to pick from the waypoint's own menu; the messages; and the
    // destroyer designed to the page's recipe, ten of them queued.
    shell.off_the_page = true;
    shell.right_click_planet_and_pick(WALLABY, "Stalwart Defender #5");
    assert_eq!(shell.selected_fleet_id(), Some(4));
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.press("menu", "Commands");
    shell.press("menu", "Ship Design…");
    shell.press("designer", "Available Hull Types");
    shell.press("designer", "Designs");
    shell.press("designer", "Destroyer");
    shell.press("designer", "Copy Selected Design");
    shell.frame();
    assert!(
        shell
            .app
            .designer
            .as_ref()
            .is_some_and(|d| d.editing.is_some()),
        "the editor is open on the Destroyer"
    );
    shell.press("designer", "Parts");
    shell.press("designer", "Engines");
    fit(shell, "Radiating Hydro-Ram Scoop", "slot 0");
    shell.press("designer", "Parts");
    shell.press("designer", "Armor");
    fit(shell, "Carbonic Armor", "slot 4");
    fit(shell, "Carbonic Armor", "slot 4");
    shell.press("designer", "Parts");
    shell.press("designer", "Beam Weapons");
    fit(shell, "Yakimora Light Phaser", "slot 1");
    fit(shell, "Yakimora Light Phaser", "slot 2");
    fit(shell, "Yakimora Light Phaser", "slot 3");
    shell.press("designer", "Parts");
    shell.press("designer", "Mechanical");
    fit(shell, "Fuel Tank", "slot 5");
    shell.press("designer", "Parts");
    shell.press("designer", "Electrical");
    fit(shell, "Battle Computer", "slot 6");
    {
        let editing = shell
            .app
            .designer
            .as_ref()
            .and_then(|d| d.editing.as_ref())
            .expect("the editor");
        let counts: Vec<u8> = editing
            .design
            .slots
            .iter()
            .take(7)
            .map(|s| s.count)
            .collect();
        assert_eq!(counts, vec![1, 1, 1, 1, 2, 1, 1], "the destroyer's slots");
        let mass = editing.design.mass().expect("a mass");
        assert!(mass <= 100, "{mass} kT: under a hundred for the gates");
    }
    shell.press("designer", "picture right");
    shell.press("designer", "OK");
    shell.press("designer", "Done");
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    shell.press("planet", "Change");
    shell.press("production", "Top of the Queue");
    shell.scroll_to("production", "Destroyer", "Factory");
    shell.press("production", "Destroyer");
    shell.press_with(egui::Modifiers::SHIFT, "production", "Add ->");
    {
        // The destroyer is this player's eighth design, slot 7, where the
        // original's — a Mine Layer before it — was the ninth.
        let dialog = shell.app.production.as_ref().expect("open");
        let destroyer = shell
            .app
            .game
            .as_ref()
            .and_then(|g| g.designs[0].iter().position(|d| d.name == "Destroyer"))
            .expect("the destroyer design");
        assert_eq!(
            (
                dialog.queue[0].ship,
                usize::from(dialog.queue[0].item),
                dialog.queue[0].count
            ),
            (true, destroyer, 10),
            "{:?}",
            dialog.queue
        );
    }
    shell.press("production", "OK");
    shell.off_the_page = false;
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("no colony ship to intercept, no new Teamsters; on to 2428");
    }
    assert_eq!(shell.page(), 70, "2428");

    // --- 2428 -------------------------------------------------------------
    // Page 70: Stalwart Defender #5 to Wallaby, where it already is; the
    // new destroyers — five of them, one fleet — sent to Hacker; the
    // messages.
    shell.off_the_page = true;
    let armada = {
        let game = shell.app.game.as_ref().expect("a game");
        game.fleets
            .iter()
            .find(|f| f.owner == 0 && f.stacks.iter().any(|s| s.design == 7))
            .map(|f| f.id)
            .expect("the destroyers")
    };
    shell.next_message_until(stars_core::message::Goto::Fleet(armada));
    shell.press("messages", "Goto");
    assert_eq!(
        shell.selected_fleet_id(),
        Some(armada),
        "the destroyer armada"
    );
    shell.shift_click_planet(HACKER);
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        assert_eq!(fleet.waypoints[1].target, Some(HACKER as u16));
    }
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.off_the_page = false;
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("the destroyer is at Wallaby already; on to 2429");
    }
    assert_eq!(shell.page(), 71, "2429");

    // --- 2429 -------------------------------------------------------------
    // Page 71: Teamster #4's leg home slowed to warp 5 on the tile's warp
    // gauge; the new destroyers to Hacker — the page's fleet 13, whatever
    // number they have here; Stove Top routed to Hacker with a
    // control-click.
    shell.off_the_page = true;
    let hauler = shell.fleet_on_screen(3);
    shell.click_at(hauler);
    assert_eq!(shell.selected_fleet_id(), Some(3), "Teamster #4");
    shell.press("fleet", "Stove Top");
    assert_eq!(shell.app.selection.waypoint, Some(1));
    shell.frame();
    let gauge = shell
        .app
        .drawn_button("fleet", "Warp gauge")
        .expect("the warp gauge")
        .rect;
    shell.click_at(egui::pos2(
        gauge.left() + gauge.width() * 5.5 / 11.0,
        gauge.center().y,
    ));
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        assert_eq!(fleet.waypoints[1].warp, 5, "warp 5 home");
    }
    shell.frame();
    assert_eq!(shell.app.tutor.as_ref().map(|t| t.bold), Some(564));
    for _ in 0..4 {
        if shell.app.message_next(false).is_some() {
            shell.press("messages", "Next");
        }
    }
    let new_destroyers = {
        let game = shell.app.game.as_ref().expect("a game");
        game.fleets
            .iter()
            .find(|f| {
                f.owner == 0
                    && f.orbiting == Some(STOVE_TOP as u16)
                    && f.stacks.iter().any(|s| s.design == 7)
            })
            .map(|f| f.id)
    };
    if let Some(id) = new_destroyers {
        shell.right_click_planet_and_pick(STOVE_TOP, &format!("Destroyer #{}", id + 1));
        assert_eq!(shell.selected_fleet_id(), Some(id));
        shell.shift_click_planet(HACKER);
    }
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    assert_eq!(shell.app.selection.planet, Some(STOVE_TOP));
    let hacker = shell.planet_on_screen(HACKER);
    shell.modifiers = egui::Modifiers::COMMAND;
    shell.click_at(hacker);
    shell.modifiers = egui::Modifiers::NONE;
    shell.frame();
    {
        let game = shell.app.game.as_ref().expect("a game");
        let home = game
            .planets
            .iter()
            .find(|p| p.id == STOVE_TOP)
            .expect("home");
        assert_eq!(home.route_dest, Some(HACKER), "routed to Hacker");
    }

    // Page 72: three messages; Energy next; the rest; Teamster #7 scrapped
    // where it sits.
    for _ in 0..3 {
        if shell.app.message_next(false).is_some() {
            shell.press("messages", "Next");
        }
    }
    shell.press("menu", "Commands");
    shell.press("menu", "Research…");
    shell.app.research_dialog.as_mut().expect("the dialog").next =
        stars_core::research::NextField::Field(0);
    shell.press("research", "Done");
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    // Teamster #7, its colonists put down at Sea Squared and too little
    // fuel to come home, scrapped where it sits — which is Sea Squared
    // with one Teamster's tank, and wherever a pair's larger tank got it.
    let at_sea_squared = shell.app.game.as_ref().is_some_and(|game| {
        game.fleets
            .iter()
            .any(|f| f.owner == 0 && f.id == 6 && f.orbiting == Some(SEA_SQUARED as u16))
    });
    if at_sea_squared {
        shell.right_click_planet_and_pick(SEA_SQUARED, "Teamster #7");
    } else {
        assert!(shell.app.goto_fleet(6), "Teamster #7 is still ours");
        shell.frame();
    }
    assert_eq!(shell.selected_fleet_id(), Some(6), "Teamster #7");
    shell.press("fleet", "Waypoint Task");
    shell.press("fleet", "Scrap Fleet");
    {
        let game = shell.app.game.as_ref().expect("a game");
        let fleet = &game.fleets[shell.app.selection.fleet.expect("in hand")];
        // On the waypoint in hand: where it sits, or where it is bound.
        assert!(
            fleet
                .waypoints
                .iter()
                .any(|w| w.task == stars_formats::task::SCRAP),
            "{:?}",
            fleet.waypoints
        );
    }

    // Page 73, still 2429: the B-17 Bomber — Radiating Hydro-Ram Scoops,
    // four Black Cat Bombs on each bomb slot, a Fuel Tank in the slot left
    // — and ten of them queued at Stove Top.
    shell.press("menu", "Commands");
    shell.press("menu", "Ship Design…");
    shell.press("designer", "Available Hull Types");
    shell.press("designer", "Designs");
    // The list of hulls has grown past the dropdown: rolled down to it.
    shell.scroll_to("designer", "B-17 Bomber", "Destroyer");
    shell.press("designer", "B-17 Bomber");
    shell.press("designer", "Copy Selected Design");
    shell.frame();
    assert!(
        shell
            .app
            .designer
            .as_ref()
            .is_some_and(|d| d.editing.is_some()),
        "the editor is open on the bomber"
    );
    shell.press("designer", "Parts");
    shell.press("designer", "Engines");
    shell.modifiers = egui::Modifiers::SHIFT;
    fit(shell, "Radiating Hydro-Ram Scoop", "slot 0");
    shell.modifiers = egui::Modifiers::NONE;
    shell.press("designer", "Parts");
    shell.press("designer", "Bombs");
    shell.modifiers = egui::Modifiers::SHIFT;
    fit(shell, "Black Cat Bomb", "slot 1");
    fit(shell, "Black Cat Bomb", "slot 2");
    shell.modifiers = egui::Modifiers::NONE;
    shell.press("designer", "Parts");
    shell.press("designer", "Mechanical");
    fit(shell, "Fuel Tank", "slot 3");
    {
        let editing = shell
            .app
            .designer
            .as_ref()
            .and_then(|d| d.editing.as_ref())
            .expect("the editor");
        let counts: Vec<u8> = editing
            .design
            .slots
            .iter()
            .take(4)
            .map(|s| s.count)
            .collect();
        assert_eq!(counts, vec![2, 4, 4, 1], "the bomber's slots");
    }
    shell.press("designer", "OK");
    shell.press("designer", "Done");
    let bomber = shell
        .app
        .game
        .as_ref()
        .and_then(|g| g.designs[0].iter().position(|d| d.name == "B-17 Bomber"))
        .expect("the bomber design");
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    shell.press("planet", "Change");
    shell.press("production", "Top of the Queue");
    shell.scroll_to("production", "B-17 Bomber", "Factory");
    shell.press("production", "B-17 Bomber");
    shell.press_with(egui::Modifiers::SHIFT, "production", "Add ->");
    {
        let dialog = shell.app.production.as_ref().expect("open");
        assert_eq!(
            (
                dialog.queue[0].ship,
                usize::from(dialog.queue[0].item),
                dialog.queue[0].count
            ),
            (true, bomber, 10),
            "{:?}",
            dialog.queue
        );
    }
    shell.press("production", "OK");
    shell.off_the_page = false;
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("the destroyers have other numbers here; on to 2430");
    }
    assert_eq!(shell.page(), 74, "2430");

    // --- 2430 -------------------------------------------------------------
    // Page 74: the messages, the new bombers among them when Stove Top
    // has built any.
    shell.off_the_page = true;
    let bombers = {
        let game = shell.app.game.as_ref().expect("a game");
        game.fleets
            .iter()
            .find(|f| f.owner == 0 && f.stacks.iter().any(|s| usize::from(s.design) == bomber))
            .map(|f| f.id)
    };
    if let Some(id) = bombers {
        shell.next_message_until(stars_core::message::Goto::Fleet(id));
        shell.press("messages", "Goto");
        assert_eq!(shell.selected_fleet_id(), Some(id), "the bombers");
    }
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.off_the_page = false;
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("no bombers yet; on to 2431");
    }
    assert_eq!(shell.page(), 75, "2431");

    // --- 2431 -------------------------------------------------------------
    // Page 75: four messages, Wallaby, a hundred mines with Control on Add,
    // the rest of the messages.
    shell.off_the_page = true;
    for _ in 0..4 {
        if shell.app.message_next(false).is_some() {
            shell.press("messages", "Next");
        }
    }
    let wallaby = shell.planet_on_screen(WALLABY);
    shell.click_at(wallaby);
    assert_eq!(shell.app.selection.planet, Some(WALLABY));
    shell.press("planet", "Change");
    shell.frame();
    // Wallaby offers mines only while its people can run more of them;
    // the original's Wallaby had been filling for years.
    let mines_offered = shell.app.drawn_button("production", "Mine").is_some()
        || shell
            .app
            .production_inventory()
            .iter()
            .any(|row| !row.ship && row.item == stars_core::production::item::MINE);
    if mines_offered {
        shell.scroll_to("production", "Mine", "Factory");
        shell.press("production", "Mine");
        shell.press("production", "Top of the Queue");
        shell.press_with(egui::Modifiers::COMMAND, "production", "Add ->");
        // As many as Wallaby's people can run, which the inventory caps
        // the hundred at.
        let dialog = shell.app.production.as_ref().expect("open");
        assert_eq!(
            dialog.queue[0].item,
            stars_core::production::item::MINE,
            "{:?}",
            dialog.queue
        );
        assert!(dialog.queue[0].count > 0);
    }
    shell.press("production", "OK");
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.off_the_page = false;
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("on to 2432");
    }
    assert_eq!(shell.page(), 76, "2432");

    // --- 2432 -------------------------------------------------------------
    // Page 76: the destroyers in hand — damaged, if they have fought — and
    // the messages, the attack on Hacker among them.
    shell.off_the_page = true;
    for _ in 0..3 {
        if shell.app.message_next(false).is_some() {
            shell.press("messages", "Next");
        }
    }
    {
        let armada = {
            let game = shell.app.game.as_ref().expect("a game");
            game.fleets
                .iter()
                .find(|f| f.owner == 0 && f.stacks.iter().any(|s| s.design == 7))
                .map(|f| (f.id, f.position))
        };
        if let Some((id, at)) = armada {
            let pos = shell.point_on_screen(at, HACKER);
            shell.click_at(pos);
            if shell.selected_fleet_id() != Some(id) {
                shell.double_click_at(pos);
            }
        }
    }
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.off_the_page = false;
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("on to 2433");
    }
    assert_eq!(shell.page(), 77, "2433");

    // --- 2433 -------------------------------------------------------------
    // Page 77: six messages, Stove Top, ten more bombers, the rest.
    shell.off_the_page = true;
    for _ in 0..6 {
        if shell.app.message_next(false).is_some() {
            shell.press("messages", "Next");
        }
    }
    shell.right_click_planet_and_pick(STOVE_TOP, "Stove Top");
    shell.press("planet", "Change");
    shell.press("production", "Top of the Queue");
    shell.scroll_to("production", "B-17 Bomber", "Factory");
    shell.press("production", "B-17 Bomber");
    shell.press_with(egui::Modifiers::SHIFT, "production", "Add ->");
    shell.press("production", "OK");
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.off_the_page = false;
    if shell.app.tutor_waiting() {
        shell.generate();
    } else {
        shell.generate_anyway("on to 2434");
    }
    assert_eq!(shell.page(), 78, "2434");

    // --- 2434 and 2435 ----------------------------------------------------
    // Pages 78 and 79: the messages, and the year.
    for (page, year) in [(79, "2435"), (80, "2436")] {
        while shell.app.message_next(false).is_some() {
            shell.press("messages", "Next");
        }
        shell.frame();
        assert!(
            shell.app.tutor_waiting(),
            "the messages read see the page done"
        );
        shell.generate();
        assert_eq!(shell.page(), page, "{year}");
    }

    // --- 2436 -------------------------------------------------------------
    // Page 80, the last: the score (F10 is the shell's key), the messages,
    // and the Generate that ends the tutorial.
    shell.press("menu", "Report");
    shell.press("menu", "Score…");
    shell.frame();
    assert!(shell.app.score_sheet.is_some(), "the score is up");
    shell.app.close_score_sheet();
    while shell.app.message_next(false).is_some() {
        shell.press("messages", "Next");
    }
    shell.frame();
    assert!(shell.app.tutor_waiting(), "the last page is done");
    shell.press("menu", "Turn");
    shell.press_unchecked("menu", "Generate");
    shell.frame();
    assert!(
        shell.app.tutor.as_ref().is_none_or(|t| t.finished),
        "the tutorial is over: you are on your own"
    );
}
