//! Lucide icon helpers for the embedded editor UI.
//!
//! The frontend registers `lucide.ttf` in the egui context at startup. This
//! module keeps the editor crate independent while sharing the same icon
//! codepoint style used by Bonnie-32 and the emulator shell.

#![allow(dead_code)]

use egui::{FontFamily, FontId, RichText};

pub const ARROW_UP_FROM_LINE: char = '\u{e45a}';
pub const AUDIO_LINES: char = '\u{e2b2}';
pub const BLEND: char = '\u{e59c}';
pub const BOX: char = '\u{e061}';
pub const BRICK_WALL: char = '\u{e581}';
pub const CHEVRON_DOWN: char = '\u{e06d}';
pub const CHEVRON_LEFT: char = '\u{e06e}';
pub const CHEVRON_RIGHT: char = '\u{e06f}';
pub const CHEVRON_UP: char = '\u{e070}';
pub const CIRCLE_DOT: char = '\u{e345}';
pub const COPY: char = '\u{e09e}';
pub const COLUMNS: char = '\u{e098}';
pub const FOOTPRINTS: char = '\u{e3b9}';
pub const EYE: char = '\u{e0ba}';
pub const EYE_OFF: char = '\u{e0bb}';
pub const FILE: char = '\u{e0c5}';
pub const FILE_PLUS: char = '\u{e0c9}';
pub const FLAME: char = '\u{e0d2}';
pub const FOCUS: char = '\u{e29e}';
pub const FOLDER: char = '\u{e0d8}';
pub const GRID: char = '\u{e0e9}';
pub const HAMMER: char = '\u{e0ec}';
pub const HOUSE: char = '\u{e0f5}';
pub const LAYERS: char = '\u{e529}';
pub const LAYERS_2: char = '\u{e52a}';
pub const MAP_PIN: char = '\u{e111}';
pub const MAP: char = '\u{e110}';
pub const MOUNTAIN: char = '\u{e231}';
pub const MOVE: char = '\u{e121}';
pub const PALETTE: char = '\u{e12f}';
pub const PIPETTE: char = '\u{e4c6}';
pub const PENCIL: char = '\u{e1f9}';
pub const REDO: char = '\u{e2a0}';
pub const SEARCH: char = '\u{e151}';
pub const UNDO: char = '\u{e2a1}';
pub const IMAGE_PLUS: char = '\u{e1f7}';
pub const KEYBOARD: char = '\u{e284}';
pub const PEN_LINE: char = '\u{e130}';
pub const PLAY: char = '\u{e13c}';
pub const PLUS: char = '\u{e13d}';
pub const POINTER: char = '\u{e1e8}';
pub const ROWS_3: char = '\u{e58a}';
pub const ROTATE_3D: char = '\u{e2ea}';
pub const ROTATE_CCW: char = '\u{e144}';
pub const SAVE: char = '\u{e14d}';
pub const SCALE_3D: char = '\u{e2eb}';
pub const SCAN: char = '\u{e257}';
pub const SQUARE: char = '\u{e167}';
pub const SUN: char = '\u{e178}';
pub const TERMINAL: char = '\u{e183}';
pub const STAIRS: char = '\u{e06a}';
pub const TRENDING_UP: char = '\u{e191}';
pub const TRASH: char = '\u{e18d}';
pub const WAYPOINT: char = '\u{e546}';

pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("lucide".into()))
}

pub fn text(ch: char, size: f32) -> RichText {
    RichText::new(ch.to_string()).font(font(size))
}

pub fn label(ch: char, label: &str) -> String {
    format!("{ch}  {label}")
}
