//! The tool strip: five labelled groups, every tool its own button with its
//! name and key, read from the one catalog.
//!
//! EDIT, BUILD, TERRAIN, SURFACE and OBJECTS, in that order. A group is never
//! split across rows: when the window is too narrow for one row the next
//! group wraps onto a second, and the strip grows to fit instead of clipping.
//! Place has one chip per kind of thing it puts down beside its button.

use super::catalog::tools_of;
use super::*;

/// Height of one button of the strip.
const BUTTON_HEIGHT: f32 = 26.0;
/// Vertical gap between two rows of the strip.
const ROW_GAP: f32 = 4.0;
/// Width a thin divider and its gaps take between two groups.
const GROUP_GAP_WIDTH: f32 = 13.0;
/// Slack added to every measured button for the spacing between buttons.
const BUTTON_SLACK: f32 = 3.0;

/// The kinds Place puts down: the word on the chip, the node it places.
const OBJECT_KINDS: [(&str, PlaceKind); 3] = [
    ("Light", PlaceKind::PointLightMarker),
    ("Prop", PlaceKind::ModelInstance),
    ("Actor", PlaceKind::Character),
];

/// A button with the tool's name and, in weak small type beside it, its key.
fn name_and_key_button(ui: &mut egui::Ui, name: &str, key: &str, selected: bool) -> egui::Response {
    let (name_galley, key_galley) = name_and_key_galleys(ui, name, key);
    let width = button_width(name_galley.size().x, key_galley.size().x);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, BUTTON_HEIGHT), Sense::click());
    let radius = egui::CornerRadius::same(CONTROL_RADIUS);
    if selected {
        ui.painter().rect_filled(rect, radius, STUDIO_SELECTION);
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, STUDIO_ACCENT),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() {
        ui.painter().rect_filled(rect, radius, STUDIO_HOVER);
    }
    let name_pos = Pos2::new(
        rect.min.x + 9.0,
        rect.center().y - name_galley.size().y / 2.0,
    );
    let key_pos = Pos2::new(
        name_pos.x + name_galley.size().x + 6.0,
        rect.center().y - key_galley.size().y / 2.0 + 1.0,
    );
    ui.painter().galley(name_pos, name_galley, STUDIO_TEXT);
    ui.painter().galley(key_pos, key_galley, STUDIO_TEXT_WEAK);
    response
}

/// The width of a button whose name and key measure `name` and `key`.
fn button_width(name: f32, key: f32) -> f32 {
    if key > 0.0 {
        9.0 + name + 6.0 + key + 9.0
    } else {
        9.0 + name + 9.0
    }
}

fn name_and_key_galleys(
    ui: &egui::Ui,
    name: &str,
    key: &str,
) -> (std::sync::Arc<egui::Galley>, std::sync::Arc<egui::Galley>) {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let key_font = egui::FontId::proportional(10.5);
    ui.fonts(|f| {
        (
            f.layout_no_wrap(name.to_string(), font, STUDIO_TEXT),
            f.layout_no_wrap(key.to_string(), key_font, STUDIO_TEXT_WEAK),
        )
    })
}

/// The small word that names a group.
fn group_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).color(STUDIO_TEXT_WEAK).size(9.5));
    ui.add_space(2.0);
}

/// A thin line between two groups.
fn group_gap(ui: &mut egui::Ui) {
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 16.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, STUDIO_BORDER);
    ui.add_space(6.0);
}

fn tool_hover(tool: GridTool) -> String {
    format!(
        "{} ({})\n{}",
        tool.label(),
        tool.key_label(),
        tool.tooltip()
    )
}

/// What one group of the strip holds.
fn group_tools(group: ToolGroup) -> Vec<GridTool> {
    tools_of(group)
        .filter(|tool| !CLASSIC.contains(tool) && !MORE_BUILD.contains(tool))
        .collect()
}

/// The tools the single Edit tool replaces. They stay available, in one menu
/// of the EDIT group, until the Edit tool has proven itself.
pub(crate) const CLASSIC: [GridTool; 3] = [GridTool::Select, GridTool::Height, GridTool::Extrude];

/// The older Build tools and the keyless Colonnade, in one menu of the BUILD
/// group so the strip stays within a row or two.
pub(crate) const MORE_BUILD: [GridTool; 6] = [
    GridTool::Colonnade,
    GridTool::Floor,
    GridTool::Ceiling,
    GridTool::Wall,
    GridTool::Plane,
    GridTool::Mesh,
];

/// The width the strip needs for `group`: its label, its buttons and, for
/// OBJECTS, the kind chips.
fn group_width(ctx: &egui::Context, group: ToolGroup) -> f32 {
    let button_font = egui::TextStyle::Button.resolve(&ctx.style());
    let key_font = egui::FontId::proportional(10.5);
    let label_font = egui::FontId::proportional(9.5);
    ctx.fonts(|f| {
        let label = f
            .layout_no_wrap(group.label().to_string(), label_font, STUDIO_TEXT_WEAK)
            .size()
            .x;
        let mut width = 4.0 + label + 2.0 + BUTTON_SLACK;
        for tool in group_tools(group) {
            let name = f
                .layout_no_wrap(tool.label().to_string(), button_font.clone(), STUDIO_TEXT)
                .size()
                .x;
            let key = f
                .layout_no_wrap(
                    tool.key_label().to_string(),
                    key_font.clone(),
                    STUDIO_TEXT_WEAK,
                )
                .size()
                .x;
            width += button_width(name, key) + BUTTON_SLACK;
        }
        if matches!(group, ToolGroup::Edit | ToolGroup::Build) {
            let word = if group == ToolGroup::Edit {
                "Classic"
            } else {
                "More"
            };
            let name = f
                .layout_no_wrap(word.to_string(), button_font.clone(), STUDIO_TEXT)
                .size()
                .x;
            width += button_width(name, 0.0) + BUTTON_SLACK;
        }
        if group == ToolGroup::Objects {
            for (kind, _) in OBJECT_KINDS {
                let name = f
                    .layout_no_wrap(kind.to_string(), button_font.clone(), STUDIO_TEXT)
                    .size()
                    .x;
                width += button_width(name, 0.0) + BUTTON_SLACK;
            }
        }
        width + GROUP_GAP_WIDTH
    })
}

