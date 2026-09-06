//! Who has won.
//!
//! A game is set up with some of ten victory conditions switched on, and a
//! number of them a player must meet. Every year the host works out which
//! conditions each player has met — those are the flags the scoreboard carries
//! — and, once the game is old enough, declares anybody meeting enough of them
//! a winner.
//!
//! From `UpdatePlayerScores` (`10b8:6258`); the conditions themselves and the
//! sliders behind them are decoded in [`stars_formats::GameInfo`]. See
//! `docs/formulas/scores.md`.

use crate::score::{unpack, PlayerScore};
use crate::GameState;
use stars_formats::victory;

/// Which conditions a player meets, as the scoreboard's flag bits.
///
/// The bit positions are the ones `docs/formats/score.md` lists, so this word
/// can go straight into a score block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Met {
    /// The flag bits, `1 << 6` for planets through `1 << 12` for the
    /// longest-highest-score condition.
    pub bits: u16,
    /// How many of the conditions the game is **using** this player meets,
    /// which is what decides a win.
    pub count: u16,
}

/// Bit position in the scoreboard word for each condition.
const BIT: [u16; 8] = [
    1 << 6,  // planet control
    1 << 7,  // tech level in so many fields
    0,       // tech fields: part of the one above
    1 << 8,  // score
    1 << 9,  // exceeds second place
    1 << 10, // production
    1 << 11, // capital ships
    1 << 12, // highest score after so many years
];

/// The scoreboard flag for one condition, or `0` for the two that are settings
/// rather than conditions.
#[must_use]
pub fn bit(condition: usize) -> u16 {
    BIT.get(condition).copied().unwrap_or(0)
}

/// Whether the game is playing for a condition (`GetVCCheck`).
#[must_use]
pub fn active(settings: &[u8; victory::COUNT], condition: usize) -> bool {
    Settings(settings).active(condition)
}

/// What a condition is set to (`GetVCVal`).
///
/// The byte in the file is a **slider position**, not the threshold; this is
/// the threshold it stands for.
#[must_use]
pub fn value(settings: &[u8; victory::COUNT], condition: usize) -> i32 {
    Settings(settings).value(condition)
}

/// Which victory conditions a player meets.
///
/// Each is only counted toward a win when the game is **using** it, but the
/// flag is set either way — the original sets the bit first and asks
/// `GetVCCheck` second, which is why a scoreboard can show a condition met in a
/// game that is not playing for it.
///
/// `second_score` is the runner-up's score, which only the "exceeds second
/// place" condition needs; pass the leader's own score when there is no second
/// player, as the original does.
#[must_use]
pub fn met(
    state: &GameState,
    player: usize,
    score: &PlayerScore,
    leader: bool,
    second_score: i32,
) -> Met {
    let info = Settings(&state.victory);
    let mut out = Met::default();
    let mut mark = |condition: usize, is_met: bool| {
        if !is_met {
            return;
        }
        out.bits |= BIT[condition];
        if info.active(condition) {
            out.count += 1;
        }
    };

    // Owns a share of every planet in the galaxy.
    let wanted =
        i64::from(state.galaxy_planets) * i64::from(info.value(victory::PLANET_CONTROL)) / 100;
    mark(
        victory::PLANET_CONTROL,
        state.galaxy_planets > 0 && i64::from(score.planets) >= wanted,
    );

    // A tech level, in so many fields.
    let level = info.value(victory::TECH_LEVEL);
    let fields = state.players.get(player).map_or(0, |p| {
        p.research
            .levels
            .iter()
            .filter(|l| i32::from(**l) >= level)
            .count()
    });
    mark(
        victory::TECH_LEVEL,
        i32::try_from(fields).unwrap_or(0) >= info.value(victory::TECH_FIELDS),
    );

    mark(victory::SCORE, score.score >= info.value(victory::SCORE));

    // Exceeds the second player by a margin. The original only asks this of the
    // leader, and only while more than one player is still alive.
    let margin = i64::from(second_score) * i64::from(100 + info.value(victory::SCORE_EXCESS)) / 100;
    mark(
        victory::SCORE_EXCESS,
        leader && i64::from(score.score) >= margin,
    );

    mark(
        victory::PRODUCTION,
        score.resources / 1000 >= info.value(victory::PRODUCTION),
    );

    // The capital-ship count is compared as the scoreboard stores it, packed
    // and unpacked again, so a large fleet is compared roughly.
    mark(
        victory::CAPITAL_SHIPS,
        unpack(crate::score::pack(score.capital_ships)) >= info.value(victory::CAPITAL_SHIPS),
    );

    mark(
        victory::HIGH_SCORE_AT,
        leader && i32::from(state.turn) >= info.value(victory::HIGH_SCORE_AT),
    );

    out
}

