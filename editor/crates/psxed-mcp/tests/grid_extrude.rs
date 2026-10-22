//! grid_extrude: the editor's Extrude tool over the protocol. grid_raise
//! stays as a marked alias of the floor case.

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
    let project = projects.join("extrude");
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

fn client(label: &str) -> (Client, PathBuf) {
    let tmp =
        std::env::temp_dir().join(format!("psxed-mcp-extrude-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let project = new_project(&tmp);
    (
        Client::start(&["--project", project.to_str().unwrap()], &[], &repo_root()),
        tmp,
    )
}

/// A reply without the note about staged edits.
fn bare(text: &str) -> &str {
    text.split("\n[").next().unwrap_or(text).trim_end()
}

/// The 6 x 6 starter room's north wall at cell `x`.
fn north(x: i32) -> serde_json::Value {
    json!({"cell": [x, 0], "edge": "north", "kind": "closure"})
}

#[test]
fn grid_extrude_is_listed_and_grid_raise_is_a_marked_alias() {
    let (mut c, tmp) = client("list");
    let tools = c.tools();
    let description = |name: &str| {
        tools
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("{name} is not a tool"))["description"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let extrude = description("grid_extrude");
    for word in ["alcove", "niche", "fills", "refused"] {
        assert!(extrude.contains(word), "{word} in {extrude}");
    }
    let raise = description("grid_raise");
    assert!(
        raise.contains("Deprecated") && raise.contains("grid_extrude"),
        "{raise}"
    );
    // Old scripts still run.
    let raised = c
        .ok(
            "grid_raise",
            json!({"from": [0, 0], "to": [1, 1], "amount": 256}),
        )
        .text;
    assert!(raised.contains("raised 4 sector(s)"), "{raised}");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn a_floor_extrude_moves_the_floors_and_leaves_the_ceilings() {
    let (mut c, tmp) = client("floor");
    let said = c
        .ok(
            "grid_extrude",
            json!({"surface": "floor", "from": [0, 0], "to": [1, 1], "amount": 512}),
        )
        .text;
    assert!(
        said.contains("Extrude floor") && said.contains("raised 4 sector(s)"),
        "{said}"
    );
    let sector = c.ok("grid_sector", json!({"cell": [0, 0]})).text;
    assert!(
        sector.contains("floor: corners nw/ne/se/sw [512, 512, 512, 512]"),
        "{sector}"
    );
    // The editor leaves the ceiling where it was: raise would have lifted it.
    assert!(
        sector.contains("ceiling: corners nw/ne/se/sw [2048, 2048, 2048, 2048]"),
        "{sector}"
    );
    // Down again.
    let down = c
        .ok(
            "grid_extrude",
            json!({"surface": "floor", "from": [0, 0], "to": [1, 1], "amount": -512}),
        )
        .text;
    assert!(down.contains("lowered 4 sector(s)"), "{down}");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn a_ceiling_extrude_moves_the_ceilings_and_holds_a_click_over_the_floor() {
    let (mut c, tmp) = client("ceiling");
    let said = c
        .ok(
            "grid_extrude",
            json!({"surface": "ceiling", "from": [2, 2], "to": [3, 3], "amount": 512}),
        )
        .text;
    assert!(said.contains("moved 4 ceiling(s)"), "{said}");
    let sector = c.ok("grid_sector", json!({"cell": [2, 2]})).text;
    assert!(
        sector.contains("ceiling: corners nw/ne/se/sw [2560, 2560, 2560, 2560]"),
        "{sector}"
    );
    // Lowered past the floor: held one click over it.
    let held = c
        .ok(
            "grid_extrude",
            json!({"surface": "ceiling", "from": [2, 2], "amount": -4096}),
        )
        .text;
    assert!(held.contains("held one click over their floor"), "{held}");
    let sector = c.ok("grid_sector", json!({"cell": [2, 2]})).text;
    assert!(
        sector.contains("ceiling: corners nw/ne/se/sw [256, 256, 256, 256]"),
        "{sector}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn a_wall_pushed_out_makes_an_alcove_and_pushed_in_fills_it_again() {
    let (mut c, tmp) = client("wall");
    let before = c
        .ok("grid_heights", json!({"from": [0, -2], "to": [5, 0]}))
        .text;
    let out = c
        .ok(
            "grid_extrude",
            json!({"surface": "wall", "wall": north(2), "steps": 1}),
        )
        .text;
    assert!(
        out.contains("Extrude out 1 sector") && out.contains("made"),
        "{out}"
    );
    // The room grew a sector behind the wall: floor and ceiling like the
    // hall's, and its own derived walls close it.
    let alcove = c.ok("grid_sector", json!({"cell": [2, -1]})).text;
    assert!(
        alcove.contains("floor: corners nw/ne/se/sw [0, 0, 0, 0]"),
        "{alcove}"
    );
    assert!(
        alcove.contains("ceiling: corners nw/ne/se/sw [2048, 2048, 2048, 2048]"),
        "{alcove}"
    );
    assert!(alcove.contains("derived wall north closure"), "{alcove}");
    // The wall that closed the hall at z = 0 is gone, the new one is at z = -1.
    let walls = c
        .ok("grid_walls", json!({"from": [2, -1], "to": [2, 0]}))
        .text;
    assert!(!walls.contains("[2, 0] north closure"), "{walls}");
    // In: the alcove fills again (the new north wall goes in by one).
    let back = c
        .ok(
            "grid_extrude",
            json!({"surface": "wall", "wall": {"cell": [2, -1], "edge": "north", "kind": "closure"}, "steps": -1}),
        )
        .text;
    assert!(back.contains("Extrude in 1 sector"), "{back}");
    let after = c
        .ok("grid_heights", json!({"from": [0, -2], "to": [5, 0]}))
        .text;
    assert_eq!(
        bare(&before),
        bare(&after),
        "out then in is the room it was"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn a_band_pushed_out_makes_a_niche_of_its_own_height() {
    let (mut c, tmp) = client("band");
    c.ok(
        "grid_band_cut",
        json!({"cell": [3, 0], "edge": "north", "kind": "closure", "anchor": "abs", "value": 1024}),
    );
    let niche = c
        .ok(
            "grid_extrude",
            json!({"surface": "wall", "wall": north(3), "band": 1, "steps": 1}),
        )
        .text;
    assert!(
        niche.contains("Extrude out 1 sector") && niche.contains("band 1"),
        "{niche}"
    );
    // The niche opens from the band's bottom to its top, not the floor.
    let at = c.ok("grid_sector", json!({"cell": [3, -1]})).text;
    assert!(
        at.contains("floor: corners nw/ne/se/sw [1024, 1024, 1024, 1024]"),
        "{at}"
    );
    assert!(
        at.contains("ceiling: corners nw/ne/se/sw [2048, 2048, 2048, 2048]"),
        "{at}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn a_push_that_cannot_be_made_is_refused_with_the_reason_and_changes_nothing() {
    let (mut c, tmp) = client("refuse");
    let refused = |c: &mut Client, args: serde_json::Value| -> String {
        c.call("grid_extrude", args)
            .expect_err("a refusal is an error")
    };
    let why = refused(
        &mut c,
        json!({"surface": "sideways", "from": [0, 0], "amount": 256}),
    );
    assert!(why.contains("floor, ceiling or wall"), "{why}");
    let why = refused(
        &mut c,
        json!({"surface": "wall", "wall": north(2), "steps": 0}),
    );
    assert!(why.contains("steps is 0"), "{why}");
    let why = refused(&mut c, json!({"surface": "wall"}));
    assert!(why.contains("needs `wall`"), "{why}");
    let why = refused(&mut c, json!({"surface": "floor", "amount": 256}));
    assert!(why.contains("needs `from`"), "{why}");
    let why = refused(&mut c, json!({"surface": "floor", "from": [0, 0]}));
    assert!(why.contains("needs `amount`"), "{why}");
    // A wall that is not there.
    let why = refused(
        &mut c,
        json!({"surface": "wall", "wall": {"cell": [2, 2], "edge": "north"}}),
    );
    assert!(why.contains("no wall") || why.contains("none"), "{why}");
    // Cells of several sizes: whole sectors only.
    c.ok("grid_subdivide", json!({"from": [4, 4]}));
    let before = c
        .ok("grid_heights", json!({"from": [0, -2], "to": [5, 5]}))
        .text;
    let why = refused(
        &mut c,
        json!({"surface": "wall", "wall": north(2), "steps": 1}),
    );
    assert!(why.contains("several sizes"), "{why}");
    let after = c
        .ok("grid_heights", json!({"from": [0, -2], "to": [5, 5]}))
        .text;
    assert_eq!(bare(&before), bare(&after), "a refusal changes nothing");
    let _ = std::fs::remove_dir_all(&tmp);
}
