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
- `hst.md` — host & player state (`.hst` / `.mN`): block inventory decoded,
  planet record fully decoded (see `planet.md`); player/fleet/design records in
  progress
- `planet.md` — planet blocks (types 13/14/15) shared by `.hst`/`.mN`/`.hN`:
  **fully decoded & verified** (`stars-formats::planet`)
- `race-r.md` — race definition (`.rN`): record largely decoded
- player history (`.hN`) and player orders (`.xN`): container round-trips; record
  layouts not yet documented
