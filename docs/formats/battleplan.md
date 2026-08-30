# Battle-plan block (type 30)

Status: **decoded & verified** — implemented in `stars-formats::battleplan`.

Each player carries a small set of battle plans. A fresh game ships five
defaults: *Default*, *Kill Starbase*, *Max-Defense*, *Sniper*, *Chicken*. A
battle plan describes how a fleet assigned to it behaves in combat.

## Layout (5-byte header + packed name)

| Offset | Bits / Size | Field            | Notes                                |
|--------|-------------|------------------|--------------------------------------|
| 0      | low 4 bits  | race / player id |                                      |
| 0      | high 4 bits | battle-plan id   |                                      |
| 1      | 1 byte      | tactic           | disengage / minimise / maximise / …  |
| 2      | low 4 bits  | primary target   |                                      |
| 2      | high 4 bits | secondary target |                                      |
| 3      | 1 byte      | attack who       | see below                            |
| 4      | 1 byte      | name length      |                                      |
| 5..    | var         | name             | Stars! packed-string encoding        |

`attack who`: 0 = Nobody, 1 = Enemies, 2 = Neutrals & Enemies, 3 = Everyone,
4.. = a specific race id (`value − 4`).

Names use the packed-string codec documented in `strings.md`.

## Evidence

`fixtures/incoming/turn0/Game.hst` has 15 type-30 blocks (5 per player). Player
0's plans decode, in file order, to exactly
`Default, Kill Starbase, Max-Defense, Sniper, Chicken`, with the first plan
carrying plan id 0 and `race_id = 0`.

## Source

- stars-4x `decompiled`: `Structures/Structure30.xml` ("Battle Orders").

## Open questions

- The exact enumerations for `tactic` and primary/secondary target values.
- In the sample data two plans share the same high-nibble plan id; the field is
  exposed raw rather than assumed unique.
