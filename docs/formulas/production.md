# Subsystem: Production & Resource Allocation

- **Status:** in progress — the resource split is verified; the build queue needs the components table
- **Ghidra routine(s):** `10b8:0000` `Produce`, `10b8:0c92` `CBuildProdItem`, `10b8:19b2` `FBuildObject`
- **Manual reference:** `MANUAL.PDF` p. 20-13 (recycled resources), ch. 7 (production)
- **Uses RNG:** no (the mining it invokes does)
- **Implemented in:** `crates/stars-core/src/production.rs`

`Produce` is the heart of a turn: it mines, it decides how each planet's output
is split between research and building, it runs every build queue, and it ends
by growing populations and advancing research.

## What `Produce` does, in order

```
MineMinerals()                              # see mining.md
zero every player's lResLastYear

for each planet:
    if it has no production-queue object:
        all of its resources go to research
    else if it is unowned or its queue array is empty:
        it contributes nothing at all
    else:
        resources = CResourcesAtPlanet(planet, owner) + recycled surplus
        if not planet.fNoResearch:
            skim = resources * player.pctResearch / 100
            resources -= skim
            player.lResLastYear += skim
        run the build queue against (minerals, resources)
        write the minerals back
        player.lResLastYear += whatever resources are left

UpdatePopulations()                         # see population.md
UpdateResearchStatus(1)                     # see research.md
if not game.fNoRandom: RandomEvents()
```

Two consequences worth stating plainly, because they surprise people:

- **Unspent production becomes research.** A planet whose queue is empty, or
  which cannot afford the next item, puts everything it did not spend into
  research. This is why a player who lets their queues run dry still advances.
- **A planet holding an empty queue *array* contributes nothing** — not even
  its research skim. The common case is different: when a queue empties the
  original frees the object and leaves a null pointer, which is the "all to
  research" branch above.

## Recycled resources

Surplus resources delivered to a planet are not additive. The manual gives the
rule directly (p. 20-13):

```
Resources = (Current_production x Extra_resources) /
            (Current_production + Extra_resources)
```

and the original *adds* that to the planet's own output. The effect saturates:
a planet already producing a lot gains very little.

## Edge cases & clamps

- A player flagged as cheating has production resources forced to 32.
- The research skim truncates, per planet, so a hundred small planets lose more
  to rounding than one large one.
- `fNoResearch` (a per-planet flag) exempts a planet from the skim entirely.

## Verification

`crates/stars-core/tests/differential_resources.rs` checks the one bound that
does not require modelling the build queue: research can never receive more
than the planets produced. Across 27 consecutive year-pairs of the Exodus game
the engine's `lResLastYear` never exceeds our computed total, and averages 43%
of it against a 30% research setting — above the skim, because unspent
production falls through.

That is a one-sided check. It would catch a resource formula that produced too
little (immediately, in every year) but not one that produced too much. A
two-sided check needs the build queue, and therefore the components table.

## What a planet builds directly costs

From `GetProductionCosts` (`produce.c`). Ship designs are costed from their
hull and components (`design.md`); these are the things a planet builds
without a design:

| Item | Minerals | Resources |
|------|----------|-----------|
| Mine | — | `rsMineBuild` (Humanoid 5) |
| Factory | 4 germanium, 3 with Cheap Factories — but **2 of every mineral** (1 with the trait) while the tutorial runs | `rsFactBuild` (Humanoid 10) |
| Defence | the SDI's entry in the planetary table | likewise; Inner Strength pays 3/5 of both |
| Mineral alchemy | — | 100, or 25 with the trait |
| Terraform | — | 100, 70 with Total Terraforming, halved again for Claim Adjuster |

The tutorial price is a branch on bit 11 of the client's `gd` word
(`10d0:4885`), which `StartTutor` sets (`10f8:0748`) and `EndTutor` clears.
It is not a game setting and is in no file: the tutorial is generated on the
player's own machine, so the host code sees the client's flag. It matters,
because the tutorial's home world builds a hundred and fifty factories in its
first ten years and germanium alone would not stretch to them — with the
flat price it is ironium that runs short, which is what the pages' Teamster
shuttle from Prune and the seventy mines of 2411 are about. Carried as
`GameState::tutorial`.

The **auto-build** form of an item is a separate id — 0..=6, against 7 and up
for the plain items — and costs the same. See `../formats/production.md` for
the full table and the evidence, which this project had backwards until the
ship-design work went past `FillProdSrcLB`.

A queue item carries partial progress in its `completion` field, so a colony
earning 2 resources a year does eventually finish a 9-resource factory. Any
check on what a planet could afford has to allow for one carried item.

## Building an item

From `CBuildProdItem` (`10b8:0c92`):

```
paid = cost * carried percentage / 100      # progress from previous years

while some are still wanted:
    if every input covers (cost - paid):
        complete one; reset paid; continue
    # cannot finish the next one: pay for as much of it as possible
    pct = the smallest, over the four inputs, of (available + paid) * 100 / cost
    if blocked on minerals and this is an auto-build item: stop, banking nothing
    else: pay pct's worth, bank pct, stop
```

