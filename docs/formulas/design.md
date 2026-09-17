# Subsystem: Ship & Starbase Design

- **Status:** verified — mass, armour, shields, capacities and scanner ranges
- **Ghidra routine(s):** `UpdateShdefCost` (`1038:47b0` — armour, mass and cost), `GetTruePartCost` (`1050:cd00` — miniaturisation), `FLookupPart` (`1008:524e` — who may build what), `WtMaxShdefStat` (fuel and cargo), `DpShieldOfShdef` (shields), `WriteRtShDef` (`save.c`, which field is stored), the `rghuldef` / `rghuldefSB` tables
- **Manual reference:** `MANUAL.PDF` ch. 9 (Ship and Starbase Design), p. 8-2 and p. 20-14 (miniaturisation), ch. 23 (armour and shields)
- **Uses RNG:** no
- **Implemented in:** `crates/stars-core/src/design.rs` and `crates/stars-core/src/parts.rs`, hull tables in `crates/stars-core/src/components.rs`
- **Records decoded by:** `stars_formats::DesignRecord`, see `../formats/design.md`

A design is a hull plus up to sixteen slots, each holding some number of one
component. Everything else is derived, which is why the game stores only the
slots and recomputes the rest — and why this layer gates combat, the build
queue and fleet fuel.

## Slots

A slot records a **category bitmask**, an **item index** and a **count**. The
item is a zero-based index into whichever component table the category names:
the original's own lookups index the arrays directly (`LpengineFromId(id)` is
`&rgengine[id]`) rather than searching for an id field.

In practice a design record's category is always a single specific category,
even where the hull's own slot accepts several — a hull slot that takes "shield
or armour" is stored as one or the other once something is fitted.

## Derived values

| Value | Rule |
|-------|------|
| Mass | hull `wtEmpty` + Σ component mass × count. **Cargo is not included** |
| Armour | hull `dp` + Σ armour `dp` × count (halved with Regenerating Shields), + 65 per Croby Sharmor or Langston Shell, + 50 per Multi Cargo Pod |
| Shields | Σ shield `dp` × count, plus 50 per Fielded Kelarium and 100 per Mega Poly Shell; +40% with Regenerating Shields |
| Fuel capacity | hull `wtFuelMax` + 250/Fuel Tank + 500/Super Fuel Tank + 200/Anti-Matter Generator |
| Cargo capacity | hull `wtCargoMax` + 50/Cargo Pod + 100/Super Cargo Pod + 250/Multi Cargo Pod |
| Scanner range | fourth root of the sum of fourth powers, normal and penetrating separately |
| Cost | hull cost + Σ component cost × count |

That two armours also carry shielding is easy to miss and comes straight from
`DpShieldOfShdef`.

## Verification

`crates/stars-core/tests/differential_designs.rs`:

- **Mass is verified through the battle recordings.** A design record does not
  store a mass — the game recomputes it — but every battle token does. Of 85
  tokens whose design is available, **73 reproduce the computed mass exactly**
  and the remaining 12 exceed it by no more than that design's cargo capacity,
  which is what a laden freighter looks like. Nothing computes heavier than the
  engine recorded.
- All 581 full designs across every fixture resolve to a known hull.
- **Armour is verified exactly**: 493 designs across every player file, with
  zero disagreements. 461 match their file owner's rule and 32 match the other
  variant, which is what foreign designs learned in battle must do.

## Armour

Read out of `UpdateShdefCost` (`util.c`), which is where the game maintains the
cached `hul.dp` that a design record stores:

```
dp = hull.dp
for each fitted slot:
    if category == Shield and item is Croby Sharmor or Langston Shell:
        dp += count * 65
    else if category == Armor:
        fitted = count * armour.dp
        if the owner has Regenerating Shields: fitted /= 2
        dp += fitted
    else if category == Multi Cargo Pod:
        dp += count * 50
```

Three things here are easy to get wrong:

- **Regenerating Shields halves fitted armour.** That is the price the trait
  pays for its shields, and it is a property of the *owning race*, not of the
  design. Because a player file also carries foreign designs learned by
  fighting them, two designs on the same hull with the same armour in the same
  file can legitimately store different values.
- **Two shields are also armour.** The Croby Sharmor and the Langston Shell add
  65 damage points each, and a Multi Cargo Pod adds 50.
- **Slot categories are compared exactly, not as bitmasks.** A slot must be
  exactly `hstShield` or exactly `hstArmor` to count.

The stored value is a **cache the host maintains**, so it is only meaningful
once a turn has been generated. In a game's very first files ships carry zero
and the starbase design carries a placeholder; both are skipped rather than
compared.

