//! The Transport task: what a fleet loads and unloads at its waypoint.
//!
//! Source: the `grTaskXfer` arm of `SatisfyOrders` (`10b0:686a`–`80f5`,
//! `turn3.c`), which runs **four times a year**: passes 1 and 2 before
//! the fleets move (`DoOrders(0)`), 3 and 4 after (`DoOrders(1)`). The odd
//! passes **unload**, the even ones **load**, so a fleet arriving with a
//! task that says "unload ironium, load germanium" does the first in pass
//! 3 and the second in pass 4, and one whose task was set at the waypoint
//! it already stands at does the same in passes 1 and 2 and then flies
//! off with the result.
//!
//! An unload pass always keeps the task, with each item done marked as
//! done. A load pass **consumes** the task unless something it asked for
//! could not be had yet — a load from a planet the player does not
//! control, a "wait for" percentage not reached, fuel enough for the next
//! leg not found — when the task is kept and the fleet waits: `MoveFleets`
//! does not move a fleet whose current waypoint still carries a Transport
//! task. On the last pass of the year the waits that can never be met are
//! given up on with a message.
//!
//! See `docs/formulas/waypoint-tasks.md`, *Transport*.

use crate::fleet::grobj;
use crate::message::{fleet_object, id, Message};
use crate::orders::ColonistDrop;
use crate::GameState;
use stars_formats::XferAction;

/// The four cargo kinds and fuel, as the task and the fleet index them.
const KINDS: usize = 5;
/// Colonists' index.
const COLONISTS: usize = 3;
/// Fuel's index.
const FUEL: usize = 4;

/// The Pick Pocket Scanner's index in [`crate::components::SCANNERS`]:
/// its bearer may take from another player's **fleet**
/// (`GetShdefScannerRange`, `1038:50d0`, `piSteal |= 1`).
pub const PICK_POCKET: u8 = 5;
/// The Robber Baron Scanner's: from a planet too (`piSteal = 3`).
pub const ROBBER_BARON: u8 = 0xe;

/// What one of the year's four passes is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pass(pub u8);

impl Pass {
    /// Whether this pass loads (the even passes) rather than unloads.
    #[must_use]
    pub fn loads(self) -> bool {
        (self.0 - 1) & 1 == 1
    }
    /// The last pass of the year, when the waiting stops.
    #[must_use]
    pub fn is_last(self) -> bool {
        self.0 == 4
    }
}

/// What a pass did with a fleet's task.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Whether the task stays on the waypoint for the next pass.
    pub keep: bool,
    /// Whether anything at all changed hands.
    pub acted: bool,
    /// Colonists put down on somebody else's planet, to be settled with
    /// the year's other landings.
    pub drops: Vec<ColonistDrop>,
    /// Whether the task was cancelled with a message rather than done.
    pub cancelled: bool,
}

/// The far side of a transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Planet(i16),
    Fleet(usize),
    Packet(usize),
    Space,
}

/// The stealing a fleet's scanners allow: `0` none, `1` from fleets (a
/// Pick Pocket), `3` from planets as well (a Robber Baron).
fn steal_level(state: &GameState, index: usize) -> i16 {
    let fleet = &state.fleets[index];
    let Some(designs) = usize::try_from(fleet.owner)
        .ok()
        .and_then(|o| state.designs.get(o))
    else {
        return 0;
    };
    let mut level = 0;
    for stack in fleet.stacks.iter().filter(|s| s.count > 0) {
        let Some(design) = designs.get(usize::from(stack.design)) else {
            continue;
        };
        for slot in design
            .slots
            .iter()
            .filter(|s| s.count != 0 && s.is(crate::components::slot::SCANNER))
        {
            if slot.item == ROBBER_BARON {
                level = 3;
            } else if slot.item == PICK_POCKET && level == 0 {
                level = 1;
            }
        }
    }
    level
}

