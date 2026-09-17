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
//! same way. A **Packet Physics** race's packets in flight scan as they go,
//! penetrating, to the square of their warp, and a **Space Demolition**
//! race's minefields show the fleets loose inside them, a cloaked one on a
//! roll (`SetVisPFThings`, `1070:b9ee`). An **Interstellar Traveler** sees
//! every gated planet within reach of their own gates. The marking of
//! space objects that the passes also settle is not modelled here.
//!
//! Each entry carries how much is seen — a [`Detail`] — which is what the
//! host writes for it: a planet a scannerless fleet sits at only by its
//! owner, a scanned one by its environment, a Robber Baron's by its
//! surface minerals too. The turn writer (`save.rs`) turns the view into
//! the partial planet, fleet and design records of the player's file.

use std::collections::{BTreeMap, BTreeSet};

use crate::fleet::Fleet;
use crate::movement::Point;
use crate::planet::Planet;
use crate::race::{lrt, Prt};
use crate::scanning::ScannerRange;
use crate::GameState;

/// How much of a planet or fleet a player sees — the `det` of the record
/// the host writes for it (`enums.h`: `detMinimal` 1, `detObscure` 2,
/// `detSome` 3, `detMore` 4, `detAll` 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Detail {
    /// A planet a scannerless fleet of the player's sits at: its id, its
    /// owner and whether it has a starbase, nothing more (`1070:9654`).
    Minimal = 1,
    /// A planet within penetrating range whose cloaked starbase keeps it
    /// out of the cut-down reach (`1070:ab3c`): written as [`Detail::Some`]
    /// with `fInclude` clear.
    Obscure = 2,
    /// Environment and concentrations, the owner's population and defence
    /// guesses; a fleet's ships, heading and mass.
    Some = 3,
    /// [`Detail::Some`] and the cargo — a Pick Pocket Scanner's fleets at
    /// the same spot, a Robber Baron's planet, or an unowned planet the
    /// player is remote-mining (`1070:9654`).
    More = 4,
    /// The player's own: everything.
    Full = 7,
}

impl Detail {
    /// The `det` byte of the record.
    #[must_use]
    pub fn code(self) -> u8 {
        self as u8
    }
}

/// What one player can see this year.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct View {
    /// The planets the player has a record of this year, and how much of
    /// each: their own in full, and every one within penetrating range or
    /// with one of their fleets at it.
    pub planets: BTreeMap<i16, Detail>,
    /// The fleets on the player's map, as indices into
    /// [`GameState::fleets`], and how much of each: their own in full, and
    /// every other player's within range.
    pub fleets: BTreeMap<usize, Detail>,
    /// The minefields the player's scanners reach this year, as indices
    /// into [`GameState::minefields`] — not their own, which they always
    /// know, nor ones detected in an earlier year and out of reach now,
    /// which their file still carries (`grbitPlr`); this is `grbitPlrNow`.
    pub minefields: BTreeSet<usize>,
    /// The mineral packets in flight the player sees, as indices into
    /// [`GameState::packets`]: every one for a Packet Physics race, else
    /// those within a scanner's normal range — their own included only
    /// when a scanner reaches them.
    pub packets: BTreeSet<usize>,
    /// The wormhole ends the player sees this year, as indices into
    /// [`GameState::wormholes`]: within a scanner's normal range once seen
    /// before (`grbitPlr`), else its penetrating range.
    pub wormholes: BTreeSet<usize>,
}

impl View {
    /// `MarkPlanet` (`1070:8adc`): the planet is on the map, at the
    /// greater of the detail it had and `detail`.
    pub fn mark_planet(&mut self, id: i16, detail: Detail) {
        let entry = self.planets.entry(id).or_insert(detail);
        *entry = (*entry).max(detail);
    }

    /// `MarkFleet` (`1070:885e`): likewise for a fleet.
    pub fn mark_fleet(&mut self, index: usize, detail: Detail) {
        let entry = self.fleets.entry(index).or_insert(detail);
        *entry = (*entry).max(detail);
    }

