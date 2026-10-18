//! A two-storey room over the stdio protocol: a tall hall, a slab inserted
//! over it, a 2 by 2 hole cut through the slab and a ramp in the hole.
//!
//! The geometry stage always runs: the storeys and the columns read back,
//! the cook accepts the spans, and the saved project builds a package whose
//! room holds the extra span. The play stage is opt in (`PSXED_MCP_VERIFY=1`
//! and a built `target/release/frontend`, with `PSOXIDE_GUEST_STAGE_ROOT` at
//! a private directory): the player walks the ground floor under the slab
//! and the tracked path must never rise to the upper floor.

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

#[test]
fn a_two_storey_room_is_built_cooked_and_walked_on_its_ground_floor() {
    let repo = repo_root();
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-storeys-{}", std::process::id()));
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

    // ---- the hall: tall, then a slab 12 clicks up, then a hole in it ----------
    c.ok(
        "grid_set_heights",
        json!({"kind": "ceiling", "from": [0, 0], "to": [5, 5], "height": 6144}),
    );
    let slab = c
        .ok(
            "grid_insert_slab",
            json!({"rects": [[0, 0, 5, 5]], "y": 3072}),
        )
        .text;
    assert!(slab.contains("36 cell(s) changed"), "{slab}");
    let storeys = c.ok("grid_storeys", json!({})).text;
    assert!(storeys.contains("storey 0: floor at 0 clicks"), "{storeys}");
    assert!(
        storeys.contains("storey 1: floor at 12 clicks"),
        "{storeys}"
    );
    let column = c.ok("grid_column", json!({"cell": [0, 0]})).text;
    assert!(column.contains("2 span(s)"), "{column}");
    assert!(column.contains("span 1: floor 12 ceiling 24"), "{column}");
    assert!(
        column.contains("solid below it 1 clicks"),
        "the slab is one click thick: {column}"
    );

    let hole = c
        .ok("grid_cut_hole", json!({"rects": [[2, 2, 3, 3]], "y": 3072}))
        .text;
    assert!(hole.contains("4 cell(s) changed"), "{hole}");
    let open = c.ok("grid_column", json!({"cell": [2, 2]})).text;
    assert!(open.contains("1 span(s)"), "{open}");

    // A ramp rising through the hole, to the steepest slope the character walks.
    let ramp = c
        .ok(
            "grid_ramp",
            json!({"from": [2, 2], "to": [3, 3], "keep": "south", "walkable": true, "follow_ceiling": false}),
        )
        .text;
    assert!(!ramp.contains("too steep"), "{ramp}");

    // Fill and cut again through the same slab: the inverse round trip.
    let refill = c
        .ok(
            "grid_fill_hole",
            json!({"rects": [[2, 2, 3, 3]], "y": 3072}),
        )
        .text;
    assert!(refill.contains("4 cell(s) changed"), "{refill}");

    // A column of several spans refuses a surface verb rather than guessing.
    let refused = c
        .call(
            "grid_add_surface",
            json!({"kind": "floor", "from": [0, 0], "height": 512}),
        )
        .expect_err("a multi-span cell must be refused");
    assert!(refused.contains("has 2 spans"), "{refused}");
    assert!(
        refused.contains("storey"),
        "it names the way out: {refused}"
    );

    // Aimed at the upper storey, the same verb edits the span above the slab
    // and leaves the ground floor alone.
    c.ok(
        "grid_set_heights",
        json!({"kind": "floor", "from": [0, 0], "to": [0, 0], "height": 3328, "storey": 1}),
    );
    let lifted = c.ok("grid_column", json!({"cell": [0, 0]})).text;
    assert!(lifted.contains("span 0: floor 0 "), "{lifted}");
    assert!(lifted.contains("span 1: floor 13 "), "{lifted}");

    // ---- the player on the ground floor, south of the stair hole ---------------
    c.ok(
        "move_node",
        json!({"node": "Aletha (Player)", "position": [640, 0, 640]}),
    );
    let cook = c.ok("cook_check", json!({})).text;
    assert!(cook.contains("cook: OK"), "{cook}");
    c.ok("save", json!({}));
    let doc = psxed_project::ProjectDocument::load_from_path(project.join("project.ron"))
        .expect("load the saved project");
    let world = doc.scenes[0].world.grid().expect("a grid world");
    assert_eq!(world.rooms[0].sector(0, 0).unwrap().span_count(), 2);
    let (package, report) = psxed_project::playtest::build_package(&doc, &project);
    assert!(report.is_ok(), "{:?}", report.error_messages());
    assert!(package.is_some());

    // ---- PLAY: walk east along the ground row under the slab ------------------
    if std::env::var("PSXED_MCP_VERIFY").as_deref() == Ok("1") {
        assert!(
            frontend.is_file(),
            "PSXED_MCP_VERIFY needs {}",
            frontend.display()
        );
        let run = c.ok(
            "playtest",
            json!({
                "route": [{"polls": 500, "dir": "right"}],
                "warmup_polls": 450,
                "frames": [200, 400],
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
        // The upper floor is 3072 grid units, 3840 editor units: never reached.
        let highest = path.iter().map(|p| p[1]).max().unwrap();
        assert!(highest < 1000, "rose to the upper floor: {path:?}");
        // The walk went east under the slab.
        let (first, last) = (path.first().unwrap(), path.last().unwrap());
        assert!(
            last[0] > first[0] + 1000,
            "did not walk under the slab: {path:?}"
        );
    }
}
