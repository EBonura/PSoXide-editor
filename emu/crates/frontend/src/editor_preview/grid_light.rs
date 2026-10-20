//! The global light of the preview's grid world, solved off the UI thread.
//!
//! Direct light and its shadows are cheap and bake the moment a face is first
//! drawn after an edit. The ambient occlusion, sky light and bounce of a
//! world that gathers its light (`GridWorld::global_light`) take a while, so
//! they are solved on a worker thread from a copy of the world and swapped in
//! when done: until then the preview shows the flat room ambient (or the
//! previous solution, for the faces it still matches), and the faces are
//! baked again once the new field arrives. An edit while a solve runs cancels
//! it and starts another.

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
}

struct Job {
    cancel: Arc<AtomicBool>,
    done: Receiver<Option<Solved>>,
}

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
                let _ = tx.send(solve_world(&project, &world, &lights, project_root, &flag));
            });
        if spawned.is_ok() {
            self.job = Some(Job { cancel, done });
        }
    }

    /// Take a finished solve, if one is waiting. True when the field changed.
    pub(super) fn poll(&mut self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        match job.done.try_recv() {
            Ok(solved) => {
                self.job = None;
                match solved {
                    Some(solved) => {
                        self.last_took = Some(solved.took);
                        self.field = Some(Arc::new(solved.field));
                        self.generation = self.generation.wrapping_add(1);
                        true
                    }
                    None => false,
                }
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.job = None;
                false
            }
        }
    }

    /// Block until the solve in flight is done and take it: for a one-shot
    /// frame (the command line, a test) that has no later frame to refine in.
    pub(super) fn wait(&mut self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        let solved = job.done.recv().ok().flatten();
        self.job = None;
        match solved {
            Some(solved) => {
                self.last_took = Some(solved.took);
                self.field = Some(Arc::new(solved.field));
                self.generation = self.generation.wrapping_add(1);
                true
            }
            None => false,
        }
    }
}

fn solve_world(
    project: &ProjectDocument,
    world: &GridWorld,
    lights: &[GridLight],
    project_root: Option<PathBuf>,
    cancel: &AtomicBool,
) -> Option<Solved> {
    let started = Instant::now();
    let scene = LightScene::new(world).ok()?;
    let faces = psxed_project::grid::faces::all_faces_with(world, &());
    let root = project_root.unwrap_or_default();
    let albedo = AlbedoTable::from_project(project, &root, faces.iter().map(|f| f.material));
    drop(faces);
    let field = solve(&GatherInput {
        world,
        units: scene.units(),
        lights,
        probe: scene.probe(),
        materials: &albedo,
        cancel: Some(cancel),
    })?;
    let took = started.elapsed();
    eprintln!(
        "[grid light] gathered {} samples, {} rays, {} passes on {} threads in {:.0} ms",
        field.stats.samples,
        field.stats.rays,
        field.stats.passes,
        field.stats.threads,
        took.as_secs_f64() * 1000.0
    );
    Some(Solved { field, took })
}