    /// How much the player sees of a planet, if it is on their map at all.
    #[must_use]
    pub fn planet(&self, id: i16) -> Option<Detail> {
        self.planets.get(&id).copied()
    }

    /// How much the player sees of a fleet, if it is on their map at all.
    #[must_use]
    pub fn fleet(&self, index: usize) -> Option<Detail> {
        self.fleets.get(&index).copied()
    }
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

/// Whether a fleet carries any scanner at all — the `-1` of
/// `GetCachedFleetScannerRange` that leaves a planet it sits at known only
/// minimally (`1070:9654`): a scanner part, one of the three parts that
/// scan on the side, or a Jack of All Trades hull's built-in scanner.
fn fleet_has_scanner(state: &GameState, fleet: &Fleet) -> bool {
    use crate::components::slot;
    let Some(owner) = usize::try_from(fleet.owner).ok() else {
        return false;
    };
    let Some(designs) = state.designs.get(owner) else {
        return false;
    };
    let joat = state
        .players
        .get(owner)
        .is_some_and(|p| p.race.prt() == Some(Prt::Joat));
    fleet
        .stacks
        .iter()
        .filter(|s| s.count > 0)
        .filter_map(|s| designs.get(usize::from(s.design)))
        .any(|d| {
            (joat && (4..=6).contains(&d.hull_id))
                || d.slots.iter().any(|s| {
                    s.count != 0
                        && (s.is(slot::SCANNER)
                            || (s.is(slot::ARMOR) && s.item == 9)
                            || (s.is(slot::BEAM) && s.item == 18)
                            || (s.is(slot::SHIELD) && s.item == 6))
                })
        })
}

/// What one scanner at `at` adds to a view: fleets of other players within
/// its normal range (in orbit, within its penetrating range), and planets
/// within its penetrating range — each range cut by the target's cloak,
/// which the scanner's detectors leave `tachyon` percent of. A planet whose
/// cloaked starbase keeps it out of the cut-down reach is still marked, as
/// [`Detail::Obscure`] (`1070:ab3c`).
fn scan_from(
    state: &GameState,
    player: i16,
    at: Point,
    range: ScannerRange,
    tachyon: i32,
    fleet_pass: bool,
    view: &mut View,
) {
    scan_things(state, player, at, range, fleet_pass, view);
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
                    view.mark_fleet(index, Detail::Some);
                }
            }
        }
    }
    if range.penetrating > 0 {
        for planet in state.planets.iter().chain(state.known_planets.iter()) {
            if planet.owner == Some(player) {
                continue;
            }
            let Some(p) = planet
                .position
                .filter(|p| within(at, *p, range.penetrating))
            else {
                continue;
            };
            let cloak = starbase_cloak(state, planet);
            if within_starbase_cloak(at, p, range.penetrating, cloak) {
                view.mark_planet(planet.id, Detail::Some);
            } else {
                view.mark_planet(planet.id, Detail::Obscure);
            }
        }
    }
}

/// The space objects one scanner at `at` reaches — the things loop of
/// `SetVisPFFleets` (`1070:a100`) and of `SetVisPFPlanets` (`1070:abde`),
/// which differ in one respect: a fleet's scanner considers every
/// minefield, a planet's only those within its normal range.
///
/// A **packet** is seen within the normal range. A **wormhole** end is seen
/// within the normal range once the player has seen it before
/// (`grbitPlr`), and otherwise only within the penetrating range. A
/// **minefield** is seen within the normal range once detected before,
/// within the penetrating range regardless, and always from inside it —
/// the squared distance to its centre no more than its mine count.
fn scan_things(
    state: &GameState,
    player: i16,
    at: Point,
    range: ScannerRange,
    fleet_pass: bool,
    view: &mut View,
) {
    let bit = 1u16 << (u16::try_from(player).unwrap_or(0) & 15);
    for (index, packet) in state.packets.iter().enumerate() {
        if within(at, packet.position, range.normal) {
            view.packets.insert(index);
        }
    }
    for (index, hole) in state.wormholes.iter().enumerate() {
        let reach = if hole.detected_by & bit != 0 {
            range.normal
        } else {
            range.penetrating
        };
        if within(at, hole.position, reach) {
            view.wormholes.insert(index);
        }
    }
    for (index, field) in state.minefields.iter().enumerate() {
        if field.owner == player {
            continue;
        }
        let known = field.detected_by & bit != 0;
        let near = within(at, field.position, range.normal);
        let seen = if fleet_pass {
            (known && near) || within(at, field.position, range.penetrating) || field.contains(at)
        } else {
            near && (known || within(at, field.position, range.penetrating) || field.contains(at))
        };
        if seen {
            view.minefields.insert(index);
        }
    }
}

