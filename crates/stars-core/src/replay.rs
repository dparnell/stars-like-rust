//! Replaying a player's submitted order log against the host's state.
//!
//! A `.xN` file is not a list of intentions. The player's client has already
//! carried the orders out against its own copy of the game; the log records
//! what it did so the host can do the same and stay in step. Replaying one is
//! therefore a **state mutation**, not a queue of things to do during the
//! year — with one exception: cargo transfers belong at `DoOrders(0)`, the
//! first step of turn generation, because a transfer applied there feeds the
//! same year's growth. Those are collected into a [`TurnOrders`] rather than
//! applied here.
//!
//! ## What a host must not take on trust
//!
//! An order log arrives from the player's machine and can say anything. Every
//! operation replayed here is checked against the player it came from: a
//! waypoint must name one of their fleets, a queue or routing change one of
//! their planets, a design change their own design list. A cargo transfer may
//! legitimately involve someone else's planet — that is how colonists are
//! landed and invasions launched — so only the fleet end is checked. Anything
//! that fails is counted in [`ReplayReport::rejected`] and dropped.
//!
//! Source: the log-replay arms of `log.c`, and the file format in
//! `docs/formats/orders-x.md`.

use stars_formats::{
    object_owner, CargoTransfer, CargoTransferRecord, FleetMerge, FleetName, FleetOrderDelete,
    FleetOrderTask, GrobjClass, LogRecord, LogRecordType, OrderLog, PlanetRoutingOrder,
    ResearchOrder, ShipDesignChange, WaypointOrder,
};

use crate::fleet::Waypoint;
use crate::movement::Point;
use crate::orders::CARGO_KINDS;
use crate::production::QueueItem;
use crate::research::NextField;
use crate::turn::TurnOrders;
use crate::GameState;

/// What replaying one player's log did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayReport {
    /// Which player's log this was.
    pub player: usize,
    /// Cargo transfers collected for `DoOrders(0)`.
    pub cargo: usize,
    /// Waypoint orders inserted, updated or deleted.
    pub waypoints: usize,
    /// Production queues replaced.
    pub queues: usize,
    /// Research settings changed.
    pub research: usize,
    /// Ship designs created, changed or deleted.
    pub designs: usize,
    /// Planet routing/no-research settings changed.
    pub routing: usize,
    /// Ships moved between two of the player's fleets, which is what a split
    /// and a manual regroup both come down to.
    pub ship_moves: usize,
    /// Fleets merged into another.
    pub merges: usize,
    /// Fleets renamed.
    pub renames: usize,
    /// Fleet settings changed: the battle plan, the repeat-orders flag, or a
    /// waypoint's task on its own.
    pub fleet_settings: usize,
    /// Player-relations tables replaced.
    pub relations: usize,
    /// Operations dropped because they named something the player does not own,
    /// or an object this state does not hold.
    pub rejected: usize,
    /// Operation kinds this replay does not implement, each named once.
    pub unsupported: Vec<LogRecordType>,
}

impl ReplayReport {
    /// How many operations were replayed.
    #[must_use]
    pub fn applied(&self) -> usize {
        self.cargo
            + self.waypoints
            + self.queues
            + self.research
            + self.designs
            + self.routing
            + self.ship_moves
            + self.merges
            + self.renames
            + self.fleet_settings
            + self.relations
    }
}

/// Replay one player's order log.
///
/// Cargo transfers are appended to `orders` for the turn generator to apply at
/// `DoOrders(0)`; everything else is applied to `state` immediately, which is
/// where the client had already applied it.
pub fn replay(
    state: &mut GameState,
    player: usize,
    log: &OrderLog,
    orders: &mut TurnOrders,
) -> ReplayReport {
    let mut report = ReplayReport {
        player,
        ..ReplayReport::default()
    };
    for record in &log.records {
        apply(state, player, record, orders, &mut report);
    }
    report
}

/// Replay several players' logs, collecting the cargo transfers they contain.
///
/// The logs are replayed in the order given, which is the order the host
/// receives them in; a transfer's effect can depend on what an earlier one did
/// to the same planet.
pub fn replay_logs(
    state: &mut GameState,
    logs: &[(usize, OrderLog)],
) -> (TurnOrders, Vec<ReplayReport>) {
    let mut orders = TurnOrders::default();
    let reports = logs
        .iter()
        .map(|(player, log)| replay(state, *player, log, &mut orders))
        .collect();
    (orders, reports)
}

