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
| 1 | 1 | `brcDest` | square it moved to, packed as above |
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

- `grfWeapon` bit meanings (beam vs torpedo vs sapper) are not yet pinned down.
- The tactics and movement words are decoded as packed values but their
  sub-fields are not yet mapped to the battle-plan UI.
- The version difference matters: `BTLREC26` is a 2.6-era variant with the same
  size but a different field split, converted on load by `UpdateBattleRecords`
  when the file's minor version is below 80. Our fixtures are 2.81, so only the
  modern layout is exercised.
