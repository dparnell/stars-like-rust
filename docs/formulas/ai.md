# The computer players

Status: **identification and the planet list verified; terraform decision
verified; mine/factory decision reproduces the choice but not the amounts.**

Stars! ships seven computer opponents. This document records which player a
save file hands to which opponent, maps the roughly 95 functions that make up
their decision-making, and records how far each recovered piece has been
checked against a real game.

The corpus is `fixtures/games/all-computer-players`: 101 turns (2400-2500) of a
game with sixteen computer players covering six of the seven personalities and
all four difficulty settings.

## Identifying a computer player

The player record's flags byte (offset 7 of a type-6 block, see
`../formats/player.md`) is the high half of the player's 16-bit mode word.
`DoAiTurn` switches on `mode >> 13`, which is that byte's top three bits.

| Bits of the flags byte | Meaning                                    | Confidence |
|------------------------|--------------------------------------------|------------|
| 1                      | set for a computer player                  | confirmed  |
| 2–3                    | the difficulty setting                     | confirmed as a four-valued field |
| 5–7                    | which of the seven opponents               | confirmed  |

`fixtures/games/all-computer-players` settles this. Its sixteen players cover
six of the seven personalities and all four values of bits 2–3:

| Flags | Personality | Bits 2–3 | Players |
|-------|-------------|----------|---------|
| `0x03` | Robotoid   | 0 | 1 |
| `0x0f` | Robotoid   | 3 | 1 |
| `0x27` | Turindrone | 1 | 3 |
| `0x2b` | Turindrone | 2 | 1 |
| `0x47` | Automitron | 1 | 1 |
| `0x4f` | Automitron | 3 | 2 |
| `0x67` | Rototill   | 1 | 2 |
| `0x6b` | Rototill   | 2 | 1 |
| `0x87` | Cyber      | 1 | 2 |
| `0xa3` | Macinti    | 0 | 1 |
| `0xaf` | Macinti    | 3 | 1 |

Bits 2–3 take all four values independently of the personality, which is what
a difficulty setting looks like; the game was set up with mixed difficulties.
Which value is "Easy" is still not read out of the binary, so `stars-core`
exposes them as `skill_bits` rather than naming them.

The personality numbering is confirmed independently by behaviour, not just by
the dispatch table — see the terraform signature below.

### The dispatch table

`DoAiTurn` (`1088:0000`):

| `mode >> 13` | Routine                 | Address     |
|--------------|-------------------------|-------------|
| 0            | `DoRobotoidAiTurn`      | `1088:0312` |
| 1            | `DoTurinDroneAiTurn`    | `1088:3670` |
| 2            | `DoAutomitronAiTurn`    | `1098:01e0` |
| 3            | `DoRototillAiTurn`      | `1098:1e22` |
| 4            | `DoCyberAiTurn`         | `10a8:002a` |
| 5            | `DoMacintiAiTurn`       | `10a0:0008` |
| 6            | *(no case; runs no AI)* | —           |
| 7            | `DoMaidAiTurn`          | `1098:0000` |

## The shape of an AI turn

`DoAiTurn` runs for one player at a time. It sets the global `fAi`, reloads the
game from disk *as that player* — so the AI sees exactly the same partial
information a human player would — prepares shared working state, dispatches to
the personality, then writes the player's log and history files.

The preparation is the same for all seven:

1. `ComputeShdefPowers` — rate every ship design the AI can see.
2. `MarkPlanetsUnderAttack` — flag the AI's planets with enemies in orbit.
3. `IncreaseAIMinefieldSizes` — the AI's minefields grow for free. This is one
   of the concrete advantages a computer opponent is given.
4. `InitRandomPlanetList` — collect every planet the player owns and shuffle
   it, so the AI does not always consider planets in id order. This is the list
   every per-planet routine walks; see below.

A player marked dead is skipped entirely.

## What a personality does

Taking `DoTurinDroneAiTurn` as the worked example — four of the sixteen players
in the corpus use it — its 42 named callees group into six jobs:

| Job | Functions |
|-----|-----------|
| Ship design | `EnsureTurinDroneShdefs`, `CheckAiShdefStatus`, `MergeAllShdefs`, `SplitOutShdefs` |
| Production | `FillProductionQueue`, `InitProduction`, `AddItemToQueue`, `FinishProduction`, `GetResourcesAvailable`, `GetProdQCost`, `GetTrueHullCost` |
| Colonisation | `IdNearestColonizablePlanet`, `FColonizeAiFleet`, `PctPlanetDesirability`, `PctPlanetOptValue`, `PctTrueMaxGrowth`, `LpplFindBestEnum`, `LpplFindClosestEnum` |
| War | `FIsAiAttack`, `FIsTurinDroneAiAttack`, `FPotentRobWarFleet`, `IdTargetScout`, `IdTargetFreighter` |
| Movement | `FMoveAiFleet`, `FMoveToNearestStarbase`, `FGotoWormholeAiFleet` |
| Housekeeping | `HandleBasicAiTasks`, `ClearAiCurrentTask`, `IroEnsureAi`, `XferAiSupply` |

