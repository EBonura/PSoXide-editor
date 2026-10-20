//! The Objects panel: the Scene Graph as the author thinks of it. One row per
//! object (an actor is one row, its parts are sections of the Inspector),
//! grouped by kind or shown as a tree.

use super::*;

/// What the list is made of, top to bottom.
enum Entry<'a> {
    Group {
        group: ObjectGroup,
        count: usize,
        open: bool,
    },
    Row {
        row: &'a ObjectRow,
        /// Indent in rows' worth of 16 px.
        indent: usize,
    },
}

/// How one row is drawn.
struct RowState {
    selected: bool,
    collapsed: bool,
    directly_hidden: bool,
    effectively_hidden: bool,
    can_snap_to_floor: bool,
    /// Whether the list is a tree (a drop can land before a row, and a
    /// folder shows a chevron) or grouped by kind.
    tree: bool,
    /// Scroll this row into view.
    reveal: bool,
}

impl EditorWorkspace {
    pub(crate) fn draw_objects_panel(&mut self, ui: &mut egui::Ui) {
        tool_panel_frame().show(ui, |ui| {
            ui.set_min_height(ui.available_height());
            tool_panel_header(ui, icons::LAYERS, "Objects", |ui| {
                let by_kind = self.objects_by_kind;
                if ui
                    .add(
                        egui::Button::new(icons::text(
                            if by_kind {
                                icons::ROWS_3
                            } else {
                                icons::LAYERS_2
                            },
                            14.0,
                        ))
                        .min_size(Vec2::new(26.0, 22.0)),
                    )
                    .on_hover_text(if by_kind {
                        "Grouped by kind. Click to show the tree (folders and their contents)."
                    } else {
                        "Shown as a tree. Click to group by kind."
                    })
                    .clicked()
                {
                    self.objects_by_kind = !by_kind;
                    self.objects_last_subject = None;
                }
                ui.menu_button(icons::text(icons::PLUS, 14.0), |ui| {
                    for (label, kind) in scene_graph_addable_kinds() {
                        if ui.button(label).clicked() {
                            self.add_child(kind, label);
                            ui.close_menu();
                        }
                    }
                })
                .response
                .on_hover_text(
                    "Add an actor or a folder. Place lights, props and the rest from the toolbar.",
                );
            });
            tool_panel_body(ui, |ui| self.draw_objects_panel_body(ui));
        });
    }

    /// The object whose row the selection stands on: an actor for one of its
    /// parts. `None` for nothing, or the World.
    pub(crate) fn selected_object(&self) -> Option<NodeId> {
        let id = self.selection.selected_node;
        let scene = self.project.active_scene();
        (id != NodeId::ROOT && scene.node(id).is_some()).then(|| owner_object(scene, id))
    }

    /// A selection that did not come from a click on a row: open what hides
    /// the row (its group, the folders above it, a filter that excludes it)
    /// and ask for it to be scrolled into view.
    fn reveal_selected_object(&mut self, rows: &[ObjectRow]) {
        let subject = self.selected_object();
        if subject == self.objects_last_subject {
            return;
        }
        self.objects_last_subject = subject;
        let Some(id) = subject else {
            return;
        };
        let Some(row) = rows.iter().find(|row| row.id == id) else {
            return;
        };
        self.collapsed_object_groups.remove(&row.group());
        let mut parent = row.parent;
        while let Some(p) = parent {
            self.collapsed_scene_nodes.remove(&p);
            parent = self.project.active_scene().node(p).and_then(|n| n.parent);
        }
        let filter = self.scene_filter.to_ascii_lowercase();
        if !object_matches_filter(row, &filter) {
            self.scene_filter.clear();
        }
        self.objects_reveal = Some(id);
    }

