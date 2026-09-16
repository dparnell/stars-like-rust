# The tutorial

Status: **built** — the text, all eighty pages, the checks, the advance
machine, the tutorial's world and its window. What is left is listed at the
foot: the galaxy is not seed-identical, so the pages name the wrong planets.

The original ships a tutorial that walks a new player through **36 years of a
sample game**, and it is worth having for a reason beyond teaching: it
exercises nearly every screen in the program and states, in the game's own
words, what each is supposed to do. Fifty-five call sites reach into it.

## Shape

| piece | routine | what it is |
|-------|---------|-----------|
| the text | `CchTutorString` (`1100:5a14`) | 80 pages of 8 paragraphs |
| the step machine | `FTutorTaskDone` (`10f8:0fbc`) | one predicate per page |
| stepping on | `AdvanceTutor` (`10f8:0a30`) | checks, steps, redraws |
| starting | `StartTutor` (`10f8:06b4`) | |
| stopping | `EndTutor` (`10f8:0c02`), `FAskKillTutor` (`10f8:0f36`) | |
| the window | `TutorDlg` (`10f8:0000`), `ShowTutor`, `DrawTutorText` | |
| the world | `CreateTutorWorld` (`1078:5e5e`) | the sample game itself |
| complaints | `TutorError` (`10f8:67ae`) | |

`tutor` is one global: `idt`, the current paragraph; `idtBold`, the paragraph
to embolden, which is how a page points at the one thing you have to do;
`idsError`, a complaint; `idh`, a help topic; and a handful of flags.

`AdvanceTutor` steps `idt` by **eight** and loops while the next page's task
is already done, so a page you have satisfied in advance is skipped rather
than shown. Past `0x27f` — the last paragraph of page 80 — it calls
`TutorError(0x20a)` and `EndTutor`.

One thing stops the loop: **bit 3 of `tutor.fVisible`**, the *wait for the
turn* bit. Every year's last page sets it as it reports itself done —
thirty-seven pages, each with a `fVisible |= 8` beside a bold of its
"Press F9" paragraph — and `AdvanceTutor`, seeing it, leaves the page up
with that paragraph emboldened and sets `idh` to `0xdb6` instead of
stepping. The frame clears the bit when the new year arrives (`1020:1093`)
and asks again; and since every arm but the first year's answers *done*
for a page of an earlier year, the old page then steps aside. The step
table carries this as `Step::wait`, the paragraph to show; `App::
tutor_waiting` is the bit, and the ring goes to the Turn menu's Generate.
Without it the tutorial turned to page 6 as soon as Weapons was chosen in
2400, "Read the message in the Messages pane" emboldened, nothing ringed
and nothing to do — page 6's checks belong to 2401 — which is where a
reader from the desktop got stuck.

`StartTutor` runs the same loop before it shows anything, which is why a
new game **must open with something to do**: page one asks you to read your
messages, and `GenerateWorld` (`1078:0136`) gives every player five — four
playing tips (`idm 0x7f`–`0x82`) and `idmHomePlanetPeopleReadyLeaveNestExplore`
(`0xa9`) about the home planet — which is what the page means by *"There are
five messages in the Messages pane"*. Until this project's new games sent
them, page one was satisfied the moment it opened and the tutorial began on
page two.

Fifty-five call sites reach `AdvanceTutor` in the original — after a click on
the map, a change of selection, a message read, a waypoint added. This
project asks once a frame instead, after the frame's input has been handled,
which covers every one of those places and costs only reads.

## The text

Every word of the original's tutorial is the game's own writing, so **none of
it is in this repository**. It is read at run time out of the player's copy of
`stars.exe`, exactly as the artwork is.

With no copy of the game to read from, the tutorial shows this project's own
**retelling** of the same eighty pages instead —
`crates/stars-ui/src/tutorial_text.rs`. It keeps the original's shape exactly:
eight slots to a page, the same paragraph breaks, the same page endings, and
each slot saying what the original's slot says — the same planet, fleet, key
and number — so that the emphasis the step machine points at by slot lands on
the same instruction whichever text is showing. `tests/tutorial_text.rs`
checks that shape slot for slot against the original when a copy is to hand,
and that no slot is the original's words. The game's own words are preferred
whenever they can be read.

The text used to be the only thing missing, and it made the window say so
instead of teaching; a player starting the tutorial from a build that had not
found a copy of the game saw a note about where the words come from.

`CchTutorString` uses the same nibble coding as the main string table — a
per-string nibble count, a per-block start pointer, and a character table that
the running sum of nibbles indexes, with `0xf` as an escape that adds fifteen
and consumes no character. Only the tables differ, and all three sit inside
**segment `0x1100`**:

| table | offset | what |
|-------|--------|------|
| lengths | `0x5734` | one byte per paragraph, `0x40` to a block |
| blocks | `0x59b4` | one word per block of 64, where its nibbles begin |
| characters | `0x59c8` | the alphabet the running sums index |

A page is eight consecutive paragraphs because `idt` steps by eight, and the
page number in the window's title is `idt / 8 + 1`. 80 pages, 640 paragraphs,
about 48,000 characters of text.

`crates/stars-formats/src/tutorial.rs` reads it; `segment_offset` in
`crates/stars-formats/src/resources/mod.rs` finds the segment.

## The step machine

`FTutorTaskDone` is a `switch (game.turn)` — **37 turns** — and inside each
turn a chain of `if (tutor.idt == n)`, one per page. Each arm sets
`tutor.idtBold` to the paragraph the player should be reading and then asks a
predicate whether the thing has been done. Eighty pages, and the predicates
come from a vocabulary of **fifteen** verbs:

