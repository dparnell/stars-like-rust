//! What a player's turn file says of everybody else.
//!
//! The host writes each `.mN` from what that player's scanners reach —
//! `SetVisiblePlanFleet` (`1070:95bc`) — and from what the year's battles
//! showed them (`WriteBattles`, `1070:709c`): partial records of the other
//! players' planets and fleets, their designs in outline or, once fought or
//! revealed, in full, and the space objects on the map. See
//! `docs/formats/writing.md` and `docs/formulas/scanning.md`.

use stars_core::components::slot;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_core::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use stars_core::minefield::Minefield;
use stars_core::movement::Point;
use stars_core::planet::Planet;
use stars_core::race::{Prt, Race, RaceStat};
use stars_core::visibility::{view, Detail};
use stars_core::{generate_turn, save, GameState, Player, Rng};
use stars_formats::battle::{BattleRecord, BattleToken, Square};
use stars_formats::{DesignRecord, FleetRecord, PlanetRecord, PlayerRecord, StarsFile};

const FIRST_BASE: usize = stars_core::startup::FIRST_STARBASE_SLOT as usize;

/// A Scout with a Ferret Scanner: 185 light years, 50 penetrating.
fn scout() -> ShipDesign {
    ShipDesign {
        name: "Ferret Scout".to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: 0,
        slots: vec![
            DesignSlot {
                category: slot::ENGINE,
                item: 1,
                count: 1,
            },
            DesignSlot {
                category: slot::SCANNER,
                item: 7,
                count: 1,
            },
        ],
    }
}

/// A player's designs: the scout in slot 0, a Space Station in the first
/// starbase slot.
fn designs() -> Vec<ShipDesign> {
    let mut designs = vec![scout()];
    while designs.len() < FIRST_BASE {
        designs.push(ShipDesign {
            name: String::new(),
            picture: 0,
            stored_armor: 0,
            obsolete: false,
            designed: 0,
            built: 0,
            hull_id: -1,
            slots: Vec::new(),
        });
    }
    designs.push(ShipDesign {
        name: "Station".to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: 34,
        slots: Vec::new(),
    });
    designs
}

fn planet(id: i16, at: Point, owner: i16) -> Planet {
    let mut planet = Planet::unowned(id);
    planet.owner = Some(owner);
    planet.position = Some(at);
    planet.pop = 25_000;
    planet.surface_min = [100, 200, 300];
    planet.starbase = true;
    planet.starbase_design = Some(0);
    planet
}

