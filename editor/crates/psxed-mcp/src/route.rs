//! A pad route for a headless playtest, from a simple path description.
//!
//! The route becomes a poll-bound input tape (`clock=pad_poll`, one sample per
//! completed port-1 poll). Poll-bound means the same route plays the same on
//! any build and any emulator speed: sample N is what the guest reads at its
//! Nth poll, and the guest polls once per simulation tick (60 a second).
//!
//! The tape starts at poll 0, at boot, so it opens with a warm-up while the
//! level loads (an occasional CROSS tap clears any screen waiting for one),
//! and the legs begin at the poll `warmup_polls`.

use schemars::JsonSchema;
use serde::Deserialize;

/// Pad button bits, as `emulator_core::pad::button` defines them (a set bit is
/// a pressed button).
fn button_bit(name: &str) -> Option<u16> {
    Some(match name.to_ascii_lowercase().as_str() {
        "select" => 1 << 0,
        "l3" => 1 << 1,
        "r3" => 1 << 2,
        "start" => 1 << 3,
        "up" => 1 << 4,
        "right" => 1 << 5,
        "down" => 1 << 6,
        "left" => 1 << 7,
        "l2" => 1 << 8,
        "r2" => 1 << 9,
        "l1" => 1 << 10,
        "r1" => 1 << 11,
        "triangle" => 1 << 12,
        "circle" | "o" => 1 << 13,
        "cross" | "x" => 1 << 14,
        "square" => 1 << 15,
        _ => return None,
    })
}

/// One stretch of the route.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct Leg {
    /// How long the leg lasts, in polls (simulation ticks, 60 a second).
    pub polls: u32,
    /// Left stick `[x, y]`, each -1 to 1: x positive is right, y positive is
    /// FORWARD. The game reads it relative to its camera.
    pub stick: Option<[f32; 2]>,
    /// Shorthand for `stick` at full tilt: `forward`, `back`, `left`,
    /// `right`, `forward_left`, `forward_right`, `back_left`, `back_right`.
    pub dir: Option<String>,
    /// Right stick `[x, y]` in the same convention (turns the camera).
    pub camera: Option<[f32; 2]>,
    /// Buttons held for the whole leg: cross, circle, square, triangle,
    /// l1, r1, l2, r2, start, select, up, down, left, right.
    pub buttons: Option<Vec<String>>,
}

fn stick_bytes(stick: [f32; 2]) -> (u8, u8) {
    let x = (128.0 + stick[0].clamp(-1.0, 1.0) * 127.0).round() as u8;
    // The pad reports 0 for "up".
    let y = (128.0 - stick[1].clamp(-1.0, 1.0) * 127.0).round() as u8;
    (x, y)
}

fn dir_stick(name: &str) -> Result<[f32; 2], String> {
    let d = std::f32::consts::FRAC_1_SQRT_2;
    Ok(match name.to_ascii_lowercase().as_str() {
        "forward" => [0.0, 1.0],
        "back" | "backward" => [0.0, -1.0],
        "left" => [-1.0, 0.0],
        "right" => [1.0, 0.0],
        "forward_left" => [-d, d],
        "forward_right" => [d, d],
        "back_left" => [-d, -d],
        "back_right" => [d, -d],
        other => {
            return Err(format!(
                "unknown dir {other:?}; use forward, back, left, right, forward_left, forward_right, back_left or back_right"
            ))
        }
    })
}

/// Longest route one call builds, polls (about 17 minutes).
pub const MAX_POLLS: u32 = 60_000;
/// Default warm-up while the level loads, polls. A small grid level reaches
/// gameplay after about 400 polls.
pub const DEFAULT_WARMUP: u32 = 450;

/// A built route.
#[derive(Debug)]
pub struct Route {
    /// The tape as CSV text.
    pub csv: String,
    /// Total polls the tape covers.
    pub polls: u32,
    /// Poll at which the first leg starts.
    pub start: u32,
}

/// Build the tape: a warm-up of boot taps, then the legs.
pub fn build(warmup: u32, legs: &[Leg]) -> Result<Route, String> {
    let total: u64 = u64::from(warmup) + legs.iter().map(|l| u64::from(l.polls)).sum::<u64>();
    if total == 0 {
        return Err("the route is empty".to_string());
    }
    if total > u64::from(MAX_POLLS) {
        return Err(format!(
            "the route is {total} polls; the most one call runs is {MAX_POLLS}"
        ));
    }
    // (buttons, right x, right y, left x, left y) per poll.
    let mut samples: Vec<(u16, u8, u8, u8, u8)> = Vec::with_capacity(total as usize);
    for poll in 0..warmup {
        // A tap every 240 polls from poll 60, held 12: enough to clear a
        // title card or a splash that waits for CROSS, and only during load.
        let tapped = poll >= 60 && (poll - 60) % 240 < 12;
        samples.push((if tapped { 1 << 14 } else { 0 }, 128, 128, 128, 128));
    }
    for (index, leg) in legs.iter().enumerate() {
        let mut buttons = 0u16;
        for name in leg.buttons.iter().flatten() {
            buttons |= button_bit(name).ok_or_else(|| {
                format!("leg {index}: unknown button {name:?}; use cross, circle, square, triangle, l1, r1, l2, r2, start, select, up, down, left or right")
            })?;
        }
        let stick = match (&leg.stick, &leg.dir) {
            (Some(s), _) => Some(*s),
            (None, Some(name)) => Some(dir_stick(name).map_err(|e| format!("leg {index}: {e}"))?),
            (None, None) => None,
        };
        let (lx, ly) = stick.map_or((128, 128), stick_bytes);
        let (rx, ry) = leg.camera.map_or((128, 128), stick_bytes);
        for _ in 0..leg.polls {
            samples.push((buttons, rx, ry, lx, ly));
        }
    }
    let mut csv = String::from(
        "psoxide-tape,v2,clock=pad_poll,start_poll=0\nframe,buttons,right_x,right_y,left_x,left_y\n",
    );
    for (n, (b, rx, ry, lx, ly)) in samples.iter().enumerate() {
        csv.push_str(&format!("{n},{b},{rx},{ry},{lx},{ly}\n"));
    }
    Ok(Route {
        csv,
        polls: total as u32,
        start: warmup,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_route_is_a_warmup_of_taps_then_the_legs() {
        let legs = vec![
            Leg {
                polls: 100,
                dir: Some("forward".into()),
                ..Leg::default()
            },
            Leg {
                polls: 50,
                stick: Some([1.0, 0.0]),
                buttons: Some(vec!["cross".into()]),
                ..Leg::default()
            },
        ];
        let route = build(300, &legs).unwrap();
        assert_eq!((route.polls, route.start), (450, 300));
        let rows: Vec<&str> = route.csv.lines().skip(2).collect();
        assert_eq!(rows.len(), 450);
        // A tap at poll 60 of the warm-up.
        assert!(rows[60].starts_with("60,16384,"), "{}", rows[60]);
        assert!(rows[100].starts_with("100,0,"), "{}", rows[100]);
        // Forward is stick up (0), centred x.
        assert_eq!(rows[300], "300,0,128,128,128,1");
        // Right at full tilt, cross held.
        assert_eq!(rows[400], "400,16384,128,128,255,128");
        assert!(build(0, &[]).is_err());
        let bad = Leg {
            polls: 1,
            buttons: Some(vec!["fire".into()]),
            ..Leg::default()
        };
        assert!(build(0, &[bad]).unwrap_err().contains("unknown button"));
    }
}
