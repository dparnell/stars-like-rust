//! The tutorial's own text, read out of the game's executable.
//!
//! The tutorial is eighty pages of eight paragraphs, and every word of it is
//! the game's own writing. None of it is copied into this repository: it is
//! read at run time from the player's copy of `stars.exe`, exactly as the
//! artwork is. Without a copy of the game there is no tutorial text, and the
//! frontend says so rather than inventing any.
//!
//! `CchTutorString` (`1100:5a14`) decodes it, and the scheme is the same
//! nibble coding the main string table uses — a per-string nibble count, a
//! per-block start pointer, and a character table the running nibble sums
//! index into — with its own three tables, all inside **segment `0x1100`**:
//!
//! | table | offset | what |
//! |-------|--------|------|
//! | lengths | `0x5734` | one byte per paragraph, `0x40` per block |
//! | blocks | `0x59b4` | one word per block of 64, where its nibbles start |
//! | characters | `0x59c8` | the alphabet the running sums index |
//!
//! A nibble of `0xf` is an escape: it adds fifteen to the running sum and
//! takes no character, which is how the coding reaches past the sixteen
//! characters a single nibble could name.

use crate::resources::segment_offset;

/// The segment the tutorial's text lives in.
const SEGMENT: u16 = 0x1100;
/// Per-paragraph nibble counts, `0x40` to a block.
const LENGTHS: usize = 0x5734;
/// Where each block of sixty-four paragraphs starts, as a word each.
const BLOCKS: usize = 0x59b4;
/// The character table the running nibble sums index into.
const CHARACTERS: usize = 0x59c8;

/// How many paragraphs make up one page.
///
/// `AdvanceTutor` (`10f8:0a30`) steps `tutor.idt` by eight, so a page is eight
/// consecutive paragraphs and the page number is `idt / 8 + 1`.
pub const PARAGRAPHS_PER_PAGE: usize = 8;

/// How many pages the tutorial has.
///
/// `AdvanceTutor` ends it once `idt` passes `0x27f`, which is the last
/// paragraph of the eightieth page.
pub const PAGES: usize = 80;

/// Every paragraph there is.
pub const PARAGRAPHS: usize = PAGES * PARAGRAPHS_PER_PAGE;

/// Read one paragraph of the tutorial out of an executable.
///
/// `idt` is the game's own paragraph number, `0` to `639`. Returns `None` when
/// the file is not the game, or has no tutorial text in it, or the number is
/// out of range.
#[must_use]
pub fn paragraph(exe: &[u8], idt: usize) -> Option<String> {
    if idt >= PARAGRAPHS {
        return None;
    }
    let (base, len) = segment_offset(exe, SEGMENT)?;
    let seg = exe.get(base..base + len)?;
    crate::resources::text::nibble_string(seg, LENGTHS, BLOCKS, CHARACTERS, idt)
}

/// Read a whole page: its eight paragraphs, in order.
///
/// `page` is one-based, as the tutorial's own title bar counts them. Trailing
/// blank paragraphs are kept, because which paragraph a step emboldens is an
/// index into all eight.
#[must_use]
pub fn page(exe: &[u8], page: usize) -> Option<Vec<String>> {
    if page == 0 || page > PAGES {
        return None;
    }
    let first = (page - 1) * PARAGRAPHS_PER_PAGE;
    (first..first + PARAGRAPHS_PER_PAGE)
        .map(|idt| paragraph(exe, idt))
        .collect()
}

/// Whether an executable carries the tutorial's text at all.
#[must_use]
pub fn present(exe: &[u8]) -> bool {
    paragraph(exe, 0).is_some_and(|first| !first.trim().is_empty())
}
