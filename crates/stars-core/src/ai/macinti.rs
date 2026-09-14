//! The Macinti's turn: `DoMacintiAiTurn` (`10a0:0008`), its designs
//! (`EnsureMacintiShdefs`, `10a0:2e9c`), its starbase recycling table
//! (`EnsureMacintiStarbaseDesigns`, `1090:7688`), its war test
//! (`FPotentMacWarFleet`, `10a0:42ec`), its armada (`TargetMacArmada`,
//! `10a0:4146`), its haulers (`IdTargetMacFreighter`, `10a0:39d9`) and its
//! miners' retargeting (`FRetargetMiner`, `10a0:3e7e`), with the shared
//! routines reused from [`super::turindrone`], [`super::robotoid`],
//! [`super::automitron`] and [`super::cyber`].
//!
//! The Macinti is the Alternate Reality opponent, which lives on its
//! starbases. Its slots: Frigate mine layers in 0, colony ships in 1 (and
//! in 7 for its first forty years), Cruisers in 2 to 4, Battleships or
//! Nubians in 5 to 7, bombers in 8 and 9, freighters in 10 and 11,
//! Destroyers or Nubians in 12 and 13, and remote miners in 14 and 15.
//! Its haulers move people between its planets by the resources they
//! would make there, its miners move to the richest ground within reach,
//! and from turn 121 its mass drivers throw its surplus at its own
//! planets. See `docs/formulas/ai.md`, *The Macinti's turn*.

use crate::ai::colonise::{nearest_colonisable, Mark};
use crate::ai::cyber::{should_build_colonizer, target_potent_armada};
use crate::ai::dispatch::ideal_warp;
use crate::ai::parts::{create_design, Fitting};
use crate::ai::personality::Profile;
use crate::ai::robotoid::{
    find_buddy_and_join_up, is_attack_fleet, is_transport, random_planet_nearby,
    should_build_colonizers, target_attack,
};
use crate::ai::turindrone::{
    basic_tasks, check_status, ensure_research, fill_production_queues, install_design, lay_leg,
    marks, merge_all, mineral_worth, move_to_nearest_starbase, split_out_designs, Report,
};
use crate::ai::AiPersonality;
use crate::fleet::grobj;
use crate::movement::Point;
use crate::parts::Builder;
use crate::production::{item, QueueItem};
use crate::rng::Rng;
use crate::GameState;

/// The mine layers' slot.
pub const LAYER_SLOT: u8 = 0;
/// The colony ships' slot.
pub const COLONY_SLOT: u8 = 1;
/// The slot that holds a second colony-ship design for the first forty
/// years, and a warship after.
pub const EARLY_COLONY_SLOT: u8 = 7;
/// The remote miners' slots.
pub const MINER_SLOTS: [u8; 2] = [14, 15];
/// The bombers' slots.
pub const BOMBER_SLOTS: [u8; 2] = [8, 9];
/// A planet's queue is only filled when it has a starbase and more than
/// this many hundreds of colonists (`10a0:0a7e`: the population word
/// against 199).
pub const QUEUE_MIN_POP: i32 = 199;

/// The fittings, from `10a0:2a44` by the word offsets at `10a0:2a06`;
/// each byte is a part class ([`crate::ai::parts::PART_CLASSES`]) per hull
/// slot.
pub mod fitting {
    use super::Fitting;
    /// Entries 0 to 7: the Destroyers (6) of slots 12 (the first four) and
    /// 13 (the last four).
    pub const DESTROYERS: [Fitting; 8] = [
        &[8, 4, 4, 4, 17, 18, 19],
        &[8, 3, 3, 14, 17, 18, 19],
        &[8, 4, 3, 2, 17, 18, 20],
        &[8, 4, 4, 5, 17, 18, 20],
        &[8, 0, 0, 10, 17, 18, 19],
        &[8, 0, 0, 11, 17, 18, 19],
        &[8, 1, 1, 11, 17, 18, 19],
        &[8, 1, 1, 11, 17, 18, 11],
    ];
    /// Entries 8 and 9: the B-52 (19) bombers of slots 8 and 9.
    pub const B52S: [Fitting; 2] = [&[24, 21, 23, 23, 23, 12, 10], &[24, 21, 22, 22, 22, 12, 10]];
    /// Entry 10: the Frigate (5) mine layer of slot 0.
    pub const FRIGATE_LAYER: Fitting = &[24, 26, 25, 10];
    /// Entries 11 to 18: the Battleships (9) of slots 5 to 7 — slot 5
    /// draws from the first four, slot 6 from the last four, slot 7 from
    /// either.
    pub const BATTLESHIPS: [Fitting; 8] = [
        &[8, 11, 10, 1, 1, 1, 1, 2, 9, 11, 19],
        &[8, 13, 10, 1, 1, 0, 0, 0, 9, 11, 19],
        &[8, 13, 10, 0, 0, 1, 1, 0, 9, 11, 19],
        &[8, 13, 10, 0, 0, 0, 0, 3, 9, 11, 19],
        &[8, 13, 10, 4, 4, 4, 4, 4, 9, 20, 19],
        &[8, 13, 10, 4, 3, 3, 7, 2, 9, 20, 19],
        &[8, 13, 10, 2, 3, 7, 7, 3, 9, 20, 19],
        &[8, 13, 10, 4, 4, 3, 3, 5, 9, 20, 19],
    ];
    /// Entry 19: the Battleship bomber tried first for slots 8 and 9.
    pub const BATTLESHIP_BOMBER: Fitting = &[8, 14, 10, 33, 33, 33, 33, 33, 17, 20, 19];
    /// Entry 20: the Colony Ship (15).
    pub const COLONY_SHIP: Fitting = &[8, 40];
    /// Entry 21: the Maxi-Miner (23) of a player below skill 2.
    pub const MAXI_MINER: Fitting = &[24, 41, 42, 42, 42, 42];
    /// Entry 22: the Ultra-Miner (24) of a player at skill 2 or more, and
    /// the Miner (22) it falls back to.
    pub const ULTRA_MINER: Fitting = &[24, 41, 43, 43, 43, 43];
    /// Entry 23: the Mini-Miner (21) a player below skill 2 falls back to.
    pub const MINI_MINER: Fitting = &[24, 41, 42, 42];
    /// Entry 24: the Large Freighter (2) of slots 10 and 11, and the
    /// Medium Freighter (1) slot 10 falls back to.
    pub const FREIGHTER: Fitting = &[44, 16, 37];
    /// Entries 25 to 28: the Cruisers (7) of slots 2 to 4.
    pub const CRUISERS: [Fitting; 4] = [
        &[8, 13, 12, 3, 3, 2, 10],
        &[8, 37, 12, 1, 1, 11, 9],
        &[8, 13, 12, 4, 4, 7, 10],
        &[8, 11, 12, 0, 0, 0, 17],
    ];
    /// Entry 29: the Nubian (29) tried for slots 5 to 7.
    pub const NUBIAN_WARSHIP: Fitting = &[8, 9, 9, 3, 3, 33, 33, 10, 17, 10, 6, 7, 20];
    /// Entry 30: the Nubian tried for slots 12 and 13.
    pub const NUBIAN_ESCORT: Fitting = &[8, 10, 10, 7, 5, 20, 20, 4, 4, 19, 4, 2, 3];
}

/// The armada potencies (`10a0:0138`): six, from turn 131 `6 + (turn −
/// 120)/20`, at most fifty; half that; six, from turn 116 `6 + (turn −
/// 100)/22`, at most twelve; and half that less one, at most three.
#[must_use]
pub fn potency(turn: i16) -> [u8; 4] {
    let a = if turn > 130 { 6 + (turn - 120) / 20 } else { 6 }.min(50);
    let c = if turn > 115 { 6 + (turn - 100) / 22 } else { 6 }.min(12);
    let d = (c / 2 - 1).min(3);
    [
        u8::try_from(a).unwrap_or(50),
        u8::try_from(a / 2).unwrap_or(25),
        u8::try_from(c).unwrap_or(12),
        u8::try_from(d).unwrap_or(3),
    ]
}

/// `PctPlanetCapacity` (`1048:6b2c`): how full a planet is, in percent,
/// `(max/2 + pop × 100) / max`, at most 999; zero when the maximum is
/// not known (an Alternate Reality race's, which is not modelled).
#[must_use]
pub fn pct_planet_capacity(planet: &crate::planet::Planet, race: &crate::race::Race) -> i32 {
    let Some(max) = crate::hab::calc_planet_max_pop(planet, race) else {
        return 0;
    };
    if max <= 0 {
        return 0;
    }
    let pct = (i64::from(max) / 2 + i64::from(planet.pop) * 100) / i64::from(max);
    i32::try_from(pct.min(999)).unwrap_or(999)
}

