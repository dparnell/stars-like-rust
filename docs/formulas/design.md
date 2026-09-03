# Subsystem: Ship & Starbase Design

- **Status:** verified — mass, armour, shields, capacities and scanner ranges
- **Ghidra routine(s):** `UpdateShdefCost` (armour, mass and cost, `util.c`), `WtMaxShdefStat` (fuel and cargo), `DpShieldOfShdef` (shields), `WriteRtShDef` (`save.c`, which field is stored), the `rghuldef` / `rghuldefSB` tables
- **Manual reference:** `MANUAL.PDF` ch. 9 (Ship and Starbase Design), ch. 23 (armour and shields)
- **Uses RNG:** no
- **Implemented in:** `crates/stars-core/src/design.rs`, hull tables in `crates/stars-core/src/components.rs`
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

## Open questions

- Battle initiative: the hull's base initiative plus battle computers; the
  bonus each computer gives is in the specials table but the combination rule
  (`InitFromHuldef`) has not been read yet.
- Stock designs (`rgshdefT`, `rgshdefSBT`) are not transcribed; only the hull
  tables are.
- `grfAbilities` bit meanings across the component tables.
