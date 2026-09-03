# The computer players

Status: **identification verified; behaviour mapped but not implemented.**

Stars! ships seven computer opponents. This document records which player a
given save file hands to which opponent — that part is implemented in
`stars-core::ai` and tested against the fixtures — and maps the roughly 95
functions that make up their decision-making, which is **not** implemented.
The last section explains why, and what would unblock it.

## Identifying a computer player

The player record's flags byte (offset 7 of a type-6 block, see
`../formats/player.md`) is the high half of the player's 16-bit mode word.
`DoAiTurn` switches on `mode >> 13`, which is that byte's top three bits.

| Bits of the flags byte | Meaning                                    | Confidence |
|------------------------|--------------------------------------------|------------|
| 1                      | set for a computer player                  | confirmed  |
| 2–3                    | believed to be the difficulty setting      | **unconfirmed** |
| 5–7                    | which of the seven opponents               | confirmed  |

The fixtures hold `0x01` for the human player and `0x27` for both computer
players, which is bit 1 set and `0x27 >> 5 == 1` — Turindrone.

Bits 2–3 are `1` for every computer player available, so nothing in the data
distinguishes a difficulty field from any other two-bit value. `stars-core`
exposes them raw as `skill_bits` rather than naming them.

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
4. `InitRandomPlanetList` — shuffle the planet list, so the AI does not always
   consider planets in id order.

A player marked dead is skipped entirely.

## What a personality does

Taking `DoTurinDroneAiTurn` as the worked example — it is the personality both
computer players in `fixtures/incoming` use — its 42 named callees group into
six jobs:

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

Steps 5 and 6 are read from the decompilation rather than the disassembly, and
the decompiler has lost the arguments to the bitfield helper at `1118:0e32` at
every call site, so the exact comparisons in step 5 are not yet pinned down.

## Why this is not implemented

Every other subsystem in this project was recovered the same way: transcribe
from the binary, then check the transcription against real saved games — 438
planet-years for the economy, 47 battles for combat, 493 designs for the ship
model. Where a transcription could not be checked, as with the movement
scoring, that was reported rather than papered over.

The AI cannot currently be checked at all:

- The fixtures contain **two** computer players, both Turindrone at the same
  skill setting, across **one** turn transition (`fixtures/incoming/turn0` to
  `turn1`). Each owns a single planet.
- What those two players did on that turn is to queue five ships of design 0,
  plus one of design 18 for one of them. That exercises the ship-building path,
  not `FFillProdMinesAndFactories`, and a single sample cannot separate a
  correct transcription from a plausible one.
- The Exodus game, which supplies the 40-turn corpus everything else is checked
  against, has no computer players. The files in `fixtures/games/exodus/Races`
  have names like `OFFENDER` and `DEFENDER` that read like AI archetypes, but
  every one has a zero flags byte — they are human races. A test pins this so
  the mistake is not made later.

Writing 95 functions of AI from disassembly against that corpus would produce a
large body of code that looks right and cannot be shown to be right. That is
precisely the failure this project's method exists to prevent, and it has
already caught two such errors: the starbase-armour rule that fitted six of
seven samples but was not the rule the program implements, and the movement
scoring that looked 86% accurate until a chance-rate control showed the best
candidate set was 78% of the field.

### What would unblock it

A game with computer players, run for enough turns to produce a corpus
comparable to Exodus. Concretely, the useful artefact is a `.hst` per turn (or a
`.hst` plus the AI players' `.mN` files) from a game with:

- several computer players, ideally covering more than one of the seven
  personalities and more than one difficulty setting, which would also settle
  what bits 2–3 of the flags byte mean;
- enough turns for the AI to move past its opening — colonisation, ship
  building and the first attacks are the interesting decisions, and none of
  them appear in a single turn;
- at least one AI that comes under attack, so `MarkPlanetsUnderAttack` and the
  war functions are exercised.

With that in hand the AI becomes tractable in the same way combat was: transcribe
`FFillProdMinesAndFactories` first, since production is the most constrained
decision and the easiest to score, then colonisation, then war.

## Source

- `DoAiTurn` (`1088:0000`) — dispatch table and turn preparation.
- `DoTurinDroneAiTurn` (`1088:3670`) — the worked example above.
- `FillProductionQueue` (`10a8:2ce2`), `FFillProdMinesAndFactories`
  (`10a8:2d72`), `AddItemToQueue` (`1090:3e50`).
- The reconstructed C in `sirgwain/stars-decompile` is almost entirely stubs
  here — `ai.c` 10 of 12, `ai2.c` 5 of 6, `ai3.c` 6 of 6, `ai4.c` 16 of 17 and
  `aiutil.c` 58 of 61 functions are empty — so it is of no help for this
  subsystem.
