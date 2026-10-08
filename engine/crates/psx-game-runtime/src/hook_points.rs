//! Fixed-duration hook traversal and deterministic screen-space acquisition.
use psx_engine::RoomPoint;

/// Visible anticipation before the polygon burst.
pub const LAUNCH_TICKS: u32 = 12;
/// Leave three capture ticks after anticipation before moving.
pub const DEPARTURE_TICKS: u32 = LAUNCH_TICKS + 3;
/// Body reconstruction begins after the wire reaches the destination.
pub const TRAVEL_TICKS: u32 = LAUNCH_TICKS + 30;
/// Fixed cap keeps per-tick visibility and collision work bounded.
pub const MAX_HOOK_POINTS: usize = psx_level::MAX_HOOK_POINTS;
/// Maximum acquisition distance in runtime world units.
pub const MAX_RANGE: i32 = 1600;
/// Visual anchor height above the landing.
pub const MARKER_HEIGHT: i32 = 96;
/// Acquisition radius around the reticle, in native screen pixels.
pub const AIM_RADIUS: i32 = 30;

/// One flight, driven by the fixed simulation clock.
#[derive(Clone, Copy, Debug)]
pub struct HookTravel {
    /// Departure foot position.
    pub start: RoomPoint,
    /// Validated destination foot position.
    pub landing: RoomPoint,
    /// Start simulation tick.
    pub started: u32,
}
impl HookTravel {
    /// Fixed trailing heading from the departure-to-landing bearing. Using the
    /// original endpoints avoids a flip or lost heading on the vertical descent.
    pub fn camera_yaw(&self, fallback: psx_engine::Angle) -> psx_engine::Angle {
        psx_engine::yaw_to_point(self.start, self.landing)
            .map(|yaw| yaw.add(psx_engine::Angle::HALF))
            .unwrap_or(fallback)
    }
    /// Direct attraction to the perch avoids an artificial apex hitting indoor ceilings.
    pub fn position(&self, now: u32) -> RoomPoint {
        let age = now.wrapping_sub(self.started).min(TRAVEL_TICKS);
        let t = age.saturating_sub(DEPARTURE_TICKS) as i32;
        let duration = (TRAVEL_TICKS - DEPARTURE_TICKS) as i32;
        let lerp = |a: i32, b: i32| a + (b - a) * t / duration;
        RoomPoint::new(
            lerp(self.start.x, self.landing.x),
            lerp(self.start.y, self.landing.y),
            lerp(self.start.z, self.landing.z),
        )
    }
    /// Whether the wire has reached the landing.
    pub fn finished(&self, now: u32) -> bool {
        now.wrapping_sub(self.started) >= TRAVEL_TICKS
    }
}

/// Perches within a bounded radius are eligible, including lower arches.
/// The caller additionally proves visibility and body clearance.
pub fn in_range(player: RoomPoint, landing: RoomPoint) -> bool {
    let dx = landing.x.saturating_sub(player.x);
    let dy = landing.y.saturating_sub(player.y);
    let dz = landing.z.saturating_sub(player.z);
    if (dx == 0 && dy == 0 && dz == 0)
        || [dx, dy, dz].iter().any(|v| v.saturating_abs() > MAX_RANGE)
    {
        return false;
    }
    dx * dx + dy * dy + dz * dz <= MAX_RANGE * MAX_RANGE
}

/// Squared reticle distance for an eligible projected hook.
pub fn aim_score(x: i32, y: i32, aim_x: i32, aim_y: i32) -> Option<i32> {
    let dx = x - aim_x;
    let dy = y - aim_y;
    let score = dx * dx + dy * dy;
    (score <= AIM_RADIUS * AIM_RADIUS).then_some(score)
}

/// Small anticipation squeeze, then a wider flight lens. Zero is the player's
/// normal lens; negative values shorten focal length and widen field of view.
pub fn lens_target_q8(age: Option<u32>) -> i32 {
    match age {
        Some(age) if age < LAUNCH_TICKS => 8,
        Some(_) => -52,
        None => 0,
    }
}

