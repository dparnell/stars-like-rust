# Stars! file I/O — binary map (`STARS!.EXE`)

Where the container / block / cipher logic lives in the original binary, recovered
from the Ghidra project (`ghidra/Stars`, `STARS!.EXE`). This grounds every
`stars-formats` claim in the **actual shipped code** rather than third-party
reconstructions. Addresses are `seg:off`; the file's data segment (DGROUP) is NE
segment 36 (selector `0x1118`), so its globals show up as `DAT_1118_*`.

## How the anchors were found

The Win16 file APIs are imported from `KERNEL` by ordinal. Parsing the NE
per-segment relocation records for `KERNEL.82 _lread`, `KERNEL.86 _lwrite`,
`KERNEL.74 OpenFile`, `KERNEL.81 _lclose` gives the exact call-site offsets:

- `_lread`  → `1038:ae34`, `1068:2d98`
- `_lwrite` → `1038:8d65`, `1068:54d3`
- `OpenFile`→ `1038:8d17`, `1068:2ca6`
- `_lclose` → `1038:ade1`, `1068:2d46`

Two file-I/O clusters exist: segment **`0x1068`** is the core block/container
module (equivalent to the decompiled `file.c`/`save.c`); segment **`0x1038`**
holds the cipher/PRNG plus a bulk copy helper.

## Recovered functions

| function | address | role | our code |
|----------|---------|------|----------|
| `FUN_1068_2d52` | `1068:2d52` | `ReadBytes(buf, len)` — copy `len` bytes from the in-memory image (`DAT_1118_08ac`, when a file was slurped whole) else `_lread` | `StarsFile::decode` input cursor |
| `FUN_1068_54be` | `1068:54be` | `WriteBytes(buf, len)` — `_lwrite` with error handling | `StarsFile::encode` output |
| `FUN_1068_2bd2` | `1068:2bd2` | **`ReadBlock`** — read 2-byte header word `DAT_1118_279a`, read `size = word & 0x3ff` payload into `DAT_1118_4c3a`, then dispatch (see below) | `block::split_blocks` + `file::decode` |
| `FUN_1068_5422` | `1068:5422` | **`WriteBlock(type, size)`** — emit header word then payload; seed/encrypt as below | `block::join_blocks` + `file::encode` |
| `FUN_1038_89e4` | `1038:89e4` | **seed the cipher PRNG** from header fields (`rtBOF`) | `FileHeader::init_rng` |
| `FUN_1038_8b20` | `1038:8b20` | **XOR stream cipher** over a payload (decrypt == encrypt) | `StarsRng::apply` |
| `FUN_1038_8a58` | `1038:8a58` | the **PRNG step** (L'Ecuyer combined LCG) | `StarsRng::next_u32` — see `../rng/prng.md` |

## Block framing — confirmed

`ReadBlock` (`FUN_1068_2bd2`):

```c
ReadBytes(&hdr, 2);                       // hdr = DAT_1118_279a (little-endian u16)
size = hdr & 0x3ff;                       // low 10 bits
if (size) ReadBytes(&payload, size);      // payload = DAT_1118_4c3a
if (((hdr >> 8) & 0xfc) == 0x20)          // type == 8 (rtBOF, header)?
    SeedCipher(...header fields...);      //   -> FUN_1038_89e4
else if (hdr & 0xfc00)                    // type != 0 (not rtEOF)?
    Decrypt(&payload, size);              //   -> FUN_1038_8b20
```

`WriteBlock` (`FUN_1068_5422`):

```c
if (type == 8)      SeedCipher(...);      // seed when writing the header
else if (type != 0) Encrypt(&payload, size);
hdr = (type << 10) | (size & 0x3ff);      // pack type:6 | size:10
WriteBytes(&hdr, 2);
WriteBytes(&payload, size);
```

This confirms, from ground truth, everything `stars-formats::block` and
`stars-formats::file` implement:

- **Header word** = `type << 10 | (size & 0x3ff)` (type = high 6 bits, size =
  low 10 bits). Max payload 1023 bytes.
- **Type 8 (`rtBOF`)** is the plaintext header and *seeds* the keystream.
- **Type 0 (`rtEOF`)** is plaintext and terminates the stream (`hdr & 0xfc00 == 0`).
- **Every other block** has its payload XOR-encrypted with the continuous
  keystream.

## Header field extraction (feeds seeding)

`ReadBlock`/`WriteBlock` seed the cipher via:

```
SeedCipher(lidGameLo   = DAT_1118_4c3e,          // game id low word (payload +0x04)
           salt        = DAT_1118_4c46 >> 5,     // payload +0x0c, high 11 bits
           turnWord    = DAT_1118_4c44,          // payload +0x0a
           iPlayer     = DAT_1118_4c46 & 0x1f,   // payload +0x0c, low 5 bits
           crippled    = (DAT_1118_4c49 & 0x10) >> 4)  // payload +0x0f bit 4
```

`DAT_1118_4c3a` is the payload buffer base; the offsets above are relative to it
and line up with `stars-formats::header::FileHeader` (which is verified
byte-for-byte on every fixture). See `../rng/prng.md` for the seeding math.

## Not statically recoverable

- The **seed/prime table** used by `SeedCipher` sits at `DS:0x7fe0`, which is
  past NE segment 36's file image (`len 0x6504`) — i.e. it is BSS built at
  runtime, so it cannot be dumped from the file. `crypt::PRIMES` reproduces its
  effect (byte-verified).

**Source:** `ghidra/Stars` / `STARS!.EXE`, decompiled via the Ghidra MCP bridge.
