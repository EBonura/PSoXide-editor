//! The geometry of plane handles: where they sit, what a drag does to the
//! plane, and how drags snap.
//!
//! Everything works in absolute grid units (`x` and `z` of the world lattice,
//! `y` including the room's vertical offset) as `f64`, independent of the
//! views. The tools turn pointer positions into points in the plane's
//! surface, in a horizontal plane or in a vertical plane, and ask this module
//! for the edited plane. Nothing here touches the document.

use psxed_project::grid::plane::{wrap_degrees, GridPlane, MAX_PLANE_SIZE, MIN_PLANE_SIZE};
use psxed_project::grid::units::{CLICK_UNITS, FINE_UNITS};

/// A point or vector in absolute grid units.
pub(crate) type V3 = [f64; 3];

pub(crate) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub(crate) fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Snapping of a drag: the step, or none for free movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Snap(pub Option<i32>);

impl Snap {
    /// 256 grid units, the default click.
    pub const CLICK: Self = Self(Some(CLICK_UNITS));

    /// The step the held modifiers ask for, the one rule every grid tool
    /// shares: Alt is the fine step (64 units), Shift+Alt is free,
    /// otherwise a click.
    pub fn from_modifiers(modifiers: egui::Modifiers) -> Self {
        if modifiers.alt && modifiers.shift {
            Self(None)
        } else if modifiers.alt {
            Self(Some(FINE_UNITS))
        } else {
            Self::CLICK
        }
    }

    /// Snap a value. Free movement rounds to whole grid units.
    pub fn apply(self, value: f64) -> f64 {
        match self.0 {
            Some(step) => (value / f64::from(step)).round() * f64::from(step),
            None => value.round(),
        }
    }
}

/// A plane's frame in absolute grid units.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub center: V3,
    pub right: V3,
    pub up: V3,
    pub normal: V3,
    pub width: f64,
    pub height: f64,
}

impl Frame {
    /// The frame of `plane` in a room lifted by `y_offset`.
    pub fn of(plane: &GridPlane, y_offset: i32) -> Self {
        let (right, up, normal) = plane.axes();
        Self {
            center: [
                f64::from(plane.center[0]),
                f64::from(plane.center[1] + y_offset),
                f64::from(plane.center[2]),
            ],
            right,
            up,
            normal,
            width: f64::from(plane.size[0]),
            height: f64::from(plane.size[1]),
        }
    }

    /// The point `a` along `right` and `b` along `up` from the centre.
    pub fn at(&self, a: f64, b: f64) -> V3 {
        add(self.center, add(scale(self.right, a), scale(self.up, b)))
    }

    /// Corners top-left, top-right, bottom-right, bottom-left.
    pub fn corners(&self) -> [V3; 4] {
        let (w, h) = (self.width / 2.0, self.height / 2.0);
        [
            self.at(-w, h),
            self.at(w, h),
            self.at(w, -h),
            self.at(-w, -h),
        ]
    }

    /// In-plane coordinates `(a, b)` of a point (its distance off the plane
    /// is dropped).
    pub fn coords(&self, p: V3) -> (f64, f64) {
        let d = sub(p, self.center);
        (dot(d, self.right), dot(d, self.up))
    }

    /// The horizontal unit direction the plane's front faces, or a fallback
    /// when it lies flat.
    pub fn facing(&self) -> V3 {
        let len = self.normal[0].hypot(self.normal[2]);
        if len < 1e-6 {
            [0.0, 0.0, 1.0]
        } else {
            [self.normal[0] / len, 0.0, self.normal[2] / len]
        }
    }
}

/// One of the four edges of a plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Top, Self::Bottom];

    /// `(axis, sign)`: axis 0 is `right`, 1 is `up`; the sign is the side of
    /// the centre the edge lies on.
    fn axis(self) -> (usize, f64) {
        match self {
            Self::Left => (0, -1.0),
            Self::Right => (0, 1.0),
            Self::Top => (1, 1.0),
            Self::Bottom => (1, -1.0),
        }
    }
}

/// What a press on a plane's gizmo grabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Handle {
    /// Stretch one edge.
    Edge(Side),
    /// Stretch two edges: `(sign along right, sign along up)`.
    Corner(i8, i8),
    /// Turn about the vertical axis.
    Yaw,
    /// Tilt the normal up and down.
    Pitch,
    /// Slide the whole plane.
    Body,
}

impl Handle {
    /// Every edge and corner, in hit-test order (corners first).
    pub fn grips() -> Vec<Self> {
        let mut out = vec![
            Self::Corner(-1, 1),
            Self::Corner(1, 1),
            Self::Corner(1, -1),
            Self::Corner(-1, -1),
        ];
        out.extend(Side::ALL.map(Self::Edge));
        out
    }
}

