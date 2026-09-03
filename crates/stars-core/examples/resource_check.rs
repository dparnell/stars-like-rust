//! Score `resources_at_planet` against a recorded game.
//!
//! A player's resources are not stored anywhere directly. `PLAYER.lResLastYear`
//! records what research received, but that is *not* `resources x
//! research_pct`: resources the production queue did not consume also fall
//! through to research, so a player with `research_pct = 0` still shows a
//! non-zero figure. It cannot measure resource output directly.
//!
//! What it does give is a hard **lower bound**: research can never exceed the
//! resources that produced it. A player-year where our modelled output is
//! below the recorded research is a definite under-estimate, and that is what
//! this scores.

use std::collections::BTreeMap;

use stars_core::resources::resources_at_planet;
use stars_core::GameState;
use stars_formats::StarsFile;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: resource_check <dir>");
    let verbose = std::env::args().any(|a| a == "-v");
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    let mut scored = 0usize;
    let mut exact = 0usize;
    let mut within_1pct = 0usize;
    let mut unmodelled = 0usize;
    let mut by_prt: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut shown = 0usize;

    for year in &years {
        let Ok(bytes) = std::fs::read(year.join("Game.hst")) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let (state, _) = GameState::from_file(&file);

        for (i, player) in state.players.iter().enumerate() {
            let recorded = player.research_last_year;
            if recorded <= 0 {
                continue;
            }
            let owner = i16::try_from(i).unwrap_or(-1);

            let mut total = 0i64;
            let mut missing = false;
            for planet in state.planets.iter().filter(|p| p.owner == Some(owner)) {
                match resources_at_planet(
                    planet,
                    &player.race,
                    i16::from(player.research.levels[0]),
                ) {
                    Some(r) => total += i64::from(r),
                    None => missing = true,
                }
            }
            if missing {
                unmodelled += 1;
                continue;
            }

            scored += 1;
            let prt = format!("{:?}", player.race.prt());
            let slot = by_prt.entry(prt).or_default();
            slot.1 += 1;

            let want = i64::from(recorded);
            if total >= want {
                exact += 1;
                slot.0 += 1;
            }
            if total >= want {
                within_1pct += 1;
            } else if verbose && shown < 12 {
                shown += 1;
                let planets: Vec<_> = state
                    .planets
                    .iter()
                    .filter(|p| p.owner == Some(owner))
                    .map(|p| {
                        (
                            p.id,
                            p.pop,
                            p.factories,
                            resources_at_planet(
                                p,
                                &player.race,
                                i16::from(player.research.levels[0]),
                            ),
                        )
                    })
                    .collect();
                println!(
                    "  {} player {i:>2} ({:?}): resources {total} < research {want}, \
                     pct {} planets {planets:?}",
                    state.year(),
                    player.race.prt(),
                    player.research_pct
                );
            }
        }
    }

    println!("{scored} player-years scored ({unmodelled} skipped: unmodelled race)");
    println!(
        "  resources >= recorded research: {exact} ({}%)",
        pct(exact, scored)
    );
    let _ = within_1pct;
    println!("\nby primary trait:");
    for (prt, (ok, total)) in &by_prt {
        println!("  {prt:<12} {ok:>5} / {total:<6} {}%", pct(*ok, *total));
    }
}

fn pct(n: usize, total: usize) -> usize {
    n.checked_mul(100)
        .and_then(|x| x.checked_div(total))
        .unwrap_or(0)
}
