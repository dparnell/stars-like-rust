//! The Cybertron's turn: `DoCyberAiTurn` (`10a8:002a`), its designs
//! (`EnsureCyberAiShdefs`, `10a8:4826`), its armada (`TargetCyberArmada`,
//! `10a8:51a4`), its colonist freighters (`DoCyberFreighter`,
//! `10a8:37b0`), its warship queue (`iAddAttackFleet`, `10a8:4eba`) and
//! its mass drivers (`DoCyberPackets`, `10a8:1a78`), with the shared
//! routines reused from [`super::turindrone`] and [`super::robotoid`].
//!
//! The Cybertron is the Packet Physics opponent and plays unlike the
//! others: it spreads its people by **freighter** rather than colony
//! ship, throws mineral packets at its own short planets and at its
//! enemies, fires packets into the dark to scan with them, and keeps two
//! four-slot warship groups (6–9 and 10–13: cruisers or battleships, a
//! Nubian at the top, a bomber in the last) that its armadas are built
//! from. Its slots: **Frigate mine layers in 0**, colony ships in 1,
//! Privateer colonist freighters in 2 and 3, Destroyers in 4 and 5, the
//! two groups, and starbase defenders in 14 and 15. See
//! `docs/formulas/ai.md`, *The Cybertron's turn*.
//!
//! Its per-planet scratch (`vlpbAiData`, read as words) has a lasting
//! half — [`crate::Player::cyber_words`] — and a half zeroed each year.
//! Its armada reads `vrgAiArmadaPotency`, which its own turn never sets
//! ([`crate::GameState::ai_armada_potency`]).

use std::collections::BTreeSet;

