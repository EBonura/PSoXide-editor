//! `showcase-vector`: the cooked-SVG runtime at native resolution, and its
//! cycle meter.
//!
//! One frame shows the test gallery (every cooker feature), a rotated and
//! scaled group, a clipped group, a faded group, and the Cortex stance HUD in
//! two states. Every draw is timed with root counter 2 (system clock / 8) and
//! the figures are written to [`MEASURE`], a RAM block tagged with a magic
//! header that a headless run finds in `--dump-ram`.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate psx_rt;

mod hud;

use hud::{draw_hud, Bar, HudIds, SCRATCH_VERTICES};
use psx_engine::{App, Config, Ctx, OtFrame, Scene};
use psx_gpu::ot::OrderingTable;
use psx_gpu::material::BlendMode;
use psx_io::timers::{self, Timer};
use psx_vector::{
    include_vector, BlendOverride, ClipRect, Emit, Placement, Scratch, Style, VectorAsset,
    VectorPacket,
};

static GALLERY: &[u8] = include_vector!("../assets/gallery.psvg");
static STANCE_HUD: &[u8] = include_vector!("../../editor-playtest/assets/hud/stance_hud.psvg");

static mut SCRATCH: Scratch<SCRATCH_VERTICES> = Scratch::new();

/// Ordering table and packet pool for the packet path (HUD B).
static mut OT: OrderingTable<128> = OrderingTable::new();
static mut POOL: [VectorPacket; 700] = [VectorPacket::ZERO; 700];

const HORIZON: [u8; 3] = [234, 82, 48];
const ZENITH: [u8; 3] = [76, 202, 181];

/// One timed draw: an 8-byte tag, the triangles emitted, and the cycles.
#[repr(C)]
#[derive(Clone, Copy)]
struct Row {
    tag: [u8; 8],
    triangles: u32,
    cycles: u32,
}

/// The result block. `magic` is `PSVGMEAS` followed by the row count.
#[repr(C)]
struct Measure {
    magic: [u8; 8],
    rows_used: u32,
    frames: u32,
    rows: [Row; 48],
}

static mut MEASURE: Measure = Measure {
    magic: *b"PSVGMEAS",
    rows_used: 0,
    frames: 0,
    rows: [Row { tag: [0; 8], triangles: 0, cycles: 0 }; 48],
};

/// Where each group's packets sit in [`POOL`], recorded when they are built.
struct Plan {
    groups: usize,
    spans: [(u16, u16); 64],
}

static mut PLAN: Plan = Plan { groups: 0, spans: [(0, 0); 64] };

/// Cycles of a timer read pair, subtracted from every row.
static mut OVERHEAD: u32 = 0;

struct Demo {
    ids: Option<HudIds>,
    gallery: Option<VectorAsset<'static>>,
    hud: Option<VectorAsset<'static>>,
}

#[inline(always)]
fn ticks() -> u16 {
    timers::counter(Timer::Timer2)
}

/// Record `cycles` for `tag` in the next row (or update the row of that tag).
fn record(tag: &str, triangles: u32, ticks8: u16) {
    // SAFETY: single-threaded guest; MEASURE is only touched here and in init.
    let m = unsafe { &mut MEASURE };
    let mut t = [0u8; 8];
    for (d, s) in t.iter_mut().zip(tag.bytes()) {
        *d = s;
    }
    let cycles = (ticks8 as u32 * 8).saturating_sub(unsafe { OVERHEAD });
    let used = m.rows_used as usize;
    let slot = m.rows[..used].iter().position(|r| r.tag == t).unwrap_or(used);
    if slot < m.rows.len() {
        m.rows[slot] = Row { tag: t, triangles, cycles };
        if slot == used {
            m.rows_used += 1;
        }
    }
}

impl Scene for Demo {
    fn init(&mut self, _ctx: &mut Ctx) {
        // Timer 2 on system clock / 8, free running.
        timers::set_mode(Timer::Timer2, 0x0200);
        let (a, b) = (ticks(), ticks());
        // SAFETY: single-threaded guest.
        unsafe { OVERHEAD = (b.wrapping_sub(a) as u32 * 8).min(64) };
        self.gallery = VectorAsset::parse(GALLERY).ok();
        self.hud = VectorAsset::parse(STANCE_HUD).ok();
        self.ids = self.hud.as_ref().and_then(HudIds::new);
    }

    fn update(&mut self, _ctx: &mut Ctx) {}

