//! The battle VCR: playing back a recorded battle, frame by frame.
//!
//! This is view logic, not simulation. A battle recording carries every move,
//! every shot and every casualty the engine produced, so the VCR **plays the
//! recording** rather than re-deriving it. That distinction is not stylistic:
//! `docs/rng/prng.md` establishes that the gameplay generator's state is not in
//! the save files, and combat draws on it for every movement tie-break and
//! every torpedo, so a re-simulation would disagree with the recording it is
//! supposed to be showing.
//!
//! Reading a recording correctly takes three details that are easy to miss, all
//! of them established in `docs/formulas/combat.md`:
//!
//! * **A firing record repeats the token's own square.** Telling a move from a
//!   shot means tracking each token's *current* position, not comparing against
//!   where it started.
//! * **`brcDest` of `0xFF` is not a square.** It is the record of a token
//!   leaving the battle, which the format layer surfaces as `destination:
//!   None`.
//! * **`initMin == 0xFF` means the token has no weapons.** A player's file
//!   holds only that player's own designs, so an opponent's ship cannot be
//!   looked up by design slot; this is the one thing the recording says about
//!   it directly.

use stars_formats::{BattleRecord, BattleToken};

/// A square of the ten-by-ten battle board.
pub const BOARD: u8 = 10;

/// One token, as the VCR knows it at a point in the battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// Its index in the recording, which is how actions refer to it.
    pub index: usize,
    /// Owning player.
    pub player: u8,
    /// Its design slot — sixteen and up for a starbase — which is where its
    /// picture comes from.
    pub design: u8,
    /// Where it is, or `None` once it has left the battle.
    pub square: Option<(u8, u8)>,
    /// Ships still in the stack.
    pub ships: i32,
    /// Shield points per ship.
    pub shields: i32,
    /// Whether it carries any weapon at all (`initMin != 0xFF`).
    pub armed: bool,
    /// Whether it is still in the battle.
    pub active: bool,
}

impl Token {
    fn from_record(index: usize, t: &BattleToken) -> Self {
        Self {
            index,
            player: t.player,
            design: t.design,
            square: Some((t.square.x, t.square.y)),
            ships: i32::from(t.ships),
            shields: i32::from(t.shields),
            armed: t.initiative_min != 0xFF,
            active: true,
        }
    }
}

/// What happened in one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A token moved, one step.
    Move {
        /// The token that moved.
        token: usize,
        /// Where it came from.
        from: (u8, u8),
        /// Where it went.
        to: (u8, u8),
    },
    /// A token fired, and what its shot did.
    Fire {
        /// The token that fired.
        attacker: usize,
        /// The token it fired at.
        target: usize,
        /// Range in squares, as recorded.
        range: u8,
        /// Ships destroyed by this shot, across all tokens hit.
        ships_killed: u32,
        /// Every token the shot reached, in the order the kills were
        /// recorded, with the weapon flags of all the kills on it.
        shots: Vec<Shot>,
    },
    /// A token left the battle.
    Disengage {
        /// The token that left.
        token: usize,
    },
}

/// One token a shot reached (`AnimateAttack`, `10e8:3ac2`, works through
/// the kills of a record a target at a time, or-ing their weapon flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shot {
    /// The token hit.
    pub target: usize,
    /// The `grfWeapon` flags of every kill on it, or-ed together: bits 0
    /// and 1 a beam (bit 1 the pen in the friendlier blue), bit 2 a
    /// torpedo, bit 6 the torpedoes deflected — nothing lands.
    pub weapon: u8,
    /// Ships destroyed on it.
    pub ships_killed: u32,
}

impl Shot {
    /// A beam was fired at this token.
    #[must_use]
    pub fn beam(self) -> bool {
        self.weapon & 0x03 != 0
    }

    /// A torpedo was fired at this token.
    #[must_use]
    pub fn torpedo(self) -> bool {
        self.weapon & 0x04 != 0
    }

    /// The torpedoes were deflected: no burst lands.
    #[must_use]
    pub fn deflected(self) -> bool {
        self.weapon & 0x40 != 0
    }
}

/// One step of the playback: what happened, and the board just after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The battle round this happened in, `0..=15`.
    pub round: u8,
    /// What happened.
    pub event: Event,
    /// Every token, as it stands after this frame.
    pub tokens: Vec<Token>,
}