/// Replay one operation.
fn apply(
    state: &mut GameState,
    player: usize,
    record: &LogRecord,
    orders: &mut TurnOrders,
    report: &mut ReplayReport,
) {
    match record.record_type {
        LogRecordType::CargoXfer8 | LogRecordType::CargoXfer16 | LogRecordType::CargoXfer32 => {
            let Some(transfer) = record.as_cargo_transfer() else {
                report.rejected += 1;
                return;
            };
            match cargo_record(player, &transfer) {
                Some(converted) => {
                    orders.cargo.push(converted);
                    report.cargo += 1;
                }
                None => report.rejected += 1,
            }
        }
        // The fleet-to-fleet form moves **ships**, not cargo: its wider mask
        // names design slots. See `stars_formats::CargoTransfer`.
        LogRecordType::FleetCargoXfer => {
            let Some(transfer) = record.as_cargo_transfer() else {
                report.rejected += 1;
                return;
            };
            if move_ships(state, player, &transfer) {
                report.ship_moves += 1;
            } else {
                report.rejected += 1;
            }
        }
        // A split says only which fleet is being split; the ship transfer
        // that follows names the new fleet and what goes into it, and
        // `move_ships` creates the fleet when it does not exist yet.
        LogRecordType::FleetSplit => {
            let Some(split) = record.as_fleet_split() else {
                report.rejected += 1;
                return;
            };
            if find_fleet(state, player, split.fleet_id).is_none() {
                report.rejected += 1;
            }
        }
        LogRecordType::FleetMerge => {
            let Some(merge) = record.as_fleet_merge() else {
                report.rejected += 1;
                return;
            };
            if merge_fleets(state, player, &merge) {
                report.merges += 1;
            } else {
                report.rejected += 1;
            }
        }
        LogRecordType::FleetName => {
            let Some(rename) = record.as_fleet_name() else {
                report.rejected += 1;
                return;
            };
            if rename_fleet(state, player, &rename) {
                report.renames += 1;
            } else {
                report.rejected += 1;
            }
        }
        LogRecordType::FleetOrderInsert | LogRecordType::FleetOrderUpdate => {
            let Some(order) = record.as_waypoint() else {
                report.rejected += 1;
                return;
            };
            let insert = record.record_type == LogRecordType::FleetOrderInsert;
            if set_waypoint(state, player, &order, insert) {
                report.waypoints += 1;
            } else {
                report.rejected += 1;
            }
        }
        LogRecordType::FleetOrderDelete => {
            let Some(order) = record.as_fleet_order_delete() else {
                report.rejected += 1;
                return;
            };
            if delete_waypoint(state, player, order) {
                report.waypoints += 1;
            } else {
                report.rejected += 1;
            }
        }
        LogRecordType::PlanetProdQueue => {
            let Some(queue) = record.as_production_queue() else {
                report.rejected += 1;
                return;
            };
            if set_queue(state, player, &queue) {
                report.queues += 1;
            } else {
                report.rejected += 1;
            }
        }
        LogRecordType::Research => {
            let Some(order) = record.as_research() else {
                report.rejected += 1;
                return;
            };
            if set_research(state, player, order) {
                report.research += 1;
            } else {
                report.rejected += 1;
            }
        }
        LogRecordType::PlanetRouting => {
            let Some(order) = record.as_planet_routing() else {
                report.rejected += 1;
                return;
            };
            if set_routing(state, player, order) {
                report.routing += 1;
            } else {
                report.rejected += 1;
            }
        }
        LogRecordType::ShipDesign => {
            let Some(change) = record.as_ship_design_change() else {
                report.rejected += 1;
                return;
            };
            if set_design(state, player, &change) {
                report.designs += 1;
            } else {
                report.rejected += 1;
            }
        }
        LogRecordType::FleetFlagBit => {
            let Some(order) = record.as_repeat_orders() else {
                report.rejected += 1;
                return;
            };
            match find_fleet(state, player, order.fleet_id) {
                Some(index) => {
                    state.fleets[index].repeat_orders = order.repeat;
                    report.fleet_settings += 1;
                }
                None => report.rejected += 1,
            }
        }
        LogRecordType::FleetPlan => {
            let Some(order) = record.as_fleet_plan() else {
                report.rejected += 1;
                return;
            };
            match find_fleet(state, player, order.fleet_id) {
                Some(index) => {
                    state.fleets[index].battle_plan = order.plan;
                    report.fleet_settings += 1;
                }
                None => report.rejected += 1,
            }
        }
        LogRecordType::FleetOrderAttrNib => {
            let Some(order) = record.as_order_task() else {
                report.rejected += 1;
                return;
            };
            if set_order_task(state, player, order) {
                report.fleet_settings += 1;
            } else {
                report.rejected += 1;
            }
        }
        LogRecordType::Relations => {
            let Some(order) = record.as_relations() else {
                report.rejected += 1;
                return;
            };
            match state.players.get_mut(player) {
                Some(record) => {
                    record.relations = order.toward;
                    report.relations += 1;
                }
                None => report.rejected += 1,
            }
        }
        kind => {
            if !report.unsupported.contains(&kind) {
                report.unsupported.push(kind);
            }
        }
    }
}

