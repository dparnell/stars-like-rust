//! The tile both panes end with: `DrawPlanetShipList` (`1048:377e`).
//!
//! See `docs/ui/fleet-pane.md`.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::App;

/// A game with two fleets of ours at the homeworld, and one of theirs there
/// too, so the tile has something to leave out and something it cannot know.
fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "here".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app
}

/// Where our first fleet is, and its index.
fn a_fleet(app: &App) -> (usize, stars_core::movement::Point) {
    let game = app.game.as_ref().expect("a game");
    let index = game
        .fleets
        .iter()
        .position(|f| f.owner == 0)
        .expect("a fleet of ours");
    (index, game.fleets[index].position)
}

#[test]
fn the_title_says_which_pane_it_is_in() {
    let mut app = a_game();
    // A planet selected: everything in orbit.
    app.selection.on_fleet = false;
    assert_eq!(app.pane_fleets_title(), "Fleets in Orbit");

    // A fleet selected: the others here.
    let (index, _) = a_fleet(&app);
    app.selection.fleet = Some(index);
    app.selection.on_fleet = true;
    assert_eq!(app.pane_fleets_title(), "Other Fleets Here");
}

/// With a fleet selected the tile leaves that fleet out — the original passes
/// it as `idSkip` — and lists whatever else is at the same place.
#[test]
fn the_selected_fleet_is_the_one_left_out() {
    let mut app = a_game();
    let (index, at) = a_fleet(&app);
    let fleet = &app.game.as_ref().expect("a game").fleets[index];
    let key = (fleet.owner, fleet.id);

    app.selection.fleet = Some(index);
    app.selection.on_fleet = false;
    let in_orbit = app.pane_fleet_list();
    assert!(
        in_orbit.iter().any(|entry| entry.key == key),
        "a planet's tile lists everything in orbit"
    );

    app.selection.on_fleet = true;
    let others = app.pane_fleet_list();
    assert!(
        !others.iter().any(|entry| entry.key == key),
        "a fleet's tile leaves that fleet out"
    );
    assert_eq!(others.len() + 1, in_orbit.len());
    assert!(
        others
            .iter()
            .chain(in_orbit.iter())
            .all(|entry| app.game.as_ref().expect("a game").fleets[entry.index].position == at),
        "only what is at this place"
    );
}

#[test]
fn the_dropdown_starts_on_the_first_and_remembers_a_choice() {
    let mut app = a_game();
    app.selection.on_fleet = false;
    let list = app.pane_fleet_list();
    assert!(list.len() > 1, "the homeworld starts with several fleets");
    assert_eq!(app.pane_fleet_choice(), Some(list[0].index));

    app.choose_pane_fleet(list[1].key);
    assert_eq!(app.pane_fleet_choice(), Some(list[1].index));

    // A choice that is no longer in the list falls back to the first.
    app.choose_pane_fleet((7, 9999));
    assert_eq!(app.pane_fleet_choice(), Some(list[0].index));
}

#[test]
fn the_gauges_read_the_chosen_fleet() {
    let mut app = a_game();
    app.selection.on_fleet = false;
    let list = app.pane_fleet_list();
    app.choose_pane_fleet(list[0].key);

    let gauges = app.pane_fleet_gauges().expect("our own fleet");
    let fleet = &app.game.as_ref().expect("a game").fleets[list[0].index];
    assert_eq!(gauges.fuel, fleet.cargo.fuel);
    assert_eq!(gauges.minerals, fleet.cargo.minerals);
    assert_eq!(gauges.colonists, fleet.cargo.colonists);
    assert_eq!(
        gauges.cargo(),
        fleet.cargo.minerals.iter().sum::<i32>() + fleet.cargo.colonists
    );
    assert!(gauges.fuel_capacity > 0, "a starting fleet holds fuel");
    assert!(gauges.fuel <= gauges.fuel_capacity);
    assert!(gauges.cargo() <= gauges.cargo_capacity);
}

/// Somebody else's fleet is seen, not known: the original draws gauges only
/// for what it has in full detail.
#[test]
fn another_player_s_fleet_gets_no_gauges() {
    let mut app = a_game();
    // Put one of theirs where we can see it, at our own fleet's position.
    let (index, at) = a_fleet(&app);
    let theirs = {
        let game = app.game.as_mut().expect("a game");
        let theirs = game
            .fleets
            .iter()
            .position(|f| f.owner == 1)
            .expect("a fleet of theirs");
        game.fleets[theirs].position = at;
        (game.fleets[theirs].owner, game.fleets[theirs].id)
    };
    app.selection.fleet = Some(index);
    app.selection.on_fleet = false;

    app.choose_pane_fleet(theirs);
    let entry = app
        .pane_fleet_list()
        .into_iter()
        .find(|entry| entry.key == theirs)
        .expect("theirs is listed");
    assert!(!entry.mine);
    assert!(
        app.pane_fleet_gauges().is_none(),
        "their cargo is not ours to read"
    );
}

#[test]
fn nothing_here_is_an_empty_list_and_no_gauges() {
    let mut app = a_game();
    if let Some(game) = app.game.as_mut() {
        game.fleets.clear();
    }
    app.selection.fleet = None;
    app.selection.on_fleet = false;
    assert!(app.pane_fleet_list().is_empty());
    assert_eq!(app.pane_fleet_choice(), None);
    assert!(app.pane_fleet_gauges().is_none());
}
