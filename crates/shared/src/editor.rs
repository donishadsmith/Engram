use crate::{script::API, structs::PendingDialog};
use arboard::Clipboard;
use egui::{Context, ScrollArea, Window, text::CCursorRange};
use egui_code_editor::{CodeEditor, ColorTheme, Completer, Syntax};
use egui_phosphor::regular as icons;
use macroquad::input::{
    KeyCode::{self, RightShift},
    is_key_down, is_key_pressed,
};
use regex::Regex;
use rfd::FileDialog;
use std::{
    collections::{HashMap, HashSet},
    fs::{read_to_string, write},
    hash::Hash,
    mem::take,
    path::PathBuf,
    time::{Duration, Instant},
};

const COPY_DELAY: Duration = Duration::from_millis(200);
const SHORTCUT: &str = r#"
Shift+Enter: run the current script
Ctrl+N: new script (max of 5)
Ctrl+S: save the current script
Ctrl+C: copy the selected text

F8: open or close the Lua editor

Notes:
    - Cmd button can replace Ctrl
    - Other shortcuts such as Ctrl+Z, Ctrl+V, Ctrl+A, also usable
"#;

const MAX_TABS: usize = 5;

#[derive(PartialEq)]
enum HelpTab {
    Api,
    Shortcut,
}

#[derive(Clone, Copy)]
enum EditorDialog {
    Open,
    Save,
}

pub fn super_key_down() -> bool {
    is_key_down(KeyCode::LeftControl)
        || is_key_down(KeyCode::LeftSuper)
        || is_key_down(KeyCode::RightControl)
        || is_key_down(KeyCode::RightSuper)
}

fn save_shortcut() -> bool {
    super_key_down() && is_key_pressed(KeyCode::S)
}

fn new_file_shortcut() -> bool {
    super_key_down() && is_key_pressed(KeyCode::N)
}

fn member_table(code: &str, cursor: usize) -> Option<&str> {
    let is_word = |character: char| character.is_alphanumeric() || character == '_';
    let end = code
        .char_indices()
        .nth(cursor)
        .map_or(code.len(), |(index, _)| index);

    let before = code[..end].trim_end_matches(is_word).strip_suffix('.')?;
    let table = &before[before.trim_end_matches(is_word).len()..];

    (!table.is_empty()).then_some(table)
}

#[derive(Clone, Debug)]
struct ScriptTab {
    id: u64,
    name: String,
    code: String,
    path: Option<PathBuf>,
}

impl Hash for ScriptTab {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

// https://github.com/microsoft/vscode/blob/main/extensions/theme-defaults/themes/hc_black.json
// goated theme
pub const DARK_HIGH_CONTRAST: ColorTheme = ColorTheme {
    name: "Dark High Contrast",
    dark: true,
    bg: "000000",
    cursor: "FFFFFF",
    selection: "264F78",
    comments: "7ca668",
    functions: "DCDCAA",
    keywords: "569cd6",
    literals: "9cdcfe",
    numerics: "b5cea8",
    punctuation: "d4d4d4",
    strs: "ce9178",
    types: "4EC9B0",
    special: "DCDCAA",
};

// the illusion of choice, but primarily just different dark theme variants
// just adding a single white theme
#[derive(Clone, Copy)]
pub enum EditorTheme {
    DarkHighContrast,
    GithubDark,
    GithubLight, // yikes
    AyuDark,
    AyuMirage,
    GruvboxDark,
}

impl EditorTheme {
    pub fn from_name(name: String) -> EditorTheme {
        match name.as_str() {
            "Ayu Dark" => EditorTheme::AyuDark,
            "Dark High Contrast" => EditorTheme::DarkHighContrast,
            "Gruvbox Dark" => EditorTheme::GruvboxDark,
            "Ayu Mirage" => EditorTheme::AyuMirage,
            "Github Light" => EditorTheme::GithubLight,
            _ => EditorTheme::GithubDark,
        }
    }

    pub fn to_name(self) -> String {
        match self {
            EditorTheme::AyuDark => "Ayu Dark".to_string(),
            EditorTheme::DarkHighContrast => "Dark High Contrast".to_string(),
            EditorTheme::AyuMirage => "Ayu Mirage".to_string(),
            EditorTheme::GruvboxDark => "Gruvbox Dark".to_string(),
            EditorTheme::GithubLight => "Github Light".to_string(),
            _ => "Github Dark".to_string(),
        }
    }

