//! Compare two games for the objects "random events" would produce.
use stars_formats::{thing_records, StarsFile, ThingKind};
use std::collections::BTreeMap;
fn main() {
    for arg in std::env::args().skip(1) {
        let dir = std::path::PathBuf::from(&arg);
        let mut years: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        years.sort();
        let mut kinds: BTreeMap<&'static str, usize> = BTreeMap::new();
        for y in &years {
            let Ok(b) = std::fs::read(y.join("Game.hst")) else {
                continue;
            };
            let Ok(f) = StarsFile::decode(&b) else {
                continue;
            };
            for t in thing_records(&f) {
                let name = match t.kind {
                    ThingKind::Minefield(_) => "minefield",
                    ThingKind::MineralPacket(_) => "packet",
                    ThingKind::Wormhole(_) => "wormhole",
                    ThingKind::MysteryTrader(_) => "mystery trader",
                    _ => "other",
                };
                *kinds.entry(name).or_default() += 1;
            }
        }
        println!("{arg}: {kinds:?}");
    }
}
