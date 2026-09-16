//! Print the tutorial's map to a bitmap, as File (Print Map...) would.
//!
//! `cargo run -p stars-ui --example render_print -- out.bmp [across down]`

use stars_ui::App;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "map.bmp".to_string());
    let across: i16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let down: i16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    if let Ok(exe) = std::fs::read(root.join("stars.2.7j.exe")) {
        app.load_art(exe, "stars.2.7j.exe").expect("art");
    }
    app.print_pages = [across, down];
    let image = app.print_page_image(0);
    std::fs::write(&out, stars_formats::resources::write_bmp(&image)).expect("written");
    println!("{out}: {}x{}", image.width, image.height);
}
