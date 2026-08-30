# Authoritative on-disk record structs (NB09 debug symbols)

**Source:** `sirgwain/stars-asm` — a reverse-engineering toolkit that extracts
the **CodeView NB09 debug information embedded in a debug build of
`Stars! 2.7j`** (`dasm/input/stars.exe`, 4.2 MB, contains an `NB09` directory at
file offset `0x35e0c0`). The debug build carries the compiler's original symbol
and type database, so the struct/enum/field **names and offsets** below are the
game's *actual* declarations rather than a reconstruction.

> Note: our shipped `binary/STARS!.EXE` (3.15 MB, the retail 2.7 build) contains
> **no** NB09/NB10 debug data — it is stripped. `stars-asm`'s `stars.exe` is a
> different, larger image with symbols. The two are the same program lineage, so
> its recovered types apply to our files.

This supersedes the earlier third-party references (`starsapi`, TotalHost's
`Stars*.pl/pm`, the stars-4x `Structures/*.xml`) as the **top authoritative
format reference**. Those remain useful cross-checks; where they disagree with
the NB09 structs, the NB09 structs win.

The full headers live (git-ignored) in `tmp/stars-asm/decompiled/structs.h` and
`enums.h`. This doc captures the structs relevant to the **on-disk file
formats**, each mapped to the module that implements it and flagged as
`confirmed` (matches our verified decoder) or `new` (previously open).

---

## File-type tag (`DtFileType`)

The header's `dt` byte (`RTBOF.dt`, low byte of payload `+0x0e`) selects the
file kind:

| `dt` | name (`enums.h`) | extension | our [`FileType`](../../crates/stars-formats/src/header.rs) |
|-----:|------------------|-----------|-----------|
| 0 | `dtXY`   | `.xy`  | `Universe` |
| 1 | `dtLog`  | `.xN`  | `Orders`   |
| 2 | `dtHost` | `.hst` | `Host`     |
| 3 | `dtTurn` | `.mN`  | `Turn`     |
| 4 | `dtHist` | `.hN`  | `History`  |
| 5 | *(omitted from the decompiled enum)* | `.rN` | `Race` |

The `DtFileType` enum in the debug symbols only enumerates 0–4 (the values its
decompiled code paths reference). **Race files really do carry `dt = 5`** — every
`fixtures/r/*.r1` header decodes to 5 — so our `FileType::Race => 5` mapping is
correct and verified against real files. `confirmed`.

## Block header (`HDR`)

```c
typedef struct _hdr {
    uint16_t cb : 10,   /* +0x00 payload size, low 10 bits  */
        rt : 6;         /*        record type, high 6 bits  */
} HDR;                  /* size=0x2 */
```

Confirms the framing word `type << 10 | (size & 0x3ff)` in
[`stars-formats::block`](../../crates/stars-formats/src/block.rs). `confirmed`.

## File header (`RTBOF`, `rt = 8`)

```c
typedef struct _rtbof {
    char     rgid[4];      /* +0x00 "J3J3" magic                       */
    int32_t  lidGame;      /* +0x04 per-game id                        */
    uint16_t verInc : 5,   /* +0x08 version increment                  */
        verMinor : 7,      /*       version minor                      */
        verMajor : 4;      /*       version major                      */
    uint16_t turn;         /* +0x0A turn counter                       */
    int16_t  iPlayer : 5,  /* +0x0C player index                       */
        lSaltTime : 11;    /*       encryption salt                    */
    uint16_t dt : 8,       /* +0x0E file type (DtFileType)             */
        fDone : 1,         /*       .x: turn submitted                 */
        fInUse : 1,        /*       host is using this file            */
        fMulti : 1,        /*       .m: multiple turns included        */
        fGameOverMan : 1,  /*       game over                          */
        fCrippled : 1,     /*       shareware/crippled edition         */
        wGen : 3;          /*       generation counter                 */
} RTBOF;                   /* size=0x10 */
```

Confirms [`FileHeader`](../../crates/stars-formats/src/header.rs) field-for-field
(magic, `game_id`, the packed version, turn, `iPlayer`/`lSaltTime` split, and the
`dt`+flags word feeding the cipher seed). The only field we don't yet surface is
`wGen` (bits 13–15). `confirmed`.

## Planet block (`RTPLANET`, `rt = 13/14/15`)

