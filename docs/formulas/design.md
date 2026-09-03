# Subsystem: Ship & Starbase Design

- **Status:** mass, capacities, shields and scanner ranges verified; armour partly verified
- **Ghidra routine(s):** `WtMaxShdefStat` (fuel and cargo), `DpShieldOfShdef` (`util.c`), the `rghuldef` / `rghuldefSB` tables
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
| Armour | hull `dp` + Σ armour `dp` × count (halved on a starbase — see below) |
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

## Armour: what is and is not established

Armour is the one derived value not fully pinned down.

- In the three-player sample game and the shipped tutorial, every ship design's
  armour is reproduced exactly by `hull armour + fitted armour`.
- In the Exodus game 426 of 518 do, but a recurring group does not. A Stalwart
  Defender there is a Destroyer (hull armour 200) carrying two Crobmnium (75
  each). That should be 350; the engine stored 275. The gap is neither a
  constant nor a constant factor, and across designs it goes in both
  directions.
- For **starbases**, fitted armour appears to count half: an Orbital Fort (100)
  with three Tritanium (50 each) stores 175, and the same rule gives 400, 900
  and 1300 for three other starbases on two hulls with armour counts from 12 to
  32. One stock "Starbase" on a Space Station stores twice its hull's armour
  with nothing fitted, which this rule does not explain.

The routine that computes armour is a stub in the reconstructed sources, so
rather than invent a rule that fits one game the implementation uses the
straightforward sum (with the starbase halving) and the test asserts only the
games where that is known to hold, reporting the rest. Reading the real
computation out of our binary is the next step.

Note also that the stored armour is a **cache the host fills in while
generating a turn**: a game's very first files, written before any turn has run,
carry zero for every design.

## Open questions

- The armour computation, as above.
- Battle initiative: the hull's base initiative plus battle computers; the
  bonus each computer gives is in the specials table but the combination rule
  (`InitFromHuldef`) has not been read yet.
- Stock designs (`rgshdefT`, `rgshdefSBT`) are not transcribed; only the hull
  tables are.
- `grfAbilities` bit meanings across the component tables.
