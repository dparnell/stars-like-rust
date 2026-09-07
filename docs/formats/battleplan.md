# Battle-plan block (type 30)

Status: **decoded & verified** — implemented in `stars-formats::battleplan`.
The same bytes are also an **order operation**; see
[the `.xN` spec](orders-x.md#battle-plan-definition-rtbtlplan-id-30).

Each player carries a small set of battle plans. A fresh game ships five
defaults: *Default*, *Kill Starbase*, *Max-Defense*, *Sniper*, *Chicken*. A
battle plan describes how a fleet assigned to it behaves in combat.

## Layout (5-byte header + packed name)

| Offset | Bits / Size | Field            | Notes                                |
|--------|-------------|------------------|--------------------------------------|
| 0      | low 4 bits  | race / player id | the owner                            |
| 0      | high 4 bits | battle-plan id   | the slot; see the caveat below       |
| 1      | low 4 bits  | tactic           | `0..=5`; the host accepts `0..=6`    |
| 1      | bit 6       | deleted          | `PLAN_DELETED`, `0x40`               |
| 1      | bit 7       | dump cargo       | `PLAN_DUMP_CARGO`, `0x80`            |
| 1      | bits 4,5    | unread           | zero in every fixture, preserved     |
| 2      | low 4 bits  | primary target   | `0..=7`; the host accepts `0..=8`    |
| 2      | high 4 bits | secondary target | `0..=7`; the host accepts `0..=8`    |
| 3      | 1 byte      | attack who       | see below                            |
| 4      | 1 byte      | name length      |                                      |
| 5..    | var         | name             | Stars! packed-string encoding        |

`attack who`: 0 = Nobody, 1 = Enemies, 2 = Neutrals & Enemies, 3 = Everyone,
4.. = a specific race id (`value − 4`).

Names use the packed-string codec documented in `strings.md`.

The **bounds** come from the host's replay of a type-30 order record
(`1048:c287`), which is the only place the game validates one: it refuses a
tactic above 6 (`1048:c324`) and either target above 8 (`1048:c336`,
`1048:c350`), and accepts at most 16 plans per player (`1048:c36f`). The
validator is looser than anything the game can produce — the dialog offers six
tactics and eight targets, and its own dialog stops at fifteen plans.

Byte 1 is therefore not a whole tactic value: the replay reads the tactic from
its low nibble, the deleted flag from bit 6 and *dump cargo* from bit 7. Every
plan in the fixtures has both flags clear, so the field decodes the same either
way; `BattlePlanRecord` keeps the byte whole and offers `tactic_nibble()`,
`deleted()` and `dump_cargo()` over it.

## What the values mean

The Battle Plans dialog fills each combo with consecutive strings and uses the
combo index as the stored value, so its lists **are** the enumerations. See
`../ui/battle-plans.md`, which gives the full tables and their strings; in
short:

| tactic | | target | |
|-------:|--|-------:|--|
| 0 | Disengage               | 0 | None/Disengage     |
| 1 | Disengage if challenged | 1 | Any                |
| 2 | Minimize damage to self | 2 | Starbase           |
| 3 | Maximize net damage     | 3 | Armed Ships        |
| 4 | Maximize damage ratio   | 4 | Bombers/Freighters |
| 5 | Maximize damage         | 5 | Unarmed Ships      |
|   |                         | 6 | Fuel Transports    |
|   |                         | 7 | Freighters         |

Modelled as `stars_core::battle::Tactic` and `TargetClass`, whose values the
combat code has always used — with the last three tactics in the wrong order
until the dialog was built. See `../ui/battle-plans.md`.

## The plan id is not reliably the slot

The default plans ship with ids `0, 1, 2, 3, 3` — *Sniper* and *Chicken* share
`3`, which is what the turn-0 fixture holds:

```
00 04 13 02 05 b3 2d 71 de 5a   plan 0  Default
10 04 32 02 08 ba 45 50 c2 a1 8d 41 92   plan 1  Kill Starbase
20 03 43 02 08 bc 1e 1e 5b 32 d7 26 92   plan 2  Max-Defense
30 01 05 02 04 c2 64 dc 28   plan 3  Sniper
30 00 00 02 05 b2 34 d5 da 26   plan 3 again, Chicken
```

The nibble is nonetheless what an order record is **routed by**, and what
`ReadBattlePlan` (`1070:40ce`) stamps back into a plan when it stores one, so a
plan the player has edited carries its true slot. This project stamps the slot
whenever it writes a plan — into a log record and into a block — and leaves the
loaded defaults alone, so a file it did not touch comes back byte for byte.

## Deleting

`DeleteBattlePlan` (`10f0:1706`) does more than drop an entry. The plans after
it move up a slot and are restamped with their new index, the player's plan
count falls by one, and **every one of that player's fleets** with a plan index
at or past the deleted slot has its index decremented — including a fleet that
was using the deleted plan, which lands on the slot before it. The subtraction
is a plain byte decrement, so deleting slot 0 leaves a fleet that used it
pointing at 255. The engine keeps the wrap rather than clamping: a host that
clamped would diverge from the client that wrote the log.

## Evidence

`fixtures/incoming/turn0/Game.hst` has 15 type-30 blocks (5 per player). Player
0's plans decode, in file order, to exactly
`Default, Kill Starbase, Max-Defense, Sniper, Chicken`, with the first plan
carrying plan id 0 and `race_id = 0`.

## Source

- stars-4x `decompiled`: `Structures/Structure30.xml` ("Battle Orders").
- `WriteBattlePlan` (`1070:89b8`) writes the block and the log record from one
  buffer; `ReadBattlePlan` (`1070:40ce`) reads either back; the host's replay
  arm at `1048:c287` is where the field bounds come from.

## Open questions

- Bits 4 and 5 of byte 1, which are zero in every fixture and in everything the
  dialog can produce.
