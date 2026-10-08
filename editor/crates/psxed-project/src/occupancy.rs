//! Occupancy report: how full the PS1's three memories are for one cooked
//! project's guest executable.
//!
//! RAM comes from the linker's own map (`PSOXIDE_GUEST_LINK_MAP`, an lld `-Map`
//! file): section sizes, the stack reserve, the exact headroom before the link
//! would fail, and every large static (the runtime arenas, the baked tables).
//! Heap use needs the live bump allocator, so it is read from a RAM dump
//! taken from an emulator run, at the symbol the map names. VRAM and SPU RAM
//! come in two forms: what the cook asks for (texture sizes, SFX bank bytes)
//! and, when an emulator dump is supplied, what is actually in the memory.
//!
//! Every row carries its source, so a number from the link map is never
//! mistaken for one measured in a run:
//!
//! - `link-map`, `exe-header`: the build.
//! - `manifest`, `cook`: the cooked project (`level_manifest.rs`, the package).
//! - `layout-contract`: constants of `engine/examples/editor-playtest`
//!   (`vram_runtime.rs`, `runtime_config.rs`, `game_app.rs`), restated here.
//! - `emulator-ram`, `emulator-vram`, `emulator-spu`: read out of a dump.
//!
//! No part of this changes the cook or the runtime.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

use crate::playtest::{
    audit_resident_assets, cooked_playtest_budgets, PlaytestAssetKind, PlaytestPackage,
    PLAYTEST_RESIDENT_ASSET_PAGES,
};
use crate::ProjectDocument;

/// Main RAM of the console.
pub const RAM_BYTES: u32 = 2 * 1024 * 1024;
/// SPU RAM of the console.
pub const SPU_RAM_BYTES: u32 = 512 * 1024;
/// SPU RAM below this belongs to the hardware's decode buffers.
pub const SPU_HARDWARE_BYTES: u32 = 0x1000;
/// End of the silence block `psx_spu::init` writes at 0x1000.
pub const SPU_SILENCE_END: u32 = 0x1010;
/// Where the engine puts the first cooked UI/gameplay SFX sample
/// (`UI_SFX_SAMPLE_BASE_BYTES` in `game_app.rs`).
pub const SPU_SFX_BANK_BASE: u32 = 0x30000;
/// The most SFX samples the engine uploads (`MAX_UI_SFX_SAMPLES`).
pub const SPU_SFX_SAMPLE_LIMIT: usize = 64;
/// `psx_spu::init` parks the reverb work area at 0xFFFE * 8.
pub const SPU_REVERB_BASE: u32 = 0xFFFE * 8;
/// VRAM width in halfwords and height in lines.
pub const VRAM_WIDTH: usize = 1024;
/// VRAM height in lines.
pub const VRAM_HEIGHT: usize = 512;
/// Halfwords in one 4bpp texture page column (64) and its height (256).
pub const PAGE_WIDTH: usize = 64;
/// Lines in one texture page.
pub const PAGE_HEIGHT: usize = 256;

/// A failure to read an input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccupancyError(pub String);

impl std::fmt::Display for OccupancyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for OccupancyError {}

fn err<T>(message: impl Into<String>) -> Result<T, OccupancyError> {
    Err(OccupancyError(message.into()))
}

// ---------------------------------------------------------------- the rows

/// One reported number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    /// What it is.
    pub name: String,
    /// The value.
    pub value: i64,
    /// `bytes`, `count`, `pages`, `halfwords`, ...
    pub unit: &'static str,
    /// Where it came from (see the module docs).
    pub source: &'static str,
    /// A short qualification, or empty.
    pub note: String,
}

impl Row {
    fn new(name: &str, value: i64, unit: &'static str, source: &'static str, note: &str) -> Self {
        Row {
            name: name.to_string(),
            value,
            unit,
            source,
            note: note.to_string(),
        }
    }
}

// ---------------------------------------------------------------- link map

/// One output section of the link (`.text`, `.data`, `.bss`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    /// Section name.
    pub name: String,
    /// Load address.
    pub address: u32,
    /// Size in bytes.
    pub size: u32,
}

/// One symbol the map places.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Symbol {
    /// Demangled name as lld printed it.
    pub name: String,
    /// Address.
    pub address: u32,
    /// Size in bytes (0 for labels).
    pub size: u32,
    /// Output section it lives in.
    pub section: String,
}

/// What the link map says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkMap {
    /// Output sections in link order.
    pub sections: Vec<Section>,
    /// Every sized symbol.
    pub symbols: Vec<Symbol>,
    /// Linker-script scalars and `name = .` markers (`__bss_end`, `STACK_INIT`).
    pub defs: BTreeMap<String, u64>,
}

fn parse_number(token: &str) -> Option<u64> {
    let token = token.trim();
    let (digits, scale) = if let Some(rest) = token.strip_suffix(['K', 'k']) {
        (rest, 1024)
    } else if let Some(rest) = token.strip_suffix(['M', 'm']) {
        (rest, 1024 * 1024)
    } else {
        (token, 1)
    };
    let value = if let Some(hex) = digits.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()?
    } else {
        digits.parse::<u64>().ok()?
    };
    Some(value * scale)
}

/// Evaluate `A + B - C` over numbers and earlier definitions.
fn evaluate(expression: &str, defs: &BTreeMap<String, u64>) -> Option<u64> {
    let mut total: i128 = 0;
    let mut sign = 1i128;
    let mut term = String::new();
    let flush = |term: &mut String, sign: i128, total: &mut i128| -> Option<()> {
        let text = term.trim();
        if text.is_empty() {
            return Some(());
        }
        let value = parse_number(text).or_else(|| defs.get(text).copied())?;
        *total += sign * i128::from(value);
        term.clear();
        Some(())
    };
    for ch in expression.chars() {
        if ch == '+' || ch == '-' {
            flush(&mut term, sign, &mut total)?;
            sign = if ch == '+' { 1 } else { -1 };
        } else {
            term.push(ch);
        }
    }
    flush(&mut term, sign, &mut total)?;
    u64::try_from(total).ok()
}

/// Parse an lld `-Map` file: `VMA LMA Size Align Out In Symbol` rows, where
/// the indentation after the fourth column says whether the row is an output
/// section (none), an input section (8 columns) or a symbol (16 columns).
pub fn parse_link_map(text: &str) -> Result<LinkMap, OccupancyError> {
    let mut map = LinkMap::default();
    let mut section = String::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let (Some(vma), Some(_lma), Some(size), Some(align)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let (Some(vma), Some(size)) = (
            u64::from_str_radix(vma, 16).ok(),
            u64::from_str_radix(size, 16).ok(),
        ) else {
            continue;
        };
        if align.parse::<u32>().is_err() {
            continue;
        }
        // Everything after the fourth field, separator included.
        let mut rest = line;
        for _ in 0..4 {
            let trimmed = rest.trim_start();
            let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
            rest = &trimmed[end..];
        }
        let indent = rest.len() - rest.trim_start().len();
        let body = rest.trim();
        if body.is_empty() {
            continue;
        }
        if let Some((name, expression)) = body.split_once(" = ") {
            let name = name.trim();
            if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                let expression = expression.trim().trim_end_matches(';');
                let value = if expression == "." {
                    Some(vma)
                } else {
                    evaluate(expression, &map.defs)
                };
                if let Some(value) = value {
                    map.defs.insert(name.to_string(), value);
                }
                continue;
            }
        }
        match indent {
            1 if body.starts_with('.') && !body.contains(' ') => {
                section = body.to_string();
                map.sections.push(Section {
                    name: section.clone(),
                    address: vma as u32,
                    size: size as u32,
                });
            }
            17 => map.symbols.push(Symbol {
                name: body.to_string(),
                address: vma as u32,
                size: size as u32,
                section: section.clone(),
            }),
            _ => {}
        }
    }
    if map.sections.is_empty() {
        return err("no output sections: is this an lld -Map file?");
    }
    Ok(map)
}

