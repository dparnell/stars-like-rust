//! The pictures, read out of the game's own executable.
//!
//! `stars.exe` is a 16-bit **NE** (New Executable), and everything the game
//! draws — the component pictures, the race emblems, the planets, the ships,
//! the toolbar — is a Windows bitmap in its resource table. Nothing is
//! extracted into this repository: this module walks a copy of the executable
//! the player already has and decodes what it finds, so the game's artwork
//! stays where it is and the reimplementation borrows it at run time.
//!
//! Two layers, both pure functions over bytes:
//!
//! * [`resources`] walks the NE resource table and says what is in it and
//!   where;
//! * [`read_dib`] decodes one device-independent bitmap to straight RGBA.
//!
//! [`art`] then names them — which resource is the race emblems, how the
//! component pictures are tiled — transcribed from the calls that load and
//! blit them.
//!
//! See `docs/formats/resources.md`.

use crate::{FormatError, Result};

/// The `RT_BITMAP` resource type.
pub const RT_BITMAP: u16 = 2;
/// The `RT_ICON` resource type.
pub const RT_ICON: u16 = 3;
/// The `RT_CURSOR` resource type.
pub const RT_CURSOR: u16 = 1;

/// How a resource or a resource type is named: by number, or by a string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Name {
    /// An integer id — the high bit of the stored word, cleared.
    Id(u16),
    /// A name, from the string table at the end of the resource table.
    Text(String),
}

impl Name {
    /// The integer id, if it has one.
    #[must_use]
    pub fn id(&self) -> Option<u16> {
        match self {
            Name::Id(id) => Some(*id),
            Name::Text(_) => None,
        }
    }

    /// Whether this names the same resource as `other`, comparing text
    /// case-insensitively as `LoadBitmap` does.
    #[must_use]
    pub fn matches(&self, other: &Name) -> bool {
        match (self, other) {
            (Name::Id(a), Name::Id(b)) => a == b,
            (Name::Text(a), Name::Text(b)) => a.eq_ignore_ascii_case(b),
            _ => false,
        }
    }
}

/// One entry of the resource table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    /// The resource type: `RT_BITMAP` and friends, or a name.
    pub kind: Name,
    /// What this resource is called.
    pub name: Name,
    /// Where its bytes start in the file.
    pub offset: usize,
    /// How many bytes are reserved for it. This is rounded **up** to the
    /// table's alignment unit, so it is usually a little longer than the
    /// resource itself.
    pub len: usize,
}

impl Resource {
    /// The resource's bytes.
    #[must_use]
    pub fn data<'a>(&self, exe: &'a [u8]) -> Option<&'a [u8]> {
        exe.get(self.offset..self.offset.saturating_add(self.len))
    }
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([
        *data.get(at)?,
        *data.get(at.checked_add(1)?)?,
    ]))
}

/// Read a length-prefixed name out of the resource table's string area.
fn pascal_string(exe: &[u8], at: usize) -> Option<String> {
    let len = usize::from(*exe.get(at)?);
    let bytes = exe.get(at + 1..at + 1 + len)?;
    Some(bytes.iter().map(|b| char::from(*b)).collect())
}

