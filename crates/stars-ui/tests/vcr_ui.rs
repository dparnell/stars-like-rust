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
