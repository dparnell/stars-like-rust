//! The turn password a player can put on their game.
//!
//! Stars! does not store a password. It stores a 32-bit value derived from the
//! typed string by `LSaltFromSz` (`1040:59ce`) — the game calls it a *salt* —
//! and compares that against the salt of whatever is typed next time
//! (`FCheckPassword`, `1040:58d8`). The value lives at offset 12 of the player
//! block, four bytes, and `0` means no password.
//!
//! It is a checksum, not a modern password hash: seventeen characters at most,
//! alternately added and multiplied into a 32-bit accumulator. This module
//! exists so that a file this project writes carries the value the original
//! would have written for the same password, and so that a `.xN` password
//! change replays. It offers no way to go the other way, and there is no reason
//! for one: the game only ever compares salts.

/// The `stars.ini` section a default password is kept in (`idsMisc`).
pub const DEFAULT_PASSWORD_INI_SECTION: &str = "Misc";

/// The `stars.ini` key that holds it (`idsDefaultpassword`, string `0x00ae`).
///
/// `InitStuff` reads it into a 16-byte buffer (`GetPrivateProfileString(...,
/// 0x10, ...)`), and `FCheckPassword` skips the prompt when what is there folds
/// to the salt being asked for. It is a convenience for a player who does not
/// want to type their own password back at themselves, and it is stored in
/// plain text, which is worth knowing before using one.
pub const DEFAULT_PASSWORD_INI_KEY: &str = "DefaultPassword";

/// Offset of the salt inside the fixed region of a full player block.
///
/// Named "password" in `docs/formats/race-r.md`, which recovered the field
/// independently; the host's replay of a type-36 record writes there too
/// (`PLAYER + 0x0c`, `1048:c692`).
pub const PASSWORD_OFFSET: usize = 12;

/// The longest password the game's dialog can read.
///
/// `NewPasswordDlg` reads the edit box into an 18-byte buffer
/// (`GetWindowText(..., 0x12)`, `1040:5dfc`), so seventeen characters and a
/// terminator. What can actually be typed is one less — see
/// [`PASSWORD_FIELD_LIMIT`].
pub const MAX_PASSWORD_LEN: usize = 17;

/// The longest password the Change Password dialog will let you **type**.
///
/// `NewPasswordDlg`'s `WM_INITDIALOG` sends both edit boxes `EM_LIMITTEXT` with
/// 16, so the seventeenth character the buffer would hold can never be entered.
pub const PASSWORD_FIELD_LIMIT: usize = 16;

/// The salt the game stores for `text`, from `LSaltFromSz` (`1040:59ce`).
///
/// The string's bytes are folded in pairs — the first added, the second
/// multiplied, and so on — with 32-bit wrapping throughout. The original works
/// on the typed string's **ANSI** bytes, so this agrees with it for an ASCII
/// password and only for one; a password with accented characters would fold
/// this project's UTF-8 bytes instead and would not match what Stars! stores.
/// Every password in the fixtures is a mystery either way — see the module
/// note — and ASCII is what the game's edit box invites.
#[must_use]
pub fn salt(text: &str) -> u32 {
    salt_bytes(text.as_bytes())
}

/// [`salt`] over raw bytes, which is what the original folds.
///
/// The original walks a C string, so a zero byte ends it. Two conventions come
/// out of the code and are kept: an empty password is `0`, and a non-empty one
/// that happens to fold to `0` is bumped to `1`, so `0` unambiguously means
/// *no password*.
#[must_use]
pub fn salt_bytes(bytes: &[u8]) -> u32 {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    let bytes = &bytes[..end];
    if bytes.is_empty() {
        return 0;
    }
    let mut salt: i32 = 0;
    for pair in bytes.chunks(2) {
        salt = salt.wrapping_add(i32::from(pair[0] as i8));
        if let Some(mul) = pair.get(1) {
            salt = salt.wrapping_mul(i32::from(*mul as i8));
        }
    }
    if salt == 0 {
        salt = 1;
    }
    salt as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_password_is_zero() {
        assert_eq!(salt(""), 0);
        // A C string ends at its first zero byte, so this is empty too.
        assert_eq!(salt_bytes(&[0, b'a', b'b']), 0);
    }

    #[test]
    fn folds_in_pairs() {
        // 'a' = 97, 'b' = 98: (0 + 97) * 98 = 9506.
        assert_eq!(salt("ab"), 9506);
        // A trailing odd character is added, not multiplied.
        assert_eq!(salt("abc"), 9506 + 99);
        // One character on its own is just itself.
        assert_eq!(salt("a"), 97);
    }

    #[test]
    fn characters_are_signed() {
        // 0xff is -1, not 255: the original sign-extends each `char`.
        // (0 + 1) * -1 = -1.
        assert_eq!(salt_bytes(&[1, 0xff]), (-1i32) as u32);
    }

    #[test]
    fn a_password_never_salts_to_nothing() {
        // (0 + 1) * -1 + 1 = 0, which would read as "no password"; the
        // original bumps it to 1 rather than let that happen.
        assert_eq!(salt_bytes(&[1, 0xff, 1]), 1);
    }

    #[test]
    fn wraps_rather_than_overflowing() {
        // Seventeen characters is the longest the dialog takes; the fold runs
        // past 32 bits several times over and must simply wrap.
        let long = "z".repeat(MAX_PASSWORD_LEN);
        assert_eq!(salt(&long), salt(&long));
        assert_ne!(salt(&long), 0);
    }
}