Only `EnsureTurinDroneShdefs` and `FIsTurinDroneAiAttack` are specific to this
personality; the rest are shared, which suggests the seven opponents differ
mainly in which ships they build and when they decide to attack.

### Production

`FillProductionQueue` (`10a8:2ce2`) is short and fully legible: for each planet
in the AI's shuffled list it selects the planet, calls `InitProduction`, calls
`FFillProdMinesAndFactories` (`10a8:2d72`), and calls `FinishProduction` with
that function's return value as the "did anything change" flag.

`FFillProdMinesAndFactories` itself computes, in outline:

1. `GetResourcesAvailable` and `GetProdQCost`, and subtracts one from the other
   to get what is left after the existing queue is paid for. It returns
   immediately if no resources remain.
2. Sums the mines and factories the queue already holds.
3. Walks the queue backwards, costing each `mdIdleMine`, `mdIdleFactory` and
   `mdIdleAlchemy` entry, and capping mines and factories at
   `CMaxOperableMines - CMinesOperating` and
   `CMaxOperableFactories - CFactoriesOperating` less what is already queued.
4. Special-cases `mdIdleTerraform` for the Macinti personality (`mode >> 13 == 5`).
5. Chooses between mines and factories: if minerals or resources are short it
   prefers mines, otherwise factories first and then mines with what is left.
6. Queues `mdIdleAlchemy` only once every tech level has reached 26 *and* the
   turn is past 100.

The decompiler loses the arguments to the bitfield helper at `1118:0e32` at
every call site, which made these item tests unreadable until the queue-entry
layout was pinned down from `AddItemToQueue` (see `../formats/production.md`).
With that layout the two shifts are unambiguous: `>> 0x11 & 7` is the entry's
`GrobjClass` and `>> 0xa & 0x7f` is its item id. How this transcription scores
is below.

## What the corpus confirms

`fixtures/games/all-computer-players` is 101 turns (2400–2500) of a sixteen-AI
game. Every turn's `.hst` records each AI planet's queue, which makes the
production decisions checkable. Across the corpus the AI queued, by item:

| Item | What | Planet-turns |
|------|------|--------------|
| 12 `mdIdleTerraform` | auto terraforming | 5785 |
| 11 `mdIdleAlchemy` + 3 `iobjAlchemy` | mineral alchemy | ~700 |
| 7 / 8 `mdIdleFactory` / `mdIdleMine` | factories and mines | 85 |

Terraforming dominates; mines and factories are rare.

### Terraforming — verified

`FQueueAiTerraforming` (`1090:8d28`) is reached from all seven personalities
through `HandleBasicAiTasks` (`1090:95a4`). It declines unless the player is
not Cyber, the planet's population is above 199 (units of 100 colonists), the
queue holds no auto-terraform entry already, and some environment variable
differs from the race's ideal. It then queues `min(steps available, 4)`.

Three of those read straight off the corpus:

- **The cap of 4 holds exactly.** Of 5785 entries, none exceeds 4.
- **Cyber is excluded, and the data shows it.** Counts by personality:

  | Personality | Counts queued |
  |-------------|---------------|
  | Turindrone  | `{1: 139, 2: 145, 3: 165, 4: 114}` |
  | Automitron  | `{1: 74, 2: 74, 3: 83, 4: 71}` |
  | Macinti     | `{1: 2970, 2: 251, 3: 286, 4: 723}` |
  | **Cyber**   | `{1: 690}` |
  | Robotoid    | none |
  | Rototill    | none |

  Every personality that reaches the routine uses its full 1–4 range. Cyber —
  the one the code refuses — shows only ever a count of 1, from some other
  source. Nothing else in the data would single out that one personality, so
  this confirms both the gate and that personality 4 really is Cyber.

- **The population gate holds where the routine is the only source**: 14 of 563
  Turindrone entries and 7 of 302 Automitron entries sit below it (2%), which
  is consistent with population falling after the queue was set. Macinti is far
  over (2565 of 4230) because it has a second source: the branch inside
  `FFillProdMinesAndFactories` that only Macinti takes.

Implemented as `ai::production::queue_ai_terraforming` and asserted in
`crates/stars-core/tests/ai_production.rs`. The one part still stubbed is how
many terraform *steps* a planet has available, which needs the terraforming
model this project does not have yet; the caller supplies it.

Robotoid and Rototill queue no terraforming at all despite calling
`HandleBasicAiTasks`. That is unexplained and worth chasing.

