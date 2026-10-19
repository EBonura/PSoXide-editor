//! The grid tool strip, its option row and the status bar.

use super::*;

use psxed_project::grid::limits::MAX_ROOM_FACES;
use psxed_project::grid::{GridBudget, CLICK_UNITS, FINE_UNITS};

impl EditorWorkspace {
    /// The tools on two rows: Pick, Draw and Finish, then Build.
    pub(crate) fn draw_grid_tool_rows(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            self.draw_tool_groups(ui, |g| g != ToolGroup::Build);
        });
        ui.horizontal_wrapped(|ui| {
            self.draw_tool_groups(ui, |g| g == ToolGroup::Build);
        });
    }

    /// Every tool button, in one run.
    pub(crate) fn draw_grid_tool_strip(&mut self, ui: &mut egui::Ui) {
        self.draw_tool_groups(ui, |_| true);
    }

    fn draw_tool_groups(&mut self, ui: &mut egui::Ui, take: impl Fn(ToolGroup) -> bool) {
        let mut group = None;
        for tool in GridTool::ALL.into_iter().filter(|t| take(t.group())) {
            if group != Some(tool.group()) {
                if group.is_some() {
                    ui.separator();
                }
                group = Some(tool.group());
                ui.label(
                    RichText::new(tool.group().label())
                        .color(STUDIO_TEXT_WEAK)
                        .size(9.5),
                );
            }
            let selected = self.grid.tool == tool;
            let response = ui
                .add(
                    egui::Button::new(icons::label(tool.icon(), tool.label()))
                        .selected(selected)
                        .min_size(Vec2::new(28.0, 23.0)),
                )
                .on_hover_text(format!(
                    "{} ({})\n{}",
                    tool.label(),
                    tool.key_label(),
                    tool.tooltip()
                ));
            if response.clicked() {
                self.set_grid_tool(tool);
            }
        }
    }

    /// Options of the active tool, next to the strip.
    pub(crate) fn draw_grid_context_controls(&mut self, ui: &mut egui::Ui) {
        if super::cell_size::takes_size(self.grid.tool) {
            self.draw_cell_size_selector(ui);
        }
        match self.grid.tool {
            GridTool::Floor | GridTool::Ceiling => {
                let ceiling = self.grid.tool == GridTool::Ceiling;
                let value = if ceiling {
                    &mut self.grid.ceiling_height
                } else {
                    &mut self.grid.floor_height
                };
                let mut clicks = *value as f32 / CLICK_UNITS as f32;
                ui.label(RichText::new("Height").color(STUDIO_TEXT_WEAK).size(11.0));
                if ui
                    .add(
                        egui::DragValue::new(&mut clicks)
                            .speed(0.05)
                            .suffix(" clicks")
                            .max_decimals(2),
                    )
                    .on_hover_text("Height of the next surface, in 256-unit clicks. Shift-drag sets it while dragging.")
                    .changed()
                {
                    *value = psxed_project::grid::units::snap_to(
                        (clicks * CLICK_UNITS as f32).round() as i32,
                        FINE_UNITS,
                    );
                }
            }
            GridTool::Wall => {
                let label = format!("Direction: {}", self.grid.wall_dir.label());
                if ui
                    .button(label)
                    .on_hover_text(
                        "R turns the wall direction: auto, N, E, S, W, then the two diagonals",
                    )
                    .clicked()
                {
                    self.grid.wall_dir = self.grid.wall_dir.next();
                }
            }
            GridTool::Plane => {
                ui.label(
                    RichText::new("New plane")
                        .color(STUDIO_TEXT_WEAK)
                        .size(11.0),
                );
                egui::ComboBox::from_id_salt("plane_new_collision")
                    .selected_text(self.grid.new_plane.collision.label())
                    .width(104.0)
                    .show_ui(ui, |ui| {
                        for class in psxed_project::grid::PlaneCollision::ALL {
                            ui.selectable_value(
                                &mut self.grid.new_plane.collision,
                                class,
                                class.label(),
                            );
                        }
                    })
                    .response
                    .on_hover_text("Collision class a new plane starts with");
                ui.checkbox(&mut self.grid.new_plane.double_sided, "Both sides")
                    .on_hover_text("New planes draw their back too");
            }
            GridTool::Mesh => {
                let name = self
                    .grid
                    .mesh_asset
                    .and_then(|a| {
                        self.project
                            .active_scene()
                            .world
                            .grid()
                            .and_then(|w| w.mesh_assets.get(usize::from(a)))
                            .map(|a| a.name.clone())
                    })
                    .unwrap_or_else(|| "none (pick one in the palette)".to_string());
                ui.label(
                    RichText::new(format!(
                        "Mesh: {name}   heading {}\u{b0}",
                        self.grid.mesh_turn
                    ))
                    .color(STUDIO_TEXT_WEAK)
                    .size(11.0),
                );
            }
            GridTool::Select | GridTool::Height => {
                // Link welds the corners being lifted: Height always has
                // some, Select once something is picked. Move and Global are
                // the gizmo of a selected node.
                let picked = !self.grid.selection.is_empty();
                if self.grid.tool == GridTool::Height || picked {
                    ui.toggle_value(&mut self.grid.link, "Link")
                        .on_hover_text("L: coincident corners move together");
                }
                let node_selected = self.selection.selected_node != NodeId::ROOT
                    || self
                        .selection
                        .selected_nodes
                        .iter()
                        .any(|n| *n != NodeId::ROOT);
                if self.grid.tool == GridTool::Select && node_selected {
                    self.draw_transform_gizmo_toolbar_controls(ui);
                }
            }
            GridTool::Place => {
                self.draw_add_menu(ui);
                ui.label(
                    RichText::new(self.place_kind.label())
                        .color(STUDIO_TEXT_WEAK)
                        .size(11.0),
                );
                self.draw_active_place_options(ui);
            }
            GridTool::Paint => {
                let name = self
                    .grid
                    .material
                    .and_then(|id| self.project.resource_name(id))
                    .unwrap_or("none")
                    .to_string();
                ui.label(
                    RichText::new(format!("Material: {name}"))
                        .color(STUDIO_TEXT_WEAK)
                        .size(11.0),
                );
            }
            GridTool::Split => {}
            GridTool::Raise
            | GridTool::Ramp
            | GridTool::Stairs
            | GridTool::Spiral
            | GridTool::Break
            | GridTool::Rough
            | GridTool::Slab => self.draw_verb_options(ui),
            GridTool::Cells => self.draw_cells_options(ui),
            GridTool::Bands => {
                use psxed_project::grid::edit::bands::BandScope;
                ui.label(RichText::new("Scope").color(STUDIO_TEXT_WEAK).size(11.0));
                for scope in [BandScope::Run, BandScope::Room] {
                    let on = self.grid.band_scope == scope;
                    if ui
                        .selectable_label(on, super::bands_tool::scope_label(scope))
                        .on_hover_text(match scope {
                            BandScope::Run => {
                                "Edit just the selected run: it gets its own copy of a shared set"
                            }
                            BandScope::Room => "Edit the set in place, wherever the room uses it",
                        })
                        .clicked()
                    {
                        self.grid.band_scope = scope;
                    }
                }
            }
        }
        ui.separator();
        let snap_text = if matches!(self.grid.tool, GridTool::Plane | GridTool::Select)
            && !self.grid.plane_sel.is_empty()
            || matches!(self.grid.tool, GridTool::Plane | GridTool::Mesh)
        {
            format!("Snap {CLICK_UNITS} (Alt {FINE_UNITS}, Shift+Alt free)")
        } else {
            format!("Snap {CLICK_UNITS} (Alt {FINE_UNITS})")
        };
        ui.label(RichText::new(snap_text).color(STUDIO_TEXT_WEAK).size(11.0));
    }

    /// The overlays and the key page: Walk, Cells and Perf toggles, and the
    /// button that opens the page of every key and gesture.
    pub(crate) fn draw_grid_overlay_controls(&mut self, ui: &mut egui::Ui) {
        // The row is laid out right to left, so the rightmost button comes
        // first: Keys, Perf (and its reserve), Sizes, Walk.
        if ui
            .selectable_label(self.grid.help_open, icons::label(icons::KEYBOARD, "Keys"))
            .on_hover_text("Every tool key and gesture (?)")
            .clicked()
        {
            self.grid.help_open = !self.grid.help_open;
        }
        if self.grid.overlays.perf {
            let mut thousands = self.grid.overlays.reserve / 1000;
            if ui
                .add(egui::DragValue::new(&mut thousands).range(0..=1000).suffix("k reserve"))
                .on_hover_text("Cycles the frame spends outside the world: the player and the sky (470k + 58k measured on the Cathedral)")
                .changed()
            {
                self.grid.overlays.reserve = thousands * 1000;
            }
        }
        if ui
            .selectable_label(
                self.grid.overlays.perf,
                icons::label(icons::FLAME, "Perf"),
            )
            .on_hover_text("Performance heatmap (Alt+P): a model of the world render cost at every spot, against the 30 fps budget")
            .clicked()
        {
            self.toggle_perf_overlay();
        }
        if ui
            .selectable_label(
                self.grid.overlays.leaves,
                icons::label(icons::GRID, "Sizes"),
            )
            .on_hover_text(
                "Cell sizes (Alt+L): every cell outlined and labelled with its size in sectors",
            )
            .clicked()
        {
            self.toggle_leaf_overlay();
        }
        if ui
            .selectable_label(
                self.grid.overlays.walk,
                icons::label(icons::FOOTPRINTS, "Walk"),
            )
            .on_hover_text("Walkability overlay (Alt+W): too steep, low headroom, steps taller than the body climbs, drops, and floors no spawn reaches")
            .clicked()
        {
            self.toggle_walk_overlay();
        }
    }

    /// Key hints for the active tool.
    fn grid_key_hints(&self) -> &'static str {
        if self.grid.paste.is_some() {
            return "Click place | Shift keep stamping | H/V flip | R turn | Esc cancel";
        }
        if self.grid.plane_paste.is_some() {
            return "Click place | Shift keep stamping | R turn | H flip | Esc cancel";
        }
        if !self.grid.plane_sel.is_empty()
            && matches!(self.grid.tool, GridTool::Select | GridTool::Plane)
        {
            return "Drag a handle: corners and edges stretch, green ring turns, blue tip tilts | Drag body slides, Shift up/down | Alt 64, Shift+Alt free | R turn | Del | Ctrl+C/V";
        }
        match self.grid.tool {
            GridTool::Plane => {
                "Click a surface to place a plane | Drag sizes it | Click a plane to select | Alt 64, Shift+Alt free"
            }
            GridTool::Select => {
                "Click pick | Shift range | Ctrl toggle | Drag box | Shift-drag height | Del delete | Ctrl+C/V copy"
            }
            GridTool::Mesh => {
                "Click a floor to place | Drag moves | R turns, [ ] 15 | - = scale | Del removes | Alt 64"
            }
            GridTool::Floor => "Drag rectangle | Shift height | Ctrl delete | Esc cancel",
            GridTool::Ceiling => "Drag rectangle | Shift height | Ctrl delete | Esc cancel",
            GridTool::Wall => "Drag a run | R direction | Ctrl remove | Esc cancel",
            GridTool::Height => "Drag up/down | Alt fine | Alt+drag a face: one triangle | L link | PgUp/PgDn nudge | Esc cancel",
            GridTool::Paint => "Click or drag paint | Ctrl sample | pick in the palette",
            GridTool::Split => "Click a floor or ceiling flips its diagonal | Shift+click voids a triangle | Click again for the ceiling in Top",
            GridTool::Bands => "Hover a wall, click to cut | Drag a line | Del removes it | Shift+click picks a band | Esc cancel",
            GridTool::Raise => "Pick sectors | Press on them, drag up/down | Tab Offset/Level | Alt fine | Shift add | Esc cancel",
            GridTool::Ramp => "Pick sectors | Press near the side to keep, drag up/down | R turn the side | Tab hold to walkable | Esc cancel",
            GridTool::Stairs => "Click waypoints, Enter | Press on the path, drag up/down | [ ] width | Tab steps/ramp | Esc cancel",
            GridTool::Spiral => "Click the corners of a ring, Enter closes it | Press on it, drag up for the climb of a storey | Storeys in the option row | Alt+Up/Down level | Esc cancel",
            GridTool::Break => "Pick sectors | Press on them, drag up for intensity | Tab target | Shift+X re-roll | Enter commit | Esc cancel",
            GridTool::Rough => "Pick sectors | Press on them, drag up for the level 0-3 | Enter commit | Esc cancel",
            GridTool::Slab => "Pick sectors | Insert: press on them and drag up for the height | Cut, Fill: click them | Tab mode | Alt+Up/Down level | G go to level | Z cutaway | Esc cancel",
            GridTool::Cells => "Paint to subdivide | Alt+paint merges | [ ] brush | Enter / Shift+Enter subdivide / merge the selection | Alt+L cell sizes",
            GridTool::Place => "Click to place at the sector centre | Shift+click keeps placing | Alt 64 | Shift+Alt free",
        }
    }

    /// Faces facing the camera with their centre inside the 3D view: a cheap
    /// stand-in for how much the preview has to draw.
    fn grid_faces_in_view(&self) -> Option<usize> {
        if self.view_2d {
            return None;
        }
        let rect = self.grid.view_rect.get()?;
        let camera = self.viewport_3d_camera();
        let cam = pick::Cam::new(camera, rect);
        let eye = camera.position_f32();
        self.with_pick_cache(|cache| {
            cache
                .faces
                .iter()
                .filter(|pf| {
                    let n = pf.points.len() as f32;
                    let centre = pf.points.iter().fold([0.0; 3], |a, p| {
                        [a[0] + p[0] / n, a[1] + p[1] / n, a[2] + p[2] / n]
                    });
                    let to_eye = [eye[0] - centre[0], eye[1] - centre[1], eye[2] - centre[2]];
                    let facing = pf.normal[0] * to_eye[0]
                        + pf.normal[1] * to_eye[1]
                        + pf.normal[2] * to_eye[2];
                    (facing > 0.0 || pf.double_sided)
                        && cam.project(centre).is_some_and(|(p, _)| rect.contains(p))
                })
                .count()
        })
    }

    fn grid_budget(&self) -> Option<GridBudget> {
        let world = self.project.active_scene().world.grid()?;
        let mut slot = self.grid.budget.borrow_mut();
        if slot.map(|(rev, _)| rev) != Some(self.grid.rev) {
            *slot = Some((self.grid.rev, GridBudget::of(world)));
        }
        slot.map(|(_, budget)| budget)
    }

    /// The bottom bar: last message, key hints, budget.
    pub(crate) fn draw_grid_status_bar(&mut self, ctx: &egui::Context) {
        let Some(budget) = self.grid_budget() else {
            return;
        };
        let room = self.grid_target_room();
        let (room_sectors, room_name, room_count) = self
            .project
            .active_scene()
            .world
            .grid()
            .map(|w| {
                let r = w.rooms.get(room);
                (
                    r.map_or(0, |r| r.leaf_count()),
                    r.map_or(String::new(), |r| r.name.clone()),
                    w.rooms.len(),
                )
            })
            .unwrap_or_default();
        let in_view = self.grid_faces_in_view();
        let worst = if self.grid.overlays.perf {
            self.heat_run().filter(|r| r.fresh).and_then(|r| {
                let w = *r.map.worst()?;
                let pct = w.cycles as f32 * 100.0 / r.map.options.world_budget() as f32;
                Some(format!(
                    "   cooked {} quads, worst spot {pct:.0}% (model)",
                    r.map.quads_total
                ))
            })
        } else {
            None
        };
        let percent = budget.faces_percent();
        let hot = percent >= 85;
        let chip_colour = if hot {
            Color32::from_rgb(240, 150, 90)
        } else {
            STUDIO_TEXT
        };
        egui::TopBottomPanel::bottom("psxed_grid_status_bar")
            .exact_height(30.0)
            .frame(
                egui::Frame::new()
                    .fill(STUDIO_PANEL_HEADER)
                    .inner_margin(egui::Margin::symmetric(10, 5)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // The right end first: the budget chips, so the message
                    // and the hints share what is left.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let view_text = in_view.map(|n| format!("{n} in view"));
                        if let Some(text) = view_text {
                            ui.label(RichText::new(text).color(STUDIO_TEXT_WEAK).size(12.0));
                            ui.separator();
                        }
                        if let Some(text) = &worst {
                            ui.label(
                                RichText::new(text.trim())
                                    .color(STUDIO_TEXT_WEAK)
                                    .size(12.0),
                            );
                            ui.separator();
                        }
                        ui.label(
                            RichText::new(format!(
                                "{} / {MAX_ROOM_FACES} faces ({percent}%)",
                                budget.largest_room_faces
                            ))
                            .color(chip_colour)
                            .size(12.0),
                        )
                        .on_hover_text(format!(
                            "The busiest room's faces against the cap; the world has {} rooms and {} faces",
                            room_count, budget.faces
                        ));
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "{}: {room_sectors} {}",
                                if room_name.is_empty() {
                                    format!("Room {room}")
                                } else {
                                    format!("Room {room_name}")
                                },
                                budget
                                    .levels_text()
                                    .map_or("sectors".to_string(), |t| format!("cells ({t})")),
                            ))
                            .color(STUDIO_TEXT)
                            .size(12.0),
                        );
                        ui.separator();
                        // What is left: the message, then the hints.
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&self.status)
                                        .color(Color32::from_rgb(120, 220, 140))
                                        .size(13.0),
                                )
                                .truncate(),
                            );
                            ui.separator();
                            ui.add(
                                egui::Label::new(
                                    RichText::new(self.grid_key_hints())
                                        .color(STUDIO_TEXT_WEAK)
                                        .size(12.0),
                                )
                                .truncate(),
                            );
                        });
                    });
                });
            });
    }
}