| verb | uses | what it asks |
|------|------|--------------|
| `FCheckSelection` (`10f8:6af4`) | 68 | is *that* object selected |
| `FCheckFleetWP` (`10f8:6df4`) | 66 | does fleet *f* have waypoint *n* at *x* |
| `FCheckMessages` (`10f8:6c48`) | 60 | have the messages been read — or, with `fFilter`, has this kind been filtered out |
| `FCheckQueue` (`10f8:7442`) | 35 | is *that* in the production queue |
| `FCheckSummary` (`10f8:69e2`) | 18 | is the summary pane showing *that* |
| `FCheckColonizeWP` (`10f8:70c0`) | 16 | a colonize task set where it should be |
| `FCheckCargo` (`10f8:7664`) | 15 | is the right cargo aboard |
| `FCheckXferWP` (`10f8:7280`) | 13 | a transport task set up — **actions only**, except `UnloadExact` and `SetAmount`, where the figure counts too |
| `FCheckShipBuilder` (`10f8:7964`) | 12 | a design built to spec |
| `FCheckResearch` (`10f8:6da4`) | 11 | the research settings |
| `FCheckScanner` (`10f8:685c`) | 5 | the scanner's view and filters |
| `FCheckBuilderPart` (`10f8:77d8`) | 5 | a part in a design slot |
| `FCheckTemplate` (`10f8:666e`) | 3 | the default template equals one of two canned queues at `10f8:663a` — flag, count and every entry |
| `FCheckZip` (`10f8:6460`) | 1 | a zip order |
| `FCheckPlanetRoute` (`10f8:6f86`) | 1 | a planet's route |

That table is the argument for doing this at all: between them those fifteen
verbs read almost everything the UI can set.

Two things the arms ask that are not in the table, both read straight off a
count byte rather than through a verb:

* **how many orders a fleet has** (`pfl->cord`), which is how the tutorial
  checks that a waypoint has been *deleted* — page 16 is done when Armed Probe
  #1 no longer has its six;
* **how many entries a planet's queue has**, which is how it checks that
  something has been *added* — page 18 is done when the homeworld's queue has
  grown to three.

Both are compared three ways — fewer, exactly, not exactly — because the arms
use all three.

## A page is a chain, not a check

A page is not one predicate. The original writes each arm as a chain:

```c
tutor.idtBold = 0xb;
if (FCheckSelection(grobjFleet, 0)) {
    tutor.idtBold = 0xf;
    done = FCheckFleetWP(0, 1, grobjPlanet, 0xc, 0, 0xffff);
}
```

— so each rung both **emboldens a paragraph** and **gates the next**. The page
is done when every rung passes, and the paragraph shown in bold is the first
rung that does not.

The emphasis is set **immediately before** the check it belongs to, so a
rung's paragraph is the instruction you have *not yet carried out* — not the
one you just did. That is worth stating because it is easy to transcribe the
other way round and end up a paragraph ahead all the way down the page. That is how a page manages to say "select the scout"
first and "now send it to Bandersnatch" second without being two pages.

Several arms open with an **escape hatch**:

```c
if (FCheckFleetWP(1, 1, grobjPlanet, 0x15, 0, 0xffff)) done = 1;
else { ...the chain... }
```

— where the check is usually the **next** page's task. A player who has run
ahead is not made to go back and do this page a step at a time. It is the
same idea as `AdvanceTutor`'s skipping loop, written inside one page, and
`Step::escape` carries it.

And not every check in an arm is part of the answer. Some are asked **only to
decide where the emphasis goes**: page 9 asks whether you have read the fourth
message and then, whatever the answer, gates on the summary pane instead. A
rung like that is `hint` rather than `ask` — skipped when deciding whether the
page is done, but still catching the emphasis on its way past.

A few pages go further and pick a different paragraph for each *wrong* answer
— page 4 has one for each of the three fleets you might have selected instead
of the right one. Those alternatives are not reproduced; the rung points at
the paragraph naming the right answer.

## `grbitScan`, in full

`FCheckScanner` compares against `grbitScan`, and recovering it turned up the
whole toolbar bit table from `ExecuteButton` (`1068:0db6`), which is worth
having on its own. The six views are a radio group in the **low nibble** —
switching keeps `grbitScan & 0x3ff0` — and every toggle owns one bit:

| bit | button |
|-----|--------|
| `0x000f` | the chosen view, 0 to 5 |
| `0x0010` | `Add Way Points Mode` |
| `0x0020` | `Scanner Coverage Overlay` |
| `0x0040` | `Mine Fields Overlay` |
| `0x0080` | `Fleet Paths Overlay` |
| `0x0100` | `Idle Fleets Filter` |
| `0x0200` | `Ship Design Filter` |
| `0x0400` | `Planet Names Overlay` |
| `0x0800` | `Enemy Ship Class Filter` |
| `0x1000` | `Ship Counts Overlay` |
| `0x2000` | `Player Colors`, which is the View menu's own |

`App::grbit_scan` assembles it, so a check can be written the way the original
writes it.

## The text is a specification

Reading the eighty pages as prose turns up a list of things the UI is
*stated* to do, which is the point of building this at all. What the pages
name, and where each stands:

| the tutorial says | routine | state |
|-------------------|---------|-------|
| "Hit the **n** key to look at your next fleet" | `SelectAdjFleet` (`1050:3d32`) | **added** |
| "press the **Next** button in the tile showing Long Range Scout #2" | `rghwndBtn[5]` | **added** |
| "Press the tile's **Goto** button" | `rghwndBtn[1]` | **added** |
| "Hit **F9** to generate the next year" | | **added** |
| "Hit the **q** key" — opens the production queue | | **added** |
| "The **shift** key causes the Add button to add 10 items at a time" | `App::production_step` | already right |
| "click on the **Next** button, or use the **down arrow** key" — messages | | already right |
| "Hit the **v** key to pinpoint it for you" | `CtrPointScan` | **not done**: this map fits the whole galaxy and has no scroll to recentre |
| "Hit **F3** to open the Planet Summary Report" / "Hit the **Esc** key to close" it | menu `0x6d4` | **added** |
| "Open the Planet Summary Report and **sort by Population**" | `SortReportCache` (`1108:589c`) | **added** — see `reports.md` |
| "Find the **Min Conc** column, right click and Reverse Sort by Mineral Concentration - Weighted Average" | `ReportColumnPopup` (`1108:74d4`) | **added** |
| "select **Generate** from the **Turn** menu" | menu `0x6d4` | **added** — the whole menu bar, see `menus.md` |
| "Choose **Research** on the **Commands** menu" | menu `0x6d4` | **added** |
| "choose **Tutorial** from the **Help** menu" | `0x9c5` | **added** |
| "hit the **Split All** button in the Fleet Composition tile" | `FFleetSplitAll` (`1038:3a00`) | **added** |
| "right click on the **blue diamond** … select **QuikDrop**" | `vrgZip`, `ZipOrderDlg` (`1080:0175`) | **added** |
| "hit the **Split** button" — a dialog for moving ships one at a time | `TransferDlg` in ship mode | **added** — see `cargo-transfer.md` |
| "Click on the **Xfer** button … drag in the Colonists gauge" | `TransferDlg` | **added** — see `cargo-transfer.md` |
| "Click and **drag in the fuel gauge** in the Other Fleets Here tile" | | **added** |
| "Click in various places in the **Summary pane** to get popup explanations" | | not done |