impl LinkMap {
    fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    fn def(&self, name: &str) -> Option<u32> {
        self.defs.get(name).map(|&v| v as u32)
    }

    /// The runtime bump allocator's two words (`next`, then `end`), if linked.
    fn allocator_words(&self) -> Option<(u32, u32)> {
        let mut words: Vec<u32> = self
            .symbols
            .iter()
            .filter(|s| s.name.starts_with("psx_rt::heap::ALLOCATOR"))
            .map(|s| s.address)
            .collect();
        words.sort_unstable();
        match words.as_slice() {
            [next, end, ..] => Some((*next, *end)),
            _ => None,
        }
    }
}

// --------------------------------------------------------------- exe header

/// The fields of a PS-X EXE header the report quotes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExeHeader {
    /// Entry point.
    pub initial_pc: u32,
    /// Load address of the image.
    pub load_address: u32,
    /// Bytes the kernel copies from the file (everything but `.bss`).
    pub payload_bytes: u32,
    /// Initial stack pointer.
    pub stack_pointer: u32,
    /// Size of the whole file.
    pub file_bytes: u64,
}

/// Parse the 0x800-byte PS-X EXE header (psx-spx "CDROM File Formats").
pub fn parse_exe(bytes: &[u8]) -> Result<ExeHeader, OccupancyError> {
    if bytes.len() < 0x800 || &bytes[..8] != b"PS-X EXE" {
        return err("not a PS-X EXE");
    }
    let word =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Ok(ExeHeader {
        initial_pc: word(0x10),
        load_address: word(0x18),
        payload_bytes: word(0x1C),
        stack_pointer: word(0x30),
        file_bytes: bytes.len() as u64,
    })
}

// ------------------------------------------------------------------- RAM

/// A large static, and which section it is in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Arena {
    /// Symbol name.
    pub name: String,
    /// Size in bytes.
    pub bytes: u32,
    /// Section.
    pub section: String,
    /// Address.
    pub address: u32,
}

/// Main RAM at link time, plus the heap if a dump was supplied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RamReport {
    /// Every row, source-labelled.
    pub rows: Vec<Row>,
    /// Statics of at least [`ARENA_MIN_BYTES`], largest first.
    pub arenas: Vec<Arena>,
}

/// Statics smaller than this are left out of the arena list.
pub const ARENA_MIN_BYTES: u32 = 1024;

/// RAM occupancy from the link map, the executable header and an optional
/// RAM dump (2 MiB, physical address 0 = 0x80000000, as the emulator writes it).
pub fn ram_report(map: &LinkMap, exe: Option<&ExeHeader>, ram_dump: Option<&[u8]>) -> RamReport {
    let mut rows = Vec::new();
    let size_of = |name: &str| map.section(name).map_or(0, |s| i64::from(s.size));
    let (text, data, bss) = (size_of(".text"), size_of(".data"), size_of(".bss"));
    rows.push(Row::new(".text", text, "bytes", "link-map", "machine code"));
    rows.push(Row::new(
        ".data",
        data,
        "bytes",
        "link-map",
        "initialised data, read-only tables included",
    ));
    rows.push(Row::new(
        ".bss",
        bss,
        "bytes",
        "link-map",
        "zero-initialised, not stored in the exe",
    ));
    if let Some(header) = map.section(".psx_exe_header") {
        rows.push(Row::new(
            "exe header",
            i64::from(header.size),
            "bytes",
            "link-map",
            "",
        ));
    }

    let ram_base = map.def("RAM_BASE").unwrap_or(0x8000_0000);
    let ram_size = map.def("RAM_SIZE").unwrap_or(RAM_BYTES);
    let load = map.def("LOAD_ADDR").unwrap_or(ram_base + 0x1_0000);
    let reserve = map.def("STACK_RESERVE");
    let stack_init = map.def("STACK_INIT");
    let bss_end = map.def("__bss_end").or_else(|| {
        let bss = map.section(".bss")?;
        Some(bss.address + bss.size)
    });
    let region_end = reserve.map(|reserve| ram_base + ram_size - reserve);
    if let Some(reserve) = reserve {
        rows.push(Row::new(
            "stack reserve",
            i64::from(reserve),
            "bytes",
            "link-map",
            "carved out of the region the statics may use",
        ));
    }
    if let Some(top) = stack_init {
        rows.push(Row::new(
            "initial stack pointer",
            i64::from(top),
            "address",
            "link-map",
            "",
        ));
    }
    if let (Some(end), Some(bss_end)) = (region_end, bss_end) {
        rows.push(Row::new(
            "static region",
            i64::from(end) - i64::from(load),
            "bytes",
            "link-map",
            "text + data + bss may use this much, from the load address",
        ));
        let image = i64::from(bss_end) - i64::from(load);
        rows.push(Row::new(
            "static image",
            image,
            "bytes",
            "link-map",
            "text + data + bss, from the load address",
        ));
        rows.push(Row::new(
            "static headroom",
            i64::from(end) - i64::from(bss_end),
            "bytes",
            "link-map",
            "exact: the link fails at a negative value",
        ));
    }
    let heap_start = map.def("__heap_start").or(bss_end);
    let heap_end = map
        .def("__heap_end")
        .or(stack_init.zip(reserve).map(|(top, r)| top - r));
    if let (Some(start), Some(end)) = (heap_start, heap_end) {
        rows.push(Row::new(
            "heap capacity",
            i64::from(end) - i64::from(start),
            "bytes",
            "link-map",
            "__heap_start to __heap_end, what a bump allocator may hand out",
        ));
    }
    if let Some(exe) = exe {
        rows.push(Row::new(
            "exe payload",
            i64::from(exe.payload_bytes),
            "bytes",
            "exe-header",
            "what the kernel copies from disc",
        ));
        rows.push(Row::new(
            "exe file",
            exe.file_bytes as i64,
            "bytes",
            "exe-header",
            &format!("{} sectors", exe.file_bytes.div_ceil(2048)),
        ));
        let linked = text + data;
        rows.push(Row::new(
            "exe payload minus text+data",
            i64::from(exe.payload_bytes) - linked,
            "bytes",
            "exe-header",
            "0 when the header and the map describe the same link",
        ));
    }
    match (map.allocator_words(), ram_dump) {
        (Some((next_at, end_at)), Some(dump)) if dump.len() >= RAM_BYTES as usize => {
            let read = |address: u32| -> u32 {
                let at = (address & (RAM_BYTES - 1)) as usize;
                u32::from_le_bytes([dump[at], dump[at + 1], dump[at + 2], dump[at + 3]])
            };
            let (next, end) = (read(next_at), read(end_at));
            let start = heap_start.unwrap_or(0);
            rows.push(Row::new(
                "heap used",
                i64::from(next) - i64::from(start),
                "bytes",
                "emulator-ram",
                &format!("allocator next {next:#010x}"),
            ));
            rows.push(Row::new(
                "heap free",
                i64::from(end) - i64::from(next),
                "bytes",
                "emulator-ram",
                &format!("allocator end {end:#010x}; at the instant of the dump"),
            ));
        }
        (None, Some(_)) => rows.push(Row::new(
            "heap free",
            -1,
            "bytes",
            "emulator-ram",
            "no psx_rt::heap::ALLOCATOR in the map: the alloc feature is off, no heap",
        )),
        _ => {}
    }

    let mut arenas: Vec<Arena> = map
        .symbols
        .iter()
        .filter(|s| s.size >= ARENA_MIN_BYTES && (s.section == ".bss" || s.section == ".data"))
        .map(|s| Arena {
            name: s.name.clone(),
            bytes: s.size,
            section: s.section.clone(),
            address: s.address,
        })
        .collect();
    arenas.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.name.cmp(&b.name)));
    RamReport { rows, arenas }
}

