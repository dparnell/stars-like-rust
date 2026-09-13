//! Print every fleet in a save, for a look at where things stand.
use stars_core::GameState;
use stars_formats::StarsFile;

fn main() {
    let path = std::env::args().nth(1).expect("a save file");
    let file = StarsFile::decode(&std::fs::read(&path).expect("read")).expect("decode");
    for block in file.segment_blocks(file.latest_segment()) {
        if (13..=15).contains(&block.type_id) {
            if let Some(record) = stars_formats::PlanetRecord::decode(&block.data, block.type_id) {
                println!(
                    "block type {} planet id {} owner {:?}",
                    block.type_id, record.id, record.owner
                );
            } else {
                println!("block type {} len {}", block.type_id, block.data.len());
            }
        }
    }
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
            "planet {} owner {:?} min {:?} f {} m {} pop {} scanner {:?} queue {:?}",
            p.id, p.owner, p.surface_min, p.factories, p.mines, p.pop, p.scanner, p.queue
        );
    }
    for p in &state.known_planets {
        println!(
            "known planet {} owner {:?} detail {:?} env {:?}",
            p.id, p.owner, p.detail, p.env
        );
    }
    for (i, pl) in state.players.iter().enumerate() {
        println!(
            "player {i} levels {:?} control {:?}",
            pl.research.levels, pl.control
        );
    }
}