`SelectAdjFleet` is worth stating exactly, because two of those entries are
it: with a non-zero step it walks **your own** fleet list — never somebody
else's — finds where the selected fleet sits, moves by the step and **wraps**,
past the end to the first and before the start to the last. With a step of
zero it is Goto: select that fleet and nothing more. It also recentres the
scanner on what it lands on, which this frontend cannot do.

The fleet pane's buttons are `rghwndBtn`, wired in `ShipCommandProc`
(`1050:2640`):

| index | button | what it does |
|-------|--------|--------------|
| 0 | `Xfer` | transfer with the fleet chosen in the fleets-here tile |
| 1 | `Goto` | `SelectAdjFleet(0, id)` — take command of it |
| 2 | load all | transfer everything off it |
| 3 | `Goto` | the planet the fleet is orbiting |
| 4, 5 | `Prev`, `Next` | walk your own fleets |
| 6 | `Rename` | dialog `0x7e3` |
| 7 | `Xfer` | with the planet |
| 8 | `Jettison` | |
| 9 | `Split All` | |
| 10 | `Merge` | dialog `0x52` |

## The blue diamond

The single most-used control in the tutorial's later pages, and it had no
equivalent here. `DrawShipWayPtOrders` (`1050:0912`) draws it with
`DrawDiamond(hdc, rc, hbrBBlue)` beside the Transport cargo table and
remembers where it put it in `rgrcRef[5]`, which is what makes it a click
target. Right-clicking raises a menu of **saved cargo orders**:

| entry | string | what |
|-------|--------|------|
| `QuikLoad` | `0x238` | load everything |
| `QuikDrop` | `0x23a` | unload everything |
| four slots | `0x4be` `<Unused %d>` | whatever has been saved into them |
| `<Customize>` | `0x4c0` | opens `ZipOrderDlg` |

What the two built-in ones do is not written in the code in so many words;
the tutorial says it instead — *"select QuikDrop to empty the freighter's
hold at 90210"* — so QuikDrop unloads everything and QuikLoad is its
opposite.

`ZipOrderDlg` (`1080:0175`), captioned `Customize Zip Orders` (`0x231`), is
four radio buttons naming the slots, a painted list of what the chosen one
holds, and three buttons: **Import** (`0x816`) copies the current waypoint's
cargo table into the slot and asks for a name through the rename dialog
(`0x7e3`), rename (`0x41b`) does the naming alone, and **Delete** (`0x817`)
empties the slot. An unnamed slot is called `Custom n`.

`vrgZip` is four slots of `0x18` bytes — a validity byte, the same five
`ITEMACTION` words a Transport waypoint carries, and a name kept beside them
at `0x526e + i * 0x18`. The production tile has a diamond of its own over
`vrgZipProd`, which this project already draws.

Neither is in a save. Both live in `[ZipOrders]` of `stars.ini`, the orders
under `ZipOrders1` to `ZipOrders4` and the templates under `ZipOrdersP1` to
`ZipOrdersP5`, and both are **nibble-coded into the letters `a` to `p`** —
four letters a word, so that the whole thing is printable ASCII.

The two codings are not the same, which is worth knowing before writing
either. A template's four letters are the word's nibbles, plain, from the
bottom up. A zip order's put the **action first** and then the quantity
from its bottom nibble up:

```text
p[0] = bits 12..15   the action
p[1] = bits  0..3    the quantity, low nibble
p[2] = bits  4..7
p[3] = bits  8..11
```

`ReadIniSettings` takes an order only when the value is at least twenty
characters and under thirty-three and the first twenty are all in `a`–`p`;
twenty exactly is five words and no name. A template needs its second
character to be a count under thirteen, that many words after it, and a
name under thirteen characters.

One quirk at the end of the template loop: slot 0 is read like the rest and
then **thrown away** — the routine finishes by forcing its name to
`<Default>` and marking it valid, because that slot is the player's own
default queue and lives in the save. The original writes `ZipOrdersP1` all
the same, so this project writes it too and a file it writes has the same
shape as one the game wrote.

## The reports, and their sort

`SortReportCache` (`1108:589c`) knows **four** reports — planets, your
fleets, everybody else's fleets, and battles — and each keeps `icolSort`,
`iSubsort` and `fAscending` of its own.

Two things fall out of it. The first is why all four Report entries carry
F3: the text after the tab is drawn rather than bound, and the key has an id
of its own (`0x8fe`) that `CommandHandler` turns into whichever report comes
next — nothing, planets, your fleets, everybody else's, battles, nothing
again. That is reproduced; see `menus.md`.

The second is not, and is worth writing down before anyone builds the tables:
when you sort by a new column the routine does not simply forget the old one.
It copies the current column, subsort and direction into `vicolSortPrev`,
`viSubsortPrev` and `vfAscendingPrev` **before** overwriting them, so the
comparator can break ties with whatever you sorted by last. Sorting by
population and then by name leaves equally-named planets in population order.

All of that is done, and written up in `reports.md`: the reports are column
tables now, the menu is the menu, and the three pages that ask about the
sort ask it again.

They ask for different things, which is worth recording because it shows how
much of the arm is emphasis rather than gate. Page 46 — *"Choose Planets...
from the Report menu"*, then *"Click on the title of the Value column and
Sort by Value"* — latches as soon as **any** report is open, so the sort
only chooses which paragraph is emboldened. Page 59's *"sort by
Population"* latches on `icolSort == 2`, so it really does have to be done.
Page 55 asks for all three of column, direction and mineral:
`icolSort == 0xb && !fAscending && iSubsort == 3`.

One caution about page 46's wording. The spec used to say it asks for
population; it asks for **Value**, which is column 4. The page that asks for
population is 59.

## The tutorial's own world

`CreateTutorWorld` (`1078:5e5e`) turns out to hand-place nothing. It fills in
a `GAME` by hand and calls `GenerateWorld` like any other new game, so
everything about the tutorial's galaxy comes from these settings and one
seed:

| field | value |
|-------|-------|
| `cPlayer` | 2 |
| `mdSize` | 0 — tiny, 400 light years |
| `mdDensity` | 0 — sparse |
| `mdStartDist` | 1 — close |
| flags | `0xe8` |
| `lid` | `0x008cef49` |
| `rgvc[7]`, `rgvc[8]` | `0x80`, `0x81` |
| seed | `Randomize(0x499602d2)` — 1,234,567,890 |

Two things in that table are worth reading twice. The flag word `0xe8` is
bits 3, 5, 6 and 7 — **tutorial**, BBS play, visible scores and **no random
events** — and bit 2, single-player, is *not* among them, so a game with one
human must pick that up later. And the **seed is not the game id**: almost
every game seeds its generator from its own `lid`, and this one calls
`Randomize` with a constant unrelated to it.

Player 0 is the default race named `Humanoid`; player 1 is a computer player
named `Berserker`.

### The galaxy **is** the original's

Reproduced and checked. `crates/stars-core/tests/tutorial_seed.rs` generates
from these settings and this seed and compares the result against
`fixtures/games/tutorial/tutorial.xy`, the galaxy the original makes: all 24
planets match, coordinates and names both.

So the pages mean what they say. Planet `0x0c` is **Prune**, `0x0d` is
**Stove Top**, `0x10` is **90210**, `0x0f` is **Alexander** — the worlds the
early pages send you to by name. A test pins those four, so a change to the
generator that quietly moved them would fail rather than leave the tutorial
sending players to the wrong stars.

This was written up as impossible when the generator was built, on the
grounds that the original's `qsort` leaves the order of equal x coordinates
unspecified. The reasoning was sound and the conclusion was not: unspecified
by the standard is not undetermined in a statically linked binary. What made
it look impossible is that the tutorial is the **only** galaxy whose seed can
be known — a `.xy` stores its settings but never its seed — so there was no
oracle to test against until the tutorial's world was built.

## The Hint button: `tutor.idh`

**Hint** (`0x76`) is `WINHELP(hwnd, szHelpFile, HELP_CONTEXT, tutor.idh)`
(`TutorDlg`, `10f8:01bd`; `PanicDlg` the same at `10f8:0311`). Nothing
sets `idh` for a page as such: it is left behind by the checks. Every
`FCheck*` helper saves it on entry, sets its own topic while it is
failing, and puts the saved value back as it passes — so after an arm has
run, `idh` holds the topic of the check that stopped the chain, the one
whose paragraph is emboldened. Several helpers end by asking
`FCheckSelection` about the fleet or planet concerned and take *its*
topic if that fails too, so the page on selecting things wins whenever
the object is not in hand.

