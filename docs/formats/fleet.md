# Format: fleet blocks (`rtFleetA`/`B`/`C`, types 16 / 17 / 18)

- **Status:** **decoded & verified** against real host files; typed read-only
  view implemented in `stars-formats::fleet` (`FleetRecord`, `fleet_records`)
- **Reference used:** TotalHost `StarsFleet.pl` (Rick Steeves), derived from the
  `starsapi` project; offsets cross-checked against
  `fixtures/incoming/turn0/Game.hst`
- **Appears in:** `.hst`/`.mN` (type 16, full owned fleets), `.mN`/`.hN`
  (types 17/18, partial enemy fleets). Followed in the file by their waypoint
  blocks (19/20) and optional fleet-name block (21).
- **Implemented in:** `stars-formats::fleet`; tests in `tests/fleet_files.rs`

## The fleet's name (`rtString`, type 21)

A fleet the player has renamed carries the name in a block of its own,
**immediately after that fleet's waypoint blocks**:

```text
[ fleet (16) ]
[ waypoint (20/19) ] x cord
[ name (21) ]          <- only when the fleet has been renamed
```

No file in this repository's fixtures contains one — nobody renamed a fleet in
any of the captured games — so this is recovered from the binary rather than
fixture-verified. `WriteFleet` (`1070:8776`) writes the fleet block, calls
`WriteOrders`, then tests `FLEET.lpszName` at offset `0x78` and calls
`WriteRtString` only when it is set. The loader does the mirror image: after a
fleet's orders it reads one more record and takes it as the name if it is this
type, leaving the name empty otherwise.

### The payload: a user string

`WriteRtString` (`1070:87b4`) is short enough to give in full, and the
disassembly matches the reconstructed C line for line:

```c
if (lpsz != NULL && *lpsz != '\0') {
    cOut = 0x1f;                                   // 31 bytes of packed budget
    if (FCompressUserString(lpsz, rgb + 1, &cOut) == 0) {
        strcpy(rgb + 1, lpsz);                     // the packed form did not fit
        rgb[0] = 0;                                // the escape
        cOut = strlen(lpsz) + 1;                   // the NUL is written too
    } else {
        rgb[0] = (uint8_t)cOut;                    // packed byte count
    }
    WriteRt(rtString, cOut + 1, rgb);              // MOV AX, 0x15 at 1070:884b
}
```

So the payload is the ordinary `[length][packed bytes]` field of
[`strings.md`](strings.md), with one escape: **a length byte of `0` means what
follows is the string itself, NUL-terminated**, because the packed form would
have needed more than 31 bytes. The buffer is 33 bytes, so a literal name is at
most 31 characters.

Implemented as `strings::decode_user_string` / `encode_user_string`, which the
`.xN` rename order (`RTCHGNAME`, type 44) uses too — it carries the same field.

### Writing one back

`stars_core::save` writes the block after a fleet's waypoints when the fleet has
a name, and nothing at all when it does not, which is why an ordinary game's
files contain none. Saving a game that was **loaded** keeps every block it did
not change, so a rename inserts a name block after the fleet's waypoints, a
changed name replaces the existing one, and a cleared name removes it.


## Block types

| Type | Name        | Detail | Notes                                        |
|-----:|-------------|--------|----------------------------------------------|
| 16   | `rtFleetA`  | 7      | own full fleet: ships, cargo, damage, orders |
| 17   | `rtFleetB`  | >= 4   | partial enemy fleet **with** cargo + dir/mass |
| 18   | `rtFleetC`  | < 4    | partial enemy fleet, no cargo + dir/mass     |

## Fixed header (14 bytes)

```
offset 0  u16  id word:  bits 0..8  fleet id (9 bits, 0-based per player)
                          bits 9..12 owner   (4 bits)
offset 2  u16  iPlayer (redundant owner word; ignored)
offset 4  u8   det (detail level: 3 some, 4 more, 7 all)
offset 5  u8   flags: bit0 fInclude, bit1 fRepeatOrders, bit2 fDead,
                       bit3 fByteCsh (1 = 1-byte ship counts, 0 = 2-byte)
offset 6  u16  idPlanet being orbited (0-based; 0xFFFF = deep space)
offset 8  u16  x position
offset 10 u16  y position
offset 12 u16  ship-design bitmask (which of 16 design slots are present)
```

## Variable sections (in order, cursor starts at 14)

### 1. Ship counts

For each set bit in the ship-design bitmask (slots 0..=15), read the count as
**1 byte** if `fByteCsh` is set, else **2 bytes** (little-endian).

### 2. Cargo hold — detail >= 4

- **Content-length word** (2 bytes): five 2-bit fields (ironium, boranium,
  germanium, population, fuel) each selecting a byte width via `[0,1,2,4]`.
- Each non-zero width is read little-endian in that order. **Population** is
  stored ÷ 100 (multiply by 100 for colonists).

### 3a. Full-fleet tail — type 16 (detail 7)

- **Damage bitmask** (2 bytes): which design slots have damage.
- For each set bit, a 2-byte value: `pctSh = value & 0x7F` (percent of ships
  damaged), `pctDp = (value >> 7) & 0x1FF` (armor damage percent).
- **Battle plan** (1 byte, 0-based index).
- **Waypoint count** (1 byte): number of waypoint blocks (19/20) that follow.

### 3b. Partial-fleet tail — types 17/18

- **dirLong** (4 bytes): `dirFltX` (i8), `dirFltY` (i8), `iwarpFlt` (low 4 bits
  of the third byte), one unused byte.
- **mass** (4 bytes, little-endian) — total estimated fleet mass.

## Related blocks (documented, not yet typed here)

- **Waypoint (19 `rtOrderA` / 20 `rtOrderB`):** `x`(u16), `y`(u16),
  `targetId`(u16), then a byte `warp = hi nibble`, `task = lo nibble`, and a byte
  `targetType = lo nibble`, `validTask = bit4`, `noAutoTrack = bit5`. `rtOrderA`
  carries task-specific data (transport orders, mine time, patrol, transfer).
  Tasks: 0 None, 1 Transport, 2 Colonize, 3 RemoteMine, 4 MergeFleet,
  5 ScrapFleet, 6 LayMines, 7 Patrol, 8 Route, 9 TransferFleet.
- **Fleet name (21 `rtString`):** a single packed Stars! string (see
  `strings.md`).

## Verified values (sample game, turn 0 `Game.hst`)

14 starting fleets — 6 for player 0, 4 each for players 1 and 2 — every one:

- detail 7, orbiting its **owner's homeworld** (owner 0 → planet 69, owner 1 →
  112, owner 2 → 32, matching the planet decoder)
- exactly one ship (count 1) in a single design slot
- an empty cargo hold except **fuel** (50–530 depending on the hull)
- battle plan 0, waypoint count 1, no damage, not dead

This corrects the earlier `hst.md` guess that the shared 8-byte run
`07 09 45 00 a4 05 1c 06` was a signature: those bytes are simply the flags word,
orbit planet, and x/y position of fleets that happen to share a homeworld.

## Open questions / next

- Type the waypoint (19/20) and fleet-name (21) blocks in `stars-formats`.
- Decode the ship-design block (26) — hull, slots, items — per `StarsFleet.pl`
  (see the RTSHDEF layout notes there).