fn fleet(id: u16, owner: i16, at: Point, orbiting: Option<u16>) -> Fleet {
    Fleet {
        name: None,
        repeat_orders: false,
        direction: None,
        id,
        owner,
        position: at,
        orbiting,
        stacks: vec![ShipStack {
            design: 0,
            count: 3,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo {
            minerals: [10, 0, 0],
            colonists: 0,
            fuel: 100,
        },
        battle_plan: 0,
        warp: None,
        waypoints: vec![Waypoint {
            position: at,
            target: orbiting,
            target_class: if orbiting.is_some() { 1 } else { 4 },
            warp: 0,
            task: 0,
            transport: None,
            task_data: Vec::new(),
        }],
    }
}

/// Two players: ours at (1000, 1000) with a scout at home, theirs at
/// (1030, 1000) with a scout in orbit — thirty light years off, inside the
/// Ferret's penetrating fifty.
fn a_game() -> GameState {
    let mut state = GameState::new(5);
    state.galaxy_size = 2;
    state.players = vec![Player::new(Race::humanoid()), Player::new(Race::humanoid())];
    state.players[0].name = "Ours".to_string();
    state.players[1].name = "Theirs".to_string();
    state.players[1].plural_name = "Theirses".to_string();
    state.designs = vec![designs(), designs()];
    state.planets = vec![
        planet(1, Point::new(1000, 1000), 0),
        planet(2, Point::new(1030, 1000), 1),
    ];
    state.fleets = vec![
        fleet(1, 0, Point::new(1000, 1000), Some(1)),
        fleet(1, 1, Point::new(1030, 1000), Some(2)),
    ];
    state
}

/// Decode the records of one type from a written file.
fn blocks(bytes: &[u8], type_id: u8) -> Vec<Vec<u8>> {
    StarsFile::decode(bytes)
        .expect("decodes")
        .blocks
        .iter()
        .filter(|b| b.type_id == type_id)
        .map(|b| b.data.clone())
        .collect()
}

/// A scanned neighbour: their planet, their fleet in orbit, the outline of
/// their designs and a short record of them go into our file.
#[test]
fn what_the_scanners_reach_goes_into_the_file() {
    let state = a_game();
    let seen = view(&state, 0);
    assert_eq!(seen.planet(2), Some(Detail::Some));
    assert_eq!(seen.fleet(1), Some(Detail::Some));

    let bytes = save::player_file(&state, 0).expect("writes");

    let players: Vec<PlayerRecord> = blocks(&bytes, 6)
        .iter()
        .map(|b| PlayerRecord::from_payload(b).expect("player"))
        .collect();
    assert_eq!(players.len(), 2);
    assert!(players[0].full_data);
    let theirs = &players[1];
    assert_eq!(theirs.player_number, 1);
    assert!(!theirs.full_data, "a short record of the other player");
    assert_eq!(theirs.singular_name, "Theirs");
    assert_eq!(theirs.plural_name, "Theirses");
    assert_eq!(theirs.ship_design_count, 1);
    assert_eq!(theirs.starbase_design_count, 1);
    assert_eq!(theirs.fleets, 1);

    let partial: Vec<PlanetRecord> = blocks(&bytes, 14)
        .iter()
        .map(|b| PlanetRecord::decode(b, 14).expect("planet"))
        .collect();
    assert_eq!(partial.len(), 1);
    let theirs = &partial[0];
    assert_eq!(theirs.id, 2);
    assert_eq!(theirs.owner, Some(1));
    assert_eq!(theirs.detail, 3);
    assert!(theirs.include);
    assert!(theirs.environment.is_some());
    assert!(
        theirs.pop_guess.is_some(),
        "the owner's population estimate"
    );
    assert!(theirs.surface_minerals.is_none(), "not the surface");
    assert_eq!(theirs.starbase.map(|s| s.design), Some(0));

    let fleets: Vec<FleetRecord> = blocks(&bytes, 17)
        .iter()
        .map(|b| FleetRecord::decode(b, 17).expect("fleet"))
        .collect();
    assert_eq!(fleets.len(), 1);
    let theirs = &fleets[0];
    assert_eq!(theirs.owner, 1);
    assert_eq!(theirs.detail, 3);
    assert_eq!(theirs.ships.len(), 1);
    assert_eq!(theirs.ships[0].count, 3);
    assert!(theirs.cargo.is_none(), "not the hold");
    // Three scouts and ten kilotons of ironium.
    let scout_mass = scout().mass().expect("a hull");
    assert_eq!(
        theirs.mass,
        Some(u32::try_from(scout_mass * 3 + 10).unwrap())
    );
    assert_eq!(theirs.direction(), None, "not moving");

    let designs: Vec<DesignRecord> = blocks(&bytes, 26)
        .iter()
        .map(|b| DesignRecord::from_payload(b).expect("design"))
        .collect();
    // Ours in full, then theirs in outline: hull, mass and name, no slots.
    let outline: Vec<&DesignRecord> = designs.iter().filter(|d| !d.full_design).collect();
    assert_eq!(outline.len(), 2, "{designs:?}");
    assert_eq!(outline[0].name, "Ferret Scout");
    assert!(!outline[0].starbase);
    assert_eq!(outline[0].mass, Some(u16::try_from(scout_mass).unwrap()));
    assert!(outline[0].slots.is_empty());
    assert_eq!(outline[1].name, "Station");
    assert!(outline[1].starbase);
    assert_eq!(designs.iter().filter(|d| d.full_design).count(), 2);
}

/// Out of range, nothing of theirs is written; our own file is as it was.
#[test]
fn out_of_range_nothing_of_theirs_is_written() {
    let mut state = a_game();
    state.planets[1].position = Some(Point::new(1300, 1000));
    state.fleets[1].position = Point::new(1300, 1000);
    let bytes = save::player_file(&state, 0).expect("writes");
    assert_eq!(blocks(&bytes, 6).len(), 1);
    assert!(blocks(&bytes, 14).is_empty());
    assert!(blocks(&bytes, 17).is_empty());
    assert_eq!(blocks(&bytes, 26).len(), 2);
}

/// A War Monger reads every design on their map in full (`1070:c41c`).
#[test]
fn a_war_monger_reads_the_designs_it_sees_in_full() {
    let mut state = a_game();
    state.players[0].race.attrs[RaceStat::MajorAdv as usize] = Prt::Wm as i16;
    let bytes = save::player_file(&state, 0).expect("writes");
    let designs: Vec<DesignRecord> = blocks(&bytes, 26)
        .iter()
        .map(|b| DesignRecord::from_payload(b).expect("design"))
        .collect();
    assert_eq!(designs.len(), 4);
    assert!(designs.iter().all(|d| d.full_design), "{designs:?}");
}

/// A battle shows every design that fought in full, and keeps the fleets
/// that survived on the map whether or not a scanner reaches them.
#[test]
fn a_battle_reveals_the_designs_that_fought() {
    let mut state = a_game();
    // Their fleet fought ours far from any scanner.
    state.fleets[1].position = Point::new(1300, 1000);
    state.fleets[1].orbiting = None;
    state.planets[1].position = Some(Point::new(1300, 1000));
    let token = |player: u8, class: u8, id: u16, design: u8| BattleToken {
        id,
        player,
        object_class: class,
        design,
        square: Square { x: 1, y: 4 },
        initiative_base: 0,
        initiative_min: 0,
        initiative_max: 0,
        target: 0,
        pct_cloak: 0,
        pct_jam: 0,
        pct_computer: 0,
        pct_capacitor: 0,
        pct_beam_defence: 0,
        mass: 10,
        shields: 0,
        ships: 3,
        damage: 0,
        tactics: 0,
        movement: 0,
        flags: 0,
    };
    state.battles.push(BattleRecord {
        id: 0,
        players: 2,
        player_mask: 0b11,
        planet: 2,
        position: (1300, 1000),
        tokens: vec![token(0, 2, 1, 0), token(1, 2, 1, 0), token(1, 1, 2, 16)],
        actions: Vec::new(),
        declared_len: 0,
    });
    let bytes = save::player_file(&state, 0).expect("writes");
    let designs: Vec<DesignRecord> = blocks(&bytes, 26)
        .iter()
        .map(|b| DesignRecord::from_payload(b).expect("design"))
        .collect();
    assert_eq!(designs.len(), 4);
    assert!(designs.iter().all(|d| d.full_design), "{designs:?}");
    let fleets = blocks(&bytes, 17);
    assert_eq!(fleets.len(), 1, "the survivor is on the map");
    let planets: Vec<PlanetRecord> = blocks(&bytes, 14)
        .iter()
        .map(|b| PlanetRecord::decode(b, 14).expect("planet"))
        .collect();
    assert_eq!(planets.len(), 1, "the planet they fought over");
    assert_eq!(planets[0].detail, 1, "by name alone");
}

/// A Space Demolition player's field reads the designs of what it hits
/// (`10b0:6174`): that year their file describes them in full, wherever
/// the fleet has got to.
#[test]
fn a_minefield_hit_reveals_the_fleet_to_a_space_demolition_owner() {
    // Their field lies across our scout's path, well out of any scanner's
    // reach of theirs; hitting it is a matter of chance, so the year is
    // tried on seed after seed until it happens.
    let mut state = a_game();
    for seed in 1.. {
        state = a_game();
        state.players[1].race.attrs[RaceStat::MajorAdv as usize] = Prt::Sd as i16;
        state.minefields.push(Minefield {
            id: 0,
            owner: 1,
            position: Point::new(1000, 1060),
            mines: 2_500,
            kind: 0,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        });
        state.planets[1].position = Some(Point::new(1600, 1000));
        state.fleets[1].position = Point::new(1600, 1000);
        let ours = &mut state.fleets[0];
        ours.cargo.fuel = 5_000;
        ours.warp = Some(9);
        ours.waypoints.push(Waypoint {
            position: Point::new(1000, 1400),
            target: None,
            target_class: 4,
            warp: 9,
            task: 0,
            transport: None,
            task_data: Vec::new(),
        });
        let report = generate_turn(&mut state, &mut Rng::from_seeds(seed, seed + 1));
        if !report.mine_hits.is_empty() {
            break;
        }
        assert!(seed < 50, "never hit the field");
    }
    assert!(
        state.revealed_designs.contains(&(1, 0, 0)),
        "{:?}",
        state.revealed_designs
    );

    let bytes = save::player_file(&state, 1).expect("writes");
    let designs: Vec<DesignRecord> = blocks(&bytes, 26)
        .iter()
        .map(|b| DesignRecord::from_payload(b).expect("design"))
        .collect();
    let ours: Vec<&DesignRecord> = designs
        .iter()
        .filter(|d| d.name == "Ferret Scout" && d.full_design)
        .collect();
    assert_eq!(ours.len(), 2, "theirs and, revealed, ours: {designs:?}");

    // Next year the field has nothing new to say.
    generate_turn(&mut state, &mut Rng::from_seeds(5, 6));
    assert!(state.revealed_designs.is_empty());
}

/// The year's end leaves its marks on the space objects: a field a scanner
/// reaches is detected for good and seen this year, and the seen-this-year
/// mark is cleared again at the start of the next.
#[test]
fn scanners_detect_minefields_and_wormholes_at_the_years_end() {
    let mut state = a_game();
    state.planets[1].position = Some(Point::new(1600, 1000));
    state.fleets[1].position = Point::new(1600, 1000);
    // Their field forty light years from our scout — inside its 185 but
    // outside its penetrating fifty only by the field's own radius: a
    // field is seen from inside, or within penetrating range, or within
    // normal range once known.
    state.minefields.push(Minefield {
        id: 0,
        owner: 1,
        position: Point::new(1000, 1100),
        mines: 100,
        kind: 0,
        detonating: false,
        detected_by: 0,
        visible_to: 0,
        turn: 0,
    });
    // Wormhole ends: one forty light years off, one a hundred.
    state.wormholes = vec![
        stars_core::wormhole::Wormhole {
            id: 1,
            position: Point::new(1040, 1000),
            stability: 3,
            years_still: 0,
            dest_known: false,
            include: false,
            detected_by: 0,
            traversed_by: 0,
            partner: 2,
            turn: 0,
        },
        stars_core::wormhole::Wormhole {
            id: 2,
            position: Point::new(1100, 1000),
            stability: 3,
            years_still: 0,
            dest_known: false,
            include: false,
            detected_by: 0,
            traversed_by: 0,
            partner: 1,
            turn: 0,
        },
    ];
    let seen = view(&state, 0);
    assert!(
        seen.minefields.is_empty(),
        "a hundred off, unknown: not yet"
    );
    assert_eq!(seen.wormholes.iter().copied().collect::<Vec<_>>(), vec![0]);

    // Once detected — say by hitting it — it is seen at the normal range.
    state.minefields[0].detected_by = 1;
    let seen = view(&state, 0);
    assert_eq!(seen.minefields.iter().copied().collect::<Vec<_>>(), vec![0]);
    state.minefields[0].detected_by = 0;

    // Within penetrating range it is detected outright.
    state.minefields[0].position = Point::new(1000, 1045);
    assert_eq!(
        view(&state, 0)
            .minefields
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![0]
    );

    generate_turn(&mut state, &mut Rng::from_seeds(1, 2));
    assert_eq!(state.minefields[0].detected_by & 1, 1, "detected for good");
    assert_eq!(state.minefields[0].visible_to & 1, 1, "and seen this year");
    let near = state
        .wormholes
        .iter()
        .find(|w| w.id == 1)
        .expect("near end");
    assert_eq!(near.detected_by & 1, 1);
    let far = state.wormholes.iter().find(|w| w.id == 2).expect("far end");
    assert_eq!(far.detected_by & 1, 0);

    // The file carries the field and the near end.
    let bytes = save::player_file(&state, 0).expect("writes");
    let things = blocks(&bytes, 43);
    assert_eq!(things.len(), 3, "count, field, wormhole: {things:?}");

    // Move the scout away: the field stays detected but is not seen now.
    state.fleets[0].position = Point::new(1800, 1800);
    state.fleets[0].orbiting = None;
    state.fleets[0].waypoints[0].position = Point::new(1800, 1800);
    state.fleets[0].waypoints[0].target = None;
    state.fleets[0].waypoints[0].target_class = 4;
    state.planets[0].scanner = None;
    generate_turn(&mut state, &mut Rng::from_seeds(1, 2));
    assert_eq!(state.minefields[0].detected_by & 1, 1);
    assert_eq!(
        state.minefields[0].visible_to & 1,
        0,
        "cleared at the year's start"
    );
}
