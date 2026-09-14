# The computer players

Status: **identification, the planet list and the terraform decision verified;
mine/factory decision reproduces the choice but not the amounts.**

Stars! ships seven computer opponents. This document records which player a
save file hands to which opponent, maps the roughly 95 functions that make up
their decision-making, and records how far each recovered piece has been
checked against a real game.

The corpus is two games, each 101 turns (2400-2500) with sixteen computer
players: `fixtures/games/all-computer-players`, covering six of the seven
personalities and all four difficulty settings, and
`fixtures/games/no-random-events`, created with random events off. The second
contains **no Mystery Traders at all** — 85 appear in the first, none in the
second — which is what that option removes, and which also means
`ITechLearnATech`'s Mystery Trader half never fires there.

Figures below quoting a single corpus were measured before the second was
added; those quoting both say so.

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
`crates/stars-core/tests/ai_production.rs`. The count of steps available is
supplied by the caller and comes from `terraform::terraform_steps`, which is
`IpctCanTerraformLppl` (`1048:7f56`) by way of the production catalogue — see
`terraforming.md`. Scored against the corpus, `min(steps, 4)` matches **196 of
196 fresh orders (100%)**, once the order is reconstructed as
`recorded count + clicks built that turn`: production runs later in the same
turn the AI queues the item, so the saved file always shows the order already
drawn down.

**Robotoid and Rototill queue no terraforming at all** — and neither needs a
special case, because both fail the same gate for different reasons. The order
count comes from the production catalogue, and `InitProduction` only puts a
terraform item in it when `IpctCanTerraformLppl` is above zero. Measured over
the corpus, `terraform_steps` is **0 on every one of the 3809 Robotoid and 718
Rototill planet-turns past the population gate**:

- **Robotoid is the all-immune Hyper Expansion race.** In both games its
  environment centres are all `0xFF`, so every axis is immune, nothing is ever
  terraformable, and `off-ideal on a non-immune axis` is exactly 0 across 6336
  and 5404 planet-turns. (This is the same race whose planets carry terraforming
  done by *previous* owners — see `terraforming.md`.)
- **Rototill is the Claim Adjuster**, and gets its terraforming free.
  `AutoTerraform` (`10b8:48f6`) runs after `Produce` for every planet owned by a
  PRT 3 player and sets the environment straight to the edge of its reachable
  band. By the time the production catalogue is built there is nothing left to
  queue. Rototill has 633 off-ideal planet-turns and 0 available steps, which is
  exactly that signature: off the ideal, but already as close as its technology
  can bring it.

Personality to race, as the corpus binds them:

| Personality | PRT | Immunity |
|-------------|-----|----------|
| Robotoid    | HE  | all three |
| TurinDrone  | SS  | none |
| Automitron  | IS  | temperature (in one of the two games) |
| Rototill    | CA  | none |
| Cyber       | PP  | none |
| Macinti     | AR  | none |

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
| said it would build, and it did (recall) | 7715 of 7764 (99%) |
| said build, and it did (precision) | 7715 of 10134 (76%) |
| exact counts, where it built | 4360 of 7764 (56%) |
| exact counts, nothing else queued | 4180 of 6458 (64%) |

*When* the AI builds is reproduced almost exactly. *How much* reads lower here
than it should, because this observable is contaminated — see below, where
isolating the decision from production raises it to 85%.

Two controls, because the headline number is misleading on its own. Overall
agreement is 72%, and simply predicting "nothing is ever built" scores 63% —
most planet-years build nothing, so that comparison says nothing either way.
Scoring each prediction against a different planet's outcome from the same year
gives 35%. The split above is the measure that carries information.

### The 34% is mostly the observable, not the routine

Part of the residual is structural rather than a transcription error, and a
larger part than an earlier revision of this document allowed. That revision
noted "the queue is chosen from one year's resources but built from the next
year's, after anything else queued takes its share", tried the empty-queue
control alone, saw it move exact counts only 34% → 39%, and concluded most of
the gap lay in the economy. The empty-queue control on its own is not enough,
because it does not remove the biggest competitor for a planet's resources:
**ships**.

`FFillProdMinesAndFactories(PLANET *lppl)` takes the planet and nothing else —
no resource budget is threaded into it, unlike `FQueueAiTerraforming`, which
receives `rgResAvail` and `rgResCost`. So the routine decides a count in
ignorance of what else the planet is building, and `Produce` then works the
queue in order. On a planet with a starbase, ships are queued ahead of the
installations and take their share first, so the *change in building counts* —
the observable — is production's output, not the AI's decision.

