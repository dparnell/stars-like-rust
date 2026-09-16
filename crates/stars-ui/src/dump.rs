//! File (Dump to Text File): the universe, the planets and the fleets as
//! tab-separated text.
//!
//! Three arms of `CommandHandler` — `0x55` `DumpUniverse` (`1108:851e`),
//! `0x54` `DumpPlanets` (`1108:86e4`), `0x53` `DumpFleets` (`1108:9530`)
//! — each open a text file beside the game (`szBase` with a new
//! extension), stream a header and a row per object, and put up a box
//! saying where it went. The rows are tab-separated with `\r\n` ends;
//! the header is a string-table entry with `*` for each tab.
//!
//! `gd.fPerPlayerDumps` (bit 5 of the settings word) widens the planet
//! and fleet dumps with a third block of columns and names the files by
//! player — `.p1`/`.f1` rather than `.pla`/`.fle`. This project keeps it
//! as [`crate::App::per_player_dumps`].
//!
//! The text is built here without touching a file; the shell writes it
//! out. See `docs/ui/dump.md`.

use crate::App;
use stars_core::planet::Detail;
use stars_core::Planet;

/// The header strings: `Planet Name*Owner*…` (`0x4dc`, then `0x4dd` and
/// `0x4de` for the wider dump) and `Fleet Name*X*Y*…` (`0x4df`, `0x4e0`,
/// `0x4e1`).
pub const PLANET_HEADERS: [u16; 3] = [0x4dc, 0x4dd, 0x4de];
/// See [`PLANET_HEADERS`].
pub const FLEET_HEADERS: [u16; 3] = [0x4df, 0x4e0, 0x4e1];

/// This project's own column names, for a run without the executable.
pub const OWN_PLANET_HEADERS: [&str; 3] = [
    "Planet*Owner*Starbase*Report age*Population*Value*Queue*Mines*Factories*Defence %*Surface iron*Surface boranium*Surface germanium",
    "*Iron mined*Boranium mined*Germanium mined*Iron concentration*Boranium concentration*Germanium concentration*Resources",
    "*Gravity*Temperature*Radiation*Original gravity*Original temperature*Original radiation*Terraformed value*Capacity*Scanner*Penetrating*Packets to*Packet warp*Route to*Gate range*Gate mass*Base damage %",
];
/// See [`OWN_PLANET_HEADERS`].
pub const OWN_FLEET_HEADERS: [&str; 3] = [
    "Fleet*X*Y*Planet*Destination*Battle plan",
    "*Ships*Iron*Boranium*Germanium*Colonists*Fuel",
    "*Owner*ETA*Warp*Mass*Cloak %*Scanner*Penetrating*Task*Mining*Sweeping*Laying*Terraforming*Unarmed*Scouts*Warships*Utility*Bombers",
];

/// What one dump produced: the file's name (beside the game) and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dump {
    /// The file name, with the extension the original uses.
    pub file_name: String,
    /// The text, `\r\n` ends and all.
    pub text: String,
}

/// The line end the original streams (`szCRLF`).
const CRLF: &str = "\r\n";

impl App {
    /// A header line: the string-table entries with `*` turned to tabs,
    /// two of them, or three for the wider dump.
    fn dump_header(&self, ids: [u16; 3], own: [&str; 3]) -> String {
        let exe = self.art.as_ref().map(crate::art::Art::executable);
        let count = if self.per_player_dumps { 3 } else { 2 };
        let mut out = String::new();
        for index in 0..count {
            let line = exe
                .and_then(|exe| stars_formats::resources::text::string(exe, ids[index]))
                .unwrap_or_else(|| own[index].to_string());
            out.push_str(&line.replace('*', "\t"));
        }
        out.push_str(CRLF);
        out
    }

    /// The dump file's name: the game's base name with the extension, or
    /// the per-player form.
    fn dump_file_name(&self, plain: &str, per_player: char) -> String {
        let base = self.host_file_name();
        if self.per_player_dumps {
            format!("{base}.{per_player}{}", self.local_player() + 1)
        } else {
            format!("{base}.{plain}")
        }
    }

    /// `DumpUniverse`: `#`, `X`, `Y`, `Name` for every planet of the
    /// universe, one-based, in the `.xy`'s order.
    ///
    /// `None` without a game and a player, as the original refuses.
    #[must_use]
    pub fn dump_universe(&self) -> Option<Dump> {
        self.game.as_ref()?;
        let universe = self.universe.as_ref()?;
        let mut text = String::from("#\tX\tY\tName");
        text.push_str(CRLF);
        for planet in universe.planets_resolved() {
            text.push_str(&format!(
                "{}\t{}\t{}\t{}{CRLF}",
                planet.id + 1,
                planet.x,
                planet.y,
                planet.name.unwrap_or("")
            ));
        }
        Some(Dump {
            file_name: format!("{}.map", self.host_file_name()),
            text,
        })
    }

