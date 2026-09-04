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
/// The table is a flat concatenation of the layouts for 1..=12 players; the
/// layout for `n` players begins at `n * (n - 1) / 2`. Two players start at
/// (1,4) and (8,5) — opposite sides of the board.
///
/// Transcribed from `rgbrcStart` at `10f0:0000`, verified byte for byte
/// against the binary.
///
/// # A version caveat
///
/// This is the **2.7j** table, which is the binary this project reads. It
/// matches every two-player battle in both fixture games, but not the
/// three-player battles in `fixtures/games/all-computer-players`, which was
/// saved by 2.66 — see `docs/formats/battle.md`. The same version split that
/// governs the action record layout appears to reach this table too, and a 2.6
/// binary would be needed to confirm it.
const START_SQUARES: [u8; 78] = [
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
    0x31, 0x61, 0x83, 0x86, 0x68, 0x38, 0x16, 0x13, // 9 players
    0x31, 0x68, 0x83, 0x16, 0x61, 0x38, 0x86, 0x13, 0x44, // 10 players
    0x12, 0x15, 0x18, 0x41, 0x48, 0x54, 0x71, 0x78, 0x83, 0x86, // 11 players
    0x31, 0x68, 0x83, 0x16, 0x61, 0x38, 0x86, 0x13, 0x43, 0x36, 0x66, // 12 players
    0x41, 0x58, 0x82, 0x17, 0x86, 0x13, 0x61, 0x38, 0x21, 0x84, 0x15, 0x78,
];

/// The square the `side`-th participant starts on, in a battle with `players`
/// participants.
///
/// Returns `None` for player counts the transcribed table does not cover
/// (more than twelve) or a side index beyond the count.
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
    /// Maximum range in squares, including the extra square a starbase gets.
    pub range: i32,
    /// The weapon's own maximum range, which is what the falloff is measured
    /// against.
    pub nominal_range: i32,
    /// Firing initiative: the weapon's own, before the hull's base is added.
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
/// The order matters, because each step truncates. `FAttack` computes the base
/// as `weapon dp x launchers x ships` **first**, then applies:
///
/// 1. the attacker's **capacitor**, scaling up;
/// 2. the target's **beam deflection**, scaling down;
/// 3. **range falloff**: `dp * (100 - 10 * range / max_range) / 100`, so a
///    tenth of the damage is lost at maximum range.
///
/// Note that the nominal range used for the falloff is the weapon's own, not
/// the extra square a starbase reaches with it.
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
    let mut dp = weapon.dp * weapon.count * ships;

    if capacitor_pct != 0 {
        dp = dp * capacitor_pct / 100;
    }
    if beam_deflection_pct < 100 {
        dp = dp * beam_deflection_pct / 100;
    }
    if range > 0 && weapon.nominal_range > 0 {
        dp = dp * (100 - 10 * range / weapon.nominal_range) / 100;
    }
    dp
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

/// The highest firing initiative the game tracks.
pub const MAX_INITIATIVE: i32 = 63;

/// A token as the firing loop sees it: where it is, what it has, what it can
/// still take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatToken {
    /// Owning player.
    pub player: u8,
    /// Whether the token is still in the battle.
    pub active: bool,
    /// Current square.
    pub square: Square,
    /// The hull's base initiative, added to each weapon's own.
    pub initiative_base: i32,
    /// Capacitor bonus, as a percentage; `0` for none.
    pub capacitor_pct: i32,
    /// Beam deflection, as a percentage; `100` for none.
    pub beam_deflection_pct: i32,
    /// Weapons fitted, from the design.
    pub weapons: Vec<Weapon>,
    /// Resource-plus-boranium cost of one ship, which sets how valuable this
    /// token is as a target.
    pub value: i32,
    /// Damage state.
    pub state: TokenState,
    /// The battle plan's tactic, which drives both targeting and movement.
    pub tactic: Tactic,
    /// Battle speed as the stored quarter-square index; see
    /// [`movement_this_round`].
    pub speed_index: u8,
    /// Squares of movement still available this round, which decides whether
    /// an enemy can close before the next exchange.
    pub moves_left: u8,
    /// The class this token belongs to, for other tokens' target filters.
    pub class: TargetClass,
    /// What this token shoots at by preference.
    pub primary_target: TargetClass,
    /// What it falls back to.
    pub secondary_target: TargetClass,
    /// Whether it is a starbase.
    pub is_starbase: bool,
    /// The reach of its longest useful weapon, in squares (`dxyLim`).
    pub weapon_reach: i32,
    /// Torpedo jamming, as a percentage.
    pub pct_jam: i32,
    /// Battle-computer accuracy bonus, as a percentage.
    pub pct_computer: i32,
}

