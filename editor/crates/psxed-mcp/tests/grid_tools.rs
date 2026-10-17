//! Every geometry tool answers, over the stdio protocol, on a fresh New Grid
//! Project. `grid_level.rs` builds a whole level; this runs each remaining
//! tool once and checks the numbers it reports against what a read tool says
//! afterwards.

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
    let project = projects.join("tools");
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
fn each_geometry_tool_changes_what_the_read_tools_report() {
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-tools-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let project = new_project(&tmp);
    let mut c = Client::start(&["--project", project.to_str().unwrap()], &[], &repo_root());

    // ---- height maps, sectors and points ---------------------------------
    let sector = c.ok("grid_sector", json!({"cell": [0, 0]})).text;
    assert!(sector.contains("derived wall north closure"), "{sector}");
    assert!(sector.contains("derived wall west closure"), "{sector}");
    let at = c.ok("grid_height_at", json!({"point": [3000, 3000]})).text;
    assert!(at.contains("room 0: floor 0 editor units"), "{at}");
    assert!(
        c.call("grid_sector", json!({"cell": [40, 40]})).is_err(),
        "outside the room"
    );

    // ---- a platform: surface, heights, and the risers that follow --------
    c.ok(
        "grid_add_surface",
        json!({"kind": "floor", "from": [6, 0], "to": [7, 1], "height": 0}),
    );
    c.ok(
        "grid_set_heights",
        json!({"kind": "floor", "from": [6, 0], "to": [7, 1], "height": 500}),
    );
    let map = c
        .ok("grid_heights", json!({"from": [5, 0], "to": [8, 1]}))
        .text;
    let row0: Vec<&str> = map
        .lines()
        .find(|l| l.trim_start().starts_with("0 "))
        .expect("row z=0")
        .split_whitespace()
        .collect();
    assert_eq!(
        row0,
        ["0", "0", "2", "2", "."],
        "the platform is two clicks up: {map}"
    );
    let snapped = c
        .ok(
            "grid_set_heights",
            json!({"kind": "floor", "from": [6, 0], "to": [6, 0], "height": 320, "snap": "fine"}),
        )
        .text;
    assert!(snapped.contains("320"), "{snapped}");
    // A slope from one edge: the north edge up, the south edge down.
    c.ok(
        "grid_set_heights",
        json!({"kind": "floor", "from": [7, 1], "to": [7, 1], "edge": "north", "height": 768}),
    );
    let sector = c.ok("grid_sector", json!({"cell": [7, 1]})).text;
    assert!(
        sector.contains("[512, 512, 768, 768]") || sector.contains("768"),
        "{sector}"
    );
    let bent = c
        .ok(
            "grid_set_split",
            json!({"kind": "floor", "from": [7, 1], "split": "ne_sw"}),
        )
        .text;
    assert!(bent.contains("flipped the diagonal of 1 floor"), "{bent}");
    c.ok("grid_set_split", json!({"kind": "floor", "from": [7, 1]}));
    let walls = c
        .ok(
            "grid_walls",
            json!({"from": [5, 0], "to": [5, 1], "edge": "east"}),
        )
        .text;
    assert!(
        walls.contains("riser"),
        "the platform's edge has a riser: {walls}"
    );

    // ---- authored walls: place, then remove -------------------------------
    let placed = c
        .ok("grid_place_wall", json!({"from": [1, 3], "to": [3, 3], "edge": "north", "material": "Graybox / Cliff 64"}))
        .text;
    assert!(placed.contains("placed 3"), "{placed}");
    let covered = c
        .ok(
            "grid_place_wall",
            json!({"from": [0, 0], "to": [2, 0], "edge": "north"}),
        )
        .text;
    assert!(
        covered.contains("placed 0") && covered.contains("covered"),
        "{covered}"
    );
    let sector = c.ok("grid_sector", json!({"cell": [2, 3]})).text;
    assert!(sector.contains("authored wall north 0"), "{sector}");
    let wall = json!({"cell": [2, 3], "edge": "north", "index": 0});
    let mut paint = wall.clone();
    paint["material"] = json!("DP City / Bright Blue Signal");
    paint["run"] = json!(true);
    assert!(
        c.ok("grid_paint_wall", paint)
            .text
            .contains("3 wall segment"),
        "the run is three walls"
    );
    let mut flag = wall.clone();
    flag["solid"] = json!(false);
    flag["facing"] = json!("both");
    c.ok("grid_wall_override", flag);
    let sector = c.ok("grid_sector", json!({"cell": [2, 3]})).text;
    assert!(
        sector.contains("facing Both") && sector.contains("solid false"),
        "{sector}"
    );
    let gone = c
        .ok(
            "grid_remove_wall",
            json!({"from": [1, 3], "to": [3, 3], "edge": "north"}),
        )
        .text;
    assert!(gone.contains("removed 3"), "{gone}");

    // ---- tiling: a fit patch, a world density, a material default ---------
    let fit = c
        .ok(
            "grid_tiling",
            json!({"from": [0, 0], "faces": ["floor"], "mode": "fit", "repeats": [8, 8]}),
        )
        .text;
    assert!(
        fit.contains("8 x 8 on this face") && fit.contains("warning"),
        "an 8 x 8 fit is cut: {fit}"
    );
    let world = c
        .ok("grid_tiling", json!({"from": [0, 0], "to": [1, 1], "faces": ["floor"], "mode": "world", "density": [64, 64], "turns": 2, "flip_v": true}))
        .text;
    assert!(world.contains("tiles on this face"), "{world}");
    let reset = c
        .ok(
            "grid_tiling",
            json!({"from": [0, 0], "to": [1, 1], "faces": ["floor"], "reset": true}),
        )
        .text;
    assert!(reset.contains("changed"), "{reset}");
    let default = c
        .ok(
            "grid_material_tiling",
            json!({"material": "Graybox / Ground 64", "mode": "fit", "repeats": [2, 2]}),
        )
        .text;
    assert!(default.contains("Fit"), "{default}");
    assert!(
        c.call("grid_tiling", json!({"from": [0, 0]})).is_err(),
        "nothing to change is an error"
    );
    assert!(c
        .call(
            "grid_tiling",
            json!({"from": [0, 0], "mode": "fit", "repeats": [0, 1]})
        )
        .is_err());

    // ---- bands: cut, move, delete ------------------------------------------
    let north = json!({"cell": [2, 0], "edge": "north"});
    let mut cut = north.clone();
    cut["anchor"] = json!("abs");
    cut["value"] = json!(1024);
    c.ok("grid_band_cut", cut);
    let mut mv = north.clone();
    mv["line"] = json!(0);
    mv["value"] = json!(1280);
    c.ok("grid_band_move", mv);
    assert!(c.ok("grid_bands", north.clone()).text.contains("Abs(1280)"));
    let mut del = north.clone();
    del["line"] = json!(0);
    c.ok("grid_band_delete", del);
    assert!(c
        .call(
            "grid_band_delete",
            json!({"cell": [2, 0], "edge": "north", "line": 3})
        )
        .is_err());

    // ---- planes: add, change, delete ----------------------------------------
    c.ok("grid_add_plane", json!({"center": [2560, 1024, 2560], "size": [1024, 1024], "orientation": "wall", "collision": "solid"}));
    let planes = c.ok("grid_planes", json!({})).text;
    assert!(planes.contains("wall segment"), "{planes}");
    let changed = c
        .ok(
            "grid_set_plane",
            json!({"index": 0, "collision": "decor", "double_sided": true, "camera_pass": true}),
        )
        .text;
    assert!(
        changed.contains("both sides") && changed.contains("no collision"),
        "{changed}"
    );
    assert!(c
        .ok("grid_delete_plane", json!({"indices": [0]}))
        .text
        .contains("removed 1"));
    assert!(c.call("grid_set_plane", json!({"index": 5})).is_err());

    // ---- rooms: add, change, convert, delete; world shading ------------------
    c.ok(
        "grid_add_room",
        json!({"name": "Loft", "origin": [0, 8], "y_offset": 1536, "ambient": [128, 128, 128]}),
    );
    c.ok("grid_set_room", json!({"room": 1, "name": "Upper loft", "y_offset": 2048, "open_wall": 1024, "wall_material": "Graybox / Cliff 64"}));
    let rooms = c.ok("grid_rooms", json!({})).text;
    assert!(
        rooms.contains("Upper loft") && rooms.contains("y_offset 2048"),
        "{rooms}"
    );
    assert!(c
        .ok("grid_convert_room", json!({"room": 1}))
        .text
        .contains("already derived"));
    assert!(
        c.call("grid_set_room", json!({"room": 1})).is_err(),
        "nothing to change is an error"
    );
    c.ok("grid_delete_room", json!({"room": 1}));
    assert!(
        c.call("grid_delete_room", json!({"room": 0})).is_err(),
        "the last room stays"
    );
    let shade = c
        .ok(
            "grid_set_shading",
            json!({"preset": "strong", "side_z": 100}),
        )
        .text;
    assert!(shade.contains("side_z 100"), "{shade}");
    c.ok(
        "grid_remove_surface",
        json!({"kind": "floor", "from": [6, 0], "to": [7, 1]}),
    );
    let map = c
        .ok("grid_heights", json!({"from": [5, 0], "to": [8, 1]}))
        .text;
    assert!(map.contains('.'), "{map}");

    // ---- validation and the cook see all of it --------------------------------
    let budget = c.ok("grid_budget", json!({})).text;
    assert!(!budget.contains("error:"), "{budget}");
    assert!(c.ok("cook_check", json!({})).text.contains("cook: OK"));

    // ---- nothing is on disk until save; revert drops it all --------------------
    let status = c.ok("status", json!({})).text;
    assert!(
        status.contains("staged edits:") && status.contains("unsaved"),
        "{status}"
    );
    c.ok("revert", json!({}));
    let rooms = c.ok("grid_rooms", json!({})).text;
    assert!(
        rooms.contains("1 room(s)") && !rooms.contains("Upper loft"),
        "{rooms}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}
