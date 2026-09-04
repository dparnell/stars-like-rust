//! Check `mines_operating` against the mine count the recorded mining implies.
//!
//! The existing mining test scores the *amount* mined and allows one kilotonne
//! either way, which is exactly the width an off-by-one mine count usually
//! moves the answer — so it cannot see such an error. This inverts the
//! observation instead: given a quiet planet's three mineral gains, which mine
//! counts could have produced them? Three concentrations pin the count down far
//! more tightly than one does, and where a mineral's remainder is zero the
//! answer is exact and involves no RNG at all.
use stars_core::mining::mines_operating;
use stars_core::race::RaceStat;
use stars_core::{GameState, Planet, Race};
use stars_formats::StarsFile;
use std::collections::BTreeMap;

fn load(path: &std::path::Path) -> Option<GameState> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    Some(GameState::from_file(&file).0)
}

/// What a given mine count yields for one mineral: `(quantity, remainder)`.
fn yield_for(count: i32, conc: i32, efficiency: i32) -> (i32, i32) {
    let scaled = count * conc * efficiency / 10;
    (scaled / 100, scaled % 100)
}

fn concentration(planet: &Planet, i: usize) -> i32 {
    let mut conc = i32::from(planet.min_conc[i]);
    if conc < 30 && planet.homeworld {
        conc = 30;
    }
    conc
}

fn main() {
    let dir = std::env::args().nth(1).expect("game directory");
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    let (mut scored, mut in_range, mut pinned, mut pinned_exact) = (0usize, 0usize, 0usize, 0usize);
    let mut offsets: BTreeMap<i32, usize> = BTreeMap::new();
    // The RNG-free subset: every mineral's remainder is zero, so the gain is
    // fully determined.
    let (mut exact_scored, mut exact_ok) = (0usize, 0usize);

    for pair in years.windows(2) {
        let (Some(before), Some(after)) = (
            load(&pair[0].join("Game.hst")),
            load(&pair[1].join("Game.hst")),
        ) else {
            continue;
        };
        for planet in &before.planets {
            let Some(owner) = planet.owner else { continue };
            if planet.pop == 0 || !planet.queue.is_empty() {
                continue;
            }
            let Some(player) = before.players.get(owner.max(0) as usize) else {
                continue;
            };
            let race: &Race = &player.race;
            if race.is_ar() {
                continue; // AR mines from orbit; a different rule entirely
            }
            let Some(next) = after.planets.iter().find(|p| p.id == planet.id) else {
                continue;
            };
            if next.owner != planet.owner
                || next.mines != planet.mines
                || next.factories != planet.factories
            {
                continue;
            }
            let gains: Vec<i32> = (0..3)
                .map(|i| next.surface_min[i] - planet.surface_min[i])
                .collect();
            if gains.iter().any(|g| *g < 0) {
                continue;
            }
            let efficiency = i32::from(race.stat(RaceStat::MineProd));
            let ours = i32::from(mines_operating(planet, race));

            // Every mine count consistent with all three gains at once.
            let feasible: Vec<i32> = (0..=(i32::from(planet.mines).max(ours) + 4))
                .filter(|m| {
                    (0..3).all(|i| {
                        let (q, rem) = yield_for(*m, concentration(planet, i), efficiency);
                        gains[i] == q || (rem != 0 && gains[i] == q + 1)
                    })
                })
                .collect();
            if feasible.is_empty() {
                continue; // something other than mining moved the surface
            }
            scored += 1;
            if feasible.contains(&ours) {
                in_range += 1;
            }
            // The nearest consistent count, as a signed error.
            let nearest = feasible
                .iter()
                .min_by_key(|m| (*m - ours).abs())
                .copied()
                .unwrap_or(ours);
            *offsets.entry(nearest - ours).or_default() += 1;
            if feasible.len() == 1 {
                pinned += 1;
                if feasible[0] == ours {
                    pinned_exact += 1;
                }
            }
            // The subset with no RNG in it at all.
            if (0..3).all(|i| yield_for(ours, concentration(planet, i), efficiency).1 == 0) {
                exact_scored += 1;
                if (0..3)
                    .all(|i| gains[i] == yield_for(ours, concentration(planet, i), efficiency).0)
                {
                    exact_ok += 1;
                }
            }
        }
    }

    let pct = |n: usize, d: usize| (n * 100).checked_div(d).unwrap_or(0);
    println!("{scored} quiet planet-years where mining explains the surface change");
    println!(
        "  our mine count is among those consistent with it: {in_range} ({}%)",
        pct(in_range, scored)
    );
    println!(
        "  of the {pinned} where the gains pin down a single count, ours is right: \
         {pinned_exact} ({}%)",
        pct(pinned_exact, pinned)
    );
    println!("  nearest consistent count minus ours: {offsets:?}");
    println!(
        "  RNG-free subset (every remainder zero): {exact_ok} of {exact_scored} exact ({}%)",
        pct(exact_ok, exact_scored)
    );
}