/// The groups on each row of the strip at this window width: whole groups,
/// filled greedily.
fn strip_rows(ctx: &egui::Context) -> Vec<Vec<ToolGroup>> {
    let available = ctx.screen_rect().width() - 20.0;
    let mut rows: Vec<Vec<ToolGroup>> = vec![Vec::new()];
    let mut used = 0.0;
    for group in ToolGroup::ALL {
        let width = group_width(ctx, group);
        if used + width > available && !rows.last().is_some_and(|r| r.is_empty()) {
            rows.push(Vec::new());
            used = 0.0;
        }
        used += width;
        if let Some(row) = rows.last_mut() {
            row.push(group);
        }
    }
    rows
}

/// The strip's height at this window width: one row is
/// `TOOL_ROW_HEIGHT`, each more adds a button and a gap.
pub(crate) fn tool_strip_height(ctx: &egui::Context) -> f32 {
    TOOL_ROW_HEIGHT + (strip_rows(ctx).len() as f32 - 1.0) * (BUTTON_HEIGHT + ROW_GAP)
}

impl EditorWorkspace {
    /// The strip itself, a panel under the top bar across the whole window.
    pub(crate) fn draw_grid_tool_row(&mut self, ctx: &egui::Context) {
        let rows = strip_rows(ctx);
        let height = tool_strip_height(ctx);
        egui::TopBottomPanel::top("psxed_tool_row")
            .exact_height(height)
            .frame(
                egui::Frame::new()
                    .fill(STUDIO_PANEL_HEADER)
                    .inner_margin(egui::Margin::symmetric(10, 5)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(2.0, ROW_GAP);
                ui.set_clip_rect(ui.max_rect().expand2(Vec2::new(0.0, 4.0)));
                for row in rows {
                    ui.horizontal(|ui| {
                        ui.set_height(BUTTON_HEIGHT);
                        for (n, group) in row.into_iter().enumerate() {
                            if n > 0 {
                                group_gap(ui);
                            }
                            self.draw_strip_group(ui, group);
                        }
                    });
                }
            });
    }

    fn draw_strip_group(&mut self, ui: &mut egui::Ui, group: ToolGroup) {
        group_label(ui, group.label());
        // The single Edit tool takes the first button of the EDIT group;
        // Select, Height and Extrude wait in the Classic menu beside it.
        for tool in group_tools(group) {
            self.row_tool(ui, tool);
        }
        if group == ToolGroup::Edit {
            self.row_menu(
                ui,
                "Classic",
                "psxed_classic_tools",
                &CLASSIC,
                "The separate Select, Height and Extrude tools",
            );
        }
        if group == ToolGroup::Build {
            self.row_menu(
                ui,
                "More",
                "psxed_more_build_tools",
                &MORE_BUILD,
                "Colonnade, and the floor, ceiling, wall, plane and mesh tools",
            );
        }
        if group == ToolGroup::Objects {
            self.row_place_kinds(ui);
        }
    }

    /// A menu of tools: Classic (the tools the Edit tool replaces) or More.
    fn row_menu(&mut self, ui: &mut egui::Ui, name: &str, id: &str, tools: &[GridTool], tip: &str) {
        let active = tools.contains(&self.grid.tool);
        let response = name_and_key_button(ui, name, "", active).on_hover_text(tip);
        let id = ui.make_persistent_id(id);
        if response.clicked() {
            ui.memory_mut(|m| m.toggle_popup(id));
        }
        egui::popup::popup_below_widget(
            ui,
            id,
            &response,
            egui::PopupCloseBehavior::CloseOnClick,
            |ui| {
                ui.set_min_width(130.0);
                for &tool in tools {
                    let label = format!("{}  {}", tool.label(), tool.key_label());
                    if ui
                        .selectable_label(self.grid.tool == tool, label)
                        .on_hover_text(tool_hover(tool))
                        .clicked()
                    {
                        self.set_grid_tool(tool);
                    }
                }
            },
        );
    }

    /// A plain tool button: its name, its key, lit while it is the tool.
    fn row_tool(&mut self, ui: &mut egui::Ui, tool: GridTool) {
        let response =
            name_and_key_button(ui, tool.label(), tool.key_label(), self.grid.tool == tool)
                .on_hover_text(tool_hover(tool));
        if response.clicked() {
            self.set_grid_tool(tool);
        }
    }

    /// One chip per kind Place puts down; the chosen kind is lit while Place
    /// is the tool, and a click both picks the kind and makes Place the tool.
    fn row_place_kinds(&mut self, ui: &mut egui::Ui) {
        let active = self.grid.tool == GridTool::Place;
        for (label, kind) in OBJECT_KINDS {
            let selected = active && self.place_kind == kind;
            let response = name_and_key_button(ui, label, "", selected)
                .on_hover_text(format!("Place a {} (Place, O)", label.to_lowercase()));
            if response.clicked() {
                self.place_kind = kind;
                self.set_grid_tool(GridTool::Place);
                self.status = format!("Tool: Place, {}", kind.label());
            }
        }
    }
}
