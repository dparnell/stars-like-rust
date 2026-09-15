//! The Selection Summary pane — `DrawMineSurvey` (`1028:065a`).
//!
//! See `docs/ui/mine-survey-pane.md`.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::survey;
use stars_ui::{App, ScanObject, ScanThing, Selection};

fn a_game(mine: Race) -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "summary".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(mine),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.selection = Selection::default();
    app
}

fn home(app: &App, player: i16) -> i16 {
    app.game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.homeworld && p.owner == Some(player))
        .expect("a home world")
        .id
}

/// The six rows share whatever is left once four lines of text are taken off
/// the top, and each is forced to an even height.
#[test]
fn the_six_rows_split_what_the_text_leaves() {
    // 200 pixels, a 10-pixel line: 200 - 40 - 2 = 158, / 6 = 26, + 1 = 27,
    // and then rounded down to even.
    assert_eq!(survey::row_height(200.0, 10.0), 26.0);
    // Never smaller than two, however little room there is.
    assert_eq!(survey::row_height(10.0, 10.0), 2.0);
}

/// The pane goes narrow when four of its widest label would not fit across it.
#[test]
fn the_pane_goes_narrow_when_the_labels_would_not_fit() {
    assert!(!survey::narrow(40.0, 200.0));
    assert!(survey::narrow(50.0, 200.0), "4 x 50 is exactly 200");
    assert!(survey::narrow(60.0, 200.0));
}

/// The scale's step is rounded up to a round number, and how round depends on
/// how big the scale is.
#[test]
fn the_mineral_scale_rounds_its_step() {
    // The shipped scale is 5000, which rounds to multiples of 250.
    assert_eq!(survey::scale_step(5000, 4), 1250);
    assert_eq!(survey::scale_step(5000, 7), 750, "714 rounds up to 750");
    assert_eq!(survey::scale_ticks(5000, 1250), 4);
    // A small scale rounds to tens, a huge one to thousands.
    assert_eq!(survey::scale_step(400, 7), 60, "57 rounds up to 60");
    assert_eq!(survey::scale_step(30000, 7), 5000);
}

/// The pane's labels come in pairs and switch together.
#[test]
fn the_labels_are_the_games_own_pairs() {
    assert_eq!(survey::VALUE_LABEL, ("Value: ", "Val: "));
    assert_eq!(survey::POPULATION_LABEL, ("Population:  ", "Pop:  "));
    assert_eq!(survey::ENV_LABELS[1], ("Temperature", "Temp"));
    // A narrow pane does not have a shorter word for a mineral: it draws the
    // first four characters of the long one.
    assert_eq!(
        &survey::MINERAL_LABELS[2][..survey::MINERAL_LABEL_NARROW],
        "Germ"
    );
}

/// A narrow pane switches every label at once, and the value row drops the
/// terraformed optimum it shows when there is room.
#[test]
fn a_narrow_pane_uses_the_short_labels() {
    let mut app = a_game(Race::humanoid());
    let id = home(&app, 0);
    app.select_object(ScanObject::Planet(id));

    let wide = app.survey_planet_rows(false);
    assert_eq!(wide[0].0, "Value: ");
    assert_eq!(wide[1].0, "Population:  ");
    assert!(wide.iter().any(|(_, v)| v == "Report is current"));

    let narrow = app.survey_planet_rows(true);
    assert_eq!(narrow[0].0, "Val: ");
    assert_eq!(narrow[1].0, "Pop:  ");
    assert!(narrow.iter().any(|(_, v)| v == "Current"));
    // The optimum in brackets is a wide-pane extra.
    assert!(!narrow[0].1.contains('('));
}

/// The mineral bars carry both figures: what is on the surface now and what it
/// will be once this year's mining is counted.
#[test]
fn a_mineral_bar_carries_the_mining_estimate() {
    let mut app = a_game(Race::humanoid());
    let id = home(&app, 0);
    app.select_object(ScanObject::Planet(id));
    let bars = app.survey_minerals();
    assert_eq!(bars.len(), 3);
    for bar in &bars {
        assert!(
            bar.sum >= bar.at,
            "the sum is never shorter than the surface stock"
        );
    }
    // A home world has mines, so at least one mineral gains something.
    assert!(
        bars.iter().any(|bar| bar.sum > bar.at),
        "a home world's mines add to the bars"
    );
}

