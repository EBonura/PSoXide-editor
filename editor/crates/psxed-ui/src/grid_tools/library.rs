//! The Library: everything that can be put into the world, in the bottom
//! drawer. Materials, Meshes, Actors, Styles (built in and captured) and
//! Buildings (the Build presets). Clicking an item arms the tool that uses
//! it: a material arms Paint, a style arms Corridor with that style, a mesh
//! arms Mesh, an actor arms Place, a building preset arms Building.

use super::build::options::{StyleEntry, StyleResolved};
use super::*;

/// The tabs of the Library.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LibraryTab {
    /// Surface materials, with previews.
    #[default]
    Materials,
    /// Static meshes: arches, columns, rubble, imports.
    Meshes,
    /// Characters to place.
    Actors,
    /// Corridor and room styles.
    Styles,
    /// Building presets.
    Buildings,
}

impl LibraryTab {
    /// Every tab, in order.
    pub(crate) const ALL: [Self; 5] = [
        Self::Materials,
        Self::Meshes,
        Self::Actors,
        Self::Styles,
        Self::Buildings,
    ];

    /// The tab's name.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Materials => "Materials",
            Self::Meshes => "Meshes",
            Self::Actors => "Actors",
            Self::Styles => "Styles",
            Self::Buildings => "Buildings",
        }
    }
}

/// A building preset: the options it sets.
struct BuildingPreset {
    name: &'static str,
    hint: &'static str,
    storeys: i32,
    window_every: i32,
    stair: bool,
    parapet: bool,
}

const BUILDINGS: [BuildingPreset; 4] = [
    BuildingPreset {
        name: "Cottage",
        hint: "One storey, a door, a window every two sectors",
        storeys: 1,
        window_every: 2,
        stair: false,
        parapet: false,
    },
    BuildingPreset {
        name: "Hall",
        hint: "Two storeys with a stair inside",
        storeys: 2,
        window_every: 2,
        stair: true,
        parapet: false,
    },
    BuildingPreset {
        name: "Tower",
        hint: "Three storeys, a stair, a parapet on top",
        storeys: 3,
        window_every: 3,
        stair: true,
        parapet: true,
    },
    BuildingPreset {
        name: "Keep",
        hint: "Four storeys, few windows, a stair and a parapet",
        storeys: 4,
        window_every: 4,
        stair: true,
        parapet: true,
    },
];

fn matches_search(name: &str, query: &str) -> bool {
    query.is_empty() || name.to_ascii_lowercase().contains(query)
}

impl EditorWorkspace {
    /// Open the drawer on the Library, on `tab`.
    pub(crate) fn open_library(&mut self, tab: LibraryTab) {
        self.grid.library_tab = tab;
        self.content_browser_view = ContentBrowserView::Library;
        self.resources_open = true;
    }

