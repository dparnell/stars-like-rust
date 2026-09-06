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

From `ProdCommandHandler`'s `WM_COMMAND` arm:

| id | control |
|----|---------|
| `0x416` | the inventory list |
| `0x417` | the queue list |
| `0x418` | **Add** |
| `0x419` | **Remove** |
| `0x42d` | **Clear** |
| `0x439` / `0x43a` | **Item Up** / **Item Down** |
| `0x42e` / `0x42f` | **Prev** / **Next** planet |
| `0x8b` | **Contribute only leftover resources to research** |
| `0x816` | apply a production template |
| `1`, `2`, `0x76` | OK, Cancel, Help |

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
  lends a hand to whatever follows it; as the last item it runs flat out, its
  count overwritten with 1020.

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
(`MANUAL.PDF` pp. 7-3..7-6). Four of them per player: a default, applied
automatically to any planet newly colonised or taken over, and three the player
applies by hand from the right-click menu on the blue diamond. Applying one
replaces every auto-build item in the queue with the template's, leaving the
ordinary items alone.

The default template already round-trips through this project's save code as
`stars_formats::DefaultQueue` (`ZIPPRODQ1`, order record 46) and is applied by
[`crate::orders::apply_default_queue`] when a planet changes hands. The other
three, and the `ZipProdDlg` (`10d0:5490`) editor behind `<Customize>`, are not
reproduced.

## What is reproduced

Both lists and everything that moves items between them: the inventory in the
original's own order with its counts, the queue with its `— Top of the Queue —`
first line, **Add** and **Remove** with all four modifier steps and with
double-click doing the same as the button, the merge with a neighbouring row,
**Item Up** / **Item Down**, **Clear**, **Prev** / **Next** with Shift for the
next starbase, the *Contribute only leftover resources to research* checkbox,
the cost panel against what the planet has on the surface, and a working copy
that **Cancel** throws away and **OK** — or stepping to another planet — writes
back. Every queue row carries its year and its colour, in the dialog and in the
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

* **Templates** beyond the default one, and the `<Customize>` editor.
* **Alchemy's help.** Auto alchemy standing in front of an item is modelled as
  far as standing aside and reading `As Needed`, but the *top-up* it performs —
  `CBuildProdItem`'s closing block, which turns resources into just enough
  minerals to unblock the item behind it — is not. Until it is, an alchemy row
  in front of a mineral-starved item does nothing for it.
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
