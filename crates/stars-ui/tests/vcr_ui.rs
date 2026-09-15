//! The Battle VCR on a battle the engine fought: the message's button reads
//! View, View opens the recording, the transport plays it, Done closes it —
//! what the tutorial's page 37 asks for.

use stars_core::combat;
use stars_ui::App;

/// A tutorial game with the human's armed probe and a Berserker scout at
/// the same point in space, and a plan that attacks everyone.
fn a_game_with_a_fight() -> App {
    let mut app = App::new();
    let (config, seed) = stars_core::newgame::tutorial();
    app.new_game_seeded(&config, seed).expect("a game");
    let state = app.game.as_mut().expect("game");
    let ours = state
        .fleets
        .iter()
        .position(|f| {
            f.owner == 0
                && f.stacks.iter().any(|s| {
                    state.designs[0]
                        .get(usize::from(s.design))
                        .is_some_and(stars_core::design::ShipDesign::is_armed)
                })
        })
        .expect("an armed fleet");
    let theirs = state
        .fleets
        .iter()
        .position(|f| f.owner == 1 && f.id == 0)
        .expect("a Berserker scout");
    let at = stars_core::movement::Point::new(1300, 1300);
    for index in [ours, theirs] {
        let fleet = &mut state.fleets[index];
        fleet.position = at;
        fleet.orbiting = None;
        fleet.waypoints.truncate(1);
        fleet.waypoints[0].position = at;
        fleet.waypoints[0].target = None;
    }
    for plan in &mut state.players[0].battle_plans {
        plan.attack_who = combat::attack_who::EVERYONE;
    }
    // The Berserkers' turn would move their scout away first.
    state.players[1].control = stars_core::ai::Control::Human;
    app
}

/// One frame of the shared screen, as the shell and the tutorial harness
/// draw it.
fn frame(app: &mut App) {
    let ctx = egui::Context::default();
    app.start_frame();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        stars_ui::views::frame::game_screen(app, ctx);
    });
}

#[test]
fn the_battle_message_opens_the_vcr_and_done_closes_it() {
    let mut app = a_game_with_a_fight();
    app.generate_turn();
    assert_eq!(app.battles.len(), 1, "the year's battle reached the app");

    // The battle message: its button reads View.
    let mut found = false;
    for _ in 0..40 {
        if app
            .current_message()
            .is_some_and(|m| m.id == stars_core::message::id::BATTLE)
        {
            found = true;
            break;
        }
        app.show_next_message();
    }
    assert!(found, "a battle message");
    assert_eq!(app.message_goto_label(), "View");
    assert!(matches!(
        app.message_goto(),
        stars_core::message::Goto::Position(1300, 1300)
    ));

    // View opens the recording; the tutor's check sees it.
    assert!(!app.tutor_check(&stars_ui::tutorial::Check::BattleVcr { open: true }));
    assert!(app.message_goto_follow());
    assert!(app.vcr.is_some());
    assert!(app.tutor_check(&stars_ui::tutorial::Check::BattleVcr { open: true }));

    // The VCR draws under the shared frame, with its buttons recorded.
    frame(&mut app);
    let labels: Vec<String> = app
        .drawn
        .iter()
        .filter(|w| w.scope == "vcr")
        .map(|w| w.label.clone())
        .collect();
    assert!(labels.iter().any(|l| l == "Done"), "{labels:?}");
    assert!(labels.iter().any(|l| l == ">/||"), "{labels:?}");

    // The transport plays it through.
    {
        let vcr = app.vcr.as_mut().expect("open");
        assert!(vcr.len() > 1, "the recording has frames");
        assert!(vcr.can_advance());
        vcr.end();
        assert!(!vcr.can_advance());
        assert!(vcr.can_rewind());
    }

    // Done.
    app.close_battle();
    assert!(app.vcr.is_none());
    assert!(!app.tutor_check(&stars_ui::tutorial::Check::BattleVcr { open: true }));
}