    fn to_theme(self) -> ColorTheme {
        match self {
            EditorTheme::AyuDark => ColorTheme::AYU_DARK,
            EditorTheme::DarkHighContrast => DARK_HIGH_CONTRAST,
            EditorTheme::AyuMirage => ColorTheme::AYU_MIRAGE,
            EditorTheme::GruvboxDark => ColorTheme::GRUVBOX_DARK,
            EditorTheme::GithubLight => ColorTheme::GITHUB_LIGHT,
            _ => ColorTheme::GITHUB_DARK,
        }
    }
}

pub struct LuaEditor {
    tabs: Vec<ScriptTab>,
    current_tab: usize,
    pub opened: bool,
    focused: bool,
    pub output: Vec<String>,
    pub termination_request: bool,
    pending_copy: Option<(String, Instant)>,
    clipboard: Option<Clipboard>,
    pub help_window: bool,
    help_tab: HelpTab,
    file_dialog: Option<(EditorDialog, PendingDialog)>,
    regex: Regex,
    next_tab_id: u64,
    syntax: Syntax,
    completer: Completer,
    member_completers: HashMap<&'static str, Completer>,
    last_cursor: usize,
    pub theme: EditorTheme,
}

impl LuaEditor {
    pub fn new(theme: EditorTheme) -> Self {
        let mut tabs = Vec::with_capacity(MAX_TABS);
        let script = ScriptTab {
            id: 1,
            code: String::new(),
            name: "Untitled-1".to_string(),
            path: None,
        };

        tabs.push(script);

        let mut syntax = Syntax::lua();
        let mut top_level_syntax = Syntax::lua();
        let mut member_completers: HashMap<&'static str, Completer> = HashMap::new();

        let api_call = Regex::new(r"(\w+)\.(\w+)\(").unwrap();
        for call in api_call.captures_iter(API) {
            let table = call.get(1).unwrap().as_str();
            let method = call.get(2).unwrap().as_str();

            syntax.special.insert(table);
            syntax.special.insert(method);
            top_level_syntax.special.insert(table);

            member_completers
                .entry(table)
                .or_insert_with(|| Completer::default().with_auto_indent())
                .push_word(method);
        }

        syntax.special.insert("print");
        top_level_syntax.special.insert("print");

        let completer = Completer::new_with_syntax(&top_level_syntax)
            .with_user_words()
            .with_auto_indent();

        Self {
            tabs,
            opened: false,
            focused: false,
            output: Vec::new(),
            termination_request: false,
            pending_copy: None,
            clipboard: Clipboard::new().ok(),
            help_window: false,
            help_tab: HelpTab::Api,
            file_dialog: None,
            current_tab: 0,
            regex: Regex::new(r"^Untitled-\d{1,2}$").unwrap(),
            next_tab_id: 2,
            syntax,
            completer,
            member_completers,
            last_cursor: 0,
            theme,
        }
    }

