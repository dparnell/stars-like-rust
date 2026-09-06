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
    object_owner, BattlePlanChange, CargoTransfer, CargoTransferRecord, FleetMerge, FleetName,
    FleetOrderDelete, FleetOrderTask, GrobjClass, LogRecord, LogRecordType, OrderLog,
    PlanetRoutingOrder, ResearchOrder, ShipDesignChange, WaypointOrder,
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
    /// Battle plans defined, retuned or deleted.
    pub battle_plans: usize,
    /// Turn passwords changed.
    pub passwords: usize,
    /// Message filters set.
    pub message_filters: usize,
    /// Default production queues replaced.
    pub default_queues: usize,
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
            + self.battle_plans
            + self.passwords
            + self.message_filters
            + self.default_queues
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
        LogRecordType::BattlePlan => {
            let Some(change) = record.as_battle_plan() else {
                report.rejected += 1;
                return;
            };
            match set_battle_plan(state, player, &change) {
                PlanEdit::Applied => report.battle_plans += 1,
                PlanEdit::Ignored => {}
                PlanEdit::Rejected => report.rejected += 1,
            }
        }
        LogRecordType::MessageFilter => {
            // `log.c` writes the whole bitfield each time the player changes
            // it, so the record replaces rather than edits. It is a reading
            // preference and changes nothing about the game; the host keeps it
            // so that it survives into the next turn's file.
            let Some(filter) = record.as_message_filter() else {
                report.rejected += 1;
                return;
            };
            match state.players.get_mut(player) {
                Some(record) => {
                    record.message_filter = filter;
                    report.message_filters += 1;
                }
                None => report.rejected += 1,
            }
        }
        LogRecordType::ChangePassword => {
            // `1048:c65c`: the salt goes straight into the player's own record,
            // at `PLAYER + 0x0c`. The original gates the arm on bit 1 of the
            // runtime mode word `gd` (`DS:0x7ca`), the flag that says a game is
            // open with its orders live — the same test every `Log*` writer
            // opens with. This engine has no such word; it is always applying
            // to a state it holds, so the arm always runs.
            let Some(change) = record.as_password_change() else {
                report.rejected += 1;
                return;
            };
            match state.players.get_mut(player) {
                Some(record) => {
                    record.password = change.salt;
                    report.passwords += 1;
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
        // The default production queue a player's new colonies start with.
        // The original applies this one only when generating a turn, which is
        // exactly the context a replay runs in.
        LogRecordType::PlayerZpq1 => {
            let Some(queue) = stars_formats::DefaultQueue::decode(&record.data) else {
                report.rejected += 1;
                return;
            };
            match state.players.get_mut(player) {
                Some(record) => {
                    record.default_queue = queue;
                    report.default_queues += 1;
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

/// What the replay made of a battle-plan record.
///
/// The original's arm is not simply accept/reject: a **delete** that names a
/// plan the player does not have returns success and does nothing, while a
/// definition in the same position is refused. Both are modelled so the report
/// does not call a no-op a rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanEdit {
    /// The plan list changed.
    Applied,
    /// Accepted, but there was nothing to do.
    Ignored,
    /// Refused.
    Rejected,
}

/// The largest tactic value the original accepts (`1048:c324`).
const MAX_TACTIC: u8 = 6;

/// The largest primary/secondary target value it accepts (`1048:c336`,
/// `1048:c350`).
const MAX_TARGET: u8 = 8;

/// The most battle plans a player can hold (`1048:c36f`).
const MAX_BATTLE_PLANS: usize = 16;

/// Define, retune or delete one of the player's battle plans.
///
/// Transcribed from the replay arm at `1048:c287`. The record names its owner
/// in the low nibble of byte 0 and the slot in the high one, and the host
/// accepts it only from the player whose log it is, for a slot the player
/// already has **or the next one** — appending is how a new plan is made. A
/// definition is checked field by field: tactic `0..=6`, both targets `0..=8`.
/// The "attack who" byte is not checked, and is kept as written.
///
/// Deleting shifts the rest down (`DeleteBattlePlan`, `10f0:1706`): the plans
/// after it move up a slot and are restamped with their new index, and every
/// one of the player's fleets pointing at or past the deleted slot has its
/// index decremented — including a fleet that was using the deleted plan,
/// which the original moves to the slot before it. That subtraction is a plain
/// byte decrement in the original, so deleting slot 0 leaves a fleet that used
/// it pointing at 255; the wrap is kept rather than papered over, because a
/// host that clamped would diverge from the client that wrote the log.
fn set_battle_plan(state: &mut GameState, player: usize, change: &BattlePlanChange) -> PlanEdit {
    let slot = usize::from(change.plan.plan_id);
    let Some(record) = state.players.get(player) else {
        return PlanEdit::Rejected;
    };
    if usize::from(change.plan.race_id) != player || slot > record.battle_plans.len() {
        // Out of range: a delete is shrugged off, a definition refused.
        return if change.delete {
            PlanEdit::Ignored
        } else {
            PlanEdit::Rejected
        };
    }

    if change.delete {
        if slot >= state.players[player].battle_plans.len() {
            return PlanEdit::Ignored;
        }
        state.players[player].battle_plans.remove(slot);
        for (index, plan) in state.players[player]
            .battle_plans
            .iter_mut()
            .enumerate()
            .skip(slot)
        {
            plan.plan_id = u8::try_from(index).unwrap_or(0) & 0x0F;
        }
        let owner = i16::try_from(player).unwrap_or(-1);
        let deleted = u8::try_from(slot).unwrap_or(0);
        for fleet in state.fleets.iter_mut().filter(|f| f.owner == owner) {
            if fleet.battle_plan >= deleted {
                fleet.battle_plan = fleet.battle_plan.wrapping_sub(1);
            }
        }
        return PlanEdit::Applied;
    }

    if change.plan.tactic_nibble() > MAX_TACTIC
        || change.plan.primary_target > MAX_TARGET
        || change.plan.secondary_target > MAX_TARGET
    {
        return PlanEdit::Rejected;
    }
    let mut plan = change.plan.clone();
    plan.race_id = u8::try_from(player).unwrap_or(0) & 0x0F;
    plan.plan_id = u8::try_from(slot).unwrap_or(0) & 0x0F;
    let plans = &mut state.players[player].battle_plans;
    if slot == plans.len() {
        if slot >= MAX_BATTLE_PLANS {
            return PlanEdit::Rejected;
        }
        plans.push(plan);
    } else {
        plans[slot] = plan;
    }
    PlanEdit::Applied
}

/// Set the task on one of the player's fleet waypoints.
///
/// The replay arm is at `1048:c3f0`, shared with `rtLogFleetFlagBit9`. It
/// refuses a fleet it cannot find, an order index the fleet does not have
/// (`FLEET.cord <= iOrder`) and a value word of 10 or more — the whole word,
/// not its low nibble, so `0x10` is refused even though its nibble is 0. All
/// three checks are kept.
///
/// Two deliberate differences. The original compares `cord > iOrder` signed and
/// so accepts a negative index, writing before the order array; `usize::from`
/// on a `u16` cannot express that, and an index past the end is refused here as
/// it is there. And the original leaves the rest of the flag word and the
/// order's payload bytes untouched, letting a Transport payload be reread as
/// the new task's — this model holds a decoded `transport` instead of raw
/// bytes, so it drops it when the task is no longer Transport.
fn set_order_task(state: &mut GameState, player: usize, order: FleetOrderTask) -> bool {
    if !order.value_in_range() {
        return false;
    }
    let Some(index) = find_fleet(state, player, order.fleet_id) else {
        return false;
    };
    let at = usize::from(order.order_index);
    let Some(waypoint) = state.fleets[index].waypoints.get_mut(at) else {
        return false;
    };
    waypoint.task = order.task();
    if waypoint.task != stars_formats::task::TRANSPORT {
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
            target_class: 1,
            warp: 0,
            task: 0,
            transport: None,
            task_data: Vec::new(),
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
        target_class: 1,
        warp: order.warp,
        task: order.task,
        transport: (order.task == stars_formats::task::TRANSPORT)
            .then(|| stars_formats::TransportTask::decode(&order.task_data))
            .flatten(),
        task_data: order.task_data.clone(),
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
    // The header's `ishdef` is already the **full** design slot: `SHDEF.det`
    // packs it as five bits (`det:8, fInclude:1, fFree:1, ishdef:5, fGift:1`)
    // and a starbase design is stored at 16..=25, so its top bit is set.
    // `LogChangeShDef` writes that whole field into the header word, and the
    // embedded record's own "starbase" flag is nothing but the same bit — its
    // `design_number` carries only the low four. Adding the starbase offset to
    // the header value would count it twice, and a bare delete carries no
    // record to read it from at all.
    let slot = usize::from(change.design_index);
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
                target_class: 1,
                warp: 0,
                task: 0,
                transport: None,
                task_data: Vec::new(),
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
    ///
    /// Every operation the format **names** is replayed now, so the only thing
    /// left to be unsupported is a record type the format does not define.
    #[test]
    fn an_unsupported_operation_is_reported() {
        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        log.records
            .push(LogRecord::raw(LogRecordType::Other(60), vec![0x03, 0x00]));
        log.records
            .push(LogRecord::raw(LogRecordType::Other(60), vec![0x03, 0x00]));
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.applied(), 0);
        assert_eq!(
            report.unsupported,
            vec![LogRecordType::Other(60)],
            "named once, however often it appears"
        );
    }

    /// The default production queue: replayed, then handed to a planet the
    /// player settles.
    #[test]
    fn a_default_queue_reaches_a_new_colony() {
        use crate::production::item;
        use stars_formats::{DefaultQueue, DefaultQueueItem};

        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        let queue = DefaultQueue {
            no_research: true,
            items: vec![
                DefaultQueueItem {
                    item: item::FACTORY as u8,
                    count: 12,
                },
                DefaultQueueItem {
                    item: item::MINE as u8,
                    count: 7,
                },
                DefaultQueueItem {
                    item: item::MAX_TERRAFORM as u8,
                    count: 1,
                },
            ],
        };
        log.records
            .push(LogRecord::raw(LogRecordType::PlayerZpq1, queue.encode()));

        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.default_queues, 1);
        assert_eq!(state.players[0].default_queue, queue);

        // A planet that becomes theirs gets it.
        let mut planet = Planet::unowned(11);
        planet.owner = Some(0);
        state.planets.push(planet);
        let index = state.planets.len() - 1;
        crate::orders::apply_default_queue(&mut state, index);
        assert!(state.planets[index].no_research);
        assert_eq!(
            state.planets[index]
                .queue
                .iter()
                .map(|q| (q.item, q.count))
                .collect::<Vec<_>>(),
            vec![
                (item::FACTORY, 12),
                (item::MINE, 7),
                (item::MAX_TERRAFORM, 1)
            ]
        );
    }

    /// The two racial filters drop entries rather than queueing them.
    #[test]
    fn a_default_queue_is_filtered_by_the_primary_trait() {
        use crate::production::item;
        use crate::race::Prt;
        use stars_formats::{DefaultQueue, DefaultQueueItem};

        let queue = DefaultQueue {
            no_research: false,
            items: [
                item::MINE,
                item::FACTORY,
                item::DEFENSE,
                item::ALCHEMY,
                item::MAX_TERRAFORM,
            ]
            .into_iter()
            .map(|item| DefaultQueueItem {
                item: item as u8,
                count: 1,
            })
            .collect(),
        };

        // Alternate Reality builds no planetary installation at all.
        let mut state = a_game();
        state.players[0].race = crate::newgame::stock_race(Prt::Ar);
        state.players[0].default_queue = queue.clone();
        let mut planet = Planet::unowned(11);
        planet.owner = Some(0);
        state.planets.push(planet);
        let index = state.planets.len() - 1;
        crate::orders::apply_default_queue(&mut state, index);
        assert_eq!(
            state.planets[index]
                .queue
                .iter()
                .map(|q| q.item)
                .collect::<Vec<_>>(),
            vec![item::ALCHEMY, item::MAX_TERRAFORM]
        );

        // A Claim Adjuster is never offered terraforming.
        let mut state = a_game();
        state.players[0].race = crate::newgame::stock_race(Prt::Ca);
        state.players[0].default_queue = queue;
        let mut planet = Planet::unowned(11);
        planet.owner = Some(0);
        state.planets.push(planet);
        let index = state.planets.len() - 1;
        crate::orders::apply_default_queue(&mut state, index);
        assert_eq!(
            state.planets[index]
                .queue
                .iter()
                .map(|q| q.item)
                .collect::<Vec<_>>(),
            vec![item::MINE, item::FACTORY, item::DEFENSE, item::ALCHEMY]
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
            target_class: 1,
            warp: 6,
            task: 0,
            transport: None,
            task_data: Vec::new(),
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
        log.records.push(LogRecord::order_task(FleetOrderTask::new(
            fleet_word(0, 3),
            1,
            stars_formats::task::COLONIZE,
        )));

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

    /// A password change reaches the player's own record, and only theirs.
    #[test]
    fn a_password_is_replayed() {
        use stars_formats::PasswordChange;

        let mut state = a_game();
        state.players.push(Player::new(Race::humanoid()));
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        let salt = stars_formats::password_salt("open sesame");
        assert_ne!(salt, 0);
        log.records
            .push(LogRecord::change_password(PasswordChange { salt }));

        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.passwords, 1);
        assert_eq!(report.rejected, 0);
        assert_eq!(state.players[0].password, salt);
        assert_eq!(state.players[1].password, 0, "nobody else's");

        // Clearing it is the same record with a zero salt.
        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(LogRecord::change_password(PasswordChange {
            salt: stars_formats::password_salt(""),
        }));
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.passwords, 1);
        assert_eq!(state.players[0].password, 0);
    }

    /// A plan is retuned, a new one appended, and one deleted — the three
    /// things `BattlePlansDlg` logs.
    #[test]
    fn battle_plans_are_defined_and_deleted() {
        use stars_formats::{BattlePlanChange, BattlePlanRecord};

        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        assert_eq!(state.players[0].battle_plans.len(), 5);
        state.fleets[0].battle_plan = 4;

        // Retune slot 2 and rename it.
        let mut plan = state.players[0].battle_plans[2].clone();
        plan.name = "Bombers first".to_string();
        plan.primary_target = 8;
        log.records.push(
            LogRecord::battle_plan(&BattlePlanChange {
                plan,
                delete: false,
            })
            .expect("encodes"),
        );
        // Append a sixth.
        let mut sixth = BattlePlanRecord {
            race_id: 0,
            plan_id: 5,
            tactic: 6,
            primary_target: 0,
            secondary_target: 0,
            attack_who: 3,
            name: "Last stand".to_string(),
            trailing: Vec::new(),
        };
        log.records.push(
            LogRecord::battle_plan(&BattlePlanChange {
                plan: sixth.clone(),
                delete: false,
            })
            .expect("encodes"),
        );
        // Delete slot 1.
        sixth.plan_id = 1;
        sixth.tactic = stars_formats::PLAN_DELETED;
        log.records.push(
            LogRecord::battle_plan(&BattlePlanChange {
                plan: sixth,
                delete: true,
            })
            .expect("encodes"),
        );

        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.battle_plans, 3);
        assert_eq!(report.rejected, 0);

        let plans = &state.players[0].battle_plans;
        assert_eq!(plans.len(), 5);
        assert_eq!(plans[1].name, "Bombers first");
        assert_eq!(plans[1].primary_target, 8);
        assert_eq!(plans.last().unwrap().name, "Last stand");
        // The survivors are restamped with their new slots.
        for (slot, plan) in plans.iter().enumerate() {
            assert_eq!(usize::from(plan.plan_id), slot);
        }
        // A fleet past the deleted slot follows it down.
        assert_eq!(state.fleets[0].battle_plan, 3);
    }

    /// The bounds the original puts on a definition, and the owner check.
    #[test]
    fn a_battle_plan_is_bounds_checked() {
        use stars_formats::{BattlePlanChange, BattlePlanRecord};

        let plan = |plan_id: u8, tactic: u8, primary: u8, secondary: u8, race: u8| {
            LogRecord::battle_plan(&BattlePlanChange {
                plan: BattlePlanRecord {
                    race_id: race,
                    plan_id,
                    tactic,
                    primary_target: primary,
                    secondary_target: secondary,
                    attack_who: 1,
                    name: "x".to_string(),
                    trailing: Vec::new(),
                },
                delete: false,
            })
            .expect("encodes")
        };

        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(plan(0, 7, 0, 0, 0)); // tactic above 6
        log.records.push(plan(0, 0, 9, 0, 0)); // primary above 8
        log.records.push(plan(0, 0, 0, 9, 0)); // secondary above 8
        log.records.push(plan(6, 0, 0, 0, 0)); // a slot past the end
        log.records.push(plan(0, 0, 0, 0, 1)); // someone else's plan
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.battle_plans, 0);
        assert_eq!(report.rejected, 5);
        assert_eq!(state.players[0].battle_plans.len(), 5);
    }

    /// Deleting a plan the player does not have is accepted and does nothing,
    /// which is what the arm at `1048:c2e4` does.
    #[test]
    fn deleting_a_plan_that_is_not_there_is_a_no_op() {
        use stars_formats::{BattlePlanChange, BattlePlanRecord};

        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(
            LogRecord::battle_plan(&BattlePlanChange {
                plan: BattlePlanRecord {
                    race_id: 0,
                    plan_id: 9,
                    tactic: stars_formats::PLAN_DELETED,
                    primary_target: 0,
                    secondary_target: 0,
                    attack_who: 0,
                    name: String::new(),
                    trailing: Vec::new(),
                },
                delete: true,
            })
            .expect("encodes"),
        );
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.battle_plans, 0);
        assert_eq!(report.rejected, 0);
        assert_eq!(state.players[0].battle_plans.len(), 5);
    }

    /// Every case in `docs/vectors/order-attr-nib.json` — the substitute for
    /// the fixture the corpus cannot supply, because nothing in the shipped
    /// client writes a type-11 record. The cases come from the replay arm at
    /// `1048:c3f0`.
    #[test]
    fn the_waypoint_task_vectors_replay() {
        use stars_formats::FleetOrderTask;

        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/vectors/order-attr-nib.json"
        );
        let text = std::fs::read_to_string(path).expect("vectors are versioned with the specs");
        let vectors: serde_json::Value = serde_json::from_str(&text).expect("vectors parse");
        let cases = vectors["replay"]["cases"]
            .as_array()
            .expect("cases")
            .clone();
        assert!(!cases.is_empty());

        for case in cases {
            let why = case["why"].as_str().unwrap_or_default();
            let data: Vec<u8> = (0..6)
                .map(|i| {
                    let hex = &case["bytes"].as_str().expect("bytes")[i * 2..i * 2 + 2];
                    u8::from_str_radix(hex, 16).expect("hex")
                })
                .collect();

            // The decoder reads the payload the arm reads.
            let decoded = FleetOrderTask::decode(&data).expect("six bytes decode");
            assert_eq!(
                u64::from(decoded.fleet_id),
                case["fleet_id"].as_u64().unwrap(),
                "{why}"
            );
            assert_eq!(
                u64::from(decoded.order_index),
                case["order_index"].as_u64().unwrap(),
                "{why}"
            );
            assert_eq!(
                u64::from(decoded.value),
                case["value"].as_u64().unwrap(),
                "{why}"
            );
            assert_eq!(decoded.encode().as_slice(), data.as_slice(), "{why}");

            let mut state = a_game();
            state.fleets[0].waypoints.push(Waypoint {
                position: Point::new(1300, 1400),
                target: Some(9),
                target_class: 1,
                warp: 6,
                task: 0,
                transport: None,
                task_data: Vec::new(),
            });
            let mut orders = TurnOrders::default();
            let mut log = OrderLog::new(0, [0; 11]);
            log.records.push(LogRecord::order_task(decoded));
            let report = replay(&mut state, 0, &log, &mut orders);

            if case["accepted"].as_bool().unwrap() {
                assert_eq!(report.fleet_settings, 1, "{why}");
                assert_eq!(report.rejected, 0, "{why}");
                let at = usize::from(decoded.order_index);
                assert_eq!(
                    u64::from(state.fleets[0].waypoints[at].task),
                    case["task"].as_u64().unwrap(),
                    "{why}"
                );
            } else {
                assert_eq!(report.fleet_settings, 0, "{why}");
                assert_eq!(report.rejected, 1, "{why}");
                assert!(
                    state.fleets[0].waypoints.iter().all(|w| w.task == 0),
                    "a refused record must leave every waypoint alone: {why}"
                );
            }
        }
    }

    /// A waypoint the fleet does not have is refused, and so is a task the
    /// enumeration does not define.
    #[test]
    fn a_waypoint_task_is_bounds_checked() {
        use stars_formats::FleetOrderTask;

        let mut state = a_game();
        let mut orders = TurnOrders::default();
        let mut log = OrderLog::new(0, [0; 11]);
        log.records.push(LogRecord::order_task(FleetOrderTask::new(
            fleet_word(0, 3),
            7,
            1,
        )));
        // A value of 15: its nibble is not a task the enumeration defines.
        log.records.push(LogRecord::order_task(FleetOrderTask {
            fleet_id: fleet_word(0, 3),
            order_index: 0,
            value: 0x0f,
        }));
        // A value of 0x10: the nibble alone would read as "no task", but the
        // original compares the whole word, so this is refused too.
        log.records.push(LogRecord::order_task(FleetOrderTask {
            fleet_id: fleet_word(0, 3),
            order_index: 0,
            value: 0x10,
        }));
        let report = replay(&mut state, 0, &log, &mut orders);
        assert_eq!(report.rejected, 3);
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