/// Where a handle sits.
pub(crate) fn handle_position(frame: &Frame, handle: Handle) -> V3 {
    let (w, h) = (frame.width / 2.0, frame.height / 2.0);
    match handle {
        Handle::Edge(side) => {
            let (axis, sign) = side.axis();
            if axis == 0 {
                frame.at(sign * w, 0.0)
            } else {
                frame.at(0.0, sign * h)
            }
        }
        Handle::Corner(sx, sy) => frame.at(f64::from(sx) * w, f64::from(sy) * h),
        Handle::Yaw => add(frame.center, scale(frame.facing(), yaw_radius(frame))),
        Handle::Pitch => add(frame.center, scale(frame.normal, pitch_reach(frame))),
        Handle::Body => frame.center,
    }
}

/// Distance of the yaw knob from the centre, in the horizontal plane.
pub(crate) fn yaw_radius(frame: &Frame) -> f64 {
    frame.width.hypot(frame.height) / 2.0 + 192.0
}

/// Distance of the pitch knob from the centre, along the normal.
pub(crate) fn pitch_reach(frame: &Frame) -> f64 {
    (frame.width.max(frame.height) / 2.0).max(384.0) + 128.0
}

fn clamp_size(length: f64) -> f64 {
    length.clamp(f64::from(MIN_PLANE_SIZE), f64::from(MAX_PLANE_SIZE))
}

/// The world axis `axis` is parallel to, when it is (to a hair).
fn world_axis(axis: V3) -> Option<(usize, f64)> {
    (0..3).find_map(|k| (axis[k].abs() > 0.9995).then(|| (k, axis[k].signum())))
}

/// New length of an edge pair's axis: the moving edge is at `target`
/// (coordinate along `axis` from the centre, on the `sign` side) and the
/// other edge stays at `fixed`. Snapping puts the moving edge on the lattice
/// when the axis lies along a world axis, else snaps the length.
fn stretched_length(
    frame: &Frame,
    axis: V3,
    sign: f64,
    fixed: f64,
    target: f64,
    snap: Snap,
) -> f64 {
    let anchor = add(frame.center, scale(axis, fixed));
    let mut length = sign * (target - fixed);
    if let Some((k, direction)) = world_axis(axis) {
        // Snap the moving edge's coordinate on that world axis.
        let moving = anchor[k] + direction * sign * length;
        let snapped = snap.apply(moving);
        length = direction * sign * (snapped - anchor[k]);
    } else {
        length = snap.apply(length);
    }
    clamp_size(length)
}

/// Drag an edge or corner handle: `point` is the pointer's point in the
/// plane's surface. The opposite edges stay put.
pub(crate) fn stretch(
    original: &GridPlane,
    y_offset: i32,
    handle: Handle,
    point: V3,
    snap: Snap,
) -> GridPlane {
    let frame = Frame::of(original, y_offset);
    let (a, b) = frame.coords(point);
    let mut moves: Vec<(usize, f64)> = Vec::new();
    match handle {
        Handle::Edge(side) => moves.push((side.axis().0, side.axis().1)),
        Handle::Corner(sx, sy) => {
            moves.push((0, f64::from(sx)));
            moves.push((1, f64::from(sy)));
        }
        _ => return original.clone(),
    }
    let mut size = [frame.width, frame.height];
    let mut shift = [0.0, 0.0];
    for (axis_index, sign) in moves {
        let (axis, half, target) = if axis_index == 0 {
            (frame.right, frame.width / 2.0, a)
        } else {
            (frame.up, frame.height / 2.0, b)
        };
        let fixed = -sign * half;
        let length = stretched_length(&frame, axis, sign, fixed, target, snap);
        size[axis_index] = length;
        shift[axis_index] = fixed + sign * length / 2.0;
    }
    let mut plane = original.clone();
    let centre = frame.at(shift[0], shift[1]);
    plane.center = [
        centre[0].round() as i32,
        centre[1].round() as i32 - y_offset,
        centre[2].round() as i32,
    ];
    plane.size = [size[0].round() as i32, size[1].round() as i32];
    plane
}

/// Offset to add to a move of `delta` so that the plane's centre, minimum or
/// maximum on each world axis lands on the snap lattice (whichever is
/// nearest). Axes with a zero `delta` and no snapping are left alone.
pub(crate) fn snap_move(frame: &Frame, corners: &[V3; 4], delta: V3, snap: Snap) -> V3 {
    let mut out = delta;
    for k in 0..3 {
        let lo = corners.iter().map(|c| c[k]).fold(f64::MAX, f64::min);
        let hi = corners.iter().map(|c| c[k]).fold(f64::MIN, f64::max);
        let mid = frame.center[k];
        let mut best: Option<f64> = None;
        for feature in [lo, mid, hi] {
            let moved = feature + delta[k];
            let error = snap.apply(moved) - moved;
            if best.is_none_or(|b| error.abs() < b.abs()) {
                best = Some(error);
            }
        }
        out[k] = delta[k] + best.unwrap_or(0.0);
    }
    out
}

