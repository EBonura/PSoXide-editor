//! The global light of the preview's grid world, solved off the UI thread.
//!
//! Direct light and its shadows are cheap and bake the moment a face is first
//! drawn after an edit. The ambient occlusion, sky light and bounce of a
//! world that gathers its light (`GridWorld::global_light`) take a while, so
//! they are solved on a worker thread from a copy of the world and swapped in
//! when done: until then the preview shows the flat room ambient (or the
//! previous solution, for the faces it still matches), and the faces are
//! baked again once the new field arrives. The solve refines in two steps: a
//! draft with few rays and one bounce lands first, so a large world is not
//! flat for long, then the full solve the cook would make replaces it. An
//! edit while a solve runs cancels it and starts another.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use psxed_project::grid::light::GridLight;
use psxed_project::grid::lighting::gather::{solve, GatherInput};
use psxed_project::grid::lighting::{AlbedoTable, GiField, LightScene};
use psxed_project::grid::GridWorld;
use psxed_project::ProjectDocument;

/// What a finished solve reports.
struct Solved {
    field: GiField,
    took: Duration,
    /// The solve the cook would make; false for the draft before it.
    full: bool,
}

struct Job {
    cancel: Arc<AtomicBool>,
    done: Receiver<Option<Solved>>,
}

/// Rays and bounces of the draft solve.
const DRAFT_RAYS: u8 = 16;
const DRAFT_BOUNCES: u8 = 1;

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// The solved field of the world, the solve in flight, and a counter that
/// moves whenever the field changes so a cached frame knows to be redrawn.
#[derive(Default)]
pub(super) struct GlobalLightState {
    field: Option<Arc<GiField>>,
    job: Option<Job>,
    generation: u64,
    /// How long the last finished solve took.
    last_took: Option<Duration>,
}

impl GlobalLightState {
    /// The solved field, if there is one.
    pub(super) fn field(&self) -> Option<&GiField> {
        self.field.as_deref()
    }

    /// Moves whenever the field changes.
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether a solve is still running.
    pub(super) fn pending(&self) -> bool {
        self.job.is_some()
    }

    /// How long the last finished solve took.
    #[allow(dead_code)]
    pub(super) fn last_took(&self) -> Option<Duration> {
        self.last_took
    }

    /// Forget the field and stop any solve: the world no longer gathers.
    pub(super) fn clear(&mut self) {
        self.job = None;
        if self.field.take().is_some() {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    /// Solve `world` again on a worker, in place of any solve in flight. The
    /// field already held stays in use until the new one lands.
    pub(super) fn restart(
        &mut self,
        project: &ProjectDocument,
        world: &GridWorld,
        lights: Vec<GridLight>,
        project_root: Option<PathBuf>,
    ) {
        self.job = None;
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, done) = channel();
        let (project, world) = (project.clone(), world.clone());
        let flag = Arc::clone(&cancel);
        let spawned = std::thread::Builder::new()
            .name("grid-global-light".into())
            .spawn(move || {
                let light = world.global_light.clamped();
                let draft_is_cheaper = light.rays > DRAFT_RAYS || light.bounces > DRAFT_BOUNCES;
                let mut draft_world = world.clone();
                draft_world.global_light = psxed_project::grid::GlobalLight {
                    rays: light.rays.min(DRAFT_RAYS),
                    bounces: light.bounces.min(DRAFT_BOUNCES),
                    ..light
                };
                let inputs = Inputs {
                    project: &project,
                    lights: &lights,
                    project_root,
                    cancel: &flag,
                };
                if draft_is_cheaper {
                    let draft = solve_world(&inputs, &draft_world, false);
                    if draft.is_none() || tx.send(draft).is_err() {
                        return;
                    }
                }
                let _ = tx.send(solve_world(&inputs, &world, true));
            });
        if spawned.is_ok() {
            self.job = Some(Job { cancel, done });
        }
    }

    /// Take the newest solve that has landed, if any. True when the field
    /// changed.
    pub(super) fn poll(&mut self) -> bool {
        let mut changed = false;
        loop {
            let Some(job) = &self.job else {
                return changed;
            };
            match job.done.try_recv() {
                Ok(Some(solved)) => {
                    let full = solved.full;
                    self.adopt(solved);
                    changed = true;
                    if full {
                        self.job = None;
                        return true;
                    }
                }
                // A cancelled or failed solve ends the job.
                Ok(None) | Err(TryRecvError::Disconnected) => {
                    self.job = None;
                    return changed;
                }
                Err(TryRecvError::Empty) => return changed,
            }
        }
    }

    /// Block until the solve in flight is done and take it: for a one-shot
    /// frame (the command line, a test) that has no later frame to refine in.
    pub(super) fn wait(&mut self) -> bool {
        let mut changed = false;
        while let Some(job) = &self.job {
            match job.done.recv() {
                Ok(Some(solved)) => {
                    let full = solved.full;
                    self.adopt(solved);
                    changed = true;
                    if full {
                        self.job = None;
                    }
                }
                Ok(None) | Err(_) => self.job = None,
            }
        }
        changed
    }

    fn adopt(&mut self, solved: Solved) {
        self.last_took = Some(solved.took);
        self.field = Some(Arc::new(solved.field));
        self.generation = self.generation.wrapping_add(1);
    }
}

/// What a solve reads besides the world.
struct Inputs<'a> {
    project: &'a ProjectDocument,
    lights: &'a [GridLight],
    project_root: Option<PathBuf>,
    cancel: &'a AtomicBool,
}

fn solve_world(inputs: &Inputs<'_>, world: &GridWorld, full: bool) -> Option<Solved> {
    let started = Instant::now();
    let scene = LightScene::new(world).ok()?;
    let faces = psxed_project::grid::faces::all_faces_with(world, &());
    let root = inputs.project_root.clone().unwrap_or_default();
    let albedo = AlbedoTable::from_project(inputs.project, &root, faces.iter().map(|f| f.material));
    drop(faces);
    let field = solve(&GatherInput {
        world,
        units: scene.units(),
        lights: inputs.lights,
        probe: scene.probe(),
        materials: &albedo,
        cancel: Some(inputs.cancel),
    })?;
    let took = started.elapsed();
    eprintln!(
        "[grid light] {} gather: {} samples, {} rays, {} passes on {} threads in {:.0} ms",
        if full { "full" } else { "draft" },
        field.stats.samples,
        field.stats.rays,
        field.stats.passes,
        field.stats.threads,
        took.as_secs_f64() * 1000.0
    );
    Some(Solved { field, took, full })
}