/// Turn a logged transfer into the record the cargo code applies.
///
/// The fleet end must belong to the player; the other end need not, because
/// unloading onto someone else's planet is how colonists are landed. Only the
/// five cargo kinds are carried, which is all the mask ever selects.
fn cargo_record(player: usize, transfer: &CargoTransfer) -> Option<CargoTransferRecord> {
    let class = |nibble: u8| GrobjClass::from_nibble(nibble);
    let source_class = class(transfer.grobj1);
    let destination_class = class(transfer.grobj2);

    let owns = |id: u16| usize::from(object_owner(id)) == player;
    let fleet_is_theirs = match (source_class, destination_class) {
        (Some(GrobjClass::Fleet), _) => owns(transfer.id1),
        (_, Some(GrobjClass::Fleet)) => owns(transfer.id2),
        // Neither end is a fleet: a planet-to-planet transfer is not a thing a
        // client can log, so drop it rather than guess.
        _ => false,
    };
    if !fleet_is_theirs {
        return None;
    }

    // One quantity per set bit of the mask, in ascending bit order.
    let mut quantities = [0i32; CARGO_KINDS];
    let mut next = transfer.quantities.iter();
    for (kind, quantity) in quantities.iter_mut().enumerate() {
        if transfer.items_mask & (1 << kind) != 0 {
            *quantity = next.next().copied().unwrap_or(0);
        }
    }

    Some(CargoTransferRecord {
        source: transfer.id1,
        destination: transfer.id2,
        source_class,
        destination_class,
        mode: (transfer.grobj1 & 0xF) | ((transfer.grobj2 & 0xF) << 4),
        #[allow(clippy::cast_possible_truncation)]
        selector: transfer.items_mask as u8,
        quantities,
    })
}

/// Set the task on one of the player's fleet waypoints.
///
/// The original refuses an order index the fleet does not have and a task
/// above 9, the highest the enumeration defines; both checks are kept.
fn set_order_task(state: &mut GameState, player: usize, order: FleetOrderTask) -> bool {
    if order.task > stars_formats::task::TRANSFER {
        return false;
    }
    let Some(index) = find_fleet(state, player, order.fleet_id) else {
        return false;
    };
    let at = usize::from(order.order_index);
    let Some(waypoint) = state.fleets[index].waypoints.get_mut(at) else {
        return false;
    };
    waypoint.task = order.task;
    if order.task != stars_formats::task::TRANSPORT {
        waypoint.transport = None;
    }
    true
}

/// Move ships between two of the player's fleets, creating the destination if
/// this is the second half of a split.
///
/// The mask names design slots and each quantity is a ship count, positive when
/// the **first** fleet gains. A stack cannot go below zero, and a fleet left
/// with no ships at all is removed, which is what happens to the source of a
/// split that moves everything.
fn move_ships(state: &mut GameState, player: usize, transfer: &CargoTransfer) -> bool {
    let (Some(first), Some(second)) = (
        GrobjClass::from_nibble(transfer.grobj1),
        GrobjClass::from_nibble(transfer.grobj2),
    ) else {
        return false;
    };
    if first != GrobjClass::Fleet || second != GrobjClass::Fleet {
        return false;
    }
    let Some(from) = find_fleet(state, player, transfer.id1) else {
        return false;
    };
    // The other end may not exist yet: a split names the fleet it is about to
    // create.
    let to = match find_fleet(state, player, transfer.id2) {
        Some(index) => index,
        None => match new_fleet(state, player, transfer.id2, from) {
            Some(index) => index,
            None => return false,
        },
    };
    if from == to {
        return false;
    }

    let mut next = transfer.quantities.iter();
    let mut moved = false;
    for slot in 0..16u8 {
        if transfer.items_mask & (1 << slot) == 0 {
            continue;
        }
        let Some(quantity) = next.next().copied() else {
            break;
        };
        // Positive means the first fleet gains, so the ships come from the
        // second; negative is the other way round.
        let (source, sink) = if quantity >= 0 {
            (to, from)
        } else {
            (from, to)
        };
        let wanted = quantity.abs();
        let available = state.fleets[source]
            .stacks
            .iter()
            .find(|s| s.design == slot)
            .map_or(0, |s| s.count)
            .max(0);
        let count = wanted.min(available);
        if count == 0 {
            continue;
        }
        if let Some(stack) = state.fleets[source]
            .stacks
            .iter_mut()
            .find(|s| s.design == slot)
        {
            stack.count -= count;
        }
        match state.fleets[sink]
            .stacks
            .iter_mut()
            .find(|s| s.design == slot)
        {
            Some(stack) => stack.count += count,
            None => state.fleets[sink].stacks.push(crate::fleet::ShipStack {
                design: slot,
                count,
                damaged_pct: 0,
                damage_pct: 0,
            }),
        }
        moved = true;
    }

    if moved {
        for index in [from, to] {
            state.fleets[index].stacks.retain(|s| s.count > 0);
        }
        // A fleet with nothing left in it no longer exists. Removing shifts
        // the indices, so do it after both ends are settled.
        state.fleets.retain(|f| !f.is_empty());
    }
    moved
}