/// Walk the resource table of an NE executable.
///
/// Returns every resource in table order. The layout is the documented one:
/// the DOS stub's `e_lfanew` at `0x3c` points at the NE header, whose word at
/// `+0x24` is the resource table's offset from the NE header. The table starts
/// with an alignment shift, then a run of type records each followed by its
/// own entries, ending with a type of zero.
///
/// # Errors
///
/// [`FormatError::Malformed`] if the file is not an NE executable, or its
/// resource table runs off the end.
pub fn resources(exe: &[u8]) -> Result<Vec<Resource>> {
    let bad = |what: &str| FormatError::Malformed(format!("not a readable NE executable: {what}"));

    if exe.len() < 0x40 || exe.get(..2) != Some(b"MZ") {
        return Err(bad("no DOS header"));
    }
    let ne = u32::from_le_bytes([exe[0x3c], exe[0x3d], exe[0x3e], exe[0x3f]]) as usize;
    if exe.get(ne..ne + 2) != Some(b"NE") {
        return Err(bad("no NE header"));
    }
    let table = ne + usize::from(u16_at(exe, ne + 0x24).ok_or_else(|| bad("truncated header"))?);
    // A zero offset, or one equal to the resident-name table's, means the file
    // has no resources at all rather than a broken table.
    let names = ne + usize::from(u16_at(exe, ne + 0x26).ok_or_else(|| bad("truncated header"))?);
    if table >= exe.len() || table == names {
        return Ok(Vec::new());
    }

    let shift = u32::from(u16_at(exe, table).ok_or_else(|| bad("truncated resource table"))?);
    if shift > 16 {
        return Err(bad("implausible resource alignment"));
    }
    // Both an offset and a length are stored in units of `1 << shift` bytes.
    let unit = |value: u16| (usize::try_from(u32::from(value) << shift)).unwrap_or(0);

    let read_name = |word: u16| -> Option<Name> {
        if word & 0x8000 != 0 {
            Some(Name::Id(word & 0x7fff))
        } else {
            pascal_string(exe, table + usize::from(word)).map(Name::Text)
        }
    };

    let mut out = Vec::new();
    let mut at = table + 2;
    loop {
        let kind_word = u16_at(exe, at).ok_or_else(|| bad("truncated type record"))?;
        if kind_word == 0 {
            break;
        }
        let count = usize::from(u16_at(exe, at + 2).ok_or_else(|| bad("truncated type record"))?);
        let kind = read_name(kind_word).ok_or_else(|| bad("bad type name"))?;
        at += 8;
        for _ in 0..count {
            let offset = unit(u16_at(exe, at).ok_or_else(|| bad("truncated entry"))?);
            let len = unit(u16_at(exe, at + 2).ok_or_else(|| bad("truncated entry"))?);
            let name_word = u16_at(exe, at + 6).ok_or_else(|| bad("truncated entry"))?;
            let name = read_name(name_word).ok_or_else(|| bad("bad resource name"))?;
            at += 12;
            if offset < exe.len() {
                out.push(Resource {
                    kind: kind.clone(),
                    name,
                    offset,
                    len: len.min(exe.len() - offset),
                });
            }
        }
    }
    Ok(out)
}

/// Find one resource by type and name.
#[must_use]
pub fn find(exe: &[u8], kind: u16, name: &Name) -> Option<Resource> {
    resources(exe)
        .ok()?
        .into_iter()
        .find(|r| r.kind == Name::Id(kind) && r.name.matches(name))
}

/// A decoded picture: straight RGBA, top row first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Four bytes a pixel, row by row from the top. Every pixel is opaque:
    /// these bitmaps carry no alpha, and the game blits them with `SRCCOPY`.
    pub pixels: Vec<u8>,
}

impl Image {
    /// One pixel, as `(r, g, b, a)`.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = ((y * self.width + x) * 4) as usize;
        let p = self.pixels.get(at..at + 4)?;
        Some((p[0], p[1], p[2], p[3]))
    }

    /// Cut a rectangle out, as its own image.
    ///
    /// Returns `None` when the rectangle is not wholly inside — which is how a
    /// sprite index off the end of a sheet is caught.
    #[must_use]
    pub fn crop(&self, x: u32, y: u32, width: u32, height: u32) -> Option<Image> {
        if x.checked_add(width)? > self.width || y.checked_add(height)? > self.height {
            return None;
        }
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let start = (((y + row) * self.width + x) * 4) as usize;
            pixels.extend_from_slice(self.pixels.get(start..start + (width * 4) as usize)?);
        }
        Some(Image {
            width,
            height,
            pixels,
        })
    }
}

/// Decode a device-independent bitmap — the payload of an `RT_BITMAP`.
///
/// Handles the 1, 4 and 8 bits-a-pixel palette formats, which is all the game
/// uses. A bitmap stored bottom-up, as these are, comes back the right way up.
///
/// # Errors
///
/// [`FormatError::Malformed`] for a header this does not understand, a
/// compressed bitmap, or one whose pixels run off the end of the resource.
pub fn read_dib(data: &[u8]) -> Result<Image> {
    read_dib_recoloured(data, &[])
}

/// A bitmap resource by name, with some of its colour-table entries
/// replaced — see [`read_dib_recoloured`].
///
/// # Errors
/// As [`read_bitmap`].
pub fn read_bitmap_recoloured(
    exe: &[u8],
    name: &Name,
    recolour: &[(usize, [u8; 3])],
) -> Result<Image> {
    let resource = find(exe, RT_BITMAP, name).ok_or_else(|| {
        FormatError::Malformed(format!("the executable has no bitmap named {name:?}"))
    })?;
    let data = resource
        .data(exe)
        .ok_or_else(|| FormatError::Malformed("the bitmap runs off the end".to_string()))?;
    read_dib_recoloured(data, recolour)
}

