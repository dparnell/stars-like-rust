//! Fleet movement: how far a fleet travels in a year, where it ends up, and
//! how much fuel that costs.
//!
//! Source: `MoveFleets` (`10b0:32ce`), `EstFuelUse` (`1050:9fe4`) and
//! `DGetDistance` (`1038:3fe4`) in `stars.2.7j.exe`, cross-checked against the
//! reconstructed NB09 C (`turn.c`, `ship.c`). Full derivation in
//! `docs/formulas/movement.md`.

/// A position in the universe, in light years.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: i16,
    /// Vertical coordinate.
    pub y: i16,
}

impl Point {
    /// Construct a point.
    #[must_use]
    pub fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }
}

/// Straight-line distance between two points, in light years.
///
/// The original computes this in double precision from 32-bit squared
/// differences, so it is exact and platform-independent.
#[must_use]
pub fn distance(a: Point, b: Point) -> f64 {
    let dx = f64::from(i32::from(b.x) - i32::from(a.x));
    let dy = f64::from(i32::from(b.y) - i32::from(a.y));
    (dx * dx + dy * dy).sqrt()
}

/// The distance a fleet covers in one year at a given warp factor: `warp^2`
/// light years. Warp 9 covers 81 ly a year.
#[must_use]
pub fn travel_per_year(warp: i16) -> i32 {
    i32::from(warp) * i32::from(warp)
}

/// How far the fleet actually travels toward its waypoint this year.
///
/// It covers `warp^2` unless that would overshoot the waypoint, in which case
/// it stops exactly there. `fuel_range`, when given, caps the distance at what
/// the remaining fuel allows.
///
/// The `+ 0.9999` bias reproduces the original's "close enough to arrive"
/// rounding: a fleet 81.4 ly out at warp 9 still arrives.
#[must_use]
pub fn travel_this_year(warp: i16, distance_to_waypoint: f64, fuel_range: Option<i32>) -> i32 {
    let full = travel_per_year(warp);
    let needed = (distance_to_waypoint + 0.9999) as i32;
    let mut travel = if full >= needed { needed } else { full };
    if let Some(range) = fuel_range {
        if travel > range {
            travel = range;
        }
    }
    travel
}

/// Where a fleet ends up after travelling `travel` light years from `from`
/// toward `to`.
///
/// When the fleet reaches the waypoint it lands exactly on it; otherwise the
/// position is interpolated and rounded outward along each axis, matching the
/// original's `±0.5` bias.
#[must_use]
pub fn advance(from: Point, to: Point, travel: i32) -> Point {
    let d = distance(from, to);
    if d <= 0.0001 {
        return to;
    }
    // Arrived: the caller's `travel` reached the (biased) full distance.
    if f64::from(travel) >= d - 0.99999 {
        return to;
    }

    let x_round = if to.x > from.x { 0.5 } else { -0.5 };
    let y_round = if to.y > from.y { 0.5 } else { -0.5 };
    let r = f64::from(travel) / d;

    let x =
        (f64::from(i32::from(to.x) - i32::from(from.x)) * r + x_round) as i32 + i32::from(from.x);
    let y =
        (f64::from(i32::from(to.y) - i32::from(from.y)) * r + y_round) as i32 + i32::from(from.y);
    Point {
        x: x as i16,
        y: y as i16,
    }
}

/// The fuel an engine burns at a given warp, from the component table.
///
/// A zero result means the engine runs free at that speed — which is how
/// ramscoops, and the low warps of ordinary engines, are expressed.
#[must_use]
pub fn engine_fuel_use(engine_id: i16, warp: i16) -> Option<i32> {
    let engine = crate::components::engine(engine_id)?;
    let index = usize::try_from(warp).ok()?;
    engine.fuel_used.get(index).map(|f| i32::from(*f))
}

/// One ship design's contribution to a fleet's fuel burn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuelStack {
    /// Number of ships of this design in the fleet.
    pub count: i32,
    /// Empty mass of one ship, in kT.
    pub mass_empty: i32,
    /// Cargo carried by this design's ships, in kT.
    pub cargo: i32,
    /// The engine's fuel-use figure at the chosen warp, from the engine part's
    /// `rgcFuelUsed[warp]` table (mg of fuel per kT per light year, ×10).
    pub engine_fuel_use: i32,
}

