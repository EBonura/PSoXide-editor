//! Staged write layer.
//!
//! Edits accumulate in memory and reach `project.ron` only on an explicit
//! [`Workspace::save`], which refuses when the file changed underneath. The
//! user has the same project open in the editor; the cheap, honest mitigation
//! is to detect the conflict and say so, not to lock anything.

use std::collections::hash_map::DefaultHasher;
use std::fmt::Write as _;
use std::hash::{Hash as _, Hasher as _};
use std::path::{Path, PathBuf};

use psxed_project::grid::{GridWorld, SceneWorld};
use psxed_project::{NodeId, NodeKind, ProjectDocument, ResourceData, ResourceId, Transform3};

use crate::nodes::{find_node, node_kind_from_ron};
use crate::resolve_scene;

/// A project file plus whatever edits have not been saved yet.
pub struct Workspace {
    path: PathBuf,
    doc: ProjectDocument,
    /// Hash of the file's bytes when it was last read or written.
    base: u64,
    dirty: bool,
    log: Vec<String>,
    /// In-memory snapshots of the staged document, newest last.
    checkpoints: Vec<Checkpoint>,
    /// The seed of the last `grid_break`, for the next one and for re-roll.
    last_break_seed: Option<u32>,
}

/// A named snapshot of the staged document.
struct Checkpoint {
    name: String,
    doc: ProjectDocument,
    log: Vec<String>,
}

/// How many checkpoints are kept; the oldest go first.
const CHECKPOINT_LIMIT: usize = 24;

fn hash_bytes(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn project_file(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join("project.ron")
    } else {
        path.to_path_buf()
    }
}

