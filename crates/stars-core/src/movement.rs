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
