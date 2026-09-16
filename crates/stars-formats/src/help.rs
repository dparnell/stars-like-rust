//! The player's guide — `STARS!.HLP`, a Windows 3.1 help file.
//!
//! Every dialog's Help button hands a **context number** to `WinHelp`
//! (`WINHELP(hwnd, szHelpFile, HELP_CONTEXT, id)`, the import at
//! `14f8:029c`), and the Help menu's Player's Guide asks for the contents
//! page (`HELP_INDEX`, `CommandHandler` `1020:47c9`). The viewer that answers
//! is Windows' own, so nothing about the file is in the game's code: this
//! module reads the file the way `WINHELP.EXE` does, far enough to find a
//! topic by its number and lay its text out.
//!
//! The format is Microsoft's, undocumented by them and pieced together by
//! Manfred Winterhoff's `helpdeco`; the offsets below are from its
//! `helpfile.txt`, checked against this file (`binary/STARS!.HLP`: format
//! 1.21, uncompressed, 4096-byte topic blocks). See `docs/formats/help.md`.
//!
//! A help file is a little file system:
//!
//! | offset | size | field |
//! |--------|------|-------|
//! | 0 | 4 | magic `0x0003_5f3f` |
//! | 4 | 4 | where the directory's file header is |
//! | 8 | 4 | the first free block (unused here) |
//! | 12 | 4 | the file's whole size |
//!
//! Each internal file starts with a nine-byte header — reserved size, used
//! size, a flag byte — and the directory is a B+ tree of name → offset. The
//! files that matter: `|SYSTEM` (the title and the window), `|FONT`,
//! `|CTXOMAP` (context number → topic), `|CONTEXT` (context-name hash →
//! topic, which is what the hotspots jump by), `|TTLBTREE` (topic → title),
//! `|KWBTREE`/`|KWDATA` (the Search dialog's keywords), `|TOPIC` (the text)
//! and `|bm0`…`|bm133` (the pictures).
//!
//! A **topic offset** — the number every table above uses — is a block
//! number in its high seventeen bits and a character count in its low
//! fifteen: the characters of every text record in the block before the
//! one wanted, counting each record's `TopicLength` (which is its text plus
//! one for each hotspot of a picture it carries). A **topic position**, which
//! the records link to one another by, is a block number in the high bits
//! over fourteen and a byte offset within the block below.

use crate::resources::{read_dib, Image};
use crate::{FormatError, Result};

/// The file's magic number.
pub const HELP_MAGIC: u32 = 0x0003_5f3f;

/// The `|SYSTEM` header's magic.
const SYSTEM_MAGIC: u16 = 0x036c;
/// A B+ tree header's magic.
const BTREE_MAGIC: u16 = 0x293b;
/// The size of a topic block in a Windows 3.1 file.
const TOPIC_BLOCK: usize = 4096;
/// The block header that each topic block starts with.
const TOPIC_BLOCK_HEADER: usize = 12;
/// The bytes of one block that carry text.
const TOPIC_BLOCK_PAYLOAD: usize = TOPIC_BLOCK - TOPIC_BLOCK_HEADER;
/// A topic link's fixed header.
const LINK_HEADER: usize = 21;

/// The record types of a topic link.
const RECORD_HEADER: u8 = 2;
const RECORD_TEXT: u8 = 0x20;
const RECORD_TABLE: u8 = 0x23;

fn bad(what: impl Into<String>) -> FormatError {
    FormatError::Malformed(format!("help file: {}", what.into()))
}

/// A bounds-checked cursor over the file.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], at: usize) -> Self {
        Cursor { bytes, at }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let slice = self
            .bytes
            .get(self.at..self.at.checked_add(n).ok_or_else(|| bad("overflow"))?)
            .ok_or(FormatError::UnexpectedEof {
                offset: self.at,
                needed: n,
            })?;
        self.at += n;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }

    fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }

    /// A NUL-terminated string, decoded from Windows-1252.
    fn stringz(&mut self) -> Result<String> {
        let rest = self.bytes.get(self.at..).unwrap_or(&[]);
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| bad("unterminated string"))?;
        let s = decode_1252(&rest[..len]);
        self.at += len + 1;
        Ok(s)
    }

    /// A compressed unsigned short: one byte when even (halved), else the
    /// byte halved plus 128 times the next.
    fn cus(&mut self) -> Result<u16> {
        let v = u16::from(self.u8()?);
        if v & 1 == 0 {
            Ok(v >> 1)
        } else {
            Ok((v >> 1) + 128 * u16::from(self.u8()?))
        }
    }

    /// A compressed signed short: as [`Self::cus`], biased by 64 or 16384.
    fn css(&mut self) -> Result<i16> {
        let v = i16::from(self.u8()?);
        if v & 1 == 0 {
            Ok((v >> 1) - 64)
        } else {
            Ok((v >> 1) + 128 * i16::from(self.u8()?) - 16384)
        }
    }

    /// A compressed unsigned long: a word when even (halved), else the word
    /// halved plus 32768 times the next word.
    fn cul(&mut self) -> Result<u32> {
        let v = u32::from(self.u16()?);
        if v & 1 == 0 {
            Ok(v >> 1)
        } else {
            Ok((v >> 1) + 32768 * u32::from(self.u16()?))
        }
    }

    /// A compressed signed long: as [`Self::cul`], biased by 16384 or
    /// 67108864.
    fn csl(&mut self) -> Result<i32> {
        let v = i64::from(self.u16()?);
        let value = if v & 1 == 0 {
            (v >> 1) - 16384
        } else {
            (v >> 1) + 32768 * i64::from(self.u16()?) - 67_108_864
        };
        i32::try_from(value).map_err(|_| bad("compressed long"))
    }
}