    pub fn show_ui(&mut self, egui_ctx: &Context) -> Option<String> {
        self.poll_file_dialog();
        self.flush_pending_copy();

        let mut opened = self.opened;
        let mut focused = false;
        let mut run = false;

        let mut select_tab: Option<usize> = None;
        let mut close_tab: Option<usize> = None;
        let mut add_tab = false;

        let (bg_color, text_color) = match self.theme {
            EditorTheme::DarkHighContrast => (egui::Color32::BLACK, egui::Color32::WHITE),
            EditorTheme::GithubLight => (egui::Color32::WHITE, egui::Color32::BLACK),
            _ => (egui::Color32::from_gray(27), egui::Color32::WHITE),
        };

        Window::new("Lua Editor")
            .min_height(300.0)
            .min_width(400.0)
            .default_height(700.0)
            .frame(egui::Frame::window(&egui_ctx.global_style()).fill(bg_color))
            .open(&mut opened)
            .show(egui_ctx, |ui| {
                if matches!(self.theme, EditorTheme::GithubLight) {
                    *ui.visuals_mut() = egui::Visuals::light();
                }

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.menu_button("File", |ui| {
                            if ui.button(format!("{}  Load Script", icons::FOLDER_OPEN)).clicked() && self.file_dialog.is_none() {
                                self.load_lua_script();
                            }

                            if ui.button(format!("{}  Save Script", icons::FLOPPY_DISK)).clicked() && self.file_dialog.is_none() {
                                self.save_lua_script();
                            }
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(format!("{}  Help", icons::INFO)).clicked() && !self.help_window {
                                self.help_window = true;
                            }

                            egui::Window::new("Help")
                            .default_width(650.0)
                            .vscroll(false)
                            .frame(egui::Frame::window(&ui.ctx().global_style()).fill(bg_color))
                            .open(&mut self.help_window)
                            .show(ui.ctx(), |ui| {
                                if matches!(self.theme, EditorTheme::GithubLight) {
                                    *ui.visuals_mut() = egui::Visuals::light();
                                }

                                ui.visuals_mut().override_text_color = Some(text_color);

                                ui.horizontal(|ui| {
                                    ui.selectable_value(&mut self.help_tab, HelpTab::Api, "API");
                                    ui.selectable_value(&mut self.help_tab, HelpTab::Shortcut, "Shortcuts");
                                });

                                ui.separator();

                                egui::ScrollArea::vertical().show(ui, |ui| match self.help_tab {
                                    HelpTab::Api => ui.label(API),
                                    HelpTab::Shortcut => ui.label(SHORTCUT)
                                });
                            });

                            ui.menu_button("Session", |ui| {
                                if ui.button(format!("{}  Clear Console", icons::BROOM)).on_hover_text(
                                    "Clears output in console"
                                ).clicked() {
                                    self.output.clear();
                                }

                                if ui.button(format!("{}  Stop Scripts", icons::STOP_CIRCLE)).on_hover_text(
                                    "Stops Lua scripts, callbacks, and clears all watchpoints and breakpoints"
                                    ).clicked() {
                                    self.termination_request = true;
                                }

                                ui.menu_button(format!("{}  Theme", icons::PAINT_BRUSH_BROAD), |ui| {
                                    if ui.button("Ayu Dark").clicked() {
                                        self.theme = EditorTheme::AyuDark
                                    }

                                    if ui.button("Ayu Mirage").clicked() {
                                        self.theme = EditorTheme::AyuMirage
                                    }

                                    if ui.button("Dark High Contrast").clicked() {
                                        self.theme = EditorTheme::DarkHighContrast
                                    }

                                    if ui.button("Github Dark").clicked() {
                                        self.theme = EditorTheme::GithubDark
                                    }

                                    if ui.button("Github Light").clicked() {
                                        self.theme = EditorTheme::GithubLight
                                    }

                                    if ui.button("Gruvbox Dark").clicked() {
                                        self.theme = EditorTheme::GruvboxDark
                                    }
                                });
                            });

                            if ui.button(format!("{}  Run", icons::PLAY)).clicked() {
                                run = true;
                            }

                            // getting tired of having to click run
                        if (is_key_down(KeyCode::LeftShift) | is_key_down(RightShift)) && is_key_pressed(KeyCode::Enter) {
                                run = true
                            }
                        });
                    });

                    let current_id = self.tabs[self.current_tab].id;

                    ui.horizontal(|ui| {
                        ui.add_space(40.0);
                        let can_add_tab = !self.max_tabs_reached();
                        let can_close_tab = self.tabs.len() > 1;
                        let tab_height = ui.spacing().interact_size.y;

                        let tab_count = self.tabs.len() as f32;
                        let spacing = ui.spacing().item_spacing.x;
                        let gap_count = if can_add_tab { tab_count } else { tab_count - 1.0 };

                        let plus_width = if can_add_tab { 28.0 } else { 0.0 };
                        let tab_width = (ui.available_width() - plus_width - gap_count * spacing) / tab_count;

                        egui_dnd::dnd(ui, "Tabs").with_drag_axis(egui_dnd::DragAxis::Horizontal).show_vec_sized(&mut self.tabs, egui::vec2(tab_width, tab_height), |ui, tab, handle, state| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if state.index + 1 < tab_count as usize || can_add_tab {
                                        ui.separator();
                                    }

                                    if can_close_tab
                                        && ui.add(egui::Button::new(icons::X).small().frame(false)).clicked()
                                    {
                                        close_tab = Some(state.index);
                                    }

                                    handle.ui(ui, |ui| {
                                        let text = if tab.id == current_id {
                                            egui::RichText::new(&tab.name).strong().underline()
                                        } else {
                                            egui::RichText::new(&tab.name)
                                        };

                                        let name_response = ui
                                            .add_sized(
                                                [ui.available_width(), tab_height],
                                                egui::Label::new(text)
                                                    .wrap_mode(egui::TextWrapMode::Truncate)
                                                    .selectable(false)
                                                    .sense(egui::Sense::click()),
                                            );

                                        if name_response.clicked() {
                                            select_tab = Some(state.index);
                                        }
                                    });
                                },
                            );
                        });

