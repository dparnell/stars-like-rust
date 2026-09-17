# Messages — `rtMsg` (type 12)

Status: **decoded & verified** — all 1,053 messages in the fixtures decode and
re-encode byte for byte, across 55 distinct ids. Implemented in
`stars-formats::message`; the engine's side is `stars_core::message`.

A message is how the host narrates a turn to one player: a fleet stopped by
mines, an order it could not carry out, a colony founded. What the file carries
is a numeric **id** choosing a line of text, an **object** the line is about,
and up to seven **parameters**. The text itself is a format string in the
executable's resources, which this project does not read and does not copy;
`stars_core::message::Message::summary` says the same things in its own words.

## Layout

From `PackageUpMsg` (`1030:802a`), which builds the record — minus its first
byte. In memory a record starts with a byte packing the recipient and the
parameter length; neither reaches the file, where the recipient is the file's
owner and the length is the block's.

```text
word 0: bits 0..=8   message id
        bits 9..=15  which parameters were stored as words, one bit each
word 2: the object the message is about
byte 4..: the parameters, in order — one byte each unless the mask says two
```

Two things make this harder to read than it looks.

**How many parameters a message has is not in the record.** It comes from a
table indexed by message id, one byte per id, at `1030:5b0e` — 387 entries,
ending where the next routine's padding begins. `PARAMETER_COUNT` is that table.
Get the count wrong and a block does not parse, which is what makes the
round-trip test a real check of it.

**A block is a run of messages, not one message.** The host appends every
message for a player into a single buffer (`FSendPlrMsg`, `1030:7ee8`) and the
writer emits the buffer whole, so a 16-byte block can be two 8-byte messages.
`MessageRecord::decode_all` walks them.

A parameter is written as **one byte when it fits in one**, so the same message
id produces records of different lengths depending on its arguments. A count
that will not fit in sixteen bits is passed as two parameters, low word first.

## Who gets one

`PackageUpMsg` refuses to queue a message for a player who is not receiving
them — `PLAYER.det` bit 9 clear, unless bits 13..15 read 7 — except for four
ids that always go out: `7`, `0x23`, `0x40` and `0x8f`.

## The ids

Only ids read out of `stars.2.7j.exe` are used here, each cited at the routine
that sends it. The names are the community reconstruction's, and they agree with
what the binary does at each site — which is a useful check on having read the
right routine:

| id | name | sent by |
|---:|------|---------|
| `0x27` | `idmHasRunFuel` | `MoveFleets`, a fleet dry and unable to move at any warp; `[fleet, 0]` |
| `0x2b`–`0x2e` | `idmHasLoaded`, `idmHasBeamed`, `idmHasUnloaded`, `idmHasBeamed2` | `SatisfyOrders`, a Transport task taking minerals (colonists: beamed) aboard or putting them down; `[fleet, amount lo, amount hi, kind, target class, target]` |
| `0x3e` | `idmHasCompletedOrdersProductionQueueEmpty` | `Produce` (`10b8:0371`), every year a planet with resources has an empty queue — empty to begin with or worked through; `[planet]` |
| `0x4e` | `idmHasCompletedAssignedOrders` | `KillUsedWaypoints` (`1080:189a`), a fleet at the last of its waypoints with no task there that reports for itself; `[fleet, 0]` |
| `0xaa`…`0xae`, `0x15d` | `idmHaveFound…` | written by the **client** on reading a planet record flagged first-year (`file.c`): occupied `[planet, owner\|0x30]`, hostile / habitable / terraformable `[figure, planet]`, unknown `[planet, 0]`, Claim Adjuster `[planet, value]` |
| `0x4e` | `idmHasCompletedAssignedOrders` | `SatisfyOrders`, when a fleet runs out of orders |
| `0x8b` | `idmHasRunFuelFleetsSpeedHasDecreased` | `MoveFleets`, a fleet dry and slowed to a free warp; `[fleet, warp]` |
| `0xbe` | `idmSomeoneHasSweptMinesMineField` | `SweepForMines`, to the field's owner |
| `0xc2` | `idmHasSweptMinesMineField` | `SweepForMines`, to the sweeping fleet's owner |
| `0xc3` | `idmHasDispersedMines` | the lay-mines arm at `10b0:999e` |
| `0xf4` | `idmStarbaseHasSweptMinesMineField` | `SweepForMines`, for a planet's starbase |
| `0xc0` | `idmMysteryTraderHasDecidedMakeAnotherPass` | `MoveThings`, `10b0:1e86` — to **every** player |
| `0x108` | `idmMysteryTraderHasRefusedGiveCaptainAudience` | `DoThingInteractions`, `1110:0cad` |
| `0x110` | `idmMysteryTraderHeadingHasVanishedOrdersHave` | the waypoint check as a player's file is written |
| `0x130` | `idmMysteryTraderHasUnexplicablyChangedHisCourse` | `MoveThings`, `10b0:1b40` — to **every** player |
| `0x109` | `idmHasAbsorbedMysteryTraderTraderHasGiven` | `1110:0f57` — technology for a load of minerals |
| `0x10a` | `idmHasAbsorbedMysteryTraderReturnTraderHas` | `1110:0f5f` — the same, for a player who holds every part |
| `0x10b` | a Mystery Trader part | `IdmGiveTraderPart`, `1110:1a96` |
| `0x10c` | the same, worded for a hull | `IdmGiveTraderPart` |
| `0x10e` | the Trader had nothing to give | `1110:0e2a` |
| `0x10f` | the same, worded for the Genesis Device | `IdmGiveTraderPart` |
| `0x118` | `idmMysteryTraderEyesCaptainSuspiciously…` | `1110:0d91` — this player has already traded |
| `0x14f` | the Trader gave a ship | `1110:142e` |
| `0x150` | the Trader meant to and could not | `1110:133b` |
| `0x149` | `idmCouldntGiveAwayBecauseThereColonistsBoard` | the give arm at `10b0:9436` |

