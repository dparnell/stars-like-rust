//! The Battle Plans dialog — Commands (Battle Plans...), F6.
//!
//! See `docs/ui/battle-plans.md`.

use stars_core::battle::{Tactic, TargetClass};
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::App;

/// A two-player game with the dialog open.
fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "plans".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.open_battle_plans();
    app
}

#[test]
fn it_opens_on_the_five_plans_a_new_game_gives() {
    let app = a_game();
    let names: Vec<&str> = app
        .battle_plan_list()
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "Default",
            "Kill Starbase",
            "Max-Defense",
            "Sniper",
            "Chicken"
        ]
    );
    assert_eq!(app.battle_plans.as_ref().expect("open").selected, 0);
}

/// The stock plans, read through the enumerations the dialog's combos hold.
#[test]
fn the_stock_plans_read_as_the_dialog_would_show_them() {
    let app = a_game();
    let plans = app.battle_plan_list();
    let tactic = |slot: usize| Tactic::from_raw(plans[slot].tactic_nibble());
    let primary = |slot: usize| TargetClass::from_raw(plans[slot].primary_target);
    let secondary = |slot: usize| TargetClass::from_raw(plans[slot].secondary_target);

    assert_eq!(tactic(0), Some(Tactic::MaximiseDamageRatio));
    assert_eq!(primary(0), TargetClass::ArmedShips);
    assert_eq!(secondary(0), TargetClass::Any);

    assert_eq!(primary(1), TargetClass::Starbase, "Kill Starbase");
    // Sniper shoots at what cannot shoot back and runs the moment it is hit.
    assert_eq!(tactic(3), Some(Tactic::DisengageIfChallenged), "Sniper");
    assert_eq!(primary(3), TargetClass::UnarmedShips, "Sniper");
    // Chicken does not fight at all.
    assert_eq!(tactic(4), Some(Tactic::Disengage), "Chicken");
    assert_eq!(primary(4), TargetClass::None, "Chicken");

    // None of them dumps cargo, and none is deleted.
    assert!(plans.iter().all(|p| !p.dump_cargo() && !p.deleted()));
}

#[test]
fn every_field_can_be_set_and_comes_back() {
    let mut app = a_game();
    app.select_battle_plan(2);
    assert!(app.set_battle_plan_tactic(Tactic::MinimiseDamageToSelf));
    assert!(app.set_battle_plan_target(true, TargetClass::Freighters));
    assert!(app.set_battle_plan_target(false, TargetClass::Starbase));
    assert!(app.set_battle_plan_attack_who(1));
    assert!(app.set_battle_plan_dump_cargo(true));

    let plan = app.selected_battle_plan().expect("a plan");
    assert_eq!(
        Tactic::from_raw(plan.tactic_nibble()),
        Some(Tactic::MinimiseDamageToSelf)
    );
    assert_eq!(
        TargetClass::from_raw(plan.primary_target),
        TargetClass::Freighters
    );
    assert_eq!(
        TargetClass::from_raw(plan.secondary_target),
        TargetClass::Starbase
    );
    assert_eq!(plan.attack_who, 1);
    assert!(plan.dump_cargo());
}

/// The tactic and the two flags share a byte, so setting one must not disturb
/// the others.
#[test]
fn dump_cargo_and_the_tactic_share_a_byte_without_colliding() {
    let mut app = a_game();
    app.select_battle_plan(1);
    app.set_battle_plan_dump_cargo(true);
    app.set_battle_plan_tactic(Tactic::MaximiseDamage);
    let plan = app.selected_battle_plan().expect("a plan");
    assert!(plan.dump_cargo(), "setting the tactic cleared dump cargo");
    assert_eq!(plan.tactic_nibble(), Tactic::MaximiseDamage as u8);
    assert!(!plan.deleted());

    app.set_battle_plan_dump_cargo(false);
    let plan = app.selected_battle_plan().expect("a plan");
    assert!(!plan.dump_cargo());
    assert_eq!(plan.tactic_nibble(), Tactic::MaximiseDamage as u8);
}

