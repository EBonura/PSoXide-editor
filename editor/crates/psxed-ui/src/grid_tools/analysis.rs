//! The overlays' analyses, kept per document revision.
//!
//! Both overlays are computed from the document alone, cached against
//! [`GridEditState::rev`] and dropped the moment the document changes, so a
//! stale tint is never drawn. The walkability analysis is cheap (milliseconds
//! on the biggest level) and runs on the drawing thread; the time of the last
//! run is kept for the legend.

use std::rc::Rc;
use std::time::Instant;

use psxed_project::grid::walk::{analyse, locate_spawn, Spawn, WalkAnalysis};
use psxed_project::grid::GridWorld;
use psxed_project::NodeKind;

use super::*;

/// Which overlays are on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Overlays {
    /// Walkability (Alt+W).
    pub walk: bool,
    /// Performance heatmap (Alt+P).
    pub perf: bool,
}

/// The cached analyses.
#[derive(Default)]
pub(crate) struct AnalysisCache {
    walk: Option<WalkEntry>,
}

struct WalkEntry {
    rev: u64,
    analysis: Rc<WalkAnalysis>,
    spawns: usize,
    millis: f32,
}

/// What the overlay needs of a walkability run.
pub(crate) struct WalkRun {
    pub analysis: Rc<WalkAnalysis>,
    /// Spawns the reachability started from.
    pub spawns: usize,
    /// How long the run took, milliseconds.
    pub millis: f32,
}

/// The player spawns of the active scene, as the sectors they stand over.
pub(crate) fn player_spawns(project: &ProjectDocument, world: &GridWorld) -> Vec<Spawn> {
    let scene = project.active_scene();
    let mut out = Vec::new();
    for node in scene.nodes() {
        let is_spawn = matches!(node.kind, NodeKind::SpawnPoint { player: true, .. })
            || node.children.iter().any(|c| {
                scene.node(*c).is_some_and(|k| {
                    matches!(k.kind, NodeKind::CharacterController { player: true, .. })
                })
            });
        if is_spawn {
            if let Some(spawn) = locate_spawn(world, node.transform.translation) {
                out.push(spawn);
            }
        }
    }
    out
}

impl EditorWorkspace {
    /// The walkability analysis of the current document, computed once per
    /// revision.
    pub(crate) fn walk_run(&self) -> Option<WalkRun> {
        let world = self.project.active_scene().world.grid()?;
        let mut cache = self.grid.analysis.borrow_mut();
        if cache.walk.as_ref().is_none_or(|e| e.rev != self.grid.rev) {
            let spawns = player_spawns(&self.project, world);
            let started = Instant::now();
            let analysis = analyse(world, &spawns, &psx_engine::WalkLimits::DEFAULT)?;
            cache.walk = Some(WalkEntry {
                rev: self.grid.rev,
                analysis: Rc::new(analysis),
                spawns: spawns.len(),
                millis: started.elapsed().as_secs_f32() * 1000.0,
            });
        }
        let entry = cache.walk.as_ref()?;
        Some(WalkRun {
            analysis: entry.analysis.clone(),
            spawns: entry.spawns,
            millis: entry.millis,
        })
    }

    /// Alt+W.
    pub(crate) fn toggle_walk_overlay(&mut self) {
        self.grid.overlays.walk = !self.grid.overlays.walk;
        self.status = if self.grid.overlays.walk {
            "Walkability overlay on (Alt+W)".to_string()
        } else {
            "Walkability overlay off".to_string()
        };
    }
}
