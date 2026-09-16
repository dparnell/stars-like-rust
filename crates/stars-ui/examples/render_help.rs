//! Draw the help viewer on a topic and write the frame out as a picture.
//!
//! egui lays a frame out without a window; what it hands back is a list of
//! textured triangles. This rasterises them in software — flat, no
//! anti-aliasing beyond what the mesh carries — so a screen can be looked
//! at from a shell that has no display. A development tool.
//!
//! `cargo run -p stars-ui --example render_help -- binary/STARS!.HLP 0x433 out.ppm`
//! (`contents` for the contents page; a decimal number is a topic offset;
//! `popup:<offset>` raises that topic as a popup over the contents page;
//! `search` and `history` open those windows over it; `about` is the About
//! box with its ordering information, a few seconds in).

use std::collections::HashMap;

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

    let ctx = egui::Context::default();
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width as f32, height as f32),
        )),
        ..Default::default()
    };
    // A few frames, half a second apart, so the window settles at its
    // default size and its fade-in is over.
    let mut textures: HashMap<egui::TextureId, egui::ColorImage> = HashMap::new();
    let mut full = None;
    for _ in 0..3 {
        app.start_frame();
        let output = ctx.run(input.clone(), |ctx| {
            egui::CentralPanel::default().show(ctx, |_ui| {});
            stars_ui::views::frame::dialogs(&mut app, ctx);
        });
        for (id, delta) in &output.textures_delta.set {
            apply(&mut textures, *id, delta);
        }
        input.time = Some(input.time.unwrap_or(0.0) + 1.5);
        full = Some(output);
    }
    let output = full.expect("a frame");
    let meshes = ctx.tessellate(output.shapes, 1.0);

    let mut canvas = vec![[0xf0u8, 0xf0, 0xf0]; width * height];
    for primitive in meshes {
        let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        let clip = primitive.clip_rect;
        let texture = textures.get(&mesh.texture_id);
        for tri in mesh.indices.chunks(3) {
            let v = [
                mesh.vertices[tri[0] as usize],
                mesh.vertices[tri[1] as usize],
                mesh.vertices[tri[2] as usize],
            ];
            triangle(&mut canvas, width, height, &v, texture, clip);
        }
    }

    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    for px in canvas {
        bytes.extend_from_slice(&px);
    }
    std::fs::write(&out, bytes).expect("written");
    println!("{out}: {width}x{height}");
}

fn apply(
    textures: &mut HashMap<egui::TextureId, egui::ColorImage>,
    id: egui::TextureId,
    delta: &egui::epaint::ImageDelta,
) {
    let image = match &delta.image {
        egui::ImageData::Color(c) => (**c).clone(),
        egui::ImageData::Font(f) => {
            let pixels: Vec<egui::Color32> = f.srgba_pixels(None).collect();
            egui::ColorImage {
                size: f.size,
                pixels,
            }
        }
    };
    match delta.pos {
        None => {
            textures.insert(id, image);
        }
        Some([x, y]) => {
            if let Some(whole) = textures.get_mut(&id) {
                for row in 0..image.size[1] {
                    for col in 0..image.size[0] {
                        let at = (y + row) * whole.size[0] + x + col;
                        if let Some(p) = whole.pixels.get_mut(at) {
                            *p = image.pixels[row * image.size[0] + col];
                        }
                    }
                }
            }
        }
    }
}

fn sample(texture: Option<&egui::ColorImage>, uv: egui::Pos2) -> egui::Color32 {
    let Some(t) = texture else {
        return egui::Color32::WHITE;
    };
    let x = ((uv.x * t.size[0] as f32) as usize).min(t.size[0].saturating_sub(1));
    let y = ((uv.y * t.size[1] as f32) as usize).min(t.size[1].saturating_sub(1));
    t.pixels[y * t.size[0] + x]
}

/// One triangle, flat-shaded per pixel from the vertex colours and the
/// texture, alpha-blended over the canvas, clipped to the clip rect.
fn triangle(
    canvas: &mut [[u8; 3]],
    width: usize,
    height: usize,
    v: &[egui::epaint::Vertex; 3],
    texture: Option<&egui::ColorImage>,
    clip: egui::Rect,
) {
    let (x0, y0) = (v[0].pos.x, v[0].pos.y);
    let (x1, y1) = (v[1].pos.x, v[1].pos.y);
    let (x2, y2) = (v[2].pos.x, v[2].pos.y);
    let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    if area.abs() < 1e-6 {
        return;
    }
    let min_x = x0.min(x1).min(x2).max(clip.left()).floor().max(0.0) as usize;
    let max_x = (x0.max(x1).max(x2).min(clip.right()).ceil() as usize).min(width);
    let min_y = y0.min(y1).min(y2).max(clip.top()).floor().max(0.0) as usize;
    let max_y = (y0.max(y1).max(y2).min(clip.bottom()).ceil() as usize).min(height);
    for py in min_y..max_y {
        for px in min_x..max_x {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
            let w0 = ((x1 - x) * (y2 - y) - (x2 - x) * (y1 - y)) / area;
            let w1 = ((x2 - x) * (y0 - y) - (x0 - x) * (y2 - y)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let uv = egui::pos2(
                w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x,
                w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y,
            );
            let tex = sample(texture, uv);
            let mix = |i: usize| -> f32 {
                let c = |c: egui::Color32| c.to_array()[i] as f32;
                w0 * c(v[0].color) + w1 * c(v[1].color) + w2 * c(v[2].color)
            };
            // Vertex colours are premultiplied; so is the texture.
            let t = tex.to_array();
            let r = mix(0) * t[0] as f32 / 255.0;
            let g = mix(1) * t[1] as f32 / 255.0;
            let b = mix(2) * t[2] as f32 / 255.0;
            let a = mix(3) * t[3] as f32 / 255.0 / 255.0;
            let dst = &mut canvas[py * width + px];
            for (channel, src) in dst.iter_mut().zip([r, g, b]) {
                *channel = (src + *channel as f32 * (1.0 - a))
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }
}
