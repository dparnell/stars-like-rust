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
| `0x4e` | `idmHasCompletedAssignedOrders` | `SatisfyOrders`, when a fleet runs out of orders |
| `0xbe` | `idmSomeoneHasSweptMinesMineField` | `SweepForMines`, to the field's owner |
| `0xc2` | `idmHasSweptMinesMineField` | `SweepForMines`, to the sweeping fleet's owner |
| `0xc3` | `idmHasDispersedMines` | the lay-mines arm at `10b0:999e` |
| `0xf4` | `idmStarbaseHasSweptMinesMineField` | `SweepForMines`, for a planet's starbase |
| `0x108` | `idmMysteryTraderHasRefusedGiveCaptainAudience` | `DoThingInteractions`, `1110:0cad` |
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

## What is not modelled

The engine sends the six messages above. Every other event the original narrates
— and there are hundreds of ids — passes silently. Nothing reads a message
**filter** (`rtMsgFilt`, type 33) either, so a file's filter settings are
carried but not obeyed.