```c
typedef struct _rtplanet {
    int16_t  id : 11,       /* +0x00 planet id                */
        iPlayer : 5;        /*       owner (31 = unowned)      */
    uint16_t det : 7,       /* +0x02 detail level             */
        fHomeworld : 1,     /*   @b7                           */
        fInclude : 1,       /*   @b8                           */
        fStarbase : 1,      /*   @b9                           */
        fIncEVO : 1,        /*   @b10 terraformed (orig env)   */
        fIncImp : 1,        /*   @b11 installations present    */
        fIsArtifact : 1,    /*   @b12                          */
        fIncSurfMin : 1,    /*   @b13 surface minerals         */
        fRouting : 1,       /*   @b14 route destination        */
        fFirstYear : 1;     /*   @b15                          */
} RTPLANET;                 /* size=0x4 */
```

Matches [`PlanetRecord`](../../crates/stars-formats/src/planet.rs) bit-for-bit
(the `id:11 | iPlayer:5` split we corrected earlier, and every `fInc*`/flag bit),
and matches [`PlanetHeader`](../../crates/stars-formats/src/records.rs).
`confirmed`.

## Ship/starbase design (`RTSHDEF`, `rt = 26`) and slot (`HS`)

```c
typedef struct _rtshdef {
    uint16_t det : 8,   /* +0x00 detail                    */
        fInclude : 1,   /*   @b8                            */
        fFree : 1,      /*   @b9                            */
        ishdef : 5,     /*   @b10 design slot index         */
        fGift : 1;      /*   @b15                           */
    uint8_t  ihuldef;   /* +0x02 hull index                */
    uint8_t  ibmp;      /* +0x03 picture index             */
    uint16_t wtEmpty;   /* +0x04 empty weight / dp union   */
    uint8_t  chs;       /* +0x06 slot count                */
    uint16_t turn;      /* +0x07 turn designed             */
    uint32_t cBuilt;    /* +0x09 number built              */
    uint32_t cExist;    /* +0x0D number existing           */
    HS       rghs[0];   /* +0x11 per-slot equipment        */
} RTSHDEF;              /* size=0x11 + chs*4 */

typedef struct _hs {
    HullSlotType grhst;    /* +0x00 slot category           */
    uint16_t     iItem : 8,/* +0x02 item id                 */
        cItem : 8;         /*       item count              */
} HS;                      /* size=0x4 */
```

Confirms [`DesignRecord`](../../crates/stars-formats/src/design.rs) and its
`Slot`. `confirmed`.

## Production queue item (`PROD`, `rt = 28/29`)

```c
typedef struct _prod {
    uint32_t cItem : 10,   /* +0x00 quantity                */
        iItem : 7,         /*       item id                 */
        ... ;
} PROD;                    /* size=0x4 */
```

Confirms [`ProductionQueueRecord`](../../crates/stars-formats/src/production.rs).
`confirmed`.

## Player scores (`SCORE`/`SCOREX`, `rt = 45`)

```c
typedef struct _score {
    int32_t  lScore;       /* +0x00 */
    int32_t  cResources;   /* +0x04 */
    int16_t  cPlanet;      /* +0x08 */
    int16_t  cStarbase;    /* +0x0A */
    uint16_t rgcsh[3];     /* +0x0C unarmed/escort/capital ship counts */
    int16_t  cTechLevels;  /* +0x12 */
} SCORE;                   /* size=0x14 */

typedef struct _scorex {
    uint16_t iPlayer : 5, fValid : 1, grbitVC : 8, fWinner : 1, fHistory : 1; /* +0x00 */
    uint16_t iRank; /* or turn */                                             /* +0x02 */
    SCORE    score;                                                           /* +0x04 */
} SCOREX;                  /* size=0x18 */
```

Confirms [`ScoreRecord`](../../crates/stars-formats/src/score.rs) (24-byte
`SCOREX`: player/VC word, rank, then the 20-byte `SCORE`). `confirmed`.

## History-file header (`RTHISTHDR`, `rt = 32`)

```c
typedef struct _rthisthdr {
    int16_t cPlanet;      /* +0x00 number of planet records that follow */
    int16_t cPlanetExtra; /* +0x02 low 12 bits of the player's rgplr word */
} RTHISTHDR;              /* size=0x4 */
```

Every `.hN` (`dtHist`) file begins with this record right after the plaintext
header; `file.c` rejects the file if it is missing. `cPlanet` is the number of
planet blocks the file carries (the planets that player knows), **not** the
universe total. Implemented as
[`HistoryHeader`](../../crates/stars-formats/src/history.rs) and verified:
`cPlanet` equals the planet-block count in `Game.h1/2/3` and `tutorial.h1`.
`new` (previously an open item).

## Order/log records (`.xN`, `dtLog`)

