//! Applying the orders a player's `.x` file records.
//!
//! The order log is not a list of intentions — it is a replay of what the
//! player's client already did, and the host re-applies it verbatim to keep the
//! two in step. This module covers the cargo transfers, which are the part that
//! moves simulation state rather than settings.
//!
//! Source: the `rtLogCargoXfer8/16/32` arm of the order-log replay and
//! `ChgCargo` (`ship.c` in the reconstructed NB09 sources). See
//! `docs/formats/cargo.md`.

use stars_formats::{CargoTransferRecord, GrobjClass};

use crate::ground::Landing;

use crate::planet::MINERALS;
use crate::GameState;

/// Cargo kinds, in the order the transfer mask indexes them.
pub const CARGO_KINDS: usize = 5;
/// Index of colonists among the cargo kinds.
pub const COLONISTS: usize = 3;
/// Index of fuel among the cargo kinds.
pub const FUEL: usize = 4;

/// Split a raw fleet id word into its owner and per-player fleet number.
///
/// The low 9 bits are the fleet number and bits 9..=12 the owner, the same
/// packing `stars_formats::FleetRecord` unpacks.
#[must_use]
pub fn split_fleet_id(word: u16) -> (i16, u16) {
    #[allow(clippy::cast_possible_wrap)] // the mask keeps this in 0..=15
    let owner = ((word >> 9) & 0x0f) as i16;
    (owner, word & 0x1ff)
}

/// One end of a transfer, resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    Planet(usize),
    Fleet(usize),
    /// Jettisoned, or an object this crate does not model (a mineral packet).
    Nowhere,
}

fn resolve(state: &GameState, class: Option<GrobjClass>, id: u16) -> Option<End> {
    match class? {
        GrobjClass::Planet => {
            let target = i16::try_from(id).ok()?;
            state
                .planets
                .iter()
                .position(|p| p.id == target)
                .map(End::Planet)
        }
        GrobjClass::Fleet => {
            let (owner, number) = split_fleet_id(id);
            state
                .fleets
                .iter()
                .position(|f| f.owner == owner && f.id == number)
                .map(End::Fleet)
        }
        // A jettison has no destination; a mineral packet is a "thing", which
        // the simulation does not carry yet. Both are "the cargo left the
        // source and this crate cannot follow it".
        GrobjClass::None | GrobjClass::Thing => Some(End::Nowhere),
    }
}

/// Change one cargo kind on one object, returning how much actually moved.
///
/// This is `ChgCargo`. It clamps twice: an object can never give more than it
/// holds, and a fleet can never take more than it has room for. A planet has no
/// capacity limit and no fuel — `ChgCargo` returns 0 outright for fuel on a
/// planet, which is why a fuel transfer to a planet silently does nothing.
fn chg_cargo(state: &mut GameState, end: End, kind: usize, mut delta: i32) -> i32 {
    if delta == 0 {
        return 0;
    }
    match end {
        End::Nowhere => 0,
        End::Planet(index) => {
            let planet = &mut state.planets[index];
            if kind == FUEL {
                return 0;
            }
            let current = if kind == COLONISTS {
                planet.pop
            } else if kind < MINERALS {
                planet.surface_min[kind]
            } else {
                return 0;
            };
            if current + delta < 0 {
                delta = -current;
            }
            if kind == COLONISTS {
                planet.pop += delta;
            } else {
                planet.surface_min[kind] += delta;
            }
            delta
        }
        End::Fleet(index) => {
            let designs = state
                .designs
                .get(usize::try_from(state.fleets[index].owner).unwrap_or(usize::MAX))
                .cloned()
                .unwrap_or_default();
            let fleet = &mut state.fleets[index];
            let current = match kind {
                FUEL => fleet.cargo.fuel,
                COLONISTS => fleet.cargo.colonists,
                k if k < MINERALS => fleet.cargo.minerals[k],
                _ => return 0,
            };
            if current + delta < 0 {
                delta = -current;
            }
            let free = if kind == FUEL {
                fleet.fuel_capacity(&designs) - fleet.cargo.fuel
            } else {
                fleet.cargo_capacity(&designs) - fleet.cargo.mass()
            };
            if free < delta {
                delta = free;
            }
            if delta == 0 {
                return 0;
            }
            match kind {
                FUEL => fleet.cargo.fuel += delta,
                COLONISTS => fleet.cargo.colonists += delta,
                k => fleet.cargo.minerals[k] += delta,
            }
            delta
        }
    }
}

/// Apply one recorded cargo transfer, returning how much of each kind moved.
///
/// A **positive** quantity means the source gains and the destination loses —
/// a fleet listed as the source with `+25` colonists has loaded 25 from the
/// planet. See [`stars_formats::CargoTransferRecord`].
///
/// The replay makes two passes over the five kinds, applying every negative
/// before every positive, so that room freed by unloading is available to
/// whatever loads afterwards. Each side is applied on the *opposite* pass from
/// the other, and if one side moves less than asked — a full hold, an empty
/// planet — the other is held to that smaller amount.
pub fn apply_cargo_transfer(
    state: &mut GameState,
    record: &CargoTransferRecord,
) -> [i32; CARGO_KINDS] {
    let Some(source) = resolve(state, record.source_class, record.source) else {
        return [0; CARGO_KINDS];
    };
    let destination =
        resolve(state, record.destination_class, record.destination).unwrap_or(End::Nowhere);

    let mut wanted = record.quantities;
    let mut moved = [0i32; CARGO_KINDS];

    for pass in 0..2 {
        for kind in 0..CARGO_KINDS {
            let quantity = wanted[kind];
            if quantity == 0 {
                continue;
            }
            let source_now = (pass == 0) == (quantity < 0);
            if source_now {
                let applied = chg_cargo(state, source, kind, quantity);
                if applied != quantity {
                    wanted[kind] = applied;
                }
                moved[kind] = applied;
            } else {
                chg_cargo(state, destination, kind, -wanted[kind]);
            }
        }
    }
    moved
}

/// Colonists a fleet has put down on a planet it does not own.
///
/// The order replay does not settle these as it goes. It accumulates them into
/// a `COLDROP` array and `DropColonists` resolves the lot afterwards, so that
/// several fleets landing on the same planet in one turn are weighed against
/// each other and against the defenders together rather than one at a time.
///
/// The replay's condition for recording one, from the `rtLogCargoXfer` arm:
///
/// ```c
/// if ((i == 3) && (cXfer != 0) && gd.fGeneratingTurn &&
///     (dstClass == grobjPlanet) && ((srcdst & 0x0F) == grobjFleet) &&
///     (rgxf[0].fl.iPlayer != rgxf[1].fl.iPlayer))
/// ```
///
/// — colonists only, from a fleet, onto a planet, and only when the fleet's
/// owner is not the planet's. Moving colonists onto your own planet is just
/// cargo. Only an **unload** counts (`cXfer <= 0`, the source losing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColonistDrop {
    /// The planet landed on.
    pub planet: i16,
    /// Whose colonists.
    pub player: i16,
    /// How many, in units of 100 as the planet stores them.
    pub colonists: i32,
}