use crate::ai::colonise::{nearest_colonisable, Mark};
use crate::ai::dispatch::ideal_warp;
use crate::ai::parts::{create_design, Fitting};
use crate::ai::personality::Profile;
use crate::ai::robotoid::{find_buddy_and_join_up, random_planet_nearby, target_attack};
use crate::ai::turindrone::{
    armada_dest, basic_tasks, check_status, ensure_research, fill_production_queues,
    install_design, lay_leg, marks, merge_all, mineral_worth, move_to_nearest_starbase,
    split_out_designs, Report,
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
/// How many colonists a colony ship is loaded with: `XferAiSupply(…, 3,
/// 0xfa)`, twenty-five thousand people.
pub const COLONISTS_ABOARD: i32 = 250;
/// How many colonists a freighter loads or unloads at a time
/// (`XferAiSupply(…, 3, ±1000)`).
pub const FREIGHTER_LOAD: i32 = 1000;
/// How far `MoveToNearestPlanetOrEnemy` looks for an enemy planet.
pub const ENEMY_RANGE: i64 = 450;
/// One mineral packet's worth, in kT (`0x46`).
pub const PACKET_KT: i32 = 70;

/// The fittings, from `10a8:46f8` by the word offsets at `10a8:46b0`;
/// each byte is a part class ([`crate::ai::parts::PART_CLASSES`]) per hull
/// slot.
pub mod fitting {
    use super::Fitting;
    /// Entries 0 to 9: the Destroyers (6) of slots 4 and 5 — slot 4 draws
    /// from the first five, slot 5 from the last five.
    pub const DESTROYERS: [Fitting; 10] = [
        &[8, 4, 4, 18, 17, 18, 20],
        &[8, 4, 4, 5, 17, 18, 20],
        &[8, 4, 4, 4, 17, 18, 19],
        &[8, 3, 3, 14, 17, 18, 19],
        &[8, 4, 3, 2, 17, 18, 20],
        &[8, 0, 0, 18, 17, 18, 19],
        &[8, 0, 0, 10, 17, 18, 19],
        &[8, 0, 0, 11, 17, 18, 19],
        &[8, 1, 1, 11, 17, 18, 19],
        &[8, 1, 1, 11, 17, 18, 11],
    ];
    /// Entries 10 and 11: the Privateer (11) colonist freighters of slots
    /// 2 and 3.
    pub const PRIVATEERS: [Fitting; 2] = [&[44, 10, 15, 4, 4], &[44, 17, 11, 0, 0]];
    /// Entry 12: the Frigate (5) mine layer of slot 0.
    pub const FRIGATE_LAYER: Fitting = &[24, 26, 25, 10];
    /// Entries 13 and 14: the B-52 (19) bombers tried last for the top of
    /// a group.
    pub const B52S: [Fitting; 2] = [&[8, 21, 23, 23, 23, 12, 10], &[8, 21, 22, 22, 22, 12, 10]];
    /// Entry 15: the Battleship (9) bomber tried second for the top of a
    /// group.
    pub const BATTLESHIP_BOMBER: Fitting = &[8, 14, 10, 33, 33, 33, 33, 33, 17, 20, 19];
    /// Entry 16: the Nubian (29) bomber tried first for the top of a
    /// group.
    pub const NUBIAN_BOMBER: Fitting = &[8, 18, 20, 33, 33, 33, 33, 33, 33, 33, 33, 33, 33];
    /// Entries 17 to 25: the Cruisers (7), in three sets of three.
    pub const CRUISERS: [Fitting; 9] = [
        &[8, 20, 19, 4, 4, 13, 17],
        &[8, 20, 19, 4, 3, 3, 17],
        &[8, 20, 19, 3, 2, 10, 17],
        &[8, 19, 11, 0, 0, 0, 17],
        &[8, 19, 11, 0, 0, 18, 17],
        &[8, 19, 11, 0, 0, 10, 17],
        &[8, 19, 11, 1, 1, 11, 17],
        &[8, 19, 11, 1, 1, 0, 17],
        &[8, 19, 11, 1, 1, 10, 17],
    ];
    /// Entries 26 to 32: the Battleships (9).
    pub const BATTLESHIPS: [Fitting; 7] = [
        &[8, 18, 10, 2, 2, 3, 3, 2, 17, 20, 20],
        &[8, 20, 10, 2, 2, 3, 3, 2, 17, 20, 20],
        &[8, 18, 10, 0, 0, 3, 3, 2, 17, 20, 11],
        &[8, 18, 10, 1, 1, 0, 0, 1, 17, 11, 11],
        &[8, 11, 10, 1, 1, 0, 0, 1, 17, 11, 11],
        &[8, 20, 10, 1, 1, 2, 2, 1, 17, 11, 11],
        &[8, 11, 10, 1, 1, 1, 1, 1, 17, 11, 11],
    ];
    /// Entries 33 to 35: the Nubians (29).
    pub const NUBIANS: [Fitting; 3] = [
        &[8, 11, 11, 1, 1, 1, 20, 20, 2, 3, 3, 15, 19],
        &[8, 11, 11, 1, 1, 1, 1, 1, 1, 19, 19, 15, 19],
        &[8, 20, 20, 2, 2, 2, 3, 3, 3, 19, 19, 15, 19],
    ];
}

/// The Cybertron's own potencies (`vrgAiCyberArmadaPotency`, `10a8:00c0`):
/// three from turn 131 rising by one every twenty years to fifty, and
/// half that; six from turn 116 rising by one every twenty-two years to
/// twelve, and half that less one, at most three. Set and never read —
/// its armada reads the shared table.
#[must_use]
pub fn potency(turn: i16) -> [u8; 4] {
    let a = if turn > 130 { 3 + (turn - 120) / 20 } else { 3 }.min(50);
    let c = if turn > 115 { 6 + (turn - 100) / 22 } else { 6 }.min(12);
    let d = (c / 2 - 1).min(3);
    [
        u8::try_from(a).unwrap_or(50),
        u8::try_from(a / 2).unwrap_or(25),
        u8::try_from(c).unwrap_or(12),
        u8::try_from(d).unwrap_or(3),
    ]
}

/// The attack strength for the year (`10a8:0090`): one, plus one for
/// every ten years past 50, plus from turn 101 the tens past 100 times
/// the hundreds of the year.
#[must_use]
pub fn attack_strength(turn: i16) -> i32 {
    let turn = i32::from(turn);
    let mut s = 1;
    if turn > 50 {
        s = (turn - 50) / 10 + 1;
    }
    if turn > 100 {
        s += (turn - 100) / 10 * (turn / 100);
    }
    s
}

/// The recycling period: 50 years, 70 from turn 120, 100 from 200, 300
/// from 400.
#[must_use]
pub fn recycle_period(turn: i16) -> i16 {
    if turn < 120 {
        50
    } else if turn < 200 {
        70
    } else if turn < 400 {
        100
    } else {
        300
    }
}

/// Whether a starbase design slot is one of the small kinds (`isb & 0xf`
/// of 1, 3, 6 or 8), which the packet routines hold to a lower mineral
/// threshold.
fn small_starbase(planet: &crate::planet::Planet) -> bool {
    planet
        .starbase_design
        .is_some_and(|d| matches!(d & 0xf, 1 | 3 | 6 | 8))
}

/// The lasting word of a planet (`vlpbAiData[id + 1]`): bits 0–2 the
/// scanner-packet direction, bit 3 a colony ship queued last year, bit 4
/// a scanner packet just sent, bits 5–6 a three-year cooldown after
/// packets were thrown at the planet, bit 7 more packets owed to the
/// last target.
mod lasting {
    pub const DIRECTION: u16 = 0x0007;
    pub const COLONIZER: u16 = 0x0008;
    pub const SENT: u16 = 0x0010;
    pub const COOLDOWN: u16 = 0x0060;
    pub const MORE: u16 = 0x0080;
}

/// The year's word of a planet (`vlpbAiData[cPlanMax + 1 + id]`): bit 0
/// a colony ship here found nowhere to settle, bits 1–2 freighters that
/// unloaded here, bits 3–4 freighters bound here, bit 5 starbase
/// defenders here short of the attack strength, bit 6 defenders here,
/// bits 8–10 the planet short of ironium, boranium, germanium. Only the
/// mineral bits are ever set where they are read: the rest are written
/// through a stale pointer (see `turn`) and land past the planet.
mod yearly {
    pub const NOWHERE: u16 = 0x0001;
    pub const UNLOADED: u16 = 0x0006;
    pub const BOUND: u16 = 0x0018;
    pub const DEFENDERS_SHORT: u16 = 0x0020;
    pub const DEFENDERS: u16 = 0x0040;
    pub const SHORT_IRONIUM: u16 = 0x0100;
    pub const SHORT_BORANIUM: u16 = 0x0200;
    pub const SHORT_GERMANIUM: u16 = 0x0400;
}

/// `EnsureCyberAiShdefs` (`10a8:4826`): the designs the personality wants
/// in its slots. `Random(c)` is rolled with `c` counting down from the
/// number of tries, as the routine does.
fn ensure_designs(
    state: &mut GameState,
    player: usize,
    skill: u8,
    rng: &mut Rng,
    report: &mut Report,
) {
    let me = i16::try_from(player).unwrap_or(-1);
    let turn = state.turn;
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
    /// `for (c = tries; c > 0; c--) { i = Random(c); try set[i] }`.
    #[allow(clippy::too_many_arguments)]
    fn draw_counting_down(
        state: &mut GameState,
        player: usize,
        slot: usize,
        hull: i16,
        set: &[Fitting],
        tries: i16,
        rng: &mut Rng,
        report: &mut Report,
    ) -> bool {
        let mut c = tries;
        while c > 0 {
            let i = usize::try_from(rng.random(c)).unwrap_or(0);
            if let Some(f) = set.get(i) {
                if draw(state, player, slot, hull, f, rng, report) {
                    return true;
                }
            }
            c -= 1;
        }
        false
    }

    // Slot 0: the design the player began with, when it is no Frigate, is
    // retired at skill 2 or more once no ship of it is left, from turn 6;
    // a retired slot gets the Frigate mine layer.
    let slot0_frigate = state.designs[player]
        .first()
        .is_some_and(|d| d.hull_id == 5);
    let exists0 = state
        .fleets
        .iter()
        .filter(|f| f.owner == me)
        .any(|f| f.stacks.iter().any(|s| s.design == 0 && s.count > 0));
    if !retired(state, 0) && !slot0_frigate && skill > 1 && !exists0 && turn > 5 {
        state.designs[player][0].obsolete = true;
    }
    if retired(state, 0) {
        draw(state, player, 0, 5, fitting::FRIGATE_LAYER, rng, report);
    }
    // Slots 4 and 5: Destroyers from turn 31 — the first fitting of each
    // set until turn 75, then (or when that fails) a countdown draw.
    if retired(state, 4)
        && turn > 30
        && (turn > 75 || !draw(state, player, 4, 6, fitting::DESTROYERS[0], rng, report))
    {
        draw_counting_down(
            state,
            player,
            4,
            6,
            &fitting::DESTROYERS[0..5],
            5,
            rng,
            report,
        );
    }
    if retired(state, 5)
        && !retired(state, 4)
        && designed(state, 4) + 20 < turn
        && (turn > 75 || !draw(state, player, 5, 6, fitting::DESTROYERS[5], rng, report))
    {
        draw_counting_down(
            state,
            player,
            5,
            6,
            &fitting::DESTROYERS[5..10],
            5,
            rng,
            report,
        );
    }
    // Slots 2 and 3: the colonist freighters, from turn 21, slot 3 twenty
    // years after 2.
    if retired(state, 2) && turn > 20 {
        draw(state, player, 2, 11, fitting::PRIVATEERS[0], rng, report);
    }
    if retired(state, 3) && !retired(state, 2) && designed(state, 2) + 20 < turn {
        draw(state, player, 3, 11, fitting::PRIVATEERS[1], rng, report);
    }
    // The two warship groups, 6–9 and 10–13: drawn when the group's base
    // is retired, the first from turn 41 and the second thirty years
    // after the first's base was designed. The top slot is filled first
    // — a Nubian, else a Battleship — and each success moves the target
    // down; cruisers fill the rest, from sets chosen by the slot; the
    // fourth slot gets a bomber.
    for base in [6usize, 10] {
        let due =
            (base == 6 && turn > 40) || (!retired(state, 6) && designed(state, 6) + 30 < turn);
        if !retired(state, base) || !due {
            continue;
        }
        let mut slot = base + 2;
        if draw_counting_down(state, player, slot, 29, &fitting::NUBIANS, 3, rng, report) {
            slot -= 1;
        }
        if draw_counting_down(
            state,
            player,
            slot,
            9,
            &fitting::BATTLESHIPS[3..7],
            4,
            rng,
            report,
        ) {
            slot -= 1;
        }
        if draw_counting_down(
            state,
            player,
            slot,
            9,
            &fitting::BATTLESHIPS[0..3],
            3,
            rng,
            report,
        ) {
            slot -= 1;
        }
        if slot >= base {
            let mut n = 9 - 3 * (slot - base);
            let mut start = 9 - n;
            loop {
                let set = &fitting::CRUISERS[start..start + n];
                draw_counting_down(
                    state,
                    player,
                    slot,
                    7,
                    set,
                    i16::try_from(n).unwrap_or(3),
                    rng,
                    report,
                );
                if slot == 0 || slot - 1 < base {
                    break;
                }
                slot -= 1;
                n = 3;
                start = if slot == base { 0 } else { 3 };
            }
        }
        let top = base + 3;
        if !draw(state, player, top, 29, fitting::NUBIAN_BOMBER, rng, report)
            && !draw(
                state,
                player,
                top,
                9,
                fitting::BATTLESHIP_BOMBER,
                rng,
                report,
            )
            && !draw(state, player, top, 19, fitting::B52S[1], rng, report)
        {
            draw(state, player, top, 19, fitting::B52S[0], rng, report);
        }
    }
    // Slots 14 and 15: the starbase defenders, from turn 31 (slot 15
    // twenty years after 14): a Battleship, else a Cruiser, else a
    // Destroyer.
    for slot in [14usize, 15] {
        let due =
            (slot == 14 && turn > 30) || (!retired(state, 14) && designed(state, 14) + 20 < turn);
        if !retired(state, slot) || !due {
            continue;
        }
        if draw_counting_down(
            state,
            player,
            slot,
            9,
            &fitting::BATTLESHIPS,
            7,
            rng,
            report,
        ) {
            continue;
        }
        if draw_counting_down(state, player, slot, 7, &fitting::CRUISERS, 9, rng, report) {
            continue;
        }
        draw_counting_down(
            state,
            player,
            slot,
            6,
            &fitting::DESTROYERS,
            10,
            rng,
            report,
        );
    }
}

/// The squared distance between two points.
fn d2(a: Point, b: Point) -> i64 {
    let dx = i64::from(a.x) - i64::from(b.x);
    let dy = i64::from(a.y) - i64::from(b.y);
    dx * dx + dy * dy
}

/// `IWarpMAFromLppl` (`1090:5d5e`): the warp of the best mass driver on
/// a planet's starbase — an orbital slot's item 7 to 15 less 2 — and
/// whether a second of that warp is fitted. Zero without a starbase, or
/// for somebody else's whose design we do not know (taken as known).
fn driver_warp(state: &GameState, planet: &crate::planet::Planet) -> (i32, bool) {
    if !planet.starbase {
        return (0, false);
    }
    let Some(owner) = planet.owner.and_then(|o| usize::try_from(o).ok()) else {
        return (0, false);
    };
    let designs = state.designs.get(owner).cloned().unwrap_or_default();
    let driver = crate::production::mass_driver(planet, &designs);
    (driver.warp, driver.paired)
}

/// A leg to a planet or fleet, written as `FMoveAiFleet` does: an order
/// for where the fleet stands is folded into its first waypoint.
#[allow(clippy::too_many_arguments)]
fn move_fleet(
    fleet: &mut crate::fleet::Fleet,
    at: Point,
    target: Option<u16>,
    class: u8,
    warp: u8,
    task: u8,
    task_data: Vec<u8>,
) {
    fleet.waypoints.truncate(1);
    if fleet.waypoints[0].position == at {
        fleet.waypoints[0].task = task;
        fleet.waypoints[0].task_data = task_data;
        fleet.waypoints[0].transport = None;
        return;
    }
    fleet.waypoints.push(crate::fleet::Waypoint {
        position: at,
        target,
        target_class: class,
        warp,
        task,
        transport: None,
        task_data,
    });
    fleet.warp = Some(warp);
}

/// `FShouldPlanetBuildColonizer` (`1090:9f30`): always before turn 60;
/// then by the nearest planet whose `vlpbAiPlanet[+13]` is clear —
/// `is_candidate` says which those are — within 350 light years always;
/// within 300 one time in two; else within 250 one time in two of that.
/// The Cybertron never sets that byte, so for it every planet is a
/// candidate, the planet asking included, and the answer is always yes;
/// the Macinti marks its own planets, so it asks about the nearest planet
/// not its own.
pub(crate) fn should_build_colonizer(
    state: &GameState,
    from: Point,
    is_candidate: &dyn Fn(&crate::planet::Planet) -> bool,
    rng: &mut Rng,
) -> bool {
    if state.turn < 60 {
        return true;
    }
    let mut best: i64 = 10_000_000;
    for planet in &state.planets {
        let Some(at) = planet.position else { continue };
        if !is_candidate(planet) {
            continue;
        }
        best = best.min(d2(at, from));
    }
    (best < 122_501 && (best < 90_001 || rng.random(2) == 0))
        && (best < 62_501 || rng.random(2) == 0)
}

/// `iAddAttackFleet` (`10a8:4eba`): what a planet queues for the war.
/// Nothing, ninety times in a hundred when it could still operate a
/// hundred more mines or factories, sixty otherwise. Else, with a
/// warship group and a roll of 51 or more: two of the group's first two
/// slots, one of the third three times in four and one of its bomber
/// one time in two (1). Else with a starbase defender and a roll of 26 or
/// more: one (2). Else with a Destroyer: one (3). Else nothing (0).
#[allow(clippy::too_many_arguments)]
fn add_attack_fleet(
    state: &mut GameState,
    player: usize,
    index: usize,
    destroyer: Option<u8>,
    group: Option<usize>,
    defender: Option<u8>,
    rng: &mut Rng,
    added: &mut Vec<(u8, i32)>,
) -> i32 {
    let race = state.players[player].race.clone();
    let planet = &state.planets[index];
    let r = rng.random(100);
    let mines_short = i32::from(crate::resources::max_operable_mines(planet, &race, false))
        - i32::from(crate::mining::mines_operating(planet, &race));
    let factories_short = i32::from(crate::resources::max_operable_factories(
        planet, &race, false,
    )) - i32::from(crate::resources::factories_operating(planet, &race));
    let r2 = rng.random(100);
    let threshold = if mines_short < 100 || factories_short < 100 {
        90
    } else {
        60
    };
    if r2 < threshold {
        return 0;
    }
    let live = |state: &GameState, slot: usize| {
        state.designs[player]
            .get(slot)
            .is_some_and(|d| d.hull().is_some() && !d.obsolete)
    };
    match group {
        Some(g) if r >= 51 => {
            let base = g * 4 + 6;
            if live(state, base) {
                added.push((u8::try_from(base).unwrap_or(0), 2));
            }
            if live(state, base + 1) {
                added.push((u8::try_from(base + 1).unwrap_or(0), 2));
            }
            if live(state, base + 2) && rng.random(100) < 75 {
                added.push((u8::try_from(base + 2).unwrap_or(0), 1));
            }
            if live(state, base + 3) && rng.random(100) < 50 {
                added.push((u8::try_from(base + 3).unwrap_or(0), 1));
            }
            1
        }
        _ => match defender {
            Some(d) if r >= 26 => {
                added.push((d, 1));
                2
            }
            _ => match destroyer {
                Some(d) => {
                    added.push((d, 1));
                    3
                }
                None => 0,
            },
        },
    }
}

/// The Cybertron's turn.
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
    let planet_count = state.planets.len().max(
        state
            .planets
            .iter()
            .map(|p| usize::try_from(p.id).unwrap_or(0) + 1)
            .max()
            .unwrap_or(0),
    );

    let seen = crate::visibility::view(state, player).planets;
    state.players[player].explored.extend(seen);
    let explored = state.players[player].explored.clone();

    // The lasting words, sized to the galaxy the first time.
    if state.players[player].cyber_words.len() < planet_count {
        state.players[player].cyber_words.resize(planet_count, 0);
    }
    // `DoCyberAiTurn` walks its planets with a pointer to each one's
    // yearly word (`10a8:0599`) and never resets it: both fleet passes and
    // `DoCyberFreighter` (`10a8:1019`, `10a8:1173`) then index from the
    // *last* planet's word, so every yearly bit they set lands `skew`
    // words past the planet it was meant for — beyond every read but
    // the last planet's. The corpus bears it out: a colony ship is queued
    // in the very year one found nowhere to go. The reads in the queue
    // pass and the drop-off enumerators go through the global and are
    // sound, as are `DoCyberPackets`'s.
    let skew = state
        .planets
        .last()
        .map_or(0, |p| usize::try_from(p.id).unwrap_or(0));
    let mut yearly: Vec<u16> = vec![0; planet_count + skew];

    // `IroEnsureAi(vrgbCyberRes, 42, &ishdefSBLatest, 17)`. The Cybertron
    // keeps no starbase history.
    report.research = ensure_research(state, player, profile.plan, profile.research_pct(turn));
    ensure_designs(state, player, skill, rng, &mut report);
    // `MergeAllShdefs`, every year: the mine layers, the Destroyers, the
    // starbase defenders, and each warship group.
    for mask in [0x0001u16, 0x0030, 0xc000, 0x03c0, 0x3c00] {
        merge_all(state, me, mask, &mut report);
    }
    let attack_str = attack_strength(turn);
    let _own_potency = potency(turn);
    let potency = state.ai_armada_potency;
    let recycle = recycle_period(turn);

    // `CheckAiShdefStatus`: the Destroyers, the starbase defenders and
    // the colonist freighters, the newest of each never counted old.
    let mut old = [false; 16];
    let destroyers = check_status(state, player, me, 4, 5, recycle, &mut old);
    if let Some(l) = destroyers.latest {
        old[usize::from(l)] = false;
    }
    let defenders = check_status(state, player, me, 14, 15, recycle, &mut old);
    if let Some(l) = destroyers.latest {
        old[usize::from(l)] = false;
    }
    let freighters = check_status(state, player, me, 2, 3, recycle, &mut old);
    if let Some(l) = freighters.latest {
        old[usize::from(l)] = false;
    }
    // The warship groups: a group whose base design is live but past the
    // recycling period has its unused designs retired from the top down
    // — the base itself only when a higher slot was kept — and the rest
    // marked old; the group with the newest live base is the one to
    // build.
    let mut group: Option<usize> = None;
    for g in 0..2usize {
        let base = g * 4 + 6;
        let base_live = state.designs[player]
            .get(base)
            .is_some_and(|d| d.hull().is_some() && !d.obsolete);
        if base_live && turn - state.designs[player][base].designed > recycle {
            let mut none_kept = true;
            for slot in (base..=base + 3).rev() {
                let live = state.designs[player]
                    .get(slot)
                    .is_some_and(|d| d.hull().is_some() && !d.obsolete);
                if !live {
                    continue;
                }
                let exists = state.fleets.iter().filter(|f| f.owner == me).any(|f| {
                    f.stacks
                        .iter()
                        .any(|s| usize::from(s.design) == slot && s.count > 0)
                });
                if !exists && (none_kept || slot != base) {
                    state.designs[player][slot].obsolete = true;
                } else {
                    old[slot] = true;
                    none_kept = false;
                }
            }
        }
        let base_live = state.designs[player]
            .get(base)
            .is_some_and(|d| d.hull().is_some() && !d.obsolete);
        if base_live
            && group.is_none_or(|b| {
                state.designs[player][b * 4 + 6].designed < state.designs[player][base].designed
            })
        {
            group = Some(g);
        }
    }
    let colony_ships: i64 = state
        .fleets
        .iter()
        .filter(|f| f.owner == me)
        .flat_map(|f| f.stacks.iter())
        .filter(|s| s.design == COLONY_SLOT)
        .map(|s| i64::from(s.count))
        .sum();
    // `SplitOutShdefs` from turn 81: the old designs, then the colonist
    // freighters each on their own.
    if turn > 80 {
        split_out_designs(state, player, me, &old, &mut report);
        for slot in [2usize, 3] {
            let mut only = [false; 16];
            only[slot] = true;
            split_out_designs(state, player, me, &only, &mut report);
        }
        let mut both = [false; 16];
        both[2] = true;
        both[3] = true;
        split_out_designs(state, player, me, &both, &mut report);
    }

    // --- The planet marks: every planet's cooldown counts down; an own
    // starbase planet is marked short of each mineral under 1,000 kT
    // (10 kT for the small starbases); the other players' planets are
    // worth a point per 250 colonists, at most six, plus one for a
    // starbase.
    let mut worth_marks: Vec<u8> = vec![0; planet_count];
    for planet in &state.planets {
        let Ok(id) = usize::try_from(planet.id) else {
            continue;
        };
        let w = &mut state.players[player].cyber_words[id];
        if *w & lasting::COOLDOWN != 0 {
            let down = (*w - 0x20) & lasting::COOLDOWN;
            *w = (*w & !lasting::COOLDOWN) | down;
        }
        match planet.owner {
            Some(o) if o == me => {
                if planet.starbase {
                    let floor = if small_starbase(planet) { 10 } else { 1000 };
                    let bits = [
                        yearly::SHORT_IRONIUM,
                        yearly::SHORT_BORANIUM,
                        yearly::SHORT_GERMANIUM,
                    ];
                    for (m, bit) in planet.surface_min.iter().zip(bits) {
                        if *m < floor {
                            yearly[id] |= bit;
                        }
                    }
                }
            }
            Some(_) => {
                let mut v = ((planet.pop / 4) & 0xfff) / 250 + 1;
                if v > 6 {
                    v = 6;
                }
                if planet.starbase {
                    v += 1;
                }
                worth_marks[id] = u8::try_from(v).unwrap_or(7);
            }
            None => {}
        }
    }

    let mut marks = marks(state, player, me, &explored, AiPersonality::Cyber);
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let positions: Vec<(i16, Point)> = state
        .planets
        .iter()
        .filter_map(|p| p.position.map(|at| (p.id, at)))
        .collect();
    let position_of = |id: i16| positions.iter().find(|(p, _)| *p == id).map(|(_, at)| *at);

    // --- The first fleet pass: the counts and the claims.
    let mut layer_fleets = 0i32;
    let mut frigates = 0i32;
    let mut defender_fleets = 0i32;
    let mut roaming = 0i32;
    let mut bound = 0i32;
    let mut attack_fleets: Vec<u16> = Vec::new();
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
        if count(14) > 0 || count(15) > 0 {
            defender_fleets += 1;
            continue;
        }
        if count(LAYER_SLOT) > 0 {
            layer_fleets += 1;
            frigates += count(LAYER_SLOT);
        }
        let warship = (4..14u8).find(|s| count(*s) > 0);
        let has_leg = fleet.waypoints.len() > 1;
        let planet_leg = has_leg && fleet.waypoints[1].target_class == grobj::PLANET;
        if warship.is_some() {
            attack_fleets.push(fleet.id);
            if !planet_leg && fleet.orbiting.is_none() {
                roaming += 1;
            } else {
                let dest = if planet_leg {
                    fleet.waypoints[1].target
                } else {
                    fleet.orbiting
                };
                if let Some(w) = dest.map(usize::from).and_then(|i| worth_marks.get_mut(i)) {
                    if *w != 0 {
                        *w |= 0x80;
                    }
                }
                bound += 1;
            }
        } else if (count(2) > 0 || count(3) > 0) && planet_leg && fleet.cargo.colonists > 0 {
            if let Some(w) = fleet.waypoints[1]
                .target
                .map(usize::from)
                .and_then(|i| yearly.get_mut(skew + i))
            {
                let n = (*w & yearly::BOUND) >> 3;
                if n < 3 {
                    *w = (*w & !yearly::BOUND) | ((n + 1) << 3);
                }
            }
        }
    }

    // --- The second fleet pass: the orders.
    let first_starbase = state
        .planets
        .iter()
        .find(|p| p.owner == Some(me) && p.starbase)
        .map(|p| p.id);
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
        let orbit_index = orbiting.and_then(|id| state.planets.iter().position(|p| p.id == id));
        let has_leg = fleet.waypoints.len() > 1;
        let scrap = |state: &mut GameState, report: &mut Report| {
            let f = &mut state.fleets[index];
            f.waypoints.truncate(1);
            f.waypoints[0].task = stars_formats::task::SCRAP;
            f.waypoints[0].task_data = Vec::new();
            f.waypoints[0].transport = None;
            report.scrapped.push(fleet_id);
        };
        // Old ships home to be scrapped.
        let all_old = fleet
            .stacks
            .iter()
            .filter(|s| s.count > 0)
            .all(|s| old.get(usize::from(s.design)) == Some(&true));
        if all_old {
            if let Some(p) = orbit_index.filter(|&p| state.planets[p].owner == Some(me)) {
                if state.planets[p].starbase || rng.random(5) == 0 {
                    scrap(state, &mut report);
                    continue;
                }
            }
            if (has_leg && orbiting.is_none()) || move_to_nearest_starbase(state, me, index, false)
            {
                continue;
            }
        }
        let defenders_aboard = count(14) > 0 || count(15) > 0;
        if defenders_aboard && orbiting.is_some() {
            // Starbase defenders at a planet: those still fresh — designed
            // within ten years of the recycling period — count for it.
            let mut fresh = 0;
            for slot in [14u8, 15] {
                let age = turn - designs.get(usize::from(slot)).map_or(0, |d| d.designed);
                if age < recycle - 10 {
                    fresh += count(slot);
                }
            }
            if fresh > 0 {
                if let Some(w) = orbiting
                    .and_then(|id| usize::try_from(id).ok())
                    .and_then(|i| yearly.get_mut(skew + i))
                {
                    *w |= yearly::DEFENDERS;
                    if fresh < attack_str * 2 {
                        *w |= yearly::DEFENDERS_SHORT;
                    } else {
                        *w &= !yearly::DEFENDERS_SHORT;
                    }
                }
            }
            continue;
        }
        let warship = (4..14u8).find(|s| count(*s) > 0);
        if let Some(first_slot) = warship {
            if count(4) < 1 {
                // An armada.
                if let Some(to) =
                    target_cyber_armada(state, player, me, index, &potency, skill, rng)
                {
                    report.attacking.push((fleet_id, to));
                }
                if state.fleets[index].waypoints.len() == 1 && rng.random(100) < 75 {
                    let (lo, hi) = if first_slot < 10 { (6, 9) } else { (10, 13) };
                    if find_buddy_and_join_up(state, me, index, lo, hi, 100, 200, rng) {
                        report.merged.push((fleet_id, fleet_id));
                    }
                }
            } else {
                // Destroyers: after the enemy once there are enough of them
                // for the year's attack strength, or already under way.
                if 2 * count(4) >= attack_str || has_leg {
                    if let Some(to) = target_attack(state, player, me, index, &attack_fleets, rng) {
                        report.attacking.push((fleet_id, to));
                    }
                }
                if state.fleets[index].waypoints.len() == 1
                    && rng.random(100) < 75
                    && find_buddy_and_join_up(state, me, index, 4, 5, 100, 200, rng)
                {
                    report.merged.push((fleet_id, fleet_id));
                }
            }
            continue;
        }
        if has_leg {
            continue;
        }
        // The scouts the player began with are scrapped in the first years.
        if turn < 6 && count(0) > 0 {
            scrap(state, &mut report);
            continue;
        }
        if count(COLONY_SLOT) > 0 {
            // A colony ship: the nearest colonisable planet, else the
            // planet it stands at is marked as having nowhere to send one.
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
                if let Some(w) = orbiting
                    .and_then(|id| usize::try_from(id).ok())
                    .and_then(|i| yearly.get_mut(skew + i))
                {
                    *w |= yearly::NOWHERE;
                }
                continue;
            };
            if let Some(p) = orbit_index.filter(|&p| state.planets[p].owner == Some(me)) {
                let room =
                    state.fleets[index].cargo_capacity(&designs) - state.fleets[index].cargo.mass();
                let take = COLONISTS_ABOARD.min(state.planets[p].pop).min(room).max(0);
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
            continue;
        }
        if count(2) > 0 || count(3) > 0 {
            // A colonist freighter, on battle plan 4.
            state.fleets[index].battle_plan = 4;
            if let Some(to) =
                cyber_freighter(state, player, me, index, &mut yearly, skew, &mut report)
            {
                report.hauling.push((fleet_id, to));
            }
            continue;
        }
        // Mine layers, from turn 41.
        if count(LAYER_SLOT) > 0 && turn > 40 {
            if (layer_fleets > 55 || (layer_fleets > 40 && rng.random(3) != 0))
                && find_buddy_and_join_up(state, me, index, 0, 0, 72, 108, rng)
            {
                report.merged.push((fleet_id, fleet_id));
                continue;
            }
            if count(LAYER_SLOT) > 6 && rng.random(5) == 0 {
                if let Some(id) = random_planet_nearby(state, fleet.position, 105, true, rng) {
                    if Some(id) != orbiting {
                        if let Some(at) = position_of(id) {
                            // `0x1146`: a leg at warp 4 ending in Lay Mines.
                            move_fleet(
                                &mut state.fleets[index],
                                at,
                                u16::try_from(id).ok(),
                                grobj::PLANET,
                                4,
                                stars_formats::task::LAY_MINES,
                                vec![5, 0],
                            );
                            report.scouted.push((fleet_id, id));
                            continue;
                        }
                    }
                }
            }
            let f = &mut state.fleets[index];
            if f.waypoints[0].task != stars_formats::task::LAY_MINES {
                f.waypoints[0].task = stars_formats::task::LAY_MINES;
                f.waypoints[0].task_data = vec![5, 0];
                report.laying.push(fleet_id);
            }
        }
    }
    let _ = first_starbase;

    // --- The planet pass.
    let race = state.players[player].race.clone();
    let levels = state.players[player].research.levels;
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
    for id in order {
        let Some(index) = state.planets.iter().position(|p| p.id == id) else {
            continue;
        };
        let Ok(idx) = usize::try_from(id) else {
            continue;
        };
        let planet = state.planets[index].clone();
        let mut added: Vec<(u8, i32)> = Vec::new();
        let mut items: Vec<(u16, i32)> = Vec::new();
        let growth =
            i64::from(planet.pop) * i64::from(crate::population::pct_true_max_growth(&race));
        let available = crate::ai::production::resources_available(
            &planet,
            &race,
            state.players[player].research_pct,
            i16::from(levels[0]),
        );
        let committed = crate::ai::production::queue_cost(&planet.queue, &race);
        // A planet that cannot pay for its queue is left alone.
        if available.iter().zip(committed.iter()).any(|(a, c)| a < c) {
            continue;
        }
        let resources_left = available[3] - committed[3];
        let desirability = crate::hab::pct_planet_desirability(&planet, &race);
        let reach = crate::terraform::optimal_env(&planet, &race, levels);
        let opt_value = crate::ai::colonise::pct_planet_opt_value(&planet, &race, reach);
        if desirability < 10 {
            // Terraforming, as much as the resources left buy at seventy
            // apiece, plus one.
            let n = resources_left / 70 + 1;
            items.push((item::TERRAFORM, n.max(1)));
        } else {
            if desirability < opt_value && resources_left > 70 {
                items.push((item::TERRAFORM, 1));
            }
            if planet.starbase && !small_starbase(&planet) {
                let w = state.players[player].cyber_words[idx];
                // A colony ship: unless one was queued last year and the
                // planet grows under 5,500 a year; not where a colony ship
                // found nowhere to go; while under forty exist.
                let may = (w & lasting::COLONIZER == 0 || growth > 5500)
                    && yearly[idx] & yearly::NOWHERE == 0
                    && colony_ships < 40;
                let built = may
                    && planet
                        .position
                        .is_some_and(|at| should_build_colonizer(state, at, &|_| true, rng));
                if built {
                    added.push((COLONY_SLOT, 1));
                    state.players[player].cyber_words[idx] |= lasting::COLONIZER;
                    if growth > 15_000 && turn < 100 {
                        added.push((COLONY_SLOT, 1));
                    }
                } else {
                    state.players[player].cyber_words[idx] &= !lasting::COLONIZER;
                }
                // A colonist freighter (newest of 2, 3) where over 200,000
                // people live, under fifty exist, no freighter unloaded
                // here this year, and a planet within 170 light years
                // wants people (`FEnumDropOffStage2`).
                if planet.pop > 2000
                    && designs
                        .get(2)
                        .is_some_and(|d| d.hull().is_some() && !d.obsolete)
                    && freighters.count < 50
                    && yearly[idx] & yearly::UNLOADED == 0
                {
                    if let Some(latest) = freighters.latest {
                        let wants = planet.position.is_some_and(|from| {
                            drop_off_stage2(state, me, from, &yearly).is_some()
                        });
                        if wants {
                            added.push((latest, 1));
                        }
                    }
                }
                // Mine layers, four at a time, one roll in four while under
                // ten thousand Frigates fly.
                if designs.first().is_some_and(|d| d.hull_id == 5)
                    && rng.random(4) == 0
                    && frigates < 10_000
                {
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
                // A starbase defender: where the defenders here are short;
                // else, with none here and under forty defender fleets,
                // one time in two when an enemy planet lies within 300
                // light years, one in ten otherwise.
                let mut defender = if yearly[idx] & yearly::DEFENDERS_SHORT != 0 {
                    defenders.latest
                } else {
                    None
                };
                if defender.is_none()
                    && yearly[idx] & yearly::DEFENDERS == 0
                    && defender_fleets < 40
                {
                    let near_enemy = planet.position.is_some_and(|from| {
                        state
                            .planets
                            .iter()
                            .filter(|p| p.owner.is_some_and(|o| o != me))
                            .filter_map(|p| p.position.map(|at| d2(at, from)))
                            .min()
                            .is_some_and(|d| d <= 90_000)
                    });
                    let chance = if near_enemy { 50 } else { 10 };
                    if rng.random(100) < chance {
                        defender = defenders.latest;
                    }
                }
                let destroyer = if roaming > 120 {
                    None
                } else {
                    destroyers.latest
                };
                let group_now = if bound > 250 { None } else { group };
                let r = add_attack_fleet(
                    state, player, index, destroyer, group_now, defender, rng, &mut added,
                );
                match r {
                    1 => bound += 1,
                    2 => defender_fleets += 1,
                    3 => roaming += 1,
                    _ => {}
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

    let worth = mineral_worth(state, &explored, marks.len());
    basic_tasks(
        state,
        player,
        me,
        &worth,
        AiPersonality::Cyber,
        rng,
        &mut report,
    );
    cyber_packets(state, player, me, &mut yearly, rng, &mut report);
    fill_production_queues(state, player, me, AiPersonality::Cyber, rng, &mut report);
    let _ = BTreeSet::<i16>::new();
    report
}

/// `FEnumDropOffStage1` (`10a8:3d00`): the nearest own planet under
/// 20,000 people that no freighter is bound for, within 170 light years.
fn drop_off_stage1(state: &GameState, me: i16, from: Point, yearly: &[u16]) -> Option<i16> {
    let mut best: Option<(i64, i16)> = None;
    for planet in &state.planets {
        let (Some(at), Ok(idx)) = (planet.position, usize::try_from(planet.id)) else {
            continue;
        };
        if planet.owner != Some(me) || planet.pop >= 200 {
            continue;
        }
        if yearly.get(idx).is_some_and(|w| w & yearly::BOUND != 0) {
            continue;
        }
        let d = d2(at, from);
        if d >= 28_929 {
            continue;
        }
        if best.is_none_or(|(b, _)| d < b) {
            best = Some((d, planet.id));
        }
    }
    best.map(|(_, id)| id)
}

/// `FEnumDropOffStage2` (`10a8:3dfe`): the nearest own planet that, with
/// the freighters bound for it counted at 21,000 people each, is under
/// 100,000, fewer than three bound, within 170 light years.
fn drop_off_stage2(state: &GameState, me: i16, from: Point, yearly: &[u16]) -> Option<i16> {
    let mut best: Option<(i64, i16)> = None;
    for planet in &state.planets {
        let (Some(at), Ok(idx)) = (planet.position, usize::try_from(planet.id)) else {
            continue;
        };
        if planet.owner != Some(me) {
            continue;
        }
        let bound = i32::from(yearly.get(idx).map_or(0, |w| (w & yearly::BOUND) >> 3));
        if bound >= 3 || planet.pop + bound * 210 >= 1000 {
            continue;
        }
        let d = d2(at, from);
        if d >= 28_929 {
            continue;
        }
        if best.is_none_or(|(b, _)| d < b) {
            best = Some((d, planet.id));
        }
    }
    best.map(|(_, id)| id)
}

/// `DoCyberFreighter` (`10a8:37b0`): a colonist freighter's year.
///
/// In deep space it heads for the nearest planet. At an own planet under
/// 200,100 people it unloads a thousand kT of colonists, else it loads
/// a thousand; at an unowned planet, an Alternate Reality player's or
/// one with a starbase it keeps what it has; at anyone else's it drops
/// everything (a Transport order) and heads for the nearest own
/// starbase. Empty, it goes to pick up at the nearest own starbase
/// planet over 220,000 (`FEnumPickUp`); loaded, to the nearest planet
/// wanting people (`FEnumDropOffStage1`, then `2`), and with none it
/// unloads where it stands if that is ours.
fn cyber_freighter(
    state: &mut GameState,
    player: usize,
    me: i16,
    index: usize,
    yearly: &mut [u16],
    skew: usize,
    report: &mut Report,
) -> Option<i16> {
    let fleet = state.fleets[index].clone();
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let here = fleet
        .orbiting
        .and_then(|p| i16::try_from(p).ok())
        .and_then(|id| state.planets.iter().position(|p| p.id == id));
    let nearest_planet = |state: &GameState, from: Point| -> Option<i16> {
        state
            .planets
            .iter()
            .filter_map(|p| p.position.map(|at| (d2(at, from), p.id)))
            .min()
            .map(|(_, id)| id)
    };
    let target: Option<i16>;
    let mut loaded = false;
    match here {
        None => {
            target = nearest_planet(state, fleet.position);
        }
        Some(p) => {
            let planet = state.planets[p].clone();
            let transfer = |state: &mut GameState, amount: i32| {
                // `XferAiSupply`: capped by what the source has and what
                // the destination can take.
                if amount >= 0 {
                    let room = state.fleets[index].cargo_capacity(&designs)
                        - state.fleets[index].cargo.mass();
                    let take = amount.min(state.planets[p].pop).min(room).max(0);
                    state.planets[p].pop -= take;
                    state.fleets[index].cargo.colonists += take;
                } else {
                    let give = (-amount).min(state.fleets[index].cargo.colonists).max(0);
                    state.fleets[index].cargo.colonists -= give;
                    state.planets[p].pop += give;
                }
            };
            if planet.owner == Some(me) {
                if planet.pop < 2001 {
                    transfer(state, -FREIGHTER_LOAD);
                } else {
                    transfer(state, FREIGHTER_LOAD);
                    loaded = true;
                }
            } else {
                let ar = planet
                    .owner
                    .and_then(|o| usize::try_from(o).ok())
                    .and_then(|o| state.players.get(o))
                    .is_some_and(|q| q.race.prt() == Some(crate::race::Prt::Ar));
                if planet.owner.is_none() || ar || planet.starbase {
                    loaded = state.fleets[index].cargo.colonists > 0;
                } else {
                    use stars_formats::{ItemAction, TransportTask, XferAction};
                    let mut items = [ItemAction {
                        quantity: 0,
                        action: XferAction::None,
                    }; 5];
                    items[3] = ItemAction {
                        quantity: 0,
                        action: XferAction::UnloadAll,
                    };
                    let f = &mut state.fleets[index];
                    f.waypoints.truncate(1);
                    f.waypoints[0].task = stars_formats::task::TRANSPORT;
                    f.waypoints[0].transport = Some(TransportTask { items });
                    f.waypoints[0].task_data = Vec::new();
                    report.dropping.push((fleet.id, planet.id));
                    move_to_nearest_starbase(state, me, index, false);
                    return None;
                }
            }
            if loaded {
                target = drop_off_stage1(state, me, fleet.position, yearly)
                    .or_else(|| drop_off_stage2(state, me, fleet.position, yearly));
                match target {
                    None => {
                        if state.planets[p].owner == Some(me) {
                            transfer(state, -FREIGHTER_LOAD);
                            // Written through the skewed pointer the
                            // caller hands `DoCyberFreighter`.
                            if let Some(w) = usize::try_from(planet.id)
                                .ok()
                                .and_then(|i| yearly.get_mut(skew + i))
                            {
                                let n = (*w & yearly::UNLOADED) >> 1;
                                if n < 3 {
                                    *w = (*w & !yearly::UNLOADED) | ((n + 1) << 1);
                                }
                            }
                        }
                    }
                    Some(t) => {
                        if let Some(w) = usize::try_from(t)
                            .ok()
                            .and_then(|i| yearly.get_mut(skew + i))
                        {
                            let n = (*w & yearly::BOUND) >> 3;
                            if n < 3 {
                                *w = (*w & !yearly::BOUND) | ((n + 1) << 3);
                            }
                        }
                    }
                }
            } else {
                // `FEnumPickUp`: the nearest own starbase planet over
                // 220,000 people.
                target = state
                    .planets
                    .iter()
                    .filter(|q| q.owner == Some(me) && q.pop > 2200 && q.starbase)
                    .filter_map(|q| q.position.map(|at| (d2(at, fleet.position), q.id)))
                    .min()
                    .map(|(_, id)| id);
            }
        }
    }
    let target = target?;
    let at = state
        .planets
        .iter()
        .find(|p| p.id == target)
        .and_then(|p| p.position)?;
    let stacks: Vec<(&crate::design::ShipDesign, i32)> = state.fleets[index]
        .stacks
        .iter()
        .filter_map(|s| designs.get(usize::from(s.design)).map(|d| (d, s.count)))
        .collect();
    let warp = ideal_warp(&stacks, false);
    move_fleet(
        &mut state.fleets[index],
        at,
        u16::try_from(target).ok(),
        grobj::PLANET,
        warp,
        stars_formats::task::NONE,
        Vec::new(),
    );
    Some(target)
}

/// `MoveToNearestPlanetOrEnemy(fleet, 450)` (`1090:7040`): a fleet in
/// deep space heads for the nearest of the other players' planets within
/// 450 light years of its next waypoint, else the nearest planet of any
/// kind, at warp 4.
pub(crate) fn move_to_nearest_planet_or_enemy(
    state: &mut GameState,
    me: i16,
    index: usize,
) -> Option<i16> {
    let fleet = state.fleets[index].clone();
    let from = fleet
        .waypoints
        .get(1)
        .map_or(fleet.position, |w| w.position);
    let enemy = state
        .planets
        .iter()
        .filter(|p| p.owner.is_some_and(|o| o != me))
        .filter_map(|p| p.position.map(|at| (d2(at, from), p.id)))
        .min()
        .filter(|(d, _)| *d <= ENEMY_RANGE * ENEMY_RANGE)
        .map(|(_, id)| id);
    let target = match enemy {
        Some(id) => id,
        None => state
            .planets
            .iter()
            .filter_map(|p| p.position.map(|at| (d2(at, fleet.position), p.id)))
            .min()
            .map(|(_, id)| id)?,
    };
    if fleet.orbiting == u16::try_from(target).ok() {
        return None;
    }
    let at = state
        .planets
        .iter()
        .find(|p| p.id == target)
        .and_then(|p| p.position)?;
    move_fleet(
        &mut state.fleets[index],
        at,
        u16::try_from(target).ok(),
        grobj::PLANET,
        4,
        stars_formats::task::NONE,
        Vec::new(),
    );
    Some(target)
}

/// `TargetCyberArmada` (`10a8:51a4`): where a warship-group fleet goes.
///
/// A fleet under way keeps going when it is chasing a fleet within 250
/// light years, or bound for a planet somebody else holds, an own planet
/// with a starbase, or a planet not seen this year. Its weight of war is
/// its ships in slots 6, 7, 10, 11 plus twice those in 8 and 12; its
/// bombers those in 9 and 13. In deep space it goes by
/// [`move_to_nearest_planet_or_enemy`]. At an own planet it waits while
/// under the first and third potencies unless — at skill 2 or more —
/// its weight is over 60 and over twice the first potency, and a run of
/// rolls (five in ten; then over three times the potency or three in
/// ten; then over 120 or three in ten) sends it anyway. At another
/// player's planet, too weak for the second and fourth potencies, it
/// clears its task and goes home to the nearest own starbase — unless
/// at skill 2 or more the same rolls keep it in the war; strong enough,
/// it stays. Otherwise the target is [`armada_dest`], claimed; with
/// none, the nearest fleet not ours. The leg is laid at warp 4.
fn target_cyber_armada(
    state: &mut GameState,
    player: usize,
    me: i16,
    index: usize,
    potency: &[u8; 4],
    skill: u8,
    rng: &mut Rng,
) -> Option<i16> {
    let fleet = &state.fleets[index];
    let count = |slot: u8| -> i32 {
        fleet
            .stacks
            .iter()
            .filter(|s| s.design == slot)
            .map(|s| s.count)
            .sum()
    };
    let weight = count(6) + count(7) + count(8) * 2 + count(10) + count(11) + count(12) * 2;
    let bombers = count(9) + count(13);
    target_potent_armada(
        state, player, me, index, potency, skill, weight, bombers, rng,
    )
}

/// The armada dispatch the Cybertron and the Macinti share
/// (`TargetCyberArmada` `10a8:51a4`, `TargetMacArmada` `10a0:4146`), given
/// the fleet's weight of war and its bombers as each personality counts
/// them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn target_potent_armada(
    state: &mut GameState,
    player: usize,
    me: i16,
    index: usize,
    potency: &[u8; 4],
    skill: u8,
    weight: i32,
    bombers: i32,
    rng: &mut Rng,
) -> Option<i16> {
    let fleet = state.fleets[index].clone();
    if fleet.waypoints.len() > 1 {
        let next = &fleet.waypoints[1];
        let far = d2(fleet.position, next.position) >= 62_500;
        if !(far && next.target_class == grobj::FLEET) {
            if next.target_class == grobj::FLEET {
                return None;
            }
            if next.target_class == grobj::PLANET {
                let planet = next
                    .target
                    .and_then(|t| i16::try_from(t).ok())
                    .and_then(|id| state.planets.iter().find(|p| p.id == id))?;
                if planet.owner.is_some_and(|o| o != me) {
                    return None;
                }
                if planet.owner == Some(me) && planet.starbase {
                    return None;
                }
                if !state.players[player].explored.contains(&planet.id) {
                    return None;
                }
            }
        }
    }
    let p0 = i32::from(potency[0]);
    let Some(here) = fleet
        .orbiting
        .and_then(|p| i16::try_from(p).ok())
        .and_then(|id| state.planets.iter().find(|p| p.id == id).cloned())
    else {
        return move_to_nearest_planet_or_enemy(state, me, index);
    };
    let mut retreat = false;
    if here.owner == Some(me) {
        if weight < p0 || bombers < i32::from(potency[2]) {
            if skill < 2 {
                return None;
            }
            if weight <= p0 * 2 && weight < 60 {
                return None;
            }
            if rng.random(10) > 4 && (weight <= p0 * 3 || rng.random(10) > 6) {
                if weight < 121 {
                    return None;
                }
                if rng.random(10) > 6 {
                    return None;
                }
            }
        }
    } else if weight < i32::from(potency[1]) || bombers < i32::from(potency[3]) {
        let f = &mut state.fleets[index];
        f.waypoints[0].task = stars_formats::task::NONE;
        f.waypoints[0].task_data = Vec::new();
        f.waypoints[0].transport = None;
        if skill < 2
            || ((weight <= p0 * 2 || rng.random(10) > 4)
                && (weight <= p0 * 4 || rng.random(10) > 6)
                && (weight < 121 || rng.random(10) > 6))
        {
            retreat = true;
        }
    } else if here.owner.is_some() {
        return None;
    }
    let from = here.position?;
    let target: Option<(i16, Point)> = if retreat {
        state
            .planets
            .iter()
            .filter(|p| p.owner == Some(me) && p.starbase)
            .filter_map(|p| p.position.map(|at| (d2(at, from), p.id, at)))
            .min_by_key(|(d, _, _)| *d)
            .map(|(_, id, at)| (id, at))
    } else {
        let claimed: BTreeSet<u16> = state
            .fleets
            .iter()
            .filter(|f| f.owner == me && f.id != fleet.id && f.waypoints.len() > 1)
            .filter(|f| {
                f.stacks
                    .iter()
                    .any(|s| (6..14).contains(&s.design) && s.count > 0)
            })
            .filter_map(|f| f.waypoints[1].target)
            .collect();
        armada_dest(state, player, me, here.id, from, &claimed, rng)
    };
    match target {
        Some((id, at)) => {
            move_fleet(
                &mut state.fleets[index],
                at,
                u16::try_from(id).ok(),
                grobj::PLANET,
                4,
                stars_formats::task::NONE,
                Vec::new(),
            );
            Some(id)
        }
        None => {
            // `LpflFindClosestEnum(fleet, FEnumCalcEnemyFleets)`.
            let enemy = state
                .fleets
                .iter()
                .filter(|f| f.owner != me && !f.is_empty())
                .map(|f| (d2(f.position, fleet.position), f))
                .min_by_key(|(d, _)| *d)
                .map(|(_, f)| {
                    (
                        f.position,
                        (u16::try_from(f.owner).unwrap_or(0) << 9) | (f.id & 0x1ff),
                    )
                })?;
            move_fleet(
                &mut state.fleets[index],
                enemy.0,
                Some(enemy.1),
                grobj::FLEET,
                4,
                stars_formats::task::NONE,
                Vec::new(),
            );
            i16::try_from(enemy.1).ok()
        }
    }
}

/// The fraction of a packet that lands on a planet, and how much must be
/// thrown for `want` to land (`DoCyberPackets`, `10a8:2278`): the
/// packet's speed squared less the receiving driver's, times a hundred
/// less the defence guess and five, over sixteen thousand.
fn packet_need(speed: i32, their_warp: i32, defense_guess: i32, want: i32) -> Option<i64> {
    let frac = f64::from((speed * speed - their_warp * their_warp) * (100 - (defense_guess + 5)))
        / 16_000.0;
    if frac <= 0.0 {
        return None;
    }
    Some((f64::from(want) / frac) as i64)
}

/// The decay a packet suffers in flight, as the routine estimates it:
/// `pow(0.875, years)` with a pair of drivers, `pow(0.75, years)`
/// otherwise, for `distance / speed²` years.
fn decay(paired: bool, distance: f64, speed: i32) -> f64 {
    let base: f64 = if paired { 0.875 } else { 0.75 };
    base.powf(distance / f64::from(speed * speed))
}

/// The population guess of another player's planet, as `uPopGuess` holds
/// it: a quarter of the population, in hundreds, twelve bits; and the
/// defence guess nibble, taken here from the defences built.
fn guesses(planet: &crate::planet::Planet) -> (i32, i32) {
    let pop = (planet.pop / 4) & 0xfff;
    let defense = i32::from(planet.defenses / 10).min(15);
    (pop, defense)
}

/// `DoCyberPackets` (`10a8:1a78`): the mass drivers.
///
/// At every own planet with a starbase, in turn:
///
/// 1. **Supplies.** Unless more packets are owed to a target (bit 7): a
///    planet that is no small starbase with over 700 kT of some mineral
///    and at least 35 resources left over — a seventh of half of them
///    per packet — finds the nearest own starbase planet, cooldown
///    over, short of a mineral it has over 700 kT of, within 3.5 times
///    the lesser driver's warp squared (`FEnumNeedMinerals`); for each
///    such mineral (under 10 kT at a small starbase, under 1,000 kT
///    else) it queues up to seven packets and aims the driver at the
///    planet at the lesser of the two drivers' warps.
/// 2. **Attack.** Otherwise, with no packets owed, at skill 2 or more or
///    at skill 1 one time in three: what could be thrown — each mineral
///    less 70 kT summed, capped at seventy times a fifth of half the
///    resources less five — when over 150 kT, at the nearest planet of
///    another player's (not Alternate Reality, cooldown over) within
///    2.5 times the speed squared that enough would reach to wipe out
///    (`FEnumPktAttack`): the packets, of whatever mineral is most to
///    hand, are queued to land `min(1000, 4 × (population guess + 25))`
///    kT through the defences after the decay of the flight; the driver
///    is aimed at warp 3 over its own; the target's cooldown is set to
///    three years, and when the flight takes over a year bit 7 marks
///    more to come.
/// 3. **Scanning.** With no scanner packet just sent (bit 4), or more
///    owed: a direction is rolled (bits 0–2, never the same twice) and
///    `IdGetBestScannerDest` finds a planet not ours near the galaxy's
///    edge that way, a year's flight or more off; one packet of the
///    mineral most to hand (`FAddPacketToQueue`, 170 kT or more of it
///    left) is queued for it at warp 3 over the driver's, the target's
///    cooldown set and bit 4 raised — or, when more were owed, the old
///    target kept and bit 7 cleared.
#[allow(clippy::too_many_lines)]
fn cyber_packets(
    state: &mut GameState,
    player: usize,
    me: i16,
    yearly: &mut [u16],
    rng: &mut Rng,
    report: &mut Report,
) {
    let race = state.players[player].race.clone();
    let levels = state.players[player].research.levels;
    let skill = match state.players[player].control {
        crate::ai::Control::Computer { skill_bits, .. } => skill_bits,
        crate::ai::Control::Human => 0,
    };
    let order: Vec<usize> = (0..state.planets.len())
        .filter(|&i| state.planets[i].owner == Some(me) && state.planets[i].starbase)
        .collect();
    for index in order {
        let planet = state.planets[index].clone();
        let Ok(idx) = usize::try_from(planet.id) else {
            continue;
        };
        let Some(from) = planet.position else {
            continue;
        };
        let word = state.players[player].cyber_words[idx];
        let mut available = crate::ai::production::resources_available(
            &planet,
            &race,
            state.players[player].research_pct,
            i16::from(levels[0]),
        );
        let committed = crate::ai::production::queue_cost(&planet.queue, &race);
        let mut done = false;
        let (my_warp, my_paired) = driver_warp(state, &planet);
        // A starbase without a mass driver cannot fling. The original
        // queues its packets regardless and production never builds
        // them; they are not queued here.
        if my_warp == 0 {
            continue;
        }

        // 1. Supplies.
        if word & lasting::MORE == 0 {
            let rich = !small_starbase(&planet) && planet.surface_min.iter().any(|m| *m > 700);
            let per = available[3] / 2 / 5;
            if rich && per >= 7 {
                let mut n = per;
                // `FEnumNeedMinerals`.
                let mut best: Option<(i64, usize)> = None;
                for (i, other) in state.planets.iter().enumerate() {
                    if i == index || other.owner != Some(me) || !other.starbase {
                        continue;
                    }
                    let (Some(at), Ok(oi)) = (other.position, usize::try_from(other.id)) else {
                        continue;
                    };
                    if state.players[player].cyber_words[oi] & lasting::COOLDOWN != 0 {
                        continue;
                    }
                    let short = [
                        yearly::SHORT_IRONIUM,
                        yearly::SHORT_BORANIUM,
                        yearly::SHORT_GERMANIUM,
                    ];
                    let wanted =
                        (0..3).any(|m| planet.surface_min[m] > 700 && yearly[oi] & short[m] != 0);
                    if !wanted {
                        continue;
                    }
                    let (tw, tp) = driver_warp(state, other);
                    let ws = my_warp + i32::from(my_paired);
                    let wt = tw + i32::from(tp);
                    let m = f64::from(ws.min(wt));
                    let reach = m * m * 3.5;
                    let d = d2(at, from);
                    if (d as f64) > reach * reach {
                        continue;
                    }
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, i));
                    }
                }
                if let Some((_, ti)) = best {
                    let target = state.planets[ti].clone();
                    let Ok(tidx) = usize::try_from(target.id) else {
                        continue;
                    };
                    let floor = if small_starbase(&target) { 10 } else { 1000 };
                    let mut queued = false;
                    let items = [
                        item::PACKET_IRONIUM,
                        item::PACKET_BORANIUM,
                        item::PACKET_GERMANIUM,
                    ];
                    let bits = [
                        yearly::SHORT_IRONIUM,
                        yearly::SHORT_BORANIUM,
                        yearly::SHORT_GERMANIUM,
                    ];
                    for m in 0..3 {
                        if target.surface_min[m] < floor && planet.surface_min[m] > 700 {
                            let count = n.min(7);
                            if count > 0 {
                                state.planets[index].queue.push(QueueItem {
                                    count,
                                    item: items[m],
                                    ship: false,
                                    completion: 0,
                                });
                                n -= count;
                                yearly[tidx] &= !bits[m];
                                queued = true;
                            }
                        }
                    }
                    if queued {
                        let (tw, tp) = driver_warp(state, &target);
                        let mine = my_warp - if my_paired { 3 } else { 4 };
                        let theirs = tw - if tp { 3 } else { 4 };
                        let p = &mut state.planets[index];
                        p.fling_dest = Some(target.id);
                        p.fling_warp = u8::try_from(mine.min(theirs).clamp(0, 15)).unwrap_or(0);
                        report.flung.push((planet.id, target.id));
                        done = true;
                    }
                }
            }
        }
        if done {
            continue;
        }
        // 2. Attack.
        if word & lasting::MORE == 0 && (skill > 1 || (skill == 1 && rng.random(3) == 0)) {
            for (have, spent) in available.iter_mut().zip(committed.iter()) {
                *have -= spent;
            }
            let excess = i64::from(available[0] - 70)
                + i64::from(available[1] - 70)
                + i64::from(available[2] - 70);
            let cap = i64::from((available[3] / 2 - 5) / 5) * i64::from(PACKET_KT);
            let mut total = excess.min(cap);
            let speed = my_warp + 3;
            if total >= 151 {
                // `FEnumPktAttack`.
                let mut best: Option<(i64, usize)> = None;
                for (i, other) in state.planets.iter().enumerate() {
                    let Some(owner) = other.owner.filter(|o| *o != me) else {
                        continue;
                    };
                    let (Some(at), Ok(oi)) = (other.position, usize::try_from(other.id)) else {
                        continue;
                    };
                    if state.players[player].cyber_words[oi] & lasting::COOLDOWN != 0 {
                        continue;
                    }
                    let ar = usize::try_from(owner)
                        .ok()
                        .and_then(|o| state.players.get(o))
                        .is_some_and(|q| q.race.prt() == Some(crate::race::Prt::Ar));
                    if ar {
                        continue;
                    }
                    let (tw, tp) = driver_warp(state, other);
                    let their_warp = if other.starbase {
                        tw + i32::from(tp)
                    } else {
                        0
                    };
                    if speed == their_warp {
                        continue;
                    }
                    let distance = crate::movement::distance(from, at);
                    if distance > f64::from(speed * speed) * 2.5 {
                        continue;
                    }
                    let arrival = (total as f64 * decay(my_paired, distance, speed)) as i64;
                    let (pop_guess, defense_guess) = guesses(other);
                    if pop_guess == 0 {
                        continue;
                    }
                    let want = (4 * (pop_guess + 25)).min(1000);
                    let Some(need) = packet_need(speed, their_warp, defense_guess, want) else {
                        continue;
                    };
                    if arrival < need {
                        continue;
                    }
                    let d = d2(at, from);
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, i));
                    }
                }
                if let Some((_, ti)) = best {
                    let target = state.planets[ti].clone();
                    let Ok(tidx) = usize::try_from(target.id) else {
                        continue;
                    };
                    let Some(at) = target.position else { continue };
                    let (tw, tp) = driver_warp(state, &target);
                    let their_warp = if target.starbase {
                        tw + i32::from(tp)
                    } else {
                        0
                    };
                    let distance = crate::movement::distance(from, at);
                    let (pop_guess, defense_guess) = guesses(&target);
                    let want = (4 * (pop_guess + 25)).min(1000);
                    if let Some(need) = packet_need(speed, their_warp, defense_guess, want) {
                        total = total.min(need);
                    }
                    total = (total as f64 / decay(my_paired, distance, speed)) as i64;
                    let mut counts = [0i32; 3];
                    let mut left = [available[0], available[1], available[2]];
                    while total > 0 {
                        let mut pick = 0;
                        if left[1] > left[0] {
                            pick = 1;
                        }
                        if left[2] > left[pick] {
                            pick = 2;
                        }
                        counts[pick] += 1;
                        left[pick] -= PACKET_KT;
                        total -= i64::from(PACKET_KT);
                    }
                    let items = [
                        item::PACKET_IRONIUM,
                        item::PACKET_BORANIUM,
                        item::PACKET_GERMANIUM,
                    ];
                    for (m, count) in counts.iter().enumerate() {
                        if *count > 0 {
                            state.planets[index].queue.push(QueueItem {
                                count: *count,
                                item: items[m],
                                ship: false,
                                completion: 0,
                            });
                        }
                    }
                    let p = &mut state.planets[index];
                    p.fling_dest = Some(target.id);
                    p.fling_warp = u8::try_from((speed - 4).clamp(0, 15)).unwrap_or(0);
                    state.players[player].cyber_words[tidx] |= lasting::COOLDOWN;
                    let w = &mut state.players[player].cyber_words[idx];
                    if f64::from(speed * speed) < distance {
                        *w |= lasting::MORE;
                    }
                    *w &= !lasting::SENT;
                    report.flung.push((planet.id, target.id));
                    continue;
                }
            }
        }
        // 3. Scanning.
        let word = state.players[player].cyber_words[idx];
        if word & lasting::SENT == 0 || word & lasting::MORE != 0 {
            let mut dest: Option<i16> = None;
            if word & lasting::MORE == 0 {
                let mut dir = rng.random(7);
                if dir == i16::try_from(word & lasting::DIRECTION).unwrap_or(-1) {
                    dir += 1;
                }
                let w = &mut state.players[player].cyber_words[idx];
                *w = (*w & !lasting::DIRECTION) | (u16::try_from(dir).unwrap_or(0) & 7);
                dest = best_scanner_dest(state, me, &planet, my_warp, dir, rng);
            }
            let more = state.players[player].cyber_words[idx] & lasting::MORE != 0;
            if more || dest.is_some() {
                let cool = dest
                    .and_then(|d| usize::try_from(d).ok())
                    .is_some_and(|i| state.players[player].cyber_words[i] & lasting::COOLDOWN != 0);
                if !cool {
                    let mut left = available;
                    for (have, spent) in left.iter_mut().zip(committed.iter()) {
                        *have -= spent;
                    }
                    // `FAddPacketToQueue`: one packet of the mineral most
                    // to hand, when 170 kT of it are.
                    let mut pick = 0;
                    if left[1] > left[0] {
                        pick = 1;
                    }
                    if left[2] > left[pick] {
                        pick = 2;
                    }
                    if left[pick] >= 170 {
                        let items = [
                            item::PACKET_IRONIUM,
                            item::PACKET_BORANIUM,
                            item::PACKET_GERMANIUM,
                        ];
                        state.planets[index].queue.push(QueueItem {
                            count: 1,
                            item: items[pick],
                            ship: false,
                            completion: 0,
                        });
                        if let Some(i) = dest.and_then(|d| usize::try_from(d).ok()) {
                            state.players[player].cyber_words[i] |= lasting::COOLDOWN;
                        }
                        let w = &mut state.players[player].cyber_words[idx];
                        if more {
                            *w &= !lasting::SENT;
                        } else {
                            *w |= lasting::SENT;
                        }
                    } else {
                        state.players[player].cyber_words[idx] &= !lasting::SENT;
                    }
                } else {
                    state.players[player].cyber_words[idx] &= !lasting::SENT;
                }
                let p = &mut state.planets[index];
                p.fling_warp = u8::try_from((my_warp - 1).clamp(0, 15)).unwrap_or(0);
                if more {
                    state.players[player].cyber_words[idx] &= !lasting::MORE;
                } else {
                    p.fling_dest = dest;
                }
                if let Some(d) = dest {
                    report.flung.push((planet.id, d));
                }
            }
        } else {
            state.players[player].cyber_words[idx] &= !lasting::SENT;
        }
    }
}

