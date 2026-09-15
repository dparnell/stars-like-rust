# The Research dialog

Status: **layout, wording and numbers recovered and reimplemented**; what is
not reproduced is named at the end.

`ResearchDlg` (`10d8:0000`), reached with **F5**, with `DrawResearchDlg`
(`10d8:090a`) painting it and `FTrackResearchDlg` (`10d8:1a8c`) handling the
drag on its spin buttons. `MANUAL.PDF` chapter 8 describes the same screen.

Two columns: the six fields of study, and what is being researched now.

## The controls

| id | control |
|----|---------|
| `0x431`…`0x436` | the six field radio buttons — `iResTechNow = wParam - 0x431` |
| `0x43b` | the **Next field to research** dropdown |
| `2`, `0x76` | **Done**, Help — the dialog template at `0x3480ee` in the executable captions them `Done` and `&Help` |

There is no Cancel, and no OK: the dialog edits three globals — `pctResGlob`,
`iResTechNow` and the dropdown — and control `2`, **Done**, is the one that
**commits** them. Closing the dialog any other way is the same button, so
whatever was set is kept; the tutorial's first year ("select Weapons and
press Done") depends on the button being called that. It writes `PLAYER.pctResearch` and both nibbles of `PLAYER.iTechCur`, and
logs a single two-byte `rtLogResearch` order (type 34) carrying
`iTechCur << 8 | pctResearch` — but only if something actually changed.

## Technology Status

The six fields with the level held in each, headed `Field Of Study` and
`Level`, with the radio buttons in the first column. Levels run 0 to 26.

## Expected Research Benefits

What the coming levels will bring. `DrawResearchDlg` walks every component of
every category, asks `FLookupPart` how far off it is, and lists those between
one and nine levels away, nearest first — the next level in green, the three
after it in red, everything further off in black, and everything ten or more
levels away gathered into the last group.

**It is not a preview of the selected field**, although it looks as though it
should be. The original sets the player's current field to whichever radio
button is chosen before it asks, and puts it back afterwards — and that makes
no difference to the answer. `TechStatus` only consults the current field to
choose between its `LookupNear` (2) and the general `(need - have) + 1`, and
for a component one level short those are the same number; anything short in
more than one field is reported as unreachable whatever is being studied. So
the list is the same whichever button is selected. A test pins that, because it
is exactly the sort of thing that would otherwise be "fixed" later.

A component a racial trait forbids never appears, however close its technology
is.

## Currently Researching

Three lines and a dropdown:

* `%s, Tech Level %d` — the field and the level being worked on, which is one
  above what is held;
* `Resources needed to complete:` — that level's cost less what the field has
  banked, or **`Maxed Out`** at level 26;
* `Estimated time to completion:` — `ceil(remaining / budget)`, as `%ld year%c`
  so that one year is singular; **`Never`** when nothing is budgeted. A race
  with **Generalized Research** only sends half its budget to the field in
  hand, so that field takes twice as long;
* `Next field to research:` — eight choices: `<Same field>`, the six fields,
  and `<Lowest field>`. They are stored in the high nibble of `iTechCur` as
  6, 0…5 and 7 respectively, which is why the dialog maps the dropdown index
  through `index == 0 ? 6 : index - 1`.

## Resource Allocation

* `Annual resources from all planets:` — the sum of `CResourcesAtPlanet` over
  the player's planets.
* `Total resources spent on research last year:` — `PLAYER.lResLastYear`.
* `Resources budgeted for research:` — the percentage, with a pair of spin
  buttons beside it (`rcSpinTop`, `rcSpinBot`).
* `Next year's projected research budget:` — `ProjectedResearchSpending`.

That last one is the interesting figure, because it is **not** simply the
percentage of the empire's output:

* a planet with an **empty queue** gives research *everything* it makes — the
  manual's "you receive all resources from planets with nothing in the
  production queue" (p. 8-4);
* a planet with a queue gives the skim **plus whatever the queue fails to
  spend**, which is why a blocked queue quietly funds research.

The second is `EstimateItemProdSched` called with a negative item, which runs
one year of the queue and reports the leftovers — so the projection shares the
whole of the production simulation, stopping rules and alchemy included. See
`production.md`.

Under the box, a race with **Generalized Research** or **Bleeding Edge
Technology** gets a line saying so, since both change how research behaves.

## What is reproduced

Both columns and all their wording, the six radio buttons and the eight-choice
dropdown with its nibble encoding, the remaining cost and the `Maxed Out` and
`Never` cases, the year estimate with Generalized Research's halving, the four
allocation figures including the projection with its two rules, the benefits
list ordered and coloured by distance with the component pop-up a press on a
line raises (`technology-browser.md`, *The hover help is this panel*), and
the trait notes. Nothing is committed until Done, and Done writes nothing
when nothing changed. The Help button is drawn but dead, there being no help
reader yet.

## What is not

* **The Technology Browser** (F2), which is `BrowserDlg` (`research.c`) and a
  screen of its own rather than part of this dialog.
* The two **spin buttons** are a slider here, which is the same control with
  its whole range on show.
* The dialog's exact pixel geometry, which the original computes from the
  widest field name and the widest label.
