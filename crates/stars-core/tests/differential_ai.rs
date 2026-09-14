//! The seven personalities checked against the two sixteen-player computer
//! games in `fixtures/games` — `all-computer-players` and
//! `no-random-events`, 101 turns each. See `docs/formulas/ai.md`, *What
//! the corpus confirms of the personalities*.
//!
//! When year *N + 1* is generated, every computer player's turn is run on
//! that player's own view of year *N*: its `.mN` file, with the planets
//! its `.hN` history remembers and the positions of the universe file.
//! The host file of year *N + 1* holds what came of it. Each
//! personality's turn is run here on that view and what it wrote is
//! compared with what the original left behind, where that is visible
//! after the year has run:
//!
//! * the research field and share (`IroEnsureAi`), exact;
//! * the designs drawn that year (`Ensure…Shdefs`), by slot and hull —
//!   the fittings drawn at random are reproduced only when no `Random`
//!   went into them;
//! * where the colony ships are sent (`IdNearestColonizablePlanet`);
//! * the ships the planets queue, where no `Random` gates them;
//! * the year the starting scouts are broken up, and the slot each
//!   waypoint task is written for.
//!
//! The original's random draws are not reproduced — the seed it drew from
//! is not in the files — so the tests measure agreement and hold it to a
//! floor rather than demand it item by item.
//!
//! Fixtures are optional: the tests skip when the games are absent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use stars_core::ai::personality::{Profile, Shape};
use stars_core::ai::{AiPersonality, Control};
use stars_core::planet::{Detail, Planet};
use stars_core::GameState;
use stars_formats::{StarsFile, Universe};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

fn load(path: &Path) -> Option<GameState> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    let (mut state, _) = GameState::from_file(&file);
    let universe = Universe::decode(&std::fs::read(path.with_file_name("Game.xy")).ok()?).ok()?;
    state.apply_universe(&universe);
    Some(state)
}

/// One year of one game: its directory, the host state the year began
/// from and the host state it left.
struct Year {
    game: &'static str,
    dir: PathBuf,
    prev: GameState,
    next: GameState,
}

/// The consecutive years of both games.
fn years() -> Vec<Year> {
    let root = workspace_root();
    let mut out = Vec::new();
    for game in ["all-computer-players", "no-random-events"] {
        let dir = root.join("fixtures/games").join(game);
        let Some(mut prev) = load(&dir.join("2400/Game.hst")) else {
            continue;
        };
        for year in 2401..=2500 {
            let Some(next) = load(&dir.join(format!("{year}/Game.hst"))) else {
                break;
            };
            out.push(Year {
                game,
                dir: dir.join(format!("{}", year - 1)),
                prev,
                next: next.clone(),
            });
            prev = next;
        }
    }
    out
}

/// The state player `player` ran its turn on: its own file, plus the
/// planets its history remembers (a planet the history still calls the
/// player's own but the file no longer lists has been lost, and is
/// treated as unowned), plus every planet of the universe it has never
/// seen at all. The planets the file or the history list are the ones it
/// has explored.
fn ai_view(dir: &Path, player: usize) -> Option<GameState> {
    let me = i16::try_from(player).ok()?;
    let own = std::fs::read(dir.join(format!("Game.m{}", player + 1))).ok()?;
    let (mut state, _) = GameState::from_file(&StarsFile::decode(&own).ok()?);
    let mut have: BTreeSet<i16> = state
        .planets
        .iter()
        .chain(state.known_planets.iter())
        .map(|p| p.id)
        .collect();
    if let Some(history) = std::fs::read(dir.join(format!("Game.h{}", player + 1)))
        .ok()
        .and_then(|bytes| StarsFile::decode(&bytes).ok())
    {
        let (remembered, _) = GameState::from_file(&history);
        for mut planet in remembered
            .known_planets
            .into_iter()
            .chain(remembered.planets)
        {
            if !have.insert(planet.id) {
                continue;
            }
            if planet.owner == Some(me) {
                planet.owner = None;
            }
            state.known_planets.push(planet);
        }
    }
    let explored = have.clone();
    let universe = Universe::decode(&std::fs::read(dir.join("Game.xy")).ok()?).ok()?;
    for (i, _) in universe.planets_resolved().iter().enumerate() {
        let id = i16::try_from(i).ok()?;
        if have.insert(id) {
            let mut planet = Planet::unowned(id);
            planet.detail = Detail::Minimal;
            state.known_planets.push(planet);
        }
    }
    state.apply_universe(&universe);
    let known = std::mem::take(&mut state.known_planets);
    state.planets.extend(known);
    state.planets.sort_by_key(|p| p.id);
    state.players.get_mut(player)?.explored = explored;
    Some(state)
}

