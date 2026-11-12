//! Every command in one list: its menu, its name, its keys, and when it
//! can be used. The menus are drawn from this list, the keyboard sheet
//! (Help → Keyboard shortcuts) is printed from it, the keys are read from
//! it, and Ctrl+K searches it. So a command added to a menu is in the
//! search, on the sheet and on its key without anything else to remember.

use std::sync::LazyLock;

use eframe::egui::ThemePreference;
use eframe::egui::{self, Button, Key, KeyboardShortcut, Modifiers, vec2};
use quill_engine::file::Background;

use crate::edit::EditAction;
use crate::files::FileAction;
use crate::filter_tool::FilterKind;
use crate::transform_tool::LayerAction;

/// What a command does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    File(FileAction),
    Edit(EditAction),
    Layer(LayerAction),
    Filter(FilterKind),
    App(AppAct),
    /// Choose a tool on the canvas.
    Tool(ToolAct),
    /// Move the view of the canvas, or change the brush size.
    View(ViewAct),
}

/// The commands the app itself carries out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppAct {
    Undo,
    Redo,
    Home,
    NewWindow,
    Search,
    AnimationTools,
    PixelGrid,
    HideReferences,
    AddReference,
    Theme(ThemeChoice),
    HomeAtStart,
    UpdatesAtStart,
    PlayPause,
    PreviousFrame,
    NextFrame,
    NewFrame,
    DuplicateFrame,
    DeleteFrame,
    AnimateLayer,
    OnionSkin,
    Shortcuts,
    About,
    CheckForUpdates,
    /// The update banner's button: Update (installed) or Download.
    StartUpdate,
    CanvasScreen,
    PenTestScreen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeChoice {
    Dark,
    Light,
    System,
}

impl ThemeChoice {
    pub fn preference(self) -> ThemePreference {
        match self {
            ThemeChoice::Dark => ThemePreference::Dark,
            ThemeChoice::Light => ThemePreference::Light,
            ThemeChoice::System => ThemePreference::System,
        }
    }
}

/// The tools (the M and L keys go round two shapes each).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolAct {
    Brush,
    Eraser,
    Pick,
    Fill,
    Gradient,
    Text,
    /// Rectangle, then ellipse.
    SelectBox,
    /// Lasso, then polygon.
    Lasso,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewAct {
    ZoomIn,
    ZoomOut,
    TurnLeft,
    TurnRight,
    Upright,
    Fit,
    BrushSmaller,
    BrushBigger,
}

/// Where a command lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    File,
    Edit,
    Select,
    Layer,
    Filters,
    View,
    Animation,
    Help,
    /// The tool buttons beside the canvas.
    Tools,
    /// Keys that move the canvas.
    Canvas,
    /// File → Settings.
    Settings,
}

impl Menu {
    pub fn name(self) -> &'static str {
        match self {
            Menu::File => "File",
            Menu::Edit => "Edit",
            Menu::Select => "Select",
            Menu::Layer => "Layer",
            Menu::Filters => "Filters",
            Menu::View => "View",
            Menu::Animation => "Animation",
            Menu::Help => "Help",
            Menu::Tools => "Tools",
            Menu::Canvas => "Canvas",
            Menu::Settings => "Settings",
        }
    }
}

/// The keys for a command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Keys {
    None,
    /// With Ctrl (or none, like Delete); the first is shown in menus.
    Shortcut(&'static [KeyboardShortcut]),
    /// A key on its own, read on the canvas (not while typing).
    Plain(&'static [Key]),
}

/// One command.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub menu: Menu,
    pub name: &'static str,
    pub keys: Keys,
    /// Said when the pointer rests on it, and searched too.
    pub tip: &'static str,
    /// Other words it's found by.
    pub words: &'static str,
    pub act: Act,
    /// A line after it in the menu.
    pub line_after: bool,
    /// Shown in a submenu of this name.
    pub submenu: Option<&'static str>,
}

const fn ctrl(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}
const fn ctrl_shift(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), key)
}

const fn entry(menu: Menu, name: &'static str, act: Act) -> Entry {
    Entry {
        menu,
        name,
        keys: Keys::None,
        tip: "",
        words: "",
        act,
        line_after: false,
        submenu: None,
    }
}