/// Apply a run of recorded transfers, in order.
///
/// Returns how many were applied to an object this state actually holds, and
/// the colonist landings they produced, which [`resolve_colonist_drops`] then
/// settles.
pub fn apply_cargo_transfers(
    state: &mut GameState,
    records: &[CargoTransferRecord],
) -> (usize, Vec<ColonistDrop>) {
    let mut applied = 0;
    let mut drops: Vec<ColonistDrop> = Vec::new();
    for record in records {
        // Who owns each end, before the transfer changes anything.
        let landing = colonist_landing(state, record);
        let moved = apply_cargo_transfer(state, record);
        if moved.iter().any(|q| *q != 0) {
            applied += 1;
        }
        // The recorded quantity is what the *source* gained, so an unload is
        // negative and the colonists that land are its magnitude.
        if let Some((planet, player)) = landing {
            let landed = -moved[COLONISTS];
            if landed > 0 {
                match drops
                    .iter_mut()
                    .find(|d| d.planet == planet && d.player == player)
                {
                    Some(existing) => existing.colonists += landed,
                    None => drops.push(ColonistDrop {
                        planet,
                        player,
                        colonists: landed,
                    }),
                }
            }
        }
    }
    (applied, drops)
}

/// Whether this transfer is a landing, and on whose planet.
fn colonist_landing(state: &GameState, record: &CargoTransferRecord) -> Option<(i16, i16)> {
    if record.source_class != Some(GrobjClass::Fleet)
        || record.destination_class != Some(GrobjClass::Planet)
        || record.quantities[COLONISTS] >= 0
    {
        return None;
    }
    let (owner, number) = split_fleet_id(record.source);
    let fleet = state
        .fleets
        .iter()
        .find(|f| f.owner == owner && f.id == number)?;
    let planet_id = i16::try_from(record.destination).ok()?;
    let planet = state.planets.iter().find(|p| p.id == planet_id)?;
    // Onto your own planet this is cargo, not a landing.
    if planet.owner == Some(fleet.owner) {
        return None;
    }
    Some((planet_id, fleet.owner))
}

/// Settle every colonist landing, the way `DropColonists` does.
///
/// Landings are grouped by planet so that rival claims on one planet are
/// resolved together — see [`crate::ground::resolve_landings`], which holds the
/// weights and the winner rule. Returns the planets whose ownership or
/// population changed.
pub fn resolve_colonist_drops(state: &mut GameState, drops: &[ColonistDrop]) -> Vec<i16> {
    let mut planets: Vec<i16> = drops.iter().map(|d| d.planet).collect();
    planets.sort_unstable();
    planets.dedup();

    let mut changed = Vec::new();
    for id in planets {
        let Some(index) = state.planets.iter().position(|p| p.id == id) else {
            continue;
        };
        let landings: Vec<Landing> = drops
            .iter()
            .filter(|d| d.planet == id)
            .map(|d| Landing {
                player: d.player,
                colonists: d.colonists,
                prt: usize::try_from(d.player)
                    .ok()
                    .and_then(|i| state.players.get(i))
                    .and_then(|p| p.race.prt()),
            })
            .collect();
        if landings.is_empty() {
            continue;
        }
        let defender = state.planets[index].owner.and_then(|owner| {
            let prt = usize::try_from(owner)
                .ok()
                .and_then(|i| state.players.get(i))
                .and_then(|p| p.race.prt());
            (state.planets[index].pop > 0).then_some((state.planets[index].pop, prt))
        });

        match crate::ground::resolve_landings(defender, &landings) {
            crate::ground::Outcome::Nothing => {}
            crate::ground::Outcome::Settled { player, colonists } => {
                let planet = &mut state.planets[index];
                planet.owner = Some(player);
                planet.pop = colonists;
                // A planet that changes hands starts on its new owner's
                // default queue, not an empty one.
                apply_default_queue(state, index);
                changed.push(id);
                // "Your colonists now control …": `DropColonists` sends 10,
                // or 11 to an Alternate Reality race, with the planet as
                // both object and parameter.
                if let Ok(who) = usize::try_from(player) {
                    let ar = state
                        .players
                        .get(who)
                        .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Ar));
                    state.messages.push(crate::message::Message {
                        player: who,
                        id: if ar {
                            crate::message::id::COLONISTS_CONTROL_AR
                        } else {
                            crate::message::id::COLONISTS_CONTROL
                        },
                        object: id,
                        params: vec![id],
                    });
                }
            }
            crate::ground::Outcome::Taken { player, colonists } => {
                let planet = &mut state.planets[index];
                planet.owner = Some(player);
                planet.pop = colonists;
                apply_default_queue(state, index);
                changed.push(id);
            }
            crate::ground::Outcome::Held { colonists } => {
                state.planets[index].pop = colonists;
                changed.push(id);
            }
        }
    }
    changed
}

/// Give a planet the queue its new owner starts colonies with.
///
/// A player keeps a **default production queue** (`PLAYER.zpq1`), and the game
/// hands it to a planet the moment the planet becomes theirs — settled or taken
/// — along with its "no research" flag. The two racial filters are the ones
/// `template_allows` already applies to the build list: an Alternate Reality
/// race gets no planetary installation, and a Claim Adjuster no terraforming,
/// so those entries are dropped rather than queued and skipped.
///
/// A queue that filters down to nothing leaves the planet with none at all,
/// which is what the original does when its count reaches zero.
///
/// Source: the block after the ground-combat resolution in `turn2.c`, which
/// reads `zpq1.cpq` entries as `mdIdle:6, cQuan:10`.
pub fn apply_default_queue(state: &mut GameState, planet: usize) {
    let Some(owner) = state.planets.get(planet).and_then(|p| p.owner) else {
        return;
    };
    let Some(player) = usize::try_from(owner)
        .ok()
        .and_then(|i| state.players.get(i))
    else {
        return;
    };
    let prt = player.race.prt();
    let queue: Vec<crate::production::QueueItem> = player
        .default_queue
        .items
        .iter()
        .filter(|entry| crate::ground::template_allows(prt, u16::from(entry.item)))
        .map(|entry| crate::production::QueueItem {
            count: i32::from(entry.count),
            item: u16::from(entry.item),
            ship: false,
            completion: 0,
        })
        .collect();
    let no_research = player.default_queue.no_research;

    let Some(planet) = state.planets.get_mut(planet) else {
        return;
    };
    planet.no_research = no_research;
    planet.queue = queue;
}