/// One personality's turn, as `turn.rs` dispatches it.
fn run_turn(
    state: &mut GameState,
    player: usize,
    rng: &mut stars_core::rng::Rng,
) -> Option<AiPersonality> {
    let Control::Computer {
        personality: Some(p),
        ..
    } = state.players[player].control
    else {
        return None;
    };
    let profile = Profile::of(p);
    match profile.shape {
        Shape::Basic => {
            stars_core::ai::turindrone::basic_turn(state, player, rng, &profile);
        }
        Shape::TurinDrone => {
            stars_core::ai::turindrone::turn_as(state, player, rng, &profile);
        }
        Shape::Robotoid => {
            stars_core::ai::robotoid::turn(state, player, rng, &profile);
        }
        Shape::Automitron => {
            stars_core::ai::automitron::turn(state, player, rng, &profile);
        }
        Shape::Cyber => {
            stars_core::ai::cyber::turn(state, player, rng, &profile);
        }
        Shape::Rototill => {
            stars_core::ai::rototill::turn(state, player, rng, &profile);
        }
        Shape::Macinti => {
            stars_core::ai::macinti::turn(state, player, rng, &profile);
        }
    }
    Some(p)
}

/// Every computer player's turn of every year, run on the player's own
/// view: `(year, player, personality, the view after the turn)`.
fn every_turn(years: &[Year], salt: u32) -> Vec<(&Year, usize, AiPersonality, GameState)> {
    let mut out = Vec::new();
    for year in years {
        for player in 0..year.prev.players.len() {
            let Some(mut sim) = ai_view(&year.dir, player) else {
                continue;
            };
            assert_eq!(
                sim.turn,
                year.prev.turn,
                "{} {}: the player's file is of another year",
                year.game,
                year.dir.display()
            );
            let mut rng =
                stars_core::rng::Rng::randomize(u32::try_from(sim.turn).unwrap_or(0) + salt);
            let Some(p) = run_turn(&mut sim, player, &mut rng) else {
                continue;
            };
            out.push((year, player, p, sim));
        }
    }
    out
}

/// The research field and share each personality sets are the ones the
/// original set: the share exactly, and the field either the one chosen
/// or — when the year's research completed that level — the next one the
/// plan names.
#[test]
fn research_follows_each_plan() {
    let years = years();
    if years.is_empty() {
        eprintln!("skipping: the computer games are absent");
        return;
    }
    let mut checked = 0;
    let mut pct_wrong = Vec::new();
    let mut field_wrong = Vec::new();
    for (year, player, p, sim) in every_turn(&years, 1) {
        let (game, prev, next) = (year.game, &year.prev, &year.next);
        checked += 1;
        let ours = &sim.players[player].research;
        let theirs = &next.players[player].research;
        if sim.players[player].research_pct != next.players[player].research_pct {
            pct_wrong.push(format!(
                "{game} t{} p{player} {}: share {} vs {}",
                prev.turn,
                p.name(),
                sim.players[player].research_pct,
                next.players[player].research_pct
            ));
        }
        let advanced = theirs.levels[ours.current_field]
            > prev.players[player].research.levels[ours.current_field];
        let next_ok = match ours.next_field {
            stars_core::research::NextField::Field(f) => theirs.current_field == f,
            _ => false,
        };
        if theirs.current_field != ours.current_field && !(advanced && next_ok) {
            field_wrong.push(format!(
                "{game} t{} p{player} {}: field {} vs {} (levels {:?})",
                prev.turn,
                p.name(),
                ours.current_field,
                theirs.current_field,
                theirs.levels
            ));
        }
    }
    eprintln!(
        "research: {checked} player-years, {} shares wrong, {} fields wrong",
        pct_wrong.len(),
        field_wrong.len()
    );
    for line in pct_wrong.iter().chain(field_wrong.iter()).take(20) {
        eprintln!("  {line}");
    }
    assert!(checked >= 3000, "{checked} player-years checked");
    assert!(
        pct_wrong.is_empty(),
        "{} research shares differ",
        pct_wrong.len()
    );
    assert!(
        field_wrong.is_empty(),
        "{} of {checked} research fields differ",
        field_wrong.len()
    );
}