/// Slide the plane by `delta` (grid units), snapping features to the lattice.
pub(crate) fn translate(original: &GridPlane, y_offset: i32, delta: V3, snap: Snap) -> GridPlane {
    let frame = Frame::of(original, y_offset);
    let moved = snap_move(&frame, &frame.corners(), delta, snap);
    let mut plane = original.clone();
    plane.center = [
        (frame.center[0] + moved[0]).round() as i32,
        (frame.center[1] + moved[1]).round() as i32 - y_offset,
        (frame.center[2] + moved[2]).round() as i32,
    ];
    plane
}

/// Yaw that points the front at `point` seen from the centre, snapped.
pub(crate) fn yaw_toward(original: &GridPlane, y_offset: i32, point: V3, snap: Snap) -> GridPlane {
    let frame = Frame::of(original, y_offset);
    let d = sub(point, frame.center);
    let mut plane = original.clone();
    if d[0].hypot(d[2]) < 1.0 {
        return plane;
    }
    let degrees = d[0].atan2(d[2]).to_degrees();
    plane.rotation[0] = wrap_degrees(snap_degrees(degrees, snap));
    plane
}

/// Pitch that points the normal at `point` (a point in the vertical plane
/// through the centre along the current facing), snapped.
pub(crate) fn pitch_toward(
    original: &GridPlane,
    y_offset: i32,
    point: V3,
    snap: Snap,
) -> GridPlane {
    let frame = Frame::of(original, y_offset);
    let d = sub(point, frame.center);
    let along = dot(d, frame.facing());
    let mut plane = original.clone();
    if along.hypot(d[1]) < 1.0 {
        return plane;
    }
    let degrees = d[1].atan2(along).to_degrees();
    plane.rotation[1] = wrap_degrees(snap_degrees(degrees, snap));
    plane
}

