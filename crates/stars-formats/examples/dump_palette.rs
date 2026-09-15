//! Print a DIB resource's header and one colour-table entry: `dump_palette <exe> <id> <index>`.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let exe = std::fs::read(&args[1]).unwrap();
    let id: u16 = args[2].parse().unwrap();
    let index: usize = args[3].parse().unwrap();
    let res =
        stars_formats::resources::find(&exe, 2, &stars_formats::resources::Name::Id(id)).unwrap();
    let data = res.data(&exe).unwrap();
    let header = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let bits = u16::from_le_bytes([data[14], data[15]]);
    let used = u32::from_le_bytes([data[32], data[33], data[34], data[35]]);
    println!(
        "header {header} bits {bits} clrUsed {used} len {}",
        data.len()
    );
    let at = header + index * 4;
    println!(
        "entry {index} at +{at:#x}: B {} G {} R {}",
        data[at],
        data[at + 1],
        data[at + 2]
    );
}
