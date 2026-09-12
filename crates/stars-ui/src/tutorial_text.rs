//! The tutorial's eighty pages, in this project's own words.
//!
//! The original's pages are the game's own writing and none of them are
//! copied here. What is here is a retelling: the same eighty pages, the same
//! eight paragraphs to a page, each paragraph saying what the original's
//! paragraph in that slot says — the same planet, the same fleet, the same
//! key, the same number — so that the step machine's emphasis, which points
//! at a paragraph by its slot, lands on the right instruction whichever text
//! is showing.
//!
//! When a copy of the original is found, its own words are shown instead
//! (`stars_formats::tutorial`), and this is the text the player sees when
//! there is none, so the tutorial teaches either way.
//!
//! Two conventions are `DrawTutorText`'s (`10f8:03c0`) and are kept so the
//! panel flows the same: a paragraph beginning with a **capital letter**
//! starts a new one with half a line of air above it, a paragraph beginning
//! with a **space** runs straight on from the last, and a paragraph of one
//! character ends the page. `crates/stars-ui/tests/tutorial_text.rs` checks, when
//! the original is to hand, that every slot here has the same shape as the
//! original's.

use stars_formats::tutorial::{PAGES as PAGE_COUNT, PARAGRAPHS_PER_PAGE};

/// One page: eight paragraph slots.
pub type Page = [&'static str; PARAGRAPHS_PER_PAGE];

/// The page's paragraphs, one-based as the title bar counts pages.
#[must_use]
pub fn page(number: usize) -> Option<Vec<String>> {
    let index = number.checked_sub(1)?;
    PAGES
        .get(index)
        .map(|page| page.iter().map(|p| (*p).to_string()).collect())
}

/// The eighty pages.
pub static PAGES: [Page; PAGE_COUNT] = [
    // --- Year 2400 ---------------------------------------------------------
    // 1
    [
        "Welcome to Stars! Over the next 36 years of a sample game this tutorial will show you how it is played. You begin",
        " with one home planet, two scouts, a destroyer, a freighter, a colony ship and a remote miner.",
        "Five messages are waiting in the Messages pane. Every year the game writes to you ",
        "about your planets and your fleets, and about things that everybody gets to hear of.",
        "This year's five are all playing tips, and none of them needs anything done about it.",
        "Read every one of your messages now.",
        " The Next button steps through them, and so does the down arrow key.",
        " ",
    ],
    // 2
    [
        "Have a look at the tiles of the Command pane, top left of the screen.",
        " Between them they tell you everything about your planet and let you give it orders.",
        "The Fleets in Orbit tile lists the fuel and cargo aboard Armed Probe #1.",
        " Press that tile's Goto button to take command of Armed Probe #1.",
        "The pane is now about your fleet: what it is, and what it should do.",
        " Let's send this scout out to look around. Your starbase has already filled its tank, so it can leave at once.",
        "The Scanner pane is your map of the universe.",
        " Hold down shift and left-click on the planet Prune.",
    ],
    // 3
    [
        "The Fleet Waypoints tile says the trip to Prune takes 2 years. Look at the Other Fleets Here tile and notice that",
        " Long Range Scout #2 carries six times as much fuel as Armed Probe #1 does.",
        "Press the n key to move on to your next fleet.",
        "Long Range Scout #2 has no weapons. It is less likely to meet anybody if we point it",
        " at the planets up and to the right.",
        "Hold down shift and left-click on the planet 90210.",
        " ",
        " ",
    ],
    // 4
    [
        "On to the next fleet.",
        " This time use the Next button in the tile that shows Long Range Scout #2.",
        "Santa Maria #3 is your colony ship. There is nowhere to send it yet, so",
        " press Next once more.",
        "Teamster #4 is your freighter. It has nothing to carry yet either, so",
        " press Next again.",
        "Stalwart Defender #5 is a destroyer, and for now it makes a perfectly good scout.",
        " Hold down shift and click on Alexander.",
    ],
    // 5
    [
        "Press the n key.",
        " Cotton Picker #6 is your remote miner. It will go out as soon as we find a planet worth mining.",
        "Press n once more.",
        " That brings you round to Armed Probe #1 again: you have seen every fleet you own.",
        "One thing remains for this year: choosing what to research.",
        " Open Research from the Commands menu.",
        " Set the Field of Study to Weapons and press Done.",
        "That is everything for this year. Press F9 to generate the next one.",
    ],
    // --- Year 2401 ---------------------------------------------------------
    // 6
    [
        "Read the message in the Messages pane. There are plenty of minerals on hand for production, so a good first move is",
        " to build some more factories.",
        "Press Change on the Production tile.",
        " Pick Factory in the list on the left, hold down shift and press Add twice, giving 20 factories in all, then press OK.",
        " With shift held, Add puts in 10 of an item at a time rather than 1.",
        "That was the only message this year. The Scanner pane shows your fleets still on their way,",
        " so there is nothing more to do.",
        "Press F9 to generate the next year.",
    ],
    // --- Year 2402 ---------------------------------------------------------
    // 7
    [
        "Start with your first message and press Goto in the Messages pane.",
        " Let's give Armed Probe #1 a whole string of places to visit, so it can be left alone for a while.",
        "Hold down shift and left-click on Hiho,",
        " then on No Vacancy,",
        " Slime,",
        " Wallaby",
        " and lastly Oxygen.",
        "Read the next message and press Goto to take command of Long Range Scout #2.",
    ],
    // 8
    [
        "This one is to explore the region above and to the right of home, so",
        " hold down shift and pick Dwarte,",
        " Mobius,",
        " Castle",
        " and lastly Moholdi.",
        "Read the next message",
        " and",
        " Goto Stalwart Defender #5 from it.",
    ],
    // 9
    [
        "Hold down shift and pick Shaggy Dog,",
        " Sea Squared,",
        " Red Storm,",
        " Bloop",
        " and lastly Kalamazoo.",
        "Read the next two messages",
        " and",
        " press Goto so that the Summary pane shows Prune.",
    ],
    // 10
    [
        "The top graph in the Summary pane shows that Prune runs a little too hot for you to bring into range by terraforming",
        " with the technology you have now.",
        "The diamonds on the lower graph show Prune's mineral concentrations are quite high, so let's send Cotton Picker #6",
        " to dig them up.",
        "Right-click on Stove Top and pick Cotton Picker #6.",
        " Shift-click on Prune.",
        "Now find the Waypoint Task tile,",
        " open its dropdown and set the task to Remote Mining.",
    ],
    // 11
    [
        "Step to your next message and Goto Alexander.",
        "  The diamonds on the lower graph show that Alexander's minerals are far thinner than Prune's.",
        "So Cotton Picker #6 is headed to the right place.",
        "Try clicking around the Summary pane: the graphs and figures each explain themselves in a popup.",
        "Read the next message",
        " and",
        " Goto the planet 90210.",
        " ",
    ],
    // 12
    [
        "Now, 90210 is a good planet with rich minerals, so let's send Santa Maria #3 to settle it.",
        " Right-click on Stove Top and pick Santa Maria #3.",
        "Press the Xfer button on the tile called Orbiting Stove Top.",
        " Drag in the Colonists gauge until the hold carries 25kT of colonists, then press OK.",
        "Shift-click on 90210.",
        " In the Waypoint Task tile's dropdown, choose Colonize.",
        "That is everything for this year.",
        "Press F9 to generate the next year.",
    ],
    // --- Year 2403 ---------------------------------------------------------
    // 13
    [
        "Your first message is one you will get very often, and there is no need to see it every year.",
        " Switch it off by clicking the blue check mark at the top left of the Messages pane.",
        "Step to the next message and Goto Stove Top.",
        " Rather than keep adding factories to the queue by hand, let's have the queue keep building them itself,",
        " up to 30 factories a year.",
        "Press Change in the Production tile.",
        " Pick Factories (Auto Build) in the list on the left, hold down shift and press Add three times, then press OK.",
        " ",
    ],
    // 14
    [
        "Read the next two messages and Goto 90210.",
        " Its production queue is empty, and that wants fixing.",
        " Press the q key.",
        "Double-click Factory three times and then Mine three times, so the queue reads 3 factories then 3 mines. Tick the 'Contribute only leftover...' box and press OK.",
        "The 3 factories will take 10 years, and the mines a couple more. 90210 needs more people.",
        " Right-click on Stove Top and pick Teamster #4.",
        "Press the Xfer button in the Command pane,",
        " fill the hold with colonists and press OK.",
    ],
    // 15
    [
        "Shift-click on 90210 and",
        " set the waypoint task to Transport.",
        " Then right-click the blue diamond in the Waypoint Task tile and choose QuikDrop, which empties the freighter at 90210.",
        "Read the next message and Goto Hiho.",
        " Armed Probe #1 — the blue triangle just right of Hiho — has not reached it yet, but Hiho's figures are already in.",
        "The dark yellow ring around Armed Probe #1 is how far out it can read a planet's values.",
        " Double-click on Armed Probe #1.",
        " ",
    ],
    // 16
    [
        "Armed Probe #1 need not go all the way to Hiho now, so let's drop that stop.",
        " Click Hiho",
        " and",
        " press the Delete key.",
        "Armed Probe #1 will carry straight on to No Vacancy without losing time at Hiho.",
        "That is everything for this year.",
        "For variety, instead of pressing F9,",
        " choose Generate from the Turn menu.",
    ],
    // --- Year 2404 ---------------------------------------------------------
    // 17
    [
        "Start with your first message and Goto Shaggy Dog.",
        " We will queue a colony ship shortly, but first let's tidy Stalwart Defender #5's route.",
        "Double-click on Stalwart Defender #5, just above the planet,",
        " click its waypoint at Shaggy Dog,",
        " and hit Delete.",
        "Read the next message and Goto Dwarte.",
        " Not a nice place at all.",
        "Double-click on Stove Top.",
    ],
    // 18
    [
        "Press Change in the Production tile to open Stove Top's queue.",
        " Double-click Santa Maria in the list on the left and press OK.",
        "Notice that the Factory at the head of the queue and the Santa Maria are shown in green",
        " in the Production tile: green means finished this year.",
        "Blue items make some progress this year. Black ones need more than a year.",
        " Red ones may never be finished at all.",
        "Then go ahead and",
        " generate when you are ready.",
    ],
    // --- Year 2405 ---------------------------------------------------------
    // 19
    [
        "Read your first message and Goto the new Santa Maria.",
        " It has taken over as fleet #3; the old fleet #3 was broken up when it settled 90210.",
        "Click in the cargo gauge on the Fuel and Cargo tile.",
        " That is the same as pressing Xfer.",
        " Fill the hold with colonists and press OK.",
        "Now, where was this colony ship meant to go?",
        " Press the % button on the toolbar and the map colours planets by how habitable they are.",
        "Shift-click on the big green Shaggy Dog, below Stove Top.",
    ],
    // 20
    [
        "Make the waypoint task Colonize.",
        "Put the Scanner back in normal view with the leftmost toolbar button.",
        "Read the next 2 messages and Goto Teamster #4.",
        " Shift-click on Stove Top to bring it home.",
        "Select 90210 with the Goto button in the tile called Orbiting 90210.",
        " The factories are now 2 years off rather than 10.",
        "Read the next message and Delete Armed Probe #1's waypoint at No Vacancy.",
        " That is everything for this year. Generate when ready.",
    ],
    // --- Year 2406 ---------------------------------------------------------
    // 21
    [
        "Start with your first message and Goto Teamster #4.",
        " Cotton Picker #6 has been mining Prune for some years now. Time to start carrying the",
        " minerals home to Stove Top.",
        "Shift-click on Prune.",
        " make the waypoint task Transport,",
        " right-click the blue diamond and choose QuikLoad from the Zip menu.",
        "Now shift-click on Stove Top again.",
        " ",
    ],
    // 22
    [
        "Notice the waypoint task has carried over from the previous stop. What we want at Stove Top is to put the",
        " Prune minerals down.",
        " Right-click the blue diamond and choose QuikDrop from the Zip menu.",
        "Now tick Repeat Orders in the Fleet Waypoints tile.",
        " Teamster #4 will shuttle minerals from Prune to Stove Top until told otherwise.",
        "Now select Stove Top",
        " and put a Santa Maria in its production queue.",
        "Go on and generate. Dare you.",
    ],
    // --- Year 2407 ---------------------------------------------------------
    // 23
    [
        "Read your first message, Goto the new Santa Maria and load it up with colonists.",
        "Press the % toolbar button to see planets by value.",
        " Red Storm is plainly the best planet on offer.",
        " Give Santa Maria #7 a Colonize task at Red Storm.",
        "At least two more planets can be settled, Sea Squared and Slime, and another may turn up soon, so",
        " add three more Santa Marias to Stove Top's queue.",
        "Press the leftmost toolbar button to return the Scanner to normal view.",
        " ",
    ],
    // 24
    [
        "Step to the next message.",
        " Mine-building messages like this one will keep coming for ever.",
        " Switch them off with the blue check mark in the Messages pane,",
        " then step to the next message.",
        "Goto 90210 and open its production queue.",
        " Shift-double-click 'Factories (Auto Build)' and then 'Mines (Auto Build)' in the list on the left, queueing 10 of each, then press OK.",
        "Go through the rest of your messages.",
        " ",
    ],
    // 25
    [
        "Click the red triangle that sits between Slime and No Vacancy.",
        " That is an enemy scout. Right-clicking the fleet picture in the Summary pane shows what we know of its design, and for now we cannot say whether",
        " it carries weapons. The path drawn on the scanner has it bound for No Vacancy, and the tick marks say it arrives in two years.",
        "Armed Probe #1 cannot catch it. We need more warships.",
        " Put two Armed Probes in Stove Top's queue.",
        "That is everything for this year.",
        " Generate when you are ready.",
        " ",
    ],
    // --- Year 2408 ---------------------------------------------------------
    // 26
    [
        "Read your first message and Goto the new colony ships.",
        "There are only two places to send them, so",
        " press the Split button in the Fleet Composition tile.",
        " Move one Santa Maria across to Fleet #10 and press OK.",
        "That leaves two Santa Marias in the fleet you are commanding.",
        " Fill it with colonists and give it a Colonize task at Slime.",
        "Both colony ships should not go to Slime, so press Split All in the Fleet Composition tile.",
        " ",
    ],
    // 27
    [
        "This fleet now holds a single Santa Maria; the other went to a fleet of its own.",
        "Click the waypoint at Slime and drag it to Sea Squared.",
        " Notice there are now waypoint lines from Stove Top to both Slime and Sea Squared.",
        " Santa Maria #8 and Santa Maria #11 have Colonize orders at Sea Squared and Slime, which is just what we wanted.",
        "Read the next message and Goto the new Armed Probes.",
        " Fly straight to No Vacancy and that enemy scout will be gone long before you arrive.",
        "Let's cut it off instead.",
        " Shift-click on Hiho.",
    ],
    // 28
    [
        "Read the next message and switch it off.",
        "Take your last message and Goto Wallaby.",
        " Wallaby belongs to the Berserkers, but it is thinly settled and very nearly within terraforming reach.",
        "Click the green Radiation bar in the Summary pane and read what pops up.",
        " That 7% rings a bell.",
        " Press F5 to open the Research dialog.",
        " ",
        " ",
    ],
    // 29
    [
        "There it is: Radiation Terraform 7% is among the Expected Benefits of the Weapons research already under way.",
        " A benefit shown in blue needs more than one further level of research.",
        "Click the word Radiation in the dialog to see what it takes.",
        " At the present rate, Weapons level 5 is a long way off.",
        "Raise the 'Resources budgeted to research' to 30% and press Done.",
        "A troop ship will go down to Wallaby before long, but that is all for now.",
        " Generate when you are ready.",
        " ",
    ],
    // --- Year 2409 ---------------------------------------------------------
    // 30
    [
        "Start with your first message and Goto the Research dialog.",
        " Notice Radiation 7 is now shown in green: it arrives the moment the current level",
        " of Weapons is finished, and that is reckoned at 4 years away.",
        "Notice too that 'Next field to research' reads Same Field.",
        " Change 'Next field to research' to Construction and press Done.",
        " When Weapons reaches level 5, research will turn to Construction of its own accord.",
        "Read the next message and switch it off.",
        " ",
    ],
    // 31
    [
        "Read the next message and Goto Oxygen.",
        " Right-click on Stove Top and pick Santa Maria #10.",
        " Fill it with colonists",
        " and send it to settle Oxygen.",
        "Oxygen has been looked at now, so Armed Probe #1 can be put to other uses.",
        " Select Armed Probe #1 and drag its waypoint off Oxygen and onto Mozart.",
        "That is everything for this year.",
        "Generate when you are ready.",
    ],
    // --- Year 2410 ---------------------------------------------------------
    // 32
    [
        "Switch off the message about the colony ship being dismantled.",
        " Read the next message and Goto Shaggy Dog.",
        "Open the production queue at Shaggy Dog.",
        " Put in 3 'Factories (Auto build)' and 3 'Mines (Auto build)', tick 'Contribute only leftover' and press OK.",
        "It would be worth setting up a default queue as well, so the other four",
        " colony ships on their way need not be done by hand.",
        "Open the production queue once more.",
        " Right-click the blue diamond and choose Customize.",
    ],
    // 33
    [
        "Press Import to copy Shaggy Dog's queue into the default template, and press OK.",
        " Then press OK on the production dialog too.",
        " From now on every planet you settle starts with a copy of the default template as its queue.",
        "Read the next message and Goto Bloop. It looks a pleasant place.",
        " Put a Santa Maria in Stove Top's queue.",
        "We mean to take Wallaby. Teamster #4 is tied up carrying minerals from Prune to Stove Top,",
        " so add a fresh Teamster to Stove Top's queue.",
        "Generate when you are ready.",
    ],
    // --- Year 2411 ---------------------------------------------------------
    // 34
    [
        "Start with your first message and Goto Armed Probe #1.",
        " Shift-click on Hacker.",
        "Read the next message and Goto Long Range Scout #2.",
        " This ship has little left to offer: it is far from the unexplored planets and carries no weapons.",
        " Shift-click on Stove Top and set the waypoint task to Scrap Fleet.",
        "Read the next message and send Stalwart Defender #5 to Stove Top.",
        " Your starbase will refuel it as soon as it arrives.",
        "Read the next message, Goto Armed Probe #9 and shift-click the red triangle below Hiho.",
    ],
    // 35
    [
        "Read the next message and Goto the new Santa Maria.",
        " Use the toolbar, the Scanner and the Summary pane to find which available planet has the best growth figure,",
        " and order Santa Maria #3 to settle it. (Load colonists first!)",
        "Read the next message and Goto Teamster #12.",
        " Fill it with colonists and give it a waypoint at Wallaby.",
        "Set the waypoint task to Transport.",
        " Make the Waypoint Task tile's second dropdown read Colonists",
        " and its third Unload All.",
    ],
    // 36
    [
        "Read the next message and put 70 mines at the top of Stove Top's production queue.",
        "Read the next message and Goto the Research dialog.",
        " Keep the Field of Study on Construction, but set Next field to research to Biotechnology.",
        "Read the next message and press Goto to open the Technology Browser.",
        " When you have finished reading about the Beta Torpedo, close the browser.",
        "Read the next two messages, looking things up in the browser if you like.",
        "Read the next message and Goto the Armed Probe.",
        " ",
    ],
    // 37
    [
        "Well done, you got one of them. Notice the button that usually reads Goto now reads View.",
        " Press View and the Battle VCR opens.",
        " Use the VCR controls to play the battle back, then press Done.",
        "Go through the rest of your messages.",
        " Stove Top is busy with mines this year, so the colony ships can wait until next year.",
        "As a rule, walking through the year's messages and acting on the ones that matter is 90% of",
        " the work of any year.",
        "Generate when you are ready.",
    ],
    // --- Year 2412 ---------------------------------------------------------
    // 38
    [
        "Read your first message. Its Goto button is greyed out — odd. One of the other messages this year may explain it.",
        "Read the next message and Goto Armed Probe #9.",
        " It seems you got another one. There is a yellow diamond on the same spot as Armed Probe #9, and that mark is",
        " salvage left over from a battle.",
        "Right-click on Armed Probe #9 and pick Salvage.",
        " The Summary pane shows a few kT of Ironium and Germanium. Not a lot, but perhaps worth sending",
        " a freighter for.",
        "Choose View/Find, type Teamster #4 and press OK.",
    ],
    // 39
    [
        "If you cannot see where the yellow selection arrow went, press the v key and the scanner will centre on it.",
        " Teamster #4 is at Stove Top and will be back at Prune next year. It can be diverted then, if the salvage is still there.",
        "Read the next message.",
        " So that is what became of Armed Probe #1.",
        "Watch that sorry battle if you wish, then step to the next message.",
        " Better. This one is worth watching.",
        "Read the next message and Goto Red Storm.",
        " ",
    ],
    // 40
    [
        "Apart from being a little short of colonists, Red Storm is doing well.",
        " Read the next message and Goto Slime.",
        "Look at the Summary pane. Slime is outside your habitable range, but you have the technology to terraform it into range.",
        " What is needed is some terraforming in the production queue.",
        "Open Slime's production queue, add two Terraform Environments, and press OK.",
        " Look at the Production tile. A few more colonists here would see that done in a sensible time.",
        "Put two Teamsters in Stove Top's production queue.",
        " Generate when you are ready.",
    ],
    // --- Year 2413 ---------------------------------------------------------
    // 41
    [
        "Read your first message, fill Teamster #1 with colonists,",
        " and send it to Slime with orders to unload every one of them.",
        "Read the next message.",
        " The reinforcements aboard Teamster #1 are going to matter.",
        "Read the next message, open the Research dialog and set 'Next field to research' to Propulsion.",
        "Read the next message and look up the Robo-Miner in the Tech Browser.",
        " More miners would help strip Prune faster. Let's design a ship around these new robots.",
        "Press F4 to open the Ship Designer.",
    ],
    // 42
    [
        "Pick Available Hull Types,",
        " choose Mini-Miner from the dropdown",
        " and press Copy Selected Design.",
        "The left-hand side lists every part you are able to fit to a ship.",
        "Drag a Long Hump 6 engine from the parts list onto the ship's Engine slot.",
        "Drag a Rhino Scanner onto the Scanner/Elect/Mech slot.",
        "Choose Mining Robots in the parts category dropdown",
        " and drag a Robo-Miner onto each of the Mining slots.",
    ],
    // 43
    [
        "The design's name and picture will do as they are.",
        " Press OK to finish the design",
        " and Done after that to close the designer.",
        "Add one of the new Mini-Miners to Stove Top's production queue.",
        " It shows 6 years to build. You simply are not mining fast enough, so let's put 100 mines at the head of the queue.",
        "Open the queue at Stove Top once more.",
        " Pick Mine in the list on the left and the top of the queue on the right, hold down Ctrl, press Add, then OK.",
        " ",
    ],
    // 44
    [
        "Click each item in the Production tile. The Mini-Miner now takes only 4 years.",
        "Read your last message.",
        " Minerals are short enough now that another new design would not be much use. The new Privateer hull can wait.",
        "Armed Probe #9 was left loitering near Prune. The salvage seems to have faded away; never mind.",
        " Order Armed Probe #9 home to Stove Top.",
        "That will do for this year.",
        " Generate the year.",
        " ",
    ],
    // --- Year 2414 ---------------------------------------------------------
    // 45
    [
        "Start with your first message and Goto Sea Squared.",
        " Apart from wanting more people, it is coming along nicely.",
        "Read your last message and Goto Oxygen.",
        " Oxygen sits just outside your habitable range, so terraforming should go into the default production template.",
        "Add 'Min Terraform Up To 2%' to Oxygen's queue, right-click the blue diamond, choose <Customize> then Import, and press OK on both dialogs.",
        "That is everything for this year. Automation makes some years pass quicker than others.",
        "Generate when you are ready.",
        " ",
    ],
    // --- Year 2415 ---------------------------------------------------------
    // 46
    [
        "Start with your first message and order Armed Probe #9 to Neil.",
        "Read the next message, Goto the new Teamster and fill it with colonists.",
        " Those colonists should go wherever they are wanted most.",
        "Pick Planets... on the Report menu.",
        " Click the heading of the Value column and Sort by Value.",
        " The planet with the largest negative value is Wallaby: colonists die there faster than anywhere else you hold.",
        "Press Esc to close the Planet Summary Report.",
        " Order Teamster #7 to Wallaby to Unload All Colonists.",
    ],
    // 47
    [
        "Read the next two messages and open the Research dialog.",
        " Every Expected Research Benefit is blue or black, meaning at least two more levels of Propulsion before",
        " anything is learned. 'Next field to research' is already Same Field, so just",
        " shut the dialog.",
        "Go through your remaining messages and order Teamster #12 home to Stove Top.",
        "The Mine Dispenser 50 sounds promising, but this is not the moment for a new design.",
        "Generate the year.",
        " ",
    ],
    // --- Year 2416 ---------------------------------------------------------
    // 48
    [
        "Start with your first message and order Stalwart Defender #5 to Wallaby.",
        "Read the next message and Goto the new Mini-Miner.",
        " Shift-click on Prune and set the Waypoint Task to Merge with Fleet.",
        " Notice the waypoint at Prune now reads Cotton Picker #6.",
        "Read the next message and open Stove Top's production queue.",
        " Raise the Auto Build factories to 60. Add 60 Auto Build mines at the foot of the queue and close it.",
        "Read the next two messages and set 'Next field to research' to Construction.",
        " Read your last message and generate.",
    ],
    // --- Year 2417 ---------------------------------------------------------
    // 49
    [
        "Go through all of your messages",
        " and order Teamster #1 home to Stove Top.",
        "An easy year, that one.",
        "Generate the year.",
        " ",
        " ",
        " ",
        " ",
    ],
    // --- Year 2418 ---------------------------------------------------------
    // 50
    [
        "Start with your first message and put a Teamster in Stove Top's production queue.",
        "Read the next message and Goto 90210.",
        " The queue could be fiddled with, but it looks fine for another year or two.",
        "Read the next two messages and open the Research dialog.",
        " Click the entries in the Expected benefits box one by one.",
        " The Stargate sounds like fun, and the Frigate is interesting too.",
        " Close the dialog without changing anything.",
        "Go through your remaining messages and generate the year.",
    ],
    // --- Year 2419 ---------------------------------------------------------
    // 51
    [
        "Read your first message and fill Teamster #12 with colonists.",
        " Give it a waypoint at Wallaby to Unload All Colonists.",
        " Shift-click back on Stove Top and set that task to Load All Available.",
        " Tick Repeat Orders in the Fleet Waypoints tile.",
        "The Est Fuel Usage says you need more fuel than you carry, but once the colonists are off at Wallaby",
        " the fleet is lighter and the trip home takes far less.",
        "Read the next message. Goto the new Teamster",
        " and put colonists aboard.",
    ],
    // 52
    [
        "Give it a waypoint at Oxygen to Unload All Colonists.",
        " Shift-click back on Stove Top and set that task to Load All Available.",
        " Tick Repeat Orders in the Fleet Waypoints tile.",
        "Both freighters will keep carrying colonists away from Stove Top until told to stop.",
        "Read the next message and add a Max Terraform (Auto Build) at the foot of 90210's production queue.",
        "Read the next 3 messages and send Teamster #7 back to Stove Top.",
        "Take your last message and generate the year.",
        " ",
    ],
    // --- Year 2420 ---------------------------------------------------------
    // 53
    [
        "Start with your first two messages and order Armed Probe #9 to La Te Da.",
        " Read the next 3 messages and set 'Next field to research' to Weapons.",
        "Read your last message and press F4 to open the Ship Designer.",
        " Pick Starbases and Copy Selected Design.",
        " Choose the Orbital parts category and drag a Stargate 100/250 onto the 'Orbital or Elect' slot on the left of the hull.",
        "Rename the design Gater, press the right arrow under the starbase picture once, OK the design and Close the dialog.",
        "Put a Gater in Stove Top's queue",
        " and then generate.",
    ],
    // --- Year 2421 ---------------------------------------------------------
    // 54
    [
        "Read your first message, fill Teamster #1 with colonists",
        " and order it to Unload All Colonists at Wallaby.",
        "This comes up often enough to be worth shortening.",
        " Right-click the blue diamond in the Waypoint Task tile and choose Customize.",
        " Press Import, name the order DropCol, and press OK on both dialogs.",
        " From now on a fleet can be given this job with far fewer clicks.",
        "Shift-click on Stove Top and set the transport option to Load All Available.",
        " Then tick Repeat Orders.",
    ],
    // 55
    [
        "Go through the rest of your messages.",
        " The starbase at Stove Top has been upgraded, but there is no second planet with a starbase to jump to yet.",
        " Press F3 to open the Planet Summary Report.",
        "Find the Min Conc column, right-click it, and Reverse Sort by Mineral Concentration - Weighted Average.",
        " Oxygen, Sea Squared, Red Storm and Wallaby all have fine mineral concentrations, but",
        " Wallaby has the most people and lies nearest the Berserker worlds.",
        " Wallaby's population is still falling, though, so keep the colonists coming for a while before queueing a Gater there.",
        "Generate when you are ready.",
    ],
    // --- Year 2422 ---------------------------------------------------------
    // 56
    [
        "Start with your first message and Goto Teamster #4.",
        " With the Mini-Miner added at Prune, each load is now more than the freighter can carry on one tank of fuel.",
        "Teamster #4 could fly each leg slower, or take on some fuel from Cotton Picker #6.",
        "Since the miners now turn out more",
        " than one freighter can carry per trip, the answer is another freighter merged into Teamster #4.",
        "Drag in the fuel gauge of the Other Fleets Here tile until Teamster #4 holds 383mg of fuel.",
        " ",
        " ",
    ],
    // 57
    [
        "Put a Teamster in Stove Top's queue.",
        "Read the next 4 messages and set 'Next field to research' to Propulsion.",
        "Go through your remaining messages and open the Ship Designer.",
        " View Available Hull Types, pick Frigate from the dropdown and press Copy Selected Design.",
        "Drag a Daddy Long Legs 7 onto the Engine slot.",
        " Choose Mine Layers from the dropdown and drag 3 Mine Dispenser 50s onto the General Purpose slot.",
        "Rename the design Mine Layer, OK it, and press Done to close the dialog.",
        " Put a Mine Layer in Stove Top's queue.",
    ],
    // 58
    [
        "Click the red triangle beside Wallaby.",
        " It is a Berserker colony ship, bound for No Vacancy at warp 6.",
        "Right-click on Wallaby and pick Stalwart Defender #5.",
        " Then shift-click on the enemy fleet.",
        "That should do for this year.",
        "Generate when you are ready.",
        " ",
        " ",
    ],
    // --- Year 2423 ---------------------------------------------------------
    // 59
    [
        "Start with your first message and Goto Stalwart Defender #5.",
        " The fleet is drawn in purple on the Scanner, which means an enemy is at the same spot.",
        " So you caught their colony ship, but did not manage to destroy it.",
        " A message about the battle will come later.",
        "For now, read the next message and Goto Teamster #7.",
        "Open the Planet Summary Report and sort it by Population.",
        " Oxygen has the fewest people, but a freighter is already on its way there. Sea Squared is next lowest.",
        "Press ESC to close the report.",
    ],
    // 60
    [
        "Fill Teamster #7 with colonists and send it to Sea Squared.",
        " make the waypoint task Transport,",
        " right-click the blue diamond and choose DropCol.",
        "Read the next two messages and Goto the new Teamster.",
        " This is the freighter built to merge with the one shuttling to Prune.",
        "Pick Teamster #4 in the list on the Other Fleets Here tile and press Goto.",
        "Press Merge in the Fleet Composition tile.",
        "Click Teamster #3 in the Merge Fleets dialog and press OK.",
    ],
    // 61
    [
        "Read the next message and give Mine Layer #8 a Lay Mine Field task at Stove Top.",
        "Read the next message and add a Mini-Miner to Stove Top's queue.",
        "Read the next 2 messages and watch the battle.",
        " The Berserker Santa Maria is 80% damaged; it can be finished off next year.",
        " Right-click the blue diamond in the Fleet Waypoints tile and pick Berserker Santa Maria #5.",
        "Read your last message and double-click Armed Probe #9, just above La Te Da.",
        " Drag its waypoint from La Te Da to Speed Bump,",
        " then shift-click on Lever and generate.",
    ],
    // --- Year 2424 ---------------------------------------------------------
    // 62
    [
        "Start with your first message and order Stalwart Defender #5 back to Wallaby.",
        "Read the next message, Goto Mini-Miner #3 and send it to Prune with a Merge with Fleet task.",
        "Go through your remaining messages.",
        "That is all,",
        " generate whenever you like.",
        " ",
        " ",
        " ",
    ],
    // --- Year 2425 ---------------------------------------------------------
    // 63
    [
        "Start with your first two messages.",
        " Press the % button on the toolbar.",
        " Notice how many green worlds there are waiting to be settled. Before that, though, let's improve the colony ship design.",
        "Press F4 to open the Ship Designer.",
        "Pick the Santa Maria from the dropdown",
        " and press Edit Selected Design.",
        "Drag the Long Hump 6 engine off the design back into the parts list, and put a Daddy Long Legs 7 in its place.",
        " OK the design and press Done to close the dialog.",
    ],
    // 64
    [
        "Put 3 of the improved Santa Marias in Stove Top's production queue.",
        "Read the next message and Goto Sea Squared.",
        " Sea Squared has as many factories and mines as it can run.",
        " Add 'Max Terraform (Auto Build) Up To 2%' at the END of its production queue.",
        "Read the next 2 messages and set 'Next field to research' to Construction.",
        "Go through the rest of your messages",
        " and",
        " generate when you are ready.",
    ],
    // --- Year 2426 ---------------------------------------------------------
    // 65
    [
        "Start with your first three messages",
        " and Goto the 3 new Santa Marias.",
        " Fill them with colonists and send them to settle Lever.",
        "Press Split All in the Fleet Composition tile.",
        " Drag the waypoint of Santa Maria #10 to Speed Bump",
        " and that of Santa Maria #11 to Bloop.",
        "Go through the rest of your messages.",
        " ",
    ],
    // 66
    [
        "Teamster #12 reaches Stove Top next year.",
        " Put 3 Teamsters in Stove Top's queue",
        " to build up the troop lift.",
        "Click the enemy ship near Wallaby.",
        " The Berserkers are having another go at settling No Vacancy. Some people never learn.",
        " Select Stalwart Defender #5 and drag its destination onto the enemy colony ship.",
        "Next year a new destroyer will be needed, to go after wherever those colony ships come from.",
        "Generate when you are ready.",
    ],
    // --- Year 2427 ---------------------------------------------------------
    // 67
    [
        "Start with your first message and Goto Stalwart Defender #5.",
        " Once again the colony ship was hurt but not finished.",
        "Right-click the blue diamond in the Fleet Waypoints tile and pick Berserker Santa Maria #6.",
        " That orders your destroyer to chase the enemy colony ship down and destroy it.",
        "Read the next message and Goto Armed Probe #9.",
        " It can stay where it is and guard Lever, which is too fine a world to leave alone.",
        "Read the next message, Goto the new fleet",
        " and load it with colonists.",
    ],
    // 68
    [
        "The new Teamsters belong with the other fleet carrying colonists to Wallaby.",
        " Select Teamster #12, press Merge in the Fleet Composition tile, and merge it into Teamster #13.",
        "Read the next message.",
        " Stove Top can wait until all the messages have been read.",
        "Read the rest of your messages, watching the battle with the colony ship if you like.",
        "As promised last year, let's design a destroyer to deal with the Berserker starbase at Hacker.",
        "Press F4 to open the Ship Designer.",
        " ",
    ],
    // 69
    [
        "It should hit hard, but weigh no more than 100kT, so it can use the 100kT stargates once those are built.",
        "Pick Available Hull Types, choose Destroyer from the dropdown and press Copy Selected Design.",
        "Fit a Radiating Hydro-Ram Scoop, 2 Carbonic armors and 3 Yakimora Light Phasers.",
        " Put a fuel tank in the Mechanical slot and a Battle Computer in the Electrical slot.",
        " The design comes to 97kT all told.",
        " Press the right arrow under the ship picture, OK the design and close the dialog.",
        "Queue 10 Destroyers at Stove Top",
        " and then generate.",
    ],
    // --- Year 2428 ---------------------------------------------------------
    // 70
    [
        "Start with your first message and Goto Stalwart Defender #5.",
        " Once more,",
        " order it to Wallaby.",
        "Read the next 4 messages and Goto the new Destroyer armada.",
        "Send it to make trouble at the Berserker starbase at Hacker.",
        "Go through the rest of your messages",
        " and",
        " generate the year.",
    ],
    // --- Year 2429 ---------------------------------------------------------
    // 71
    [
        "Start with your first message and Goto Teamster #4.",
        " The leg to Stove Top needs slowing down: the minerals mined at Prune are outrunning what can be carried.",
        "Click Stove Top in the Fleet Waypoints tile and lower the Warp Gauge to 5.",
        " The trip home takes just one year longer.",
        "Take the next 4 messages.",
        " Send the new Destroyer to Hacker as well.",
        " Rather than send every new fleet to Hacker by hand, let's set up a Route.",
        " Select Stove Top, then Control-click on Hacker.",
    ],
    // 72
    [
        "Notice the Production tile now says new ships are routed to Hacker.",
        "Take the next 3 messages.",
        " Open the Research dialog and set Next field to research to Energy.",
        "Go through the rest of your messages",
        " and then Goto Teamster #7.",
        " It has too little fuel to get back to Stove Top, so its parts may as well be put to use.",
        " Order Teamster #7 to Scrap Fleet.",
        " ",
    ],
    // --- Year 2430 ---------------------------------------------------------
    // 73
    [
        "Let's finish the Berserkers for good by building a bombing fleet.",
        "Press F4 to open the Ship Designer.",
        " Pick Available Hull Types, choose B-17 Bomber from the dropdown and press Copy Selected Design.",
        " Fit Radiating Hydro-Ram Scoop engines,",
        " then hold down shift and drag 4 Black Cat Bombs onto each bomb slot. Put a Fuel Tank in the slot that is left.",
        " OK the design and shut the ship designer.",
        "Put 10 B-17 Bombers in Stove Top's production queue.",
        "Generate when you are ready.",
    ],
    // --- Year 2431 ---------------------------------------------------------
    // 74
    [
        "Congratulations — you have been declared the winner.",
        " The tutorial carries on a few more years with some further pointers, and then the mopping up is yours to finish.",
        "Read your first 3 messages and Goto the new B-17s.",
        " Notice they have already been routed to Hacker.",
        "Go through the rest of your messages.",
        " Everything else is running itself.",
        "Generate when you are ready.",
        " ",
    ],
    // --- Year 2432 ---------------------------------------------------------
    // 75
    [
        "Start with your first 4 messages and Goto Wallaby.",
        " The terraforming has finally paid off. A starbase here would be welcome, but the minerals are not there for it.",
        "Add 100 mines to Wallaby's queue.",
        " (Control-click the Add button to add 100 of an item.)",
        "Go through the rest of your messages.",
        " Nothing presses this year. Next year the armada should reach Hacker.",
        "Generate whenever you like.",
        " ",
    ],
    // --- Year 2433 ---------------------------------------------------------
    // 76
    [
        "Start with your first 3 messages and Goto Destroyer #13.",
        " Notice your 9 Destroyers are drawn with a red bar about halfway across their name in the Fleet Composition tile: the ships are damaged.",
        " Click the Destroyer in the Fleet Composition tile.",
        " At 800x600 or better you will see that the ships are 51% damaged.",
        "Read the next 8 messages and watch your attack on the enemy starbase.",
        " The Berserkers should not be building any more colony ships now... though notice the one that slipped our trap, still bound for No Vacancy.",
        "Go through the rest of your messages and generate.",
        " ",
    ],
    // --- Year 2434 ---------------------------------------------------------
    // 77
    [
        "Start with your first 6 messages and Goto Stove Top.",
        "Put another 10 B-17 Bombers in the production queue.",
        "Nothing to do now but wait for the bombers to reach Hacker.",
        "Go through the rest of your messages and watch the battles.",
        "Generate when you are ready.",
        " ",
        " ",
        " ",
    ],
    // --- Year 2435 ---------------------------------------------------------
    // 78
    [
        "A quiet year. Watch the battle at Hacker and you will see your destroyers are too slow to get within range of the Berserker mine layers.",
        "A faster ship is wanted: better engines, or Maneuvering Jets in the design.",
        "The first bombers arrive next year.",
        "Go through all your messages and generate when you are ready.",
        " ",
        " ",
        " ",
        " ",
    ],
    // --- Year 2436 ---------------------------------------------------------
    // 79
    [
        "Work through your messages.",
        " Your 2 B-17 Bombers killed a few of the enemy without making much of a mark.",
        " The ones arriving over the next few years should tell.",
        "There is plenty that could be done on the planets, but with the Berserkers so near their end it is hardly worth the time.",
        "Generate when you are ready.",
        " ",
        " ",
        " ",
    ],
    // 80
    [
        "Congratulations — you have reached the end of the tutorial.",
        "Press F10 to see the score. Notice the Berserkers still hold 3 planets and 3 ships.",
        "Finish bombing the Berserker planets, and build a ship that can deal with those troublesome Berserker Saguaros.",
        "Only then will the galaxy truly be yours.",
        "Go through all your messages,",
        " and once you Generate, you are on your own!",
        " ",
        " ",
    ],
];
