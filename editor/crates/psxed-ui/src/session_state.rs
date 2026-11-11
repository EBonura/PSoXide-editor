//! What the editor remembers between runs.
//!
//! Per user (one file in the user's configuration folder): which docks are
//! open and how wide they are, whether the Library drawer is open, its tab,
//! and how tall each of its views is. Per project (a file beside
//! `project.ron`, never part of the project): the active tool and the storey
//! the cursor was on. Both are plain `key value` lines, so a damaged or
//! older file is read as far as it goes and never stops the editor opening.

use std::path::{Path, PathBuf};

/// The project's file of what was last being done in it.
pub(crate) const SESSION_FILE: &str = ".psxed-session.ron";
/// The user's layout file name inside the configuration folder.
pub(crate) const LAYOUT_FILE: &str = "editor-layout.txt";

/// The layout the user arranged.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UserLayout {
    pub left_open: bool,
    pub inspector_open: bool,
    pub drawer_open: bool,
    /// 0 Assets, 1 Console, 2 Library.
    pub drawer_view: u8,
    /// The Library's tab, by its index.
    pub library_tab: u8,
    pub left_width: Option<f32>,
    pub inspector_width: Option<f32>,
    /// Heights of the drawer per view: Assets, Console, Library.
    pub drawer_heights: [Option<f32>; 3],
    /// Share of the left dock Structure takes.
    pub structure_fraction: f32,
}

impl Default for UserLayout {
    fn default() -> Self {
        Self {
            left_open: true,
            inspector_open: true,
            drawer_open: false,
            drawer_view: 0,
            library_tab: 0,
            left_width: None,
            inspector_width: None,
            drawer_heights: [None; 3],
            structure_fraction: 0.6,
        }
    }
}

fn opt(value: Option<f32>) -> String {
    value.map_or("auto".to_string(), |v| format!("{v:.0}"))
}

fn parse_opt(text: &str) -> Option<f32> {
    text.parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
}

impl UserLayout {
    /// The file's text.
    pub(crate) fn to_text(&self) -> String {
        let mut out = String::new();
        let mut line = |k: &str, v: String| {
            out.push_str(k);
            out.push(' ');
            out.push_str(&v);
            out.push('\n');
        };
        line("left_open", self.left_open.to_string());
        line("inspector_open", self.inspector_open.to_string());
        line("drawer_open", self.drawer_open.to_string());
        line("drawer_view", self.drawer_view.to_string());
        line("library_tab", self.library_tab.to_string());
        line("left_width", opt(self.left_width));
        line("inspector_width", opt(self.inspector_width));
        line("assets_height", opt(self.drawer_heights[0]));
        line("console_height", opt(self.drawer_heights[1]));
        line("library_height", opt(self.drawer_heights[2]));
        line(
            "structure_fraction",
            format!("{:.2}", self.structure_fraction),
        );
        out
    }

    /// Read a file's text over the defaults; lines it does not understand
    /// are skipped.
    pub(crate) fn from_text(text: &str) -> Self {
        let mut out = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once(' ') else {
                continue;
            };
            let value = value.trim();
            match key {
                "left_open" => out.left_open = value != "false",
                "inspector_open" => out.inspector_open = value != "false",
                "drawer_open" => out.drawer_open = value == "true",
                "drawer_view" => out.drawer_view = value.parse().unwrap_or(0).min(2),
                "library_tab" => out.library_tab = value.parse().unwrap_or(0),
                "left_width" => out.left_width = parse_opt(value),
                "inspector_width" => out.inspector_width = parse_opt(value),
                "assets_height" => out.drawer_heights[0] = parse_opt(value),
                "console_height" => out.drawer_heights[1] = parse_opt(value),
                "library_height" => out.drawer_heights[2] = parse_opt(value),
                "structure_fraction" => {
                    if let Ok(v) = value.parse::<f32>() {
                        out.structure_fraction = v.clamp(0.2, 0.85);
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// Where the user's layout file lives, or `None` when this run must not
/// read or write one (`PSXED_NO_USER_PREFS`, which headless captures set, and
/// every test). `PSXED_CONFIG_DIR` moves it, like the frontend's
/// `--config-dir`.
pub(crate) fn user_layout_path() -> Option<PathBuf> {
    if cfg!(test) || std::env::var_os("PSXED_NO_USER_PREFS").is_some() {
        return None;
    }
    if let Some(dir) = std::env::var_os("PSXED_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join(LAYOUT_FILE));
    }
    directories::ProjectDirs::from("com", "psoxide", "PSoXide")
        .map(|dirs| dirs.config_dir().join(LAYOUT_FILE))
}

/// Read the layout at `path`, if there is one.
pub(crate) fn read_layout(path: &Path) -> Option<UserLayout> {
    std::fs::read_to_string(path)
        .ok()
        .map(|text| UserLayout::from_text(&text))
}

/// Write the layout to `path`, creating its folder.
pub(crate) fn write_layout(path: &Path, layout: &UserLayout) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, layout.to_text())
}

/// What a project remembers of its last session.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProjectSession {
    /// The tool, by its label.
    pub tool: Option<String>,
    /// The storey cursor: `None` is all storeys.
    pub storey: Option<i32>,
}

impl ProjectSession {
    pub(crate) fn to_text(&self) -> String {
        let mut out = String::new();
        if let Some(tool) = &self.tool {
            out.push_str(&format!("tool {tool}\n"));
        }
        out.push_str(&match self.storey {
            Some(y) => format!("storey {y}\n"),
            None => "storey all\n".to_string(),
        });
        out
    }

    pub(crate) fn from_text(text: &str) -> Self {
        let mut out = Self::default();
        for line in text.lines() {
            match line.split_once(' ') {
                Some(("tool", name)) => out.tool = Some(name.trim().to_string()),
                Some(("storey", v)) => out.storey = v.trim().parse().ok(),
                _ => {}
            }
        }
        out
    }

    pub(crate) fn read(project_dir: &Path) -> Option<Self> {
        std::fs::read_to_string(project_dir.join(SESSION_FILE))
            .ok()
            .map(|t| Self::from_text(&t))
    }

    pub(crate) fn write(&self, project_dir: &Path) -> std::io::Result<()> {
        std::fs::write(project_dir.join(SESSION_FILE), self.to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layout_round_trips_and_a_damaged_file_still_reads() {
        let layout = UserLayout {
            left_open: false,
            inspector_open: true,
            drawer_open: true,
            drawer_view: 2,
            library_tab: 3,
            left_width: Some(310.0),
            inspector_width: None,
            drawer_heights: [None, Some(180.0), Some(420.0)],
            structure_fraction: 0.45,
        };
        assert_eq!(UserLayout::from_text(&layout.to_text()), layout);
        let damaged = "left_open false\nnonsense\nleft_width wide\ndrawer_view 9\n";
        let read = UserLayout::from_text(damaged);
        assert!(!read.left_open);
        assert_eq!(read.left_width, None);
        assert_eq!(
            read.drawer_view, 2,
            "an out of range view is held to the last"
        );
    }

    #[test]
    fn a_session_round_trips() {
        let s = ProjectSession {
            tool: Some("Corridor".to_string()),
            storey: Some(3072),
        };
        assert_eq!(ProjectSession::from_text(&s.to_text()), s);
        let all = ProjectSession::from_text("tool Edit\nstorey all\n");
        assert_eq!(all.storey, None);
    }
}
