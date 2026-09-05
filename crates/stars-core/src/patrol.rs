//! The Patrol waypoint task: hunting for something to intercept.
//!
//! A patrolling fleet looks for the nearest enemy fleet within its patrol
//! range and sets a waypoint to intercept it. The original does this while it
//! writes each player's turn file (`save.c`), not while it generates the turn,
//! because what a patrol does is decided from **that player's** view of the
//! galaxy; this engine runs it at the end of a generated year, which is the
//! same moment.
//!
//! Two rules it needs come from elsewhere and are worth having on their own,
//! because combat needs them too: whether a fleet will attack a player at all
//! (`FAttackPlayer`, `10f0:ae06`) and whether a fleet counts as a target of a
//! given class (`FMatchTarget`, `1038:6612`).
//!
//! See `docs/formulas/waypoint-tasks.md`.

use crate::battle::TargetClass;
use crate::fleet::{Fleet, Waypoint};
use crate::GameState;

/// The object class of a waypoint that names a fleet.
const GROBJ_FLEET: u8 = 2;

/// How far a patrol looks, in light years, from the stored setting.
///
/// `iDist × 50 + 50`, so `0` is fifty light years and each step adds fifty —
/// except that the tenth step, which would be 550, means **as far as it takes**
/// (`save.c`). Every patrol in the fixtures is set to `0`.
#[must_use]
pub fn patrol_range(setting: u16) -> i32 {
    let range = i32::from(setting) * 50 + 50;
    if range == 550 {
        10_000
    } else {
        range
    }
}

/// Whether a fleet will attack a player at all.
///
/// `FAttackPlayer` (`10f0:ae06`) reads the "attack who" byte of the fleet's
/// battle plan: `0` nobody, `1` enemies, `2` anyone who is not a friend, `3`
/// everyone, and `4 + n` only player `n`. The relations it consults are the
/// fleet owner's own.
#[must_use]
pub fn attacks(state: &GameState, fleet: &Fleet, other: i16) -> bool {
    let Ok(owner) = usize::try_from(fleet.owner) else {
        return false;
    };
    let Some(player) = state.players.get(owner) else {
        return false;
    };
    let attack_who = player
        .battle_plans
        .get(usize::from(fleet.battle_plan))
        .map_or(0, |plan| plan.attack_who);
    let relation = usize::try_from(other)
        .ok()
        .and_then(|i| player.relations.get(i))
        .copied()
        .unwrap_or(0);
    match attack_who {
        0 => false,
        1 => relation == 2,
        2 => relation != 1,
        3 => true,
        race => i16::from(race) - 4 == other,
    }
}

/// Whether a fleet counts as a target of the given class.
///
/// `FMatchTarget` (`1038:6612`), which classifies by the **hull's** category
/// rather than by what is fitted to it: `1` is a freighter, `2..=4` are the
/// armed hulls, `5` a bomber and `7` a fuel transport.
///
/// The original takes an "exact" flag as well, and with the flag clear — which
/// is how the patrol search calls it — *anything* matches None, Any or
/// Starbase. That is transcribed rather than tidied.
#[must_use]
pub fn matches_target(state: &GameState, fleet: &Fleet, class: TargetClass) -> bool {
    let Ok(owner) = usize::try_from(fleet.owner) else {
        return false;
    };
    let designs = state.designs.get(owner);
    let category = |stack: &crate::fleet::ShipStack| -> Option<u8> {
        designs
            .and_then(|d| d.get(usize::from(stack.design)))
            .and_then(crate::design::ShipDesign::hull)
            .map(|hull| hull.category)
    };
    let any = |want: &dyn Fn(u8) -> bool| {
        fleet
            .stacks
            .iter()
            .filter(|s| s.count > 0)
            .filter_map(category)
            .any(want)
    };
    let armed = |c: u8| (2..=4).contains(&c);

    match class {
        TargetClass::ArmedShips => any(&armed),
        TargetClass::BombersFreighters => any(&|c| c == 1 || c == 5),
        TargetClass::UnarmedShips => !any(&armed),
        TargetClass::FuelTransports => any(&|c| c == 7),
        TargetClass::Freighters => any(&|c| c == 1),
        // None, Any and Starbase share the original's fall-through arm, which
        // matches whenever the exact flag is clear.
        TargetClass::None | TargetClass::Any | TargetClass::Starbase => true,
    }
}

