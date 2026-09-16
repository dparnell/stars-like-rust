//! A software rasteriser for what egui paints.
//!
//! egui lays a frame out without a window and hands back textured
//! triangles; this fills them, flat-shaded from the vertex colours and
//! the texture, alpha-blended over a canvas. It is how a page of the
//! printed map becomes a picture with no printer or display behind it,
//! and how a screen is looked at from a shell that has neither.

use std::collections::HashMap;

/// A picture: straight RGBA, top row first.
pub type Image = stars_formats::resources::Image;

/// Lay out one frame at `size` and rasterise it.
///
/// `paint` is given the context as the shell would give it; it draws
/// whatever it likes. The frame is run twice half a second apart, so
/// anything that fades in has finished.
#[must_use]
pub fn render(size: (usize, usize), paint: &mut dyn FnMut(&egui::Context)) -> Image {
    let (width, height) = size;
    let ctx = egui::Context::default();
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width as f32, height as f32),
        )),
        ..Default::default()
    };
    let mut textures: HashMap<egui::TextureId, Image> = HashMap::new();
    let mut output = None;
    for _ in 0..2 {
        let out = ctx.run(input.clone(), |ctx| paint(ctx));
        for (id, delta) in &out.textures_delta.set {
            apply(&mut textures, *id, delta);
        }
        input.time = Some(input.time.unwrap_or(0.0) + 0.5);
        output = Some(out);
    }
    let output = output.expect("a frame");
    let meshes = ctx.tessellate(output.shapes, 1.0);

    let mut canvas = vec![[0xffu8, 0xff, 0xff]; width * height];
    for primitive in meshes {
        let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        let texture = textures.get(&mesh.texture_id);
        for tri in mesh.indices.chunks(3) {
            let v = [
                mesh.vertices[tri[0] as usize],
                mesh.vertices[tri[1] as usize],
                mesh.vertices[tri[2] as usize],
            ];
            triangle(&mut canvas, width, height, &v, texture, primitive.clip_rect);
        }
    }
    let mut pixels = Vec::with_capacity(width * height * 4);
    for [r, g, b] in canvas {
        pixels.extend_from_slice(&[r, g, b, 0xff]);
    }
    Image {
        width: width as u32,
        height: height as u32,
        pixels,
    }
}

/// Keep a texture up to date from the deltas egui sends.
fn apply(
    textures: &mut HashMap<egui::TextureId, Image>,
    id: egui::TextureId,
    delta: &egui::epaint::ImageDelta,
) {
    let (size, pixels): ([usize; 2], Vec<[u8; 4]>) = match &delta.image {
        egui::ImageData::Color(c) => (c.size, c.pixels.iter().map(|p| p.to_array()).collect()),
        egui::ImageData::Font(f) => (f.size, f.srgba_pixels(None).map(|p| p.to_array()).collect()),
    };
    let flat = |pixels: &[[u8; 4]]| -> Vec<u8> { pixels.iter().flatten().copied().collect() };
    match delta.pos {
        None => {
            textures.insert(
                id,
                Image {
                    width: size[0] as u32,
                    height: size[1] as u32,
                    pixels: flat(&pixels),
                },
            );
        }
        Some([x, y]) => {
            if let Some(whole) = textures.get_mut(&id) {
                let stride = whole.width as usize;
                for row in 0..size[1] {
                    for col in 0..size[0] {
                        let at = ((y + row) * stride + x + col) * 4;
                        if let Some(dst) = whole.pixels.get_mut(at..at + 4) {
                            dst.copy_from_slice(&pixels[row * size[0] + col]);
                        }
                    }
                }
            }
        }
    }
}

fn sample(texture: Option<&Image>, uv: egui::Pos2) -> [u8; 4] {
    let Some(t) = texture else {
        return [0xff; 4];
    };
    let x = ((uv.x * t.width as f32) as usize).min((t.width as usize).saturating_sub(1));
    let y = ((uv.y * t.height as f32) as usize).min((t.height as usize).saturating_sub(1));
    let at = (y * t.width as usize + x) * 4;
    t.pixels
        .get(at..at + 4)
        .map_or([0xff; 4], |p| [p[0], p[1], p[2], p[3]])
}

/// One triangle, clipped, blended over the canvas.
fn triangle(
    canvas: &mut [[u8; 3]],
    width: usize,
    height: usize,
    v: &[egui::epaint::Vertex; 3],
    texture: Option<&Image>,
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
    let max_x = (x0.max(x1).max(x2).min(clip.right()).ceil().max(0.0) as usize).min(width);
    let min_y = y0.min(y1).min(y2).max(clip.top()).floor().max(0.0) as usize;
    let max_y = (y0.max(y1).max(y2).min(clip.bottom()).ceil().max(0.0) as usize).min(height);
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
            let t = sample(texture, uv);
            let mix = |i: usize| -> f32 {
                let c = |c: egui::Color32| f32::from(c.to_array()[i]);
                w0 * c(v[0].color) + w1 * c(v[1].color) + w2 * c(v[2].color)
            };
            // Vertex colours are premultiplied; so is the texture.
            let r = mix(0) * f32::from(t[0]) / 255.0;
            let g = mix(1) * f32::from(t[1]) / 255.0;
            let b = mix(2) * f32::from(t[2]) / 255.0;
            let a = mix(3) * f32::from(t[3]) / 255.0 / 255.0;
            let dst = &mut canvas[py * width + px];
            for (channel, src) in dst.iter_mut().zip([r, g, b]) {
                *channel = (src + f32::from(*channel) * (1.0 - a))
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }
}