/// Run the waypoint tasks of every fleet that has arrived somewhere.
///
/// Source: `SatisfyOrders` (`10b0:6798`), which the turn pipeline runs after
/// movement. Five of the ten tasks are performed here; remote mining runs in
/// its own pass, and the other four are recognised and left alone — see
/// `docs/formulas/waypoint-tasks.md` for what each of those still needs.
///
/// **Colonize.** The fleet puts its whole colonist load on the planet it
/// orbits. The original's own arm does nothing but validate and cancel,
/// because the *client* had already performed the transfer and logged it — but
/// this crate is both client and host, so the transfer is made here. It goes
/// through the ordinary cargo path so the landing is settled by
/// [`resolve_colonist_drops`] exactly as a hand-made transfer would be.
///
/// **Transport.** Each of the five cargo kinds carries its own instruction
/// (`ITEMACTION`: `cQuan:12, iAction:4`). `LoadAll`, `UnloadAll`, `LoadExact`,
/// `UnloadExact` and `FillPercent` are performed. `LoadDunnage`, `WaitPercent`,
/// `SetAmount` and `SetWaypoint` are **not**: their behaviour depends on parts
/// of the routine that could not be read confidently, and none of them appears
/// in this repository's fixtures.
///
/// **Merge.** The waypoint must name a *fleet* — the class nibble decides, since
/// a bare id cannot tell planet 7 from fleet 7 — belonging to the same player,
/// alive, and not this fleet. Its ships and cargo move into that fleet and this
/// one ceases to exist, which is `Merge2Fleets(dest, this, 1)` in the original:
/// the fleet **carrying** the order is the one that disappears.
///
/// **Scrap.** Each ship gives back a third of each mineral it cost to build,
/// and everything in the hold is added to that. At a planet the planet keeps
/// **80%** of the total if it has a starbase and **50%** if it does not; in
/// deep space the original leaves a salvage object behind, which this engine
/// does not model, so the minerals are simply lost. Fuel and colonists aboard
/// are not recovered either way. Transcribed from `CreateSalvage`
/// (`10f0:7ee8`); the Bleeding Edge Tech recosting it does first is not
/// modelled.
///
/// **Route.** A fleet sitting at one of its owner's planets that has a route
/// destination set is given a waypoint to that planet, which is
/// `AutoRouteFleet` (`1080:1e52`). The original picks the speed with
/// `IFindIdealWarp` and a stargate check; this uses the fleet's own warp
/// setting, so the leg is right and its speed may not be.
///
/// A task is **consumed** once it runs, which is why every waypoint in a saved
/// game that has already been reached reads `0`.
///
/// **Loading at a planet nobody owns.** `SatisfyOrders` gives a fleet
/// nothing from such a planet unless one of its owner's own fleets is there,
/// has been there all turn (`fHereAllTurn`) and carries mining robots
/// (`CMineFromLpfl`): then the load is what the robots have dug — the
/// planet's surface — and is reported as `idmHasLoadedMiningRobotsWorking`
/// rather than `idmHasLoaded`. Without such a fleet the original refuses
/// the load and says so; that refusal is not modelled here yet.
///
/// Returns the tasks performed and the colonist landings they caused.
pub fn execute_arrival_tasks(state: &mut GameState) -> (Vec<(u16, u8)>, Vec<ColonistDrop>) {
    execute_arrival_tasks_after_moving(state, &std::collections::BTreeSet::new())
}

/// [`execute_arrival_tasks`], told which fleets (by index) travelled this
/// year, which is what "here all turn" is decided by.
pub fn execute_arrival_tasks_after_moving(
    state: &mut GameState,
    travelled: &std::collections::BTreeSet<usize>,
) -> (Vec<(u16, u8)>, Vec<ColonistDrop>) {
    use stars_formats::{task, XferAction};

    let mut done = Vec::new();
    let mut drops: Vec<ColonistDrop> = Vec::new();
    // Fleets that merged away or were scrapped. They are emptied as they go and
    // swept up at the end, because removing one mid-loop would renumber the
    // rest.
    let mut scrapped: Vec<usize> = Vec::new();

    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        let Some(waypoint) = fleet.waypoints.first() else {
            continue;
        };
        let job = waypoint.task;
        if job == task::NONE {
            continue;
        }
        // Three tasks do not need a planet under the fleet, so they are settled
        // before the orbit check the rest share.
        match job {
            task::MERGE => {
                if merge_into_target(state, index) {
                    done.push((state.fleets[index].id, job));
                    scrapped.push(index);
                }
                state.fleets[index].waypoints[0].task = task::NONE;
                continue;
            }
            task::SCRAP => {
                scrap_fleet(state, index);
                done.push((state.fleets[index].id, job));
                scrapped.push(index);
                continue;
            }
            task::ROUTE => {
                if route_fleet(state, index) {
                    done.push((state.fleets[index].id, job));
                }
                state.fleets[index].waypoints[0].task = task::NONE;
                continue;
            }
            task::TRANSFER => {
                // Reported under the number it had while it was still theirs:
                // a fleet that changes hands is renumbered.
                let was = state.fleets[index].id;
                if give_fleet(state, index) {
                    done.push((was, job));
                }
                state.fleets[index].waypoints[0].task = task::NONE;
                continue;
            }
            // Everything else is somebody else's pass: remote mining and
            // laying mines run later in the year, patrol at the end of it, and
            // giving a fleet away is not modelled. None of them is cancelled
            // here, and none of them needs a planet.
            task::COLONIZE | task::TRANSPORT => {}
            _ => continue,
        }
        let Some(orbiting) = fleet.orbiting else {
            // Colonising and transporting need somewhere to do it; in deep
            // space the original reports the mistake and cancels the order.
            state.fleets[index].waypoints[0].task = task::NONE;
            continue;
        };
        let Ok(planet_id) = i16::try_from(orbiting) else {
            continue;
        };
        let owner = fleet.owner;
        let transport = waypoint.transport;

        match job {
            task::COLONIZE => {
                let carried = state.fleets[index].cargo.colonists;
                let unowned = state
                    .planets
                    .iter()
                    .chain(state.known_planets.iter())
                    .find(|p| p.id == planet_id)
                    .is_some_and(|p| p.owner.is_none());
                if carried > 0 && unowned {
                    if let Some(drop) = unload_colonists(state, index, planet_id, owner, carried) {
                        drops.push(drop);
                    }
                    done.push((state.fleets[index].id, job));
                    // The colony ship does not survive its colony: the fleet
                    // is dismantled where it lands and its minerals go down
                    // with the colonists, which is what the tutorial means by
                    // "the old fleet #3 was recycled when you colonized
                    // 90210". The player is told the way a scrapping is told
                    // — `idmHasDismantledKtMinerals...` with the fleet named
                    // by `WFromLpfl` and the tonnage — and the fleet number
                    // comes free for the next ship built.
                    dismantle_colony_fleet(state, index, planet_id);
                    scrapped.push(index);
                    continue;
                }
            }
            task::TRANSPORT => {
                if let Some(orders) = transport {
                    let mut moved = false;
                    // Fuel only changes hands at a starbase: a planet with
                    // none has no tanks, so a QuikDrop's "unload all" of the
                    // fifth kind moves nothing there rather than draining
                    // the fleet into the ground.
                    let starbase = state
                        .planets
                        .iter()
                        .find(|p| p.id == planet_id)
                        .is_some_and(|p| p.starbase);
                    // The robots whose digging a load at an unowned planet
                    // takes: another fleet of ours, here all turn, with
                    // remote miners aboard (`turn3.c`, `fMining = 2`).
                    let unowned = state
                        .planets
                        .iter()
                        .find(|p| p.id == planet_id)
                        .is_some_and(|p| p.owner.is_none());
                    let miner = if unowned {
                        usize::try_from(owner)
                            .ok()
                            .and_then(|o| state.designs.get(o))
                            .and_then(|designs| {
                                state.fleets.iter().enumerate().find_map(|(i, f)| {
                                    (i != index
                                        && f.owner == owner
                                        && f.orbiting == Some(orbiting)
                                        && !travelled.contains(&i)
                                        && crate::mining::remote_mines(designs, &f.stacks) > 0)
                                        .then_some(f.id)
                                })
                            })
                    } else {
                        None
                    };
                    for (kind, item) in orders.items.iter().enumerate() {
                        if kind == FUEL && !starbase {
                            continue;
                        }
                        let amount = match item.action {
                            XferAction::LoadAll => i32::MAX,
                            XferAction::LoadExact => i32::from(item.quantity),
                            XferAction::UnloadAll => -held(state, index, kind),
                            XferAction::UnloadExact => -i32::from(item.quantity),
                            XferAction::FillPercent => {
                                fill_to(state, index, kind, i32::from(item.quantity))
                            }
                            // Not performed: see the note above.
                            _ => 0,
                        };
                        if amount == 0 {
                            continue;
                        }
                        let went = move_cargo(state, index, planet_id, owner, kind, amount);
                        if went == 0 {
                            continue;
                        }
                        moved = true;
                        // What moved is reported: minerals are loaded and
                        // unloaded, colonists beamed up and down
                        // (`SatisfyOrders`, `10b0:6798`), with the fleet,
                        // the amount as a long, the kind, and the planet.
                        let kind_word = i16::try_from(kind).unwrap_or(0);
                        let id = if went > 0 {
                            if kind == COLONISTS {
                                crate::message::id::HAS_BEAMED_UP
                            } else if miner.is_some() {
                                crate::message::id::MINING_ROBOTS_LOADED
                            } else {
                                crate::message::id::HAS_LOADED
                            }
                        } else if kind == COLONISTS {
                            crate::message::id::HAS_BEAMED_DOWN
                        } else {
                            crate::message::id::HAS_UNLOADED
                        };
                        let fleet_id = state.fleets[index].id;
                        let [lo, hi] = crate::message::Message::long(went.abs());
                        if let Ok(player) = usize::try_from(owner) {
                            state.messages.push(crate::message::Message {
                                player,
                                id,
                                object: crate::message::fleet_object(fleet_id),
                                params: vec![
                                    fleet_id as i16,
                                    lo,
                                    hi,
                                    kind_word,
                                    // The fifth word is the class of the far
                                    // side, or the miner's number when it
                                    // is the robots' haul.
                                    match (id, miner) {
                                        (crate::message::id::MINING_ROBOTS_LOADED, Some(m)) => {
                                            m as i16
                                        }
                                        _ => i16::from(crate::fleet::grobj::PLANET),
                                    },
                                    planet_id,
                                ],
                            });
                        }
                    }
                    if moved {
                        done.push((state.fleets[index].id, job));
                    }
                }
            }
            // The match above only lets these two through.
            _ => continue,
        }
        state.fleets[index].waypoints[0].task = task::NONE;
    }

    if !scrapped.is_empty() {
        state
            .fleets
            .retain(|f| !f.stacks.iter().all(|s| s.count <= 0));
    }
    (done, drops)
}

