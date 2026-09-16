//! The game's string table, read out of its executable.
//!
//! `PszGetCompressedString` (`1010:0f5c`) keeps every message, caption and
//! credit the game shows in one nibble-coded table inside **segment
//! `0x1010`** — the same coding the tutorial's text uses
//! (`crate::tutorial`), with its own three tables:
//!
//! | table | offset | what |
//! |-------|--------|------|
//! | lengths | `0x6e32` | one byte per string, `0x40` per block of 64 |
//! | blocks | `0x73b8` | one word per block, where its nibbles start |
//! | characters | `0x73e6` | the alphabet the running sums index |
//!
//! A string is a run of nibbles: each adds to a running sum, and a nibble
//! that is not `0xf` then takes the character at that sum and clears it —
//! so `0xf` is an escape that reaches past the sixteen characters a nibble
//! could name on its own. Nothing here is copied into the repository: it
//! is decoded at run time from the player's own copy, as the artwork is.

use crate::resources::segment_offset;

/// The segment the string table lives in.
const SEGMENT: u16 = 0x1010;
/// Per-string nibble counts, `0x40` to a block.
const LENGTHS: usize = 0x6e32;
/// Where each block of sixty-four strings starts, as a word each.
const BLOCKS: usize = 0x73b8;
/// The character table the running nibble sums index into.
const CHARACTERS: usize = 0x73e6;

/// The About box's credits: `idsDesignProgramming` and the seventy-six
/// lines after it, which `About` (`1018:1252`) scrolls past —
/// `0x277`..=`0x2c3`.
pub const CREDITS: std::ops::RangeInclusive<u16> = 0x277..=0x2c3;

/// `idsVersionD02dC`: the About box's version line, `Version %d.%02d%c`,
/// which `SzVersion` (`1018:1212`) fills with [`VERSION`].
pub const VERSION_FORMAT: u16 = 0x22d;

/// What `SzVersion` prints: major 2, minor 60, letter `j` — *Version
/// 2.60j*.
pub const VERSION: (u16, u16, char) = (2, 0x3c, 'j');

/// Read one string of the table by its id.
///
/// Returns `None` when the file is not the game or has no such string.
#[must_use]
pub fn string(exe: &[u8], ids: u16) -> Option<String> {
    let (base, len) = segment_offset(exe, SEGMENT)?;
    let seg = exe.get(base..base + len)?;
    nibble_string(seg, LENGTHS, BLOCKS, CHARACTERS, usize::from(ids))
}

/// The version line as the About box shows it.
#[must_use]
pub fn version(exe: &[u8]) -> Option<String> {
    let format = string(exe, VERSION_FORMAT)?;
    let (major, minor, letter) = VERSION;
    // `wsprintf` with `%d`, `%02d` and `%c`, in that order.
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    let mut args = 0;
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut spec = String::new();
        while let Some(&n) = chars.peek() {
            chars.next();
            spec.push(n);
            if n.is_ascii_alphabetic() {
                break;
            }
        }
        match (args, spec.as_str()) {
            (0, "d") => out.push_str(&major.to_string()),
            (1, "02d") => out.push_str(&format!("{minor:02}")),
            (2, "c") => out.push(letter),
            _ => out.push_str(&spec),
        }
        args += 1;
    }
    Some(out)
}

/// The credits, one line each, blanks included — what the About box
/// scrolls.
#[must_use]
pub fn credits(exe: &[u8]) -> Option<Vec<String>> {
    CREDITS.map(|ids| string(exe, ids)).collect()
}

/// Decode one entry of a nibble-coded string table.
///
/// `lengths` holds one byte per string in blocks of sixty-four, `blocks`
/// one word per block saying where its nibbles start in the segment, and
/// `characters` the alphabet the running sums index.
#[must_use]
pub(crate) fn nibble_string(
    seg: &[u8],
    lengths: usize,
    blocks: usize,
    characters: usize,
    index: usize,
) -> Option<String> {
    let (block, index) = (index / 64, index % 64);

    // Where this string's nibbles start, counted in nibbles from the
    // block's own start.
    let lengths = lengths + block * 0x40;
    let start: usize = (0..index)
        .map(|i| seg.get(lengths + i).copied().map_or(0, usize::from))
        .sum();
    let mut count = usize::from(*seg.get(lengths + index)?);

    let word = seg.get(blocks + block * 2..blocks + block * 2 + 2)?;
    let mut at = usize::from(u16::from_le_bytes([word[0], word[1]])) + start / 2;
    let mut high = start.is_multiple_of(2);

    let mut out = String::new();
    let mut sum = 0usize;
    while count > 0 {
        let byte = *seg.get(at)?;
        let nibble = if high {
            usize::from(byte >> 4)
        } else {
            at += 1;
            usize::from(byte & 0x0f)
        };
        high = !high;
        count -= 1;
        sum += nibble;
        if nibble != 0x0f {
            out.push(char::from(*seg.get(characters + sum)?));
            sum = 0;
        }
    }
    Some(out)
}
