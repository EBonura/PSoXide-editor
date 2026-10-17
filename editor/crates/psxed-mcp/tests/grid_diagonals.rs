//! Diagonal terrain in Play: a plateau whose corners are cut at 45 degrees,
//! walked along and walked off, and a void pillar the character cannot enter.
//!
//! The geometry stages always run; `PSXED_MCP_VERIFY=1` (with
//! `PSOXIDE_GUEST_STAGE_ROOT` set to a private directory) cooks a disc and plays
//! two routes with the real character motor. `PSXED_MCP_PNG_DIR` keeps the
//! pictures.

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

fn keep(png: &[u8], name: &str) {
    let Some(dir) = std::env::var_os("PSXED_MCP_PNG_DIR") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("create the picture directory");
    let temp = dir.join(format!(".{name}.tmp"));
    std::fs::write(&temp, png).expect("write the picture");
    std::fs::rename(&temp, dir.join(format!("{name}.png"))).expect("move the picture");
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
    let project = projects.join("diagonals");
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

fn path_of(text: &str) -> Vec<[i32; 3]> {
    text.lines()
        .find(|l| l.starts_with("path, route poll"))
        .expect("a path line")
        .split_whitespace()
        .filter_map(|tok| tok.split_once(":[").map(|(_, v)| v.trim_end_matches(']')))
        .filter_map(|v| {
            let n: Vec<i32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
            (n.len() == 3).then(|| [n[0], n[1], n[2]])
        })
        .collect()
}

#[test]
fn a_diagonal_plateau_edge_and_a_void_pillar_hold_in_play() {
    let repo = repo_root();
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-diag-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let project = new_project(&tmp);
    let frontend = repo.join("target/release/frontend");
    let scratch = tmp.join("scratch");
    let mut args = vec![
        "--project".to_string(),
        project.to_string_lossy().into_owned(),
        "--scratch".to_string(),
        scratch.to_string_lossy().into_owned(),
    ];
    if frontend.is_file() {
        args.push("--frontend".to_string());
        args.push(frontend.to_string_lossy().into_owned());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut c = Client::start(&arg_refs, &[], &repo);

    // A 16 by 16 roofed room with a plateau three clicks up in the middle, its
    // four corners cut at 45 degrees (the cut half drops back to the floor).
    c.ok(
        "grid_remove_surface",
        json!({"kind": "floor", "from": [0, 0], "to": [5, 5]}),
    );
    c.ok(
        "grid_remove_surface",
        json!({"kind": "ceiling", "from": [0, 0], "to": [5, 5]}),
    );
    c.ok(
        "grid_set_room",
        json!({"wall_material": "Graybox / Cliff 64"}),
    );
    c.ok(
        "grid_polygon",
        json!({"points": [[0.0, 0.0], [16.0, 0.0], [16.0, 16.0], [0.0, 16.0]], "chamfer": "none",
               "floor_height": 0, "ceiling_height": 4096, "material": "Graybox / Ground 64",
               "ceiling_material": "Graybox / Cliff 64"}),
    );
    c.ok(
        "grid_raise",
        json!({"from": [5, 5], "to": [10, 10], "amount": 768, "follow_ceiling": false}),
    );
    let cut = c
        .ok(
            "grid_chamfer",
            json!({"from": [5, 5], "to": [10, 10], "mode": "crack", "drop": 768}),
        )
        .text;
    assert!(cut.contains("4 convex corner(s) cut"), "{cut}");
    // The cut corners are split-level cells whose halves differ by a cliff:
    // a derived diagonal face and a one-sided wall record each.
    let walls = c
        .ok("grid_walls", json!({"from": [5, 5], "to": [10, 10]}))
        .text;
    assert_eq!(walls.matches(" riser |").count(), 4, "{walls}");
    assert_eq!(walls.matches("diagonal N").count(), 4, "{walls}");
    assert_eq!(walls.matches("one-sided cliff wall").count(), 4, "{walls}");
    // The straight sides of the plateau are cliffs too (hosted by the cells
    // around it).
    let around = c
        .ok("grid_walls", json!({"from": [4, 4], "to": [11, 11]}))
        .text;
    assert_eq!(
        around.matches("one-sided cliff wall").count(),
        4 + 4 * 4,
        "{around}"
    );

    // A void pillar: three by three cells, centre and sides empty, corners cut.
    for cell in [[2, 2], [1, 2], [3, 2], [2, 1], [2, 3]] {
        c.ok("grid_void", json!({"from": cell, "triangle": "both"}));
    }
    for (cell, split, tri) in [
        ([1, 1], "ne_sw", "b"),
        ([3, 1], "nw_se", "b"),
        ([1, 3], "nw_se", "a"),
        ([3, 3], "ne_sw", "a"),
    ] {
        c.ok(
            "grid_set_split",
            json!({"kind": "floor", "from": cell, "split": split}),
        );
        c.ok(
            "grid_set_split",
            json!({"kind": "ceiling", "from": cell, "split": split}),
        );
        c.ok("grid_void", json!({"from": cell, "triangle": tri}));
    }
    // The player starts south-west of the plateau's south-west corner cut
    // (cell [5, 10]). Editor units: a sector is 1280.
    c.ok(
        "move_node",
        json!({"node": "Aletha (Player)", "position": [5400, 64, 15000]}),
    );
    c.ok(
        "move_node",
        json!({"node": "Starter Light", "position": [8000, 3000, 8000]}),
    );
    let cook = c.ok("cook_check", json!({})).text;
    assert!(cook.starts_with("cook: OK"), "{cook}");

    let verify = std::env::var("PSXED_MCP_VERIFY").ok();
    if verify.is_some() {
        assert!(
            frontend.is_file(),
            "PSXED_MCP_VERIFY needs {}",
            frontend.display()
        );
        // The plateau corner from the low side, with the diagonal wall, and
        // the same view with the diagonal wall hidden: the hole before it
        // existed.
        let pose = json!([
            {"label": "the cut plateau corner", "position": [3200, 1500, 15000], "look_at": [8200, 500, 11500]},
            {"label": "the void pillar", "position": [800, 1300, 7200], "look_at": [3400, 500, 3400]}
        ]);
        let shots = c.ok("screenshot", json!({"shots": pose}));
        keep(&shots.images[0], "diag-plateau-corner-after");
        keep(&shots.images[1], "diag-void-pillar");
        let walls = c
            .ok("grid_walls", json!({"from": [5, 10], "to": [5, 10]}))
            .text;
        assert!(walls.contains("diagonal"), "{walls}");
        c.ok("checkpoint", json!({"name": "with-walls"}));
        c.ok(
            "grid_wall_override",
            json!({"cell": [5, 10], "edge": "nwse", "kind": "riser", "hidden": true}),
        );
        let hole = c.ok("screenshot", json!({"shots": [pose[0].clone()]}));
        keep(&hole.images[0], "diag-plateau-corner-before-hole");
        c.ok("restore", json!({"name": "with-walls"}));
    }
    if verify.as_deref() == Some("1") {
        // North-east into the cut corner from the low side: the cliff wall
        // stops the character at the diagonal, it never climbs.
        let into = c.ok(
            "playtest",
            json!({"route": [{"polls": 240, "dir": "forward_right"}],
                   "warmup_polls": 450, "frames": [0, 120, 239], "track_player": true}),
        );
        println!("{}", into.text);
        for (i, png) in into.images.iter().enumerate() {
            keep(png, &format!("diag-play-into-the-cut-corner-{i}"));
        }
        let path = path_of(&into.text);
        assert!(path.len() >= 3, "{path:?}");
        assert!(
            path.iter().all(|p| p[1] < 200),
            "the character stayed on the low floor: {path:?}"
        );
        let end = *path.last().unwrap();
        // The cut corner is cell [5, 10] (x 6400..7680, z 12800..14080): the
        // low half is south-west of the diagonal from (6400, 12800) to
        // (7680, 14080), x - z = -6400. The character stops on the low side
        // (its radius is 192, so the centre stays a little short of it).
        assert!(
            end[0] - end[2] <= -6400 + 40,
            "stopped before the diagonal: {end:?}"
        );
        assert!(
            path.iter().any(|p| p[0] - p[2] > -6400 - 700),
            "it walked up to the diagonal: {path:?}"
        );

        // The void pillar: from the south-west, north-east into the cell
        // whose far half is void. The diagonal closure wall stops it.
        c.ok(
            "move_node",
            json!({"node": "Aletha (Player)", "position": [700, 64, 6400]}),
        );
        let pillar = c.ok(
            "playtest",
            json!({"route": [{"polls": 240, "dir": "forward_right"}],
                   "warmup_polls": 450, "frames": [0, 119, 239], "track_player": true}),
        );
        println!("{}", pillar.text);
        for (i, png) in pillar.images.iter().enumerate() {
            keep(png, &format!("diag-play-into-the-void-{i}"));
        }
        let path = path_of(&pillar.text);
        assert!(path.len() >= 3, "{path:?}");
        // Cell [1, 3] (x 1280..2560, z 3840..5120) is cut along x - z = -2560;
        // its north-east half is void. Nothing of the path is past it.
        assert!(
            path.iter().all(|p| p[0] - p[2] <= -2560 + 40),
            "the character never entered the void half: {path:?}"
        );
        assert!(
            path.iter().any(|p| p[0] - p[2] > -2560 - 700),
            "it walked up to the void's wall: {path:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&tmp);
    let project = new_project(&tmp);
    let frontend = repo.join("target/release/frontend");
    let scratch = tmp.join("scratch");
    let mut args = vec![
        "--project".to_string(),
        project.to_string_lossy().into_owned(),
        "--scratch".to_string(),
        scratch.to_string_lossy().into_owned(),
    ];
    if frontend.is_file() {
        args.push("--frontend".to_string());
        args.push(frontend.to_string_lossy().into_owned());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut c = Client::start(&arg_refs, &[], &repo);

    // A 16 by 16 roofed room with a plateau three clicks up in the middle, its
    // four corners cut at 45 degrees (the cut half drops back to the floor).
    c.ok(
        "grid_remove_surface",
        json!({"kind": "floor", "from": [0, 0], "to": [5, 5]}),
    );
    c.ok(
        "grid_remove_surface",
        json!({"kind": "ceiling", "from": [0, 0], "to": [5, 5]}),
    );
    c.ok(
        "grid_set_room",
        json!({"wall_material": "Graybox / Cliff 64"}),
    );
    c.ok(
        "grid_polygon",
        json!({"points": [[0.0, 0.0], [16.0, 0.0], [16.0, 16.0], [0.0, 16.0]], "chamfer": "none",
               "floor_height": 0, "ceiling_height": 4096, "material": "Graybox / Ground 64",
               "ceiling_material": "Graybox / Cliff 64"}),
    );
    c.ok(
        "grid_raise",
        json!({"from": [5, 5], "to": [10, 10], "amount": 768, "follow_ceiling": false}),
    );
    let cut = c
        .ok(
            "grid_chamfer",
            json!({"from": [5, 5], "to": [10, 10], "mode": "crack", "drop": 768}),
        )
        .text;
    assert!(cut.contains("4 convex corner(s) cut"), "{cut}");
    // The cut corners are split-level cells whose halves differ by a cliff:
    // a derived diagonal face and a one-sided wall record each.
    let walls = c
        .ok("grid_walls", json!({"from": [5, 5], "to": [10, 10]}))
        .text;
    assert_eq!(walls.matches(" riser |").count(), 4, "{walls}");
    assert_eq!(walls.matches("diagonal N").count(), 4, "{walls}");
    assert_eq!(walls.matches("one-sided cliff wall").count(), 4, "{walls}");
    // The straight sides of the plateau are cliffs too (hosted by the cells
    // around it).
    let around = c
        .ok("grid_walls", json!({"from": [4, 4], "to": [11, 11]}))
        .text;
    assert_eq!(
        around.matches("one-sided cliff wall").count(),
        4 + 4 * 4,
        "{around}"
    );

    // A void pillar: three by three cells, centre and sides empty, corners cut.
    for cell in [[2, 2], [1, 2], [3, 2], [2, 1], [2, 3]] {
        c.ok("grid_void", json!({"from": cell, "triangle": "both"}));
    }
    for (cell, split, tri) in [
        ([1, 1], "ne_sw", "b"),
        ([3, 1], "nw_se", "b"),
        ([1, 3], "nw_se", "a"),
        ([3, 3], "ne_sw", "a"),
    ] {
        c.ok(
            "grid_set_split",
            json!({"kind": "floor", "from": cell, "split": split}),
        );
        c.ok(
            "grid_set_split",
            json!({"kind": "ceiling", "from": cell, "split": split}),
        );
        c.ok("grid_void", json!({"from": cell, "triangle": tri}));
    }
    // The player starts south-west of the plateau's south-west corner cut
    // (cell [5, 10]). Editor units: a sector is 1280.
    c.ok(
        "move_node",
        json!({"node": "Aletha (Player)", "position": [5400, 64, 15000]}),
    );
    c.ok(
        "move_node",
        json!({"node": "Starter Light", "position": [8000, 3000, 8000]}),
    );
    let cook = c.ok("cook_check", json!({})).text;
    assert!(cook.starts_with("cook: OK"), "{cook}");

    let verify = std::env::var("PSXED_MCP_VERIFY").ok();
    if verify.is_some() {
        assert!(
            frontend.is_file(),
            "PSXED_MCP_VERIFY needs {}",
            frontend.display()
        );
        // The plateau corner from the low side, with the diagonal wall, and
        // the same view with the diagonal wall hidden: the hole before it
        // existed.
        let pose = json!([
            {"label": "the cut plateau corner", "position": [3200, 1500, 15000], "look_at": [8200, 500, 11500]},
            {"label": "the void pillar", "position": [800, 1300, 7200], "look_at": [3400, 500, 3400]}
        ]);
        let shots = c.ok("screenshot", json!({"shots": pose}));
        keep(&shots.images[0], "diag-plateau-corner-after");
        keep(&shots.images[1], "diag-void-pillar");
        let walls = c
            .ok("grid_walls", json!({"from": [5, 10], "to": [5, 10]}))
            .text;
        assert!(walls.contains("diagonal"), "{walls}");
        c.ok("checkpoint", json!({"name": "with-walls"}));
        c.ok(
            "grid_wall_override",
            json!({"cell": [5, 10], "edge": "nwse", "kind": "riser", "hidden": true}),
        );
        let hole = c.ok("screenshot", json!({"shots": [pose[0].clone()]}));
        keep(&hole.images[0], "diag-plateau-corner-before-hole");
        c.ok("restore", json!({"name": "with-walls"}));
    }
    if verify.as_deref() == Some("1") {
        // North-east into the cut corner from the low side: the cliff wall
        // stops the character at the diagonal, it never climbs.
        let into = c.ok(
            "playtest",
            json!({"route": [{"polls": 240, "dir": "forward_right"}],
                   "warmup_polls": 450, "frames": [0, 120, 239], "track_player": true}),
        );
        println!("{}", into.text);
        for (i, png) in into.images.iter().enumerate() {
            keep(png, &format!("diag-play-into-the-cut-corner-{i}"));
        }
        let path = path_of(&into.text);
        assert!(path.len() >= 3, "{path:?}");
        assert!(
            path.iter().all(|p| p[1] < 200),
            "the character stayed on the low floor: {path:?}"
        );
        let end = *path.last().unwrap();
        // The plateau's corner cell (5, 10) spans x 6400..7680, z 12800..14080
        // (the cell that is cut); the diagonal there runs from (6400, 14080)
        // to (7680, 12800). The character stops on the low side of it.
        assert!(
            end[0] + end[2] < 6400 + 14080 + 40,
            "stopped before the diagonal: {end:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