/// Move a fleet's ships and cargo into the fleet its waypoint names.
///
/// `Merge2Fleets(dest, this, 1)` in `SatisfyOrders`: the destination must be a
/// **fleet** the same player owns, and must not be this fleet. The one carrying
/// the order is the one that goes.
///
/// Returns whether the merge happened; the source is left with no ships for the
/// caller to sweep up.
fn merge_into_target(state: &mut GameState, index: usize) -> bool {
    use crate::fleet::grobj::FLEET as GROBJ_FLEET;

    let Some(waypoint) = state.fleets[index].waypoints.first() else {
        return false;
    };
    if waypoint.target_class != GROBJ_FLEET {
        return false;
    }
    let Some(target) = waypoint.target else {
        return false;
    };
    let owner = state.fleets[index].owner;
    // The waypoint holds a full object id: the fleet number in the low nine
    // bits, the owner above it.
    let wanted = target & 0x1FF;
    let Some(destination) = state
        .fleets
        .iter()
        .position(|f| f.owner == owner && f.id & 0x1FF == wanted)
    else {
        return false;
    };
    if destination == index {
        return false;
    }

    let taken = state.fleets[index].clone();
    let into = &mut state.fleets[destination];
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
    state.fleets[index].stacks.clear();
    true
}

/// What one scrapped fleet gives back, per mineral.
///
/// A third of each ship's build cost, times the ships, plus the hold. From
/// `CreateSalvage` (`10f0:7ee8`), which truncates the third per design rather
/// than over the whole sum.
#[must_use]
pub fn scrap_value(state: &GameState, index: usize) -> [i32; MINERALS] {
    let mut recovered = [0i32; MINERALS];
    let Some(fleet) = state.fleets.get(index) else {
        return recovered;
    };
    let designs = state
        .designs
        .get(usize::try_from(fleet.owner).unwrap_or(usize::MAX));
    for stack in &fleet.stacks {
        let Some(cost) = designs
            .and_then(|d| d.get(usize::from(stack.design)))
            .and_then(crate::design::ShipDesign::cost)
        else {
            continue;
        };
        for (kind, total) in recovered.iter_mut().enumerate() {
            *total += stack.count * cost.minerals[kind] / 3;
        }
    }
    for (kind, total) in recovered.iter_mut().enumerate() {
        *total += fleet.cargo.minerals[kind];
    }
    recovered
}

/// Scrap a fleet where it stands.
///
/// The planet it orbits keeps 80% of [`scrap_value`] if it has a starbase and
/// 50% if it does not (`CreateSalvage`, `10f0:7ee8`). Scrapped in deep space
/// the original drops a salvage object, which this engine does not model, so
/// nothing is kept. The fleet is left with no ships for the caller to sweep up.
fn scrap_fleet(state: &mut GameState, index: usize) {
    let recovered = scrap_value(state, index);
    let orbiting = state.fleets[index]
        .orbiting
        .and_then(|id| i16::try_from(id).ok());
    if let Some(planet) = orbiting.and_then(|id| state.planets.iter_mut().find(|p| p.id == id)) {
        let share = if planet.starbase { 8 } else { 5 };
        for (kind, amount) in recovered.iter().enumerate() {
            planet.surface_min[kind] += amount * share / 10;
        }
    }
    state.fleets[index].stacks.clear();
    state.fleets[index].cargo = crate::fleet::Cargo::default();
}

/// Break up a fleet that has just planted a colony, and say so.
///
/// **Two thirds** of each mineral in the ships' cost goes down with the
/// colonists, truncated per mineral, and whatever minerals the hold carried
/// with it. The tutorial's own turn-3 file is the check: its Santa Maria
/// costs 27/10/26 and the message says 41kT were put down on 90210, which
/// is 18 + 6 + 17. The message is id `89`, object the planet, then the
/// fleet's name word, the tonnage as a long, and the planet again.
fn dismantle_colony_fleet(state: &mut GameState, index: usize, planet: i16) {
    let mut recovered = [0i32; MINERALS];
    {
        let fleet = &state.fleets[index];
        let designs = state
            .designs
            .get(usize::try_from(fleet.owner).unwrap_or(usize::MAX));
        for stack in &fleet.stacks {
            let Some(cost) = designs
                .and_then(|d| d.get(usize::from(stack.design)))
                .and_then(crate::design::ShipDesign::cost)
            else {
                continue;
            };
            for (kind, total) in recovered.iter_mut().enumerate() {
                *total += stack.count * (cost.minerals[kind] * 2 / 3);
            }
        }
        for (kind, total) in recovered.iter_mut().enumerate() {
            *total += fleet.cargo.minerals[kind];
        }
    }
    let total: i32 = recovered.iter().sum();
    let fleet = &state.fleets[index];
    let owner = fleet.owner;
    let id = fleet.id;
    let designs = usize::try_from(owner)
        .ok()
        .and_then(|o| state.designs.get(o))
        .map_or(&[][..], Vec::as_slice);
    let name = crate::fleet::primary_design(fleet, designs).map_or(
        crate::message::fleet_name_word(id, 0, false),
        |primary| {
            crate::message::fleet_name_word(
                id,
                u8::try_from(primary.design).unwrap_or(0),
                primary.distinct > 1,
            )
        },
    );
    if let Some(planet) = state.planets.iter_mut().find(|p| p.id == planet) {
        for (kind, amount) in recovered.iter().enumerate() {
            planet.surface_min[kind] += amount;
        }
    }
    state.fleets[index].stacks.clear();
    state.fleets[index].cargo = crate::fleet::Cargo::default();
    if let Ok(player) = usize::try_from(owner) {
        let mut params = vec![name];
        params.extend_from_slice(&crate::message::Message::long(total));
        params.push(planet);
        state.messages.push(crate::message::Message {
            player,
            id: crate::message::id::FLEET_DISMANTLED,
            object: planet,
            params,
        });
    }
}

