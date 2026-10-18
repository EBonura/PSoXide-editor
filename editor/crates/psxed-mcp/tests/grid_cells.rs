//! Cells of several sizes over the stdio protocol (T9): subdivide a corner
//! of a floor, shape its fine cells, read them back three ways, cook, merge.

mod common;

use std::path::{Path, PathBuf};

use common::Client;
use serde_json::json;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root")
}

fn new_project(tmp: &Path) -> PathBuf {
    let projects = tmp.join("projects");
    std::fs::create_dir_all(&projects).expect("create projects");
    for name in ["default", "graybox-reach"] {
        std::os::unix::fs::symlink(
            repo_root().join("editor/projects").join(name),
            projects.join(name),
        )
        .expect("link a base project");
    }
    let project = projects.join("cells");
    let made = std::process::Command::new(env!("CARGO_BIN_EXE_psxed-mcp"))
        .args(["--new", project.to_str().unwrap()])
        .env("PSOXIDE_PROJECTS_DIR", &projects)
        .output()
        .expect("run --new");
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    project
}

#[test]
fn a_corner_of_a_big_floor_is_subdivided_shaped_read_cooked_and_merged() {
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-cells-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let project = new_project(&tmp);
    let mut c = Client::start(&["--project", project.to_str().unwrap()], &[], &repo_root());

    let before = c.ok("grid_leaves", json!({})).text;
    assert!(before.contains("36 leaves: 36 of 1 sector"), "{before}");

    // Sector (1,1) becomes quarter-sector cells; the balance splits the
    // sectors around it.
    let r = c
        .ok("grid_subdivide", json!({"from": [1, 1], "size": "quarter"}))
        .text;
    assert!(r.contains("leaves"), "{r}");
    let leaves = c.ok("grid_leaves", json!({})).text;
    assert!(leaves.contains("tree: balanced"), "{leaves}");
    assert!(leaves.contains("of 1/4 sector"), "{leaves}");

    // The height map marks the sector made of finer cells.
    let map = c
        .ok("grid_heights", json!({"from": [0, 0], "to": [3, 3]}))
        .text;
    assert!(map.contains('%'), "{map}");
    // grid_sector lists its leaves.
    let sector = c.ok("grid_sector", json!({"cell": [1, 1]})).text;
    assert!(sector.contains("leaf level 4"), "{sector}");

    // Steps: raise the clicks of the first column by one click each.
    for (i, click) in [4, 5, 6, 7].into_iter().enumerate() {
        let r = c
            .ok(
                "grid_raise",
                json!({"from": [click, 4], "to": [click, 7], "unit": "click",
                       "amount": 256 * (i as i32 + 1), "mode": "level", "reference": 0}),
            )
            .text;
        assert!(r.contains("raised"), "{r}");
    }
    let sector = c.ok("grid_sector", json!({"cell": [1, 1]})).text;
    assert!(sector.contains("[1024, 1024, 1024, 1024]"), "{sector}");

    // The cook takes it and warns about the sector it cannot collide exactly.
    let check = c.ok("cook_check", json!({})).text;
    assert!(!check.contains("error"), "{check}");
    assert!(check.contains("sector 1,1"), "{check}");

    // The plan draws the cell borders.
    let plan = c.ok("grid_plan", json!({"leaves": true}));
    assert!(plan.text.contains("room"), "{}", plan.text);

    // Level the steps again and merge everything back.
    c.ok(
        "grid_raise",
        json!({"from": [4, 4], "to": [7, 7], "unit": "click", "amount": 0, "mode": "level", "reference": 0}),
    );
    let merged = c
        .ok("grid_merge", json!({"from": [0, 0], "to": [5, 5]}))
        .text;
    assert!(merged.contains("merge"), "{merged}");
    let after = c.ok("grid_leaves", json!({})).text;
    assert!(after.contains("36 leaves: 36 of 1 sector"), "{after}");
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Builds the T9 demo project through the protocol: a 12 by 12 sector open
/// floor of big cells with one corner of quarter-sector cells holding a
/// staircase of fine treads. Run it with
/// `PSXED_CELLS_DEMO=<dir under editor/projects> cargo test -p psxed-mcp --test grid_cells -- --ignored build_the_cells_demo`.
#[test]
#[ignore]
fn build_the_cells_demo() {
    let Ok(dir) = std::env::var("PSXED_CELLS_DEMO") else {
        return;
    };
    let projects = repo_root().join("editor/projects");
    let project = projects.join(&dir);
    let _ = std::fs::remove_dir_all(&project);
    let made = std::process::Command::new(env!("CARGO_BIN_EXE_psxed-mcp"))
        .args(["--new", project.to_str().unwrap()])
        .output()
        .expect("run --new");
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    let mut c = Client::start(&["--project", project.to_str().unwrap()], &[], &repo_root());

    // A 12 by 12 hall: floor, ceiling, merged into big cells.
    c.ok(
        "grid_add_surface",
        json!({"kind": "floor", "from": [0, 0], "to": [11, 11], "height": 0}),
    );
    c.ok(
        "grid_add_surface",
        json!({"kind": "ceiling", "from": [0, 0], "to": [11, 11], "height": 3072}),
    );
    if std::env::var("PSXED_CELLS_DEMO_STOP").as_deref() == Ok("seam") {
        // A slope of whole sectors, finer cells in half of it with a bump:
        // the seam between coarse and fine cells on a tilted floor.
        c.ok(
            "grid_ramp",
            json!({"from": [2, 2], "to": [5, 3], "keep": "north", "rise": 1024}),
        );
        c.ok(
            "grid_subdivide",
            json!({"from": [3, 2], "to": [4, 3], "size": "half"}),
        );
        c.ok(
            "grid_subdivide",
            json!({"from": [3, 2], "to": [3, 2], "size": "quarter"}),
        );
        c.ok("grid_raise", json!({"from": [13, 9], "to": [14, 10], "unit": "click", "amount": 128, "mode": "offset", "snap": "fine"}));
        println!("{}", c.ok("grid_leaves", json!({})).text);
        c.ok("save", json!({}));
        return;
    }
    let merged = c
        .ok(
            "grid_merge",
            json!({"from": [0, 0], "to": [11, 11], "up_to": "four"}),
        )
        .text;
    println!("{merged}");
    let stop = std::env::var("PSXED_CELLS_DEMO_STOP").unwrap_or_default();
    if stop == "merged" {
        c.ok("save", json!({}));
        return;
    }
    // The south-east corner: two sectors of quarter cells.
    let sub = c
        .ok(
            "grid_subdivide",
            json!({"from": [8, 8], "to": [9, 9], "size": "quarter"}),
        )
        .text;
    println!("{sub}");
    if stop == "subdivided" {
        c.ok("save", json!({}));
        return;
    }
    // A staircase of fine treads, 64 grid units a tread, one quarter sector
    // deep: eight treads across two sectors.
    for k in 0..8 {
        let click = 32 + k;
        c.ok(
            "grid_raise",
            json!({"from": [click, 32], "to": [click, 39], "unit": "click",
                   "amount": 64 * (k + 1), "mode": "level", "reference": 0, "snap": "fine",
                   "follow_ceiling": false}),
        );
    }
    println!("{}", c.ok("grid_leaves", json!({})).text);
    c.ok("delete_node", json!({"node": "Starter Light"}));
    c.ok("add_light", json!({"name": "Hall lamp", "position": [8800, 2200, 8800], "radius": 7000, "color": [255, 226, 190], "intensity": 0.6}));
    c.ok("add_light", json!({"name": "Stair lamp", "position": [12000, 1200, 12000], "radius": 6000, "color": [255, 200, 150], "intensity": 0.5}));
    // The player starts west of the stairs, facing east.
    c.ok(
        "move_node",
        json!({"node": "Aletha (Player)", "position": [8000, 64, 12400]}),
    );
    println!("{}", c.ok("cook_check", json!({})).text);
    c.ok("save", json!({}));
}

/// Plays the demo: the player walks east up the fine staircase. Needs the
/// built frontend and `PSOXIDE_GUEST_STAGE_ROOT`; pictures go to
/// `PSXED_MCP_PNG_DIR`.
#[test]
#[ignore]
fn play_the_cells_demo() {
    let Ok(dir) = std::env::var("PSXED_CELLS_DEMO") else {
        return;
    };
    let project = repo_root().join("editor/projects").join(dir);
    let mut c = Client::start(&["--project", project.to_str().unwrap()], &[], &repo_root());
    let run = c.ok(
        "playtest",
        json!({
            "route": [ {"polls": 700, "dir": "right"} ],
            "warmup_polls": 450,
            "frames": [0, 45, 70, 95, 120, 140],
            "track_player": true,
            "telemetry": false
        }),
    );
    println!("{}\nimages: {}", run.text, run.images.len());
    if let Some(out) = std::env::var_os("PSXED_MCP_PNG_DIR") {
        let out = PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        for (i, png) in run.images.iter().enumerate() {
            std::fs::write(out.join(format!("play-{i}.png")), png).unwrap();
        }
    }
    assert!(!run.text.contains("stalled"), "{}", run.text);
}