impl Entry {
    const fn keys(mut self, keys: &'static [KeyboardShortcut]) -> Self {
        self.keys = Keys::Shortcut(keys);
        self
    }
    const fn key(mut self, keys: &'static [Key]) -> Self {
        self.keys = Keys::Plain(keys);
        self
    }
    const fn tip(mut self, tip: &'static str) -> Self {
        self.tip = tip;
        self
    }
    const fn words(mut self, words: &'static str) -> Self {
        self.words = words;
        self
    }
    const fn line(mut self) -> Self {
        self.line_after = true;
        self
    }
    const fn under(mut self, submenu: &'static str) -> Self {
        self.submenu = Some(submenu);
        self
    }

    /// The keys as written on the menu and the sheet ("Ctrl+S", ",").
    pub fn keys_text(&self, ctx: &egui::Context) -> String {
        match self.keys {
            Keys::None => String::new(),
            Keys::Shortcut(list) => list
                .iter()
                .map(|s| ctx.format_shortcut(s))
                .collect::<Vec<_>>()
                .join(" or "),
            Keys::Plain(list) => list
                .iter()
                .map(|k| plain_key_name(*k))
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

/// How a plain key is written ("+", ",", "B").
pub fn plain_key_name(key: Key) -> &'static str {
    match key {
        Key::Plus => "+",
        Key::Equals => "=",
        Key::Minus => "−",
        Key::Comma => ",",
        Key::Period => ".",
        Key::Slash => "/",
        Key::Questionmark => "?",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Num0 => "0",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        other => other.name(),
    }
}

use Act::{App, Edit, File, Layer, Tool, View};
use Menu as M;

/// The list. (The filters come from the Filters' own list, below.)
const ENTRIES: &[Entry] = &[
    // File
    entry(M::File, "Home", App(AppAct::Home))
        .tip("New, recent pictures and unsaved work, on one page")
        .words("start welcome recent")
        .line(),
    entry(M::File, "New…", File(FileAction::New))
        .keys(&[ctrl(Key::N)])
        .words("picture canvas blank"),
    entry(M::File, "New animation…", File(FileAction::NewAnimation)).words("frames"),
    entry(M::File, "New pixel sprite or tile…", File(FileAction::NewSprite))
        .words("pixel art game"),
    entry(M::File, "New window", App(AppAct::NewWindow))
        .tip("Another Duck Quill window, for two pictures side by side")
        .line(),
    entry(M::File, "Open…", File(FileAction::Open)).keys(&[ctrl(Key::O)]),
    entry(M::File, "Save", File(FileAction::Save)).keys(&[ctrl(Key::S)]),
    entry(M::File, "Save as…", File(FileAction::SaveAs))
        .keys(&[ctrl_shift(Key::S)])
        .line(),
    entry(
        M::File,
        "Export PNG…",
        File(FileAction::Export(Background::Paper)),
    )
    .keys(&[ctrl(Key::E)])
    .words("image picture"),
    entry(
        M::File,
        "Export PNG, transparent…",
        File(FileAction::Export(Background::Transparent)),
    )
    .keys(&[ctrl_shift(Key::E)])
    .words("no paper background"),
    entry(M::File, "Play strokes…", File(FileAction::PlayStrokes)).words("script json"),
    entry(M::File, "Export strokes…", File(FileAction::ExportStrokes)).words("script json"),
    entry(
        M::File,
        "Export animation…",
        File(FileAction::ExportAnimation),
    )
    .words("gif sprite sheet frames")
    .line(),
    entry(M::File, "Settings…", File(FileAction::Settings)).words("options preferences"),
    // Edit
    entry(M::Edit, "Undo", App(AppAct::Undo)).keys(&[ctrl(Key::Z)]),
    entry(M::Edit, "Redo", App(AppAct::Redo))
        .keys(&[ctrl(Key::Y), ctrl_shift(Key::Z)])
        .line(),
    entry(M::Edit, "Cut", Edit(EditAction::Cut)).keys(&[ctrl(Key::X)]),
    entry(M::Edit, "Copy", Edit(EditAction::Copy)).keys(&[ctrl(Key::C)]),
    entry(M::Edit, "Paste", Edit(EditAction::Paste))
        .keys(&[ctrl(Key::V)])
        .tip("Onto a new layer")
        .line(),
    entry(
        M::Edit,
        "Delete inside the selection",
        Edit(EditAction::Delete),
    )
    .keys(&[KeyboardShortcut::new(Modifiers::NONE, Key::Delete)])
    .words("erase clear"),
    entry(M::Edit, "Transform selection…", Edit(EditAction::Transform))
        .keys(&[ctrl(Key::T)])
        .words("move resize rotate turn scale")
        .line(),
    entry(M::Edit, "Fill with the brush colour", Edit(EditAction::Fill)).words("colour color"),
    // Select
    entry(M::Select, "All", Edit(EditAction::SelectAll))
        .keys(&[ctrl(Key::A)])
        .words("select everything"),
    entry(M::Select, "Deselect", Edit(EditAction::Deselect)).keys(&[ctrl(Key::D)]),
    entry(M::Select, "Invert", Edit(EditAction::Invert)).keys(&[ctrl_shift(Key::I)]),
    entry(M::Select, "Layer contents", Edit(EditAction::SelectLayer)),
    entry(M::Select, "Feather…", Edit(EditAction::Feather)).words("soft edge"),
    // Layer
    entry(M::Layer, "Flip horizontal", Layer(LayerAction::FlipHorizontal))
        .tip("Mirror the selected layer: left and right swap")
        .words("mirror"),
    entry(M::Layer, "Flip vertical", Layer(LayerAction::FlipVertical))
        .tip("Mirror the selected layer: top and bottom swap")
        .words("mirror"),
    entry(M::Layer, "Transform…", Layer(LayerAction::Transform))
        .tip("Move, resize and turn the selected layer with handles")
        .words("move resize rotate scale"),
    // View
    entry(M::View, "Dark", App(AppAct::Theme(ThemeChoice::Dark))).under("Theme"),
    entry(M::View, "Light", App(AppAct::Theme(ThemeChoice::Light))).under("Theme"),
    entry(
        M::View,
        "Same as Windows",
        App(AppAct::Theme(ThemeChoice::System)),
    )
    .under("Theme")
    .line(),
    entry(M::View, "Add reference picture…", App(AppAct::AddReference))
        .tip("Pin a photo or drawing beside the painting, to look at and pick colours from"),
    entry(M::View, "Hide reference pictures", App(AppAct::HideReferences)),
    entry(M::View, "Pixel grid", App(AppAct::PixelGrid))
        .tip("Lines between the pixels when you zoom in close")
        .line(),
    entry(M::View, "Animation tools", App(AppAct::AnimationTools))
        .tip("The timeline and the Animation menu, for drawing frame by frame")
        .words("timeline frames onion"),
    entry(M::View, "Canvas", App(AppAct::CanvasScreen)).words("screen"),
    entry(M::View, "Pen test", App(AppAct::PenTestScreen)).words("screen pressure tablet"),
    // Animation (while the animation tools are on)
    entry(M::Animation, "Play or pause", App(AppAct::PlayPause)).key(&[Key::Slash]),
    entry(M::Animation, "Previous frame", App(AppAct::PreviousFrame)).key(&[Key::Comma]),
    entry(M::Animation, "Next frame", App(AppAct::NextFrame))
        .key(&[Key::Period])
        .line(),
    entry(M::Animation, "New frame", App(AppAct::NewFrame)).words("add"),
    entry(M::Animation, "Duplicate frame", App(AppAct::DuplicateFrame)).words("copy"),
    entry(M::Animation, "Delete frame", App(AppAct::DeleteFrame))
        .words("remove")
        .line(),
    entry(
        M::Animation,
        "Animate this layer, or make it still",
        App(AppAct::AnimateLayer),
    )
    .tip("Animated: a drawing per frame. Still: one drawing on every frame")
    .words("film strip"),
    entry(M::Animation, "Onion skin", App(AppAct::OnionSkin))
        .tip("See the frames before (red) and after (blue) faintly")
        .words("ghost"),
    // Help
    entry(M::Help, "Find a command…", App(AppAct::Search))
        .keys(&[ctrl(Key::K)])
        .words("search command palette"),
    entry(M::Help, "Keyboard shortcuts", App(AppAct::Shortcuts))
        .key(&[Key::Questionmark])
        .words("keys"),
    entry(M::Help, "Check for updates", App(AppAct::CheckForUpdates))
        .words("new version upgrade"),
    entry(M::Help, "About Duck Quill", App(AppAct::About)).words("version"),
    // Tools
    entry(M::Tools, "Brush", Tool(ToolAct::Brush))
        .key(&[Key::B])
        .words("paint pen pencil"),
    entry(M::Tools, "Eraser", Tool(ToolAct::Eraser))
        .key(&[Key::E])
        .tip("Or turn the pen round"),
    entry(M::Tools, "Pick a colour", Tool(ToolAct::Pick))
        .key(&[Key::I])
        .tip("Or hold Alt, or the pen's side button")
        .words("eyedropper colour color"),
    entry(M::Tools, "Fill", Tool(ToolAct::Fill))
        .key(&[Key::F])
        .words("paint bucket"),
    entry(M::Tools, "Gradient", Tool(ToolAct::Gradient)).key(&[Key::G]),
    entry(M::Tools, "Text", Tool(ToolAct::Text))
        .key(&[Key::T])
        .words("words type letters"),
    entry(M::Tools, "Select a rectangle; again for an ellipse", Tool(ToolAct::SelectBox))
        .key(&[Key::M])
        .words("marquee circle oval"),
    entry(M::Tools, "Lasso; again for a polygon", Tool(ToolAct::Lasso))
        .key(&[Key::L])
        .words("select freehand"),
    entry(M::Tools, "Brush or eraser smaller", View(ViewAct::BrushSmaller))
        .key(&[Key::OpenBracket])
        .words("size"),
    entry(M::Tools, "Brush or eraser bigger", View(ViewAct::BrushBigger))
        .key(&[Key::CloseBracket])
        .words("size"),
    // Canvas
    entry(M::Canvas, "Zoom in", View(ViewAct::ZoomIn))
        .key(&[Key::Plus, Key::Equals])
        .words("closer bigger"),
    entry(M::Canvas, "Zoom out", View(ViewAct::ZoomOut))
        .key(&[Key::Minus])
        .words("further smaller"),
    entry(M::Canvas, "Turn 15° left", View(ViewAct::TurnLeft))
        .key(&[Key::Num4])
        .words("rotate canvas"),
    entry(M::Canvas, "Turn 15° right", View(ViewAct::TurnRight))
        .key(&[Key::Num6])
        .words("rotate canvas"),
    entry(M::Canvas, "Turn back upright", View(ViewAct::Upright))
        .key(&[Key::Num5])
        .words("rotate reset straight"),
    entry(M::Canvas, "Fit the picture in the window", View(ViewAct::Fit))
        .key(&[Key::Num0])
        .words("zoom whole"),
    // Settings
    entry(M::Settings, "Animation tools", App(AppAct::AnimationTools))
        .tip("The timeline and the Animation menu, for drawing frame by frame. Off, Duck Quill is a plain drawing app; frames are never lost")
        .words("timeline frames onion"),
    entry(M::Settings, "Pixel grid", App(AppAct::PixelGrid))
        .tip("Lines between the pixels when you zoom in close"),
    entry(M::Settings, "Show Home at start", App(AppAct::HomeAtStart))
        .tip("Unless Duck Quill was started by opening a picture")
        .words("welcome"),
    entry(M::Settings, "Check for updates at start", App(AppAct::UpdatesAtStart))
        .tip("Asks GitHub for the newest version's number; sends nothing about you or your pictures")
        .words("new version"),
    entry(M::Settings, "Dark", App(AppAct::Theme(ThemeChoice::Dark))).under("Theme"),
    entry(M::Settings, "Light", App(AppAct::Theme(ThemeChoice::Light))).under("Theme"),
    entry(
        M::Settings,
        "Same as Windows",
        App(AppAct::Theme(ThemeChoice::System)),
    )
    .under("Theme"),
];

/// Every command, the filters included (in menu order).
pub fn all() -> &'static [Entry] {
    static ALL: LazyLock<Vec<Entry>> = LazyLock::new(|| {
        let filters = FilterKind::ALL.into_iter().map(|kind| {
            entry(M::Filters, kind.label(), Act::Filter(kind))
                .tip(kind.tip())
                .words("effect")
        });
        let mut all: Vec<Entry> = Vec::new();
        for entry in ENTRIES {
            if entry.menu == M::View && all.iter().all(|e: &Entry| e.menu != M::Filters) {
                all.extend(filters.clone());
            }
            all.push(*entry);
        }
        all
    });
    &ALL
}

/// The commands of one menu, in order.
pub fn of(menu: Menu) -> impl Iterator<Item = &'static Entry> {
    all().iter().filter(move |e| e.menu == menu)
}

