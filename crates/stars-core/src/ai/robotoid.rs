//! The Robotoid's turn: `DoRobotoidAiTurn` (`1088:0312`), its designs
//! (`EnsureRobotoidShdefs`, `1088:20ae`) and its war test
//! (`FPotentRobWarFleet`, `1088:31bc`), with the shared routines it calls
//! transcribed in [`super::turindrone`] and reused from there.
//!
//! The shape is the TurinDrone's — research, merges by slot mask, the
//! armada potencies from the year, `CheckAiShdefStatus` over the slot
//! ranges, `SplitOutShdefs`, the designs, a pass over the planets' queues,
//! passes over the fleets, then `HandleBasicAiTasks` and
//! `FillProductionQueue` — but the slots mean other things. The Robotoid
//! keeps **mine-laying Frigates in slot 0**, colony ships in slot 1, Meta
//! Morph warships in 2 to 5, Battleships in 6 and 7, bombers in 9 and 10,
//! Privateer or Meta Morph cruisers — its haulers — in 11 to 13, and its
//! Nubian or Destroyer armada ships in 14 and 15. It never scouts: its
//! starting scouts are scrapped before turn 21, and from turn 41 a fleet
//! of slot-0 Frigates lays mines where it sits, one in five wandering to
//! a planet within 105 light years. See `docs/formulas/ai.md`, *The
//! Robotoid's turn*.

use std::collections::BTreeSet;

use crate::ai::colonise::{nearest_colonisable, Mark};
use crate::ai::dispatch::ideal_warp;
use crate::ai::parts::{create_design, Fitting};
use crate::ai::personality::Profile;
use crate::ai::turindrone::{
    basic_tasks, check_status, colony_design, ensure_research, fill_production_queues,
    install_design, lay_leg, marks, merge_all, mineral_worth, move_to_nearest_starbase,
    split_out_designs, target_armada, target_freighter, validate_starbase_history, valued_planets,
    Report, COLONY_SLOT, QUEUE_MIN_POP,
};
use crate::ai::AiPersonality;
use crate::fleet::grobj;
use crate::movement::Point;
use crate::parts::Builder;
use crate::production::QueueItem;
use crate::rng::Rng;
use crate::GameState;

/// The Frigate mine layers' slot.
pub const LAYER_SLOT: u8 = 0;
/// How far a wandering mine layer looks for a planet
/// (`IdRandomPlanetNearby(pt, 105, 1)`).
pub const WANDER_RANGE: i32 = 105;
/// How many colonists a colony ship is loaded with: `XferAiSupply(..., 3,
/// 10)`, a thousand people.
pub const COLONISTS_ABOARD: i32 = 10;

/// The fittings, from `1088:1f80` by the word offsets at `1088:1f34`; each
/// byte is a part class ([`crate::ai::parts::PART_CLASSES`]) per hull slot.
pub mod fitting {
    use super::Fitting;
    /// Offsets 0 to 7: the Meta Morph (31) warships for slots 2 to 5, the
    /// even slots drawing from the first four and the odd from the last.
    pub const META_MORPHS: [Fitting; 8] = [
        &[8, 4, 10, 10, 13, 9, 9],
        &[8, 10, 5, 4, 13, 12, 15],
        &[8, 10, 4, 7, 13, 12, 14],
        &[8, 10, 3, 3, 13, 12, 14],
        &[8, 9, 1, 1, 11, 11, 12],
        &[8, 0, 9, 10, 13, 11, 12],
        &[8, 9, 0, 0, 10, 11, 12],
        &[8, 1, 9, 12, 12, 11, 11],
    ];
    /// Offsets 8 to 13: the Meta Morph cruisers for slots 11 to 13, once
    /// Construction reaches 10.
    pub const META_MORPH_CRUISERS: [Fitting; 6] = [
        &[8, 10, 16, 16, 3, 12, 2],
        &[8, 16, 4, 3, 14, 12, 13],
        &[8, 3, 16, 10, 16, 12, 14],
        &[8, 16, 1, 11, 12, 10, 10],
        &[8, 16, 11, 12, 16, 1, 0],
        &[8, 10, 16, 16, 11, 0, 0],
    ];
    /// Offsets 14 and 15: the Privateer (11) cruisers before that, slot 11
    /// taking the first and 12 and 13 the second.
    pub const PRIVATEERS: [Fitting; 2] = [&[8, 10, 15, 4, 4], &[8, 9, 11, 0, 0]];
    /// Offsets 16 to 19: the Destroyer (6) for slot 14, when no Nubian can
    /// be built.
    pub const DESTROYERS_14: [Fitting; 4] = [
        &[8, 4, 4, 4, 17, 18, 19],
        &[8, 3, 3, 14, 17, 18, 19],
        &[8, 4, 3, 2, 17, 18, 20],
        &[8, 4, 4, 5, 17, 18, 20],
    ];
    /// Offsets 20 to 23: the Destroyer for slot 15.
    pub const DESTROYERS_15: [Fitting; 4] = [
        &[8, 0, 0, 10, 17, 18, 19],
        &[8, 0, 0, 11, 17, 18, 19],
        &[8, 1, 1, 11, 17, 18, 19],
        &[8, 1, 1, 11, 17, 18, 11],
    ];
    /// Offsets 24 and 25: the B-52 (19) bombers for slots 9 and 10, when
    /// the Battleship bomber cannot be built.
    pub const B52S: [Fitting; 2] = [&[24, 21, 23, 23, 23, 12, 10], &[24, 21, 22, 22, 22, 12, 10]];
    /// Offset 26 (`1088:2032`): the Frigate (5) mine layer of slot 0.
    pub const FRIGATE_LAYER: Fitting = &[24, 26, 25, 10];
    /// Offsets 27 to 30: the Battleships (9) for slot 6.
    pub const BATTLESHIPS_6: [Fitting; 4] = [
        &[8, 11, 10, 1, 1, 1, 1, 2, 9, 11, 19],
        &[8, 13, 10, 1, 1, 0, 0, 0, 9, 11, 19],
        &[8, 13, 10, 0, 0, 1, 1, 0, 9, 11, 19],
        &[8, 13, 10, 0, 0, 0, 0, 3, 9, 11, 19],
    ];
    /// Offsets 31 to 34: the Battleships for slot 7.
    pub const BATTLESHIPS_7: [Fitting; 4] = [
        &[8, 13, 10, 4, 4, 4, 4, 4, 9, 20, 19],
        &[8, 13, 10, 4, 3, 3, 7, 2, 9, 20, 19],
        &[8, 13, 10, 2, 3, 7, 7, 3, 9, 20, 19],
        &[8, 13, 10, 4, 4, 3, 3, 5, 9, 20, 19],
    ];
    /// Offset 36 (`1088:2095`): the Battleship bomber for slots 9 and 10,
    /// tried before the B-52.
    pub const BATTLESHIP_BOMBER: Fitting = &[8, 13, 10, 33, 33, 33, 33, 33, 17, 20, 19];
    /// Offset 37 (`1088:20a0`): the Nubian (29) for slots 14 and 15.
    pub const NUBIAN: Fitting = &[8, 10, 10, 7, 5, 20, 20, 4, 4, 19, 4, 2, 3];
}

