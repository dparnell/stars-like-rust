//! The game's own pictures, borrowed at run time.
//!
//! Everything the original draws lives in its executable's resource table, and
//! [`stars_formats::resources`] reads it. Nothing is copied into this
//! repository: a player points this at the `stars.exe` they already have and
//! the reimplementation draws with the real artwork; without one it draws
//! exactly as it did before, so the pictures are an improvement and never a
//! requirement.
//!
//! One texture is uploaded per **sheet** and each sprite is drawn as a
//! rectangle of it, which is what the original does too — it keeps one DIB per
//! sheet and blits out of it.
//!
//! See `docs/formats/resources.md`.

use std::collections::HashMap;

use stars_formats::resources::{art::Cell, read_bitmap, resources, Image, Name, RT_BITMAP};

/// The pictures out of one copy of the game.
pub struct Art {
    /// The executable itself, kept so sheets can be decoded on demand — most
    /// of a game never asks for most of them.
    executable: Vec<u8>,
    /// Sheets already decoded. A `None` is a sheet that would not decode, kept
    /// so it is not tried again every frame.
    decoded: HashMap<Name, Option<Image>>,
    /// Sheets already uploaded.
    textures: HashMap<Name, egui::TextureHandle>,
    /// Monochrome sheets uploaded as **stencils** — the drawn part opaque and
    /// the rest clear, so the caller can tint it.
    stencils: HashMap<Name, egui::TextureHandle>,
    /// Single cells uploaded with a **mask** cell applied as their alpha, keyed
    /// by the sheet and the two cells' corners.
    masked: HashMap<MaskedKey, egui::TextureHandle>,
    /// Where it came from, for the frontend to show.
    pub source: String,
}

/// What a masked cell is cached under: the sheet, the cell's corner, the
/// mask's corner and the size of both.
type MaskedKey = (Name, u32, u32, u32, u32, u32, u32);

impl Art {
    /// Take a copy of the executable and check that it really has the
    /// pictures in it.
    ///
    /// # Errors
    ///
    /// A message suitable for showing to the player, when the file is not a
    /// 16-bit executable or holds none of the bitmaps the game loads.
    pub fn open(executable: Vec<u8>, source: &str) -> Result<Art, String> {
        let table = resources(&executable).map_err(|e| format!("{source}: {e}"))?;
        let bitmaps = table
            .iter()
            .filter(|r| r.kind == Name::Id(RT_BITMAP))
            .count();
        if bitmaps == 0 {
            return Err(format!("{source} has no bitmaps in it"));
        }
        Ok(Art {
            executable,
            decoded: HashMap::new(),
            textures: HashMap::new(),
            stencils: HashMap::new(),
            masked: HashMap::new(),
            source: source.to_string(),
        })
    }

    /// How many bitmaps the executable holds.
    #[must_use]
    pub fn bitmaps(&self) -> usize {
        resources(&self.executable)
            .map(|table| {
                table
                    .iter()
                    .filter(|r| r.kind == Name::Id(RT_BITMAP))
                    .count()
            })
            .unwrap_or(0)
    }

    /// One decoded sheet, decoding it the first time it is asked for.
    pub fn sheet(&mut self, name: &Name) -> Option<&Image> {
        if !self.decoded.contains_key(name) {
            let image = read_bitmap(&self.executable, name).ok();
            self.decoded.insert(name.clone(), image);
        }
        self.decoded.get(name).and_then(Option::as_ref)
    }

    /// One sheet as a texture, uploading it the first time it is asked for.
    ///
    /// Nearest-neighbour, because these are pixels drawn one at a time and
    /// smoothing them is not kindness.
    pub fn texture(&mut self, ctx: &egui::Context, name: &Name) -> Option<&egui::TextureHandle> {
        if !self.textures.contains_key(name) {
            let image = self.sheet(name)?;
            let size = [image.width as usize, image.height as usize];
            let colour = egui::ColorImage::from_rgba_unmultiplied(size, &image.pixels);
            let handle = ctx.load_texture(
                format!("stars-art-{name:?}"),
                colour,
                egui::TextureOptions::NEAREST,
            );
            self.textures.insert(name.clone(), handle);
        }
        self.textures.get(name)
    }

