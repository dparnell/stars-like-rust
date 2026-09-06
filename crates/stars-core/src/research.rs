//! Research: technology levels, what each level costs, and the annual advance.
//!
//! Source: `GetTechLevelCost` (`10d8:1dba`) and `UpdateResearchStatus`
//! (`10b8:80fe`) in `stars.2.7j.exe`, cross-checked against the reconstructed
//! NB09 C (`research.c`, `turn2.c`) and `MANUAL.PDF` pp. 8-5..8-6. Full
//! derivation in `docs/formulas/research.md`.

use crate::race::{lrt, Prt, Race, RaceStat};

/// The six technology fields, in the order they are stored.
pub const TECH_FIELDS: usize = 6;

/// The highest technology level the game allows.
pub const MAX_TECH_LEVEL: u8 = 26;

/// A technology field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum TechField {
    /// Energy.
    Energy = 0,
    /// Weapons.
    Weapons = 1,
    /// Propulsion.
    Propulsion = 2,
    /// Construction.
    Construction = 3,
    /// Electronics.
    Electronics = 4,
    /// Biotechnology.
    Biotechnology = 5,
}

impl TechField {
    /// The six fields, in the order the game lists them — which is also the
    /// order the Research dialog's radio buttons and the six research levels
    /// are in.
    pub const ALL: [TechField; TECH_FIELDS] = [
        TechField::Energy,
        TechField::Weapons,
        TechField::Propulsion,
        TechField::Construction,
        TechField::Electronics,
        TechField::Biotechnology,
    ];

    /// What the game calls it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            TechField::Energy => "Energy",
            TechField::Weapons => "Weapons",
            TechField::Propulsion => "Propulsion",
            TechField::Construction => "Construction",
            TechField::Electronics => "Electronics",
            TechField::Biotechnology => "Biotechnology",
        }
    }

    /// The short form the Technology Status box uses — `Ener:`, `Weap:` and
    /// so on (strings 91 to 96).
    #[must_use]
    pub fn short_name(self) -> &'static str {
        match self {
            TechField::Energy => "Ener",
            TechField::Weapons => "Weap",
            TechField::Propulsion => "Prop",
            TechField::Construction => "Const",
            TechField::Electronics => "Elect",
            TechField::Biotechnology => "Bio",
        }
    }
}

/// Base resource cost of reaching each technology level, before the
/// per-field and whole-empire adjustments.
///
/// Verified byte-for-byte against `rglTechCost` at `10d8:1d4e` in our binary.
/// The manual describes it as "increasing in a Fibonacci type series"
/// (p. 8-5), which holds up to about level 12 before the series flattens.
pub const TECH_LEVEL_COST: [i32; 27] = [
    0, 50, 80, 130, 210, 340, 550, 890, 1440, 2330, 3770, 6100, 9870, 13850, 18040, 22440, 27050,
    31870, 36900, 42140, 47590, 53250, 59120, 65200, 71490, 77990, 84700,
];

/// What a player wants to research after the current field completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextField {
    /// Switch to this specific field.
    Field(usize),
    /// Stay on the current field (stored as `6`).
    Same,
    /// Switch to whichever field is currently lowest (stored as `7`).
    Lowest,
}

impl NextField {
    /// Decode the high nibble of the game's `iTechCur` byte.
    #[must_use]
    pub fn from_raw(v: u8) -> Self {
        match v {
            6 => Self::Same,
            7 => Self::Lowest,
            f => Self::Field(usize::from(f).min(TECH_FIELDS - 1)),
        }
    }
}

/// A player's research state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Research {
    /// Current level in each field.
    pub levels: [u8; TECH_FIELDS],
    /// Resources accumulated toward the next level in each field.
    pub points: [i32; TECH_FIELDS],
    /// The field currently being researched.
    pub current_field: usize,
    /// What to research once the current field completes.
    pub next_field: NextField,
}

impl Default for Research {
    fn default() -> Self {
        Self {
            levels: [0; TECH_FIELDS],
            points: [0; TECH_FIELDS],
            current_field: 0,
            next_field: NextField::Same,
        }
    }
}