### Which planets an AI considers — recovered

`InitRandomPlanetList` (`1090:a0d7`) builds `vrglpplAi`, the list every
per-planet AI routine walks, and sets `vclpplAi` to its length. It is one of
`DoAiTurn`'s four preparation steps, and it is simple:

```c
vclpplAi = 0;
for (p in every planet)
    if (p->iPlayer == idPlayer)
        vrglpplAi[vclpplAi++] = p;
if (!(gameFlags >> 11 & 1))
    for (i = 0; i < vclpplAi - 1; i++)
        swap(vrglpplAi[i], vrglpplAi[i + Random(vclpplAi - i)]);
```

It collects **every planet the player owns** and shuffles it with a forward
Fisher-Yates using the game's own `Random`. It is not a filtered working set,
which is what an earlier revision of this document guessed it was. Implemented
as `ai::planet_order`.

The shuffle is skipped when bit 11 of the game flags word is set — the same
flag `FFillProdMinesAndFactories` tests when costing factories — which makes an
AI turn reproducible.

### Mines and factories — the decision is right, the amounts are not

Because there is no selection gate, `FFillProdMinesAndFactories` runs on every
AI planet every turn, and the transcription in
`ai::production::fill_prod_mines_and_factories` was scored against that.

**The recorded queue is the wrong observable.** The next turn's production
builds these entries and empties the queue, so across the corpus mines or
factories grow on 7684 planet-year pairs while only 80 ever show a queue entry.
An earlier revision of this document scored against the queue and concluded the
transcription "over-fires 140x". That was an artifact of the observable, not a
property of the routine, and it is withdrawn.

Scored against the change in a planet's mine and factory counts, over 21,508
planet-year pairs:

| measure | result |
|---------|--------|
| said it would build, and it did (recall) | 7735 of 7764 (99%) |
| said build, and it did (precision) | 7735 of 10728 (72%) |
| exact counts, where it built | 2699 of 7764 (34%) |
| exact counts, nothing else queued | 2565 of 6458 (39%) |

*When* the AI builds is reproduced almost exactly; *how much* is right about a
third of the time.

Two controls, because the headline number is misleading on its own. Overall
agreement is 62%, and simply predicting "nothing is ever built" scores 63% —
most planet-years build nothing, so that comparison says nothing either way.
Scoring each prediction against a different planet's outcome from the same year
gives 33%. The split above is the measure that carries information.

Part of the residual is structural rather than a transcription error: the queue
is chosen from one year's resources but built from the next year's, after
anything else queued takes its share. Restricting to planets with nothing else
queued moves exact counts from 34% to only 39%, so most of the gap lies
elsewhere. The likely candidates are the estimated mining in
`GetResourcesAvailable` and the planet's resource output, both of which the
whole-turn replay already shows are imperfect (67% on mines, 71% on factories)
— which means this may improve for free as the economy does.

`cargo run --release -p stars-core --example ai_production --
fixtures/games/all-computer-players` reproduces every figure here.

### Shipbuilding — the starbase decision

Ship orders behave differently from installations in one useful way: a ship
takes several turns to pay for, so its queue entry survives to be recorded.
There are 5873 of them in the corpus, against 80 for mines and factories, so
these decisions can be scored against the queue directly.

A ship is queued as `class = grobjFleet` with the design slot as the item.
`QueueAiStarbases` (`1090:8524`) settles the slot layout: it emits
`AddItemToQueue(ishdefSBLatest + 0x10, 1, grobjFleet, 1)`, so slots 0-15 are
the 16 ship designs and 16-25 the 10 starbase designs. Every ship entry in the
fixtures falls in that range.

The routine equips a planet that has **no** starbase. It declines when the
player has no starbase design yet, when the player is Macinti, when the planet
already holds a real starbase design (0-9), when population is not above 79,
when the AI's own scratch state has the planet busy or untracked, and when the
queue already holds a starbase order. Implemented as
`ai::ships::queue_ai_starbase`.

Across 2440 planet-turns with a starbase queued:

| claim | violations |
|-------|------------|
| never more than one starbase order at a time | 0 |
| the order's count is always 1 | 0 |
| population is above 79 | 1 |
| never re-queues the design the planet already has | 0 of 1237 |

The single population violation is a planet at 14, which lost people after the
order was placed.

### Replacing a starbase

`FUpgradeAiStarbase` (`1090:882a`) handles a planet that already has one. Of
the 2440 starbase orders, 1203 go to planets with none — `QueueAiStarbases` —
while 934 upgrade to a newer design and 303 queue an *older* one. Not one
queues the design already in place. Implemented as
`ai::ships::upgrade_ai_starbase`.

