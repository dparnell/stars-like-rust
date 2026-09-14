//! Print every owned planet in a save: people, installations, minerals, queue.
use stars_core::GameState;
use stars_formats::StarsFile;

fn main() {
    let path = std::env::args().nth(1).expect("a save file");
    let file = StarsFile::decode(&std::fs::read(&path).expect("read")).expect("decode");
    let (state, _) = GameState::from_file(&file);
    println!("turn {}", state.turn);
    for p in state.planets.iter().chain(state.known_planets.iter()) {
        if p.owner.is_some() || p.id < 2 {
            println!(
                "planet {} home {} owner {:?} pop {} mines {} fact {} conc {:?} surface {:?} queue {:?}",
                p.id,
                p.homeworld,
                p.owner,
                p.pop,
                p.mines,
                p.factories,
                p.min_conc,
                p.surface_min,
                p.queue
                    .iter()
                    .map(|q| (q.item, q.ship, q.count, q.completion))
                    .collect::<Vec<_>>()
            );
        }
    }
}
