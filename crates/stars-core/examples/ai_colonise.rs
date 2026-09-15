//! Score the AI's choice of planet to colonise against a recorded game.
//!
//! `IdNearestColonizablePlanet` (`1090:0e3e`) marks every planet the AI knows
//! about, then picks the **nearest** one still marked colonisable, by squared
//! distance from the colony fleet. A planet passing from unowned to owned is
//! recorded turn by turn, so the choice can be scored directly.
//!
//! Two measurements. The first scores the landings: 513 planets pass from
//! unowned to owned, and the choice is ranked against the alternatives by
//! distance from the player's nearest planet — a proxy, since the colony fleet
//! is consumed by the landing.
//!
//! The second scores the decision itself. An AI fleet carrying colonists with a
//! waypoint on a planet *is* the choice this routine made, evaluated from the
//! fleet's own position, and there are far more of those than there are
//! landings.

use std::collections::{BTreeMap, HashMap};

use stars_core::ai::{AiPersonality, Control};
use stars_core::hab::pct_planet_desirability;
use stars_core::rng::Rng;
use stars_core::GameState;
use stars_formats::{StarsFile, Universe};

fn main() {
    let dir = std::env::args().nth(1).expect("usage: ai_colonise <dir>");
    let verbose = std::env::args().any(|a| a == "-v");
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    // Planet coordinates come from the .xy file, which does not change.
    let xy = years
        .iter()
        .find_map(|y| std::fs::read(y.join("Game.xy")).ok())
        .expect("a Game.xy");
    let universe = Universe::decode(&xy).expect("decode Game.xy");
    let pos: HashMap<i16, (i64, i64)> = universe
        .planets_resolved()
        .iter()
        .map(|p| {
            (
                i16::try_from(p.id).unwrap_or(-1),
                (i64::from(p.x), i64::from(p.y)),
            )
        })
        .collect();

    let mut prev: Option<GameState> = None;
    let mut events = 0usize;
    let mut habitable = 0usize;
    let mut rank_sum = 0usize;
    let mut rank_best = 0usize;
    let mut rank_top3 = 0usize;
    let mut chance_best = 0usize;
    let mut by_personality: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
    let mut rng = Rng::randomize(99);
    let mut shown = 0usize;
    let (mut fleet_events, mut fleet_best, mut fleet_top3) = (0usize, 0usize, 0usize);
    let (mut fleet_rank_sum, mut fleet_chance) = (0usize, 0usize);

    for year in &years {
        let Ok(bytes) = std::fs::read(year.join("Game.hst")) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let (state, _) = GameState::from_file(&file);

        if let Some(before) = &prev {
            // A host file records only owned planets, so a planet absent last
            // turn was unowned then. One taken from another player would have
            // been present under its old owner, so absence means colonised.
            let owned_before: HashMap<i16, i16> = before
                .planets
                .iter()
                .filter_map(|p| p.owner.map(|o| (p.id, o)))
                .collect();

            for planet in &state.planets {
                let Some(owner) = planet.owner else { continue };
                // Newly settled: not owned by anyone last turn, ours now.
                if owned_before.contains_key(&planet.id) {
                    continue;
                }
                let Some(player) = state.players.get(owner as usize) else {
                    continue;
                };
                let Control::Computer { personality, .. } = player.control else {
                    continue;
                };
                let name = personality.map_or("none", AiPersonality::name);
                events += 1;

                // Was the planet one the AI would even consider? Robotoid and
                // Macinti skip the habitability test entirely.
                let skips_test = matches!(
                    personality,
                    Some(AiPersonality::Robotoid) | Some(AiPersonality::Macinti)
                );
                let value = pct_planet_desirability(planet, &player.race);
                let slot = by_personality.entry(name).or_default();
                slot.1 += 1;
                if value >= 0 {
                    habitable += 1;
                    slot.0 += 1;
                } else if verbose && shown < 12 && !skips_test {
                    shown += 1;
                    println!(
                        "  {} planet {:>3} owner {owner} ({name}): value {value}",
                        state.year(),
                        planet.id
                    );
                }

                // Where does this planet rank, by distance from the player's
                // nearest planet last turn, among everything that was then
                // unowned and habitable for this race?
                let Some(&here) = pos.get(&planet.id) else {
                    continue;
                };
                let home: Vec<(i64, i64)> = before
                    .planets
                    .iter()
                    .filter(|p| p.owner == Some(owner))
                    .filter_map(|p| pos.get(&p.id).copied())
                    .collect();
                if home.is_empty() {
                    continue;
                }
                let dist = |a: (i64, i64)| {
                    home.iter()
                        .map(|h| (a.0 - h.0).pow(2) + (a.1 - h.1).pow(2))
                        .min()
                        .unwrap_or(i64::MAX)
                };

                // Candidates are everything nobody held last turn. The host
                // file records unowned planets it has seen as partial records,
                // which is where their environment comes from; a planet with no
                // record at all is judged habitable, since the AI cannot rule
                // out what it has not seen.
                let env: HashMap<i16, &stars_core::planet::Planet> = before
                    .planets
                    .iter()
                    .chain(before.known_planets.iter())
                    .map(|p| (p.id, p))
                    .collect();
                let mut candidates: Vec<(i64, i16)> = pos
                    .iter()
                    .filter(|(id, _)| !owned_before.contains_key(id))
                    .filter(|(id, _)| {
                        skips_test
                            || env
                                .get(id)
                                .is_none_or(|p| pct_planet_desirability(p, &player.race) >= 0)
                    })
                    .map(|(id, &c)| (dist(c), *id))
                    .collect();
                if candidates.len() < 2 {
                    continue;
                }
                candidates.sort_unstable();
                let rank = candidates
                    .iter()
                    .position(|(_, id)| *id == planet.id)
                    .unwrap_or(candidates.len());
                rank_sum += rank;
                if rank == 0 {
                    rank_best += 1;
                }
                if rank < 3 {
                    rank_top3 += 1;
                }
                let _ = here;

                // Chance control: a candidate drawn at random.
                let pick = (rng.next_raw().unsigned_abs() as usize) % candidates.len();
                if pick == 0 {
                    chance_best += 1;
                }
            }
        }
        // Second measurement: a colony fleet under way names its target.
        for fleet in &state.fleets {
            let Some(player) = state.players.get(fleet.owner.max(0) as usize) else {
                continue;
            };
            let Control::Computer { personality, .. } = player.control else {
                continue;
            };
            if fleet.cargo.colonists <= 0 {
                continue;
            }
            // The choice is made once, when the fleet sets out. A ship already
            // under way has flown past planets that were not candidates when it
            // was given its orders, so only a fleet still in orbit over one of
            // its own planets is scored.
            let Some(orbiting) = fleet.orbiting else {
                continue;
            };
            let orbiting = i16::try_from(orbiting).unwrap_or(-1);

            // The waypoint after the fleet's current position is its target.
            let Some(target) = fleet.waypoints.get(1).and_then(|w| w.target) else {
                continue;
            };
            let target = i16::try_from(target).unwrap_or(-1);
            if !pos.contains_key(&target) {
                continue;
            }
            let skips_test = matches!(
                personality,
                Some(AiPersonality::Robotoid) | Some(AiPersonality::Macinti)
            );
            let owned: std::collections::HashSet<i16> = state
                .planets
                .iter()
                .filter(|p| p.owner.is_some())
                .map(|p| p.id)
                .collect();
            if owned.contains(&target) {
                continue; // not a colonisation target
            }
            if state
                .planets
                .iter()
                .find(|p| p.id == orbiting)
                .and_then(|p| p.owner)
                != Some(fleet.owner)
            {
                continue; // not setting out from home
            }
            let here = (i64::from(fleet.position.x), i64::from(fleet.position.y));
            let env: HashMap<i16, &stars_core::planet::Planet> =
                state.planets.iter().map(|p| (p.id, p)).collect();
            let mut cands: Vec<(i64, i16)> = pos
                .iter()
                .filter(|(id, _)| !owned.contains(id))
                .filter(|(id, _)| {
                    skips_test
                        || env
                            .get(id)
                            .is_none_or(|p| pct_planet_desirability(p, &player.race) >= 0)
                })
                .map(|(id, c)| ((c.0 - here.0).pow(2) + (c.1 - here.1).pow(2), *id))
                .collect();
            if cands.len() < 2 {
                continue;
            }
            cands.sort_unstable();
            let r = cands
                .iter()
                .position(|(_, id)| *id == target)
                .unwrap_or(cands.len());
            fleet_events += 1;
            if r == 0 {
                fleet_best += 1;
            }
            if r < 3 {
                fleet_top3 += 1;
            }
            fleet_rank_sum += r;
            let pick = (rng.next_raw().unsigned_abs() as usize) % cands.len();
            if pick == 0 {
                fleet_chance += 1;
            }
        }

        prev = Some(state);
    }

    println!("{events} colonisation events");
    println!(
        "  planet habitable for the race: {habitable} ({}%)",
        pct(habitable, events)
    );
    println!("\nrank of the chosen planet among candidates, nearest first:");
    println!("  nearest:      {rank_best} ({}%)", pct(rank_best, events));
    println!("  top three:    {rank_top3} ({}%)", pct(rank_top3, events));
    println!(
        "  mean rank:    {:.1}",
        rank_sum as f64 / events.max(1) as f64
    );
    println!(
        "  chance control (random candidate is nearest): {chance_best} ({}%)",
        pct(chance_best, events)
    );
    println!("\n{fleet_events} colony fleets under way, scored from the fleet's own position:");
    println!(
        "  target is the nearest candidate: {fleet_best} ({}%)",
        pct(fleet_best, fleet_events)
    );
    println!(
        "  target in the nearest three:     {fleet_top3} ({}%)",
        pct(fleet_top3, fleet_events)
    );
    println!(
        "  mean rank: {:.1}   chance control: {fleet_chance} ({}%)",
        fleet_rank_sum as f64 / fleet_events.max(1) as f64,
        pct(fleet_chance, fleet_events)
    );
    println!("\nhabitable by personality (habitable / events):");
    for (name, (ok, total)) in &by_personality {
        println!("  {name:<12} {ok:>4} / {total:<5} {}%", pct(*ok, *total));
    }
}

fn pct(n: usize, total: usize) -> usize {
    n.checked_mul(100)
        .and_then(|x| x.checked_div(total))
        .unwrap_or(0)
}
