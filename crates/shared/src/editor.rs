use crate::script::API;
use arboard::Clipboard;
use egui::{Context, ScrollArea, Window, text::CursorRange};
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};
use egui_phosphor::regular as icons;
use macroquad::input::{
    KeyCode::{self, RightShift},
    is_key_down, is_key_pressed,
};
use rfd::FileDialog;
use std::{
    fs::{read_to_string, write},
    mem::take,
    path::PathBuf,
    time::{Duration, Instant},
};

const COPY_DELAY: Duration = Duration::from_millis(200);
const SHORTCUT: &str = r#"
Shift+Enter: run the current script
Ctrl+N: new script
Ctrl+S: save the current script
Ctrl+C: copy the selected text
F8: open or close the Lua editor

Notes:
    - Cmd button can replace Ctrl
    - Other shortcuts such as Ctrl+Z, Ctrl+V, Ctrl+A, also usable
"#;
#[derive(PartialEq)]
enum HelpTab {
    Api,
    Shortcut,
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

pub struct LuaEditor {
    pub code: String,
    pub opened: bool,
    focused: bool,
    pub output: Vec<String>,
    pub termination_request: bool,
    pending_copy: Option<(String, Instant)>,
    clipboard: Option<Clipboard>,
    pub help_window: bool,
    script_name: String, // very vs coded
    script_path: Option<PathBuf>,
    help_tab: HelpTab,
}

impl LuaEditor {
    pub fn new() -> Self {
        Self {
            code: String::new(),
            opened: false,
            focused: false,
            output: Vec::new(),
            termination_request: false,
            pending_copy: None,
            clipboard: Clipboard::new().ok(),
            help_window: false,
            script_name: "Untitled-1.lua".to_string(),
            script_path: None,
            help_tab: HelpTab::Api,
        }
    }

    pub fn show_ui(&mut self, egui_ctx: &Context) -> Option<String> {
        self.flush_pending_copy();

        let mut opened = self.opened;
        let mut focused = false;
        let mut run = false;

        Window::new("Lua Editor")
            .open(&mut opened)
            .show(egui_ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.menu_button("File", |ui| {
                            if ui.button(format!("{}  New Script", icons::FILE)).clicked() {
                                self.new_file();
                            }

                            if ui.button(format!("{}  Load Script", icons::FOLDER_OPEN)).clicked() {
                                if let Some(path) = open_lua_script() {
                                    match read_to_string(&path) {
                                        Ok(text) => {
                                            self.code = text;
                                            self.update_script_path(path);
                                        },
                                        Err(e) => {
                                            self.output.push(format!("Failed to read script: {e}"))
                                        }
                                    }
                                }
                            }

                            if ui.button(format!("{}  Save Script", icons::FLOPPY_DISK)).clicked() {
                                self.save_lua_script();
                            }
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(format!("{}  Help", icons::INFO)).clicked() && !self.help_window {
                                self.help_window = true;
                            }

                            egui::Window::new("Help").default_width(500.0).vscroll(false).open(&mut self.help_window).show(ui.ctx(), |ui| {
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

                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [200.0, ui.available_height()],
                            egui::Label::new(&self.script_name)
                                .wrap_mode(egui::TextWrapMode::Truncate)
                                .selectable(false)
                        );
                    });
                });

                egui::TopBottomPanel::top("Lua")
                    .resizable(true)
                    .default_height(200.0)
                    .show_inside(ui, |ui| {
                        let editor = CodeEditor::default()
                            .id_source("Lua Editpr")
                            .with_rows(16)
                            .with_fontsize(14.0)
                            .with_theme(ColorTheme::GITHUB_DARK)
                            .with_syntax(Syntax::lua())
                            .with_numlines(true)
                            .vscroll(true)
                            .show(ui, &mut self.code);

                        focused = editor.response.has_focus();

                        self.bootleg_copy_shortcut(editor.cursor_range);
                    });

                ui.add_space(3.0);

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
                                ui.monospace(line);
                            }
                        },
                    );

                if new_file_shortcut() {
                    self.new_file();
                }

                if save_shortcut() {
                    if let Some(path) = self.script_path.clone() && path.exists() {
                        self.quick_save(&path);
                    } else {
                        self.save_lua_script();
                    }
                }
            });

        self.opened = opened;
        self.focused = focused;

        run.then(|| self.code.clone())
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

    pub fn new_file(&mut self) {
        self.code.clear();
        self.script_path = None;
        self.script_name = "Untitled-1.lua".to_string();
    }

    pub fn quick_save(&mut self, path: &PathBuf) {
        match write(path, self.code.clone()) {
            Ok(_) => {
                self.update_script_path(path.to_path_buf());
                self.output.push(format!(
                    "current script saved to: {}",
                    path.to_str().unwrap()
                ));
            }
            Err(e) => {
                self.output.push(e.to_string());

                return;
            }
        }
    }

    fn save_lua_script(&mut self) {
        let source_path = if let Some(script_path) = self.script_path.clone() {
            script_path
        } else {
            PathBuf::from(self.script_name.clone())
        };

        let destination_path = FileDialog::new()
            .set_file_name(source_path.file_name().unwrap().to_string_lossy())
            .save_file();

        match destination_path {
            Some(path) => self.quick_save(&path),
            None => {}
        }
    }

    fn update_script_path(&mut self, path: PathBuf) {
        if let Some(filename) = path.file_name()
            && let Some(script_name) = filename.to_owned().into_string().ok()
        {
            self.script_name = script_name;
        }

        self.script_path = Some(path);
    }

    // found that only way to get copy to clipboard is work is by delaying setting it to clipboard
    // else it just fails/gets wiped: https://github.com/not-fl3/miniquad/blob/master/src/native/windows/clipboard.rs
    fn bootleg_copy_shortcut(&mut self, cursor_range: Option<CursorRange>) {
        let Some(range) = cursor_range else { return };

        if !(super_key_down() && is_key_pressed(KeyCode::C)) {
            return;
        }

        let primary = range.primary.ccursor.index;
        let secondary = range.secondary.ccursor.index;
        let (start_index, end_index) = (primary.min(secondary), primary.max(secondary));

        if start_index != end_index {
            let selected_text = self
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

fn open_lua_script() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select Lua Script")
        .add_filter("Lua", &["lua"])
        .pick_file()
}