                        if can_add_tab && ui.button(icons::PLUS).on_hover_text("New script").clicked() {
                            add_tab = true;
                        }
                    });

                    self.current_tab = self.tabs.iter().position(|tab| tab.id == current_id).unwrap();
                });

                if let Some(index) = close_tab {
                    self.drop_tab(index);
                } else if let Some(index) = select_tab {
                    self.current_tab = index;
                }

                if add_tab {
                    self.add_tab();
                }

                ui.separator();
                if let Some(path) = self.get_script_ref().path.clone() {
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        let steal_vs_code_breadcrumb_design = path
                        .components()
                        .filter(|component| !matches!(component.as_os_str().to_str(), Some("\\") | Some("/")))
                        .map(|component| component.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(" > ");

                        ui.add(egui::Label::new(steal_vs_code_breadcrumb_design).wrap_mode(egui::TextWrapMode::Truncate));
                    });
                } else {
                    ui.label("");
                }

                egui::Panel::top("Lua")
                    .resizable(true)
                    .frame(egui::Frame::window(&egui_ctx.global_style()).fill(bg_color))
                    .default_size(400.0)
                    .show(ui, |ui| {
                        let completer = if let Some(table) = member_table(&self.tabs[self.current_tab].code, self.last_cursor)
                            && let Some(member_completer) = self.member_completers.get_mut(table)
                        {
                            member_completer
                        } else {
                            &mut self.completer
                        };

                        let (editor, _) = CodeEditor::default()
                            .id_source("Lua Editpr")
                            .with_rows(16)
                            .with_fontsize(14.0)
                            .with_theme(self.theme.to_theme())
                            .with_numlines(true)
                            .vscroll(true)
                            .show_with_completer(ui, &mut self.tabs[self.current_tab].code, &self.syntax, completer);

                        focused = editor.response.has_focus();

                        if let Some(range) = editor.cursor_range {
                            self.last_cursor = range.primary.index.into();
                        }

                        self.bootleg_copy_shortcut(editor.cursor_range);
                    });

                ui.add_space(3.0);
                ui.separator();
                ui.monospace(egui::RichText::new(format!("{} Console", icons::TERMINAL_WINDOW)).color(text_color));
                ui.separator();

                ScrollArea::vertical()
                    .id_salt("Lua Output")
                    .max_height(ui.available_height())
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show_rows(
                        ui,
                        ui.text_style_height(&egui::TextStyle::Monospace),
                        self.output.len(),
                        |ui, range| {
                            for line in &self.output[range] {
                                ui.monospace(egui::RichText::new(line).color(text_color));
                            }
                        },
                    );

                if new_file_shortcut() && self.file_dialog.is_none() {
                    self.add_tab();
                }

                if save_shortcut() {
                    if let Some(path) = self.get_script_ref().path.clone() && path.exists() && self.file_dialog.is_none() {
                        self.quick_save(&path);
                    } else {
                        self.save_lua_script();
                    }
                }
            });

        self.opened = opened;
        self.focused = focused;

        run.then(|| self.get_script_ref().code.clone())
    }

    pub fn poll_file_dialog(&mut self) {
        let Some((kind, dialog)) = &self.file_dialog else {
            return;
        };

        let kind = *kind;

        let Some(result) = dialog.poll() else {
            return;
        };

        self.file_dialog = None;

        if let Some(path) = result {
            match kind {
                EditorDialog::Open => self.load_script(path),
                EditorDialog::Save => self.quick_save(&path),
            }
        }
    }

    pub fn push_output(&mut self, lines: Vec<String>) {
        self.output.extend(lines);
        if self.output.len() > 1000 {
            let excess = self.output.len() - 1000;
            self.output.drain(..excess);
        }
    }

    pub fn occupied(&self) -> bool {
        self.opened && self.focused
    }

    pub fn take_termination_request(&mut self) -> bool {
        take(&mut self.termination_request)
    }

    fn get_script_mut(&mut self) -> &mut ScriptTab {
        self.tabs.get_mut(self.current_tab).unwrap()
    }

    fn get_script_ref(&self) -> &ScriptTab {
        self.tabs.get(self.current_tab).unwrap()
    }

    fn steal_vs_code_naming_scheme(&self) -> String {
        let mut values: HashSet<u64> = HashSet::with_capacity(MAX_TABS);
        for tab in self.tabs.iter() {
            if self.regex.is_match(&tab.name) {
                values.insert(
                    tab.name
                        .split("-")
                        .last()
                        .and_then(|string| string.parse::<u64>().ok())
                        .unwrap(),
                );
            }
        }

        let possible_values: HashSet<u64> = (1..=MAX_TABS as u64).collect();
        let differences = possible_values
            .difference(&values)
            .cloned()
            .collect::<Vec<u64>>();
        let number = differences.iter().min().unwrap();

        format!("Untitled-{number}")
    }

    fn max_tabs_reached(&self) -> bool {
        self.tabs.len() == MAX_TABS
    }

    fn add_tab(&mut self) {
        // for safety in case i forget to add protection elsewhere
        if self.max_tabs_reached() {
            return;
        }

        let id = self.next_tab_id;
        self.next_tab_id += 1;

        let tab = ScriptTab {
            id,
            code: String::new(),
            name: self.steal_vs_code_naming_scheme(),
            path: None,
        };

        self.tabs.push(tab);

        self.current_tab = self.tabs.len() - 1
    }

    fn drop_tab(&mut self, index: usize) {
        if self.tabs.len() == 1 {
            return;
        }

        self.tabs.remove(index);

        if self.current_tab > index || self.current_tab == self.tabs.len() {
            self.current_tab -= 1;
        }
    }

    fn quick_save(&mut self, path: &PathBuf) {
        match write(path, self.get_script_ref().code.clone()) {
            Ok(_) => {
                self.update_script_path(path.to_path_buf());
                self.output.push(format!(
                    "current script saved to: {}",
                    path.to_str().unwrap()
                ));
            }
            Err(err) => {
                self.output.push(err.to_string());

                return;
            }
        }
    }

    fn load_script(&mut self, path: PathBuf) {
        match read_to_string(&path) {
            Ok(text) => {
                self.get_script_mut().code = text;
                self.update_script_path(path);
            }
            Err(err) => self.output.push(format!("Failed to read script: {err}")),
        }
    }

    fn load_lua_script(&mut self) {
        if self.file_dialog.is_some() {
            return;
        }

        self.file_dialog = Some((EditorDialog::Open, PendingDialog::open_lua_script()));
    }

    fn save_lua_script(&mut self) {
        if self.file_dialog.is_some() {
            return;
        }

        let source_path = if let Some(script_path) = self.get_script_ref().path.clone() {
            script_path
        } else {
            let mut name = self.get_script_ref().name.clone();
            name.push_str(".lua");

            PathBuf::from(name)
        };

        self.file_dialog = Some((
            EditorDialog::Save,
            PendingDialog::new(move || {
                FileDialog::new()
                    .set_file_name(source_path.file_name().unwrap().to_string_lossy())
                    .save_file()
            }),
        ));
    }

    fn update_script_path(&mut self, path: PathBuf) {
        if let Some(filename) = path.file_name()
            && let Some(script_name) = filename.to_owned().into_string().ok()
        {
            self.get_script_mut().name = script_name;
        }

        self.get_script_mut().path = Some(path);
    }

    // found that only way to get copy to clipboard is work is by delaying setting it to clipboard
    // else it just fails/gets wiped: https://github.com/not-fl3/miniquad/blob/master/src/native/windows/clipboard.rs
    fn bootleg_copy_shortcut(&mut self, cursor_range: Option<CCursorRange>) {
        let Some(range) = cursor_range else { return };

        if !(super_key_down() && is_key_pressed(KeyCode::C)) {
            return;
        }

        let primary: usize = range.primary.index.into();
        let secondary: usize = range.secondary.index.into();
        let (start_index, end_index) = (primary.min(secondary), primary.max(secondary));

        if start_index != end_index {
            let selected_text = self
                .get_script_ref()
                .code
                .chars()
                .skip(start_index)
                .take(end_index - start_index)
                .collect();

            self.pending_copy = Some((selected_text, Instant::now() + COPY_DELAY));
        }
    }

    fn flush_pending_copy(&mut self) {
        let Some((text, copy_deadline)) = self.pending_copy.take() else {
            return;
        };

        if Instant::now() <= copy_deadline {
            self.pending_copy = Some((text, copy_deadline));

            return;
        }

        match self
            .clipboard
            .as_mut()
            .map(|clipboard| clipboard.set_text(text.clone()))
        {
            Some(Ok(())) => {}
            Some(Err(err)) => self
                .output
                .push(format!("Failed to copy to clipboard: {err}")),
            None => self.output.push("Clipboard unavailable".to_string()),
        }
    }
}