/// A battle at a planet is found by the planet the message names.
#[test]
fn a_battle_at_a_planet_is_found_by_the_planet() {
    let mut app = a_game_with_a_fight();
    let state = app.game.as_mut().expect("game");
    let home = state
        .planets
        .iter()
        .find(|p| p.owner == Some(1))
        .cloned()
        .expect("their home");
    let at = home.position.expect("placed");
    for fleet in state
        .fleets
        .iter_mut()
        .filter(|f| f.position == stars_core::movement::Point::new(1300, 1300))
    {
        fleet.position = at;
        fleet.orbiting = Some(home.id as u16);
        fleet.waypoints[0].position = at;
        fleet.waypoints[0].target = Some(home.id as u16);
    }
    app.generate_turn();
    assert_eq!(app.battles.len(), 1);
    assert_eq!(app.battles[0].planet, home.id as u16);
    for _ in 0..40 {
        if app
            .current_message()
            .is_some_and(|m| m.id == stars_core::message::id::BATTLE)
        {
            break;
        }
        app.show_next_message();
    }
    assert_eq!(app.message_goto_label(), "View");
    assert!(app.message_goto_follow(), "the battle at the planet opens");
    assert!(app.vcr.is_some());
}

/// The board draws the recording the original's way with the game's own
/// pictures — the ship, its owner's emblem, the bursts and torpedoes of
/// `AnimateAttack` — and a click on a square picks its stack out, cycling
/// through the stacks there.
#[test]
fn the_board_draws_with_the_pictures_and_a_click_picks_a_stack() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    let Ok(bytes) = std::fs::read(root.join("stars.2.7j.exe")) else {
        eprintln!("skipping: no copy of the original");
        return;
    };
    // A recorded battle with shots in it — the Exodus game's, when the
    // fixtures are there — draws every frame, the firing ones with their
    // beams, torpedoes and bursts.
    let fixtures = root.join("../fixtures/games/exodus");
    let mut recorded = None;
    for year in (2400..2500).step_by(2) {
        let mut app = App::new();
        if app
            .open(&fixtures.join(format!("{year}/exodus.m6")))
            .is_ok()
            && !app.battles.is_empty()
        {
            recorded = Some(app);
            break;
        }
    }
    if let Some(mut app) = recorded {
        app.load_art(bytes.clone(), "the test's copy")
            .expect("the pictures load");
        app.open_battle(0);
        let frames = app.vcr.as_ref().expect("open").len();
        let mut fired = 0;
        for position in 0..=frames {
            app.vcr.as_mut().expect("open").seek(position);
            if let Some(stars_ui::vcr::Event::Fire { shots, .. }) =
                app.vcr.as_ref().expect("open").frame().map(|f| &f.event)
            {
                fired += shots.len();
                for shot in shots {
                    assert!(shot.beam() || shot.torpedo() || shot.weapon == 0);
                }
            }
            frame(&mut app);
        }
        assert!(fired > 0, "the recording has shots in it");
    }

    let mut app = a_game_with_a_fight();
    app.load_art(bytes, "the test's copy")
        .expect("the pictures load");
    app.generate_turn();
    assert_eq!(app.battles.len(), 1, "the year's battle reached the app");
    app.open_battle(0);

    // The focus follows a click on the board: the first stack on the
    // square, then the next on another click.
    let ctx = egui::Context::default();
    let mut clicks = 0;
    app.vcr.as_mut().expect("open").rewind();
    let square = app.vcr.as_ref().expect("open").tokens()[0]
        .square
        .expect("on the board");
    for pass in 0..3 {
        let mut events = Vec::new();
        if pass > 0 {
            // The board's origin is ten pixels into its painter, whose top
            // left the first pass records; a click on the token's square.
            if let Some(origin) = app
                .drawn
                .iter()
                .find(|w| w.label == "board")
                .map(|w| w.rect.min)
            {
                let at = origin
                    + egui::vec2(10.0, 10.0)
                    + egui::vec2(
                        f32::from(square.0) * 35.0 + 16.0,
                        f32::from(square.1) * 35.0 + 16.0,
                    );
                events.push(egui::Event::PointerMoved(at));
                events.push(egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                });
                events.push(egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                });
                clicks += 1;
            }
        }
        app.start_frame();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 800.0),
                )),
                time: Some(f64::from(pass) * 2.0),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::Window::new("Battle VCR")
                    .default_width(640.0)
                    .show(ctx, |ui| stars_ui::views::battles::view(&mut app, ui));
            },
        );
    }
    assert!(clicks > 0, "the board was drawn and clicked");
    assert_eq!(
        app.vcr.as_ref().expect("open").focus,
        Some(0),
        "the click picked the stack on the square"
    );
}
