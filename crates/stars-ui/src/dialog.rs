//! Dialog templates read out of the game's own resources.
//!
//! The original's dialogs are Windows `DIALOG` resources, so their layout is
//! data rather than code: a size and a list of controls, each with a class, a
//! position and a caption, all in **dialog units**. Reproducing one properly
//! means laying the controls out where the template puts them rather than
//! stacking them in whatever order they are written.
//!
//! A dialog unit is a quarter of the font's average character width across and
//! an eighth of its height down, so the same template comes out at different
//! pixel sizes under different fonts. Here the whole dialog is scaled to the
//! room it is given, which keeps the proportions the template asks for.

/// What one horizontal dialog unit is worth in pixels, for MS Sans Serif 8pt
/// at 96 dpi — the font every one of these dialogs asks for.
pub const DLU_X: f32 = 1.5;
/// And one vertical unit: the font's height is thirteen, over eight.
pub const DLU_Y: f32 = 13.0 / 8.0;

/// What a control is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// A push button, a checkbox or a radio button, by its style.
    Button,
    /// A list box.
    ListBox,
    /// A label.
    Static,
    /// A combo box.
    ComboBox,
    /// A text field.
    Edit,
}

/// One control of a dialog template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    /// The control id `WM_COMMAND` names it by.
    pub id: u16,
    /// What it is.
    pub class: Class,
    /// Where it goes and how big it is, in dialog units.
    pub at: (i16, i16, i16, i16),
    /// Its caption, `&` and all — the ampersand marks the accelerator.
    pub text: &'static str,
}

impl Control {
    /// The caption with the accelerator marker taken out.
    #[must_use]
    pub fn label(&self) -> String {
        self.text.replace('&', "")
    }
}

/// Where one control goes, given the dialog's own origin and scale.
#[must_use]
pub fn place(origin: egui::Pos2, scale: f32, at: (i16, i16, i16, i16)) -> egui::Rect {
    let (x, y, w, h) = at;
    egui::Rect::from_min_size(
        origin + egui::vec2(f32::from(x) * DLU_X * scale, f32::from(y) * DLU_Y * scale),
        egui::vec2(f32::from(w) * DLU_X * scale, f32::from(h) * DLU_Y * scale),
    )
}

/// A dialog template.
#[derive(Debug, Clone, Copy)]
pub struct Template {
    /// The title bar's text.
    pub caption: &'static str,
    /// How big the dialog is in dialog units.
    pub size: (i16, i16),
    /// Its controls, in the template's own order.
    pub controls: &'static [Control],
}

impl Template {
    /// Where a control sits, scaled into `rect`.
    ///
    /// The whole dialog is scaled by the smaller of the two ratios so the
    /// template's proportions survive being squeezed.
    #[must_use]
    pub fn place(&self, rect: egui::Rect, control: &Control) -> egui::Rect {
        let scale = self.scale(rect);
        let (x, y, w, h) = control.at;
        egui::Rect::from_min_size(
            rect.min + egui::vec2(f32::from(x) * DLU_X * scale, f32::from(y) * DLU_Y * scale),
            egui::vec2(f32::from(w) * DLU_X * scale, f32::from(h) * DLU_Y * scale),
        )
    }

    /// How much the template has to be squeezed to fit `rect`.
    #[must_use]
    pub fn scale(&self, rect: egui::Rect) -> f32 {
        let want = self.pixels();
        (rect.width() / want.x)
            .min(rect.height() / want.y)
            .clamp(0.4, 1.0)
    }

    /// How big the dialog wants to be, in pixels.
    #[must_use]
    pub fn pixels(&self) -> egui::Vec2 {
        egui::vec2(
            f32::from(self.size.0) * DLU_X,
            f32::from(self.size.1) * DLU_Y,
        )
    }

    /// One control, by id.
    #[must_use]
    pub fn control(&self, id: u16) -> Option<&'static Control> {
        self.controls.iter().find(|control| control.id == id)
    }
}

