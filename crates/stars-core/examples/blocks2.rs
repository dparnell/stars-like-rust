use stars_formats::StarsFile;
use std::collections::BTreeMap;
fn main() {
    for path in std::env::args().skip(1) {
        let Ok(b) = std::fs::read(&path) else {
            continue;
        };
        let Ok(f) = StarsFile::decode(&b) else {
            continue;
        };
        let mut all: BTreeMap<u8, usize> = BTreeMap::new();
        for blk in &f.blocks {
            *all.entry(blk.type_id).or_default() += 1;
        }
        let mut seg: BTreeMap<u8, usize> = BTreeMap::new();
        for blk in f.segment_blocks(f.latest_segment()) {
            *seg.entry(blk.type_id).or_default() += 1;
        }
        println!(
            "{path}\n  segments {} all {all:?}\n  latest {seg:?}",
            f.segments().len()
        );
    }
}