Removing both competitors at once isolates the decision. On the 7,997
planet-year pairs with **no starbase and an empty queue**, where the AI's
decision and the year's building are the same quantity:

| measure | result |
|---------|--------|
| both counts exact | 6,826 (**85%**) |
| exact where it built | 3,164 of 4,015 (78%) |
| mines exact | 7,388 (92%) |
| factories exact | 6,930 (87%) |
| *chance control* — same predictions, another clean planet's outcome | 2,312 (28%) |
| *predict-nothing control* | 3,982 (49%) |

85% against a 49% floor and a 28% chance rate is a materially different reading
of the routine from "right about a third of the time". The headline figure is a
real number about *whole-turn outcomes* and is the right thing to quote when
asking how well the pipeline reproduces a year; it is the wrong thing to quote
when asking whether `FFillProdMinesAndFactories` is transcribed correctly.

### The population the cap is measured against

Isolating the decision this way also made a real transcription error visible,
which the contaminated figure had buried. The first measurement of the clean
subset gave 72%, with a lopsided error: mine over-prediction outnumbered
under-prediction roughly ten to one, and the commonest wrong answer was **one
mine too many**. Splitting those cases by which limit was binding showed 379 of
487 bound by the operable-mine cap rather than by affordability — so the cap was
one too high, not the resource estimate.

`CMaxOperableMines(lppl, iplr, fNextYear)` takes a flag saying whether to
advance the planet's population by this year's growth first. The implementation
passed `true`. Both call sites in `FFillProdMinesAndFactories` push a literal
zero:

```asm
10a8:2fbd  MOV AX,0x0
10a8:2fc0  PUSH AX                  ; fNextYear = 0
10a8:2fc1  PUSH word ptr [0x18c]    ; idPlayer
10a8:2fc5  PUSH [BP+0x8] / [BP+0x6] ; planet, far
10a8:2fcb  CALLF 0x1048:7304        ; CMaxOperableMines
```

and the same shape at `10a8:308b` before `CMaxOperableFactories` at
`10a8:3099`. The decompiler mangles this argument list — the far planet pointer
takes two slots, so it reports `fNextYear` where `iplr` belongs — which is why
the flag had been read wrongly in the first place.

**The AI sizes its order against the population the planet has now, not the one
it will have after growing.** With that corrected the clean subset went 72% →
85%, mines 75% → 92%, and the cap-bound over-predictions fell from 379 to 4. It
lifted every headline figure too: exact-where-it-built 34% → 56%, precision 72%
→ 76%, recall unchanged.

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

### The TurinDrone turn — recovered, being written

`DoTurinDroneAiTurn` (`1088:3670`, about eight kilobytes) is the
personality the tutorial's Berserkers use (`tutorial.hst` gives them
`0x27`: TurinDrone, skill 0). Read from Ghidra; the community decompile
has it as a stub. What follows is the routine's shape, in order, with the
design slots it works in. `ai::turindrone` is the transcription, begun
with the parts the tutorial's first years need — scouting, colonising and
the year-0 queue — and marked where it stops.

#### The design slots

The personality keeps a fixed meaning for each of the sixteen design
slots, and `EnsureTurinDroneShdefs` (`1088:58ba`) fills a slot with
`FCreateAiShdef(slot, hull, parts)` when it is empty or obsolete (`det`
bit 9) and the tech allows:

| slot | role | hull | needs |
|-----:|------|------|-------|
| 0 | scout | Frigate (5) | — (the starting Scout design is scrapped once Construction > 5) |
| 1 | colony ship | Colony Ship (15) | — (a starting design here that is not a Privateer is obsoleted first) |
| 2–3 | remote miners | Miner (22) | Con > 6, Elec > 3 |
| 4–5 | battleships | Battleship (9), one of four fittings at random | Ener > 4, Elec > 5, Con > 12, Weap > 6 |
| 6–7 | freighters (counted in the queue pass as `cPlanMax/12 + 8`) | | |
| 8 | cruisers | Rogue (12) | Weap > 4, Con > 7 |
| 9 | cruisers | Galleon (13) | Weap > 6, Con > 10 |
| 10–11 | destroyers | Destroyer (6), one of two fittings | Ener > 4, Elec > 4, Con > 3, Weap > 4 |
| 12 | mine layer | Privateer (11) | Con > 3, Bio > 3 |
| 13 | bomber | Stealth Bomber (18) | Ener > 7, Elec > 6, Con > 5 |
| 14 | bomber | Stealth Bomber (18) | Ener > 10, Elec > 11, Con > 14, Weap > 8 |
| 15 | | Rogue (12) | Ener > 4, Elec > 5, Con > 12, Weap > 6 |