impl Workspace {
    /// Read the project from disk.
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = project_file(path);
        let text = std::fs::read_to_string(&file)
            .map_err(|error| format!("read {}: {error}", file.display()))?;
        let doc = ProjectDocument::from_ron_str(&text)
            .map_err(|error| format!("parse {}: {error}", file.display()))?;
        Ok(Self {
            path: file,
            base: hash_bytes(&text),
            doc,
            dirty: false,
            log: Vec::new(),
            checkpoints: Vec::new(),
            last_break_seed: None,
        })
    }

    /// The document to read from: the staged one if edited, else a fresh read
    /// so the GUI's own saves are picked up.
    pub fn document(&mut self) -> Result<&ProjectDocument, String> {
        if !self.dirty {
            let reloaded = Self::open(&self.path)?;
            self.doc = reloaded.doc;
            self.base = reloaded.base;
        }
        Ok(&self.doc)
    }

    /// Directory the project file sits in, which asset paths resolve against.
    pub fn root(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new("."))
    }

    /// Whether anything is staged.
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Everything staged since the last save, newest last.
    pub fn log(&self) -> &[String] {
        &self.log
    }

    fn record(&mut self, entry: String) {
        self.dirty = true;
        self.log.push(entry);
    }

    /// Edit the grid world of a scene through `body`, atomically.
    ///
    /// `body` works on a copy of the world and may read the whole project (to
    /// look materials up). An error leaves the document untouched; a change is
    /// staged under `label`. The report gains the validation findings the edit
    /// introduced, so a bad edit says so at once.
    pub fn grid_edit(
        &mut self,
        scene: Option<usize>,
        label: &str,
        body: impl FnOnce(&mut GridWorld, &ProjectDocument) -> Result<String, String>,
    ) -> Result<String, String> {
        self.document()?;
        let index = resolve_scene(&self.doc, scene)?;
        let original = self.doc.scenes[index]
            .world
            .grid()
            .cloned()
            .ok_or_else(|| format!("scene {index} has no grid world"))?;
        let before: std::collections::BTreeSet<String> =
            crate::grid::issue_texts(&original).into_iter().collect();
        let mut world = original.clone();
        let mut report = body(&mut world, &self.doc)?;
        if world == original {
            report.push_str("\n(the world did not change)");
            return Ok(report);
        }
        report.push_str(&crate::grid::new_issues(&before, &world));
        self.doc.scenes[index].world = SceneWorld::Grid(world);
        self.record(label.to_string());
        Ok(report)
    }

    /// Edit the project document (resources, settings) through `body`,
    /// atomically, like [`Self::grid_edit`].
    pub fn project_edit(
        &mut self,
        label: &str,
        body: impl FnOnce(&mut ProjectDocument) -> Result<String, String>,
    ) -> Result<String, String> {
        self.document()?;
        let mut candidate = self.doc.clone();
        let mut report = body(&mut candidate)?;
        if candidate == self.doc {
            report.push_str("\n(the project did not change)");
            return Ok(report);
        }
        self.doc = candidate;
        self.record(label.to_string());
        Ok(report)
    }

    /// Snapshot the staged document under `name` (or the next free `cp-N`).
    /// Same name replaces. Cheap: it is an in-memory clone.
    pub fn checkpoint(&mut self, name: Option<&str>) -> Result<String, String> {
        self.document()?;
        let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
            Some(name) => name.to_string(),
            None => {
                let mut n = self.checkpoints.len() + 1;
                while self.checkpoints.iter().any(|c| c.name == format!("cp-{n}")) {
                    n += 1;
                }
                format!("cp-{n}")
            }
        };
        self.checkpoints.retain(|c| c.name != name);
        self.checkpoints.push(Checkpoint {
            name: name.clone(),
            doc: self.doc.clone(),
            log: self.log.clone(),
        });
        while self.checkpoints.len() > CHECKPOINT_LIMIT {
            self.checkpoints.remove(0);
        }
        Ok(format!(
            "checkpoint {name:?} taken ({} edit(s) staged at the time); {} checkpoint(s) kept",
            self.log.len(),
            self.checkpoints.len()
        ))
    }

    /// Put the staged document back to a checkpoint (the newest when `name`
    /// is omitted). Later checkpoints stay, so a restore can be undone by
    /// restoring a later one. Nothing is written to disk.
    pub fn restore(&mut self, name: Option<&str>) -> Result<String, String> {
        let position = match name.map(str::trim).filter(|n| !n.is_empty()) {
            Some(name) => self
                .checkpoints
                .iter()
                .position(|c| c.name == name)
                .ok_or_else(|| {
                    format!(
                        "no checkpoint {name:?}; have: {}",
                        self.checkpoint_names().join(", ")
                    )
                })?,
            None => self
                .checkpoints
                .len()
                .checked_sub(1)
                .ok_or("there are no checkpoints; call checkpoint first")?,
        };
        let (taken, doc, log) = {
            let cp = &self.checkpoints[position];
            (cp.name.clone(), cp.doc.clone(), cp.log.clone())
        };
        let on_disk = Self::open(&self.path)?;
        self.doc = doc;
        self.log = log;
        // Clean only when it equals what is on disk; otherwise it is a
        // staged change like any other, and save writes it.
        self.dirty = self.doc != on_disk.doc;
        if !self.dirty {
            self.log.clear();
        }
        Ok(format!(
            "restored checkpoint {taken:?}; {} edit(s) staged, {}",
            self.log.len(),
            if self.dirty {
                "not yet saved"
            } else {
                "identical to the file on disk"
            }
        ))
    }

    /// The seed of the last break this session, if any.
    pub fn last_break_seed(&self) -> Option<u32> {
        self.last_break_seed
    }

    /// Remember the seed of a break that was applied.
    pub fn set_last_break_seed(&mut self, seed: Option<u32>) {
        self.last_break_seed = seed;
    }

    /// Checkpoint names, oldest first.
    pub fn checkpoint_names(&self) -> Vec<String> {
        self.checkpoints.iter().map(|c| c.name.clone()).collect()
    }

    /// A directory the renderer can read the CURRENT document from.
    ///
    /// With nothing staged that is the project directory. With staged edits it
    /// is a sibling directory (`.mcp-stage-<name>`, next to the project so the
    /// `../default/assets` style paths in project.ron still resolve) holding
    /// the staged project.ron plus links to everything else the project has,
    /// so a screenshot or a playtest shows the edits before they are saved.
    /// The guard deletes it when dropped.
    pub fn render_root(&mut self) -> Result<RenderRoot, String> {
        self.render_root_without(None)
    }

    /// [`Self::render_root`] with the models of some actors taken off in the
    /// copy (never in the staged document), for shots that hide the player.
    /// Always a scratch copy then, even when nothing is staged.
    pub fn render_root_without(
        &mut self,
        hide: Option<crate::shot::ActorHide>,
    ) -> Result<RenderRoot, String> {
        self.document()?;
        let real = std::fs::canonicalize(self.root())
            .map_err(|error| format!("resolve {}: {error}", self.root().display()))?;
        if !self.dirty && hide.is_none() {
            return Ok(RenderRoot {
                path: real,
                cleanup: None,
            });
        }
        let name = real.file_name().map_or_else(
            || "project".to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let parent = real
            .parent()
            .ok_or("the project directory has no parent")?
            .to_path_buf();
        let tag = match hide {
            None => "",
            Some(crate::shot::ActorHide::Player) => "-no-player",
            Some(crate::shot::ActorHide::All) => "-no-actors",
        };
        let stage = parent.join(format!(".mcp-stage-{name}{tag}"));
        let _ = std::fs::remove_dir_all(&stage);
        std::fs::create_dir_all(&stage)
            .map_err(|error| format!("create {}: {error}", stage.display()))?;
        let mut doc = self.doc.clone();
        if let Some(hide) = hide {
            crate::shot::without_actor_models(&mut doc, hide);
        }
        let text = doc
            .to_ron_string()
            .map_err(|error| format!("serialize the staged project: {error}"))?;
        std::fs::write(stage.join("project.ron"), text)
            .map_err(|error| format!("write the staged project.ron: {error}"))?;
        for entry in std::fs::read_dir(&real)
            .map_err(|error| format!("read {}: {error}", real.display()))?
            .flatten()
        {
            let file_name = entry.file_name();
            let lossy = file_name.to_string_lossy();
            if lossy == "project.ron" || lossy == "baked" || lossy.starts_with(".mcp-") {
                continue;
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(entry.path(), stage.join(&file_name))
                .map_err(|error| format!("link {lossy} into the stage: {error}"))?;
            #[cfg(not(unix))]
            return Err("staged rendering needs a unix host".to_string());
        }
        Ok(RenderRoot {
            path: stage.clone(),
            cleanup: Some(stage),
        })
    }

    /// Replace one set's timing tracks atomically after full cook validation.
    pub fn set_combat_windows(&mut self, set_id: u64, windows_ron: &str) -> Result<String, String> {
        self.document()?;
        let windows: Vec<psxed_project::AnimationCombatWindow> =
            ron::from_str(windows_ron).map_err(|e| format!("combat window RON: {e}"))?;
        let mut candidate = self.doc.clone();
        let id = candidate
            .resources
            .iter()
            .find(|r| r.id.raw() == set_id)
            .ok_or("unknown animation set")?
            .id;
        let resource = candidate.resource_mut(id).ok_or("unknown animation set")?;
        let ResourceData::AnimationSet(set) = &mut resource.data else {
            return Err("resource is not an Animation Set".into());
        };
        set.combat_windows = windows;
        let (package, report) = psxed_project::playtest::build_package(&candidate, self.root());
        if !report.is_ok() {
            return Err(format!("combat timing rejected: {:?}", report.errors));
        }
        let player_resource = package
            .as_ref()
            .and_then(|p| {
                p.player_controller
                    .as_ref()
                    .and_then(|controller| p.characters.get(usize::from(controller.character)))
            })
            .map(|c| c.source_resource);
        let player_uses_set = player_resource
            .and_then(|id| candidate.resource(id))
            .is_some_and(
                |r| matches!(&r.data, ResourceData::Character(c) if c.animation_set == Some(id)),
            );
        if !player_uses_set {
            return Err("Combat permissions currently require the active player's Animation Set; NPC timing is not yet supported".into());
        }
        let text =
            format!("updated combat windows for Animation Set {set_id}; cook validation passed");
        self.doc = candidate;
        self.record(text.clone());
        Ok(text)
    }

    /// Write to disk, refusing if the file moved underneath.
    pub fn save(&mut self) -> Result<String, String> {
        if !self.dirty {
            return Ok("nothing staged".to_string());
        }
        let on_disk = std::fs::read_to_string(&self.path)
            .map_err(|error| format!("read {}: {error}", self.path.display()))?;
        if hash_bytes(&on_disk) != self.base {
            return Err(format!(
                "{} changed on disk since these {} edit(s) were staged, probably \
                 saved from the editor. Nothing was written. Call revert and redo \
                 the work against the current file.",
                self.path.display(),
                self.log.len()
            ));
        }
        let text = self
            .doc
            .to_ron_string()
            .map_err(|error| format!("serialize project: {error}"))?;
        // Snapshot what is being replaced. Staging plus revert covers a bad
        // edit before it lands; nothing covered a bad edit after it landed.
        let backup = self.snapshot(&on_disk)?;
        std::fs::write(&self.path, &text)
            .map_err(|error| format!("write {}: {error}", self.path.display()))?;
        self.base = hash_bytes(&text);
        self.dirty = false;
        let count = self.log.len();
        self.log.clear();
        Ok(format!(
            "saved {count} edit(s) to {}. Reload the project in the editor to see them.\n\
             The previous file is backed up as {}; undo_last_edit restores it.",
            self.path.display(),
            backup
                .file_name()
                .map_or_else(|| "?".into(), |name| name.to_string_lossy().into_owned())
        ))
    }

    /// Throw away everything staged.
    pub fn revert(&mut self) -> Result<String, String> {
        let count = self.log.len();
        let reloaded = Self::open(&self.path)?;
        self.doc = reloaded.doc;
        self.base = reloaded.base;
        self.dirty = false;
        self.log.clear();
        Ok(format!("dropped {count} staged edit(s)"))
    }

    /// Directory holding pre-save snapshots.
    fn backup_dir(&self) -> PathBuf {
        self.root().join(".mcp-backups")
    }

    /// Copy the current file aside before it is overwritten, keeping the most
    /// recent [`BACKUP_LIMIT`] and deleting older ones.
    fn snapshot(&self, contents: &str) -> Result<PathBuf, String> {
        let dir = self.backup_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("create {}: {error}", dir.display()))?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let path = dir.join(format!("project-{stamp}.ron"));
        std::fs::write(&path, contents)
            .map_err(|error| format!("write {}: {error}", path.display()))?;
        let mut existing = list_backups(&dir);
        while existing.len() > BACKUP_LIMIT {
            if let Some(oldest) = existing.pop() {
                let _ = std::fs::remove_file(oldest);
            }
        }
        Ok(path)
    }

    /// Restore the most recent snapshot, backing up the current file first so
    /// the undo is itself undoable.
    pub fn undo_last_edit(&mut self) -> Result<String, String> {
        if self.dirty {
            return Err(
                "there are staged edits; call revert to drop those, or save them first. \
                 undo_last_edit rolls back the file on disk, not the staging buffer."
                    .to_string(),
            );
        }
        let dir = self.backup_dir();
        let mut backups = list_backups(&dir);
        if backups.is_empty() {
            return Err(format!(
                "no snapshots in {}; one is written each time save replaces the file",
                dir.display()
            ));
        }
        let newest = backups.remove(0);
        let restored = std::fs::read_to_string(&newest)
            .map_err(|error| format!("read {}: {error}", newest.display()))?;
        let current = std::fs::read_to_string(&self.path)
            .map_err(|error| format!("read {}: {error}", self.path.display()))?;
        self.snapshot(&current)?;
        std::fs::write(&self.path, &restored)
            .map_err(|error| format!("write {}: {error}", self.path.display()))?;
        let reloaded = Self::open(&self.path)?;
        self.doc = reloaded.doc;
        self.base = reloaded.base;
        self.dirty = false;
        self.log.clear();
        let _ = std::fs::remove_file(&newest);
        Ok(format!(
            "restored {} over {}. The version it replaced was snapshotted first, so this is \
             itself undoable.",
            newest
                .file_name()
                .map_or_else(|| "?".into(), |name| name.to_string_lossy().into_owned()),
            self.path.display()
        ))
    }

    /// Snapshots, newest first.
    pub fn backups(&self) -> Vec<String> {
        list_backups(&self.backup_dir())
            .into_iter()
            .filter_map(|path| {
                let name = path.file_name()?.to_string_lossy().into_owned();
                let size = path.metadata().ok()?.len();
                Some(format!("{name} ({size} bytes)"))
            })
            .collect()
    }

    /// Clone an existing node, with its whole subtree, to a new position.
    ///
    /// The subtree is the point: an enemy is a host Entity plus Model
    /// Renderer, Animator, Character Controller and Camera children, so
    /// copying the host alone yields something inert. Cloning a working
    /// example is also how an agent places a kind it has no constructor for.
    pub fn place_node(
        &mut self,
        scene: Option<usize>,
        source: &str,
        position: [i32; 3],
        name: Option<&str>,
    ) -> Result<String, String> {
        self.document()?;
        let scene_index = resolve_scene(&self.doc, scene)?;
        let scene_doc = &mut self.doc.scenes[scene_index];
        let source_id = find_node(scene_doc, source)?;
        let source_node = scene_doc
            .node(source_id)
            .ok_or_else(|| format!("node {source:?} vanished"))?;
        let parent = source_node.parent.unwrap_or(NodeId::ROOT);
        let label = source_node.kind.label();
        let new_name = name
            .map(str::to_string)
            .unwrap_or_else(|| format!("{} copy", source_node.name));

        let new_id = clone_subtree(scene_doc, source_id, parent, &new_name)?;
        let copied = count_subtree(scene_doc, new_id);
        if let Some(node) = scene_doc.node_mut(new_id) {
            node.transform.translation =
                [position[0] as f32, position[1] as f32, position[2] as f32];
        }
        self.record(format!(
            "place_node {source:?} -> {new_name:?} at {position:?}"
        ));
        let mut out = format!(
            "cloned {source:?} ({label}) as {new_name:?} (id {}) at {position:?}: {copied} node(s) \
             including its component children",
            new_id.raw()
        );
        // A clone copies identity-bearing fields too, and the cook rejects a
        // reused persistence id outright. Cheaper to say so here than to have
        // the whole build fail later with no obvious cause.
        let duplicates = duplicate_persistence_ids(&self.doc.scenes[scene_index]);
        if !duplicates.is_empty() {
            let _ = write!(
                out,
                "\n\nWARNING: persistence id(s) {} are now used by more than one node. The cook \
                 REJECTS that. Use get_node on the clone's children (ids are listed) and \
                 set_node to give it its own id.",
                duplicates.join(", ")
            );
        }
        Ok(out)
    }

    /// Replace a node's kind payload from RON text.
    ///
    /// The generic escape hatch. There are 28 NodeKind variants and typed
    /// setters for each would be a lot of code that goes stale; the format
    /// round-trips, so text is the honest interface. Read with get_node first.
    pub fn set_node(
        &mut self,
        scene: Option<usize>,
        needle: &str,
        kind_ron: &str,
    ) -> Result<String, String> {
        self.document()?;
        let scene_index = resolve_scene(&self.doc, scene)?;
        let scene_doc = &mut self.doc.scenes[scene_index];
        let id = find_node(scene_doc, needle)?;
        let parsed = node_kind_from_ron(kind_ron)?;
        let node = scene_doc
            .node_mut(id)
            .ok_or_else(|| format!("node {needle:?} vanished"))?;
        let was = node.kind.label();
        let now = parsed.label();
        if was != now {
            return Err(format!(
                "that RON is a {now}, but {needle:?} is a {was}. Changing a node's kind in \
                 place would orphan its component children; delete it and place a new one."
            ));
        }
        node.kind = parsed;
        let name = node.name.clone();
        self.record(format!("set_node {name:?}"));
        Ok(format!("updated {name:?} ({now})"))
    }

    /// Move a node to a world position.
    pub fn move_node(
        &mut self,
        scene: Option<usize>,
        needle: &str,
        position: [i32; 3],
    ) -> Result<String, String> {
        self.document()?;
        let scene_index = resolve_scene(&self.doc, scene)?;
        let scene_doc = &mut self.doc.scenes[scene_index];
        let id = find_node(scene_doc, needle)?;
        let node = scene_doc
            .node_mut(id)
            .ok_or_else(|| format!("node {needle:?} vanished"))?;
        let from = node.transform.translation;
        node.transform.translation = [position[0] as f32, position[1] as f32, position[2] as f32];
        let name = node.name.clone();
        self.record(format!("move_node {name:?} -> {position:?}"));
        Ok(format!(
            "moved {name:?} from [{:.0}, {:.0}, {:.0}] to {position:?}",
            from[0], from[1], from[2]
        ))
    }

    /// Remove a node and its subtree.
    pub fn delete_node(&mut self, scene: Option<usize>, needle: &str) -> Result<String, String> {
        self.document()?;
        let scene_index = resolve_scene(&self.doc, scene)?;
        let scene_doc = &mut self.doc.scenes[scene_index];
        let id = find_node(scene_doc, needle)?;
        if id == NodeId::ROOT {
            return Err("the scene root cannot be deleted".to_string());
        }
        let node = scene_doc
            .node(id)
            .ok_or_else(|| format!("node {needle:?} vanished"))?;
        let name = node.name.clone();
        let removed = count_subtree(scene_doc, id);
        if !scene_doc.remove_node(id) {
            return Err(format!("the scene refused to remove {name:?}"));
        }
        self.record(format!("delete_node {name:?}"));
        Ok(format!(
            "removed {name:?} and its subtree, {removed} node(s)"
        ))
    }

    /// Place a static point light.
    ///
    /// Radius is authored in SECTORS, not world units, unlike every other
    /// length these tools take. That asymmetry is in the format, not here, so
    /// the tool takes world units and converts, and reports both.
    pub fn add_light(
        &mut self,
        scene: Option<usize>,
        position: [i32; 3],
        radius_units: i32,
        color: [u8; 3],
        intensity: f32,
        name: Option<&str>,
    ) -> Result<String, String> {
        self.document()?;
        let scene_index = resolve_scene(&self.doc, scene)?;
        if radius_units <= 0 {
            return Err("radius must be positive".to_string());
        }
        if !(0.0..=8.0).contains(&intensity) {
            return Err(format!(
                "intensity {intensity} is outside the sane range 0..=8"
            ));
        }
        let scene_doc = &mut self.doc.scenes[scene_index];
        let sector = scene_doc
            .world_sector_size_for_node(NodeId::ROOT)
            .unwrap_or(crate::SECTOR)
            .max(1);
        let radius_sectors = radius_units as f32 / sector as f32;
        let id = scene_doc.add_node(
            NodeId::ROOT,
            name.unwrap_or("Point Light").to_string(),
            NodeKind::PointLight {
                color,
                intensity,
                radius: radius_sectors,
            },
        );
        if let Some(node) = scene_doc.node_mut(id) {
            node.transform = Transform3 {
                translation: [position[0] as f32, position[1] as f32, position[2] as f32],
                ..node.transform
            };
        }
        self.record(format!("add_light at {position:?} r={radius_units}"));

        let out = format!(
            "placed {:?} at {position:?}: radius {radius_units} units ({radius_sectors:.2} sectors), \
             colour {color:?}, intensity {intensity}",
            name.unwrap_or("Point Light")
        );
        Ok(out)
    }

    /// Every point light in the scene, as (name, world position, radius units).
    pub fn lights(&mut self) -> Result<Vec<(String, [f32; 3], f32)>, String> {
        let project = self.document()?;
        let scene = project
            .scenes
            .first()
            .ok_or_else(|| "the project has no scenes".to_string())?;
        let sector = scene
            .world_sector_size_for_node(NodeId::ROOT)
            .unwrap_or(crate::SECTOR)
            .max(1) as f32;
        Ok(scene
            .nodes()
            .iter()
            .filter_map(|node| match node.kind {
                NodeKind::PointLight { radius, .. } => Some((
                    node.name.clone(),
                    node.transform.translation,
                    radius * sector,
                )),
                _ => None,
            })
            .collect())
    }
}

