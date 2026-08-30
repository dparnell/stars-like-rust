# Production-queue blocks (types 28 & 29)

Status: **decoded & verified** — implemented in `stars-formats::production`.

A production queue is a planet's build list. Type 28 (`PRODUCTION_QUEUE`) is the
form stored in `.hst`/`.mN`; type 29 (`PRODUCTION_QUEUE_CHANGE`) is the `.xN`
order form, which prefixes a 2-byte planet id.

## Layout

Type 28 — a bare list of packed 32-bit items:

| Bits    | Field       | Notes                                              |
|---------|-------------|----------------------------------------------------|
| 0–9     | count       | quantity to build (10 bits)                        |
| 10–19   | item        | item id (10 bits); `>= 256` = auto-build items     |
| 20–31   | completion  | resources/minerals applied to the first unit (12b) |

The block holds `len / 4` items; a partial trailing word (if any) is ignored.

Type 29 (`.xN`) — identical but with a leading `u16` planet id, then the item
list.

## Evidence

`fixtures/incoming/turn1/Game.hst` has two type-28 queues. They decode to runs
of auto-build items (item ids in the 256+ range, e.g. 256 and 274), with only
the head item carrying a non-zero completion — matching how Stars! applies
resources to the front of the queue.

## Source

- stars-4x `decompiled`: `Structures/Structure28.xml` and `Structure29.xml`.

## Open questions

- The full item-id table (design slots < 256; the auto-build pseudo-item ids
  such as 256 = auto factories, 274 = …). To be captured in `stars-core`
  alongside the hull/component database rather than in the on-disk format.
- Whether `completion` counts resources or a percentage; exposed raw for now.
