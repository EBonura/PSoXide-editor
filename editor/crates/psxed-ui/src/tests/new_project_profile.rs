//! File > New Project, clicked through in the drawn editor: the world it makes
//! carries the standard world profile (flood streaming, facing shade, global
//! light), so new projects start on the same footing.

use super::grid_ui::{draw_with_events, full_context};
use super::*;

/// Centre of the first drawn text for which `pick` holds.
fn find_text(output: &egui::FullOutput, pick: impl Fn(&str) -> bool) -> Option<Pos2> {
    output.shapes.iter().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if pick(t.galley.text()) => {
            Some(t.pos + t.galley.size() / 2.0)
        }
        _ => None,
    })
}

/// One real click: move, press, release, each in its own frame.
fn click(ws: &mut EditorWorkspace, ctx: &egui::Context, at: Pos2) -> egui::FullOutput {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    draw_with_events(ws, ctx, vec![egui::Event::PointerMoved(at)]);
    draw_with_events(ws, ctx, vec![button(true)]);
    draw_with_events(ws, ctx, vec![button(false)])
}

#[test]
fn clicking_through_new_project_makes_a_world_with_the_standard_profile() {
    let name = format!("Profile Click {}", std::process::id());
    let dir = psxed_project::projects_dir().join(psxed_project::project_file_stem(&name));
    let _scratch = ScratchProjectDir::new(dir.clone());
    let ctx = full_context();
    let mut ws = EditorWorkspace::with_project(
        test_temp_dir("new-project-profile"),
        psxed_project::ProjectDocument::starter(),
    );
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);

    // File menu, then New Project...
    let screen = draw_with_events(&mut ws, &ctx, Vec::new());
    let file = find_text(&screen, |t| t == "File").expect("a File menu");
    click(&mut ws, &ctx, file);
    let menu = draw_with_events(&mut ws, &ctx, Vec::new());
    let entry = find_text(&menu, |t| t.contains("New Project")).expect("New Project... entry");
    click(&mut ws, &ctx, entry);

    // The dialog: type the name, then click Create.
    let dialog = draw_with_events(&mut ws, &ctx, Vec::new());
    assert!(
        find_text(&dialog, |t| t == "Create").is_some(),
        "the New Project dialog is open"
    );
    draw_with_events(&mut ws, &ctx, vec![egui::Event::Text(name.clone())]);
    let typed = draw_with_events(&mut ws, &ctx, Vec::new());
    let create = find_text(&typed, |t| t == "Create").expect("a Create button");
    click(&mut ws, &ctx, create);

    assert!(dir.join("project.ron").is_file(), "{}", ws.status);
    let world = ws
        .project
        .active_scene()
        .world
        .grid()
        .expect("a grid world")
        .clone();
    let standard = psxed_project::grid::WorldProfile::STANDARD;
    assert_eq!(world.shading, standard.shading);
    assert_eq!(world.global_light, standard.global_light);
    let mut stream = standard.stream;
    stream.flood.panels = world.stream.flood.panels;
    assert_eq!(world.stream, stream);
    assert!(world.stream.flood.on, "flood streaming is on");

    // What is on disk carries it too.
    let saved = ProjectDocument::load_from_path(dir.join("project.ron")).unwrap();
    assert_eq!(saved.active_scene().world.grid(), Some(&world));
}
