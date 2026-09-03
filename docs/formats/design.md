# Design block (type 26) — ship / starbase design

**Status:** fully decoded & verified. Implemented in `stars-formats::design`
([`DesignRecord`](../../crates/stars-formats/src/design.rs)) and verified against
`fixtures/incoming/turn0/Game.hst`.

A design block describes one hull design a player has created. It comes in two
shapes, selected by the `fullData` bit (bit 2 of byte 0):

- **Full design** — hull, picture, armor, per-slot equipment, the turn it was
  designed, and total built / remaining counts.
- **Partial design** — identity plus a stored `mass` only (used when a player
  merely *sees* a foreign design, not its internals).

Layout recovered from the stars-4x `starsapi` project (`DesignBlock.java`).

## Header (both shapes)

| Offset | Bits     | Field                                        |
|--------|----------|----------------------------------------------|
| 0      | bit 2    | `fullData` flag (`1` = full design)          |
| 0      | bits 0–1 | must be `3`                                   |
| 1      | bit 0    | must be `1`                                   |
| 1      | bits 2–5 | design number (0..=15)                        |
| 1      | bit 6    | is-starbase                                   |
| 1      | bit 7    | is-transferred                                |
| 2      | all      | hull id                                       |
| 3      | all      | picture / icon index                          |

## Full design (bit 2 of byte 0 set)

| Offset       | Size | Field                                          |
|--------------|------|------------------------------------------------|
| 4–5          | u16  | armor points                                   |
| 6            | u8   | slot count `s`                                 |
| 7–8          | u16  | turn designed                                  |
| 9–12         | u32  | total built                                    |
| 13–16        | u32  | total remaining                                |
| 17 .. +4·s   | s×4  | slots: `[category:u16][item_id:u8][count:u8]`  |
| then         | —    | packed name field (`[len][packed]`)            |

## Partial design (bit 2 of byte 0 clear)

| Offset | Size | Field                               |
|--------|------|-------------------------------------|
| 4–5    | u16  | mass                                |
| 6      | —    | packed name field (`[len][packed]`) |

## Notes

- A full design's **mass** and **fuel capacity** are *computed* by the game from
  the hull and component tables, so `DesignRecord` only exposes the directly
  stored partial-design `mass`, not a computed full-design mass.
  `stars_core::design::ShipDesign` computes them; see
  `../formulas/design.md`, where the computed mass is checked against the
  battle recordings.
- The stored **armour** is a value the host caches while generating a turn. A
  game's very first files, written before any turn has been generated, carry
  zero for every design, so a zero there means "not yet computed" rather than
  "no armour".
- Slot `category` is a tech-category bitmask (engine, beam weapon, armor,
  scanner, …); resolving `(category, item_id)` to concrete component names needs
  the item database (a later `stars-core` concern).

## Verified values (`Game.hst`, turn 0 — 15 designs)

Player 0 (Humanoid): *Armed Probe* (hull 4, 3 slots), *Long Range Scout* (4),
*Santa Maria* colony ship (hull 15), *Teamster* (hull 1), *Stalwart Defender*
(hull 6, 7 slots), *Cotton Picker* (hull 21). Players 1 & 2 get three ship
designs each. Every player has one **Starbase** design: hull 34, armor 1000, 12
slots. All starting designs are full designs made on turn 1.