/// `FPotentMacWarFleet` (`10a0:42ec`): the fleet's weight of war is its
/// ships in slots 2 to 4 plus twice those in 5 to 7 — and, when that is
/// under the first potency, twice those in 8 and 9 whose designs are
/// live Battleships — and it is potent when that reaches the first
/// potency (over it, when the bombers were counted).
#[must_use]
pub fn war_weight(
    state: &GameState,
    player: usize,
    fleet: &crate::fleet::Fleet,
    p0: i32,
) -> (bool, i32) {
    let count = |slot: u8| -> i32 {
        fleet
            .stacks
            .iter()
            .filter(|s| s.design == slot)
            .map(|s| s.count)
            .sum()
    };
    let mut weight: i32 = (2..5u8).map(count).sum::<i32>() + (5..8u8).map(count).sum::<i32>() * 2;
    if weight < p0 {
        for slot in BOMBER_SLOTS {
            let battleship = state.designs[player]
                .get(usize::from(slot))
                .is_some_and(|d| !d.obsolete && d.hull_id == 9);
            if count(slot) != 0 && battleship {
                weight += count(slot) * 2;
            }
        }
        if weight <= p0 {
            return (false, weight);
        }
    }
    (true, weight)
}

/// The squared distance between two points.
fn d2(a: Point, b: Point) -> i64 {
    let dx = i64::from(a.x) - i64::from(b.x);
    let dy = i64::from(a.y) - i64::from(b.y);
    dx * dx + dy * dy
}

/// Whether the player can build a part.
fn can_build(state: &GameState, player: usize, category: u16, item: usize) -> bool {
    let who = Builder::player(&state.players[player]);
    crate::parts::availability(&who, category, item).is_available()
}

/// `EnsureMacintiShdefs` (`10a0:2e9c`): the designs the personality wants
/// in its slots, each made when the slot is retired and the tech allows
/// (`rgTech` 0–5 = Energy, Weapons, Propulsion, Construction,
/// Electronics, Biotechnology).
#[allow(clippy::too_many_lines)]
fn ensure_designs(
    state: &mut GameState,
    player: usize,
    skill: u8,
    rng: &mut Rng,
    report: &mut Report,
) {
    let me = i16::try_from(player).unwrap_or(-1);
    let turn = state.turn;
    let levels = state.players[player].research.levels;
    let above = |field: usize, n: u8| levels[field] > n;
    let retired = |state: &GameState, slot: usize| {
        state
            .designs
            .get(player)
            .and_then(|d| d.get(slot))
            .is_none_or(|d| d.obsolete || d.hull().is_none())
    };
    let designed = |state: &GameState, slot: usize| {
        state
            .designs
            .get(player)
            .and_then(|d| d.get(slot))
            .map_or(i16::MIN / 2, |d| d.designed)
    };
    let exists = |state: &GameState, slot: u8| {
        state
            .fleets
            .iter()
            .filter(|f| f.owner == me)
            .any(|f| f.stacks.iter().any(|s| s.design == slot && s.count > 0))
    };
    let first_item = |state: &GameState, slot: usize| -> Option<u8> {
        state.designs[player]
            .get(slot)
            .and_then(|d| d.slots.first())
            .map(|s| s.item)
    };
    fn draw(
        state: &mut GameState,
        player: usize,
        slot: usize,
        hull: i16,
        fitting: Fitting,
        rng: &mut Rng,
        report: &mut Report,
    ) -> bool {
        let who = Builder::player(&state.players[player]);
        let Some(design) = create_design(hull, fitting, &who) else {
            return false;
        };
        install_design(
            state,
            player,
            u8::try_from(slot).unwrap_or(0),
            design,
            rng,
            report,
        );
        true
    }
    fn retire(state: &mut GameState, player: usize, slot: usize) {
        if let Some(d) = state.designs[player].get_mut(slot) {
            d.obsolete = true;
        }
    }

    // Slots 14 and 15, the miners: at skill 2 or more an Ultra-Miner —
    // slot 15 only past Construction 14 — else a Maxi-Miner; slot 14
    // falls back to a Miner or a Mini-Miner.
    let skilled = skill > 1;
    for slot in [14usize, 15] {
        if !retired(state, slot) {
            continue;
        }
        if skilled && slot == 15 && !above(3, 14) {
            continue;
        }
        let (hull, fit) = if skilled {
            (24, fitting::ULTRA_MINER)
        } else {
            (23, fitting::MAXI_MINER)
        };
        if !draw(state, player, slot, hull, fit, rng, report) && slot == 14 {
            if skilled {
                draw(state, player, 14, 22, fitting::ULTRA_MINER, rng, report);
            } else {
                draw(state, player, 14, 21, fitting::MINI_MINER, rng, report);
            }
        }
    }
    // Slots 12 and 13: a Nubian, else five tries at a Destroyer.
    if retired(state, 12)
        && above(1, 4)
        && above(2, 5)
        && !draw(state, player, 12, 29, fitting::NUBIAN_ESCORT, rng, report)
    {
        for _ in 0..5 {
            let i = usize::try_from(rng.random(4)).unwrap_or(0);
            if draw(state, player, 12, 6, fitting::DESTROYERS[i], rng, report) {
                break;
            }
        }
    }
    if retired(state, 13)
        && above(1, 9)
        && above(2, 8)
        && !draw(state, player, 13, 29, fitting::NUBIAN_ESCORT, rng, report)
    {
        for _ in 0..5 {
            let i = usize::try_from(rng.random(4)).unwrap_or(0) + 4;
            if draw(state, player, 13, 6, fitting::DESTROYERS[i], rng, report) {
                break;
            }
        }
    }
    // Slots 10 and 11, the freighters.
    if retired(state, 10) && !draw(state, player, 10, 2, fitting::FREIGHTER, rng, report) {
        draw(state, player, 10, 1, fitting::FREIGHTER, rng, report);
    }
    if retired(state, 11) {
        draw(state, player, 11, 2, fitting::FREIGHTER, rng, report);
    }
    // Before turn 20 the starting design in slot 2 is retired once no
    // ship of it is left.
    if turn < 20 && !retired(state, 2) && !exists(state, 2) {
        retire(state, player, 2);
    }
    // Slots 2 to 4, the Cruisers, each twenty years after the one before.
    for slot in 2..5usize {
        if !retired(state, slot) {
            continue;
        }
        if slot != 2 && turn - designed(state, slot - 1) <= 20 {
            continue;
        }
        for _ in 0..5 {
            let i = usize::try_from(rng.random(4)).unwrap_or(0);
            if draw(state, player, slot, 7, fitting::CRUISERS[i], rng, report) {
                break;
            }
        }
    }
    // Slot 7 holds a second colony ship for the first forty years.
    if turn < 40 && retired(state, 7) {
        draw(state, player, 7, 15, fitting::COLONY_SHIP, rng, report);
    }
    // Slot 1: once the Enigma Pulsar can be built, a live colony design
    // without it is retired and redrawn when no ship of it is left.
    if can_build(state, player, crate::components::slot::ENGINE, 15)
        && !retired(state, 1)
        && !exists(state, 1)
        && first_item(state, 1) != Some(15)
    {
        retire(state, player, 1);
        draw(state, player, 1, 15, fitting::COLONY_SHIP, rng, report);
    }
    // Slots 5 to 7, the warships: a Nubian two times in three, else five
    // tries at a Battleship from the slot's set — slot 7 picking a set
    // at random — each twenty years after the one before.
    for slot in 5..8usize {
        if !retired(state, slot) {
            continue;
        }
        if slot != 5 && (retired(state, slot - 1) || turn - designed(state, slot - 1) <= 20) {
            continue;
        }
        let mut base = if slot == 5 { 0 } else { 4 };
        if slot == 7 {
            base = if rng.random(2) == 0 { 4 } else { 0 };
        }
        if rng.random(3) == 0
            || !draw(
                state,
                player,
                slot,
                29,
                fitting::NUBIAN_WARSHIP,
                rng,
                report,
            )
        {
            for _ in 0..5 {
                let i = usize::try_from(rng.random(4)).unwrap_or(0) + base;
                if draw(state, player, slot, 9, fitting::BATTLESHIPS[i], rng, report) {
                    break;
                }
            }
        }
    }
    // Slots 8 and 9, the bombers, from Weapons 14: a Battleship bomber,
    // else a B-52; slot 9 fifteen years after 8.
    for slot in [8usize, 9] {
        if !retired(state, slot) || !above(1, 13) {
            continue;
        }
        if slot != 8 && (retired(state, slot - 1) || turn - designed(state, slot - 1) <= 15) {
            continue;
        }
        if !draw(
            state,
            player,
            slot,
            9,
            fitting::BATTLESHIP_BOMBER,
            rng,
            report,
        ) {
            draw(
                state,
                player,
                slot,
                19,
                fitting::B52S[slot - 8],
                rng,
                report,
            );
        }
    }
    // Slot 0: at skill 2 or more, once no ship of a non-Frigate design is
    // left and the tech allows, the Frigate mine layer.
    let slot0_frigate = state.designs[player]
        .first()
        .is_some_and(|d| d.hull_id == 5);
    if !slot0_frigate
        && skilled
        && !exists(state, 0)
        && above(5, 3)
        && above(4, 4)
        && above(3, 5)
        && above(2, 5)
        && above(0, 5)
    {
        retire(state, player, 0);
        draw(state, player, 0, 5, fitting::FRIGATE_LAYER, rng, report);
    }
}

