//! The Build group over the stdio protocol, on a temporary project (never a
//! tracked one): room, corridor, opening, door and lever, buildings, terrain,
//! styles and the wall API.

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
        std::os::unix::fs::symlink(repo_root().join("projects").join(name), projects.join(name))
            .expect("link a base project");
    }
    projects
}

fn client(label: &str) -> Client {
    let repo = repo_root();
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let projects = projects_dir(&tmp);
    let project = projects.join("build");
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
fn rooms_corridors_openings_doors_and_levers_make_a_small_level() {
    let mut c = client("build");
    // A room beside the starter one, then a corridor to the east of it.
    let room = c
        .ok(
            "grid_room",
            json!({"from": [10, 0], "to": [15, 5], "height": 3072}),
        )
        .text;
    assert!(room.contains("New room"), "{room}");
    let refused = c
        .call(
            "grid_room",
            json!({"from": [10, 0], "to": [15, 5], "height": 256, "room": 99}),
        )
        .expect_err("no such room");
    assert!(!refused.is_empty());
    let corridor = c
        .ok(
            "grid_corridor",
            json!({"waypoints": [[16, 2], [22, 2]], "width": 1, "style": "gothic nave"}),
        )
        .text;
    assert!(corridor.contains("Styled corridor"), "{corridor}");
    let rooms = c.ok("grid_rooms", json!({})).text;
    assert!(rooms.matches("sectors").count() >= 3, "{rooms}");
    // A door in the north wall of the new room, then its lever.
    let door = c
        .ok(
            "grid_door",
            json!({"room": 1, "cell": [12, 0], "side": "north", "from": 48, "to": 52,
                   "top": 2048, "lock": "one_way", "refuse_from": "north",
                   "refusal": "Barred from this side."}),
        )
        .text;
    assert!(door.contains("Door 1"), "{door}");
    let lever = c
        .ok(
            "grid_lever",
            json!({"cell": [14, 3], "operates": "Door 1", "behavior": "one_shot"}),
        )
        .text;
    assert!(lever.contains("operates Door 1"), "{lever}");
    let bad = c
        .call(
            "grid_lever",
            json!({"cell": [14, 3], "operates": "Nothing"}),
        )
        .expect_err("no such door");
    assert!(bad.contains("no node called"), "{bad}");
}

#[test]
fn walls_read_set_and_reset_through_one_api() {
    let mut c = client("wall");
    c.ok(
        "grid_room",
        json!({"from": [10, 0], "to": [10, 0], "height": 3072}),
    );
    let before = c
        .ok(
            "grid_wall_get",
            json!({"room": 1, "cell": [10, 0], "side": "north"}),
        )
        .text;
    assert!(before.contains("automatic"), "{before}");
    let set = c
        .ok(
            "grid_wall_set",
            json!({"room": 1, "cell": [10, 0], "side": "north", "heights": [null, null, 2048, 2048]}),
        )
        .text;
    assert!(set.contains("Wall heights set"), "{set}");
    let mid = c
        .ok(
            "grid_wall_get",
            json!({"room": 1, "cell": [10, 0], "side": "north"}),
        )
        .text;
    assert!(mid.contains("(set)"), "{mid}");
    let reset = c
        .ok(
            "grid_wall_reset",
            json!({"room": 1, "cell": [10, 0], "side": "north"}),
        )
        .text;
    assert!(reset.contains("automatic"), "{reset}");
}

#[test]
fn styles_are_listed_and_a_captured_bay_joins_them() {
    let mut c = client("styles");
    let list = c.ok("grid_style_list", json!({})).text;
    assert!(list.contains("gothic"), "{list}");
    c.ok(
        "grid_corridor",
        json!({"waypoints": [[10, 0], [17, 0]], "style": "gothic nave"}),
    );
    let captured = c
        .ok(
            "grid_capture_style",
            json!({"room": 1, "from": [10, 0], "to": [11, 0], "name": "My bay"}),
        )
        .text;
    assert!(captured.contains("My bay"), "{captured}");
    let after = c.ok("grid_style_list", json!({})).text;
    assert!(after.contains("My bay (captured bay)"), "{after}");
}

#[test]
fn a_building_terrain_a_ruin_a_move_and_a_tilt_each_answer_in_numbers() {
    let mut c = client("more");
    let b = c
        .ok(
            "grid_building",
            json!({"from": [10, 0], "to": [15, 5], "storeys": 2}),
        )
        .text;
    assert!(b.contains("Building in room"), "{b}");
    let t = c
        .ok(
            "grid_terrain",
            json!({"room": 0, "from": [0, 0], "to": [5, 5], "seed": 7}),
        )
        .text;
    assert!(t.contains("Terrain over"), "{t}");
    let r = c
        .ok(
            "grid_ruin",
            json!({"room": 0, "from": [0, 0], "to": [5, 5], "intensity": 30}),
        )
        .text;
    assert!(r.contains("Ruin:"), "{r}");
    let m = c
        .ok(
            "grid_move",
            json!({"room": 0, "from": [0, 0], "to": [1, 1], "delta": [20, 0]}),
        )
        .text;
    assert!(m.contains("Moved"), "{m}");
    let bad = c
        .call(
            "grid_tilt",
            json!({"room": 0, "from": [2, 2], "to": [3, 3], "hinge": "up", "rise": 256}),
        )
        .expect_err("a hinge is a side");
    assert!(bad.contains("unknown hinge"), "{bad}");
}
