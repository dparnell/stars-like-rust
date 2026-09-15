//! Dump one bitmap resource of the executable as a PPM: `dump_bitmap <exe> <id> <out.ppm>`.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let exe = std::fs::read(&args[1]).unwrap();
    let id: u16 = args[2].parse().unwrap();
    let img = stars_formats::resources::read_bitmap(&exe, &stars_formats::resources::Name::Id(id))
        .unwrap();
    eprintln!("{}x{}", img.width, img.height);
    let mut out = format!("P6\n{} {}\n255\n", img.width, img.height).into_bytes();
    for y in 0..img.height {
        for x in 0..img.width {
            let (r, g, b, _) = img.pixel(x, y).unwrap();
            out.extend([r, g, b]);
        }
    }
    std::fs::write(&args[3], out).unwrap();
}
