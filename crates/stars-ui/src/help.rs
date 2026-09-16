//! The help viewer's state — what `WINHELP.EXE` keeps for the player's guide.
//!
//! The game itself has no help code: every Help button is one `WinHelp`
//! call naming `STARS!.HLP` and a context number (see
//! `docs/formats/help.md` for the table), and Windows' own viewer does the
//! rest. So this is a small model of that viewer — the topic on show, the
//! Back stack, the browse buttons, the Search and History windows and the
//! popups — over [`stars_formats::HelpFile`]. The window it draws into is
//! `views::help`.

use std::collections::HashMap;

use stars_formats::help::{Hotspot, Jump, Topic};
use stars_formats::HelpFile;

/// The context numbers the original's dialogs ask for, one per `WINHELP`
/// call site; the addresses are where each `MOV AX, imm` sits before the
/// call at `14f8:029c`.
pub mod context {
    /// The Help menu's Introduction (`CommandHandler`, `1020:479e`).
    pub const INTRODUCTION: u32 = 0x1195;
    /// The cargo transfer dialog (`TransferDlg`, `1050:59be`).
    pub const CARGO_TRANSFER: u32 = 0x433;
    /// The ship transfer dialog — the same routine with `[0x984] == 1`
    /// (`1050:59b7`).
    pub const SHIP_TRANSFER: u32 = 0x438;
    /// The Battle VCR (`VCRDlg`, `10e8:18b3`).
    pub const BATTLE_VCR: u32 = 0x43a;
    /// Player Relations (`RelationsDlg`, `10f0:0434`).
    pub const RELATIONS: u32 = 0x43b;
    /// Battle Plans and its name prompt (`BattlePlansDlg`, `10f0:16ac`;
    /// `NewPlanNameDlg`, `10f0:05f8`).
    pub const BATTLE_PLANS: u32 = 0x439;
    /// Research (`ResearchDlg`, `10d8:088f`).
    pub const RESEARCH: u32 = 0x42e;
    /// Production (`ProdCommandHandler`, `10d0:3393`).
    pub const PRODUCTION: u32 = 0x423;
    /// The production templates dialog (`ZipProdDlg`, `10d0:5d8e`).
    pub const PRODUCTION_TEMPLATES: u32 = 0x452;
    /// Custom zip orders (`ZipOrderDlg`, `1080:07cf`).
    pub const ZIP_ORDERS: u32 = 0x44a;
    /// Rename Fleet (`RenameDlg`, `1080:0ca3`).
    pub const RENAME_FLEET: u32 = 0x447;
    /// Merge Fleets (`MergeFleetsDlg`, `1080:35f7`).
    pub const MERGE_FLEETS: u32 = 0x453;
    /// The Ship Designer (`SlotDlg`, `10c8:25bd`), and the page for building
    /// a ship from scratch it asks for instead when `[0x5466] == 4`.
    pub const SHIP_DESIGNER: u32 = 0x42a;
    /// See [`SHIP_DESIGNER`].
    pub const SHIP_DESIGNER_EDIT: u32 = 0xbdf;
    /// Host mode and its options (`HostModeDialog`, `1020:753f`;
    /// `HostOptionsDialog`, `1020:7694`).
    pub const HOST_MODE: u32 = 0x440;
    /// The password prompt (`PasswordDlg`, `1040:5c5f`) — a number the
    /// file has no topic for.
    pub const PASSWORD: u32 = 0x441;
    /// Change Password (`NewPasswordDlg`, `1040:5f65`).
    pub const CHANGE_PASSWORD: u32 = 0x43c;
    /// Find (`FindDlg`, `1058:9400`).
    pub const FIND: u32 = 0x43d;
    /// The score sheet (`ScoreXDlg`, `1108:12c1`).
    pub const SCORE: u32 = 0x455;
    /// The save-on-exit question (`AskSaveDialog`, `1070:4399`) — another
    /// number the file has no topic for.
    pub const ASK_SAVE: u32 = 0x442;
    /// Print Map (`PrintMapDlg`, `1108:a3c0`).
    pub const PRINT_MAP: u32 = 0xc3c;
    /// The basic New Game dialog (`SimpleNewGameDlg`, `1078:7e2f`).
    pub const NEW_GAME_SIMPLE: u32 = 0x3ea;
    /// The advanced New Game's three steps (`NewGameDlg`…`3`, `1078:8517`,
    /// `1078:9441`, `1078:9c00`).
    pub const NEW_GAME_STEPS: [u32; 3] = [0x3f4, 0x3fc, 0x3fd];
    /// The Race Wizard's six pages (`RaceWizardDlg1`…`6`, `10e0:0bd3`,
    /// `161f`, `2967`, `35c5`, `3a83`, `3f3b`).
    pub const RACE_WIZARD_PAGES: [u32; 6] = [0x3ff, 0x41d, 0x420, 0x408, 0x411, 0x421];
}

