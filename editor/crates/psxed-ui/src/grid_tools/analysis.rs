//! The overlays' analyses, kept per document revision.
//!
//! Both overlays are computed from the document alone, cached against
//! [`GridEditState::rev`] and dropped the moment the document changes, so a
//! stale tint is never drawn. The walkability analysis is cheap (milliseconds
//! on the biggest level) and runs on the drawing thread; the time of the last
//! run is kept for the legend.

use std::rc::Rc;
use std::time::Instant;

use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use psxed_project::grid::heat::{heatmap, HeatMap, HeatOptions};
use psxed_project::grid::walk::{analyse, WalkAnalysis};

use super::*;

/// Which overlays are on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Overlays {
    /// Walkability (Alt+W).
    pub walk: bool,
    /// Performance heatmap (Alt+P).
    pub perf: bool,
    /// Cycles the frame spends outside the world, editable: the player and
    /// the sky (470,000 and 58,000 measured on the Cathedral).
    pub reserve: u32,
    /// Compute the heatmap on the drawing thread (poses and tests, which
    /// capture one frame and cannot wait for a worker).
    pub heat_sync: bool,
}

impl Default for Overlays {
    fn default() -> Self {
        Self {
            walk: false,
            perf: false,
            reserve: HeatOptions::default().reserve,
            heat_sync: false,
        }
    }
}

/// The cached analyses.
#[derive(Default)]
pub(crate) struct AnalysisCache {
    walk: Option<WalkEntry>,
    heat: Option<HeatEntry>,
    job: Option<HeatJob>,
    /// The revision last seen and when it appeared, for the debounce.
    seen: Option<(u64, Instant)>,
    /// What the last failed run said.
    heat_error: Option<String>,
}

struct HeatEntry {
    rev: u64,
    reserve: u32,
    map: Arc<HeatMap>,
}

struct HeatJob {
    rev: u64,
    reserve: u32,
    rx: Receiver<Result<HeatMap, String>>,
}

/// How long the document must stay still before the heatmap is recomputed.
const HEAT_DEBOUNCE_MS: u128 = 500;

/// What the heatmap overlay needs.
pub(crate) struct HeatRun {
    pub map: Arc<HeatMap>,
    /// The result is for the current document and reserve.
    pub fresh: bool,
    /// A run is in flight.
    pub computing: bool,
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

impl EditorWorkspace {
    /// The walkability analysis of the current document, computed once per
    /// revision.
    pub(crate) fn walk_run(&self) -> Option<WalkRun> {
        let world = self.project.active_scene().world.grid()?;
        let mut cache = self.grid.analysis.borrow_mut();
        if cache.walk.as_ref().is_none_or(|e| e.rev != self.grid.rev) {
            let spawns = psxed_project::grid::walk::player_spawns(&self.project, world);
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

    /// The options of the heatmap for this document: the editable reserve
    /// and the eye height of the world camera.
    fn heat_options(&self, coarse: bool) -> HeatOptions {
        let mut o = HeatOptions {
            reserve: self.grid.overlays.reserve,
            ..HeatOptions::default()
        };
        if let Some(camera) = self
            .project
            .active_scene()
            .world_camera_for_node(psxed_project::NodeId::ROOT)
        {
            o.eye = (camera.height / psxed_project::units::WORLD_UNIT_DIVISOR).max(8);
            o.arm = (camera.distance / psxed_project::units::WORLD_UNIT_DIVISOR).max(0);
        }
        if coarse {
            o.stride = 2;
            o.yaws = 4;
        }
        o
    }

    /// The heatmap of the current document, if there is one yet. A fresh
    /// result is returned as such; otherwise the stale one (or none) and a
    /// run is started once the document has been still for half a second
    /// (a coarse pass while a gesture is in flight and there is nothing to
    /// show). With `heat_sync` the run happens right here.
    pub(crate) fn heat_run(&self) -> Option<HeatRun> {
        let world = self.project.active_scene().world.grid()?;
        let mut cache = self.grid.analysis.borrow_mut();
        let rev = self.grid.rev;
        let reserve = self.grid.overlays.reserve;
        // Collect a finished job.
        if let Some(job) = &cache.job {
            match job.rx.try_recv() {
                Ok(Ok(map)) => {
                    let (rev, reserve) = (job.rev, job.reserve);
                    cache.heat = Some(HeatEntry {
                        rev,
                        reserve,
                        map: Arc::new(map),
                    });
                    cache.heat_error = None;
                    cache.job = None;
                }
                Ok(Err(e)) => {
                    cache.heat_error = Some(e);
                    cache.job = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => cache.job = None,
            }
        }
        if cache.seen.is_none_or(|(r, _)| r != rev) {
            cache.seen = Some((rev, Instant::now()));
        }
        let fresh = cache
            .heat
            .as_ref()
            .is_some_and(|e| e.rev == rev && e.reserve == reserve);
        if !fresh && cache.job.is_none() {
            let calm = cache
                .seen
                .is_some_and(|(_, at)| at.elapsed().as_millis() >= HEAT_DEBOUNCE_MS);
            let busy = !matches!(self.grid.gesture, Gesture::Idle);
            let coarse = busy && cache.heat.is_none();
            if self.grid.overlays.heat_sync {
                match heatmap(world, self.heat_options(false)) {
                    Ok(map) => {
                        cache.heat = Some(HeatEntry {
                            rev,
                            reserve,
                            map: Arc::new(map),
                        });
                        cache.heat_error = None;
                    }
                    Err(e) => cache.heat_error = Some(e),
                }
            } else if (calm && !busy) || coarse {
                let world = world.clone();
                let options = self.heat_options(coarse);
                let (tx, rx) = channel();
                std::thread::spawn(move || {
                    let _ = tx.send(heatmap(&world, options));
                });
                cache.job = Some(HeatJob { rev, reserve, rx });
            }
        }
        let computing = cache.job.is_some();
        let entry = cache.heat.as_ref()?;
        Some(HeatRun {
            map: entry.map.clone(),
            fresh: entry.rev == rev && entry.reserve == reserve,
            computing,
        })
    }

    /// The last heatmap failure, if the overlay has nothing to show.
    pub(crate) fn heat_error(&self) -> Option<String> {
        self.grid.analysis.borrow().heat_error.clone()
    }

    /// Alt+P.
    pub(crate) fn toggle_perf_overlay(&mut self) {
        self.grid.overlays.perf = !self.grid.overlays.perf;
        self.status = if self.grid.overlays.perf {
            "Performance heatmap on (Alt+P): a model, see the legend".to_string()
        } else {
            "Performance heatmap off".to_string()
        };
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