The fittings are byte strings of **AI part classes**, one per hull slot,
in the personality's own segment (`1088:35c2`, reached through a table of
offsets at `1088:35b0`); `FGetAIPart` (`1090:043e`) turns a class into a
component. A class is a short list of candidates — slot type, item, and
how many items below it to try — in `vrgcAiParts` (`1120:1450`, 45
classes) and 139 words at `1090:0000`, and the first candidate the player
can build (`FLookupPart`, the same gate the designer uses) wins; the slot
is filled to its capacity. A fitting with a class nothing fills fails as a
whole. The engine class (8) is the Trans-Star 10, the five scoops from the
Galaxy Scoop down, and the Fuel Mizer — never the Quick Jump 5 or the Long
Hump 6 — so a young TurinDrone designs nothing until it has Propulsion 8
with Energy 2, or Propulsion 2 with Improved Fuel Efficiency, which the
tutorial's Berserkers have. `ai::parts` holds the tables, `pick_part` and
`create_design`; the fittings it names are the TurinDrone's:

| offset | hull | classes |
|-------:|------|---------|
| 0 | Colony Ship | engine, colony module |
| 2 | Frigate scout | engine, scanner, torpedo, shield |
| 6 | Destroyer | engine, torpedo ×3, armour, mechanical, electrical |
| 48–81 | Battleship, four fittings | |
| 92 | Rogue | |
| 101 | Stealth Bomber | engine, bombs ×2, mechanical, electrical |
| 106 | Privateer mine layer | engine, shield, mechanical, mines ×2 |
| 111 | Galleon | |
| 123 | Miner | engine, mechanical, mining ×4 |

`PickANameAndBmp` names the design from one of nine lists of the game's
(strings `0x03d4`–`0x0431`: Easter Bunny, Lying Bastard, Pidgeon, Ground
Hog, Egg, Glovebox, Prickly Pear, Zombie, Scrapper), choosing a name no
live design carries and, after twenty misses, a name with a number; which
list a hull takes is read from a word of the hull's not yet pinned down,
so `ai::parts::name_group` goes by the hull's role — which agrees with the
tutorial, whose Berserker mine layers are *Saguaros*, a Prickly Pear
name. The picture is the hull's, the first of its four variants not in
use.

A design slot is **free** when its retired bit (`det` bit 9) is set,
which is also how an empty slot reads; `ShipDesign::obsolete` is that bit,
byte 1 bit 1 of the design block. `EnsureTurinDroneShdefs` retires the
scout, colony and miner slots' old design *before* trying to make the new
one, and asks for the colony ship, scout, miner and mine layer again
whenever no ship of the design exists — so a freshly made design is made
afresh each year until one is built.
`CheckAiShdefStatus(from, to, recycle, &latest, old)` counts the ships of
a slot range, notes the newest design, and after `recycle` years — 50
before turn 120, 70 before 200, 100 after — obsoletes an unused design or
marks a used one for `SplitOutShdefs` (`1090:98d8`), which from turn 61
and while the player has fewer than 501 fleets takes the first live fleet
carrying both a marked design and an unmarked one, moves the marked
designs' ships to a new fleet (`LpflNewSplit`: same place, orders copied)
with their share of the cargo (`FleetTransferCargoBalance`), and starts
the search again until no such fleet is left.

#### Before the planets

* `IroEnsureAi(lpbRes, cRes, &ishdefSBLatest, pct)` (`1090:425a`), called
  at `1088:36a4` with the thirty-one bytes at `1088:3650` and `pct = 15`:
  the research share becomes `pct`, or 0 when every field is at 24 or
  more; the field becomes that of the first entry (`field = b >> 5`,
  `level = b & 0x1f`) whose level is not yet reached, and when the level
  is one away the *next* field is set from the entry after it; past the
  end of the list the lowest field (first on a tie) is studied and the
  routine answers `0x39e`. The TurinDrone's plan: Prop 2, Con 4, Bio 4,
  En 4, Weap 5, Prop 6, Con 6, Weap 8, En 6, Elec 6, Prop 9, Bio 7,
  Con 8, Elec 8, Bio 5 (a no-op by then), Con 9, En 7, Elec 10, Weap 10,
  Prop 12, Con 11, En 10, Weap 12, Elec 13, Prop 16, Weap 14, Con 15,
  Elec 14, Bio 10, Weap 16, En 14. The tutorial's Berserkers start with
  `[0, 0, 0, 0, 5, 0]`.
