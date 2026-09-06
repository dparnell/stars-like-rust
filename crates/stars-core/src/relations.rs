//! How a player regards the others.
//!
//! Each player keeps a byte per player — `PLAYER.rgmdRelation`, at offset
//! `0x70` — saying whether they are neutral, a friend or an enemy. The table
//! is the player's own: it says how *they* regard everyone else and nothing
//! about how anyone regards them, and only its owner can change it.
//!
//! Set in the Player Relations dialog (`RelationsDlg`, `10f0:0088`, F7) and
//! sent to the host as a `rtLogRelations` order carrying the whole table. See
//! `docs/ui/player-relations.md`.

use crate::GameState;

/// How one player regards another.
///
/// The values are the dialog's control ids less `0x7d4`, so they are fixed by
/// the resource: `IDC` 2004 neutral, 2005 friend, 2006 enemy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Relation {
    /// Neither friend nor enemy, which is where everybody starts.
    #[default]
    Neutral,
    /// A friend: remote terraforming helps their planets, and their minefields
    /// are drawn apart from everyone else's.
    Friend,
    /// An enemy.
    Enemy,
}

impl Relation {
    /// The three, **in the order the dialog stacks them** — which is not the
    /// order of their values. `Friend` sits at the top, `Neutral` under it and
    /// `Enemy` at the bottom (dialog 2008, controls at y = 0x14, 0x26, 0x38).
    pub const ALL: [Relation; 3] = [Relation::Friend, Relation::Neutral, Relation::Enemy];

    /// The byte the file stores.
    #[must_use]
    pub fn value(self) -> u8 {
        match self {
            Relation::Neutral => 0,
            Relation::Friend => 1,
            Relation::Enemy => 2,
        }
    }

    /// Read one back. Anything else is taken as neutral, which is what an
    /// absent table amounts to.
    #[must_use]
    pub fn from_value(value: u8) -> Relation {
        match value {
            1 => Relation::Friend,
            2 => Relation::Enemy,
            _ => Relation::Neutral,
        }
    }

    /// The radio button's caption, without its accelerator.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Relation::Neutral => "Neutral",
            Relation::Friend => "Friend",
            Relation::Enemy => "Enemy",
        }
    }
}

/// How `player` regards `toward`.
///
/// A player always regards themselves as neutral — the dialog does not offer
/// the row at all — and a player whose file carried no table regards everybody
/// as neutral.
#[must_use]
pub fn regard(state: &GameState, player: usize, toward: usize) -> Relation {
    if player == toward {
        return Relation::Neutral;
    }
    state
        .players
        .get(player)
        .and_then(|p| p.relations.get(toward))
        .copied()
        .map_or(Relation::Neutral, Relation::from_value)
}

/// Whether the game lets a player set relations at all.
///
/// `RelationsDlg` is refused outright in a **single-player** game
/// (`GAME.wCrap` bit 2): the menu item is there, and choosing it does nothing.
/// There is nobody to declare anything to, so the table stays as it is — which
/// still matters, because remote terraforming and the scanner's minefield
/// filters read it.
#[must_use]
pub fn can_be_set(state: &GameState) -> bool {
    !state.single_player
}

/// The players a given player may set a relation toward: everybody but
/// themselves, in player order.
///
/// The dialog's listbox is filled this way, which is why its selection has to
/// be mapped back — the original adds one to the index once it is past its own
/// player.
#[must_use]
pub fn others(state: &GameState, player: usize) -> Vec<usize> {
    (0..state.players.len())
        .filter(|other| *other != player)
        .collect()
}

/// Which of four groups another player falls into, from one player's point of
/// view.
///
/// This is the grouping the scanner colours minefields by — `MANUAL.PDF`
/// p. 5-14: *"Your minefields are blue, friends are yellow, and enemies and
/// neutrals are red"* — and the one its minefield menu offers a tick for each
/// of. The last two share a colour on the map but are separate here, exactly
/// as the menu separates them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Party {
    /// The player themselves.
    Yours = 0,
    /// Somebody they have marked a friend.
    Friends = 1,
    /// Neither friend nor enemy.
    Neutrals = 2,
    /// Somebody they have marked an enemy.
    Enemies = 3,
}

impl Party {
    /// All four, in the order the minefield menu lists them.
    pub const ALL: [Party; 4] = [
        Party::Yours,
        Party::Friends,
        Party::Neutrals,
        Party::Enemies,
    ];

    /// The name the menu gives it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Party::Yours => "Yours",
            Party::Friends => "Friends",
            Party::Neutrals => "Neutrals",
            Party::Enemies => "Enemies",
        }
    }

    /// Its number, which is the bit the minefield filter uses.
    #[must_use]
    pub fn index(self) -> u8 {
        self as u8
    }
}

/// Which group `owner` falls into, seen by `player`.
#[must_use]
pub fn party(state: &GameState, player: usize, owner: usize) -> Party {
    if player == owner {
        return Party::Yours;
    }
    match regard(state, player, owner) {
        Relation::Friend => Party::Friends,
        Relation::Neutral => Party::Neutrals,
        Relation::Enemy => Party::Enemies,
    }
}
