//! The player's path through a playtest, from the guest's camera-pose log.
//!
//! A disc built with the guest's `camera-pose-log` feature prints one
//! `camera-pose,tick,player.x,player.y,player.z,...` line per update. The
//! emulator stamps each line with the bus cycle it arrived at
//! (`[guest f<frame> c<cycles>] ...`), and the route log maps bus cycles to
//! port-1 polls, so every position lands on the poll it belongs to. Positions
//! are runtime units; times 16 is editor units, the same units nodes use.

use std::fmt::Write as _;

/// Editor units per runtime unit.
pub const EDITOR_PER_RUNTIME: i32 = 16;

/// The player at one poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    /// Port-1 poll (simulation tick) the position belongs to.
    pub poll: u64,
    /// Player position, EDITOR units.
    pub editor: [i32; 3],
}

/// Pull the player's positions out of the emulator's output, each placed on
/// its poll through `route_csv`.
pub fn parse(output: &str, route_csv: &str) -> Vec<Sample> {
    let mut lines = route_csv.lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let cols: Vec<&str> = header.split(',').collect();
    let (Some(cycle_col), Some(poll_col)) = (
        cols.iter().position(|c| *c == "bus_cycles"),
        cols.iter().position(|c| *c == "port1_polls"),
    ) else {
        return Vec::new();
    };
    let ticks: Vec<(i64, u64)> = lines
        .filter_map(|line| {
            let cells: Vec<&str> = line.split(',').collect();
            Some((
                cells.get(cycle_col)?.parse().ok()?,
                cells.get(poll_col)?.parse().ok()?,
            ))
        })
        .collect();
    let mut out = Vec::new();
    for line in output.lines() {
        let Some(rest) = line.split("] camera-pose,").nth(1) else {
            continue;
        };
        let Some(cycles) = line
            .split(" c")
            .nth(1)
            .and_then(|s| s.split(']').next())
            .and_then(|s| s.parse::<i64>().ok())
        else {
            continue;
        };
        let values: Vec<i32> = rest
            .split(',')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        if values.len() < 4 {
            continue;
        }
        // The last route row at or before this line's bus cycle.
        let at = ticks.partition_point(|(c, _)| *c <= cycles);
        let poll = at.checked_sub(1).map_or(0, |i| ticks[i].1);
        out.push(Sample {
            poll,
            editor: [
                values[1] * EDITOR_PER_RUNTIME,
                values[2] * EDITOR_PER_RUNTIME,
                values[3] * EDITOR_PER_RUNTIME,
            ],
        });
    }
    out
}

/// The sample at or after `poll`, else the last one.
pub fn at_poll(samples: &[Sample], poll: u64) -> Option<Sample> {
    samples
        .iter()
        .find(|s| s.poll >= poll)
        .or_else(|| samples.last())
        .copied()
}

/// A short account of the path: where the route's polls start and end, the
/// positions at the polls asked for, and how high and low the player went.
pub fn describe(samples: &[Sample], from_poll: u64, wanted: &[u64]) -> String {
    let inside: Vec<&Sample> = samples.iter().filter(|s| s.poll >= from_poll).collect();
    let (Some(first), Some(last)) = (inside.first(), inside.last()) else {
        return "player path: no camera-pose lines came back (the disc needs the guest's camera-pose-log feature; build without skip_build)\n".to_string();
    };
    let mut out = format!(
        "player path (editor units): start [{}, {}, {}] -> end [{}, {}, {}] over {} update(s)\n",
        first.editor[0],
        first.editor[1],
        first.editor[2],
        last.editor[0],
        last.editor[1],
        last.editor[2],
        inside.len()
    );
    let lowest = inside.iter().map(|s| s.editor[1]).min().unwrap_or(0);
    let highest = inside.iter().map(|s| s.editor[1]).max().unwrap_or(0);
    let _ = writeln!(
        out,
        "height range {lowest}..{highest} (a fall shows as a low minimum)"
    );
    for poll in wanted {
        if let Some(s) = at_poll(samples, from_poll + poll) {
            let _ = writeln!(
                out,
                "  at route poll {poll}: [{}, {}, {}]",
                s.editor[0], s.editor[1], s.editor[2]
            );
        }
    }
    // The whole path at a glance, one point every half second.
    let last_poll = last.poll;
    let mut trace = String::new();
    let mut poll = from_poll;
    while poll <= last_poll {
        if let Some(s) = at_poll(samples, poll) {
            let _ = write!(
                trace,
                " {}:[{},{},{}]",
                poll - from_poll,
                s.editor[0],
                s.editor[1],
                s.editor[2]
            );
        }
        poll += 30;
    }
    let _ = writeln!(out, "path, route poll:[x,y,z] every 30 polls:{trace}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pose_lines_land_on_their_polls_in_editor_units() {
        let route = "route_tick,bus_cycles,port1_polls\n0,0,0\n1,1000,10\n2,2000,20\n3,3000,30\n";
        let log = "[guest f1 c500] camera-pose,0,280,4,360,1,2,3\n\
                   [guest f2 c2500] camera-pose,1,281,5,300,1,2,3\n\
                   [guest f3 c9999] unrelated line\n";
        let samples = parse(log, route);
        assert_eq!(samples.len(), 2);
        assert_eq!(
            samples[0],
            Sample {
                poll: 0,
                editor: [4480, 64, 5760]
            }
        );
        assert_eq!(samples[1].poll, 20);
        assert_eq!(at_poll(&samples, 5).unwrap().poll, 20);
        assert_eq!(at_poll(&samples, 99).unwrap().poll, 20);
        let text = describe(&samples, 0, &[0, 20]);
        assert!(text.contains("start [4480, 64, 5760]"), "{text}");
        assert!(text.contains("height range 64..80"), "{text}");
    }
}