* `MergeAllShdefs` merges fleets of the same slot at the same place for
  the bombers (13), the mine layers (12), the destroyers (10, 11) and the
  miners (2, 3).
* Armada potency: `vrgAiArmadaPotency[0] = 3 + (turn − 120)/20` after
  turn 130, at most 50; `[1]` half of it; `[2] = 6 + (turn − 100)/22`
  after turn 115, at most 12; `[3] = [2]/2 − 1`, at most 3.
* Two counts over every planet: unowned planets the player has scanned
  (`det & 0xff > 2`) with `PctPlanetOptValue > 0`, and other players'
  planets with a positive value — either being non-zero is what makes the
  queue pass build colony ships.

#### The planet pass

For every planet in `lpPlanets`: a scratch byte per planet
(`vlpbAiPlanet[id*16 + k]`) is filled — `[1]` the mineral worth of an
unowned scanned planet (each concentration halved, capped at 75, summed,
capped at 127), `[2]` set on an own planet with negative desirability,
`[3]` the opt value of somebody else's planet, `[9]` always 1, `[10]`
whether a foreign planet has a starbase.

An own planet with a **starbase** (`det` bit 9) and at least 200 kT of
colonists gets its queue looked at. If the queue already holds a ship
(`AddItemToQueue` type 2, item < 16) nothing is added. Otherwise, in
this order:

1. **turn 0**: one scout per thirty planets in the universe (per hundred
   past 190);
2. otherwise, while slot 0 is a Frigate and not obsolete: a scout when
   fewer than `min(cPlanMax/4, 32)` exist and ten times the built count is
   under the existing count;
3. a **cruiser** (slots 8–9) when Weapons > 4 and the count is under
   `max(planets/10, 2 × the AI's own tally)`, or under ten sevenths of
   that with a one-in-four roll;
4. **four colony ships** when there is anywhere to settle and fewer than
   two exist;
5. **three mine layers** with a one-in-three roll, when the fleet of them
   at the planet is under ten (under seventeen with one in eight) and a
   roll of `2 × count + 1` comes up zero;
6. a **bomber** when a war fleet at the planet already holds
   `potency[2]` bombers;
7. then, each paid for against the resources left after the queue
   (`GetResourcesAvailable − GetProdQCost`, then `GetTrueHullCost` per
   ship, stopping at the first that cannot be paid): up to five
   **battleships** while fewer than `cPlanMax/24 + 4` exist, five
   **freighters** under `cPlanMax/12 + 8`, five **destroyers** under
   `cPlanMax/4 + 12`, and five of slot 15 under `cPlanMax/12 + 8`.

`FinishProduction` writes the queue back. Then `HandleBasicAiTasks`:
`KeepFleetsMoving` (every fleet with orders re-speeded by
`SetAiFleetIdealSpeed`, transports at 30 and everything else at the ideal
warp, 16 for the first five turns), `QueueAiStarbases`, and for every
planet with 60 kT of colonists or more: `FUpgradeAiStarbase`, `FAIFling`,
`FQueueAiScanner`, `FQueueAiDefenses`, then `FQueueAiTerraforming` when
none of those queued; `FixPlanetsUnderAttack` from turn `20 + 10 ×
size`; `AddMinesToBlockedQueues`; and last `FillProductionQueue`, the
mines-and-factories fill of *Mines and factories* above.

#### The fleet pass

First a walk over every fleet (`1088:4932`–`1088:4a70`): the player's
attack fleets (`FIsTurinDroneAiAttack`: any hull 4 to 10 aboard) are
chained together, other players' fleets likewise, `det` bit 15 cleared,
and stale orders dealt with. A fleet's *destination* is the planet it
orbits when it has no orders, else its next waypoint's planet. For a
fleet with no miners aboard, or whose last order has no task:

* a **colony ship** (slot 1) with no freighters: a destination that is
  somebody else's planet with a positive opt value (`vlpbAiPlanet[+3]`)
  goes to the drop check below; one another player has taken otherwise
  has the orders blown away (`LBlowAwayOrders`: cut to the current
  waypoint, `ClearAiCurrentTask` clearing its task);
