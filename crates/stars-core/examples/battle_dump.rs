//! Dump one battle recording: its raw bytes and the actions decoded from them.
//!
//! Written to chase the two sixteen-AI-game records that break the movement and
//! firing-range invariants — see `docs/formats/battle.md`.
//!
//! ```text
//! cargo run --release -p stars-core --example battle_dump -- <year dir> <hex id>
//! ```
use stars_formats::{battle_records_in_with, ActionLayout, StarsFile};
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let want = u16::from_str_radix(&std::env::args().nth(2).unwrap(), 16).unwrap();
    let names: Vec<String> = if dir.contains("exodus") {
        vec!["exodus.m6".into()]
    } else {
        (1..=16).map(|n| format!("Game.m{n}")).collect()
    };
    for name in names {
        let Ok(b) = std::fs::read(format!("{dir}/{name}")) else {
            continue;
        };
        let Ok(f) = StarsFile::decode(&b) else {
            continue;
        };
        let seg = f.latest_segment();
        for blk in f.segment_blocks(seg) {
            if blk.type_id != 31 {
                continue;
            }
            let id = u16::from_le_bytes([blk.data[0], blk.data[1]]);
            if id != want {
                continue;
            }
            println!(
                "{name} battle {id:#06x} len {} declared {}",
                blk.data.len(),
                u16::from_le_bytes([blk.data[6], blk.data[7]])
            );
            println!("  players {} tokens {}", blk.data[2], blk.data[3]);
            for (i, c) in blk.data.chunks(16).enumerate() {
                println!(
                    "  {:04x}  {}",
                    i * 16,
                    c.iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            }
            let h = &f.latest_segment().header;
            let layout = ActionLayout::for_version(h.version_major, h.version_minor);
            println!(
                "  version {}.{} -> {layout:?}",
                h.version_major, h.version_minor
            );
            for r in battle_records_in_with(std::slice::from_ref(blk), layout) {
                for (i, a) in r.actions.iter().enumerate() {
                    println!(
                        "  act {i:>2}: token {} dest {:?} round {} range {} target {} kills {}",
                        a.token,
                        a.destination,
                        a.round,
                        a.range,
                        a.target,
                        a.kills.len()
                    );
                }
            }
            return;
        }
    }
}
