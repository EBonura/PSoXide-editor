//! Styles the Style picker lists: the built-ins, then the ones stored with
//! the project in its `styles/` folder (`<name>.style.ron` for a style,
//! `<name>.bay.ron` for a bay captured from the level).

use std::path::Path;

use psxed_project::grid::edit::verbs::capture::{capture_bay, BayTemplate};
use psxed_project::grid::edit::verbs::style::{built_in, Style};
use psxed_project::grid::edit::CellRect;
use psxed_project::grid::GridWorld;

use super::options::{StyleEntry, StyleResolved};

/// The folder a project keeps its styles in.
pub(crate) fn dir(project_dir: &Path) -> std::path::PathBuf {
    project_dir.join("styles")
}

/// Every style: plain, the built-ins, then the project's.
pub(crate) fn list(project_dir: &Path) -> Vec<StyleEntry> {
    let mut out = vec![StyleEntry {
        name: "Plain".to_string(),
        what: StyleResolved::Plain,
        project: false,
    }];
    out.extend(
        built_in()
            .into_iter()
            .filter(|s| s.name != "plain" && s.name != "Plain")
            .map(|s| StyleEntry {
                name: s.name.clone(),
                what: StyleResolved::Style(s),
                project: false,
            }),
    );
    let mut stored: Vec<StyleEntry> = Vec::new();
    if let Ok(read) = std::fs::read_dir(dir(project_dir)) {
        for entry in read.flatten() {
            let path = entry.path();
            let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(name) = file.strip_suffix(".style.ron") {
                if let Ok(style) = Style::from_ron(&text) {
                    stored.push(StyleEntry {
                        name: name.to_string(),
                        what: StyleResolved::Style(style),
                        project: true,
                    });
                }
            } else if let Some(name) = file.strip_suffix(".bay.ron") {
                if let Ok(bay) = ron::from_str::<BayTemplate>(&text) {
                    stored.push(StyleEntry {
                        name: format!("{name} (bay)"),
                        what: StyleResolved::Bay(bay),
                        project: true,
                    });
                }
            }
        }
    }
    stored.sort_by(|a, b| a.name.cmp(&b.name));
    out.extend(stored);
    out
}

/// Capture the bay `rect` of `room` as a named style stored with the project.
pub(crate) fn capture(
    project_dir: &Path,
    world: &GridWorld,
    room: usize,
    rect: CellRect,
    name: &str,
) -> Result<String, String> {
    let name: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        return Err("Name the style first".to_string());
    }
    let bay = capture_bay(world, room, rect).map_err(|e| e.describe())?;
    let text = ron::ser::to_string_pretty(&bay, ron::ser::PrettyConfig::default())
        .map_err(|e| e.to_string())?;
    let folder = dir(project_dir);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    std::fs::write(folder.join(format!("{name}.bay.ron")), text).map_err(|e| e.to_string())?;
    Ok(name)
}