// ---- When a command can be used ---------------------------------------------------

/// What the app is doing, for "can this be used now?" and tick marks.
#[derive(Clone, Copy, Debug, Default)]
pub struct State {
    /// A script is playing or a transform must be finished first.
    pub busy: bool,
    /// The canvas is showing (not Home or the pen test).
    pub on_canvas: bool,
    /// The frames shown came from a script ("Edit frames by hand" first).
    pub watching: bool,
    pub selected: bool,
    pub frames: usize,
    pub can_undo: bool,
    pub can_redo: bool,
    pub animation_tools: bool,
    pub pixel_grid: bool,
    pub references_hidden: bool,
    pub onion: bool,
    pub playing: bool,
    pub theme: Option<ThemePreference>,
    pub home_at_start: bool,
    pub updates_at_start: bool,
    pub pen_test: bool,
    /// The selected layer has a drawing per frame.
    pub layer_animated: bool,
}

impl State {
    /// Whether `act` can be used now; if not, why (in words).
    pub fn why_not(&self, act: Act) -> Option<&'static str> {
        let drawing = !self.busy && self.on_canvas;
        let editing = drawing && !self.watching;
        let animation = |extra: bool| {
            if !self.animation_tools {
                Some("Turn on View → Animation tools first")
            } else if !drawing || !extra {
                Some("Not now")
            } else {
                None
            }
        };
        match act {
            Act::File(_) if self.busy => Some("Finish what's on the canvas first"),
            Act::File(_) => None,
            Act::Edit(action) => {
                use EditAction::*;
                let needs_selection =
                    matches!(action, Cut | Delete | Transform | Fill | Deselect | Feather);
                if !editing {
                    Some("Not now")
                } else if needs_selection && !self.selected {
                    Some("Select something first")
                } else {
                    None
                }
            }
            Act::Layer(_) | Act::Filter(_) => (!drawing).then_some("Not now"),
            Act::Tool(_) | Act::View(_) => (!self.on_canvas).then_some("On the canvas"),
            Act::App(app) => match app {
                AppAct::Undo if !self.can_undo || self.busy => Some("Nothing to undo"),
                AppAct::Redo if !self.can_redo || self.busy => Some("Nothing to redo"),
                AppAct::Home if self.busy => Some("Finish what's on the canvas first"),
                AppAct::AddReference => (!drawing).then_some("Not now"),
                AppAct::PlayPause | AppAct::PreviousFrame | AppAct::NextFrame => animation(true),
                AppAct::NewFrame | AppAct::DuplicateFrame => animation(!self.watching),
                AppAct::DeleteFrame => animation(!self.watching && self.frames > 1),
                AppAct::AnimateLayer => animation(!self.watching),
                AppAct::OnionSkin => animation(true),
                _ => None,
            },
        }
    }

    /// For commands that are switched on and off: whether it's on.
    pub fn is_on(&self, act: Act) -> Option<bool> {
        match act {
            Act::App(AppAct::AnimationTools) => Some(self.animation_tools),
            Act::App(AppAct::PixelGrid) => Some(self.pixel_grid),
            Act::App(AppAct::HideReferences) => Some(self.references_hidden),
            Act::App(AppAct::OnionSkin) => Some(self.onion),
            Act::App(AppAct::HomeAtStart) => Some(self.home_at_start),
            Act::App(AppAct::UpdatesAtStart) => Some(self.updates_at_start),
            Act::App(AppAct::Theme(choice)) => Some(self.theme == Some(choice.preference())),
            Act::App(AppAct::CanvasScreen) => Some(!self.pen_test),
            Act::App(AppAct::PenTestScreen) => Some(self.pen_test),
            _ => None,
        }
    }
}