/// Give a fleet to another player.
///
/// The arm is at `10b0:932b`, on pass 4. Four things have to be true, and the
/// original checks them in this order:
///
/// * **Who gets it** is the waypoint's `id`, but counted among the *other*
///   players: an index at or above the giver's own is shifted up by one, so
///   the number is a position in the list of everybody else. It must land on a
///   player who is in the game.
/// * **The fleet must not be carrying colonists** (`FLEET.rgwtMin[3]`,
///   `10b0:9436`). You cannot hand people over.
/// * **The receiving player must have room for the designs.** Every design the
///   fleet uses is looked for in the recipient's own list first, and only a
///   design they do not already have needs a free slot. If any design has
///   nowhere to go, the whole gift is refused.
/// * The fleet then changes hands: its stacks are renumbered to the
///   recipient's design slots, it takes the lowest fleet number they are not
///   using, and it stops where it is.
///
/// Returns whether the fleet changed hands.
fn give_fleet(state: &mut GameState, index: usize) -> bool {
    let fleet = &state.fleets[index];
    let owner = fleet.owner;
    // The waypoint names a player, numbered among everyone but the giver.
    let Some(named) = fleet.waypoints.first().and_then(|w| w.target) else {
        return false;
    };
    let mut recipient = i16::try_from(named).unwrap_or(-1);
    if recipient >= owner {
        recipient += 1;
    }
    let Ok(to) = usize::try_from(recipient) else {
        return false;
    };
    if to >= state.players.len() || recipient == owner {
        return false;
    }
    // People are not a gift, and the player is told so.
    if fleet.cargo.colonists > 0 {
        if let Ok(player) = usize::try_from(owner) {
            let id = fleet.id;
            state.messages.push(crate::message::Message {
                player,
                id: crate::message::id::GIFT_HAS_COLONISTS,
                object: crate::message::fleet_object(id),
                params: vec![i16::try_from(id).unwrap_or(0)],
            });
        }
        return false;
    }

    let Ok(from) = usize::try_from(owner) else {
        return false;
    };
    let mine = state.designs.get(from).cloned().unwrap_or_default();
    if state.designs.len() <= to {
        state.designs.resize_with(to + 1, Vec::new);
    }

    // Work out where each of the fleet's designs would live, without changing
    // anything: a design the recipient already has is reused, and the rest need
    // free slots. Ship designs live below the starbase slots.
    let limit = usize::from(crate::startup::FIRST_STARBASE_SLOT);
    let mut moved: Vec<(u8, u8)> = Vec::new();
    let mut claimed: Vec<usize> = Vec::new();
    for stack in &state.fleets[index].stacks {
        if stack.count <= 0 {
            continue;
        }
        let Some(design) = mine.get(usize::from(stack.design)) else {
            return false;
        };
        let theirs = &state.designs[to];
        let slot = theirs.iter().position(|d| d == design).or_else(|| {
            (0..limit).find(|slot| {
                !claimed.contains(slot)
                    && theirs
                        .get(*slot)
                        .is_none_or(|d| d.hull_id < 0 || d.name.is_empty())
            })
        });
        let Some(slot) = slot else {
            // No room for one of the designs: the whole gift is refused.
            return false;
        };
        claimed.push(slot);
        let Ok(slot) = u8::try_from(slot) else {
            return false;
        };
        moved.push((stack.design, slot));
    }

    // Nothing has been changed until here.
    for (old, new) in &moved {
        let design = mine[usize::from(*old)].clone();
        let theirs = &mut state.designs[to];
        if theirs.len() <= usize::from(*new) {
            theirs.resize_with(usize::from(*new) + 1, || crate::design::ShipDesign {
                name: String::new(),
                picture: 0,
                stored_armor: 0,
                obsolete: false,
                hull_id: -1,
                slots: Vec::new(),
            });
        }
        theirs[usize::from(*new)] = design;
    }

    let id = crate::turn::next_fleet_id(state, recipient);
    let fleet = &mut state.fleets[index];
    for stack in &mut fleet.stacks {
        if let Some((_, new)) = moved.iter().find(|(old, _)| *old == stack.design) {
            stack.design = *new;
        }
    }
    fleet.owner = recipient;
    fleet.id = id;
    fleet.name = None;
    fleet.battle_plan = 0;
    fleet.repeat_orders = false;
    fleet.warp = None;
    let here = fleet.position;
    let orbiting = fleet.orbiting;
    fleet.waypoints = vec![crate::fleet::Waypoint {
        position: here,
        target: orbiting,
        target_class: if orbiting.is_some() { 1 } else { 4 },
        warp: 0,
        task: stars_formats::task::NONE,
        transport: None,
        task_data: Vec::new(),
    }];
    true
}

/// Send a fleet on to the route its planet sets.
///
/// `AutoRouteFleet` (`1080:1e52`): the fleet must be sitting at one of its
/// owner's planets, that planet must have a route destination, and the
/// destination must be a planet this state knows where to find. The original
/// chooses the speed with `IFindIdealWarp`, including a stargate case; this
/// keeps the fleet's own warp setting.
///
/// Returns whether a leg was added.
fn route_fleet(state: &mut GameState, index: usize) -> bool {
    let fleet = &state.fleets[index];
    let owner = fleet.owner;
    let warp = fleet.warp.unwrap_or(0);
    let Some(here) = fleet.orbiting.and_then(|id| i16::try_from(id).ok()) else {
        return false;
    };
    let Some(destination) = state
        .planets
        .iter()
        .find(|p| p.id == here && p.owner == Some(owner))
        .and_then(|p| p.route_dest)
    else {
        return false;
    };
    if destination == here {
        return false;
    }
    let Some(position) = state
        .planets
        .iter()
        .chain(state.known_planets.iter())
        .find(|p| p.id == destination)
        .and_then(|p| p.position)
    else {
        return false;
    };
    let fleet = &mut state.fleets[index];
    fleet.waypoints.truncate(1);
    fleet.waypoints.push(crate::fleet::Waypoint {
        position,
        target: u16::try_from(destination).ok(),
        target_class: 1,
        warp,
        task: stars_formats::task::NONE,
        transport: None,
        task_data: Vec::new(),
    });
    true
}

/// What a fleet holds of one cargo kind.
fn held(state: &GameState, fleet: usize, kind: usize) -> i32 {
    let cargo = &state.fleets[fleet].cargo;
    match kind {
        FUEL => cargo.fuel,
        COLONISTS => cargo.colonists,
        k if k < MINERALS => cargo.minerals[k],
        _ => 0,
    }
}

