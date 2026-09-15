//! Print who controls each player in a saved game, with names from the record.
use stars_formats::{player_records_in, StarsFile};
fn main() {
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).unwrap();
        let file = StarsFile::decode(&bytes).unwrap();
        let seg = file.latest_segment();
        println!("{path}");
        for r in &player_records_in(file.segment_blocks(seg)).unwrap() {
            println!(
                "  player {:>2}: flags {:#04x} ({:08b}) personality {} skill {}  {} / {}",
                r.player_number,
                r.flags_byte,
                r.flags_byte,
                r.flags_byte >> 5,
                (r.flags_byte >> 2) & 3,
                r.singular_name,
                r.plural_name
            );
        }
    }
}
