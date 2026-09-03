//! Combat: the battle board, movement, targeting and weapon accuracy.
//!
//! Source: `battle.c` routines in `stars.2.7j.exe` — `DxyFromSpdRound`
//! (`10e8:...`), `DzFromBrcBrc`, `CTorpHit`,
//! `ScoreFromGiveAndTakeAndTactic` and the `rgbrcStart` table — cross-checked
//! against `MANUAL.PDF` pp. 23-2..23-10. Full derivation in
//! `docs/formulas/combat.md`.
//!
//! Battles play out on a 10x10 board over up to 16 rounds. Each round every
//! token may move a fraction of a square (accumulated so that, say, a
//! 1½-square ship alternates two squares and one), then weapons fire in
//! initiative order.
//!
//! What is implemented here is the deterministic skeleton: geometry, starting
//! positions, the movement schedule, target scoring and weapon accuracy. The
//! full fire-resolution loop needs the ship-design layer, because a token's
//! weapon slots come from its design.

use crate::rng::Rng;

/// Width and height of the usable battle board.
pub const BOARD_SIZE: u8 = 10;

/// The maximum number of rounds a battle runs for.
pub const MAX_ROUNDS: u8 = 16;

/// A square on the battle board.
///
/// Stored in one byte as `y << 4 | x`, which is what the recordings and the
/// original both use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Square {
    /// Column.
    pub x: u8,
    /// Row.
    pub y: u8,
}

impl Square {
    /// Construct a square.
    #[must_use]
    pub fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }

    /// Decode the packed byte the game stores.
    #[must_use]
    pub fn from_brc(brc: u8) -> Self {
        Self {
            x: brc & 0x0f,
            y: brc >> 4,
        }
    }

    /// Re-pack to the stored byte.
    #[must_use]
    pub fn to_brc(self) -> u8 {
        ((self.y & 0x0f) << 4) | (self.x & 0x0f)
    }

    /// Whether this square is on the usable board.
    #[must_use]
    pub fn on_board(self) -> bool {
        self.x < BOARD_SIZE && self.y < BOARD_SIZE
    }
}

/// Distance between two squares, in squares.
///
/// Movement and weapon range are both **Chebyshev** distance — the larger of
/// the two axis differences — so diagonals cost the same as orthogonals
/// (`DzFromBrcBrc`).
#[must_use]
pub fn distance(a: Square, b: Square) -> u8 {
    let dx = a.x.abs_diff(b.x);
    let dy = a.y.abs_diff(b.y);
    dx.max(dy)
}

/// Starting squares, laid out by how many players are in the battle.
///
/// The table is a flat concatenation of the layouts for 1..=16 players; the
/// layout for `n` players begins at `n * (n - 1) / 2`. Two players start at
/// (1,4) and (8,5) — opposite sides of the board.
///
/// Transcribed from `rgbrcStart` at the start of the battle segment.
const START_SQUARES: [u8; 36] = [
    // 1 player
    0x44, // (4,4)
    // 2 players
    0x41, 0x58, // (1,4) (8,5)
    // 3 players
    0x14, 0x88, 0x81, // (4,1) (8,8) (1,8)
    // 4 players
    0x11, 0x88, 0x81, 0x18, // (1,1) (8,8) (1,8) (8,1)
    // 5 players
    0x14, 0x86, 0x41, 0x48, 0x82, // (4,1) (6,8) (1,4) (8,4) (2,8)
    // 6 players
    0x41, 0x58, 0x82, 0x17, 0x86, 0x13, // (1,4) (8,5) (2,8) (7,1) (6,8) (3,1)
    // 7 players
    0x11, 0x51, 0x82, 0x86, 0x68, 0x28, 0x15, // (1,1) (1,5) (2,8) (6,8) (8,6) (8,2) (5,1)
    // 8 players
    0x31, 0x61, 0x83, 0x86, 0x68, 0x38, 0x16, 0x13,
];

/// The square the `side`-th participant starts on, in a battle with `players`
/// participants.
///
/// Returns `None` for player counts the transcribed table does not cover
/// (more than eight) or a side index beyond the count.
#[must_use]
pub fn start_square(players: u8, side: u8) -> Option<Square> {
    if players == 0 || side >= players {
        return None;
    }
    let base = usize::from(players) * (usize::from(players) - 1) / 2;
    let index = base + usize::from(side);
    START_SQUARES.get(index).map(|brc| Square::from_brc(*brc))
}

