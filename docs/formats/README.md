# Format specs

One spec per on-disk file format, created from
`../templates/format-layout-template.md`.

Shared foundation:

- `blocks.md` — **block framing** shared by every format (implemented &
  round-trip tested in `stars-formats::block`); also tracks the pending payload
  encryption recovery.

Per-format (payload record layouts, populated as decoded in Step 2):

- `xy.md` — universe definition (`.xy`)
- `player-m.md` — player state (`.mN`)
- `player-h.md` — player history (`.hN`)
- `player-x.md` — player orders (`.xN`)
- `race-r.md` — race definition (`.rN`)
- `host-hst.md` — host state (`.hst`)