/// The designs each personality draws land in the slots and on the hulls
/// the original's do. A design the original drew is *matched* when ours
/// drew the same hull into the same slot that year, and *exact* when the
/// fittings are the same too — which they are whenever no `Random` went
/// into them. No design of ours may land on a different hull.
#[test]
fn designs_land_on_the_same_slots_and_hulls() {
    let years = years();
    if years.is_empty() {
        eprintln!("skipping: the computer games are absent");
        return;
    }
    // Per personality: (matched, exact, hull differs, missed, extra).
    let mut tally: BTreeMap<&'static str, [u32; 5]> = BTreeMap::new();
    let mut notes = Vec::new();
    for (year, player, p, sim) in every_turn(&years, 7) {
        let (game, prev, next) = (year.game, &year.prev, &year.next);
        let turn = prev.turn;
        let changed = |before: Option<&stars_core::design::ShipDesign>,
                       after: &stars_core::design::ShipDesign| {
            before.is_none_or(|b| b.hull_id != after.hull_id || b.slots != after.slots)
        };
        let drawn = |designs: &[stars_core::design::ShipDesign]| -> Vec<(usize, i16, Vec<_>)> {
            designs
                .iter()
                .enumerate()
                .take(16)
                .filter(|(s, d)| {
                    d.hull().is_some()
                        && !d.obsolete
                        && d.designed == turn
                        && changed(prev.designs.get(player).and_then(|d| d.get(*s)), d)
                })
                .map(|(s, d)| (s, d.hull_id, d.slots.clone()))
                .collect()
        };
        let theirs = drawn(next.designs.get(player).map_or(&[][..], Vec::as_slice));
        let ours = drawn(sim.designs.get(player).map_or(&[][..], Vec::as_slice));
        let t = tally.entry(p.name()).or_default();
        for (slot, hull, fittings) in &theirs {
            match ours.iter().find(|(s, _, _)| s == slot) {
                Some((_, h, f)) if h == hull => {
                    t[0] += 1;
                    if f == fittings {
                        t[1] += 1;
                    }
                }
                Some((_, h, _)) => {
                    t[2] += 1;
                    notes.push(format!(
                        "{game} t{turn} p{player} {} slot {slot}: hull {hull} vs ours {h}",
                        p.name()
                    ));
                }
                None => {
                    t[3] += 1;
                    notes.push(format!(
                        "{game} t{turn} p{player} {} slot {slot} hull {hull}: not drawn by ours",
                        p.name()
                    ));
                }
            }
        }
        for (slot, hull, _) in &ours {
            if !theirs.iter().any(|(s, _, _)| s == slot) {
                t[4] += 1;
                notes.push(format!(
                    "{game} t{turn} p{player} {} slot {slot} hull {hull}: drawn by ours only",
                    p.name()
                ));
            }
        }
    }
    for (name, t) in &tally {
        eprintln!(
            "{name}: {} matched ({} exact), {} other hull, {} missed, {} extra",
            t[0], t[1], t[2], t[3], t[4]
        );
    }
    for line in notes.iter().take(40) {
        eprintln!("  {line}");
    }
    let sum = |i: usize| -> u32 { tally.values().map(|t| t[i]).sum() };
    let (matched, exact, other, missed, extra) = (sum(0), sum(1), sum(2), sum(3), sum(4));
    assert!(matched >= 300, "{matched} designs matched");
    assert_eq!(other, 0, "designs drawn on another hull: {other}");
    // The draws that differ are the random fittings failing where the
    // original's succeeded (or the reverse), which moves a slot; a
    // handful in a thousand.
    assert!(
        missed * 50 <= matched,
        "{missed} designs missed of {matched}"
    );
    assert!(
        extra * 50 <= matched,
        "{extra} designs drawn by ours only of {matched}"
    );
    assert!(
        exact * 3 >= matched * 2,
        "{exact} of {matched} matched designs have the original's fittings"
    );
}

