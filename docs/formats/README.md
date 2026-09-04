# Format specs

One spec per on-disk file format, created from
`../templates/format-layout-template.md`.

> **Authoritative reference:** [`sirgwain/stars-asm`](https://github.com/sirgwain/stars-asm)
> extracts the **CodeView NB09 debug symbols** from a debug build of `Stars! 2.7j`,
> yielding the game's *actual* struct/enum/field names and offsets — the highest
> authority available. Its sibling [`sirgwain/stars-decompile`](https://github.com/sirgwain/stars-decompile)
> reconstructs the C source (`types.h`/`enums.h`, `file.c`/`save.c`). Together they
> supersede the earlier tool-derived names (TotalHost, starsapi, stars-4x). See
> `nb09-structs.md` and `record-types.md`.
>
> Our shipped `binary/STARS!.EXE` is the stripped retail build (no debug data);
> the NB09 symbols come from stars-asm's separate debug image but describe the
> same program lineage, so they apply to our files.

Shared foundation:

- `nb09-structs.md` — the **authoritative on-disk record structs** recovered
  from the NB09 debug symbols (`HDR`, `RTBOF`, `RTPLANET`, `RTSHDEF`/`HS`, `PROD`,
  `SCORE`/`SCOREX`, `RTHISTHDR`, the `.xN` `rtLog*`/`RTXFER*` order records, the
  `DtFileType` tags), each mapped to our module and flagged confirmed/new.
- `record-types.md` — the **record-type (block) registry**
  (`rt*` ids → our `BlockType`), the header (`RTBOF`) layout and the `dt`
  file-type table, taken from the decompiled `enums.h`/`save.c`.
- `blocks.md` — **block framing** shared by every format (implemented &
  round-trip tested in `stars-formats::block`); also tracks the payload
  encryption.
- `file-io-map.md` — **binary map**: where the container / block / cipher / PRNG
  logic lives in `STARS!.EXE` (Ghidra addresses), mapped to our modules. Grounds
  the whole format layer in the actual shipped code. See also `../rng/prng.md`.
- `strings.md` — the **packed-string codec** used for user-supplied text (race
  names, etc.), implemented & tested in `stars-formats::strings`.
- `writing.md` — **writing files from scratch**: the per-record encoders, what
  the record-by-record round trip over 1,576,529 blocks found, and the
  block-for-block comparison against a real host file and its turn files.

Per-format (payload record layouts, populated as decoded in Step 2):

- `xy.md` — universe definition (`.xy`): header + game-info decoded; planet
  region investigated (4-byte packing, still open)
- `hst.md` — host & player state (`.hst` / `.mN`): block inventory decoded;
  player, planet, fleet and design records decoded (see below)
- `player.md` — player blocks (type 6) in `.hst`/`.mN`: header + names
  **verified**, embedded race struct reused from `race-r.md`
  (`stars-formats::player`)
- `planet.md` — planet blocks (types 13/14/15) shared by `.hst`/`.mN`/`.hN`:
  **fully decoded & verified** (`stars-formats::planet`)
- `fleet.md` — fleet blocks (types 16/17/18) shared by `.hst`/`.mN`/`.hN`:
  **fully decoded & verified** (`stars-formats::fleet`)
- `waypoint.md` — waypoint blocks (type 20) that follow full fleets:
  **decoded & verified** (`stars-formats::waypoint`)
- `design.md` — ship/starbase design blocks (type 26) shared by `.hst`/`.mN`:
  **fully decoded & verified** (`stars-formats::design`)
- `battleplan.md` — battle-plan blocks (type 30) in `.hst`/`.mN`:
  **decoded & verified** (`stars-formats::battleplan`)
- `production.md` — production-queue blocks (types 28/29) in `.hst`/`.mN`/`.xN`:
  **decoded & verified** (`stars-formats::production`)
- `score.md` — player-scores blocks (type 45) in `.mN`/`.hN`:
  **decoded & verified** (`stars-formats::score`)
- `thing.md` — space objects (type 43: minefields, mineral packets, wormholes,
  mystery traders) in `.hst`/`.mN`/`.hN`: count + `THING` records
- `battle.md` — battle recordings (types 31/39: the VCR) in `.mN`:
  **decoded & verified** (`stars-formats::battle`)
  **decoded & verified** (`stars-formats::thing`)
- `orders-x.md` — player orders (`.xN`, the order log): the log header
  (`RTLOGHDR`, type 9) and the common `rtLog*` operations (waypoints, cargo,
  research, planet routing, fleet order edits) are **decoded & verified**
  (`stars-formats::orders`), backed by the 40-turn `EXODUS.X6` fixtures.
- `race-r.md` — race definition (`.rN`): record largely decoded
- player history (`.hN`): container round-trips; the **history header**
  (`RTHISTHDR`, type 32) is decoded & verified (`stars-formats::history`) and the
  score record (type 45) is decoded; other record layouts not yet decoded.