/// `IdGetBestScannerDest` (`10a8:282a`): a planet to throw a scanning
/// packet at. From the planet's coordinates (less the 1,000 the map is
/// offset by) a point on the galaxy's edge — the galaxy is `400 × size +
/// 400` across — is taken in one of eight directions: east; north-east
/// by sliding along the diagonal; north; north-west; west; south-west;
/// south; south-east. The point is jittered along the edge by
/// `Random(0.3 × edge) − 0.15 × edge`, kept on the edge, and stepped
/// inward by up to the speed squared. The planet nearest that point,
/// when it is not ours and at least a year's flight off, is the answer.
fn best_scanner_dest(
    state: &GameState,
    me: i16,
    planet: &crate::planet::Planet,
    my_warp: i32,
    dir: i16,
    rng: &mut Rng,
) -> Option<i16> {
    let at = planet.position?;
    let size = i32::from(state.galaxy_size) * 400 + 400;
    let speed = my_warp + 3;
    let speed2 = speed * speed;
    let mut x = i32::from(at.x) - 1000;
    let mut y = i32::from(at.y) - 1000;
    match dir {
        0 => x = size,
        1 => {
            if y < size - x {
                x += y;
                y = 0;
            } else {
                y -= size - x;
                x = size;
            }
        }
        2 => y = 0,
        3 => {
            if y < x {
                x -= y;
                y = 0;
            } else {
                y -= x;
                x = 0;
            }
        }
        4 => x = 0,
        5 => {
            if y < size - x {
                y += x;
                x = 0;
            } else {
                x -= size - y;
                y = size;
            }
        }
        6 => y = size,
        7 => {
            if y < x {
                y += size - x;
                x = size;
            } else {
                x += size - y;
                y = size;
            }
        }
        _ => return None,
    }
    let spread = i16::try_from((f64::from(size) * 0.3) as i32).unwrap_or(i16::MAX);
    let jitter = i32::from(rng.random(spread)) - (f64::from(size) * 0.15) as i32;
    if x == 0 || x == size {
        y += jitter;
        let over = if y > size {
            let o = y - size;
            y = size;
            o
        } else if y < 0 {
            let o = -y;
            y = 0;
            o
        } else {
            0
        };
        x += if x != 0 { -over } else { over };
    } else {
        x += jitter;
        let over = if x > size {
            let o = x - size;
            x = size;
            o
        } else if x < 0 {
            let o = -x;
            x = 0;
            o
        } else {
            0
        };
        if y == 0 {
            y = over;
        } else {
            y -= over;
        }
    }
    let step = i32::from(rng.random(i16::try_from(speed2).unwrap_or(i16::MAX)));
    if y != 0 {
        if y == size {
            y -= step;
        } else if x == 0 {
            x = step;
        } else if x == size {
            x -= step;
        }
    } else {
        y = step;
    }
    let point = Point {
        x: i16::try_from(x + 1000).unwrap_or(i16::MAX),
        y: i16::try_from(y + 1000).unwrap_or(i16::MAX),
    };
    let nearest = state
        .planets
        .iter()
        .filter_map(|p| p.position.map(|pp| (d2(pp, point), p)))
        .min_by_key(|(d, _)| *d)
        .map(|(_, p)| p)?;
    if nearest.owner == Some(me) {
        return None;
    }
    let d = d2(nearest.position?, at);
    if d >= i64::from(speed2) * i64::from(speed2) {
        Some(nearest.id)
    } else {
        None
    }
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
        for f in &fitting::PRIVATEERS {
            assert_eq!(f.len(), slots(11));
        }
        assert_eq!(fitting::FRIGATE_LAYER.len(), slots(5));
        for f in &fitting::B52S {
            assert_eq!(f.len(), slots(19));
        }
        assert_eq!(fitting::BATTLESHIP_BOMBER.len(), slots(9));
        assert_eq!(fitting::NUBIAN_BOMBER.len(), slots(29));
        for f in &fitting::CRUISERS {
            assert_eq!(f.len(), slots(7));
        }
        for f in &fitting::BATTLESHIPS {
            assert_eq!(f.len(), slots(9));
        }
        for f in &fitting::NUBIANS {
            assert_eq!(f.len(), slots(29));
        }
    }

    #[test]
    fn the_strength_and_period_follow_the_years() {
        assert_eq!(attack_strength(0), 1);
        assert_eq!(attack_strength(50), 1);
        assert_eq!(attack_strength(60), 2);
        assert_eq!(attack_strength(100), 6);
        assert_eq!(attack_strength(150), 11 + 5);
        assert_eq!(attack_strength(250), 21 + 15 * 2);
        assert_eq!(recycle_period(0), 50);
        assert_eq!(recycle_period(120), 70);
        assert_eq!(recycle_period(200), 100);
        assert_eq!(recycle_period(400), 300);
        assert_eq!(potency(0), [3, 1, 6, 2]);
    }

    #[test]
    fn a_packet_need_follows_the_defences() {
        // Warp 10 driver: speed 13 against no driver, no defences.
        let need = packet_need(13, 0, 0, 1000).expect("a fraction");
        // (169 × 95) / 16000 = 1.0034…: about the same again.
        assert!((996..=1000).contains(&need), "{need}");
        // Against a warp 10 receiver the fraction shrinks to (169−100)×95/16000.
        let need = packet_need(13, 10, 0, 1000).expect("a fraction");
        assert!(need > 2000, "{need}");
        assert!(packet_need(10, 10, 0, 1000).is_none());
    }
}