    fn draw_objects_panel_body(&mut self, ui: &mut egui::Ui) {
        let rows = object_rows(self.project.active_scene());
        self.reveal_selected_object(&rows);
        ui.add(
            egui::TextEdit::singleline(&mut self.scene_filter)
                .hint_text("Filter objects")
                .desired_width(f32::INFINITY),
        );
        ui.separator();

        let filter = self.scene_filter.to_ascii_lowercase();
        let by_kind = self.objects_by_kind;
        let matching: Vec<ObjectRow> = if filter.is_empty() {
            rows.clone()
        } else {
            rows.iter()
                .filter(|row| object_matches_filter(row, &filter))
                .cloned()
                .collect()
        };
        let mut entries: Vec<Entry<'_>> = Vec::with_capacity(matching.len() + 6);
        if by_kind {
            for group in ObjectGroup::ALL {
                let members = rows_of_group(&matching, group);
                if members.is_empty() {
                    continue;
                }
                let open = !filter.is_empty() || !self.collapsed_object_groups.contains(&group);
                entries.push(Entry::Group {
                    group,
                    count: members.len(),
                    open,
                });
                if open {
                    entries.extend(members.into_iter().map(|row| Entry::Row { row, indent: 1 }));
                }
            }
        } else if filter.is_empty() {
            entries.extend(
                tree_display_rows(&matching, &self.collapsed_scene_nodes)
                    .into_iter()
                    .map(|row| Entry::Row {
                        row,
                        indent: row.depth,
                    }),
            );
        } else {
            entries.extend(matching.iter().map(|row| Entry::Row { row, indent: 0 }));
        }
        let visible_node_order: Vec<NodeId> = entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Row { row, .. } => Some(row.id),
                Entry::Group { .. } => None,
            })
            .collect();

        let reveal = self.objects_reveal.take();
        let mut toggle_group: Option<ObjectGroup> = None;
        let mut actions: Vec<TreeAction> = Vec::new();
        let collapsed_scene_nodes = self.collapsed_scene_nodes.clone();
        let hidden_scene_nodes = self.hidden_scene_nodes.clone();
        let selected_nodes = self.selection.selected_nodes.clone();
        let selected_node = self.selection.selected_node;
        let selected_object = self.selected_object();
        let scene = self.project.active_scene();
        let renaming = &mut self.renaming;
        let pending_focus = &mut self.pending_rename_focus;
        let tree_scroll_height = (ui.available_height() - 30.0).max(24.0);
        egui::ScrollArea::vertical()
            .id_salt("psxed_scene_tree")
            .auto_shrink([false, false])
            .max_height(tree_scroll_height)
            .show(ui, |ui| {
                for entry in &entries {
                    match entry {
                        Entry::Group { group, count, open } => {
                            if draw_group_header(ui, *group, *count, *open) {
                                toggle_group = Some(*group);
                            }
                        }
                        Entry::Row { row, indent } => {
                            let selected = selected_nodes.contains(&row.id)
                                || (selected_nodes.is_empty() && selected_node == row.id)
                                || selected_object == Some(row.id);
                            let state = RowState {
                                selected,
                                collapsed: collapsed_scene_nodes.contains(&row.id),
                                directly_hidden: hidden_scene_nodes.contains(&row.id),
                                effectively_hidden: scene_node_hidden(
                                    scene,
                                    &hidden_scene_nodes,
                                    row.id,
                                ),
                                can_snap_to_floor: owning_entity_id(scene, row.id).is_some(),
                                tree: !by_kind,
                                reveal: reveal == Some(row.id),
                            };
                            draw_object_row(
                                ui,
                                row,
                                *indent,
                                &state,
                                renaming,
                                pending_focus,
                                &mut actions,
                            );
                        }
                    }
                }
                autoscroll_tree_drag::<NodeId>(ui);
            });

        if let Some(group) = toggle_group {
            if !self.collapsed_object_groups.remove(&group) {
                self.collapsed_object_groups.insert(group);
            }
        }
        for action in actions {
            let from_row = matches!(action, TreeAction::Select { .. });
            self.apply_tree_action(action, &visible_node_order);
            if from_row {
                // The row the author clicked is where they are looking.
                self.objects_last_subject = self.selected_object();
            }
        }
        ui.horizontal(|ui| {
            if ui.button(icons::label(icons::COPY, "Duplicate")).clicked() {
                self.duplicate_selected();
            }
            let can_delete = self.selection.selected_node != NodeId::ROOT;
            if ui
                .add_enabled(
                    can_delete,
                    egui::Button::new(icons::label(icons::TRASH, "Delete")),
                )
                .clicked()
            {
                self.delete_selected();
            }
        });
    }
}