    /// One cell of a sheet, drawn at its own size.
    ///
    /// For a cell that is not square — the toolbar's buttons are 24 by 23 —
    /// this is what keeps it from being stretched.
    pub fn sprite_rect(&mut self, ctx: &egui::Context, cell: Cell) -> Option<egui::Image<'_>> {
        let size = egui::vec2(cell.width as f32, cell.height as f32);
        self.sprite_sized(ctx, cell, size)
    }

    /// One cell of a sheet, as something to put in a `ui`.
    ///
    /// `None` when the picture is missing or the cell falls outside it — which
    /// is how a sprite index off the end of a half-width sheet is caught.
    pub fn sprite(
        &mut self,
        ctx: &egui::Context,
        cell: Cell,
        size: f32,
    ) -> Option<egui::Image<'_>> {
        self.sprite_sized(ctx, cell, egui::vec2(size, size))
    }

    /// One cell of a sheet at a size of the caller's choosing.
    fn sprite_sized(
        &mut self,
        ctx: &egui::Context,
        cell: Cell,
        size: egui::Vec2,
    ) -> Option<egui::Image<'_>> {
        self.sprite_at(
            ctx,
            &cell.name(),
            cell.x,
            cell.y,
            cell.width,
            cell.height,
            size,
        )
    }

    /// A rectangle of any sheet, named however it is named.
    ///
    /// [`Cell`] can only name a bitmap by number, and a few of the game's are
    /// named by string — the scanner's own sheet among them — so this is the
    /// way in for those.
    #[allow(clippy::too_many_arguments)]
    pub fn sprite_at(
        &mut self,
        ctx: &egui::Context,
        name: &Name,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        size: egui::Vec2,
    ) -> Option<egui::Image<'_>> {
        let cell = Cell {
            resource: 0,
            x,
            y,
            width,
            height,
        };
        let name = name.clone();
        let handle = self.texture(ctx, &name)?;
        let sheet = handle.size_vec2();
        if cell.x + cell.width > sheet.x as u32 || cell.y + cell.height > sheet.y as u32 {
            return None;
        }
        let uv = egui::Rect::from_min_max(
            egui::pos2(cell.x as f32 / sheet.x, cell.y as f32 / sheet.y),
            egui::pos2(
                (cell.x + cell.width) as f32 / sheet.x,
                (cell.y + cell.height) as f32 / sheet.y,
            ),
        );
        Some(
            egui::Image::new((handle.id(), size))
                .uv(uv)
                .fit_to_exact_size(size),
        )
    }
}

impl std::fmt::Debug for Art {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Art")
            .field("source", &self.source)
            .field("decoded", &self.decoded.len())
            .finish()
    }
}

/// Draw one cell, and say whether there was anything to draw.
pub fn draw(app: &mut crate::App, ui: &mut egui::Ui, cell: Cell, size: f32) -> bool {
    draw_with(app.art.as_mut(), ui, cell, size)
}

/// The same, for a view that has taken the pictures out of the app because it
/// is already holding a borrow of the game.
pub fn draw_with(art: Option<&mut Art>, ui: &mut egui::Ui, cell: Cell, size: f32) -> bool {
    let ctx = ui.ctx().clone();
    let Some(art) = art else {
        return false;
    };
    let Some(image) = art.sprite(&ctx, cell, size) else {
        return false;
    };
    ui.add(image);
    true
}

