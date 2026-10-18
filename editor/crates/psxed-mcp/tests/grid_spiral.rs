//! Spiral stairs, aimed stairs and the click Raise over the stdio protocol:
//! the tools that edit the floor of a column of several floors name which
//! one, and a refusal says how.

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

fn projects_dir(tmp: &Path) -> PathBuf {
    let projects = tmp.join("projects");
    std::fs::create_dir_all(&projects).expect("create projects");
    for name in ["default", "graybox-reach"] {
        std::os::unix::fs::symlink(
            repo_root().join("editor/projects").join(name),
            projects.join(name),
        )
        .expect("link a base project");
    }
    projects
}

fn client(label: &str) -> Client {
    let repo = repo_root();
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let projects = projects_dir(&tmp);
    let project = projects.join("tower");
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
    Client::start(&["--project", project.to_str().unwrap()], &[], &repo)
}

#[test]
fn a_spiral_winds_two_storeys_round_a_ring_and_the_column_reads_back() {
    let mut c = client("spiral");
    // A 6 by 6 hall, 24 clicks tall.
    c.ok(
        "grid_set_heights",
        json!({"kind": "ceiling", "from": [0, 0], "to": [5, 5], "height": 6144}),
    );
    let ring = json!([[0, 0], [5, 0], [5, 5], [0, 5]]);
    // A line is no ring, and a bad riser is named.
    let line = c
        .call(
            "grid_spiral_stairs",
            json!({"ring": [[0, 0], [5, 0]], "rise_per_storey": 1024}),
        )
        .expect_err("a line is not a ring");
    assert!(line.contains("closed ring"), "{line}");
    let climb = c
        .call(
            "grid_spiral_stairs",
            json!({"ring": ring, "rise_per_storey": 0}),
        )
        .expect_err("no climb");
    assert!(climb.contains("above zero"), "{climb}");
    let riser = c
        .call(
            "grid_spiral_stairs",
            json!({"ring": ring, "rise_per_storey": 1024, "riser": "tall"}),
        )
        .expect_err("a riser is click, max or a number");
    assert!(riser.contains("unknown riser"), "{riser}");
    let laid = c
        .ok(
            "grid_spiral_stairs",
            json!({"ring": ring, "storeys": 2, "rise_per_storey": 1024}),
        )
        .text;
    // Four clicks a storey at a click a tread: 8 treads, 16 to a lap.
    assert!(laid.contains("spiral of 8 treads"), "{laid}");
    assert!(laid.contains("slab(s) laid"), "{laid}");
    // The ring starts on a corner (a landing), so tread 1 is the next cell.
    let first = c.ok("grid_column", json!({"cell": [1, 0]})).text;
    assert!(first.contains("floor 1"), "{first}");
    let second = c.ok("grid_column", json!({"cell": [2, 0]})).text;
    assert!(second.contains("floor 2"), "{second}");
    // The hall inside the ring is untouched.
    let inside = c.ok("grid_column", json!({"cell": [2, 2]})).text;
    assert!(inside.contains("1 span(s)"), "{inside}");

    let cook = c.ok("cook_check", json!({})).text;
    assert!(cook.contains("cook: OK"), "{cook}");
}

#[test]
fn a_spiral_over_a_slab_floor_starts_from_the_storey_it_is_aimed_at() {
    let mut c = client("spiral-aim");
    c.ok(
        "grid_set_heights",
        json!({"kind": "ceiling", "from": [0, 0], "to": [5, 5], "height": 9216}),
    );
    // A slab at 12 clicks over everything: two storeys everywhere.
    c.ok(
        "grid_insert_slab",
        json!({"rects": [[0, 0, 5, 5]], "y": 3072}),
    );
    let refused = c
        .call(
            "grid_spiral_stairs",
            json!({"ring": [[0, 0], [5, 0], [5, 5], [0, 5]], "rise_per_storey": 1024}),
        )
        .expect_err("two floors and no aim");
    assert!(refused.contains("has 2 spans"), "{refused}");
    // Aimed at the upper storey the climb starts at 12 clicks.
    let laid = c
        .ok(
            "grid_spiral_stairs",
            json!({"ring": [[0, 0], [5, 0], [5, 5], [0, 5]], "rise_per_storey": 1024, "storey": 1}),
        )
        .text;
    assert!(laid.contains("spiral of 4 treads"), "{laid}");
    let column = c.ok("grid_column", json!({"cell": [1, 0]})).text;
    assert!(column.contains("floor 13"), "{column}");
}

#[test]
fn stairs_and_the_click_raise_name_the_floor_in_a_column_of_several() {
    let mut c = client("stairs-aim");
    c.ok(
        "grid_set_heights",
        json!({"kind": "ceiling", "from": [0, 0], "to": [5, 5], "height": 6144}),
    );
    c.ok(
        "grid_insert_slab",
        json!({"rects": [[0, 0, 5, 5]], "y": 3072}),
    );

    // Stairs refuse rather than guess, then climb the storey they are aimed at.
    let refused = c
        .call(
            "grid_stairs",
            json!({"path": [[0, 0], [3, 0]], "rise": 1024}),
        )
        .expect_err("two floors and no aim");
    assert!(refused.contains("has 2 spans"), "{refused}");
    c.ok(
        "grid_stairs",
        json!({"path": [[0, 0], [3, 0]], "rise": 1024, "storey": 1}),
    );
    let upper = c.ok("grid_column", json!({"cell": [1, 0]})).text;
    assert!(upper.contains("span 1: floor 14"), "{upper}");
    let ground = c.ok("grid_column", json!({"cell": [1, 0]})).text;
    assert!(ground.contains("span 0: floor 0 "), "{ground}");

    // The click Raise used to skip the refusal and edit the ground span.
    let click = c
        .call(
            "grid_raise",
            json!({"from": [4, 4], "to": [5, 5], "unit": "click", "amount": 256}),
        )
        .expect_err("the click raise refuses a column of several floors");
    assert!(click.contains("2 spans"), "{click}");
    assert!(click.contains("fine verbs"), "{click}");
    // And a span aim does not apply to cells smaller than a sector.
    let aimed = c
        .call(
            "grid_raise",
            json!({"from": [4, 4], "to": [5, 5], "unit": "click", "amount": 256, "storey": 1}),
        )
        .expect_err("a span aim needs whole sectors");
    assert!(aimed.contains("whole sector"), "{aimed}");
    // Sector Raise aimed at the upper storey works and leaves the ground.
    c.ok(
        "grid_raise",
        json!({"from": [4, 4], "to": [5, 5], "amount": 256, "storey": 1}),
    );
    let raised = c.ok("grid_column", json!({"cell": [4, 4]})).text;
    assert!(raised.contains("span 1: floor 13"), "{raised}");
    assert!(raised.contains("span 0: floor 0 "), "{raised}");
}