/// Windows-1252 to text: Latin-1 with the printable characters Microsoft
/// put in the `0x80`–`0x9f` gap.
fn decode_1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0x80 => '€',
            0x82 => '‚',
            0x83 => 'ƒ',
            0x84 => '„',
            0x85 => '…',
            0x86 => '†',
            0x87 => '‡',
            0x88 => 'ˆ',
            0x89 => '‰',
            0x8a => 'Š',
            0x8b => '‹',
            0x8c => 'Œ',
            0x8e => 'Ž',
            0x91 => '‘',
            0x92 => '’',
            0x93 => '“',
            0x94 => '”',
            0x95 => '•',
            0x96 => '–',
            0x97 => '—',
            0x98 => '˜',
            0x99 => '™',
            0x9a => 'š',
            0x9b => '›',
            0x9c => 'œ',
            0x9e => 'ž',
            0x9f => 'Ÿ',
            b => char::from(b),
        })
        .collect()
}

/// The internal file header: reserved size, used size, a flag byte.
///
/// Returns the data's start and its used length.
fn file_data(bytes: &[u8], at: usize) -> Result<(usize, usize)> {
    let mut c = Cursor::new(bytes, at);
    let _reserved = c.u32()?;
    let used = c.u32()? as usize;
    let _flags = c.u8()?;
    let start = c.at;
    if bytes.len() < start + used {
        return Err(FormatError::UnexpectedEof {
            offset: start,
            needed: used,
        });
    }
    Ok((start, used))
}

/// The leaf pages of a B+ tree, as `(first entry, entry count)` pairs in
/// key order.
///
/// The header is thirty-eight bytes — magic, flags, page size, a sixteen-byte
/// structure string, zero, splits, root page, minus one, page count, level
/// count, entry count — and the pages follow it. An index page (eight bytes
/// of header: unused, count, previous) is descended by its `PreviousPage`
/// to the leftmost leaf, and the leaves (unused, count, previous, next)
/// chain by `NextPage`.
fn btree_leaves(bytes: &[u8], at: usize) -> Result<Vec<(usize, usize)>> {
    let (start, _used) = file_data(bytes, at)?;
    let mut c = Cursor::new(bytes, start);
    let magic = c.u16()?;
    if magic != BTREE_MAGIC {
        return Err(FormatError::BadMagic {
            expected: u32::from(BTREE_MAGIC),
            found: u32::from(magic),
        });
    }
    let _flags = c.u16()?;
    let page_size = usize::from(c.u16()?);
    let _structure = c.take(16)?;
    let _zero = c.i16()?;
    let _splits = c.i16()?;
    let root = c.i16()?;
    let _minus_one = c.i16()?;
    let total_pages = c.i16()?;
    let levels = c.i16()?;
    let _entries = c.i32()?;
    let pages = c.at;
    if page_size == 0 {
        return Err(bad("empty btree page"));
    }
    let page_at = |page: i16| -> Result<usize> {
        if page < 0 || page >= total_pages {
            return Err(bad(format!("btree page {page} of {total_pages}")));
        }
        Ok(pages + page as usize * page_size)
    };
    // Down the left edge to the first leaf.
    let mut page = root;
    for _ in 1..levels {
        let mut c = Cursor::new(bytes, page_at(page)?);
        let _unused = c.u16()?;
        let _count = c.i16()?;
        page = c.i16()?;
    }
    // Then along the leaves.
    let mut leaves = Vec::new();
    let mut seen = 0;
    while page >= 0 {
        let mut c = Cursor::new(bytes, page_at(page)?);
        let _unused = c.u16()?;
        let count = c.i16()?;
        let _previous = c.i16()?;
        let next = c.i16()?;
        leaves.push((c.at, usize::try_from(count).unwrap_or(0)));
        page = next;
        seen += 1;
        if seen > total_pages {
            return Err(bad("btree leaves loop"));
        }
    }
    Ok(leaves)
}

/// One face the file's fonts use, from `|FONT`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    /// Bold, italic, underlined, struck through, doubly underlined, small
    /// capitals — the six attribute bits, in that order.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underlined.
    pub underline: bool,
    /// Struck through.
    pub strike_out: bool,
    /// The size, in half points.
    pub half_points: u8,
    /// The face's name — `Helv`, `Arial`, `Symbol`.
    pub face: String,
    /// The text colour.
    pub colour: [u8; 3],
}

impl Font {
    /// The size in points.
    #[must_use]
    pub fn points(&self) -> f32 {
        f32::from(self.half_points) / 2.0
    }
}

/// The main window, from `|SYSTEM`'s window record.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Window {
    /// Where it opens, in thousandths of the screen.
    pub x: i16,
    /// Where it opens, in thousandths of the screen.
    pub y: i16,
    /// How wide, in thousandths of the screen.
    pub width: i16,
    /// How tall, in thousandths of the screen.
    pub height: i16,
    /// Whether it opens maximised.
    pub maximised: bool,
    /// The scrolling region's background, when the file sets one.
    pub background: Option<[u8; 3]>,
    /// The non-scrolling region's background, when the file sets one.
    pub band_background: Option<[u8; 3]>,
}

/// One keyword of the Search dialog and the topics it is filed under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword {
    /// The word.
    pub word: String,
    /// The topics, as topic offsets.
    pub topics: Vec<i32>,
}

/// The help file, read far enough to find and lay out any topic.
#[derive(Debug, Clone)]
pub struct HelpFile {
    bytes: Vec<u8>,
    /// The internal directory: name and file-header offset.
    files: Vec<(String, usize)>,
    /// The file's title (`|SYSTEM` record 1).
    title: String,
    /// The contents topic (`|SYSTEM` record 3).
    contents: i32,
    /// The main window, when the file describes one.
    window: Option<Window>,
    fonts: Vec<Font>,
    /// `|CTXOMAP`: context number → topic offset, sorted by number.
    context_map: Vec<(u32, i32)>,
    /// `|CONTEXT`: context-name hash → topic offset, sorted by hash.
    hashes: Vec<(u32, i32)>,
    /// `|TTLBTREE`: topic offset → title, sorted by offset.
    titles: Vec<(i32, String)>,
    /// `|KWBTREE` with `|KWDATA`: the keywords in order.
    keywords: Vec<Keyword>,
    /// `|TOPIC`'s data: start and used length.
    topic: (usize, usize),
}

