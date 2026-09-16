# The help viewer — the Help buttons and the player's guide

Status: **the file, the topics, the button bar, hotspots, popups, Search
and History recovered**; the pictures the file stores as metafiles, and
italic text, are not drawn as such.

There is no help code in `STARS!.EXE`. Every Help button is one call of
`WINHELP(hwnd, szHelpFile, HELP_CONTEXT, id)` — the import at `14f8:029c`,
thirty-six call sites — and the Help menu's Player's Guide (`0x101`, F1) is
`WINHELP(…, HELP_INDEX, 0)` (`CommandHandler`, `1020:47c9`). Windows'
`WINHELP.EXE` then opens `STARS!.HLP`, the player's guide, on the topic
whose context number was given. The numbers, one per dialog, are tabled in
`../formats/help.md`, which is also where the file's format is.

## What WinHelp 3.1 shows

The viewer is Windows' own, so what this project reproduces is that
program's main window as Windows 3.1 shipped it:

- the caption is the file's title, `Stars! Player's Guide` (`|SYSTEM`
  record 1);
- a menu bar of **File, Edit, Bookmark, Help**;
- a button bar of **Contents, Search, Back, History** and — because the
  file's `[CONFIG]` runs `BrowseButtons()` — **`<<`** and **`>>`**. (The
  same section registers a `hyprfind.dll` Find+ button, which needs a DLL
  nobody ships; it is left out.)
- the window opens at 710 by 909 thousandths of the screen (`|SYSTEM`
  record 6), not maximised;
- a topic's **non-scrolling region** across the top — the title and, on
  most pages, a green link to the section it belongs to — on white (the
  record's `RgbNsr`), with a rule under it; then the **scrolling region**;
- **hotspots** in green, underlined; a click on one either **jumps** to a
  topic or raises a **popup** — a bordered box at the pointer holding the
  topic, which goes away at the next click or Escape;
- **pictures**, some of them maps: the contents page is one picture of the
  game's screen with eight hotspots on it.

Two numbers the game asks for — the password prompt's `0x441` and the
save-on-exit question's `0x442` — and the menu's `0x9c1` (`0x32ca`) are not
in the file's map. WinHelp answers those with *Help topic does not exist*;
this project's notice says the same in its own words. With no copy of
the file at all, every Help button puts up a notice saying where to put
one.

## Search

WinHelp's Search dialog: *Type a word, or select one from the list* over
the file's keyword list (`|KWBTREE`, 413 words), a **Show Topics** button
that lists the topics the word is filed under (`|KWDATA`, titled from
`|TTLBTREE`), and **Go To** for the one picked. Typing narrows the list to
the words that start with what was typed; picking a word shows its topics
at once, as a double-click does in WinHelp.

## History

WinHelp's *Windows Help History* window: every topic shown this session,
most recent first, each once; a click opens it.

## Back and browse

**Back** retraces the topics shown, one at a time, and is disabled at the
bottom of the stack. **`<<`** and **`>>`** follow the file's browse
sequences (the topic header's `BrowseBck` / `BrowseFor`) and are disabled
at either end of one — the contents page starts a sequence, so `<<` is
dead there.

## Text

The file names 43 fonts (`|FONT`): Helv and Arial at 8 to 14 points,
Courier for tables of figures, Symbol for a bullet or two. Sizes are the
file's, in half points, at ninety-six dots an inch; the colours are the
file's, with its (1, 1, 0) — WinHelp's "the window's text colour" — drawn
black. Paragraph spacing, indents, alignment and the tables' column widths
(`docs/formats/help.md`) are honoured; tab stops are drawn as four spaces.

egui's bundled set has no bold or italic face, the gap the status bar and
the relations dialog note. Bold is made here by painting the paragraph a
second time, seven tenths of a pixel to the right, with everything but
the bold runs transparent; italic is drawn regular. A picture stored as a
metafile (five of the 134) is left out; the 129 bitmaps draw at their own
size.

## Which button asks for what

| where | button | context | routine and site |
|-------|--------|---------|------------------|
| Help menu | Introduction | `0x1195` | `CommandHandler` `1020:479e` |
| Help menu, F1 | Player's Guide | *contents* (`HELP_INDEX`) | `1020:47c9` |
| Cargo Transfer | Help | `0x433` | `TransferDlg` `1050:59be` |
| Ship Transfer | Help | `0x438` | `TransferDlg` `1050:59b7` |
| Production | Help | `0x423` | `ProdCommandHandler` `10d0:3393` |
| `<Customize>` | Help | `0x452` | `ZipProdDlg` `10d0:5d8e` |
| Ship Designer | Help | `0x42a`; `0xbdf` in the editor | `SlotDlg` `10c8:25cf`, `25c8` |
| Research | Help | `0x42e` | `ResearchDlg` `10d8:088f` |
| Battle VCR | Help | `0x43a` | `VCRDlg` `10e8:18b3` |
| Battle Plans | Help | `0x439` | `BattlePlansDlg` `10f0:16ac` |
| Player Relations | Help | `0x43b` | `RelationsDlg` `10f0:0434` |
| Merge Fleets | Help | `0x453` | `MergeFleetsDlg` `1080:35f7` |
| Race Wizard, pages 1–6 | Help | `0x3ff`, `0x41d`, `0x420`, `0x408`, `0x411`, `0x421` | `RaceWizardDlg1`…`6` |
| Game Parameters, steps 1–3 | Help | `0x3f4`, `0x3fc`, `0x3fd` | `NewGameDlg`…`3` |
| Change Password | Help | `0x43c` | `NewPasswordDlg` `1040:5f65` |
| Password prompt | Help | `0x441` — *not in the file* | `PasswordDlg` `1040:5c5f` |
| Host Mode | Help | `0x440` | `HostModeDialog` `1020:753f` |
| Score sheet | Help | `0x455` | `ScoreXDlg` `1108:12c1` |
| Find | Help | `0x43d` | `FindDlg` `1058:9400` |
| Tutor | Hint | `tutor.idh` | `TutorDlg` `10f8:01bd` |

The race wizard's first page, `0x3ff`, points into the middle of a topic
block rather than at a topic header: the offset lands in the text of
*Step 1: Basic Definition*, whose header is the block before's
`LastTopicHeader`, which is how WinHelp resolves it too.

## What this project does

`stars_formats::HelpFile` reads the file (`../formats/help.md`);
`stars_ui::help::Help` is the viewer's state — the topic on show, the
Back stack, the visited list, the popup, the Search and History windows,
a notice — with `App::help_context` as `HELP_CONTEXT`, `help_contents`
as `HELP_INDEX`, `help_jump`, `help_back`, `help_browse`, `help_goto`,
`help_search_open` / `help_keyword_matches` / `help_show_topics` /
`help_search_go`. `views::help::windows` draws it with the other dialogs
(`views::frame::dialogs`). The desktop shell looks for `STARS!.HLP` where
it looks for the executable: beside the game opened, beside a
`STARS_EXE`, the working directory and `binary/`.

`tests/help_viewer.rs` covers the lot against the real file — every
context number above finds its page, the missing three put up the
notice, Back and the browse pair, hotspots and popups, Search, and a
sweep of every seventh titled topic through a real egui pass.

## What is not

* Italic text, and the five metafile pictures.
* Bookmarks, annotations, printing, and the Find+ full-text search that
  the file's `hyprfind.dll` macros would add.
* The tutor's `idh`: the page checks that set it are not transcribed, so
  Hint opens the contents page rather than the page's own topic.
* Popups are dismissed by a click anywhere or Escape; WinHelp dismisses
  one on any key. A popup taller than the screen scrolls here, where
  WinHelp cuts it off.

## Looking at it

`cargo run -p stars-ui --example render_help -- binary/STARS!.HLP 0x433
out.ppm` lays the viewer out on a topic through a real egui pass and
rasterises the frame to a portable pixmap in software, for a look at a
screen from a shell with no display. `contents`, a decimal topic offset
and `popup:<offset>` are the other subjects. The desktop shell's
`STARS_HELP_TOPIC=0x433` opens the viewer on that number at start.
