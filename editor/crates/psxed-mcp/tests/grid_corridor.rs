//! The streaming corridor: sixteen rooms in a row, built only with the server's
//! tools, with a short draw distance so the player can walk further than the
//! room pool holds. It is the project the streamer is measured on
//! (`editor/projects/grid-corridor`): every step east asks the disc for a room
//! ahead and gives one behind back.
//!
//! `PSXED_CORRIDOR_OUT=<dir>` builds the project at `<dir>` (which must not
//! exist yet) instead of a temporary one: how `editor/projects/grid-corridor`
//! is made.

mod common;

use std::path::PathBuf;

use common::Client;
use serde_json::json;

/// Rooms in the row.
const ROOMS: i32 = 16;
/// Sectors along the corridor and across it, per room.
const LENGTH: i32 = 12;
const WIDTH: i32 = 6;
const FLOOR_MATERIAL: &str = "Graybox / Ground 64";
const WALL_MATERIAL: &str = "Graybox / Warm Stone 64";
const CEILING_MATERIAL: &str = "Graybox / Dark Trim 64";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root")
}

#[test]
fn the_corridor_is_built_with_the_tools_and_cooks() {
    let repo = repo_root();
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-corridor-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let out = std::env::var_os("PSXED_CORRIDOR_OUT").map(PathBuf::from);
    let project = out
        .clone()
        .unwrap_or_else(|| tmp.join("projects/corridor"));
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

    // The starter room's 6 by 6 hall goes; the corridor grows its own rooms.
    c.ok(
        "grid_remove_surface",
        json!({"kind": "floor", "from": [0, 0], "to": [5, 5]}),
    );
    c.ok(
        "grid_remove_surface",
        json!({"kind": "ceiling", "from": [0, 0], "to": [5, 5]}),
    );
    for room in 0..ROOMS {
        let x0 = room * LENGTH;
        if room > 0 {
            c.ok(
                "grid_add_room",
                json!({"name": format!("Hall {room}"), "origin": [x0, 0], "ambient": [112, 112, 112]}),
            );
        }
        c.ok(
            "grid_set_room",
            json!({"room": room, "name": format!("Hall {room}"), "wall_material": WALL_MATERIAL, "ambient": [112, 112, 112]}),
        );
        c.ok(
            "grid_add_surface",
            json!({"room": room, "kind": "floor", "from": [x0, 0], "to": [x0 + LENGTH - 1, WIDTH - 1], "height": 0, "material": FLOOR_MATERIAL}),
        );
        c.ok(
            "grid_add_surface",
            json!({"room": room, "kind": "ceiling", "from": [x0, 0], "to": [x0 + LENGTH - 1, WIDTH - 1], "height": 2048, "material": CEILING_MATERIAL}),
        );
        // A low platform in the middle of each hall: risers to draw, and
        // something to step up on while a room is arriving.
        c.ok(
            "grid_add_surface",
            json!({"room": room, "kind": "floor", "from": [x0 + 5, 2], "to": [x0 + 6, 3], "height": 256, "material": FLOOR_MATERIAL}),
        );
    }
    // The wall between two halls is two closure walls: open every sector of it.
    for room in 0..ROOMS - 1 {
        let east = room * LENGTH + LENGTH - 1;
        for z in 0..WIDTH {
            for (r, cell, edge) in [
                (room, [east, z], "east"),
                (room + 1, [east + 1, z], "west"),
            ] {
                c.ok(
                    "grid_wall_override",
                    json!({"room": r, "cell": cell, "edge": edge, "kind": "closure", "hidden": true, "solid": false}),
                );
            }
        }
    }
    // A short draw distance: the hall the player is in, the ones either side,
    // and a lead ring one hall further. The level is sixteen halls long.
    c.ok(
        "grid_set_stream",
        json!({"draw_far": 10, "active_radius": 8, "lead_radius": 6}),
    );

    let rooms = c.ok("grid_rooms", json!({})).text;
    assert!(rooms.contains("Hall 15"), "{rooms}");
    let cook = c.ok("cook_check", json!({})).text;
    assert!(cook.contains("cook: OK"), "{cook}");
    let budget = c.ok("grid_budget", json!({})).text;
    assert!(budget.contains("validate: no findings"), "{budget}");
    if out.is_some() {
        c.ok("save", json!({}));
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
