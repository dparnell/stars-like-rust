# The Change Password dialog

Status: **built**, for a player. The host branch is recovered and documented
here but has nothing to attach to: this project has no host mode.

Commands (Change Password...), the **last item** of that menu (id `0x10e`, no
accelerator). `NewPasswordDlg`, `IDD_NEW_PASSWORD` (141).

## The template

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

## What OK does

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

## What is not

* **Host mode**, and so the second note and the alternative caption.
* The **prompt** that asks for a password — `IDD_PASSWORD` (140), a single box
  with the question in a run-time static (string `0x035f`), checked by
  `FCheckPassword` against the stored salt (string `0x00ea` when it is wrong).
  Nothing in this project asks for a password before opening a turn.
* The **Help** button.