What each helper leaves, read off the decompilation (all are context
numbers of `STARS!.HLP`, `help.md`; the titles are the file's):

| helper | topic | title |
|--------|-------|-------|
| `FCheckSelection` (`10f8:6af4`) | `0x36b1` when what is wanted is what the message in front points at; `0x5ea` when a planet is wanted and a fleet at it is in hand; `0x5e6` when a fleet is wanted and its planet is in hand; else `0x5f6` | Messages Pane / Location Tile / Fleets in Orbit Tile / Selecting an Object to Command |
| `FCheckSummary` (`10f8:69e2`) | `0x36d2` | Key to the Scanner |
| `FCheckMessages` (`10f8:6c48`) | `0x36b1` | Messages Pane |
| `FCheckFleetWP` (`10f8:6df4`) | `0x5f6` no such fleet; `0xbf6` waypoint not there; `0xbf7` wrong place; `0x5ef` wrong task; `0x5ee` wrong warp — then the selection's | … / Adding Fleet Waypoints / Moving Fleet Waypoints / Waypoint Task Tile / Fleet Waypoints Tile |
| `FCheckColonizeWP` (`10f8:70c0`) | `0x5f2`, unless the cargo check at the home world or the waypoint check under it fails | Colonize |
| `FCheckXferWP` (`10f8:7280`) | the waypoint's, else `0x5ef` for the wrong actions, then the selection's | Waypoint Task Tile |
| `FCheckLayingWP` (`10f8:6ff4`), `FCheckPatrolWP` (`10f8:71ac`) | `0x5f8`, `0xc17`, the waypoint's under them | Lay Mine Fields / Patroling |
| `FCheckCargo` (`10f8:7664`) | `0x433`, then the selection's | Cargo Transfer Dialogs |
| `FCheckQueue` (`10f8:7442`) | `0x5e3` in the first two years, `0x423` after, then the selection's | Production Tile / Production Dialog |
| `FCheckResearch` (`10f8:6da4`) | `0x42e` | Research Dialog |
| `FCheckScanner` (`10f8:685c`) | `0x36b8` for the view, `0x36c6` for the zoom | Choosing Your View of the Universe / Zooming |
| `FCheckPlanetRoute` (`10f8:6f86`) | `0x5fb` | Route |
| `FCheckShipBuilder` (`10f8:7964`) | `0x42a` | Ship Designer |
| `FCheckBuilderPart` (`10f8:77d8`) | `0x42a` designer shut; `0xbe0` up but not editing; `0xbdf` wrong part | Ship Designer / Editing an Existing Ship Design / Designing a New Ship from Scratch |
| `FCheckZip` (`10f8:6460`) | `0x44a` slot empty with the dialog up, `0x5f0` with it shut; `0x5ef` wrong actions | Custom Zip Orders dialog / Transport / Waypoint Task Tile |
| `FCheckTemplate` (`10f8:666e`) | `0xc2d`, set even as it passes | Production Templates |
| `FCheckBtlPlan` (`10f8:760a`) | `0xc21` | Changing the Contents of a Battle Plan |
| `FCheckFleetName` (`10f8:690a`) | `0xbee` | Naming Fleets |

The arms that read the game directly set a topic outright where they
need one: `0x5ee` (Fleet Waypoints Tile) around Repeat Orders, `0x5ed`
(Other Fleets Here Tile) for the fuel page and for picking a fleet out at
a planet, `0x5ec` (Fleet Composition Tile) for a split or merge to start
and `0x453` (Merge Fleets dialog) once it has, `0x1771` (Keyboard
Shortcuts) for page 3's fleet and for the design-count pages, `0x5e3`
before the queue-length reads of pages 23 and 25, `0x423` before those of
pages 22 and 33, and `0x3e9` with the designer shut on pages 56 and 63 —
a number the file has **no topic for**, so the original's Hint puts up
WinHelp's *topic does not exist* there. So does `AdvanceTutor`'s `0xdb6`
(`10f8:0aff`), set while a page waits for the turn.

`App::tutor_help` computes the same: the emboldened rung's own topic
(`Stage::help`) where the arm sets one, else the check's by the table
above (`App::tutor_check_help`), `0xdb6` while waiting; `Tutor.help`
keeps the last value, as the global does. `tests/tutorial_hints.rs`
follows the first pages and checks every named topic against the file.

## What this project does

The text and the segment reader; the `Tutor` state; the `Check` vocabulary and
eleven of the fifteen verbs; the chain-of-rungs page model; and
`AdvanceTutor`'s skipping loop, which steps on by eight while the page's task
is done so a page satisfied in advance is never shown.

**All eighty pages** are transcribed, years zero to thirty-six. Pages not yet transcribed are simply
absent from the table and the tutorial stops at the first gap rather than
pretending to know what comes next.

And, from reading the text as a specification: `Prev`, `Next` and `Goto` on
the fleet pane's tiles, `Goto` on the location tile, and the `n`, `q` and
`F9` keys.

### Playing it through the panes

`crates/stars-ui/tests/tutorial_ui.rs` plays year zero through the interface
itself: every frame laid out by egui as the shell lays it out, each step a
press on a button where the pane drew it or a shift-click on the map where
the scanner drew the planet. Every pane button records itself as it is drawn
— caption, pane, rectangle, whether it was enabled and whether it lay wholly
inside its clip rectangle (`App::drawn`, `views::placed_button` and
`flow_button`) — and the scanner records where it put the map
(`App::map_frame`), which is how the test finds "Next" and Prune.

Its first run found two buttons the pages name that could not be pressed:
the fleet tile's Next, below the tile's foot, and the Fleets in Orbit tile's
Goto, likewise; and one planet that could not be reached, because the map
did not scroll. See `fleet-pane.md` and `scanner.md`.

### How far it plays

`crates/stars-ui/tests/tutorial_walkthrough.rs` plays the tutorial from its
pages, through the same calls the panes make, and checks that every task
turns the page as `AdvanceTutor` would. `tutorial_ui.rs` plays it through
the panes themselves — every step a press on a button where a pane drew it,
a pick from a menu or dropdown where it opened, or a click on the map where
the scanner drew the planet — from the first page to the last; see
*Through the panes* and *The run, to the end* below. The first four years
took three corrections to what was here:

* the five opening messages, above;
* `FCheckCargo`'s figures, which had been transcribed as 25kT for every
  fleet. Reading the fifteen call sites again: a Santa Maria's hold is 25,
  a Teamster's is **210**, the two Santa Marias of page 26 are 50 and the
  three merged Teamsters of page 67 are 630;
* `FCheckMessages` with `fFilter` tests `bitfMsgFiltered` and nothing else —
  it does not ask whether a message of that kind is in this year's list —
  where this project had required one to be present, which would have
  stalled page 13 for ever in a year with no factory message.

And one to the client: `IWarpBestForWaypoint`'s rule for the warp a new leg
gets (`scanner.md`), without which the colony ship crawled to 90210 at warp
6 and page 14 found the planet still unowned.

The years after that were the turn engine's to catch up on, and each
needed something the pages take for granted — recorded in the commits
that added them, and in `../formulas/` where a formula came out of it:
accelerated BBS play (the tutorial's home worlds start with 100,000
colonists); the year's messages — factories and mines built, a queue
worked through, a ship built, a colony ship dismantled where it lands and
the colony declared; auto-build leaving its part-built unit as an entry of
its own; new ships as fleets of their own with full tanks; `FuelFleets`;
the previous leg's task copied onto a new waypoint; and fuel staying
aboard where there is no starbase to take it.

Page 26's **Split** showed that a
split-off fleet must take a **copy of every order** the old one had
(`LpflNewSplit`, `1038:3372`): the page expects the lone Santa Maria to be
bound for Slime with Colonize already set, not sitting at Wallaby with a
fresh single waypoint.

Pages 32 and 33 are where `FCheckTemplate` gets real. Its table is not in
the data segment at all but in the tutor's own code segment, `10f8:663a`:
two `ZIPPRODQ1` records, and the default template must match one **exactly**
— the no-research flag, the count, then every packed entry word. Page 33
also caught the economy out: Stove Top's queue is meant to hold a
part-built factory in front of the auto-build order, and it did not,
because the home world had spent its last germanium. In the tutorial a
factory costs **two of every mineral**, not four germanium — `gd` bit 11,
which `StartTutor` sets — and with that the shortage moves to ironium,
which is the one the Teamster is hauling. That Teamster, in turn, strands
itself on the way home with 210 kT aboard and a warp chosen while it was
empty; the original's answer is `MoveFleets`' drop to the fastest free warp
(`../formulas/movement.md`, *Running dry*), so it creeps on at warp 1.

### Through the panes

`tutorial_ui.rs` found what the App-level walkthrough could not, because it
must press what the text names:

* the Research dialog had **OK** and **Cancel** where the original has
  **Done** and **Help**, and its window grew every frame until the buttons
  were off the screen (`research.md`);
* the Production dialog's list boxes padded their rows to six, so
  **Factory**, the seventh row, was below the fold — the original's 84
  dialog units are ten rows of text;
* every year from 2402 opens on a message's **Goto**, so the year's news
  must be there to press: a fleet's arrival (`0x4e`), the empty queue
  (`0x3e`, every year it is empty), and the client's own "found a planet"
  messages, one per planet first seen, whose Gotos are how pages 9 to 11
  put Prune, Alexander and 90210 in the Summary pane;
* the tutor window floats over the map's corner, and a planet under it
  cannot be shift-clicked until the window is dragged aside — which the
  test does, as a player would;
* page 12's **Xfer** button and the Cargo Transfer dialog behind it did not
  exist (`cargo-transfer.md`), and the location tile had a line of this
  project's own where the original has only its two buttons;
* the Waypoint Task tile spread the Transport table over five rows where
  the original has a **cargo** dropdown and an **action** dropdown — "the
  tile's second dropdown … and its third", page 35 — and the blue diamond
  beside them, which had been pushed off the tile;
* a row past the tenth of a list box is below the fold, in the original
  too, so the harness rolls the wheel to it as a player would; and its
  clicks are a second apart, or egui takes every one for the third of a
  triple and no double-click ever registers;
* the Fuel & Cargo tile gave its figures as text where the original draws
  two gauges and figures, and a press in either gauge is Xfer (page 19);
  and the toolbar's buttons were not pressable by name;
* a message's Goto matched a fleet by number alone and, on page 19, landed
  on the **Berserkers'** fleet with that number; a Transport task said
  nothing when it loaded or unloaded, where the original reports each kind
  moved (`0x2b`–`0x2e`), which is the message page 20 Gotos the Teamster
  from;
* the Fleet Waypoints tile had no **Repeat Orders** checkbox and no list of
  the waypoints (page 22, page 17); its figures had two location rows where
  `DrawShipOrders` draws one; and every tile's rows were egui's eighteen
  pixels where the original's are a line, so the tiles the table sizes for
  a line a row overflowed;
* the Production dialog's lists kept their scroll position from one opening
  to the next, so a list rolled down for 90210 opened rolled down for Stove
  Top and page 25's double-click on Armed Probe fell on the wrong row;
* page 26's **Split** button opened nothing; the Ship Transfer dialog is
  the Cargo Transfer template with a row per design (`cargo-transfer.md`);
* page 28 switches off the message that the Teamster **unloaded** at Stove
  Top, and there was none, for two reasons in the engine: the Prune miner
  had never mined, because a fleet's `warp` stayed set after it arrived and
  the *here all turn* test read that instead of whether the fleet moved
  this year (`MoveFleets` clears it; the engine now keeps the set of fleets
  that moved and asks that); and the Teamster had "loaded" 210 kT from a
  planet with nothing on it, where `XferSupply` gives only what the planet
  has. With both, Prune yields its 7/3/9 a year and the Teamster carries
  and unloads it, and page 28 has its message;
* page 27's drag from Slime to Sea Squared needs both on the screen, which
  at the opening zoom they are not; the harness takes the map down a step
  first, as a player would, and drags at the scanner's own grab reach;
* page 30 opens on a message whose Goto is the **Research dialog**, and
  the window Gotos (`-2` to `-7`, `message-pane.md`) were all dead here;
  the message itself, `DoResearch`'s level-gained report, was never sent;
* page 30 switches off the **robots' haul** — a Transport load at an
  unowned planet where a fleet of yours with mining robots sits is reported
  as `idmHasLoadedMiningRobotsWorking`, not `idmHasLoaded` — and there was
  no such load to report, twice over: a split fleet took **no cargo and no
  fuel** with it (`FleetTransferCargoBalance` was not run on the ship
  transfer, so page 26's two colony ships flew with 600 mg and 50
  colonists in one 25 kT hold), and **Repeat Orders** did nothing, so the
  Teamster never went back to Prune (`KillUsedWaypoints` puts the
  waypoint reached back at the end of the route);
* page 32's blue diamond sat under the leftover checkbox, because the
  diamond was placed by egui's small line height where the template's
  controls are placed by dialog units; sized by the same units it sits its
  seven pixels clear;
* page 32 turns on the **template** (`FCheckTemplate`), not on the
  Customize box opening, so Import is what turns it — the text of page 33
  is read after the fact, as in the original.

### A hint decorates the gate it stands before

The arms are nested: `if (queue done) { ask about the freighter } else {
bold by the message and the selection }`. A hint therefore only matters
while the gate that follows it is unpassed. Page 14's "Goto 90210" is asked
about only while 90210's queue is unfilled; the step table used to check
every rung from the top, so picking Teamster #4 — once the queue was done
— sent the bold back to "Goto 90210", 90210 no longer being selected.
`App::tutor_rungs` is the window the page is on: from the rung after the
last gate passed up to the first gate not yet passed. Within a window an
`Any` check reads a nested `else` as one rung (page 31: the Oxygen
paragraph only while neither Oxygen is shown nor the ship is in hand).

`tutorial_ui.rs` now checks, at every press, that the bold never goes
back up a page.

### Bit 10: what has been seen

Beside the wait bit the original keeps **bit 10** of `tutor.fVisible`:
*something the page was watching for has been seen*. The arms set it as a
summary they wait on first comes up (pages 25 and 33, `| 0x400`), the
panes set it on their own — `FinishProduction` (`10d0:11ed`) when a queue
is OK'd, `PopupWndProc` (`10c0:00cd`) when a pop-up opens, `VCRDlg`
(`10e8:1879`) when a battle is played, `PlanetWndProc` (`1048:0c55`) on a
click in the pane — and seventeen arms read it instead of asking again.
`AdvanceTutor`'s loop clears it with each page turned. `Tutor::seen` is
the bit, `seen()` a rung that stays passed once it has passed, and
`App::tutor_note_seen` what the panes call. Page 28 in the original
gates its Research step on the bit — the pop-ups the page invites you to
open — and page 25 gates on the Berserker scout having been looked at;
neither is gated here yet, the summary pop-ups being undrawn and the scout
unflown.

### Page 4, read the other way up

Most arms of `FTutorTaskDone` are a chain of *if this is done, embolden
the next paragraph and ask the next thing*. Page 4's (`10f8:0fbc`, the
`idt == 0x18` arm) is written upside down: fleet 4 in hand, else 29 for
fleet 3, 27 for fleet 2, 25 for anything else — the paragraph follows
whichever fleet the reader has reached with the tile's Next. The step
table had only "25 until fleet 4", so the bold never moved and nothing
told the reader they were getting anywhere. A rung can now be **held**
(`mark`): emboldened while its check holds, gating nothing and asking for
nothing.

