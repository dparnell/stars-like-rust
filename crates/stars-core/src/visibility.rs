//! What a player can see this year.
//!
//! The host decides what goes into each player's file with two passes,
//! `SetVisPFPlanets` (`1070:abde`) and `SetVisPFFleets` (`1070:a100`): the
//! first walks the player's **planets** and what their scanners reach, the
//! second the player's **fleets** and theirs. Between them they settle the
//! fog of war — which planets a player has a record of and which fleets of
//! other players are on their map — and what they say is not what one
//! might guess:
//!
//! * a fleet is seen within a scanner's **normal** range, but a fleet **in
//!   orbit** only within its **penetrating** range;
//! * a **planet** is learned only within **penetrating** range — the loop
//!   over planets sits inside `if (penetrating > 0)` in both passes — or
//!   by a fleet of the player's being **at** it;
//! * cloaking shrinks the range a fleet is seen at, and a cloaked starbase
//!   shrinks the range its planet is learned at.
//!
//! The tutorial's own files show the rule at work. Its player is a Jack of
//! All Trades, whose Scout hull carries a built-in scanner — a fixed twenty
//! light years penetrating in the tutorial (`design.rs`): the Armed Probe
//! learns Hiho when it comes within seventeen light years, in 2403, and not
//! from Prune, forty-three away, the year before, nor is Prune itself
//! learned from twenty-four light years out in 2401, which is why the
//! "found a planet" messages come in 2402 when the scouts arrive; and the
//! home world's Scoper 150, which does not penetrate, learns nothing at all
//! — which is why `tutorial.h1` knows five planets in 2403 and not the
//! dozen within 150 light years of Stove Top.
//!
//! What a player has **ever** seen is kept by the client, in its history
//! file; this module only answers for one year. **Cloaking** is the
//! original's (`SetVisPFFleets`, `1070:a100`): a fleet's cloak
//! ([`Fleet::cloak_pct`]), cut by the scanning fleet's Tachyon Detectors,
//! shrinks the range it is seen at to `range × (100 − cloak) / 100`, and a
//! planet's cloaked starbase shrinks the range the planet is learned at the
//! same way. The ranges of stargates and space objects that the two passes
//! also settle are not modelled.

use std::collections::BTreeSet;

use crate::fleet::Fleet;
use crate::movement::Point;
use crate::planet::Planet;
use crate::race::{lrt, Prt};
use crate::scanning::ScannerRange;
use crate::GameState;

/// What one player can see this year.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct View {
    /// The planets the player has a record of this year: their own, and
    /// every one within penetrating range or with one of their fleets at it.
    pub planets: BTreeSet<i16>,
    /// The fleets on the player's map, as indices into
    /// [`GameState::fleets`]: their own, and every other player's within
    /// range.
    pub fleets: BTreeSet<usize>,
}

/// Whether a point is within `range` light years of `at` — the original's
/// test, a bounding-box check and then the squares, with no square root.
fn within(at: Point, p: Point, range: i32) -> bool {
    within_cloaked(at, p, range, 0)
}

/// [`within`], against a fleet cloaked `cloak` percent: past the plain
/// test the squared range is cut to `range² × (100 − cloak) / 100 ×
/// (100 − cloak) / 100`, the two divisions in that order (`1070:a1f6`).
fn within_cloaked(at: Point, p: Point, range: i32, cloak: i32) -> bool {
    let Some((d2, range2)) = squares_within(at, p, range) else {
        return false;
    };
    if cloak <= 0 {
        return true;
    }
    let left = i64::from(100 - cloak);
    d2 <= range2 * left / 100 * left / 100
}