### How it was found

The rule was originally guessed at from the data, and the guess was wrong: the
halving looked like a starbase rule, because the starbases in the fixtures
happened to belong to Regenerating Shields races. It only resolved by reading
the computation out of the binary. That is worth remembering — six of seven
starbases fitted the wrong rule exactly.

## What a design really costs

The table price is not what anyone pays. `UpdateShdefCost` (`1038:47b0`) adds
the hull and every fitted component through `GetTruePartCost` (`1050:cd00`),
and the designer and the production queue then make two more adjustments to a
starbase.

### Miniaturisation

Every technology level held **above** what a component needs makes it cheaper:

| | per level | floor |
|-|----------:|------:|
| ordinarily | 4% | 25% of list |
| with Bleeding Edge Technology | 5% | 20% of list |

The excess counted is the **smallest** margin over any field the component
actually requires; a component that requires nothing at all — a Scout hull, an
Orbital Fort — is measured against the player's *weakest* field instead.
Nineteen levels of margin is as far as it counts, which is exactly where both
floors are reached. The cut is `MulDiv(cost, pct, 100)` — rounded to nearest —
and nothing ever falls to free: a figure that would reach zero is held at one.

Two categories are excluded: **terraforming modules never get cheaper**, and of
the planetary items only the five defences do — not the scanners and not the
Genesis Device.

Bleeding Edge Technology pays for its steeper curve up front: until the player
is past **every** one of a component's requirements by a level, anything that
requires any technology at all costs **twice** as much.

`MANUAL.PDF` p. 20-14 states both curves and both floors, and agrees with the
binary. **Page 8-2 does not** — it says 5% a level and 75%, which matches
neither trait — and is the page to distrust.

### Starbases

Two more adjustments, applied by the designer's own panel (`DrawBuildSelHull`)
and by `GetProductionCosts` alike:

1. **Improved Starbases**, and Alternate Reality, take a fifth off;
2. every starbase cost is then **halved**, because the hull table stores it
   doubled. The Orbital Fort is listed at 80 resources, 24 ironium and 34
   germanium and is built for 40, 12 and 17.

Upgrading a starbase in place is cheaper again — the planet is credited for the
parts it already has, in full for an identical component, four fifths for a
different one of the same kind and seven tenths for a different kind, or half
the old hull's cost when the hull itself changes. That is a production rule
rather than a design one and is not implemented here.

## Where a design sits on the schematic

Each hull carries the layout of its own schematic: `HULDEF.rgbrc[16]` at
`+0x7F` places its slots on a grid and `HULDEF.wrcCargo` at `+0x7D` places its
hold. Transcribed into `Hull::slot_pos` and `Hull::cargo_pos`, recorded for all
37 hulls in `../vectors/hull-schematics.json`, and described in
`../ui/ship-design.md`.

## Who may build what

`FLookupPart` (`1008:524e`) decides whether a player may put a component on a
hull at all, before any question of cost. Three gates, in order: a **primary
racial trait** that reserves the component for somebody else, a **lesser trait**
that forbids it, and the **Mystery Trader** for the twelve components only it
hands out (`FShouldPartBeHidden`, `research.c`; see `wanderers.md`). What
survives all three goes to `TechStatus`, which reports whether the technology is
there — and treats being one level short *in the field currently being
researched* as its own answer, because that component arrives on its own.

Implemented in `crates/stars-core/src/parts.rs`. Two rules the reconstructed C
had **backwards**, both pinned by tests:

- **Hyper Expansion cannot build a stargate at all.** The reconstruction reads
  `if (majorAdv != raCheapCol) return LookupDisallowed`, which would make
  stargates HE's alone; the binary disallows them *for* HE, and `MANUAL.PDF`
  p. 6-10 says so in as many words.
- **The Mine Dispenser 50 is denied to War Monger**, not reserved for it.

## Open questions

- ~~Battle initiative.~~ `combat::fittings`: the hull's initiative plus one,
  two and three per Battle Computer, Super Computer and Nexus, capped at 63.
- ~~Stock designs (`rgshdefT`, `rgshdefSBT`) are not transcribed.~~ They are
  `startup::SHIPS` and `startup::STARBASES`, from which a new game's
  designs are made.
- ~~`LComputePower`, the design's `Rating:`, and the jammer and initiative
  figures the designer shows.~~ `score::design_power` is the rating and the
  designer's figure rows draw it, the cloak and jamming, and the initiative
  (`../ui/ship-design.md`).
- `grfAbilities` bit meanings across the component tables.