/// What a side holds of one kind (`ChgCargo` with a change of zero).
fn holds(state: &GameState, target: Target, kind: usize) -> i32 {
    match target {
        Target::Planet(id) => state
            .planets
            .iter()
            .find(|p| p.id == id)
            .map_or(0, |p| match kind {
                COLONISTS => p.pop,
                FUEL => 0,
                k => p.surface_min[k],
            }),
        Target::Fleet(i) => state.fleets.get(i).map_or(0, |f| match kind {
            COLONISTS => f.cargo.colonists,
            FUEL => f.cargo.fuel,
            k => f.cargo.minerals[k],
        }),
        Target::Packet(i) => state
            .packets
            .get(i)
            .map_or(0, |p| p.minerals.get(kind).map_or(0, |m| i32::from(*m))),
        Target::Space => 0,
    }
}

/// Room left in a fleet's hold or tank (`GetCargoFree`, `GetFuelFree`).
fn room(state: &GameState, index: usize, kind: usize) -> i32 {
    let fleet = &state.fleets[index];
    let designs = usize::try_from(fleet.owner)
        .ok()
        .and_then(|o| state.designs.get(o))
        .cloned()
        .unwrap_or_default();
    if kind == FUEL {
        fleet.fuel_capacity(&designs) - fleet.cargo.fuel
    } else {
        fleet.cargo_capacity(&designs) - fleet.cargo.mass()
    }
}

/// `ChgCargo` (`1050:…`): change what a side holds of one kind by
/// `delta`, as far as it can — a loss no further than what it holds, a
/// gain on a fleet no further than its room, on a packet no further than
/// its capacity, and none at all of fuel on a planet or of anything but
/// minerals on a packet. Returns the change made.
fn change(state: &mut GameState, target: Target, kind: usize, delta: i32) -> i32 {
    if delta == 0 {
        return 0;
    }
    match target {
        Target::Planet(id) => {
            if kind == FUEL {
                return 0;
            }
            let Some(planet) = state.planets.iter_mut().find(|p| p.id == id) else {
                return 0;
            };
            let slot = if kind == COLONISTS {
                &mut planet.pop
            } else {
                &mut planet.surface_min[kind]
            };
            let applied = delta.max(-*slot);
            *slot += applied;
            applied
        }
        Target::Fleet(i) => {
            let free = room(state, i, kind);
            let Some(fleet) = state.fleets.get_mut(i) else {
                return 0;
            };
            let slot = match kind {
                COLONISTS => &mut fleet.cargo.colonists,
                FUEL => &mut fleet.cargo.fuel,
                k => &mut fleet.cargo.minerals[k],
            };
            let applied = delta.max(-*slot).min(free.max(0));
            *slot += applied;
            applied
        }
        Target::Packet(i) => {
            if kind >= COLONISTS {
                return 0;
            }
            let Some(packet) = state.packets.get_mut(i) else {
                return 0;
            };
            let capacity = crate::orders::packet_capacity(packet);
            let carried: i32 = packet.minerals.iter().map(|m| i32::from(*m)).sum();
            let free = (capacity - carried).max(0);
            let held = i32::from(packet.minerals[kind]);
            let applied = delta.max(-held).min(free);
            packet.minerals[kind] = i16::try_from(held + applied).unwrap_or(i16::MAX);
            applied
        }
        Target::Space => 0,
    }
}