Whole units are completed while they can be afforded outright; the leftover
part-pays the next one and is banked as a percentage. That is what lets a
colony earning 2 resources a year eventually finish a 9-resource factory.

The one asymmetry: an **auto-build** item blocked for want of *minerals* banks
nothing and stops, rather than part-paying something it cannot finish. Blocked
on resources it behaves like any other item.

Auto-build mines, factories and defences are additionally capped by what the
planet will be able to **operate** next year, not by what it could ever hold,
so an auto-build queue keeps pace with population instead of racing ahead of
it. Terraforming is capped by how much is left, and the **minimum** variant
only acts while the planet is not both growing and habitable. An auto-build
item's `up to N` is a target rather than a countdown: it is clamped to that cap
each year and the stored figure is untouched.

## What a finished item becomes

`FBuildObject` (`10b8:19b2`) takes each finished unit. Installations go on
the planet; a ship joins a new fleet at the planet with its tank full;
packets are thrown (`packets.md`); and a **starbase** (a design slot of 16
and up) goes up over the planet, `turn::install_starbase`:

- the planet's base becomes the design, a new one where there was none, and
  the old design gives back one of its built count;
- a base replacing one with a **smaller hull number** empties the ships out
  of the planet's queue (`KillQueuedShips`, `10c8:6c3a`), the starbase items
  keeping their place but losing their progress;
- a planet whose old base had **no mass driver** looks at the new one: none,
  and the packet items come off the queue too (`KillQueuedMassPackets`,
  `10c8:6a52`) with the fling setting cleared; one, and the fling warp is set
  to the driver's, a pair counting one faster (`10b8:1bdb`);
- the owner is told (`0xcd`; `0xce` for a base with a dock, `0xcf` for one
  that builds any size) with the planet, the design word and the dock's
  size.

`tests/differential_production.rs::a_queued_starbase_goes_up_over_the_planet`
covers it; before it a queued base was launched as a ship.

The rest of `FBuildObject`'s planetary arms, each with the message it sends:

- a **planetary scanner** (`0x12`–`0x1a`, or the generic `0x1b`, which
  `LookupBestPlanetaryScanner` resolves to the owner's best) sets the
  planet's `iScanner` to the scanner's index — Viewer 50 is 0, which is
  what every corpus home world stores — and tells the owner
  (`idmHasBuiltNewPlanetaryScanner`, `0x7c`, `[planet, -0x8000, index]`);
  `turn::install_planetary_scanner`;
- the **Genesis Device** (`0xd`) tells every player
  (`idmStrongFundamentalForcesHaveRebirthed`, `0x11b`); unless the owner
  is Alternate Reality it levels the mines, factories and defences and
  sets `iScanner` to 31; and for each of the three axes it empties the
  surface minerals and redraws the climate — `rgEnvVarOrig` and
  `rgEnvVar` alike — as `Random(50) + Random(50) + 1` and the
  concentration as `Random(40) + Random(40) + 25`. The population is not
  touched. `turn::genesis_device`, drawing from the turn's generator;
- the **packets** (`0xe`–`0x11`, and the auto packet `6`) are thrown
  (`packets.md`, `packet::fling`).

The queue costs these through `GetProductionCosts` like anything else —
`production::item_cost`, which covers the packets, the scanners and the
Genesis Device (the last two miniaturised as components) as well as the
installations — so they are paid for over the years and then built. Until
this was wired the queue costed only the installations, and a queued
packet, scanner or device sat unpriced and was never finished.
`tests/differential_production.rs::a_queued_planetary_scanner_is_installed`,
`::a_genesis_device_remakes_the_planet` and the packet half of the
starbase test cover the three.

## Where the queue stops

`CBuildProdItem` returns an `mdProdStat` alongside the count, and `Produce`
reads it: **anything above `mdProdStatNoneAuto` (4) stops the queue for the
year**. So an ordinary item that cannot be finished holds up everything behind
it — the manual's "your people will not work to complete the original item
until the new item you placed in the queue is complete" (p. 7-1) — while an
auto-build item that cannot be finished is simply passed over.

Auto alchemy is the other special case: in front of another item it stands
aside and marks the next item as alchemy-assisted; as the last item in the
queue it runs flat out, with its count overwritten by 1020.

**What "assisted" means**: if the item is short of a *mineral*, resources are
turned into minerals to make up the gap — one kT of each of the three for 100
resources, or 25 with the Mineral Alchemy trait — up to the shortfall and no
further, and then the build is retried. It is no help when *resources* are what
ran out, since that is what alchemy is bought with. An auto-build item short of
minerals, which would normally bank nothing and be passed over, asks for the
top-up first; if the top-up cannot cover the gap it reports as ordinarily
blocked and the queue stops behind it.

Both rules are shared with the year estimate the Production dialog shows, which
is why they live in `production.rs`. See `../ui/production.md`.

## Open questions

- Ordering the queue as a whole — which item runs first, how a partially built
  item is re-inserted at the front, and the alchemy chaining that lets mineral
  alchemy feed the item behind it — is still to do. The per-item build above is
  the piece it drives.
- Starbase upgrades are costed part-by-part against the existing base, with
  full credit for identical components and 80% for same-category
  replacements; that path is read but not implemented.
