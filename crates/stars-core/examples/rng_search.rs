//! Try to recover the gameplay RNG's state at the start of a recorded turn.
//!
//! The generator is seeded once per process by `Randomize2(GetTickCount())`,
//! and that seeding reaches at most 128*128 states because it indexes a
//! 128-entry primes table with two 7-bit values. So the unknown is not the
//! 62-bit state — it is a 14-bit seed choice plus however many draws the host
//! consumed before it reached `MineMinerals`.
//!
//! `MineMinerals` walks every planet in id order and draws exactly one
//! `Random(100)` per mineral whose hundredths remainder is non-zero, and
//! nothing else in that loop draws. On a planet that built nothing and had an
//! empty queue, the change in surface minerals is exactly what was mined, which
//! says whether that draw rounded up. That is a long, ordered sequence of
//! one-bit constraints — enough to identify a state if one exists.
use stars_core::mining::{minerals_mined, mines_operating};
use stars_core::race::RaceStat;
use stars_core::rng::Rng;
use stars_core::GameState;
use stars_formats::{crypt::PRIMES, StarsFile};

/// One draw the turn must have made, and what we know about its outcome.
#[derive(Clone, Copy)]
struct Draw {
    /// The `lQuanRem` the roll is compared against.
    remainder: i32,
    /// `Some(true)` if the roll must have been below the remainder, `Some(false)`
    /// if it must not, `None` where the planet's surface moved for other reasons.
    rounded_up: Option<bool>,
}

fn load(path: &std::path::Path) -> Option<GameState> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    Some(GameState::from_file(&file).0)
}

/// The ordered draws `MineMinerals` makes in the turn that turns `before` into
/// `after`.
fn draws(before: &GameState, after: &GameState) -> Vec<Draw> {
    let mut planets: Vec<_> = before.planets.iter().collect();
    planets.sort_by_key(|p| p.id);

    let mut out = Vec::new();
    for planet in planets {
        // EstMineralsMined returns before drawing for an unowned or empty planet.
        let Some(owner) = planet.owner else { continue };
        if planet.pop == 0 {
            continue;
        }
        let Some(player) = before.players.get(owner.max(0) as usize) else {
            continue;
        };
        let race = &player.race;
        let count = i32::from(mines_operating(planet, race));
        let efficiency = if race.is_ar() {
            10
        } else {
            i32::from(race.stat(RaceStat::MineProd))
        };
        let next = after.planets.iter().find(|p| p.id == planet.id);
        let same = next.is_some_and(|n| {
            n.owner == planet.owner && n.mines == planet.mines && n.factories == planet.factories
        });
        let truncated = minerals_mined(planet, race, None, None);

        for (i, truncated_i) in truncated.iter().enumerate() {
            let mut conc = i32::from(planet.min_conc[i]);
            if conc < 30 && planet.homeworld {
                conc = 30;
            }
            let remainder = (count * conc * efficiency / 10) % 100;
            if remainder == 0 {
                continue; // no draw at all
            }
            // A planet that spent minerals lands far from the mined figure, so
            // the 0/1 test filters those out on its own; requiring an empty
            // queue as well throws away most of the usable constraints.
            let rounded_up = if same {
                next.and_then(|n| {
                    let gain = n.surface_min[i] - planet.surface_min[i];
                    match gain - *truncated_i {
                        0 => Some(false),
                        1 => Some(true),
                        _ => None,
                    }
                })
            } else {
                None
            };
            out.push(Draw {
                remainder,
                rounded_up,
            });
        }
    }
    out
}

