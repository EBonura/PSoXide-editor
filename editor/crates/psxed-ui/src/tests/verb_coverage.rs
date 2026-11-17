//! The Character inspector's combat verb coverage table: every verb of the
//! contract as bound, fallback or missing, from the same rules the cook warns
//! with. Plus a headless shot of the inspector section (ignored; run with
//! `PSXED_VERB_SHOT_DIR=<dir> cargo test -p psxed-ui --lib verb_coverage -- --ignored`).

use super::grid_screens::{fonts, save_png};
use super::raster::{render, Textures};
use super::*;
use psx_level::combat_verbs::CombatKit;
use psxed_project::CharacterAnimationAction as A;

fn project(name: &str) -> ProjectDocument {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../projects")
        .join(name);
    ProjectDocument::load_from_path(dir.join("project.ron")).expect("project parses")
}

/// The context, the character and its set option for `character` in `project`.
fn character_on_set(
    project: &ProjectDocument,
    character: &str,
) -> (CharacterEditorContext, psxed_project::CharacterResource) {
    let resource = project
        .resources
        .iter()
        .find_map(|r| match &r.data {
            ResourceData::Character(c) if r.name == character => Some(c.clone()),
            _ => None,
        })
        .expect("character");
    (build_character_editor_context(project), resource)
}

fn cell(rows: &[VerbCoverageRow], action: A) -> VerbCoverageCell {
    rows.iter()
        .find(|row| row.action == action)
        .expect("verb")
        .cell
}

#[test]
fn the_table_shows_bound_fallback_and_missing_per_kit() {
    let project = project("grid-dungeon");
    let (ctx, aletha) = character_on_set(&project, "Aletha");
    let set = ctx.animation_set(aletha.animation_set).expect("set");
    let (kits, rows) =
        combat_verb_coverage_rows(&ctx.verb_bound(set, aletha.model), aletha.shoot_action());
    assert!(kits.can_swap() && kits.can_dodge());
    assert_eq!(
        cell(&rows, A::LightAttack),
        VerbCoverageCell::Bound(A::LightAttack)
    );
    // Her Knockdown clip is bound (2026-11-17); Parried still falls back.
    assert_eq!(
        cell(&rows, A::Knockdown),
        VerbCoverageCell::Bound(A::Knockdown)
    );
    assert_eq!(cell(&rows, A::Parried), VerbCoverageCell::Fallback(A::Stun));
    assert_eq!(cell(&rows, A::GetUp), VerbCoverageCell::Missing);
    assert_eq!(cell(&rows, A::StanceSwap), VerbCoverageCell::Missing);
    assert_eq!(
        cell(&rows, A::CritShot),
        VerbCoverageCell::Fallback(A::RangedAttack)
    );
    assert_eq!(cell(&rows, A::VertLightAttack), VerbCoverageCell::Optional);
    // Every kit's verbs are listed in contract order.
    assert_eq!(rows.len(), psx_level::combat_verbs::COMBAT_VERB_COUNT);
    assert_eq!(rows[0].kit, CombatKit::Core);

    // The light enemy shoots from its Zenith light (its shot is bound there)
    // and dashes (its mobility kit).
    let (ctx, light) = character_on_set(&project, "Light Enemy");
    let set = ctx.animation_set(light.animation_set).expect("set");
    let (kits, rows) =
        combat_verb_coverage_rows(&ctx.verb_bound(set, light.model), light.shoot_action());
    assert!(kits.can_swap() && kits.can_dodge());
    assert_eq!(
        cell(&rows, A::RangedAttack),
        VerbCoverageCell::Bound(A::VertLightAttack)
    );
    // Its mobility is the two dashes; it binds no roll of its own.
    assert_eq!(cell(&rows, A::DashLeft), VerbCoverageCell::Bound(A::DashLeft));
    assert_eq!(cell(&rows, A::Roll), VerbCoverageCell::Missing);

    // The default project's Aletha has no cannon: ranged and swap are "no kit".
    let project = project_default();
    let (ctx, aletha) = character_on_set(&project, "Aletha");
    let set = ctx.animation_set(aletha.animation_set).expect("set");
    let (kits, rows) =
        combat_verb_coverage_rows(&ctx.verb_bound(set, aletha.model), aletha.shoot_action());
    assert!(!kits.can_shoot() && !kits.can_swap() && kits.can_melee());
    assert_eq!(cell(&rows, A::RangedAim), VerbCoverageCell::NoKit);
    assert_eq!(cell(&rows, A::CritStrike), VerbCoverageCell::NoKit);
}

fn project_default() -> ProjectDocument {
    project("default")
}

/// Draw `character`'s inspector Effective actions list (what the inspector
/// had before) or the coverage table drawn under it, and rasterise it.
fn shot(project: &ProjectDocument, character: &str, with_table: bool) -> super::raster::Frame {
    const W: usize = 520;
    const H: usize = 1400;
    let (ctx, resource) = character_on_set(project, character);
    let set = ctx
        .animation_set(resource.animation_set)
        .expect("set")
        .clone();
    let shoot = resource.shoot_action();
    let bound = ctx.verb_bound(&set, resource.model);
    let egui_ctx = egui::Context::default();
    egui_ctx.set_fonts(fonts());
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
        let out = egui_ctx.run(input, |c| {
            apply_studio_visuals(c);
            egui::CentralPanel::default().show(c, |ui| {
                // The inspector draws the table right under Effective
                // actions; each shot shows one of the two in full.
                if with_table {
                    draw_combat_verb_coverage(ui, &set, &bound, shoot, &ctx);
                } else {
                    draw_character_effective_roles(ui, Some(&set), &ctx);
                }
            });
        });
        textures.apply(&out.textures_delta);
        last = Some(out);
    }
    let out = last.unwrap();
    let primitives = egui_ctx.tessellate(out.shapes, out.pixels_per_point);
    render(&primitives, &textures, W, H)
}

#[test]
#[ignore = "writes PNGs; set PSXED_VERB_SHOT_DIR"]
fn inspector_coverage_shot() {
    let dir = PathBuf::from(std::env::var("PSXED_VERB_SHOT_DIR").expect("PSXED_VERB_SHOT_DIR"));
    let project = project("grid-dungeon");
    for (character, file) in [("Aletha", "aletha"), ("Light Enemy", "light_enemy")] {
        save_png(
            &dir.join(format!("{file}_before.png")),
            &shot(&project, character, false),
        );
        save_png(
            &dir.join(format!("{file}_after.png")),
            &shot(&project, character, true),
        );
    }
}