Two of the Trader's messages go to **every player at once** rather than to one:
what it is doing is the one thing the whole galaxy learns together. They carry
the object `-6`, which stands for the Trader itself rather than for anything of
the player's own.

A message about a fleet the Mystery Trader has just absorbed cannot point at
it, because it no longer exists. Those pass `-1` as the object and a packed
name word from `WFromLpfl` (`1038:2b10`) as the first parameter instead: the
fleet number in the low nine bits, its main design in the next four, and bit 13
set when the fleet held more than one design — enough to say "Long Range Scout
#7" rather than a bare "Fleet #7".

The colonists one is worth pointing at: the check it reports — a fleet carrying
colonists cannot be given away — was read out of the binary as a comparison
against `FLEET.rgwtMin[3]`, and the message the same routine sends says exactly
that in words. Two independent readings of the same rule.

## The filter (`rtMsgFilt`, type 33)

A player can silence a kind of message they do not want to read again. The
choice is a **bitfield with one bit per message id** — bit set means filtered —
and it is a *reading* choice only: a filtered message is still sent, still
written to the file, and still counted. All that changes is that the message
list steps over it (`IMsgNext`, `IMsgPrev`).

| | |
|---|---|
| record | type 33, **45 bytes** — 360 ids' worth |
| bit | id `n` is bit `n % 8` of byte `n / 8` |
| where | the player's history (`.hN`), and their order log |

Ids from 360 up cannot be filtered at all. The record is 45 bytes in every one
of the 3,204 filter records in the fixtures, and **not one bit is set in any of
them**: nobody in the captured games ever silenced a message. So the fixtures
confirm the record's shape and nothing about its meaning.

**Filtering is by family, not by id.** `SetFilteringGroups` (`1030:a018`)
silences every other wording of the same event along with the one the player
picked — the game has several sentences for one happening, and a reader who does
not want one does not want any. The families, read from that routine's own
comparisons:

| ids | what they say |
|-----|---------------|
| `0x2b`–`0x2e` | a fleet loaded, beamed or unloaded cargo at a planet |
| `0x2f`–`0x30` | your starbase built a ship, or several |
| `0x35`–`0x36` | you built a factory, or several |
| `0x37`–`0x38` | you built a mine, or several |
| `0x39`–`0x3a` | you built a defence, or several |
| `0x42`–`0x43` | you transferred cargo to another player |
| `0x44`–`0x45` | you received cargo from another player |
| `0x46`–`0x47` | a transfer arrived short |
| `0x48`–`0x49` | a delivery arrived short |
| `0x4a`–`0x4b` | a transfer arrived not at all |
| `0x4c`–`0x4d` | a delivery arrived not at all |
| `0x60`–`0x64` | your bombers hit a planet, five ways |
| `0x6a`–`0x6e` | somebody bombed one of yours, the same five |
| `0x79`–`0x7a` | a fleet loaded or beamed cargo from another fleet |
| `0x91`–`0xa8` | a battle report, in any of its two dozen forms |

The pairs are adjacent ids, so a pair and a range are one rule. The original
writes a pair as `id ^ a ^ b`, which for two adjacent ids comes to the same
thing, and the community reconstruction's `^ 0x0f` and `^ 0x1f` companions do
not exist in this binary.

## What is sent

The engine sends the ids `crates/stars-core/src/message.rs` lists, each
with the original's parameters; a record carries exactly as many as
`PARAMETER_COUNT` gives the id (`Message::record` pads or trims to it,
which is what `PackageUpMsg` does with whatever it is handed), and
`tests/messages.rs` checks a decade of a real game sends no id with the
wrong count. What the original narrates and this engine does not is
whatever no transcribed routine sends.