impl Research {
    /// Sum of all six levels, the quantity the "added cost of research" is
    /// charged against.
    #[must_use]
    pub fn total_levels(&self) -> i32 {
        self.levels.iter().map(|l| i32::from(*l)).sum()
    }
}

/// A technology breakthrough, reported so callers can message the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breakthrough {
    /// The field that advanced.
    pub field: usize,
    /// The level reached.
    pub level: u8,
}

/// The resource cost of reaching `level` in `field`.
///
/// Three things move the base table value:
///
/// * the **added cost of research** — 10 resources per level already held,
///   across every field, which is why researching everything costs the same in
///   the end (`MANUAL.PDF` p. 8-5);
/// * the race's per-field research setting — `0` costs 75% extra, `1` is
///   normal, `2` halves it;
/// * the game's "slower tech advances" option, which doubles everything.
#[must_use]
pub fn tech_level_cost(
    field: usize,
    level: u8,
    research: &Research,
    race: &Race,
    slow_tech: bool,
) -> i32 {
    let index = usize::from(level).min(TECH_LEVEL_COST.len() - 1);
    let mut cost = research.total_levels() * 10 + TECH_LEVEL_COST[index];

    // rsTechBonus1..6 are the six per-field research settings.
    let setting = race.attrs[RaceStat::TechBonus1 as usize + field];
    match setting.cmp(&1) {
        // Stored 0: cost * 2 - cost/4, i.e. 75% extra.
        std::cmp::Ordering::Less => cost = cost * 2 - (cost >> 2),
        // Stored 2: half price.
        std::cmp::Ordering::Greater => cost /= 2,
        std::cmp::Ordering::Equal => {}
    }

    if slow_tech {
        cost *= 2;
    }
    cost
}

/// Add a year's research resources to a player and spend them.
///
/// `resources` is what production allocated to research this year
/// (`lResLastYear`). Returns every level gained, in the order they happened.
///
/// Generalized Research splits the budget: half to the current field and 15%
/// of the whole to each of the other five (`MANUAL.PDF` p. 8-6). Note that
/// this means such a race spends 125% of its research budget in total, which
/// is deliberate — the trait buys breadth, and the 50% to the primary field is
/// the price.
pub fn add_research(
    research: &mut Research,
    race: &Race,
    resources: i32,
    slow_tech: bool,
) -> Vec<Breakthrough> {
    let current = research.current_field.min(TECH_FIELDS - 1);

    if race.has_lrt(lrt::GENERALIZED_RESEARCH) {
        // Half to the primary field, rounding up.
        research.points[current] += (resources + 1) / 2;
        // 15% of the budget to every other field, rounding up.
        let share = (resources * 3 + 19) / 20;
        for (field, points) in research.points.iter_mut().enumerate() {
            if field != current {
                *points += if slow_tech { share / 2 } else { share };
            }
        }
    } else {
        research.points[current] += resources;
    }

    spend_research(research, race, slow_tech)
}

/// Spend whatever each field has accumulated, levelling up as far as it goes.
///
/// Field switching is handled here: when the field being researched completes
/// a level and the player has asked for a different field next, the remaining
/// resources move with them.
fn spend_research(research: &mut Research, race: &Race, slow_tech: bool) -> Vec<Breakthrough> {
    let mut gained = Vec::new();

    // Bounded to keep a pathological state from looping forever; six fields of
    // 26 levels cannot legitimately exceed this.
    for _ in 0..(TECH_FIELDS * usize::from(MAX_TECH_LEVEL) + TECH_FIELDS) {
        let mut advanced = false;

        for field in 0..TECH_FIELDS {
            if research.levels[field] >= MAX_TECH_LEVEL {
                continue;
            }
            let cost =
                tech_level_cost(field, research.levels[field] + 1, research, race, slow_tech);
            if research.points[field] < cost {
                continue;
            }

            research.points[field] -= cost;
            research.levels[field] += 1;
            gained.push(Breakthrough {
                field,
                level: research.levels[field],
            });
            advanced = true;

            // Completing the *current* field can hand research over to another.
            if field == research.current_field {
                if let Some(next) = switch_target(research) {
                    let leftover = research.points[field];
                    research.points[field] = 0;
                    research.points[next] += leftover;
                    research.current_field = next;
                }
            }
            break;
        }

        if !advanced {
            break;
        }
    }

    gained
}

