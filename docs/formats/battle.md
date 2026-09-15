# Format: battle recording (VCR) — block types 31 & 39

- **Status:** decoded & verified — implemented in `stars-formats::battle`
- **Block types:** `31` `rtBtlData`, `39` `rtContinue`
- **Original files analysed:** `fixtures/games/exodus/*/exodus.m6` (47 recordings across 40 turns)
- **NB09 structs:** `BTLDATA` (0x0e), `TOK` (0x1d), `BTLREC` (0x06), `KILL` (0x08)
- **Ghidra references:** `save.c` writer, `CBattleKills` in `vcr.c` for the record walk
- **Implemented in:** `crates/stars-formats/src/battle.rs`; tests in `tests/battle_files.rs`

A battle recording is what the in-game VCR plays back: which stacks took part,
where each stood, and then a stream of actions saying who moved where, who shot
whom, and what died. It is the only place the game writes down what combat
actually did, which is what makes it the reference a combat implementation can
be tested against.

## Overall shape

```
BTLDATA        14 bytes
TOK[ctok]      29 bytes each
BTLREC ...     6 bytes + 8 per kill, repeated until cbData is reached
```

`cbData` counts the **whole** record, header included.

## `BTLDATA` — the header (14 bytes)

| Offset | Size | Field | Meaning |
|-------:|-----:|-------|---------|
| 0 | 2 | `id` | battle id; low byte counts battles within the turn |
| 2 | 1 | `cplr` | number of players involved |
| 3 | 1 | `ctok` | number of tokens |
| 4 | 2 | `grfPlr` | bitmask of participating players |
| 6 | 2 | `cbData` | total size of the record |
| 8 | 2 | `idPlanet` | planet the battle happened at |
| 10 | 4 | `pt` | universe location, two `int16` |

## `TOK` — a participating stack (29 bytes)

| Offset | Size | Field | Notes |
|-------:|-----:|-------|-------|
| 0 | 2 | `id` | source fleet or planet id |
| 2 | 1 | `iplr` | owner |
| 3 | 1 | `grobj` | object class: `1` planet, `2` fleet |
| 4 | 1 | `ishdef` | design slot; **`>= 16` is a starbase** |
| 5 | 1 | `brc` | starting square, packed `y << 4 \| x` |
| 6 | 1 | `initBase` | hull initiative |
| 7 | 1 | `initMin` | lowest firing initiative |
| 8 | 1 | `initMac` | highest firing initiative |
| 9 | 1 | `itokTarget` | token being targeted |
| 10 | 1 | `pctCloak` | cloaking % |
| 11 | 1 | `pctJam` | torpedo jamming % |
| 12 | 1 | `pctBC` | battle-computer accuracy % |
| 13 | 1 | `pctCap` | capacitor damage bonus % |
| 14 | 1 | `pctBeamDef` | beam deflection % |
| 15 | 2 | `wt` | mass of one ship, kT |
| 17 | 2 | `dpShield` | shield points for the stack |
| 19 | 2 | `csh` | ships in the stack |
| 21 | 2 | `dv` | packed damage, `pctSh:7, pctDp:9` |
| 23 | 2 | tactics | `mdTarget1:4, mdTarget2:4, mdTactic:4, mdTarget0:4` |
| 25 | 2 | movement | `dxyLim:4, dxyMax:4, spd:4, cTarget:4` |
| 27 | 2 | flags | `fActive:1, fDetector:1, fTorp:1, fRegen:1, fMoved:1, dzDis:5, dwt:4, dMovesLeft:2` |

## `BTLREC` — one action (6 bytes + kills)

| Offset | Size | Field | Meaning |
|-------:|-----:|-------|---------|
| 0 | 1 | `itok` | the token acting |
| 1 | 1 | `brcDest` | square it moved to, packed as above; **`0xFF` means the token left the battle** |
| 2 | 2 | `ctok` | **number of `KILL` records that follow** |
| 4 | 2 | packed | `iRound:4, dzDis:4, itokAttack:8` |

Records are walked by advancing `6 + ctok * 8` bytes at a time until `cbData`
is reached — exactly what `CBattleKills` does in the original.

### `KILL` (8 bytes)

| Offset | Size | Field |
|-------:|-----:|-------|
| 0 | 1 | `itok` — the token damaged |
| 1 | 1 | `grfWeapon` — weapon class flags |
| 2 | 2 | `cshKill` — ships destroyed |
| 4 | 2 | `dpShield` — shield damage |
| 6 | 2 | `dv` — remaining damage, `pctSh:7, pctDp:9` |

## Observed semantics

From the 47 Exodus recordings:

- Records with no kills are **movement**: `itokAttack` is the acting token
  itself, so it carries no target.
- Records with kills are **firing**: `itokAttack` names the victim, and it
  matches the `itok` of the `KILL` that follows.
- `dzDis` is the range recorded with the action, measured before the move.
- A token that disengages is written with `brcDest` = `0xFF`, which is not a
  square — read literally it would be (15,15), off the 10x10 board. Across the
  941 actions in the Exodus recordings this sentinel occurs 31 times and no
  other off-board value occurs at all, so `BattleAction::destination` is an
  `Option<Square>` and `None` means "left the battle".
- Defenders all start on one square and attackers on another, across the board.
- A defending planet appears as a token with `grobj == 1` and a starbase design
  slot; its `wt` is `0xFFFF`.

## Large battles

A record of 1024 bytes or more is split by the writer: the first block (type
31) carries the header and as many tokens as fit, and the remainder follows in
`rtContinue` blocks (type 39). `battle_records_in` stitches them back together,
so a caller always sees whole recordings. None of the 47 Exodus battles is
large enough to split, so that path is implemented from the writer in `save.c`
but not yet exercised by a fixture.

