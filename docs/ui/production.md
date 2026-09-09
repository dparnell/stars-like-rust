# The Production dialog

Status: **inventory, costs and queue rules recovered and reimplemented**; what
is not reproduced is named at the end.

`ChangeProduction` (`10d0:0000`) opens it and `ProductionDlg` (`10d0:1204`)
runs it, with `ProdCommandHandler` (`10d0:1994`) doing the work. The player
reaches it from the **Change** button in the planet pane's Production tile, or
by clicking a planet's Production column in the Planets report.

Two lists side by side: the **inventory** of everything this planet can build,
and the planet's **queue**. `MANUAL.PDF` chapter 7 describes the same screen.

## The controls

The dialog is **resource 93**, `Planet Production`, 294 by 191 dialog units in
MS Sans Serif 8pt, with thirteen controls — so its layout is data, not code:

| id | class | x, y | w x h | caption |
|----|-------|------|-------|---------|
| `0x416` | LISTBOX | 8, 12 | 109x84 | the inventory |
| `0x417` | LISTBOX | 174, 12 | 111x84 | the queue |
| `0x418` | BUTTON | 125, 10 | 40x14 | `&Add ->` |
| `0x419` | BUTTON | 125, 35 | 40x14 | `<- &Remove` |
| `0x439` | BUTTON | 125, 0 | 40x14 | `Item &Up` |
| `0x43a` | BUTTON | 125, 90 | 40x14 | `Item &Down` |
| `0x42d` | BUTTON | 125, 60 | 40x14 | `&Clear` |
| `0x76` | BUTTON | 125, 70 | 40x14 | `&Help` |
| `0x8b` | BUTTON | 6, 168 | 90x14 | `Contribute only &leftover resources to research` |
| `0x42e` | BUTTON | 100, 168 | 40x14 | `&Prev` |
| `0x42f` | BUTTON | 147, 168 | 40x14 | `&Next` |
| `1` | BUTTON | 198, 168 | 40x14 | `OK` |
| `2` | BUTTON | 245, 168 | 40x14 | `Cancel` |

`ChangeProduction` puts it up with `DialogBox(..., 0x5d, ...)` and nothing
moves afterwards. `0x816` — apply a production template — is not a control at
all: it comes from the diamond's menu.

**Two pairs of buttons overlap in the resource as shipped.** Every button is
fourteen units tall, so `Item Up` at `y = 0` runs into `Add ->` at `y = 10`,
and `Clear` at `y = 60` runs into `Help` at `y = 70` — four units each. That is
what is in the file: thirteen controls, no extra data on any of them, verified
by hand off the raw bytes. This project reproduces the table unchanged and then
opens the two overlaps out before drawing, because two buttons on top of each
other leave one of them unclickable.

Everything between the lists (which end at `y = 96`) and the row along the foot
(`y = 168`) is **drawn rather than placed**: the cost panel under each list,
and the blue diamond.

### The cost panel

`DrawProductionDlg` (`10d0:35dc`) draws it twice, once under each list, for
whichever row that list has selected. `Required Minerals:`
(`idsRequiredMinerals`) in Arial 8 bold on the list's own left edge, then four
rows a line apart, inset twenty pixels on each side:

| row | colour |
|-----|--------|
| Ironium | blue |
| Boranium | dark green |
| Germanium | yellow |
| **Resources** | black |

The fourth is `rgpszMin`'s **sixth** entry, not its fourth — the routine
rewrites the index as `5` on the last pass, skipping colonists and fuel. Each
label is bold in its own `rgcrMin` colour and its figure is right-aligned in
plain Arial 8 in `crWindowText`, printed with `"%ld"`; `kT` (`idsKt`) follows
the first three and nothing follows resources.

Under the **queue's** panel only, a line and a half further down, goes
`"%d%% Done,   Completion "` (`idsDDoneCompletion`) with `PszProductionETA`'s
wording after it.

Double-clicking a row in either list does what its button does: the handler
falls through from the list's `LBN_DBLCLK` into the same code.

**Prev** and **Next** move to another planet without closing the dialog —
`FinishProduction(1)` writes the queue out first, and holding **Shift** jumps
to the next planet **with a starbase** instead of the next planet.

## The inventory

`InitProduction` (`10d0:015e`) builds it, in this order:

1. **Ship designs** — only when the planet has a starbase whose hull has a
   space dock, and only designs the dock is big enough for: a design's mass
   must not exceed the starbase hull's `wtCargoMax`, which is 200kT for a Space
   Dock and unlimited for the three larger ones. Unlimited count.
2. **Starbase designs** — every one the player has, except the design already
   in orbit. One each. *A planet needs no starbase to build a starbase*
   (`MANUAL.PDF` p. 9-1).