/// [`within`], against a planet whose starbase is cloaked `cloak` percent:
/// the turn caches `(100 − cloak)²` on the starbase design (`10b0:120d`)
/// and the pass hides the planet when `range² × that / 10,000 < d²`
/// (`1070:ab3c`).
fn within_starbase_cloak(at: Point, p: Point, range: i32, cloak: i32) -> bool {
    let Some((d2, range2)) = squares_within(at, p, range) else {
        return false;
    };
    if cloak <= 0 {
        return true;
    }
    let left = i64::from(100 - cloak);
    d2 <= range2 * left * left / 10_000
}

/// The squared distance and squared range when the point is within the
/// plain range, else nothing.
fn squares_within(at: Point, p: Point, range: i32) -> Option<(i64, i64)> {
    let dx = i64::from(p.x) - i64::from(at.x);
    let dy = i64::from(p.y) - i64::from(at.y);
    if dx.abs() > i64::from(range) || dy.abs() > i64::from(range) {
        return None;
    }
    let d2 = dx * dx + dy * dy;
    let range2 = i64::from(range) * i64::from(range);
    (d2 <= range2).then_some((d2, range2))
}

/// The cloaking a fleet shows to a scanner whose detectors leave
/// `tachyon` percent of it: [`Fleet::cloak_pct`] scaled, as
/// `SetVisPFFleets` does at `1070:a1d0`.
fn fleet_cloak(state: &GameState, fleet: &Fleet, tachyon: i32) -> i32 {
    let Some(owner) = usize::try_from(fleet.owner).ok() else {
        return 0;
    };
    let (Some(designs), Some(player)) = (state.designs.get(owner), state.players.get(owner)) else {
        return 0;
    };
    let pct = fleet.cloak_pct(designs, &player.race);
    if tachyon == 100 {
        pct
    } else {
        pct * tachyon / 100
    }
}

/// The cloaking of a planet's starbase, which is what a planet is learned
/// through (`1070:ab3c`): the starbase design's [`ShipDesign::cloak_pct`],
/// or nothing without one.
fn starbase_cloak(state: &GameState, planet: &Planet) -> i32 {
    let Some(owner) = planet.owner.and_then(|o| usize::try_from(o).ok()) else {
        return 0;
    };
    let (Some(designs), Some(player)) = (state.designs.get(owner), state.players.get(owner)) else {
        return 0;
    };
    crate::production::starbase_hull(planet, designs)
        .and_then(|_| {
            planet
                .starbase_design
                .map(usize::from)
                .map(|s| usize::from(crate::startup::FIRST_STARBASE_SLOT) + s)
                .and_then(|s| designs.get(s))
        })
        .map_or(0, |d| d.cloak_pct(&player.race))
}

/// The share of a cloak a fleet's scanners see through: the best (lowest)
/// of its designs' [`ShipDesign::tachyon_pct`] (`GetFleetScannerRange`,
/// `1038:4fb8`).
fn fleet_tachyon(state: &GameState, fleet: &Fleet) -> i32 {
    let Some(owner) = usize::try_from(fleet.owner).ok() else {
        return 100;
    };
    let Some(designs) = state.designs.get(owner) else {
        return 100;
    };
    fleet
        .stacks
        .iter()
        .filter(|s| s.count > 0)
        .filter_map(|s| designs.get(usize::from(s.design)))
        .map(crate::design::ShipDesign::tachyon_pct)
        .min()
        .unwrap_or(100)
}

/// A fleet's scanner ranges: the **largest** of its designs', each counted
/// separately (`GetFleetScannerRange`, `1038:4fb8`).
#[must_use]
pub fn fleet_scan(state: &GameState, fleet: &Fleet) -> ScannerRange {
    let mut out = ScannerRange::default();
    let Some(owner) = usize::try_from(fleet.owner).ok() else {
        return out;
    };
    let Some(designs) = state.designs.get(owner) else {
        return out;
    };
    let player = state.players.get(owner);
    let joat = player.filter(|p| p.race.prt() == Some(Prt::Joat)).map(|p| {
        if state.tutorial_game {
            (40, 20)
        } else {
            let electronics = i32::from(p.research.levels[4]);
            (20 * electronics, 10 * electronics)
        }
    });
    let nas = player.is_some_and(|p| p.race.has_lrt(lrt::NO_ADV_SCANNER));
    for stack in &fleet.stacks {
        if stack.count <= 0 {
            continue;
        }
        let Some(design) = designs.get(usize::from(stack.design)) else {
            continue;
        };
        let range = design.scanner_range_for(joat, nas);
        out.normal = out.normal.max(range.normal);
        out.penetrating = out.penetrating.max(range.penetrating);
    }
    out
}

