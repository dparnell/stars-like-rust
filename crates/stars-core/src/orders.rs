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
                changed.push(id);
            }
            crate::ground::Outcome::Taken { player, colonists } => {
                let planet = &mut state.planets[index];
                planet.owner = Some(player);
                planet.pop = colonists;
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
                warp: 0,
                task: 0,
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
