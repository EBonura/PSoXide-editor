//! Golden screenshots of the grid editor at 1920 x 1080.
//!
//! Each key screen is drawn through the real `EditorWorkspace::draw`, rasterised
//! on the CPU (`raster.rs`, no GPU, so every machine gets the same pixels) and
//! compared with a PNG in `editor/crates/psxed-ui/golden/`. A layout change
//! that moves, hides or resizes anything fails here with the actual and a diff
//! image written to the temp directory.
//!
//! The 3D viewport is a flat placeholder (the real picture is the GPU preview
//! of the frontend); everything the editor itself draws is in the image: the
//! docks, the strips, the plan view, the overlays and the inspectors.
//!
//! Intended change: `make ui-bless` (or `PSXED_BLESS=1 cargo test -p psxed-ui
//! --lib grid_screens`) rewrites the goldens; review the PNG diff in git.
//!
//! Two worlds: `grid-starter` (the tracked starter project) and a stand-in for
//! Manny's dp-world (650 palette materials named `DP p.c`, a bigger hall, a
//! few lights), built in code because dp-world is not a tracked project.

use super::grid_rig::*;
use super::raster::{render, Frame, Textures};
use super::*;

const W: usize = 1920;
const H: usize = 1080;
/// A pixel differs when any channel is further apart than this.
const CHANNEL_TOLERANCE: u8 = 12;
/// A shot passes with at most this many differing pixels (a one-word text
/// change is a few hundred; font rounding on another platform is a handful).
const MAX_DIFFERING_PIXELS: usize = 120;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("golden")
}

pub(super) fn fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "lucide".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../../../emu/crates/frontend/assets/fonts/lucide.ttf"
        ))),
    );
    // As the app binds it: its own family, and a fallback of the text ones.
    fonts.families.insert(
        egui::FontFamily::Name("lucide".into()),
        vec!["lucide".into()],
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("lucide".into());
    }
    fonts
}

/// Draw `ws` four frames (layout and animations settle) and return the last.
fn rasterise(ws: &mut EditorWorkspace) -> Frame {
    let ctx = egui::Context::default();
    ctx.set_fonts(fonts());
    let texture = ctx.load_texture(
        "screens-viewport",
        egui::ColorImage::new([640, 480], egui::Color32::from_rgb(16, 20, 28)),
        egui::TextureOptions::NEAREST,
    );
    let viewport = EditorViewport3dPresentation::edit(texture.id(), Vec::new());
    let mut textures = Textures::default();
    let mut last = None;
    for frame in 0..4u32 {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(W as f32, H as f32),
            )),
            time: Some(f64::from(frame) / 60.0),
            focused: true,
            ..egui::RawInput::default()
        };
        let out = ctx.run(input, |ctx| {
            ws.draw(ctx, viewport.clone(), EditorPlaytestStatus::Idle);
        });
        textures.apply(&out.textures_delta);
        last = Some(out);
    }
    let out = last.unwrap();
    let primitives = ctx.tessellate(out.shapes, out.pixels_per_point);
    render(&primitives, &textures, W, H)
}

pub(super) fn save_png(path: &Path, frame: &Frame) {
    let rgb: Vec<u8> = frame
        .rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = std::fs::File::create(path).unwrap();
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        std::io::BufWriter::new(file),
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::Adaptive,
    );
    image::ImageEncoder::write_image(
        encoder,
        &rgb,
        frame.width as u32,
        frame.height as u32,
        image::ExtendedColorType::Rgb8,
    )
    .unwrap();
}

/// Compare with the golden of `name`; on a mismatch write the actual and a
/// diff image next to the other test scratch files and fail.
fn check(name: &str, frame: &Frame) {
    let path = golden_dir().join(format!("{name}.png"));
    if std::env::var_os("PSXED_BLESS").is_some() {
        save_png(&path, frame);
        return;
    }
    let golden = image::open(&path)
        .unwrap_or_else(|e| {
            panic!(
                "no golden {}: {e}\nrun `make ui-bless` to create it",
                path.display()
            )
        })
        .to_rgba8();
    assert_eq!(
        (golden.width() as usize, golden.height() as usize),
        (frame.width, frame.height),
        "{name}: the golden has another size"
    );
    let mut differing = 0usize;
    let mut diff = Frame {
        width: frame.width,
        height: frame.height,
        rgba: Vec::with_capacity(frame.rgba.len()),
    };
    for (got, want) in frame.rgba.chunks_exact(4).zip(golden.pixels()) {
        let off = got
            .iter()
            .zip(want.0.iter())
            .take(3)
            .any(|(a, b)| a.abs_diff(*b) > CHANNEL_TOLERANCE);
        if off {
            differing += 1;
            diff.rgba.extend_from_slice(&[255, 0, 0, 255]);
        } else {
            diff.rgba
                .extend_from_slice(&[want.0[0] / 3, want.0[1] / 3, want.0[2] / 3, 255]);
        }
    }
    if differing > MAX_DIFFERING_PIXELS {
        let dir = std::env::temp_dir().join("psxed-ui-screens");
        let (actual, diff_path) = (
            dir.join(format!("{name}.actual.png")),
            dir.join(format!("{name}.diff.png")),
        );
        save_png(&actual, frame);
        save_png(&diff_path, &diff);
        panic!(
            "{name}: {differing} pixels differ (more than {MAX_DIFFERING_PIXELS}).\n\
             golden {}\nactual {}\ndiff   {}\n\
             If the change is intended: `make ui-bless` and commit the PNGs.",
            path.display(),
            actual.display(),
            diff_path.display()
        );
    }
}

// ---- the two worlds ----

enum World {
    Starter,
    DpLike,
}

