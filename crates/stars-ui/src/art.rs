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
    /// Where it came from, for the frontend to show.
    pub source: String,
}

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
        let name = cell.name();
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