/// Work out the whole scoreboard's victory flags, and who has won.
///
/// A win needs three things: the year to have reached the game's "least years",
/// the game to require at least one condition, and the player to meet that
/// many. The original also ends the game when only one player is left alive,
/// which is checked here too.
///
/// Returns the flags per player and the winners.
#[must_use]
pub fn resolve(state: &GameState, scores: &[PlayerScore]) -> (Vec<Met>, Vec<usize>) {
    let info = Settings(&state.victory);
    let best = scores.iter().map(|s| s.score).max().unwrap_or(0);
    let leaders = scores.iter().filter(|s| s.score == best).count();
    let second = scores
        .iter()
        .map(|s| s.score)
        .filter(|s| *s != best)
        .max()
        .unwrap_or(best);

    let flags: Vec<Met> = scores
        .iter()
        .enumerate()
        .map(|(player, score)| {
            // Only a sole leader can "exceed second place" or "hold the highest
            // score": a tie is nobody's win.
            let leader = score.score == best && leaders == 1;
            met(state, player, score, leader, second)
        })
        .collect();

    let alive = state.players.iter().filter(|p| !p.dead).count();
    let mut winners: Vec<usize> = Vec::new();
    if alive == 1 && state.players.len() > 1 {
        // Everybody else is dead, so the survivor has won however the game was
        // set up.
        winners.extend(state.players.iter().position(|p| !p.dead));
    } else {
        let needed = info.value(victory::MUST_MEET);
        if needed > 0 && i32::from(state.turn) >= info.value(victory::LEAST_YEARS) {
            for (player, flag) in flags.iter().enumerate() {
                if i32::from(flag.count) >= needed
                    && state.players.get(player).is_some_and(|p| !p.dead)
                {
                    winners.push(player);
                }
            }
        }
    }
    (flags, winners)
}

/// The game's victory settings, read the way `GetVCVal` and `GetVCCheck` read
/// them.
struct Settings<'a>(&'a [u8; victory::COUNT]);

impl Settings<'_> {
    fn active(&self, condition: usize) -> bool {
        self.0.get(condition).is_some_and(|b| b & 0x80 != 0)
    }

    fn value(&self, condition: usize) -> i32 {
        let raw = i32::from(self.0.get(condition).copied().unwrap_or(0) & 0x7F);
        match condition {
            victory::PLANET_CONTROL => raw * 5 + 20,
            victory::TECH_LEVEL => raw + 8,
            victory::TECH_FIELDS => raw + 2,
            victory::SCORE => raw * 1000 + 1000,
            victory::SCORE_EXCESS => raw * 10 + 20,
            victory::PRODUCTION | victory::CAPITAL_SHIPS => raw * 10 + 10,
            victory::HIGH_SCORE_AT | victory::LEAST_YEARS => raw * 10 + 30,
            victory::MUST_MEET => {
                let active = (0..8)
                    .filter(|c| *c != victory::TECH_FIELDS)
                    .filter(|c| self.active(*c))
                    .count();
                raw.min(i32::try_from(active).unwrap_or(raw))
            }
            _ => raw,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race::Race;
    use crate::Player;

    /// A game playing for two conditions, needing one of them, from year 30.
    fn a_game() -> GameState {
        let mut state = GameState::new(1);
        state.players = vec![Player::new(Race::humanoid()), Player::new(Race::humanoid())];
        state.galaxy_planets = 100;
        state.turn = 40;
        state.victory[victory::PLANET_CONTROL] = 0x80; // active, 20%
        state.victory[victory::SCORE] = 0x80; // active, 1000
        state.victory[victory::MUST_MEET] = 1;
        state.victory[victory::LEAST_YEARS] = 0; // year 30
        state
    }

    fn score(score: i32, planets: i32) -> PlayerScore {
        PlayerScore {
            score,
            planets,
            ..PlayerScore::default()
        }
    }

    /// Meeting one of the two conditions the game asks for is a win.
    #[test]
    fn meeting_enough_conditions_wins() {
        let state = a_game();
        let scores = vec![score(1200, 5), score(100, 3)];
        let (flags, winners) = resolve(&state, &scores);
        assert!(flags[0].bits & (1 << 8) != 0, "over a thousand points");
        assert_eq!(flags[0].count, 1);
        assert_eq!(winners, vec![0]);
        assert_eq!(flags[1].count, 0);
    }

    /// A condition the game is not playing for still shows on the scoreboard,
    /// but does not count toward a win — which is what the original does, and
    /// why a scoreboard can show a condition met in a game nobody can win that
    /// way.
    #[test]
    fn an_unused_condition_shows_but_does_not_count() {
        let mut state = a_game();
        state.victory[victory::SCORE] = 0x00; // switched off, threshold 1000
        let scores = vec![score(1200, 1), score(100, 1)];
        let (flags, winners) = resolve(&state, &scores);
        assert!(flags[0].bits & (1 << 8) != 0, "the flag is still set");
        assert_eq!(flags[0].count, 0, "but it counts for nothing");
        assert!(winners.is_empty());
    }

    /// Nobody wins before the game is old enough, however well they are doing.
    #[test]
    fn a_win_waits_for_the_years_to_pass() {
        let mut state = a_game();
        state.turn = 20; // the earliest is 30
        let scores = vec![score(5000, 90), score(1, 1)];
        let (_, winners) = resolve(&state, &scores);
        assert!(winners.is_empty());
    }

    /// A tie for the lead is nobody's lead: neither of the two conditions that
    /// compare players can be met while the top score is shared.
    #[test]
    fn a_tie_is_nobody_s_lead() {
        let mut state = a_game();
        state.victory[victory::HIGH_SCORE_AT] = 0x80;
        let scores = vec![score(500, 1), score(500, 1)];
        let (flags, _) = resolve(&state, &scores);
        assert_eq!(flags[0].bits & (1 << 12), 0);
        assert_eq!(flags[1].bits & (1 << 12), 0);
    }

    /// The last player standing wins whatever the game was set up for.
    #[test]
    fn the_survivor_wins() {
        let mut state = a_game();
        state.victory = [0; victory::COUNT]; // nothing to play for
        state.players[1].dead = true;
        let scores = vec![score(10, 1), score(0, 0)];
        let (_, winners) = resolve(&state, &scores);
        assert_eq!(winners, vec![0]);
    }
}