impl CombatToken {
    /// Whether this token can still shoot or be shot at.
    #[must_use]
    pub fn alive(&self) -> bool {
        self.active && self.state.ships > 0
    }

    /// The initiative each of this token's weapons fires at.
    #[must_use]
    pub fn firing_initiatives(&self) -> Vec<i32> {
        let mut out: Vec<i32> = self
            .weapons
            .iter()
            .map(|w| (w.initiative + self.initiative_base).min(MAX_INITIATIVE))
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// One token damaging another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageEvent {
    /// The token that fired.
    pub attacker: usize,
    /// The token that was hit.
    pub target: usize,
    /// Shield points stripped.
    pub shield_damage: i32,
    /// Ships destroyed.
    pub ships_killed: i32,
}

/// How attractive a token is as a target for a beam weapon.
///
/// The original scores value against how much damage it would take to get
/// through, and picks the **highest**: cost per point of work. A token that is
/// expensive and nearly dead scores best.
#[must_use]
pub fn beam_target_score(target: &CombatToken, sapper: bool) -> i32 {
    let ships = target.state.ships;
    if ships <= 0 {
        return 0;
    }
    let mut value = target.value.saturating_mul(ships);
    value = if value < 100_000 {
        value * 100
    } else {
        10_000_000
    };
    if target.beam_deflection_pct < 100 {
        value = value * target.beam_deflection_pct / 100;
    }

    let shields_left = target.state.shields * ships;
    let armor_single = target.state.armor;
    let mut armor_left = armor_single * ships;
    if target.state.damage.pct_damage != 0 {
        armor_left -=
            armor_single * target.state.damage.pct_damage / 10 * target.state.damage.pct_ships / 10
                * ships
                / 500;
    }
    if armor_left <= 0 {
        armor_left = 1;
    }

    if sapper {
        // A sapper only cares about shields, and is useless without them.
        if shields_left <= 0 {
            0
        } else {
            ceil_div(value * 100, shields_left)
        }
    } else {
        (value * 100 / (armor_left + shields_left + 1)).max(1)
    }
}

/// Fire one weapon, spilling any overkill onto the next-best target.
///
/// Returns the damage done, in order. The original re-picks a target after
/// each kill and scales the surviving damage down in proportion to what got
/// through, which is what stops one enormous volley from sweeping a whole
/// side.
fn fire_weapon(
    tokens: &mut [CombatToken],
    attacker: usize,
    weapon: Weapon,
    events: &mut Vec<DamageEvent>,
) {
    if weapon.torpedo {
        // Torpedoes roll per shot; not resolved here (see the module note).
        return;
    }
    let (player, square, capacitor, ships) = {
        let t = &tokens[attacker];
        (t.player, t.square, t.capacitor_pct, t.state.ships)
    };
    let sapper = weapon.is_sapper();
    let mut remaining = weapon.dp * weapon.count * ships;

    // Up to one pass per token: each pass either damages something or stops.
    for _ in 0..tokens.len() {
        if remaining <= 0 {
            break;
        }

        // Pick the best target in range.
        let mut best: Option<(usize, i32)> = None;
        for (i, t) in tokens.iter().enumerate() {
            if i == attacker || !t.alive() || t.player == player {
                continue;
            }
            if i32::from(distance(square, t.square)) > weapon.range {
                continue;
            }
            let score = beam_target_score(t, sapper);
            if score > 0 && best.is_none_or(|(_, b)| score > b) {
                best = Some((i, score));
            }
        }
        let Some((target, _)) = best else { break };

        let range = distance(square, tokens[target].square);
        let deflection = tokens[target].beam_deflection_pct;

        // Recompute this volley's damage against this particular target, then
        // scale it to whatever is left of the weapon's output.
        let full = weapon.dp * weapon.count * ships;
        let mut dp = beam_damage(weapon, ships, i32::from(range), capacitor, deflection);
        if full > 0 {
            dp = (i64::from(dp) * i64::from(remaining) / i64::from(full)) as i32;
        }
        if dp <= 0 {
            break;
        }

        let before = tokens[target].state;
        let result = apply_damage(before, dp, sapper);
        tokens[target].state = result.after;
        if result.after.ships == 0 {
            tokens[target].active = false;
        }

        if result.shield_damage > 0 || result.ships_killed > 0 {
            events.push(DamageEvent {
                attacker,
                target,
                shield_damage: result.shield_damage,
                ships_killed: result.ships_killed,
            });
        }

        // Overkill spills, scaled down in proportion to what got through.
        if result.overflow > 0 && dp > 0 {
            let scaled = i64::from(remaining) * i64::from(result.overflow) / i64::from(dp);
            remaining = (remaining - 1).min(scaled as i32);
        } else {
            remaining = 0;
        }
    }
}

/// Resolve one round of firing at the tokens' current positions.
///
/// Weapons fire in **initiative order**, highest first, where a weapon's
/// initiative is its own plus its hull's base. Everything at the same
/// initiative fires before anything below it, so a fast ship can destroy a
/// slower one before it ever shoots.
///
/// Within one initiative the original scans the token array **backwards**, so
/// the later token fires first. The array itself was shuffled at the start of
/// the battle (`RandomizeTokOrder`), and a recording stores it post-shuffle —
/// so replaying from a recording gets the real order for free.
///
/// Torpedoes are skipped: they roll per shot, and reproducing a recorded
/// battle needs the generator in the right state.
pub fn fire_round(tokens: &mut [CombatToken]) -> Vec<DamageEvent> {
    let mut events = Vec::new();

    for initiative in (0..=MAX_INITIATIVE).rev() {
        // The original scans tokens from the **last** index down to zero, so
        // where two tokens fire at the same initiative the later one shoots
        // first. In a symmetric duel that decides who survives, which is why
        // the direction matters rather than being an implementation detail.
        for attacker in (0..tokens.len()).rev() {
            if !tokens[attacker].alive() {
                continue;
            }
            let base = tokens[attacker].initiative_base;
            let firing: Vec<Weapon> = tokens[attacker]
                .weapons
                .iter()
                .filter(|w| (w.initiative + base).min(MAX_INITIATIVE) == initiative)
                .copied()
                .collect();

            for weapon in firing {
                if !tokens[attacker].alive() {
                    break;
                }
                fire_weapon(tokens, attacker, weapon, &mut events);
            }
        }
    }

    events
}

/// Where a token would like to be, and how it gets there.
///
/// Source: `DxyMoveTokTo` (`10f0:5f18`), whose selection and step-toward logic
/// are recovered in full; the square scoring it relies on
/// (`ScoreGuessBattleDamage`, `10f0:598c`) is only partly recovered — see
/// [`score_square`] and `docs/formulas/combat.md`.
///
/// Every stage of the choice breaks ties with `Random`, so a token's movement
/// cannot be reproduced without the generator in the same state.
///
/// A token that scores its current square best simply stays.
pub fn choose_move(tokens: &[CombatToken], mover: usize, radius: i32, rng: &mut Rng) -> Square {
    let token = &tokens[mover];
    let here = token.square;
    if radius <= 0 {
        return here;
    }

    let x0 = i32::from(here.x);
    let y0 = i32::from(here.y);
    let board = i32::from(BOARD_SIZE) - 1;

    // The eight neighbours plus the current square, kept because the
    // step-toward logic below chooses among them by score.
    let mut near = [[i32::MAX; 3]; 3];

    let mut best_score = i32::MAX;
    let mut best_distance = i32::MAX;
    let mut ties = 0;
    let mut best = here;

    for x in (x0 - radius).max(0)..=(x0 + radius).min(board) {
        for y in (y0 - radius).max(0)..=(y0 + radius).min(board) {
            let square = Square::new(x as u8, y as u8);
            let mut score = score_square(tokens, mover, square);

            // Disengaging tokens avoid piling onto friends and prefer to move.
            if token.tactic == Tactic::Disengage {
                let crowd = tokens
                    .iter()
                    .filter(|t| t.alive() && t.player == token.player && t.square == square)
                    .count();
                score += 2 * i32::try_from(crowd).unwrap_or(0);
                if square == here {
                    score -= 1;
                }
            }

            let away = i32::from(distance(here, square));
            if away <= 1 {
                near[(x - x0 + 1) as usize][(y - y0 + 1) as usize] = score;
            }

            // Better score wins; equal score prefers the nearer square; an
            // exact tie is broken by reservoir sampling, as the original does.
            if score < best_score || (score == best_score && away <= best_distance) {
                if score == best_score && away == best_distance {
                    ties += 1;
                    if rng.random(i16::try_from(ties).unwrap_or(i16::MAX)) == 0 {
                        best = square;
                    }
                } else {
                    ties = 1;
                    best_score = score;
                    best_distance = away;
                    best = square;
                }
            }
        }
    }

    if distance(here, best) <= 1 {
        return best;
    }
    step_toward(near, here, best, rng)
}

/// Take one step from `here` toward `target`, choosing among the neighbours by
/// their scores.
///
/// A diagonal target is approached diagonally. When the target is straight
/// along one axis the token still picks which of the three squares on that
/// side to use, by score, breaking ties randomly — which is why two identical
/// ships closing on each other do not always take the same path.
fn step_toward(near: [[i32; 3]; 3], here: Square, target: Square, rng: &mut Rng) -> Square {
    let dx = i32::from(target.x) - i32::from(here.x);
    let dy = i32::from(target.y) - i32::from(here.y);
    let mut x = i32::from(here.x);
    let mut y = i32::from(here.y);

    if dx.abs() == dy.abs() {
        x += dx.signum();
        y += dy.signum();
    } else if dx == 0 {
        let col = usize::from(dy > 0) * 2;
        y += if dy > 0 { 1 } else { -1 };
        x += pick_lowest(&[near[0][col], near[1][col], near[2][col]], rng) - 1;
    } else if dy == 0 {
        let row = usize::from(dx > 0) * 2;
        x += if dx > 0 { 1 } else { -1 };
        y += pick_lowest(&[near[row][0], near[row][1], near[row][2]], rng) - 1;
    } else {
        // Neither straight nor diagonal: try the corner, and the square that
        // keeps moving along the longer axis.
        let cx = usize::from(dx > 0) * 2;
        let cy = usize::from(dy > 0) * 2;
        let second = if dx.abs() > dy.abs() {
            (cx, 1)
        } else {
            (1, cy)
        };
        let take_first = near[cx][cy] < near[second.0][second.1]
            || (near[cx][cy] == near[second.0][second.1] && rng.random(2) == 0);
        let (px, py) = if take_first { (cx, cy) } else { second };
        x += i32::try_from(px).unwrap_or(1) - 1;
        y += i32::try_from(py).unwrap_or(1) - 1;
    }

    Square::new(
        x.clamp(0, i32::from(BOARD_SIZE) - 1) as u8,
        y.clamp(0, i32::from(BOARD_SIZE) - 1) as u8,
    )
}

/// Index of the lowest of three scores, ties broken randomly.
fn pick_lowest(scores: &[i32; 3], rng: &mut Rng) -> i32 {
    let lowest = *scores.iter().min().unwrap_or(&0);
    let count = scores.iter().filter(|s| **s == lowest).count();
    let mut nth = rng.random(i16::try_from(count).unwrap_or(1));
    for (i, s) in scores.iter().enumerate() {
        if *s == lowest {
            if nth == 0 {
                return i32::try_from(i).unwrap_or(1);
            }
            nth -= 1;
        }
    }
    1
}

/// Damage one token would do to another at a given range.
///
/// Source: `DpFromPtokBrcToBrc` (`10f0:4d2e`), transcribed from the
/// disassembly. This is the *estimate* that drives target scoring and
/// movement, not the resolution of an actual shot — the two differ, and it is
/// worth knowing which is which.
///
/// `proximity` drops both the range checks, so a token can weigh threats that
/// cannot quite reach it yet. A disengaging token uses it.
#[must_use]
pub fn damage_estimate(
    attacker: &CombatToken,
    target: &CombatToken,
    range: i32,
    proximity: bool,
) -> i32 {
    // A token-level reach check comes first: past its longest weapon, nothing
    // it carries is worth evaluating.
    if !proximity && range > attacker.weapon_reach {
        return 0;
    }

    let mut total = 0;
    for w in &attacker.weapons {
        let out_of_range = range > w.range;
        if out_of_range && !proximity {
            continue;
        }

        let mut dp = if w.torpedo {
            // The count is scaled by 200 so that misses can contribute a
            // fraction of a torpedo's damage to shields.
            let salvo = w.count * attacker.state.ships * 200;
            let hits =
                salvo * torpedo_accuracy(w.accuracy, target.pct_jam, attacker.pct_computer) / 100;
            let mut dp = hits * w.dp / 200;
            if target.state.shields > 0 {
                // Collateral: torpedoes that miss still splash the shields,
                // for an eighth of their damage each.
                dp += (salvo - hits) * w.dp / 1600;
            }
            dp
        } else {
            // Note the order: the estimate applies the range falloff *before*
            // beam deflection, where `FAttack` applies deflection first. Each
            // step truncates, so they are not interchangeable.
            let mut dp = w.dp * w.count;
            if attacker.capacitor_pct != 0 {
                dp = dp * attacker.capacitor_pct / 100;
            }
            let effective = range.min(w.range);
            if effective > 0 && w.nominal_range > 0 {
                dp -= dp * effective / 10 / w.nominal_range;
            }
            if target.beam_deflection_pct < 100 {
                dp = dp * target.beam_deflection_pct / 100;
            }
            if w.is_sapper() {
                // A sapper cannot do more than the shields it is there to
                // strip.
                dp = dp.min(target.state.shields * attacker.state.ships);
            }
            dp
        };

        if out_of_range {
            // Beyond reach the threat is discounted the further away it is,
            // but never below one point per launcher.
            let divisor = range + 10 - w.range;
            if divisor > 0 {
                dp /= divisor;
            }
            dp = dp.max(w.count);
        }

        // Beam damage is per ship; the torpedo path already folded the count in.
        total += if w.torpedo {
            dp
        } else {
            dp * attacker.state.ships
        };
    }

    // Never more than the target could still absorb: shields plus armour,
    // less whatever damage it is already carrying.
    if !proximity {
        let ships = target.state.ships;
        let mut capacity = (target.state.shields + target.state.armor) * ships;
        let dv = target.state.damage;
        if (dv.pct_ships != 0 || dv.pct_damage != 0) && capacity > 0 {
            capacity -= target.state.armor * dv.pct_damage / 10 * dv.pct_ships / 10 * ships / 500;
            if capacity <= 0 {
                capacity = 1;
            }
        }
        if total > capacity {
            total = capacity;
        }
    }
    total
}

/// How good a square would be for a token, lower being better.
///
/// Source: `ScoreGuessBattleDamage` (`10f0:598c`), transcribed from the
/// disassembly because the decompiler loses which token is which across the
/// nested damage-estimate calls.
///
/// For every enemy it could engage, the token asks: *if that enemy moves to
/// whichever range suits them best, what does the exchange look like there?*
/// It then keeps the **best damage it could deal** to any one of them and the
/// **total damage it would take** from all of them, and combines the two by
/// its own tactic.
///
/// The range band an enemy can reach is the crux, and it is what makes the
/// scoring discriminate between squares that otherwise tie: an enemy with at
/// least as many moves left as the mover can close by one square, so the band
/// runs from one nearer to the furthest corner of their reachable box.
#[must_use]
pub fn score_square(tokens: &[CombatToken], mover: usize, square: Square) -> i32 {
    let us = &tokens[mover];
    let mut given_best = 0;
    let mut taken_total = 0;
    let board = i32::from(BOARD_SIZE) - 1;

    // The plan hunts its primary class if anything of that class is present,
    // and falls back to the secondary otherwise — decided once, for the whole
    // scoring pass, exactly as `DxyMoveTokTo` does.
    let hunting = if primary_target_exists(tokens, mover) {
        us.primary_target
    } else {
        us.secondary_target
    };

    for (i, them) in tokens.iter().enumerate() {
        if i == mover || !them.alive() || them.player == us.player {
            continue;
        }
        // A token of the wrong class still threatens us; we just cannot shoot
        // back at it. So it contributes to what we take but not to what we
        // deal.
        let we_attack = is_target_of(them, hunting);

        let straight = i32::from(distance(square, them.square));

        // Can this enemy close on us? Only if they have at least as much
        // movement left as we do.
        let closes = i32::from(them.moves_left >= us.moves_left);
        let (near, far) = if closes == 0 {
            (straight, straight)
        } else {
            let ex = i32::from(them.square.x);
            let ey = i32::from(them.square.y);
            let mut far = straight;
            for x in [ex - closes, ex + closes] {
                for y in [ey - closes, ey + closes] {
                    let corner = Square::new(x.clamp(0, board) as u8, y.clamp(0, board) as u8);
                    far = far.max(i32::from(distance(square, corner)));
                }
            }
            ((straight - closes).max(0), far)
        };

        // The enemy will pick whichever range in that band suits their tactic.
        let proximity = us.tactic == Tactic::Disengage;
        let mut their_best = 30_000_000;
        let mut taken_at_best = 0;
        let mut given_at_best = 0;

        for range in near..=far {
            let given = if we_attack {
                damage_estimate(us, them, range, false)
            } else {
                0
            };
            let taken = damage_estimate(them, us, range, proximity);
            // Scored from the enemy's point of view, with the enemy's tactic:
            // what they deal is `taken`, what they suffer is `given`.
            let theirs = target_score(taken, given, them.tactic);
            if theirs <= their_best {
                their_best = theirs;
                taken_at_best = taken;
                given_at_best = given;
            }
        }

        given_best = given_best.max(given_at_best);
        taken_total += taken_at_best;
    }

    target_score(given_best, taken_total, us.tactic)
}

/// What class of ship a battle plan will shoot at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TargetClass {
    /// Nothing; the token is disengaging.
    None = 0,
    /// Anything.
    Any = 1,
    /// Starbases only.
    Starbase = 2,
    /// Armed ships.
    ArmedShips = 3,
    /// Bombers and freighters.
    BombersFreighters = 4,
    /// Unarmed ships.
    UnarmedShips = 5,
    /// Fuel transports.
    FuelTransports = 6,
    /// Freighters.
    Freighters = 7,
}