/// `EnsureMacintiStarbaseDesigns` (`1090:7688`), as far as its recycling
/// table `vAiMacRecycleSB` goes — the starbase designs it would draw
/// (`FCreateAiStarbase`) are not, as no personality's starbase designs
/// are. Slots 1 to 3 (the small starbases): a live design never built is
/// 0 (drawn afresh; 1 when the drawing fails); one under 35 years old is
/// 0 — or, for slot 1 from turn 26 when the design does not have eight
/// slots, 3; under 50, 2; else 3 — and of those over 1 only the highest
/// (the older on a tie) is kept. Slots 4 to 9: retired slots 1; the older
/// of the two groups (4–6, 7–9, by their first design's year) 2 when under
/// thirty years old, else 3; and when the newer group's first hull is
/// small, slot 3 (and slot 2 when smaller still) 2.
fn starbase_recycle_table(state: &GameState, player: usize) -> [u8; 10] {
    let base = usize::from(crate::startup::FIRST_STARBASE_SLOT);
    let design = |slot: usize| state.designs[player].get(base + slot);
    let live = |slot: usize| design(slot).is_some_and(|d| d.hull().is_some() && !d.obsolete);
    let turn = state.turn;
    let mut table = [0u8; 10];
    for slot in 1..4usize {
        if live(slot) {
            let d = design(slot).expect("live");
            if d.built == 0 {
                table[slot] = 0;
                continue;
            }
            let age = turn - d.designed;
            if age < 35 {
                if turn < 26 || slot != 1 || design(1).is_some_and(|d| d.slots.len() == 8) {
                    table[slot] = 0;
                } else {
                    table[1] = 3;
                }
            } else if age < 50 {
                table[slot] = 2;
            } else {
                table[slot] = 3;
            }
        } else {
            table[slot] = 0;
        }
    }
    let mut best: Option<usize> = None;
    for slot in 1..4usize {
        if table[slot] > 1 {
            let better = match best {
                None => true,
                Some(b) => {
                    table[b] < table[slot]
                        || (table[b] == table[slot]
                            && design(slot).map_or(0, |d| d.designed)
                                < design(b).map_or(0, |d| d.designed))
                }
            };
            if better {
                best = Some(slot);
            }
        }
    }
    for (slot, value) in table.iter_mut().enumerate().take(4).skip(1) {
        if *value > 1 && Some(slot) != best {
            *value = 0;
        }
    }
    for (slot, value) in table.iter_mut().enumerate().skip(4) {
        *value = u8::from(!live(slot));
    }
    let (newer, older) =
        if design(4).map_or(0, |d| d.designed) < design(7).map_or(0, |d| d.designed) {
            (7usize, 4usize)
        } else {
            (4, 7)
        };
    let value = if turn - design(older).map_or(0, |d| d.designed) < 30 {
        2
    } else {
        3
    };
    for entry in table.iter_mut().skip(older).take(3) {
        *entry = value;
    }
    let newer_hull = design(newer).map_or(0, |d| d.hull_id) - 32;
    if newer_hull < 4 {
        table[3] = 2;
        if newer_hull < 3 {
            table[2] = 2;
        }
    }
    table
}