/// `SetVisPFPlanets`' second pass (`1070:abde`): an Interstellar Traveler
/// sees every planet with a stargate within the range of each of their
/// own gates — all of them from an unlimited gate — cut by the target
/// starbase's cloak the way a penetrating scan is.
fn stargate_view(state: &GameState, player: usize, me: i16, view: &mut View) {
    use crate::stargate::{gate_of, RANGE};
    let Some(designs) = state.designs.get(player) else {
        return;
    };
    let all: Vec<&Planet> = state
        .planets
        .iter()
        .chain(state.known_planets.iter())
        .collect();
    for mine in all.iter().filter(|p| p.owner == Some(me)) {
        let Some(gate) = gate_of(mine, designs) else {
            continue;
        };
        let Some(at) = mine.position else {
            continue;
        };
        let range = i32::from(RANGE[gate]);
        for other in &all {
            if view.planet(other.id) >= Some(Detail::Some) || !other.starbase {
                continue;
            }
            let Some(their_designs) = other
                .owner
                .and_then(|o| usize::try_from(o).ok())
                .and_then(|o| state.designs.get(o))
            else {
                continue;
            };
            if gate_of(other, their_designs).is_none() {
                continue;
            }
            let Some(p) = other.position else {
                continue;
            };
            if range < 0 {
                view.mark_planet(other.id, Detail::Some);
                continue;
            }
            let cloak = starbase_cloak(state, other);
            if within_starbase_cloak(at, p, range, cloak) {
                view.mark_planet(other.id, Detail::Some);
            }
        }
    }
}

/// Everything `player` can see this year.
///
/// The Space Demolition pass rolls for each cloaked fleet inside one of the
/// player's fields, which the host does once a year with the game's
/// generator; here the roll is seeded from the seed, the year and the
/// player, so a frontend recomputing the view frame by frame sees the same
/// answer all year. [`view_with`] takes the generator to use instead.
#[must_use]
pub fn view(state: &GameState, player: usize) -> View {
    let mut rng = crate::rng::Rng::randomize(
        state
            .seed
            .wrapping_add(u32::try_from(state.turn).unwrap_or(0).wrapping_mul(7919))
            .wrapping_add(u32::try_from(player).unwrap_or(0).wrapping_mul(104_729)),
    );
    view_with(state, player, &mut rng)
}