/// A group's header: a chevron, its name and how many it holds. Returns
/// whether it was clicked.
fn draw_group_header(ui: &mut egui::Ui, group: ObjectGroup, count: usize, open: bool) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
    let painter = ui.painter_at(rect);
    paint_list_row(&painter, rect, false, response.hovered());
    painter.text(
        Pos2::new(rect.left() + 10.0, rect.center().y),
        Align2::CENTER_CENTER,
        if open {
            icons::CHEVRON_DOWN
        } else {
            icons::CHEVRON_RIGHT
        }
        .to_string(),
        icons::font(12.0),
        STUDIO_TEXT_WEAK,
    );
    painter.text(
        Pos2::new(rect.left() + 24.0, rect.center().y),
        Align2::LEFT_CENTER,
        group.label(),
        FontId::proportional(12.5),
        STUDIO_TEXT,
    );
    let pill = Rect::from_min_size(
        Pos2::new(rect.right() - 30.0, rect.center().y - 8.0),
        Vec2::new(24.0, 16.0),
    );
    painter.rect_filled(pill, 8.0, Color32::from_rgba_unmultiplied(9, 14, 18, 138));
    painter.text(
        pill.center(),
        Align2::CENTER_CENTER,
        count.to_string(),
        FontId::monospace(10.0),
        STUDIO_TEXT_WEAK,
    );
    response
        .on_hover_text(if open {
            "Click to fold the group"
        } else {
            "Click to list the group"
        })
        .clicked()
}

