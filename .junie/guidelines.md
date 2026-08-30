# Project Guidelines

## Version control

- **Commit often.** Commit work to git whenever a task or a meaningful chunk of
  work is completed — do not batch many unrelated changes into one large commit.
- **Commit work in progress too.** It is preferred to commit even when things are
  still unfinished rather than leaving work uncommitted.
- **Mark in-progress commits.** Prefix any commit that captures unfinished work
  with `WIP:` (e.g. `WIP: reverse-engineer .xy header layout`). Completed,
  self-contained work uses a normal (non-`WIP:`) message.
- **Do not version Ghidra working files.** The live Ghidra project under
  `ghidra/` (`*.gpr`, `*.rep/`, `*.lock`) is a volatile, regenerable database and
  is git-ignored; only the original assets under `binary/` are versioned.