impl HelpFile {
    /// Read a help file.
    ///
    /// # Errors
    ///
    /// [`FormatError::BadMagic`] for a file that is not a help file, and
    /// [`FormatError::Malformed`] or [`FormatError::UnexpectedEof`] for one
    /// this reader cannot follow — a compressed one, say, which this file
    /// is not.
    pub fn read(bytes: Vec<u8>) -> Result<HelpFile> {
        let mut c = Cursor::new(&bytes, 0);
        let magic = c.u32()?;
        if magic != HELP_MAGIC {
            return Err(FormatError::BadMagic {
                expected: HELP_MAGIC,
                found: magic,
            });
        }
        let directory = usize::try_from(c.i32()?).map_err(|_| bad("directory offset"))?;
        let _free = c.i32()?;
        let _size = c.u32()?;

        let mut files = Vec::new();
        for (start, count) in btree_leaves(&bytes, directory)? {
            let mut c = Cursor::new(&bytes, start);
            for _ in 0..count {
                let name = c.stringz()?;
                let at = usize::try_from(c.i32()?).map_err(|_| bad("file offset"))?;
                files.push((name, at));
            }
        }
        let find = |name: &str| -> Option<usize> {
            files.iter().find(|(n, _)| n == name).map(|(_, at)| *at)
        };

        // |SYSTEM: a twelve-byte header, then records of type, size, data.
        let system = find("|SYSTEM").ok_or_else(|| bad("no |SYSTEM"))?;
        let (start, used) = file_data(&bytes, system)?;
        let mut c = Cursor::new(&bytes, start);
        let magic = c.u16()?;
        if magic != SYSTEM_MAGIC {
            return Err(FormatError::BadMagic {
                expected: u32::from(SYSTEM_MAGIC),
                found: u32::from(magic),
            });
        }
        let minor = c.u16()?;
        let _major = c.u16()?;
        let _generated = c.u32()?;
        let flags = c.u16()?;
        if minor <= 16 {
            return Err(FormatError::UnsupportedVersion(u32::from(minor)));
        }
        if flags != 0 {
            // 4 and 8 are LZ77-compressed topic blocks.
            return Err(bad(format!("compressed topics (flags {flags:#x})")));
        }
        let mut title = String::new();
        let mut contents = 0;
        let mut window = None;
        while c.at < start + used {
            let kind = c.u16()?;
            let size = usize::from(c.u16()?);
            let data = c.take(size)?;
            match kind {
                1 => title = Cursor::new(data, 0).stringz().unwrap_or_default(),
                3 if size >= 4 => {
                    contents = i32::from_le_bytes([data[0], data[1], data[2], data[3]])
                }
                6 if window.is_none() && size >= 90 => {
                    window = Some(read_window(data)?);
                }
                _ => {}
            }
        }

        let fonts = match find("|FONT") {
            Some(at) => read_fonts(&bytes, at)?,
            None => Vec::new(),
        };

        let mut context_map = Vec::new();
        if let Some(at) = find("|CTXOMAP") {
            let (start, _used) = file_data(&bytes, at)?;
            let mut c = Cursor::new(&bytes, start);
            let count = c.u16()?;
            for _ in 0..count {
                let id = c.u32()?;
                let topic = c.i32()?;
                context_map.push((id, topic));
            }
            context_map.sort_unstable();
        }

        let mut hashes = Vec::new();
        if let Some(at) = find("|CONTEXT") {
            for (start, count) in btree_leaves(&bytes, at)? {
                let mut c = Cursor::new(&bytes, start);
                for _ in 0..count {
                    let hash = c.u32()?;
                    let topic = c.i32()?;
                    hashes.push((hash, topic));
                }
            }
            hashes.sort_unstable();
        }

        let mut titles = Vec::new();
        if let Some(at) = find("|TTLBTREE") {
            for (start, count) in btree_leaves(&bytes, at)? {
                let mut c = Cursor::new(&bytes, start);
                for _ in 0..count {
                    let topic = c.i32()?;
                    let title = c.stringz()?;
                    titles.push((topic, title));
                }
            }
            titles.sort_by_key(|(topic, _)| *topic);
        }

        let mut keywords = Vec::new();
        if let (Some(tree), Some(data)) = (find("|KWBTREE"), find("|KWDATA")) {
            let (data_start, data_used) = file_data(&bytes, data)?;
            for (start, count) in btree_leaves(&bytes, tree)? {
                let mut c = Cursor::new(&bytes, start);
                for _ in 0..count {
                    let word = c.stringz()?;
                    let count = usize::try_from(c.i16()?).unwrap_or(0);
                    let at = usize::try_from(c.i32()?).map_err(|_| bad("keyword data offset"))?;
                    if at + count * 4 > data_used {
                        return Err(bad("keyword data runs off |KWDATA"));
                    }
                    let mut d = Cursor::new(&bytes, data_start + at);
                    let mut topics = Vec::with_capacity(count);
                    for _ in 0..count {
                        topics.push(d.i32()?);
                    }
                    keywords.push(Keyword { word, topics });
                }
            }
        }

        let topic = find("|TOPIC").ok_or_else(|| bad("no |TOPIC"))?;
        let topic = file_data(&bytes, topic)?;

        Ok(HelpFile {
            bytes,
            files,
            title,
            contents,
            window,
            fonts,
            context_map,
            hashes,
            titles,
            keywords,
            topic,
        })
    }

    /// The file's title — the caption of the help window.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The contents topic, which the Contents button and `HELP_INDEX` open.
    #[must_use]
    pub fn contents(&self) -> i32 {
        self.contents
    }

