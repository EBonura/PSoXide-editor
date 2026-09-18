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

use psxed_project::brush::Brush;
use psxed_project::brush_primitives::{self, BrushDrawSettings};
use psxed_project::brush_world::BrushWorldCookMode;
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
}

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
        std::fs::write(&self.path, &text)
            .map_err(|error| format!("write {}: {error}", self.path.display()))?;
        self.base = hash_bytes(&text);
        self.dirty = false;
        let count = self.log.len();
        self.log.clear();
        Ok(format!(
            "saved {count} edit(s) to {}. Reload the project in the editor to see them.",
            self.path.display()
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

    /// Append brushes to a scene, returning the range they occupy.
    fn push_brushes(
        &mut self,
        scene_index: usize,
        brushes: Vec<Brush>,
    ) -> Result<(usize, usize), String> {
        let scene = self
            .doc
            .scenes
            .get_mut(scene_index)
            .ok_or_else(|| format!("scene {scene_index} is gone"))?;
        let first = scene.brushes.len();
        let count = brushes.len();
        scene.brushes.extend(brushes);
        Ok((first, count))
    }

    /// Build a parametric primitive and add it to the scene.
    pub fn add_shape(
        &mut self,
        scene: Option<usize>,
        min: [i32; 3],
        max: [i32; 3],
        settings: BrushDrawSettings,
        step: i32,
        material: Option<&str>,
    ) -> Result<String, String> {
        self.document()?;
        let scene_index = resolve_scene(&self.doc, scene)?;
        let material = match material {
            Some(needle) => Some(find_material(&self.doc, needle)?),
            None => None,
        };
        let mut generated = brush_primitives::generate(min, max, settings, step)?;
        if let Some(id) = material {
            for face in generated
                .brushes
                .iter_mut()
                .flat_map(|brush| brush.faces.iter_mut())
            {
                face.material = Some(id);
            }
        }
        let faces = generated.face_count();
        let warnings = std::mem::take(&mut generated.warnings);
        let (first, count) = self.push_brushes(scene_index, generated.brushes)?;
        self.record(format!(
            "add_shape {:?} -> brushes {first}..{}",
            settings.shape,
            first + count
        ));

        let mut out = String::new();
        let _ = writeln!(
            out,
            "added {count} brush(es), {faces} faces, as indices {first}..{} in scene {scene_index}",
            first + count
        );
        if material.is_none() {
            out.push_str("no material set: these faces will cook untextured\n");
        }
        for warning in &warnings {
            let _ = writeln!(out, "warning: {warning}");
        }
        let _ = write!(
            out,
            "pass first={first} count={count} to array, set_material or delete"
        );
        Ok(out)
    }

    /// Hollow box: a room of the given INNER dimensions, walls grown outward.
    ///
    /// Inner rather than outer because every spatial judgement about a room is
    /// about the space inside it, and getting that convention wrong is the
    /// commonest way to author a room a player cannot fit through.
    pub fn make_room(
        &mut self,
        scene: Option<usize>,
        inner_min: [i32; 3],
        inner_max: [i32; 3],
        thickness: i32,
        material: Option<&str>,
    ) -> Result<String, String> {
        self.document()?;
        let scene_index = resolve_scene(&self.doc, scene)?;
        if (0..3).any(|axis| inner_min[axis] >= inner_max[axis]) {
            return Err(format!(
                "the room has no interior: min {inner_min:?} is not strictly below max {inner_max:?}"
            ));
        }
        let thickness = thickness.max(1);
        let material = match material {
            Some(needle) => Some(find_material(&self.doc, needle)?),
            None => None,
        };
        let outer_min = std::array::from_fn(|axis| inner_min[axis] - thickness);
        let outer_max = std::array::from_fn(|axis| inner_max[axis] + thickness);
        let shell = Brush::cuboid(outer_min, outer_max)
            .hollow(thickness)
            .ok_or_else(|| {
                format!("a {thickness}-unit shell does not fit around this interior")
            })?;
        let mut shell = shell;
        if let Some(id) = material {
            for face in shell.iter_mut().flat_map(|brush| brush.faces.iter_mut()) {
                face.material = Some(id);
            }
        }
        let faces: usize = shell.iter().map(|brush| brush.faces.len()).sum();
        let (first, count) = self.push_brushes(scene_index, shell)?;
        self.record(format!("make_room -> brushes {first}..{}", first + count));

        let interior = [
            inner_max[0] - inner_min[0],
            inner_max[1] - inner_min[1],
            inner_max[2] - inner_min[2],
        ];
        Ok(format!(
            "room with a {} x {} x {} interior ({:.1} player heights tall, floor {:.1} x {:.1} \
             player widths): {count} brushes, {faces} faces, indices {first}..{}",
            interior[0],
            interior[1],
            interior[2],
            f64::from(interior[1]) / f64::from(crate::PLAYER_HEIGHT),
            f64::from(interior[0]) / f64::from(crate::PLAYER_RADIUS * 2),
            f64::from(interior[2]) / f64::from(crate::PLAYER_RADIUS * 2),
            first + count
        ))
    }

    /// Repeat a run of brushes along a line or around a centre.
    ///
    /// This is what turns one pillar into a colonnade and one arch into an
    /// arcade, which is the step the editor has no tool for today.
    pub fn array(
        &mut self,
        scene: Option<usize>,
        first: usize,
        count: usize,
        copies: u32,
        offset: [i32; 3],
        radial: Option<RadialArray>,
    ) -> Result<String, String> {
        self.document()?;
        let scene_index = resolve_scene(&self.doc, scene)?;
        let source = self.brush_slice(scene_index, first, count)?.to_vec();
        if copies == 0 {
            return Err("copies must be at least 1".to_string());
        }
        if radial.is_none() && offset == [0, 0, 0] {
            return Err(
                "a linear array needs a non-zero offset, or pass a radial centre".to_string(),
            );
        }
        let mut added = Vec::new();
        for index in 1..=copies {
            for brush in &source {
                let mut copy = brush.clone();
                match radial {
                    Some(radial) => {
                        let degrees =
                            f64::from(radial.degrees) * f64::from(index) / f64::from(copies);
                        rotate_brush_y(&mut copy, radial.center, degrees);
                    }
                    None => copy.translate([
                        offset[0] * index as i32,
                        offset[1] * index as i32,
                        offset[2] * index as i32,
                    ]),
                }
                added.push(copy);
            }
        }
        let faces: usize = added.iter().map(|brush| brush.faces.len()).sum();
        let (new_first, new_count) = self.push_brushes(scene_index, added)?;
        self.record(format!(
            "array x{copies} of brushes {first}..{} -> {new_first}..{}",
            first + count,
            new_first + new_count
        ));
        Ok(format!(
            "added {new_count} brushes ({faces} faces) as indices {new_first}..{}. \
             The run is now {} instances totalling {} faces, all potentially in one \
             visibility leaf; check a plan_view and keep an eye on sightlines.",
            new_first + new_count,
            copies + 1,
            faces + source.iter().map(|b| b.faces.len()).sum::<usize>()
        ))
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
        self.record(format!("place_node {source:?} -> {new_name:?} at {position:?}"));
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
        Ok(format!("removed {name:?} and its subtree, {removed} node(s)"))
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
            return Err(format!("intensity {intensity} is outside the sane range 0..=8"));
        }
        let draft = self.doc.bsp_cook_mode == BrushWorldCookMode::Draft;
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

        let mut out = format!(
            "placed {:?} at {position:?}: radius {radius_units} units ({radius_sectors:.2} sectors), \
             colour {color:?}, intensity {intensity}",
            name.unwrap_or("Point Light")
        );
        if draft {
            out.push_str(
                "\n\nWARNING: bsp_cook_mode is Draft, and Draft packs every surface fullbright \
                 and never runs the light bake. This light will do NOTHING until the project \
                 cooks in Release. Nothing you place will be visible in the editor preview or a \
                 Draft playtest, so light the room by intent and verify in Release.",
            );
        }
        Ok(out)
    }

    /// Switch the BSP cook between Draft (fullbright, fast) and Release
    /// (bakes point lights over a dark ambient).
    pub fn set_cook_mode(&mut self, release: bool) -> Result<String, String> {
        self.document()?;
        let wanted = if release {
            BrushWorldCookMode::Release
        } else {
            BrushWorldCookMode::Draft
        };
        if self.doc.bsp_cook_mode == wanted {
            return Ok(format!("already {wanted:?}"));
        }
        self.doc.bsp_cook_mode = wanted;
        self.record(format!("bsp_cook_mode -> {wanted:?}"));
        Ok(if release {
            "bsp_cook_mode is now Release: point lights bake over a dark ambient. Unlit \
             surfaces go roughly 8x darker than Draft, so a room with no lights in it will \
             read as nearly black."
                .to_string()
        } else {
            "bsp_cook_mode is now Draft: every surface is fullbright and lights are ignored."
                .to_string()
        })
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

    /// Cut a box-shaped void out of a run of brushes.
    ///
    /// A doorway through a wall is the commonest authoring move there is, and
    /// without it `make_room` produces a sealed shell nothing can enter. The
    /// kernel's `subtracted_by` returns the remainder as convex pieces, which
    /// is what the BSP wants anyway.
    pub fn carve(
        &mut self,
        scene: Option<usize>,
        first: usize,
        count: usize,
        min: [i32; 3],
        max: [i32; 3],
        material: Option<&str>,
    ) -> Result<String, String> {
        self.document()?;
        let scene_index = resolve_scene(&self.doc, scene)?;
        if (0..3).any(|axis| min[axis] >= max[axis]) {
            return Err(format!(
                "the cutter has no volume: min {min:?} is not strictly below max {max:?}"
            ));
        }
        self.brush_slice(scene_index, first, count)?;
        let material = match material {
            Some(needle) => Some(find_material(&self.doc, needle)?),
            None => None,
        };
        let cutter = Brush::cuboid(min, max);

        // Back to front, so an earlier index stays valid while a later brush
        // is being replaced by a different number of pieces.
        let mut cut = 0usize;
        let mut produced = 0usize;
        let mut collapsed = Vec::new();
        for position in (first..first + count).rev() {
            let source = self.doc.scenes[scene_index].brushes[position].clone();
            // The cut faces inherit a material, or they cook untextured: the
            // inside of a doorway reveal is a surface the player looks at.
            let mut cutter = cutter.clone();
            let fill = material.or_else(|| source.faces.first().and_then(|face| face.material));
            for face in cutter.faces.iter_mut() {
                face.material = fill;
            }
            let Some(pieces) = source.subtracted_by(&cutter) else {
                continue;
            };
            cut += 1;
            let pieces: Vec<Brush> = pieces
                .into_iter()
                .filter(|piece| piece.solve().is_valid())
                .map(|mut piece| {
                    piece.contents = source.contents;
                    piece.mover = source.mover;
                    piece.group = source.group;
                    piece
                })
                .collect();
            if pieces.is_empty() {
                collapsed.push(position);
            }
            produced += pieces.len();
            self.doc.scenes[scene_index]
                .brushes
                .splice(position..=position, pieces);
        }

        if cut == 0 {
            return Err(format!(
                "the cutter {min:?}..{max:?} does not intersect any of brushes {first}..{}",
                first + count
            ));
        }
        let total = self.doc.scenes[scene_index].brushes.len();
        self.record(format!(
            "carve {min:?}..{max:?} out of brushes {first}..{}",
            first + count
        ));
        let mut out = format!(
            "carved {cut} of {count} brush(es) into {produced} pieces; scene now has {total} \
             brushes and every index above {first} has moved. Re-read scene_info before \
             using an older index."
        );
        if !collapsed.is_empty() {
            let _ = write!(
                out,
                "\n{} brush(es) were removed entirely, the cutter swallowed them",
                collapsed.len()
            );
        }
        Ok(out)
    }

    /// Assign a material to a run of brushes, optionally only the faces whose
    /// normal points along one axis (floors, ceilings, one wall direction).
    pub fn set_material(
        &mut self,
        scene: Option<usize>,
        first: usize,
        count: usize,
        material: &str,
        normal: Option<[i32; 3]>,
    ) -> Result<String, String> {
        self.document()?;
        let scene_index = resolve_scene(&self.doc, scene)?;
        let id = find_material(&self.doc, material)?;
        let name = self
            .doc
            .resources
            .iter()
            .find(|resource| resource.id == id)
            .map_or_else(String::new, |resource| resource.name.clone());
        let brushes = self.brush_slice_mut(scene_index, first, count)?;
        let mut touched = 0usize;
        for brush in brushes.iter_mut() {
            for face in brush.faces.iter_mut() {
                if let Some(wanted) = normal {
                    let Some(plane) = psxed_project::brush::Plane::from_points(face.points) else {
                        continue;
                    };
                    // Same direction, allowing any magnitude: the plane normal
                    // is an unnormalized cross product.
                    let dot = plane.normal[0] * i64::from(wanted[0])
                        + plane.normal[1] * i64::from(wanted[1])
                        + plane.normal[2] * i64::from(wanted[2]);
                    let cross_free = plane.normal.iter().zip(wanted.iter()).all(|(n, w)| {
                        (*w != 0) || (*n == 0)
                    });
                    if dot <= 0 || !cross_free {
                        continue;
                    }
                }
                face.material = Some(id);
                touched += 1;
            }
        }
        self.record(format!(
            "set_material {name:?} on {touched} faces of brushes {first}..{}",
            first + count
        ));
        Ok(format!("set {name:?} on {touched} faces"))
    }

    /// Remove a run of brushes. Later indices shift down, so this reports the
    /// shift rather than leaving the caller to discover it.
    pub fn delete(
        &mut self,
        scene: Option<usize>,
        first: usize,
        count: usize,
    ) -> Result<String, String> {
        self.document()?;
        let scene_index = resolve_scene(&self.doc, scene)?;
        self.brush_slice(scene_index, first, count)?;
        let scene = &mut self.doc.scenes[scene_index];
        scene.brushes.drain(first..first + count);
        let remaining = scene.brushes.len();
        self.record(format!("delete brushes {first}..{}", first + count));
        Ok(format!(
            "deleted {count} brush(es); every index above {first} shifted down by {count}, \
             {remaining} brushes remain"
        ))
    }

    fn brush_slice(
        &self,
        scene_index: usize,
        first: usize,
        count: usize,
    ) -> Result<&[Brush], String> {
        let scene = &self.doc.scenes[scene_index];
        let end = first.checked_add(count).ok_or("range overflows")?;
        if count == 0 {
            return Err("count must be at least 1".to_string());
        }
        scene
            .brushes
            .get(first..end)
            .ok_or_else(|| {
                format!(
                    "brushes {first}..{end} are out of range: scene {scene_index} has {}",
                    scene.brushes.len()
                )
            })
    }

    fn brush_slice_mut(
        &mut self,
        scene_index: usize,
        first: usize,
        count: usize,
    ) -> Result<&mut [Brush], String> {
        self.brush_slice(scene_index, first, count)?;
        let scene = &mut self.doc.scenes[scene_index];
        Ok(&mut scene.brushes[first..first + count])
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
    let floor = node.floor;
    let children = node.children.clone();
    let new_id = scene.add_node(parent, name, kind);
    if let Some(copy) = scene.node_mut(new_id) {
        copy.transform = transform;
        copy.floor = floor;
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

/// Persistence ids claimed by more than one node, which the cook rejects.
fn duplicate_persistence_ids(scene: &psxed_project::Scene) -> Vec<String> {
    let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for node in scene.nodes() {
        if let NodeKind::PointOfInterest {
            persistence_id, ..
        } = &node.kind
        {
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

/// Centre and sweep of a radial [`Workspace::array`].
#[derive(Clone, Copy, Debug)]
pub struct RadialArray {
    /// World point to rotate about, on the XZ plane.
    pub center: [i32; 3],
    /// Total sweep in degrees, divided evenly across the copies.
    pub degrees: i32,
}

/// Rotate a brush about a vertical axis through `center`.
///
/// Plane points are integers, so the rotated points are rounded. That is the
/// same quantization the editor's own rotate does, and it is why a radial
/// array of a fine curve can drift off the grid: rotate coarse pieces.
fn rotate_brush_y(brush: &mut Brush, center: [i32; 3], degrees: f64) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    for face in brush.faces.iter_mut() {
        for point in face.points.iter_mut() {
            let x = f64::from(point[0] - center[0]);
            let z = f64::from(point[2] - center[2]);
            point[0] = center[0] + (x * cos - z * sin).round() as i32;
            point[2] = center[2] + (x * sin + z * cos).round() as i32;
        }
    }
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
    use psxed_project::brush_primitives::BrushDrawShape;

    fn empty_workspace(dir: &Path) -> Workspace {
        let mut doc = ProjectDocument::default();
        // `ProjectDocument::default()` is the embedded starter level, not an
        // empty document.
        doc.scenes[0].brushes.clear();
        let path = dir.join("project.ron");
        std::fs::write(&path, doc.to_ron_string().unwrap()).unwrap();
        Workspace::open(&path).unwrap()
    }

    /// The write path end to end: build, array, retexture, delete, and the
    /// staleness guard that stops a save clobbering the editor's own.
    #[test]
    fn staged_edits_apply_and_save_refuses_a_changed_file() {
        let dir = std::env::temp_dir().join(format!("psxed-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut workspace = empty_workspace(&dir);

        // A pillar wide enough to keep all eight sides.
        let width = brush_primitives::minimum_pillar_footprint(8, 64).unwrap();
        let settings = BrushDrawSettings {
            shape: BrushDrawShape::Cylinder,
            ..BrushDrawSettings::default()
        };
        let report = workspace
            .add_shape(None, [0, 0, 0], [width, 2048, width], settings, 64, None)
            .expect("the pillar builds");
        assert!(report.contains("added 1 brush"), "{report}");
        assert!(report.contains("untextured"), "{report}");
        assert!(workspace.is_dirty());

        // Seven more, 1024 apart: a colonnade.
        let arrayed = workspace
            .array(None, 0, 1, 7, [1024, 0, 0], None)
            .expect("the array applies");
        assert!(arrayed.contains("added 7 brushes"), "{arrayed}");
        assert_eq!(workspace.doc.scenes[0].brushes.len(), 8);

        // A radial array sweeps instead of translating.
        workspace
            .array(
                None,
                0,
                1,
                3,
                [0, 0, 0],
                Some(RadialArray {
                    center: [0, 0, 0],
                    degrees: 180,
                }),
            )
            .expect("the radial array applies");
        assert_eq!(workspace.doc.scenes[0].brushes.len(), 11);

        // A linear array with no offset is refused rather than stacking
        // copies invisibly inside each other.
        assert!(workspace.array(None, 0, 1, 2, [0, 0, 0], None).is_err());

        // Deleting says how the indices move.
        let deleted = workspace.delete(None, 8, 3).expect("delete applies");
        assert!(deleted.contains("shifted down by 3"), "{deleted}");
        assert_eq!(workspace.doc.scenes[0].brushes.len(), 8);

        // Save writes, and a second save has nothing to do.
        assert!(workspace.save().unwrap().contains("saved"));
        assert!(!workspace.is_dirty());

        // Now simulate the editor saving underneath a staged edit.
        workspace
            .add_shape(None, [0, 0, 0], [512, 512, 512], BrushDrawSettings::default(), 64, None)
            .expect("a box builds");
        std::fs::write(&workspace.path, "// touched by the editor\n").unwrap();
        let refused = workspace.save().expect_err("a changed file must block the save");
        assert!(refused.contains("changed on disk"), "{refused}");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Carving is what stops `make_room` producing a sealed box, so it has to
    /// actually open a hole, keep the remainder solid, and say that indices
    /// moved.
    #[test]
    fn carve_opens_a_doorway_and_reports_the_index_shift() {
        let dir = std::env::temp_dir().join(format!("psxed-mcp-carve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut workspace = empty_workspace(&dir);

        workspace
            .make_room(None, [0, 0, 0], [4096, 2048, 4096], 256, None)
            .expect("the room builds");
        let before = workspace.doc.scenes[0].brushes.len();
        assert_eq!(before, 6, "a hollow box is six slabs");

        // A doorway through the -X wall, which spans x -256..0.
        let report = workspace
            .carve(None, 0, before, [-256, 0, 1536], [0, 1280, 2560], None)
            .expect("the cutter meets the wall");
        assert!(report.contains("carved 1 of 6"), "{report}");
        assert!(report.contains("has moved"), "{report}");
        let after = workspace.doc.scenes[0].brushes.len();
        assert!(after > before, "one wall became several pieces: {after}");

        // Every piece still encloses volume; a carve that leaves slivers
        // would poison the cook.
        assert!(workspace.doc.scenes[0]
            .brushes
            .iter()
            .all(|brush| brush.solve().is_valid()));

        // The opening is really empty: a point mid-doorway is inside nothing.
        let inside_doorway = [-128.0, 640.0, 2048.0];
        assert!(!workspace.doc.scenes[0]
            .brushes
            .iter()
            .any(|brush| crate::contains(brush, inside_doorway)));
        // ... while the wall beside it is still solid.
        let beside = [-128.0, 640.0, 512.0];
        assert!(workspace.doc.scenes[0]
            .brushes
            .iter()
            .any(|brush| crate::contains(brush, beside)));

        // A cutter that touches nothing is an error, not a silent no-op.
        assert!(workspace
            .carve(None, 0, 1, [90_000, 0, 0], [91_000, 100, 100], None)
            .is_err());

        std::fs::remove_dir_all(&dir).ok();
    }
}
