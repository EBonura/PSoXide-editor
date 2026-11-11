//! The grid tool options and the status bar.

use super::*;

use psxed_project::grid::limits::MAX_ROOM_FACES;
use psxed_project::grid::{GridBudget, CLICK_UNITS, FINE_UNITS};

impl EditorWorkspace {
    /// Options of the active tool.
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
            GridTool::Room
            | GridTool::Corridor
            | GridTool::Opening
            | GridTool::Arcade
            | GridTool::Colonnade
            | GridTool::Bridge
            | GridTool::Building
            | GridTool::Cathedral
            | GridTool::Terrain
            | GridTool::Ruin
            | GridTool::Door
            | GridTool::Lever
            | GridTool::Checkpoint => self.draw_build_options(ui),
            GridTool::Edit => self.draw_edit_options(ui),
            GridTool::Select | GridTool::Height => {
                // Weld moves the corners that share a position together: Height always has
                // some, Select once something is picked. Move and Global are
                // the gizmo of a selected node.
                let picked = !self.grid.selection.is_empty();
                if self.grid.tool == GridTool::Height || picked {
                    ui.toggle_value(&mut self.grid.link, "Weld").on_hover_text(
                        "L: weld, so coincident corners move together (off by default)",
                    );
                }
                if self.grid.tool == GridTool::Select {
                    self.draw_merge_buttons(ui);
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
            GridTool::Paint => self.draw_paint_options(ui),
            GridTool::Split => {
                ui.label(
                    RichText::new("Click a floor or ceiling to flip its diagonal")
                        .color(STUDIO_TEXT_WEAK)
                        .size(11.0),
                );
            }
            GridTool::Extrude
            | GridTool::Ramp
            | GridTool::Stairs
            | GridTool::Spiral
            | GridTool::Break
            | GridTool::Slab => self.draw_verb_options(ui),
            GridTool::Sculpt => self.draw_sculpt_options(ui),
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
                            BandScope::Piece => "Edit just one wall segment",
                        })
                        .clicked()
                    {
                        self.grid.band_scope = scope;
                    }
                }
                ui.separator();
                let line = self.grid.band_line.is_some();
                if ui
                    .add_enabled(line, egui::Button::new("Merge bands"))
                    .on_hover_text("Shift+M: take the picked cut away; the bands either side of it join and the lower one keeps its material (click a line to pick it)")
                    .clicked()
                {
                    super::bands_tool::merge_line(self);
                }
            }
        }
    }

    /// The Snap rule at the right end of the options bar.
    fn grid_snap_text(&self) -> String {
        if matches!(self.grid.tool, GridTool::Plane | GridTool::Select)
            && !self.grid.plane_sel.is_empty()
            || matches!(self.grid.tool, GridTool::Plane | GridTool::Mesh)
        {
            format!("Snap {CLICK_UNITS} (Alt {FINE_UNITS}, Shift+Alt free)")
        } else {
            format!("Snap {CLICK_UNITS} (Alt {FINE_UNITS})")
        }
    }

    /// The options bar as a panel across the window, directly under the tool
    /// strip: always in the same place, one fixed height.
    pub(crate) fn draw_grid_options_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("psxed_options_bar")
            .exact_height(OPTIONS_BAR_HEIGHT + 8.0)
            .frame(
                egui::Frame::new()
                    .fill(STUDIO_PANEL_HEADER)
                    .inner_margin(egui::Margin::symmetric(10, 4)),
            )
            .show(ctx, |ui| self.draw_grid_options_bar(ui));
    }

    /// The options bar: a bar of one fixed height, whatever the tool. It
    /// shows only the active tool's options (nothing when it has none) and
    /// Snap at its right end. Nothing in it can grow the bar, so switching
    /// tools never moves the viewport.
    pub(crate) fn draw_grid_options_bar(&mut self, ui: &mut egui::Ui) {
        let (bar, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), OPTIONS_BAR_HEIGHT),
            Sense::hover(),
        );
        ui.painter()
            .rect_filled(bar, egui::CornerRadius::same(CONTROL_RADIUS), STUDIO_INPUT);
        // The right end first: Snap.
        let snap = ui.painter().layout_no_wrap(
            self.grid_snap_text(),
            egui::FontId::proportional(11.0),
            STUDIO_TEXT_WEAK,
        );
        let snap_pos = Pos2::new(
            bar.max.x - 10.0 - snap.size().x,
            bar.center().y - snap.size().y / 2.0,
        );
        let snap_left = snap_pos.x;
        ui.painter().galley(snap_pos, snap, STUDIO_TEXT_WEAK);
        // What is left: the options, or the hints. A tool with more options
        // than the pane has room for scrolls them sideways (the arrows at the
        // ends say so) instead of wrapping, which would grow the bar.
        let mut rest = Rect::from_min_max(
            bar.min + Vec2::new(8.0, 0.0),
            Pos2::new(snap_left - 10.0, bar.max.y),
        );
        // A notice that needs an answer takes the whole bar: the buttons at
        // its right end, the text in what is left.
        if let Some(notice) = self.bar_notice() {
            let mut buttons = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rest)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            buttons.set_clip_rect(rest.intersect(ui.clip_rect()));
            buttons.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
            let mut chosen = None;
            for (label, action) in notice.buttons.iter().rev() {
                if buttons
                    .add(egui::Button::new(RichText::new(*label).size(12.0)))
                    .clicked()
                {
                    chosen = Some(*action);
                }
            }
            let text_rect = Rect::from_min_max(
                rest.min,
                Pos2::new(buttons.min_rect().min.x - 10.0, rest.max.y),
            );
            ui.put(
                text_rect,
                egui::Label::new(
                    RichText::new(&notice.text)
                        .color(Color32::from_rgb(240, 180, 90))
                        .size(12.0),
                )
                .truncate(),
            )
            .on_hover_text(&notice.text);
            if let Some(action) = chosen {
                self.run_bar_action(action);
            }
            return;
        }
        // The level verbs lead with the next step, pinned at the left; the
        // options scroll in what is left.
        if let Some(step) = self.verb_step_text() {
            let galley = ui.painter().layout_no_wrap(
                step.clone(),
                egui::FontId::proportional(12.0),
                STUDIO_ACCENT,
            );
            let width = galley.size().x.min(rest.width() * 0.8);
            let lead = Rect::from_min_size(rest.min, Vec2::new(width, bar.height()));
            ui.put(
                lead,
                egui::Label::new(RichText::new(&step).color(STUDIO_ACCENT).size(12.0)).truncate(),
            )
            .on_hover_text(step);
            let divider = lead.max.x + 6.0;
            ui.painter().line_segment(
                [
                    Pos2::new(divider, bar.min.y + 6.0),
                    Pos2::new(divider, bar.max.y - 6.0),
                ],
                Stroke::new(1.0, STUDIO_TEXT_WEAK.gamma_multiply(0.4)),
            );
            rest.min.x = divider + 8.0;
        }
        if self.grid.options_scroll_tool != self.grid.tool {
            self.grid.options_scroll_tool = self.grid.tool;
            self.grid.options_scroll = 0.0;
        }
        let mut inner = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rest)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        inner.set_clip_rect(rest.intersect(ui.clip_rect()));
        inner.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
        let view_width = rest.width();
        let area = egui::ScrollArea::horizontal()
            .id_salt("grid_options_scroll")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .horizontal_scroll_offset(self.grid.options_scroll)
            .auto_shrink([false, false])
            .show(&mut inner, |ui| {
                let options = ui.scope(|ui| self.draw_grid_context_controls(ui));
                let _ = options;
            });
        self.grid.options_scroll = area.state.offset.x;
        let overflow = area.content_size.x - view_width;
        if overflow > 1.0 {
            let mut step = 0.0;
            if self.grid.options_scroll > 0.5 {
                let rect = Rect::from_min_size(rest.min, Vec2::new(18.0, bar.height()));
                if scroll_arrow(ui, rect, icons::CHEVRON_LEFT, "More options to the left") {
                    step = -160.0;
                }
            }
            if self.grid.options_scroll < overflow - 0.5 {
                let rect = Rect::from_min_size(
                    Pos2::new(rest.max.x - 18.0, rest.min.y),
                    Vec2::new(18.0, bar.height()),
                );
                if scroll_arrow(ui, rect, icons::CHEVRON_RIGHT, "More options to the right") {
                    step = 160.0;
                }
            }
            if step != 0.0 {
                self.grid.options_scroll = (self.grid.options_scroll + step).clamp(0.0, overflow);
            }
            // The wheel over the bar scrolls it sideways too.
            if ui.rect_contains_pointer(bar) {
                let wheel = ui.input(|i| i.raw_scroll_delta.y);
                if wheel != 0.0 {
                    self.grid.options_scroll =
                        (self.grid.options_scroll - wheel).clamp(0.0, overflow);
                }
            }
        }
    }

    /// Key hints for the active tool.
    fn grid_key_hints(&self) -> &'static str {
        if self.grid.grab.is_some() {
            return "Click drops the copy | Up/Down lift a click, Alt fine | Esc cancels";
        }
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
        self.grid.tool.hint()
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

    /// What the pointer is over, in words, for the hint bar. `None` over
    /// nothing worth naming.
    pub(crate) fn grid_hover_text(&self) -> Option<String> {
        let room_cell = |room: usize, cell: [i32; 2]| {
            let name = self
                .project
                .active_scene()
                .world
                .grid()
                .and_then(|w| w.rooms.get(room))
                .map_or_else(|| format!("room {room}"), |r| r.name.clone());
            format!("{name}, sector {}, {}", cell[0], cell[1])
        };
        Some(match &self.grid.hover {
            GridHover::None => return None,
            GridHover::Cell { room, cell, height } => format!(
                "Over {} at {:.2} clicks",
                room_cell(*room, *cell),
                *height as f32 / CLICK_UNITS as f32
            ),
            GridHover::Wall {
                room, cell, edge, ..
            } => format!("Over a wall of {} ({edge:?})", room_cell(*room, *cell)),
            GridHover::Elements(list) => match list.len() {
                0 => return None,
                1 => "Over one element".to_string(),
                n => format!("Over {n} elements"),
            },
            GridHover::Face { .. } => "Over a face".to_string(),
            GridHover::Spot { .. } => "Over a spot to place at".to_string(),
            GridHover::Band { .. } => "Over a wall, ready to cut".to_string(),
            GridHover::Leaf { room, .. } => format!("Over a cell of room {room}"),
            GridHover::Plane(_) => "Over a plane".to_string(),
            GridHover::Mesh(_) => "Over a mesh".to_string(),
            GridHover::MeshSpot { .. } => "Over a floor a mesh can stand on".to_string(),
            GridHover::Grip(..) => "Over a plane handle".to_string(),
            GridHover::PlaneSpot(..) => "Over a surface a plane can go on".to_string(),
        })
    }

    /// The left of the hint bar: what the pointer is over, then the keys of
    /// the active tool. A tab without tools says what its pointer does.
    pub(crate) fn status_hint(&self) -> String {
        if self.is_grid_world() && self.active_workspace == WorkspaceView::Room {
            if !self.edit_hint().is_empty() {
                return self.edit_hint().to_string();
            }
            if !self.build_hint().is_empty() {
                return self.build_hint().to_string();
            }
            let keys = self.grid_key_hints();
            return match self.grid_hover_text() {
                Some(over) => format!("{over}   |   {keys}"),
                None => keys.to_string(),
            };
        }
        match self.active_workspace {
            WorkspaceView::Room => "Open or create a grid project to edit a world".to_string(),
            WorkspaceView::Ui => {
                "Pick a node in the tree to edit it in the Inspector | Ctrl+S saves | Ctrl+Z undoes"
                    .to_string()
            }
            WorkspaceView::Animation => {
                "Pick a model or a clip in the Assets drawer | Ctrl+S saves | Ctrl+Z undoes"
                    .to_string()
            }
            WorkspaceView::Material => {
                "Pick a material in the Assets drawer to edit it | Ctrl+S saves | Ctrl+Z undoes"
                    .to_string()
            }
        }
    }

    /// The bottom bar on every tab: the hint for the active tool and hover
    /// target on the left, the last message on the right (so a Save or an
    /// error is seen wherever it happened), and on the World tab the budget
    /// chips beside the message.
    pub(crate) fn draw_status_bar(&mut self, ctx: &egui::Context) {
        let world_tab = self.is_grid_world() && self.active_workspace == WorkspaceView::Room;
        let budget = if world_tab { self.grid_budget() } else { None };
        let hint = self.status_hint();
        let chips = budget.map(|budget| {
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
            (budget, room, room_sectors, room_name, room_count)
        });
        let in_view = if world_tab {
            self.grid_faces_in_view()
        } else {
            None
        };
        let worst = if world_tab && self.grid.overlays.perf {
            self.heat_run().filter(|r| r.fresh).and_then(|r| {
                let w = *r.map.worst()?;
                let pct = w.cycles as f32 * 100.0 / r.map.options.world_budget() as f32;
                Some(format!(
                    "cooked {} quads, worst spot {pct:.0}% (model)",
                    r.map.quads_total
                ))
            })
        } else {
            None
        };
        let message = self.status.clone();
        let toast = self.live_undo_toast().map(str::to_string);
        let mut undo_clicked = false;
        // A narrow window keeps the hint readable: the secondary chips go
        // first (the face budget stays, it is the cap that matters).
        let screen_width = ctx.screen_rect().width();
        let in_view = in_view.filter(|_| screen_width >= 1500.0);
        let worst = worst.filter(|_| screen_width >= 1500.0);
        let show_room_chip = screen_width >= 1500.0;
        let half = (ctx.screen_rect().width() * 0.42).max(160.0);
        egui::TopBottomPanel::bottom("psxed_status_bar")
            .exact_height(STATUS_BAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(STUDIO_PANEL_HEADER)
                    .inner_margin(egui::Margin::symmetric(10, 5)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // The right end first, so the hint takes what is left:
                    // the message outermost, the chips beside it.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.scope(|ui| {
                            ui.set_max_width(half);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&message)
                                        .color(Color32::from_rgb(120, 220, 140))
                                        .size(13.0),
                                )
                                .truncate(),
                            )
                            .on_hover_text(&message);
                        });
                        if toast.is_some() && ui.link("Undo").clicked() {
                            undo_clicked = true;
                        }
                        if let Some((budget, room, room_sectors, room_name, room_count)) = &chips {
                            let percent = budget.faces_percent();
                            let colour = if percent >= 85 {
                                Color32::from_rgb(240, 150, 90)
                            } else {
                                STUDIO_TEXT
                            };
                            ui.separator();
                            ui.label(
                                RichText::new(format!(
                                    "{} / {MAX_ROOM_FACES} faces ({percent}%)",
                                    budget.largest_room_faces
                                ))
                                .color(colour)
                                .size(12.0),
                            )
                            .on_hover_text(format!(
                                "The busiest room's faces against the cap; the world has {} rooms and {} faces",
                                room_count, budget.faces
                            ));
                            if show_room_chip {
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
                            }
                        }
                        if let Some(text) = &worst {
                            ui.separator();
                            ui.label(RichText::new(text).color(STUDIO_TEXT_WEAK).size(12.0));
                        }
                        if let Some(n) = in_view {
                            ui.separator();
                            ui.label(
                                RichText::new(format!("{n} in view"))
                                    .color(STUDIO_TEXT_WEAK)
                                    .size(12.0),
                            );
                        }
                        ui.separator();
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let font = egui::FontId::proportional(12.0);
                            let shown = elide_hint(ui, &hint, ui.available_width() - 8.0, &font);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&shown).color(STUDIO_TEXT_WEAK).size(12.0),
                                )
                                .truncate(),
                            )
                            .on_hover_text(&hint);
                        });
                    });
                });
            });
        if undo_clicked {
            self.do_undo();
            self.undo_toast = None;
        }
    }
}