/// Where the buttons in the production dialog's middle column go, once the
/// two overlaps in the shipped resource are opened out.
///
/// The template really does place `Item Up` at `y = 0` and `Add ->` at
/// `y = 10`, and `Clear` at `y = 60` and `Help` at `y = 70`, with every button
/// fourteen units tall — so each pair overlaps by four. That is what is in the
/// file (resource 93, thirteen controls, no extra data on any of them), and it
/// is reproduced in [`PRODUCTION`] unchanged. This is the same column with
/// each overlap pushed apart, which is what gets drawn, because two buttons
/// on top of each other would leave one of them unclickable.
#[must_use]
pub fn unoverlapped(template: &Template) -> Vec<(u16, (i16, i16, i16, i16))> {
    let mut out: Vec<(u16, (i16, i16, i16, i16))> = template
        .controls
        .iter()
        .map(|control| (control.id, control.at))
        .collect();
    // Only the middle column, and only downwards, so the row along the foot
    // and the two lists stay exactly where the template puts them.
    let mut column: Vec<usize> = (0..out.len()).filter(|&i| out[i].1 .0 == 125).collect();
    column.sort_by_key(|&i| out[i].1 .1);
    for pair in column.windows(2) {
        let (above, below) = (pair[0], pair[1]);
        let bottom = out[above].1 .1 + out[above].1 .3;
        if out[below].1 .1 < bottom {
            out[below].1 .1 = bottom;
        }
    }
    out
}