3. **Genesis Device**, if the Mystery Trader has given it and the technology is
   there. One.
4. **Mineral packets**, all four kinds, once the planet's starbase carries a
   mass driver. Unlimited.
5. **Factory**, **Mine**, **Defenses** — each with the planet's *remaining*
   capacity as its count, so they drop out of the list when the planet is full.
6. **Mineral Alchemy**. Unlimited.
7. **Planetary Scanner**, if the planet has none and the race is not Alternate
   Reality. One — a planet builds a scanner once and it upgrades itself as
   technology arrives, which is why it is never offered twice.
8. **Terraform Environment**, as many steps as there is terraforming left.
9. The seven **auto-build** items, unlimited. Alternate Reality is not offered
   mines, factories or defences and a Claim Adjuster is not offered
   terraforming, because neither has any use for them.

Then everything already in the queue is **subtracted** from those counts, and
`FillProdSrcLB` (`10d0:3b00`) drops any row whose count has reached zero. That
is what the manual means by "when this kind of item is added to the queue, it
disappears from the inventory" (p. 7-2).

Two details worth keeping:

* The count lives in a **ten-bit** field, so anything above 1023 is stored as,
  and is indistinguishable from, "no limit". A grown planet reaches that for
  mines and factories long before it runs out of room for them.
* An **upgrade** — a starbase design on the same hull as the one in orbit — is
  labelled ` (upgrade)` or ` (downgrade)` by `PszNameProdItem`, which compares
  the two hulls.

## Adding and removing

`ProdCommandHandler`, checked with `GetAsyncKeyState` at the moment of the
click:

| held | Add / Remove |
|------|--------------|
| nothing | 1 |
| Shift | 10 |
| Ctrl | 100 |
| Ctrl + Shift | **1020** — "as many as possible" |

which is the manual's account on p. 7-3, and 1020 is what "as many as possible"
turns out to mean: three short of the ten-bit field's 1023.

**Add** puts the item **under** the selected queue row, so selecting
`— Top of the Queue —` puts it first. If the row above is the same item, the
counts merge rather than making a second entry. Removing everything from a row
takes the row out and gives the count back to the inventory.

**Item Up** and **Item Down** swap a row with its neighbour. **Clear** empties
the queue and returns everything to the inventory.

## When will it be done?

`EstimateItemProdSched` (`10d0:4f40`) answers that, and it does not estimate —
it **simulates**. It takes a copy of the planet and runs up to ninety-nine
years of the whole queue over it: mining, resources, the research skim, the
queue's own stopping rules, and population growth between years. Nothing
cheaper would do, because an item's date depends on everything ahead of it in
the queue and on how the planet grows underneath it while it waits.

It returns two years — when the **first** of them is finished and when the
**last** is — and three of the values are not years at all:

| value | meaning |
|-------|---------|
| `100` | a century or more: **never** |
| `0` | nothing to do: **skipped** |
| `-1` | auto alchemy standing by: **as needed** |

`PszProductionETA` (`1048:310c`) turns the pair into words, and this uses the
game's own: `Never`, `Unknown`, `Skipped`, `As Needed`, `1 year`, `4 years`,
`2 - 9 years`, `3 - ??? years`. An **auto-build** item that never completes
reads `Unknown` rather than `Never`, because it has no schedule to miss.

Two rules inside the simulation are the ones that matter, and both are
`Produce`'s as well as the estimate's — so the turn generator and the estimate
now agree, which they did not before:

* **The queue stops** at the first ordinary item it cannot finish
  (`if (mdStatus > 4)`). Everything behind it waits a year. An **auto-build**
  item that cannot finish does not stop it — the manual's "auto-build items
  that require only resources will continue to be produced" (p. 7-1).
* **Auto alchemy** stands aside unless it is the last item in the queue, and
  lends a hand to whatever follows it — see below; as the last item it runs
  flat out, its count overwritten with 1020.

A third, which the estimate depends on and which was wrong here: an auto-build
item's `up to N` is a **target, not a countdown**. It is clamped to the year's
cap before building and the stored figure is left alone, so it means the same
thing next year.

The caps themselves come from the opening of `CBuildProdItem`: mines,
factories and defences are held to what the planet can **operate**, not to what
it could ever hold; maximum terraforming to how much is left; and **minimum**
terraforming to the same but only while the planet is not both growing and
habitable — which is exactly what makes it the minimum. Packets need a mass
driver and something on the surface to fling.

## Alchemy's top-up

The block that closes `CBuildProdItem`. When the entry immediately before the
one being built is **auto alchemy**, and the item is short of a **mineral**,
resources are turned into minerals to make up the gap:

```
each   = 100 resources, or 25 with the Mineral Alchemy trait
bought = min(resources / each, the shortfall)
        -> bought kT of ironium, boranium AND germanium
        -> each * bought resources spent
if bought covered the whole shortfall: try to build again
```

Three things fall out of that and are worth stating plainly:

* it buys **up to the shortfall and no further** — the manual's "if you already
  have enough minerals, Mineral Alchemy isn't needed and doesn't happen"
  (p. 7-9);
* it is no help at all when **resources** are what ran out, because alchemy is
  bought with resources. The original works this out with two flags that are
  not opposites: one records that a mineral was at some point the tightest
  input, the other that resources were the tightest in the end;
* every unit makes one kT of **all three** minerals, so unblocking a factory's
  germanium leaves ironium and boranium on the surface as well.

An **auto-build** item short of minerals normally banks nothing and gives up;
with alchemy in front of it, it asks for the top-up first instead. If the
alchemy still cannot cover the gap, the item reports as ordinarily blocked —
which **stops the queue**, where without the alchemy in front it would merely
have been passed over. That is what the binary does, and it is easy to get
backwards.

Reading the parameters correctly is what this hangs on: Ghidra binds
`CBuildProdItem`'s arguments one slot out — its `fCalcOnly` is really
`fAlchemy`, and its `pmdStatus` is really `rgRes` — so on the face of it the
whole top-up looks unreachable. It is reached exactly when there is alchemy in
front.

## Reading the queue

`FillPlanetProdLB` (`1048:6692`) builds each row as `"%c%5d%s"` — a status
character, the count, the name — and `DrawProductionItem` (`1048:6208`) reads
that first character to decide how to draw it. From the same two figures:

| char | meaning |
|------|---------|
| `&` | nothing to do this year, or not known |
| `*` | first and last both next year: the whole order lands at once |
| `#` | the first lands next year but the rest take longer |
| ` ` | ordinary — somewhere between two and ninety-nine years |
| `!` | a hundred years or more: **never**, and the manual's red row (p. 7-7) |

Two more markers are hidden in the same string: the count field's first
character is bumped for an auto-build item and for terraforming, and the
alchemy row's last digit is overwritten with `*`. They are private signals to
the drawing routine, not text.

## Templates

A **production template** is a saved run of auto-build items
(`MANUAL.PDF` pp. 7-3..7-6). Four of them per player: a **default**, applied on
its own to any planet newly colonised or taken over, and three the player
applies by hand from the right-click menu on the blue diamond. Applying one
replaces every auto-build item in the queue with the template's, leaving the
ordinary items where they are, and takes the *Contribute only leftover
resources* setting from it too.

```c
typedef struct _zipprodq {
    char      szName[13]; /* +0x00 */
    uint8_t   fValid;     /* +0x0D  an empty slot reads <Unused 2> */
    ZIPPRODQ1 zpq1;       /* +0x0E  the same record PLAYER.zpq1 holds */
} ZIPPRODQ;               /* size 40 */
```

**They are not in a save.** `vrgZipProd` is a global, and `InitStars` fills it
from `stars.ini` — section `ZipOrders`, keys `ZipOrdersP1`…`ZipOrdersP5` — with
the whole record packed into printable letters:

| characters | |
|---|---|
| 0 | the "no research" flag, `'a'` or `'b'` |
| 1 | how many entries follow, `'a' + n` |
| 2… | four per entry: the nibbles of the `PRODQ1` word, **lowest first**, each `'a' + nibble` |
| then | the name, up to twelve characters |

A value is rejected outright if it is under three characters or over 64, if the
count is above twelve, if the packed body is not all in `'a'..='p'`, or if the
name is thirteen characters or more. Two things are **clamped** rather than
rejected: a quantity above 1020 becomes **1**, and an item id above 6 becomes
**0** — which is the seventh independent statement in the binary that a
template holds nothing but the auto-build items.

So only the **default** ever reaches the host, through `PLAYER.zpq1` and the
`rtLogPlayerZpq1` order; the other three follow the installation rather than
the game. `stars_formats::ProductionTemplate` reads and writes the text form,
and the desktop shell keeps a `stars.ini` beside the save, rewriting only the
`ZipOrders` keys it owns.

### The blue diamond

`DrawProductionDlg` puts a small raised blue diamond at the bottom left of the
dialog and remembers its rectangle in `rcProdDiamond`:

```
top    = client.bottom - (dyArial8 * 5 / 2 + 12)
bottom = top + (dyArial8 | 1)          odd, so it has a middle row
left   = 6
right  = 6 + dyArial8
```

in `hbrBBlue`, with `Apply or define a production template`
(`idsApplyDefineProductionTemplate`) four pixels past its right edge.
`ProductionDlg` hit-tests that three ways:

* **hovering** it swaps in `hcurArrowHelp`, the arrow with a question mark;
* a **left** click puts up a balloon that tells you to use the other button:
  *"Right click on the blue diamond to apply a production template to this
  queue, or choose `<Customize>` to define a template based on the auto build
  items in the current queue."*;
* a **right** click brings up the menu — every template that has something in
  it, a separator, then `<Customize>`.

`DrawDiamond` (`1028:4b60`) draws the shape itself scanline by scanline, laying
a highlight along the upper-left edges and a shadow along the lower-right
before filling the middle, which is what makes it look raised.

Choosing `<Customize>` copies the whole `ZIPPRODQ[4]` array first and puts it
back if the dialog is cancelled — so **Cancel undoes an Import or a Delete**,
not just a rename. The default queue goes back with it, and setting it to what
it already was writes no order, which is the `memcmp` guard in
`LogChangeZpq1`.

### `<Customize>`

`ZipProdDlg` (`10d0:5490`). Four radio buttons, `0x431`…`0x434`, one per slot,
each labelled with its template's name or `<Unused n>`; the chosen template's
contents listed below (`FillZipProdLB`, `10d0:5e58`); and **Import** (`0x816`),
**Delete** (`0x817`) and **Rename** (`0x41b`).

* **Import** takes the current planet's queue, keeps the auto-build entries in
  order, up to twelve, and makes them the template — which is why the manual
  tells you to arrange the queue and set the research checkbox *before*
  importing (p. 7-5).
* **Delete** and **Rename** are greyed for slot 0 and for an empty slot
  (`EnableZipProdBtns`): the default cannot be renamed, and the only way to
  empty it is to import an empty queue over it.
* Each line reads `Factories (100)`, except that an entry of exactly one — and
  alchemy whatever its count — is shown by name alone. An empty template reads
  `<No Auto Build Orders>`.

The array is `ZIPPRODQ[5]` and both the reader and the writer walk all five,
but the dialog's radio buttons only reach the first four, so the fifth
round-trips through `stars.ini` without ever being usable. The manual's count
of "three other templates" is the dialog's, not the array's.

## What is reproduced

The **template**, in `crates/stars-ui/src/dialog.rs`: the dialog's size, every
control's class, position and caption, and the two overlaps opened out. Both
lists and everything that moves items between them: the inventory in the
original's own order with its counts, the queue with its `— Top of the Queue —`
first line, **Add** and **Remove** with all four modifier steps and with
double-click doing the same as the button, the merge with a neighbouring row,
**Item Up** / **Item Down**, **Clear**, **Prev** / **Next** with Shift for the
next starbase, the *Contribute only leftover resources to research* checkbox,
the **cost panel** under each list in the game's own four colours with `kT`
after the three minerals and the completion line under the queue's, and a working copy
that **Cancel** throws away and **OK** — or stepping to another planet — writes
back. All four production templates, reached from the blue diamond exactly as
the original reaches them, with `<Customize>`'s Import, Delete, Rename and its
restoring Cancel, and the `stars.ini` encoding they persist in. Every queue row carries
its year and its colour, in the dialog and in the
planet pane's Production tile alike — the original fills both from the same
routine, so an item that will practically never be built is red in both.

The queue is written to the save through the same path a queue edited on the
Planets screen takes, so it reaches the `.xN` as an `rtLogPlanetProdQ` order.

In `crates/stars-core/src/production.rs`:

* `inventory(planet, builder, designs, queue)` — the list above, counts and
  all, with the queue already subtracted.
* `item_name(id)` — `PszNameProdItem`'s wording.
* `item_cost(id, builder, tutorial)` — what one costs, including the packets
  and the two items that are really components and are miniaturised like any
  other: the Genesis Device and the planetary scanners. Ship designs are costed
  by `ShipDesign::true_cost`.
* `mass_driver_warp(planet, designs)` — whether packets are on offer at all.
* `eta(planet, builder, research_pct, designs, index)` — the simulation above,
  returning an [`Eta`] that knows both its wording and its colour.
* `build_item`'s [`BuildStatus`], which is `mdProdStat` and is what decides
  whether the queue carries on.

## What is not reproduced

* The estimate is recomputed from scratch for every row on every frame, as the
  original recomputes it whenever it refills the list. It costs about a
  millisecond for a seven-row queue in a debug build, which is affordable; a
  very long queue on a slow machine would want caching.
* **Building what the new inventory rows offer.** A queued packet, planetary
  scanner or Genesis Device is priced and can be ordered, but the turn
  generator does not yet build any of them: `run_queue` skips an item it has no
  effect for. Packets are already on the plan's list of what is missing.
* The starbase **upgrade** discount, which is `GetProductionCosts` rather than
  the dialog: rebuilding on the same hull credits the parts already in orbit.