/// Hand a player a level in one field outright, keeping their progress.
///
/// The Mystery Trader gives technology, and `DoThingInteractions`
/// (`1110:10e3`) does it by paying for it rather than by writing a level down:
/// it **doubles** what the field has already accumulated and adds what
/// `CostOfDevelopingItem` (`research.c`) says is still owed — the cost of the
/// next level less what is already spent. The two together come to exactly
/// `cost + spent`, so the level always lands and the player's part-finished
/// research survives it, which is the whole reason for the doubling.
///
/// Returns every level the payment bought, which is normally one.
pub fn grant_level(
    research: &mut Research,
    race: &Race,
    slow_tech: bool,
    field: usize,
) -> Vec<Breakthrough> {
    let field = field.min(TECH_FIELDS - 1);
    if research.levels[field] >= MAX_TECH_LEVEL {
        return Vec::new();
    }
    let spent = research.points[field];
    let cost = tech_level_cost(field, research.levels[field] + 1, research, race, slow_tech);
    // `CostOfDevelopingItem` counts the spending twice over under "slower tech
    // advances", which is what costs such a game its part-finished level.
    let already = if slow_tech { spent * 2 } else { spent };
    research.points[field] = spent * 2 + (cost - already).max(0);
    spend_research(research, race, slow_tech)
}

/// The field to move to after completing a level in the current one, or `None`
/// to stay put.
fn switch_target(research: &Research) -> Option<usize> {
    match research.next_field {
        NextField::Same => None,
        NextField::Field(f) if f == research.current_field => None,
        NextField::Field(f) => Some(f.min(TECH_FIELDS - 1)),
        NextField::Lowest => {
            let mut lowest = 0;
            for field in 1..TECH_FIELDS {
                if research.levels[field] < research.levels[lowest] {
                    lowest = field;
                }
            }
            (lowest != research.current_field).then_some(lowest)
        }
    }
}

/// Research stolen by Super Stealth races from everyone else's spending.
///
/// A Super Stealth player gains `spent / living_players / 2` in every field
/// that anyone spent in this year. Returns the amount granted per field.
#[must_use]
pub fn super_stealth_gain(
    race: &Race,
    field_spending: &[i32; TECH_FIELDS],
    living_players: i32,
) -> [i32; TECH_FIELDS] {
    let mut gain = [0i32; TECH_FIELDS];
    if race.prt() != Some(Prt::Ss) || living_players <= 1 {
        return gain;
    }
    for (field, spent) in field_spending.iter().enumerate() {
        if *spent > 0 {
            let stolen = spent / living_players / 2;
            if stolen > 1 {
                gain[field] = stolen;
            }
        }
    }
    gain
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A level the Mystery Trader gives is paid for, not written down — and the
    /// payment is arranged so that part-finished research survives it.
    #[test]
    fn a_granted_level_keeps_what_was_already_spent() {
        let race = Race::humanoid();
        let mut research = Research::default();
        let cost = tech_level_cost(0, 1, &research, &race, false);
        // Half way to the first level in energy.
        research.points[0] = cost / 2;

        let gained = grant_level(&mut research, &race, false, 0);
        assert_eq!(gained.len(), 1);
        assert_eq!(research.levels[0], 1);
        // The half-level of progress is still there afterwards.
        assert_eq!(research.points[0], cost / 2);
    }
}