    /// The main window, when the file describes one.
    #[must_use]
    pub fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }

    /// The fonts, indexed by the text's font numbers.
    #[must_use]
    pub fn fonts(&self) -> &[Font] {
        &self.fonts
    }

    /// One font, when the number is one the file has.
    #[must_use]
    pub fn font(&self, number: u16) -> Option<&Font> {
        self.fonts.get(usize::from(number))
    }

    /// The Search dialog's keywords, in order.
    #[must_use]
    pub fn keywords(&self) -> &[Keyword] {
        &self.keywords
    }

    /// The internal files, by name.
    pub fn file_names(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(|(name, _)| name.as_str())
    }

    /// The topic a context number names — what a dialog's Help button asks
    /// for.
    #[must_use]
    pub fn topic_for_context(&self, id: u32) -> Option<i32> {
        self.context_map
            .binary_search_by_key(&id, |(id, _)| *id)
            .ok()
            .map(|i| self.context_map[i].1)
    }

    /// The topic a context-name hash names — what a hotspot jumps by.
    #[must_use]
    pub fn topic_for_hash(&self, hash: u32) -> Option<i32> {
        self.hashes
            .binary_search_by_key(&hash, |(hash, _)| *hash)
            .ok()
            .map(|i| self.hashes[i].1)
    }

    /// Every topic that has a title, in file order.
    #[must_use]
    pub fn titled_topics(&self) -> &[(i32, String)] {
        &self.titles
    }

    /// A topic's title, when it has one.
    #[must_use]
    pub fn title_of(&self, topic: i32) -> Option<&str> {
        self.titles
            .binary_search_by_key(&topic, |(topic, _)| *topic)
            .ok()
            .map(|i| self.titles[i].1.as_str())
    }

    /// Where a topic's header record is, as a topic position.
    ///
    /// The block's links are walked from its first, adding up the text
    /// records' lengths until the wanted character count is reached. A
    /// count that lands on a header is that topic; one that lands inside a
    /// text record is the topic that record belongs to, whose header is
    /// the last one passed — or, before any, the header the block names
    /// as the last one of the block before (`LastTopicHeader`).
    fn find_topic(&self, offset: i32) -> Result<i32> {
        let block = (offset >> 15) as usize;
        let wanted = u32::from((offset & 0x7fff) as u16);
        let stream = TopicStream::new(self);
        let block_at = block
            .checked_mul(TOPIC_BLOCK)
            .filter(|at| *at < self.topic.1)
            .ok_or_else(|| bad(format!("topic {offset:#x} is past the text")))?;
        let mut c = Cursor::new(&self.bytes, self.topic.0 + block_at);
        let _last_link = c.i32()?;
        let first_link = c.i32()?;
        let mut last_header = c.i32()?;
        let mut pos = first_link;
        let mut count = 0;
        loop {
            let link = stream.link(pos)?;
            if link.record == RECORD_HEADER {
                if count == wanted {
                    return Ok(pos);
                }
                last_header = pos;
            } else if matches!(link.record, RECORD_TEXT | RECORD_TABLE) {
                let mut c = Cursor::new(&link.data1, 0);
                let _size = c.csl()?;
                let length = u32::from(c.cus()?);
                if wanted >= count && wanted < count + length && last_header > 0 {
                    return Ok(last_header);
                }
                count += length;
            }
            if count > wanted {
                break;
            }
            if link.next <= 0 || (link.next >> 14) as usize != block {
                break;
            }
            pos = link.next;
        }
        Err(bad(format!("no topic at {offset:#x}")))
    }

    /// Lay a topic out.
    ///
    /// # Errors
    ///
    /// [`FormatError::Malformed`] when no topic starts at `offset`, or its
    /// records cannot be followed.
    pub fn topic(&self, offset: i32) -> Result<Topic> {
        let stream = TopicStream::new(self);
        let header_pos = self.find_topic(offset)?;
        let header = stream.link(header_pos)?;
        let mut c = Cursor::new(&header.data1, 0);
        let _size = c.i32()?;
        let browse_back = c.i32()?;
        let browse_forward = c.i32()?;
        let _number = c.i32()?;
        let non_scroll = c.i32()?;
        let scroll = c.i32()?;
        let _next_topic = c.i32()?;
        // The title, which the file stores without its terminator.
        let title = Strings::new(&header.data2).next();

        let mut topic = Topic {
            offset,
            title,
            browse_back: (browse_back >= 0).then_some(browse_back),
            browse_forward: (browse_forward >= 0).then_some(browse_forward),
            band: Vec::new(),
            body: Vec::new(),
        };
        let mut pos = header.next;
        let mut seen = 0;
        while pos > 0 {
            let link = stream.link(pos)?;
            if link.record == RECORD_HEADER {
                break;
            }
            let block = self.read_record(&link)?;
            if let Some(block) = block {
                if non_scroll >= 0 && pos < scroll {
                    topic.band.push(block);
                } else {
                    topic.body.push(block);
                }
            }
            pos = link.next;
            seen += 1;
            if seen > 100_000 {
                return Err(bad("topic never ends"));
            }
        }
        Ok(topic)
    }

    /// One text or table record as a block of the topic.
    fn read_record(&self, link: &Link) -> Result<Option<Block>> {
        match link.record {
            RECORD_TEXT => Ok(Some(Block::Text(self.read_text(link)?))),
            RECORD_TABLE => Ok(Some(Block::Table(self.read_table(link)?))),
            _ => Ok(None),
        }
    }

    /// A text record: one paragraph style, any number of paragraphs.
    fn read_text(&self, link: &Link) -> Result<Vec<Paragraph>> {
        let mut c = Cursor::new(&link.data1, 0);
        let _size = c.csl()?;
        let _length = c.cus()?;
        let mut text = Strings::new(&link.data2);
        let style = read_style(&mut c)?;
        read_paragraphs(self, &mut c, &mut text, &style)
    }

    /// A table record: columns, then paragraphs each tagged with its column.
    fn read_table(&self, link: &Link) -> Result<Table> {
        let mut c = Cursor::new(&link.data1, 0);
        let _size = c.csl()?;
        let _length = c.cus()?;
        let column_count = usize::from(c.u8()?);
        let kind = c.u8()?;
        let mut columns = Vec::with_capacity(column_count);
        if kind == 0 || kind == 2 {
            let _min_width = c.i16()?;
        }
        for _ in 0..column_count {
            let width = c.i16()?;
            let gap = c.i16()?;
            columns.push(Column { width, gap });
        }
        let mut text = Strings::new(&link.data2);
        let mut rows: Vec<Vec<Cell>> = Vec::new();
        let mut last_column: Option<usize> = None;
        while c.at < link.data1.len() {
            let column = c.i16()?;
            if column < 0 {
                break;
            }
            let column = usize::try_from(column).unwrap_or(0);
            let _unknown = c.i16()?;
            let _always = c.u8()?;
            let style = read_style(&mut c)?;
            let paragraphs = read_paragraphs(self, &mut c, &mut text, &style)?;
            // A column no higher than the last starts a new row; a new
            // column starts a new cell; the same column adds to it.
            let new_row = rows.is_empty() || last_column.is_some_and(|last| column < last);
            if new_row {
                rows.push(Vec::new());
            }
            let row = rows.last_mut().expect("a row");
            match row.last_mut() {
                Some(cell) if cell.column == column => cell.paragraphs.extend(paragraphs),
                _ => row.push(Cell { column, paragraphs }),
            }
            last_column = Some(column);
        }
        Ok(Table { columns, rows })
    }

    /// A picture by number — `|bm<n>` — decoded, with its hotspots.
    ///
    /// # Errors
    ///
    /// [`FormatError::Malformed`] when there is no such picture or it is
    /// not a bitmap this reader decodes — a metafile, say.
    pub fn picture(&self, number: u16) -> Result<Picture> {
        let name = format!("|bm{number}");
        let at = self
            .files
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, at)| *at)
            .ok_or_else(|| bad(format!("no picture {name}")))?;
        let (start, used) = file_data(&self.bytes, at)?;
        read_picture(self, &self.bytes[start..start + used])
    }

    /// How many pictures the file holds.
    #[must_use]
    pub fn picture_count(&self) -> usize {
        self.files
            .iter()
            .filter(|(n, _)| n.starts_with("|bm"))
            .count()
    }
}