// ---- Drawing menus ------------------------------------------------------------------

/// One command as a menu line: a tick box for an on/off command, else a
/// button with its keys. Returns true if it was clicked.
pub fn item(ui: &mut egui::Ui, entry: &Entry, state: &State) -> bool {
    let why_not = state.why_not(entry.act);
    let text = match entry.act {
        Act::App(AppAct::PlayPause) if state.playing => "Pause",
        Act::App(AppAct::PlayPause) => "Play",
        Act::App(AppAct::AnimateLayer) if state.layer_animated => "Make this layer still",
        Act::App(AppAct::AnimateLayer) => "Animate this layer",
        _ => entry.name,
    };
    let response = match state.is_on(entry.act) {
        // One of a few choices (the theme): a round button.
        Some(on) if entry.submenu.is_some() => {
            ui.add_enabled(why_not.is_none(), egui::RadioButton::new(on, text))
        }
        Some(on) => {
            let mut on = on;
            ui.add_enabled(why_not.is_none(), egui::Checkbox::new(&mut on, text))
        }
        None => {
            let button = Button::new(text)
                .shortcut_text(entry.keys_text(ui.ctx()))
                .min_size(vec2(0.0, 28.0)); // easy to tap
            ui.add_enabled(why_not.is_none(), button)
        }
    };
    let response = if entry.tip.is_empty() {
        response
    } else {
        response.on_hover_text(entry.tip)
    };
    let response = match why_not {
        Some(why) => response.on_disabled_hover_text(why),
        None => response,
    };
    response.clicked()
}