    /// `DumpPlanets`: a row for every planet the player has a record of,
    /// the columns filling in as the record allows — a scanned planet
    /// has its environment and minerals, an owned one everything.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn dump_planets(&self) -> Option<Dump> {
        let game = self.game.as_ref()?;
        let me = self.local_player();
        let player = game.players.get(me)?;
        let race = &player.race;
        let levels = player.research.levels;
        let mut text = self.dump_header(PLANET_HEADERS, OWN_PLANET_HEADERS);

        let mut planets: Vec<&Planet> = game
            .planets
            .iter()
            .chain(game.known_planets.iter())
            .collect();
        planets.sort_by_key(|p| p.id);
        for planet in planets {
            let mut row: Vec<String> = Vec::new();
            let scanned = planet.detail != Detail::Minimal;
            let full = planet.detail == Detail::Full;
            row.push(self.planet_name(planet.id));
            // Owner: `PszPlayerName(owner, 1, 0, 0, 0)`.
            row.push(
                planet
                    .owner
                    .and_then(|o| usize::try_from(o).ok())
                    .map_or_else(String::new, |o| self.psz_player_name(o)),
            );
            // Starbase type: the base's design, where there is one.
            row.push(
                planet
                    .owner
                    .filter(|_| planet.starbase)
                    .and_then(|o| usize::try_from(o).ok())
                    .and_then(|o| {
                        let slot = planet.starbase_design?;
                        game.designs
                            .get(o)?
                            .get(crate::app::starbase_slot(slot))
                            .map(|d| d.name.clone())
                    })
                    .unwrap_or_default(),
            );
            // Report age: `game.turn - PLANET.turn`. This engine keeps no
            // such stamp; a planet of the player's own is current.
            row.push(if full { "0".to_string() } else { String::new() });
            // Population: ours exactly (`pop * 100`), anybody else's as
            // the scan's estimate (`uPopGuess * 400`, which this engine
            // holds in hundreds like the summary pane prints it) when the
            // planet is scanned.
            row.push(match (full, planet.owner, scanned) {
                (true, _, _) | (false, Some(_), true) => (i64::from(planet.pop) * 100).to_string(),
                _ => String::new(),
            });
            // Value: `PctPlanetDesirability`, as a percentage.
            row.push(if scanned {
                format!(
                    "{}%",
                    stars_core::hab::pct_planet_desirability(planet, race)
                )
            } else {
                String::new()
            });
            // Production queue: the first item, as the dialog names it.
            row.push(if full {
                planet
                    .queue
                    .first()
                    .map(|entry| {
                        format!(
                            "{} {}",
                            entry.count,
                            self.production_item_name(entry.item, entry.ship)
                        )
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            });
            // Mines, factories and defence coverage to a tenth of a percent
            // (`CalcPctSurvive`) for the player's own; blank otherwise —
            // and, for the wider dump, the estimate of somebody else's
            // defences where the scan gave one, which the original tacks
            // on as a fourth field.
            if full {
                let (survive, _) = stars_core::bombing::pct_survive(planet, race, levels);
                let tenths = ((1.0 - survive) * 1000.0).round() as i64;
                row.push(planet.mines.to_string());
                row.push(planet.factories.to_string());
                row.push(format!("{}.{}%", tenths / 10, tenths % 10));
            } else {
                row.push(String::new());
                row.push(String::new());
                row.push(String::new());
                if self.per_player_dumps && planet.owner.is_some() && scanned && planet.defenses > 0
                {
                    row.push(format!("{}.0%", planet.defenses));
                }
            }
            // Surface minerals, when scanned.
            for i in 0..3 {
                row.push(if scanned {
                    planet.surface_min[i].to_string()
                } else {
                    String::new()
                });
            }
            // Mining rates, when the scan went deeper than the surface.
            let mined = if planet.detail == Detail::Full {
                Some(stars_core::mining::minerals_mined(planet, race, None, None))
            } else {
                None
            };
            for i in 0..3 {
                row.push(mined.map_or_else(String::new, |m| m[i].to_string()));
            }
            // Concentrations, when scanned.
            for i in 0..3 {
                row.push(if scanned {
                    planet.min_conc[i].to_string()
                } else {
                    String::new()
                });
            }
            // Resources, for the player's own.
            row.push(if full {
                stars_core::resources::resources_at_planet(planet, race, i16::from(levels[0]))
                    .map_or_else(String::new, |r| r.to_string())
            } else {
                String::new()
            });
            if self.per_player_dumps {
                if !scanned {
                    row.extend(std::iter::repeat_n(String::new(), 7));
                } else {
                    for i in 0..3 {
                        row.push(crate::app::env_text(i, planet.env[i]));
                    }
                    let orig = planet.env_orig.unwrap_or(planet.env);
                    for (i, clicks) in orig.iter().enumerate() {
                        row.push(crate::app::env_text(i, *clicks));
                    }
                    let reach = stars_core::terraform::optimal_env(planet, race, levels);
                    row.push(format!(
                        "{}%",
                        stars_core::ai::colonise::pct_planet_opt_value(planet, race, reach)
                    ));
                }
                if full {
                    // Capacity, the scanner's ranges, the packet target and
                    // warp, the route, the gate and the base's damage.
                    let capacity =
                        stars_core::hab::calc_planet_max_pop(planet, race).map_or(0, |max| {
                            if max <= 0 {
                                0
                            } else {
                                i64::from(planet.pop) * 100 / i64::from(max)
                            }
                        });
                    row.push(capacity.to_string());
                    let range = stars_core::scanning::planet_scanner_range_for_tech(
                        planet,
                        race,
                        &levels,
                        planet.scanner.is_some(),
                    );
                    row.push(range.normal.to_string());
                    row.push(range.penetrating.to_string());
                    // The mass driver's target and warp — the stored warp is
                    // four below the one flung at, as the tile shows it.
                    let (driver_to, driver_warp) =
                        planet.fling_dest.map_or((String::new(), 0), |target| {
                            (self.planet_name(target), i32::from(planet.fling_warp) + 4)
                        });
                    row.push(driver_to);
                    row.push(driver_warp.to_string());
                    row.push(
                        planet
                            .route_dest
                            .map_or_else(String::new, |to| self.planet_name(to)),
                    );
                    match self.planet_gate(planet) {
                        Some((range, mass)) => {
                            row.push(range.to_string());
                            row.push(mass.to_string());
                        }
                        None => {
                            row.push("0".to_string());
                            row.push("0".to_string());
                        }
                    }
                    row.push(
                        if planet.starbase {
                            i32::from(planet.starbase_damage >> 4)
                        } else {
                            0
                        }
                        .to_string(),
                    );
                }
            }
            text.push_str(&row.join("\t"));
            text.push_str(CRLF);
        }
        Some(Dump {
            file_name: self.dump_file_name("pla", 'p'),
            text,
        })
    }

    /// `DumpFleets`: a row for every fleet on the player's map, the
    /// player's own filled in fully, anybody else's as far as the scan
    /// saw.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn dump_fleets(&self) -> Option<Dump> {
        let game = self.game.as_ref()?;
        let me = self.local_player();
        let player = game.players.get(me)?;
        let mut text = self.dump_header(FLEET_HEADERS, OWN_FLEET_HEADERS);
        for (index, fleet) in game.fleets.iter().enumerate() {
            let owner = usize::try_from(fleet.owner).ok();
            let ours = owner == Some(me);
            let designs = owner
                .and_then(|o| game.designs.get(o))
                .map_or(&[][..], Vec::as_slice);
            let mut row: Vec<String> = Vec::new();
            // The name with the owner's race in front, as `PszGetFleetName`
            // gives it with `idPlayer` set aside.
            row.push(self.fleet_name_with_owner(index));
            row.push(fleet.position.x.to_string());
            row.push(fleet.position.y.to_string());
            row.push(
                fleet
                    .orbiting
                    .and_then(|p| i16::try_from(p).ok())
                    .map_or_else(String::new, |p| self.planet_name(p)),
            );
            // Destination: the next waypoint's place, for the player's own.
            row.push(if ours {
                fleet.waypoints.get(1).map_or_else(String::new, |w| {
                    self.location_name(w.target_class, w.target, w.position)
                })
            } else {
                String::new()
            });
            // The battle plan, by name.
            row.push(if ours {
                player
                    .battle_plans
                    .iter()
                    .find(|p| p.plan_id == fleet.battle_plan)
                    .map_or_else(String::new, |p| p.name.clone())
            } else {
                String::new()
            });
            let ships: i64 = fleet.stacks.iter().map(|s| i64::from(s.count)).sum();
            row.push(ships.to_string());
            for i in 0..3 {
                row.push(fleet.cargo.minerals[i].to_string());
            }
            row.push(fleet.cargo.colonists.to_string());
            row.push(fleet.cargo.fuel.to_string());
            if self.per_player_dumps {
                row.push((fleet.owner + 1).to_string());
                // ETA to the next waypoint, in years; `0` with nowhere to
                // go.
                let next = fleet.waypoints.get(1);
                let warp = next.map_or(0, |w| i16::from(w.warp));
                let eta = next.map_or(0, |w| {
                    let dx = f64::from(i32::from(w.position.x) - i32::from(fleet.position.x));
                    let dy = f64::from(i32::from(w.position.y) - i32::from(fleet.position.y));
                    let per_year = stars_core::movement::travel_per_year(warp);
                    if per_year <= 0 {
                        0
                    } else {
                        (dx.hypot(dy) / f64::from(per_year)).ceil() as i64
                    }
                });
                row.push(eta.to_string());
                // Warp: the next leg's, or the speed a scan saw.
                row.push(if next.is_some() {
                    warp.to_string()
                } else {
                    fleet.warp.map_or(0, i16::from).to_string()
                });
                row.push(fleet.mass(designs).to_string());
                row.push(fleet.cloak_pct(designs, &player.race).to_string());
                let range = self.fleet_scan_range(fleet);
                row.push(range.normal.max(0).to_string());
                row.push(range.penetrating.max(0).to_string());
                row.push(if ours {
                    next.map_or_else(String::new, |w| crate::app::task_name(w.task).to_string())
                } else {
                    String::new()
                });
                row.push(stars_core::mining::remote_mines(designs, &fleet.stacks).to_string());
                row.push(stars_core::minefield::fleet_sweep(fleet, designs).to_string());
                row.push(stars_core::minefield::mines_laid(fleet, designs, 0).to_string());
                let stacks: Vec<(&stars_core::design::ShipDesign, i32)> = fleet
                    .stacks
                    .iter()
                    .filter_map(|s| Some((designs.get(usize::from(s.design))?, s.count)))
                    .collect();
                row.push(stars_core::terraform::orbital_adjusters(&stacks).to_string());
                // Ships by class: unarmed is everything not scout, warship,
                // utility or bomber.
                use stars_core::design::ShipClass;
                let by_class = |wanted: Option<ShipClass>| -> i64 {
                    stacks
                        .iter()
                        .filter(|(design, _)| match wanted {
                            Some(class) => design.ship_class() == Some(class),
                            None => !matches!(
                                design.ship_class(),
                                Some(
                                    ShipClass::Scout
                                        | ShipClass::Warship
                                        | ShipClass::Utility
                                        | ShipClass::Bomber
                                )
                            ),
                        })
                        .map(|(_, count)| i64::from(*count))
                        .sum()
                };
                row.push(by_class(None).to_string());
                for class in [
                    ShipClass::Scout,
                    ShipClass::Warship,
                    ShipClass::Utility,
                    ShipClass::Bomber,
                ] {
                    row.push(by_class(Some(class)).to_string());
                }
            }
            text.push_str(&row.join("\t"));
            text.push_str(CRLF);
        }
        Some(Dump {
            file_name: self.dump_file_name("fle", 'f'),
            text,
        })
    }