**Every arm is gated on `Random(100)`**, so no single decision can be checked
against a recording without the RNG in the same state. What *is* checkable is
the arithmetic each arm uses once it fires, which is deterministic given the
design being replaced.

The ten starbase slots are two banks of five: slot `n` and slot `n + 5` are the
same hull, which is why the routine works in `isb % 5` throughout and why the
four slots it singles out — 1, 3, 6 and 8 — are hull types 1 and 3 in both
banks. Those take the orbital-fort branch and measure themselves against
`IshdefAiSBLatestOF` rather than the newest design overall.

For everyone but Macinti, a design counted as outdated is replaced with
`latest + (isb % 5)`, one lower for the orbital-fort family. How eagerly
depends on the design's age: no eagerness for its first ten turns, then half
the turns elapsed, plus a flat five percent. Failing that, the AI may still
move sideways to `isb + 2`, but only six times in a hundred and only on a
planet holding at least 200 of every surface mineral. Cyber does none of this
before turn 40.

Macinti is different, and is why it has 1060 of the starbase orders — the most
of any personality. Both Macinti players here are Alternate Reality races,
which live on their starbases. Its arm consults `vAiMacRecycleSB`, a per-turn
table indexed by design: a design marked 3 is never left alone, one marked 2 is
left alone nine times in ten. When the table has *not* claimed the design, the
only move available is a one-in-twelve nudge to `isb + 1`, and only from the
four slots that have one above them. When the table *has* claimed it, the AI
jumps three hulls — `isb + 3`, wrapping down to `isb - 3` where that would pass
9. Designs below 4 instead walk the table upward for the first unclaimed slot.

The corpus bears the arithmetic out exactly. Every Macinti pairing of current
design to queued design with a current design of 4 or more:

| from | to | orders | move |
|------|----|--------|------|
| 4 | 5 | 61 | nudge |
| 5 | 6 | 21 | nudge |
| 7 | 8 | 144 | nudge |
| 8 | 9 | 17 | nudge |
| 4 | 7 | 23 | jump |
| 5 | 8 | 21 | jump |
| 6 | 9 | 4 | jump |
| 7 | 4 | 59 | wrapped jump |
| 8 | 5 | 111 | wrapped jump |
| 9 | 6 | 55 | wrapped jump |

516 replacements, and not one is a move the routine cannot make. The nudges
appear from 4, 5, 7 and 8 and never from 6 or 9, exactly as the code excludes
them.

Two of the routine's inputs cannot come from a fixture and are taken from the
caller: `vAiMacRecycleSB`, which lives only for the turn, and bit 9 at offset
`0x7b` of a design record, whose meaning is not recovered.

The AI's per-planet scratch state — `vlpbAiPlanet`, sixteen bytes per planet,
and `vlpbAiData`, the list of planets it is working on — is allocated inside
`DoAiTurn` and never written to a save file, so neither can be recovered from
fixtures. `ai::ships::PlanetTask` takes them from the caller.

### Other queue sources

Eighteen functions call `AddItemToQueue`. Besides the two above, the AI-side
ones are `FQueueAiDefenses` (`1090:939a`), `FQueueAiScanner` (`1090:90d6`),
`QueueAiStarbases` (`1090:8524`), `FUpgradeAiStarbase` (`1090:882a`),
`AddMinesToBlockedQueues` (`1090:1792`), `QuickBuildDefenses` (`1090:6a7e`,
reached from `FixPlanetsUnderAttack`), `FAddPacketToQueue` (`10a8:2bc0`),
`DoCyberPackets` (`10a8:1a78`), `FAIFling` (`1090:7dd6`) and
`iAddAttackFleet` (`10a8:4eba`), plus each personality routine directly.
Attributing a recorded entry to one of these needs more of them transcribed —
which is why Cyber's always-1 terraform entries have no known source yet.

## Source

- `DoAiTurn` (`1088:0000`) — dispatch table and turn preparation.
- `DoTurinDroneAiTurn` (`1088:3670`) — the worked example above.
- `FillProductionQueue` (`10a8:2ce2`), `FFillProdMinesAndFactories`
  (`10a8:2d72`), `AddItemToQueue` (`1090:3e50`).
- `FQueueAiTerraforming` (`1090:8d28`) and `HandleBasicAiTasks` (`1090:95a4`).
- `GetResourcesAvailable` (`1090:56d0`) and `GetProdQCost` (`1090:57c0`) —
  what the production decision reads, via `CMaxOperableMines`,
  `CMinesOperating` and their factory counterparts.
- The reconstructed C in `sirgwain/stars-decompile` is almost entirely stubs
  here — `ai.c` 10 of 12, `ai2.c` 5 of 6, `ai3.c` 6 of 6, `ai4.c` 16 of 17 and
  `aiutil.c` 58 of 61 functions are empty — so it is of no help for this
  subsystem.