/// The lines of `menu`, drawn from the list. `after` is called after each
/// entry, for something that isn't a command (File → Open recent).
pub fn menu_items(
    ui: &mut egui::Ui,
    menu: Menu,
    state: &State,
    mut after: impl FnMut(&mut egui::Ui, &Entry),
) -> Option<Act> {
    let mut chosen = None;
    let entries: Vec<&Entry> = of(menu).collect();
    let mut index = 0;
    while index < entries.len() {
        let entry = entries[index];
        if let Some(submenu) = entry.submenu {
            let group: Vec<&Entry> = entries[index..]
                .iter()
                .take_while(|e| e.submenu == Some(submenu))
                .copied()
                .collect();
            let mut choices = |ui: &mut egui::Ui| {
                for entry in &group {
                    if item(ui, entry, state) {
                        chosen = Some(entry.act);
                    }
                }
            };
            if menu == Menu::Settings {
                // In the Settings window, side by side after the name.
                ui.horizontal(|ui| {
                    ui.label(submenu);
                    choices(ui);
                });
            } else {
                ui.menu_button(submenu, choices);
            }
            index += group.len();
            if group.last().is_some_and(|e| e.line_after) {
                ui.separator();
            }
            continue;
        }
        if item(ui, entry, state) {
            chosen = Some(entry.act);
        }
        after(ui, entry);
        if entry.line_after {
            ui.separator();
        }
        index += 1;
    }
    chosen
}

