# Player-scores block (type 45)

Status: **decoded & verified** — implemented in `stars-formats::score`.

Each score block (`PLAYER_SCORES`) is one player's public scoreboard row for
the turn. A `.mN` player file carries the owning player's own row; a `.hN`
history file carries one row per recorded turn.

## Layout (fixed 24 bytes)

| Offset | Size | Field                         | Notes                          |
|--------|------|-------------------------------|--------------------------------|
| 0–1    | 2    | player id + flags             | see below                      |
| 2–3    | 2    | rank, **or** turn             | 1 = leader; a turn on a history row |
| 4–7    | 4    | score                         |                                |
| 8–11   | 4    | resources                     |                                |
| 12–13  | 2    | planets                       |                                |
| 14–15  | 2    | starbases                     |                                |
| 16–17  | 2    | unarmed ships                 |                                |
| 18–19  | 2    | escort ships                  |                                |
| 20–21  | 2    | capital ships                 |                                |
| 22–23  | 2    | tech levels (sum of 6 fields) |                                |

The first `u16` packs the player id, three flags and the victory-condition
bits. The field names are `SCOREX`'s own, from the NB09 symbols:
`iPlayer:5, fValid:1, grbitVC:8, fWinner:1, fHistory:1`.

| Bits  | Field                                  |
|-------|----------------------------------------|
| 0–4   | player id (`iPlayer`)                  |
| 5     | `fValid` — the row carries figures     |
| 6     | owns X planets                         |
| 7     | attains tech X in Y fields             |
| 8     | exceeds a score of X                   |
| 9     | exceeds second place score by X        |
| 10    | has production capacity of X           |
| 11    | owns X capital ships                   |
| 12    | has the highest score after X years    |
| 13    | (spare — `grbitVC` has room for eight) |
| 14    | `fWinner` — has met enough to win      |
| 15    | `fHistory` — a past year, not now      |

Three of those decide how a row is read, and the Score sheet turns on all
three — see `docs/ui/score-sheet.md`:

* **`fValid`.** A player file holds a row for *every* player and fills in only
  the ones that player may see. Without Public Player Scores the other fifteen
  rows are present and empty, which is why the sheet shows blank columns rather
  than zeroes. `fixtures/games/no-random-events/2500/Game.m1` has all sixteen
  filled in; `fixtures/games/all-computer-players/2450/Game.m1` has one.
* **`fWinner`.** Set by `UpdatePlayerScores` (`10b8:6258`) once a player has met
  as many conditions as the game requires and the year is late enough.
* **`fHistory`.** The row is one year of a `.hN` timeline, and the word at
  offset 2 is then the **turn** rather than the rank. `io.c` files every score
  block it reads into that player's timeline, keeping one row per turn in turn
  order; a row *without* the flag is filed under the game's current turn, which
  is how the timeline reaches this year from a history file that stops at the
  last one.

The three ship counts are **packed** — thirteen bits of value and three of a
shift (`WPackLong`, `1038:4ba2`) — so a fleet over 8,191 ships is recorded
roughly. `DrawScoreReport` unpacks exactly those three rows and no other.

## Evidence

`fixtures/incoming/turn1/Game.m1` has one score block: player 0, rank 1,
score 25, resources 39, 1 planet, 1 starbase, 4 unarmed / 2 escort / 0 capital
ships, tech total 18, and no victory conditions met on turn 1.

`fixtures/games/no-random-events/2500/Game.m1` is a public-scores game and has
sixteen, all `fValid`: player 3 leads at rank 1 with 1,542 points, 34 planets
and 23 starbases, and is the only one with a victory bit — bit 12, the highest
score after enough years. The `Game.h1` beside it carries 99 rows for player 0
and 80 for each of the others, whose turns start at 20 rather than 1.

## Source

- stars-4x `decompiled`: `Structures/Structure45.xml` ("Scores").
