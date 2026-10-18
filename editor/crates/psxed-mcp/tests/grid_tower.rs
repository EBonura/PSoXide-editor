//! The octagonal tower (`docs/grid/multi-storey.md`, section 8) built only
//! with the server's tools, from `grid_tower.script`: the footprint, three
//! floors inserted afterwards, a well cut through them and one spiral stair
//! round the outer ring. The geometry reads back as the design says, the cook
//! accepts it, and nothing the stair needs is unreachable.
//!
//! `PSXED_MCP_VERIFY=1` (with a built `target/release/frontend` and
//! `PSOXIDE_GUEST_STAGE_ROOT` at a private directory) adds the play stage: a
//! poll-bound route that climbs the stair from the ground floor to the top
//! storey, a lap and a half round the ring, and must end on the third floor.
//!
//! `PSXED_TOWER_OUT=<dir>` builds the project at `<dir>` (which must not exist
//! yet) instead of a temporary one: how `editor/projects/grid-tower` is made.

mod common;

use std::path::PathBuf;

use common::Client;
use serde_json::{json, Value};

const SCRIPT: &str = include_str!("grid_tower.script");

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root")
}

/// Run every line of the script on `c`, panicking with the line that failed.
fn run_script(c: &mut Client) {
    for line in SCRIPT.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (tool, args) = line.split_once(' ').unwrap_or((line, "{}"));
        let args: Value = serde_json::from_str(args).unwrap_or_else(|e| panic!("{line}: {e}"));
        if let Err(error) = c.call(tool, args) {
            panic!("{tool} failed: {error}\n{line}");
        }
    }
}

#[test]
fn the_tower_is_built_from_the_script_and_reads_back_as_designed() {
    let repo = repo_root();
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-tower-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let out = std::env::var_os("PSXED_TOWER_OUT").map(PathBuf::from);
    let project = out.clone().unwrap_or_else(|| tmp.join("projects/tower"));
    std::fs::create_dir_all(project.parent().unwrap()).unwrap();
    if out.is_none() {
        for name in ["default", "graybox-reach"] {
            std::os::unix::fs::symlink(
                repo.join("editor/projects").join(name),
                tmp.join("projects").join(name),
            )
            .expect("link a base project");
        }
    }
    let made = std::process::Command::new(env!("CARGO_BIN_EXE_psxed-mcp"))
        .args(["--new", project.to_str().unwrap()])
        .output()
        .expect("run --new");
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    let mut c = Client::start(&["--project", project.to_str().unwrap()], &[], &repo);
    run_script(&mut c);

    // Four storeys, 12 clicks apart, and nothing else that counts as one.
    let storeys = c.ok("grid_storeys", json!({})).text;
    for (i, clicks) in [0, 12, 24, 36].iter().enumerate() {
        assert!(
            storeys.contains(&format!("storey {i}: floor at {clicks} clicks")),
            "{storeys}"
        );
    }
    assert!(!storeys.contains("storey 4"), "{storeys}");
    // 132 sectors: the octagon.
    let rooms = c.ok("grid_rooms", json!({})).text;
    assert!(rooms.contains("132 with geometry"), "{rooms}");
    // A platform sector away from the stair has all four floors; the well
    // is one tall hall; a top-side stair sector holds the undercroft and
    // two treads a storey apart.
    let platform = c.ok("grid_column", json!({"cell": [3, 3]})).text;
    assert!(platform.contains("4 span(s)"), "{platform}");
    for floor in ["floor 0 ", "floor 12 ", "floor 24 ", "floor 36 "] {
        assert!(platform.contains(floor), "{platform}");
    }
    let well = c.ok("grid_column", json!({"cell": [5, 5]})).text;
    assert!(well.contains("1 span(s)"), "{well}");
    let tread = c.ok("grid_column", json!({"cell": [6, 0]})).text;
    assert!(tread.contains("floor 4 "), "{tread}");
    assert!(tread.contains("floor 28 "), "{tread}");
    // The chamfer cells are half cells that close themselves on every storey.
    let chamfer = c.ok("grid_column", json!({"cell": [9, 0]})).text;
    assert!(
        chamfer.matches("closure on the diagonal").count() >= 3,
        "{chamfer}"
    );

    // The cook accepts it, and every stair tread is reachable from the spawn.
    let cook = c.ok("cook_check", json!({})).text;
    assert!(cook.contains("cook: OK"), "{cook}");
    let walk = c.ok("grid_walkability", json!({})).text;
    assert!(!walk.contains("no player spawn"), "{walk}");
    let budget = c.ok("grid_budget", json!({})).text;
    assert!(budget.contains("validate: no findings"), "{budget}");
    if out.is_some() {
        c.ok("save", json!({}));
    }

    if std::env::var("PSXED_MCP_VERIFY").as_deref() == Ok("1") {
        climb(&mut c, &repo);
    }
}