/// [`view`], rolling the Space Demolition pass on `rng`.
#[must_use]
pub fn view_with(state: &GameState, player: usize, rng: &mut crate::rng::Rng) -> View {
    let mut out = View::default();
    let Ok(me) = i16::try_from(player) else {
        return out;
    };
    let prt = state.players.get(player).and_then(|p| p.race.prt());

    // SetVisPFInit (`1070:9654`): the player's own planets and fleets in
    // full, and the planet each fleet sits at — minimally without a
    // scanner, in some detail with one, and more for a Robber Baron or a
    // fleet remote-mining an unowned planet.
    for planet in state.planets.iter().chain(state.known_planets.iter()) {
        if planet.owner == Some(me) {
            out.mark_planet(planet.id, Detail::Full);
        }
    }
    for (index, fleet) in state.fleets.iter().enumerate() {
        if fleet.owner != me || fleet.stacks.is_empty() {
            continue;
        }
        out.mark_fleet(index, Detail::Full);
        let Some(id) = fleet.orbiting.and_then(|p| i16::try_from(p).ok()) else {
            continue;
        };
        let steal = crate::transport::steal_level(state, index);
        let mut detail = if fleet_has_scanner(state, fleet) {
            Detail::Some
        } else {
            Detail::Minimal
        };
        if steal > 1 {
            detail = Detail::More;
        }
        let mining = fleet
            .waypoints
            .first()
            .is_some_and(|w| w.task == stars_formats::task::REMOTE_MINING)
            && state
                .planets
                .iter()
                .chain(state.known_planets.iter())
                .any(|p| p.id == id && p.owner.is_none())
            && state
                .designs
                .get(player)
                .is_some_and(|d| crate::mining::remote_mines(d, &fleet.stacks) > 0);
        if mining {
            detail = Detail::More;
        }
        out.mark_planet(id, detail);
    }

    // SetVisPFFleets (`1070:a100`): what the player's fleets' scanners
    // reach; a Pick Pocket's fleets at the same spot with their cargo; and
    // anybody's fleet at one of the player's planets.
    for (index, fleet) in state.fleets.iter().enumerate() {
        if fleet.stacks.is_empty() {
            continue;
        }
        if fleet.owner != me {
            if let Some(id) = fleet.orbiting.and_then(|p| i16::try_from(p).ok()) {
                if out.planet(id) == Some(Detail::Full) {
                    out.mark_fleet(index, Detail::Some);
                }
            }
            continue;
        }
        let steal = crate::transport::steal_level(state, index);
        if steal & 1 != 0 {
            for (other_index, other) in state.fleets.iter().enumerate() {
                if other.owner != me && !other.stacks.is_empty() && other.position == fleet.position
                {
                    out.mark_fleet(other_index, Detail::More);
                }
            }
        }
        scan_from(
            state,
            me,
            fleet.position,
            fleet_scan(state, fleet),
            fleet_tachyon(state, fleet),
            true,
            &mut out,
        );
    }

    // SetVisPFPlanets (`1070:abde`): what the player's planets' scanners
    // reach, and an Interstellar Traveler's view through their gates.
    for planet in state.planets.iter().chain(state.known_planets.iter()) {
        if planet.owner != Some(me) {
            continue;
        }
        let Some(at) = planet.position else {
            continue;
        };
        scan_from(
            state,
            me,
            at,
            planet_scan(state, planet),
            100,
            false,
            &mut out,
        );
    }
    if prt == Some(Prt::It) {
        stargate_view(state, player, me, &mut out);
    }

    // SetVisPFThings (`1070:b9ee`): a Packet Physics race's packets in
    // flight scan, penetrating, to the square of their warp; a Space
    // Demolition race's minefields show the fleets loose inside them.
    if prt == Some(Prt::Pp) {
        for packet in &state.packets {
            if packet.owner != me || packet.warp == 0 {
                continue;
            }
            let range = packet.speed() * packet.speed();
            scan_from(
                state,
                me,
                packet.position,
                ScannerRange {
                    normal: range,
                    penetrating: range,
                },
                100,
                true,
                &mut out,
            );
        }
    } else if prt == Some(Prt::Sd) {
        for field in &state.minefields {
            if field.owner != me {
                continue;
            }
            // The field itself scans to its radius (`1070:b9ee`), normal
            // and penetrating alike.
            #[allow(clippy::cast_possible_truncation)]
            let radius = field.radius() as i32;
            scan_things(
                state,
                me,
                field.position,
                ScannerRange {
                    normal: radius,
                    penetrating: radius,
                },
                true,
                &mut out,
            );
            for (index, other) in state.fleets.iter().enumerate() {
                if other.owner == me || other.stacks.is_empty() || other.orbiting.is_some() {
                    continue;
                }
                if !field.contains(other.position) {
                    continue;
                }
                // A cloaked fleet is caught on a roll against its cloak.
                let cloak = fleet_cloak(state, other, 100);
                if cloak == 0 || i32::from(rng.random(100)) >= cloak {
                    out.mark_fleet(index, Detail::Some);
                }
            }
        }
    }
    out
}
