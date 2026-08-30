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
- `race-r.md` — race definition (`.rN`): record largely decoded
- player history (`.hN`) and player orders (`.xN`): container round-trips; the
  score record (type 45) in `.hN` is decoded; other record layouts not yet
  documented