/// How far a token moves in a given round.
///
/// `speed` is the game's quarter-square index: `0` is half a square per round
/// and each step adds a quarter, so a speed of `2` is exactly one square and
/// `6` is two. Fractional speeds are realised by moving different distances in
/// different rounds — a 1½-square ship alternates 2 and 1 — which is why this
/// depends on the round number.
///
/// Reproduces the manual's "Movement in Squares per Round" table
/// (`MANUAL.PDF` p. 23-9) for every listed speed.
#[must_use]
pub fn movement_this_round(speed: u8, round: u8) -> u8 {
    let mut dxy = (u16::from(speed) + 2) / 4;
    match speed & 3 {
        0 => dxy += u16::from(round & 1 == 0),
        1 => dxy += u16::from(round & 3 != 2),
        3 => dxy += u16::from(round & 3 == 0),
        // A speed of exactly n squares moves n every round.
        _ => {}
    }
    u8::try_from(dxy).unwrap_or(u8::MAX)
}

/// Total squares a token covers over `rounds` rounds at a constant speed.
#[must_use]
pub fn movement_over(speed: u8, rounds: u8) -> u32 {
    (0..rounds)
        .map(|r| u32::from(movement_this_round(speed, r)))
        .sum()
}

/// A battle plan's tactic, which decides how a token picks its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Tactic {
    /// Run for the edge of the board.
    Disengage = 0,
    /// Run once damaged.
    DisengageIfChallenged = 1,
    /// Take as little damage as possible.
    MinimiseDamageToSelf = 2,
    /// Maximise damage dealt, ignoring damage taken.
    MaximiseDamage = 3,
    /// Maximise damage dealt minus damage taken.
    MaximiseNetDamage = 4,
    /// Maximise the ratio of damage dealt to damage taken.
    MaximiseDamageRatio = 5,
}

impl Tactic {
    /// Decode the stored tactic nibble.
    #[must_use]
    pub fn from_raw(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::Disengage,
            1 => Self::DisengageIfChallenged,
            2 => Self::MinimiseDamageToSelf,
            3 => Self::MaximiseDamage,
            4 => Self::MaximiseNetDamage,
            5 => Self::MaximiseDamageRatio,
            _ => return None,
        })
    }
}

/// Score a candidate target, given the damage a token would deal and take.
///
/// Lower is better: the original picks the minimum score. The tactic decides
/// what is being minimised — damage taken, damage dealt (negated), or the
/// ratio between them.
#[must_use]
pub fn target_score(damage_given: i32, damage_taken: i32, tactic: Tactic) -> i32 {
    match tactic {
        // Only damage taken matters.
        Tactic::Disengage | Tactic::MinimiseDamageToSelf => damage_taken,
        // Only damage dealt matters; negate so more is better.
        Tactic::DisengageIfChallenged | Tactic::MaximiseDamage => -damage_given,
        // Ratio of dealt to taken, scaled by 100 and capped just below zero so
        // that any damage dealt still beats dealing none.
        Tactic::MaximiseNetDamage | Tactic::MaximiseDamageRatio => {
            if damage_given == 0 {
                damage_taken
            } else {
                let score = i64::from(-damage_given) * 100 / (i64::from(damage_taken) + 1);
                i32::try_from(score.min(-1)).unwrap_or(i32::MIN)
            }
        }
    }
}

/// Torpedo accuracy against a target, as a percentage.
///
/// Jammers reduce accuracy; battle computers reduce **inaccuracy**, which is
/// not the same thing and matters more the more accurate the torpedo already
/// is. When both are present they cancel one for one first
/// (`MANUAL.PDF` p. 23-6).
///
/// A 75% torpedo fired through a 50% battle computer is
/// `100 - (100 - 75) * 50 / 100 = 88%`, exactly the manual's worked example.
#[must_use]
pub fn torpedo_accuracy(base_pct: i32, jam_pct: i32, computer_pct: i32) -> i32 {
    let mut jam = jam_pct;
    let mut computer = computer_pct;

    // Jammers and battle computers cancel on a one-for-one basis.
    if jam != 0 && computer != 0 {
        let net = jam - computer;
        if net < 0 {
            computer = -net;
            jam = 0;
        } else {
            jam = net;
            computer = 0;
        }
    }

    let accuracy = if computer == 0 {
        if jam == 0 {
            base_pct
        } else {
            base_pct * (100 - jam) / 100
        }
    } else {
        100 - (100 - base_pct) * (100 - computer) / 100
    };

    accuracy.max(1)
}

