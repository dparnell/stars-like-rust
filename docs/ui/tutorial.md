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
| "hit the **Split** button" — a dialog for moving ships one at a time | | not done: split from the Fleets screen |
| "Click on the **Xfer** button … drag in the Colonists gauge" | `TransferDlg` | not done |
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
the scanner drew the planet — and so far gets through **2400 to 2403**,
pages one to sixteen; see *Through the panes* below. It gets through **2400 to 2410** —
pages one to thirty-three — and stops on page 34. The first four years took three corrections to
what was here:

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

Page 26's **Split** goes through `App::split_fleet` and `split_all`
directly — the Split dialog itself is not drawn yet — and showed that a
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
  triple and no double-click ever registers.

### Where it stops, and why

Page 34 (2411) is where the client's picture of the game stops matching
the original's, and three pieces of the engine are wanted before the
walkthrough can go on:

1. ~~**The player's own view of the galaxy.**~~ Done: `App::known_planets`
   and `App::in_view`, from `stars_core::visibility` (`../formulas/scanning.md`),
   refreshed each year. The scouts now fly at the warps the tutorial's
   turn-3 file records, the probe reaches Mozart, and the client's own
   "found a planet" messages arrive in 2402 as pages 9 to 11 expect.
2. **The computer player's turn.** The red triangle below Hiho on page 34
   is the Berserkers' scout, which their `DoAiTurn` has flown there; this
   engine's computer players do not move (`../formulas/ai.md`).
3. **Battles.** Page 37's Battle VCR needs the fight at Hiho in 2411, and
   `DoBattles` (`10f0:3a26`, inside `DoOrders`) is not in the turn yet —
   `../formulas/combat.md` has the board, movement and firing, but not who
   fights whom.

Two smaller things came out of getting to page 34 all the same: a waypoint
aimed at a fleet holds the fleet's **full object word**, owner and all,
which is how page 34 can ask for `0x200`; and a new leg is priced together
with the legs before it.

Still to come here: `FCheckLayingWP`, `FCheckPatrolWP`, `FCheckBtlPlan` and
`FCheckFleetName`; the help topic each check sets on failure; and the two
restarts the Panic! dialog offers.