#[test]
fn copying_a_plan_numbers_it() {
    let mut app = a_game();
    app.select_battle_plan(1);
    assert!(app.copy_battle_plan());
    assert_eq!(app.battle_plan_count(), 6);
    let plan = app.selected_battle_plan().expect("the copy");
    assert_eq!(plan.name, "Kill Starbase (2)");
    // The copy is selected, with its rename box open — `Copy` falls straight
    // through into the rename dialog in the original.
    assert_eq!(app.battle_plans.as_ref().expect("open").selected, 5);
    assert_eq!(
        app.battle_plans.as_ref().expect("open").rename.as_deref(),
        Some("Kill Starbase (2)")
    );

    // Copying the copy bumps the number rather than adding another suffix.
    assert!(app.copy_battle_plan());
    assert_eq!(
        app.selected_battle_plan().expect("the copy").name,
        "Kill Starbase (3)"
    );
}

#[test]
fn the_number_wraps_at_nine() {
    assert_eq!(stars_ui::copied_plan_name("Plan"), "Plan (2)");
    assert_eq!(stars_ui::copied_plan_name("Plan (2)"), "Plan (3)");
    assert_eq!(stars_ui::copied_plan_name("Plan (9)"), "Plan (0)");
    // A name of 28 characters or more is copied as it stands.
    let long = "x".repeat(28);
    assert_eq!(stars_ui::copied_plan_name(&long), long);
}

#[test]
fn there_can_be_fifteen_plans() {
    let mut app = a_game();
    while app.battle_plan_count() < stars_ui::MAX_BATTLE_PLANS {
        assert!(app.copy_battle_plan(), "should still have room");
    }
    assert_eq!(app.battle_plan_count(), 15);
    assert!(
        !app.copy_battle_plan(),
        "fifteen is as many as there can be"
    );
}

#[test]
fn the_first_plan_cannot_be_renamed_or_deleted() {
    let mut app = a_game();
    app.select_battle_plan(0);
    assert!(!app.rename_battle_plan("Something else"));
    assert!(!app.delete_selected_battle_plan());
    assert_eq!(app.battle_plan_count(), 5);
    assert_eq!(app.battle_plan_list()[0].name, "Default");
}

#[test]
fn renaming_and_deleting_a_later_plan_works() {
    let mut app = a_game();
    app.select_battle_plan(3);
    assert!(app.rename_battle_plan("Ambush"));
    assert_eq!(app.battle_plan_list()[3].name, "Ambush");

    assert!(app.delete_selected_battle_plan());
    assert_eq!(app.battle_plan_count(), 4);
    assert_eq!(app.battle_plans.as_ref().expect("open").selected, 2);
    let names: Vec<&str> = app
        .battle_plan_list()
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Default", "Kill Starbase", "Max-Defense", "Chicken"]
    );
}

/// The four fixed choices, then everyone else — never yourself.
#[test]
fn attack_who_lists_the_other_players() {
    let app = a_game();
    let options = app.battle_plan_attack_options();
    let names: Vec<&str> = options.iter().map(|(_, name)| name.as_str()).collect();
    assert_eq!(
        names[..4],
        ["Nobody", "Enemies", "Neutrals & Enemies", "Everyone"]
    );
    assert_eq!(
        options[..4].iter().map(|(v, _)| *v).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    // Two players, so one other, stored as `4 + their index`.
    assert_eq!(options.len(), 5);
    assert_eq!(options[4].0, 5);
    assert!(!names[4..].contains(&app.player_name(app.local_player()).as_str()));
}

#[test]
fn it_opens_on_the_selected_fleet_s_plan() {
    let mut app = a_game();
    app.close_battle_plans();
    // Give the first fleet a plan other than the default and select it.
    let fleet = app
        .game
        .as_ref()
        .expect("a game")
        .fleets
        .iter()
        .position(|f| f.owner == 0)
        .expect("a fleet of ours");
    app.selection.fleet = Some(fleet);
    assert!(app.set_battle_plan(fleet, 2));
    app.open_battle_plans();
    assert_eq!(app.battle_plans.as_ref().expect("open").selected, 2);
}