/// Fuel a fleet burns covering `distance` light years.
///
/// Each design burns `mass * engine_fuel_use * distance / 2000`, summed and
/// then divided by ten with the remainder rounded up. Improved Fuel Efficiency
/// cuts each engine's figure by 15% first.
///
/// A stack whose engine figure is `0` (a ramscoop at a free warp, or a warp
/// the engine runs for nothing) burns nothing.
#[must_use]
pub fn fuel_used(stacks: &[FuelStack], distance: i32, improved_fuel_efficiency: bool) -> i32 {
    let mut total: i64 = 0;
    for stack in stacks {
        if stack.count <= 0 {
            continue;
        }
        let mut efficiency = i64::from(stack.engine_fuel_use);
        if improved_fuel_efficiency {
            efficiency -= efficiency * 15 / 100;
        }
        let mass = i64::from(stack.cargo) + i64::from(stack.count) * i64::from(stack.mass_empty);
        let scaled = efficiency * i64::from(distance);
        if mass <= 0 || scaled <= 0 {
            continue;
        }
        total += mass * scaled / 2000;
    }
    ((total + 9) / 10) as i32
}

/// The best speed a fleet can cruise at (`IFindIdealWarp`, `1050:a76e`).
///
/// The client suggests this warp whenever it sets a leg for you, and it is the
/// slowest engine's answer to one question: **what is the fastest warp at which
/// this engine burns less than 121% fuel?** Then two adjustments, in the
/// original's order:
///
/// * if that warp costs anything at all and the engine is not a ram scoop, and
///   a **free** warp (zero fuel) lies one, two or three steps below it, drop to
///   that instead — the difference is not worth the fuel;
/// * warp 10 is only offered by the five engines that can hold it: Interspace-10,
///   the Enigma Pulsar, Trans-Star 10 and the two Mizer/Galaxy scoops. Every
///   other engine is capped at 9.
///
/// A design with no engine at all answers 0, and the fleet takes the lowest
/// answer of any design aboard.
///
/// `ignore_scoops` skips the free-warp step, which is what the original passes
/// when it wants the raw figure.
#[must_use]
pub fn ideal_warp(
    stacks: &[crate::fleet::ShipStack],
    designs: &[crate::design::ShipDesign],
    ignore_scoops: bool,
) -> i16 {
    use crate::components::{slot, ENGINES};

    /// The engines that can hold warp 10, by name — the original tests their
    /// component ids.
    const WARP_TEN: [&str; 5] = [
        "Interspace-10",
        "Enigma Pulsar",
        "Trans-Star 10",
        "Trans-Galactic Mizer Scoop",
        "Galaxy Scoop",
    ];

    let mut worst: i16 = 10;
    for stack in stacks.iter().filter(|s| s.count > 0) {
        let Some(design) = designs.get(usize::from(stack.design)) else {
            continue;
        };
        let Some(engine) = design
            .slots
            .iter()
            .find(|s| s.category & slot::ENGINE != 0)
            .and_then(|s| ENGINES.get(usize::from(s.item)))
        else {
            // A design with no engine cannot fly at all.
            return 0;
        };
        let scoop = engine.name.contains("Scoop");
        while worst > 0 {
            let index = usize::try_from(worst).unwrap_or(0);
            let burn = engine.fuel_used.get(index).copied().unwrap_or(i16::MAX);
            if burn >= 121 {
                worst -= 1;
                continue;
            }
            // Drop to a free warp just below, if there is one.
            if burn > 0 && !ignore_scoops && !scoop {
                let free = |w: i16| {
                    usize::try_from(w)
                        .ok()
                        .and_then(|i| engine.fuel_used.get(i))
                        .is_some_and(|f| *f == 0)
                };
                if worst >= 5 && free(worst - 1) {
                    worst -= 1;
                } else if worst >= 6 && free(worst - 2) {
                    worst -= 2;
                } else if worst > 6 && free(worst - 3) {
                    worst -= 3;
                }
            }
            if worst == 10 && !WARP_TEN.contains(&engine.name) {
                worst = 9;
            }
            break;
        }
    }
    worst
}