### The screen the test sees

`views::frame::game_screen` draws the game screen — the panes down the
left, the dialogs over the map, the map, and the tutor's ring last, once
everything that records a widget has drawn — and both the desktop and
`tutorial_ui.rs` call it, so what the test presses is what the player
sees. The harness used to lay the panes out its own way, with the planet
and fleet panes side by side and wider than the desktop's one pane, and
so could see and press things a player could not: the first film made
that plain. Two things came of drawing the real screen: the pane's width
(`planet-pane.md`, *The pane's width*), and a pane that **scrolls to the
ringed widget** when it is drawn out of view (`views::note`) — the
original's panes hold every tile at once, and a page that wants Xfer
pressed is no help with Xfer below the fold and nothing to say so.

### Filming the run

The same harness can film itself. With `STARS_TUTORIAL_VIDEO=<path>` set,
every frame's shapes are tessellated and rasterised in software — a
triangle at a time, vertex colours and textures blended as egui's shader
blends them — and appended raw to the path; the game's own pictures and
text are loaded when a copy of the original is under `binary/`. Run it in
release, since two thousand frames of 1920 by 1080 are drawn on the CPU:

```sh
STARS_TUTORIAL_VIDEO=/tmp/frames.rgba cargo test --release -p stars-ui --test tutorial_ui the_tutorial
ffmpeg -f rawvideo -pix_fmt rgba -s 1920x1080 -r 12 -i /tmp/frames.rgba \
       -c:v libx264 -crf 23 -pix_fmt yuv420p tutorial.mp4
```

Each page turn is held for two seconds so it can be read. The raw file is
large — eight megabytes a frame — and is not kept.

### Watching it play

The driver and the pages live in `stars_ui::autopilot` (`Shell`, the
`Sink` its frames go to, and `script::tutorial`), so the same run can be
watched as it happens rather than filmed: the desktop's
`autoplay_tutorial` example plays the script on a thread at a human pace
— eight tenths of a second a step by default, `STARS_AUTOPLAY_DELAY_MS`
to change it, three times that on a page turn — rasterising every frame
into a window. Nothing is clicked; the run stops where the walkthrough
stops and the status line says so.

```sh
cargo run --release -p stars-desktop --example autoplay_tutorial
```

### The halo

One thing here the original does not have, asked for rather than found: a
**pulsing ring** around whatever the page would like pressed next. The
original's only pointer is the emboldened paragraph. `App::tutor_target`
reads the check the page is waiting on (`tutor_pending`, the first stage
not satisfied) and names the nearest thing to do about it — the message
pane's **Next** while messages are unread and its **Goto** when the
message in front points at what the page wants, the fleet tile's **Next**
or **Prev** when another fleet of yours is in hand (the walk pages 3 to 5
name), the planet or fleet on the map otherwise, **Change** for a queue
and then the row, **Add** and **OK**
inside the dialog, **Xfer** and the gauge for a hold, the Waypoint Task
dropdown for a task — as a drawn widget by the name a pane recorded it
under, or a point on the map. `views::tutorial::halo` finds where that was
drawn this frame and rings it on the tooltip layer, swelling and fading
over a second and a half. The menu bar's four game menus are drawn by
`views::menubar` and recorded under `"menu"`, so a key's job has a ring
too: the Turn menu, then Generate, when the year is done; the Commands
menu, then Research…, when the Research dialog is wanted. Where nothing
on screen answers — a dialog with no button into it — there is no ring. `tutorial_ui.rs` checks
that the ring sits on what it is about to press at the start of pages 1,
2, 4, 6 and 12 — and, before **every** press, that the ring is somewhere
whenever the page is waiting on something, with two exceptions it names:
another player's fleet that is not in view, and a fleet of ours that a
pane cannot reach.

### The run, to the end

The walkthrough now plays **all eighty pages**, 2400 to 2436, and the
Generate that ends the tutorial; `crates/stars-ui/tests/tutorial_ui.rs`
asserts the tutor finished. The pieces that were wanted for the years
after 2410 are in — the player's own view of the galaxy
(`stars_core::visibility`), the Berserkers' turn (`../formulas/ai.md`,
*The TurinDrone turn*), `DoBattles` (`../formulas/combat.md`, *The battle
around the board*) with the Battle VCR, a fleet's chase after another
(`../formulas/movement.md`, *Chasing a fleet*), the parts a level of
research brings (`../formulas/research.md`), the designer's editor laid
out as the original's (`ship-design.md`), the Merge Fleets dialog, the
warp gauge and Merge with Fleet's choice of fleet (`fleet-pane.md`), a
planet's route and the leg a new ship takes from it
(`../formulas/movement.md`, *A new ship follows its planet's route*), and a
planet given up when its people are gone (`../formulas/population.md`).

What the harness cannot do is make this world the original's. The
tutorial's pages name the ships the original's game built, by number, and
its computer player's fleets where the original's generator put them;
this engine's game is its own from the moment its turns run, and from
2411 on many a page's rung names a fleet that is not here, or is here
under another number:

* **Stove Top's ironium.** Page 35 goes on to *Teamster #12* (fleet 11),
  which the original's Stove Top built in 2410 alongside Santa Maria #3.
  Here it had 33 kT of ironium against the Teamster's 34 and the Santa
  Maria's 27, so only the Santa Maria came, and the Teamster is still on
  the ways in 2413 with page 40's two behind it and 4 kT on the surface.
  The economy matches the original's turn-3 file to the kilotonne in
  population, positions and fuel and to within three in minerals
  (`tutorial.m1`: 390 / 458 / 609 against 393 / 458 / 606); ten years of
  factories at two of every mineral (`../formulas/production.md`) turn
  that into a Teamster's worth. Where the last thirty kilotonnes go is
  not found: every figure the fixtures give up to 2403 agrees —
  `tutorial.h1`'s resources of 125, 152 and 181 are this engine's 115 + 10,
  132 + 20 (before the Santa Maria's colonists came aboard) and
  149 + 30 + 2 for Shaggy Dog; the Cotton Picker's two Robo-Mini-Miners
  dig Prune's 87 % ironium as eight mines (`CMineFromLpfl`, `1080:25d2`:
  count times rating, and the Mini-Miner rates 4), seven kilotonnes a
  year, which Teamster #4 brings home fourteen at a time — and nothing
  the fixtures hold reaches past 2403. The three kilotonnes of 2403 are
  themselves a small puzzle: with the home-world floor of 30 on the yield
  (`../formulas/mining.md`) ironium comes at 3.0 a year, no roll, which
  makes 393 and not the fixture's 390; germanium's 8.4 would have to roll
  long all three years for 609. Read without the floor, ironium's 2.5
  rolled short three times fits — but the manual, the mineral popup's
  own wording and the corpus all say there is a floor, so it stays.
* **The red triangle below Hiho.** Page 34's enemy scout is thirteen light
  years short of Hiho here, still on its way in 2411, and page 37's fight
  happens a year late, in 2412 — where the chase catches it. The
  Berserkers' scouting draws on the generator, and the original's draws
  are not reproduced (`../rng/prng.md`).
* **Long Range Scout #2** still has a leg to fly in 2411, where the
  original's had finished; the test deletes the leg and lays the Scrap
  order the page asks for, as a player would.
* **Page 38's salvage.** With the fight a year late there is no salvage
  under Armed Probe #9 in 2412, so the page's hint to look at it is
  passed over.
* **Page 40's Teamsters.** The page wants them at the second slot, behind
  page 36's seventy mines; here they join the Teamster of 2410 at the
  head of the queue, so the rung is not seen done. The year is generated
  regardless — the page's last line says to — and the tutor turns to
  2413's page as it would for any page left standing when the year ends.

* **Page 43's queue.** The page wants the Mini-Miner right behind the
  hundred mines; the original's Stove Top had nothing else queued, this
  one still has a Teamster of page 40's on the ways, so the harness moves
  the Mini-Miner up past it with Item Up, as a player would.
* **The freighters of 2415 to 2417.** Page 46's *Teamster #7* and page
  47's *Teamster #12* are ships the original's Stove Top had by then;
  here the Mini-Miner took 2415's ironium and the Teamsters came in
  2416 — as fleets 6 and 2, *Teamster #7* among them, a year late — so
  the pages' freighter rungs are passed over and the years generated. The
  Mini-Miner is fleet 1 here, *Mini-Miner #2* in the number Long Range
  Scout #2 left, where page 48 names fleet 7; its merge with the Cotton
  Picker is ordered all the same, and happens.

* **Research a level behind.** Page 53's Stargate 100/250 wants
  Construction 5 and page 57's Frigate Construction 6, with the Mine
  Dispenser 50 asking Energy 2 and Biotechnology 4; the original's player
  had them by 2420 and 2422 and this one, Propulsion having run since
  2413, has not. The designer is opened as the page says and the design
  left for a year the parts are there.
* **Teamster #4's fuel.** Page 56 has the freighter take fuel from the
  Cotton Picker at Prune; here it had already left, and ran dry a light
  year short of home with the bigger load the Mini-Miner digs — as the
  page says it would — with nobody beside it to take fuel from.

* **Stalwart Defender #5's chase.** Pages 62 and 66 send the destroyer
  after Berserker colony ships bound for No Vacancy and back to Wallaby;
  here it stayed at Wallaby and fought them there, one in 2424 and one in
  2426, so it has nowhere to go back to and no ship left to chase.
* **The ships' numbers from 2423.** The Mini-Miner of 2424 is fleet 2,
  the three Santa Marias of 2426 fleet 7, split into 2, 7 and 9 — where
  the pages name 9, 10 and 11 — so the pages' colony-ship rungs are not
  seen done though the orders are laid; the merge of Teamster #3 into
  Teamster #4, and the Mini-Miner's into the Cotton Picker, happen as
  the pages have them.

Where a page's rung cannot be reached in this world the harness does
what the page asks all the same, says why, and generates the year anyway
(`Shell::generate_anyway`), and the tutor carries on with the year — a
page left standing when the year ends counts as done, which is what the
original does for a player who generates with a page unfinished. From
2427 the ships all have other numbers than the pages' — the destroyer is
the eighth design, not the ninth, the Mine Layer never having been drawn,
and the bombers the ninth — so the last dozen pages are played by their
sense: the destroyer to the page's recipe and ten of them to Hacker,
Teamster #4 slowed to warp 5, Stove Top routed, Teamster #7 scrapped, the
bomber designed and twenty queued, Wallaby's mines, and every message read
to the end. Getting 2413 to play brought four things out of the original:
fleet numbers start at **zero** (`LpflNew`, `1038:300c`), which is how
the Teamster Stove Top builds in 2413 is *Teamster #1*, in the number
Armed Probe #1 left; a level of research reports **each part it brings**
(`../formulas/research.md`, *What a level brings*), which is the
Robo-Miner message page 41 reads and the Privateer one page 44 does; a
fleet that ends a year standing on a planet is in orbit of it
(`../formulas/movement.md`, *Chasing a fleet*), which is where page 44
finds Armed Probe #9; and "read your last message" (`FCheckMessages`
with 9999) is done when Next has nowhere to go, filtered messages not
counting. The designer's editor was also laid out as the original's
(`ship-design.md`), the parts list having been drawn where the schematic
needed to be. 2414 to 2418 brought four more: page 45's arm reads the
other way up, like page 4's — the paragraph follows whichever of Sea
Squared and Oxygen is in hand (`10f8:3a0c`); the Customize box is a
dialog over the production dialog, drawn on a layer of its own, where
its OK had been sharing a spot with the leftover-research checkbox
underneath (which was page 40's mystery); the Planet Summary Report is
a window over the map in the shell as on the desktop, its headings and
sort menu recorded for the ring; and Merge with Fleet picks its fleet
(`fleet-pane.md`). 2419 to 2422 added the Customize Zip Orders box's
widgets and its OK, the report menu's submenus, and a ring for a fuel
check; and a page whose own task cannot be finished here — page 56's
fuel — is played around with `Shell::off_the_page`, which lets the bold
follow what is in hand without the harness calling it a step backward.
2423 to 2426 added the fleet tile's **Merge** button and the Merge
Fleets dialog (`fleet-pane.md`), a ring for an edited design, the map
zoomed out when two planets will not both fit on it, and the view of the
fleets recomputed after an order that changes the fleet list — a merge
drops fleets from it and every fleet after them moves up, which had left
the map's idea of what was in view pointing at the wrong fleets. 2427 to
2436 added the warp gauge, a planet's route by control-click and the leg
a new ship takes from it, a task on a fleet's own waypoint, a load of
fuel capped at the tank, a freighter's colonists put down on a planet not
its owner's settled as a landing, and a planet with nobody left given up.

Two smaller things came out of getting to page 34 all the same: a waypoint
aimed at a fleet holds the fleet's **full object word**, owner and all,
which is how page 34 can ask for `0x200`; and a new leg is priced together
with the legs before it.

Still to come here: `FCheckLayingWP`, `FCheckPatrolWP`, `FCheckBtlPlan` and
`FCheckFleetName`; the help topic each check sets on failure; and the two
restarts the Panic! dialog offers.