/// A directory to render or build from, deleted on drop when it was staged.
pub struct RenderRoot {
    /// The directory to pass to the renderer.
    pub path: PathBuf,
    cleanup: Option<PathBuf>,
}

impl RenderRoot {
    /// Whether this is a scratch copy of staged edits rather than the project.
    pub fn is_staged(&self) -> bool {
        self.cleanup.is_some()
    }
}

impl Drop for RenderRoot {
    fn drop(&mut self) {
        if let Some(dir) = &self.cleanup {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Copy `source` and every descendant under `parent`, returning the new root.
fn clone_subtree(
    scene: &mut psxed_project::Scene,
    source: NodeId,
    parent: NodeId,
    name: &str,
) -> Result<NodeId, String> {
    let node = scene
        .node(source)
        .ok_or_else(|| "the node vanished mid-clone".to_string())?;
    let kind = node.kind.clone();
    let transform = node.transform;
    let children = node.children.clone();
    let new_id = scene.add_node(parent, name, kind);
    if let Some(copy) = scene.node_mut(new_id) {
        copy.transform = transform;
    }
    for child in children {
        // Component children keep their own names: a Model Renderer named
        // anything else stops reading as one in the scene tree.
        let child_name = scene
            .node(child)
            .map_or_else(String::new, |node| node.name.clone());
        clone_subtree(scene, child, new_id, &child_name)?;
    }
    Ok(new_id)
}

/// How many pre-save snapshots to keep, matching the TrenchBroom MCP's cap.
const BACKUP_LIMIT: usize = 20;

/// Snapshot files in `dir`, newest first.
fn list_backups(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "ron")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("project-"))
        })
        .collect();
    // Names carry a unix timestamp, so lexical order is chronological.
    paths.sort();
    paths.reverse();
    paths
}