/// How much to load to bring a hold to a given percentage of capacity.
fn fill_to(state: &GameState, fleet: usize, kind: usize, percent: i32) -> i32 {
    let Some(designs) = usize::try_from(state.fleets[fleet].owner)
        .ok()
        .and_then(|i| state.designs.get(i))
    else {
        return 0;
    };
    let capacity = if kind == FUEL {
        state.fleets[fleet].fuel_capacity(designs)
    } else {
        state.fleets[fleet].cargo_capacity(designs)
    };
    let want = capacity * percent.clamp(0, 100) / 100;
    (want - held(state, fleet, kind)).max(0)
}

/// Move cargo between a fleet and a planet, recording nothing.
///
/// `amount` is what the fleet gains, matching the order log's convention.
fn move_cargo(
    state: &mut GameState,
    fleet: usize,
    planet: i16,
    owner: i16,
    kind: usize,
    amount: i32,
) -> i32 {
    use stars_formats::{CargoTransferRecord, GrobjClass};

    // A load is what the planet has to give (`SatisfyOrders` asks
    // `ChgCargo` on the planet first), never more: the replay below settles
    // the planet's side without holding the fleet to it, which is right for
    // a logged order the client has already checked and wrong for "load all
    // available" at a bare planet.
    let amount = if amount > 0 {
        let stock = state
            .planets
            .iter()
            .find(|p| p.id == planet)
            .map_or(0, |p| match kind {
                COLONISTS => p.pop,
                FUEL => i32::MAX,
                k => p.surface_min[k],
            });
        amount.min(stock.max(0))
    } else {
        amount
    };
    if amount == 0 {
        return 0;
    }
    let owner_bits = u16::try_from(owner.max(0)).unwrap_or(0);
    let source = (owner_bits << 9) | (state.fleets[fleet].id & 0x1ff);
    let mut quantities = [0i32; CARGO_KINDS];
    quantities[kind] = amount;
    let record = CargoTransferRecord {
        source,
        destination: u16::try_from(planet).unwrap_or(0),
        source_class: Some(GrobjClass::Fleet),
        destination_class: Some(GrobjClass::Planet),
        mode: 0x12,
        selector: 1 << kind,
        quantities,
    };
    apply_cargo_transfer(state, &record)[kind]
}