/// A menu in the top bar, drawn from the list.
pub fn menu(ui: &mut egui::Ui, menu: Menu, state: &State) -> Option<Act> {
    let mut chosen = None;
    let button = Button::new(menu.name()).min_size(vec2(56.0, 26.0));
    egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
        ui.set_min_width(220.0);
        chosen = menu_items(ui, menu, state, |_, _| {});
    });
    chosen
}

// ---- Reading the keys ------------------------------------------------------------

/// The Ctrl shortcuts pressed (each used up so nothing else sees it),
/// those with Shift first: egui ignores an extra Shift when matching, so
/// Ctrl+S would also match Ctrl+Shift+S. Not while typing in a text box.
/// Only the commands `wanted` picks, and only those usable now.
pub fn shortcuts_pressed(
    ctx: &egui::Context,
    state: &State,
    wanted: impl Fn(&Entry) -> bool,
) -> Option<Act> {
    if ctx.egui_wants_keyboard_input() {
        return None;
    }
    let mut keyed: Vec<(KeyboardShortcut, &Entry)> = all()
        .iter()
        .filter(|e| wanted(e))
        .flat_map(|e| match e.keys {
            Keys::Shortcut(list) => list.iter().map(|s| (*s, e)).collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect();
    keyed.sort_by_key(|(s, _)| std::cmp::Reverse(s.modifiers.shift));
    ctx.input_mut(|i| {
        keyed
            .into_iter()
            .find(|(shortcut, _)| i.consume_shortcut(shortcut))
            .map(|(_, entry)| entry.act)
            // Used up either way: Ctrl+T with nothing selected mustn't
            // reach the canvas as T (the Text tool).
            .filter(|act| state.why_not(*act).is_none())
    })
}

/// The plain keys (no Ctrl) of `menus` pressed since the last frame, each
/// as many times as it was pressed. Not while typing.
pub fn keys_pressed(ui: &egui::Ui, menus: &[Menu]) -> Vec<(Act, u32)> {
    if ui.ctx().egui_wants_keyboard_input() {
        return Vec::new();
    }
    let mut pressed: Vec<(Act, u32)> = Vec::new();
    for entry in all().iter().filter(|e| menus.contains(&e.menu)) {
        if let Keys::Plain(list) = entry.keys {
            let count: u32 = list
                .iter()
                .map(|k| {
                    ui.input(|i| {
                        // Only the key on its own (Ctrl+E is Export, not Eraser).
                        if i.modifiers.command {
                            0
                        } else {
                            i.num_presses(*k) as u32
                        }
                    })
                })
                .sum();
            if count > 0 {
                pressed.push((entry.act, count));
            }
        }
    }
    pressed
}

// ---- Searching ---------------------------------------------------------------------

/// Lower case letters and digits only, split into words.
fn words_of(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The commands whose menu, name, tip or other words hold every word of
/// `query` (at the start of a word), best first: those whose name starts
/// with the query, then those whose name holds every word, then the rest.
pub fn search(query: &str) -> Vec<&'static Entry> {
    let query_words = words_of(query);
    if query_words.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(u8, usize, &'static Entry)> = all()
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let name = words_of(entry.name);
            let everything = words_of(&format!(
                "{} {} {} {}",
                entry.menu.name(),
                entry.name,
                entry.tip,
                entry.words
            ));
            let holds = |words: &[String]| {
                query_words
                    .iter()
                    .all(|q| words.iter().any(|w| w.starts_with(q.as_str())))
            };
            if !holds(&everything) {
                return None;
            }
            let rank = if name.join(" ").starts_with(&query_words.join(" ")) {
                0
            } else if holds(&name) {
                1
            } else {
                2
            };
            Some((rank, index, entry))
        })
        .collect();
    found.sort_by_key(|(rank, index, _)| (*rank, *index));
    found.into_iter().map(|(_, _, entry)| entry).collect()
}

/// The Ctrl+K window.
#[derive(Default)]
pub struct Search {
    pub open: bool,
    query: String,
    chosen: usize,
    focus: bool,
}

impl Search {
    pub fn show(&mut self) {
        self.open = true;
        self.query.clear();
        self.chosen = 0;
        self.focus = true;
    }

    /// The words typed (the test driver's).
    #[cfg(feature = "demo")]
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_owned();
        self.chosen = 0;
    }

    /// Shows the window; returns the command picked (with Enter or a click).
    pub fn ui(&mut self, ctx: &egui::Context, state: &State) -> Option<Act> {
        if !self.open {
            return None;
        }
        let mut picked = None;
        let mut close = false;
        egui::Window::new("Find a command")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0])
            .fixed_size([420.0, 0.0])
            .show(ctx, |ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Find a command: onion, gif, flip…")
                        .desired_width(f32::INFINITY),
                );
                if self.focus {
                    field.request_focus();
                    self.focus = false;
                }
                if field.changed() {
                    self.chosen = 0;
                }
                let results: Vec<&Entry> = search(&self.query).into_iter().take(12).collect();
                let (up, down, enter, escape) = ui.input(|i| {
                    (
                        i.key_pressed(Key::ArrowUp),
                        i.key_pressed(Key::ArrowDown),
                        i.key_pressed(Key::Enter),
                        i.key_pressed(Key::Escape),
                    )
                });
                if down && self.chosen + 1 < results.len() {
                    self.chosen += 1;
                }
                if up {
                    self.chosen = self.chosen.saturating_sub(1);
                }
                if escape {
                    close = true;
                }
                if !self.query.trim().is_empty() && results.is_empty() {
                    ui.weak("No command has that name.");
                }
                for (index, entry) in results.iter().enumerate() {
                    let why_not = state.why_not(entry.act);
                    let mut text = format!("{}: {}", entry.menu.name(), entry.name);
                    if let Some(on) = state.is_on(entry.act) {
                        text.push_str(if on { "  (on)" } else { "  (off)" });
                    }
                    let row = ui.add_enabled(
                        why_not.is_none(),
                        Button::selectable(index == self.chosen, text)
                            .shortcut_text(entry.keys_text(ui.ctx()))
                            .min_size(vec2(ui.available_width(), 26.0)),
                    );
                    let row = match why_not {
                        Some(why) => row.on_disabled_hover_text(why),
                        None => row,
                    };
                    if row.clicked() || (enter && index == self.chosen && why_not.is_none()) {
                        picked = Some(entry.act);
                    }
                }
            });
        if picked.is_some() || close {
            self.open = false;
        }
        picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_turns_up_in_the_search_under_its_own_name() {
        for entry in all() {
            let found = search(entry.name);
            assert!(
                found
                    .iter()
                    .any(|e| e.menu == entry.menu && e.name == entry.name),
                "{}: {} isn't found by its name",
                entry.menu.name(),
                entry.name
            );
            // And with the menu's name in front.
            let found = search(&format!("{} {}", entry.menu.name(), entry.name));
            assert!(found.iter().any(|e| std::ptr::eq(*e, entry)));
        }
    }

    #[test]
    fn searching_finds_commands_by_other_words_best_first() {
        let names = |q: &str| -> Vec<String> {
            search(q)
                .iter()
                .map(|e| format!("{}: {}", e.menu.name(), e.name))
                .collect()
        };
        assert_eq!(names("onion")[0], "Animation: Onion skin");
        assert!(names("onion").contains(&"Settings: Animation tools".to_owned()));
        assert!(names("onion").contains(&"View: Animation tools".to_owned()));
        assert_eq!(names("gif")[0], "File: Export animation…");
        assert_eq!(names("exp png")[0], "File: Export PNG…");
        assert_eq!(names("blur")[0], "Filters: Blur…");
        assert_eq!(names("eyedropper")[0], "Tools: Pick a colour");
        assert_eq!(names("update")[0], "Help: Check for updates");
        assert!(names("").is_empty());
        assert!(names("zzzz").is_empty());
    }

    #[test]
    fn no_key_does_two_things() {
        let mut seen: Vec<(String, Act)> = Vec::new();
        for entry in all() {
            let keys: Vec<String> = match entry.keys {
                Keys::None => vec![],
                Keys::Shortcut(list) => list.iter().map(|s| format!("{s:?}")).collect(),
                Keys::Plain(list) => list.iter().map(|k| format!("{k:?}")).collect(),
            };
            for key in keys {
                if let Some((_, act)) = seen.iter().find(|(k, _)| *k == key) {
                    assert_eq!(*act, entry.act, "{key} is used twice");
                } else {
                    seen.push((key, entry.act));
                }
            }
        }
        // The keys today's users know are all still there.
        for key in [
            "N", "O", "S", "E", "Z", "Y", "X", "C", "V", "T", "A", "D", "I", "K",
        ] {
            assert!(
                seen.iter().any(|(k, _)| k.contains(&format!("key: {key}"))),
                "Ctrl+{key}"
            );
        }
    }

    #[test]
    fn a_command_can_be_used_only_when_it_makes_sense() {
        let state = State {
            on_canvas: true,
            frames: 1,
            ..State::default()
        };
        assert!(state.why_not(Act::Edit(EditAction::Copy)).is_none());
        assert_eq!(
            state.why_not(Act::Edit(EditAction::Cut)),
            Some("Select something first")
        );
        assert_eq!(
            state.why_not(Act::App(AppAct::OnionSkin)),
            Some("Turn on View → Animation tools first")
        );
        let animating = State {
            animation_tools: true,
            ..state
        };
        assert!(animating.why_not(Act::App(AppAct::NewFrame)).is_none());
        assert!(
            animating.why_not(Act::App(AppAct::DeleteFrame)).is_some(),
            "only one frame"
        );
        let busy = State {
            busy: true,
            ..state
        };
        assert!(busy.why_not(Act::File(FileAction::Save)).is_some());
        assert!(busy.why_not(Act::App(AppAct::Shortcuts)).is_none());
    }
}
