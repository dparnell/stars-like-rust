//! Which components a player may build.
//!
//! Every component table in [`crate::components`] lists what *exists*; this
//! module answers the narrower question the ship designer asks: of those, which
//! ones may **this** player put on a hull? Three things can stand in the way —
//! a primary racial trait that reserves a component for somebody else, a lesser
//! trait that forbids it, and technology not yet researched.
//!
//! Source: `FLookupPart` (`1008:524e`), its tech gate `TechStatus`
//! (`1008:6148`) and `FShouldPartBeHidden` (`research.c`). The designer's parts
//! list is exactly the components this returns [`Availability::Available`] for
//! — see `docs/ui/ship-design.md`.

use crate::components::{
    slot, TechRequirement, ARMORS, BEAMS, BOMBS, ENGINES, HULLS, MINE_LAYERS, MINING, PLANETARY,
    SCANNERS, SHIELDS, SPECIALS_E, SPECIALS_M, SPECIALS_SB, STARBASE_HULLS, TECH_FIELDS,
    TERRAFORMING, TORPEDOES,
};
use crate::race::{lrt, Prt, Race};
use crate::wormhole::part as trader;

/// What a player can do with one component right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// The category has no component at that index — the end of a table.
    Missing,
    /// A racial trait puts it out of reach permanently.
    Forbidden,
    /// Buildable now.
    Available,
    /// One level short, in the field being researched — so it arrives next.
    Nearly,
    /// Exactly one field is short. The figure is the game's own: the number of
    /// levels missing **plus one**, which is what `TechStatus` returns and what
    /// the research screen counts down.
    Levels(i16),
    /// More than one field is short.
    Far,
}

impl Availability {
    /// Whether the component can go on a hull today.
    #[must_use]
    pub fn is_available(self) -> bool {
        self == Availability::Available
    }
}

/// The header every component table shares, flattened so callers do not have to
/// match on the category to read a name or a cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part {
    /// The category it came from (see [`crate::components::slot`]).
    pub category: u16,
    /// Its index within that category's table.
    pub item: usize,
    /// The name the game shows.
    pub name: &'static str,
    /// Levels required in each of the six fields.
    pub tech: TechRequirement,
    /// Mass in kT — for a hull, the mass of the bare hull.
    pub mass: i32,
    /// Resources to build one.
    pub resource_cost: i32,
    /// Minerals to build one: ironium, boranium, germanium.
    pub ore_cost: [i32; 3],
}

/// Look one component up by category and index, without asking who is building
/// it.
#[must_use]
pub fn part(category: u16, item: usize) -> Option<Part> {
    macro_rules! header {
        ($table:expr, $mass:ident) => {{
            let p = $table.get(item)?;
            Part {
                category,
                item,
                name: p.name,
                tech: p.tech,
                mass: i32::from(p.$mass),
                resource_cost: i32::from(p.resource_cost),
                ore_cost: [
                    i32::from(p.ore_cost[0]),
                    i32::from(p.ore_cost[1]),
                    i32::from(p.ore_cost[2]),
                ],
            }
        }};
    }

    // Matched exactly, not as a bitmask: a hull slot's category names several
    // tables at once, so a caller that wants "the part in this slot" has to
    // pick the table itself.
    Some(match category {
        slot::ENGINE => header!(ENGINES, mass),
        slot::SCANNER => header!(SCANNERS, mass),
        slot::SHIELD => header!(SHIELDS, mass),
        slot::ARMOR => header!(ARMORS, mass),
        slot::BEAM => header!(BEAMS, mass),
        slot::TORPEDO => header!(TORPEDOES, mass),
        slot::BOMB => header!(BOMBS, mass),
        slot::MINING => header!(MINING, mass),
        slot::MINES => header!(MINE_LAYERS, mass),
        slot::SPECIAL_SB => header!(SPECIALS_SB, mass),
        slot::SPECIAL_E => header!(SPECIALS_E, mass),
        slot::SPECIAL_M => header!(SPECIALS_M, mass),
        slot::TERRA => header!(TERRAFORMING, mass),
        slot::PLANETARY => header!(PLANETARY, mass),
        slot::HULL => header!(HULLS, empty_mass),
        slot::SB_HULL => header!(STARBASE_HULLS, empty_mass),
        _ => return None,
    })
}

/// Every category the ship designer's parts list can show, in the order the
/// original's filter dropdown lists them (`rggrbitParts`, `10c8:0038`, with the
/// names at `rgidsParts`).
///
/// The first entry is the "All" filter: a mask of the other twelve.
pub const SHIP_FILTERS: [(u16, &str); 13] = [
    (0x19ff, "All"),
    (slot::ARMOR, "Armor"),
    (slot::BEAM, "Beam Weapons"),
    (slot::BOMB, "Bombs"),
    (slot::SPECIAL_E, "Electrical"),
    (slot::ENGINE, "Engines"),
    (slot::SPECIAL_M, "Mechanical"),
    (slot::MINES, "Mine Layers"),
    (slot::MINING, "Mining Robots"),
    (slot::SCANNER, "Scanners"),
    (slot::SHIELD, "Shields"),
    (slot::TORPEDO, "Torpedoes"),
    (slot::BEAM | slot::TORPEDO, "Weapons"),
];