impl Art {
    /// One cell of a sheet, with a second cell used as its **mask**.
    ///
    /// This is the pair of blits the game uses for a glyph that has to sit on
    /// whatever is behind it — the mask `AND`ed in and then the image `OR`ed
    /// (`0x8800c6` then `0xee0086`). A Windows AND-mask is **white where the
    /// background shows through**, so that is where the alpha goes to zero.
    pub fn sprite_masked_at(
        &mut self,
        ctx: &egui::Context,
        name: &Name,
        cell: (u32, u32),
        mask: (u32, u32),
        size: (u32, u32),
        draw_at: egui::Vec2,
    ) -> Option<egui::Image<'_>> {
        let key = (name.clone(), cell.0, cell.1, mask.0, mask.1, size.0, size.1);
        if !self.masked.contains_key(&key) {
            let sheet = self.sheet(name)?;
            let (width, height) = (size.0 as usize, size.1 as usize);
            let mut pixels = Vec::with_capacity(width * height * 4);
            for row in 0..height {
                for column in 0..width {
                    let at = |x: u32, y: u32| -> Option<[u8; 4]> {
                        let x = x as usize + column;
                        let y = y as usize + row;
                        let (sheet_w, sheet_h) = (sheet.width as usize, sheet.height as usize);
                        if x >= sheet_w || y >= sheet_h {
                            return None;
                        }
                        let i = (y * sheet_w + x) * 4;
                        Some([
                            sheet.pixels[i],
                            sheet.pixels[i + 1],
                            sheet.pixels[i + 2],
                            sheet.pixels[i + 3],
                        ])
                    };
                    let (Some(colour), Some(mask)) = (at(cell.0, cell.1), at(mask.0, mask.1))
                    else {
                        return None;
                    };
                    // White in the mask is background; anything darker is the
                    // glyph.
                    let clear = mask[0] > 0x7f && mask[1] > 0x7f && mask[2] > 0x7f;
                    pixels.extend_from_slice(&[
                        colour[0],
                        colour[1],
                        colour[2],
                        if clear { 0x00 } else { 0xff },
                    ]);
                }
            }
            let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels);
            let handle = ctx.load_texture(
                format!("stars-masked-{:?}-{}-{}", name, cell.0, cell.1),
                image,
                egui::TextureOptions::NEAREST,
            );
            self.masked.insert(key.clone(), handle);
        }
        let handle = self.masked.get(&key)?;
        Some(egui::Image::new((handle.id(), draw_at)).fit_to_exact_size(draw_at))
    }

    /// A rectangle of a **monochrome** sheet, as a stencil to be tinted.
    ///
    /// The game's one-bit sheets are not pictures: they are shapes, blitted
    /// through a mask so that the colour comes from the pen rather than the
    /// bitmap — `SRCAND` then `SRCPAINT` with the text colour set. Drawn as
    /// they are stored, they would be black-and-white squares.
    ///
    /// This turns one into a texture whose **drawn part is opaque white and
    /// whose ground is clear**, so `.tint(colour)` finishes the job. The
    /// "drawn part" is the black pixel: in a one-bit bitmap the shape is index
    /// zero and the ground is index one.
    #[allow(clippy::too_many_arguments)]
    pub fn stencil_at(
        &mut self,
        ctx: &egui::Context,
        name: &Name,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        size: egui::Vec2,
    ) -> Option<egui::Image<'_>> {
        if !self.stencils.contains_key(name) {
            let image = self.sheet(name)?;
            let pixels: Vec<u8> = image
                .pixels
                .chunks(4)
                .flat_map(|p| {
                    let drawn = p[0] == 0 && p[1] == 0 && p[2] == 0;
                    let alpha = if drawn { 0xff } else { 0x00 };
                    [0xff, 0xff, 0xff, alpha]
                })
                .collect();
            let colour = egui::ColorImage::from_rgba_unmultiplied(
                [image.width as usize, image.height as usize],
                &pixels,
            );
            let handle = ctx.load_texture(
                format!("stars-stencil-{name:?}"),
                colour,
                egui::TextureOptions::NEAREST,
            );
            self.stencils.insert(name.clone(), handle);
        }
        let handle = self.stencils.get(name)?;
        let sheet = handle.size_vec2();
        if x + width > sheet.x as u32 || y + height > sheet.y as u32 {
            return None;
        }
        let uv = egui::Rect::from_min_max(
            egui::pos2(x as f32 / sheet.x, y as f32 / sheet.y),
            egui::pos2((x + width) as f32 / sheet.x, (y + height) as f32 / sheet.y),
        );
        Some(
            egui::Image::new((handle.id(), size))
                .uv(uv)
                .fit_to_exact_size(size),
        )
    }
}