/// Slow a leg down as far as it can go without arriving any later
/// (`IWarpBestForWaypoint`, `1058:7f83`).
///
/// A leg takes `ceil(distance / warp²)` years. The original walks the warp down
/// while that figure does not change, so a fleet never burns fuel for speed
/// that buys it nothing: at 100 light years, warp 9 and warp 8 both arrive in
/// two years, so the answer is warp 8. It never goes below warp 2.
#[must_use]
pub fn settle_warp(warp: i16, distance: i32) -> i16 {
    if warp < 2 {
        return warp;
    }
    let years = |w: i16| -> i32 {
        let per_year = i32::from(w) * i32::from(w);
        (distance + per_year - 1) / per_year
    };
    let want = years(warp);
    let mut warp = warp;
    while warp > 2 && years(warp - 1) == want {
        warp -= 1;
    }
    warp
}

#[cfg(test)]
mod warp_tests {
    use super::*;

    /// A leg is slowed until slowing it further would cost a year.
    #[test]
    fn a_leg_flies_no_faster_than_it_needs_to() {
        // A hundred light years: warp 9 covers it in two years (81 a year) and
        // so does warp 8 (64 a year, 128 in two). Warp 7 makes only 98 in two
        // years and would cost a third, so the answer is warp 8.
        assert_eq!(settle_warp(9, 100), 8);
        // At 98 light years warp 7 does reach in two, and that is the answer.
        assert_eq!(settle_warp(9, 98), 7);
        // Exactly one year's flight at warp 9 stays at warp 9.
        assert_eq!(settle_warp(9, 81), 9);
        // A long haul cannot be slowed at all without costing a year.
        assert_eq!(settle_warp(9, 810), 9);
        // It never drops below warp 2.
        assert_eq!(settle_warp(9, 1), 2);
        assert_eq!(settle_warp(1, 1), 1, "and leaves warp 1 alone");
    }

    /// The cruising warp is the fastest one under 121% fuel, backed off to a
    /// free warp just below it, and capped at 9 for all but five engines.
    #[test]
    fn a_fleet_cruises_at_its_engines_best() {
        use crate::components::{slot, ENGINES};
        use crate::design::{DesignSlot, ShipDesign};
        use crate::fleet::ShipStack;

        let design = |engine: u8| ShipDesign {
            name: "test".to_string(),
            picture: 0,
            stored_armor: 0,
            obsolete: false,
            hull_id: 0,
            slots: vec![DesignSlot {
                category: slot::ENGINE,
                item: engine,
                count: 1,
            }],
        };
        let one = |n: i32| {
            vec![ShipStack {
                design: 0,
                count: n,
                damaged_pct: 0,
                damage_pct: 0,
            }]
        };

        // Every engine answers with a warp it can actually hold.
        for (index, engine) in ENGINES.iter().enumerate() {
            let Ok(item) = u8::try_from(index) else {
                continue;
            };
            let warp = ideal_warp(&one(1), &[design(item)], false);
            assert!(
                (1..=10).contains(&warp),
                "{} answered warp {warp}",
                engine.name
            );
            // Only the five warp-10 engines are ever offered warp 10.
            if warp == 10 {
                assert!(
                    matches!(
                        engine.name,
                        "Interspace-10"
                            | "Enigma Pulsar"
                            | "Trans-Star 10"
                            | "Trans-Galactic Mizer Scoop"
                            | "Galaxy Scoop"
                    ),
                    "{} should be capped at 9",
                    engine.name
                );
            }
            // And the warp it names is one it can hold under 121% fuel.
            let burn = engine.fuel_used[usize::try_from(warp).unwrap_or(0)];
            assert!(burn < 121, "{} burns {burn}% at warp {warp}", engine.name);
        }

        // A design with no engine cannot fly.
        let mut stranded = design(0);
        stranded.slots.clear();
        assert_eq!(ideal_warp(&one(1), &[stranded], false), 0);
    }
}
