//! Draw the help viewer on a topic and write the frame out as a picture.
//!
//! egui lays a frame out without a window and `stars_ui::raster` fills
//! the triangles it hands back, so a screen can be looked at from a shell
//! that has no display. A development tool.
//!
//! `cargo run -p stars-ui --example render_help -- binary/STARS!.HLP 0x433 out.ppm`
//! (`contents` for the contents page; a decimal number is a topic offset;
//! `popup:<offset>` raises that topic as a popup over the contents page;
//! `search` and `history` open those windows over it; `about` is the About
//! box with its ordering information, a few seconds in).

use stars_ui::App;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("a help file");
    let what = args.next().unwrap_or_else(|| "contents".to_string());
    let out = args.next().unwrap_or_else(|| "help.ppm".to_string());
    let (width, height) = (1024usize, 768usize);

    let mut app = App::new();
    app.load_help(std::fs::read(&path).expect("readable"), &path)
        .expect("a help file");
    match what.as_str() {
        "contents" => app.help_contents(),
        s if s.starts_with("0x") => {
            app.help_context(u32::from_str_radix(&s[2..], 16).expect("hex"));
        }
        "search" => {
            app.help_contents();
            app.help_search_open();
            app.help.search.as_mut().expect("open").text = "cargo".to_string();
            let first = app.help_keyword_matches()[0];
            app.help.search.as_mut().expect("open").keyword = Some(first);
            app.help_show_topics();
        }
        "about" => {
            let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
            if let Ok(exe) = std::fs::read(root.join("stars.2.7j.exe")) {
                app.load_art(exe, "stars.2.7j.exe").expect("art");
            }
            app.open_about(0.0);
            app.open_original_about(0.0);
            app.about.as_mut().expect("open").order_info = true;
        }
        "history" => {
            app.help_contents();
            app.help_context(0x433);
            app.help_context(0x43a);
            app.help.history_open = true;
        }
        s if s.starts_with("popup:") => {
            app.help_contents();
            app.help_popup(s[6..].parse().expect("a topic offset"), [200.0, 200.0]);
        }
        s => {
            app.help_goto(s.parse().expect("a topic offset"));
        }
    }

    let image = stars_ui::raster::render((width, height), &mut |ctx| {
        app.start_frame();
        egui::CentralPanel::default().show(ctx, |_ui| {});
        stars_ui::views::frame::dialogs(&mut app, ctx);
    });
    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    for px in image.pixels.chunks(4) {
        bytes.extend_from_slice(&px[..3]);
    }
    std::fs::write(&out, bytes).expect("written");
    println!("{out}: {width}x{height}");
}
