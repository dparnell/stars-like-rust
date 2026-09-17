//! Seed-identical generation, measured against the original's own tutorial
//! universe.
//!
//! `fixtures/games/tutorial/tutorial.xy` is the galaxy `CreateTutorWorld`
//! (`1078:5e5e`) produces: game id `0x008cef49`, two players, 24 planets. Its
//! settings and its seed are both known, so it is an exact oracle for
//! generating the same universe from the same seed.

use std::path::{Path, PathBuf};

fn fixture(relative: &str) -> Option<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(relative);
    path.exists().then_some(path)
}

/// What the original's tutorial universe holds.
fn original() -> Option<stars_formats::xy::Universe> {
    let path = fixture("games/tutorial/tutorial.xy")?;
    let bytes = std::fs::read(path).ok()?;
    stars_formats::xy::Universe::decode(&bytes).ok()
}

#[test]
fn the_fixture_is_the_tutorial_world() {
    let Some(universe) = original() else {
        return;
    };
    let (config, seed) = stars_core::newgame::tutorial();
    assert_eq!(
        universe.game().expect("game info").id,
        config.id,
        "the fixture is the game CreateTutorWorld makes"
    );
    assert_eq!(universe.planets.len(), 24);
    assert_eq!(seed, 0x4996_02d2);
}

/// The whole point: the same seed and settings must give the same galaxy.
#[test]
fn the_same_seed_gives_the_same_galaxy() {
    let Some(universe) = original() else {
        panic!("the tutorial fixture must be present for this test");
    };
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    let made = stars_core::newgame::generate(&config, &mut rng).expect("generates");

    let want: Vec<(u32, u16)> = universe
        .planets_resolved()
        .iter()
        .map(|p| (p.x, p.y))
        .collect();
    let got: Vec<(u32, u16)> = made
        .universe
        .planets_resolved()
        .iter()
        .map(|p| (p.x, p.y))
        .collect();
    println!("want ({}): {:?}", want.len(), want);
    println!("got  ({}): {:?}", got.len(), got);
    assert_eq!(got, want, "planet positions, in file order");
}

/// The names too, not just the positions: the tutorial's pages send you to
/// planets by name — "send this scout off exploring ... the planet Prune" —
/// so a galaxy with the right coordinates and the wrong names would still
/// be the wrong galaxy.
#[test]
fn the_same_seed_gives_the_same_planet_names() {
    let Some(universe) = original() else {
        return;
    };
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    let made = stars_core::newgame::generate(&config, &mut rng).expect("generates");

    let want: Vec<Option<&str>> = universe.planets_resolved().iter().map(|p| p.name).collect();
    let got: Vec<Option<&str>> = made
        .universe
        .planets_resolved()
        .iter()
        .map(|p| p.name)
        .collect();
    println!("want: {want:?}");
    println!("got:  {got:?}");
    assert_eq!(got, want, "planet names, in id order");
}

/// The planets the tutorial's pages name are the planets it generates.
///
/// The pages send you to planets by id, and the ids only mean anything if
/// the galaxy is the original's.
#[test]
fn the_pages_planet_ids_name_the_right_worlds() {
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    let made = stars_core::newgame::generate(&config, &mut rng).expect("generates");
    let planets = made.universe.planets_resolved();

    for (id, name) in [
        // "Click the right mouse button on Stove Top" — the homeworld.
        (0x0d_usize, "Stove Top"),
        // "the mineral concentrations on Prune are fairly high. Let's send
        // Cotton Picker #6 there to mine the planet."
        (0x0c, "Prune"),
        // "Goto planet 90210."
        (0x10, "90210"),
        // "Hold down the shift key and select Alexander."
        (0x0f, "Alexander"),
    ] {
        assert_eq!(
            planets.get(id).and_then(|p| p.name),
            Some(name),
            "planet {id:#x}"
        );
    }
}

