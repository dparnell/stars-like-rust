# Format specs

One spec per on-disk file format, created from
`../templates/format-layout-template.md`.

Shared foundation:

- `blocks.md` — **block framing** shared by every format (implemented &
  round-trip tested in `stars-formats::block`); also tracks the pending payload
  encryption recovery.
- `strings.md` — the **packed-string codec** used for user-supplied text (race
  names, etc.), implemented & tested in `stars-formats::strings`.

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
  **fully decoded & verified** (`stars-formats::fleet`); waypoint/name layouts
  documented for follow-up
- `design.md` — ship/starbase design blocks (type 26) shared by `.hst`/`.mN`:
  **fully decoded & verified** (`stars-formats::design`)
- `race-r.md` — race definition (`.rN`): record largely decoded
- player history (`.hN`) and player orders (`.xN`): container round-trips; record
  layouts not yet documented