/// A **Claim Adjuster** is shown the *owner's* habitable band on somebody
/// else's planet, because that is the band the planet would be terraformed
/// towards.
#[test]
fn a_claim_adjuster_sees_the_owners_band() {
    let mut ca = Race::humanoid();
    ca.attrs[stars_core::race::RaceStat::MajorAdv as usize] = stars_core::race::Prt::Ca as i16;
    let mut app = a_game(ca);
    let theirs = home(&app, 1);
    app.select_object(ScanObject::Planet(theirs));
    let bars = app.survey_environment();
    assert_eq!(bars.len(), 3);

    let (mine, theirs_race) = {
        let game = app.game.as_ref().expect("a game");
        (game.players[0].race.clone(), game.players[1].race.clone())
    };
    let shown: Vec<(i32, i32)> = bars.iter().map(|b| (b.low, b.high)).collect();
    let owner: Vec<(i32, i32)> = (0..3)
        .map(|i| {
            (
                i32::from(theirs_race.env_min[i]),
                i32::from(theirs_race.env_max[i]),
            )
        })
        .collect();
    assert_eq!(shown, owner, "the owner's band, not this player's");

    // And an ordinary race is shown its own.
    let mut app = a_game(Race::humanoid());
    let theirs = home(&app, 1);
    app.select_object(ScanObject::Planet(theirs));
    let bars = app.survey_environment();
    let shown: Vec<(i32, i32)> = bars.iter().map(|b| (b.low, b.high)).collect();
    let ours: Vec<(i32, i32)> = (0..3)
        .map(|i| (i32::from(mine.env_min[i]), i32::from(mine.env_max[i])))
        .collect();
    assert_eq!(shown, ours);
}

/// The fleet half. A fleet the player commands gets everything; somebody
/// else's gets its ship count, its mass and — only when it was scanned — its
/// speed.
#[test]
fn a_fleet_shows_only_what_is_known_about_it() {
    use stars_core::fleet::{Fleet, ShipStack, Waypoint};
    use stars_core::movement::Point;

    let mut app = a_game(Race::humanoid());
    let mine = Fleet {
        id: 0,
        owner: 0,
        position: Point::new(100, 100),
        orbiting: None,
        stacks: vec![ShipStack {
            design: 0,
            count: 2,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: stars_core::fleet::Cargo::default(),
        battle_plan: 0,
        warp: None,
        waypoints: vec![Waypoint {
            position: Point::new(100, 100),
            target: None,
            target_class: 4,
            warp: 0,
            task: 0,
            transport: None,
            task_data: Vec::new(),
        }],
        name: None,
        repeat_orders: false,
        direction: None,
    };
    let mut theirs = mine.clone();
    theirs.owner = 1;
    theirs.warp = Some(7);
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![mine, theirs];
    }

    app.select_object(ScanObject::Fleet(0));
    let ours = app.survey_fleet(false);
    assert_eq!(ours.ships, "Ship Count: 2");
    assert!(ours.mass.starts_with("Fleet Mass: "));
    assert!(ours.fuel.is_some() && ours.cargo.is_some());
    assert_eq!(ours.orders[0], "Next Waypoint: (none)");
    assert_eq!(
        ours.orders.last().expect("a speed"),
        "Warp Speed: (stopped)"
    );

    app.selection = Selection::default();
    app.select_object(ScanObject::Fleet(1));
    let hers = app.survey_fleet(false);
    assert_eq!(hers.ships, "Ship Count: 2");
    assert!(
        hers.fuel.is_none() && hers.cargo.is_none(),
        "somebody else's insides are not on file"
    );
    assert_eq!(hers.orders, vec!["Warp Speed: 7".to_string()]);
    assert!(hers.sweeping.is_none());
}

/// The narrow form shortens every fleet label too, and a leg through a
/// stargate says so instead of naming a warp.
#[test]
fn the_fleet_labels_go_narrow_and_a_gate_leg_says_so() {
    use stars_core::fleet::{Fleet, ShipStack, Waypoint};
    use stars_core::movement::Point;

    let mut app = a_game(Race::humanoid());
    let leg = |warp: u8, at: Point| Waypoint {
        position: at,
        target: None,
        target_class: 4,
        warp,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    };
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![Fleet {
            id: 0,
            owner: 0,
            position: Point::new(100, 100),
            orbiting: None,
            stacks: vec![ShipStack {
                design: 0,
                count: 1,
                damaged_pct: 0,
                damage_pct: 0,
            }],
            cargo: stars_core::fleet::Cargo::default(),
            battle_plan: 0,
            warp: None,
            waypoints: vec![
                leg(0, Point::new(100, 100)),
                leg(survey::STARGATE_WARP, Point::new(300, 300)),
            ],
            name: None,
            repeat_orders: false,
            direction: None,
        }];
    }
    app.select_object(ScanObject::Fleet(0));

    let wide = app.survey_fleet(false);
    assert!(wide.mass.starts_with("Fleet Mass: "));
    // A waypoint on nothing is named `Space (x, y)`, not a bare pair.
    assert_eq!(wide.orders[0], "Next Waypoint: Space (300, 300)");
    assert_eq!(wide.orders[1], "Waypoint Task: (no task here)");
    assert_eq!(wide.orders[2], "Use Stargate");

    let narrow = app.survey_fleet(true);
    assert!(narrow.mass.starts_with("Mass: "));
    assert!(narrow.orders[0].starts_with("WP: "));
    assert!(narrow.orders[1].starts_with("Task: "));
    // `Use Stargate` has no short form of its own.
    assert_eq!(narrow.orders[2], "Use Stargate");
}