// -------------------------------------------------------------- manifest

/// Capacities and counts stated by `generated/level_manifest.rs`:
/// every `pub const NAME: type = number;` line.
pub fn manifest_rows(text: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("pub const ") else {
            continue;
        };
        let Some((name, rest)) = rest.split_once(':') else {
            continue;
        };
        let Some((ty, value)) = rest.split_once('=') else {
            continue;
        };
        let (ty, value) = (ty.trim(), value.trim().trim_end_matches(';').trim());
        if !matches!(ty, "usize" | "u32" | "u16" | "u8") {
            continue;
        }
        let digits: String = value.chars().filter(|&c| c != '_').collect();
        let Some(number) = parse_number(&digits) else {
            continue;
        };
        rows.push(Row::new(
            name.trim(),
            number as i64,
            "count",
            "manifest",
            ty,
        ));
    }
    rows
}

/// Arena sizes the manifest's counts imply: pages are 2 KiB disc pages
/// (`PLAYTEST_RESIDENT_ASSET_PAGE_BYTES`), and the PXBSP face chain is `u16`s.
pub fn derived_arena_rows(manifest: &[Row]) -> Vec<Row> {
    let mut rows = Vec::new();
    for row in manifest {
        if row.name.ends_with("_PAGE_COUNT") {
            rows.push(Row::new(
                &row.name.replace("_COUNT", "_BYTES"),
                row.value * 2048,
                "bytes",
                "manifest",
                "pages x 2048",
            ));
        } else if row.name == "PXBSP_FACE_CHAIN_CAPACITY" {
            rows.push(Row::new(
                "PXBSP_FACE_CHAIN_BYTES",
                row.value * 2,
                "bytes",
                "manifest",
                "u16 per face, one overlay in RUNTIME_ARENAS",
            ));
        }
    }
    rows
}

// ------------------------------------------------------------------ VRAM

/// A rectangle in VRAM halfwords and lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

/// What the editor-playtest guest claims in VRAM (`vram_runtime.rs`,
/// `runtime_config.rs`): fixed by the layout contract, not by the dump.
const VRAM_CLAIMS: &[(Rect, &str)] = &[
    (
        Rect {
            x: 0,
            y: 0,
            w: 320,
            h: 480,
        },
        "framebuffers (2 x 320x240, 16bpp)",
    ),
    (
        Rect {
            x: 384,
            y: 256,
            w: 128,
            h: 256,
        },
        "model atlas (8bpp, up to 128 hw wide)",
    ),
    (
        Rect {
            x: 640,
            y: 0,
            w: 384,
            h: 512,
        },
        "room material pages (4bpp, 64 hw stride)",
    ),
    (
        Rect {
            x: 0,
            y: 480,
            w: 1024,
            h: 32,
        },
        "CLUT band (rows 480 to 511)",
    ),
];

fn overlap(a: Rect, b: Rect) -> usize {
    let x = a.x.max(b.x)..(a.x + a.w).min(b.x + b.w);
    let y = a.y.max(b.y)..(a.y + a.h).min(b.y + b.h);
    x.len() * y.len()
}

/// One 64x256 page of VRAM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VramPage {
    /// Column 0 to 15 (x / 64).
    pub column: usize,
    /// Row 0 or 1 (y / 256).
    pub row: usize,
    /// Halfwords that are not zero, or `None` with no dump.
    pub used_halfwords: Option<usize>,
    /// The layout-contract owners overlapping this page, largest first.
    pub claimed_by: Vec<String>,
}

/// One CLUT row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClutRow {
    /// VRAM line.
    pub y: usize,
    /// Non-zero halfwords on the line.
    pub used_halfwords: usize,
    /// First and last non-zero x, if any.
    pub extent: Option<(usize, usize)>,
}

/// VRAM page occupancy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VramReport {
    /// Where the numbers come from.
    pub source: &'static str,
    /// The 32 pages, row-major.
    pub pages: Vec<VramPage>,
    /// CLUT-band rows with data.
    pub clut_rows: Vec<ClutRow>,
    /// Summary rows.
    pub rows: Vec<Row>,
}

fn page_claims(column: usize, row: usize) -> Vec<String> {
    let page = Rect {
        x: column * PAGE_WIDTH,
        y: row * PAGE_HEIGHT,
        w: PAGE_WIDTH,
        h: PAGE_HEIGHT,
    };
    let mut found: Vec<(usize, &str)> = VRAM_CLAIMS
        .iter()
        .map(|(rect, name)| (overlap(page, *rect), *name))
        .filter(|(area, _)| *area > 0)
        .collect();
    found.sort_by_key(|found| std::cmp::Reverse(found.0));
    if found.is_empty() {
        return vec!["unified allocator (fonts, sky, decals, UI images)".to_string()];
    }
    found
        .into_iter()
        .map(|(_, name)| name.to_string())
        .collect()
}

/// Decode a VRAM dump into per-halfword "is it blank" flags: a P6 PPM of
/// 1024x512 (the frontend's `--dump-vram`; a pixel is blank when black) or
/// 1 MiB of little-endian halfwords.
fn vram_used_flags(bytes: &[u8]) -> Result<Vec<bool>, OccupancyError> {
    if bytes.starts_with(b"P6") {
        let mut at = 2;
        let mut numbers = Vec::new();
        while numbers.len() < 3 {
            while at < bytes.len() && bytes[at].is_ascii_whitespace() {
                at += 1;
            }
            if bytes.get(at) == Some(&b'#') {
                while at < bytes.len() && bytes[at] != b'\n' {
                    at += 1;
                }
                continue;
            }
            let start = at;
            while at < bytes.len() && bytes[at].is_ascii_digit() {
                at += 1;
            }
            let Ok(number) = std::str::from_utf8(&bytes[start..at])
                .unwrap_or("")
                .parse::<usize>()
            else {
                return err("bad PPM header");
            };
            numbers.push(number);
        }
        at += 1;
        let (w, h) = (numbers[0], numbers[1]);
        if (w, h) != (VRAM_WIDTH, VRAM_HEIGHT) || numbers[2] != 255 {
            return err(format!("VRAM PPM is {w}x{h}, expected 1024x512"));
        }
        let pixels = &bytes[at.min(bytes.len())..];
        if pixels.len() < w * h * 3 {
            return err("VRAM PPM is truncated");
        }
        Ok(pixels[..w * h * 3]
            .chunks_exact(3)
            .map(|p| p != [0, 0, 0])
            .collect())
    } else if bytes.len() == VRAM_WIDTH * VRAM_HEIGHT * 2 {
        Ok(bytes.chunks_exact(2).map(|p| p != [0, 0]).collect())
    } else {
        err("VRAM dump is neither a 1024x512 P6 PPM nor 1 MiB of halfwords")
    }
}

