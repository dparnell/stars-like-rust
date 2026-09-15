//! Dump an icon group as a PPM with the mask shown as magenta: `dump_icons <exe> <group> <out.ppm>`.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let exe = std::fs::read(&args[1]).unwrap();
    let img = stars_formats::resources::read_icon(
        &exe,
        &stars_formats::resources::Name::Text(args[2].clone()),
    )
    .unwrap();
    eprintln!("{}x{}", img.width, img.height);
    let mut out = format!("P6\n{} {}\n255\n", img.width, img.height).into_bytes();
    for y in 0..img.height {
        for x in 0..img.width {
            let (r, g, b, a) = img.pixel(x, y).unwrap();
            if a == 0 {
                out.extend([255, 0, 255])
            } else {
                out.extend([r, g, b])
            }
        }
    }
    std::fs::write(&args[3], out).unwrap();
}
