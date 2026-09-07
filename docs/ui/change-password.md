# The password dialogs

Status: **built**, for a player — both the dialog that sets a password and the
prompt that asks for one. The host branch is recovered and documented here but
has nothing to attach to: this project has no host mode.

Two dialogs, and they are not the same one. `IDD_NEW_PASSWORD` (141) sets the
password; `IDD_PASSWORD` (140) asks for it.

## Change Password

Commands (Change Password...), the **last item** of that menu (id `0x10e`, no
accelerator). `NewPasswordDlg`, `IDD_NEW_PASSWORD` (141).

### The template

199x70 dialog units, eight controls:

| id      | control  | what it is                                  |
|---------|----------|---------------------------------------------|
| `0x10c` | edit     | `New Password:`, `ES_PASSWORD`              |
| `0x10d` | edit     | `Retype Password:`, `ES_PASSWORD`           |
| `1`     | button   | `OK`                                        |
| `2`     | button   | `Cancel`                                    |
| `0x76`  | button   | `&Help`                                     |
| `0x7e2` | static   | the note, filled in at run time             |

**There is no box for the old password**, and that is not an oversight to fix:
the game stores a salt of the password rather than the password (see
`docs/formats/orders-x.md` and `stars_formats::password`), and anyone who can
open the file can change it. The dialog only asks for the new one, twice.

`WM_INITDIALOG` sends both edits `EM_LIMITTEXT` with **16**, so sixteen
characters is what can be typed — one short of the seventeen the buffer behind
them would take (`GetWindowText(..., 0x12)`). Both constants are in
`stars_formats::password`.

### What OK does

The two boxes are folded to salts and **the salts are compared**, not the text:

```c
lSalt  = LSaltFromSz(new);
lSalt2 = LSaltFromSz(retype);
if (lSalt2 != lSalt) { alert; focus and select the first box; stay open; }
```

Two strings that fold together therefore count as the same password, which is
of no consequence — a salt is all the game ever compares — but it is what the
code does and what this reproduces. On a mismatch the original puts up a
message box (string `0x00eb`) and selects the first box; this shows the reason
in the dialog and clears both boxes.

When they match:

* **a player** logs the salt as an `rtChgPassword` record — `WriteMemRt(0x24, 4,
  &lSalt)`, order type 36 with a four-byte payload;
* **the host** (`idPlayer == -1`) sets the salt and writes the host file there
  and then, rolling back if the write fails.

That difference is what the note at the bottom of the dialog says: string
`0x035c` for a player, because the new password travels in the turn they submit
and only binds from the next one, and string `0x035d` for the host, because a
host password is effective immediately. In host mode the **caption** also
changes, to string `0x035e`, `Change Host Password`.

An empty password is how a password is removed: `LSaltFromSz("")` is `0`, and
`0` means none. The dialog here adds a `Clear` button that does exactly that,
which is a shortcut for leaving both boxes empty and nothing more.

## The prompt

`PasswordDlg`, `IDD_PASSWORD` (140), put up by `FCheckPassword` (`1040:58d8`).
145x60 dialog units and five controls: a static filled at run time (string
`0x035f`, *Enter the password:*), one `ES_PASSWORD` edit (`0x10c`), OK, Cancel
and Help. The edit is limited to **fifteen** characters — one fewer than the
Change Password dialog allows, which is the original's own inconsistency and is
reproduced rather than tidied.

### When it appears

`FCheckPassword` asks for nothing at all if any of these holds:

* the file carries no password (`lSaltCur == 0`);
* it is the password already given this session (`lSaltLast == lSaltCur`),
  which is why the same password is asked for once rather than once per file;
* the player is a computer player (`fAi`);
* `stars.ini`'s `[Misc] DefaultPassword` (`vszDefPass`) folds to the same salt.
  A password kept there is stored in plain text, which is worth knowing before
  keeping one.

Otherwise it prompts — except in validate mode, where it fails instead of
asking, because there is nobody there to answer.

Which salt is being asked for comes from the file being loaded: a player's file
names its player in the header and that player's salt is the one
(`lSaltCur = rgplr[iPlayer].lSalt`), while a host file names none and the
original takes the host's from a leading `rtChgPassword` record that this
project neither writes nor reads. So a `.hst` asks for nothing here.

### Getting it wrong

The typed text is folded and compared against the salt. A wrong answer is
counted in `vcPasswordFailures` — a session-long counter that is never reset —
and costs a **wait** before the next attempt, from a three-step ladder:

| failures so far | wait |
|-----------------|------|
| under 10        | 1 second |
| under 100       | 5 seconds |
| 100 or more     | 10 seconds |

The original calls `Delay` with those three constants, which freezes the
program; this counts the same wait down and refuses OK until it has passed.
That ladder is the only thing standing between a stored salt and a dictionary,
so it is worth having even though a salt is not a password hash.

A right answer is remembered (`lSaltLast = lSaltCur`), and the loader carries
on. **Cancelling fails the load** — the original goes `goto LError` — so the
file is dropped and whatever was already open stays open. That is what this
does: a guarded save is read but held back, and only installed once the prompt
is answered.

### Who gets asked

Only a frontend with somebody at the keyboard: `App::prompt_for_password` is
what turns the prompt on, and the desktop sets it. Everything else — a test, a
tool reading a save — leaves it clear and opens the file straight through.

The original draws the same line in the same place, with `ini.fValidate`: in
batch mode `FCheckPassword` refuses rather than asking, because there is nobody
to answer. The difference is that this reads the file where the original would
refuse it, and that is the honest choice: the salt gates the interface and has
never encrypted anything, so a tool re-encoding a save is not getting past a
protection. Every real save in `fixtures/` carries a salt, and the differential
test that re-encodes them all is exactly this case.

## What is not

* The **host password**, and so the second note and the alternative caption.
  There is a host mode now (`host-mode.md`), but its `Password...` button is
  disabled: the salt has nowhere to go until the `.hst`'s leading
  `rtChgPassword` record is written and read.
* **Validate mode**, which fails rather than asking.
* The **Help** buttons.