/// Width of `text` in `font`, in points.
fn text_width(ui: &egui::Ui, text: &str, font: &egui::FontId) -> f32 {
    ui.fonts(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE))
        .size()
        .x
}

/// `hint` cut to fit `width`, from the middle: the hint is a run of parts
/// divided by `|`, and the first part (what the pointer is over) and the last
/// parts (the keys that act) stay while the ones between give way to an
/// ellipsis. A single part too long for the room is cut at its end.
pub(crate) fn elide_hint(ui: &egui::Ui, hint: &str, width: f32, font: &egui::FontId) -> String {
    if text_width(ui, hint, font) <= width {
        return hint.to_string();
    }
    let parts: Vec<&str> = hint
        .split('|')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() >= 2 {
        // Keep the first part, then take parts from the end while they fit.
        let join = |tail: &[&str]| format!("{} | ... | {}", parts[0], tail.join(" | "));
        let mut best: Option<String> = None;
        for keep in 1..parts.len() {
            let candidate = join(&parts[parts.len() - keep..]);
            if text_width(ui, &candidate, font) <= width {
                best = Some(candidate);
            } else {
                break;
            }
        }
        if let Some(best) = best {
            return best;
        }
    }
    // One long part: keep its head, and its tail, with the ellipsis between.
    let chars: Vec<char> = hint.chars().collect();
    let mut head = chars.len() / 2;
    while head > 4 {
        let tail = (head / 2).max(4).min(chars.len() - head);
        let candidate: String = chars[..head]
            .iter()
            .chain(['.', '.', '.'].iter())
            .chain(chars[chars.len() - tail..].iter())
            .collect();
        if text_width(ui, &candidate, font) <= width {
            return candidate;
        }
        head -= 2;
    }
    hint.to_string()
}

/// A small arrow button on the edge of the options bar that scrolls it.
fn scroll_arrow(ui: &mut egui::Ui, rect: Rect, icon: char, hover: &str) -> bool {
    let response = ui
        .interact(rect, ui.id().with(("options_arrow", icon)), Sense::click())
        .on_hover_text(hover);
    let fill = if response.hovered() {
        STUDIO_HOVER
    } else {
        STUDIO_PANEL_HEADER
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(CONTROL_RADIUS), fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon.to_string(),
        icons::font(12.0),
        STUDIO_TEXT,
    );
    response.clicked()
}