/// What is still owed on the level being researched, or `None` at the top.
///
/// The dialog's `Resources needed to complete:` line — the level's cost less
/// what the field has already banked. A field at [`MAX_TECH_LEVEL`] reads
/// `Maxed Out` instead, which is the `None`.
///
/// Source: `DrawResearchDlg` (`10d8:090a`).
#[must_use]
pub fn remaining_cost(
    field: usize,
    research: &Research,
    race: &Race,
    slow_tech: bool,
) -> Option<i32> {
    let field = field.min(TECH_FIELDS - 1);
    let level = research.levels[field];
    if level >= MAX_TECH_LEVEL {
        return None;
    }
    let cost = tech_level_cost(field, level + 1, research, race, slow_tech);
    Some((cost - research.points[field]).max(0))
}

/// How long the level being researched will take, in years.
///
/// `None` is the dialog's **Never**: nothing is being put aside for research,
/// so the level will not arrive. A level that is already paid for reads one
/// year.
///
/// A race with **Generalized Research** only sends half its budget to the
/// field it is studying, so only half counts toward this.
///
/// Source: `DrawResearchDlg`, which divides rounding **up**.
#[must_use]
pub fn years_to_next(remaining: i32, budget: i32, generalized: bool) -> Option<i32> {
    if remaining <= 0 {
        return Some(1);
    }
    if budget <= 0 {
        return None;
    }
    // Generalized Research spreads the budget over every field; half of it
    // reaches the one being studied.
    let toward = if generalized {
        budget - budget / 2
    } else {
        budget
    };
    if toward <= 0 {
        return None;
    }
    Some((remaining + toward - 1) / toward)
}

/// One thing a future level of research will bring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Benefit {
    /// How many levels away it is: `1` is the level being researched now.
    ///
    /// The dialog groups these — the next level in green, the three after it
    /// in red, and everything further off in black — and gathers everything
    /// beyond nine levels into one last group.
    pub levels_away: i16,
    /// The component's category (see [`crate::components::slot`]).
    pub category: u16,
    /// Its index within that category.
    pub item: usize,
    /// What it is called.
    pub name: &'static str,
}

/// What the next levels of research will unlock — the dialog's **Expected
/// Research Benefits**.
///
/// `DrawResearchDlg` walks every component in every category, asks
/// `FLookupPart` how far away it is, and lists those between one and nine
/// levels off, nearest first. The distance is `TechStatus`' answer less one,
/// so a component one level away comes back as `1`; everything ten or more
/// levels off is reported as nine, which is the group the dialog draws last.
///
/// **It is not a preview of one field.** The original sets the player's
/// current field to whichever radio button is selected before it asks, and
/// puts it back afterwards — but that makes no difference to the answer.
/// `TechStatus` only consults the current field to decide between its
/// `LookupNear` (2) and the general `(need - have) + 1`, and for a component
/// one level short those are the same number; anything short in more than one
/// field is reported as unreachable whatever is being studied. So the list is
/// the same for every field, and it is a list of what research in general is
/// about to bring, sorted by how soon.
///
/// A component the player's traits forbid outright never appears, however
/// close its technology is.
#[must_use]
pub fn expected_benefits(who: &crate::parts::Builder<'_>) -> Vec<Benefit> {
    use crate::components::slot;
    use crate::parts::Availability;

    /// How far ahead the dialog looks before lumping the rest together.
    const HORIZON: i16 = 9;

    let who = *who;
    let mut out = Vec::new();
    for category in crate::parts::CATEGORY_ORDER.iter().copied().chain([
        slot::HULL,
        slot::SB_HULL,
        slot::PLANETARY,
    ]) {
        for item in 0.. {
            let away = match crate::parts::availability(&who, category, item) {
                Availability::Missing => break,
                // One level off in the field being studied.
                Availability::Nearly => 1,
                Availability::Levels(n) => n - 1,
                // Already buildable, or a trait forbids it outright.
                Availability::Available | Availability::Forbidden => continue,
                Availability::Far => continue,
            };
            if away < 1 {
                continue;
            }
            let Some(part) = crate::parts::part(category, item) else {
                continue;
            };
            out.push(Benefit {
                levels_away: away.min(HORIZON),
                category,
                item,
                name: part.name,
            });
        }
    }
    out.sort_by_key(|b| b.levels_away);
    out
}
