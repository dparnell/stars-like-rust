# Format: player messages (`rtPlrMsg`, block 40)

- **Status:** decoded from the binary; no fixture carries one, so the codec
  is checked by round trip and by the engine's own delivery test rather
  than against a real file
- **Ghidra routine(s):** `FFinishPlrMsgEntry` (`1030:9bd6`),
  `WritePlayerMessages` (`1030:97c0`), `ReadPlayerMessages` (`1030:9a5a`),
  `MarkPlayersThatSentMsgs` (`1030:95f6`), `FLoadLogFile`'s `rtPlrMsg` arm
  (`1048:cbb8`)
- **Implemented in:** `crates/stars-formats/src/player_message.rs`,
  `stars_core::GameState::player_messages`, `TurnOrders::messages`,
  `App::outgoing` and the writing mode in `crates/stars-ui/src/app.rs`

A letter one player writes to another, or to everybody. The client keeps
its outgoing letters in a list (`vlpmsgplrOut`, `MSGPLR`s), writes them into
its `.x` file after the orders, and the host gathers every player's into its
own list, then writes each into the turn files of those it is for.

## The record

`WritePlayerMessages` writes each `MSGPLR` verbatim — `WriteRt(rtPlrMsg,
cb + 12, lpmsgplr)` — and `ReadPlayerMessages` reads it straight back into a
fresh one, overwriting the first four bytes with the list link:

| offset | size | field |
|--------|------|-------|
| 0 | 4 | `lpmsgplrNext`, the in-memory link — junk on disk, written as zero here |
| 4 | 2 | `iPlrFrom`, the sender |
| 6 | 2 | `iPlrTo`: `0` everybody, else the recipient plus one |
| 8 | 2 | `iInRe`: the index in the sender's message list of the message replied to (`iMsgCur` when the letter was begun) |
| 10 | 2 | `cLen`: the text's length, **negative** when the text is stored as typed |
| 12 | … | the text |

`FFinishPlrMsgEntry` packs the text with `FCompressUserString` — the nibble
code of `strings.md` — giving it the text's own length as the room it has,
so the packed form is kept only when it is no longer than the text; else
the text is stored raw with `cLen = -1 - length`. The box holds 1,000
characters (`GetWindowText(hwndMsgEdit, lpb2k, 1000)`).

## Who gets what

`WritePlayerMessages(iPlayer)` writes, after the year's `rtMsg` blocks, every
letter with `iPlrTo == 0` whose sender is not `iPlayer`, and every letter
with `iPlrTo - 1 == iPlayer` — `PlayerMessage::is_for`. The sender never
gets a copy. `MarkPlayersThatSentMsgs` sets bit 8 of each sender's `det`
word and its low three bits to 3 in the recipient's file, which this
project does not write (nothing reads them).

`ReadPlayerMessages` strings the delivered letters onto `vlpmsgplrIn` after
the year's messages, which is why the pane counts them in
(`cMsg + vcmsgplrIn`) — see `../ui/message-pane.md`.

In this project the host side is `TurnOrders::messages`: the replay hands
each `rtPlrMsg` record on with the file's owner stamped as its sender, the
turn puts the lot into `GameState::player_messages` at its start (last
year's are cleared with the messages), and `save::player_file` writes each
player's share. `tests/player_messages.rs` covers the round.