* a **freighter** (slots 8, 9): with no destination, a next waypoint on
  a bare point (`grobj` 4) has the orders blown away; with one that is
  gone or another player's, the drop check (`LCheckForColDrop`): a valued
  planet of theirs, colonists aboard, the player not Alternate Reality
  and no starbase there, and the colonists are dropped — at the planet,
  the current waypoint gets a Transport task with the colonists' item at
  `0x2000` (unload all; the order word `0x1101`) and the fleet is then
  sent to the nearest own starbase (`FMoveToNearestStarbase`, `1090:6f7e`:
  `IdplFindClosestStarbase` from the current waypoint, a leg at `0x1140`,
  warp 4, through `FMoveAiFleet` with `fAppend` 0, which replaces the
  orders after the current waypoint); away from the planet the drop order
  is written into the next waypoint and that same move then overwrites
  it, so the fleet only heads for the starbase. Anything else has the
  orders blown away.

A fleet **with miners** whose last order has a task: in deep space with a
next waypoint, that waypoint's task becomes Remote Mining and its planet
is claimed (`vlpbAiPlanet[+1] |= 0x80`); at an unowned planet, the planet
is claimed; at an owned one, the orders are blown away.

Then the orders, by what the fleet carries, for fleets with **no
orders** (`cord < 2`) and no miners aboard:

* a **colony ship** (slot 1): `IdNearestColonizablePlanet`, loading 25
  kT of colonists first at an own planet (`XferAiSupply`), then
  `FColonizeAiFleet` — a waypoint on the planet with the Colonize task at
  `IFindIdealWarp`, `det` bit 15 set; with nowhere to go it heads for the
  nearest own planet, or one in ten takes a wormhole;
* **cruisers** (8–9): `IdTargetFreighter` — they are the haulers;
* **bombers** (13–14): `LpplFindBestEnum` for a target and a move at
  `0x1140`;
* **scouts and destroyers** (0, 10, 11): a Scout-hull scout is scrapped
  once Construction > 5; otherwise `IdTargetScout`: a fleet with teeth
  looks for the nearest enemy fleet nobody else is chasing and follows
  it, or the nearest planet no other of our fleets is bound for; one
  without goes to `IdNearestUnknownPlanet` — the nearest planet marked
  unknown (`0x10`) — or, when everything is known, the best of
  `LpplFindBestEnum` from home, or a random planet; the leg is laid at
  `IFindIdealWarp` with `FMoveAiFleet`;
* a **mine layer** (12) alone: the Lay Mines task, five years.

At **turn 0** a fleet that has orders, or that carries the starting
freighter or colony ship (slots 2–3 as the turn-0 designs stand), is
given the **Scrap** task instead: the Berserkers recycle their opening
Santa Maria and Teamster.

#### What `ai::turindrone` does so far

