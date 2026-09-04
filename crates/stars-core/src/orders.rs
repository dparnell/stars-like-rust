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

/// Apply a run of recorded transfers, in order.
///
/// Returns how many were applied to an object this state actually holds.
pub fn apply_cargo_transfers(state: &mut GameState, records: &[CargoTransferRecord]) -> usize {
    records
        .iter()
        .filter(|r| apply_cargo_transfer(state, r).iter().any(|q| *q != 0))
        .count()
}