/// The colony ships are sent where the original sent them. A leg is
/// compared the year it first appears; the target is the nearest planet
/// the personality counts colonisable, so the two agree wherever the
/// player's view is read the same way. The rest are the ships the
/// original loaded and sent a year apart from ours, and the Macinti's
/// freighters that colonise on the way.
#[test]
fn colony_ships_are_sent_where_the_original_sent_them() {
    let years = years();
    if years.is_empty() {
        eprintln!("skipping: the computer games are absent");
        return;
    }
    // Per personality: (their new legs, the same target, ours laid).
    let mut tally: BTreeMap<&'static str, [u32; 3]> = BTreeMap::new();
    for (year, player, p, sim) in every_turn(&years, 3) {
        let (prev, next) = (&year.prev, &year.next);
        let me = i16::try_from(player).unwrap_or(-1);
        let colonise = |f: &stars_core::fleet::Fleet| {
            f.waypoints
                .get(1)
                .filter(|w| w.task == stars_formats::task::COLONIZE)
                .map(|w| w.target)
        };
        for fleet in next.fleets.iter().filter(|f| f.owner == me) {
            let Some(target) = colonise(fleet) else {
                continue;
            };
            let before = prev
                .fleets
                .iter()
                .find(|g| g.id == fleet.id && g.owner == me)
                .and_then(colonise);
            if before == Some(target) {
                continue;
            }
            let t = tally.entry(p.name()).or_default();
            t[0] += 1;
            let ours = sim
                .fleets
                .iter()
                .find(|g| g.id == fleet.id && g.owner == me)
                .and_then(colonise);
            if ours == Some(target) {
                t[1] += 1;
            }
            if ours.is_some() {
                t[2] += 1;
            }
        }
    }
    let mut total = [0u32; 3];
    for (name, t) in &tally {
        eprintln!(
            "{name}: {} colonise legs, {} to the same planet, {} laid by ours",
            t[0], t[1], t[2]
        );
        for i in 0..3 {
            total[i] += t[i];
        }
        assert!(
            t[1] * 5 >= t[0] * 4,
            "{name}: {} of {} colonise legs go where the original's went",
            t[1],
            t[0]
        );
    }
    assert!(total[0] >= 1500, "{} colonise legs seen", total[0]);
    assert!(
        total[1] * 10 >= total[0] * 9,
        "{} of {} colonise legs go where the original's went",
        total[1],
        total[0]
    );
}