/// Give every patrolling fleet something to chase.
///
/// From `save.c`, in order:
///
/// * a fleet whose **first** waypoint has no task but whose second patrols
///   takes up the patrol itself, settings and all;
/// * a fleet already chasing a fleet is left alone;
/// * otherwise every other player's fleet is considered, nearest first, and
///   only those the battle plan says to attack and that match its primary
///   target class. A target nobody else is already chasing is preferred to a
///   nearer one that is — one patroller per target — and the chosen target is
///   marked so the next patroller looks elsewhere;
/// * if the best is within the patrol range, a waypoint naming that fleet is
///   inserted after the first.
///
/// Returns the interceptions ordered, as `(patrolling fleet, target fleet)`.
pub fn patrol(state: &mut GameState) -> Vec<(u16, u16)> {
    let mut ordered = Vec::new();
    let mut taken: Vec<usize> = Vec::new();

    // A fleet sitting still under a patrolling second waypoint patrols too.
    for fleet in &mut state.fleets {
        let inherit = fleet
            .waypoints
            .first()
            .is_some_and(|w| w.task == stars_formats::task::NONE)
            && fleet
                .waypoints
                .get(1)
                .is_some_and(|w| w.task == stars_formats::task::PATROL);
        if inherit {
            let settings = fleet.waypoints[1].task_data.clone();
            let here = &mut fleet.waypoints[0];
            here.task = stars_formats::task::PATROL;
            here.task_data = settings;
        }
    }

    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        if fleet.waypoints.first().map(|w| w.task) != Some(stars_formats::task::PATROL) {
            continue;
        }
        // Already chasing something.
        if fleet
            .waypoints
            .get(1)
            .is_some_and(|w| w.target_class == GROBJ_FLEET)
        {
            continue;
        }
        // A fleet in deep space with repeat orders searches from where it is
        // going rather than from where it is.
        let from = if fleet.orbiting.is_none() && fleet.repeat_orders && fleet.waypoints.len() > 1 {
            fleet.waypoints[1].position
        } else {
            fleet.position
        };
        let class = state
            .players
            .get(usize::try_from(fleet.owner).unwrap_or(usize::MAX))
            .and_then(|p| p.battle_plans.get(usize::from(fleet.battle_plan)))
            .map_or(TargetClass::Any, |plan| {
                TargetClass::from_raw(plan.primary_target)
            });

        let mut best: Option<usize> = None;
        let mut best_distance = i64::MAX;
        let mut found_unmarked = false;
        for (other, target) in state.fleets.iter().enumerate() {
            if target.owner == fleet.owner || target.stacks.is_empty() {
                continue;
            }
            let marked = taken.contains(&other);
            if found_unmarked && marked {
                continue;
            }
            let dx = i64::from(target.position.x) - i64::from(from.x);
            let dy = i64::from(target.position.y) - i64::from(from.y);
            let distance = dx * dx + dy * dy;
            if best.is_some() && distance >= best_distance && (found_unmarked || marked) {
                continue;
            }
            if !matches_target(state, target, class) || !attacks(state, fleet, target.owner) {
                continue;
            }
            best = Some(other);
            best_distance = distance;
            found_unmarked = !marked;
        }

        let Some(target) = best else { continue };
        if best_distance == 0 {
            continue; // already on top of it
        }
        let setting = state.fleets[index]
            .waypoints
            .first()
            .and_then(|w| w.task_data.get(2..4))
            .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
        let range = i64::from(patrol_range(setting));
        if best_distance > range * range {
            continue;
        }

        if found_unmarked {
            taken.push(target);
        }
        let (position, id) = (state.fleets[target].position, state.fleets[target].id);
        let warp = state.fleets[index].warp.unwrap_or(0);
        let fleet = &mut state.fleets[index];
        fleet.waypoints.insert(
            1,
            Waypoint {
                position,
                target: Some(id),
                target_class: GROBJ_FLEET,
                warp,
                task: stars_formats::task::NONE,
                transport: None,
                task_data: Vec::new(),
            },
        );
        ordered.push((fleet.id, id));
    }
    ordered
}