    /// The content of the Library tab of the drawer.
    pub(crate) fn draw_library(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for tab in LibraryTab::ALL {
                ui.selectable_value(&mut self.grid.library_tab, tab, tab.label());
            }
        });
        ui.separator();
        match self.grid.library_tab {
            LibraryTab::Materials => self.draw_grid_palette_in(ui, true),
            LibraryTab::Meshes => self.draw_library_meshes(ui),
            LibraryTab::Actors => self.draw_library_actors(ui),
            LibraryTab::Styles => self.draw_library_styles(ui),
            LibraryTab::Buildings => self.draw_library_buildings(ui),
        }
    }

    fn library_search_box(&mut self, ui: &mut egui::Ui, what: &str) -> String {
        ui.add(
            egui::TextEdit::singleline(&mut self.grid.library_search)
                .hint_text(format!("Search {what}"))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(4.0);
        self.grid.library_search.to_ascii_lowercase()
    }

    fn draw_library_meshes(&mut self, ui: &mut egui::Ui) {
        let query = self.library_search_box(ui, "meshes");
        let assets: Vec<(usize, String)> = self
            .project
            .active_scene()
            .world
            .grid()
            .map(|w| {
                w.mesh_assets
                    .iter()
                    .enumerate()
                    .map(|(i, a)| (i, a.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let mut chosen = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, name) in assets.iter().filter(|(_, n)| matches_search(n, &query)) {
                let on = self.grid.mesh_asset == Some(*i as u16);
                if ui
                    .selectable_label(on, icons::label(icons::CUBOID, name))
                    .on_hover_text("Arms the Mesh tool with this mesh")
                    .clicked()
                {
                    chosen = Some(*i);
                }
            }
            if assets.is_empty() {
                ui.label(RichText::new("No meshes in this world yet").color(STUDIO_TEXT_WEAK));
            }
        });
        if let Some(i) = chosen {
            self.grid.mesh_asset = Some(i as u16);
            self.set_grid_tool(GridTool::Mesh);
            self.status = "Tool: Mesh, click a floor to place it".to_string();
        }
    }

    fn draw_library_actors(&mut self, ui: &mut egui::Ui) {
        let query = self.library_search_box(ui, "actors");
        let actors: Vec<(ResourceId, String)> = self
            .project
            .resources
            .iter()
            .filter(|r| ResourceFilter::Character.matches(&r.data))
            .map(|r| (r.id, r.name.clone()))
            .collect();
        let mut chosen = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (id, name) in actors.iter().filter(|(_, n)| matches_search(n, &query)) {
                let on =
                    self.place_resource == Some(*id) && self.place_kind == PlaceKind::Character;
                if ui
                    .selectable_label(on, icons::label(icons::PERSON_STANDING, name))
                    .on_hover_text("Arms Place with this actor")
                    .clicked()
                {
                    chosen = Some(*id);
                }
            }
            if actors.is_empty() {
                ui.label(RichText::new("No actors in this project yet").color(STUDIO_TEXT_WEAK));
            }
        });
        if let Some(id) = chosen {
            self.place_kind = PlaceKind::Character;
            self.place_resource = Some(id);
            self.set_grid_tool(GridTool::Place);
            self.status = "Tool: Place, click a floor to put the actor there".to_string();
        }
    }

    fn draw_library_styles(&mut self, ui: &mut egui::Ui) {
        if !self.grid.build.styles_loaded {
            self.refresh_styles();
        }
        let query = self.library_search_box(ui, "styles");
        let entries: Vec<StyleEntry> = self.grid.build.styles.clone();
        let mut chosen = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for entry in entries.iter().filter(|e| matches_search(&e.name, &query)) {
                let on = self.grid.build.opts.corridor.style == entry.what;
                let text = if entry.project {
                    format!("{}  (captured)", entry.name)
                } else {
                    format!("{}  (built in)", entry.name)
                };
                if ui
                    .selectable_label(on, icons::label(icons::SWATCH_BOOK, &text))
                    .on_hover_text("Arms Corridor with this style")
                    .clicked()
                {
                    chosen = Some(entry.what.clone());
                }
            }
        });
        if let Some(style) = chosen {
            if let StyleResolved::Bay(bay) = &style {
                self.grid.build.opts.corridor.width = bay.size[0];
            }
            self.grid.build.opts.corridor.style = style;
            self.set_grid_tool(GridTool::Corridor);
            self.status = "Tool: Corridor, click waypoints in this style".to_string();
        }
    }

    fn draw_library_buildings(&mut self, ui: &mut egui::Ui) {
        let query = self.library_search_box(ui, "buildings");
        let mut chosen = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, preset) in BUILDINGS
                .iter()
                .enumerate()
                .filter(|(_, p)| matches_search(p.name, &query))
            {
                if ui
                    .selectable_label(false, icons::label(icons::BUILDING, preset.name))
                    .on_hover_text(preset.hint)
                    .clicked()
                {
                    chosen = Some(i);
                }
            }
        });
        match chosen {
            Some(i) => {
                let preset = &BUILDINGS[i];
                let b = &mut self.grid.build.opts.building;
                b.storeys = preset.storeys;
                b.window_every = preset.window_every;
                b.stair = preset.stair;
                b.parapet = preset.parapet;
                b.door = true;
                self.set_grid_tool(GridTool::Building);
                self.status = format!("Tool: Building, {}: drag a footprint", preset.name);
            }
            None => {}
        }
    }
}
