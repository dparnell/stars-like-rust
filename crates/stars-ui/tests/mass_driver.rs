//! The planet pane's **Mass Driver** and **Destination** rows, with the
//! Set Dest button and the warp gauge beside them.
//!
//! `DrawPlanetStarbase` (`1048:22cc`) over `IWarpMAFromLppl` (`1048:7b10`)
//! and `DrawMassWarpGauge` (`1048:2afa`); see `docs/ui/planet-pane.md`.

use stars_core::design::DesignSlot;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::production::mass_driver;
use stars_core::{opponents, Race};
use stars_ui::app::Risk;
use stars_ui::App;

/// The orbital-special category, and the table index of `Mass Driver 5`.
const SPECIAL_SB: u16 = stars_core::components::slot::SPECIAL_SB;
const DRIVER_5: u8 = 7;

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "drivers".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    // Select the player's home world, which is what the pane shows.
    let home = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|planet| planet.homeworld && planet.owner == Some(0))
        .expect("a home world")
        .id;
    app.selection.planet = Some(home);
    app
}

/// Fit the player's starbase with these orbital specials, one to a slot.
fn fit(app: &mut App, items: &[u8]) {
    let slot = usize::from(stars_core::startup::FIRST_STARBASE_SLOT);
    let game = app.game.as_mut().expect("a game");
    let design = game.designs[0].get_mut(slot).expect("a starbase design");
    design
        .slots
        .retain(|fitted| fitted.category & SPECIAL_SB == 0);
    for item in items {
        design.slots.push(DesignSlot {
            category: SPECIAL_SB,
            item: *item,
            count: 1,
        });
    }
}

/// The best driver fitted wins, and the stargates that share the table are
/// not drivers at all.
#[test]
fn the_best_driver_fitted_is_the_one_that_counts() {
    let mut app = a_game();
    let home = app.selection.planet.expect("a selection");
    let of = |app: &App| {
        let game = app.game.as_ref().expect("a game");
        let planet = game
            .planets
            .iter()
            .find(|p| p.id == home)
            .expect("the home world");
        mass_driver(planet, &game.designs[0])
    };

    // A new game's starbase has no driver.
    assert_eq!(of(&app).warp, 0);

    // The first seven orbital specials are stargates, and none of them flings.
    fit(&mut app, &[0, 6]);
    assert_eq!(of(&app).warp, 0, "stargates are not drivers");

    // `Mass Driver 5` is index 7 and flings at warp 5; `Ultra Driver 13` is
    // index 15 and flings at 13 — the rating is the warp.
    fit(&mut app, &[DRIVER_5]);
    assert_eq!(of(&app).warp, 5);
    fit(&mut app, &[15]);
    assert_eq!(of(&app).warp, 13);
}

/// Two drivers of the same rating are a **pair**, which the row marks with a
/// `+`. A better one takes over and cancels it, and two in the *same slot*
/// are not a pair at all — `IWarpMAFromLppl` walks slots, not counts.
#[test]
fn a_second_driver_of_the_same_rating_is_a_pair() {
    let mut app = a_game();
    let of = |app: &App| {
        let game = app.game.as_ref().expect("a game");
        let planet = game
            .planets
            .iter()
            .find(|p| Some(p.id) == app.selection.planet);
        mass_driver(planet.expect("the home world"), &game.designs[0])
    };

    fit(&mut app, &[DRIVER_5]);
    assert!(!of(&app).paired, "one is not a pair");

    fit(&mut app, &[DRIVER_5, DRIVER_5]);
    let driver = of(&app);
    assert_eq!((driver.warp, driver.paired), (5, true));

    // Three of them: still a pair, and still warp 5.
    fit(&mut app, &[DRIVER_5, DRIVER_5, DRIVER_5]);
    assert!(of(&app).paired);

    // A better one takes over and clears the flag, whichever way round the
    // two ratings come.
    fit(&mut app, &[DRIVER_5, DRIVER_5, 9]);
    let driver = of(&app);
    assert_eq!((driver.warp, driver.paired), (7, false), "5, 5 then 7");
    fit(&mut app, &[DRIVER_5, 9, 9]);
    let driver = of(&app);
    assert_eq!((driver.warp, driver.paired), (7, true), "5, 7 then 7");

    // Two in one slot are one slot, so they are not a pair.
    {
        let slot = usize::from(stars_core::startup::FIRST_STARBASE_SLOT);
        let game = app.game.as_mut().expect("a game");
        let design = game.designs[0].get_mut(slot).expect("a starbase design");
        design
            .slots
            .retain(|fitted| fitted.category & SPECIAL_SB == 0);
        design.slots.push(DesignSlot {
            category: SPECIAL_SB,
            item: DRIVER_5,
            count: 2,
        });
    }
    let driver = of(&app);
    assert_eq!(
        (driver.warp, driver.paired),
        (5, false),
        "the routine never looks past the slot being occupied"
    );
}

