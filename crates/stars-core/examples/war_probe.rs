use stars_core::GameState;
use stars_formats::StarsFile;
use std::collections::{BTreeMap, HashMap};
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();
    let mut blocks: BTreeMap<u8, usize> = BTreeMap::new();
    let mut conquests = 0usize;
    let mut prev: HashMap<i16, i16> = HashMap::new();
    for y in &years {
        let Ok(b) = std::fs::read(y.join("Game.hst")) else {
            continue;
        };
        let Ok(f) = StarsFile::decode(&b) else {
            continue;
        };
        for blk in f.segment_blocks(f.latest_segment()) {
            *blocks.entry(blk.type_id).or_default() += 1;
        }
        let (state, _) = GameState::from_file(&f);
        let now: HashMap<i16, i16> = state
            .planets
            .iter()
            .filter_map(|p| p.owner.map(|o| (p.id, o)))
            .collect();
        for (id, o) in &now {
            if let Some(was) = prev.get(id) {
                if was != o {
                    conquests += 1;
                }
            }
        }
        prev = now;
    }
    println!("block types across the corpus: {blocks:?}");
    println!("planets changing hands between owners: {conquests}");
}