/// [`read_dib`], with the colour-table entries named in `recolour` — an
/// index and the red, green and blue to put there — replaced before the
/// pixels are looked up.
///
/// That is what the game does to two of its own sheets: `FGetSystemColors`
/// (`1018:08d2`) writes `COLOR_BTNFACE` into entry 253 of the toolbar's
/// palette and entry 249 of the plaque's, so the magenta the artist keyed
/// the toolbar's background with comes out as the button face. See
/// [`art::SYSTEM_COLOURED`].
///
/// # Errors
/// As [`read_dib`].
pub fn read_dib_recoloured(data: &[u8], recolour: &[(usize, [u8; 3])]) -> Result<Image> {
    let bad = |what: &str| FormatError::Malformed(format!("not a readable bitmap: {what}"));
    let u32_at = |at: usize| -> Result<u32> {
        let b = data
            .get(at..at + 4)
            .ok_or_else(|| bad("truncated header"))?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };

    let header = u32_at(0)? as usize;
    if header < 40 {
        return Err(bad("not a BITMAPINFOHEADER"));
    }
    let width = i32::from_le_bytes(u32_at(4)?.to_le_bytes());
    let height = i32::from_le_bytes(u32_at(8)?.to_le_bytes());
    let bpp = u32::from(u16_at(data, 14).ok_or_else(|| bad("truncated header"))?);
    let compression = u32_at(16)?;
    let used = u32_at(32)?;

    if compression != 0 {
        return Err(bad("compressed"));
    }
    if !matches!(bpp, 1 | 4 | 8) {
        return Err(bad(&format!("{bpp} bits a pixel")));
    }
    let width = u32::try_from(width).map_err(|_| bad("negative width"))?;
    // A negative height means the rows are stored top-down instead.
    let top_down = height < 0;
    let height = height.unsigned_abs();
    if width == 0 || height == 0 {
        return Err(bad("empty"));
    }

    // The palette follows the header: `used` entries, or the full 1 << bpp
    // when it says zero. Each is blue, green, red, and a byte that is not
    // alpha and is ignored.
    let colours = if used == 0 { 1u32 << bpp } else { used } as usize;
    let palette_at = header;
    let mut palette = Vec::with_capacity(colours);
    for index in 0..colours {
        let at = palette_at + index * 4;
        let entry = data
            .get(at..at + 4)
            .ok_or_else(|| bad("truncated palette"))?;
        palette.push([entry[2], entry[1], entry[0], 0xff]);
    }
    for &(index, [r, g, b]) in recolour {
        if let Some(entry) = palette.get_mut(index) {
            *entry = [r, g, b, 0xff];
        }
    }

    // Rows are padded out to four bytes.
    let stride = ((width * bpp) as usize).div_ceil(32) * 4;
    let bits_at = palette_at + colours * 4;
    let needed = stride
        .checked_mul(height as usize)
        .ok_or_else(|| bad("implausible size"))?;
    let bits = data
        .get(bits_at..bits_at + needed)
        .ok_or_else(|| bad("truncated pixels"))?;

    let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
    for row in 0..height as usize {
        // Bottom-up storage is the normal case: the first row in the file is
        // the bottom row of the picture.
        let source = if top_down {
            row
        } else {
            height as usize - 1 - row
        };
        let line = &bits[source * stride..source * stride + stride];
        for column in 0..width as usize {
            let index = match bpp {
                8 => usize::from(line[column]),
                4 => {
                    let byte = line[column / 2];
                    usize::from(if column % 2 == 0 {
                        byte >> 4
                    } else {
                        byte & 0x0f
                    })
                }
                _ => {
                    let byte = line[column / 8];
                    usize::from((byte >> (7 - (column % 8))) & 1)
                }
            };
            let colour = palette.get(index).copied().unwrap_or([0, 0, 0, 0xff]);
            let at = (row * width as usize + column) * 4;
            pixels[at..at + 4].copy_from_slice(&colour);
        }
    }

    Ok(Image {
        width,
        height,
        pixels,
    })
}

/// The resource type of an icon group (`RT_GROUP_ICON`).
pub const RT_GROUP_ICON: u16 = 14;