/// The Search window (`Search` on the button bar): a word typed or picked,
/// and the topics it is filed under once Show Topics has been pressed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Search {
    /// What has been typed, which the keyword list scrolls to.
    pub text: String,
    /// The keyword picked from the list, by index.
    pub keyword: Option<usize>,
    /// The topics shown for it, once asked for: `(offset, title)`.
    pub topics: Vec<(i32, String)>,
    /// The topic picked from those, by index.
    pub topic: Option<usize>,
}

/// One notice the viewer puts up instead of a topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// No copy of `STARS!.HLP` was found.
    NoFile,
    /// The file has no topic for the number asked for — WinHelp's "Help
    /// topic does not exist".
    NoTopic(u32),
    /// The viewer's About box.
    About,
}

/// The viewer.
#[derive(Default)]
pub struct Help {
    file: Option<HelpFile>,
    /// Where the file came from, for the record.
    pub source: String,
    /// Whether the window is up.
    pub open: bool,
    /// The topic on show.
    topic: Option<Topic>,
    /// The topics left by Back, most recent last.
    back: Vec<i32>,
    /// Every topic shown, most recent first — the History window's list.
    pub visited: Vec<i32>,
    /// A popup, while one is up: its topic and where it was raised.
    pub popup: Option<(Topic, [f32; 2])>,
    /// How many frames the popup has been up, so the click that raised it
    /// does not also dismiss it.
    pub popup_age: u32,
    /// The Search window, while it is up.
    pub search: Option<Search>,
    /// Whether the History window is up.
    pub history_open: bool,
    /// A notice to show instead.
    pub notice: Option<Notice>,
    /// Pictures already handed to the renderer, by number, with their
    /// hotspots.
    pub textures: HashMap<u16, (egui::TextureHandle, Vec<Hotspot>)>,
    /// Pictures that would not decode, so they are not asked for again.
    pub undecodable: std::collections::HashSet<u16>,
}

impl std::fmt::Debug for Help {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Help")
            .field("file", &self.file.is_some())
            .field("open", &self.open)
            .field("topic", &self.topic.as_ref().map(|t| t.offset))
            .field("back", &self.back)
            .finish_non_exhaustive()
    }
}

impl Help {
    /// The file, once one has been loaded.
    #[must_use]
    pub fn file(&self) -> Option<&HelpFile> {
        self.file.as_ref()
    }

    /// The topic on show.
    #[must_use]
    pub fn topic(&self) -> Option<&Topic> {
        self.topic.as_ref()
    }

    /// The title on show — the topic's, or the file's while nothing is.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.topic.as_ref().map(|t| t.title.as_str())
    }

    /// Whether Back has anywhere to go.
    #[must_use]
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    /// The window's caption: the file's title, as `WINHELP` shows it.
    #[must_use]
    pub fn caption(&self) -> String {
        self.file
            .as_ref()
            .map_or_else(|| "Help".to_string(), |f| f.title().to_string())
    }

    /// Show a topic, pushing the one on show onto the Back stack.
    fn show(&mut self, offset: i32) -> bool {
        let Some(file) = self.file.as_ref() else {
            return false;
        };
        let Ok(topic) = file.topic(offset) else {
            return false;
        };
        if let Some(was) = self.topic.take() {
            if was.offset != offset {
                self.back.push(was.offset);
            }
        }
        self.visited.retain(|&v| v != offset);
        self.visited.insert(0, offset);
        self.topic = Some(topic);
        self.popup = None;
        self.open = true;
        self.notice = None;
        true
    }
}

impl crate::App {
    /// Take the help file in, from wherever the shell found it.
    ///
    /// # Errors
    ///
    /// The reader's complaint, when the bytes are not a help file this
    /// project can read.
    pub fn load_help(&mut self, bytes: Vec<u8>, source: &str) -> Result<(), String> {
        let file = HelpFile::read(bytes).map_err(|e| format!("{source}: {e}"))?;
        self.help.file = Some(file);
        self.help.source = source.to_string();
        Ok(())
    }

    /// Whether a help file has been loaded.
    #[must_use]
    pub fn has_help(&self) -> bool {
        self.help.file.is_some()
    }

    /// `WinHelp(HELP_CONTEXT, id)`: open the viewer on the topic a context
    /// number names, or put up the notice WinHelp would.
    pub fn help_context(&mut self, id: u32) {
        let Some(file) = self.help.file.as_ref() else {
            self.help.notice = Some(Notice::NoFile);
            return;
        };
        match file.topic_for_context(id) {
            Some(offset) if self.help.show(offset) => {}
            _ => self.help.notice = Some(Notice::NoTopic(id)),
        }
    }

