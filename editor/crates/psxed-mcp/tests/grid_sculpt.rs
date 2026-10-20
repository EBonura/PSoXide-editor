//! `grid_sculpt` over the stdio protocol: dunes and a valley on a fresh New
//! Grid Project, read back with the height tools, with the cook still happy.

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
    let project = projects.join("sculpt");
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
fn sculpting_through_the_tool_shapes_the_ground_and_the_cook_accepts_it() {
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-sculpt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let project = new_project(&tmp);
    let mut c = Client::start(&["--project", project.to_str().unwrap()], &[], &repo_root());
    let pristine = c.ok("grid_budget", json!({})).text;
    // A bigger floor to work on.
    c.ok(
        "grid_add_surface",
        json!({"kind": "floor", "from": [0, 0], "to": [15, 15], "height": 0}),
    );
    let before = c.ok("grid_budget", json!({})).text;
    // Dunes across it, then a dug valley.
    let dunes = c
        .ok(
            "grid_sculpt",
            json!({"path": [[2.0, 8.0], [14.0, 8.0]], "mode": "dunes", "radius": 2.0,
                   "wavelength": 3.0, "dune_height": 256, "detail": "auto"}),
        )
        .text;
    assert!(dunes.contains("sculpt Dunes"), "{dunes}");
    assert!(!dunes.contains("0 corner(s) moved"), "{dunes}");
    let valley = c
        .ok(
            "grid_sculpt",
            json!({"path": [[3.0, 3.0], [8.0, 4.0], [13.0, 3.0]], "radius": 1.5,
                   "strength": 1.0, "invert": true}),
        )
        .text;
    assert!(valley.contains("sculpt Raise/Lower"), "{valley}");
    // The ground moved: a sector in the valley is lower than the flat floor.
    let low = c.ok("grid_sector", json!({"cell": [8, 3]})).text;
    assert!(low.contains("floor:"), "{low}");
    assert!(!low.contains("heights [0, 0, 0, 0]"), "{low}");
    let walk = c.ok("grid_walkability", json!({})).text;
    assert!(walk.contains("too steep"), "{walk}");
    let cook = c.ok("cook_check", json!({})).text;
    assert!(cook.starts_with("cook: OK"), "{cook}");
    let after = c.ok("grid_budget", json!({})).text;
    assert_ne!(before, after);
    // Bad input is refused with a reason.
    let bad = c
        .call("grid_sculpt", json!({"path": [[1.0, 1.0]], "mode": "melt"}))
        .map(|o| o.text)
        .unwrap_or_else(|e| e);
    assert!(bad.contains("unknown mode"), "{bad}");
    // Dropping the staged edits puts the ground back.
    c.ok("revert", json!({}));
    assert_eq!(c.ok("grid_budget", json!({})).text, pristine);
}
