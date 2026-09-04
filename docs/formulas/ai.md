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

### Colonisation

`IdNearestColonizablePlanet` (`1090:0e3e`) chooses the target; `FColonizeAiFleet`
(`1090:0c78`) sets the waypoint at the warp `IFindIdealWarp` picks. Implemented
as `ai::colonise`.

The routine writes a one-byte mark for every planet into its scratch array and
then returns the **nearest** planet still marked colonisable, by squared
distance from the colony fleet. Nothing but distance is weighed — an adequate
planet nearby beats an ideal one further out.

| Mark | Meaning |
|------|---------|
| `0x00` | unowned and worth settling — the only value the search accepts |
| `0x01` | already ours |
| `0x02` | held by somebody else |
| `0x04` | unowned, but another of our colony fleets is already going there |
| `0x08` | unowned and not worth settling |
| `0x10` | a planet this player knows nothing about |

Robotoid and Macinti differ twice over: they skip the habitability test
entirely, and they default an *unknown* planet to colonisable rather than
`0x10`, so they will strike out into unexplored space where the others will
not. For Macinti that follows from what it is — the Macinti players here are
Alternate Reality races, which live on their starbases.

#### `PctPlanetOptValue` — habitability *after* terraforming

The habitability test is not the plain desirability. `PctPlanetOptValue`
(`1048:6b88`) asks `FCanTerraformLppl` how far each environment variable could
be moved, temporarily writes those values into the planet, takes
`PctPlanetDesirability`, and then puts the original values back.

This distinction is load-bearing: a world that would kill colonists today but
can be terraformed into something habitable is a legitimate target. It is why
the corpus shows computer players settling planets whose *current* value is
negative — 368 of 513 settled planets are habitable as they stand, and the
shortfall is concentrated in exactly the personalities that apply the test.

How far a planet can be terraformed is not modelled yet, so
`ai::colonise::pct_planet_opt_value` takes the reachable environment from the
caller. This is the same gap that leaves the AI's terraform step count
unmodelled, and closing it would settle both.

#### What the corpus does and does not settle

513 planets pass from unowned to owned across the game. Habitability as it
stands, by personality:

| Personality | habitable / settled |
|-------------|---------------------|
| Robotoid | 168 / 168 |
| Rototill | 6 / 6 |
| Automitron | 18 / 24 |
| Turindrone | 48 / 72 |
| Cyber | 41 / 75 |
| Macinti | 87 / 168 |

Consistent with the post-terraforming gate, but not proof of it: Robotoid
skips the test altogether and still settled nothing hostile, which says more
about the planets near it than about the rule.

**The "nearest planet" rule is not yet scored, and the reason is worth
recording.** Two attempts failed for instructive reasons:

- Ranking the settled planet against all unowned planets, by distance from the
  player's nearest planet, gives 19% exact and 41% within the nearest three
  against a chance rate near zero. Better than chance, but the candidate set is
  wrong: it includes planets the AI has never seen, which for every personality
  but Robotoid and Macinti are not candidates at all.
- Scoring a colony fleet against its own position gives 1% against a 1% chance
  rate — no signal. That measurement is simply invalid: a ship under way has
  already flown past planets that were not candidates when its orders were
  given. Restricting to fleets still in orbit over their origin, which is the
  moment of the decision, leaves **4** cases in the whole game, because a fleet
  departs within the same turn generation that gives it orders.

Partial planet records are now loaded, into `GameState::known_planets`. That
improved the landing measurement — from 19% exact and 41% within three to **27%
and 52%**, with mean rank falling from 20.3 to 7.7 against a chance rate of 1%
— because an unowned planet's environment comes from the host's partial record,
so the candidate set can finally be filtered by habitability.

It did **not** do what was predicted of it, and that is worth recording. The
expectation was that a player's `.mN` file would carry its own view of the
galaxy as partial records. It does not: across the corpus a player's file holds
full records for exactly the planets it owns and almost nothing else — usually
one further planet, occasionally eighteen. There is no planet-knowledge block
in a player file at all; the block types present are messages, fleets,
waypoints, designs, battle plans, battle recordings, scores and objects.

So the AI's known-planet set still cannot be reconstructed, and the residual
gap to "strictly nearest" is still unmeasured. The remaining proxy is the one
that matters: distance is taken from the player's nearest planet rather than
from the colony fleet, whose position at the moment of decision no file
records.

### War and fleet dispatch

Both are transcribed; neither is verified, and the corpus is the reason.

#### Recognising a warship

Two predicates decide whether a fleet is an attack fleet, and both work purely
from the **hull** of the designs aboard — not from weapons, not from strength.

`FIsTurinDroneAiAttack` (`1090:4b9a`) is the simple one: any design present
whose hull index is 4 to 10. `FIsAiAttack` (`1090:4a72`), used by the other
personalities, is stricter: hulls 6 to 10 count outright, hull 5 counts only if
the design is actually armed, and hulls 29 and 31 count only when armed *and*
`WtMaxShdefStat` is under 500. Implemented as `ai::dispatch`.

#### How fast a fleet is sent

`IFindIdealWarp` walks down from warp 10 to the first speed the engine sustains
— fuel use below 121 — then applies two adjustments:

- **Fuel economy.** If the fleet still burns fuel there, and it is not a
  ramscoop (engines 14 and 15), it steps down by up to three to reach a speed
  the engine runs free at.
- **The warp 10 rule.** Only engines 7, 8, 9, 14 and 15 are sent at warp 10;
  everything else is held to 9.

A design with no engine is a starbase and gives warp 0.

The third step-down test reads, in the decompilation, off the end of the ore
cost array and into the fuel table that follows it. With the two tests above it
stepping down one and two to reach a free speed, three is the only reading that
fits, and that is what is implemented.

#### Why neither is scored

**War outcomes are barely present.** The host files carry no battle records —
across the corpus the block types are 0, 6, 8, 13, 16, 19, 20, 26, 28, 30 and
43, with nothing from the battle format — and only **56** planets change hands
between owners in 101 turns. There is no way to ask whether the AI attacked
when it should have.

**Dispatch is barely present either.** A fleet recorded in a host file has
already arrived: its warp is clear and its orders are spent. Across 101 turns
only **36** AI waypoint legs still carry a warp. On those, `ideal_warp` matches
3 with the fuel back-off applied and 19 with it skipped, and every disagreement
in the first case is an under-prediction of one or two — precisely what an
unwanted back-off looks like.

That points at `fIgnoreScoops` being set at the call sites. `FColonizeAiFleet`
passes a second argument the decompiler renders as a planet id, so what reaches
the flag is unresolved. 36 samples cannot settle it, and choosing the mode that
scores better would be fitting a reading to 19 data points, so both modes are
exposed and neither is asserted.

Scoring either properly needs the `.x` order files, which record what a player
*submitted* rather than what survived the turn. The corpus has none — it has
`.hst`, `.mN` and `.xy` only.

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