/// The window record: flags, a ten-byte type, a nine-byte name, a
/// fifty-one-byte caption, five words of geometry and two colours. Each
/// field is only meaningful when its flag bit is set.
fn read_window(data: &[u8]) -> Result<Window> {
    let mut c = Cursor::new(data, 0);
    let flags = c.u16()?;
    let _kind = c.take(10)?;
    let _name = c.take(9)?;
    let _caption = c.take(51)?;
    let x = c.i16()?;
    let y = c.i16()?;
    let width = c.i16()?;
    let height = c.i16()?;
    let maximise = c.i16()?;
    let rgb = c.u32()?;
    let rgb_band = c.u32()?;
    let colour = |v: u32| {
        [
            (v & 0xff) as u8,
            ((v >> 8) & 0xff) as u8,
            ((v >> 16) & 0xff) as u8,
        ]
    };
    Ok(Window {
        x: if flags & 0x08 != 0 { x } else { 0 },
        y: if flags & 0x10 != 0 { y } else { 0 },
        width: if flags & 0x20 != 0 { width } else { 0 },
        height: if flags & 0x40 != 0 { height } else { 0 },
        maximised: flags & 0x80 != 0 && maximise & 1 != 0,
        background: (flags & 0x100 != 0).then(|| colour(rgb)),
        band_background: (flags & 0x200 != 0).then(|| colour(rgb_band)),
    })
}

/// `|FONT`: counts and offsets of the face names and the descriptors, the
/// names as fixed-width strings, then eleven-byte descriptors — attributes,
/// half points, family, face index, foreground and background colours.
fn read_fonts(bytes: &[u8], at: usize) -> Result<Vec<Font>> {
    let (start, _used) = file_data(bytes, at)?;
    let mut c = Cursor::new(bytes, start);
    let face_count = usize::from(c.u16()?);
    let descriptor_count = usize::from(c.u16()?);
    let faces_at = usize::from(c.u16()?);
    let descriptors_at = usize::from(c.u16()?);
    if faces_at >= 12 {
        // The multimedia layouts are not this file's.
        return Err(bad("unsupported font descriptors"));
    }
    let width = descriptors_at
        .saturating_sub(faces_at)
        .checked_div(face_count)
        .unwrap_or(0);
    let mut faces = Vec::with_capacity(face_count);
    for i in 0..face_count {
        let mut c = Cursor::new(bytes, start + faces_at + i * width);
        let raw = c.take(width)?;
        let len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        faces.push(decode_1252(&raw[..len]));
    }
    let mut fonts = Vec::with_capacity(descriptor_count);
    let mut c = Cursor::new(bytes, start + descriptors_at);
    for _ in 0..descriptor_count {
        let attributes = c.u8()?;
        let half_points = c.u8()?;
        let _family = c.u8()?;
        let face = usize::from(c.u16()?);
        let fg = c.take(3)?;
        let _bg = c.take(3)?;
        fonts.push(Font {
            bold: attributes & 1 != 0,
            italic: attributes & 2 != 0,
            underline: attributes & 4 != 0,
            strike_out: attributes & 8 != 0,
            half_points,
            face: faces.get(face).cloned().unwrap_or_default(),
            colour: [fg[0], fg[1], fg[2]],
        });
    }
    Ok(fonts)
}

/// One topic link, with its two data parts gathered across block
/// boundaries.
struct Link {
    record: u8,
    next: i32,
    data1: Vec<u8>,
    data2: Vec<u8>,
}

/// `|TOPIC` as the byte stream the records live in: the blocks' payloads
/// laid end to end, each block's twelve-byte header skipped.
struct TopicStream<'a> {
    bytes: &'a [u8],
    start: usize,
    used: usize,
}

impl<'a> TopicStream<'a> {
    fn new(file: &'a HelpFile) -> Self {
        TopicStream {
            bytes: &file.bytes,
            start: file.topic.0,
            used: file.topic.1,
        }
    }

    /// A topic position as a payload-stream offset.
    fn stream_offset(pos: i32) -> Result<usize> {
        if pos < TOPIC_BLOCK_HEADER as i32 {
            return Err(bad(format!("topic position {pos:#x}")));
        }
        let block = (pos >> 14) as usize;
        let within = (pos & 0x3fff) as usize;
        if !(TOPIC_BLOCK_HEADER..TOPIC_BLOCK).contains(&within) {
            return Err(bad(format!("topic position {pos:#x}")));
        }
        Ok(block * TOPIC_BLOCK_PAYLOAD + within - TOPIC_BLOCK_HEADER)
    }