/// Bounded lens easing also settles cancelled moves back to the base profile.
pub fn ease_lens_q8(current: i32, target: i32) -> i32 {
    current + (target - current).clamp(-6, 6)
}

/// Authored left-hand contact relative to the motor root (Aletha ArchPerch v1).
pub const PERCH_HAND: [i32; 3] = [12, 81, -33];
/// Quantized aiming lattice: five yaw columns and five pitch rows, two recoil banks.
pub const PERCH_FRAMES: u16 = 50;

/// Two adjacent pitch rows, each interpolated horizontally by the asset sampler.
/// Angles are Q12 turns; the returned phases never interpolate into another row.
pub fn perch_aim_sample(yaw: i32, pitch: i32, recoil: bool) -> (u32, u32, u16) {
    let x = ((yaw.clamp(-910, 910) + 910) * 4 * 4096 / 1820) as u32;
    let y = ((pitch.clamp(-683, 683) + 683) * 4 * 4096 / 1366) as u32;
    let row = (y >> 12).min(4);
    let next = (row + 1).min(4);
    let bank = if recoil { 25 } else { 0 };
    (
        ((bank + row * 5) << 12) + x,
        ((bank + next * 5) << 12) + x,
        (y & 4095) as u16,
    )
}
/// Ticks of holding fire on an arch to reach a full charge.
pub const CHARGE_TICKS: u16 = 75;
/// A press released within this many ticks is an ordinary shot, fired on
/// release; holding past it starts the charge.
pub const CHARGE_TAP_TICKS: u16 = 10;
/// Energy a charged shot costs, twice an ordinary one. It is fixed to the
/// arch: a perched player cannot dodge, loses refill while charging and holds
/// still for over a second, so the shot is paid for in both resources and risk.
pub const CHARGED_ENERGY_COST: u16 = 40;
/// Health damage of a charged bolt as a multiple of the ordinary bolt's.
pub const CHARGED_DAMAGE_MULTIPLIER: u16 = 3;
/// Poise damage a charged bolt delivers. The ordinary bolt's poise is capped
/// at 10, so this is a real contribution: against an enemy capacity of 50 it
/// breaks poise together with one light melee hit, and against an opposite
/// colour (poise x2) on its own.
pub const CHARGED_POISE_DAMAGE: u16 = 30;
/// Collision radius of a charged bolt as a multiple of the ordinary one, in
/// quarters (6 = 1.5x), so the slower, costlier shot is easier to land.
pub const CHARGED_RADIUS_QUARTERS: u16 = 6;

/// Prototype motion: tower and duel arches move laterally over eight seconds.
pub fn arch_offset(slot: usize, tick: u32) -> i32 {
    if slot != 1 && slot != 3 {
        return 0;
    }
    psx_math::sin_q12(((tick % 480) * 4096 / 480) as u16) * 64 / 4096
}

/// What releasing the fire button after an arch hold produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargeRelease {
    /// No hold was in progress.
    None,
    /// A tap or a hold cut short: an ordinary shot.
    Ordinary,
    /// A full charge: the empowered shot.
    Charged,
}

/// What the pad and the player's state say this tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChargeInput {
    /// Attached, Zenith, aim ready, no action running, Energy for a shot.
    pub can_hold: bool,
    /// The fire button went down this tick.
    pub pressed: bool,
    /// The fire button is down.
    pub held: bool,
    /// Energy covers the charged shot.
    pub may_charge: bool,
}

/// What one tick of the charge decided.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChargeStep {
    /// Queue an ordinary shot request.
    pub ordinary_shot: bool,
    /// Queue a charged shot request.
    pub charged_shot: bool,
    /// The hold just outlasted a tap and the charge began building.
    pub began_charging: bool,
    /// The charge just became full.
    pub became_ready: bool,
}

