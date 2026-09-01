/*
 * stars-types.h — consolidated, self-contained C header of the Stars! 2.7j
 * game types (enums + structs) recovered from the game's own CodeView NB09
 * debug symbols via sirgwain's `stars-asm` project.
 *
 * This file is GENERATED. It is the concatenation of:
 *   1. this hand-written prelude (fixed-width + Win16 shim typedefs), then
 *   2. tmp/stars-asm/decompiled/enums.h   (the 74 game enums), then
 *   3. tmp/stars-asm/decompiled/structs.h (the 110 game structs),
 * with the `#include`/include-guard lines of structs.h stripped so the whole
 * thing parses cleanly on its own.
 *
 * It is meant to be fed to Ghidra's C parser (see apply_types.py / the
 * "Parse C Source" GUI action) to populate the data-type manager of the open
 * `stars.2.7j.exe` program. See docs/ghidra/README.md for the workflow and for
 * how to regenerate this file.
 *
 * Sizing note: the target program is x86:LE:16, where Ghidra's data
 * organisation uses char=1, short=2, int=2, long=4. The fixed-width shims below
 * therefore map int32_t/uint32_t onto `long`/`unsigned long` (4 bytes), not
 * `int`, and the Win16 object handles onto a 2-byte word.
 */

/* ---- fixed-width integer shims (stdint.h is not available to the parser) --- */
typedef signed char        int8_t;
typedef unsigned char      uint8_t;
typedef short              int16_t;
typedef unsigned short     uint16_t;
typedef long               int32_t;
typedef unsigned long      uint32_t;

/* ---- minimal Win16 shims used by the game structs/globals ------------------ */
/* Win16 GDI/USER object handles are 16-bit words. */
typedef uint16_t HANDLE;
typedef uint16_t HWND;
typedef uint16_t HDC;
typedef uint16_t HMENU;
typedef uint16_t HBRUSH;
typedef uint16_t HPEN;
typedef uint16_t HFONT;
typedef uint16_t HBITMAP;
typedef uint16_t HPALETTE;
typedef uint16_t HCURSOR;
typedef uint16_t HICON;
typedef uint16_t HRGN;
typedef uint16_t HINSTANCE;
typedef uint16_t HGLOBAL;
typedef uint16_t HRSRC;

typedef uint32_t COLORREF;

typedef struct tagPOINT {
    int16_t x;
    int16_t y;
} POINT;

typedef struct tagRECT {
    int16_t left;
    int16_t top;
    int16_t right;
    int16_t bottom;
} RECT;

/*
 * Win16 owner-draw control notification. The game's list/combo owner-draw
 * routines (HandleFocusState, DrawCBEntireItem, DrawDlgLBEntireItem) take a
 * `DRAWITEMSTRUCT *`. It is a standard Windows type (not part of the game's own
 * NB09 structs), and is only ever used here behind a pointer, so this minimal
 * Win16 layout (UINT=16-bit, DWORD=32-bit) is enough for the parameter to type.
 */
typedef struct tagDRAWITEMSTRUCT {
    uint16_t CtlType;
    uint16_t CtlID;
    uint16_t itemID;
    uint16_t itemAction;
    uint16_t itemState;
    HWND     hwndItem;
    HDC      hDC;
    RECT     rcItem;
    uint32_t itemData;
} DRAWITEMSTRUCT;

/*
 * The 16-bit Stars! structures are byte-packed (their in-memory layout matches
 * the on-disk record layout — e.g. THING is exactly 18 bytes). Without this the
 * C parser inserts natural-alignment padding and structs come out too large
 * (THING -> 24, SHDEF -> 216, ...). Ghidra's CParser honours #pragma pack even
 * when fed a raw string (no preprocessor run), so declaring it here makes every
 * generated game struct pack to its true size.
 */
#pragma pack(1)