    /// `len` bytes from stream offset `at`.
    fn read(&self, at: usize, len: usize) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(len);
        let mut at = at;
        let mut left = len;
        while left > 0 {
            let block = at / TOPIC_BLOCK_PAYLOAD;
            let within = at % TOPIC_BLOCK_PAYLOAD;
            let file_at = self.start + block * TOPIC_BLOCK + TOPIC_BLOCK_HEADER + within;
            let take = left.min(TOPIC_BLOCK_PAYLOAD - within);
            let end = file_at + take;
            if end > self.start + self.used {
                return Err(FormatError::UnexpectedEof {
                    offset: file_at,
                    needed: take,
                });
            }
            out.extend_from_slice(&self.bytes[file_at..end]);
            at += take;
            left -= take;
        }
        Ok(out)
    }

    /// The link at a topic position: a twenty-one-byte header — block size,
    /// the second part's length, the previous and next positions, the first
    /// part's length (header included), the record type — then the parts.
    fn link(&self, pos: i32) -> Result<Link> {
        let at = Self::stream_offset(pos)?;
        let header = self.read(at, LINK_HEADER)?;
        let mut c = Cursor::new(&header, 0);
        let block_size = c.i32()?;
        let len2 = c.i32()?;
        let _previous = c.i32()?;
        let next = c.i32()?;
        let len1 = c.i32()?;
        let record = c.u8()?;
        let len1 = usize::try_from(len1)
            .ok()
            .filter(|l| *l >= LINK_HEADER)
            .ok_or_else(|| bad("link length"))?;
        let stored2 = usize::try_from(block_size)
            .ok()
            .and_then(|b| b.checked_sub(len1))
            .ok_or_else(|| bad("link block size"))?;
        let len2 = usize::try_from(len2).map_err(|_| bad("link text length"))?;
        if len2 > stored2 {
            // Only a phrase-compressed file stores less than it means, and
            // this reader does not decompress phrases.
            return Err(bad("phrase-compressed text"));
        }
        let data1 = self.read(at + LINK_HEADER, len1 - LINK_HEADER)?;
        let data2 = self.read(at + len1, len2)?;
        Ok(Link {
            record,
            next,
            data1,
            data2,
        })
    }
}

/// The NUL-separated strings of a record's text, handed out in order.
struct Strings<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Strings<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Strings { bytes, at: 0 }
    }

    /// The next string; empty once the text is used up.
    fn next(&mut self) -> String {
        let rest = self.bytes.get(self.at..).unwrap_or(&[]);
        let len = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        let s = decode_1252(&rest[..len]);
        self.at += (len + 1).min(rest.len());
        s
    }
}

/// How a paragraph is aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// Against the left margin.
    #[default]
    Left,
    /// Against the right margin.
    Right,
    /// Centred.
    Centre,
}

/// A paragraph's layout, in half points.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Style {
    /// The alignment.
    pub align: Align,
    /// Space above.
    pub above: i16,
    /// Space below.
    pub below: i16,
    /// The left indent.
    pub left: i16,
    /// The right indent.
    pub right: i16,
    /// The first line's extra indent — negative for a hanging indent.
    pub first: i16,
    /// The tab stops, from the left indent.
    pub tabs: Vec<i16>,
    /// Whether the paragraph is boxed or ruled.
    pub border: bool,
}

/// The paragraph info: two bytes nobody has named, an id, then a word of
/// flags saying which of the optional fields follow.
fn read_style(c: &mut Cursor) -> Result<Style> {
    let _unknown = c.u8()?;
    let _biased = c.u8()?;
    let _id = c.u16()?;
    let bits = c.u16()?;
    let mut style = Style::default();
    if bits & 0x0001 != 0 {
        let _unknown = c.csl()?;
    }
    if bits & 0x0002 != 0 {
        style.above = c.css()?;
    }
    if bits & 0x0004 != 0 {
        style.below = c.css()?;
    }
    if bits & 0x0008 != 0 {
        let _line_spacing = c.css()?;
    }
    if bits & 0x0010 != 0 {
        style.left = c.css()?;
    }
    if bits & 0x0020 != 0 {
        style.right = c.css()?;
    }
    if bits & 0x0040 != 0 {
        style.first = c.css()?;
    }
    if bits & 0x0100 != 0 {
        let _border = c.u8()?;
        let _width = c.i16()?;
        style.border = true;
    }
    if bits & 0x0200 != 0 {
        let count = c.css()?;
        for _ in 0..count.max(0) {
            let stop = c.cus()?;
            if stop & 0x4000 != 0 {
                let _kind = c.cus()?;
            }
            style.tabs.push((stop & 0x3fff) as i16);
        }
    }
    if bits & 0x0400 != 0 {
        style.align = Align::Right;
    }
    if bits & 0x0800 != 0 {
        style.align = Align::Centre;
    }
    Ok(style)
}

/// What a hotspot does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Jump {
    /// Open a topic in the window.
    Topic(i32),
    /// Show a topic in a popup.
    Popup(i32),
    /// Run a macro — which this project does not.
    Macro(String),
    /// A topic the file does not have.
    Missing,
}

/// Where a picture sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// In the line, like a character.
    Inline,
    /// Against the left margin, with the text alongside.
    Left,
    /// Against the right margin, with the text alongside.
    Right,
}

/// One piece of a paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    /// Text in one font, all of it a hotspot or none of it.
    Text {
        /// The font number, into [`HelpFile::fonts`].
        font: u16,
        /// The text.
        text: String,
        /// The hotspot it is part of, if any.
        jump: Option<Jump>,
    },
    /// A picture from the file's own store.
    Picture {
        /// The `|bm<n>` number.
        number: u16,
        /// Where it sits.
        placement: Placement,
    },
    /// A tab to the next stop.
    Tab,
    /// A line break within the paragraph.
    Break,
}

/// One paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paragraph {
    /// Its layout.
    pub style: Style,
    /// Its pieces, in order.
    pub runs: Vec<Run>,
}

/// One column of a table: its width and the gap before it, in half points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    /// The width.
    pub width: i16,
    /// The gap before it — the left margin for the first.
    pub gap: i16,
}

/// One cell of a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Which column it is in.
    pub column: usize,
    /// Its paragraphs.
    pub paragraphs: Vec<Paragraph>,
}

