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

### The seven personalities — what is each one's own

**Status:** the research plans and shares recovered for all seven, and
every turn transcribed: the Maid's in `turindrone::basic_turn`; the
TurinDrone's in `turindrone.rs`; the Robotoid's in `robotoid.rs` (*The
Robotoid's turn*, below); the Automitron's in `automitron.rs` (*The
Automitron's turn*); the Cybertron's in `cyber.rs` (*The Cybertron's
turn*); the Rototill's in `rototill.rs` (*The Rototill's turn*); the
Macinti's in `macinti.rs` (*The Macinti's turn*).

Every `Do…AiTurn` opens with `IroEnsureAi(plan, count, &ishdefSBLatest,
pct)` and closes with `HandleBasicAiTasks` then `FillProductionQueue`.
The plan is a byte table in the personality's own code segment — a field
in the top three bits, a level in the low five — and the share is a
constant, most of them zero for the first years:

| Personality | Routine | Plan | Share |
|-------------|---------|------|-------|
| Robotoid | `1088:0312` | 36 bytes at `1088:02ee` | 15 % from turn 10 |
| TurinDrone | `1088:3670` | 31 bytes at `1088:3650` | 15 % |
| Automitron | `1098:01e0` | 18 bytes at `1098:01ce` | 20 % from turn 10 |
| Rototill | `1098:1e22` | none — the lowest field | 15 % from turn 20 |
| Cyber | `10a8:002a` | 42 bytes at `10a8:0000` | 17 % |
| Macinti | `10a0:0008` | 8 bytes at `10a0:0000` | 15 % |
| Maid | `1098:0000` | none | 15 % from turn 20 |

`crates/stars-core/src/ai/personality.rs` holds the tables (`Profile::of`)
and `tests/personalities.rs` runs each in the Berserkers' seat.

**The Maid** (`DoMaidAiTurn`) is nothing but the frame: `IroEnsureAi`
with no plan, `fMarkedPlanets = 0`, `HandleBasicAiTasks(iroCur, rgprod,
-1, …)`, `FillProductionQueue`. It designs no ships and gives no fleet an
order; its planets build mines and factories and whatever the basic tasks
queue for anyone. `turindrone::basic_turn` is it.

**The other five** have a middle of their own of the TurinDrone's size —
a design table (`EnsureRobotoidShdefs` `1088:20ae`, `EnsureISShdefs`
`1098:1af2` for the Automitron, `EnsureMacintiShdefs` `10a0:2e9c`, and
the Rototill's and Cyber's), a pass over the planets' queues and a pass
over the fleets, with their own war tests (`FPotentRobWarFleet`
`1088:31bc`, `FPotentISWarFleet` `1098:012e`, `FPotentMacWarFleet`
`10a0:42ec`) and targets (`IdTargetArmada` `1088:288e`,
`TargetMacArmada` `10a0:4146`, `IdTargetMacFreighter` `10a0:39d9`). The
shape is the TurinDrone's — merges by slot mask, `vrgAiArmadaPotency`
from the year, `CheckAiShdefStatus` over the slot ranges,
`SplitOutShdefs`, then the queues and the fleets — with the ranges, the
thresholds and the designs differing. All five are transcribed, below.

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

The tech bytes it compares are `rgplr + 0x1b` to `0x1f`; `rgTech` starts
at `0x1a`, so they are Weapons, Propulsion, Construction, Electronics and
Biotechnology (an earlier reading of this table had Energy and Weapons
where Weapons and Propulsion belong).

| slot | role | hull | needs |
|-----:|------|------|-------|
| 0 | scout | Frigate (5) | — (the starting Scout design is scrapped once Construction > 5) |
| 1 | colony ship | Colony Ship (15) | — (a starting design here that is not a Privateer is obsoleted first) |
| 2–3 | remote miners | Miner (22) | Con > 6, Elec > 3 |
| 4–5 | battleships | Battleship (9), one of four fittings at random | Weap > 4, Elec > 5, Con > 12, Prop > 6 |
| 6–7 | freighters (counted in the queue pass as `cPlanMax/12 + 8`) | | |
| 8 | cruisers | Rogue (12) | Prop > 4, Con > 7 |
| 9 | cruisers | Galleon (13) | Prop > 6, Con > 10 |
| 10–11 | destroyers | Destroyer (6), one of two fittings | Weap > 4, Elec > 4, Con > 3, Prop > 4 |
| 12 | mine layer | Privateer (11) | Con > 3, Bio > 3 |
| 13 | bomber | Stealth Bomber (18) | Weap > 7, Elec > 6, Con > 5 |
| 14 | bomber | Stealth Bomber (18) | Weap > 10, Elec > 11, Con > 14, Prop > 8 |
| 15 | | Rogue (12) | Weap > 4, Elec > 5, Con > 12, Prop > 6 |

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
3. a **cruiser** (slots 8–9) when Propulsion > 4 and the count is under
   `max(planets/10, 2 × the starbase history's entries)` (see *The
   starbase history* below), or under ten sevenths of that with a
   one-in-four roll;
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
(slots 8 and 9) by `IdTargetFreighter(lpfl, lpplHome)` (`1090:286c`),
`lpplHome` being the planet of the hauler's starbase-history entry (see
*The starbase history* below), else the first own planet with a starbase
— with none of those the fleet pass stops there. Home's **scarcity** is
read first: its least-stocked mineral is the *scarce* one, and the
scarcity is 2 when that stock is under a quarter of the next-least, 1
under a half, else 0; what a planet "has" of use is its stock of the
scarce mineral alone at scarcity 2, or the sum of the three with the
other two halved at scarcity 1. Every planet but the one we are at and
those another hauler of the same design is bound for is scored, the best
winning, distance entering as `(d + 24) / 25` (at least 1): an unowned
planet a miner of ours claims at its worth × 500 over the distance; home
when the hold is over a third full (25,000 when full, else the fill × 20
over the distance); an own planet without a starbase and without a
starbase at the head of its queue at 25,000 when it is hostile and we are
at home, else what it has, over nine, as a share of the hold capped by
the room left, × 100 over the distance; and, for the TurinDrone, another
player's planet worth settling (`vlpbAiPlanet[+3]`) while we are at
home, scored like an own planet. Then salvage
(`FSalvageTargetFreighter2`, `1090:395a`): every stationary packet
(`ith` 1, warp 0) within 200 light years scored like a planet on what it
has; one at our own position is emptied into the hold on the spot — the
scarce mineral, then all three unless the scarcity is 2 — and a hold
that is then full sends us home. The orders (`0x1041`, Transport at
warp 4): to home, Unload All of the three minerals; anywhere else, Load
All — or, when home is short, only the scarce one, unless the planet is
owned and holds less of it than the hold has room for, when all three
are loaded to 66 % (the scarce) and 33 % — and no task at all to salvage
(the order's `grobj` 8). At home with 12,000 people or more, a thousand
kT of colonists come aboard for an owned planet with fewer people than
home; and to an owned planet other than home the colonists are unloaded
— which is how the TurinDrone's haulers drop settlers on a neighbour's
planet. `MergeAllShdefs`
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

#### The starbase history

`vlpbAiData` is the personality's own table, kept in the player's
history file (`FWriteHistFile`, `1048:d637`): a size word, an entry
count, and up to 64 entries of twenty bytes — a planet, a hauler count,
and room for eight hauler ids. `ValidateStarbaseHistory` (`1090:4cf0`),
run from `IroEnsureAi` for every personality but the Cybertron and the
Macinti, from turn 20, keeps it: entries whose planet is no longer ours
are dropped; every own planet with a starbase not yet listed is added;
so is every own planet without one that has 8,000 people or more, mines
and factories both past nineteen, and minerals worth 7,000 kT — each
surface stock plus the square of its concentration over four
(`1090:50b1`) — unless (the Robotoid only) it lies within fifty light
years of a listed planet; every own transport (`FIsAiTransport`,
`1090:4c08`: a hull from the Small Freighter to the Super Freighter, or
the Privateer, Rogue or Galleon) listed nowhere is assigned to the
nearest listed planet with room; and an entry with fewer than four
haulers takes the last hauler of the first entry with at least two more
than it. The TurinDrone reads it twice: the cruiser limit is
`max(planets/10, 2 × entries)`, and a hauler's home is its entry's
planet. `Player::starbase_history` holds it for the game in hand; the
history file's copy is not read or written.

The claim bits in the scratch bytes, which last the turn: `[+1] |= 0x80`
a miner's planet, `[+3] |= 0x80` a colonist drop's planet (no hauler
goes for it), `[+10] |= 0x80` an armada's target, and `[+15] = 4` a
colony ship's planet (`Claimed`, so the next colony ship of the turn
goes elsewhere). `[+2] & 0x80`, which `IdTargetFreighter` tests on a
hostile own planet, is set by nothing in the TurinDrone's turn.

