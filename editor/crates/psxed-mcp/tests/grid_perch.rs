//! An arch made with `grid_arch_span`, perched on in a headless Play.
//!
//! The geometry stage always runs: build the scene through the MCP (raise the
//! ceiling, span an arch between two floor points, put the player south of it)
//! and check that the world cooks, that the arch cooks to a hook point under
//! its crown and that the saved project round-trips. The play stage is opt in
//! (`PSXED_MCP_VERIFY=1`, a built `target/release/frontend`, and
//! `PSOXIDE_GUEST_STAGE_ROOT` at a private directory): it cooks a disc, plays
//! a route that swaps to the ranged stance, aims at the crown (the camera
//! pitched up with the right stick), fires the hook, waits to be attached,
//! drops with Circle and falls to the floor, and asserts from the guest's own
//! `arch:attached` and `arch:drop` lines and the tracked path.
//! `PSXED_MCP_PNG_DIR` names where the frames go.

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
fn an_arch_spans_two_points_cooks_a_hook_under_the_crown_and_is_perched_on() {
    let repo = repo_root();
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-perch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let projects = projects_dir(&tmp);
    let project = projects.join("perch");
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

    // ---- the scene: a tall room, an arch across it, the player south of it ----
    c.ok(
        "grid_set_heights",
        json!({"kind": "ceiling", "from": [0, 0], "to": [5, 5], "height": 4096}),
    );
    let arch = c
        .ok(
            "grid_arch_span",
            json!({
                "from": [2048, 0, 2048], "to": [4096, 0, 2048],
                "material": "Graybox / Cliff 64",
                "spring": 1024, "rise": 1024, "thickness": 256, "depth": 1024
            }),
        )
        .text;
    assert!(arch.contains("span 2048"), "{arch}");
    assert!(arch.contains("hook socket"), "{arch}");
    // On the arch's axis, 240 runtime units south of it.
    c.ok(
        "move_node",
        json!({"node": "Aletha (Player)", "position": [3840, 64, 6400]}),
    );
    let cook = c.ok("cook_check", json!({})).text;
    assert!(cook.contains("cook: OK"), "{cook}");
    // The haunch pieces over-block and the cook says so, once.
    assert!(cook.contains("over-blocks"), "{cook}");

    let shots = c.ok(
        "screenshot",
        json!({"shots": [
            {"label": "the arch from the south", "position": [4000, 500, 9000], "look_at": [3840, 350, 2560]},
            {"label": "from above", "position": [3840, 3500, 5200], "look_at": [3840, 200, 2560]}
        ]}),
    );
    if frontend.is_file() {
        assert_eq!(shots.images.len(), 2, "{}", shots.text);
        keep(&shots.images[0], "mesh-arch-preview-front");
    }

    // ---- the saved project holds the arch and its hook cooks -----------------
    c.ok("save", json!({}));
    let doc = psxed_project::ProjectDocument::load_from_path(project.join("project.ron"))
        .expect("the saved project loads");
    let world = doc.active_scene().world.grid().expect("a grid world");
    assert_eq!(world.mesh_assets.len(), 1);
    assert_eq!(world.rooms[0].meshes.len(), 1);
    let (package, report) = psxed_project::playtest::build_package(&doc, &project);
    assert!(report.is_ok(), "{:?}", report.error_messages());
    let package = package.expect("a package");
    let hooks: Vec<_> = package
        .entities
        .iter()
        .filter(|e| e.kind == psxed_project::playtest::PlaytestEntityKind::HookPoint)
        .collect();
    assert_eq!(hooks.len(), 1);
    // At the arch's centre (240, 160 runtime units), 96 under the grip a
    // little under the crown.
    assert_eq!((hooks[0].x, hooks[0].z), (240, 160));
    assert_eq!(hooks[0].y, 62);
    assert_eq!(hooks[0].flags, 2, "HOOK_BARE: the arch is the masonry, no floating ruin");

    // ---- PLAY: perch on the crown and drop off ------------------------------
    if std::env::var("PSXED_MCP_VERIFY").as_deref() == Ok("1") {
        assert!(frontend.is_file(), "PSXED_MCP_VERIFY needs {}", frontend.display());
        // Triangle swaps to the ranged stance (72 ticks), L2 aims, the right
        // stick up raises the camera until the crown is at the reticle, R2
        // fires the hook, Circle lets go.
        let run = c.ok(
            "playtest",
            json!({
                "route": [
                    {"polls": 4, "buttons": ["triangle"]},
                    {"polls": 90},
                    {"polls": 6, "buttons": ["l2"]},
                    {"polls": 44, "buttons": ["l2"], "camera": [0.0, 1.0]},
                    {"polls": 21, "buttons": ["l2"]},
                    {"polls": 4, "buttons": ["l2", "r2"]},
                    {"polls": 110},
                    {"polls": 4, "buttons": ["circle"]},
                    {"polls": 90}
                ],
                "warmup_polls": 450,
                "frames": [60, 150, 185, 280, 320, 360],
                "track_player": true,
                "telemetry": false
            }),
        );
        println!("{}", run.text);
        assert!(!run.text.contains("stalled"), "{}", run.text);
        let events: Vec<(u64, String)> = run
            .text
            .lines()
            .find(|l| l.starts_with("hook events"))
            .expect("hook events in the report")
            .trim_start_matches("hook events (route poll):")
            .split(',')
            .filter_map(|e| {
                let e = e.trim();
                let (name, poll) = e.split_once(" at ")?;
                Some((poll.parse().ok()?, name.to_string()))
            })
            .collect();
        let attached = events
            .iter()
            .find(|(_, n)| n == "arch:attached")
            .expect("the player attached to the crown");
        let dropped = events
            .iter()
            .find(|(_, n)| n == "arch:drop")
            .expect("the player let go");
        assert!(attached.0 < dropped.0, "{events:?}");
        assert!(!events.iter().any(|(_, n)| n == "arch:blocked"), "{events:?}");
        // The path: up to the crown (an editor unit is a sixteenth of a
        // runtime unit; the hang is about 77 units up, 1232 editor), then
        // down to the floor.
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
        let highest = path.iter().map(|p| p[1]).max().unwrap();
        assert!(highest > 1000, "hung under the crown: {path:?}");
        let end = path.last().unwrap();
        assert!(end[1].abs() < 40, "fell back to the floor: {end:?}");
        // Under the arch: z within its depth (2560 +- 640 editor).
        assert!(
            path.iter().any(|p| p[1] > 1000 && (p[2] - 2560).abs() < 700),
            "{path:?}"
        );
        for (i, png) in run.images.iter().enumerate() {
            keep(png, &format!("mesh-perch-{i:02}"));
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