impl Cell {
    /// The cell's text, paragraphs one to a line.
    #[must_use]
    pub fn plain(&self) -> String {
        plain_text(self.paragraphs.iter())
    }
}

/// A table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// The columns.
    pub columns: Vec<Column>,
    /// The rows, each its cells in column order.
    pub rows: Vec<Vec<Cell>>,
}

/// One block of a topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// Paragraphs.
    Text(Vec<Paragraph>),
    /// A table.
    Table(Table),
}

/// A topic, laid out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topic {
    /// Its topic offset.
    pub offset: i32,
    /// Its title.
    pub title: String,
    /// The previous topic of its browse sequence.
    pub browse_back: Option<i32>,
    /// The next topic of its browse sequence.
    pub browse_forward: Option<i32>,
    /// The non-scrolling region at the top, if it has one.
    pub band: Vec<Block>,
    /// The scrolling text.
    pub body: Vec<Block>,
}

impl Topic {
    /// Every paragraph, band then body, tables flattened cell by cell.
    pub fn paragraphs(&self) -> impl Iterator<Item = &Paragraph> {
        self.band.iter().chain(self.body.iter()).flat_map(|block| {
            let out: Vec<&Paragraph> = match block {
                Block::Text(paragraphs) => paragraphs.iter().collect(),
                Block::Table(table) => table
                    .rows
                    .iter()
                    .flat_map(|row| row.iter().flat_map(|cell| cell.paragraphs.iter()))
                    .collect(),
            };
            out
        })
    }

    /// The text, paragraphs one to a line.
    #[must_use]
    pub fn plain_text(&self) -> String {
        plain_text(self.paragraphs())
    }
}

/// Paragraphs as text, one to a line.
fn plain_text<'a>(paragraphs: impl Iterator<Item = &'a Paragraph>) -> String {
    let mut out = String::new();
    for paragraph in paragraphs {
        for run in &paragraph.runs {
            match run {
                Run::Text { text, .. } => out.push_str(text),
                Run::Tab => out.push('\t'),
                Run::Break => out.push('\n'),
                Run::Picture { .. } => {}
            }
        }
        out.push('\n');
    }
    out
}

/// The paragraphs one style covers: strings from the text alternate with
/// formatting commands until the `0xff` that ends them.
fn read_paragraphs(
    file: &HelpFile,
    c: &mut Cursor,
    text: &mut Strings,
    style: &Style,
) -> Result<Vec<Paragraph>> {
    let mut paragraphs = Vec::new();
    let mut runs: Vec<Run> = Vec::new();
    let mut font = 0u16;
    let mut jump: Option<Jump> = None;
    let push_text = |runs: &mut Vec<Run>, s: String, font: u16, jump: &Option<Jump>| {
        if s.is_empty() {
            return;
        }
        if let Some(Run::Text {
            font: f,
            text,
            jump: j,
        }) = runs.last_mut()
        {
            if *f == font && *j == *jump {
                text.push_str(&s);
                return;
            }
        }
        runs.push(Run::Text {
            font,
            text: s,
            jump: jump.clone(),
        });
    };
    loop {
        let s = text.next();
        push_text(&mut runs, s, font, &jump);
        let command = c.u8()?;
        match command {
            0xff => break,
            0x20 => {
                let _vfld = c.i32()?;
            }
            0x21 => {
                let _dtype = c.i16()?;
            }
            0x80 => font = c.u16()?,
            0x81 => runs.push(Run::Break),
            0x82 => {
                paragraphs.push(Paragraph {
                    style: style.clone(),
                    runs: std::mem::take(&mut runs),
                });
            }
            0x83 => runs.push(Run::Tab),
            0x86..=0x88 => {
                let placement = match command {
                    0x87 => Placement::Left,
                    0x88 => Placement::Right,
                    _ => Placement::Inline,
                };
                let kind = c.u8()?;
                let size = usize::try_from(c.csl()?).map_err(|_| bad("picture size"))?;
                if kind == 0x22 {
                    let _hotspots = c.cus()?;
                }
                let start = c.at;
                if kind == 3 || kind == 0x22 {
                    let embedded = c.i16()?;
                    if embedded == 0 {
                        let number = c.u16()?;
                        runs.push(Run::Picture { number, placement });
                    }
                    // An embedded picture's bytes follow; nothing in this
                    // file embeds one, and it is skipped either way.
                }
                c.at = start + size;
            }
            0x89 => jump = None,
            0x8b => push_text(&mut runs, "\u{a0}".to_string(), font, &jump),
            0x8c => {}
            0xc8 | 0xcc => {
                let len = usize::try_from(c.i16()?).map_err(|_| bad("macro length"))?;
                let raw = c.take(len.saturating_sub(2))?;
                let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
                jump = Some(Jump::Macro(decode_1252(&raw[..end])));
            }
            0xe0 | 0xe1 => {
                let target = c.i32()?;
                jump = Some(if command == 0xe0 {
                    Jump::Popup(target)
                } else {
                    Jump::Topic(target)
                });
            }
            0xe2 | 0xe3 | 0xe6 | 0xe7 => {
                let hash = c.u32()?;
                jump = Some(match (file.topic_for_hash(hash), command & 1) {
                    (Some(t), 0) => Jump::Popup(t),
                    (Some(t), _) => Jump::Topic(t),
                    (None, _) => Jump::Missing,
                });
            }
            0xea | 0xeb | 0xee | 0xef => {
                let len = usize::try_from(c.i16()?).map_err(|_| bad("jump length"))?;
                let raw = c.take(len)?;
                let mut inner = Cursor::new(raw, 0);
                let kind = inner.u8()?;
                let target = inner.i32()?;
                // Types 4 and 6 name another file, which this project does
                // not open; 0 and 1 stay in this one.
                jump = Some(match (kind, command) {
                    (0 | 1, 0xea | 0xeb) => Jump::Popup(target),
                    (0 | 1, _) => Jump::Topic(target),
                    _ => Jump::Missing,
                });
            }
            other => return Err(bad(format!("formatting command {other:#04x}"))),
        }
    }
    if !runs.is_empty() {
        paragraphs.push(Paragraph {
            style: style.clone(),
            runs,
        });
    }
    Ok(paragraphs)
}

