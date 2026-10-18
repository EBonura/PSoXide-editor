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