Not yet: `FixPlanetsUnderAttack`, which never runs in a tutorial game
(flag bit 3); the `det` bit 15 the first pass clears and the colonise
and armada targeting set (nothing in the TurinDrone's turn reads it). A
fleet of
battleships or Rogues with no bombers
aboard is given nothing by the TurinDrone — its ladder of `rgcsh` tests
(`1088:4932`–`1088:4eb0`: miners, orders pending, colony ships, freighters,
bombers, scouts and destroyers, mine layers) has no rung for one, and it
waits at its starbase to be merged with bombers; `IdTargetArmada`
(`1088:288e`) is called only from `DoRobotoidAiTurn`. None of it is
verified against a corpus turn yet.

### The Robotoid's turn — transcribed

`DoRobotoidAiTurn` (`1088:0312`) is `ai::robotoid::turn`, run for a
Robotoid player in place of the TurinDrone's. Its skill is bits 10–12
of the mode word (`Control::skill_bits`). In order:

1. `IroEnsureAi(vrgbRobotoidRes, 36, &ishdefSBLatest, turn < 10 ? 0 :
   15)`, then `ValidateStarbaseHistory`.
2. From turn 51, `MergeAllShdefs` with `0x6fc` (slots 2–7, 9, 10), `1`
   (slot 0) and `0xc000` (14, 15).
3. The potencies (`1088:0399`): `[0]` = 4, from turn 131 `4 + (turn −
   120) / 20`, at most 50; `[1]` = half that; `[2]` = 6, from turn 116
   `6 + (turn − 100) / 22`, at most 12; `[3]` = `min([2]/2 − 1, 3)`.
   `robotoid::potency`.
4. `CheckAiShdefStatus` with the recycling period 50 (70 from turn 120,
   100 from 200): slots 14–15, after which an old Nubian (hull 29) in
   14 or 15 is unmarked; 11–13; 9–10; 2–5; and 6–7 at one and a half
   times the period. From turn 81, `SplitOutShdefs` for the old designs,
   then slot 0, then slot 1, then slots 11–13.
5. `EnsureRobotoidShdefs` (`1088:20ae`), below.
6. `FShouldWeBuildColonizers` (`1090:476a`): skill 0 builds none in odd
   years; before turn 30 always; then with no live colony-ship design
   (hull 14 or 15) none; with more colony fleets than `20 × size + 10`
   (`mdSize`, `GameState::galaxy_size`) none; with colony fleets plus
   every player's planets past four fifths of the galaxy none; else yes
   when the colony ships built so far exceed that sum by under 26, or
   with under five colony fleets one roll in two.
7. Other players' planets are marked worth `min(pop/250 + 1, 6)`, plus
   one for a starbase (`vlpbAiPlanet[+10]`), with `[+9] = 1`.
8. **The planet pass**, over own planets with a starbase and at least
   20,000 people that have no ship queued (`robotoid.rs`, *The planet
   pass*):
   - cruisers (newest of 11–13): while under eight tenths of
     `max(4 × history entries, owned/8)`, or under all of it one roll
     in three, one;
   - colony ships (slot 1), when `FShouldWeBuildColonizers` said yes or
     under 26 colony fleets exist and `Random(8 × history entries)` is
     zero, from turn 5: two before turn 21, one after, plus one where
     `pop × pct_true_max_growth > 2300` and the planet makes over 35
     resources at skill 1 or more, plus another past 3,600 and 50 at
     skill 2 or more;
   - mine layers (a Frigate in slot 0), one roll in four: four, when the
     fleet of them here is under ten (under seventeen one roll in ten)
     and `Random(2 × count + 1)` is zero;
   - bombers (newest of 9–10): where an own fleet here passes
     `FPotentRobWarFleet` (ships in 2–5 plus twice those in 6–7 at least
     `potency[0]`) and holds under `potency[2]` bombers, four — six on
     a rich planet (every surface mineral at 5,000 kT or more) — and
     the planet is done;
   - warships (newest of 2–5, or one roll in two the newest of 6–7):
     skipped one roll in two once `planets/7 + 6` of the newest exist;
     the queue's cost is taken from the planet's resources and a
     shortfall ends the planet; three fifths of the design's cost is
     taken next and a shortfall skips to the armada; else five on a rich
     planet, one otherwise;
   - armada ships (newest of 14–15), while under `planets/12 + 8`
     exist: up to five paid for in full out of what is left.
9. **The first fleet pass**: a leg to a space object (`grobj` 8) over
   200 light years off is cut. A fleet of slot-0 ships from turn 41 with
   no leg is a mine layer: one with over six, one roll in five, wanders
   to `IdRandomPlanetNearby(pt, 105, avoid starbases)` (`1090:5f54`, a
   reservoir draw, redrawn up to twice when it lands on a starbase) at
   warp 4; the rest set Lay Mines on their current waypoint. Otherwise
   an attack fleet (`FIsAiAttack` `1090:4a72`: hull 5–10, or a Meta
   Morph or Nubian with under 500 kT of cargo) loses a Lay Mines task
   and, with ships in 2–7, claims the planet it is bound for or sits at
   (`[+10] |= 0x80`); a transport (`FIsAiTransport`) with nothing aboard
   bound for a planet not ours has its orders blown away. Then: before
   turn 21 a fleet with slot-0 ships is scrapped (the starting scouts);
   a colony ship (slot 1) with no leg, from turn 5 (at once when
   `mdStartDist` is 0), at somebody else's non-AR planet with colonists
   aboard and skill over 1 unloads them as an invasion (a Transport task
   on its current waypoint) and heads for the nearest own starbase;
   otherwise it takes a thousand colonists from the own planet it sits
   at (`XferAiSupply(…, 10)`) and goes to colonise the nearest
   colonisable planet, or is scrapped where it sits when there is none.
10. **The haulers**, once an own starbase exists: every transport with
    no leg is put on battle plan 4 and sent by `IdTargetFreighter` from
    its starbase-history planet.
11. **The last pass**: a fleet whose ships are all old is scrapped at an
    own planet (one roll in five at one without a starbase), or sent to
    the nearest starbase. A fleet with ships in 2–10 goes to
    `IdTargetArmada`. An attack fleet not chasing a fleet, with over
    seventy armada fleets about (fifty from turn 121), or over sixty
    (forty) one roll in three, joins a buddy by `FFindBuddyAndJoinUp(14,
    15, 36, 72)` (`1090:9d18`: the nearest other own fleet with 14/15
    ships, within 36 ly, or 72 one roll in two, at warp 6) — unless it
    already holds twenty of the newest armada ship and rolls nineteen in
    twenty; else `IdTargetAttack` (`1090:1ffe`): the nearest of the
    other players' fleets (a computer player's passed over when the
    computer players band together, `ais_band`) not too many of ours
    are after (one already chased is passed over one roll in three, one
    chased by five times our ships one in fifteen), the target when
    within 180 ly; further off, a fleet under half fuel goes home to a
    starbase, else the nearest planet to that fleet none of ours is
    bound for; with no enemy fleet in view, the nearest planet of
    somebody else's, the nearest nobody holds, or one at random. The leg
    is laid at warp 4 (`FMoveAiFleet`).