/// Occupancy of the 32 texture pages and the CLUT rows from a VRAM dump.
pub fn vram_from_dump(bytes: &[u8]) -> Result<VramReport, OccupancyError> {
    let used = vram_used_flags(bytes)?;
    let mut pages = Vec::new();
    for row in 0..VRAM_HEIGHT / PAGE_HEIGHT {
        for column in 0..VRAM_WIDTH / PAGE_WIDTH {
            let mut count = 0;
            for y in row * PAGE_HEIGHT..(row + 1) * PAGE_HEIGHT {
                for x in column * PAGE_WIDTH..(column + 1) * PAGE_WIDTH {
                    count += usize::from(used[y * VRAM_WIDTH + x]);
                }
            }
            pages.push(VramPage {
                column,
                row,
                used_halfwords: Some(count),
                claimed_by: page_claims(column, row),
            });
        }
    }
    let mut clut_rows = Vec::new();
    for y in 480..VRAM_HEIGHT {
        let line = &used[y * VRAM_WIDTH..(y + 1) * VRAM_WIDTH];
        let count = line.iter().filter(|&&u| u).count();
        if count > 0 {
            let first = line.iter().position(|&u| u).unwrap_or(0);
            let last = line.iter().rposition(|&u| u).unwrap_or(0);
            clut_rows.push(ClutRow {
                y,
                used_halfwords: count,
                extent: Some((first, last)),
            });
        }
    }
    let total_used: usize = pages.iter().filter_map(|p| p.used_halfwords).sum();
    let framebuffer = Rect {
        x: 0,
        y: 0,
        w: 320,
        h: 480,
    };
    let mut framebuffer_used = 0;
    for y in 0..framebuffer.h {
        for x in 0..framebuffer.w {
            framebuffer_used += usize::from(used[y * VRAM_WIDTH + x]);
        }
    }
    let rows = vec![
        Row::new(
            "VRAM total",
            (VRAM_WIDTH * VRAM_HEIGHT * 2) as i64,
            "bytes",
            "layout-contract",
            "1 MiB",
        ),
        Row::new(
            "VRAM non-blank",
            (total_used * 2) as i64,
            "bytes",
            "emulator-vram",
            "non-zero halfwords; black texels count as blank",
        ),
        Row::new(
            "VRAM outside the framebuffers",
            ((VRAM_WIDTH * VRAM_HEIGHT - framebuffer.w * framebuffer.h) * 2) as i64,
            "bytes",
            "layout-contract",
            "what textures, CLUTs and the font may use",
        ),
        Row::new(
            "VRAM non-blank outside the framebuffers",
            ((total_used - framebuffer_used) * 2) as i64,
            "bytes",
            "emulator-vram",
            "",
        ),
    ];
    Ok(VramReport {
        source: "emulator-vram",
        pages,
        clut_rows,
        rows,
    })
}

/// A cooked texture the project asks the runtime to put in VRAM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CookedTexture {
    /// Source label (room material or model atlas).
    pub name: String,
    /// Width in texels.
    pub width: u16,
    /// Height in texels.
    pub height: u16,
    /// 4, 8 or 15.
    pub depth: u8,
    /// CLUT entries.
    pub clut_entries: u16,
    /// Halfwords of pixel data the upload writes.
    pub halfwords: u32,
}

/// Cooked VRAM demand, summarised.
pub fn cooked_vram_rows(textures: &[CookedTexture]) -> Vec<Row> {
    let mut rows = Vec::new();
    let pixels: u64 = textures.iter().map(|t| u64::from(t.halfwords) * 2).sum();
    let cluts: u64 = textures.iter().map(|t| u64::from(t.clut_entries) * 2).sum();
    rows.push(Row::new(
        "cooked textures",
        textures.len() as i64,
        "count",
        "cook",
        "distinct texture assets",
    ));
    rows.push(Row::new(
        "cooked texture pixels",
        pixels as i64,
        "bytes",
        "cook",
        "if every texture were resident at once",
    ));
    rows.push(Row::new(
        "cooked CLUT entries",
        cluts as i64,
        "bytes",
        "cook",
        "two bytes an entry",
    ));
    for depth in [4u8, 8, 15] {
        let n = textures.iter().filter(|t| t.depth == depth).count();
        rows.push(Row::new(
            &format!("textures at {depth}bpp"),
            n as i64,
            "count",
            "cook",
            "",
        ));
    }
    if let Some(largest) = textures.iter().max_by_key(|t| t.halfwords) {
        rows.push(Row::new(
            "largest texture",
            i64::from(largest.halfwords) * 2,
            "bytes",
            "cook",
            &format!(
                "{} ({}x{} at {}bpp)",
                largest.name, largest.width, largest.height, largest.depth
            ),
        ));
    }
    rows
}

// ------------------------------------------------------------------- SPU

/// A cooked SFX sample the engine uploads to SPU RAM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpuBank {
    /// Source label.
    pub name: String,
    /// ADPCM bytes the upload writes.
    pub bytes: u32,
}

/// One region of SPU RAM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpuRegion {
    /// What it is.
    pub name: String,
    /// First byte.
    pub start: u32,
    /// One past the last byte.
    pub end: u32,
    /// Where the region comes from.
    pub source: &'static str,
}

/// SPU RAM occupancy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpuReport {
    /// The map, in address order.
    pub regions: Vec<SpuRegion>,
    /// Summary rows.
    pub rows: Vec<Row>,
}