    /// `WinHelp(HELP_INDEX)` and the Contents button: the contents page.
    pub fn help_contents(&mut self) {
        let Some(file) = self.help.file.as_ref() else {
            self.help.notice = Some(Notice::NoFile);
            return;
        };
        let contents = file.contents();
        self.help.show(contents);
    }

    /// Follow a hotspot.
    pub fn help_jump(&mut self, jump: &Jump) {
        match jump {
            Jump::Topic(offset) => {
                self.help.show(*offset);
            }
            Jump::Popup(offset) => {
                self.help_popup(*offset, [0.0, 0.0]);
            }
            Jump::Macro(_) | Jump::Missing => {}
        }
    }

    /// Open a topic by its offset — a hotspot, a History entry, a Search
    /// result.
    pub fn help_goto(&mut self, offset: i32) -> bool {
        self.help.show(offset)
    }

    /// Raise a popup topic at a point of the screen.
    pub fn help_popup(&mut self, offset: i32, at: [f32; 2]) {
        let Some(file) = self.help.file.as_ref() else {
            return;
        };
        if let Ok(topic) = file.topic(offset) {
            self.help.popup = Some((topic, at));
        }
    }

    /// Back: the topic shown before this one.
    pub fn help_back(&mut self) -> bool {
        let Some(offset) = self.help.back.pop() else {
            return false;
        };
        let Some(file) = self.help.file.as_ref() else {
            return false;
        };
        let Ok(topic) = file.topic(offset) else {
            return false;
        };
        self.help.topic = Some(topic);
        self.help.popup = None;
        self.help.visited.retain(|&v| v != offset);
        self.help.visited.insert(0, offset);
        true
    }

    /// `<<` and `>>`: the previous or next topic of the browse sequence.
    pub fn help_browse(&mut self, forward: bool) -> bool {
        let Some(topic) = self.help.topic.as_ref() else {
            return false;
        };
        let next = if forward {
            topic.browse_forward
        } else {
            topic.browse_back
        };
        next.is_some_and(|offset| self.help.show(offset))
    }

    /// Whether `<<` or `>>` has anywhere to go.
    #[must_use]
    pub fn help_can_browse(&self, forward: bool) -> bool {
        self.help.topic.as_ref().is_some_and(|topic| {
            if forward {
                topic.browse_forward.is_some()
            } else {
                topic.browse_back.is_some()
            }
        })
    }

    /// Close the viewer, popups and all. The Back stack is kept, as
    /// WinHelp keeps it for the session.
    pub fn help_close(&mut self) {
        self.help.open = false;
        self.help.popup = None;
        self.help.search = None;
        self.help.history_open = false;
    }

    /// Open the Search window.
    pub fn help_search_open(&mut self) {
        if self.help.file.is_some() {
            self.help.search = Some(Search::default());
        }
    }

    /// The keywords that match what has been typed so far, as indexes into
    /// the file's list — the whole list when nothing has.
    #[must_use]
    pub fn help_keyword_matches(&self) -> Vec<usize> {
        let Some(file) = self.help.file.as_ref() else {
            return Vec::new();
        };
        let typed = self
            .help
            .search
            .as_ref()
            .map(|s| s.text.to_ascii_lowercase())
            .unwrap_or_default();
        file.keywords()
            .iter()
            .enumerate()
            .filter(|(_, k)| k.word.to_ascii_lowercase().starts_with(&typed))
            .map(|(i, _)| i)
            .collect()
    }

    /// Show Topics: list the topics the picked keyword is filed under.
    pub fn help_show_topics(&mut self) {
        let Some(file) = self.help.file.as_ref() else {
            return;
        };
        let Some(search) = self.help.search.as_mut() else {
            return;
        };
        let Some(keyword) = search.keyword.and_then(|k| file.keywords().get(k)) else {
            return;
        };
        search.topics = keyword
            .topics
            .iter()
            .map(|&offset| {
                (
                    offset,
                    file.title_of(offset).unwrap_or("(untitled)").to_string(),
                )
            })
            .collect();
        search.topic = if search.topics.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Go To: open the picked topic and close the Search window.
    pub fn help_search_go(&mut self) -> bool {
        let Some(search) = self.help.search.as_ref() else {
            return false;
        };
        let Some(&(offset, _)) = search.topic.and_then(|t| search.topics.get(t)) else {
            return false;
        };
        let shown = self.help.show(offset);
        if shown {
            self.help.search = None;
        }
        shown
    }
}