/// Create an empty fleet alongside `beside`, for a split to fill.
fn new_fleet(state: &mut GameState, player: usize, id: u16, beside: usize) -> Option<usize> {
    if usize::from(object_owner(id)) != player {
        return None;
    }
    let source = &state.fleets[beside];
    let fleet = crate::fleet::Fleet {
        id: id & 0x1FF,
        owner: source.owner,
        position: source.position,
        orbiting: source.orbiting,
        stacks: Vec::new(),
        cargo: crate::fleet::Cargo::default(),
        battle_plan: source.battle_plan,
        warp: None,
        waypoints: vec![crate::fleet::Waypoint {
            position: source.position,
            target: source.orbiting,
            warp: 0,
            task: 0,
            transport: None,
        }],
        name: None,
        repeat_orders: false,
    };
    state.fleets.push(fleet);
    Some(state.fleets.len() - 1)
}

/// Merge fleets into the first of them.
///
/// Everything the absorbed fleets hold — ships, cargo — joins the survivor, and
/// they cease to exist. A fleet the state does not hold is skipped rather than
/// failing the whole operation, because the host may have destroyed it since
/// the player's client last saw the game.
fn merge_fleets(state: &mut GameState, player: usize, merge: &FleetMerge) -> bool {
    let Some(survivor_id) = merge.survivor() else {
        return false;
    };
    let Some(survivor) = find_fleet(state, player, survivor_id) else {
        return false;
    };

    let mut merged = false;
    for absorbed_id in merge.absorbed() {
        let Some(absorbed) = find_fleet(state, player, *absorbed_id) else {
            continue;
        };
        if absorbed == survivor {
            continue;
        }
        let taken = state.fleets[absorbed].clone();
        let into = &mut state.fleets[survivor];
        for stack in taken.stacks {
            match into.stacks.iter_mut().find(|s| s.design == stack.design) {
                Some(existing) => existing.count += stack.count,
                None => into.stacks.push(stack),
            }
        }
        for (kind, amount) in taken.cargo.minerals.iter().enumerate() {
            into.cargo.minerals[kind] += amount;
        }
        into.cargo.colonists += taken.cargo.colonists;
        into.cargo.fuel += taken.cargo.fuel;
        state.fleets[absorbed].stacks.clear();
        merged = true;
    }
    if merged {
        state.fleets.retain(|f| !f.is_empty());
    }
    merged
}

/// Rename one of the player's fleets.
fn rename_fleet(state: &mut GameState, player: usize, rename: &FleetName) -> bool {
    let Some(index) = find_fleet(state, player, rename.id) else {
        return false;
    };
    state.fleets[index].name = (!rename.name.is_empty()).then(|| rename.name.clone());
    true
}

/// The index of one of the player's fleets, by the object id a log names.
fn find_fleet(state: &GameState, player: usize, id: u16) -> Option<usize> {
    if usize::from(object_owner(id)) != player {
        return None;
    }
    let number = id & 0x1FF;
    let owner = i16::try_from(player).ok()?;
    state
        .fleets
        .iter()
        .position(|f| f.owner == owner && f.id == number)
}

/// Insert or overwrite one of the player's fleet waypoints.
fn set_waypoint(state: &mut GameState, player: usize, order: &WaypointOrder, insert: bool) -> bool {
    let Some(index) = find_fleet(state, player, order.fleet_id) else {
        return false;
    };
    let fleet = &mut state.fleets[index];
    let at = usize::from(order.waypoint_index);
    // A log may only extend the list by one; anything further would mean the
    // host and the client disagree about the fleet.
    if at > fleet.waypoints.len() {
        return false;
    }

    let waypoint = Waypoint {
        position: Point::new(order.x, order.y),
        target: (order.target_id != 0).then(|| order.target_id.unsigned_abs()),
        warp: order.warp,
        task: order.task,
        transport: (order.task == stars_formats::task::TRANSPORT)
            .then(|| stars_formats::TransportTask::decode(&order.task_data))
            .flatten(),
    };

    if insert || at == fleet.waypoints.len() {
        fleet.waypoints.insert(at, waypoint);
    } else {
        fleet.waypoints[at] = waypoint;
    }
    // The fleet travels at the warp of the leg it is on.
    if at == 1 {
        fleet.warp = (order.warp > 0).then_some(order.warp);
    }
    true
}