/// `Planet Production`, dialog resource 93.
///
/// Thirteen controls: the inventory and the queue side by side, the buttons
/// that move items between them in a column down the middle, and a row along
/// the foot. Everything between the lists and that row is drawn by
/// `DrawProductionDlg` (`10d0:35dc`) rather than being controls — the cost
/// panel under each list, and the blue diamond.
pub const PRODUCTION: Template = Template {
    caption: "Planet Production",
    size: (294, 191),
    controls: &[
        Control {
            id: 0x416,
            class: Class::ListBox,
            at: (8, 12, 109, 84),
            text: "",
        },
        Control {
            id: 0x417,
            class: Class::ListBox,
            at: (174, 12, 111, 84),
            text: "",
        },
        Control {
            id: 0x439,
            class: Class::Button,
            at: (125, 0, 40, 14),
            text: "Item &Up",
        },
        Control {
            id: 0x418,
            class: Class::Button,
            at: (125, 10, 40, 14),
            text: "&Add ->",
        },
        Control {
            id: 0x419,
            class: Class::Button,
            at: (125, 35, 40, 14),
            text: "<- &Remove",
        },
        Control {
            id: 0x42d,
            class: Class::Button,
            at: (125, 60, 40, 14),
            text: "&Clear",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (125, 70, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x43a,
            class: Class::Button,
            at: (125, 90, 40, 14),
            text: "Item &Down",
        },
        Control {
            id: 0x8b,
            class: Class::Button,
            at: (6, 168, 90, 14),
            text: "Contribute only &leftover resources to research",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (100, 168, 40, 14),
            text: "&Prev",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (147, 168, 40, 14),
            text: "&Next",
        },
        Control {
            id: 0x1,
            class: Class::Button,
            at: (198, 168, 40, 14),
            text: "OK",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (245, 168, 40, 14),
            text: "Cancel",
        },
    ],
};

/// `Required Minerals:` — the heading of the panel `DrawProductionDlg` draws
/// under each list.
pub const REQUIRED_MINERALS: &str = "Required Minerals:";

/// The four rows of that panel, which are the first three of `rgpszMin` and
/// then its **sixth**: the three minerals and then resources.
pub const COST_ROWS: [(&str, [u8; 3]); 4] = [
    ("Ironium", [0x00, 0x00, 0xff]),
    ("Boranium", [0x00, 0x7f, 0x00]),
    ("Germanium", [0xff, 0xff, 0x00]),
    // `rgcrMin[5]` is black, and the row carries no `kT` after it.
    ("Resources", [0x00, 0x00, 0x00]),
];

/// What follows the first three rows' figures (`idsKt`).
pub const KT: &str = "kT";

/// The completion line under the queue's own cost panel
/// (`idsDDoneCompletion`), with `PszProductionETA`'s wording after it.
pub const COMPLETION: &str = "% Done,   Completion ";

/// What is written beside the blue diamond
/// (`idsApplyDefineProductionTemplate`).
pub const TEMPLATE_HINT: &str = "Apply or define a production template";

/// The diamond's own colour, `hbrBBlue`.
pub const DIAMOND: [u8; 3] = [0x00, 0x00, 0xff];

/// Where the diamond goes, given the dialog's client rectangle and a line of
/// its font.
///
/// `DrawProductionDlg` puts it `dyArial8 * 5 / 2 + 12` up from the bottom, six
/// pixels in, `dyArial8` wide and `dyArial8 | 1` tall — odd, so it has a
/// middle row to be pointed at.
#[must_use]
pub fn diamond(rect: egui::Rect, line: f32) -> egui::Rect {
    #[allow(clippy::cast_possible_truncation)]
    let odd = ((line as i32) | 1) as f32;
    let top = rect.bottom() - (line * 2.5 + 12.0);
    egui::Rect::from_min_size(egui::pos2(rect.left() + 6.0, top), egui::vec2(line, odd))
}

/// `Ship & Starbase Designer`, dialog resource 92 (`0x5c`).
///
/// One window with two faces. `ShowMainControls` (`10c8:0160`) swaps them by
/// hiding or showing nine of these fifteen controls — the two Design radios,
/// the four View radios and the three buttons — and doing two more things that
/// read backwards at first glance: **OK is hidden in the browser and shown in
/// the editor**, and the button beside it is relabelled `Done` for the browser
/// and `Cancel` for the editor. So the browser's only way out is the button
/// the template calls `Cancel`.
///
/// The template places the dialog at 11, 52 rather than centring it, and its
/// parts list runs ten units past the bottom of the dialog it is in — 90 plus
/// 170 against a height of 250. That is what the resource says.
pub const DESIGNER: Template = Template {
    caption: "Ship & Starbase Designer",
    size: (351, 250),
    controls: &[
        Control {
            id: 0x810,
            class: Class::Button,
            at: (14, 8, 91, 13),
            text: "Ships",
        },
        Control {
            id: 0x811,
            class: Class::Button,
            at: (14, 21, 91, 13),
            text: "Starbases",
        },
        Control {
            id: 0x812,
            class: Class::Button,
            at: (14, 48, 91, 13),
            text: "Existing Designs",
        },
        Control {
            id: 0x813,
            class: Class::Button,
            at: (14, 62, 91, 13),
            text: "Available Hull Types",
        },
        Control {
            id: 0x814,
            class: Class::Button,
            at: (14, 76, 91, 13),
            text: "Enemy Hulls",
        },
        Control {
            id: 0x815,
            class: Class::Button,
            at: (14, 90, 91, 13),
            text: "Components",
        },
        Control {
            id: 0x816,
            class: Class::Button,
            at: (13, 114, 86, 16),
            text: "&Copy Selected Design",
        },
        Control {
            id: 0x817,
            class: Class::Button,
            at: (13, 134, 86, 16),
            text: "&Delete Selected Design",
        },
        Control {
            id: 0x818,
            class: Class::Button,
            at: (13, 154, 86, 16),
            text: "&Edit Selected Design",
        },
        Control {
            id: 0x81a,
            class: Class::ComboBox,
            at: (180, 20, 142, 78),
            text: "",
        },
        Control {
            id: 0x81b,
            class: Class::Edit,
            at: (186, 52, 132, 15),
            text: "",
        },
        Control {
            id: 0x80c,
            class: Class::ListBox,
            at: (114, 90, 135, 170),
            text: "",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (281, 134, 33, 13),
            text: "&Help",
        },
        Control {
            id: 0x1,
            class: Class::Button,
            at: (281, 222, 33, 13),
            text: "OK",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (281, 238, 33, 13),
            text: "Cancel",
        },
    ],
};

/// What the designer's second button reads in each face (`idsDone` and
/// `idsCancel`).
pub const DESIGNER_CLOSE: (&str, &str) = ("Done", "Cancel");

/// Which of the designer's controls `ShowMainControls` hides for the editor
/// and shows for the browser.
pub const DESIGNER_BROWSER_ONLY: [u16; 9] = [
    0x816, 0x818, 0x817, 0x810, 0x811, 0x812, 0x813, 0x814, 0x815,
];

/// Where a hull's schematic grid starts, and where the plaque under it goes.
///
/// `UpdateSlotGlobals` (`10c8:6528`) works in half-cells of 32 pixels — a slot
/// is two of them square — and puts the origin in one of two places depending
/// on which window is asking:
///
/// * the **designer** (`hwndSlotDlg != 0`): `ptslotGlob.x - 0x14a` across and
///   32 down;
/// * the `grPopupShdef` **pop-up**: 12 across and `dyArial8 + 12` down.
///
/// The plaque — the `n of m` under the picture — is a fixed offset from
/// whichever origin was used, and the hull's cargo bay comes out of
/// `HULDEF.wrcCargo`: its high byte is the top-left cell and its low byte the
/// bottom-right, each nibble a half-cell like `rgbrc`.
pub const SLOT_CELL: f32 = 32.0;
/// How far the designer's own grid is left of `ptslotGlob.x`.
pub const SLOT_ORIGIN_BACK: f32 = 0x14a as f32;
/// How far down the designer's grid starts.
pub const SLOT_ORIGIN_TOP: f32 = 32.0;
/// Where the plaque sits from the grid's origin.
pub const PLAQUE_OFFSET: (f32, f32) = (0x102 as f32, 0x111 as f32);

// --- The race wizard ------------------------------------------------------
//
// Six dialogs, `IDD_RACE_WIZARD_1` (146) through `_6` (151), all **261 by 209**
// dialog units and all carrying the same five-button footer at `y = 190`. The
// trap the spec already records is visible here: pages 2 and 3 hold almost no
// controls, because the habitability sliders and the economy bars are painted
// in `WM_PAINT` rather than laid out.

/// The five buttons every page of the wizard carries, in the template's own
/// left-to-right order.
pub const WIZARD_FOOTER: [u16; 5] = [0x76, 0x2, 0x42e, 0x42f, 0x430];

/// Where that footer sits — every page puts it at the same `y`.
pub const WIZARD_FOOTER_Y: i16 = 190;
/// `IDD_RACE_WIZARD_1` (resource 146) — the Race page.
///
/// 21 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_1: Template = Template {
    caption: "Race",
    size: (261, 209),
    controls: &[
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (6, 7, 68, 10),
            text: "Race Name:",
        },
        Control {
            id: 0x10c,
            class: Class::Edit,
            at: (76, 6, 90, 12),
            text: "",
        },
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (6, 23, 68, 10),
            text: "Plural Race Name:",
        },
        Control {
            id: 0x81b,
            class: Class::Edit,
            at: (76, 22, 90, 12),
            text: "",
        },
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (6, 39, 68, 10),
            text: "Password:",
        },
        Control {
            id: 0x10d,
            class: Class::Edit,
            at: (76, 38, 60, 12),
            text: "",
        },
        Control {
            id: 0x10f,
            class: Class::Button,
            at: (26, 64, 70, 14),
            text: "Humanoid",
        },
        Control {
            id: 0x110,
            class: Class::Button,
            at: (26, 80, 70, 14),
            text: "Rabbitoid",
        },
        Control {
            id: 0x111,
            class: Class::Button,
            at: (26, 96, 70, 14),
            text: "Insectoid",
        },
        Control {
            id: 0x112,
            class: Class::Button,
            at: (26, 112, 70, 14),
            text: "Nucleotid",
        },
        Control {
            id: 0x113,
            class: Class::Button,
            at: (112, 64, 70, 14),
            text: "Silicanoid",
        },
        Control {
            id: 0x114,
            class: Class::Button,
            at: (112, 80, 70, 14),
            text: "Antetheral",
        },
        Control {
            id: 0x115,
            class: Class::Button,
            at: (112, 96, 70, 14),
            text: "Random",
        },
        Control {
            id: 0x116,
            class: Class::Button,
            at: (112, 112, 70, 14),
            text: "Custom",
        },
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (26, 144, 170, 12),
            text: "Spend up to 50 leftover advantage points on:",
        },
        Control {
            id: 0x81a,
            class: Class::ComboBox,
            at: (26, 156, 156, 74),
            text: "",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// `IDD_RACE_WIZARD_2` (resource 147) — the Habitability page.
///
/// 8 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_2: Template = Template {
    caption: "Habitability",
    size: (261, 209),
    controls: &[
        Control {
            id: 0x123,
            class: Class::Button,
            at: (86, 110, 70, 12),
            text: "Immune to &Gravity",
        },
        Control {
            id: 0x124,
            class: Class::Button,
            at: (86, 110, 70, 12),
            text: "Immune to &Temperature",
        },
        Control {
            id: 0x125,
            class: Class::Button,
            at: (86, 110, 70, 12),
            text: "Immune to &Radiation",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// `IDD_RACE_WIZARD_3` (resource 148) — the Economy page.
///
/// 6 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_3: Template = Template {
    caption: "Economy",
    size: (261, 209),
    controls: &[
        Control {
            id: 0x123,
            class: Class::Button,
            at: (135, 126, 115, 14),
            text: "Factories cost 1kT less of Germanium to build",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// `IDD_RACE_WIZARD_4` (resource 149) — the Primary Racial Trait page.
///
/// 15 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_4: Template = Template {
    caption: "Primary Racial Trait",
    size: (261, 209),
    controls: &[
        Control {
            id: 0x10f,
            class: Class::Button,
            at: (16, 36, 84, 10),
            text: "Hyper-Expansion",
        },
        Control {
            id: 0x110,
            class: Class::Button,
            at: (16, 48, 84, 10),
            text: "Super Stealth",
        },
        Control {
            id: 0x111,
            class: Class::Button,
            at: (16, 60, 84, 10),
            text: "War Monger",
        },
        Control {
            id: 0x112,
            class: Class::Button,
            at: (16, 72, 84, 10),
            text: "Claim Adjuster",
        },
        Control {
            id: 0x113,
            class: Class::Button,
            at: (16, 84, 84, 10),
            text: "Inner-Strength",
        },
        Control {
            id: 0x114,
            class: Class::Button,
            at: (136, 36, 84, 10),
            text: "Space Demolition",
        },
        Control {
            id: 0x115,
            class: Class::Button,
            at: (136, 48, 84, 10),
            text: "Packet Physics",
        },
        Control {
            id: 0x116,
            class: Class::Button,
            at: (136, 60, 84, 10),
            text: "Interstellar Traveler",
        },
        Control {
            id: 0x117,
            class: Class::Button,
            at: (136, 72, 84, 10),
            text: "Alternate Reality",
        },
        Control {
            id: 0x118,
            class: Class::Button,
            at: (136, 84, 84, 10),
            text: "Jack of All Trades",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// `IDD_RACE_WIZARD_5` (resource 150) — the Lesser Racial Traits page.
///
/// 20 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_5: Template = Template {
    caption: "Lesser Racial Traits",
    size: (261, 209),
    controls: &[
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (80, 14, 80, 12),
            text: "Lesser Racial Traits",
        },
        Control {
            id: 0x123,
            class: Class::Button,
            at: (10, 26, 115, 14),
            text: "",
        },
        Control {
            id: 0x124,
            class: Class::Button,
            at: (10, 40, 115, 14),
            text: "",
        },
        Control {
            id: 0x125,
            class: Class::Button,
            at: (10, 54, 115, 14),
            text: "",
        },
        Control {
            id: 0x126,
            class: Class::Button,
            at: (10, 68, 115, 14),
            text: "",
        },
        Control {
            id: 0x127,
            class: Class::Button,
            at: (10, 82, 115, 14),
            text: "",
        },
        Control {
            id: 0x128,
            class: Class::Button,
            at: (10, 96, 115, 14),
            text: "",
        },
        Control {
            id: 0x129,
            class: Class::Button,
            at: (10, 110, 115, 14),
            text: "",
        },
        Control {
            id: 0x12a,
            class: Class::Button,
            at: (135, 26, 115, 14),
            text: "",
        },
        Control {
            id: 0x12b,
            class: Class::Button,
            at: (135, 40, 115, 14),
            text: "",
        },
        Control {
            id: 0x12c,
            class: Class::Button,
            at: (135, 54, 115, 14),
            text: "",
        },
        Control {
            id: 0x12d,
            class: Class::Button,
            at: (135, 68, 115, 14),
            text: "",
        },
        Control {
            id: 0x12e,
            class: Class::Button,
            at: (135, 82, 115, 14),
            text: "",
        },
        Control {
            id: 0x12f,
            class: Class::Button,
            at: (135, 96, 115, 14),
            text: "",
        },
        Control {
            id: 0x130,
            class: Class::Button,
            at: (135, 110, 115, 14),
            text: "",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// `IDD_RACE_WIZARD_6` (resource 151) — the Research Costs page.
///
/// 24 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_6: Template = Template {
    caption: "Research Costs",
    size: (261, 209),
    controls: &[
        Control {
            id: 0x10f,
            class: Class::Button,
            at: (16, 36, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x110,
            class: Class::Button,
            at: (16, 48, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x111,
            class: Class::Button,
            at: (16, 60, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x112,
            class: Class::Button,
            at: (16, 84, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x113,
            class: Class::Button,
            at: (16, 96, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x114,
            class: Class::Button,
            at: (16, 108, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x115,
            class: Class::Button,
            at: (16, 132, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x116,
            class: Class::Button,
            at: (16, 144, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x117,
            class: Class::Button,
            at: (16, 156, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x118,
            class: Class::Button,
            at: (140, 36, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x119,
            class: Class::Button,
            at: (140, 48, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x11a,
            class: Class::Button,
            at: (140, 60, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x11b,
            class: Class::Button,
            at: (140, 84, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x11c,
            class: Class::Button,
            at: (140, 96, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x11d,
            class: Class::Button,
            at: (140, 108, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x11e,
            class: Class::Button,
            at: (140, 132, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x11f,
            class: Class::Button,
            at: (140, 144, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x120,
            class: Class::Button,
            at: (140, 156, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x123,
            class: Class::Button,
            at: (12, 174, 210, 14),
            text: "",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// The wizard's six pages in order.
pub const RACE_WIZARD: [&Template; 6] = [
    &RACE_WIZARD_1,
    &RACE_WIZARD_2,
    &RACE_WIZARD_3,
    &RACE_WIZARD_4,
    &RACE_WIZARD_5,
    &RACE_WIZARD_6,
];

/// The wizard's caption, string `0x010e`: `"Custom Race Wizard - Step %d of 6"`.
pub const WIZARD_CAPTION: &str = "Custom Race Wizard - Step {} of 6";