12. `HandleBasicAiTasks`, `FillProductionQueue`.

#### The Robotoid's design slots

`EnsureRobotoidShdefs` draws into a slot that is free — empty or retired
— when the tech allows (`rgTech` 0–5 = Energy, Weapons, Propulsion,
Construction, Electronics, Biotechnology), some slots only so many years
after the one before was designed. Each is five tries of
`FCreateAiShdef` at a fitting drawn by `Random`; the fittings are the
part-class bytes at `1088:1f80` by the offsets at `1088:1f34`
(`robotoid::fitting`). `FChangeAiShdef` (`1090:08a2`) stamps the new
design with the year (`turindrone::install_design`).

| slot | role | hull | needs |
|-----:|------|------|-------|
| 0 | mine layer | Frigate (5), `[24,26,25,10]` | skill > 1, no ship of the slot left, the slot's design not a Frigate, Bio > 3, Elec > 4, Con > 5, Prop > 5, Energy > 5 — the old design retired first |
| 1 | colony ship | never redrawn | |
| 2–5 | warships | Meta Morph (31), fittings 0–3 for the even slots and 4–7 for the odd | Weap > 9, Con > 9, Prop > 8, Energy > 5; slots 3–5 twelve years after the one before |
| 6–7 | battleships | Battleship (9), fittings 27–30 for 6 and 31–34 for 7 | Bio > 3, Elec > 9, Con > 11, Prop > 11, Energy > 5, Weap > 14; slot 7 twenty years after 6 |
| 9–10 | bombers | Battleship, `[8,13,10,33,33,33,33,33,17,20,19]` (`1088:2095`), else B-52 (19) fitting 24 or 25 | Weap > 13; slot 10 fifteen years after 9 |
| 11–13 | cruisers (haulers) | Privateer (11) fitting 14 (slot 11) or 15 while Con < 10, else Meta Morph fittings 8–13 | Prop > 1, Con ≥ 4, 7, 10 in turn; 12 and 13 fifteen years after the one before |
| 14 | armada | Nubian (29) `[8,10,10,7,5,20,20,4,4,19,4,2,3]` (`1088:20a0`), else Destroyer (6) fittings 16–19 | Weap > 4, Elec > 5, Con > 5, Prop > 5, Energy > 1 |
| 15 | armada | Nubian, else Destroyer fittings 20–23 | Elec > 9, Con > 7, Prop > 8, Weap > 13 |

`tests/robotoid.rs` runs it in the Berserkers' seat on the tutorial's
world: the starting scouts scrapped, colony ships queued two at a time
and sent out, nothing scouting, the designs drawn on the hulls the table
names. Not verified against a corpus turn: the Robotoid games in
`fixtures/` are being read for that.

### The Automitron's turn — transcribed

`DoAutomitronAiTurn` (`1098:01e0`) is `ai::automitron::turn`. In order:

1. `IroEnsureAi(vrgbAutomitronRes, 18, &ishdefSBLatest, turn < 10 ? 0 :
   20)`, then `ValidateStarbaseHistory`.
2. `MergeAllShdefs(0x4000)` (slot 14) from turn 31; from turn 51 `0x1e0c`
   (slots 2, 3, 9–12), `0x40` (slot 6) and `0x4000`.
3. The potencies (`1098:0280`): `[0]` = 3, from turn 131 `3 + (turn −
   120) / 20`, at most 50; `[1]` = half that; `[2]` = 6, from turn 116
   `6 + (turn − 100) / 22`, at most 12; `[3]` = `min([2]/2 − 1, 3)`.
4. `CheckAiShdefStatus` with the recycling period 50 (70 from turn 120,
   100 from 200) over slots 11–12, 4–5, 2–3 and 9–10; from turn 61,
   `SplitOutShdefs` for the old designs.
5. `EnsureISShdefs` (`1098:1938`), below.
6. The planets that could be settled are counted: unowned, scanned
   (`det & 0xff > 2`), `PctPlanetOptValue > 0`.
7. **The planet pass**, in planet order: every planet gets `[+9] = 1`;
   an own planet hostile as it stands (`PctPlanetDesirability < 0`) is
   marked `[+2] = 1` and passed over; somebody else's planet is marked
   `[+10]` = 1 or 2 with a starbase and `[+3] = 1` when its opt value is
   positive. An own planet with a starbase and over 150,000 people
   (`rgwtMin[3] > 0x5db`) that has no ship queued:
   - haulers (newest of 4–5) from Propulsion 5: while under
     `max(2 × history entries, owned/10)`, or under ten sevenths of it
     one roll in four, one;
   - a colony freighter (slot 1), from turn 11, while there is somewhere
     to settle and no ship of the design exists, one;
   - mine layers (slot 6 live), one roll in three: three, when the fleet
     of them here is under ten (under seventeen one roll in eight) and
     `Random(2 × count + 1)` is zero;
   - bombers (newest of 2–3): where an own fleet here passes
     `FPotentISWarFleet` (ships in 11–12 plus twice those in 9–10 at
     least `potency[0]`) and already holds `potency[2]` bombers, four,
     and the planet is done;
   - then, paid for out of what is left after the queue, up to five of
     each while under the limit, stopping at the first that cannot be
     paid: the newest of 11–12 to `planets/12 + 8`, the newest
     Battleship (9–10) to `planets/24 + 4`.