/// The armada potencies for the year — `vrgAiArmadaPotency` as
/// `DoRobotoidAiTurn` sets them (`1088:0399`): four from turn 131 rising by
/// one every twenty years to fifty, and half that; six from turn 116
/// rising by one every twenty-two years to twelve, and half that less one,
/// at most three.
#[must_use]
pub fn potency(turn: i16) -> [u8; 4] {
    let a = if turn > 130 { 4 + (turn - 120) / 20 } else { 4 }.min(50);
    let c = if turn > 115 { 6 + (turn - 100) / 22 } else { 6 }.min(12);
    let d = (c / 2 - 1).min(3);
    [
        u8::try_from(a).unwrap_or(50),
        u8::try_from(a / 2).unwrap_or(25),
        u8::try_from(c).unwrap_or(12),
        u8::try_from(d).unwrap_or(3),
    ]
}

/// `FPotentRobWarFleet(fleet, 2)`: the fleet's weight of war is its ships
/// in slots 2 to 5 plus twice those in 6 and 7, and it is potent when that
/// reaches the first potency. (With a potency argument under 2 the routine
/// answers yes to anything; the turn always passes 2.)
#[must_use]
pub fn is_potent_war_fleet(fleet: &crate::fleet::Fleet, potency: &[u8; 4]) -> bool {
    let weight: i32 = fleet
        .stacks
        .iter()
        .map(|s| match s.design {
            2..=5 => s.count,
            6 | 7 => s.count * 2,
            _ => 0,
        })
        .sum();
    weight >= i32::from(potency[0])
}

/// `FIsAiAttack` (`1090:4a72`): a warship aboard — any design on a hull
/// 6 to 10, a Frigate, or a Meta Morph or Nubian carrying under 500 kT.
#[must_use]
pub fn is_attack_fleet(state: &GameState, player: usize, fleet: &crate::fleet::Fleet) -> bool {
    fleet.stacks.iter().any(|s| {
        s.count > 0
            && state.designs[player]
                .get(usize::from(s.design))
                .is_some_and(|d| {
                    (5..=10).contains(&d.hull_id)
                        || ((d.hull_id == 29 || d.hull_id == 31)
                            && d.cargo_capacity().unwrap_or(0) < 500)
                })
    })
}

/// `FIsAiTransport` (`1090:4c08`): a freighter hull (0 to 3), a Privateer,
/// Rogue or Galleon, or a Meta Morph carrying 500 kT or more.
#[must_use]
pub fn is_transport(state: &GameState, player: usize, fleet: &crate::fleet::Fleet) -> bool {
    fleet.stacks.iter().any(|s| {
        s.count > 0
            && state.designs[player]
                .get(usize::from(s.design))
                .is_some_and(|d| {
                    (0..=3).contains(&d.hull_id)
                        || (11..=13).contains(&d.hull_id)
                        || (d.hull_id == 31 && d.cargo_capacity().unwrap_or(0) > 499)
                })
    })
}

/// `FShouldWeBuildColonizers` (`1090:476a`): whether colony ships are
/// wanted, and how many fleets carry one (`*pcCol`).
///
/// A skill-0 player builds none in odd years; everyone builds them before
/// turn 30. After that: none with no live colony-ship design (a hull 14 or
/// 15); none with more colony fleets than twenty times the galaxy size
/// plus ten; none when those fleets plus every player's planets exceed
/// four fifths of the galaxy; else yes when the colony ships built so far
/// exceed that sum by under twenty-six, or with under five fleets one roll
/// in two.
#[must_use]
pub fn should_build_colonizers(
    state: &GameState,
    player: usize,
    me: i16,
    skill: u8,
    rng: &mut Rng,
) -> (bool, i32) {
    if skill == 0 && state.turn & 1 != 0 {
        return (false, 0);
    }
    if state.turn < 30 {
        return (true, 0);
    }
    let colony: Vec<u8> = state.designs[player]
        .iter()
        .enumerate()
        .filter(|(_, d)| !d.obsolete && (d.hull_id == 14 || d.hull_id == 15))
        .filter_map(|(i, _)| u8::try_from(i).ok())
        .collect();
    if colony.is_empty() {
        return (false, 0);
    }
    let fleets = i32::try_from(
        state
            .fleets
            .iter()
            .filter(|f| f.owner == me)
            .filter(|f| {
                f.stacks
                    .iter()
                    .any(|s| s.count > 0 && colony.contains(&s.design))
            })
            .count(),
    )
    .unwrap_or(i32::MAX);
    if fleets > i32::from(state.galaxy_size) * 20 + 10 {
        return (false, fleets);
    }
    let held: i32 = state
        .planets
        .iter()
        .filter(|p| p.owner.is_some())
        .count()
        .try_into()
        .unwrap_or(i32::MAX);
    let galaxy = i32::try_from(state.planets.len()).unwrap_or(0);
    if fleets + held > galaxy * 4 / 5 {
        return (false, fleets);
    }
    let built: i64 = colony
        .iter()
        .map(|&d| i64::from(state.designs[player][usize::from(d)].built))
        .sum();
    let short = built - i64::from(fleets + held);
    let yes = (0..26).contains(&short) || (fleets < 5 && rng.random(2) == 0);
    (yes, fleets)
}

/// `IdRandomPlanetNearby(pt, range, fAvoidStarbases)` (`1090:5f54`): a
/// planet within `range` chosen uniformly by reservoir — the first is
/// kept, the second replaces it one time in two, the third one in three —
/// drawn again up to twice when the one drawn has a starbase.
pub(crate) fn random_planet_nearby(
    state: &GameState,
    at: Point,
    range: i32,
    avoid_starbases: bool,
    rng: &mut Rng,
) -> Option<i16> {
    let mut tries = if avoid_starbases { 2 } else { 0 };
    loop {
        let mut n: i16 = 1;
        let mut pick = None;
        for planet in &state.planets {
            let Some(p) = planet.position else {
                continue;
            };
            let dx = i64::from(p.x) - i64::from(at.x);
            let dy = i64::from(p.y) - i64::from(at.y);
            if dx * dx + dy * dy <= i64::from(range) * i64::from(range) {
                if rng.random(n) == 0 {
                    pick = Some((planet.id, planet.starbase));
                }
                n = n.saturating_add(1);
            }
        }
        match pick {
            Some((id, starbase)) => {
                if starbase && tries > 0 {
                    tries -= 1;
                    continue;
                }
                return Some(id);
            }
            None => return None,
        }
    }
}