/// Why the other universes in the fixtures cannot be used the same way.
///
/// A `.xy` carries its settings but **not** its seed. `GenNewGameFromFile`
/// (`1078:4b0d`) calls `Randomize` only when a game-definition file supplies
/// one; an ordinary new game seeds from the clock, and that seed is written
/// nowhere. So a real game's universe cannot be regenerated even in
/// principle, and a mismatch against one says nothing about the generator.
///
/// The tutorial is the exception, and the reason it is a usable oracle:
/// `CreateTutorWorld` calls `Randomize` with a constant compiled into the
/// program.
#[test]
fn other_universes_carry_no_seed() {
    use stars_core::newgame::{Density, NewGame, NewPlayer, Size, StartDistance};

    let Some(path) = fixture("incoming/turn0/Game.xy") else {
        return;
    };
    let bytes = std::fs::read(path).expect("readable");
    let universe = stars_formats::xy::Universe::decode(&bytes).expect("a universe");
    let info = universe.game().expect("game info");

    let config = NewGame {
        name: info.name.clone(),
        id: info.id,
        size: Size::Small,
        density: Density::Normal,
        start_distance: StartDistance::Close,
        players: (0..info.players)
            .map(|_| NewPlayer::human(stars_core::Race::humanoid()))
            .collect(),
        ..NewGame::default()
    };
    let mut rng = stars_core::rng::Rng::randomize(info.id);
    let made = stars_core::newgame::generate(&config, &mut rng).expect("generates");

    // The right number of planets, because that comes from the settings.
    assert_eq!(
        made.universe.planets_resolved().len(),
        universe.planets_resolved().len()
    );
    // But a wrong seed gives a wholly different galaxy, not a nearly-right
    // one — which is what tells the two failure modes apart.
    let agreed = made
        .universe
        .planets_resolved()
        .iter()
        .zip(universe.planets_resolved().iter())
        .filter(|(a, b)| a.x == b.x && a.y == b.y)
        .count();
    assert!(agreed < 4, "{agreed} of 128 agreed by chance");
}

/// The tutorial's own victory conditions, as `CreateTutorWorld` sets them:
/// the highest score after a hundred years, and one condition enough — and
/// they are what the real universe file carries.
#[test]
fn the_tutorials_victory_conditions_are_the_originals() {
    use stars_core::newgame::{generate, tutorial};
    let (config, seed) = tutorial();
    let made = generate(&config, &mut stars_core::Rng::randomize(seed)).expect("generates");
    let mut expected = [0u8; stars_formats::victory::COUNT];
    expected[7] = 0x80;
    expected[8] = 0x81;
    assert_eq!(made.state.victory, expected);
    let info = made.universe.game().expect("game info");
    assert_eq!(info.victory_bytes(), expected);
    if let Some(bytes) = fixture("games/tutorial/tutorial.xy").and_then(|p| std::fs::read(p).ok()) {
        let real = stars_formats::Universe::decode(&bytes).expect("decodes");
        assert_eq!(
            real.game().expect("info").victory_bytes(),
            expected,
            "the fixture's own"
        );
    }
}

/// An ordinary new game starts with the New Game dialog's victory
/// conditions, the least years scaled by the universe's size.
#[test]
fn a_new_game_has_the_dialogs_victory_conditions() {
    use stars_core::newgame::{default_victory, generate, NewGame, Size};
    for (size, years) in [(Size::Tiny, 30), (Size::Small, 50), (Size::Huge, 110)] {
        let config = NewGame {
            size,
            ..NewGame::default()
        };
        let made = generate(&config, &mut stars_core::Rng::randomize(3)).expect("generates");
        assert_eq!(made.state.victory, default_victory(size));
        let info = made.universe.game().expect("game info");
        assert!(info.victory_active(stars_formats::victory::PLANET_CONTROL));
        assert_eq!(
            info.victory_value(stars_formats::victory::PLANET_CONTROL),
            60
        );
        assert_eq!(info.victory_value(stars_formats::victory::TECH_LEVEL), 22);
        assert_eq!(info.victory_value(stars_formats::victory::TECH_FIELDS), 4);
        assert!(!info.victory_active(stars_formats::victory::SCORE));
        assert_eq!(info.victory_value(stars_formats::victory::SCORE), 11_000);
        assert_eq!(
            info.victory_value(stars_formats::victory::HIGH_SCORE_AT),
            100
        );
        assert_eq!(info.victory_value(stars_formats::victory::MUST_MEET), 1);
        assert_eq!(
            info.victory_value(stars_formats::victory::LEAST_YEARS),
            years
        );
    }
}