/// The same list for a starbase (`rggrbitPartsSB`, `10c8:006c`), which is
/// shorter because a starbase takes no engines, bombs, mining robots, mine
/// layers or scanners.
pub const STARBASE_FILTERS: [(u16, &str); 8] = [
    (0x0a3c, "All"),
    (slot::ARMOR, "Armor"),
    (slot::BEAM, "Beam Weapons"),
    (slot::SPECIAL_E, "Electrical"),
    (slot::SPECIAL_SB, "Orbital"),
    (slot::SHIELD, "Shields"),
    (slot::TORPEDO, "Torpedoes"),
    (slot::BEAM | slot::TORPEDO, "Weapons"),
];

/// The categories that make up the "All" filter, in the order the original
/// walks them: lowest bit first, which is why the list reads engines, scanners,
/// shields, armour, beams, torpedoes, bombs, mining robots, mine layers,
/// orbital, electrical, mechanical.
///
/// `FillBuildPartsLB` starts at `hstEngine` and shifts left until the mask runs
/// out, so a filter's parts always come out in category order and then in table
/// order.
pub const CATEGORY_ORDER: [u16; 13] = [
    slot::ENGINE,
    slot::SCANNER,
    slot::SHIELD,
    slot::ARMOR,
    slot::BEAM,
    slot::TORPEDO,
    slot::BOMB,
    slot::MINING,
    slot::MINES,
    slot::SPECIAL_SB,
    slot::SPECIAL_E,
    slot::SPECIAL_M,
    slot::TERRA,
];

/// What the player needs before a component appears in the designer.
///
/// Everything `FLookupPart` reads about the player, gathered in one place so
/// callers can ask about a hypothetical race as easily as about a live one.
#[derive(Debug, Clone, Copy)]
pub struct Builder<'a> {
    /// The race asking.
    pub race: &'a Race,
    /// Level reached in each of the six fields.
    pub levels: [u8; TECH_FIELDS],
    /// The field currently being researched, which is the one
    /// [`Availability::Nearly`] is about.
    pub researching: usize,
    /// Which Mystery Trader parts this player has been given
    /// (`PLAYER.grbitTrader`), as a mask of [`crate::wormhole::part`].
    pub trader_parts: u16,
    /// Whether the game is in starbase mode. Only [`slot::SPECIAL_E`] cares:
    /// the designer drops the Tachyon Detector and the Anti-matter Generator
    /// from a starbase's parts list.
    pub starbase: bool,
}

impl<'a> Builder<'a> {
    /// The builder for one of the players in a game.
    #[must_use]
    pub fn player(player: &'a crate::Player) -> Self {
        Self {
            race: &player.race,
            levels: player.research.levels,
            researching: player.research.current_field,
            trader_parts: player.trader_parts,
            starbase: false,
        }
    }

    /// The same builder, designing a starbase rather than a ship.
    #[must_use]
    pub fn designing_starbase(mut self, starbase: bool) -> Self {
        self.starbase = starbase;
        self
    }
}

/// Whether this player may build one component, and if not, why not.
#[must_use]
pub fn availability(who: &Builder<'_>, category: u16, item: usize) -> Availability {
    let Some(p) = part(category, item) else {
        return Availability::Missing;
    };
    if forbidden(who, category, item) {
        return Availability::Forbidden;
    }
    tech_status(&p.tech, &who.levels, who.researching)
}

/// One thing a component asks of the race that builds it.
///
/// `FLookupPart` states these as a long run of conditions; naming them lets
/// the Technology Browser explain *why* something is out of reach using the
/// same list the gate is enforced from, so the two cannot drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    /// Only this primary racial trait may build it.
    Prt(Prt),
    /// Only a race with one of these two may build it.
    EitherPrt(Prt, Prt),
    /// This primary racial trait may **not** build it.
    NotPrt(Prt),
    /// It needs this lesser racial trait.
    Lrt(u32),
    /// This lesser racial trait rules it out.
    NotLrt(u32),
    /// The Mystery Trader has to hand it over first.
    Trader(u16),
}

