# The tutorial

Status: **in progress** — the text, the checks and the advance machine are
written; eighteen of the eighty pages are transcribed, and the hooks, the window
and the tutorial world are not written yet.

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

## The text

Every word of the tutorial is the game's own writing, so **none of it is in
this repository**. It is read at run time out of the player's copy of
`stars.exe`, exactly as the artwork is; with no copy of the game there is no
tutorial text and the frontend says so.

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
| `FCheckTemplate` (`10f8:666e`) | 3 | a production template |
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
| "Open the Planet Summary Report and **sort by Population**" | `SortReportCache` (`1108:589c`) | not done: the reports here are lists, not sortable tables |
| "select **Generate** from the **Turn** menu" | menu `0x6d4` | **added** — the whole menu bar, see `menus.md` |
| "Choose **Research** on the **Commands** menu" | menu `0x6d4` | **added** |
| "choose **Tutorial** from the **Help** menu" | `0x9c5` | **added** |
| "hit the **Split All** button in the Fleet Composition tile" | `FFleetSplitAll` (`1038:3a00`) | **added** |
| "right click on the **blue diamond** … select **QuikDrop**" | `vrgZip`, `ZipOrderDlg` (`1080:0175`) | **added** |
| "hit the **Split** button" — a dialog for moving ships one at a time | | not done: split from the Fleets screen |
| "Click on the **Xfer** button … drag in the Colonists gauge" | `TransferDlg` | not done |
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

## The reports, and their sort

`SortReportCache` (`1108:589c`) knows **four** reports — planets, your
fleets, everybody else's fleets, and battles — and each keeps `icolSort`,
`iSubsort` and `fAscending` of its own.

Two things fall out of it. The first is why all four Report entries carry
F3: it is **one** key that opens whichever report was last up, not four
accelerators for the same key. That is reproduced.

The second is not, and is worth writing down before anyone builds the tables:
when you sort by a new column the routine does not simply forget the old one.
It copies the current column, subsort and direction into `vicolSortPrev`,
`viSubsortPrev` and `vfAscendingPrev` **before** overwriting them, so the
comparator can break ties with whatever you sorted by last. Sorting by
population and then by name leaves equally-named planets in population order.

This project's reports are lists rather than column tables, so there is
nothing to click yet — the tutorial's *"sort by Population"* has no home.

## What this project does

The text and the segment reader; the `Tutor` state; the `Check` vocabulary and
eleven of the fifteen verbs; the chain-of-rungs page model; and
`AdvanceTutor`'s skipping loop, which steps on by eight while the page's task
is done so a page satisfied in advance is never shown.

Eighteen of the eighty pages are transcribed — the first five years, whole. Pages not yet transcribed are simply
absent from the table and the tutorial stops at the first gap rather than
pretending to know what comes next.

And, from reading the text as a specification: `Prev`, `Next` and `Goto` on
the fleet pane's tiles, `Goto` on the location tile, and the `n`, `q` and
`F9` keys.

Still to come: the remaining seventy-four pages; `FCheckLayingWP`,
`FCheckPatrolWP`, `FCheckBtlPlan` and `FCheckFleetName`, which the pages
transcribed so far do not reach; the help topic each check sets on failure;
the fifty-five hooks; `CreateTutorWorld`; and the window.