`EnsureTurinDroneShdefs` with every slot's tech and fittings;
`CheckAiShdefStatus` over each range (`ai::turindrone::check_status`:
the count, the newest by `ShipDesign::designed`, and a design past the
recycling period retired when no ship of it is left); the whole queue
pass in the order above, with the armada potencies, the cruiser and
mine-layer rolls, and the four paid-for classes costed against
`resources_available − queue_cost` and each ship's `true_cost`; scouting
to unknown planets, and colony ships to the nearest colonisable planet,
with the marks table above, from the personality's own slots; the scrap
at year 0 (the miners, and any idle cruiser); the miners' moves — at a
planet worth under four, to the best-worth planet by `FEnumCalcMinerDest`
(a claimed one passed over three times in four), with the Remote Mining
task at warp 6; a lone mine layer's Lay Mines for ever; and the haulers
(slots 8 and 9) by the heart of `IdTargetFreighter` (`1090:2b2e`) —
every planet scored, the best winning: an unowned planet a miner of ours
claims at its worth × 500 over `d/25 + 24`, home when the hold is over a
third full (25,000 when full), an own planet without a starbase and
without a ship in its queue at 25,000 when it is hostile and we are at
home, else what it holds of the two minerals home is shortest of as a
share of the hold, capped by the room left, × 100 over the distance;
Load All of the minerals outward to a mined planet, Unload All to an own
planet with the colonists too — a thousand kT taken aboard at home when
home has 1,200 kT and the planet fewer — and home. `MergeAllShdefs`
(`1090:5a6c`) four times over — the armada classes (slots 4–7, 13–15),
the mine layers, the destroyers, the miners — each fleet of ours joining
the first of its kind at the same place, cargo following the ships; and
the **armadas**, fleets with bombers aboard: at an own starbase they wait
until they hold `potency[2]` bombers and `potency[1]` battleships, at
somebody else's planet they stay unless one of their warships is there
too, and otherwise go for the best planet by `FEnumCalcArmadaDest`
(`1088:3286`) — a foreign planet's 1, or 2 with a starbase, plus 7, 5, 4,
3, 2 or 1 for lying within 50, 100, 150, 200, 300 or 500 light years of
the base, a planet another armada claims counting one time in four, the
nearer of equals winning and a score of one no target; with the "computer
players form alliances" option (`GameState::ais_band`, flag bit 4) the
human players' planets are tried first (`FEnumCalcArmadaHumanDest`,
`1088:3406`). The research plan (`IroEnsureAi`, above; the starbase
design upkeep it also does is not). And `HandleBasicAiTasks` from the
pieces already here: `KeepFleetsMoving` re-speeding every fleet with orders to
`IFindIdealWarp`'s warp, `QueueAiStarbases` by `ai::ships::
queue_ai_starbase`, then for every own planet with 60 kT of people (or
one that is hostile) whose queue's minerals are covered, a starbase
upgrade by `ai::ships::upgrade_ai_starbase`, else `FAIFling`, else
`FQueueAiScanner`, else `FQueueAiDefenses`, and when none of them wrote,
terraforming by `ai::production::queue_ai_terraforming`; then
`AddMinesToBlockedQueues`; and `FillProductionQueue`, the
mines-and-factories fill at every own planet, mines at the front of the
queue and factories at the back. Of those:

* `FQueueAiScanner` (`1090:90d6`) never queues anything: it searches the
  queue and the inventory for a planetary item numbered 18 to 26 — the
  individual scanners — but `InitProduction` (`10d0:015e`) offers a
  scanner only as the generic item 27, so it always answers 0.
* `FQueueAiDefenses` (`1090:939a`): a planet of 160,000 people or more
  wants one defence per 8,000; short of that and with none queued, up
  to four go on the back of the queue, limited by `CMaxDefenses` less
  those built. Resources are not checked.
* `FAIFling` (`1090:7dd6`): for a player of skill 2 or more (bits 10–12
  of the player's `det`), not the Cybertron, with no packet already
  queued: a planet with a starbase whose mass driver reaches warp 10
  (`IWarpMAFromLppl`), more than 3,000 kT of minerals available and a
  one-in-four roll picks, one-in-*n*, another player's planet seen
  within two years within 84 light years (225 with a pair of drivers;
  the squares 7056 and 50625 at `1090:7dce`, tripled when a surface
  mineral tops 12,500 kT) whose defence guess is under 14 or population
  guess under 750 (a quarter of the population — under 300,000 people),
  not Alternate Reality, and not Packet Physics if it has a starbase. The
  driver is aimed at it at warp 13 and the packets queued: eighty
  germanium at 649 resources and a two-in-three roll; then thirty mixed
  at 3,001 / 4,001 / 3,001 kT of ironium / boranium / germanium, fifteen
  at 1,501 / 2,251 / 1,501, else per mineral over 1,250 kT (boranium
  2,500) one packet of it per 200 kT over, one to twenty-five.
* `AddMinesToBlockedQueues` (`1090:1792`): at every own planet whose
  queue's head is not a mine, alchemy or terraforming and is not due
  next year (`PszProductionETA`; "as needed" is taken as 600 years),
  when the planet's resources after the research share over the years
  until it is due would cover its resource cost — so it is waiting on
  minerals — mines go in front of it: the resources divided by the
  race's mine cost, at most the operable deficit; none affordable puts
  one auto alchemy in front instead. The mines are then re-estimated
  and taken out again when they do not help (`1090:1b7c`–`1090:1c41`;
  the transcription keeps them only when the head's date comes forward
  and does not write the count trim).

Not yet: `FixPlanetsUnderAttack`, which never runs in a tutorial game
(flag bit 3); salvage and the drops onto enemy planets in the freighter's
scoring; the `det` bit 15 the first pass clears and the colonise and
armada targeting set (nothing in the TurinDrone's turn reads it); the
AI's own tally that widens the cruiser limit (`vlpbAiData`, the haulers'
assignments). A fleet of battleships or Rogues with no bombers
aboard is given nothing by the TurinDrone — its ladder of `rgcsh` tests
(`1088:4932`–`1088:4eb0`: miners, orders pending, colony ships, freighters,
bombers, scouts and destroyers, mine layers) has no rung for one, and it
waits at its starbase to be merged with bombers; `IdTargetArmada`
(`1088:288e`) is called only from `DoRobotoidAiTurn`. None of it is
verified against a corpus turn yet.

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