/// A planet's scanner ranges (`GetPlanetScannerRange`, `1038:4c02`): its
/// owner's best planetary scanner, or for an Alternate Reality race its
/// starbase by population.
#[must_use]
pub fn planet_scan(state: &GameState, planet: &Planet) -> ScannerRange {
    let Some(owner) = planet.owner.and_then(|o| usize::try_from(o).ok()) else {
        return ScannerRange::default();
    };
    let Some(player) = state.players.get(owner) else {
        return ScannerRange::default();
    };
    crate::scanning::planet_scanner_range_for_tech(
        planet,
        &player.race,
        &player.research.levels,
        planet.scanner.is_some(),
    )
}

/// What one scanner at `at` adds to a view: fleets of other players within
/// its normal range (in orbit, within its penetrating range), and planets
/// within its penetrating range — each range cut by the target's cloak,
/// which the scanner's detectors leave `tachyon` percent of.
fn scan_from(
    state: &GameState,
    player: i16,
    at: Point,
    range: ScannerRange,
    tachyon: i32,
    view: &mut View,
) {
    if range.normal > 0 {
        for (index, other) in state.fleets.iter().enumerate() {
            if other.owner == player || other.stacks.is_empty() {
                continue;
            }
            let reach = if other.orbiting.is_some() {
                range.penetrating
            } else {
                range.normal
            };
            if reach > 0 && within(at, other.position, reach) {
                let cloak = fleet_cloak(state, other, tachyon);
                if within_cloaked(at, other.position, reach, cloak) {
                    view.fleets.insert(index);
                }
            }
        }
    }
    if range.penetrating > 0 {
        for planet in state.planets.iter().chain(state.known_planets.iter()) {
            if planet.owner == Some(player) {
                continue;
            }
            if planet
                .position
                .is_some_and(|p| within(at, p, range.penetrating))
            {
                let cloak = starbase_cloak(state, planet);
                if planet
                    .position
                    .is_some_and(|p| within_starbase_cloak(at, p, range.penetrating, cloak))
                {
                    view.planets.insert(planet.id);
                }
            }
        }
    }
}

/// Everything `player` can see this year.
#[must_use]
pub fn view(state: &GameState, player: usize) -> View {
    let mut out = View::default();
    let Ok(me) = i16::try_from(player) else {
        return out;
    };

    // SetVisPFPlanets: the player's own planets, and what their scanners
    // reach.
    for planet in state.planets.iter().chain(state.known_planets.iter()) {
        if planet.owner != Some(me) {
            continue;
        }
        out.planets.insert(planet.id);
        let Some(at) = planet.position else {
            continue;
        };
        scan_from(state, me, at, planet_scan(state, planet), 100, &mut out);
    }

    // SetVisPFFleets: the player's own fleets, the planet each is at, and
    // what their scanners reach.
    for (index, fleet) in state.fleets.iter().enumerate() {
        if fleet.owner != me || fleet.stacks.is_empty() {
            continue;
        }
        out.fleets.insert(index);
        if let Some(planet) = fleet.orbiting {
            if let Ok(id) = i16::try_from(planet) {
                out.planets.insert(id);
            }
        }
        scan_from(
            state,
            me,
            fleet.position,
            fleet_scan(state, fleet),
            fleet_tachyon(state, fleet),
            &mut out,
        );
    }
    out
}
