# The Battle Plans dialog

Status: **built** — every control the template holds, and the behaviour behind
each one.

Commands (Battle Plans...), `IDM` `0x7dc`, **F6**. `BattlePlansDlg`,
`IDD_BATTLE_PLANS` (2013). A player carries a short list of battle plans and a
fleet points at one of them by index; this dialog edits one plan at a time.

## The template

239x115 dialog units, sixteen controls:

| id      | control  | what it is                                    |
|---------|----------|-----------------------------------------------|
| `0x41e` | combo    | `&Plan:` — the player's plans by name          |
| `0x41b` | button   | `&Rename...`                                   |
| `0x41f` | combo    | `P&rimary Target:`                             |
| `0x420` | combo    | `S&econdary Target:`                           |
| `0x421` | combo    | `&Tactic:`                                     |
| `0x422` | combo    | `Attack &Who:`                                 |
| `0x41d` | checkbox | `D&ump Cargo`                                  |
| `0x817` | button   | `&Delete`                                      |
| `0x41c` | button   | `&Copy`                                        |
| `1`     | button   | `Close`                                        |
| `0x76`  | button   | `&Help`                                        |

The combos are filled at run time, and **the index of an item is the value that
is stored** — `WM_INITDIALOG` sets each combo's selection straight from the
plan's field and the `WM_COMMAND` arms write `CB_GETCURSEL` straight back. That
is what fixes the enumerations below.

## The enumerations

`docs/formats/battleplan.md` had the ranges but not the meanings. The dialog
supplies them.

**Tactic** (strings `0x198`–`0x19d`), the low nibble of byte 1:

| value | name                    |
|------:|-------------------------|
| 0     | Disengage               |
| 1     | Disengage if challenged |
| 2     | Minimize damage to self |
| 3     | Maximize net damage     |
| 4     | Maximize damage ratio   |
| 5     | Maximize damage         |

**Primary / secondary target** (strings `0x190`–`0x197`), the two nibbles of
byte 2:

| value | name               | value | name             |
|------:|--------------------|------:|------------------|
| 0     | None/Disengage     | 4     | Bombers/Freighters |
| 1     | Any                | 5     | Unarmed Ships    |
| 2     | Starbase           | 6     | Fuel Transports  |
| 3     | Armed Ships        | 7     | Freighters       |

**Attack who** (strings `0x78`–`0x7b`), byte 3: `Nobody`, `Enemies`,
`Neutrals & Enemies`, `Everyone`, and then every **other** player by name,
stored as `4 + player`. The combo skips the local player, so the original
converts between the two indexes as it reads and writes:
`if (idPlayer + 4 <= i) i--` on the way in and `i++` on the way out. In a
single-player game the combo holds `Everyone` alone and is disabled.

**Dump Cargo** is bit 7 of byte 1 — `BTLPLAN`'s `fDumpCargo`, the bit above
`fDelete`. `MANUAL.PDF` p. 15-14: "Dump Cargo — Jettison cargo at the start of
battle."

### The tactic order was wrong in this project

`stars_core::battle::Tactic` had *Maximise damage* at 3, *net damage* at 4 and
*damage ratio* at 5 — the last three shuffled. Values 3 and 5 therefore scored
their targets by the wrong rule, since `target_score` transcribes
`ScoreFromGiveAndTakeAndTactic`, which groups the six by value: `{0, 2}` score
by damage taken, `{1, 5}` by damage given, `{3, 4}` by the ratio.

Three sources agree on the corrected order: the `BattleTactic` enum in the NB09
symbols, this dialog's combo, and `MANUAL.PDF` p. 15-14, which lists the six in
the same sequence. So does the shape of the stock plans, which only read
sensibly this way — *Sniper* is `Disengage if challenged` against `Unarmed
Ships`, and *Chicken* is `Disengage` against nothing.

The battle-replay differential is unchanged by the fix (22 of 31 first beam
hits, 455 of 478 movement squares): the recorded fleets use tactics whose
grouping did not move.

## Behaviour

* **The first plan cannot be renamed or deleted.** The original disables both
  buttons whenever the selection is plan 0, which is the plan every fleet falls
  back on.
* **Copy** duplicates the plan on show and bumps its name: a trailing `" (n)"`
  is incremented (`'9'` wraps to `'0'`), and `" (2)"` is appended when there is
  none — neither to a name of 28 characters or more. It then drops straight
  into the rename box, which is what the original does by falling through into
  `IDD_RENAME`. It refuses at **fifteen** plans, though the host will accept a
  sixteenth arriving in a log record.
* **Delete** warns first when fleets are using the plan (string `0x035b`),
  because deleting one moves every fleet at or past it down a slot — see
  `docs/formats/battleplan.md`.
* The dialog **opens on the selected fleet's plan** when a fleet is selected
  (`sel.fl.iplan`), and on the first plan otherwise.
* Every change is logged. The original collects them and logs when it leaves a
  plan (`LogChangeBtlplan`); this logs per change and collapses a run of edits
  to the same plan into the last of them, which the host applies identically.

## What is not

* The **Help** button.
* The rename box is drawn **in the dialog** rather than as the separate modal
  `IDD_RENAME` the original puts up.
* The plan list is not shown while a plan is being renamed, since there is only
  one window.