/// Run one pass of a fleet's Transport task.
///
/// `here_all_turn` names the fleets that did not travel this year, which
/// decides whether another fleet's mining robots or tanks are at the
/// fleet's service. Returns nothing when the fleet has no Transport task
/// at its current waypoint.
#[allow(clippy::too_many_lines)]
pub fn run(
    state: &mut GameState,
    index: usize,
    pass: Pass,
    here_all_turn: &dyn Fn(&crate::fleet::Fleet) -> bool,
) -> Option<Outcome> {
    let fleet = state.fleets.get(index)?;
    let waypoint = fleet.waypoints.first()?;
    if waypoint.task != stars_formats::task::TRANSPORT {
        return None;
    }
    let mut items = waypoint.transport?.items;
    let owner = fleet.owner;
    let owner_index = usize::try_from(owner).ok()?;
    let fleet_id = fleet.id;
    let fleet_param = fleet_id as i16;
    let target_class = waypoint.target_class;
    let target_id = waypoint.target;
    let target_point = waypoint.position;
    let designs = state.designs.get(owner_index).cloned().unwrap_or_default();
    let ife = state
        .players
        .get(owner_index)
        .is_some_and(|p| p.race.has_lrt(crate::race::lrt::IFE));
    let alternate = state
        .players
        .get(owner_index)
        .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Ar));

    let mut out = Outcome {
        keep: true,
        ..Outcome::default()
    };
    let tell = |state: &mut GameState, which: u16, params: Vec<i16>| {
        state.messages.push(Message {
            player: owner_index,
            id: which,
            object: fleet_object(fleet_id),
            params,
        });
    };

    // Where the fleet stands.
    let planet_here = fleet
        .orbiting
        .and_then(|id| i16::try_from(id).ok())
        .and_then(|id| state.planets.iter().position(|p| p.id == id));
    let planet_id = planet_here.map(|i| state.planets[i].id);
    let planet_owner = planet_here.and_then(|i| state.planets[i].owner);

    // The far side, and what the fleet may take from it: `iSteal` is 3 at
    // a planet of the player's own, the scanners' allowance elsewhere.
    let mut target = Target::Space;
    let mut place = (target_point.x, target_point.y);
    let mut steal: i16 = 1;
    let mut stealing = false;
    let mut mining: u8 = 0;
    let mut fueling = false;
    let mut miner: Option<usize> = None;
    match target_class {
        grobj::PLANET => {
            let pid = planet_id?;
            target = Target::Planet(pid);
            place = (-1, pid);
            steal = if planet_owner == Some(owner) { 3 } else { 0 };
            if steal == 0 {
                steal = steal_level(state, index);
                if steal == 1 {
                    steal = 0;
                }
                if steal == 0 {
                    if planet_owner.is_none() {
                        // Another fleet of ours, here all turn, with
                        // mining robots: the load is what they dug.
                        let robots = state.fleets.iter().enumerate().find(|(i, f)| {
                            *i != index
                                && f.owner == owner
                                && !f.is_empty()
                                && f.orbiting == fleet.orbiting
                                && here_all_turn(f)
                                && crate::mining::remote_mines(&designs, &f.stacks) > 0
                        });
                        if let Some((i, _)) = robots {
                            mining = 2;
                            steal = 1;
                            miner = Some(i);
                            target = Target::Fleet(i);
                            place = (-1, (state.fleets[i].id | 0x8000) as i16);
                        }
                    }
                } else {
                    stealing = true;
                }
            }
        }
        grobj::FLEET => {
            let word = target_id?;
            let (their_owner, number) = crate::orders::split_fleet_id(word);
            let far = state
                .fleets
                .iter()
                .position(|f| f.owner == their_owner && f.id == number && !f.is_empty());
            let Some(far) = far else {
                // Nothing of the sort here: the original walks into a
                // null fleet; this engine gives the order up.
                out.keep = false;
                out.cancelled = true;
                return Some(out);
            };
            target = Target::Fleet(far);
            place = (-1, (number | 0x8000) as i16);
            let theirs = state.fleets[far].owner != owner;
            steal = i16::from(!theirs);
            if theirs {
                steal = steal_level(state, index);
                if steal != 0 {
                    stealing = true;
                }
            } else if here_all_turn(&state.fleets[far]) && planet_here.is_some() {
                let robots = crate::mining::remote_mines(&designs, &state.fleets[far].stacks);
                let unowned_or_ours = planet_owner.is_none() || planet_owner == Some(owner);
                if robots < 1 || !unowned_or_ours {
                    let hold = state.fleets[far].cargo_capacity(&designs);
                    if hold == 0 && planet_owner == Some(owner) {
                        fueling = true;
                    }
                } else {
                    mining = 1;
                    miner = Some(far);
                }
            }
        }
        grobj::THING => {
            let word = target_id?;
            let Some(i) = state
                .packets
                .iter()
                .position(|p| crate::orders::packet_word(p) == word && p.position == target_point)
            else {
                // Not a packet, or not here: "a futile pursuit".
                tell(
                    state,
                    id::TRANSFER_FUTILE,
                    vec![fleet_param, (word >> 13) as i16],
                );
                out.keep = false;
                out.cancelled = true;
                return Some(out);
            };
            target = Target::Packet(i);
            place = (-2, target_point.y);
        }
        _ => {}
    }
    let planet_target = Target::Planet(planet_id.unwrap_or(-1));
    let far_fleet_id = match target {
        Target::Fleet(i) => Some(state.fleets[i].id),
        _ => None,
    };

    let loading = pass.loads();
    let mut done = true;
    let mut all_ok = true;
    let mut dunnage: u8 = 0;
    let mut opt_fuel = false;
    let mut optimal_fuel_loaded: i32 = 0;
    // A fleet that may not take and may not give ends the year; the
    // original cancels with a message and this closure says which.
    let mut cancel = false;

    'dunnage: loop {
        #[allow(clippy::needless_range_loop)]
        for kind in 0..KINDS {
            let item = items[kind];
            let code = item.action;
            let quantity = i32::from(item.quantity);
            if code == XferAction::None
                || (dunnage == 2 && code != XferAction::LoadDunnage)
                || (kind == FUEL && !matches!(target, Target::Fleet(_) | Target::Space))
                || (kind > 2 && matches!(target, Target::Packet(_)))
            {
                continue;
            }
            // What the far side has of this kind.
            let available = match target {
                Target::Planet(_) => {
                    if kind < FUEL {
                        holds(state, target, kind)
                    } else {
                        0
                    }
                }
                Target::Fleet(_) => {
                    if (mining != 0 || fueling) && kind != FUEL {
                        holds(state, planet_target, kind)
                    } else {
                        holds(state, target, kind)
                    }
                }
                Target::Packet(_) => {
                    if kind < COLONISTS {
                        holds(state, target, kind)
                    } else {
                        0
                    }
                }
                Target::Space => 0,
            };
            let held = holds(state, Target::Fleet(index), kind);

            // How much the item asks for.
            let asked = match code {
                XferAction::LoadAll => {
                    if !loading {
                        continue;
                    }
                    available
                }
                XferAction::LoadDunnage => available,
                XferAction::UnloadAll => {
                    if loading {
                        continue;
                    }
                    held
                }
                XferAction::LoadExact => {
                    if !loading {
                        continue;
                    }
                    quantity
                }
                XferAction::UnloadExact => {
                    if loading {
                        continue;
                    }
                    quantity
                }
                XferAction::SetAmount | XferAction::SetWaypoint => quantity,
                XferAction::FillPercent | XferAction::WaitPercent => {
                    if !loading {
                        continue;
                    }
                    let fleet = &state.fleets[index];
                    let capacity = if kind == FUEL {
                        fleet.fuel_capacity(&designs)
                    } else {
                        fleet.cargo_capacity(&designs)
                    }
                    .clamp(0, 2_000_000);
                    let want = if capacity < 0x10000 {
                        capacity * quantity / 100
                    } else {
                        capacity / 100 * quantity
                    };
                    (want - held).max(0)
                }
                XferAction::None | XferAction::Other(_) => continue,
            };

            // Which way it goes.
            enum Way {
                Load(i32),
                Unload(i32),
                Skip,
            }
            let way = match code {
                XferAction::LoadAll
                | XferAction::LoadExact
                | XferAction::FillPercent
                | XferAction::WaitPercent => Way::Load(asked),
                XferAction::UnloadAll | XferAction::UnloadExact => Way::Unload(asked),
                XferAction::SetAmount => {
                    let delta = asked - held;
                    if delta < 0 {
                        if loading {
                            Way::Skip
                        } else {
                            Way::Unload(-delta)
                        }
                    } else if loading {
                        // The far side must be able to provide it; the
                        // original's own test, kept as it stands.
                        let short = available <= delta && {
                            if available < delta {
                                true
                            } else {
                                done = false;
                                pass.is_last()
                            }
                        };
                        if short {
                            let which = if kind == COLONISTS {
                                id::SET_NUMBER_SHORT
                            } else {
                                id::SET_AMOUNT_SHORT
                            };
                            #[allow(clippy::cast_possible_truncation)]
                            tell(
                                state,
                                which,
                                vec![
                                    fleet_param,
                                    kind as i16,
                                    quantity as i16,
                                    0,
                                    place.0,
                                    place.1,
                                ],
                            );
                        }
                        Way::Load(delta)
                    } else {
                        Way::Skip
                    }
                }
                XferAction::SetWaypoint => {
                    let delta = available - asked;
                    if delta <= 0 {
                        if loading {
                            Way::Skip
                        } else {
                            Way::Unload((-delta).min(held))
                        }
                    } else if loading {
                        Way::Load(delta)
                    } else {
                        Way::Skip
                    }
                }
                XferAction::LoadDunnage => {
                    if kind == FUEL {
                        optimal_fuel_loaded = 0;
                        opt_fuel = true;
                        Way::Skip
                    } else if loading {
                        if dunnage > 1 && available != 0 {
                            Way::Load(available)
                        } else {
                            dunnage = 1;
                            Way::Skip
                        }
                    } else {
                        Way::Skip
                    }
                }
                XferAction::None | XferAction::Other(_) => Way::Skip,
            };

            match way {
                Way::Skip => {}
                Way::Load(mut amount) => {
                    if !loading || amount == 0 {
                        continue;
                    }
                    amount = amount.min(room(state, index, kind));
                    if steal == 0 || matches!(target, Target::Space) {
                        if kind == FUEL && opt_fuel {
                            // The optimal-fuel item speaks for the tank.
                        } else if pass.is_last() {
                            let which = match target {
                                Target::Planet(_) => id::LOAD_NOT_YOUR_PLANET,
                                Target::Fleet(_) => id::LOAD_NOT_YOUR_FLEET,
                                _ => id::LOAD_FROM_SPACE,
                            };
                            #[allow(clippy::cast_possible_truncation)]
                            tell(state, which, vec![fleet_param, kind as i16]);
                            cancel = true;
                            break 'dunnage;
                        } else {
                            done = false;
                        }
                        continue;
                    }
                    if stealing && (kind == COLONISTS || kind == FUEL) {
                        done = true;
                    }
                    if amount == 0 {
                        if code == XferAction::WaitPercent {
                            if kind == FUEL {
                                done = false;
                            } else {
                                all_ok = false;
                            }
                        }
                        continue;
                    }
                    let wanted = available.min(amount);
                    let mut got = 0;
                    let from_far = -change(state, target, kind, -wanted);
                    if from_far != 0 {
                        got = change(state, Target::Fleet(index), kind, from_far);
                        if got != 0 {
                            out.acted = true;
                            let [lo, hi] = Message::long(got);
                            #[allow(clippy::cast_possible_truncation)]
                            if far_fleet_id.is_some() && stealing {
                                tell(
                                    state,
                                    id::HAS_STOLEN,
                                    vec![fleet_param, lo, hi, kind as i16, place.1 & 0x7fff],
                                );
                            } else {
                                let which = if kind == COLONISTS {
                                    id::HAS_BEAMED_UP
                                } else {
                                    id::HAS_LOADED
                                };
                                tell(
                                    state,
                                    which,
                                    vec![fleet_param, lo, hi, kind as i16, place.0, place.1],
                                );
                            }
                        }
                    }
                    // A tanker or a miner in the way: the rest comes off
                    // the planet.
                    if (fueling || mining != 0) && amount != from_far {
                        let rest = amount - from_far;
                        let off_planet = -change(state, planet_target, kind, -rest);
                        if off_planet != 0 {
                            got = change(state, Target::Fleet(index), kind, off_planet);
                            out.acted = true;
                            let [lo, hi] = Message::long(got);
                            let pid = planet_id.unwrap_or(-1);
                            #[allow(clippy::cast_possible_truncation)]
                            if mining == 0 {
                                let which = if kind == COLONISTS {
                                    id::HAS_BEAMED_UP
                                } else {
                                    id::HAS_LOADED
                                };
                                tell(
                                    state,
                                    which,
                                    vec![fleet_param, lo, hi, kind as i16, -1, pid],
                                );
                            } else {
                                let robots = miner.map_or(0, |m| state.fleets[m].id as i16);
                                tell(
                                    state,
                                    id::MINING_ROBOTS_LOADED,
                                    vec![fleet_param, lo, hi, kind as i16, robots, pid],
                                );
                            }
                        } else {
                            got = 0;
                        }
                    }
                    if amount != got && code == XferAction::WaitPercent {
                        all_ok = false;
                    }
                    if got != 0 && code == XferAction::LoadDunnage && kind == FUEL {
                        optimal_fuel_loaded = got;
                    }
                }
                Way::Unload(mut amount) => {
                    amount = amount.min(held);
                    if loading {
                        continue;
                    }
                    // Colonists onto a planet that is not the player's are
                    // a landing, or refused.
                    if kind == COLONISTS
                        && matches!(target, Target::Planet(_))
                        && planet_owner != Some(owner)
                    {
                        let planet = planet_here.map(|i| &state.planets[i]);
                        // `fWasInhabited` is set at the top of `DoOrders`
                        // from the owner then; nobody loses a planet
                        // between there and here, so it is the owner now.
                        let refusal = if planet.is_some_and(|p| p.owner.is_none()) {
                            Some(id::BEAM_DOWN_UNINHABITED)
                        } else if alternate {
                            Some(id::BEAM_DOWN_OVERRULED)
                        } else if planet.is_some_and(|p| p.starbase) {
                            Some(id::BEAM_DOWN_STARBASE)
                        } else {
                            None
                        };
                        if let Some(which) = refusal {
                            tell(state, which, vec![fleet_param, planet_id.unwrap_or(-1)]);
                            cancel = true;
                            break 'dunnage;
                        }
                        if amount > 0 {
                            let amount = amount.max(1);
                            change(state, Target::Fleet(index), kind, -amount);
                            out.acted = true;
                            out.drops.push(ColonistDrop {
                                planet: planet_id.unwrap_or(-1),
                                player: owner,
                                colonists: amount,
                                can_colonize: true,
                            });
                        }
                        items[kind].action = XferAction::None;
                        continue;
                    }
                    if kind == COLONISTS {
                        if let Target::Fleet(far) = target {
                            if state.fleets[far].owner != owner {
                                tell(state, id::COLONISTS_TO_ANOTHER, vec![fleet_param, 0]);
                                cancel = true;
                                break 'dunnage;
                            }
                        }
                        if matches!(target, Target::Space) {
                            tell(state, id::BEAM_DOWN_SPACE, vec![fleet_param, 0]);
                            cancel = true;
                            break 'dunnage;
                        }
                    }
                    if let Target::Fleet(far) = target {
                        // A fleet whose owner counts us an enemy takes
                        // nothing.
                        let far_owner = usize::try_from(state.fleets[far].owner).ok();
                        let snubbed = far_owner.is_some_and(|fo| {
                            fo != owner_index
                                && crate::relations::regard(state, fo, owner_index)
                                    == crate::relations::Relation::Enemy
                        });
                        if snubbed {
                            amount = 0;
                        }
                    }
                    if amount != 0 {
                        let (given, pl) = if !fueling || kind == FUEL {
                            (change(state, target, kind, amount), place)
                        } else {
                            (
                                change(state, planet_target, kind, amount),
                                (-1, planet_id.unwrap_or(-1)),
                            )
                        };
                        if given > 0 {
                            let which = if kind == COLONISTS {
                                id::HAS_BEAMED_DOWN
                            } else {
                                id::HAS_UNLOADED
                            };
                            let [lo, hi] = Message::long(given);
                            #[allow(clippy::cast_possible_truncation)]
                            tell(
                                state,
                                which,
                                vec![fleet_param, lo, hi, kind as i16, pl.0, pl.1],
                            );
                        }
                        amount = given;
                    }
                    if amount != 0 {
                        change(state, Target::Fleet(index), kind, -amount);
                        out.acted = true;
                    }
                    items[kind].action = XferAction::None;
                }
            }
        }

        // The "load optimal fuel" item, settled once the rest is aboard:
        // with nowhere to go, all the fuel goes back; else the next leg's
        // need is found and the excess given back, or the shortfall
        // reported and waited for.
        if opt_fuel && loading && dunnage != 1 {
            let fleet = &state.fleets[index];
            let leg = fleet.waypoints.get(1).map(|w| {
                #[allow(clippy::cast_possible_truncation)]
                let d = crate::movement::distance(fleet.position, w.position).ceil() as i32;
                (w.warp, d)
            });
            let give_back = |state: &mut GameState, excess: i32| -> i32 {
                if excess == 0 {
                    return 0;
                }
                let taken = change(state, target, FUEL, excess);
                if taken != 0 {
                    change(state, Target::Fleet(index), FUEL, -taken)
                } else {
                    0
                }
            };
            match leg {
                None => {
                    let held = state.fleets[index].cargo.fuel;
                    let moved = give_back(state, held);
                    if moved != 0 {
                        out.acted = true;
                        let net = moved + optimal_fuel_loaded;
                        let [lo, hi] = Message::long(net.abs());
                        let which = if net < 0 {
                            id::HAS_UNLOADED
                        } else {
                            id::HAS_LOADED
                        };
                        tell(
                            state,
                            which,
                            vec![fleet_param, lo, hi, FUEL as i16, place.0, place.1],
                        );
                    }
                }
                Some((warp, distance)) => {
                    let need = state.fleets[index].fuel_use(&designs, warp, distance, ife);
                    let held = state.fleets[index].cargo.fuel;
                    if held < need {
                        done = false;
                        if optimal_fuel_loaded != 0 {
                            let [lo, hi] = Message::long(optimal_fuel_loaded);
                            tell(
                                state,
                                id::HAS_LOADED,
                                vec![fleet_param, lo, hi, FUEL as i16, place.0, place.1],
                            );
                        }
                        if pass.is_last() && !stealing {
                            tell(
                                state,
                                id::FUEL_LOAD_FAILED,
                                vec![fleet_param, place.0, place.1],
                            );
                        } else if pass.0 == 2 {
                            let capacity = state.fleets[index].fuel_capacity(&designs);
                            #[allow(clippy::cast_possible_truncation)]
                            if capacity < need {
                                tell(
                                    state,
                                    id::FUEL_NEVER_ENOUGH,
                                    vec![fleet_param, capacity as i16, need as i16],
                                );
                            } else {
                                tell(
                                    state,
                                    id::FUEL_NOT_AVAILABLE,
                                    vec![place.0, place.1, fleet_param, (need - held) as i16],
                                );
                            }
                        }
                        break 'dunnage;
                    }
                    // The least that still makes the leg: the need shrinks
                    // as the tank lightens, so it is asked again until it
                    // settles.
                    let mut need = need;
                    loop {
                        let fuel = need;
                        let f = &mut state.fleets[index];
                        f.cargo.fuel = fuel;
                        need = f.fuel_use(&designs, warp, distance, ife);
                        if need >= fuel {
                            break;
                        }
                    }
                    state.fleets[index].cargo.fuel = held;
                    let excess = held - need;
                    let moved = give_back(state, excess);
                    if moved != 0 {
                        out.acted = true;
                        let net = moved + optimal_fuel_loaded;
                        let [lo, hi] = Message::long(net.abs());
                        let which = if net < 0 {
                            id::HAS_UNLOADED
                        } else {
                            id::HAS_LOADED
                        };
                        tell(
                            state,
                            which,
                            vec![fleet_param, lo, hi, FUEL as i16, place.0, place.1],
                        );
                    }
                }
            }
        }

        if !all_ok && room(state, index, 0) > 0 {
            done = false;
        }
        // Dunnage: a second round for what is left over, once everything
        // else has loaded and there is still room.
        if done && dunnage == 1 && (room(state, index, 0) >= 1 || opt_fuel) {
            dunnage = 2;
            continue 'dunnage;
        }
        break;
    }

    if cancel {
        out.keep = false;
        out.cancelled = true;
        return Some(out);
    }
    // An unload pass keeps the task with its done items marked; a load
    // pass keeps it only while something is still waited for.
    if !done || !loading {
        if let Some(t) = state.fleets[index]
            .waypoints
            .first_mut()
            .and_then(|w| w.transport.as_mut())
        {
            t.items = items;
        }
        out.keep = true;
    } else {
        out.keep = false;
    }
    Some(out)
}
