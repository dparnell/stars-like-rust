//! Play the tutorial's world for a few years and print what the Berserkers
//! did each year — a debugging aid for the TurinDrone transcription.

fn main() {
    let years: i16 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(8);
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    let mut state = stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state;
    let mut rng = stars_core::rng::Rng::randomize(1);
    for _ in 0..years {
        let report = stars_core::generate_turn(&mut state, &mut rng);
        println!(
            "== {} research {:?} field {} pct {} spent {}",
            state.year(),
            state.players[1].research.levels,
            state.players[1].research.current_field,
            state.players[1].research_pct,
            state.players[1].research_last_year
        );
        for (player, did) in &report.ai {
            println!("ai {player}: {did:?}");
        }
        for f in state.fleets.iter().filter(|f| f.owner == 1) {
            println!(
                "  fleet {} at {:?} orbiting {:?} stacks {:?} next {:?}",
                f.id,
                f.position,
                f.orbiting,
                f.stacks
                    .iter()
                    .map(|s| (s.design, s.count))
                    .collect::<Vec<_>>(),
                f.waypoints.get(1).map(|w| (w.target, w.warp, w.task))
            );
        }
        let race = state.players[1].race.clone();
        let levels = state.players[1].research.levels;
        for id in &state.players[1].explored {
            if let Some(p) = state.planets.iter().find(|p| p.id == *id) {
                let reach = stars_core::terraform::optimal_env(p, &race, levels);
                let now = stars_core::hab::pct_planet_desirability(p, &race);
                let opt = stars_core::ai::colonise::pct_planet_opt_value(p, &race, reach);
                println!(
                    "  knows {} owner {:?} env {:?} now {now} opt {opt} reach {reach:?}",
                    p.id, p.owner, p.env
                );
            }
        }
        for p in state.planets.iter().filter(|p| p.owner == Some(1)) {
            println!(
                "  planet {} pop {} queue {:?}",
                p.id,
                p.pop,
                p.queue
                    .iter()
                    .map(|q| (q.item, q.ship, q.count))
                    .collect::<Vec<_>>()
            );
        }
    }
}
