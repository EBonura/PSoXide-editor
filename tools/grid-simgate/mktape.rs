//! mktape OUT SEGMENT... : a poll-bound PXITAPE2 input tape (one sample per
//! controller poll, so every build sees the same input on the same simulation
//! tick whatever its frame rate).
//!
//! The tape opens with the standard CROSS taps that clear the loading and title
//! screens (polls 200, 400, 600, 800 for 20 polls each, 900 for 240), then each
//! SEGMENT writes a stick or button hold:
//!
//!   L:FROM:TO:X:Y     left stick X,Y (0..255, 128 centred, Y 0 = forward) for polls FROM..TO
//!   R:FROM:TO:X:Y     right stick (camera)
//!   B:FROM:TO:MASK    pad button mask (hex, e.g. 0x4000 is CROSS)
//!
//! `mktape out.pxtape L:1200:1900:128:0 R:1950:2250:255:128` is the standard
//! Cathedral-style route: walk forward, then turn the camera.
use std::fs;

const CROSS: u16 = 1 << 14;
/// Polls the tape holds unless `MKTAPE_POLLS` says otherwise (a route through a
/// long level needs more than the 3000 the Cathedral-style routes use).
const DEFAULT_TOTAL: usize = 3000;

fn num(text: &str) -> u16 {
    match text.strip_prefix("0x") {
        Some(hex) => u16::from_str_radix(hex, 16).expect("hex mask"),
        None => text.parse().expect("number"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).expect("usage: mktape OUT SEGMENT...");
    let total: usize = std::env::var("MKTAPE_POLLS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TOTAL);
    // (buttons, right x, right y, left x, left y)
    let mut s = vec![(0u16, 128u8, 128u8, 128u8, 128u8); total];
    for (start, hold) in [(200, 20), (400, 20), (600, 20), (800, 20), (900, 240)] {
        for p in start..start + hold {
            s[p].0 |= CROSS;
        }
    }
    for seg in &args[2..] {
        let f: Vec<&str> = seg.split(':').collect();
        let (from, to) = (num(f[1]) as usize, num(f[2]) as usize);
        let to = to.min(total);
        for p in from..to {
            match f[0] {
                "L" => (s[p].3, s[p].4) = (num(f[3]) as u8, num(f[4]) as u8),
                "R" => (s[p].1, s[p].2) = (num(f[3]) as u8, num(f[4]) as u8),
                "B" => s[p].0 |= num(f[3]),
                other => panic!("unknown segment kind {other}"),
            }
        }
    }
    let mut b = Vec::new();
    b.extend_from_slice(b"PXITAPE2");
    b.extend_from_slice(&(total as u32).to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    for (btn, rx, ry, lx, ly) in s {
        b.extend_from_slice(&btn.to_le_bytes());
        b.extend_from_slice(&[rx, ry, lx, ly]);
    }
    fs::write(out, b).unwrap();
}