/// Delete one of the player's fleet waypoints.
fn delete_waypoint(state: &mut GameState, player: usize, order: FleetOrderDelete) -> bool {
    let Some(index) = find_fleet(state, player, order.fleet_id) else {
        return false;
    };
    let fleet = &mut state.fleets[index];
    let at = usize::from(order.order_index);
    // Waypoint zero is where the fleet is; it is not an order and cannot go.
    if at == 0 || at >= fleet.waypoints.len() {
        return false;
    }
    if order.delete_extra {
        fleet.waypoints.truncate(at);
    } else {
        fleet.waypoints.remove(at);
    }
    if fleet.waypoints.len() < 2 {
        fleet.warp = None;
    }
    true
}

/// One of the player's own planets, by id.
fn find_planet(state: &mut GameState, player: usize, id: u16) -> Option<usize> {
    let owner = i16::try_from(player).ok()?;
    let wanted = i16::try_from(id).ok()?;
    state
        .planets
        .iter()
        .position(|p| p.id == wanted && p.owner == Some(owner))
}

/// Replace a planet's production queue.
fn set_queue(
    state: &mut GameState,
    player: usize,
    queue: &stars_formats::ProductionQueueRecord,
) -> bool {
    let Some(id) = queue.planet_id else {
        return false;
    };
    let Some(index) = find_planet(state, player, id) else {
        return false;
    };
    state.planets[index].queue = queue
        .items
        .iter()
        .map(|item| QueueItem {
            count: i32::from(item.count),
            item: item.item,
            ship: item.class == stars_formats::QueueClass::Fleet,
            completion: i32::from(item.completion),
        })
        .collect();
    true
}

/// Change a player's research settings.
fn set_research(state: &mut GameState, player: usize, order: ResearchOrder) -> bool {
    let Some(record) = state.players.get_mut(player) else {
        return false;
    };
    record.research_pct = order.pct_resources.min(100);
    record.research.current_field = usize::from(order.current_field).min(5);
    record.research.next_field = NextField::from_raw(order.next_field);
    true
}

/// Change a planet's routing and no-research settings.
fn set_routing(state: &mut GameState, player: usize, order: PlanetRoutingOrder) -> bool {
    let Some(index) = find_planet(state, player, order.planet_id) else {
        return false;
    };
    state.planets[index].no_research = order.no_research;
    true
}

