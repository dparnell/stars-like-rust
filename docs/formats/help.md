# Format: `STARS!.HLP` — the player's guide

- **Status:** verified — every one of the file's 414 titled topics is found
  at its offset and reads through; 129 of its 134 pictures decode (the other
  five are metafiles)
- **Not the game's own format.** It is a Windows 3.1 help file, read by
  `WINHELP.EXE`; the game only names it (`szHelpFile`, `1120:014a`) and asks
  for a topic by number.
- **Reference:** Manfred Winterhoff's `helpdeco` and its `helpfile.txt`,
  which is where the field names below come from. The file is in nobody's
  documentation; the structure was checked byte by byte against
  `binary/STARS!.HLP`.
- **Implemented in:** `crates/stars-formats/src/help.rs`
  (`stars_formats::HelpFile`); tests in `tests/help_file.rs`; a dump in
  `examples/help_dump.rs`

## How the game reaches it

`WINHELP(hwnd, szHelpFile, wCommand, dwData)` is imported at `14f8:029c`.
Every dialog's Help button calls it with `HELP_CONTEXT` (1) and a **context
number**; the Help menu's Player's Guide and F1 (`CommandHandler`,
`1020:47c9`, commands `0x101` and `0x8a`) with `HELP_INDEX` (3), which is the
contents page. The context numbers, read off each `MOV AX, imm` before the
call:

| dialog | routine | site | context | topic |
|--------|---------|------|---------|-------|
| Introduction (menu `0x9c2`) | `CommandHandler` | `1020:47ab` | `0x1195` | Welcome to Stars! |
| menu `0x9c1` | `CommandHandler` | `1020:47ab` | `0x32ca` | *not in the file* |
| Host mode | `HostModeDialog` | `1020:7547` | `0x440` | Host Mode Dialog |
| Host options | `HostOptionsDialog` | `1020:76ac` | `0x440` | Host Mode Dialog |
| Message box (copy protection) | `MsgDlg` | `1030:91d6` | `0xdbc` | Copy Protection |
| Password | `PasswordDlg` | `1040:5c77` | `0x441` | *not in the file* |
| Change password | `NewPasswordDlg` | `1040:5f7d` | `0x43c` | Change Password |
| Find | `FindDlg` | `1058:9418` | `0x43d` | Find Planet or Fleet |
| New game (simple) | `SimpleNewGameDlg` | `1078:7e47` | `0x3ea` | New Game Setup (Basic) |
| New game, steps 1–3 | `NewGameDlg`…`3` | `1078:852f`, `9459`, `9c18` | `0x3f4`, `0x3fc`, `0x3fd` | Step 1/2/3 |
| Race wizard, pages 1–6 | `RaceWizardDlg1`…`6` | `10e0:0beb`, `1637`, `297f`, `35dd`, `3a9b`, `3f53` | `0x3ff`, `0x41d`, `0x420`, `0x408`, `0x411`, `0x421` | the six steps (`0x3ff`'s topic has no title) |
| Cargo transfer | `TransferDlg` | `1050:59c4` | `0x433`, or `0x438` when the dialog is the ship transfer (`[0x984] == 1`) | Cargo Transfer Dialogs / Ship Transfer Dialog |
| Production | `ProdCommandHandler` | `10d0:33ab` | `0x423` | Production Dialog |
| Production templates | `ZipProdDlg` | `10d0:5da6` | `0x452` | Customize Production Templates dialog |
| Rename template | `RenameZipDlg` | `1080:0a25` | `0x452`, or `0xc1f` when renaming a zip order (`[0x16b8] != 0`) | … / Creating a Custom Transport Zip Order |
| Zip orders | `ZipOrderDlg` | `1080:07e7` | `0x44a` | Custom Zip Orders dialog |
| Rename fleet | `RenameDlg` | `1080:0cbb` | `0x447` | Rename Fleet dialog |
| Merge fleets | `MergeFleetsDlg` | `1080:360f` | `0x453` | Merge Fleets dialog |
| Ship designer | `SlotDlg` | `10c8:25d5` | `0x42a`, or `0xbdf` when `[0x5466] == 4` | Ship Designer / Designing a New Ship from Scratch |
| Research | `ResearchDlg` | `10d8:08a7` | `0x42e` | Research Dialog |
| Battle VCR | `VCRDlg` | `10e8:18cb` | `0x43a` | Battle VCR |
| Player relations | `RelationsDlg` | `10f0:044c` | `0x43b` | Player Relations Dialog |
| Battle plans, plan name | `BattlePlansDlg`, `NewPlanNameDlg` | `10f0:16c4`, `0610` | `0x439` | Battle Plans Dialog |
| Tutorial, panic | `TutorDlg`, `PanicDlg` | `10f8:01bd`, `0311` | the page's own (`[0x5212]`) | — |
| Score sheet | `ScoreXDlg` | `1108:12d9` | `0x455` | Score sheet |
| Print map | `PrintMapDlg` | `1108:a3d8` | `0xc3c` | Printing a Map of the Universe |
| Save on exit | `AskSaveDialog` | `1070:43b1` | `0x442` | *not in the file* |

Three numbers are asked for that the file does not map; WinHelp answers
those with its "topic does not exist" box, and so does this project.

## The container

| offset | size | field |
|--------|------|-------|
| 0 | 4 | magic `0x0003_5f3f` |
| 4 | 4 | offset of the directory's file header |
| 8 | 4 | first free block, `-1` |
| 12 | 4 | file size |

Internal files begin with a nine-byte header — reserved size (4), used
size (4), a flag byte — and their data follows. The directory is a **B+
tree** of `STRINGZ name → i32 offset`; this file has 143 entries.

A B+ tree: a 38-byte header — magic `0x293b`, flags, page size, a
sixteen-byte structure string, zero, page splits, root page, `-1`, page
count, level count, entry count — then the pages. Index pages (`unused`,
`count`, `previous`, then `key, page` pairs) are descended by `previous`
to the leftmost leaf; leaves (`unused`, `count`, `previous`, `next`) chain
by `next`. The trees here: the directory (1k pages), `|CONTEXT` (`u32 hash
→ i32 topic`, 413 entries), `|TTLBTREE` (`i32 topic → STRINGZ title`, 414),
`|KWBTREE` (`STRINGZ word, i16 count, i32 offset into |KWDATA`).

## `|SYSTEM`

Magic `0x036c`, minor 21 (Windows 3.1), major 1, a date, flags — **0 here,
so nothing is compressed** (4 and 8 mean LZ77 topic blocks, which this
reader refuses). Then records of `u16 type, u16 size, data`:

- 1: the title, `Stars! Player's Guide`;
- 3: the contents topic — offset 0;
- 4: macros run on opening (`BrowseButtons()` and a `hyprfind.dll` search
  the file's Find+ button needs, which this project does not have);
- 6: the main window, 90 bytes: flags, `Type[10]`, `Name[9]`,
  `Caption[51]`, x, y, width, height (thousandths of the screen), maximise,
  two `COLORREF`s. Flags `0x2fb`: the size (710 × 909) and the
  non-scrolling region's white are set; the caption and the scrolling
  region's colour are not.

## `|FONT`

`u16` face count (180), descriptor count (43), face-name offset, descriptor
offset; the names as fixed-width strings; then eleven-byte descriptors:
attribute bits (1 bold, 2 italic, 4 underline, 8 strike-out), size in
**half points**, family, face index, foreground RGB, background RGB. Font 4
is the body text (Arial 10pt), 3 the title (Arial 14pt), 8 the green
hotspot text.

## `|CTXOMAP`

`u16 count`, then `(u32 map id, i32 topic offset)` pairs — 413 of them.
This is the `[MAP]` of the help project and what `HELP_CONTEXT` looks up.

## `|TOPIC`

4096-byte blocks, each with a twelve-byte header — the last link that runs
into this block from the previous, the first link that starts here, the last
topic header — and the records ("topic links") laid end to end **through
the payloads**: a record that reaches the end of a block continues after
the next block's header.

A **topic position** is `block << 14 | offset in block`. A **topic offset**
— what every table and hotspot uses — is `block << 15 | characters`, where
the characters are the sum of the `TopicLength` of every text and table
record from the block's first link up to the one wanted. `TopicLength` is
the record's text length plus, for a picture, its hotspot count — which is
why the contents page's second topic is at 47 characters, not 38.

A record's 21-byte header: `i32` block size, `i32` text length, `i32`
previous position, `i32` next position, `i32` first part's length (header
included), `u8` record type. Then the first part (formatting) and the
second (text: NUL-separated strings). Record types:

- **2, a topic header**: 28 bytes — size, browse back and forward (topic
  offsets, or `-1`), topic number, the non-scrolling region's start, the
  scrolling region's start (positions), the next header — and the title as
  the text, *without* its terminator.
- **`0x20`, text**: compressed-long size, compressed-short `TopicLength`,
  one paragraph style, then formatting commands until `0xff`.
- **`0x23`, a table**: as text, then `u8` columns, `u8` type (0/2 variable
  width, with an `i16` minimum), `(i16 width, i16 gap)` per column, and per
  paragraph `i16 column` (`-1` ends the record), `i16`, `u8`, a style and
  its commands. A column below the previous starts a new row.

The paragraph style: two bytes, `u16` id, `u16` flag bits saying which
fields follow — `0x0002` space above, `0x0004` below, `0x0008` line
spacing, `0x0010` left indent, `0x0020` right, `0x0040` first line
(compressed signed shorts, half points); `0x0100` border (`u8`, `i16`);
`0x0200` tabs (count, then compressed shorts, bit `0x4000` adding a type);
`0x0400` right-aligned, `0x0800` centred.

Formatting: each string of the text is emitted, then one command read —
`0x80` font (`u16`), `0x81` line break, `0x82` paragraph end, `0x83` tab,
`0x86`/`0x87`/`0x88` a picture in the line / at the left / at the right
(`u8` type, compressed-long size, a hotspot count if the type is `0x22`,
then `i16` embedded flag and `i16` `|bm` number), `0x89` hotspot end,
`0x8b` non-breaking space, `0x8c` non-breaking hyphen, `0xc8`/`0xcc` a
macro (`i16` length, the string), `0xe0`/`0xe1` popup/jump to a topic
offset, `0xe2`/`0xe3` popup/jump by context-name hash (`|CONTEXT`),
`0xe6`/`0xe7` the same without the green underline, `0xea`–`0xef` jumps
into other files or windows, `0xff` end.

Compressed numbers: a short is one byte halved when even, else halved plus
128 × the next byte; a long is a word halved when even, else halved plus
32768 × the next word; the signed forms subtract 64 / 16384 (short) and
16384 / 67108864 (long). All of it unsigned first: `0x8008` is 4.

## `|bm<n>`

Magic `lp` (`0x706c`), picture count, `i32` offsets. A picture: `u8` type
(5 DDB, 6 DIB, 8 metafile), `u8` packing (0 none, 1 run-length, 2 LZ77);
for a bitmap the resolution, planes, bit count, width, height, colours
used and important (compressed), the packed size, the hotspot data's size,
and `u32` offsets of both from the type byte; a DIB's palette follows as
`RGBQUAD`s. Run-length: a byte with its top bit set is a count of literal
bytes; without, a repeat count for the next byte. The bits unpacked are a
bottom-up DIB, which `resources::read_dib` decodes.

Hotspot data: `u8` 1, `u16` count, `u32` macro data size, then per hotspot
three id bytes (`0xe3 0 0` is a visible jump, `0xe2` a popup, `0xe7`/`0xe6`
invisible), `u16` x, y, w, h and a `u32` hash. The contents page is one
such picture with eight hotspots.

## What this project does

`HelpFile::read` keeps the bytes and indexes the tables; `topic(offset)`
walks the block to the header and reads every record to the next header
into a `Topic` — its non-scrolling band and its body, as paragraphs of
runs (text in a font, with the hotspot it belongs to resolved to a topic
offset; pictures by number; tabs; breaks) and tables. `picture(n)` decodes
a `|bm` with its hotspots. `topic_for_context` is `HELP_CONTEXT`.

Not read: `|Phrases` (there is none), LZ77 (not used), embedded pictures
(none), metafiles (five, left blank), macros (run nowhere), the `|KWMAP`.
