# Stars! record (block) types — authoritative registry

Every Stars! file (`.xy`, `.mN`, `.hN`, `.xN`, `.rN`, `.hst`) is a flat sequence
of *records* (a.k.a. *blocks*). Each record starts with a little-endian 16-bit
header word: the **high 6 bits** are the record type id (`rt`), the **low 10
bits** are the payload byte count (`cb`). See `blocks.md` for the framing and the
stream cipher.

This table is the **authoritative** record-type registry, taken from the
decompiled Stars! source
([`sirgwain/stars-decompile`](https://github.com/sirgwain/stars-decompile),
`enums.h` `RecordType`), which is a reconstruction of the real game's C source
and therefore supersedes the earlier community/tool-derived names
(TotalHost, starsapi, stars-4x). The `rt*` identifiers are the original code's
names; the "our name" column is the [`BlockType`](../../crates/stars-formats/src/block.rs)
variant.

| id | `rt*` name | our `BlockType` | notes |
|----|------------|-----------------|-------|
| 0  | `rtEOF` | `FileFooter` | footer / terminator, `cb=2`, data = year; **plaintext** |
| 1  | `rtLogCargoXfer8` | `Other(1)` | host log: cargo transfer, int8 quantities |
| 2  | `rtLogCargoXfer16` | `Other(2)` | host log: cargo transfer, int16 quantities |
| 3  | `rtLogFleetOrderDelete` | `Other(3)` | order: delete 1–2 waypoints |
| 4  | `rtLogFleetOrderInsert` | `Other(4)` | order: insert waypoint at index |
| 5  | `rtLogFleetOrderUpdate` | `Other(5)` | order: overwrite waypoint at index |
| 6  | `rtPlr` | `Player` | full player (`.mN`/`.hst`) / race (`.rN`) |
| 7  | `rtGame` | `Game` | game settings; also the `.xy` game-info header |
| 8  | `rtBOF` | `FileHeader` | file header; seeds the stream cipher; **plaintext** |
| 9  | `rtFileHash` | `FileHash` | turn-submit hash marker (seen in `.xN`) |
| 10 | `rtLogFleetFlagBit9` | `Other(10)` | order: fleet flag bit toggle |
| 11 | `rtLogFleetOrderAttrNib` | `Other(11)` | order: set waypoint attr nibble |
| 12 | `rtMsg` | `Message` | message |
| 13 | `rtPlanet` | `Planet` | full planet record |
| 14 | `rtPlanetB` | `PartialPlanet` | partial planet record |
| 15 | (minimal planet) | `MinimalPlanet` | minimal planet record |
| 16 | `rtFleetA` | `Fleet` | full fleet record |
| 17 | `rtFleetB` | `PartialFleet` | partial fleet record |
| 19 | `rtOrderA` | `WaypointTask` | waypoint task / order-like |
| 20 | `rtOrderB` | `Waypoint` | waypoint |
| 21 | `rtString` | `String` | generic compressed user string (fleet & player names) |
| 22 | `rtSel` | `Selection` | UI selection state |
| 23 | `rtLogFleetCargoXfer` | `Other(23)` | host log: fleet cargo transfer |
| 24 | `rtLogFleetSplit` | `Other(24)` | order: split fleet |
| 25 | `rtLogCargoXfer32` | `Other(25)` | host log: cargo transfer, int32 quantities |
| 26 | `rtShDef` | `Design` | ship / starbase design |
| 27 | `rtLogShDef` | `Other(27)` | order: design create/update/delete |
| 28 | `rtProdQ` | `ProductionQueue` | production queue |
| 29 | `rtLogPlanetProdQ` | `ProductionQueueChange` | order: set/clear planet production queue |
| 30 | `rtBtlPlan` | `BattlePlan` | battle plan |
| 31 | `rtBtlData` | `Battle` | battle recording data (VCR) — **decoded**, see `battle.md` |
| 32 | `rtHistHdr` | `HistoryHeader` | history-file (`.hN`) header |
| 33 | `rtMsgFilt` | `MessagesFilter` | message filter bitfield |
| 34 | `rtLogResearch` | `ResearchChange` | order: research settings |
| 35 | `rtLogPlanetRouting` | `PlanetChange` | order: planet routing/starbase/infra |
| 36 | `rtChgPassword` | `ChangePassword` | order: change password |
| 37 | `rtLogFleetMerge` | `Other(37)` | order: merge fleets |
| 38 | `rtLogRelations` | `Other(38)` | order: change player relations |
| 39 | `rtContinue` | `Other(39)` | battle continuation — **decoded**, see `battle.md` |
| 40 | `rtPlrMsg` | `PlayerMessage` | per-player message |
| 41 | `rtAiData` | `Other(41)` | opaque AI data blob |
| 42 | `rtLogFleetPlan` | `Other(42)` | order: set fleet battle plan |
| 43 | `rtThing` / `rtLogThingByteParam` | `Object` | space object: minefield, packet, wormhole, MT, salvage |
| 44 | `rtLogFleetName` | `RenameFleet` | order: rename fleet |
| 45 | `rtScore` | `PlayerScores` | player scores |
| 46 | `rtLogPlayerZpq1` | `Other(46)` | host-only opaque per-player blob |

Notes:

- Ids without a dedicated `BlockType` variant are preserved as `Other(id)` so the
  container always round-trips regardless of registry completeness.
- Many of the `rtLog*` types are **order / host-log** records that only appear in
  `.xN` order files and in host processing; the structural records we currently
  decode (`Player`, `Planet`, `Fleet`, `Design`, `Waypoint`, `BattlePlan`,
  `ProductionQueue`, `PlayerScores`, `Object`) are the ones present in `.hst`/`.mN`.
- `rt=0` (`rtEOF`) doubles as the in-stream terminator: the original reader stops
  when it sees a record type of 0.

## Header record (`rtBOF` = 8) — `RTBOF`

The decompiled `RTBOF` struct (16 bytes, plaintext) confirms our header layout:

```c
typedef struct _rtbof {
    char     rgid[4];       // "J3J3" magic
    int32_t  lidGame;       // game id
    uint16_t verInc:5, verMinor:7, verMajor:4; // wVersion (0x2a60)
    uint16_t turn;          // turn
    // +0x0c: iPlayer:5 | lSaltTime:11  (salt masked to 11 bits, << 5)
    // +0x0e: dt:8 | fDone:1 | fInUse:1 | fMulti:1 | fGameOverMan:1 | fCrippled:1 | wGen:3
} RTBOF;
```

The stream cipher is seeded from `lidGame`, `lSaltTime`, `turn`, `iPlayer` and
`fCrippled` (`SetFileXorStream` in `save.c`). See `blocks.md` for details.

### Document type (`dt`, high byte of +0x0e)

| dt | file | our `FileType` |
|----|------|----------------|
| 0  | `.xy` universe | `Universe` |
| 1  | `.xN` orders (submitted) | `Orders` |
| 2  | `.hst` host | `Host` |
| 3  | `.mN` player turn | `Turn` |
| 4  | `.hN` history | `History` |
| 5  | `.rN` race | `Race` |

**Source:** `sirgwain/stars-decompile` `enums.h` (`RecordType`, `dt*`),
`save.c` (`WriteRt`, `WriteBOF`), `notes/data-structures.md`.
