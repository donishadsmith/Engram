use chrono::Local;
use egui::{Context, ScrollArea, Window};
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};
use rfd::FileDialog;
use std::{
    fs::{read_to_string, rename, write},
    path::PathBuf,
};

pub struct LuaEditor {
    pub code: String,
    pub opened: bool,
    focused: bool,
    output: Vec<String>,
}

impl LuaEditor {
    pub fn new() -> Self {
        Self {
            code: String::new(),
            opened: false,
            focused: false,
            output: Vec::new(),
        }
    }

    pub fn show_ui(&mut self, egui_ctx: &Context) -> Option<String> {
        let mut opened = self.opened;
        let mut focused = false;
        let mut run = false;

        Window::new("Lua Editor")
            .open(&mut opened)
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Load Script").clicked() {
                            if let Some(path) = open_lua_script() {
                                match read_to_string(&path) {
                                    Ok(text) => self.code = text,
                                    Err(e) => {
                                        self.output.push(format!("Failed to read script: {e}"))
                                    }
                                }
                            }
                        }

                        if ui.button("Save Script").clicked() {
                            self.save_lua_script();
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Help").clicked() {
                            self.code = "help()".to_string();
                            run = true;
                        }

                        if ui.button("Clear").clicked() {
                            self.output.clear();
                        }

                        if ui.button("Run").clicked() {
                            run = true;
                        }
                    });
                });

                focused = CodeEditor::default()
                    .id_source("Lua Editpr")
                    .with_rows(12)
                    .with_fontsize(14.0)
                    .with_theme(ColorTheme::GITHUB_DARK)
                    .with_syntax(Syntax::lua())
                    .with_numlines(true)
                    .show(ui, &mut self.code)
                    .response
                    .has_focus();

                ui.separator();

                ScrollArea::vertical()
                    .id_salt("Lua Output")
                    .max_height(150.0)
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

    fn save_lua_script(&mut self) {
        let source_path = PathBuf::from(format!(
            "script_{}.lua",
            Local::now().format("%Y%m%d_%H%M%S")
        ));

        match write(&source_path, self.code.clone()) {
            Ok(_) => {}
            Err(e) => {
                self.output.push(e.to_string());

                return;
            }
        }

        let destination_path = FileDialog::new()
            .set_file_name(source_path.file_name().unwrap().to_string_lossy())
            .save_file();

        if let Some(destination_path) = destination_path {
            match rename(&source_path, &destination_path) {
                Ok(_) => self
                    .output
                    .push(format!("File saved to: {:?}", &destination_path)),
                Err(e) => self.output.push(e.to_string()),
            }
        }
    }
}

fn open_lua_script() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select Lua Script")
        .add_filter("Lua", &["lua"])
        .pick_file()
}