/// Persistence ids claimed by more than one node, which the cook rejects.
fn duplicate_persistence_ids(scene: &psxed_project::Scene) -> Vec<String> {
    let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for node in scene.nodes() {
        if let NodeKind::PointOfInterest { persistence_id, .. } = &node.kind {
            if !persistence_id.is_empty() {
                *seen.entry(persistence_id.clone()).or_default() += 1;
            }
        }
    }
    seen.into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(id, _)| format!("{id:?}"))
        .collect()
}

/// Number of nodes in a subtree, counting its root.
fn count_subtree(scene: &psxed_project::Scene, id: NodeId) -> usize {
    scene.node(id).map_or(0, |node| {
        1 + node
            .children
            .iter()
            .map(|child| count_subtree(scene, *child))
            .sum::<usize>()
    })
}

/// Find a material resource by name: exact first, then unique
/// case-insensitive substring, listing the candidates when ambiguous.
pub fn find_material(project: &ProjectDocument, needle: &str) -> Result<ResourceId, String> {
    let materials = || {
        project
            .resources
            .iter()
            .filter(|resource| matches!(resource.data, ResourceData::Material(_)))
    };
    if let Some(resource) = materials().find(|resource| resource.name == needle) {
        return Ok(resource.id);
    }
    let lowered = needle.to_ascii_lowercase();
    let hits: Vec<_> = materials()
        .filter(|resource| resource.name.to_ascii_lowercase().contains(&lowered))
        .collect();
    match hits.as_slice() {
        [only] => Ok(only.id),
        [] => Err(format!(
            "no material matches {needle:?}. Available: {}",
            materials()
                .map(|resource| resource.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        many => Err(format!(
            "{needle:?} is ambiguous between: {}",
            many.iter()
                .map(|resource| resource.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(dir: &Path) -> Workspace {
        let path = dir.join("project.ron");
        std::fs::write(&path, ProjectDocument::starter().to_ron_string().unwrap()).unwrap();
        Workspace::open(&path).unwrap()
    }

    #[test]
    fn grid_edits_are_atomic_and_checkpoints_restore() {
        let dir = std::env::temp_dir().join(format!("psxed-mcp-grid-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut workspace = workspace(&dir);
        workspace.checkpoint(Some("clean")).unwrap();

        // An error inside the body stages nothing.
        let failed = workspace.grid_edit(None, "fail", |world, _| {
            world.rooms[0].name = "changed".to_string();
            Err("nope".to_string())
        });
        assert_eq!(failed.unwrap_err(), "nope");
        assert!(!workspace.is_dirty());

        // A change stages, and says what it introduced.
        let report = workspace
            .grid_edit(None, "rename", |world, _| {
                world.rooms[0].name = "Renamed".to_string();
                Ok("renamed".to_string())
            })
            .unwrap();
        assert_eq!(report, "renamed");
        assert!(workspace.is_dirty());
        // A body that changes nothing is reported, not staged again.
        let same = workspace
            .grid_edit(None, "again", |_, _| Ok("noop".to_string()))
            .unwrap();
        assert!(same.contains("did not change"), "{same}");
        assert_eq!(workspace.log().len(), 1);

        workspace.checkpoint(Some("renamed")).unwrap();
        workspace
            .grid_edit(None, "more", |world, _| {
                world.rooms[0].name = "Again".to_string();
                Ok(String::new())
            })
            .unwrap();
        let name = |w: &mut Workspace| {
            w.document()
                .unwrap()
                .active_scene()
                .world
                .grid()
                .unwrap()
                .rooms[0]
                .name
                .clone()
        };
        assert_eq!(name(&mut workspace), "Again");
        workspace.restore(Some("renamed")).unwrap();
        assert_eq!(name(&mut workspace), "Renamed");
        assert!(workspace.is_dirty());
        // Restoring the clean checkpoint is clean again, and the later ones stay.
        let back = workspace.restore(Some("clean")).unwrap();
        assert!(back.contains("identical to the file on disk"), "{back}");
        assert!(!workspace.is_dirty());
        assert_eq!(workspace.checkpoint_names(), ["clean", "renamed"]);
        assert!(workspace
            .restore(Some("missing"))
            .unwrap_err()
            .contains("clean, renamed"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_staged_render_copy_links_the_project_and_vanishes() {
        let base = std::env::temp_dir().join(format!("psxed-mcp-stage-{}", std::process::id()));
        let dir = base.join("proj");
        std::fs::create_dir_all(dir.join("baked")).unwrap();
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("assets/a.txt"), "a").unwrap();
        let mut workspace = workspace(&dir);

        // Clean: the project itself is rendered, nothing is copied.
        let clean = workspace.render_root().unwrap();
        assert!(!clean.is_staged());
        drop(clean);
        assert!(!base.join(".mcp-stage-proj").exists());

        workspace
            .add_light(None, [0, 100, 0], 1000, [255; 3], 1.0, Some("L"))
            .unwrap();
        let stage_path;
        {
            let staged = workspace.render_root().unwrap();
            assert!(staged.is_staged());
            stage_path = staged.path.clone();
            let text = std::fs::read_to_string(stage_path.join("project.ron")).unwrap();
            assert!(text.contains("\"L\""), "the staged edit is in the copy");
            // Assets are reachable through a link; the disc directory is not copied.
            assert_eq!(
                std::fs::read_to_string(stage_path.join("assets/a.txt")).unwrap(),
                "a"
            );
            assert!(!stage_path.join("baked").exists());
        }
        assert!(!stage_path.exists(), "the guard removed the copy");
        // The real project file is untouched.
        assert!(!std::fs::read_to_string(dir.join("project.ron"))
            .unwrap()
            .contains("\"L\""));
        std::fs::remove_dir_all(&base).ok();
    }

    /// The write path end to end: stage, save, and the staleness guard that
    /// stops a save clobbering the editor's own.
    #[test]
    fn staged_edits_apply_and_save_refuses_a_changed_file() {
        let dir = std::env::temp_dir().join(format!("psxed-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut workspace = workspace(&dir);

        let report = workspace
            .add_light(None, [0, 256, 0], 2048, [255, 200, 160], 1.0, Some("Lamp"))
            .expect("the light is placed");
        assert!(report.contains("placed"), "{report}");
        assert!(workspace.is_dirty());
        assert_eq!(
            workspace.lights().unwrap().len(),
            2,
            "the starter lamp and the new one"
        );

        // Save writes, and a second save has nothing to do.
        assert!(workspace.save().unwrap().contains("saved"));
        assert!(!workspace.is_dirty());

        // Now simulate the editor saving underneath a staged edit.
        workspace
            .add_light(
                None,
                [0, 256, 512],
                2048,
                [255, 200, 160],
                1.0,
                Some("Second"),
            )
            .expect("a second light is placed");
        std::fs::write(&workspace.path, "// touched by the editor\n").unwrap();
        let refused = workspace
            .save()
            .expect_err("a changed file must block the save");
        assert!(refused.contains("changed on disk"), "{refused}");

        std::fs::remove_dir_all(&dir).ok();
    }
}
