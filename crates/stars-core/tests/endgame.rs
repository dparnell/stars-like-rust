//! What the year's scoring tells the players: a player wiped out, and
//! the winners when the game is decided (`UpdatePlayerScores`,
//! `10b8:6258`). See `docs/formulas/scores.md`, *Victory*.

use stars_core::message::id;
use stars_core::movement::Point;
use stars_core::planet::Planet;
use stars_core::race::Race;
use stars_core::{generate_turn, GameState, Player, Rng};
use stars_formats::victory;

/// Three players; the first two with a planet apiece, the third with
/// nothing at all.
fn a_game() -> GameState {
    let mut state = GameState::new(3);
    state.galaxy_size = 1;
    state.galaxy_planets = 2;
    state.players = vec![
        Player::new(Race::humanoid()),
        Player::new(Race::humanoid()),
        Player::new(Race::humanoid()),
    ];
    state.designs = vec![Vec::new(), Vec::new(), Vec::new()];
    for (id, owner) in [(1, 0), (2, 1)] {
        let mut planet = Planet::unowned(id);
        planet.owner = Some(owner);
        planet.position = Some(Point::new(1000 + 300 * id, 1000));
        planet.pop = 10_000;
        planet.surface_min = [100, 100, 100];
        state.planets.push(planet);
    }
    state.turn = 40;
    state
}

/// The news put at the front of a player's messages this year.
fn first_news(state: &GameState, player: usize) -> Vec<(u16, Vec<i16>)> {
    state
        .messages
        .iter()
        .filter(|m| m.player == player)
        .map(|m| (m.id, m.params.clone()))
        .collect()
}

/// A player with no planet and no ship is dead from this year on, and
/// everybody else hears it; nothing is said of a player already dead.
#[test]
fn a_player_with_nothing_left_is_eliminated() {
    let mut state = a_game();
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert!(state.players[2].dead);
    assert!(!state.players[0].dead && !state.players[1].dead);
    for player in [0, 1] {
        let news = first_news(&state, player);
        assert_eq!(
            news.first(),
            Some(&(id::PLAYER_ELIMINATED, vec![2 | 0x30])),
            "player {player}: {news:?}"
        );
    }
    assert!(!state
        .messages
        .iter()
        .any(|m| m.player == 2 && m.id == id::PLAYER_ELIMINATED));
    assert!(!state.game_over, "two are still standing");

    // The year after, nothing more is said of them.
    generate_turn(&mut state, &mut Rng::randomize(2));
    assert!(!state.messages.iter().any(|m| m.id == id::PLAYER_ELIMINATED));
}

/// A win on the game's conditions ends the game and tells everyone: the
/// winner, the rest, and the dead, each their own way.
#[test]
fn a_win_is_declared_to_everybody() {
    let mut state = a_game();
    state.players[2].dead = true;
    // Playing for half the planets, needing that one condition, from year
    // 30: player 0 holds two of four, player 1 one.
    state.victory = [0; victory::COUNT];
    state.victory[victory::PLANET_CONTROL] = 0x80 | 6;
    state.victory[victory::MUST_MEET] = 1;
    state.galaxy_planets = 4;
    let mut third = Planet::unowned(3);
    third.owner = Some(0);
    third.position = Some(Point::new(1000, 1300));
    third.pop = 10_000;
    state.planets.push(third);
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert!(state.game_over, "{:?}", state.messages);
    assert_eq!(first_news(&state, 0).first(), Some(&(id::GAME_WON, vec![])));
    assert_eq!(
        first_news(&state, 1).first(),
        Some(&(id::GAME_WON_BY_OTHERS, vec![0b1]))
    );
    assert_eq!(
        first_news(&state, 2).first(),
        Some(&(id::GAME_OVER_DEAD, vec![]))
    );

    // The host file carries the flag, and a game read back knows it.
    let bytes = stars_core::save::host_file(&state).expect("writes");
    let file = stars_formats::StarsFile::decode(&bytes).expect("decodes");
    assert!(file.header.flag_game_over);
    let (again, _) = GameState::from_file(&file);
    assert!(again.game_over);
}

/// Two winners at once share the declaration, each told of the other.
#[test]
fn winners_together_are_told_of_each_other() {
    let mut state = a_game();
    state.players[2].dead = true;
    // A fifth of the planets each: both of them hold half.
    state.victory = [0; victory::COUNT];
    state.victory[victory::PLANET_CONTROL] = 0x80;
    state.victory[victory::MUST_MEET] = 1;
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(
        first_news(&state, 0).first(),
        Some(&(id::GAME_WON_SHARED, vec![0b10]))
    );
    assert_eq!(
        first_news(&state, 1).first(),
        Some(&(id::GAME_WON_SHARED, vec![0b01]))
    );
}

/// The last player standing is told they alone are left, and the others
/// that they are out — whatever the game was set up for.
#[test]
fn the_last_one_standing_rules_the_galaxy() {
    let mut state = a_game();
    state.victory = [0; victory::COUNT];
    state.planets[1].owner = None;
    state.planets[1].pop = 0;
    state.players[2].dead = true;
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert!(state.game_over);
    assert!(state.players[1].dead);
    assert_eq!(
        first_news(&state, 0).first(),
        Some(&(id::LAST_ONE_STANDING, vec![]))
    );
    assert_eq!(
        first_news(&state, 1).first(),
        Some(&(id::GAME_OVER_DEAD, vec![]))
    );
    assert_eq!(
        first_news(&state, 2).first(),
        Some(&(id::GAME_OVER_DEAD, vec![]))
    );
}