/// Read one icon by the name of its group and decode its first image, with
/// the icon's AND mask turned into alpha.
///
/// `LoadIcon` takes the group's name — `BANG1ICO`, `TORP1ICO` and the like —
/// and the group (`RT_GROUP_ICON`, 14) lists its images: six bytes of
/// header, then fourteen per entry, the entry's `RT_ICON` id in its last
/// word. The image is a `BITMAPINFOHEADER` DIB whose height is **doubled**:
/// the colour bits (the XOR mask) come first, then a one-bit AND mask of the
/// same size, set where the screen shows through.
///
/// # Errors
/// [`FormatError::Malformed`] when the group or its image is missing or does
/// not read.
pub fn read_icon(exe: &[u8], group: &Name) -> Result<Image> {
    let bad = |what: &str| FormatError::Malformed(format!("not a readable icon: {what}"));
    let resource = find(exe, RT_GROUP_ICON, group)
        .ok_or_else(|| FormatError::Malformed(format!("no icon group named {group:?}")))?;
    let data = resource
        .data(exe)
        .ok_or_else(|| bad("the group runs off the end"))?;
    let id = u16_at(data, 6 + 12).ok_or_else(|| bad("truncated group"))?;
    let image = find(exe, RT_ICON, &Name::Id(id))
        .ok_or_else(|| bad("the group names an image that is not there"))?;
    let data = image
        .data(exe)
        .ok_or_else(|| bad("the image runs off the end"))?;

    // The header says twice the height; read the colour half as a bitmap of
    // its own by patching the height, then the mask after it.
    let header = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let doubled = i32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let mut halved = data.to_vec();
    halved[8..12].copy_from_slice(&(doubled / 2).to_le_bytes());
    let mut picture = read_dib_recoloured(&halved, &[])?;
    let bpp = u32::from(u16_at(data, 14).ok_or_else(|| bad("truncated header"))?);
    let used = u32::from_le_bytes([data[32], data[33], data[34], data[35]]);
    let colours = if used == 0 { 1u32 << bpp } else { used } as usize;
    let xor_stride = ((picture.width * bpp) as usize).div_ceil(32) * 4;
    let mask_at = header + colours * 4 + xor_stride * picture.height as usize;
    let mask_stride = (picture.width as usize).div_ceil(32) * 4;
    for row in 0..picture.height as usize {
        // Bottom-up, like the colour bits.
        let source = picture.height as usize - 1 - row;
        let Some(line) =
            data.get(mask_at + source * mask_stride..mask_at + (source + 1) * mask_stride)
        else {
            break;
        };
        for column in 0..picture.width as usize {
            let bit = (line[column / 8] >> (7 - column % 8)) & 1;
            if bit == 1 {
                let at = (row * picture.width as usize + column) * 4;
                picture.pixels[at..at + 4].copy_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Ok(picture)
}

/// Read one bitmap resource by name and decode it.
///
/// # Errors
///
/// [`FormatError::Malformed`] if there is no such bitmap, or it will not
/// decode.
pub fn read_bitmap(exe: &[u8], name: &Name) -> Result<Image> {
    let resource = find(exe, RT_BITMAP, name).ok_or_else(|| {
        FormatError::Malformed(format!("the executable has no bitmap named {name:?}"))
    })?;
    let data = resource
        .data(exe)
        .ok_or_else(|| FormatError::Malformed("the bitmap runs off the end".to_string()))?;
    read_dib(data)
}

pub mod art;
pub mod text;

/// Where a segment of an NE executable starts in the file.
///
/// `selector` is the value a far pointer carries — `0x1000` for the first
/// segment, `0x1008` for the second and so on, eight apart, which is how the
/// loader hands them out and how every address in this project's Ghidra notes
/// is written. The segment table's own entries hold the offset in units of
/// `1 << shift` bytes, with the shift in the NE header at `+0x32`.
///
/// Returns `None` when the file is not an NE executable, when it has no such
/// segment, or when the segment would run off the end.
#[must_use]
pub fn segment_offset(exe: &[u8], selector: u16) -> Option<(usize, usize)> {
    if exe.len() < 0x40 || exe.get(..2) != Some(b"MZ") {
        return None;
    }
    let ne = u32::from_le_bytes([exe[0x3c], exe[0x3d], exe[0x3e], exe[0x3f]]) as usize;
    if exe.get(ne..ne + 2) != Some(b"NE") {
        return None;
    }
    let count = usize::from(u16_at(exe, ne + 0x1c)?);
    let table = ne + usize::from(u16_at(exe, ne + 0x22)?);
    let shift = u32::from(u16_at(exe, ne + 0x32)?);
    if shift > 16 {
        return None;
    }
    let index = usize::from(selector.checked_sub(0x1000)? / 8);
    if index >= count {
        return None;
    }
    let entry = table + index * 8;
    let at = usize::try_from(u32::from(u16_at(exe, entry)?) << shift).ok()?;
    let len = usize::from(u16_at(exe, entry + 2)?);
    // A zero length means 64K, which is how the format says "the whole thing".
    let len = if len == 0 { 0x1_0000 } else { len };
    (at.checked_add(len)? <= exe.len()).then_some((at, len))
}