/// `text` as one line of at most `max_width`, cut with an ellipsis only when
/// it does not fit.
fn fit_one_line(
    painter: &egui::Painter,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping {
        max_width: max_width.max(1.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    painter.layout_job(job)
}

fn draw_object_row(
    ui: &mut egui::Ui,
    row: &ObjectRow,
    indent: usize,
    state: &RowState,
    renaming: &mut Option<(NodeId, String)>,
    pending_focus: &mut bool,
    actions: &mut Vec<TreeAction>,
) {
    let row_height = 24.0;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_height),
        Sense::click_and_drag(),
    );
    if state.reveal {
        ui.scroll_to_rect(rect, None);
    }
    let painter = ui.painter_at(rect);
    let hovered = response.hovered();
    let selected = state.selected;
    paint_list_row(&painter, rect, selected, hovered);

    let is_folder = row.kind == ObjectKind::Folder;
    let content_left = rect.left() + 4.0 + indent as f32 * 16.0;
    if state.tree && indent > 0 {
        let line_x = rect.left() + 10.0 + ((indent - 1) as f32 * 16.0);
        painter.line_segment(
            [
                Pos2::new(line_x, rect.top()),
                Pos2::new(line_x, rect.bottom()),
            ],
            Stroke::new(1.0, STUDIO_TREE_GUIDE),
        );
    }

    // A folder in the tree has a chevron; every other row starts at its icon.
    let icon_left = if state.tree {
        content_left + 14.0
    } else {
        content_left
    };
    let chevron_response = if state.tree && is_folder && row.child_count > 0 {
        let chevron_rect = Rect::from_min_size(
            Pos2::new(content_left, rect.center().y - 5.0),
            Vec2::splat(10.0),
        );
        let r = ui
            .interact(
                chevron_rect.expand(4.0),
                ui.id().with(("objects_expand", row.id)),
                Sense::click(),
            )
            .on_hover_text(if state.collapsed {
                "Expand"
            } else {
                "Collapse"
            });
        painter.text(
            chevron_rect.center(),
            Align2::CENTER_CENTER,
            if state.collapsed {
                icons::CHEVRON_RIGHT
            } else {
                icons::CHEVRON_DOWN
            }
            .to_string(),
            icons::font(12.0),
            if r.hovered() {
                Color32::WHITE
            } else {
                Color32::from_rgb(160, 174, 188)
            },
        );
        Some(r)
    } else {
        None
    };

    let icon_rect = Rect::from_min_size(
        Pos2::new(icon_left, rect.center().y - 8.0),
        Vec2::splat(16.0),
    );
    painter.text(
        icon_rect.center(),
        Align2::CENTER_CENTER,
        row.kind.icon().to_string(),
        icons::font(15.0),
        if state.effectively_hidden {
            Color32::from_rgb(82, 94, 106)
        } else {
            row.icon_color()
        },
    );

    let text_color = if state.effectively_hidden {
        Color32::from_rgb(96, 110, 124)
    } else if selected {
        Color32::WHITE
    } else {
        STUDIO_TEXT
    };
    let text_left = icon_rect.right() + 7.0;
    // The eye is at the right end, a count before it for a folder with
    // contents. The name has everything between, and the detail takes what the
    // name leaves.
    let accessories_left = rect.right() - if row.child_count > 0 { 56.0 } else { 28.0 };
    let available = (accessories_left - text_left).max(0.0);
    let in_rename = matches!(renaming, Some((id, _)) if *id == row.id);

    if in_rename {
        let edit_rect = Rect::from_min_size(
            Pos2::new(text_left, rect.center().y - 10.0),
            Vec2::new(rect.right() - text_left - 30.0, 20.0),
        );
        if let Some((_, buffer)) = renaming.as_mut() {
            let edit_response = ui.put(
                edit_rect,
                egui::TextEdit::singleline(buffer)
                    .desired_width(edit_rect.width())
                    .margin(egui::Vec2::new(2.0, 1.0)),
            );
            if *pending_focus {
                edit_response.request_focus();
                *pending_focus = false;
            }
            let lost_focus = edit_response.lost_focus();
            let pressed_enter = lost_focus && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let pressed_esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if pressed_esc {
                actions.push(TreeAction::CancelRename);
            } else if pressed_enter || lost_focus {
                actions.push(TreeAction::CommitRename(row.id, buffer.clone()));
            }
        }
    }

    let name_galley_full =
        painter.layout_no_wrap(row.name.clone(), FontId::proportional(13.0), text_color);
    let detail_color = if state.effectively_hidden {
        Color32::from_rgb(84, 96, 108)
    } else if selected {
        STUDIO_TEXT
    } else {
        STUDIO_TEXT_WEAK
    };
    let detail_galley = row.detail.as_ref().map(|detail| {
        painter.layout_no_wrap(detail.clone(), FontId::proportional(11.0), detail_color)
    });
    let show_detail = detail_galley
        .as_ref()
        .is_some_and(|d| name_galley_full.size().x + 10.0 + d.size().x <= available);
    let name_width = if show_detail {
        name_galley_full.size().x
    } else {
        available
    };
    let truncated = name_galley_full.size().x > name_width + 0.5;
    if !in_rename {
        let galley = if truncated {
            fit_one_line(
                &painter,
                &row.name,
                FontId::proportional(13.0),
                text_color,
                name_width,
            )
        } else {
            name_galley_full
        };
        let pos = Pos2::new(text_left, rect.center().y - galley.size().y / 2.0);
        painter
            .with_clip_rect(Rect::from_min_max(
                Pos2::new(text_left, rect.top()),
                Pos2::new(accessories_left.max(text_left), rect.bottom()),
            ))
            .galley(pos, galley, text_color);
        if show_detail {
            if let Some(d) = detail_galley {
                let pos = Pos2::new(
                    accessories_left - d.size().x,
                    rect.center().y - d.size().y / 2.0,
                );
                painter.galley(pos, d, detail_color);
            }
        }
    }
    let response = if truncated && !in_rename {
        response.on_hover_text(row.name.clone())
    } else {
        response
    };

    if row.child_count > 0 {
        let pill = Rect::from_min_size(
            Pos2::new(rect.right() - 50.0, rect.center().y - 8.0),
            Vec2::new(24.0, 16.0),
        );
        painter.rect_filled(pill, 8.0, Color32::from_rgba_unmultiplied(9, 14, 18, 138));
        painter.text(
            pill.center(),
            Align2::CENTER_CENTER,
            row.child_count.to_string(),
            FontId::monospace(10.0),
            STUDIO_TEXT_WEAK,
        );
    }

    let eye_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 22.0, rect.center().y - 6.0),
        Vec2::new(14.0, 12.0),
    );
    let eye_response = ui
        .interact(
            eye_rect.expand2(Vec2::new(5.0, 4.0)),
            ui.id().with(("objects_visibility", row.id)),
            Sense::click(),
        )
        .on_hover_text(if state.directly_hidden {
            "Show in editor (hidden objects still ship in the build)"
        } else {
            "Hide in editor (still ships in the build, and a hidden light still lights)"
        });
    painter.text(
        eye_rect.center(),
        Align2::CENTER_CENTER,
        if state.effectively_hidden {
            icons::EYE_OFF
        } else {
            icons::EYE
        }
        .to_string(),
        icons::font(12.0),
        if eye_response.hovered() {
            Color32::WHITE
        } else if state.effectively_hidden {
            Color32::from_rgb(82, 92, 102)
        } else if selected || hovered {
            Color32::from_rgb(184, 205, 218)
        } else {
            Color32::from_rgb(88, 102, 116)
        },
    );

    if in_rename {
        return;
    }

    let chevron_clicked = chevron_response.as_ref().is_some_and(|r| r.clicked());
    if chevron_clicked {
        actions.push(TreeAction::ToggleExpanded(row.id));
    }
    if eye_response.clicked() {
        actions.push(TreeAction::ToggleVisibility(row.id));
    }
    let accessory_clicked = chevron_clicked || eye_response.clicked();

    if response.dragged() {
        response.dnd_set_drag_payload::<NodeId>(row.id);
        let pointer_pos = ui
            .ctx()
            .input(|i| i.pointer.interact_pos())
            .unwrap_or_else(|| rect.center());
        ui.painter().text(
            pointer_pos + Vec2::new(12.0, 0.0),
            Align2::LEFT_CENTER,
            row.name.clone(),
            FontId::proportional(12.0),
            STUDIO_ACCENT,
        );
    }

    // A drop lands in a folder, or (in the tree) before a row. An actor is
    // not a container: its parts are set from the Inspector.
    let pointer_pos = ui.ctx().input(|input| input.pointer.interact_pos());
    let drop_zone = tree_row_drop_zone(rect, pointer_pos, state.tree);
    let can_take = match drop_zone {
        TreeRowDropZone::Before => true,
        TreeRowDropZone::Inside => is_folder,
    };
    if can_take {
        if let Some(payload) = response.dnd_hover_payload::<NodeId>() {
            if *payload != row.id {
                match drop_zone {
                    TreeRowDropZone::Before => {
                        ui.painter().line_segment(
                            [
                                Pos2::new(rect.left() + 4.0, rect.top() + 1.0),
                                Pos2::new(rect.right() - 4.0, rect.top() + 1.0),
                            ],
                            Stroke::new(EDITOR_OUTLINE_STROKE_WIDTH, EDITOR_OUTLINE_ACCENT),
                        );
                    }
                    TreeRowDropZone::Inside => {
                        ui.painter().rect_stroke(
                            rect.shrink2(Vec2::new(2.0, 1.0)),
                            3.0,
                            Stroke::new(EDITOR_OUTLINE_STROKE_WIDTH, EDITOR_OUTLINE_ACCENT),
                            StrokeKind::Inside,
                        );
                    }
                }
            }
        }
        if let Some(payload) = response.dnd_release_payload::<NodeId>() {
            if *payload != row.id {
                match drop_zone {
                    TreeRowDropZone::Before => actions.push(TreeAction::Reparent {
                        source: *payload,
                        target_parent: row.parent.unwrap_or(NodeId::ROOT),
                        position: row.sibling_index,
                    }),
                    TreeRowDropZone::Inside => actions.push(TreeAction::Reparent {
                        source: *payload,
                        target_parent: row.id,
                        position: row.child_count,
                    }),
                }
            }
        }
    }

    if response.clicked() && !accessory_clicked {
        let modifiers = ui.input(|input| input.modifiers);
        actions.push(TreeAction::Select {
            id: row.id,
            modifiers,
        });
    }
    if response.double_clicked() && !accessory_clicked {
        actions.push(TreeAction::BeginRename(row.id));
    }

    response.context_menu(|ui| {
        if is_folder {
            ui.menu_button(icons::label(icons::PLUS, "Add"), |ui| {
                for (label, kind) in scene_graph_addable_kinds() {
                    if ui.button(label).clicked() {
                        actions.push(TreeAction::AddChild {
                            parent: row.id,
                            kind,
                            name: label,
                        });
                        ui.close_menu();
                    }
                }
            });
        }
        if ui.button(icons::label(icons::PALETTE, "Rename")).clicked() {
            actions.push(TreeAction::BeginRename(row.id));
            ui.close_menu();
        }
        if ui.button(icons::label(icons::COPY, "Duplicate")).clicked() {
            actions.push(TreeAction::Duplicate(row.id));
            ui.close_menu();
        }
        if ui
            .add_enabled(
                state.can_snap_to_floor,
                egui::Button::new(icons::label(icons::CHEVRON_DOWN, "Snap to Floor")),
            )
            .on_hover_text("Move the actor to the floor beneath it (End)")
            .clicked()
        {
            actions.push(TreeAction::SnapEntityToFloor(row.id));
            ui.close_menu();
        }
        ui.separator();
        if ui.button(icons::label(icons::TRASH, "Delete")).clicked() {
            actions.push(TreeAction::Delete(row.id));
            ui.close_menu();
        }
    });
}