/// `EnsureRobotoidShdefs` (`1088:20ae`): the designs the personality wants
/// in its slots, each made when the slot is free — empty or retired — and
/// the tech allows, some only so many years after the slot before it.
///
/// * Slots 11 to 13, the cruisers: Propulsion over 1 and Construction of
///   at least 4, 7 and 10 in turn, slots 12 and 13 fifteen years after the
///   one before; a Privateer while Construction is under 10, else one of
///   the six Meta Morph cruisers at random, five tries.
/// * Slot 14: Weapons over 4, Electronics and Construction and Propulsion
///   over 5, Energy over 1: a Nubian, five tries, or failing that a
///   Destroyer from four fittings.
/// * Slot 15: Electronics over 9, Construction over 7, Propulsion over 8,
///   Weapons over 13: the Nubian, else five tries at a Destroyer.
/// * Slots 2 to 5: Weapons and Construction over 9, Propulsion over 8,
///   Energy over 5, each after the one before is filled and twelve years
///   old: a Meta Morph, five tries from the first four fittings for the
///   even slots and the last four for the odd.
/// * Slots 6 and 7: Biotechnology over 3, Electronics over 9, Construction
///   and Propulsion over 11, Energy over 5, Weapons over 14, slot 7 twenty
///   years after 6: a Battleship, five tries from four fittings each.
/// * Slots 9 and 10: Weapons over 13, slot 10 fifteen years after 9: the
///   Battleship bomber, else a B-52.
/// * Slot 0: with a skill over 1, no ship of the slot's design left, a
///   design that is not a Frigate, and Biotechnology over 3, Electronics
///   over 4, Construction, Propulsion and Energy over 5: the old design is
///   retired and a Frigate mine layer drawn.
///
/// Slot 1, the colony ship, is never redrawn: the Robotoid builds the one
/// it started with.
fn ensure_designs(
    state: &mut GameState,
    player: usize,
    skill: u8,
    rng: &mut Rng,
    report: &mut Report,
) {
    let me = i16::try_from(player).unwrap_or(-1);
    let levels = state.players[player].research.levels;
    let above = |field: usize, n: u8| levels[field] > n;
    let free = |state: &GameState, slot: usize| {
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
    let turn = state.turn;

    // Draw a design into a slot: the first fitting that can be built.
    fn draw(
        state: &mut GameState,
        player: usize,
        slot: usize,
        hull: i16,
        fittings: &[Fitting],
        rng: &mut Rng,
        report: &mut Report,
    ) -> bool {
        let who = Builder::player(&state.players[player]);
        let Some(design) = fittings.iter().find_map(|f| create_design(hull, f, &who)) else {
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
    // Five tries at a random fitting from a set.
    fn draw_random(
        state: &mut GameState,
        player: usize,
        slot: usize,
        hull: i16,
        fittings: &[Fitting],
        rng: &mut Rng,
        report: &mut Report,
    ) -> bool {
        for _ in 0..5 {
            let pick = usize::try_from(rng.random(i16::try_from(fittings.len()).unwrap_or(1)))
                .unwrap_or(0)
                .min(fittings.len() - 1);
            if draw(state, player, slot, hull, &[fittings[pick]], rng, report) {
                return true;
            }
        }
        false
    }

    // Cruisers, slots 11 to 13.
    for slot in 11..14usize {
        let needs =
            above(2, 1) && i32::from(levels[3]) >= (i32::try_from(slot).unwrap_or(11) - 11) * 3 + 4;
        let spaced = slot == 11 || turn - designed(state, slot - 1) > 14;
        if !free(state, slot) || !needs || !spaced {
            continue;
        }
        if levels[3] < 10 {
            let fit = fitting::PRIVATEERS[usize::from(slot != 11)];
            draw(state, player, slot, 11, &[fit], rng, report);
        } else {
            draw_random(
                state,
                player,
                slot,
                31,
                &fitting::META_MORPH_CRUISERS,
                rng,
                report,
            );
        }
    }
    // The armada slots, 14 and 15.
    if free(state, 14) && above(1, 4) && above(4, 5) && above(3, 5) && above(2, 5) && above(0, 1) {
        for _ in 0..5 {
            if draw(state, player, 14, 29, &[fitting::NUBIAN], rng, report) {
                break;
            }
            let pick = usize::try_from(rng.random(4)).unwrap_or(0).min(3);
            if draw(
                state,
                player,
                14,
                6,
                &[fitting::DESTROYERS_14[pick]],
                rng,
                report,
            ) {
                break;
            }
        }
    }
    if free(state, 15)
        && above(4, 9)
        && above(3, 7)
        && above(2, 8)
        && above(1, 13)
        && !draw(state, player, 15, 29, &[fitting::NUBIAN], rng, report)
    {
        draw_random(state, player, 15, 6, &fitting::DESTROYERS_15, rng, report);
    }
    // The warships, slots 2 to 5.
    for slot in 2..6usize {
        let needs = above(1, 9) && above(3, 9) && above(2, 8) && above(0, 5);
        let spaced = slot == 2 || (!free(state, slot - 1) && turn - designed(state, slot - 1) > 12);
        if !free(state, slot) || !needs || !spaced {
            continue;
        }
        let base = if (slot - 2) % 2 == 0 { 0 } else { 4 };
        draw_random(
            state,
            player,
            slot,
            31,
            &fitting::META_MORPHS[base..base + 4],
            rng,
            report,
        );
    }
    // The battleships, slots 6 and 7.
    for slot in 6..8usize {
        let needs = above(5, 3)
            && above(4, 9)
            && above(3, 11)
            && above(2, 11)
            && above(0, 5)
            && above(1, 14);
        let spaced = slot == 6 || (!free(state, slot - 1) && turn - designed(state, slot - 1) > 20);
        if !free(state, slot) || !needs || !spaced {
            continue;
        }
        let set = if slot == 6 {
            &fitting::BATTLESHIPS_6
        } else {
            &fitting::BATTLESHIPS_7
        };
        draw_random(state, player, slot, 9, set, rng, report);
    }
    // The bombers, slots 9 and 10.
    for slot in 9..11usize {
        let spaced = slot == 9 || (!free(state, slot - 1) && turn - designed(state, slot - 1) > 15);
        if !free(state, slot) || !above(1, 13) || !spaced {
            continue;
        }
        if !draw(
            state,
            player,
            slot,
            9,
            &[fitting::BATTLESHIP_BOMBER],
            rng,
            report,
        ) {
            let fit = fitting::B52S[usize::from(slot != 9)];
            draw(state, player, slot, 19, &[fit], rng, report);
        }
    }
    // The mine layer, slot 0, in place of whatever the player began with.
    let slot0_frigate = state.designs[player]
        .first()
        .is_some_and(|d| d.hull_id == 5);
    if !slot0_frigate
        && skill > 1
        && !exists(state, LAYER_SLOT)
        && above(5, 3)
        && above(4, 4)
        && above(3, 5)
        && above(2, 5)
        && above(0, 5)
    {
        if let Some(old) = state.designs[player].first_mut() {
            old.obsolete = true;
        }
        draw(state, player, 0, 5, &[fitting::FRIGATE_LAYER], rng, report);
    }
}

/// The Robotoid's turn.
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
    state.players[player].explored.extend(seen.into_keys());
    let explored = state.players[player].explored.clone();

    // `IroEnsureAi(vrgbRobotoidRes, 36, &ishdefSBLatest, pct)`.
    report.research = ensure_research(state, player, profile.plan, profile.research_pct(turn));
    validate_starbase_history(state, player, me);
    let history_count =
        i32::try_from(state.players[player].starbase_history.len()).unwrap_or(i32::MAX);

    // `MergeAllShdefs` from turn 51: the warships and bombers (`0x6fc`),
    // the mine layers (`1`) and the armada ships (`0xc000`).
    if turn > 50 {
        for mask in [0x06fcu16, 0x0001, 0xc000] {
            merge_all(state, me, mask, &mut report);
        }
    }
    let potency = potency(turn);
    state.ai_armada_potency = potency;

    // `CheckAiShdefStatus` over the slot ranges, with the freighters'
    // recycling period half as long again; a Nubian in 14 or 15 is never
    // counted old.
    let recycle: i16 = if turn < 120 {
        50
    } else if turn < 200 {
        70
    } else {
        100
    };
    let mut old = [false; 16];
    let armada = check_status(state, player, me, 14, 15, recycle, &mut old);
    for (slot, is_old) in old.iter_mut().enumerate().skip(14) {
        if *is_old
            && state.designs[player]
                .get(slot)
                .is_some_and(|d| !d.obsolete && d.hull_id == 29)
        {
            *is_old = false;
        }
    }
    let cruisers = check_status(state, player, me, 11, 13, recycle, &mut old);
    let bombers = check_status(state, player, me, 9, 10, recycle, &mut old);
    let warships = check_status(state, player, me, 2, 5, recycle, &mut old);
    let battleships = check_status(state, player, me, 6, 7, recycle * 3 / 2, &mut old);
    // `SplitOutShdefs` from turn 81: the old designs, then the mine layers,
    // the colony ships and the cruisers each into fleets of their own.
    if turn > 80 {
        split_out_designs(state, player, me, &old, &mut report);
        let mut only = [false; 16];
        only[0] = true;
        split_out_designs(state, player, me, &only, &mut report);
        let mut only = [false; 16];
        only[1] = true;
        split_out_designs(state, player, me, &only, &mut report);
        let mut only = [false; 16];
        only[11] = true;
        only[12] = true;
        only[13] = true;
        split_out_designs(state, player, me, &only, &mut report);
    }
    ensure_designs(state, player, skill, rng, &mut report);

    // How many fleets carry an armada ship.
    let armada_fleets = if armada.latest.is_some() {
        i32::try_from(
            state
                .fleets
                .iter()
                .filter(|f| f.owner == me)
                .filter(|f| {
                    f.stacks
                        .iter()
                        .any(|s| (s.design == 14 || s.design == 15) && s.count > 0)
                })
                .count(),
        )
        .unwrap_or(i32::MAX)
    } else {
        0
    };
    let (build_colonizers, colony_fleets) = should_build_colonizers(state, player, me, skill, rng);

    // The other players' planets are worth a point per 250 colonists, at
    // most six, and one more for a starbase (`vlpbAiPlanet[+10]`); the
    // byte at `+9` says they are somebody's.
    let mut marks = marks(state, player, me, &explored, AiPersonality::Robotoid);
    let colony_design = colony_design(state, player);

    // --- The planet pass: the queue at every planet with a starbase and
    // people enough.
    let planet_count = i32::try_from(state.planets.len()).unwrap_or(0);
    let owned =
        i32::try_from(state.planets.iter().filter(|p| p.owner == Some(me)).count()).unwrap_or(0);
    let race = state.players[player].race.clone();
    let levels = state.players[player].research.levels;
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
    let order: Vec<i16> = crate::ai::planet_order(
        &state
            .planets
            .iter()
            .filter(|p| p.owner == Some(me))
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        rng,
        true,
    );
    for id in order {
        let Some(index) = state.planets.iter().position(|p| p.id == id) else {
            continue;
        };
        let planet = &state.planets[index];
        if !planet.starbase || planet.pop < QUEUE_MIN_POP {
            continue;
        }
        if planet.queue.iter().any(|q| q.ship) {
            continue;
        }
        let mut added: Vec<(u8, i32)> = Vec::new();
        // Cruisers: up to the larger of four times the starbase history
        // and an eighth of the planets owned, the last fifth of them one
        // roll in three.
        let want_cruisers = (history_count * 4).max(owned / 8);
        if let Some(latest) = cruisers.latest {
            let count = cruisers.count;
            if count < want_cruisers * 8 / 10 || (count < want_cruisers && rng.random(3) == 0) {
                added.push((latest, 1));
            }
        }
        // Colony ships: two a year before turn 21 and one after, with one
        // more where the planet grows past 2,300 a year and makes over 35
        // resources, and a second past 3,600 and 50, by skill.
        let build_here = build_colonizers
            || (colony_fleets < 26
                && rng.random(i16::try_from(history_count * 8).unwrap_or(i16::MAX)) == 0);
        if build_here && turn > 4 {
            let mut n = if turn < 21 { 2 } else { 1 };
            let growth =
                i64::from(planet.pop) * i64::from(crate::population::pct_true_max_growth(&race));
            let resources = i32::from(
                crate::resources::resources_at_planet(planet, &race, i16::from(levels[0]))
                    .unwrap_or(0),
            );
            if growth > 2300 && resources > 35 && skill > 0 {
                n += 1;
                if growth > 3600 && resources > 50 && skill > 1 {
                    n += 1;
                }
            }
            if let Some(design) = colony_design {
                added.push((design, n));
            }
        }
        // Mine layers, four at a time: a Frigate in slot 0, one roll in
        // four, the fleet of them here under ten (under seventeen with one
        // in ten), and a roll of `2 × count + 1` coming up zero.
        let layer_frigate = state.designs[player]
            .first()
            .is_some_and(|d| d.hull_id == 5 && !d.obsolete);
        if layer_frigate && rng.random(4) == 0 {
            let here = state
                .fleets
                .iter()
                .find(|f| {
                    f.owner == me
                        && f.orbiting == Some(u16::try_from(id).unwrap_or(u16::MAX))
                        && f.stacks
                            .iter()
                            .any(|s| s.design == LAYER_SLOT && s.count > 0)
                })
                .map_or(0, |f| {
                    f.stacks
                        .iter()
                        .filter(|s| s.design == LAYER_SLOT)
                        .map(|s| s.count)
                        .sum::<i32>()
                });
            if (here < 10 || (here < 17 && rng.random(10) == 0))
                && rng.random(i16::try_from(here * 2 + 1).unwrap_or(i16::MAX)) == 0
            {
                added.push((LAYER_SLOT, 4));
            }
        }
        let rich = planet.surface_min.iter().all(|m| *m >= 5000);
        // Bombers: where a potent war fleet of ours sits with fewer bombers
        // than the third potency, four of them — six on a rich planet — and
        // nothing more this year.
        let mut finished = false;
        if let Some(latest) = bombers.latest {
            let potent = state
                .fleets
                .iter()
                .find(|f| {
                    f.owner == me
                        && f.orbiting == Some(u16::try_from(id).unwrap_or(u16::MAX))
                        && is_potent_war_fleet(f, &potency)
                })
                .map(|f| {
                    f.stacks
                        .iter()
                        .filter(|s| s.design == 9 || s.design == 10)
                        .map(|s| s.count)
                        .sum::<i32>()
                });
            if let Some(aboard) = potent {
                if aboard < i32::from(potency[2]) {
                    added.push((latest, if rich { 6 } else { 4 }));
                    finished = true;
                }
            }
        }
        // Warships, paid for out of what is left after the queue: the
        // newest of slots 2 to 5 — or, one roll in two, the newest
        // battleship — while under a seventh of the planets plus six
        // exist, or with one roll in two past that; three fifths of the
        // design's cost must be there; five on a rich planet, else one.
        let mut to_armada = !finished;
        if !finished {
            if let Some(latest) = warships.latest {
                let existing = i64::from(ships_of(state, latest));
                let limit = i64::from(planet_count / 7 + 6);
                let skip = existing >= limit && rng.random(2) == 0;
                if !skip {
                    let choose = match battleships.latest {
                        Some(b) if rng.random(2) == 0 => b,
                        _ => latest,
                    };
                    let mut left = crate::ai::production::resources_available(
                        &state.planets[index],
                        &race,
                        state.players[player].research_pct,
                        i16::from(levels[0]),
                    );
                    let committed =
                        crate::ai::production::queue_cost(&state.planets[index].queue, &race);
                    let mut paid = true;
                    for (have, spent) in left.iter_mut().zip(committed.iter()) {
                        *have -= spent;
                        if *have < 0 {
                            paid = false;
                        }
                    }
                    if !paid {
                        to_armada = false;
                    } else {
                        let who = Builder::player(&state.players[player]);
                        let cost = state.designs[player]
                            .get(usize::from(choose))
                            .and_then(|d| d.true_cost(&who));
                        if let Some(cost) = cost {
                            let each = [
                                cost.minerals[0] * 3 / 5,
                                cost.minerals[1] * 3 / 5,
                                cost.minerals[2] * 3 / 5,
                                cost.resources * 3 / 5,
                            ];
                            let mut ok = true;
                            for (have, spent) in left.iter_mut().zip(each.iter()) {
                                *have -= spent;
                                if *have < 0 {
                                    ok = false;
                                }
                            }
                            if ok {
                                added.push((choose, if rich { 5 } else { 1 }));
                            }
                        }
                    }
                }
            }
        }
        // Armada ships: the newest of 14 and 15, while under a twelfth of
        // the planets plus eight exist, up to five paid for in full out of
        // what is left, stopping at the first that cannot be.
        if to_armada {
            if let Some(latest) = armada.latest {
                let existing = i64::from(ships_of(state, latest));
                if existing < i64::from(planet_count / 12 + 8) {
                    let mut left = crate::ai::production::resources_available(
                        &state.planets[index],
                        &race,
                        state.players[player].research_pct,
                        i16::from(levels[0]),
                    );
                    let committed =
                        crate::ai::production::queue_cost(&state.planets[index].queue, &race);
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
                            let mut n = 0;
                            for _ in 0..5 {
                                let each = [
                                    cost.minerals[0],
                                    cost.minerals[1],
                                    cost.minerals[2],
                                    cost.resources,
                                ];
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
    }

    // --- The first fleet pass.
    let mut worth = mineral_worth(state, &explored, marks.len());
    let positions: Vec<(i16, Point)> = state
        .planets
        .iter()
        .filter_map(|p| p.position.map(|at| (p.id, at)))
        .collect();
    let position_of = |id: i16| positions.iter().find(|(p, _)| *p == id).map(|(_, at)| *at);
    let mut attack_fleets: Vec<u16> = Vec::new();
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    for index in 0..state.fleets.len() {
        let fleet = state.fleets[index].clone();
        if fleet.owner != me || fleet.is_empty() {
            continue;
        }
        let fleet_id = fleet.id;
        let has_leg = fleet.waypoints.len() > 1;
        let count = |slot: u8| -> i32 {
            fleet
                .stacks
                .iter()
                .filter(|s| s.design == slot)
                .map(|s| s.count)
                .sum()
        };
        // A leg to a space object more than 200 light years off is blown
        // away.
        if has_leg && fleet.waypoints[1].target_class == grobj::THING {
            let dx = i64::from(fleet.position.x) - i64::from(fleet.waypoints[1].position.x);
            let dy = i64::from(fleet.position.y) - i64::from(fleet.waypoints[1].position.y);
            if dx * dx + dy * dy > 40_000 {
                state.fleets[index].waypoints.truncate(1);
                report.cleaned.push(fleet_id);
            }
        }
        let has_leg = state.fleets[index].waypoints.len() > 1;
        let layers = count(LAYER_SLOT);
        let orbiting = fleet.orbiting.and_then(|p| i16::try_from(p).ok());
        let mut to_tail = true;
        if layers >= 1 && turn >= 41 && !has_leg {
            // A fleet of mine layers: one in five of the larger ones wanders
            // to a planet within range; the rest lay mines where they sit.
            let mut wandered = false;
            if layers > 6 && rng.random(5) == 0 {
                if let Some(id) =
                    random_planet_nearby(state, fleet.position, WANDER_RANGE, true, rng)
                {
                    if Some(id) != orbiting {
                        if let Some(at) = position_of(id) {
                            lay_leg(
                                &mut state.fleets[index],
                                at,
                                id,
                                stars_formats::task::NONE,
                                4,
                            );
                            report.scouted.push((fleet_id, id));
                            wandered = true;
                        }
                    }
                }
            }
            if !wandered {
                let first = &mut state.fleets[index].waypoints[0];
                if first.task == stars_formats::task::NONE {
                    first.task = stars_formats::task::LAY_MINES;
                    first.task_data = vec![5, 0];
                    report.laying.push(fleet_id);
                }
                to_tail = false;
            }
        } else if is_attack_fleet(state, player, &fleet) {
            attack_fleets.push(fleet_id);
            if state.fleets[index].waypoints[0].task == stars_formats::task::LAY_MINES {
                state.fleets[index].waypoints[0].task = stars_formats::task::NONE;
                state.fleets[index].waypoints[0].task_data = Vec::new();
            }
            // A war fleet claims the planet it is bound for, or sits at.
            let warship_aboard = (2..8u8).any(|slot| count(slot) > 0);
            if warship_aboard {
                let dest =
                    if has_leg && state.fleets[index].waypoints[1].target_class == grobj::PLANET {
                        state.fleets[index].waypoints[1]
                            .target
                            .and_then(|t| i16::try_from(t).ok())
                    } else {
                        orbiting
                    };
                if let Some(w) = dest
                    .and_then(|d| usize::try_from(d).ok())
                    .and_then(|i| worth.get_mut(i))
                {
                    *w |= 0x80;
                }
            }
        } else if is_transport(state, player, &fleet) {
            // A transport bound for a planet that is not ours, with nothing
            // aboard, has its orders blown away.
            let dest =
                if !has_leg || state.fleets[index].waypoints[0].task != stars_formats::task::NONE {
                    orbiting
                } else if state.fleets[index].waypoints[1].target_class == grobj::PLANET {
                    state.fleets[index].waypoints[1]
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
                    let f = &mut state.fleets[index];
                    f.waypoints.truncate(1);
                    f.waypoints[0].task = stars_formats::task::NONE;
                    f.waypoints[0].task_data = Vec::new();
                    f.waypoints[0].transport = None;
                    report.cleaned.push(fleet_id);
                }
            }
        }
        if !to_tail {
            continue;
        }
        // Before turn 21 the scouts the player began with are scrapped.
        let has_leg = state.fleets[index].waypoints.len() > 1;
        if turn < 21 && layers > 0 {
            let f = &mut state.fleets[index];
            f.waypoints.truncate(1);
            f.waypoints[0].task = stars_formats::task::SCRAP;
            report.scrapped.push(fleet_id);
            continue;
        }
        // Colony ships with no orders, from turn 5 (at once when the
        // players started close).
        if !has_leg && count(COLONY_SLOT) > 0 && (turn > 4 || state.start_distance == 0) {
            // At somebody else's planet, with colonists aboard and a skill
            // over 1: the colonists go down as an invasion, and the fleet
            // heads for the nearest own starbase.
            if skill > 1 {
                if let Some(here) = orbiting {
                    let theirs = state
                        .planets
                        .iter()
                        .find(|p| p.id == here)
                        .filter(|p| p.owner.is_some_and(|o| o != me))
                        .map(|p| p.owner.unwrap_or(-1));
                    let ar = theirs
                        .and_then(|o| usize::try_from(o).ok())
                        .and_then(|o| state.players.get(o))
                        .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Ar));
                    if theirs.is_some() && fleet.cargo.colonists > 0 && !ar {
                        use stars_formats::{ItemAction, TransportTask, XferAction};
                        let mut items = [ItemAction {
                            quantity: 0,
                            action: XferAction::None,
                        }; 5];
                        items[3] = ItemAction {
                            quantity: 0,
                            action: XferAction::UnloadAll,
                        };
                        // `FMoveAiFleet` folds an order for the spot the
                        // fleet stands on into its first waypoint.
                        let f = &mut state.fleets[index];
                        f.waypoints.truncate(1);
                        let first = &mut f.waypoints[0];
                        first.task = stars_formats::task::TRANSPORT;
                        first.transport = Some(TransportTask { items });
                        first.task_data = Vec::new();
                        report.dropping.push((fleet_id, here));
                        move_to_nearest_starbase(state, me, index, false);
                        continue;
                    }
                }
            }
            let candidates: Vec<(i16, (i32, i32), Mark)> = positions
                .iter()
                .map(|(id, at)| {
                    let mark = usize::try_from(*id)
                        .ok()
                        .and_then(|i| marks.get(i).copied())
                        .unwrap_or(Mark::Unknown);
                    (*id, (i32::from(at.x), i32::from(at.y)), mark)
                })
                .collect();
            let from = fleet.position;
            let target = nearest_colonisable((i32::from(from.x), i32::from(from.y)), &candidates);
            let Some(target) = target else {
                // Nowhere to settle: broken up where it sits.
                if orbiting.is_some() {
                    let f = &mut state.fleets[index];
                    f.waypoints.truncate(1);
                    f.waypoints[0].task = stars_formats::task::SCRAP;
                    report.scrapped.push(fleet_id);
                }
                continue;
            };
            // A thousand colonists from the planet it sits at, when ours.
            if let Some(planet) = orbiting
                .and_then(|p| state.planets.iter().position(|q| q.id == p))
                .filter(|&p| state.planets[p].owner == Some(me))
            {
                let capacity =
                    state.fleets[index].cargo_capacity(&designs) - state.fleets[index].cargo.mass();
                let take = COLONISTS_ABOARD
                    .min(state.planets[planet].pop)
                    .min(capacity)
                    .max(0);
                state.planets[planet].pop -= take;
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
        }
    }

    // --- The haulers, once there is a starbase to work from: every
    // transport with no orders is put on battle plan 4 and sent by
    // `IdTargetFreighter` from its starbase-history planet.
    let first_starbase = state
        .planets
        .iter()
        .find(|p| p.owner == Some(me) && p.starbase)
        .map(|p| p.id);
    if let Some(first_starbase) = first_starbase {
        let valued = valued_planets(state, player, me);
        for index in 0..state.fleets.len() {
            let fleet = &state.fleets[index];
            if fleet.owner != me || fleet.is_empty() || fleet.waypoints.len() > 1 {
                continue;
            }
            if !is_transport(state, player, fleet) {
                continue;
            }
            let fleet_id = fleet.id;
            if fleet.battle_plan != 4 {
                state.fleets[index].battle_plan = 4;
            }
            let home = state.players[player]
                .starbase_history
                .iter()
                .find(|e| e.fleets.contains(&fleet_id))
                .map_or(first_starbase, |e| e.planet);
            if let Some(to) = target_freighter(state, player, me, index, &worth, home, &valued, rng)
            {
                report.hauling.push((fleet_id, to));
            }
        }
    }

    // --- The last pass: old ships home to be scrapped, war fleets to
    // their targets, the armada ships after the enemy.
    for index in 0..state.fleets.len() {
        let fleet = state.fleets[index].clone();
        if fleet.owner != me || fleet.is_empty() {
            continue;
        }
        let fleet_id = fleet.id;
        let all_old = fleet
            .stacks
            .iter()
            .filter(|s| s.count > 0)
            .all(|s| old.get(usize::from(s.design)) == Some(&true));
        let orbiting = fleet.orbiting.and_then(|p| i16::try_from(p).ok());
        if all_old {
            let own_planet = orbiting.and_then(|id| state.planets.iter().find(|p| p.id == id));
            if let Some(planet) = own_planet.filter(|p| p.owner == Some(me)) {
                // At an own planet: scrapped there — at one without a
                // starbase only one time in five.
                if planet.starbase || rng.random(5) == 0 {
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
        let warship = fleet
            .stacks
            .iter()
            .any(|s| (2..11).contains(&s.design) && s.count > 0);
        if warship {
            if let Some(to) = target_armada(state, player, me, index, &potency, rng) {
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
        // With enough armada fleets about — seventy before turn 121, fifty
        // after, or sixty and forty with one roll in three — a smaller one
        // joins a bigger, unless it already holds twenty of the newest
        // armada ship and rolls nineteen in twenty.
        let (many, some) = if turn < 121 { (70, 60) } else { (50, 40) };
        let join = armada_fleets > many || (armada_fleets > some && rng.random(3) == 0);
        if join {
            let big = armada.latest.is_some_and(|latest| {
                fleet
                    .stacks
                    .iter()
                    .any(|s| s.design == latest && s.count > 19)
            });
            if !(big && rng.random(20) != 0)
                && find_buddy_and_join_up(state, me, index, 14, 15, 36, 72, rng)
            {
                report.merged.push((fleet_id, fleet_id));
                continue;
            }
        }
        if let Some(to) = target_attack(state, player, me, index, &attack_fleets, rng) {
            report.attacking.push((fleet_id, to));
        }
    }

    basic_tasks(
        state,
        player,
        me,
        &worth,
        AiPersonality::Robotoid,
        rng,
        &mut report,
    );
    fill_production_queues(state, player, me, AiPersonality::Robotoid, rng, &mut report);
    report
}

/// `FFindBuddyAndJoinUp(fleet, ishLo, ishHi, dist1, dist2)` (`1090:9d18`):
/// the nearest other fleet of ours with ships in the slots named; joined
/// — a leg to where it stands at warp 6, the merge being `MergeAllShdefs`'
/// business next year — when within `dist1`, or within `dist2` one time
/// in two.
#[allow(clippy::too_many_arguments)]
pub(crate) fn find_buddy_and_join_up(
    state: &mut GameState,
    me: i16,
    index: usize,
    lo: u8,
    hi: u8,
    dist1: i64,
    dist2: i64,
    rng: &mut Rng,
) -> bool {
    let from = state.fleets[index].position;
    let mut best: Option<(i64, usize)> = None;
    for (i, other) in state.fleets.iter().enumerate() {
        if i == index || other.owner != me || other.is_empty() {
            continue;
        }
        if !other
            .stacks
            .iter()
            .any(|s| (lo..=hi).contains(&s.design) && s.count > 0)
        {
            continue;
        }
        let dx = i64::from(other.position.x) - i64::from(from.x);
        let dy = i64::from(other.position.y) - i64::from(from.y);
        let d2 = dx * dx + dy * dy;
        if best.is_none_or(|(b, _)| d2 < b) {
            best = Some((d2, i));
        }
    }
    let Some((d2, buddy)) = best else {
        return false;
    };
    if d2 > dist1 * dist1 && (d2 > dist2 * dist2 || rng.random(2) == 0) {
        return false;
    }
    let (at, word) = {
        let b = &state.fleets[buddy];
        (
            b.position,
            (u16::try_from(b.owner).unwrap_or(0) << 9) | (b.id & 0x1ff),
        )
    };
    let fleet = &mut state.fleets[index];
    fleet.waypoints.truncate(1);
    fleet.waypoints[0].task = stars_formats::task::NONE;
    fleet.waypoints[0].task_data = Vec::new();
    fleet.waypoints.push(crate::fleet::Waypoint {
        position: at,
        target: Some(word),
        target_class: grobj::FLEET,
        warp: 6,
        task: stars_formats::task::NONE,
        transport: None,
        task_data: Vec::new(),
    });
    fleet.warp = Some(6);
    true
}

/// `IdTargetAttack` (`1090:1ffe`): where an armada ship goes.
///
/// The nearest of the other players' fleets — a computer player's passed
/// over when the computer players band together — that not too many of
/// ours are after: one already chased is passed over one time in three,
/// and one chased by five times our own ships one in fifteen. Within 180
/// light years it is the target. Further off, a fleet with under half its
/// fuel goes home to a starbase first; else the nearest planet to that
/// fleet that none of ours is bound for. With no enemy fleet in view at
/// all: the nearest planet of somebody else's, or failing that the nearest
/// planet nobody holds, or failing that one at random. The leg is laid at
/// warp 4 unless the fleet is already bound for the planet chosen.
pub(crate) fn target_attack(
    state: &mut GameState,
    player: usize,
    me: i16,
    index: usize,
    attack_fleets: &[u16],
    rng: &mut Rng,
) -> Option<i16> {
    let fleet = state.fleets[index].clone();
    let ships: i32 = fleet.stacks.iter().map(|s| s.count).sum();
    let from = fleet.position;
    let only_humans = state.ais_band;
    let chased_by = |state: &GameState, target: u16| -> i32 {
        state
            .fleets
            .iter()
            .filter(|f| f.owner == me && f.id != fleet.id && attack_fleets.contains(&f.id))
            .filter(|f| {
                f.waypoints.len() > 1
                    && f.waypoints[1].target_class == grobj::FLEET
                    && f.waypoints[1].target == Some(target)
            })
            .map(|f| f.stacks.iter().map(|s| s.count).sum::<i32>())
            .sum()
    };
    let d2 = |a: Point, b: Point| {
        let dx = i64::from(a.x) - i64::from(b.x);
        let dy = i64::from(a.y) - i64::from(b.y);
        dx * dx + dy * dy
    };
    let mut best: Option<(i64, u16, Point)> = None;
    for pass in 0..2 {
        let bands = only_humans && pass == 0;
        for other in state
            .fleets
            .iter()
            .filter(|f| f.owner != me && !f.is_empty())
        {
            let computer = usize::try_from(other.owner)
                .ok()
                .and_then(|o| state.players.get(o))
                .is_some_and(|p| p.control.is_computer());
            if bands && computer {
                continue;
            }
            let word = (u16::try_from(other.owner).unwrap_or(0) << 9) | (other.id & 0x1ff);
            let already = chased_by(state, word);
            if already > 0 && rng.random(3) == 0 {
                continue;
            }
            if already * 5 > ships && rng.random(15) == 0 {
                continue;
            }
            let d = d2(other.position, from);
            if best.is_none_or(|(b, _, _)| d < b) {
                best = Some((d, word, other.position));
            }
        }
        if best.is_some() || !bands {
            break;
        }
    }
    let target: (Point, Option<u16>, u8) = match best {
        Some((d, word, at)) if d < 0x7e90 => (at, Some(word), grobj::FLEET),
        Some((_, _, at)) => {
            let designs = state.designs.get(player).cloned().unwrap_or_default();
            let capacity = fleet.fuel_capacity(&designs);
            if fleet.cargo.fuel < capacity / 2 && move_to_nearest_starbase(state, me, index, false)
            {
                return None;
            }
            let claimed: BTreeSet<u16> = state
                .fleets
                .iter()
                .filter(|f| f.owner == me && f.id != fleet.id && attack_fleets.contains(&f.id))
                .filter(|f| f.waypoints.len() > 1 && f.waypoints[1].target_class == grobj::PLANET)
                .filter_map(|f| f.waypoints[1].target)
                .collect();
            let mut nearest: Option<(i64, i16, Point)> = None;
            for planet in &state.planets {
                let Some(p) = planet.position else { continue };
                if claimed.contains(&u16::try_from(planet.id).unwrap_or(u16::MAX)) {
                    continue;
                }
                let d = d2(p, at);
                if nearest.is_none_or(|(b, _, _)| d < b) {
                    nearest = Some((d, planet.id, p));
                }
            }
            match nearest {
                Some((_, id, p)) if fleet.orbiting != u16::try_from(id).ok() => {
                    (p, u16::try_from(id).ok(), grobj::PLANET)
                }
                _ => nearest_planet_of_anyone(state, me, &fleet, rng)?,
            }
        }
        None => nearest_planet_of_anyone(state, me, &fleet, rng)?,
    };
    let (at, word, class) = target;
    let bound_for_it = fleet.waypoints.len() > 1
        && fleet.waypoints[1].target_class == grobj::PLANET
        && class == grobj::PLANET;
    if bound_for_it {
        return None;
    }
    let f = &mut state.fleets[index];
    f.waypoints.truncate(1);
    f.waypoints.push(crate::fleet::Waypoint {
        position: at,
        target: word,
        target_class: class,
        warp: 4,
        task: stars_formats::task::NONE,
        transport: None,
        task_data: Vec::new(),
    });
    f.warp = Some(4);
    word.and_then(|w| i16::try_from(w).ok())
}

/// The nearest planet somebody else holds, else the nearest nobody holds,
/// else one at random — the tail of `IdTargetAttack`.
fn nearest_planet_of_anyone(
    state: &GameState,
    me: i16,
    fleet: &crate::fleet::Fleet,
    rng: &mut Rng,
) -> Option<(Point, Option<u16>, u8)> {
    let from = fleet.position;
    let d2 = |p: Point| {
        let dx = i64::from(p.x) - i64::from(from.x);
        let dy = i64::from(p.y) - i64::from(from.y);
        dx * dx + dy * dy
    };
    let nearest = |theirs: bool| -> Option<(i16, Point)> {
        let mut best: Option<(i64, i16, Point)> = None;
        for planet in &state.planets {
            let Some(p) = planet.position else { continue };
            let wanted = if theirs {
                planet.owner.is_some_and(|o| o != me)
            } else {
                planet.owner.is_none()
            };
            if !wanted {
                continue;
            }
            let d = d2(p);
            if best.is_none_or(|(b, _, _)| d < b) {
                best = Some((d, planet.id, p));
            }
        }
        best.map(|(_, id, p)| (id, p))
    };
    let here = fleet.orbiting.and_then(|p| i16::try_from(p).ok());
    let pick = match nearest(true) {
        Some((id, p)) if Some(id) != here => Some((id, p)),
        _ => match nearest(false) {
            Some(found) => Some(found),
            None => {
                let n = i16::try_from(state.planets.len()).unwrap_or(1).max(1);
                let i = usize::try_from(rng.random(n))
                    .unwrap_or(0)
                    .min(state.planets.len() - 1);
                let planet = state.planets.get(i)?;
                planet.position.map(|p| (planet.id, p))
            }
        },
    };
    pick.map(|(id, p)| (p, u16::try_from(id).ok(), grobj::PLANET))
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
        for f in fitting::META_MORPHS
            .iter()
            .chain(fitting::META_MORPH_CRUISERS.iter())
        {
            assert_eq!(f.len(), slots(31));
        }
        for f in &fitting::PRIVATEERS {
            assert_eq!(f.len(), slots(11));
        }
        for f in fitting::DESTROYERS_14
            .iter()
            .chain(fitting::DESTROYERS_15.iter())
        {
            assert_eq!(f.len(), slots(6));
        }
        for f in &fitting::B52S {
            assert_eq!(f.len(), slots(19));
        }
        for f in fitting::BATTLESHIPS_6
            .iter()
            .chain(fitting::BATTLESHIPS_7.iter())
            .chain(std::iter::once(&fitting::BATTLESHIP_BOMBER))
        {
            assert_eq!(f.len(), slots(9));
        }
        assert_eq!(fitting::NUBIAN.len(), slots(29));
        assert_eq!(fitting::FRIGATE_LAYER.len(), slots(5));
        for f in fitting::META_MORPHS
            .iter()
            .chain(std::iter::once(&fitting::NUBIAN))
        {
            for class in f.iter() {
                assert!(usize::from(*class) < crate::ai::parts::PART_CLASSES.len());
            }
        }
    }

    #[test]
    fn the_potencies_follow_the_years() {
        assert_eq!(potency(0), [4, 2, 6, 2]);
        assert_eq!(potency(115), [4, 2, 6, 2]);
        assert_eq!(potency(116), [4, 2, 6, 2]);
        assert_eq!(potency(122), [4, 2, 7, 2]);
        assert_eq!(potency(140), [5, 2, 7, 2]);
        assert_eq!(potency(188), [7, 3, 10, 3]);
        assert_eq!(potency(300), [13, 6, 12, 3]);
        assert_eq!(potency(2000), [50, 25, 12, 3]);
    }
}