/// A hotspot drawn on a picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotspot {
    /// Its rectangle, in pixels of the picture: left, top, width, height.
    pub rect: (u16, u16, u16, u16),
    /// Where it goes.
    pub jump: Jump,
}

/// A picture, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// The pixels.
    pub image: Image,
    /// Its hotspots.
    pub hotspots: Vec<Hotspot>,
}

/// A `|bm<n>` file: magic `lp`/`lP`, a picture count, offsets to each; a
/// picture is its type and packing, then for a bitmap the resolution,
/// planes, bit count, size, colours, the packed size, the hotspot data's
/// size, and the offsets of both — the palette between.
fn read_picture(file: &HelpFile, data: &[u8]) -> Result<Picture> {
    let mut c = Cursor::new(data, 0);
    let magic = c.u16()?;
    if magic != 0x506c && magic != 0x706c {
        return Err(FormatError::BadMagic {
            expected: 0x706c,
            found: u32::from(magic),
        });
    }
    let count = c.u16()?;
    if count == 0 {
        return Err(bad("picture with no pictures"));
    }
    let first = usize::try_from(c.i32()?).map_err(|_| bad("picture offset"))?;
    let mut c = Cursor::new(data, first);
    let kind = c.u8()?;
    let packing = c.u8()?;
    if kind != 5 && kind != 6 {
        return Err(bad(format!("picture type {kind} (a metafile)")));
    }
    let _xdpi = c.cul()?;
    let _ydpi = c.cul()?;
    let planes = c.cus()?;
    let bits = c.cus()?;
    let width = c.cul()?;
    let height = c.cul()?;
    let colours_used = c.cul()?;
    let _colours_important = c.cul()?;
    let packed_size = usize::try_from(c.cul()?).map_err(|_| bad("picture size"))?;
    let hotspot_size = usize::try_from(c.cul()?).map_err(|_| bad("hotspot size"))?;
    let packed_at = usize::try_from(c.u32()?).map_err(|_| bad("picture offset"))?;
    let hotspots_at = usize::try_from(c.u32()?).map_err(|_| bad("hotspot offset"))?;
    let colours = if colours_used == 0 {
        1u32 << bits.min(8)
    } else {
        colours_used
    };
    let palette = if kind == 6 {
        c.take(colours as usize * 4)?
    } else {
        &[]
    };
    let packed = data
        .get(first + packed_at..first + packed_at + packed_size)
        .ok_or_else(|| bad("picture bits run off the file"))?;
    let bits_data = match packing {
        0 => packed.to_vec(),
        1 => run_length(packed),
        other => return Err(bad(format!("picture packing {other}"))),
    };

    // Rebuild a device-independent bitmap and let the resource decoder
    // unpack the pixels.
    let mut dib = Vec::with_capacity(40 + palette.len() + bits_data.len());
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&width.to_le_bytes());
    dib.extend_from_slice(&height.to_le_bytes());
    dib.extend_from_slice(&planes.max(1).to_le_bytes());
    dib.extend_from_slice(&bits.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&(bits_data.len() as u32).to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&colours.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(palette);
    dib.extend_from_slice(&bits_data);
    let image = read_dib(&dib)?;

    let mut hotspots = Vec::new();
    if hotspot_size > 0 && hotspots_at > 0 {
        let raw = data
            .get(first + hotspots_at..first + hotspots_at + hotspot_size)
            .ok_or_else(|| bad("hotspots run off the file"))?;
        let mut c = Cursor::new(raw, 0);
        let _one = c.u8()?;
        let count = c.u16()?;
        let _macro_size = c.u32()?;
        for _ in 0..count {
            let id0 = c.u8()?;
            let _id1 = c.u8()?;
            let _id2 = c.u8()?;
            let x = c.u16()?;
            let y = c.u16()?;
            let w = c.u16()?;
            let h = c.u16()?;
            let hash = c.u32()?;
            let jump = match id0 {
                0xe2 | 0xe6 => file.topic_for_hash(hash).map_or(Jump::Missing, Jump::Popup),
                0xe3 | 0xe7 => file.topic_for_hash(hash).map_or(Jump::Missing, Jump::Topic),
                _ => Jump::Missing,
            };
            hotspots.push(Hotspot {
                rect: (x, y, w, h),
                jump,
            });
        }
    }
    Ok(Picture { image, hotspots })
}

/// The help file's run-length packing: a byte with its top bit set says how
/// many bytes follow as they are; one without says how many times the next
/// byte repeats.
fn run_length(packed: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(packed.len() * 2);
    let mut i = 0;
    while i < packed.len() {
        let n = packed[i];
        i += 1;
        if n & 0x80 != 0 {
            let count = usize::from(n & 0x7f);
            let end = (i + count).min(packed.len());
            out.extend_from_slice(&packed[i..end]);
            i = end;
        } else if i < packed.len() {
            out.extend(std::iter::repeat_n(packed[i], usize::from(n)));
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_numbers_follow_the_spec() {
        // Even bytes halve; odd ones carry a second byte.
        let mut c = Cursor::new(&[0x44, 0x9f, 0x02], 0);
        assert_eq!(c.cus().unwrap(), 34);
        assert_eq!(c.cus().unwrap(), 335);
        // Signed shorts are biased by sixty-four.
        let mut c = Cursor::new(&[0x8c, 0x98], 0);
        assert_eq!(c.css().unwrap(), 6);
        assert_eq!(c.css().unwrap(), 12);
        // Longs are words, biased by 16384 when signed.
        let mut c = Cursor::new(&[0x1e, 0x80, 0x08, 0x80], 0);
        assert_eq!(c.csl().unwrap(), 15);
        assert_eq!(c.csl().unwrap(), 4);
    }

    #[test]
    fn run_length_unpacks_both_kinds_of_run() {
        assert_eq!(
            run_length(&[0x83, 1, 2, 3, 0x03, 9]),
            vec![1, 2, 3, 9, 9, 9]
        );
    }

    #[test]
    fn a_file_without_the_magic_is_refused() {
        let err = HelpFile::read(vec![0; 64]).unwrap_err();
        assert!(matches!(err, FormatError::BadMagic { .. }));
    }
}
