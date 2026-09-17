//! The Transport task, pass by pass.
//!
//! The `grTaskXfer` arm of `SatisfyOrders` (`10b0:686a`): four passes a
//! year, the odd ones unloading and the even ones loading, before and
//! after the fleets move; a task that cannot be finished holds the fleet
//! where it is. See `docs/formulas/waypoint-tasks.md`, *Transport*.

use stars_core::components::slot;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_core::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use stars_core::message::id;
use stars_core::movement::Point;
use stars_core::planet::Planet;
use stars_core::race::Race;
use stars_core::{generate_turn, GameState, Player, Rng};
use stars_formats::{task, ItemAction, TransportTask, XferAction};

const HOME: Point = Point { x: 1000, y: 1000 };
const AWAY: Point = Point { x: 1030, y: 1000 };

fn freighter() -> ShipDesign {
    ShipDesign {
        name: "Teamster".to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        // Medium Freighter: a 210 kT hold, a 450 mg tank.
        hull_id: 1,
        slots: vec![DesignSlot {
            category: slot::ENGINE,
            item: 3,
            count: 1,
        }],
    }
}

fn item(action: XferAction, quantity: u16) -> ItemAction {
    ItemAction { quantity, action }
}

fn nothing() -> ItemAction {
    item(XferAction::None, 0)
}

fn orders(items: [ItemAction; 5]) -> Option<TransportTask> {
    Some(TransportTask { items })
}

/// One player with a planet at home (1) and another (2) thirty light
/// years east, both theirs, and a freighter at home.
fn a_game() -> GameState {
    let mut state = GameState::new(11);
    state.players = vec![Player::new(Race::humanoid())];
    state.designs = vec![vec![freighter()]];
    let mut home = Planet::unowned(1);
    home.owner = Some(0);
    home.position = Some(HOME);
    home.pop = 1000;
    home.surface_min = [500, 400, 300];
    let mut away = Planet::unowned(2);
    away.owner = Some(0);
    away.position = Some(AWAY);
    away.pop = 100;
    away.surface_min = [0, 0, 0];
    state.planets = vec![home, away];
    state.fleets = vec![Fleet {
        name: None,
        repeat_orders: false,
        direction: None,
        id: 1,
        owner: 0,
        position: HOME,
        orbiting: Some(1),
        stacks: vec![ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo {
            minerals: [0, 0, 0],
            colonists: 0,
            fuel: 450,
        },
        battle_plan: 0,
        warp: None,
        waypoints: vec![Waypoint {
            position: HOME,
            target: Some(1),
            target_class: 1,
            warp: 0,
            task: 0,
            transport: None,
            task_data: Vec::new(),
        }],
    }];
    state
}

fn leg_to_away(warp: u8, job: u8, transport: Option<TransportTask>) -> Waypoint {
    Waypoint {
        position: AWAY,
        target: Some(2),
        target_class: 1,
        warp,
        task: job,
        transport,
        task_data: Vec::new(),
    }
}

/// A task set at the waypoint the fleet already stands at runs before
/// the fleet moves: it loads at home in pass 2 and flies off with the
/// cargo, and the task is consumed.
#[test]
fn a_task_at_the_current_waypoint_runs_before_moving() {
    let mut state = a_game();
    let fleet = &mut state.fleets[0];
    fleet.waypoints[0].task = task::TRANSPORT;
    fleet.waypoints[0].transport = orders([
        item(XferAction::LoadExact, 100),
        nothing(),
        item(XferAction::LoadAll, 0),
        nothing(),
        nothing(),
    ]);
    fleet.waypoints.push(leg_to_away(6, task::NONE, None));
    generate_turn(&mut state, &mut Rng::randomize(1));

    let fleet = &state.fleets[0];
    assert_eq!(
        fleet.cargo.minerals,
        [100, 0, 110],
        "100 asked, the rest germanium"
    );
    assert_eq!(fleet.position, AWAY, "moved after loading");
    assert_eq!(fleet.waypoints.len(), 1);
    assert_eq!(fleet.waypoints[0].task, task::NONE);
    assert_eq!(state.planets[0].surface_min, [400, 400, 190]);
}

/// A task on the waypoint the fleet arrives at runs after the move:
/// unloading in pass 3, loading in pass 4.
#[test]
fn a_task_on_arrival_unloads_then_loads() {
    let mut state = a_game();
    state.fleets[0].cargo.minerals = [50, 0, 0];
    state.planets[1].surface_min = [0, 80, 0];
    state.fleets[0].waypoints.push(leg_to_away(
        6,
        task::TRANSPORT,
        orders([
            item(XferAction::UnloadAll, 0),
            item(XferAction::LoadAll, 0),
            nothing(),
            nothing(),
            nothing(),
        ]),
    ));
    generate_turn(&mut state, &mut Rng::randomize(1));

    let fleet = &state.fleets[0];
    assert_eq!(fleet.position, AWAY);
    assert_eq!(fleet.cargo.minerals, [0, 80, 0]);
    assert_eq!(state.planets[1].surface_min, [50, 0, 0]);
    assert_eq!(fleet.waypoints[0].task, task::NONE);
    let ids: Vec<u16> = state
        .messages
        .iter()
        .filter(|m| m.id == id::HAS_UNLOADED || m.id == id::HAS_LOADED)
        .map(|m| m.id)
        .collect();
    assert_eq!(ids, vec![id::HAS_UNLOADED, id::HAS_LOADED]);
}

/// "Wait for" a percentage the planet cannot yet fill: the fleet keeps the
/// task, does not move, and takes what it can each year until the hold
/// is full.
#[test]
fn a_wait_holds_the_fleet_until_the_hold_is_full() {
    let mut state = a_game();
    state.planets[0].surface_min = [80, 0, 0];
    let fleet = &mut state.fleets[0];
    fleet.waypoints[0].task = task::TRANSPORT;
    fleet.waypoints[0].transport = orders([
        item(XferAction::WaitPercent, 100),
        nothing(),
        nothing(),
        nothing(),
        nothing(),
    ]);
    fleet.waypoints.push(leg_to_away(6, task::NONE, None));

    generate_turn(&mut state, &mut Rng::randomize(1));
    let fleet = &state.fleets[0];
    assert_eq!(fleet.position, HOME, "waiting");
    assert_eq!(fleet.waypoints[0].task, task::TRANSPORT, "the task stays");
    assert_eq!(fleet.cargo.minerals[0], 80, "took what there was");

    // Enough turns up: the hold fills to 210 and the fleet goes.
    state.planets[0].surface_min[0] = 500;
    generate_turn(&mut state, &mut Rng::randomize(2));
    let fleet = &state.fleets[0];
    assert_eq!(fleet.cargo.minerals[0], 210);
    assert_eq!(fleet.position, AWAY);
}

/// Loading from a planet that is not yours waits through the year and is
/// given up on the last pass with a message — unless a Robber Baron
/// Scanner is aboard, when it is theft.
#[test]
fn loading_from_somebody_elses_planet_is_refused() {
    let mut state = a_game();
    state.players.push(Player::new(Race::humanoid()));
    state.designs.push(Vec::new());
    state.planets[0].owner = Some(1);
    let fleet = &mut state.fleets[0];
    fleet.waypoints[0].task = task::TRANSPORT;
    fleet.waypoints[0].transport = orders([
        item(XferAction::LoadAll, 0),
        nothing(),
        nothing(),
        nothing(),
        nothing(),
    ]);
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.fleets[0].cargo.minerals, [0, 0, 0]);
    assert_eq!(state.fleets[0].waypoints[0].task, task::NONE, "given up");
    let told: Vec<_> = state
        .messages
        .iter()
        .filter(|m| m.id == id::LOAD_NOT_YOUR_PLANET)
        .collect();
    assert_eq!(told.len(), 1, "{:?}", state.messages);
    assert_eq!(told[0].params, vec![1, 0]);

    let mut state = a_game();
    state.players.push(Player::new(Race::humanoid()));
    state.designs.push(Vec::new());
    state.planets[0].owner = Some(1);
    state.designs[0][0].slots.push(DesignSlot {
        category: slot::SCANNER,
        item: stars_core::transport::ROBBER_BARON,
        count: 1,
    });
    let fleet = &mut state.fleets[0];
    fleet.waypoints[0].task = task::TRANSPORT;
    fleet.waypoints[0].transport = orders([
        item(XferAction::LoadExact, 100),
        nothing(),
        nothing(),
        nothing(),
        nothing(),
    ]);
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.fleets[0].cargo.minerals, [100, 0, 0], "robbed");
    assert_eq!(state.planets[0].surface_min[0], 400);
}