    fn render(&mut self, ctx: &mut Ctx) {
        // SAFETY: single-threaded guest; the table and pool are only used here,
        // and `submit` waits for the walk before anything is rebuilt.
        let (ot_table, pool, scratch, plan) =
            unsafe { (&mut OT, &mut POOL[..], &mut SCRATCH, &mut PLAN) };
        let mut ot = OtFrame::begin(ot_table);
        if let (Some(ids), Some(asset)) = (self.ids.as_ref(), self.hud.as_ref()) {
            // Build the packets once. The HUD state is fixed here, so every
            // later frame only links them: the "built once, relinked" case.
            if plan.groups == 0 {
                let t = ticks();
                let mut rest = &mut pool[..];
                let mut triangles = 0u32;
                let (active, inactive) =
                    (Bar { rgb: ZENITH, cells: 9 }, Bar { rgb: HORIZON, cells: 6 });
                let mut used_total = 0usize;
                draw_hud(ids, (6, 160), active, inactive, 4, &mut |g, p, s| {
                    let n = asset.emit_packets(g, p, s, scratch, rest).unwrap_or(0);
                    let (_, tail) = core::mem::take(&mut rest).split_at_mut(n);
                    rest = tail;
                    if plan.groups < plan.spans.len() {
                        plan.spans[plan.groups] = (used_total as u16, n as u16);
                        plan.groups += 1;
                    }
                    used_total += n;
                    triangles += n as u32;
                });
                record("ot_build", triangles, ticks().wrapping_sub(t));
            }
            // Link back to front: one slot per group (the first drawn deepest)
            // and, inside a slot, last packet first, because a slot is walked
            // last-in-first-out.
            let base = pool.as_mut_ptr();
            let t = ticks();
            let mut triangles = 0u32;
            for (seq, &(start, len)) in plan.spans[..plan.groups].iter().enumerate() {
                let slot = 127usize.saturating_sub(seq);
                // SAFETY: the span was handed out by `emit_packets` above, lies inside
                // `POOL`, and no other reference to it exists; the packets stay
                // untouched until the walk ends.
                let span = unsafe {
                    core::slice::from_raw_parts_mut(base.add(start as usize), len as usize)
                };
                for packet in span.iter_mut().rev() {
                    ot.add_packet(slot, packet);
                }
                triangles += len as u32;
            }
            record("ot_link", triangles, ticks().wrapping_sub(t));
        }
        let t = ticks();
        ot.submit(ctx.gpu_dma());
        record("ot_walk", 0, ticks().wrapping_sub(t));
    }