/// Salvage is a mineral packet at warp zero (`DrawMineSurvey` reads the
/// `iWarp` bits of the packet's first word, `1028:065a`), and the pane
/// treats it as its own thing: a picture of its own, and neither a speed
/// nor a destination.
#[test]
fn salvage_is_a_packet_with_nowhere_to_go() {
    use stars_core::movement::Point;
    use stars_core::packet::Packet;

    let mut app = a_game(Race::humanoid());
    let packet = |id: u16, warp: u8| Packet {
        id,
        owner: 0,
        position: Point::new(40, 40),
        target: 7,
        warp,
        minerals: [10, 20, 30],
        decay_rate: 0,
        moved: false,
        include: true,
        turn: 0,
    };
    if let Some(game) = app.game.as_mut() {
        game.packets = vec![packet(1, 6), packet(2, 0)];
    }

    app.select_object(ScanObject::Thing(ScanThing::Packet(0)));
    let flying = app.survey_thing();
    assert_eq!(flying.picture, 4);
    assert_eq!(flying.rows.len(), 2, "a speed and a destination");
    assert_eq!(flying.table.len(), 3);

    app.selection = Selection::default();
    app.select_object(ScanObject::Thing(ScanThing::Packet(1)));
    let salvage = app.survey_thing();
    assert_eq!(salvage.picture, 3, "salvage has a picture of its own");
    assert!(
        salvage.rows.is_empty(),
        "salvage is going nowhere, so neither row is drawn"
    );
    assert_eq!(salvage.table.len(), 3, "but it still lists what it holds");
}

/// The three minefield kinds are the first three pictures in `hdibThings`,
/// in the same order as their names.
#[test]
fn a_minefield_picture_follows_its_kind() {
    use stars_core::minefield::Minefield;
    use stars_core::movement::Point;

    let mut app = a_game(Race::humanoid());
    let field = |id: u16, kind: u8| Minefield {
        id,
        owner: 0,
        position: Point::new(60, 60),
        mines: 400,
        kind,
        detonating: false,
        detected_by: 0,
        visible_to: 0,
        turn: 0,
    };
    if let Some(game) = app.game.as_mut() {
        game.minefields = vec![field(1, 0), field(2, 1), field(3, 2)];
    }
    for (index, expected) in [(0usize, 0u8), (1, 1), (2, 2)] {
        app.selection = Selection::default();
        app.select_object(ScanObject::Thing(ScanThing::Minefield(index)));
        let summary = app.survey_thing();
        assert_eq!(summary.picture, expected);
        assert!(summary.emblem, "a field has an owner");
    }
}

/// The seven stability words are the game's own, in `PctWormholeMoves` order.
#[test]
fn the_stability_words_are_the_games_own() {
    assert_eq!(survey::STABILITY[0], "Rock Solid");
    assert_eq!(survey::STABILITY[6], "Extremely Volatile");
    assert_eq!(survey::WORMHOLE_LABELS[1], "Destination:");
}