/// Create, change or delete one of a player's ship designs.
fn set_design(state: &mut GameState, player: usize, change: &ShipDesignChange) -> bool {
    if usize::from(change.player) != player {
        return false;
    }
    let slot = usize::from(change.design_index)
        + if change.design.as_ref().is_some_and(|d| d.starbase) {
            usize::from(crate::startup::FIRST_STARBASE_SLOT)
        } else {
            0
        };
    if state.designs.len() <= player {
        state.designs.resize_with(player + 1, Vec::new);
    }
    let designs = &mut state.designs[player];
    if designs.len() <= slot {
        designs.resize_with(slot + 1, || crate::design::ShipDesign {
            name: String::new(),
            picture: 0,
            stored_armor: 0,
            hull_id: -1,
            slots: Vec::new(),
        });
    }
    match &change.design {
        // A change carrying a design creates or replaces it.
        Some(record) => designs[slot] = crate::load::design_from_record(record),
        // A bare header deletes: the slot becomes free, which the rest of the
        // simulation reads as "no such design".
        None => {
            designs[slot] = crate::design::ShipDesign {
                name: String::new(),
                picture: 0,
                stored_armor: 0,
                hull_id: -1,
                slots: Vec::new(),
            };
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet::{Cargo, Fleet, ShipStack};
    use crate::planet::Planet;
    use crate::race::Race;
    use crate::Player;
    use stars_formats::{FleetMerge, FleetName};

    /// A one-player game with a fleet orbiting its planet.
    fn a_game() -> GameState {
        let mut state = GameState::new(1);
        state.players.push(Player::new(Race::humanoid()));
        let mut planet = Planet::unowned(7);
        planet.owner = Some(0);
        planet.position = Some(Point::new(1100, 1200));
        planet.surface_min = [500, 500, 500];
        planet.pop = 250;
        state.planets.push(planet);
        state.fleets.push(Fleet {
            name: None,
            repeat_orders: false,
            id: 3,
            owner: 0,
            position: Point::new(1100, 1200),
            orbiting: Some(7),
            stacks: vec![ShipStack {
                design: 0,
                count: 1,
                damaged_pct: 0,
                damage_pct: 0,
            }],
            cargo: Cargo::default(),
            battle_plan: 0,
            warp: None,
            waypoints: vec![Waypoint {
                position: Point::new(1100, 1200),
                target: Some(7),
                warp: 0,
                task: 0,
                transport: None,
            }],
        });
        state
    }

    /// The object id a log uses for player `player`'s fleet `id`.
    fn fleet_word(player: u16, id: u16) -> u16 {
        (player << 9) | id
    }

    #[test]
    fn a_waypoint_is_inserted_updated_and_deleted() {
        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);

        let order = WaypointOrder {
            fleet_id: fleet_word(0, 3),
            waypoint_index: 1,
            x: 1300,
            y: 1400,
            target_id: 9,
            task: 0,
            warp: 7,
            grobj: 1,
            valid_task: false,
            flags_high: 0,
            task_data: Vec::new(),
        };
        log.records.push(LogRecord::waypoint(&order, true));
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.waypoints, 1);
        assert_eq!(report.rejected, 0);
        assert_eq!(state.fleets[0].waypoints.len(), 2);
        assert_eq!(
            state.fleets[0].waypoints[1].position,
            Point::new(1300, 1400)
        );
        assert_eq!(state.fleets[0].waypoints[1].target, Some(9));
        assert_eq!(state.fleets[0].warp, Some(7));

        // An update overwrites rather than lengthening.
        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(LogRecord::waypoint(
            &WaypointOrder {
                warp: 5,
                task: 2,
                valid_task: true,
                ..order.clone()
            },
            false,
        ));
        replay(&mut state, 0, &log, &mut orders);
        assert_eq!(state.fleets[0].waypoints.len(), 2);
        assert_eq!(state.fleets[0].waypoints[1].warp, 5);
        assert_eq!(state.fleets[0].waypoints[1].task, 2);

        // And a delete removes it, leaving the fleet where it stands.
        let mut log = OrderLog::new(0, [0; 11]);
        log.records
            .push(LogRecord::delete_waypoint(FleetOrderDelete {
                fleet_id: fleet_word(0, 3),
                order_index: 1,
                delete_extra: false,
            }));
        replay(&mut state, 0, &log, &mut orders);
        assert_eq!(state.fleets[0].waypoints.len(), 1);
        assert_eq!(state.fleets[0].warp, None);
    }

    /// Waypoint zero is where the fleet is, not an order, and cannot be deleted.
    #[test]
    fn waypoint_zero_is_not_an_order() {
        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        log.records
            .push(LogRecord::delete_waypoint(FleetOrderDelete {
                fleet_id: fleet_word(0, 3),
                order_index: 0,
                delete_extra: false,
            }));
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.rejected, 1);
        assert_eq!(state.fleets[0].waypoints.len(), 1);
    }

    /// A transfer's quantities land on the cargo kinds its mask selects.
    #[test]
    fn a_transfer_spreads_its_quantities_over_the_mask() {
        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        // Ironium (bit 0) and germanium (bit 2).
        log.records.push(LogRecord::cargo(&CargoTransfer {
            id1: fleet_word(0, 3),
            id2: 7,
            grobj1: 2,
            grobj2: 1,
            items_mask: 0b101,
            quantities: vec![11, 22],
            quantity_bytes: Vec::new(),
        }));
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.cargo, 1);
        assert_eq!(orders.cargo.len(), 1);
        assert_eq!(orders.cargo[0].quantities, [11, 0, 22, 0, 0]);
        assert_eq!(orders.cargo[0].source_class, Some(GrobjClass::Fleet));
        assert_eq!(orders.cargo[0].destination_class, Some(GrobjClass::Planet));
    }

    /// A transfer naming someone else's fleet is refused.
    #[test]
    fn a_transfer_must_name_the_players_own_fleet() {
        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(LogRecord::cargo(&CargoTransfer {
            id1: fleet_word(4, 3),
            id2: 7,
            grobj1: 2,
            grobj2: 1,
            items_mask: 1,
            quantities: vec![5],
            quantity_bytes: Vec::new(),
        }));
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.rejected, 1);
        assert!(orders.cargo.is_empty());
    }

    /// A design change creates a design, and a bare header deletes it.
    #[test]
    fn a_design_is_created_and_deleted() {
        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let design = crate::startup::SHIPS[crate::startup::ship::SANTA_MARIA].design();

        let record = stars_formats::DesignRecord {
            full_design: true,
            transferred: false,
            starbase: false,
            design_number: 2,
            hull_id: u8::try_from(design.hull_id).expect("a ship hull"),
            pic: design.picture,
            armor: Some(0),
            mass: None,
            turn_designed: Some(1),
            total_built: Some(0),
            total_remaining: Some(0),
            slots: design
                .slots
                .iter()
                .map(|s| stars_formats::Slot {
                    category: s.category,
                    item_id: s.item,
                    count: s.count,
                })
                .collect(),
            name: design.name.clone(),
            flags0: 3,
            flags1: 1,
            trailing: Vec::new(),
        };

        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(
            LogRecord::ship_design(&ShipDesignChange {
                mode: 1,
                player: 0,
                design_index: 2,
                header_high: 0,
                design: Some(record),
            })
            .expect("encodes"),
        );
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.designs, 1);
        assert_eq!(state.designs[0][2].name, "Santa Maria");
        assert_eq!(state.designs[0][2].hull_id, design.hull_id);

        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(
            LogRecord::ship_design(&ShipDesignChange {
                mode: 2,
                player: 0,
                design_index: 2,
                header_high: 0,
                design: None,
            })
            .expect("encodes"),
        );
        replay(&mut state, 0, &log, &mut orders);
        assert_eq!(state.designs[0][2].hull_id, -1, "the slot is free again");
    }

    /// An operation this replay does not implement is named, not silently
    /// dropped.
    #[test]
    fn an_unsupported_operation_is_reported() {
        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        // The zipped production-queue templates are the one operation left:
        // they are host-only bookkeeping this project does not model.
        log.records
            .push(LogRecord::raw(LogRecordType::PlayerZpq1, vec![0x03, 0x00]));
        log.records
            .push(LogRecord::raw(LogRecordType::PlayerZpq1, vec![0x03, 0x00]));
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.applied(), 0);
        assert_eq!(
            report.unsupported,
            vec![LogRecordType::PlayerZpq1],
            "named once, however often it appears"
        );
    }

    /// Splitting a fleet: the split names the fleet, the transfer that follows
    /// creates the new one and moves ships into it.
    #[test]
    fn a_fleet_splits_into_a_new_one() {
        let mut state = a_game();
        state.fleets[0].stacks = vec![ShipStack {
            design: 3,
            count: 3,
            damaged_pct: 0,
            damage_pct: 0,
        }];
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);

        log.records
            .push(LogRecord::split_fleet(stars_formats::FleetSplit {
                fleet_id: fleet_word(0, 3),
            }));
        // One ship of design 3 leaves fleet 3 for a fleet that does not exist
        // yet. Negative: the *first* fleet loses.
        log.records.push(LogRecord::ships(&CargoTransfer {
            id1: fleet_word(0, 3),
            id2: fleet_word(0, 9),
            grobj1: 2,
            grobj2: 2,
            items_mask: 1 << 3,
            quantities: vec![-1],
            quantity_bytes: Vec::new(),
        }));

        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.ship_moves, 1);
        assert_eq!(report.rejected, 0);
        assert_eq!(state.fleets.len(), 2, "the new fleet exists");

        let old = state
            .fleets
            .iter()
            .find(|f| f.id == 3)
            .expect("the original");
        let new = state
            .fleets
            .iter()
            .find(|f| f.id == 9)
            .expect("the new one");
        assert_eq!(old.stacks[0].count, 2, "two ships left behind");
        assert_eq!(
            new.stacks,
            vec![ShipStack {
                design: 3,
                count: 1,
                damaged_pct: 0,
                damage_pct: 0
            }]
        );
        assert_eq!(new.position, old.position, "it starts where it split");
        assert_eq!(new.orbiting, old.orbiting);
    }

    /// Splitting off everything leaves no source fleet behind.
    #[test]
    fn a_fleet_that_gives_up_every_ship_ceases_to_exist() {
        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(LogRecord::ships(&CargoTransfer {
            id1: fleet_word(0, 3),
            id2: fleet_word(0, 9),
            grobj1: 2,
            grobj2: 2,
            items_mask: 1,
            quantities: vec![-1],
            quantity_bytes: Vec::new(),
        }));
        replay(&mut state, 0, &log, &mut orders);
        assert_eq!(state.fleets.len(), 1);
        assert_eq!(state.fleets[0].id, 9, "only the new fleet is left");
    }

    /// Merging: the first fleet named survives and takes everything.
    #[test]
    fn fleets_merge_into_the_first_of_them() {
        let mut state = a_game();
        state.fleets[0].cargo.minerals = [10, 0, 0];
        let mut second = state.fleets[0].clone();
        second.id = 8;
        second.stacks = vec![ShipStack {
            design: 1,
            count: 2,
            damaged_pct: 0,
            damage_pct: 0,
        }];
        second.cargo.minerals = [5, 7, 0];
        second.cargo.fuel = 40;
        state.fleets.push(second);

        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(LogRecord::merge_fleets(&FleetMerge {
            fleets: vec![fleet_word(0, 3), fleet_word(0, 8)],
        }));

        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.merges, 1);
        assert_eq!(state.fleets.len(), 1, "the absorbed fleet is gone");
        let survivor = &state.fleets[0];
        assert_eq!(survivor.id, 3, "the first named survives");
        assert_eq!(survivor.ships(), 3, "it has both fleets' ships");
        assert_eq!(survivor.cargo.minerals, [15, 7, 0]);
        assert_eq!(survivor.cargo.fuel, 40);
    }

    /// A merge naming someone else's fleet is refused.
    #[test]
    fn a_merge_must_name_the_players_own_fleets() {
        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(LogRecord::merge_fleets(&FleetMerge {
            fleets: vec![fleet_word(2, 3), fleet_word(2, 8)],
        }));
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.rejected, 1);
        assert_eq!(report.merges, 0);
        assert_eq!(state.fleets.len(), 1);
    }

    /// Renaming a fleet, and clearing the name again.
    #[test]
    fn a_fleet_can_be_renamed() {
        let mut state = a_game();
        let mut orders = TurnOrders::default();

        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(LogRecord::fleet_name(&FleetName {
            id: fleet_word(0, 3),
            grobj: 2,
            name: "Bold Endeavour".into(),
        }));
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.renames, 1);
        assert_eq!(state.fleets[0].name.as_deref(), Some("Bold Endeavour"));

        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(LogRecord::fleet_name(&FleetName {
            id: fleet_word(0, 3),
            grobj: 2,
            name: String::new(),
        }));
        replay(&mut state, 0, &log, &mut orders);
        assert_eq!(state.fleets[0].name, None, "an empty name is no name");
    }

    /// The battle plan, the repeat-orders flag and a waypoint's task.
    #[test]
    fn fleet_settings_are_replayed() {
        use stars_formats::{FleetOrderTask, FleetPlan, FleetRepeatOrders};

        let mut state = a_game();
        state.fleets[0].waypoints.push(Waypoint {
            position: Point::new(1300, 1400),
            target: Some(9),
            warp: 6,
            task: 0,
            transport: None,
        });
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);

        log.records.push(LogRecord::fleet_plan(FleetPlan {
            fleet_id: fleet_word(0, 3),
            plan: 2,
        }));
        log.records
            .push(LogRecord::repeat_orders(FleetRepeatOrders {
                fleet_id: fleet_word(0, 3),
                repeat: true,
            }));
        log.records.push(LogRecord::order_task(FleetOrderTask {
            fleet_id: fleet_word(0, 3),
            order_index: 1,
            task: stars_formats::task::COLONIZE,
        }));

        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.fleet_settings, 3);
        assert_eq!(report.rejected, 0);
        assert_eq!(state.fleets[0].battle_plan, 2);
        assert!(state.fleets[0].repeat_orders);
        assert_eq!(
            state.fleets[0].waypoints[1].task,
            stars_formats::task::COLONIZE
        );
    }

    /// A waypoint the fleet does not have is refused, and so is a task the
    /// enumeration does not define.
    #[test]
    fn a_waypoint_task_is_bounds_checked() {
        use stars_formats::{FleetOrderTask, LogRecordType};

        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(LogRecord::order_task(FleetOrderTask {
            fleet_id: fleet_word(0, 3),
            order_index: 7,
            task: 1,
        }));
        // A task nibble above 9 cannot even be encoded, so build it by hand.
        log.records.push(LogRecord::raw(
            LogRecordType::FleetOrderAttrNib,
            vec![0x03, 0x00, 0x00, 0x00, 0x0f, 0x00],
        ));
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.rejected, 2);
        assert_eq!(report.fleet_settings, 0);
    }

    /// The relations table replaces whatever the player had.
    #[test]
    fn relations_are_replayed() {
        use stars_formats::Relations;

        let mut state = a_game();
        state.players.push(Player::new(Race::humanoid()));
        state.players.push(Player::new(Race::humanoid()));
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(LogRecord::relations(&Relations {
            toward: vec![0, 2, 1],
        }));

        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.relations, 1);
        assert_eq!(state.players[0].relations, vec![0, 2, 1]);
        assert!(state.players[0].regards_as_friend(2));
        assert!(!state.players[0].regards_as_friend(1));
    }
}