/// Colonists unloaded onto an empty planet die; the task is cancelled
/// and the player told to colonise first.
#[test]
fn colonists_cannot_be_unloaded_onto_an_empty_planet() {
    let mut state = a_game();
    state.planets[1].owner = None;
    state.planets[1].pop = 0;
    state.fleets[0].cargo.colonists = 100;
    state.fleets[0].waypoints.push(leg_to_away(
        6,
        task::TRANSPORT,
        orders([
            nothing(),
            nothing(),
            nothing(),
            item(XferAction::UnloadAll, 0),
            nothing(),
        ]),
    ));
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.fleets[0].cargo.colonists, 100, "kept aboard");
    assert_eq!(state.planets[1].owner, None);
    assert_eq!(state.fleets[0].waypoints[0].task, task::NONE);
    assert!(state
        .messages
        .iter()
        .any(|m| m.id == id::BEAM_DOWN_UNINHABITED));
}

/// "Set amount to" tops the hold up to the figure, or empties it down to
/// it, and never asks for what the planet has not got without saying so.
#[test]
fn set_amount_tops_up_or_empties_down() {
    let mut state = a_game();
    state.fleets[0].cargo.minerals = [30, 150, 0];
    state.planets[0].surface_min = [40, 0, 0];
    let fleet = &mut state.fleets[0];
    fleet.waypoints[0].task = task::TRANSPORT;
    fleet.waypoints[0].transport = orders([
        item(XferAction::SetAmount, 100),
        item(XferAction::SetAmount, 50),
        nothing(),
        nothing(),
        nothing(),
    ]);
    generate_turn(&mut state, &mut Rng::randomize(1));
    let fleet = &state.fleets[0];
    // Ironium: wanted 70 more, the planet had 40.
    assert_eq!(fleet.cargo.minerals, [70, 50, 0]);
    assert_eq!(state.planets[0].surface_min, [0, 100, 0]);
    assert!(state.messages.iter().any(|m| m.id == id::SET_AMOUNT_SHORT));
}

/// Dunnage: what is left over after the rest has loaded fills the hold.
#[test]
fn dunnage_fills_what_room_is_left() {
    let mut state = a_game();
    let fleet = &mut state.fleets[0];
    fleet.waypoints[0].task = task::TRANSPORT;
    fleet.waypoints[0].transport = orders([
        item(XferAction::LoadExact, 50),
        item(XferAction::LoadDunnage, 0),
        nothing(),
        nothing(),
        nothing(),
    ]);
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.fleets[0].cargo.minerals, [50, 160, 0]);
}