    fn render_overlay(&mut self, ctx: &mut Ctx) {
        let gpu = ctx.gpu();
        // SAFETY: single-threaded guest; SCRATCH is only used from here.
        let scratch = unsafe { &mut SCRATCH };
        let (Some(gallery), Some(hud_asset)) = (self.gallery.as_ref(), self.hud.as_ref()) else {
            return;
        };

        // Baseline: two timer reads and nothing between.
        let t0 = ticks();
        record("empty", 0, ticks().wrapping_sub(t0));

        // The gallery, group by group at the authored position, offset (8, 6).
        let place = Placement::at(8, 6);
        let t_all = ticks();
        let mut total = 0u32;
        for id in gallery.groups() {
            let t = ticks();
            let n = gallery
                .draw_group(gpu, id, &place, &Style::DEFAULT, scratch)
                .unwrap_or(0);
            let dt = ticks().wrapping_sub(t);
            total += n;
            record(gallery.group_name(id), n, dt);
        }
        record("gallery", total, ticks().wrapping_sub(t_all));

        // Rotation and scale: the star at three poses on the right.
        if let Some(star) = gallery.group("star") {
            let t = ticks();
            let mut n = 0;
            for (i, (scale, angle)) in [(4096u16, 0u16), (3072, 256), (2048, 1024)].iter().enumerate() {
                let p = Placement::at(180 + 44 * i as i32, 6).scaled(*scale).rotated(*angle);
                n += gallery.draw_group(gpu, star, &p, &Style::DEFAULT, scratch).unwrap_or(0);
            }
            record("star3", n, ticks().wrapping_sub(t));
        }

        // Clip: the blob through a window, tinted.
        if let Some(blob) = gallery.group("blob") {
            let clip = ClipRect { x0: 188, y0: 70, x1: 262, y1: 100 };
            let style = Style::tinted([255, 220, 160]).clipped(clip);
            let t = ticks();
            let n = gallery
                .draw_group(gpu, blob, &Placement::at(100, -4), &style, scratch)
                .unwrap_or(0);
            record("clip", n, ticks().wrapping_sub(t));
        }

        // Fade: the rects forced to average blend over the gradient.
        if let (Some(rects), Some(grad)) = (gallery.group("rects"), gallery.group("gradient")) {
            let p = Placement::at(268, 70);
            let _ = gallery.draw_group(gpu, grad, &p, &Style::DEFAULT, scratch);
            let fade = Style { blend: BlendOverride::Force(BlendMode::Average), ..Style::DEFAULT };
            let t = ticks();
            let n = gallery
                .draw_group(gpu, rects, &Placement::at(268, 66), &fade, scratch)
                .unwrap_or(0);
            record("fade", n, ticks().wrapping_sub(t));
        }

        // The HUD: A and the worst case straight to the GPU, B as packets for
        // the ordering table (built here, walked by `render`), and A once more
        // with a sink that discards, to split CPU cost from GPU time.
        if let Some(ids) = self.ids.as_ref() {
            let (horizon, zenith) = (Bar { rgb: HORIZON, cells: 6 }, Bar { rgb: ZENITH, cells: 9 });
            let t = ticks();
            let mut n = 0;
            draw_hud(ids, (6, 122), horizon, zenith, 6, &mut |g, p, s| {
                n += hud_asset.draw_group(gpu, g, p, s, scratch).unwrap_or(0);
            });
            record("hud_a", n, ticks().wrapping_sub(t));

            let t = ticks();
            let mut n = 0;
            let full = (Bar { rgb: HORIZON, cells: 10 }, Bar { rgb: ZENITH, cells: 10 });
            draw_hud(ids, (6, 198), full.0, full.1, 10, &mut |g, p, s| {
                n += hud_asset.draw_group(gpu, g, p, s, scratch).unwrap_or(0);
            });
            record("hud_max", n, ticks().wrapping_sub(t));

            let t = ticks();
            let mut n = 0u32;
            draw_hud(ids, (6, 122), horizon, zenith, 6, &mut |g, p, s| {
                let mut count = 0u32;
                let _ = hud_asset.emit_group(g, p, s, scratch, &mut |e| {
                    count += matches!(e, Emit::Triangle(_)) as u32;
                });
                n += count;
            });
            record("cpu_a", n, ticks().wrapping_sub(t));
        }
        // CPU-only microbenchmarks on the blob (36 triangles, 38 vertices): the
        // emit loop with a counting sink, varying one input at a time.
        if let Some(blob) = gallery.group("blob") {
            let around = ClipRect { x0: -1000, y0: -1000, x1: 1000, y1: 1000 };
            let part = ClipRect { x0: 100, y0: 70, x1: 130, y1: 100 };
            let rot = Placement::at(8, 6).rotated(300);
            let scl = Placement::at(8, 6).scaled(3000);
            let base = Placement::at(8, 6);
            let tint = Style::tinted([200, 160, 120]);
            let sat = Style { saturation: 100, ..Style::DEFAULT };
            let both = Style { saturation: 100, tint: [200, 160, 120], ..Style::DEFAULT };
            let cases: [(&str, &Placement, Style); 9] = [
                ("m_plain", &base, Style::DEFAULT),
                ("m_scale", &scl, Style::DEFAULT),
                ("m_rot", &rot, Style::DEFAULT),
                ("m_tint", &base, tint),
                ("m_sat", &base, sat),
                ("m_both", &base, both),
                ("m_clip0", &base, Style::DEFAULT.clipped(around)),
                ("m_clip1", &base, Style::DEFAULT.clipped(part)),
                ("m_cliptn", &base, tint.clipped(part)),
            ];
            for (tag, place, style) in cases {
                let t = ticks();
                let mut count = 0u32;
                let _ = gallery.emit_group(blob, place, &style, scratch, &mut |e| {
                    count += matches!(e, Emit::Triangle(_)) as u32;
                });
                record(tag, count, ticks().wrapping_sub(t));
            }
        }
        // SAFETY: single-threaded guest.
        unsafe { MEASURE.frames += 1 };
    }
}

#[no_mangle]
fn main() -> ! {
    let mut demo = Demo { ids: None, gallery: None, hud: None };
    let config = Config { clear_color: (16, 24, 40), ..Config::default() };
    App::run(config, &mut demo);
}