/// Snap an angle: 15 degrees by default, 5 for the fine step, whole degrees
/// when free.
pub(crate) fn snap_degrees(degrees: f64, snap: Snap) -> i32 {
    let step = match snap.0 {
        Some(FINE_UNITS) => 5.0,
        Some(_) => 15.0,
        None => 1.0,
    };
    ((degrees / step).round() * step) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall() -> GridPlane {
        // 2048 wide, 1024 tall, standing on y = 0..1024, centred x = 2048.
        GridPlane::new([2048, 512, 1024], [2048, 1024])
    }

    #[test]
    fn handles_sit_on_the_edges_and_corners() {
        let frame = Frame::of(&wall(), 0);
        assert_eq!(
            handle_position(&frame, Handle::Edge(Side::Right)),
            [3072.0, 512.0, 1024.0]
        );
        assert_eq!(
            handle_position(&frame, Handle::Edge(Side::Top)),
            [2048.0, 1024.0, 1024.0]
        );
        assert_eq!(
            handle_position(&frame, Handle::Corner(-1, -1)),
            [1024.0, 0.0, 1024.0]
        );
        // The yaw knob is out in front, the pitch knob along the normal.
        let yaw = handle_position(&frame, Handle::Yaw);
        assert!(yaw[2] > 1024.0 && yaw[1] == 512.0);
        let pitch = handle_position(&frame, Handle::Pitch);
        assert!(pitch[2] > 1024.0 && pitch[0] == 2048.0);
    }

    #[test]
    fn stretching_an_edge_keeps_the_other_edge_and_snaps_to_the_lattice() {
        // Drag the right edge to x = 4600: snaps to 4608, left edge at 1024.
        let got = stretch(
            &wall(),
            0,
            Handle::Edge(Side::Right),
            [4600.0, 512.0, 1024.0],
            Snap::CLICK,
        );
        assert_eq!(got.size, [3584, 1024]);
        assert_eq!(got.center, [2816, 512, 1024]);
        // Drag the left edge inward, free.
        let got = stretch(
            &wall(),
            0,
            Handle::Edge(Side::Left),
            [1500.0, 512.0, 1024.0],
            Snap(None),
        );
        assert_eq!(got.size, [1572, 1024]);
        assert_eq!(got.center[0], 2286);
        // Fine snap.
        let got = stretch(
            &wall(),
            0,
            Handle::Edge(Side::Right),
            [3110.0, 512.0, 1024.0],
            Snap(Some(64)),
        );
        assert_eq!(got.size[0], 2112);
    }

    #[test]
    fn a_corner_stretches_both_axes_and_clamps_to_the_limits() {
        let got = stretch(
            &wall(),
            0,
            Handle::Corner(1, 1),
            [4096.0, 2048.0, 1024.0],
            Snap::CLICK,
        );
        assert_eq!(got.size, [3072, 2048]);
        assert_eq!(got.center, [2560, 1024, 1024]);
        // Past the opposite edge it stops at the minimum size.
        let got = stretch(
            &wall(),
            0,
            Handle::Corner(1, 1),
            [0.0, -500.0, 1024.0],
            Snap::CLICK,
        );
        assert_eq!(got.size, [MIN_PLANE_SIZE, MIN_PLANE_SIZE]);
        let got = stretch(
            &wall(),
            0,
            Handle::Edge(Side::Right),
            [99_999.0, 0.0, 1024.0],
            Snap::CLICK,
        );
        assert_eq!(got.size[0], MAX_PLANE_SIZE);
    }

    #[test]
    fn a_rotated_plane_snaps_its_length_when_no_world_axis_matches() {
        let mut plane = wall();
        plane.rotation = [30, 0, 0];
        let frame = Frame::of(&plane, 0);
        let far = add(frame.at(0.0, 0.0), scale(frame.right, 1900.0));
        let got = stretch(&plane, 0, Handle::Edge(Side::Right), far, Snap::CLICK);
        // Left edge at -1024; the moving edge at 1900 gives 2924, snapped 2816.
        assert_eq!(got.size[0], 2816);
    }

    #[test]
    fn translating_puts_an_edge_or_the_centre_on_the_lattice() {
        // Centre at x 2048 is already on the lattice; dragging by 90 snaps back.
        let got = translate(&wall(), 0, [90.0, 0.0, 0.0], Snap::CLICK);
        assert_eq!(got.center, [2048, 512, 1024]);
        // Dragging by 300 lands the plane's edges on lattice lines.
        let got = translate(&wall(), 0, [300.0, 0.0, 0.0], Snap::CLICK);
        assert_eq!(got.center[0], 2304);
        // A plane 1536 wide: its edge, not its centre, reaches the lattice.
        let mut odd = wall();
        odd.size[0] = 1536;
        let got = translate(&odd, 0, [250.0, 0.0, 0.0], Snap::CLICK);
        let (lo, hi) = got.bounds();
        assert!(lo[0] % 256 == 0 || hi[0] % 256 == 0 || got.center[0] % 256 == 0);
        // Free movement is exact.
        let got = translate(&wall(), 0, [37.0, 5.0, -9.0], Snap(None));
        assert_eq!(got.center, [2085, 517, 1015]);
    }

    #[test]
    fn the_room_offset_is_taken_out_of_the_stored_height() {
        let got = translate(&wall(), 2048, [0.0, 256.0, 0.0], Snap::CLICK);
        assert_eq!(got.center[1], 768);
    }

    #[test]
    fn yaw_and_pitch_follow_the_pointer() {
        let plane = wall();
        let frame = Frame::of(&plane, 0);
        // Pointer due east of the centre: yaw 90.
        let east = add(frame.center, [1000.0, 0.0, 0.0]);
        assert_eq!(yaw_toward(&plane, 0, east, Snap::CLICK).rotation[0], 90);
        let south_west = add(frame.center, [-1000.0, 0.0, 1000.0]);
        assert_eq!(
            yaw_toward(&plane, 0, south_west, Snap::CLICK).rotation[0],
            -45
        );
        // Straight up from the centre lays the plane flat, facing up.
        let up = add(frame.center, [0.0, 1000.0, 0.0]);
        assert_eq!(pitch_toward(&plane, 0, up, Snap::CLICK).rotation[1], 90);
        let ahead_and_up = add(frame.center, [0.0, 500.0, 500.0]);
        assert_eq!(
            pitch_toward(&plane, 0, ahead_and_up, Snap::CLICK).rotation[1],
            45
        );
        // Free angles are whole degrees, fine ones multiples of five.
        let off = add(frame.center, [0.0, 400.0, 1000.0]);
        assert_eq!(pitch_toward(&plane, 0, off, Snap(None)).rotation[1], 22);
        assert_eq!(pitch_toward(&plane, 0, off, Snap(Some(64))).rotation[1], 20);
    }

    #[test]
    fn modifiers_choose_the_snap() {
        assert_eq!(Snap::from_modifiers(egui::Modifiers::NONE), Snap::CLICK);
        assert_eq!(Snap::from_modifiers(egui::Modifiers::ALT), Snap(Some(64)));
        assert_eq!(
            Snap::from_modifiers(egui::Modifiers::ALT | egui::Modifiers::SHIFT),
            Snap(None)
        );
        // Ctrl and Cmd no longer change the snap.
        assert_eq!(Snap::from_modifiers(egui::Modifiers::COMMAND), Snap::CLICK);
    }
}