fn main() {
    let dir = std::env::args().nth(1).expect("game directory");
    let span: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000);
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    // One buffer of raw draws, reused for every candidate seed.
    let mut stream: Vec<i32> = vec![0; span + 4096];
    let tolerance: usize = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    // The search only has power when the constraints far outnumber the longest
    // run a random stream reaches by luck, so use the richest year pair.
    let mut chosen: Option<(usize, std::path::PathBuf, std::path::PathBuf)> = None;
    for pair in years.windows(2) {
        let (Some(before), Some(after)) = (
            load(&pair[0].join("Game.hst")),
            load(&pair[1].join("Game.hst")),
        ) else {
            continue;
        };
        let n = draws(&before, &after)
            .iter()
            .filter(|d| d.rounded_up.is_some())
            .count();
        if chosen.as_ref().is_none_or(|(best, _, _)| n > *best) {
            chosen = Some((n, pair[0].clone(), pair[1].clone()));
        }
    }
    let Some((_, first, second)) = chosen else {
        println!("no usable year pair");
        return;
    };
    {
        let pair = [first, second];
        let (Some(before), Some(after)) = (
            load(&pair[0].join("Game.hst")),
            load(&pair[1].join("Game.hst")),
        ) else {
            return;
        };
        let mut all = draws(&before, &after);
        // Self-test: replace the observed outcomes with ones this very
        // generator produced at a known seed and offset. If the search cannot
        // find a state it planted itself, a negative result on real data means
        // nothing.
        let selftest = std::env::args().any(|a| a == "selftest");
        if selftest {
            let (sa, sb, soff) = (37usize, 71usize, 12_345usize);
            let mut rng = Rng::from_seeds(PRIMES[sa], PRIMES[sb]);
            let mut planted = vec![0i32; soff + all.len()];
            for slot in planted.iter_mut() {
                *slot = rng.next_raw();
            }
            for (i, d) in all.iter_mut().enumerate() {
                let roll = (planted[soff + i] as u32 % 100) as i32;
                d.rounded_up = Some(roll < d.remainder);
            }
            println!("self-test: planted seeds {sa},{sb} at offset {soff}");
        }
        let all = all;
        let known: Vec<(usize, Draw)> = all
            .iter()
            .enumerate()
            .filter(|(_, d)| d.rounded_up.is_some())
            .map(|(i, d)| (i, *d))
            .collect();
        if known.len() < 40 {
            println!("too few constraints: {}", known.len());
            return;
        }
        // How often would a random stream satisfy these by luck? Constraints
        // with a large remainder are nearly free, so this is well above 1/2 and
        // is what a candidate run length has to be judged against.
        let chance: f64 = known
            .iter()
            .map(|(_, d)| {
                let p = f64::from(d.remainder) / 100.0;
                if d.rounded_up.unwrap() {
                    p
                } else {
                    1.0 - p
                }
            })
            .sum::<f64>()
            / known.len() as f64;

        let mut best: Option<(usize, usize, usize, usize)> = None; // (hits, a, b, offset)
        for (a, &prime_a) in PRIMES.iter().enumerate() {
            for (b, &prime_b) in PRIMES.iter().enumerate() {
                if a == b {
                    continue;
                }
                let mut rng = Rng::from_seeds(prime_a, prime_b);
                for slot in stream.iter_mut() {
                    *slot = rng.next_raw();
                }
                for offset in 0..span {
                    // Score the whole constraint set, tolerating a few
                    // mismatches. Some observed outcomes are wrong — a planet
                    // that also spent minerals can still land one kilotonne from
                    // the mined figure by coincidence — and demanding an
                    // unbroken prefix would hide the true state behind the first
                    // such error.
                    let mut hits = 0usize;
                    let mut misses = 0usize;
                    for (i, d) in &known {
                        let raw = stream[offset + i];
                        let roll = (raw as u32 % 100) as i32;
                        if (roll < d.remainder) == d.rounded_up.unwrap() {
                            hits += 1;
                        } else {
                            misses += 1;
                            if misses > tolerance {
                                break;
                            }
                        }
                    }
                    if best.is_none_or(|(h, _, _, _)| hits > h) {
                        best = Some((hits, a, b, offset));
                    }
                    if hits == known.len() {
                        break;
                    }
                }
                if best.is_some_and(|(h, _, _, _)| h == known.len()) {
                    break;
                }
            }
            if best.is_some_and(|(h, _, _, _)| h == known.len()) {
                break;
            }
        }

        let (hits, a, b, offset) = best.unwrap_or((0, 0, 0, 0));
        println!(
            "{} -> {}: {} draws, {} constrained (a random stream satisfies {:.0}% by luck); \
             best run {hits} (seeds {a},{b} offset {offset})",
            pair[0].file_name().unwrap().to_string_lossy(),
            pair[1].file_name().unwrap().to_string_lossy(),
            all.len(),
            known.len(),
            chance * 100.0,
        );
        // What run length would chance alone reach, over every (seed, offset)
        // pair tried? If the best run is not far above this, it says nothing.
        // With mismatches tolerated, a candidate is judged on total hits. A
        // random stream scores `chance` of them, with a spread of one standard
        // deviation; over this many tries the best of them lands several
        // deviations high, so only a score well past that means anything.
        let n = known.len() as f64;
        let trials = (PRIMES.len() * (PRIMES.len() - 1) * span) as f64;
        let mean = chance * n;
        let sd = (n * chance * (1.0 - chance)).sqrt();
        let threshold = mean + sd * (2.0 * trials.ln()).sqrt();
        println!(
            "  chance alone scores about {mean:.0} of {n:.0}; the best of \
             {trials:.0e} tries reaches about {threshold:.0}"
        );
        if hits == known.len() {
            println!("  FULL MATCH");
        } else if (hits as f64) < threshold {
            println!("  -> no signal: the best score is inside what luck reaches");
        } else {
            println!("  -> possible signal, worth checking");
        }
    }
}