/// The Macinti's turn.
#[allow(clippy::too_many_lines)]
pub fn turn(state: &mut GameState, player: usize, rng: &mut Rng, profile: &Profile) -> Report {
    let mut report = Report::default();
    let Some(me) = i16::try_from(player).ok() else {
        return report;
    };
    if state.players.get(player).is_none_or(|p| p.dead) {
        return report;
    }
    let skill = match state.players[player].control {
        crate::ai::Control::Computer { skill_bits, .. } => skill_bits,
        crate::ai::Control::Human => 0,
    };
    let turn = state.turn;
    let seen = crate::visibility::view(state, player).planets;
    state.players[player].explored.extend(seen);
    let explored = state.players[player].explored.clone();

    // `IroEnsureAi(vrgbMacintiRes, 8, &ishdefSBLatest, 15)`; no starbase
    // history for the Macinti.
    report.research = ensure_research(state, player, profile.plan, profile.research_pct(turn));

    // Slot 7 holds a second colony ship for the first forty years, or
    // while a live colony design sits there; once the Enigma Pulsar can
    // be built and no ship of it is left it is retired for a warship.
    let live_colony_7 = state.designs[player]
        .get(7)
        .is_some_and(|d| !d.obsolete && d.hull_id == 15);
    let mut early_colony = false;
    if turn < 40 || live_colony_7 {
        early_colony = true;
        let exists7 = state
            .fleets
            .iter()
            .filter(|f| f.owner == me)
            .any(|f| f.stacks.iter().any(|s| s.design == 7 && s.count > 0));
        if can_build(state, player, crate::components::slot::ENGINE, 15) && !exists7 {
            if let Some(d) = state.designs[player].get_mut(7) {
                d.obsolete = true;
            }
            early_colony = false;
        }
    }
    let last_warship: u8 = if early_colony { 6 } else { 7 };
    // `MergeAllShdefs`: the warships (with slot 7 when it is not a colony
    // ship), the mine layers, the miners and the Destroyers.
    let warship_mask: u16 = if early_colony { 0x037c } else { 0x03fc };
    for mask in [warship_mask, 0x0001, 0xc000, 0x3000] {
        merge_all(state, me, mask, &mut report);
    }
    let potency = potency(turn);
    state.ai_armada_potency = potency;
    let recycle: i16 = if turn < 120 {
        50
    } else if turn < 200 {
        70
    } else {
        100
    };
    let mut old = [false; 16];
    let destroyers = check_status(state, player, me, 12, 13, recycle, &mut old);
    for (slot, is_old) in old.iter_mut().enumerate().take(14).skip(12) {
        if *is_old
            && state.designs[player]
                .get(slot)
                .is_some_and(|d| !d.obsolete && d.hull_id == 29)
        {
            *is_old = false;
        }
    }
    let miners = check_status(state, player, me, 14, 15, 5000, &mut old);
    // The older miner design is retired once the best mining robot can be
    // built, so that `EnsureMacintiShdefs` redraws it.
    let slot_item = |state: &GameState, slot: usize, at: usize| -> Option<u8> {
        state.designs[player]
            .get(slot)
            .and_then(|d| d.slots.get(at))
            .map(|s| s.item)
    };
    let live15 = state.designs[player]
        .get(15)
        .is_some_and(|d| !d.obsolete && d.hull().is_some());
    if live15 && can_build(state, player, crate::components::slot::MINING, 6) {
        let pick = if slot_item(state, 14, 2) == Some(6) {
            if slot_item(state, 15, 2) == Some(6)
                || !can_build(state, player, crate::components::slot::ENGINE, 15)
                || slot_item(state, 14, 0) == Some(15)
            {
                None
            } else {
                Some(15usize)
            }
        } else {
            Some(14)
        };
        if let Some(pick) = pick {
            let exists = state.fleets.iter().filter(|f| f.owner == me).any(|f| {
                f.stacks
                    .iter()
                    .any(|s| usize::from(s.design) == pick && s.count > 0)
            });
            if !exists {
                if let Some(d) = state.designs[player].get_mut(pick) {
                    d.obsolete = true;
                }
            }
        }
    }
    let freighters = check_status(state, player, me, 10, 11, recycle, &mut old);
    let bombers = check_status(state, player, me, 8, 9, recycle, &mut old);
    let cruisers = check_status(state, player, me, 2, 4, recycle, &mut old);
    let battleships = check_status(state, player, me, 5, last_warship, recycle, &mut old);
    // `SplitOutShdefs` from turn 81: the old designs, the mine layers,
    // the colony ships, the miners and the freighters.
    if turn > 80 {
        split_out_designs(state, player, me, &old, &mut report);
        for slots in [&[0u8][..], &[1], &[14, 15], &[10, 11]] {
            let mut only = [false; 16];
            for s in slots {
                only[usize::from(*s)] = true;
            }
            split_out_designs(state, player, me, &only, &mut report);
        }
    }
    ensure_designs(state, player, skill, rng, &mut report);
    state.players[player].mac_starbase_recycle = starbase_recycle_table(state, player);
    let (build_colonizers, mut colony_fleets) =
        should_build_colonizers(state, player, me, skill, rng);
    // Which slot the colony ship is built from: slot 1 with the Enigma
    // Pulsar, else slot 7 while it holds a live colony design, else 1.
    let colony_slot: u8 = if slot_item(state, 1, 0) == Some(15) {
        1
    } else if state.designs[player]
        .get(7)
        .is_some_and(|d| !d.obsolete && d.hull_id == 15)
    {
        7
    } else {
        1
    };
    let send_old_home = colony_slot == 1
        && early_colony
        && turn - state.designs[player].get(1).map_or(0, |d| d.designed) > 5;

    // --- The counts and marks.
    let planet_count = state.planets.len().max(
        state
            .planets
            .iter()
            .map(|p| usize::try_from(p.id).unwrap_or(0) + 1)
            .max()
            .unwrap_or(0),
    );
    let mut bound: Vec<bool> = vec![false; planet_count];
    let mut miner_fleets = 0i32;
    let mut layer_fleets = 0i32;
    let mut destroyer_fleets = 0i32;
    let mut freighter_fleets = 0i32;
    let mut warship_fleets = 0i32;
    for fleet in state
        .fleets
        .iter()
        .filter(|f| f.owner == me && !f.is_empty())
    {
        let count = |slot: u8| -> i32 {
            fleet
                .stacks
                .iter()
                .filter(|s| s.design == slot)
                .map(|s| s.count)
                .sum()
        };
        if miners.latest.is_some() && (count(14) != 0 || count(15) != 0) {
            miner_fleets += 1;
        }
        if count(0) != 0 {
            layer_fleets += 1;
        }
        if destroyers.latest.is_some() && (count(12) != 0 || count(13) != 0) {
            destroyer_fleets += 1;
        }
        if freighters.latest.is_some() && (count(10) != 0 || count(11) != 0) {
            freighter_fleets += 1;
            if fleet.orbiting.is_none()
                && fleet.waypoints.len() > 1
                && fleet.waypoints[1].target_class == grobj::PLANET
                && fleet.cargo.colonists > 0
            {
                if let Some(b) = fleet.waypoints[1]
                    .target
                    .map(usize::from)
                    .and_then(|i| bound.get_mut(i))
                {
                    *b = true;
                }
            }
        }
        if (2..10u8).any(|s| count(s) != 0) {
            warship_fleets += 1;
        }
    }
    let layer_fleets_at_start = layer_fleets;
    // Genesis Devices from turn 121, one per twenty planets, at most ten.
    let mut genesis =
        if turn > 120 && can_build(state, player, crate::components::slot::PLANETARY, 13) {
            i32::try_from(state.planets.iter().filter(|p| p.owner == Some(me)).count())
                .unwrap_or(0)
                .min(200)
                / 20
        } else {
            0
        };
    let mut worth_marks: Vec<u8> = vec![0; planet_count];
    for planet in &state.planets {
        let Ok(id) = usize::try_from(planet.id) else {
            continue;
        };
        if planet.owner.is_some_and(|o| o != me) {
            let mut v = ((planet.pop / 4) & 0xfff) / 250 + 1;
            if v > 6 {
                v = 6;
            }
            if planet.starbase {
                v += 1;
            }
            worth_marks[id] = u8::try_from(v).unwrap_or(7);
        }
    }
    let mut marks = marks(state, player, me, &explored, AiPersonality::Macinti);
    let race = state.players[player].race.clone();
    let levels = state.players[player].research.levels;
    let designs = state.designs.get(player).cloned().unwrap_or_default();

    // --- The planet pass.
    let order = crate::ai::planet_order(
        &state
            .planets
            .iter()
            .filter(|p| p.owner == Some(me))
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        rng,
        true,
    );
    let ships_of = |state: &GameState, slot: u8| -> i32 {
        state
            .fleets
            .iter()
            .filter(|f| f.owner == me)
            .flat_map(|f| f.stacks.iter())
            .filter(|s| s.design == slot)
            .map(|s| s.count)
            .sum()
    };
    for id in order {
        let Some(index) = state.planets.iter().position(|p| p.id == id) else {
            continue;
        };
        let planet = state.planets[index].clone();
        let Some(from) = planet.position else {
            continue;
        };
        // A starbase, over 19,900 people, a starbase design in a slot
        // other than 0 — and slot 1 only before turn 26 — and a queue
        // under 24 long with no ship in it.
        if !planet.starbase || planet.pop <= QUEUE_MIN_POP {
            continue;
        }
        let isb = planet.starbase_design.map_or(0, |d| d & 0xf);
        if isb == 0 || (isb == 1 && turn >= 26) {
            continue;
        }
        if planet.queue.len() >= 24 || planet.queue.iter().any(|q| q.ship && q.item < 16) {
            continue;
        }
        let mut added: Vec<(u8, i32)> = Vec::new();
        let mut items: Vec<(u16, i32)> = Vec::new();
        let resources = i32::from(
            crate::resources::resources_at_planet(&planet, &race, i16::from(levels[0]))
                .unwrap_or(0),
        );
        // From turn 121, a driver of warp 10 or more at a planet of a
        // million people, one time in four, with no packet queued: a
        // mineral over 5,000 kT, from a random one of the three round,
        // goes to the own planet with the least of it that has such a
        // driver within 302 light years — a fifth of the stock, at most
        // 20,000 kT, in hundred-kT packets, at warp 11.
        let mut done = false;
        if turn > 120 {
            let driver = crate::production::mass_driver(&planet, &designs);
            let packet_queued = planet
                .queue
                .iter()
                .any(|e| !e.ship && (item::PACKET_IRONIUM..=item::PACKET_MIXED).contains(&e.item));
            if driver.warp > 9 && planet.pop > 10_000 && rng.random(4) == 0 && !packet_queued {
                let start = usize::try_from(rng.random(3)).unwrap_or(0);
                let mineral = (start..start + 3)
                    .map(|i| i % 3)
                    .find(|&m| planet.surface_min[m] > 5000);
                if let Some(m) = mineral {
                    let mut best: Option<(i32, i16)> = None;
                    for other in state.planets.iter().filter(|p| p.owner == Some(me)) {
                        let Some(at) = other.position else { continue };
                        let least = best.map_or(100_000, |(a, _)| a);
                        if other.surface_min[m] >= least {
                            continue;
                        }
                        let their = crate::production::mass_driver(other, &designs);
                        if their.warp <= 9 {
                            continue;
                        }
                        if d2(from, at) >= 91_204 {
                            continue;
                        }
                        best = Some((other.surface_min[m], other.id));
                    }
                    if let Some((_, target)) = best {
                        let amount = (planet.surface_min[m] / 5).min(20_000);
                        let packets = amount / 100;
                        if packets > 0 {
                            items.push((
                                item::PACKET_IRONIUM + u16::try_from(m).unwrap_or(0),
                                packets,
                            ));
                            let p = &mut state.planets[index];
                            p.fling_dest = Some(target);
                            p.fling_warp = 7;
                            report.flung.push((id, target));
                            done = true;
                        }
                    }
                }
            }
        }
        if !done {
            // Freighters, one roll in three, while under 64 and under a
            // quarter of the planets owned.
            let owned = i32::try_from(state.planets.iter().filter(|p| p.owner == Some(me)).count())
                .unwrap_or(0);
            if let Some(latest) = freighters.latest {
                if freighter_fleets < 64 && freighter_fleets < owned / 4 && rng.random(3) == 0 {
                    freighter_fleets += 1;
                    added.push((latest, 1));
                }
            }
            // Colony ships: when they are wanted (or eight times in a
            // hundred when not), unless over 49 fleets carry one past turn
            // 120, and every mineral is at 30 kT — under that the planet is
            // done — by `FShouldPlanetBuildColonizer` over the planets not
            // ours: one, and from turn 5 one more where the planet grows
            // past 2,300 a year and makes over 35 resources at skill 1 or
            // more, a third past 3,600 and 50 at skill 2 or more.
            let wanted =
                build_colonizers && !(colony_fleets > 40 && (turn > 120 || colony_fleets > 100));
            let try_colony = if wanted { true } else { rng.random(100) < 8 };
            let mut finished = false;
            if try_colony && !(colony_fleets > 49 && turn > 120) {
                if planet.surface_min.iter().any(|m| *m < 30) {
                    finished = true;
                } else if should_build_colonizer(state, from, &|p| p.owner != Some(me), rng) {
                    colony_fleets += 1;
                    added.push((colony_slot, 1));
                    if turn > 4 {
                        let growth = i64::from(planet.pop)
                            * i64::from(crate::population::pct_true_max_growth(&race));
                        if growth > 2300 && resources > 35 && skill > 0 {
                            colony_fleets += 1;
                            added.push((colony_slot, 1));
                            if growth > 3600 && resources > 50 && skill > 1 {
                                added.push((colony_slot, 1));
                            }
                        }
                    }
                }
            }
            if !finished {
                // Miners (newest of 14, 15), one roll in two while under
                // sixty miner fleets and 5,000 ships: where the miners here
                // dig 1,000 mines' worth or less — more where the ground is
                // richer than they dig, three times the concentrations
                // summed being over 150; else, under thirty fleets, nine
                // rolls in ten.
                if let Some(latest) = miners.latest {
                    if miner_fleets < 60 && ships_of(state, latest) < 5000 && rng.random(2) == 0 {
                        let here = state
                            .fleets
                            .iter()
                            .find(|f| {
                                f.owner == me
                                    && f.orbiting == u16::try_from(id).ok()
                                    && f.stacks.iter().any(|s| s.design == latest && s.count > 0)
                            })
                            .map_or(0, |f| crate::mining::remote_mines(&designs, &f.stacks));
                        if here <= 1000 {
                            let conc3 =
                                planet.min_conc.iter().map(|c| i32::from(*c)).sum::<i32>() * 3;
                            let queue = if conc3 <= here || conc3 < 151 {
                                miner_fleets < 30 && rng.random(10) != 0
                            } else {
                                true
                            };
                            if queue {
                                miner_fleets += 1;
                                added.push((latest, 1));
                            }
                        }
                    }
                }
                // Mine layers, four at a time, one roll in four, under
                // sixty layer fleets and — the routine's own slip — under
                // 7,500 ships of the newest miner design.
                let layer_frigate = designs.first().is_some_and(|d| d.hull_id == 5);
                let miner_ships = miners.latest.map_or(0, |l| ships_of(state, l));
                if layer_frigate && layer_fleets < 60 && miner_ships < 7500 && rng.random(4) == 0 {
                    let here = state
                        .fleets
                        .iter()
                        .find(|f| {
                            f.owner == me
                                && f.orbiting == u16::try_from(id).ok()
                                && f.stacks.iter().any(|s| s.design == 0 && s.count > 0)
                        })
                        .map_or(0, |f| {
                            f.stacks
                                .iter()
                                .filter(|s| s.design == 0)
                                .map(|s| s.count)
                                .sum::<i32>()
                        });
                    if (here < 10 || (here < 17 && rng.random(10) == 0))
                        && rng.random(i16::try_from(here * 2 + 1).unwrap_or(i16::MAX)) == 0
                    {
                        layer_fleets += 3;
                        added.push((LAYER_SLOT, 4));
                    }
                }
                // "Rich", as the routine tests it: the first mineral under
                // 5,000 kT is germanium.
                let first_short = planet
                    .surface_min
                    .iter()
                    .position(|m| *m < 5000)
                    .unwrap_or(3);
                let rich = first_short == 2;
                // Bombers (newest of 8, 9): under 140 warship fleets and
                // (under sixty, or over 2,000 resources): past 110 fleets
                // one roll in three; else where a potent war fleet here
                // holds under the third potency of bombers — twelve on a
                // rich planet, four otherwise, and the planet is done.
                let mut to_finish = false;
                if let Some(latest) = bombers.latest {
                    if warship_fleets < 140 && (warship_fleets < 60 || resources > 2000) {
                        let mut queue = warship_fleets > 110 && rng.random(3) == 0;
                        if !queue {
                            let potent = state.fleets.iter().find(|f| {
                                f.owner == me
                                    && f.orbiting == u16::try_from(id).ok()
                                    && war_weight(state, player, f, i32::from(potency[0])).0
                            });
                            if let Some(f) = potent {
                                let aboard: i32 = f
                                    .stacks
                                    .iter()
                                    .filter(|s| BOMBER_SLOTS.contains(&s.design))
                                    .map(|s| s.count)
                                    .sum();
                                queue = aboard < i32::from(potency[2]);
                            }
                        }
                        if queue {
                            warship_fleets += 2;
                            added.push((latest, if rich { 12 } else { 4 }));
                            to_finish = true;
                        }
                    }
                }
                if !to_finish {
                    // A Genesis Device and 75 terraforming steps at a planet
                    // of a million people with none queued, unless
                    // germanium is the first mineral under 2,000 kT, by the
                    // concentrations less 12 summed: under 15 always, under
                    // 30 two rolls in three, under 60 four in five.
                    if genesis > 0 && planet.pop > 10_000 {
                        let genesis_queued = planet
                            .queue
                            .iter()
                            .any(|e| !e.ship && e.item == item::GENESIS);
                        let first_short = planet
                            .surface_min
                            .iter()
                            .position(|m| *m < 2000)
                            .unwrap_or(3);
                        if !genesis_queued && first_short != 2 {
                            let sum: i32 = planet.min_conc.iter().map(|c| i32::from(*c) - 12).sum();
                            let go = sum < 15
                                || (sum < 30 && rng.random(3) != 0)
                                || (sum < 60 && rng.random(5) != 0);
                            if go {
                                genesis -= 1;
                                items.push((item::GENESIS, 1));
                                items.push((item::TERRAFORM, 75));
                            }
                        }
                    }
                    // Cruisers (newest of 2–4) or, one roll in three, the
                    // newest Battleship: from turn 21, under 130 warship
                    // fleets and (under fifty, or over 2,000 resources); on a
                    // planet not rich only one roll in three; ten on a rich
                    // planet — and the planet is done — two otherwise.
                    let mut rich_done = false;
                    if let Some(latest) = cruisers.latest {
                        if warship_fleets < 130
                            && turn > 20
                            && (warship_fleets < 50 || resources > 2000)
                            && (rich || rng.random(3) == 0)
                        {
                            let choose = match battleships.latest {
                                Some(b) if rng.random(3) == 0 => b,
                                _ => latest,
                            };
                            warship_fleets += 1;
                            added.push((choose, if rich { 10 } else { 2 }));
                            rich_done = rich;
                        }
                    }
                    // Destroyers (newest of 12, 13), up to twenty paid for
                    // in full out of what is left, under eighty fleets
                    // (sixty from turn 120) and 2,000 ships.
                    if !rich_done {
                        if let Some(latest) = destroyers.latest {
                            let limit = if turn < 120 { 80 } else { 60 };
                            if destroyer_fleets < limit && destroyers.count < 2000 {
                                let mut left = crate::ai::production::resources_available(
                                    &state.planets[index],
                                    &race,
                                    state.players[player].research_pct,
                                    i16::from(levels[0]),
                                );
                                let committed = crate::ai::production::queue_cost(
                                    &state.planets[index].queue,
                                    &race,
                                );
                                let mut paid = true;
                                for (have, spent) in left.iter_mut().zip(committed.iter()) {
                                    *have -= spent;
                                    if *have < 0 {
                                        paid = false;
                                    }
                                }
                                if paid {
                                    let who = Builder::player(&state.players[player]);
                                    if let Some(cost) = state.designs[player]
                                        .get(usize::from(latest))
                                        .and_then(|d| d.true_cost(&who))
                                    {
                                        let each = [
                                            cost.minerals[0],
                                            cost.minerals[1],
                                            cost.minerals[2],
                                            cost.resources,
                                        ];
                                        let mut n = 0;
                                        for _ in 0..20 {
                                            let mut ok = true;
                                            for (have, spent) in left.iter_mut().zip(each.iter()) {
                                                *have -= spent;
                                                if *have < 0 {
                                                    ok = false;
                                                }
                                            }
                                            if !ok {
                                                break;
                                            }
                                            n += 1;
                                        }
                                        if n > 0 {
                                            added.push((latest, n));
                                            destroyer_fleets += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        for (design, count) in added {
            for _ in 0..count {
                state.planets[index].queue.push(QueueItem {
                    count: 1,
                    item: u16::from(design),
                    ship: true,
                    completion: 0,
                });
            }
            report.queued.push((id, design, count));
        }
        for (item_id, count) in items {
            state.planets[index].queue.push(QueueItem {
                count,
                item: item_id,
                ship: false,
                completion: 0,
            });
        }
    }

    // --- The first fleet pass.
    let positions: Vec<(i16, Point)> = state
        .planets
        .iter()
        .filter_map(|p| p.position.map(|at| (p.id, at)))
        .collect();
    let position_of = |id: i16| positions.iter().find(|(p, _)| *p == id).map(|(_, at)| *at);
    let mut attack_fleets: Vec<u16> = Vec::new();
    for index in 0..state.fleets.len() {
        let fleet = state.fleets[index].clone();
        if fleet.owner != me || fleet.is_empty() {
            continue;
        }
        let fleet_id = fleet.id;
        // A leg to a space object over 200 light years off is blown away.
        // (The routine does this to every fleet in the game; here only to
        // our own.)
        if fleet.waypoints.len() > 1
            && fleet.waypoints[1].target_class == grobj::THING
            && d2(fleet.position, fleet.waypoints[1].position) > 40_000
        {
            state.fleets[index].waypoints.truncate(1);
            report.cleaned.push(fleet_id);
        }
        let fleet = state.fleets[index].clone();
        let count = |slot: u8| -> i32 {
            fleet
                .stacks
                .iter()
                .filter(|s| s.design == slot)
                .map(|s| s.count)
                .sum()
        };
        let has_leg = fleet.waypoints.len() > 1;
        let orbiting = fleet.orbiting.and_then(|p| i16::try_from(p).ok());
        let orbit_index = orbiting.and_then(|id| state.planets.iter().position(|p| p.id == id));
        let scrap = |state: &mut GameState, report: &mut Report| {
            let f = &mut state.fleets[index];
            f.waypoints.truncate(1);
            f.waypoints[0].task = stars_formats::task::SCRAP;
            f.waypoints[0].task_data = Vec::new();
            f.waypoints[0].transport = None;
            report.scrapped.push(fleet_id);
        };
        let blow_away = |state: &mut GameState, report: &mut Report| {
            let f = &mut state.fleets[index];
            f.waypoints.truncate(1);
            f.waypoints[0].task = stars_formats::task::NONE;
            f.waypoints[0].task_data = Vec::new();
            f.waypoints[0].transport = None;
            report.cleaned.push(fleet_id);
        };
        if count(0) >= 1 && turn >= 41 {
            // Mine layers: with no leg, a buddy when over 55 layer fleets
            // fly (over 40, two rolls in three); a fleet of over six wanders
            // one time in five; else mines are laid where they are. Under
            // way, a task on the current waypoint is cleared.
            if !has_leg {
                if (layer_fleets_at_start > 55
                    || (layer_fleets_at_start > 40 && rng.random(3) != 0))
                    && find_buddy_and_join_up(state, me, index, 0, 0, 72, 108, rng)
                {
                    report.merged.push((fleet_id, fleet_id));
                    continue;
                }
                let mut wandered = false;
                if count(0) > 6 && rng.random(5) == 0 {
                    if let Some(id) = random_planet_nearby(state, fleet.position, 105, true, rng) {
                        if Some(id) != orbiting {
                            if let Some(at) = position_of(id) {
                                lay_leg(
                                    &mut state.fleets[index],
                                    at,
                                    id,
                                    stars_formats::task::LAY_MINES,
                                    4,
                                );
                                state.fleets[index].waypoints[1].task_data = vec![5, 0];
                                report.scouted.push((fleet_id, id));
                                wandered = true;
                            }
                        }
                    }
                }
                if !wandered {
                    let f = &mut state.fleets[index];
                    if f.waypoints[0].task != stars_formats::task::LAY_MINES {
                        f.waypoints[0].task = stars_formats::task::LAY_MINES;
                        f.waypoints[0].task_data = vec![5, 0];
                        report.laying.push(fleet_id);
                    }
                    continue;
                }
            } else if fleet.waypoints[0].task != stars_formats::task::NONE {
                let f = &mut state.fleets[index];
                f.waypoints[0].task = stars_formats::task::NONE;
                f.waypoints[0].task_data = Vec::new();
                f.waypoints[0].transport = None;
                continue;
            } else {
                continue;
            }
        } else if (count(14) < 1 && count(15) < 1) || has_leg {
            if is_attack_fleet(state, player, &fleet) {
                attack_fleets.push(fleet_id);
                // A warship claims the planet it is bound for or sits at.
                let warship = (2..=last_warship).any(|s| count(s) > 0);
                let planet_leg = has_leg && fleet.waypoints[1].target_class == grobj::PLANET;
                if warship && (planet_leg || orbiting.is_some()) {
                    let dest = if planet_leg {
                        fleet.waypoints[1]
                            .target
                            .and_then(|t| i16::try_from(t).ok())
                    } else {
                        orbiting
                    };
                    if let Some(w) = dest
                        .and_then(|d| usize::try_from(d).ok())
                        .and_then(|i| worth_marks.get_mut(i))
                    {
                        if *w != 0 {
                            *w |= 0x80;
                        }
                    }
                }
            } else if is_transport(state, player, &fleet) {
                // An empty transport bound for a planet not ours has its
                // orders blown away.
                let dest = if !has_leg || fleet.waypoints[0].task != stars_formats::task::NONE {
                    orbiting
                } else if fleet.waypoints[1].target_class == grobj::PLANET {
                    fleet.waypoints[1]
                        .target
                        .and_then(|t| i16::try_from(t).ok())
                } else {
                    None
                };
                if let Some(dest) = dest {
                    let ours = state
                        .planets
                        .iter()
                        .find(|p| p.id == dest)
                        .is_some_and(|p| p.owner == Some(me));
                    if !ours && fleet.cargo.colonists == 0 {
                        blow_away(state, &mut report);
                    }
                }
            }
        } else {
            // Miners with no leg: Remote Mining where they are; a buddy
            // when over 58 miner fleets fly (over 48, two rolls in three);
            // at an own planet whose concentrations sum under 30 (under 60
            // one roll in three) they move on by `FRetargetMiner`; else one
            // roll in ten.
            if fleet.waypoints[0].task != stars_formats::task::REMOTE_MINING {
                let f = &mut state.fleets[index];
                f.waypoints[0].task = stars_formats::task::REMOTE_MINING;
                f.waypoints[0].task_data = Vec::new();
                report.mining.push((fleet_id, orbiting.unwrap_or(-1)));
                continue;
            }
            if (miner_fleets > 58 || (miner_fleets > 48 && rng.random(3) != 0))
                && find_buddy_and_join_up(state, me, index, 14, 15, 72, 108, rng)
            {
                report.merged.push((fleet_id, fleet_id));
                continue;
            }
            if let Some(p) = orbit_index.filter(|&p| state.planets[p].owner == Some(me)) {
                let sum: i32 = state.planets[p]
                    .min_conc
                    .iter()
                    .map(|c| i32::from(*c))
                    .sum();
                if sum < 30 || (sum < 60 && rng.random(3) == 0) {
                    if let Some(to) = retarget_miner(state, me, index) {
                        report.mining.push((fleet_id, to));
                    }
                    continue;
                }
            }
            if rng.random(10) == 0 {
                if let Some(to) = retarget_miner(state, me, index) {
                    report.mining.push((fleet_id, to));
                }
            }
        }
        // The tail: before turn 11 the starting scouts and the starting
        // slot-2 ships are scrapped. With no leg, a colony ship — one of
        // slot 7 too, while it is a colony ship — is scrapped at a planet
        // once slot 1 is the colony design again; at skill 2 or more,
        // sitting at an unowned planet, it settles right there with
        // colonists aboard or goes home to a starbase without; else it
        // goes to the nearest colonisable planet with `pop/10` colonists
        // (at most 25) from the own planet it sits at, or is scrapped at
        // a planet when there is none. Under way, the old slot-7 colony
        // ships are sent home once slot 1's design is five years old.
        if turn < 11 && (count(0) > 0 || count(2) != 0) {
            scrap(state, &mut report);
            continue;
        }
        let is_colony = count(COLONY_SLOT) != 0 || (early_colony && count(7) != 0);
        if !has_leg {
            if !is_colony {
                continue;
            }
            if early_colony && colony_slot == 1 && orbiting.is_some() {
                scrap(state, &mut report);
                continue;
            }
            let mut settle_here = false;
            let mut go_home = false;
            let mut wait = false;
            if skill >= 2 {
                if let Some(p) = orbit_index {
                    let planet = &state.planets[p];
                    let guess = (planet.pop / 4) & 0xfff;
                    if planet.owner.is_none() {
                        if fleet.cargo.colonists == 0 {
                            go_home = true;
                        } else {
                            settle_here = true;
                        }
                    } else if planet.owner != Some(me) && !planet.starbase && guess <= 49 {
                        wait = true;
                    }
                }
            }
            if wait {
                continue;
            }
            if go_home {
                move_to_nearest_starbase(state, me, index, false);
                continue;
            }
            if settle_here {
                let f = &mut state.fleets[index];
                f.waypoints[0].task = stars_formats::task::COLONIZE;
                f.waypoints[0].task_data = Vec::new();
                report.colonising.push((fleet_id, orbiting.unwrap_or(-1)));
                continue;
            }
            let candidates: Vec<(i16, (i32, i32), Mark)> = positions
                .iter()
                .map(|(pid, at)| {
                    let mark = usize::try_from(*pid)
                        .ok()
                        .and_then(|i| marks.get(i).copied())
                        .unwrap_or(Mark::Unknown);
                    (*pid, (i32::from(at.x), i32::from(at.y)), mark)
                })
                .collect();
            let from = fleet.position;
            let target = nearest_colonisable((i32::from(from.x), i32::from(from.y)), &candidates);
            let Some(target) = target else {
                if orbiting.is_some() {
                    scrap(state, &mut report);
                }
                continue;
            };
            if let Some(p) = orbit_index.filter(|&p| state.planets[p].owner == Some(me)) {
                let want = (state.planets[p].pop / 10).min(25);
                let room =
                    state.fleets[index].cargo_capacity(&designs) - state.fleets[index].cargo.mass();
                let take = want.min(state.planets[p].pop).min(room).max(0);
                state.planets[p].pop -= take;
                state.fleets[index].cargo.colonists += take;
            }
            if let Some(at) = position_of(target) {
                let stacks: Vec<(&crate::design::ShipDesign, i32)> = state.fleets[index]
                    .stacks
                    .iter()
                    .filter_map(|s| designs.get(usize::from(s.design)).map(|d| (d, s.count)))
                    .collect();
                let warp = ideal_warp(&stacks, false);
                lay_leg(
                    &mut state.fleets[index],
                    at,
                    target,
                    stars_formats::task::COLONIZE,
                    warp,
                );
                if let Some(mark) = usize::try_from(target).ok().and_then(|i| marks.get_mut(i)) {
                    *mark = Mark::Claimed;
                }
                report.colonising.push((fleet_id, target));
            }
        } else if early_colony && send_old_home && count(7) != 0 {
            blow_away(state, &mut report);
            move_to_nearest_starbase(state, me, index, false);
        }
    }

    // --- The haulers: every transport with no leg, on battle plan 4, by
    // `IdTargetMacFreighter`.
    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        if fleet.owner != me || fleet.is_empty() || fleet.waypoints.len() > 1 {
            continue;
        }
        if !is_transport(state, player, fleet) {
            continue;
        }
        let fleet_id = fleet.id;
        state.fleets[index].battle_plan = 4;
        if let Some(to) = target_mac_freighter(state, player, me, index, &race, &mut bound) {
            report.hauling.push((fleet_id, to));
        }
    }

    // --- The last pass: old ships home; warships to the armada or a
    // buddy; Destroyers after the enemy.
    for index in 0..state.fleets.len() {
        let fleet = state.fleets[index].clone();
        if fleet.owner != me || fleet.is_empty() {
            continue;
        }
        let fleet_id = fleet.id;
        let count = |slot: u8| -> i32 {
            fleet
                .stacks
                .iter()
                .filter(|s| s.design == slot)
                .map(|s| s.count)
                .sum()
        };
        let orbiting = fleet.orbiting.and_then(|p| i16::try_from(p).ok());
        let all_old = fleet
            .stacks
            .iter()
            .filter(|s| s.count > 0)
            .all(|s| old.get(usize::from(s.design)) == Some(&true));
        if all_old {
            let own = orbiting
                .and_then(|id| state.planets.iter().find(|p| p.id == id))
                .filter(|p| p.owner == Some(me));
            if let Some(p) = own {
                if p.starbase || rng.random(5) == 0 {
                    let f = &mut state.fleets[index];
                    f.waypoints.truncate(1);
                    f.waypoints[0].task = stars_formats::task::SCRAP;
                    report.scrapped.push(fleet_id);
                    continue;
                }
            }
            let has_leg = fleet.waypoints.len() > 1;
            if (!has_leg || orbiting.is_some() || fleet.waypoints[1].target_class != grobj::PLANET)
                && move_to_nearest_starbase(state, me, index, false)
            {
                continue;
            }
        }
        let warship = (2..10u8).find(|s| count(*s) > 0 && !(*s == 7 && early_colony));
        if warship.is_some() {
            let total: i32 = (2..10u8).map(count).sum();
            let (ready, weight) = war_weight(state, player, &fleet, i32::from(potency[0]));
            let _ = ready;
            let bombers_aboard = count(8) + count(9);
            let mut buddy = false;
            if warship_fleets >= 101 || (warship_fleets >= 91 && rng.random(3) == 0) {
                // A roll beating the ships aboard less ten; else one in
                // twenty.
                let r = i32::from(rng.random(100));
                buddy = total - 10 < r || rng.random(20) == 0;
            }
            if buddy && find_buddy_and_join_up(state, me, index, 2, 9, 100, 200, rng) {
                report.merged.push((fleet_id, fleet_id));
                continue;
            }
            if let Some(to) = target_potent_armada(
                state,
                player,
                me,
                index,
                &potency,
                skill,
                weight,
                bombers_aboard,
                rng,
            ) {
                report.attacking.push((fleet_id, to));
            }
            continue;
        }
        if !is_attack_fleet(state, player, &fleet) {
            continue;
        }
        let chasing = fleet.waypoints.len() > 1 && fleet.waypoints[1].target_class == grobj::FLEET;
        if chasing {
            continue;
        }
        let (many, some) = if turn < 121 { (70, 60) } else { (50, 40) };
        let join = destroyer_fleets > many || (destroyer_fleets > some && rng.random(3) == 0);
        if join {
            let big = destroyers.latest.is_some_and(|l| count(l) > 19);
            if !(big && rng.random(20) != 0)
                && find_buddy_and_join_up(state, me, index, 12, 13, 36, 72, rng)
            {
                report.merged.push((fleet_id, fleet_id));
                continue;
            }
        }
        if let Some(to) = target_attack(state, player, me, index, &attack_fleets, rng) {
            report.attacking.push((fleet_id, to));
        }
    }
    let worth = mineral_worth(state, &explored, marks.len());
    basic_tasks(
        state,
        player,
        me,
        &worth,
        AiPersonality::Macinti,
        rng,
        &mut report,
    );
    fill_production_queues(state, player, me, AiPersonality::Macinti, rng, &mut report);
    report
}

/// `FRetargetMiner` (`10a0:3e7e`): the own planet within 80 light years
/// worth the most — ironium concentration times 8, boranium times 10,
/// germanium times 7 — when it is worth more than six fifths of the
/// planet the miners sit at; a Remote Mining leg there at warp 6.
fn retarget_miner(state: &mut GameState, me: i16, index: usize) -> Option<i16> {
    let fleet = state.fleets[index].clone();
    let here = fleet.orbiting.and_then(|p| i16::try_from(p).ok());
    let mut best: Option<(i32, i16, Point)> = None;
    let mut here_worth = 0i32;
    for planet in state.planets.iter().filter(|p| p.owner == Some(me)) {
        let Some(at) = planet.position else { continue };
        if d2(fleet.position, at) >= 0x1440 {
            continue;
        }
        let worth = i32::from(planet.min_conc[0]) * 8
            + i32::from(planet.min_conc[1]) * 10
            + i32::from(planet.min_conc[2]) * 7;
        if Some(planet.id) == here {
            here_worth = worth;
        }
        if best.is_none_or(|(w, _, _)| worth > w) {
            best = Some((worth, planet.id, at));
        }
    }
    let (worth, target, at) = best?;
    if worth <= here_worth * 6 / 5 {
        return None;
    }
    lay_leg(
        &mut state.fleets[index],
        at,
        target,
        stars_formats::task::REMOTE_MINING,
        6,
    );
    Some(target)
}

/// `IdTargetMacFreighter` (`10a0:39d9`): where a hauler goes, and what it
/// carries.
///
/// At an own planet over a quarter full and over 99,900 people it takes
/// `pop/20` colonists (as far as the hold allows) and looks at every own
/// planet within 200 light years no hauler is bound for: the resources
/// the planet would gain from the colonists that arrive — 3 % lost within
/// 50 light years, 6 within 100, 9 within 150, 12 within 200 — against
/// the resources home loses; the best gain wins when it beats the loss
/// by 5 (by 10 from turn 81, by 15 from turn 161), and the freighter
/// loads and goes, unloading all on arrival. Failing that, a mineral over
/// 2,499 kT here goes — a fifth of it — to the own planet within 200
/// light years with the least of it, under 200 kT. Failing that, the
/// fullest own planet within 200 light years (its capacity less 2, 4 or
/// 6 for lying over 50, 100 or 150 light years off) with a score over
/// zero. The leg is at warp 4 with a Transport order unloading
/// everything.
fn target_mac_freighter(
    state: &mut GameState,
    player: usize,
    me: i16,
    index: usize,
    race: &crate::race::Race,
    bound: &mut [bool],
) -> Option<i16> {
    use stars_formats::{ItemAction, TransportTask, XferAction};

    let fleet = state.fleets[index].clone();
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let energy = i16::from(state.players[player].research.levels[0]);
    let here = fleet
        .orbiting
        .and_then(|p| i16::try_from(p).ok())
        .and_then(|id| state.planets.iter().position(|p| p.id == id));
    let resources_at = |planet: &crate::planet::Planet| -> i32 {
        i32::from(crate::resources::resources_at_planet(planet, race, energy).unwrap_or(0))
    };
    let loss_pct = |d: i64| -> i32 {
        if d <= 2500 {
            3
        } else if d <= 10_000 {
            6
        } else if d <= 22_500 {
            9
        } else {
            12
        }
    };
    let mut target: Option<i16> = None;
    let mut load: Option<(usize, i32)> = None;
    if let Some(p) = here {
        let home = state.planets[p].clone();
        if home.owner == Some(me) && pct_planet_capacity(&home, race) > 25 && home.pop > 999 {
            let room = fleet.cargo_capacity(&designs) - fleet.cargo.minerals.iter().sum::<i32>();
            let take = room.min(home.pop / 20);
            if take > 0 {
                let mut after = home.clone();
                after.pop -= take;
                let loss = resources_at(&home) - resources_at(&after);
                let mut best: Option<(i32, i16)> = None;
                for other in state.planets.iter().filter(|q| q.owner == Some(me)) {
                    if other.id == home.id {
                        continue;
                    }
                    if usize::try_from(other.id)
                        .ok()
                        .and_then(|i| bound.get(i))
                        .copied()
                        == Some(true)
                    {
                        continue;
                    }
                    let Some(at) = other.position else { continue };
                    let d = d2(fleet.position, at);
                    if d > 40_000 {
                        continue;
                    }
                    let delivered = take - take * loss_pct(d) / 100;
                    let mut with = other.clone();
                    with.pop += delivered;
                    let gain = resources_at(&with) - resources_at(other);
                    if best.is_none_or(|(g, _)| gain > g) {
                        best = Some((gain, other.id));
                    }
                }
                if let Some((gain, id)) = best {
                    let turn = state.turn;
                    if gain >= loss + 5
                        && (gain >= loss + 10 || turn < 81)
                        && (gain >= loss + 15 || turn < 161)
                    {
                        target = Some(id);
                        load = Some((3, take));
                        if let Some(b) = usize::try_from(id).ok().and_then(|i| bound.get_mut(i)) {
                            *b = true;
                        }
                    }
                }
            }
        }
        if target.is_none() {
            if let Some(m) = home.surface_min.iter().position(|v| *v > 2499) {
                let mut best: Option<(i32, i16)> = None;
                for other in state.planets.iter().filter(|q| q.owner == Some(me)) {
                    if other.id == home.id {
                        continue;
                    }
                    let Some(at) = other.position else { continue };
                    if d2(fleet.position, at) > 40_000 {
                        continue;
                    }
                    let least = best.map_or(1000, |(a, _)| a);
                    if other.surface_min[m] < least {
                        best = Some((other.surface_min[m], other.id));
                    }
                }
                if let Some((amount, id)) = best.filter(|(a, _)| *a < 200) {
                    let _ = amount;
                    let room = fleet.cargo_capacity(&designs) - fleet.cargo.mass();
                    let take = room.min(home.surface_min[m] / 5);
                    target = Some(id);
                    load = Some((m, take));
                }
            }
        }
    }
    if target.is_none() {
        let mut best: Option<(i32, i16)> = None;
        for other in state.planets.iter().filter(|q| q.owner == Some(me)) {
            if here.is_some_and(|p| state.planets[p].id == other.id) {
                continue;
            }
            let Some(at) = other.position else { continue };
            let d = d2(fleet.position, at);
            if d > 40_000 {
                continue;
            }
            let mut score = pct_planet_capacity(other, race);
            if d > 22_500 {
                score -= 6;
            } else if d > 10_000 {
                score -= 4;
            } else if d > 2500 {
                score -= 2;
            }
            if score > best.map_or(0, |(s, _)| s) {
                best = Some((score, other.id));
            }
        }
        target = best.map(|(_, id)| id);
    }
    let target = target?;
    if let (Some(p), Some((what, amount))) = (here, load) {
        // `XferAiSupply`: as much as the planet has and the hold takes.
        let room = state.fleets[index].cargo_capacity(&designs) - state.fleets[index].cargo.mass();
        if what == 3 {
            let take = amount.min(state.planets[p].pop).min(room).max(0);
            state.planets[p].pop -= take;
            state.fleets[index].cargo.colonists += take;
        } else {
            let take = amount
                .min(state.planets[p].surface_min[what])
                .min(room)
                .max(0);
            state.planets[p].surface_min[what] -= take;
            state.fleets[index].cargo.minerals[what] += take;
        }
    }
    let at = state
        .planets
        .iter()
        .find(|q| q.id == target)
        .and_then(|q| q.position)?;
    let items = [ItemAction {
        quantity: 0,
        action: XferAction::UnloadAll,
    }; 5];
    let f = &mut state.fleets[index];
    f.waypoints.truncate(1);
    f.waypoints[0].task = stars_formats::task::NONE;
    f.waypoints[0].task_data = Vec::new();
    f.waypoints[0].transport = None;
    f.waypoints.push(crate::fleet::Waypoint {
        position: at,
        target: u16::try_from(target).ok(),
        target_class: grobj::PLANET,
        warp: 4,
        task: stars_formats::task::TRANSPORT,
        transport: Some(TransportTask { items }),
        task_data: Vec::new(),
    });
    f.warp = Some(4);
    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fittings_fit_their_hulls() {
        let slots = |hull: usize| {
            crate::components::HULLS[hull]
                .slots
                .iter()
                .take_while(|s| s.allowed != 0)
                .count()
        };
        for f in &fitting::DESTROYERS {
            assert_eq!(f.len(), slots(6));
        }
        for f in &fitting::B52S {
            assert_eq!(f.len(), slots(19));
        }
        assert_eq!(fitting::FRIGATE_LAYER.len(), slots(5));
        for f in &fitting::BATTLESHIPS {
            assert_eq!(f.len(), slots(9));
        }
        assert_eq!(fitting::BATTLESHIP_BOMBER.len(), slots(9));
        assert_eq!(fitting::COLONY_SHIP.len(), slots(15));
        assert_eq!(fitting::MAXI_MINER.len(), slots(23));
        assert_eq!(fitting::ULTRA_MINER.len(), slots(24));
        assert_eq!(fitting::ULTRA_MINER.len(), slots(22));
        assert_eq!(fitting::MINI_MINER.len(), slots(21));
        assert_eq!(fitting::FREIGHTER.len(), slots(2));
        assert_eq!(fitting::FREIGHTER.len(), slots(1));
        for f in &fitting::CRUISERS {
            assert_eq!(f.len(), slots(7));
        }
        assert_eq!(fitting::NUBIAN_WARSHIP.len(), slots(29));
        assert_eq!(fitting::NUBIAN_ESCORT.len(), slots(29));
    }

    #[test]
    fn the_potencies_follow_the_years() {
        assert_eq!(potency(0), [6, 3, 6, 2]);
        assert_eq!(potency(140), [7, 3, 7, 2]);
        assert_eq!(potency(1000), [50, 25, 12, 3]);
    }
}