## Verification

`crates/stars-formats/tests/battle_files.rs`:

- All 47 recordings decode, and for **every** one the walk over tokens and the
  variable-length action stream lands exactly on the declared `cbData`. This is
  the sharp check: a wrong `TOK` (29) or `KILL` (8) size would drift and fail
  on nearly every battle.
- Every token names a player present in `grfPlr`, sits on the board, and has at
  least one ship; every action and kill indexes a real token; `cplr` agrees
  with the bitmask.
- Every battle involves at least two players, and the file's own player (5) is
  in all of them — you only receive recordings of battles you fought.
- The year-2424 home-world defence is asserted in full: the defending starbase,
  twelve tokens, the lone attacker from player 0 starting across the board, and
  the single killing shot that destroyed it.

## Open questions

- `grfWeapon`: `AnimateAttack` (`10e8:3ac2`) reads bits 0 and 1 as a beam
  (bit 1 choosing the blue pen), bit 2 as a torpedo and bit 6 as the
  torpedoes deflected; the fixtures carry 1, 4, 12, 196 and 204, so bit 3
  and bit 7 (always beside bit 6) are still unnamed.
- The tactics and movement words are decoded as packed values but their
  sub-fields are not yet mapped to the battle-plan UI.
- The version difference matters: `BTLREC26` is a 2.6-era variant with the same
  size but a different field split, converted on load by `UpdateBattleRecords`
  when the file's minor version is below 80. Our fixtures are 2.81, so only the
  modern layout is exercised.


## Where the recordings live

A battle recording (type 31) is written to the **player** files of the
participants, not to the host file. The sixteen-AI game's `Game.hst` files carry
no battle blocks at all across 101 turns, while its `Game.mN` files hold 90
records — 45 distinct battles, each appearing in both participants' files.
Almost all are two-token skirmishes; two have four tokens.

That doubles the fixture corpus from Exodus's 47 records. `battle_replay.rs`
now draws its battles from both games, de-duplicating by battle id and
position.

## Two action layouts, chosen by file version

The action record has **two layouts**, and the binary's own debug symbols name
both. They occupy the same six bytes but divide the middle word differently:

| Offset | `BTLREC` (2.7 and later) | `BTLREC26` (2.6) |
|--------|--------------------------|------------------|
| 0      | `itok`                   | `itok`           |
| 1      | `brcDest`                | `brcDest`        |
| 2      | `ctok`, a 16-bit kill count | `itokAttack`, the token attacked |
| 3      | *(part of `ctok`)*       | `ctok`, a **byte** kill count |
| 4-5    | `iRound`, packed round / range / **target** | `iRound`, packed round / range |

In the newer form the attacked token rides in the top half of the packed word;
in the older one it has a byte of its own and the kill count shrinks to fill
the gap.

Which applies is decided by the **file version header**, not by anything in the
record. `ActionLayout::for_version` makes the choice, and `battle_records`
reads the version itself; `battle_records_in_with` takes the layout explicitly.

### How this was found

Adding the sixteen-AI game's recordings broke two invariants that hold across
all 47 Exodus battles: a speed-1 token appearing to move five squares in one
round, and a shot at range 5.

Byte-for-byte comparison ruled out the obvious causes — the 14-byte header, the
29-byte token records and the action framing are identical, and both records'
declared lengths match their blocks exactly. The divergence was in what the
fields *meant*: Exodus action 1 packs as `0x0170` (round 0, range 7, target 1)
while the other game's action 0 packs as `0x8100`, giving target 129 in a
two-token battle.

The versions settle it. `fixtures/games/exodus` is **2.81**;
`fixtures/games/all-computer-players` is **2.66** — and the older symbol is
named `BTLREC26`. Read with the right layout, the record that appeared to have
a token crossing five squares resolves to token 0 moving (1,4) to (2,4) to
(3,4) and token 1 moving (8,5) to (7,5), over rounds 0, 0, 1, 1, with the final
action carrying its two kills.

### A second, unrelated correction

The firing-range test asserted `range <= 4`, on the reasoning that beams reach
3 and torpedoes 4. That is wrong: the Jihad Missile and its three larger
cousins have `range_max: 5` in the component table. Exodus never fires beyond 4
only because it never researches missiles. The bound is now 5.

## The starting-square table

`rgbrcStart` at `10f0:0000` is a flat concatenation of the layouts for 1 to 12
players; the row for `n` begins at `n(n-1)/2`. It is transcribed in
`stars-core::battle::START_SQUARES` and verified byte for byte against the
binary — all 78 bytes. An earlier revision stopped at eight players; the rows
for 9 to 12 are now included.

### It does not fit the 2.66 game's three-player battles

Two-player battles agree across both fixture games. A three-player battle does
not: `2429` battle `0x0c02` in the 2.66 game puts its three sides at (3,1),
(1,8) and (8,6), where the 2.7j table's three-player row is (4,1), (8,8),
(1,8). Only one of the three matches.

The other two squares do appear in the table, but in rows for larger player
counts — (8,6) in the seven-player row, (3,1) in the six- and eight-player
rows — and no single row of the 2.7j table contains all three.

Since the binary this project reads **is** 2.7j, its table cannot be wrong for
2.7j; and the same version split that governs the action record layout above
plausibly reaches this table too. Confirming that needs a 2.6 binary, which
the project does not have. The starting-square test therefore runs on the
Exodus corpus, which is 2.81, with a comment saying why.

This is worth remembering more generally: two format details have now turned
out to be version-dependent, and every format in `docs/formats` was recovered
from Exodus or from the 2.7j binary alone.
