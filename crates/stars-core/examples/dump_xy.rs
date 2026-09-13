//! Print every planet's name and position from a universe file, with its
//! distance from a given planet id.
fn main() {
    let path = std::env::args().nth(1).expect("an .xy file");
    let from: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let universe =
        stars_formats::Universe::decode(&std::fs::read(&path).expect("read")).expect("decode");
    let planets = universe.planets_resolved();
    let origin = &planets[from];
    for (i, p) in planets.iter().enumerate() {
        let dx = f64::from(p.x) - f64::from(origin.x);
        let dy = f64::from(p.y) - f64::from(origin.y);
        println!(
            "{i:3} {:20} ({}, {}) {:6.1}",
            p.name.unwrap_or(""),
            p.x,
            p.y,
            (dx * dx + dy * dy).sqrt()
        );
    }
}