/// The ships the planets queue. An item the original's queue gained that
/// year — a design slot not in the planet's queue the year before — is
/// *covered* when ours queued that slot there too. Items that gate on
/// `Random` (the warships, the scouts) cannot be reproduced; the colony
/// ships never do, and are held to their own floor. An item the original
/// queued and built within the year is not visible and not counted.
#[test]
fn planets_queue_the_ships_the_original_queued() {
    let years = years();
    if years.is_empty() {
        eprintln!("skipping: the computer games are absent");
        return;
    }
    // Per (personality, slot): (their new items, covered by ours).
    let mut tally: BTreeMap<(&'static str, u16), [u32; 2]> = BTreeMap::new();
    for (year, player, p, sim) in every_turn(&years, 5) {
        let (prev, next) = (&year.prev, &year.next);
        let me = i16::try_from(player).unwrap_or(-1);
        let ships = |planet: &Planet| -> BTreeSet<u16> {
            planet
                .queue
                .iter()
                .filter(|i| i.ship && i.item < 16)
                .map(|i| i.item)
                .collect()
        };
        for planet in next.planets.iter().filter(|q| q.owner == Some(me)) {
            let before = prev
                .planets
                .iter()
                .find(|q| q.id == planet.id)
                .map(ships)
                .unwrap_or_default();
            let ours = sim
                .planets
                .iter()
                .find(|q| q.id == planet.id)
                .map(ships)
                .unwrap_or_default();
            for item in ships(planet).difference(&before) {
                let t = tally.entry((p.name(), *item)).or_default();
                t[0] += 1;
                if ours.contains(item) && !before.contains(item) {
                    t[1] += 1;
                }
            }
        }
    }
    let mut total = [0u32; 2];
    let mut colony = [0u32; 2];
    for ((name, slot), t) in &tally {
        eprintln!("{name} slot {slot}: {} new items, {} covered", t[0], t[1]);
        total[0] += t[0];
        total[1] += t[1];
        if *slot == 1 {
            colony[0] += t[0];
            colony[1] += t[1];
        }
    }
    assert!(total[0] >= 1500, "{} queue items seen", total[0]);
    assert!(
        total[1] * 2 >= total[0],
        "{} of {} queued ships covered",
        total[1],
        total[0]
    );
    assert!(
        colony[1] * 10 >= colony[0] * 9,
        "{} of {} queued colony ships covered",
        colony[1],
        colony[0]
    );
}

/// The Robotoid, the Cybertron and the Macinti break up the scouts the
/// player began with — before turn 21, 6 and 11 — and the Automitron its
/// starting colony ship in year 0: none of those fleets survive the year
/// the routine names, in either game.
#[test]
fn starting_fleets_are_broken_up_when_the_routines_say() {
    let years = years();
    if years.is_empty() {
        eprintln!("skipping: the computer games are absent");
        return;
    }
    let mut checked = 0;
    for year in &years {
        let (game, prev, next) = (year.game, &year.prev, &year.next);
        for (player, control) in prev.players.iter().map(|p| p.control).enumerate() {
            let Control::Computer {
                personality: Some(p),
                ..
            } = control
            else {
                continue;
            };
            let me = i16::try_from(player).unwrap_or(-1);
            let count = |state: &GameState, slot: u8| -> i32 {
                state
                    .fleets
                    .iter()
                    .filter(|f| f.owner == me)
                    .flat_map(|f| f.stacks.iter())
                    .filter(|s| s.design == slot)
                    .map(|s| s.count)
                    .sum()
            };
            let scout_design_unchanged = prev.designs[player].first().map(|d| d.hull_id)
                == next.designs[player].first().map(|d| d.hull_id)
                && prev.designs[player].first().is_some_and(|d| d.hull_id == 4);
            let gone_by = match p {
                AiPersonality::Robotoid => Some(20),
                AiPersonality::Cyber => Some(5),
                AiPersonality::Macinti => Some(10),
                _ => None,
            };
            if let Some(last) = gone_by {
                if prev.turn == last && scout_design_unchanged {
                    checked += 1;
                    assert_eq!(
                        count(next, 0),
                        0,
                        "{game} t{} p{player} {}: starting scouts still flying",
                        prev.turn,
                        p.name()
                    );
                }
            }
            if p == AiPersonality::Automitron && prev.turn == 0 {
                checked += 1;
                assert_eq!(
                    count(next, 1),
                    0,
                    "{game} p{player}: the Automitron's starting colony ship survived year 0"
                );
            }
        }
    }
    assert!(checked >= 10, "{checked} checks");
}

/// The waypoint tasks the original wrote are on the fleets each
/// personality gives them to: Lay Mines only on fleets carrying the mine
/// layers' slot, Remote Mining only on miners, Colonize only on colony
/// ships.
#[test]
fn tasks_sit_on_the_slots_each_personality_uses() {
    let years = years();
    if years.is_empty() {
        eprintln!("skipping: the computer games are absent");
        return;
    }
    let mut seen: BTreeMap<(&'static str, u8), u32> = BTreeMap::new();
    let mut odd = Vec::new();
    for year in &years {
        let (game, next) = (year.game, &year.next);
        for (player, control) in next.players.iter().map(|p| p.control).enumerate() {
            let Control::Computer {
                personality: Some(p),
                ..
            } = control
            else {
                continue;
            };
            let me = i16::try_from(player).unwrap_or(-1);
            let (layers, miners, colony): (&[u8], &[u8], &[u8]) = match p {
                AiPersonality::TurinDrone => (&[12], &[2, 3], &[1]),
                AiPersonality::Robotoid => (&[0], &[], &[1]),
                AiPersonality::Automitron => (&[6], &[], &[1]),
                AiPersonality::Rototill => (&[], &[7, 8], &[1]),
                AiPersonality::Cyber => (&[0], &[], &[1]),
                AiPersonality::Macinti => (&[0], &[14, 15], &[1, 7]),
                AiPersonality::Maid => (&[], &[], &[]),
            };
            for fleet in next.fleets.iter().filter(|f| f.owner == me) {
                let carries = |slots: &[u8]| {
                    fleet
                        .stacks
                        .iter()
                        .any(|s| slots.contains(&s.design) && s.count > 0)
                };
                for wp in &fleet.waypoints {
                    let expected: Option<&[u8]> = match wp.task {
                        stars_formats::task::LAY_MINES => Some(layers),
                        stars_formats::task::REMOTE_MINING => Some(miners),
                        stars_formats::task::COLONIZE => Some(colony),
                        _ => None,
                    };
                    let Some(slots) = expected else { continue };
                    *seen.entry((p.name(), wp.task)).or_default() += 1;
                    if !carries(slots) {
                        odd.push(format!(
                            "{game} t{} p{player} {} fleet {}: task {} on {:?}",
                            next.turn,
                            p.name(),
                            fleet.id,
                            wp.task,
                            fleet
                                .stacks
                                .iter()
                                .map(|s| (s.design, s.count))
                                .collect::<Vec<_>>()
                        ));
                    }
                }
            }
        }
    }
    for ((name, task), n) in &seen {
        eprintln!("{name}: task {task} on {n} waypoints");
    }
    for line in odd.iter().take(30) {
        eprintln!("  {line}");
    }
    let total: u32 = seen.values().sum();
    assert!(total > 100, "{total} tasks seen");
    assert!(
        odd.len() * 100 <= usize::try_from(total).unwrap_or(0),
        "{} tasks on unexpected fleets of {total}",
        odd.len()
    );
}