impl World {
    fn tag(&self) -> &'static str {
        match self {
            World::Starter => "grid-starter",
            World::DpLike => "dp-world",
        }
    }

    fn light(&self) -> &'static str {
        match self {
            World::Starter => "Starter Light",
            World::DpLike => "Point Light",
        }
    }
}

/// A workspace for the world, plus the guard that removes its directory.
fn open(world: &World) -> (EditorWorkspace, ScratchProjectDir) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "psxed-ui-screens-{}-{}-{}",
        world.tag(),
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let guard = ScratchProjectDir::new(dir.clone());
    let mut ws = match world {
        World::Starter => {
            let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../projects/grid-starter");
            crate::starter_catalogue::copy_dir_recursive(&source, &dir).unwrap();
            EditorWorkspace::open_directory(&dir).unwrap()
        }
        World::DpLike => {
            let mut ws = EditorWorkspace::with_project(dir.clone(), dp_like());
            ws.place_kind = PlaceKind::PointLightMarker;
            for (x, z) in [(2.5, 2.5), (6.5, 3.5), (9.5, 8.5)] {
                ws.place_at_world_hit([x * S * 1.25, 0.0, z * S * 1.25]);
            }
            ws.clear_node_selection_state();
            ws
        }
    };
    ws.active_workspace = WorkspaceView::Room;
    ws.frame_current_view();
    ws.status = String::new();
    (ws, guard)
}

/// Stand-in for dp-world: one 12 x 12 hall with a few raised corners and 650
/// materials, `DP p.c` in groups, beside the Graybox set.
fn dp_like() -> ProjectDocument {
    let mut project = flat_project(12, 12, true);
    let mut add = |name: String, n: usize| {
        project.add_resource(
            &name,
            psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(Some(
                format!("level_textures/{n}.psxt"),
            ))),
        );
    };
    for (i, name) in ["Ground 64", "Wall 64", "Ceiling 64"].iter().enumerate() {
        add(format!("Graybox / {name}"), i);
    }
    for piece in 0..108 {
        for cell in 0..6 {
            add(format!("DP {piece}.{cell}"), 10 + piece * 6 + cell);
        }
    }
    project
}

// ---- the screens ----

#[derive(Clone, Copy)]
enum Screen {
    World3d,
    Plan,
    Both,
    MaterialTab,
    AssetsDrawer,
    TriangleSelected,
    LightSelected,
    NothingSelected,
}

impl Screen {
    const ALL: [Screen; 8] = [
        Screen::World3d,
        Screen::Plan,
        Screen::Both,
        Screen::MaterialTab,
        Screen::AssetsDrawer,
        Screen::TriangleSelected,
        Screen::LightSelected,
        Screen::NothingSelected,
    ];

    fn tag(self) -> &'static str {
        match self {
            Screen::World3d => "world-3d",
            Screen::Plan => "plan",
            Screen::Both => "both",
            Screen::MaterialTab => "material-tab",
            Screen::AssetsDrawer => "assets-drawer",
            Screen::TriangleSelected => "triangle-selected",
            Screen::LightSelected => "light-selected",
            Screen::NothingSelected => "nothing-selected",
        }
    }
}

fn shoot(world: &World, screen: Screen) {
    let (mut ws, _guard) = open(world);
    let pose = match screen {
        Screen::World3d | Screen::NothingSelected => "view=3d".to_string(),
        Screen::Plan => "view=plan".to_string(),
        Screen::Both => "view=split".to_string(),
        Screen::MaterialTab => "view=3d".to_string(),
        Screen::AssetsDrawer => "view=3d;resources=on".to_string(),
        Screen::TriangleSelected => "view=split;tool=select;select=triangle:2,2,0".to_string(),
        Screen::LightSelected => {
            format!("view=3d;tool=select;select=node:{}", world.light())
        }
    };
    ws.apply_grid_pose(&pose).unwrap();
    if matches!(screen, Screen::MaterialTab) {
        ws.active_workspace = WorkspaceView::Material;
    }
    let frame = rasterise(&mut ws);
    check(&format!("{}-{}", world.tag(), screen.tag()), &frame);
}

macro_rules! screens {
    ($($name:ident: $world:expr, $screen:expr;)*) => {
        $(#[test] fn $name() { shoot(&$world, $screen); })*
    };
}

screens! {
    starter_world_3d: World::Starter, Screen::World3d;
    starter_plan: World::Starter, Screen::Plan;
    starter_both: World::Starter, Screen::Both;
    starter_material_tab: World::Starter, Screen::MaterialTab;
    starter_assets_drawer: World::Starter, Screen::AssetsDrawer;
    starter_triangle_selected: World::Starter, Screen::TriangleSelected;
    starter_light_selected: World::Starter, Screen::LightSelected;
    starter_nothing_selected: World::Starter, Screen::NothingSelected;
    dp_world_3d: World::DpLike, Screen::World3d;
    dp_plan: World::DpLike, Screen::Plan;
    dp_both: World::DpLike, Screen::Both;
    dp_material_tab: World::DpLike, Screen::MaterialTab;
    dp_assets_drawer: World::DpLike, Screen::AssetsDrawer;
    dp_triangle_selected: World::DpLike, Screen::TriangleSelected;
    dp_light_selected: World::DpLike, Screen::LightSelected;
    dp_nothing_selected: World::DpLike, Screen::NothingSelected;
}

/// Every screen has a test above: adding one to `Screen::ALL` without a
/// golden fails here instead of passing silently.
#[test]
fn every_screen_has_a_golden() {
    if std::env::var_os("PSXED_BLESS").is_some() {
        return;
    }
    for world in [World::Starter, World::DpLike] {
        for screen in Screen::ALL {
            let path = golden_dir().join(format!("{}-{}.png", world.tag(), screen.tag()));
            assert!(path.exists(), "missing golden {}", path.display());
        }
    }
}
