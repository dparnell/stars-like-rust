# The tutorial

Status: **in progress** — the text is recovered and read; the step machine,
the hooks and the tutorial world are not written yet.

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
| `FCheckMessages` (`10f8:6c48`) | 60 | have the messages been read |
| `FCheckQueue` (`10f8:7442`) | 35 | is *that* in the production queue |
| `FCheckSummary` (`10f8:69e2`) | 18 | is the summary pane showing *that* |
| `FCheckColonizeWP` (`10f8:70c0`) | 16 | a colonize task set where it should be |
| `FCheckCargo` (`10f8:7664`) | 15 | is the right cargo aboard |
| `FCheckXferWP` (`10f8:7280`) | 13 | a transport task set up |
| `FCheckShipBuilder` (`10f8:7964`) | 12 | a design built to spec |
| `FCheckResearch` (`10f8:6da4`) | 11 | the research settings |
| `FCheckScanner` (`10f8:685c`) | 5 | the scanner's view and filters |
| `FCheckBuilderPart` (`10f8:77d8`) | 5 | a part in a design slot |
| `FCheckTemplate` (`10f8:666e`) | 3 | a production template |
| `FCheckZip` (`10f8:6460`) | 1 | a zip order |
| `FCheckPlanetRoute` (`10f8:6f86`) | 1 | a planet's route |

That table is the argument for doing this at all: between them those fifteen
verbs read almost everything the UI can set.

## What this project does

So far: the text, and the segment reader it needs. The step table, the
fifteen verbs, the advance machine, the window, the hooks and
`CreateTutorWorld` are still to come.
