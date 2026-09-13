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
//! file; this module only answers for one year. Cloaking is not modelled
//! yet — every fleet and starbase is taken as uncloaked — and neither are
//! the ranges of stargates and space objects that the two passes also
//! settle.

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
    let dx = i64::from(p.x) - i64::from(at.x);
    let dy = i64::from(p.y) - i64::from(at.y);
    if dx.abs() > i64::from(range) || dy.abs() > i64::from(range) {
        return false;
    }
    dx * dx + dy * dy <= i64::from(range) * i64::from(range)
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
/// within its penetrating range.
fn scan_from(state: &GameState, player: i16, at: Point, range: ScannerRange, view: &mut View) {
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
                view.fleets.insert(index);
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
                view.planets.insert(planet.id);
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
        scan_from(state, me, at, planet_scan(state, planet), &mut out);
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
            &mut out,
        );
    }
    out
}
