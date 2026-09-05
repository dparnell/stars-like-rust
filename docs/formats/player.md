# Player block (type 6) — full player record

**Status:** header + names verified; embedded race struct reused from
`race-r.md`. Implemented in `stars-formats::player`
([`PlayerRecord`](../../crates/stars-formats/src/player.rs)) and verified against
`fixtures/incoming/turn0/Game.hst` and the tutorial host file.

The type-6 block is **dual-purpose**:

- In a `.rN` **race file** it holds a bare race definition — decoded by
  `RaceRecord` (see `race-r.md`).
- In a `.mN` **player file** or a `.hst` **host file** it is a *full player*
  record: an 8-byte header, then the very same 0x68-byte race struct, then a
  player-relations table, then the packed names.

Because the race struct begins at offset 8, every race field lands at the *same
absolute offset* as in a `.rN` file, so `RaceRecord::from_payload` decodes the
race portion of a player block directly (this is how `PlayerRecord` embeds it).

Layout recovered from the stars-4x `starsapi` project (`PlayerBlock.java`).

## Header (8 bytes)

| Offset | Bits      | Field                                            |
|--------|-----------|--------------------------------------------------|
| 0      | all       | player number (0-based)                          |
| 1      | all       | ship design count                                |
| 2–3    | 10 bits   | planet count (`d[2] + ((d[3] & 3) << 8)`)        |
| 3      | bits 2–7  | must be 0 (validated by starsapi)                |
| 4–5    | 10 bits   | fleet count (`d[4] + ((d[5] & 3) << 8)`)         |
| 5      | bits 4–7  | starbase design count                            |
| 6      | bit 2     | `fullData` flag (race struct present)            |
| 6      | bits 0–1  | must be `3` (validated by starsapi)              |
| 6      | bits 3–7  | logo index                                       |
| 7      | all       | flags byte — `1` for a human player, `39` for AI |

## `fullData` region (present when bit 2 of byte 6 is set)

| Offset      | Field                                                       |
|-------------|-------------------------------------------------------------|
| 8 .. 0x70   | 0x68-byte race struct (see `race-r.md`; same absolute offsets) |
| 0x70        | player-relations length `n`                                 |
| 0x71 .. +n  | player-relations table (`0` neutral, `1` friend, `2` enemy) |
| then        | packed singular name field (`[len][packed]`)                |
| then        | packed plural name field (`[len][packed]`)                  |

When the plural name length is 0 an extra padding byte follows (16-bit
alignment). A non-`fullData` block stores the names right after the 8-byte
header.

## Verified values (`Game.hst`, turn 0)

| Player | Name      | PRT  | Ship designs | Starbase designs |
|--------|-----------|------|--------------|------------------|
| 0      | Humanoid / Humanoids | JOAT | 6 | 1 |
| 1      | Tritizoid | SS   | 3            | 1                |
| 2      | Golem     | SS   | 3            | 1                |

## Open questions

- Meaning of the flags byte at offset 7 (`1` human vs `39` AI) — likely a
  human/AI + submission-state field.
- The `planets` field reads 128 for the host's player 0 (the full galaxy); its
  exact semantics per player/file-type still need confirming across saves.

## The default production queue (`PLAYER.zpq1`, offsets 86-111)

The last 26 bytes of the fixed region are a `ZIPPRODQ1`: the production queue a
planet starts with when it becomes this player's, and whether it starts exempt
from the research skim.

```c
typedef struct _zipprodq1 {
    uint8_t fNoResearch;   /* +0x00 */
    uint8_t cpq;           /* +0x01  how many entries follow */
    PRODQ1  rgpq[12];      /* +0x02 */
} ZIPPRODQ1;               /* 26 bytes, at PLAYER +0x56 */

typedef struct _prodq1 { uint16_t mdIdle : 6, cQuan : 10; } PRODQ1;
```

The entry packing is **not** the four-byte `PROD` of a planet's own queue: two
bytes, six bits of item and ten of quantity, and only planetary items.

**Verified against the corpus.** Of the 7,040 full-data player blocks in the
fixtures, 49 carry a non-empty queue, and every one of them decodes to the same
thing:

```text
100 x factories, 100 x mines, 100 x defences,  no research
```

which is the queue a Stars! player conventionally sets for new colonies. Three
item ids, all planetary; 147 quantities, every one of them 100.

The same record is what the `.xN` order operation `rtLogPlayerZpq1` (type 46)
carries, truncated to the `2 * cpq + 2` bytes it uses — see `orders-x.md`.

### What the game does with it

When a planet changes hands — settled or taken — `turn2.c` sets the planet's
`fNoResearch` from this record and builds its queue from these entries, with two
racial filters: an **Alternate Reality** race skips items `<= 2`, so it queues no
planetary installation at all, and a **Claim Adjuster** skips items 4 and 5, so
it queues no terraforming. Those are the same two filters
`stars_core::ground::template_allows` already applies to the build list. A queue
that filters down to nothing leaves the planet with none.

Implemented as `stars_core::orders::apply_default_queue`, called wherever a
planet gains an owner.