8. **The first fleet pass**: attack fleets (`FIsAiAttack`) are listed;
   a transport (`FIsAiTransport`), or a colony freighter whose
   destination is a planet marked `[+3]`, bound for — sitting at with no
   leg, else its next waypoint's planet — a planet that is gone or
   somebody else's: with colonists aboard, the planet marked `[+3]` and
   its owner not Alternate Reality, a Transport order unloading all
   colonists is written for that planet (`0x1101`, item `0x2000`) and the
   planet claimed (`[+3] |= 0x80`); otherwise the orders are blown away.
   A colony freighter bound for somebody else's planet not so marked has
   its orders blown away too. No move to a starbase follows, unlike the
   TurinDrone's.
9. **The second fleet pass**, own fleets only. Under way: a fleet with
   under 2 mg of fuel, when the scout design's engine is no scoop (item
   under 10), is scrapped. With no leg:
   - mine layers (slot 6) with no task: Lay Mines, 5 years;
   - colony freighters (slot 1): scrapped unless the slot's design is a
     Medium Freighter (`1098:12b4`); empty and not at an own planet of
     20,000 people or more: at an own starbase they wait, else they go
     to the nearest own starbase (`FMoveToNearestStarbase`, only when
     the design's engine item is over 1) or are scrapped; otherwise
     `IdNearestColonizablePlanet`, 15,000 colonists from the own planet
     they sit at (`XferAiSupply(…, 3, 0x96)`), `FColonizeAiFleet` and
     the planet claimed (`[+15] = 4`);
   - transports: `IdTargetFreighter` from the starbase-history planet,
     else the first own starbase — with none, the rest of the fleets are
     left alone;
   - bombers aboard (2, 3): the armada, at warp 4 (`0x1140`): in deep
     space from the first own starbase planet; at an own starbase it
     waits while it holds under two of either bomber and under three of
     either Battleship (`1098:16f8`); at somebody else's planet it stays
     unless one of their warships is there; the target by
     `FEnumCalcArmadaDest` (`FEnumCalcArmadaHumanDest` first with the
     alliance option), claimed with `[+10] |= 0x80`;
   - scouts (slot 0): out of fuel as above, scrapped; else
     `IdTargetScout`.
10. `HandleBasicAiTasks`, `FillProductionQueue`.

Nothing in the routine queues a scout: the slot-0 design is redrawn
every year no ship of it exists, but only the ships the player began
with ever scout, and once the starting scouts (fuel engines) are out of
fuel the Automitron sees no further than its planets. On the tutorial's
world that leaves it with two planets after eighty years; whether the
original does the same is for a corpus game to say.

#### `IdTargetScout` (`1090:61de`)

An armed fleet (`FFleetMightHaveTeeth`) looks for the nearest of the
other players' fleets — a computer player's passed over when the
computer players band together (`fOnlyHumans`, game flag bit 4), and
one another attack fleet of ours is already chasing one roll in three.
Within 180 light years (squared distance under `0x7e90`) it is the
target, a leg of class fleet. Further off, a fleet with half its fuel
capacity or less goes home (`FMoveToNearestStarbase`); else the nearest
planet to that fleet that no attack fleet of ours is bound for is the
target, unless it is the one we orbit. With no enemy in view and the
alliance option on, the search is repeated without it.

Failing all that, and for an unarmed fleet: `IdNearestUnknownPlanet`
(`1090:15a4`) — the nearest planet marked unknown (`[+15] == 0x10`); with
none, `LpplFindBestEnum(home planet, FEnumCalcArmadaDest)` from
`rgplr[].idPlanetHome`, else a planet at random. A planet target is
claimed (`[+15] = 4`) and the leg laid at `IFindIdealWarp`. The wormhole
the routine may be handed (`plpthWorm`, one time in twenty at a planet
by `IdNearestUnknownPlanet`) is not kept, as in the other transcriptions.

#### The Automitron's design slots

`EnsureISShdefs` draws into a retired slot (the scout and colony slots
also whenever no ship of the design is left, the live design retired
first) when the tech allows (`rgTech` 1–5 = Weapons, Propulsion,
Construction, Electronics, Biotechnology). The fittings are the
part-class bytes at `1098:0078` by the offsets at `1098:0064`
(`automitron::fitting`).

| slot | role | hull | fitting | needs |
|-----:|------|------|---------|-------|
| 0 | scout | Scout (4) | 1: `[30,26,4]` | — |
| 1 | colony freighter | Medium Freighter (1) | 0: `[30,31,10]` | — |
| 2 | bomber | B-17 (17) | 15: `[8,21,23,12]` | Weap > 7, Elec > 6, Con > 5, Prop > 6 |
| 3 | bomber | B-52 (19) | 16: `[8,21,23,23,23,12,10]` | Weap > 10, Elec > 11, Con > 14, Prop > 8 |
| 4 | hauler | Medium Freighter (1) | 14: `[8,16,10]` | Prop > 4 |
| 5 | hauler | Super Freighter (3) | 18: `[8,16,10,19]` | Prop > 6 |
| 6 | mine layer | Privateer (11) | 17: `[30,10,12,25,32]` | Con > 3, Prop > 4, Bio > 5 |
| 9 | battleship | Battleship (9) | 10–13, one at random, five tries | Weap > 4, Elec > 5, Con > 12, Prop > 6 |
| 14 | destroyer | Destroyer (6) | 4: `[30,0,0,13,9,18,11]`, four tries | Weap > 4, Elec > 5, Con > 3, Prop > 4 |

Slots 10, 11 and 12 are never drawn; the pass counts and builds whatever
the player began with there.

`tests/automitron.rs` runs it in the Berserkers' seat on the tutorial's
world: the starting colony ship scrapped, the scouts sent out, the
Medium Freighter colony design drawn and sent to settle.

### The Cybertron's turn — transcribed

`DoCyberAiTurn` (`10a8:002a`) is `ai::cyber::turn`. The Cybertron is the
Packet Physics opponent and plays unlike the others: it spreads its
people by freighter, throws mineral packets at its own short planets
and at its enemies, and fires packets at the galaxy's edge to scan with
them. Its per-planet scratch is `vlpbAiData` read as **words**: a lasting
half (`Player::cyber_words`: bits 0–2 the scanner-packet direction, 3 a
colony ship queued last year, 4 a scanner packet just sent, 5–6 a
three-year cooldown on a planet packets were thrown at, 7 more packets
owed) and a half zeroed each year (bit 0 a colony ship here found
nowhere to go, 1–2 freighters that unloaded here, 3–4 freighters bound
here, 5 defenders here short, 6 defenders here, 8–10 short of each
mineral). In order:

1. `IroEnsureAi(vrgbCyberRes, 42, &ishdefSBLatest, 17)` — no starbase
   history; `EnsureCyberAiShdefs` (`10a8:4826`), below; then, every
   year, `MergeAllShdefs` with `1`, `0x30`, `0xc000`, `0x3c0` and
   `0x3c00` (the mine layers, the Destroyers, the starbase defenders,
   and each warship group).
2. The attack strength (`10a8:0090`): 1, `(turn − 50)/10 + 1` past
   turn 50, plus `(turn − 100)/10 × turn/100` past 100. Its own
   potencies (`vrgAiCyberArmadaPotency`, from 3 and 6 like the
   Automitron's) are set and never read: `TargetCyberArmada` reads the
   shared `vrgAiArmadaPotency` (`1120:515c`), which only the TurinDrone,
   Robotoid and Automitron turns write — `GameState::ai_armada_potency`
   keeps that quirk, so a Cybertron alone fights at zero potency.
3. The recycling period: 50, 70 from turn 120, 100 from 200, 300 from
   400. `CheckAiShdefStatus` over 4–5, 14–15 and 2–3, the newest of each
   never counted old. Each warship group (6–9, 10–13) whose base design
   is live but past the period has its unused designs retired from the
   top down (the base only when a higher slot was kept) and the rest
   marked old; the group with the newest live base is the one to build.
   From turn 81, `SplitOutShdefs` for the old designs, then slots 2 and
   3 each and together.
4. **The planet marks**: every planet's cooldown counts down; an own
   starbase planet is marked short of each mineral under 1,000 kT (10 at
   a starbase in design slot 1, 3, 6 or 8); the other players' planets
   are worth `min(popguess/250 + 1, 6)` plus one for a starbase.
5. **The first fleet pass** (`IdNearestColonizablePlanet` marks first):
   fleets with slot 14/15 ships are counted as defenders; those with
   slot-0 ships as mine-layer fleets (and their Frigates); a fleet with
   ships in 4–13 is an attack fleet — roaming when in deep space with no
   planet leg, else its destination's mark is claimed (`0x80`) and it
   counts as bound; a colonist freighter (2, 3) under way to a planet
   with colonists aboard adds to that planet's bound count.
6. **The second fleet pass**: a fleet of old ships is scrapped at an own
   planet (one in five at one without a starbase) or sent to the nearest
   starbase. Defenders at a planet mark it defended, and short when the
   fresh ones (designed within the period less ten) are under twice the
   attack strength. A fleet with ships in 4–13: without Destroyers (slot
   4) it is an armada, `TargetCyberArmada`, then one time in four
   `FFindBuddyAndJoinUp` with its own group (6–9 or 10–13) within 100 or
   200 light years; with Destroyers, `IdTargetAttack` once twice their
   number reaches the attack strength (or under way already), then the
   same join with slots 4–5. With no leg: the starting scouts (slot 0)
   are scrapped before turn 6; a colony ship loads 25,000 colonists at
   an own planet and goes to the nearest colonisable planet, or marks
   its planet as having nowhere to send one; a colonist freighter goes
   on battle plan 4 to `DoCyberFreighter`; mine layers from turn 41 join
   a buddy (`FFindBuddyAndJoinUp(0, 0, 72, 108)`) when over 55 layer
   fleets fly (over 40, two rolls in three), a fleet of over six wanders
   one time in five to `IdRandomPlanetNearby(105)` with a Lay Mines leg
   at warp 4, and the rest lay mines where they are.
7. **The planet pass**, over own planets in AI order: a planet that
   cannot pay for its queue is left; desirability under 10 queues
   terraforming, `resources left / 70 + 1` of it; else under the opt
   value with over 70 resources left, one. Then at a starbase planet not
   in the small design slots: a colony ship unless one was queued last
   year and the planet grows under 5,500 a year, not where a colony ship
   found nowhere, while under forty exist, when
   `FShouldPlanetBuildColonizer` (`1090:9f30`: always before turn 60,
   then by the nearest colonisable planet — within 350 ly always,
   within 300 one in two, else within 250 one in two of that) — a
   second one over 15,000 growth before turn 100; a colonist freighter
   (newest of 2–3) over 200,000 people, under fifty existing, none
   unloaded here this year, where `FEnumDropOffStage2` finds a planet;
   Frigate mine layers four at a time, one in four, under ten thousand
   flying, the fleet here under ten (under seventeen one in ten), and
   `Random(2 × here + 1)` zero; a starbase defender where the defenders
   are short, else with none here and under forty defender fleets one in
   two within 300 ly of an enemy planet, one in ten otherwise; then
   `iAddAttackFleet` (below), the Destroyer withheld past 120 roaming
   fleets and the group past 250 bound.
8. `HandleBasicAiTasks`, `DoCyberPackets` (below), `FillProductionQueue`.

#### `iAddAttackFleet` (`10a8:4eba`)

Nothing, ninety times in a hundred when the planet could still operate a
hundred more mines or factories, sixty otherwise. Else with a warship
group and a roll (made first) of 51 or more: two of the group's first
two slots, one of the third three times in four, one of its bomber one
time in two (answers 1, a bound fleet). Else with a starbase defender
and a roll of 26 or more: one (2). Else a Destroyer: one (3).

#### `TargetCyberArmada` (`10a8:51a4`)

Under way, the fleet keeps going when chasing a fleet within 250 light
years, or bound for a planet somebody else holds, an own planet with a
starbase, or a planet not seen this year. Its weight of war is the ships
in slots 6, 7, 10 and 11 plus twice those in 8 and 12; its bombers those
in 9 and 13. In deep space, `MoveToNearestPlanetOrEnemy(450)`
(`1090:7040`): the nearest enemy planet within 450 ly of its next
waypoint, else the nearest planet, at warp 4. At an own planet it waits
under the first and third potencies unless, at skill 2 or more, its
weight is over 60 and over twice the first potency and a run of rolls
(five in ten; then over three times the potency or three in ten; then
over 120 or three in ten) sends it. At another player's planet, too weak
for the second and fourth, it clears its task and goes home to the
nearest own starbase (`FEnumOurStarbase`) unless at skill 2 or more the
same rolls keep it in the war; strong enough there, it stays. Otherwise
the target is `FEnumCalcArmadaDest` from where it is (the human-only
enumerator first with the alliance option), claimed; with none, the
nearest fleet not ours (`FEnumCalcEnemyFleets`, `1088:325c`). The leg is
laid at warp 4 (`0x1040`).

#### `DoCyberFreighter` (`10a8:37b0`)

In deep space the freighter heads for the nearest planet. At an own
planet under 200,100 people it unloads 1,000 kT of colonists, else it
loads 1,000 (`XferAiSupply(…, 3, ±1000)`); at an unowned planet, an
Alternate Reality player's or one with a starbase it keeps what it has;
at anyone else's it drops everything (a Transport order) and heads for
the nearest own starbase. Empty, it goes to the nearest own starbase
planet over 220,000 people (`FEnumPickUp`, `10a8:3f00`); loaded, to the
nearest own planet under 20,000 people no freighter is bound for within
170 ly (`FEnumDropOffStage1`, `10a8:3d00`), else one that with the
freighters bound for it counted at 21,000 each is under 100,000, fewer
than three bound (`FEnumDropOffStage2`, `10a8:3dfe`); with none it
unloads where it stands if that is ours and counts the unloading. The
leg is at the ideal warp.

#### `DoCyberPackets` (`10a8:1a78`)

At every own starbase planet — here only one whose starbase carries a
mass driver; the original queues packets regardless and production
never builds them:

1. **Supplies**, unless more packets are owed (bit 7): a planet not in
   the small starbase slots with over 700 kT of some mineral and `res/2
   /5 ≥ 7` finds the nearest own starbase planet, cooldown over, short
   of a mineral it has over 700 kT of, within `3.5 × min(warp)²` where
   the warps are the two drivers' (`IWarpMAFromLppl` `1090:5d5e`, plus
   one for a pair) — `FEnumNeedMinerals` (`10a8:3f5e`); for each such
   mineral up to seven packets (of the `res/2/5`) are queued and the
   driver aimed at the lesser of the two drivers' warps.
2. **Attack**, otherwise, at skill 2 or more or at skill 1 one in three:
   the throwable — each mineral less 70 kT summed, capped at `70 × ((res
   /2 − 5)/5)` — over 150 kT finds the nearest other player's planet
   (not Alternate Reality, cooldown over) within `2.5 × speed²` (speed =
   driver warp + 3) that the throwable, decayed by `pow(0.75 or 0.875
   paired, distance/speed²)`, would wipe out: a packet lands `(speed² −
   their driver²) × (100 − (defence guess + 5)) / 16000` of itself, and
   `min(1000, 4 × (population guess + 25))` kT must land
   (`FEnumPktAttack`, `10a8:4204`). The amount to throw is that need
   divided by the decay, in 70 kT packets of whatever mineral is most to
   hand; the driver is aimed at warp 3 over its own; the target's
   cooldown is set to three years; bit 7 marks more owed when the flight
   is over a year (`speed² < distance`).
3. **Scanning**, with no scanner packet just sent (bit 4) or more owed:
   a direction is rolled (`Random(7)`, plus one when it repeats the
   last) and `IdGetBestScannerDest` (`10a8:282a`) takes a point on the
   galaxy's edge that way — the galaxy is `400 × size + 400` across —
   jittered along the edge by `Random(0.3 × edge) − 0.15 × edge` and
   stepped inward by up to the speed squared; the planet nearest that
   point, not ours and a year's flight or more off, is the target. One
   packet of the mineral most to hand goes into the queue when 170 kT of
   it are left (`FAddPacketToQueue`, `10a8:2bc0`), at warp 3 over the
   driver's; the target's cooldown is set and bit 4 raised — or, with
   more owed, the old target kept and bit 7 cleared.

The population and defence guesses are read from the planet itself (a
quarter of the population; the defences over ten) rather than the
`uPopGuess` nibbles the file carries.

#### The Cybertron's design slots

`EnsureCyberAiShdefs` (`10a8:4826`); the fittings are the part-class
bytes at `10a8:46f8` by the word offsets at `10a8:46b0`
(`cyber::fitting`). A draw of *n* tries rolls `Random(c)` with `c`
counting down from *n*.

| slot | role | hull | when |
|-----:|------|------|------|
| 0 | mine layer | Frigate (5), entry 12 | the starting design retired at skill 2+ once no ship is left, from turn 6; drawn whenever retired |
| 1 | colony ship | never redrawn | |
| 2, 3 | colonist freighters | Privateer (11), entries 10 and 11 | from turn 21; 3 twenty years after 2 |
| 4, 5 | destroyers | Destroyer (6), entries 0–4 / 5–9 | from turn 31 (5 twenty years after 4): the set's first until turn 75, then five countdown tries |
| 6–9, 10–13 | warship groups | Nubian (29) 33–35, three tries; Battleship (9) 29–32, four; 26–28, three — each success moving down a slot; Cruisers (7) 17–25 fill the rest by slot; the fourth slot a Nubian bomber (16), else Battleship bomber (15), else B-52 (19) 14 then 13 | the first from turn 41, the second thirty years after the first's base |
| 14, 15 | starbase defenders | Battleship 26–32 (seven tries), else Cruiser 17–25 (nine), else Destroyer 0–9 (ten) | from turn 31; 15 twenty years after 14 |

`tests/cyber.rs` runs it in the Berserkers' seat: the starting scouts
scrapped, the groups drawn on the hulls the table names, warships queued
and sent hunting. On the tutorial's world the Cybertron — a race that is
not Packet Physics, with a starbase that has no driver — never settles a
second planet: its colony ship finds nothing colonisable in scanner
range in year 0 and marks the homeworld as having nowhere to send one,
its scouts are scrapped, and no packet flies to scan for it.

### The Rototill's turn — transcribed

`DoRototillAiTurn` (`1098:1e22`) is `ai::rototill::turn`, the simplest
of the five middles. It hands `IroEnsureAi` no plan and 15 % from turn
20; its `EnsureCAShdefs` (`1098:3020`) is an empty routine — the
Rototill **never designs a ship** and builds only what the player began
with; it merges nothing, rates no armada and recycles nothing. In order:

1. `IroEnsureAi`, `EnsureCAShdefs`, and `ValidateStarbaseHistory` (run
   for it by `IroEnsureAi`).
2. The planets that could be settled are counted (unowned, scanned, opt
   value positive), and the planet marks made: unowned scanned planets
   get their mineral worth (`vlpbAiPlanet[+1]`); an own planet hostile
   as it stands is marked `[+2] = 1` and passed over; somebody else's
   planet is marked `[+10]` (1, 2 with a starbase) and `[+3] = 1` when
   its opt value is positive.
3. **The planet pass**, in planet order: an own planet with a starbase
   and over 99,900 people (`rgwtMin[3] > 999`) that has no ship — nor
   the first starbase design (`item < 0x11`) — queued builds two scouts
   (slot 0) in year 0; after that one colony ship (slot 1) at the first
   such planet each year, while none exist or the ships plus one are
   fewer than the planets to settle.
4. **The first fleet pass**, the Automitron's (`automitron::drop_pass`)
   with `FIsTurinDroneAiAttack` listing the attack fleets, the miners
   (slots 7, 8) with orders left to their own rung: in deep space with a
   leg the leg's planet is claimed (`[+1] |= 0x80`); at an unowned
   planet that one; at an owned planet the orders are blown away.
5. **The second fleet pass**: miners at a planet worth under four move
   to the best of the rest (`FEnumCalcMinerDest`, `1088:5f32`: a
   claimed planet passed over three times in four) and dig there at warp
   6 (`0x1163`). With no leg: a colony ship (slot 1) — empty and away
   from an own planet of 5,000 people, at an own starbase it waits, else
   it goes to the nearest own starbase (only when its engine is not one
   of the first two) or is scrapped; otherwise `IdNearestColonizablePlanet`,
   2,500 colonists from the own planet it sits at (`XferAiSupply(…, 3,
   0x19)`), `FColonizeAiFleet` and the planet claimed — with nowhere to
   go the routine would try a wormhole (`FGotoWormholeAiFleet`), not
   kept; a transport: `IdTargetFreighter` from the starbase-history
   planet, else the first own starbase (with none, the rest of the
   fleets are left); bombers aboard (13, 14): the armada at warp 4, in
   deep space from the first own starbase planet, waiting at an own
   starbase while under two of either bomber, by `FEnumCalcArmadaDest`;
   scouts (slot 0): on the Quick Jump 5 with under 2 mg of fuel,
   scrapped; else `IdTargetScout`.
6. `HandleBasicAiTasks`, `FillProductionQueue`.

`tests/rototill.rs` runs it in the Berserkers' seat: no design drawn in
forty years, scouts out, colony ships settling and replaced.

### The Macinti's turn — transcribed

`DoMacintiAiTurn` (`10a0:0008`) is `ai::macinti::turn`. The Macinti is
the Alternate Reality opponent: its colony ships carry an Orbital
Construction Module (part class 40), which only an Alternate Reality race
may build — run under any other race it cannot draw them at all — and its
haulers move people between its planets by the resources they would make
there. In order:

1. `IroEnsureAi(vrgbMacintiRes, 8, &ishdefSBLatest, 15)`; no starbase
   history.
2. **Slot 7** holds a second colony-ship design for the first forty
   years, or while a live colony design sits there; once the Enigma
   Pulsar (engine 15) can be built and no ship of it is left it is
   retired for a warship (`local_66` = "early colony"; `local_38` = the
   last warship slot, 6 or 7).
3. `MergeAllShdefs` with `0x37c` (2–6, 8, 9) or `0x3fc` (2–9), then
   `1`, `0xc000` (the miners) and `0x3000` (the Destroyers).
4. The potencies (`10a0:0138`): 6, from turn 131 `6 + (turn − 120)/20`,
   at most 50; half; 6, from turn 116 `6 + (turn − 100)/22`, at most 12;
   half less one, at most 3 — written to the shared `vrgAiArmadaPotency`.
5. The recycling period 50/70/100. `CheckAiShdefStatus` over 12–13 (an
   old Nubian unmarked), 14–15 (period 5000: the miners never go old),
   10–11, 8–9, 2–4 and 5 to the last warship slot. Once mining robot 6
   can be built and slot 15 is live, the older miner design — 15 when
   14 already carries robot 6 (unless 15 does too, the Enigma Pulsar
   cannot be built, or 14 has it), else 14 — is retired when no ship of
   it is left, so that `EnsureMacintiShdefs` redraws it. From turn 81,
   `SplitOutShdefs` for the old designs, then slots 0, 1, {14, 15} and
   {10, 11}.
6. `EnsureMacintiShdefs` (`10a0:2e9c`), below; `EnsureMacintiStarbase-
   Designs` (`1090:7688`) — only its recycling table is kept
   (`Player::mac_starbase_recycle`), which the starbase upgrade reads,
   with `PctPlanetCapacity` (`1048:6b2c`: `(max/2 + pop × 100)/max`, at
   most 999; 0 for an Alternate Reality race, whose maximum is not
   modelled). `FShouldWeBuildColonizers`. The colony slot is 1 when its
   design has the Enigma Pulsar, else 7 while it holds a live colony
   design, else 1; with slot 1 chosen while slot 7 is still the early
   colony slot and slot 1's design is over five years old, the old
   slot-7 ships under way are sent home.
7. **The counts**: fleets with miners (14, 15), mine layers (0),
   Destroyers (12, 13), freighters (10, 11 — one under way to a planet
   with colonists marks it bound, `vlpbAiPlanet[+14] |= 1`), and
   warships (2–9). Own planets are marked `[+13] = 1` (what
   `FShouldPlanetBuildColonizer` steps over). From turn 121, with the
   Genesis Device buildable, `owned/20` of them (at most ten) may be
   queued this year. Other players' planets get their worth
   (`min(popguess/250 + 1, 6)`, plus one for a starbase).
8. **The planet pass**, over own planets in AI order with a starbase,
   over 19,900 people, a starbase design not in slot 0 (nor slot 1 from
   turn 26), and a queue under 24 long holding no ship:
   - **packets**, from turn 121 at a driver of warp 10 or more and a
     million people, one roll in four, with no packet queued: the first
     mineral over 5,000 kT from a random start round the three goes to
     the own planet with the least of it (under 100,000) that has such a
     driver within 302 light years — a fifth of the stock, at most
     20,000 kT, in hundred-kT packets at warp 11 — and the planet is
     done;
   - a freighter (newest of 10, 11), one roll in three, under 64 fleets
     and under a quarter of the planets owned;
   - a colony ship when `FShouldWeBuildColonizers` says so (unless over
     40 fleets carry one past turn 120 or over 100 carry one), else
     eight times in a hundred; never over 49 fleets past turn 120; every
     mineral at 30 kT or the planet is done; then by
     `FShouldPlanetBuildColonizer` (over the planets not ours): one,
     from turn 5 one more past 2,300 growth and 35 resources at skill 1
     or more, a third past 3,600 and 50 at skill 2 or more;
   - a miner (newest of 14, 15), one roll in two, under sixty miner
     fleets and 5,000 ships, where the miners here dig 1,000 mines'
     worth or less (`CMineFromLpfl`): where three times the
     concentrations summed is over what they dig and at least 151, one;
     else under thirty fleets, nine rolls in ten;
   - mine layers, four, one roll in four, under sixty layer fleets and —
     the routine's own slip — under 7,500 ships of the newest *miner*
     design, the fleet here under ten (under seventeen one in ten), and
     `Random(2 × here + 1)` zero;
   - "rich", as the routine tests it (`local_9c`): the *first* mineral
     under 5,000 kT is germanium;
   - bombers (newest of 8, 9): under 140 warship fleets and (under sixty,
     or over 2,000 resources): past 110 fleets one roll in three; else
     where a `FPotentMacWarFleet` here holds under `potency[2]` bombers
     — twelve on a rich planet, four otherwise — and the planet is done;
   - a Genesis Device with 75 terraforming steps, at a million people
     with none queued, unless germanium is the first mineral under
     2,000 kT, by the concentrations less 12 summed: under 15 always,
     under 30 two rolls in three, under 60 four in five;
   - Cruisers (newest of 2–4), or one roll in three the newest
     Battleship: from turn 21, under 130 warship fleets and (under
     fifty, or over 2,000 resources), on a planet not rich only one roll
     in three; ten on a rich planet (and the planet is done), two
     otherwise;
   - Destroyers (newest of 12, 13), up to twenty paid for in full, under
     eighty fleets (sixty from turn 120) and 2,000 ships.
9. **The first fleet pass**: a leg to a space object over 200 light years
   off is blown away (the routine does this to every fleet in the game;
   here to our own). Mine layers from turn 41: a buddy
   (`FFindBuddyAndJoinUp(0, 0, 72, 108)`) over 55 layer fleets (over 40,
   two rolls in three), a wander one in five, else Lay Mines; under way
   a current task is cleared. Otherwise, with no miners (or under way):
   attack fleets (`FIsAiAttack`) are listed and a warship claims its
   planet; an empty transport bound for a planet not ours has its orders
   blown away. Miners with no leg: Remote Mining where they are; a buddy
   (`14, 15, 72, 108`) over 58 miner fleets (over 48, two in three); at
   an own planet whose concentrations sum under 30 (under 60 one in
   three), `FRetargetMiner` (`10a0:3e7e`: the own planet within 80 light
   years worth the most — ironium × 8, boranium × 10, germanium × 7 —
   when over six fifths of the one they sit at, at warp 6); else one
   roll in ten. The tail: before turn 11 the starting scouts and slot-2
   ships are scrapped; a colony ship with no leg is scrapped at a planet
   once slot 1 is the colony design while slot 7 still is one; at skill
   2 or more at an unowned planet it settles there with colonists, or
   goes home without, and at a small foreign planet waits; else it
   takes `pop/10` colonists (at most 25) from the own planet it sits at
   and goes to the nearest colonisable planet, or is scrapped at a
   planet when there is none; the old slot-7 colony ships under way are
   sent home when due.
10. **The haulers**: every transport with no leg, on battle plan 4, by
    `IdTargetMacFreighter` (`10a0:39d9`): at an own planet over a quarter
    full and over 99,900 people it weighs `pop/20` colonists against
    every own planet within 200 light years no hauler is bound for — the
    resources gained there by the colonists that arrive (3 % lost within
    50 light years, 6 within 100, 9 within 150, 12 within 200) against
    the resources lost here — and goes when the gain beats the loss by 5
    (10 from turn 81, 15 from turn 161); failing that a fifth of a
    mineral over 2,499 kT here goes to the own planet within 200 light
    years with the least of it, under 200 kT; failing that the fullest
    own planet within 200 light years (capacity less 2, 4, 6 past 50,
    100, 150 light years) with a score over zero. The leg is at warp 4,
    unloading everything.
11. **The last pass**: old ships scrapped at an own planet (one in five
    without a starbase) or sent home; a fleet with warships (2–9, slot 7
    excepted while it is a colony slot): with 101 warship fleets or
    more (91, one roll in three) a buddy (`2, 9, 100, 200`) when
    `Random(100)` beats the ships aboard less ten, else one roll in
    twenty; else `TargetMacArmada` (`10a0:4146`, `TargetCyberArmada`'s
    twin with `FPotentMacWarFleet` — ships in 2–4 plus twice 5–7, and
    twice the Battleships in 8–9 when short — and the bombers in 8–9);
    an attack fleet not chasing a fleet joins a buddy (`12, 13, 36, 72`)
    over 70 Destroyer fleets (50 from turn 121; 60/40 one roll in three),
    unless it holds twenty of the newest Destroyer and rolls nineteen in
    twenty; else `IdTargetAttack`.
12. `HandleBasicAiTasks`, `FillProductionQueue`.

#### The Macinti's design slots

`EnsureMacintiShdefs` (`10a0:2e9c`); the fittings are the part-class
bytes at `10a0:2a44` by the word offsets at `10a0:2a06`
(`macinti::fitting`).

| slot | role | hull | fitting | when |
|-----:|------|------|---------|------|
| 0 | mine layer | Frigate (5) | 10 | skill 2+, no ship of a non-Frigate design left, Bio > 3, Elec > 4, Con > 5, Prop > 5, Energy > 5 |
| 1 | colony ship | Colony Ship (15) | 20: `[8,40]` | retired and redrawn once the Enigma Pulsar can be built, no ship is left and its engine is not the Pulsar |
| 2 (early) | starting design | — | — | retired before turn 20 once no ship is left |
| 2–4 | cruisers | Cruiser (7) | 25–28, five tries | retired; each twenty years after the one before |
| 5–7 | warships | Nubian (29) two in three, else Battleship (9) | 29; 11–14 (slot 5), 15–18 (6), either (7), five tries | retired; each twenty years after the one before |
| 7 (early) | colony ship | Colony Ship (15) | 20 | before turn 40 while retired |
| 8, 9 | bombers | Battleship bomber, else B-52 (19) | 19; 8 / 9 | Weapons > 13; 9 fifteen years after 8 |
| 10, 11 | freighters | Large Freighter (2), slot 10 else Medium (1) | 24 | retired |
| 12, 13 | escorts | Nubian, else Destroyer (6) | 30; 0–3 / 4–7, five tries | Weapons > 4 & Prop > 5; Weapons > 9 & Prop > 8 |
| 14, 15 | miners | Ultra-Miner (24) at skill 2+ (15 past Con 14), else Maxi-Miner (23); 14 falls back to Miner (22) / Mini-Miner (21) | 22 / 21; 22 / 23 | retired |

`tests/macinti.rs` runs it in the Berserkers' seat — no Alternate
Reality race, so no colony ship can be drawn: the starting scouts and
miners scrapped, the recycling table kept, the designs on the hulls the
table names.

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
- `DoMacintiAiTurn` (`10a0:0008`), `EnsureMacintiShdefs` (`10a0:2e9c`),
  `EnsureMacintiStarbaseDesigns` (`1090:7688`), `FPotentMacWarFleet`
  (`10a0:42ec`), `TargetMacArmada` (`10a0:4146`), `IdTargetMacFreighter`
  (`10a0:39d9`), `FRetargetMiner` (`10a0:3e7e`), `PctPlanetCapacity`
  (`1048:6b2c`).
- `DoRototillAiTurn` (`1098:1e22`), `EnsureCAShdefs` (`1098:3020`),
  `FEnumCalcMinerDest` (`1088:5f32`).
- `DoCyberAiTurn` (`10a8:002a`), `EnsureCyberAiShdefs` (`10a8:4826`),
  `TargetCyberArmada` (`10a8:51a4`), `DoCyberFreighter` (`10a8:37b0`),
  `DoCyberPackets` (`10a8:1a78`), `iAddAttackFleet` (`10a8:4eba`),
  `FShouldPlanetBuildColonizer` (`1090:9f30`), `IWarpMAFromLppl`
  (`1090:5d5e`), `IdGetBestScannerDest` (`10a8:282a`),
  `MoveToNearestPlanetOrEnemy` (`1090:7040`).
- `DoAutomitronAiTurn` (`1098:01e0`), `EnsureISShdefs` (`1098:1938`),
  `FPotentISWarFleet` (`1098:012e`), `IdTargetScout` (`1090:61de`),
  `IdNearestUnknownPlanet` (`1090:15a4`), `XferAiSupply` (`1090:0ad0`).
- `DoRobotoidAiTurn` (`1088:0312`), `EnsureRobotoidShdefs` (`1088:20ae`),
  `FPotentRobWarFleet` (`1088:31bc`), `FShouldWeBuildColonizers`
  (`1090:476a`), `IdTargetAttack` (`1090:1ffe`), `FFindBuddyAndJoinUp`
  (`1090:9d18`), `IdRandomPlanetNearby` (`1090:5f54`), `FChangeAiShdef`
  (`1090:08a2`).
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