    /// A fleet's name with its owner's race in front whoever owns it —
    /// `PszGetFleetName` called with `idPlayer` set to `-1`, which is how
    /// `DumpFleets` names every row.
    #[must_use]
    pub fn fleet_name_with_owner(&self, index: usize) -> String {
        let Some(game) = self.game.as_ref() else {
            return String::new();
        };
        let Some(fleet) = game.fleets.get(index) else {
            return String::new();
        };
        let plain = self.fleet_display_name(index);
        if usize::try_from(fleet.owner).is_ok_and(|o| o == self.local_player()) {
            let owner = usize::try_from(fleet.owner).unwrap_or(0);
            let prefix = game
                .players
                .get(owner)
                .map_or_else(|| format!("player {}", owner + 1), |p| p.name.clone());
            format!("{prefix} {plain}")
        } else {
            plain
        }
    }

    /// A planet's stargate, as `(range, mass)`, when its base has one —
    /// the limits the part's name carries, `Stargate <mass>/<range>`,
    /// with `any` as `-1`.
    fn planet_gate(&self, planet: &Planet) -> Option<(i32, i32)> {
        let game = self.game.as_ref()?;
        let owner = usize::try_from(planet.owner?).ok()?;
        let slot = planet.starbase_design?;
        let design = game
            .designs
            .get(owner)?
            .get(crate::app::starbase_slot(slot))?;
        design
            .slots
            .iter()
            .filter(|s| s.count > 0 && s.category & stars_core::components::slot::SPECIAL_SB != 0)
            .find_map(|s| {
                let part = stars_core::components::SPECIALS_SB.get(usize::from(s.item))?;
                let limits = part.name.strip_prefix("Stargate ")?;
                let (mass, range) = limits.split_once('/')?;
                let number = |s: &str| -> i32 {
                    if s == "any" {
                        -1
                    } else {
                        s.parse().unwrap_or(0)
                    }
                };
                Some((number(range), number(mass)))
            })
    }
}