impl TargetClass {
    /// Decode a stored target nibble.
    #[must_use]
    pub fn from_raw(v: u8) -> Self {
        match v {
            1 => Self::Any,
            2 => Self::Starbase,
            3 => Self::ArmedShips,
            4 => Self::BombersFreighters,
            5 => Self::UnarmedShips,
            6 => Self::FuelTransports,
            7 => Self::Freighters,
            _ => Self::None,
        }
    }
}

/// Whether a token belongs to the class a battle plan is hunting.
///
/// Source: `FIsTargetOfMdTarget` (`battle.c`). Two classes are broader than
/// their name: "bombers and freighters" also matches plain freighters, and
/// "unarmed ships" matches freighters and fuel transports too.
#[must_use]
pub fn is_target_of(token: &CombatToken, class: TargetClass) -> bool {
    match class {
        TargetClass::None => false,
        TargetClass::Any => true,
        TargetClass::Starbase => token.is_starbase,
        TargetClass::ArmedShips | TargetClass::FuelTransports | TargetClass::Freighters => {
            token.class == class
        }
        TargetClass::BombersFreighters => {
            token.class == TargetClass::BombersFreighters || token.class == TargetClass::Freighters
        }
        TargetClass::UnarmedShips => {
            token.class == TargetClass::UnarmedShips
                || token.class == TargetClass::Freighters
                || token.class == TargetClass::FuelTransports
        }
    }
}

