//! The World's texels per sector, in its Inspector: how many texels the
//! default mapping spends on one sector edge. A new world spends 64 (one
//! 64 by 64 texture per sector); a world made before the setting kept the 32
//! it had.

use super::*;

use psxed_project::grid::density::{set_texels_per_sector, KeepLook};
use psxed_project::grid::SectorTexels;

/// The densities offered: the 32 older worlds have, the 64 of a new world and
/// 128 (two tiles per sector).
const CHOICES: [u16; 3] = [32, 64, 128];

/// What a density comes to for a 64 by 64 texture, in words.
fn describe(texels: u16) -> String {
    match texels {
        64 => "one 64x64 texture per sector".to_string(),
        t if t < 64 => format!("a 64x64 texture covers {} sectors", 64 / t.max(1)),
        t => format!("{} textures per sector", t / 64),
    }
}

impl EditorWorkspace {
    /// The buttons and the keep-the-look choice. Returns whether the
    /// document changed (the Inspector records the undo step).
    pub(super) fn draw_texel_density(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(world) = self.project.active_scene().world.grid() else {
            return false;
        };
        let current = world.texels_per_sector;
        let keep_id = ui.id().with("keep_texture_sizes");
        let mut keep: bool = ui.data_mut(|d| d.get_persisted(keep_id)).unwrap_or(true);
        let mut picked: Option<SectorTexels> = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Texels per sector").color(STUDIO_TEXT_WEAK).size(11.0))
                .on_hover_text(
                    "How many texels the default mapping spends on one sector edge. The grid renderer tiles 64x64 textures, so 64 shows a whole texture on a sector; 32 (what worlds made before this setting have) shows a quarter of it. Faces with a mapping of their own and materials with a tiling default keep it.",
                );
            for texels in CHOICES {
                if ui
                    .selectable_label(current.get() == texels, texels.to_string())
                    .on_hover_text(describe(texels))
                    .clicked()
                {
                    picked = SectorTexels::new(texels);
                }
            }
        });
        ui.label(
            RichText::new(format!(
                "{} texels per sector: {}",
                current.get(),
                describe(current.get())
            ))
            .size(10.5)
            .color(STUDIO_TEXT_WEAK),
        );
        if ui
            .checkbox(&mut keep, "Keep faces with other texture sizes as they look")
            .on_hover_text(
                "A material whose texture is not 64x64 (the 32x32 ones) gets the density it has now as its own tiling, so it keeps its size when the world changes. The 64x64 ones follow the new density. One undo step.",
            )
            .changed()
        {
            ui.data_mut(|d| d.insert_persisted(keep_id, keep));
        }
        let Some(texels) = picked.filter(|t| *t != current) else {
            return false;
        };
        let root = self.project_dir.clone();
        let width = move |project: &psxed_project::ProjectDocument, id| -> Option<u16> {
            let (_, bytes) = psxed_project::resolve_material_texture_psxt(project, id, &root)
                .ok()
                .flatten()?;
            psx_asset::Texture::from_bytes(&bytes)
                .ok()
                .map(|t| t.width())
        };
        let how = if keep {
            KeepLook::OtherTextureSizes
        } else {
            KeepLook::Nothing
        };
        let Some(change) = set_texels_per_sector(&mut self.project, texels, how, &width) else {
            return false;
        };
        self.status = match change.materials_pinned + change.planes_pinned {
            0 => format!("{} texels per sector", texels.get()),
            n => format!(
                "{} texels per sector; {n} material(s) and plane(s) keep their size",
                texels.get()
            ),
        };
        self.grid_changed();
        true
    }
}