/// How many of `count` torpedoes hit.
///
/// Each torpedo is rolled separately up to 200 of them; beyond that the
/// original takes the average instead, which keeps a big salvo cheap and
/// removes the variance.
pub fn torpedoes_hitting(count: i32, accuracy_pct: i32, rng: &mut Rng) -> i32 {
    if count == 0 || accuracy_pct == 0 {
        return 0;
    }
    let accuracy = accuracy_pct.max(1);
    if accuracy >= 100 {
        return count;
    }
    if count < 201 {
        (0..count)
            .filter(|_| i32::from(rng.random(100)) < accuracy)
            .count()
            .try_into()
            .unwrap_or(i32::MAX)
    } else {
        count * accuracy / 100
    }
}

/// Damage a stack of ships takes, and what survives.
///
/// Shields **overlap across the whole token**: twenty scouts with 20 shield
/// points each present a single 400-point pool that must be stripped before
/// any armour is touched. Beam weapons are stopped by that pool; torpedoes
/// damage shields and armour together (`MANUAL.PDF` p. 23-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackDamage {
    /// Shield points remaining.
    pub shields: i32,
    /// Ships destroyed.
    pub ships_killed: i32,
    /// Damage carried by the surviving, partly-damaged ships.
    pub armour_damage: i32,
}

/// Apply beam damage to a token.
///
/// Beams strip the shared shield pool first and only then bite into armour.
/// Damage that would overkill the token spills over to other tokens in the
/// same square, which the caller handles; the leftover is returned as
/// `overflow`.
#[must_use]
pub fn apply_beam_damage(
    damage: i32,
    shields: i32,
    ships: i32,
    armour_per_ship: i32,
    existing_armour_damage: i32,
) -> (StackDamage, i32) {
    let absorbed = damage.min(shields.max(0));
    let mut remaining = damage - absorbed;
    let shields_left = (shields - absorbed).max(0);

    if remaining <= 0 || ships <= 0 || armour_per_ship <= 0 {
        return (
            StackDamage {
                shields: shields_left,
                ships_killed: 0,
                armour_damage: existing_armour_damage,
            },
            0,
        );
    }

    // Damage already carried counts toward killing the first ship.
    let total_armour = armour_per_ship * ships - existing_armour_damage;
    let spent = remaining.min(total_armour);
    remaining -= spent;

    let inflicted = existing_armour_damage + spent;
    let killed = (inflicted / armour_per_ship).min(ships);
    let carried = inflicted - killed * armour_per_ship;

    (
        StackDamage {
            shields: shields_left,
            ships_killed: killed,
            armour_damage: carried,
        },
        remaining,
    )
}

/// Integer division rounding up, which the original spells as `(a + b - 1) / b`.
fn ceil_div(a: i32, b: i32) -> i32 {
    if b == 0 {
        return 0;
    }
    (a + b - 1) / b
}

/// A weapon fitted to a design, flattened out of its slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weapon {
    /// Whether this is a beam or a torpedo launcher.
    pub torpedo: bool,
    /// Damage points per shot (per launcher, before any modifier).
    pub dp: i32,
    /// How many are fitted.
    pub count: i32,
    /// Maximum range in squares.
    pub range: i32,
    /// Firing initiative.
    pub initiative: i32,
    /// Base accuracy, for torpedoes.
    pub accuracy: i32,
    /// Ability flags; bit 0 marks a sapper (shields only).
    pub abilities: i32,
}

impl Weapon {
    /// Whether this weapon only damages shields.
    #[must_use]
    pub fn is_sapper(self) -> bool {
        !self.torpedo && self.abilities & 1 != 0
    }
}

/// Beam damage a stack does to a target, before shields.
///
/// The pieces, in the order `DpFromPtokBrcToBrc` applies them:
///
/// * base damage is `weapon dp x launchers x ships firing`;
/// * a **capacitor** scales it up by the attacker's `pctCap`;
/// * damage **falls off with range**, losing a tenth of itself at maximum
///   range: `dp -= dp * range / 10 / max_range`;
/// * the target's **beam deflection** scales what is left down.
///
/// A starbase reaches one square further than its weapons' nominal range.
#[must_use]
pub fn beam_damage(
    weapon: Weapon,
    ships: i32,
    range: i32,
    capacitor_pct: i32,
    beam_deflection_pct: i32,
) -> i32 {
    if weapon.torpedo || range > weapon.range {
        return 0;
    }
    let mut dp = weapon.dp * weapon.count;

    if capacitor_pct != 0 {
        dp = dp * capacitor_pct / 100;
    }
    // Ten percent of the damage is lost across the full range band.
    if range > 0 && weapon.range > 0 {
        dp -= dp * range / 10 / weapon.range;
    }
    if beam_deflection_pct < 100 {
        dp = dp * beam_deflection_pct / 100;
    }
    dp * ships
}