/// Put a fleet's colonists on a planet, and record the landing.
fn unload_colonists(
    state: &mut GameState,
    fleet: usize,
    planet: i16,
    owner: i16,
    carried: i32,
) -> Option<ColonistDrop> {
    let moved = move_cargo(state, fleet, planet, owner, COLONISTS, -carried);
    let landed = -moved;
    (landed > 0).then_some(ColonistDrop {
        planet,
        player: owner,
        colonists: landed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet::{Cargo, Fleet, ShipStack, Waypoint};
    use crate::movement::Point;
    use crate::planet::Planet;
    use crate::race::Race;
    use crate::Player;
    use stars_formats::CargoTransferRecord;

    fn game() -> GameState {
        let mut state = GameState::new(0);
        state.players = vec![Player::new(Race::humanoid()), Player::new(Race::humanoid())];
        state.designs = vec![Vec::new(), Vec::new()];
        state
    }

    fn fleet(owner: i16, id: u16, cargo: Cargo) -> Fleet {
        Fleet {
            name: None,
            repeat_orders: false,
            direction: None,
            id,
            owner,
            position: Point::new(0, 0),
            orbiting: Some(1),
            stacks: vec![ShipStack {
                design: 0,
                count: 1,
                damaged_pct: 0,
                damage_pct: 0,
            }],
            cargo,
            battle_plan: 0,
            warp: None,
            waypoints: vec![Waypoint {
                position: Point::new(0, 0),
                target: Some(1),
                target_class: 1,
                warp: 0,
                task: 0,
                transport: None,
                task_data: Vec::new(),
            }],
        }
    }

    fn transfer(source: u16, destination: u16, colonists: i32) -> CargoTransferRecord {
        let mut quantities = [0i32; CARGO_KINDS];
        quantities[COLONISTS] = colonists;
        CargoTransferRecord {
            source,
            destination,
            source_class: Some(GrobjClass::Fleet),
            destination_class: Some(GrobjClass::Planet),
            mode: 0x12,
            selector: 1 << COLONISTS,
            quantities,
        }
    }

    /// Unloading colonists onto an unowned planet settles it.
    #[test]
    fn dropping_colonists_on_an_empty_planet_colonises_it() {
        let mut state = game();
        let mut planet = Planet::unowned(1);
        planet.pop = 0;
        state.planets = vec![planet];
        state.fleets = vec![fleet(
            0,
            3,
            Cargo {
                minerals: [0; 3],
                colonists: 25,
                fuel: 0,
            },
        )];

        // Negative: the source (the fleet) loses 25.
        let (applied, drops) = apply_cargo_transfers(&mut state, &[transfer(3, 1, -25)]);
        assert_eq!(applied, 1);
        assert_eq!(
            drops,
            vec![ColonistDrop {
                planet: 1,
                player: 0,
                colonists: 25
            }]
        );

        let changed = resolve_colonist_drops(&mut state, &drops);
        assert_eq!(changed, vec![1]);
        assert_eq!(state.planets[0].owner, Some(0));
        assert_eq!(state.planets[0].pop, 25);
        assert_eq!(state.fleets[0].cargo.colonists, 0);
    }

    /// Moving colonists onto a planet you already own is cargo, not a landing.
    #[test]
    fn unloading_onto_your_own_planet_is_not_a_landing() {
        let mut state = game();
        let mut planet = Planet::unowned(1);
        planet.owner = Some(0);
        planet.pop = 100;
        state.planets = vec![planet];
        state.fleets = vec![fleet(
            0,
            3,
            Cargo {
                minerals: [0; 3],
                colonists: 25,
                fuel: 0,
            },
        )];

        let (_, drops) = apply_cargo_transfers(&mut state, &[transfer(3, 1, -25)]);
        assert!(drops.is_empty(), "own planet: {drops:?}");
        // The colonists still arrive; they are simply added to the population.
        assert_eq!(state.planets[0].pop, 125);
    }

    /// A fleet given away changes hands, taking its designs with it.
    #[test]
    fn a_given_fleet_changes_hands() {
        use crate::design::ShipDesign;
        use stars_formats::task;

        let mut state = game();
        state.planets = vec![Planet::unowned(1)];
        let design = ShipDesign {
            name: "Scout".to_string(),
            picture: 0,
            stored_armor: 0,
            obsolete: false,
            hull_id: 4,
            slots: Vec::new(),
        };
        state.designs = vec![vec![design], Vec::new()];
        let mut mine = fleet(0, 3, Cargo::default());
        mine.waypoints[0].task = task::TRANSFER;
        // Player 1, named among "everybody but me": 0 shifts up to 1.
        mine.waypoints[0].target = Some(0);
        state.fleets = vec![mine];

        let (done, _) = execute_arrival_tasks(&mut state);
        assert_eq!(done, vec![(3, task::TRANSFER)]);
        let given = &state.fleets[0];
        assert_eq!(given.owner, 1, "player 1 has it now");
        assert_eq!(given.id, 1, "and it takes their first free number");
        // The design came with it.
        assert_eq!(state.designs[1].len(), 1);
        assert_eq!(state.designs[1][0].name, "Scout");
        assert_eq!(given.stacks[0].design, 0);
    }

    /// Colonists are not a gift, and a design with nowhere to go stops the
    /// whole thing.
    #[test]
    fn a_gift_can_be_refused() {
        use crate::design::ShipDesign;
        use stars_formats::task;

        let design = |name: &str| ShipDesign {
            name: name.to_string(),
            picture: 0,
            stored_armor: 0,
            obsolete: false,
            hull_id: 4,
            slots: Vec::new(),
        };

        // Carrying colonists.
        let mut state = game();
        state.planets = vec![Planet::unowned(1)];
        state.designs = vec![vec![design("Scout")], Vec::new()];
        let mut mine = fleet(
            0,
            3,
            Cargo {
                minerals: [0; 3],
                colonists: 10,
                fuel: 0,
            },
        );
        mine.waypoints[0].task = task::TRANSFER;
        mine.waypoints[0].target = Some(0);
        state.fleets = vec![mine];
        let (done, _) = execute_arrival_tasks(&mut state);
        assert!(done.is_empty(), "people are not a gift");
        assert_eq!(state.fleets[0].owner, 0);

        // The recipient's design list is full.
        let mut state = game();
        state.planets = vec![Planet::unowned(1)];
        let full: Vec<ShipDesign> = (0..16).map(|i| design(&format!("theirs {i}"))).collect();
        state.designs = vec![vec![design("Scout")], full];
        let mut mine = fleet(0, 3, Cargo::default());
        mine.waypoints[0].task = task::TRANSFER;
        mine.waypoints[0].target = Some(0);
        state.fleets = vec![mine];
        let (done, _) = execute_arrival_tasks(&mut state);
        assert!(done.is_empty(), "nowhere to put the design");
        assert_eq!(state.fleets[0].owner, 0);
        assert_eq!(state.designs[1].len(), 16, "and nothing was copied in");
    }

    /// A task that has nothing to do with a planet is not cancelled for want of
    /// one: laying mines and patrolling both happen in deep space.
    #[test]
    fn a_deep_space_task_survives_the_arrival_pass() {
        use stars_formats::task;

        for job in [task::LAY_MINES, task::PATROL, task::REMOTE_MINING] {
            let mut state = game();
            state.planets = vec![Planet::unowned(1)];
            let mut mine = fleet(0, 3, Cargo::default());
            mine.orbiting = None;
            mine.waypoints[0].task = job;
            state.fleets = vec![mine];

            let (done, _) = execute_arrival_tasks(&mut state);
            assert!(done.is_empty(), "task {job} is settled elsewhere");
            assert_eq!(
                state.fleets[0].waypoints[0].task, job,
                "task {job} is still there"
            );
        }
    }

    /// A Merge task moves the fleet into the one its waypoint names, and the
    /// fleet carrying the order is the one that goes.
    #[test]
    fn a_merge_task_folds_the_fleet_into_its_target() {
        use stars_formats::task;

        let mut state = game();
        state.planets = vec![Planet::unowned(1)];
        let mut mine = fleet(
            0,
            3,
            Cargo {
                minerals: [10, 0, 0],
                colonists: 5,
                fuel: 20,
            },
        );
        mine.waypoints[0].task = task::MERGE;
        mine.waypoints[0].target = Some(7);
        mine.waypoints[0].target_class = 2; // a fleet, not planet 7
        let other = fleet(0, 7, Cargo::default());
        state.fleets = vec![mine, other];

        let (done, _) = execute_arrival_tasks(&mut state);
        assert_eq!(done, vec![(3, task::MERGE)]);
        assert_eq!(state.fleets.len(), 1, "the merging fleet is gone");
        let survivor = &state.fleets[0];
        assert_eq!(survivor.id, 7);
        assert_eq!(survivor.stacks[0].count, 2);
        assert_eq!(survivor.cargo.minerals[0], 10);
        assert_eq!(survivor.cargo.colonists, 5);
        assert_eq!(survivor.cargo.fuel, 20);
    }

    /// The same waypoint pointing at a *planet* with that id is not a merge:
    /// the class nibble is what tells them apart.
    #[test]
    fn a_merge_task_needs_a_fleet_target() {
        use stars_formats::task;

        let mut state = game();
        state.planets = vec![Planet::unowned(1)];
        let mut mine = fleet(0, 3, Cargo::default());
        mine.waypoints[0].task = task::MERGE;
        mine.waypoints[0].target = Some(7);
        mine.waypoints[0].target_class = 1; // planet 7
        state.fleets = vec![mine, fleet(0, 7, Cargo::default())];

        let (done, _) = execute_arrival_tasks(&mut state);
        assert!(done.is_empty());
        assert_eq!(state.fleets.len(), 2);
        assert_eq!(state.fleets[0].waypoints[0].task, task::NONE, "order spent");
    }

    /// A Scrap task gives the planet a third of what the ships cost, and the
    /// starbase decides whether it keeps 80% of that or 50%.
    #[test]
    fn a_scrap_task_recovers_minerals() {
        use crate::design::ShipDesign;
        use stars_formats::task;

        for (starbase, expect) in [(false, 5), (true, 8)] {
            let mut state = game();
            // A scout: hull 0, no parts. Its cost is the bare hull's.
            let design = ShipDesign {
                name: "Scout".to_string(),
                picture: 0,
                stored_armor: 0,
                obsolete: false,
                hull_id: 0,
                slots: Vec::new(),
            };
            let cost = design.cost().expect("a hull cost");
            state.designs = vec![vec![design], Vec::new()];
            let mut planet = Planet::unowned(1);
            planet.owner = Some(0);
            planet.starbase = starbase;
            planet.surface_min = [0; MINERALS];
            state.planets = vec![planet];

            let mut mine = fleet(
                0,
                3,
                Cargo {
                    minerals: [30, 0, 0],
                    colonists: 0,
                    fuel: 0,
                },
            );
            mine.stacks[0].count = 3;
            mine.waypoints[0].task = task::SCRAP;
            state.fleets = vec![mine];

            let value = scrap_value(&state, 0);
            assert_eq!(
                value[0],
                3 * cost.minerals[0] / 3 + 30,
                "a third of each ship, plus the hold"
            );

            let (done, _) = execute_arrival_tasks(&mut state);
            assert_eq!(done, vec![(3, task::SCRAP)]);
            assert!(state.fleets.is_empty(), "the fleet is gone");
            for (kind, recovered) in value.iter().enumerate() {
                assert_eq!(
                    state.planets[0].surface_min[kind],
                    recovered * expect / 10,
                    "starbase {starbase}, mineral {kind}"
                );
            }
        }
    }

    /// Scrapped in deep space the minerals are lost: the original leaves a
    /// salvage object, which this engine does not model.
    #[test]
    fn scrapping_in_deep_space_keeps_nothing() {
        use stars_formats::task;

        let mut state = game();
        state.planets = vec![Planet::unowned(1)];
        let mut mine = fleet(0, 3, Cargo::default());
        mine.orbiting = None;
        mine.waypoints[0].task = task::SCRAP;
        state.fleets = vec![mine];

        let (done, _) = execute_arrival_tasks(&mut state);
        assert_eq!(done, vec![(3, task::SCRAP)]);
        assert!(state.fleets.is_empty());
        assert_eq!(state.planets[0].surface_min, [0; MINERALS]);
    }

    /// A Route task sends the fleet on to wherever its planet routes to.
    #[test]
    fn a_route_task_follows_the_planet() {
        use stars_formats::task;

        let mut state = game();
        let mut home = Planet::unowned(1);
        home.owner = Some(0);
        home.position = Some(Point::new(100, 100));
        home.route_dest = Some(4);
        let mut away = Planet::unowned(4);
        away.position = Some(Point::new(300, 400));
        state.planets = vec![home, away];

        let mut mine = fleet(0, 3, Cargo::default());
        mine.warp = Some(7);
        mine.waypoints[0].task = task::ROUTE;
        state.fleets = vec![mine];

        let (done, _) = execute_arrival_tasks(&mut state);
        assert_eq!(done, vec![(3, task::ROUTE)]);
        let leg = &state.fleets[0].waypoints[1];
        assert_eq!(leg.position, Point::new(300, 400));
        assert_eq!(leg.target, Some(4));
        assert_eq!(leg.warp, 7);
        assert_eq!(state.fleets[0].waypoints[0].task, task::NONE, "order spent");
    }

    /// A planet with no route set, or someone else's planet, routes nowhere.
    #[test]
    fn a_route_task_needs_a_route() {
        use stars_formats::task;

        let mut state = game();
        let mut home = Planet::unowned(1);
        home.owner = Some(1); // not this fleet's owner
        home.position = Some(Point::new(100, 100));
        home.route_dest = Some(4);
        state.planets = vec![home];
        let mut mine = fleet(0, 3, Cargo::default());
        mine.waypoints[0].task = task::ROUTE;
        state.fleets = vec![mine];

        let (done, _) = execute_arrival_tasks(&mut state);
        assert!(done.is_empty());
        assert_eq!(state.fleets[0].waypoints.len(), 1);
    }

    /// A Colonize task settles the planet the fleet is orbiting.
    #[test]
    fn a_colonise_task_settles_the_planet() {
        use stars_formats::task;

        let mut state = game();
        let mut planet = Planet::unowned(1);
        planet.pop = 0;
        state.planets = vec![planet];
        let mut fleet = fleet(
            0,
            3,
            Cargo {
                minerals: [0; 3],
                colonists: 40,
                fuel: 0,
            },
        );
        fleet.waypoints[0].task = task::COLONIZE;
        state.fleets = vec![fleet];

        let (done, drops) = execute_arrival_tasks(&mut state);
        assert_eq!(done, vec![(3, task::COLONIZE)]);
        assert_eq!(drops.len(), 1);
        // The colony ship does not outlive its colony: the fleet is
        // dismantled on landing and the player told so, the way a scrapping
        // is told.
        assert!(state.fleets.is_empty(), "the colony fleet was dismantled");
        assert_eq!(
            state.messages.iter().map(|m| m.id).collect::<Vec<_>>(),
            vec![crate::message::id::FLEET_DISMANTLED]
        );
        assert_eq!(state.messages[0].object, 1, "about the planet");
        assert_eq!(state.messages[0].params.len(), 4);

        resolve_colonist_drops(&mut state, &drops);
        assert_eq!(state.planets[0].owner, Some(0));
        assert_eq!(state.planets[0].pop, 40);
        assert_eq!(
            state.messages.last().map(|m| m.id),
            Some(crate::message::id::COLONISTS_CONTROL)
        );
    }

    /// Colonising is refused where it would make no sense, and the order is
    /// still consumed.
    #[test]
    fn a_colonise_task_is_refused_on_an_owned_planet() {
        use stars_formats::task;

        let mut state = game();
        let mut planet = Planet::unowned(1);
        planet.owner = Some(1);
        planet.pop = 500;
        state.planets = vec![planet];
        let mut fleet = fleet(
            0,
            3,
            Cargo {
                minerals: [0; 3],
                colonists: 40,
                fuel: 0,
            },
        );
        fleet.waypoints[0].task = task::COLONIZE;
        state.fleets = vec![fleet];

        let (done, drops) = execute_arrival_tasks(&mut state);
        assert!(done.is_empty(), "someone else's planet is not colonised");
        assert!(drops.is_empty());
        assert_eq!(
            state.fleets[0].cargo.colonists, 40,
            "the colonists stay put"
        );
        assert_eq!(state.fleets[0].waypoints[0].task, task::NONE, "order spent");
    }

    /// A Transport task unloads what it is told to.
    #[test]
    fn a_transport_task_unloads() {
        use stars_formats::{task, ItemAction, TransportTask, XferAction};

        let mut state = game();
        let mut planet = Planet::unowned(1);
        planet.owner = Some(0);
        planet.surface_min = [0, 0, 0];
        state.planets = vec![planet];
        let mut fleet = fleet(
            0,
            3,
            Cargo {
                minerals: [50, 0, 0],
                colonists: 0,
                fuel: 0,
            },
        );
        fleet.waypoints[0].task = task::TRANSPORT;
        let mut items = [ItemAction {
            quantity: 0,
            action: XferAction::None,
        }; 5];
        items[0] = ItemAction {
            quantity: 0,
            action: XferAction::UnloadAll,
        };
        fleet.waypoints[0].transport = Some(TransportTask { items });
        state.fleets = vec![fleet];

        let (done, _) = execute_arrival_tasks(&mut state);
        assert_eq!(done, vec![(3, task::TRANSPORT)]);
        assert_eq!(state.fleets[0].cargo.minerals[0], 0);
        assert_eq!(state.planets[0].surface_min[0], 50);
    }

    /// A task in deep space is cancelled rather than performed.
    #[test]
    fn a_task_with_nowhere_to_do_it_is_cancelled() {
        use stars_formats::task;

        let mut state = game();
        let mut fleet = fleet(
            0,
            3,
            Cargo {
                minerals: [0; 3],
                colonists: 40,
                fuel: 0,
            },
        );
        fleet.orbiting = None;
        fleet.waypoints[0].task = task::COLONIZE;
        state.fleets = vec![fleet];

        let (done, drops) = execute_arrival_tasks(&mut state);
        assert!(done.is_empty());
        assert!(drops.is_empty());
        assert_eq!(state.fleets[0].waypoints[0].task, task::NONE);
    }

    /// A planet can never give more than it holds — and an impossible load
    /// destroys what it gave up, which is what the original does.
    ///
    /// For a positive quantity the destination is settled on pass 0 and the
    /// source on pass 1, so the planet hands over what it has *before* the
    /// fleet discovers it has nowhere to put it. `rgcXfer[i]` is then corrected
    /// down to what the fleet could take, but the planet is never credited
    /// back. A real client never emits a transfer its own fleet cannot hold, so
    /// the quirk does not bite in practice; it is reproduced here rather than
    /// quietly repaired.
    #[test]
    fn an_impossible_load_still_costs_the_planet() {
        let mut state = game();
        let mut planet = Planet::unowned(1);
        planet.owner = Some(0);
        planet.surface_min = [10, 0, 0];
        state.planets = vec![planet];
        state.fleets = vec![fleet(0, 3, Cargo::default())];

        // Ask to load 50 ironium from a planet that has 10. The fleet has no
        // cargo capacity without a design, so nothing moves at all.
        let mut record = transfer(3, 1, 0);
        record.quantities = [50, 0, 0, 0, 0];
        record.selector = 1;
        let moved = apply_cargo_transfer(&mut state, &record);
        assert_eq!(moved[0], 0, "with no hold the fleet loads nothing");
        assert_eq!(state.fleets[0].cargo.minerals[0], 0);
        assert_eq!(
            state.planets[0].surface_min[0], 0,
            "the planet still gave up what it had, as the original does"
        );
    }
}