impl Requirement {
    /// Whether this race satisfies it.
    #[must_use]
    pub fn met(self, race: &Race, trader_parts: u16) -> bool {
        match self {
            Requirement::Prt(p) => race.prt() == Some(p),
            Requirement::EitherPrt(a, b) => race.prt() == Some(a) || race.prt() == Some(b),
            Requirement::NotPrt(p) => race.prt() != Some(p),
            Requirement::Lrt(bit) => race.has_lrt(bit),
            Requirement::NotLrt(bit) => !race.has_lrt(bit),
            Requirement::Trader(bit) => trader_parts & bit != 0,
        }
    }
}

/// Everything `FLookupPart` (`1008:524e`) asks of a race before it reaches the
/// technology check.
///
/// A component with no entries here is open to everybody.
#[must_use]
pub fn requirements(category: u16, item: usize) -> Vec<Requirement> {
    use Requirement::{EitherPrt, Lrt, NotLrt, NotPrt, Prt as Needs, Trader};

    let mut out = Vec::new();
    if let Some(bit) = trader_gift(category, item) {
        out.push(Trader(bit));
    }

    match category {
        slot::ENGINE => {
            // The Settler's Delight is Hyper Expansion's alone; the ramscoops
            // go with the trait that allows them; and the Interspace-10 exists
            // only as the consolation prize for a race that has none.
            if item == 0 {
                out.push(Needs(Prt::He));
            }
            if (10..=15).contains(&item) {
                out.push(NotLrt(lrt::NO_RAMSCOOPS));
            }
            if item == 15 || item == 2 {
                out.push(Lrt(lrt::IFE));
            }
            if item == 7 {
                out.push(Lrt(lrt::NO_RAMSCOOPS));
            }
        }
        slot::SCANNER => {
            if matches!(item, 7 | 8 | 12) {
                out.push(NotLrt(lrt::NO_ADV_SCANNER));
            }
            if matches!(item, 5 | 6 | 14) {
                out.push(Needs(Prt::Ss));
            }
        }
        slot::SHIELD => {
            if item == 4 {
                out.push(Needs(Prt::Ss));
            }
            if item == 3 {
                out.push(Needs(Prt::Is));
            }
        }
        slot::ARMOR => {
            if item == 7 {
                out.push(Needs(Prt::Ss));
            }
            if item == 6 {
                out.push(Needs(Prt::Is));
            }
        }
        slot::BEAM => {
            if item == 2 {
                out.push(Needs(Prt::Is));
            }
            if matches!(item, 14 | 16) {
                out.push(Needs(Prt::Wm));
            }
        }
        slot::BOMB => {
            if (10..=14).contains(&item) {
                out.push(NotPrt(Prt::Is));
            }
            if item == 9 {
                out.push(Needs(Prt::Ca));
            }
        }
        slot::MINING => {
            if matches!(item, 0 | 2 | 3 | 4 | 5) {
                out.push(NotLrt(lrt::OBRM));
            }
            if matches!(item, 0 | 5) {
                out.push(Lrt(lrt::ARM));
            }
            if item == 7 {
                out.push(Needs(Prt::Ca));
            }
        }
        slot::MINES => {
            if matches!(item, 0 | 2 | 3 | 4 | 5 | 6 | 8 | 9) {
                out.push(Needs(Prt::Sd));
            }
            if item == 7 {
                out.push(EitherPrt(Prt::Sd, Prt::Is));
            }
            if item == 1 {
                out.push(NotPrt(Prt::Wm));
            }
        }
        slot::SPECIAL_SB => {
            if item < 7 {
                // Stargates. Hyper Expansion may not build one at all; every
                // race but Interstellar Traveler is held to the first four,
                // minus the second.
                out.push(NotPrt(Prt::He));
                if item == 1 || item > 3 {
                    out.push(Needs(Prt::It));
                }
            } else if !matches!(item, 9 | 12) {
                // Mass drivers, bar the two everybody gets.
                out.push(Needs(Prt::Pp));
            }
        }
        slot::SB_HULL => {
            if matches!(item, 1 | 3) {
                out.push(Lrt(lrt::ISB));
            }
            if item == 4 {
                out.push(Needs(Prt::Ar));
            }
        }
        slot::SPECIAL_E => {
            if matches!(item, 0 | 3) {
                out.push(Needs(Prt::Ss));
            }
            if matches!(item, 8 | 11 | 15) {
                out.push(Needs(Prt::Is));
            }
            if item == 13 {
                out.push(Needs(Prt::He));
            }
            if item == 14 {
                out.push(Needs(Prt::Sd));
            }
            if item == 16 {
                out.push(Needs(Prt::It));
            }
        }
        slot::SPECIAL_M => {
            if item == 0 {
                out.push(NotPrt(Prt::Ar));
            }
            if item == 1 {
                out.push(Needs(Prt::Ar));
            }
        }
        slot::TERRA => {
            if item < 8 {
                out.push(Lrt(lrt::TT));
            }
        }
        slot::HULL => {
            if matches!(item, 14 | 31) {
                out.push(Needs(Prt::He));
            }
            if matches!(item, 3 | 25) {
                out.push(Needs(Prt::Is));
            }
            if matches!(item, 20 | 22 | 23 | 24) {
                out.push(NotLrt(lrt::OBRM));
            }
            if matches!(item, 20 | 22 | 24) {
                out.push(Lrt(lrt::ARM));
            }
            if matches!(item, 8 | 10) {
                out.push(Needs(Prt::Wm));
            }
            if matches!(item, 12 | 18) {
                out.push(Needs(Prt::Ss));
            }
            if matches!(item, 27 | 28) {
                out.push(Needs(Prt::Sd));
            }
        }
        slot::PLANETARY => {
            let penetrating = PLANETARY.get(item).is_some_and(|p| p.ability < 0);
            if item < 9 && penetrating {
                out.push(NotLrt(lrt::NO_ADV_SCANNER));
            }
            if item < 9 {
                out.push(NotPrt(Prt::Ar));
            }
            if (10..=13).contains(&item) {
                out.push(NotPrt(Prt::Ar));
            }
            if (11..=13).contains(&item) {
                out.push(NotPrt(Prt::Wm));
            }
        }
        _ => {}
    }
    out
}