/// The SPU RAM map for a set of cooked samples, and (with a dump of the SPU
/// RAM as little-endian halfwords) how much of the bank area holds data.
pub fn spu_report(banks: &[SpuBank], dump: Option<&[u8]>) -> SpuReport {
    let uploaded = &banks[..banks.len().min(SPU_SFX_SAMPLE_LIMIT)];
    let bank_bytes: u32 = uploaded.iter().map(|b| b.bytes).sum();
    let bank_end = SPU_SFX_BANK_BASE + bank_bytes;
    let mut regions = vec![
        SpuRegion {
            name: "hardware decode buffers".into(),
            start: 0,
            end: SPU_HARDWARE_BYTES,
            source: "layout-contract",
        },
        SpuRegion {
            name: "silence block".into(),
            start: SPU_HARDWARE_BYTES,
            end: SPU_SILENCE_END,
            source: "layout-contract",
        },
        SpuRegion {
            name: "unused by the engine (below the SFX bank)".into(),
            start: SPU_SILENCE_END,
            end: SPU_SFX_BANK_BASE,
            source: "layout-contract",
        },
    ];
    let mut at = SPU_SFX_BANK_BASE;
    for bank in uploaded {
        regions.push(SpuRegion {
            name: format!("sfx {}", bank.name),
            start: at,
            end: at + bank.bytes,
            source: "cook",
        });
        at += bank.bytes;
    }
    regions.push(SpuRegion {
        name: "free".into(),
        start: bank_end,
        end: SPU_REVERB_BASE.max(bank_end),
        source: "cook",
    });
    regions.push(SpuRegion {
        name: "reverb work area (parked at the top)".into(),
        start: SPU_REVERB_BASE.max(bank_end),
        end: SPU_RAM_BYTES,
        source: "layout-contract",
    });
    let mut rows = vec![
        Row::new(
            "SPU RAM total",
            i64::from(SPU_RAM_BYTES),
            "bytes",
            "layout-contract",
            "",
        ),
        Row::new(
            "SFX samples",
            uploaded.len() as i64,
            "count",
            "cook",
            &format!("engine limit {SPU_SFX_SAMPLE_LIMIT}"),
        ),
        Row::new(
            "SFX bank bytes",
            i64::from(bank_bytes),
            "bytes",
            "cook",
            &format!("from {SPU_SFX_BANK_BASE:#x} to {bank_end:#x}"),
        ),
        Row::new(
            "free above the bank",
            i64::from(SPU_REVERB_BASE.max(bank_end)) - i64::from(bank_end),
            "bytes",
            "cook",
            "up to the parked reverb work area",
        ),
        Row::new(
            "free below the bank",
            i64::from(SPU_SFX_BANK_BASE - SPU_SILENCE_END),
            "bytes",
            "layout-contract",
            "unused by this runtime",
        ),
    ];
    if banks.len() > SPU_SFX_SAMPLE_LIMIT {
        rows.push(Row::new(
            "SFX samples over the engine limit",
            (banks.len() - SPU_SFX_SAMPLE_LIMIT) as i64,
            "count",
            "cook",
            "never uploaded",
        ));
    }
    if let Some(dump) = dump.filter(|d| d.len() >= SPU_RAM_BYTES as usize) {
        let used = |start: u32, end: u32| {
            dump[start as usize..end as usize]
                .chunks_exact(2)
                .filter(|p| *p != [0, 0])
                .count() as i64
                * 2
        };
        rows.push(Row::new(
            "SPU non-zero bytes in the bank area",
            used(SPU_SFX_BANK_BASE, bank_end),
            "bytes",
            "emulator-spu",
            "ADPCM can contain zero halfwords, so this is a lower bound",
        ));
        let beyond = used(bank_end, SPU_REVERB_BASE.max(bank_end));
        rows.push(Row::new(
            "SPU non-zero bytes above the bank",
            beyond,
            "bytes",
            "emulator-spu",
            "0 when nothing else was uploaded",
        ));
        let last = dump[..SPU_RAM_BYTES as usize]
            .chunks_exact(2)
            .rposition(|p| p != [0, 0])
            .map_or(0, |halfword| (halfword as i64 + 1) * 2);
        rows.push(Row::new(
            "SPU highest non-zero byte",
            last,
            "address",
            "emulator-spu",
            "",
        ));
    }
    SpuReport { regions, rows }
}

// ------------------------------------------------------------ the cook

/// What the cook of a project says it will need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookedInputs {
    /// Envelopes and derived arena sizes.
    pub rows: Vec<Row>,
    /// Texture assets, largest first.
    pub textures: Vec<CookedTexture>,
    /// SFX samples in upload order.
    pub banks: Vec<SpuBank>,
}

/// Read the cooked package: the cook-time envelopes the editor already shows,
/// every texture asset's VRAM footprint, and the SFX samples' ADPCM bytes.
pub fn cooked_inputs(project: &ProjectDocument, package: &PlaytestPackage) -> CookedInputs {
    let budget = cooked_playtest_budgets(project, package);
    let audit = audit_resident_assets(package);
    let mut rows = vec![
        Row::new(
            "BSP lumps",
            budget.bsp_bytes as i64,
            "bytes",
            "cook",
            "resident PXBSP image, baked into .data",
        ),
        Row::new(
            "PVS",
            budget.pvs_bytes as i64,
            "bytes",
            "cook",
            &format!("widest row {} B", budget.pvs_row_bytes),
        ),
        Row::new("lights", budget.light_bytes as i64, "bytes", "cook", ""),
        Row::new(
            "texture payloads",
            budget.texture_bytes as i64,
            "bytes",
            "cook",
            "cooked .psxt bytes",
        ),
        Row::new(
            "RAM envelope",
            budget.ram_bytes as i64,
            "bytes",
            "cook",
            &format!("{} asset slots", budget.ram_asset_slots),
        ),
        Row::new(
            "VRAM envelope",
            budget.vram_bytes as i64,
            "bytes",
            "cook",
            &format!("{} asset slots", budget.vram_asset_slots),
        ),
        Row::new(
            "primitive packets",
            budget.packet_count as i64,
            "count",
            "cook",
            "conservative envelope",
        ),
        Row::new(
            "primitive packet capacity",
            budget.packet_limit as i64,
            "count",
            "cook",
            "arena size the guest is built with",
        ),
        Row::new(
            "primitive packet arena",
            (budget.packet_limit * psx_engine::PRIMITIVE_PACKET_SLOT_WORDS * 4) as i64,
            "bytes",
            "cook",
            &format!(
                "{} words a packet slot",
                psx_engine::PRIMITIVE_PACKET_SLOT_WORDS
            ),
        ),
        Row::new(
            "session-resident assets",
            audit.total_bytes as i64,
            "bytes",
            "cook",
            "persistent gameplay clips, word-padded",
        ),
        Row::new(
            "session-resident pages",
            audit.resident_pages() as i64,
            "pages",
            "cook",
            &format!(
                "of {PLAYTEST_RESIDENT_ASSET_PAGES} (2 KiB each), {}%",
                audit.percent_of_cap()
            ),
        ),
    ];
    for (owner, bytes) in audit.by_owner().into_iter().take(6) {
        rows.push(Row::new(
            &format!("resident: {owner}"),
            bytes as i64,
            "bytes",
            "cook",
            "",
        ));
    }

    let mut textures: Vec<CookedTexture> = package
        .assets
        .iter()
        .filter(|asset| asset.kind == PlaytestAssetKind::Texture)
        .filter_map(|asset| {
            let texture = psx_asset::Texture::from_bytes(&asset.bytes).ok()?;
            Some(CookedTexture {
                name: asset.source_label.clone(),
                width: texture.width(),
                height: texture.height(),
                depth: match texture.depth() {
                    psxed_format::texture::Depth::Bit4 => 4,
                    psxed_format::texture::Depth::Bit8 => 8,
                    psxed_format::texture::Depth::Bit15 => 15,
                },
                clut_entries: texture.clut_entries(),
                halfwords: (texture.pixel_bytes().len() / 2) as u32,
            })
        })
        .collect();
    textures.sort_by(|a, b| b.halfwords.cmp(&a.halfwords).then(a.name.cmp(&b.name)));

    let banks = package
        .ui_sfx_samples
        .iter()
        .map(|sample| SpuBank {
            name: sample.filename.clone(),
            bytes: psx_asset::Audio::from_bytes(&sample.bytes)
                .map(|audio| audio.adpcm_bytes().len() as u32)
                .unwrap_or(0),
        })
        .collect();
    CookedInputs {
        rows,
        textures,
        banks,
    }
}