/// The kills of one record, a target at a time.
fn shots_of(kills: &[stars_formats::Kill]) -> Vec<Shot> {
    let mut shots: Vec<Shot> = Vec::new();
    for kill in kills {
        let target = usize::from(kill.token);
        match shots.iter_mut().find(|s| s.target == target) {
            Some(shot) => {
                shot.weapon |= kill.weapon;
                shot.ships_killed += u32::from(kill.ships_killed);
            }
            None => shots.push(Shot {
                target,
                weapon: kill.weapon,
                ships_killed: u32::from(kill.ships_killed),
            }),
        }
    }
    shots
}

/// A battle recording, prepared for playback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vcr {
    /// Battle id, as the game numbers it.
    pub id: u16,
    /// The planet it happened at, if any (`0xFFFF` means deep space).
    pub planet: Option<u16>,
    /// Players involved.
    pub players: Vec<u8>,
    frames: Vec<Frame>,
    start: Vec<Token>,
    position: usize,
    /// The token the player has picked out (`viVCRFocus`), whose square
    /// is framed in blue and whose figures the panel shows.
    pub focus: Option<usize>,
}

impl Vcr {
    /// Prepare a recording for playback.
    ///
    /// Every action becomes one frame, in the order the engine wrote them.
    #[must_use]
    pub fn new(record: &BattleRecord) -> Self {
        let start: Vec<Token> = record
            .tokens
            .iter()
            .enumerate()
            .map(|(i, t)| Token::from_record(i, t))
            .collect();

        let mut tokens = start.clone();
        let mut frames = Vec::with_capacity(record.actions.len());

        for action in &record.actions {
            let actor = usize::from(action.token);
            let event = match action.destination {
                // A token leaving the battle.
                None => {
                    if let Some(t) = tokens.get_mut(actor) {
                        t.active = false;
                        t.square = None;
                    }
                    Event::Disengage { token: actor }
                }
                Some(dest) => {
                    let to = (dest.x, dest.y);
                    let from = tokens.get(actor).and_then(|t| t.square);
                    match from {
                        // The record repeats the token's own square: a shot.
                        Some(here) if here == to => Event::Fire {
                            attacker: actor,
                            target: usize::from(action.target),
                            range: action.range,
                            ships_killed: action
                                .kills
                                .iter()
                                .map(|k| u32::from(k.ships_killed))
                                .sum(),
                            shots: shots_of(&action.kills),
                        },
                        Some(here) => {
                            if let Some(t) = tokens.get_mut(actor) {
                                t.square = Some(to);
                            }
                            Event::Move {
                                token: actor,
                                from: here,
                                to,
                            }
                        }
                        // A token that has already left cannot move; treat the
                        // record as a shot from where it stood.
                        None => Event::Fire {
                            attacker: actor,
                            target: usize::from(action.target),
                            range: action.range,
                            ships_killed: 0,
                            shots: shots_of(&action.kills),
                        },
                    }
                }
            };

            // Casualties ride the action, whatever kind it is.
            for kill in &action.kills {
                let Some(hit) = tokens.get_mut(usize::from(kill.token)) else {
                    continue;
                };
                // Shields are per ship and the recorded figure is the whole
                // pool, so it has to go through the pool — and against the ship
                // count *before* the casualties, as `apply_damage` does.
                let pool = hit.shields * hit.ships;
                let left = (pool - i32::from(kill.shield_damage)).max(0);
                if hit.ships > 0 {
                    hit.shields = left / hit.ships;
                }
                hit.ships = (hit.ships - i32::from(kill.ships_killed)).max(0);
                if hit.ships == 0 {
                    hit.active = false;
                    hit.shields = 0;
                }
            }

            frames.push(Frame {
                round: action.round,
                event,
                tokens: tokens.clone(),
            });
        }

        Self {
            id: record.id,
            planet: (record.planet != u16::MAX).then_some(record.planet),
            players: record.participants(),
            frames,
            start,
            position: 0,
            focus: None,
        }
    }