/// The trait gate: everything `FLookupPart` decides before it reaches the tech
/// check.
fn forbidden(who: &Builder<'_>, category: u16, item: usize) -> bool {
    // The designer also drops the Tachyon Detector and the Anti-matter
    // Generator from a starbase's list (`FillBuildPartsLB`) — a rule about
    // that list rather than about the component, so it is not a requirement.
    if who.starbase && category == slot::SPECIAL_E && matches!(item, 15 | 16) {
        return true;
    }
    requirements(category, item)
        .into_iter()
        .any(|need| !need.met(who.race, who.trader_parts))
}

/// The twelve components a Mystery Trader hands out, and nobody else has
/// (`FShouldPartBeHidden`, `research.c`) — and which bit of
/// [`crate::wormhole::part`] unlocks each. Until the trader has given a player
/// that bit, the component is not in any list at all.
fn trader_gift(category: u16, item: usize) -> Option<u16> {
    Some(match (category, item) {
        (slot::ENGINE, 8) => trader::ENGINE,
        (slot::SHIELD, 6) => trader::SHIELD,
        (slot::ARMOR, 9) => trader::ARMOR,
        (slot::BEAM, 18) => trader::BEAM,
        (slot::TORPEDO, 7) => trader::TORP,
        (slot::BOMB, 8) => trader::BOMB,
        (slot::MINING, 6) => trader::MINER,
        (slot::SPECIAL_E, 4) => trader::SPECIAL,
        (slot::SPECIAL_M, 4) => trader::CARGO,
        (slot::SPECIAL_M, 9) => trader::JUMPGATE,
        (slot::HULL, 30) => trader::HULL,
        (slot::PLANETARY, 14) => trader::GENESIS,
        _ => return None,
    })
}

/// How far the player is from a component's tech requirement (`TechStatus`).
///
/// The interesting case is [`Availability::Nearly`]: being one level short in
/// the field you are *already researching* is treated differently from being
/// one level short anywhere else, because that component arrives on its own.
#[must_use]
pub fn tech_status(
    need: &TechRequirement,
    levels: &[u8; TECH_FIELDS],
    researching: usize,
) -> Availability {
    let mut missing = 0;
    let mut short_field = None;
    let mut nearly = false;

    for field in 0..TECH_FIELDS {
        let have = i16::from(levels[field]);
        let want = i16::from(need[field]);
        if have < want {
            missing += 1;
            if field == researching && have + 1 == want {
                nearly = true;
            } else {
                short_field = Some(field);
            }
        }
    }

    match (missing, short_field) {
        (0, _) => Availability::Available,
        (1, _) if nearly => Availability::Nearly,
        (1, Some(f)) => Availability::Levels(i16::from(need[f]) - i16::from(levels[f]) + 1),
        _ => Availability::Far,
    }
}

/// Every component of a category this player may build now, in table order.
#[must_use]
pub fn buildable(who: &Builder<'_>, category: u16) -> Vec<Part> {
    let mut out = Vec::new();
    for item in 0.. {
        match availability(who, category, item) {
            Availability::Missing => break,
            Availability::Available => out.push(part(category, item).expect("it was just found")),
            _ => {}
        }
    }
    out
}

/// Every component the designer's parts list shows for one filter mask, in the
/// original's order: category by category from the lowest bit up, and within a
/// category in table order.
#[must_use]
pub fn filtered(who: &Builder<'_>, filter: u16) -> Vec<Part> {
    let mut out = Vec::new();
    for category in CATEGORY_ORDER {
        if filter & category != 0 {
            out.extend(buildable(who, category));
        }
    }
    out
}
