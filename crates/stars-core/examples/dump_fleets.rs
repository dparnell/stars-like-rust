//! Print every fleet in a save, for a look at where things stand.
use stars_core::GameState;
use stars_formats::StarsFile;

fn main() {
    let path = std::env::args().nth(1).expect("a save file");
    let file = StarsFile::decode(&std::fs::read(&path).expect("read")).expect("decode");
    let (state, _) = GameState::from_file(&file);
    println!("turn {}", state.turn);
    for f in &state.fleets {
        println!(
            "fleet {} owner {} at {:?} orbiting {:?} fuel {} cargo {:?} stacks {:?} wps {:?}",
            f.id,
            f.owner,
            f.position,
            f.orbiting,
            f.cargo.fuel,
            f.cargo.minerals,
            f.stacks
                .iter()
                .map(|s| (s.design, s.count))
                .collect::<Vec<_>>(),
            f.waypoints
                .iter()
                .map(|w| (w.target, w.warp, w.position, w.task))
                .collect::<Vec<_>>()
        );
    }
    for p in &state.planets {
        println!(
            "planet {} owner {:?} min {:?} f {} m {} pop {} queue {:?}",
            p.id, p.owner, p.surface_min, p.factories, p.mines, p.pop, p.queue
        );
    }
}
