//! How fast a computer player sends a fleet, and how it recognises a warship.
//!
//! Source: `IFindIdealWarp` (`1090:...`), `FMoveAiFleet` (`1090:3d0a`),
//! `FIsAiAttack` (`1090:4a72`) and `FIsTurinDroneAiAttack` (`1090:4b9a`).

use crate::components::{slot, ENGINES};
use crate::design::ShipDesign;

/// The highest warp the game will consider (`IFindIdealWarp` starts here).
pub const MAX_WARP: u8 = 10;

/// Fuel use at or above this is refused outright: the search walks down from
/// warp 10 until it finds a speed costing less than this (`1090:...` compares
/// against `0x79`).
pub const FUEL_LIMIT: i16 = 121;

/// Engines that will run at warp 10 — the three top drives and the two
/// ramscoops. Every other engine is held to warp 9.
const WARP_10_ENGINES: [i16; 5] = [7, 8, 9, 14, 15];

/// The two ramscoop engines, which are exempt from the fuel-economy back-off
/// because they refuel as they fly.
const RAMSCOOPS: [i16; 2] = [14, 15];

/// The warp the AI sends a fleet at.
///
/// Source: `IFindIdealWarp`. Starting from warp 10 it walks down to the first
/// speed the engine can sustain — fuel use below [`FUEL_LIMIT`] — and then
/// applies two adjustments:
///
/// - **Fuel economy.** If the fleet still burns fuel at that speed, and it is
///   not a ramscoop, it drops back to reach a speed the engine runs free at,
///   by up to three. This is why AI fleets so often travel one or two warp
///   below what their engine could manage.
/// - **The warp 10 rule.** Only the three top drives and the two ramscoops are
///   sent at warp 10; anything else is held to 9.
///
/// A design with no engine — a starbase — gives warp 0, and the whole fleet is
/// then held at 0.
///
/// `ignore_scoops` is the routine's second argument: set, it skips the fuel
/// economy back-off entirely.
///
/// # Not verified
///
/// The corpus offers almost nothing to score this against. A fleet recorded in
/// a host file has already arrived, so its warp is clear and its orders are
/// spent; across 101 turns only **36** AI waypoint legs still carry a warp.
///
/// On those 36, this matches 3 with the back-off applied and 19 with it
/// skipped. Every disagreement in the first case is an under-prediction of one
/// or two, which is exactly what an unwanted back-off looks like. That points
/// at `fIgnoreScoops` being set at the call sites — `FColonizeAiFleet` passes a
/// second argument the decompiler renders as a planet id, so what actually
/// reaches the flag is unresolved.
///
/// 36 samples is not enough to settle it either way, and picking the mode that
/// scores better would be choosing a reading to fit 19 data points. Both modes
/// are therefore exposed and neither is asserted.
#[must_use]
pub fn ideal_warp(designs: &[(&ShipDesign, i32)], ignore_scoops: bool) -> u8 {
    let mut warp = MAX_WARP;
    for (design, count) in designs {
        if *count <= 0 {
            continue;
        }
        // The design's engine, found the way the routine does: the first slot
        // in the engine category. A design without one is a starbase, and the
        // whole fleet is then held at warp 0.
        let Some(engine_id) = design
            .slots
            .iter()
            .find(|s| s.category == slot::ENGINE)
            .map(|s| i16::from(s.item))
        else {
            return 0;
        };
        let Some(engine) = ENGINES.iter().find(|e| e.id == engine_id) else {
            continue;
        };

        while warp > 0 {
            let fuel = engine
                .fuel_used
                .get(usize::from(warp))
                .copied()
                .unwrap_or(i16::MAX);
            if fuel >= FUEL_LIMIT {
                warp -= 1;
                continue;
            }

            let ramscoop = RAMSCOOPS.contains(&engine_id);
            if fuel > 0 && !ignore_scoops && !ramscoop {
                let free_at =
                    |w: u8| -> bool { engine.fuel_used.get(usize::from(w)).copied() == Some(0) };
                if warp >= 5 && free_at(warp - 1) {
                    warp -= 1;
                } else if warp >= 6 && free_at(warp - 2) {
                    warp -= 2;
                } else if warp > 6 && free_at(warp - 3) {
                    // The decompilation reads this third test out of the ore
                    // cost array and off its end, into the fuel table that
                    // follows. With the two tests above stepping down one and
                    // two to reach a free speed, three is the only reading that
                    // fits the pattern.
                    warp -= 3;
                }
            }
            if warp == MAX_WARP && !WARP_10_ENGINES.contains(&engine_id) {
                warp = 9;
            }
            break;
        }
    }
    warp
}

/// Whether a fleet counts as a warship, by the hull of anything it carries.
///
/// Source: `FIsTurinDroneAiAttack` (`1090:4b9a`), the simpler of the two
/// tests: any design present whose hull index is 4 to 10.
#[must_use]
pub fn is_attack_fleet_simple(hulls: &[(u8, i32)]) -> bool {
    hulls
        .iter()
        .any(|(hull, count)| *count > 0 && (4..=10).contains(hull))
}

/// The hull index range every personality but Turindrone treats as a warship
/// outright (`1090:4a72`).
pub const ATTACK_HULLS: std::ops::RangeInclusive<u8> = 6..=10;

/// Hull 5 is a warship only if the design is actually armed; so are the two
/// special hulls 29 and 31, and then only when their maximum stat is under 500
/// (`WtMaxShdefStat`).
pub const CONDITIONAL_HULLS: [u8; 3] = [5, 29, 31];

#[cfg(test)]
mod tests {
    use super::*;

    /// Hull classification follows the ranges the two predicates test.
    #[test]
    fn a_warship_is_recognised_by_its_hull() {
        assert!(is_attack_fleet_simple(&[(4, 1)]));
        assert!(is_attack_fleet_simple(&[(10, 3)]));
        assert!(!is_attack_fleet_simple(&[(3, 5)]));
        assert!(!is_attack_fleet_simple(&[(11, 5)]));
        // A design with none aboard does not count.
        assert!(!is_attack_fleet_simple(&[(6, 0)]));
        assert!(ATTACK_HULLS.contains(&6) && !ATTACK_HULLS.contains(&5));
        assert!(CONDITIONAL_HULLS.contains(&5));
    }
}