// ----------------------------------------------------------- from files

/// The files a report is built from. Only `map` is required.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// lld `-Map` file (`PSOXIDE_GUEST_LINK_MAP`).
    pub map: Option<std::path::PathBuf>,
    /// The linked PS-X EXE.
    pub exe: Option<std::path::PathBuf>,
    /// `generated/level_manifest.rs`.
    pub manifest: Option<std::path::PathBuf>,
    /// `project.ron`, cooked in memory for the texture and SFX tables.
    pub project: Option<std::path::PathBuf>,
    /// 2 MiB RAM dump (`frontend launch --dump-ram`).
    pub ram: Option<std::path::PathBuf>,
    /// VRAM dump (`--dump-vram`, a PPM).
    pub vram: Option<std::path::PathBuf>,
    /// 512 KiB SPU RAM dump (`--dump-spu-ram`).
    pub spu: Option<std::path::PathBuf>,
    /// Report heading.
    pub label: Option<String>,
}

fn read_file(path: &std::path::Path) -> Result<Vec<u8>, OccupancyError> {
    std::fs::read(path).map_err(|e| OccupancyError(format!("{}: {e}", path.display())))
}

/// Read every input that was named and assemble the report.
pub fn build_report(inputs: &Inputs) -> Result<Report, OccupancyError> {
    let map_path = inputs
        .map
        .as_ref()
        .ok_or_else(|| OccupancyError("a link map is required".into()))?;
    let map = parse_link_map(&String::from_utf8_lossy(&read_file(map_path)?))
        .map_err(|e| OccupancyError(format!("{}: {e}", map_path.display())))?;
    let exe = match &inputs.exe {
        Some(path) => Some(
            parse_exe(&read_file(path)?)
                .map_err(|e| OccupancyError(format!("{}: {e}", path.display())))?,
        ),
        None => None,
    };
    let ram_dump = inputs.ram.as_deref().map(read_file).transpose()?;
    let ram = ram_report(&map, exe.as_ref(), ram_dump.as_deref());
    let manifest = match &inputs.manifest {
        Some(path) => manifest_rows(&String::from_utf8_lossy(&read_file(path)?)),
        None => Vec::new(),
    };
    let mut manifest = manifest;
    let derived = derived_arena_rows(&manifest);
    manifest.extend(derived);
    let (cooked, textures, banks) = match &inputs.project {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| OccupancyError(format!("{}: {e}", path.display())))?;
            let project = ProjectDocument::from_ron_str(&text)
                .map_err(|e| OccupancyError(format!("{}: parse failed: {e}", path.display())))?;
            let root = path
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(|| ".".into());
            let (package, validation) = crate::playtest::build_package(&project, &root);
            let package = package.ok_or_else(|| {
                OccupancyError(format!(
                    "{}: the cook failed: {}",
                    path.display(),
                    validation
                        .errors
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                ))
            })?;
            let cooked = cooked_inputs(&project, &package);
            (cooked.rows, cooked.textures, cooked.banks)
        }
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    let vram = match &inputs.vram {
        Some(path) => Some(
            vram_from_dump(&read_file(path)?)
                .map_err(|e| OccupancyError(format!("{}: {e}", path.display())))?,
        ),
        None => None,
    };
    let spu_dump = inputs.spu.as_deref().map(read_file).transpose()?;
    let spu = (inputs.project.is_some() || spu_dump.is_some())
        .then(|| spu_report(&banks, spu_dump.as_deref()));
    Ok(Report {
        label: inputs
            .label
            .clone()
            .unwrap_or_else(|| map_path.display().to_string()),
        ram: Some(ram),
        manifest,
        cooked,
        vram,
        cooked_vram: cooked_vram_rows(&textures),
        textures,
        spu,
    })
}

// ------------------------------------------------------------- the report

/// The whole report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// What was measured (project, commit, build).
    pub label: String,
    /// Main RAM.
    pub ram: Option<RamReport>,
    /// Capacities stated by the cooked manifest.
    pub manifest: Vec<Row>,
    /// Cook-time envelopes and derived arena sizes.
    pub cooked: Vec<Row>,
    /// VRAM, from a dump.
    pub vram: Option<VramReport>,
    /// Cooked VRAM demand.
    pub cooked_vram: Vec<Row>,
    /// Cooked textures, largest first.
    pub textures: Vec<CookedTexture>,
    /// SPU RAM.
    pub spu: Option<SpuReport>,
}

fn table(out: &mut String, rows: &[Row]) {
    let width = rows.iter().map(|r| r.name.len()).max().unwrap_or(0);
    for row in rows {
        let value = if row.unit == "address" {
            format!("{:#x}", row.value)
        } else {
            row.value.to_string()
        };
        let note = if row.note.is_empty() {
            String::new()
        } else {
            format!("  {}", row.note)
        };
        let _ = writeln!(
            out,
            "  {:<width$}  {:>10} {:<7} [{}]{}",
            row.name, value, row.unit, row.source, note
        );
    }
}

/// Render the report as text.
pub fn render_text(report: &Report) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Occupancy report: {}", report.label);
    if let Some(ram) = &report.ram {
        let _ = writeln!(out, "\nMain RAM (2 MiB)");
        table(&mut out, &ram.rows);
        let _ = writeln!(
            out,
            "\n  Statics of {ARENA_MIN_BYTES} bytes or more, largest first"
        );
        for arena in &ram.arenas {
            let _ = writeln!(
                out,
                "  {:>9}  {:#010x}  {:<6} {}",
                arena.bytes, arena.address, arena.section, arena.name
            );
        }
    }
    if !report.manifest.is_empty() {
        let _ = writeln!(out, "\nCooked manifest capacities");
        table(&mut out, &report.manifest);
    }
    if !report.cooked.is_empty() {
        let _ = writeln!(out, "\nCook envelopes and derived arena sizes");
        table(&mut out, &report.cooked);
    }
    if !report.cooked_vram.is_empty() {
        let _ = writeln!(out, "\nVRAM demand of the cooked textures");
        table(&mut out, &report.cooked_vram);
        let _ = writeln!(out, "\n  Largest cooked textures");
        for texture in report.textures.iter().take(12) {
            let _ = writeln!(
                out,
                "  {:>8} B  {:>4}x{:<4} {:>2}bpp  {}",
                u64::from(texture.halfwords) * 2,
                texture.width,
                texture.height,
                texture.depth,
                texture.name
            );
        }
    }
    if let Some(vram) = &report.vram {
        let _ = writeln!(out, "\nVRAM (1 MiB), from a dump");
        table(&mut out, &vram.rows);
        let _ = writeln!(
            out,
            "\n  Pages of 64x256 halfwords (non-blank / 16384), row 0 then row 1"
        );
        for page in &vram.pages {
            let used = page.used_halfwords.unwrap_or(0);
            let _ = writeln!(
                out,
                "  page {:>2},{} x={:>4}  {:>5}/16384 {:>3}%  {}",
                page.column,
                page.row,
                page.column * PAGE_WIDTH,
                used,
                used * 100 / (PAGE_WIDTH * PAGE_HEIGHT),
                page.claimed_by.join("; ")
            );
        }
        if vram.clut_rows.is_empty() {
            let _ = writeln!(
                out,
                "\n  Lines 480 to 511 (the contract's CLUT band): no data"
            );
        } else {
            let _ = writeln!(
                out,
                "\n  Lines 480 to 511 with data (the contract's CLUT band; large textures can reach these lines too, so a row is not proof of CLUTs)"
            );
            for row in &vram.clut_rows {
                let extent = row
                    .extent
                    .map_or(String::new(), |(a, b)| format!("x {a} to {b}"));
                let _ = writeln!(
                    out,
                    "  y={:<4} {:>5} halfwords  {extent}",
                    row.y, row.used_halfwords
                );
            }
        }
    }
    if let Some(spu) = &report.spu {
        let _ = writeln!(out, "\nSPU RAM (512 KiB)");
        table(&mut out, &spu.rows);
        let _ = writeln!(out, "\n  Map");
        for region in &spu.regions {
            let _ = writeln!(
                out,
                "  {:#08x} to {:#08x}  {:>7} B  [{}] {}",
                region.start,
                region.end,
                region.end - region.start,
                region.source,
                region.name
            );
        }
    }
    out
}

