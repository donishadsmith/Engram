use crate::traits::UnsignedInt;
use chrono::Local;
use egui::Ui;
use macroquad::prelude::*;
use std::{
    fmt::Debug,
    hash::Hash,
    io::{Error, ErrorKind},
    path::PathBuf,
};

pub fn screenshot(image_dir: PathBuf) {
    let image_path = image_dir.join(format!(
        "screenshot_{}.png",
        Local::now().format("%Y%m%d_%H%M%S")
    ));

    match &image_path.to_str() {
        Some(image) => get_screen_data().export_png(&image),
        None => {}
    }
}

pub fn error_message(message: String) -> Error {
    Error::new(ErrorKind::InvalidData, message)
}

pub fn zero_arr<T: UnsignedInt>(capacity: usize) -> Box<[T]> {
    vec![T::ZERO; capacity]
        .into_boxed_slice()
        .try_into()
        .unwrap()
}

pub fn saturated_arr<T: UnsignedInt>(capacity: usize) -> Box<[T]> {
    vec![T::MAX; capacity]
        .into_boxed_slice()
        .try_into()
        .unwrap()
}

pub fn get_halfword_shift(address: u32) -> u8 {
    if address & 2 == 0 { 0 } else { 16 }
}

pub fn get_word_mask(address: u32) -> u32 {
    !(0xFFFF << get_halfword_shift(address))
}

pub fn kilobytes(value: usize) -> usize {
    value * 1024
}

pub fn centered_row<R>(
    ui: &mut egui::Ui,
    id_name: impl Hash + Debug,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let id = ui.id().with(id_name);
    let last_width: Option<f32> = ui.data(|data| data.get_temp(id));
    let available = ui.available_width();

    ui.horizontal(|ui| {
        if let Some(width) = last_width {
            ui.add_space(((available - width) / 2.0).max(0.0));
        }

        let start = ui.cursor().min.x;
        let result = add_contents(ui);
        ui.data_mut(|data| data.insert_temp(id, ui.min_rect().right() - start));

        result
    })
    .inner
}