    /// How many frames the battle has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Whether the recording carries no actions at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Where playback currently sits: `0` is before the first frame.
    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }

    /// The board as it stands now.
    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        match self.position.checked_sub(1) {
            None => &self.start,
            Some(i) => &self.frames[i].tokens,
        }
    }

    /// The frame just played, or `None` at the start.
    #[must_use]
    pub fn frame(&self) -> Option<&Frame> {
        self.position.checked_sub(1).map(|i| &self.frames[i])
    }

    /// The round playback is in.
    #[must_use]
    pub fn round(&self) -> u8 {
        self.frame().map_or(0, |f| f.round)
    }

    /// Play one frame. Returns whether anything was left to play.
    pub fn step(&mut self) -> bool {
        if self.position >= self.frames.len() {
            return false;
        }
        self.position += 1;
        true
    }

    /// Rewind one frame. Returns whether anything was left to rewind.
    pub fn back(&mut self) -> bool {
        if self.position == 0 {
            return false;
        }
        self.position -= 1;
        true
    }

    /// Jump to a frame, clamped to the recording.
    pub fn seek(&mut self, frame: usize) {
        self.position = frame.min(self.frames.len());
    }

    /// Play to the end.
    pub fn end(&mut self) {
        self.position = self.frames.len();
    }

    /// Rewind to the start.
    pub fn rewind(&mut self) {
        self.position = 0;
    }

    /// Every frame, for a renderer that wants to draw a timeline.
    #[must_use]
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// Ships each player has lost so far, indexed by player number.
    ///
    /// Played to the end this must agree with the recording's own totals,
    /// which is what `plays_back_to_the_recorded_casualties` asserts.
    #[must_use]
    pub fn losses(&self) -> Vec<(u8, i32)> {
        let mut out: Vec<(u8, i32)> = Vec::new();
        for (token, now) in self.start.iter().zip(self.tokens()) {
            let lost = token.ships - now.ships;
            if lost <= 0 {
                continue;
            }
            match out.iter_mut().find(|(p, _)| *p == token.player) {
                Some((_, total)) => *total += lost,
                None => out.push((token.player, lost)),
            }
        }
        out.sort_unstable();
        out
    }

    /// The board as a grid of the tokens standing on each square.
    ///
    /// Indexed `[y][x]`, so it can be drawn a row at a time.
    #[must_use]
    pub fn board(&self) -> Vec<Vec<Vec<usize>>> {
        let mut grid = vec![vec![Vec::new(); usize::from(BOARD)]; usize::from(BOARD)];
        for token in self.tokens() {
            let Some((x, y)) = token.square else { continue };
            if !token.active || token.ships <= 0 {
                continue;
            }
            if let Some(cell) = grid
                .get_mut(usize::from(y))
                .and_then(|row| row.get_mut(usize::from(x)))
            {
                cell.push(token.index);
            }
        }
        grid
    }
}

// --- The transport ---------------------------------------------------------
//
// `EnableVCRButtons` (`10e8:48f6`) decides which of the five buttons are alive
// and where the focus goes, and it does both from one number: `viStepVCRCur`,
// which runs from **-1** — the board before anything has happened — to
// `vcStepVCR`. This module's [`Vcr::position`] is that number plus one, so
// zero is the same "before the first move" state.

impl Vcr {
    /// Whether the two **backward** buttons are alive.
    ///
    /// The original's test is `(unsigned)viStepVCRCur < 0x8000`, which is a
    /// signed `>= 0` written the short way — so they come alive as soon as
    /// anything has been played.
    #[must_use]
    pub fn can_rewind(&self) -> bool {
        self.position > 0
    }

    /// Whether the three **forward** buttons are alive: `viStepVCRCur <
    /// vcStepVCR`, so they die on the last frame rather than after it.
    #[must_use]
    pub fn can_advance(&self) -> bool {
        self.position < self.frames.len()
    }

    /// Which control `EnableVCRButtons` leaves the focus on.
    ///
    /// At either end the original moves it by hand, because the button under
    /// the pointer has just been disabled: at the start it goes to **play**,
    /// and at the end to **Done**. In between it leaves it where it is.
    #[must_use]
    pub fn focus(&self) -> Option<u16> {
        if self.position == 0 {
            Some(0xa3)
        } else if self.position == self.frames.len() {
            Some(0x1)
        } else {
            None
        }
    }
}
