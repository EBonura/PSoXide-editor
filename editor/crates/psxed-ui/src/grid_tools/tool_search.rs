//! The tool search: `/` (or Cmd+K) opens a small searchable list of every
//! tool by name, so nothing depends on remembering a letter. Type to filter,
//! Up and Down move, Enter picks, Esc closes.

use super::catalog::CATALOG;
use super::tool_row::tool_icon;
use super::*;

/// The open search box.
#[derive(Clone, Debug, Default)]
pub(crate) struct ToolSearch {
    /// What is typed.
    pub query: String,
    /// The highlighted row of the matches.
    pub cursor: usize,
}

/// The tools whose name or group contains every word of `query`, in strip
/// order.
pub(crate) fn matches(query: &str) -> Vec<GridTool> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut found: Vec<GridTool> = CATALOG
        .iter()
        .filter(|entry| {
            let name = entry.tool.label().to_lowercase();
            let group = entry.group.label().to_lowercase();
            words
                .iter()
                .all(|word| name.contains(word.as_str()) || group.contains(word.as_str()))
        })
        .map(|entry| entry.tool)
        .collect();
    // A name typed in full comes first ("Floor" before "Upper floor").
    let typed = query.trim().to_lowercase();
    found.sort_by_key(|tool| tool.label().to_lowercase() != typed);
    found
}

impl EditorWorkspace {
    /// `/` and Cmd+K open the search; call from the grid shortcuts.
    pub(crate) fn handle_tool_search_key(&mut self, ctx: &egui::Context) {
        let open = ctx.input_mut(|i| {
            (!i.modifiers.shift && i.consume_key(egui::Modifiers::NONE, egui::Key::Slash))
                || i.consume_key(egui::Modifiers::COMMAND, egui::Key::K)
        });
        if open {
            self.grid.tool_search = Some(ToolSearch::default());
        }
    }

    /// Draw the search when it is open.
    pub(crate) fn draw_tool_search(&mut self, ctx: &egui::Context) {
        let Some(mut search) = self.grid.tool_search.take() else {
            return;
        };
        let mut picked = None;
        let mut close = false;
        let found = matches(&search.query);
        search.cursor = search.cursor.min(found.len().saturating_sub(1));
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                search.cursor = (search.cursor + 1).min(found.len().saturating_sub(1));
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                search.cursor = search.cursor.saturating_sub(1);
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                close = true;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                picked = found.get(search.cursor).copied();
            }
        });
        egui::Area::new(egui::Id::new("psxed_tool_search"))
            .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 120.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(320.0);
                    ui.label(
                        RichText::new("Go to a tool")
                            .color(STUDIO_TEXT_WEAK)
                            .size(11.0),
                    );
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut search.query)
                            .hint_text("Type a tool name")
                            .desired_width(f32::INFINITY),
                    );
                    edit.request_focus();
                    if search.query == "/" {
                        search.query.clear();
                    }
                    if edit.changed() {
                        search.cursor = 0;
                    }
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            for (row, tool) in found.iter().copied().enumerate() {
                                let key = tool.key_label();
                                let label = format!(
                                    "{}  {}   {}{}",
                                    tool_icon(tool),
                                    tool.label(),
                                    catalog::entry(tool).group.label().to_lowercase(),
                                    if key.is_empty() {
                                        String::new()
                                    } else {
                                        format!("   ({key})")
                                    }
                                );
                                let response = ui.selectable_label(row == search.cursor, label);
                                if response.clicked() {
                                    picked = Some(tool);
                                }
                            }
                            if found.is_empty() {
                                ui.label(
                                    RichText::new("No tool by that name").color(STUDIO_TEXT_WEAK),
                                );
                            }
                        });
                });
            });
        if let Some(tool) = picked {
            self.set_grid_tool(tool);
            self.status = format!("Tool: {}", tool.label());
        } else if !close {
            self.grid.tool_search = Some(search);
        }
    }
}