/// A token's damage state: how many of its ships are hurt and how badly.
///
/// Stored packed as `pctSh:7, pctDp:9` — the percentage of the stack that is
/// damaged, and the damage each of those carries as a fraction of 500.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Damage {
    /// Percentage of the stack that is damaged, `0..=100`.
    pub pct_ships: i32,
    /// Damage on each of those ships, in 500ths of the design's armour.
    pub pct_damage: i32,
}

impl Damage {
    /// Unpack the stored word.
    #[must_use]
    pub fn from_raw(dv: u16) -> Self {
        Self {
            pct_ships: i32::from(dv & 0x7f),
            pct_damage: i32::from(dv >> 7),
        }
    }

    /// Pack back to the stored word.
    #[must_use]
    pub fn to_raw(self) -> u16 {
        let ships = u16::try_from(self.pct_ships.clamp(0, 127)).unwrap_or(0);
        let damage = u16::try_from(self.pct_damage.clamp(0, 511)).unwrap_or(0);
        ships | (damage << 7)
    }
}

/// The state of one token as far as taking damage is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenState {
    /// Ships remaining in the stack.
    pub ships: i32,
    /// Shield points **per ship**; the stack's pool is this times `ships`.
    pub shields: i32,
    /// The design's armour, per ship.
    pub armor: i32,
    /// Accumulated damage.
    pub damage: Damage,
}

/// What one attack did to a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackResult {
    /// Shield points stripped.
    pub shield_damage: i32,
    /// Ships destroyed.
    pub ships_killed: i32,
    /// The token afterwards.
    pub after: TokenState,
    /// Damage left over, which spills onto other tokens in the same square.
    pub overflow: i32,
}

/// Apply damage to a token, exactly as `FDamageTok` does.
///
/// Shields pool across the whole stack and are stripped first — beam damage
/// cannot touch armour until they are gone. What remains kills ships one at a
/// time, and **already-damaged ships die first** because they cost less to
/// finish off. Whatever is left after that is spread over the survivors as
/// fresh damage.
///
/// `shields_only` marks a sapper, which strips shields and stops.
#[must_use]
pub fn apply_damage(state: TokenState, damage: i32, shields_only: bool) -> AttackResult {
    let mut token = state;
    let mut dp = damage.max(0);

    // --- shields, pooled across the stack
    let mut shield_damage = 0;
    if token.shields > 0 && token.ships > 0 {
        let pool = token.shields * token.ships;
        if dp < pool {
            shield_damage = dp;
            token.shields = (pool - dp) / token.ships;
            dp = 0;
        } else {
            shield_damage = pool;
            dp -= pool;
            token.shields = 0;
        }
    }

    if shields_only || dp == 0 || token.ships == 0 || token.armor <= 0 {
        return AttackResult {
            shield_damage,
            ships_killed: 0,
            after: token,
            overflow: if shields_only { 0 } else { dp },
        };
    }

    // --- armour
    let ships_before = token.ships;
    let armor = token.armor;

    // How many ships are already damaged, and by how much each.
    let (damaged_before, carried) = if token.damage.pct_damage == 0 {
        (0, 0)
    } else {
        (
            (ships_before * token.damage.pct_ships / 100).max(1),
            (armor * token.damage.pct_damage / 500).max(1),
        )
    };

    let mut ships = ships_before;
    let mut damaged = damaged_before;

    // Damaged ships die first: they only need finishing off.
    if damaged_before != 0 {
        let cost = armor - carried;
        let mut left = damaged_before;
        while cost <= dp && left != 0 {
            dp -= cost;
            left -= 1;
        }
        damaged = left;
        ships = left + (ships_before - damaged_before);
    }

    // Then undamaged ships, at the design's full armour each.
    while armor <= dp && ships != 0 {
        dp -= armor;
        ships -= 1;
    }

    // Whatever is left becomes damage spread over the survivors.
    let after_damage = if dp == 0 || ships == 0 {
        if damaged == 0 {
            Damage::default()
        } else {
            Damage {
                pct_ships: ceil_div(damaged * 100, ships.max(1)),
                pct_damage: token.damage.pct_damage,
            }
        }
    } else {
        let mut spread = dp;
        if damaged != 0 {
            spread += armor * damaged + (ships - 1);
        }
        spread = (spread / ships).max(1);
        Damage {
            pct_ships: 100,
            pct_damage: ceil_div(spread * 500, armor).clamp(1, 499),
        }
    };

    let killed = ships_before - ships;
    token.ships = ships;
    token.damage = if ships == 0 {
        Damage::default()
    } else {
        after_damage
    };

    AttackResult {
        shield_damage,
        ships_killed: killed,
        after: token,
        overflow: if ships == 0 { dp } else { 0 },
    }
}