/// Render the report as JSON.
pub fn render_json(report: &Report) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "\
     VMA      LMA     Size Align Out     In      Symbol
       0        0        0     1 RAM_BASE = 0x80000000
       0        0        0     1 RAM_SIZE = 2M
       0        0        0     1 BIOS_SIZE = 64K
       0        0        0     1 HEADER_SIZE = 2K
       0        0        0     1 LOAD_ADDR = RAM_BASE + BIOS_SIZE
       0        0        0     1 STACK_INIT = RAM_BASE + 0x001FFF00
       0        0        0     1 STACK_RESERVE = 0x8000
8000f800 8000f800      800     1 .psx_exe_header
80010000 80010000     1000     4 .text
80010000 80010000        0     1         __text_start = .
80010000 80010000       7c     4         /x/a.o:(.text._start)
80010000 80010000       7c     1                 _start
80011000 80011000      800     4 .data
80011000 80011000      800     4         /x/a.o:(.data.TABLE)
80011000 80011000      800     1                 editor_playtest::generated::PXBSP_WORLD
80011800 80011800     2000     8 .bss
80011800 80011800        0     1         __bss_start = .
80011800 80011800     1800     8         /x/a.o:(.bss.ARENAS)
80011800 80011800     1800     1                 editor_playtest::runtime_arenas::RUNTIME_ARENAS
80013000 80013000        4     4         /x/a.o:(.bss.heap)
80013000 80013000        4     1                 psx_rt::heap::ALLOCATOR (.0)
80013004 80013004        4     4         /x/a.o:(.bss.heap)
80013004 80013004        1     1                 psx_rt::heap::ALLOCATOR (.1)
80013800 80013800        0     1         __bss_end = .
80013800 80013800        0     1     __heap_start = .
801f7f00 801f7f00        0     1     __heap_end = STACK_INIT - STACK_RESERVE
";

    #[test]
    fn the_link_map_gives_sections_symbols_and_scalars() {
        let map = parse_link_map(MAP).unwrap();
        let names: Vec<&str> = map.sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, [".psx_exe_header", ".text", ".data", ".bss"]);
        assert_eq!(map.def("LOAD_ADDR"), Some(0x8001_0000));
        assert_eq!(map.def("STACK_INIT"), Some(0x801F_FF00));
        assert_eq!(map.def("__bss_end"), Some(0x8001_3800));
        assert_eq!(map.def("__heap_end"), Some(0x801F_7F00));
        let arena = map
            .symbols
            .iter()
            .find(|s| s.name.ends_with("RUNTIME_ARENAS"))
            .unwrap();
        assert_eq!((arena.size, arena.section.as_str()), (0x1800, ".bss"));
        assert_eq!(map.allocator_words(), Some((0x8001_3000, 0x8001_3004)));
    }

    #[test]
    fn the_static_headroom_is_exact() {
        let map = parse_link_map(MAP).unwrap();
        let ram = ram_report(&map, None, None);
        let row = |name: &str| ram.rows.iter().find(|r| r.name == name).unwrap().value;
        assert_eq!(row(".text"), 0x1000);
        assert_eq!(row(".bss"), 0x2000);
        assert_eq!(row("stack reserve"), 0x8000);
        // The region ends at RAM_BASE + RAM_SIZE - STACK_RESERVE.
        assert_eq!(row("static headroom"), 0x801F_8000i64 - 0x8001_3800);
        assert_eq!(row("static image"), 0x8001_3800i64 - 0x8001_0000);
        assert_eq!(row("heap capacity"), 0x801F_7F00i64 - 0x8001_3800);
        assert_eq!(ram.arenas.len(), 2, "the 2 KiB table and the arenas");
        assert_eq!(ram.arenas[0].bytes, 0x1800);
    }

    #[test]
    fn the_heap_is_read_from_the_dump_at_the_allocator_symbol() {
        let map = parse_link_map(MAP).unwrap();
        let mut dump = vec![0u8; RAM_BYTES as usize];
        let at = |address: u32| (address & (RAM_BYTES - 1)) as usize;
        dump[at(0x8001_3000)..][..4].copy_from_slice(&0x8001_5000u32.to_le_bytes());
        dump[at(0x8001_3004)..][..4].copy_from_slice(&0x801F_7F00u32.to_le_bytes());
        let ram = ram_report(&map, None, Some(&dump));
        let row = |name: &str| ram.rows.iter().find(|r| r.name == name).unwrap();
        assert_eq!(row("heap used").value, 0x8001_5000i64 - 0x8001_3800);
        assert_eq!(row("heap free").value, 0x801F_7F00i64 - 0x8001_5000);
        assert_eq!(row("heap free").source, "emulator-ram");
    }

    #[test]
    fn a_map_without_the_allocator_has_no_heap() {
        let text = MAP.replace("psx_rt::heap::ALLOCATOR", "other::THING");
        let map = parse_link_map(&text).unwrap();
        let ram = ram_report(&map, None, Some(&vec![0u8; RAM_BYTES as usize]));
        let heap = ram.rows.iter().find(|r| r.name == "heap free").unwrap();
        assert_eq!(heap.value, -1);
    }

    #[test]
    fn a_text_file_that_is_not_a_map_is_refused() {
        assert!(parse_link_map("hello\nworld\n").is_err());
    }

    #[test]
    fn the_exe_header_is_read() {
        let mut bytes = vec![0u8; 0x800 + 8];
        bytes[..8].copy_from_slice(b"PS-X EXE");
        bytes[0x10..0x14].copy_from_slice(&0x8001_0000u32.to_le_bytes());
        bytes[0x18..0x1C].copy_from_slice(&0x8001_0000u32.to_le_bytes());
        bytes[0x1C..0x20].copy_from_slice(&0x1800u32.to_le_bytes());
        bytes[0x30..0x34].copy_from_slice(&0x801F_FF00u32.to_le_bytes());
        let header = parse_exe(&bytes).unwrap();
        assert_eq!(header.payload_bytes, 0x1800);
        assert_eq!(header.stack_pointer, 0x801F_FF00);
        assert!(parse_exe(&bytes[..16]).is_err());
        let map = parse_link_map(MAP).unwrap();
        let ram = ram_report(&map, Some(&header), None);
        let diff = ram
            .rows
            .iter()
            .find(|r| r.name == "exe payload minus text+data")
            .unwrap();
        assert_eq!(diff.value, 0x1800 - (0x1000 + 0x800));
    }

    #[test]
    fn manifest_constants_become_rows() {
        let text = "\
pub const PLAYTEST_PACKET_CAPACITY: usize = 1536;
pub const WORLD_PACK_START_LBA: u32 = 1_024;
pub const PLAYTEST_USES_PXBSP: bool = true;
pub const PXBSP_FACE_CHAIN_CAPACITY: usize = 0x20;
pub static ASSETS: &[LevelAssetRecord] = &[];
";
        let rows = manifest_rows(text);
        let pairs: Vec<(&str, i64)> = rows.iter().map(|r| (r.name.as_str(), r.value)).collect();
        assert_eq!(
            pairs,
            [
                ("PLAYTEST_PACKET_CAPACITY", 1536),
                ("WORLD_PACK_START_LBA", 1024),
                ("PXBSP_FACE_CHAIN_CAPACITY", 32)
            ]
        );
    }

    #[test]
    fn page_counts_and_the_face_chain_become_byte_rows() {
        let manifest = manifest_rows(
            "pub const PERSISTENT_ASSET_PAGE_COUNT: usize = 314;\n\
             pub const PXBSP_FACE_CHAIN_CAPACITY: usize = 678;\n\
             pub const PLAYTEST_PACKET_CAPACITY: usize = 1536;",
        );
        let derived = derived_arena_rows(&manifest);
        let pairs: Vec<(&str, i64)> = derived.iter().map(|r| (r.name.as_str(), r.value)).collect();
        assert_eq!(
            pairs,
            [
                ("PERSISTENT_ASSET_PAGE_BYTES", 314 * 2048),
                ("PXBSP_FACE_CHAIN_BYTES", 678 * 2)
            ]
        );
    }

    fn ppm(mark: impl Fn(usize, usize) -> bool) -> Vec<u8> {
        let mut out = b"P6\n1024 512\n255\n".to_vec();
        for y in 0..VRAM_HEIGHT {
            for x in 0..VRAM_WIDTH {
                out.extend_from_slice(if mark(x, y) {
                    &[200, 100, 50]
                } else {
                    &[0, 0, 0]
                });
            }
        }
        out
    }

    #[test]
    fn vram_pages_count_their_non_blank_halfwords() {
        // Fill page (11, 0) completely, and half of page (6, 1).
        let dump = ppm(|x, y| {
            (704..768).contains(&x) && y < 256 || (384..416).contains(&x) && (256..512).contains(&y)
        });
        let report = vram_from_dump(&dump).unwrap();
        assert_eq!(report.pages.len(), 32);
        let page = |column: usize, row: usize| {
            report
                .pages
                .iter()
                .find(|p| (p.column, p.row) == (column, row))
                .unwrap()
        };
        assert_eq!(page(11, 0).used_halfwords, Some(64 * 256));
        assert_eq!(page(6, 1).used_halfwords, Some(32 * 256));
        assert_eq!(page(0, 0).used_halfwords, Some(0));
        assert!(page(11, 0).claimed_by[0].starts_with("room material"));
        assert!(page(6, 1).claimed_by[0].starts_with("model atlas"));
        assert!(page(0, 0).claimed_by[0].starts_with("framebuffers"));
        assert!(page(5, 0).claimed_by[0].starts_with("unified allocator"));
        // The CLUT band overlaps the second row of pages too.
        assert!(page(2, 1)
            .claimed_by
            .iter()
            .any(|c| c.starts_with("CLUT band")));
    }

    #[test]
    fn clut_rows_report_their_extent() {
        let dump = ppm(|x, y| y == 490 && (16..48).contains(&x));
        let report = vram_from_dump(&dump).unwrap();
        assert_eq!(report.clut_rows.len(), 1);
        assert_eq!(report.clut_rows[0].y, 490);
        assert_eq!(report.clut_rows[0].extent, Some((16, 47)));
        assert_eq!(report.clut_rows[0].used_halfwords, 32);
    }

    #[test]
    fn a_raw_halfword_dump_reads_the_same() {
        let mut raw = vec![0u8; VRAM_WIDTH * VRAM_HEIGHT * 2];
        let at = (300 * VRAM_WIDTH + 700) * 2;
        raw[at] = 1;
        let report = vram_from_dump(&raw).unwrap();
        let page = report
            .pages
            .iter()
            .find(|p| (p.column, p.row) == (10, 1))
            .unwrap();
        assert_eq!(page.used_halfwords, Some(1));
        assert!(vram_from_dump(&raw[..100]).is_err());
        assert!(vram_from_dump(b"P6\n10 10\n255\n").is_err());
    }

    #[test]
    fn the_spu_map_tiles_ram_without_gaps_or_overlap() {
        let banks = vec![
            SpuBank {
                name: "a".into(),
                bytes: 0x2000,
            },
            SpuBank {
                name: "b".into(),
                bytes: 0x800,
            },
        ];
        let report = spu_report(&banks, None);
        let mut at = 0;
        for region in &report.regions {
            assert_eq!(region.start, at, "{}", region.name);
            assert!(region.end >= region.start);
            at = region.end;
        }
        assert_eq!(at, SPU_RAM_BYTES);
        let row = |name: &str| report.rows.iter().find(|r| r.name == name).unwrap().value;
        assert_eq!(row("SFX bank bytes"), 0x2800);
        assert_eq!(
            row("free above the bank"),
            i64::from(SPU_REVERB_BASE) - 0x30000 - 0x2800
        );
    }

    #[test]
    fn the_spu_dump_confirms_what_was_uploaded() {
        let banks = vec![SpuBank {
            name: "a".into(),
            bytes: 64,
        }];
        let mut dump = vec![0u8; SPU_RAM_BYTES as usize];
        for byte in &mut dump[0x30000..0x30000 + 64] {
            *byte = 0x11;
        }
        let report = spu_report(&banks, Some(&dump));
        let row = |name: &str| report.rows.iter().find(|r| r.name == name).unwrap().value;
        assert_eq!(row("SPU non-zero bytes in the bank area"), 64);
        assert_eq!(row("SPU non-zero bytes above the bank"), 0);
        assert_eq!(row("SPU highest non-zero byte"), 0x30040);
    }

    #[test]
    fn the_report_renders_as_text_and_json() {
        let map = parse_link_map(MAP).unwrap();
        let report = Report {
            label: "test".into(),
            ram: Some(ram_report(&map, None, None)),
            manifest: manifest_rows("pub const A_COUNT: usize = 3;"),
            cooked: Vec::new(),
            vram: None,
            cooked_vram: Vec::new(),
            textures: Vec::new(),
            spu: Some(spu_report(&[], None)),
        };
        let text = render_text(&report);
        assert!(text.contains("static headroom"));
        assert!(text.contains("[link-map]"));
        assert!(text.contains("RUNTIME_ARENAS"));
        let json = render_json(&report);
        assert!(json.contains("\"static headroom\""));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["label"], "test");
    }
}
