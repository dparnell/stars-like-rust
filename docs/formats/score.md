# Player-scores block (type 45)

Status: **decoded & verified** — implemented in `stars-formats::score`.

Each score block (`PLAYER_SCORES`) is one player's public scoreboard row for
the turn. A `.mN` player file carries the owning player's own row; a `.hN`
history file carries one row per recorded turn.

## Layout (fixed 24 bytes)

| Offset | Size | Field                         | Notes                          |
|--------|------|-------------------------------|--------------------------------|
| 0–1    | 2    | player id + victory bits      | see below                      |
| 2–3    | 2    | rank                          | 1 = leader                     |
| 4–7    | 4    | score                         |                                |
| 8–11   | 4    | resources                     |                                |
| 12–13  | 2    | planets                       |                                |
| 14–15  | 2    | starbases                     |                                |
| 16–17  | 2    | unarmed ships                 |                                |
| 18–19  | 2    | escort ships                  |                                |
| 20–21  | 2    | capital ships                 |                                |
| 22–23  | 2    | tech levels (sum of 6 fields) |                                |

The first `u16` packs the player id and the victory-condition flags:

| Bits  | Field                                  |
|-------|----------------------------------------|
| 0–3   | player id                              |
| 4–5   | (unknown)                              |
| 6     | owns X planets                         |
| 7     | attains tech X in Y fields             |
| 8     | exceeds a score of X                   |
| 9     | exceeds second place score by X        |
| 10    | has production capacity of X           |
| 11    | owns X capital ships                   |
| 12    | has the highest score after X years    |
| 13–15 | (unknown)                              |

## Evidence

`fixtures/incoming/turn1/Game.m1` has one score block: player 0, rank 1,
score 25, resources 39, 1 planet, 1 starbase, 4 unarmed / 2 escort / 0 capital
ships, tech total 18, and no victory conditions met on turn 1.

## Source

- stars-4x `decompiled`: `Structures/Structure45.xml` ("Scores").