/// The climb: north to the first tread, then east along the north flight,
/// south the east side, west the south side, north the west side, east the
/// north side again and south the east side to the third floor. Diagonals
/// round the chamfers hug the walls (the sectors are 1280 editor units).
const ROUTE: &str = r#"[
  {"polls":60,"dir":"forward"},{"polls":143,"dir":"right"},
  {"polls":85,"stick":[0.7071,-0.7071]},{"polls":15,"dir":"right"},
  {"polls":170,"dir":"back"},{"polls":112,"stick":[-0.7071,-0.7071]},
  {"polls":154,"dir":"left"},{"polls":105,"stick":[-0.7071,0.7071]},
  {"polls":169,"dir":"forward"},{"polls":90,"stick":[0.7071,0.7071]},
  {"polls":168,"dir":"right"},{"polls":85,"stick":[0.7071,-0.7071]},
  {"polls":15,"dir":"right"},{"polls":170,"dir":"back"},{"polls":50,"dir":"left"}
]"#;

fn climb(c: &mut Client, repo: &std::path::Path) {
    assert!(
        repo.join("target/release/frontend").is_file(),
        "PSXED_MCP_VERIFY needs target/release/frontend"
    );
    let route: Value = serde_json::from_str(ROUTE).expect("the route parses");
    let run = c.ok(
        "playtest",
        json!({
            "route": route,
            "warmup_polls": 450,
            "frames": [200, 700, 1100, 1500, 1600],
            "track_player": true,
            "telemetry": false
        }),
    );
    println!("{}", run.text);
    assert!(!run.text.contains("stalled"), "{}", run.text);
    let path: Vec<[i32; 3]> = run
        .text
        .lines()
        .find(|l| l.starts_with("path, route poll"))
        .expect("a path line")
        .split_whitespace()
        .filter_map(|tok| tok.split_once(":[").map(|(_, v)| v.trim_end_matches(']')))
        .filter_map(|v| {
            let n: Vec<i32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
            (n.len() == 3).then(|| [n[0], n[1], n[2]])
        })
        .collect();
    assert!(!path.is_empty(), "{}", run.text);
    // The third floor is 36 clicks: 9216 grid units, 11520 editor units.
    let highest = path.iter().map(|p| p[1]).max().unwrap();
    let lowest = path.iter().map(|p| p[1]).min().unwrap();
    assert!(highest >= 11500, "never reached the third floor: {path:?}");
    assert!(lowest >= 0, "fell below the ground floor: {path:?}");
    // It ends on that floor, inside the octagon (a 12 sector square, corners
    // cut by 3 sectors: x + z and x - z stay inside the diagonals).
    let end = path.last().unwrap();
    assert!(end[1] >= 11500, "did not end on the third floor: {end:?}");
    let sector = 1280;
    assert!(
        (0..12 * sector).contains(&end[0]) && (0..12 * sector).contains(&end[2]),
        "{end:?}"
    );
}