/// Whether any enemy of the token's primary target class is present.
///
/// Source: `FDoesPrimaryTargetTypeExist`. Decided once per movement decision,
/// not per candidate square.
#[must_use]
pub fn primary_target_exists(tokens: &[CombatToken], mover: usize) -> bool {
    let us = &tokens[mover];
    tokens.iter().enumerate().any(|(i, them)| {
        i != mover
            && them.alive()
            && them.player != us.player
            && is_target_of(them, us.primary_target)
    })
}

/// How far a token looks when deciding where to move, and where to head if
/// nothing is worth engaging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveSearch {
    /// Radius of squares to score around the current position.
    pub radius: i32,
    /// When no valid target is within reach, the square to head toward
    /// instead — the nearest enemy the token could actually hurt.
    pub beeline: Option<Square>,
}

/// Decide the search radius for a token's move.
///
/// Source: `DzMoveRangeToConsider` (`10f0:5312`).
///
/// If any target of the right class is close enough that the token could
/// engage it — its distance, plus one if that enemy can close as fast, within
/// the token's weapon reach plus its remaining movement — the token searches
/// out to its full remaining movement and picks a square normally.
///
/// If nothing is in reach it stops scoring the neighbourhood and simply heads
/// for the nearest enemy it could hurt, which is why fleets close across an
/// empty board in a straight line rather than dithering.
#[must_use]
pub fn move_search(tokens: &[CombatToken], mover: usize, primary: bool) -> MoveSearch {
    let us = &tokens[mover];
    let moves = i32::from(us.moves_left);
    let class = if primary {
        us.primary_target
    } else {
        us.secondary_target
    };
    let reach = us.weapon_reach + moves;

    let mut nearest: Option<(i32, Square)> = None;

    for (i, them) in tokens.iter().enumerate() {
        if i == mover || !them.alive() || them.player == us.player {
            continue;
        }
        if !is_target_of(them, class) {
            continue;
        }

        let mut dz = i32::from(distance(us.square, them.square));
        if them.moves_left >= us.moves_left {
            dz += 1;
        }

        if dz <= reach {
            // Something is engageable: search the full movement allowance.
            return MoveSearch {
                radius: moves,
                beeline: None,
            };
        }

        // Otherwise remember the nearest enemy this token could actually hurt.
        if nearest.is_none_or(|(best, _)| dz < best) && damage_estimate(us, them, dz, true) > 0 {
            nearest = Some((dz, them.square));
        }
    }

    MoveSearch {
        radius: 1,
        beeline: nearest.map(|(_, square)| square),
    }
}