A submitted-orders file is **not** a flat set of one-record-per-type blocks like
`.hst`/`.mN`. Instead it is a *log*: a sequence of operation records whose block
`rt` ids come from the `rtLog*` family (each mutating the host's in-memory state
as it is replayed). The relevant ids (`enums.h`):

| `rt` | name | operation |
|-----:|------|-----------|
| 1 | `rtLogCargoXfer8`  | cargo transfer, int8 quantities |
| 2 | `rtLogCargoXfer16` | cargo transfer, int16 quantities |
| 3 | `rtLogFleetOrderDelete` | delete waypoint order(s) |
| 4 | `rtLogFleetOrderInsert` | insert a waypoint order |
| 5 | `rtLogFleetOrderUpdate` | overwrite a waypoint order |
| 23 | `rtLogFleetCargoXfer` | fleet cargo transfer |
| 24 | `rtLogFleetSplit` | split a fleet |
| 25 | `rtLogCargoXfer32` | cargo transfer, int32 quantities |
| 27 | `rtLogShDef` | create/update/delete a ship design |
| 29 | `rtLogPlanetProdQ` | set/clear a planet's production queue |
| 34 | `rtLogResearch` | research settings (`pctResearch` + `iTechCur`) |
| 35 | `rtLogPlanetRouting` | planet routing/starbase/infra bits |
| 38 | `rtLogRelations` | player-relations table |

The **payload shapes** used by several of these operations are declared as the
following structs (the `rt` id is the operation above, not a per-struct type):

```c
typedef struct _rtchgname {                /* rename a fleet/planet */
    int16_t id;              /* +0x00 */
    int16_t grobj;           /* +0x02 object class */
    uint8_t rgb[33];         /* +0x04 packed name */
} RTCHGNAME;                 /* size=0x25 */

typedef struct _rtChgPlanetLong {          /* planet order flags/route/fling */
    int16_t  id;             /* +0x00 */
    uint32_t fNoResearch : 1, idFling : 10, iWarpFling : 4, idRoute : 10, unused : 7; /* +0x02 */
} RTCHGPLANETLONG;           /* size=0x6 */

typedef struct _rtChgProdQ { int16_t id; PROD rgprod[0]; } RTCHGPRODQ; /* production-queue change */

/* cargo-transfer payloads, one per quantity width (see rtLogCargoXfer8/16/32) */
typedef struct _rtxfer  { uint16_t id1, id2; uint8_t grobj1:4, grobj2:4; uint8_t  grbitItems; char    rgcQuan[1]; } RTXFER;  /* size=0x7 */
typedef struct _rtxferf { uint16_t id1, id2; uint8_t grobj1:4, grobj2:4; uint16_t grbitItems; int16_t rgcQuan[1]; } RTXFERF; /* size=0x9 */
typedef struct _rtxferl { uint16_t id1, id2; uint8_t grobj1:4, grobj2:4; uint8_t  grbitItems; int32_t rgcQuan[1]; } RTXFERL; /* size=0xa */
typedef struct _rtxferx { uint16_t id1, id2; uint8_t grobj1:4, grobj2:4; uint8_t  grbitItems; int16_t rgcQuan[1]; } RTXFERX; /* size=0x8 */
```

`new` (documentation) — this maps out the `.xN` log format at the record-type
level. Typed decoders are deferred until we capture `.xN` fixtures that actually
contain such operations (the current `.x1` fixtures carry only a couple of
records), because the log is a replay stream rather than a static table.

---

## Status against our decoders

| Record | NB09 struct | our module | status |
|--------|-------------|-----------|--------|
| block header | `HDR` | `block` | confirmed |
| file header  | `RTBOF` | `header` | confirmed |
| planet | `RTPLANET` | `planet`, `records` | confirmed |
| design | `RTSHDEF`+`HS` | `design` | confirmed |
| production | `PROD` | `production` | confirmed |
| waypoint | (written field-by-field) | `waypoint` | confirmed elsewhere |
| battle plan | (`btlplan`) | `battleplan` | confirmed elsewhere |
| scores | `SCORE`/`SCOREX` | `score` | confirmed |
| history header | `RTHISTHDR` | `history` | **new (this pass)** |
| orders/log | `rtLog*` ops + `RTCHGNAME`/`RTCHGPLANETLONG`/`RTCHGPRODQ`/`RTXFER*` | — | documented, decoder pending |

Still not typed anywhere: messages (`rtMsg` 12 / `rtMsgFilt` 33), the
tagged-union `THING`/object records (`rt = 43`), and the `.xN` order bodies above.
