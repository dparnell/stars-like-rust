# Space objects — `rtThing` (type id 43)

A *thing* is any space object that is neither a planet nor a fleet: a
**minefield**, a **mineral packet** in flight, a **wormhole**, or a **mystery
trader**. They appear in `.hst`, `.mN` and `.hN` files.

Module: [`crate::thing`](../../crates/stars-formats/src/thing.rs).
Verified against `fixtures/incoming/turn{0,1}/Game.hst` and the exodus `.m6`
turn sequence.

## Section layout (count + records)

The objects are written as a small two-part section, both parts using type id
**43** (`rtThing`, aliased to `rtLogThingByteParam` for the body records):

1. one record with `cb = 2`, payload = a little-endian **`u16` count**, then
2. that many records with `cb = 18` (`sizeof(THING)`), each a full `THING`.

This was recovered from the reconstructed source
([`sirgwain/stars-decompile`](https://github.com/sirgwain/stars-decompile)),
`file.c` *"Load things"* (`cThingFile = *(uint16_t*)rgbCur;` then a
`for (i = 0; i < cThingFile; i++)` loop that `memmove`s each following record) and
`save.c` *"Count and write things"* (`WriteRt(rtThing, sizeof(int16), &c)` then
`WriteRt(rtLogThingByteParam, sizeof(THING), lpth)` per visible thing).

**Verified:** in every fixture the declared count equals the number of 18-byte
records that follow it (fresh `.hst` = 4; exodus late-game `.m6` = 1–3).

## `THING` record (18 bytes)

From the NB09 debug struct `THING` (`stars-asm`, `structs.h`):

```c
typedef struct _thing {
    union {
        uint16_t idFull;                    /* +0x00 */
        uint16_t id : 9, iplr : 4, ith : 3; /*   id | player | subtype */
    };
    POINT pt;         /* +0x02  int16 x, int16 y */
    union {           /* +0x06  10-byte subtype payload, selected by `ith` */
        uint8_t  rgb[10];
        THMINE   thm;   /* ith=0 minefield      */
        THPACK   thp;   /* ith=1 mineral packet */
        THWORM   thw;   /* ith=2 wormhole (8 bytes) */
        THTRADER tht;   /* ith=3 mystery trader */
    };
    uint16_t turn;    /* +0x10 */
} THING; /* size = 0x12 */
```

`ith` (`ThingType`): `0 = Minefield`, `1 = MineralPacket`, `2 = Wormhole`,
`3 = MysteryTrader`.

### Subtype payloads (union at +0x06)

```c
typedef struct _thmine {   /* ith=0 */
    int32_t  cMines;       /* +0 number of mines */
    uint16_t grbitPlr;     /* +4 players who have detected it */
    uint8_t  iType;        /* +6 0=std, 1=heavy, 2=speed-bump */
    uint8_t  fDetonate;    /* +7 armed to detonate */
    uint16_t grbitPlrNow;  /* +8 players who can currently see it */
} THMINE;                  /* 10 bytes */

typedef struct _thpack {   /* ith=1 */
    uint16_t idPlanet:10, iWarp:4, fMoved:1, fInclude:1; /* +0 */
    int16_t  rgwtMin[3];   /* +2 ironium/boranium/germanium (kt) */
    uint16_t wtMax:14, iDecayRate:2;                     /* +8 */
} THPACK;                  /* 10 bytes */

typedef struct _thworm {   /* ith=2 */
    uint16_t iStable:2, cLastMove:10, fDestKnown:1, fInclude:1; /* +0 */
    uint16_t grbitPlr;     /* +2 players who have detected it */
    uint16_t grbitPlrTrav; /* +4 players who have traversed it */
    uint16_t idPartner;    /* +6 idFull of the far endpoint */
} THWORM;                  /* 8 bytes */

typedef struct _thtrader { /* ith=3 */
    POINT    ptDest;       /* +0 destination x,y */
    uint16_t iWarp:4, fInclude:1, unused:11; /* +4 */
    uint16_t grbitPlr;     /* +6 players who have detected it */
    uint16_t grbitTrader;  /* +8 players who have met/traded */
} THTRADER;                /* 10 bytes */
```

## Semantic confirmations (real data)

- **Wormholes link in pairs.** In `turn0/Game.hst` the four objects are all
  wormholes (`ith=2`) whose `idPartner` cross-references the sibling endpoint:
  id 0 ↔ id 1 (`idPartner = 0x4001`/`0x4000` = `idFull` of the partner with
  `ith=2`), id 2 ↔ id 3. Their positions lie inside the universe bounds.
- **Mystery-trader `grbitTrader` tracks the player.** In exodus (`.m6`, player
  index 5) the MT decodes with `grbitTrader = 0x20 = 1 << 5`, i.e. "player 6 has
  met the trader", and its scan visibility (`grbitPlr`) grows over the turns.
- **Mineral packets** decode plausible cargo (e.g. `[69, 0, 0]` kt ironium) with
  a target-planet field.

## Status & open items

- Section framing (count + N×18) and all four subtype layouts: **verified**.
- The exact meaning of a few sub-fields (packet `iDecayRate` classes, minefield
  `iType` beyond the three known classes, wormhole `iStable` scale) is documented
  from the struct names but not yet tied to worked gameplay vectors — those
  belong to the simulation phase (`stars-core`).
- The decoder is a read-only interpreted view; byte-exact write-back still goes
  through the cipher container in `file.rs`.

**Source:** NB09 debug structs (`stars-asm/decompiled/structs.h`,
`enums.h::ThingType`); `sirgwain/stars-decompile` `file.c` / `save.c`.