/// Hold-to-charge state for the arch shot. A fresh press arms it; the hold
/// grows while the button stays down and the weapon stays ready; release
/// reports what to fire. It never accumulates on its own: a detached, swapped
/// or interrupted player simply cancels.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArchCharge {
    held: u16,
    armed: bool,
}
impl ArchCharge {
    /// No hold in progress.
    pub const EMPTY: Self = Self {
        held: 0,
        armed: false,
    };
    /// A fire press with the weapon ready starts a hold.
    pub fn arm(&mut self) {
        self.armed = true;
        self.held = 0;
    }
    /// One tick of the button held down. Without `may_charge` (not enough
    /// Energy for the charged shot) the hold stops at the end of the tap, so
    /// the release stays an ordinary shot.
    pub fn tick(&mut self, may_charge: bool) {
        if self.armed && (may_charge || self.held < CHARGE_TAP_TICKS) {
            self.held = self.held.saturating_add(1).min(CHARGE_TICKS);
        }
    }
    /// Drop the hold without firing (aim lowered, interrupted, swapped, detached).
    pub fn cancel(&mut self) {
        *self = Self::EMPTY;
    }
    /// The button came up: report the shot and end the hold.
    pub fn release(&mut self) -> ChargeRelease {
        let result = if !self.armed {
            ChargeRelease::None
        } else if self.held >= CHARGE_TICKS {
            ChargeRelease::Charged
        } else {
            ChargeRelease::Ordinary
        };
        *self = Self::EMPTY;
        result
    }
    /// Advance one tick: arm on a press, build while held, and on release
    /// report the shot. A tick that cannot hold cancels instead.
    pub fn step(&mut self, input: ChargeInput) -> ChargeStep {
        let mut out = ChargeStep::default();
        if !input.can_hold {
            self.cancel();
            return out;
        }
        if input.pressed {
            self.arm();
        }
        if !self.armed {
            return out;
        }
        if input.held {
            let (was_charging, was_ready) = (self.charging(), self.ready());
            self.tick(input.may_charge);
            out.began_charging = !was_charging && self.charging();
            out.became_ready = !was_ready && self.ready();
            return out;
        }
        match self.release() {
            ChargeRelease::None => {}
            ChargeRelease::Ordinary => out.ordinary_shot = true,
            ChargeRelease::Charged => out.charged_shot = true,
        }
        out
    }
    /// Whether a hold is in progress.
    pub fn armed(&self) -> bool {
        self.armed
    }
    /// True once the hold has outlasted a tap, so the charge is building.
    pub fn charging(&self) -> bool {
        self.armed && self.held > CHARGE_TAP_TICKS
    }
    /// Whether the charge is full and the next release fires the empowered shot.
    pub fn ready(&self) -> bool {
        self.armed && self.held >= CHARGE_TICKS
    }
    /// Charge progress, Q12: zero through the tap window, then rising to
    /// 4096 over the rest of the hold.
    pub fn progress_q12(&self) -> u16 {
        if !self.charging() {
            return 0;
        }
        let span = u32::from(CHARGE_TICKS - CHARGE_TAP_TICKS);
        (u32::from(self.held - CHARGE_TAP_TICKS) * 4096 / span).min(4096) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn perch_samples_stay_inside_their_rows_and_banks() {
        for recoil in [false, true] {
            for yaw in [-4096, -910, -455, 0, 455, 910, 4096] {
                for pitch in [-4096, -683, 0, 683, 4096] {
                    let (a, b, w) = perch_aim_sample(yaw, pitch, recoil);
                    assert!(a <= b && b < u32::from(PERCH_FRAMES) * 4096);
                    assert!(w < 4096);
                    assert_eq!((a >> 12) / 25, u32::from(recoil));
                    assert_eq!((b >> 12) / 25, u32::from(recoil));
                }
            }
        }
        assert_eq!(perch_aim_sample(0, 0, false), (12 << 12, 17 << 12, 0));
    }
    #[test]
    fn a_tap_is_an_ordinary_shot_and_a_full_hold_is_the_charged_one() {
        let mut c = ArchCharge::EMPTY;
        assert_eq!(c.release(), ChargeRelease::None);
        c.arm();
        for _ in 0..CHARGE_TAP_TICKS {
            c.tick(true);
        }
        assert!(!c.charging(), "a tap never shows charge");
        assert_eq!(c.progress_q12(), 0);
        assert_eq!(c.release(), ChargeRelease::Ordinary);
        assert!(!c.armed());
        c.arm();
        for _ in 0..CHARGE_TICKS - 1 {
            c.tick(true);
        }
        assert!(c.charging() && !c.ready());
        assert_eq!(
            c.release(),
            ChargeRelease::Ordinary,
            "a cut-short hold is ordinary"
        );
        c.arm();
        for _ in 0..CHARGE_TICKS + 40 {
            c.tick(true);
        }
        assert!(c.ready());
        assert_eq!(c.progress_q12(), 4096);
        assert_eq!(c.release(), ChargeRelease::Charged);
        assert_eq!(
            c.release(),
            ChargeRelease::None,
            "a release spends the hold"
        );
    }

    #[test]
    fn the_step_machine_arms_builds_and_fires_on_release() {
        let hold = ChargeInput {
            can_hold: true,
            held: true,
            may_charge: true,
            ..ChargeInput::default()
        };
        let press = ChargeInput {
            pressed: true,
            ..hold
        };
        let release = ChargeInput {
            held: false,
            ..hold
        };
        // A tap: nothing fires on the press, an ordinary shot on release.
        let mut c = ArchCharge::EMPTY;
        assert_eq!(c.step(press), ChargeStep::default());
        assert_eq!(c.step(hold), ChargeStep::default());
        assert_eq!(
            c.step(release),
            ChargeStep {
                ordinary_shot: true,
                ..ChargeStep::default()
            }
        );
        // A full hold reports its two milestones once each, then fires charged.
        let mut began = 0;
        let mut ready = 0;
        c.step(press);
        for _ in 0..CHARGE_TICKS + 20 {
            let step = c.step(hold);
            began += u32::from(step.began_charging);
            ready += u32::from(step.became_ready);
        }
        assert_eq!((began, ready), (1, 1));
        assert_eq!(
            c.step(release),
            ChargeStep {
                charged_shot: true,
                ..ChargeStep::default()
            }
        );
        // Releasing with nothing armed is nothing.
        assert_eq!(c.step(release), ChargeStep::default());
    }

    #[test]
    fn losing_the_ability_to_hold_cancels_without_firing() {
        let hold = ChargeInput {
            can_hold: true,
            held: true,
            may_charge: true,
            ..ChargeInput::default()
        };
        let mut c = ArchCharge::EMPTY;
        c.step(ChargeInput {
            pressed: true,
            ..hold
        });
        for _ in 0..CHARGE_TICKS {
            c.step(hold);
        }
        assert!(c.ready());
        // Aim lowered, a hit, a detach: the weapon is not ready this tick.
        let lost = ChargeInput {
            can_hold: false,
            ..hold
        };
        assert_eq!(c.step(lost), ChargeStep::default());
        assert!(!c.armed());
        // The button is still down, but without a fresh press nothing re-arms.
        assert_eq!(c.step(hold), ChargeStep::default());
        assert!(!c.armed());
        // A press while the weapon is not ready does not arm either.
        c.step(ChargeInput {
            pressed: true,
            can_hold: false,
            ..hold
        });
        assert!(!c.armed());
    }

    #[test]
    fn without_the_energy_the_hold_never_becomes_a_charge() {
        let mut c = ArchCharge::EMPTY;
        c.arm();
        for _ in 0..CHARGE_TICKS * 2 {
            c.tick(false);
        }
        assert!(!c.charging() && !c.ready());
        assert_eq!(c.release(), ChargeRelease::Ordinary);
    }

    #[test]
    fn progress_rises_monotonically_from_the_end_of_the_tap_to_full() {
        let mut c = ArchCharge::EMPTY;
        c.arm();
        let mut last = 0;
        for _ in 0..CHARGE_TICKS {
            c.tick(true);
            assert!(c.progress_q12() >= last);
            last = c.progress_q12();
        }
        assert_eq!(last, 4096);
        c.cancel();
        assert!(!c.armed() && c.progress_q12() == 0 && !c.ready());
        // Ticking without a press never builds anything.
        for _ in 0..CHARGE_TICKS * 2 {
            c.tick(true);
        }
        assert!(!c.charging() && !c.ready());
    }

    #[test]
    fn the_charged_shot_costs_more_than_one_ordinary_shot_but_less_than_the_damage_it_buys() {
        assert!(CHARGED_ENERGY_COST > crate::combat_flow::SHOT_COST);
        assert!(CHARGED_DAMAGE_MULTIPLIER * crate::combat_flow::SHOT_COST > CHARGED_ENERGY_COST);
        assert!(
            CHARGED_POISE_DAMAGE > 10,
            "above the ordinary bolt's capped poise"
        );
    }

    #[test]
    fn moving_arch_is_bounded_and_periodic() {
        for t in 0..960 {
            assert!(arch_offset(1, t).abs() <= 64);
            assert_eq!(arch_offset(1, t), arch_offset(1, t + 480));
            assert_eq!(arch_offset(0, t), 0);
        }
        assert!(in_range(RoomPoint::new(0, 200, 0), RoomPoint::ZERO));
    }
    #[test]
    fn camera_heading_follows_route_and_retains_vertical_fallback() {
        use psx_engine::Angle;
        let mut flight = HookTravel {
            start: RoomPoint::ZERO,
            landing: RoomPoint::new(200, 200, 200),
            started: 0,
        };
        let expected = psx_engine::yaw_to_point(flight.start, flight.landing)
            .unwrap()
            .add(Angle::HALF);
        assert_eq!(flight.camera_yaw(Angle::ZERO), expected);
        assert_eq!(flight.camera_yaw(Angle::HALF), expected);
        flight.landing = RoomPoint::new(0, 200, 0);
        assert_eq!(flight.camera_yaw(expected), expected);
    }
    #[test]
    fn launch_lens_widens_then_restores_without_overshoot() {
        let mut lens = 0;
        for _ in 0..12 {
            lens = ease_lens_q8(lens, lens_target_q8(Some(0)));
        }
        assert_eq!(lens, 8);
        for _ in 0..12 {
            lens = ease_lens_q8(lens, lens_target_q8(Some(LAUNCH_TICKS)));
        }
        assert_eq!(lens, -52);
        for _ in 0..12 {
            lens = ease_lens_q8(lens, lens_target_q8(None));
        }
        assert_eq!(lens, 0);
    }
    #[test]
    fn elevated_range_and_aim_are_bounded() {
        assert!(in_range(RoomPoint::ZERO, RoomPoint::new(400, 192, 200)));
        for p in [
            RoomPoint::ZERO,
            RoomPoint::new(1600, 32, 0),
            RoomPoint::new(i32::MAX, 64, 0),
        ] {
            assert!(!in_range(RoomPoint::ZERO, p));
        }
        assert_eq!(aim_score(160, 120, 160, 120), Some(0));
        assert_eq!(aim_score(190, 120, 160, 120), Some(900));
        assert_eq!(aim_score(191, 120, 160, 120), None);
    }
    #[test]
    fn departure_holds_then_rises_and_lands_exactly_without_overshoot() {
        let f = HookTravel {
            start: RoomPoint::new(-100, 0, -400),
            landing: RoomPoint::new(300, 192, 100),
            started: 100,
        };
        assert_eq!(f.position(115), f.start);
        assert!(f.position(135).y < f.landing.y);
        assert_eq!(f.position(142), f.landing);
        assert_eq!(f.position(150), f.landing);
        for t in 116..135 {
            assert!(f.position(t).y > f.start.y);
            assert!(f.position(t).y <= f.landing.y);
        }
        assert!(!f.finished(141));
        assert!(f.finished(142));
    }
}