/// What the two rows say.
#[test]
fn the_rows_read_as_the_original_writes_them() {
    let mut app = a_game();
    let rows = |app: &App| app.planet_starbase_tile().1;

    let labels: Vec<String> = rows(&app).iter().map(|(l, _)| l.clone()).collect();
    assert_eq!(
        labels,
        vec![
            "Dock Capacity",
            "Armor",
            "Shields",
            "Damage",
            "Mass Driver",
            "Destination"
        ]
    );

    // No driver and no fling: both rows say `none`.
    assert_eq!(rows(&app)[4].1, "none");
    assert_eq!(rows(&app)[5].1, "none");

    // `Warp: %d`, with the `+` a pair adds.
    fit(&mut app, &[DRIVER_5]);
    assert_eq!(rows(&app)[4].1, "Warp: 5");
    fit(&mut app, &[DRIVER_5, DRIVER_5]);
    assert_eq!(rows(&app)[4].1, "Warp: 5+");

    // The destination is named, not numbered.
    let target = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .map(|planet| planet.id)
        .find(|id| Some(*id) != app.selection.planet)
        .expect("another planet");
    app.aim_mass_driver(target);
    assert_eq!(rows(&app)[5].1, app.planet_name(target));

    // A planet with no starbase has no rows at all, and the tile says so.
    if let Some(game) = app.game.as_mut() {
        if let Some(planet) = game
            .planets
            .iter_mut()
            .find(|p| Some(p.id) == app.selection.planet)
        {
            planet.starbase = false;
        }
    }
    assert_eq!(app.planet_starbase_tile().0, "< no starbase >");
    assert!(rows(&app).is_empty());
    assert!(app.planet_mass_driver().is_none());
}

/// The Damage row: a percentage out of a figure held in 500ths of the base's
/// armour, floored at one percent, and `none` when the base is whole.
#[test]
fn the_damage_row_is_the_stored_figure_over_five() {
    use stars_ui::popup::starbase_damage_pct;

    // Whole is `none`, not `0%`.
    assert_eq!(starbase_damage_pct(0), None);
    // The stored figure is rounded up to five before the divide, so the
    // smallest damage there can be still reads one percent. Real games hold
    // twos, threes and fours.
    assert_eq!(starbase_damage_pct(1), Some(1));
    assert_eq!(starbase_damage_pct(2), Some(1));
    assert_eq!(starbase_damage_pct(4), Some(1));
    assert_eq!(starbase_damage_pct(5), Some(1));
    // And thereafter it is simply the fifth.
    assert_eq!(starbase_damage_pct(9), Some(1), "it truncates");
    assert_eq!(starbase_damage_pct(10), Some(2));
    assert_eq!(starbase_damage_pct(250), Some(50));
    assert_eq!(starbase_damage_pct(500), Some(100));
    // The largest the fixtures hold is 473, which is 94%.
    assert_eq!(starbase_damage_pct(473), Some(94));

    // What the row reads, on a real tile.
    let mut app = a_game();
    let set = |app: &mut App, damage: u16| {
        let selected = app.selection.planet;
        if let Some(game) = app.game.as_mut() {
            if let Some(planet) = game.planets.iter_mut().find(|p| Some(p.id) == selected) {
                planet.starbase_damage = damage;
            }
        }
        app.planet_starbase_tile().1[3].1.clone()
    };
    assert_eq!(set(&mut app, 0), "none", "a new game's base is whole");
    assert_eq!(set(&mut app, 3), "1%");
    assert_eq!(set(&mut app, 138), "27%");
    assert_eq!(set(&mut app, 500), "100%");
}

/// The gauge: one segment of the raw speed against the rating less one, and
/// three colours saying how far over the rating that speed is.
#[test]
fn the_gauge_runs_to_three_warps_over_the_rating() {
    let mut app = a_game();
    fit(&mut app, &[DRIVER_5]);
    let speed = |app: &mut App, raw: u8| {
        if let Some(game) = app.game.as_mut() {
            let selected = app.selection.planet;
            if let Some(planet) = game.planets.iter_mut().find(|p| Some(p.id) == selected) {
                planet.fling_warp = raw;
            }
        }
        app.planet_mass_driver().expect("a starbase")
    };

    // A file holding nothing is warp 5, which is as slow as a packet flies.
    let driver = speed(&mut app, 0);
    assert_eq!(driver.speed, 5);
    assert_eq!(driver.label, "Warp 5");
    assert_eq!(
        (driver.fill, driver.total),
        (1, 4),
        "raw speed of rating - 1"
    );
    assert_eq!(driver.most, 8, "three over a warp-5 driver");
    assert_eq!(driver.risk, Risk::Safe);

    // One and two over is the yellow band; three over is red, and is as far
    // as the gauge goes.
    assert_eq!(speed(&mut app, 2).risk, Risk::Risky, "warp 6");
    assert_eq!(speed(&mut app, 3).risk, Risk::Risky, "warp 7");
    assert_eq!(speed(&mut app, 4).risk, Risk::Dangerous, "warp 8");

    // A pair is safe one warp further out, so the same warp 6 is purple.
    fit(&mut app, &[DRIVER_5, DRIVER_5]);
    assert_eq!(speed(&mut app, 2).risk, Risk::Safe);
    assert_eq!(speed(&mut app, 3).risk, Risk::Risky);

    // With no driver there is no gauge to draw, though the row and the
    // button are still there.
    fit(&mut app, &[]);
    let driver = app.planet_mass_driver().expect("a starbase");
    assert!(!driver.present());
}

/// Set Dest arms one click: the planet under it becomes the destination, the
/// selected planet itself clears it, and the button comes back up either way.
#[test]
fn set_dest_arms_a_single_click() {
    let mut app = a_game();
    fit(&mut app, &[DRIVER_5]);
    let home = app.selection.planet.expect("a selection");
    let target = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .map(|planet| planet.id)
        .find(|id| *id != home)
        .expect("another planet");
    let aimed = |app: &App| {
        app.game
            .as_ref()
            .expect("a game")
            .planets
            .iter()
            .find(|p| p.id == home)
            .expect("the home world")
            .fling_dest
    };

    app.set_packet_dest = true;
    assert!(app.aim_mass_driver(target));
    assert_eq!(aimed(&app), Some(target));
    assert!(!app.set_packet_dest, "one shot, not a mode");

    // Clicking the planet itself calls the fling off.
    app.set_packet_dest = true;
    assert!(app.aim_mass_driver(home));
    assert_eq!(aimed(&app), None);
    assert!(!app.set_packet_dest);
}
