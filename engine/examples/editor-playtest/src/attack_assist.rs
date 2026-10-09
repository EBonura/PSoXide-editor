//! Locked-on melee assists: turning toward the target during the windup and a
//! short lunge that closes a small gap before the swing lands.
//!
//! Both read the target of a deliberate hard lock and the first active frame
//! of the current attack's authored hitbox, so they follow each clip's own
//! timing instead of a second set of per-attack numbers. Neither changes what
//! can hit: tracking only moves the body's facing and the lunge goes through
//! the ordinary motor, so walls, props and the enemy's body still stop it.
//! Every value is a starting point chosen for the Graybox Reach scale and has
//! not been tuned against hardware play.
use super::*;

/// Windup turn cap, Q12 yaw per 60 Hz tick (4096 is a full turn): about
/// 2.8 degrees a tick, 169 degrees a second. The enemy's tell turn is 16.
const TRACK_TURN_STEP_Q12: u16 = 32;
/// Tracking stops this many source frames before the first active frame, so
/// the swing direction commits shortly before it lands.
const TRACK_STOP_BEFORE_ACTIVE_FRAMES: u32 = 3;

/// Lunge band, runtime units between body centres. Below the near edge the
/// target is already inside striking range (measured light and heavy hits land
/// at 24 to 76, mostly 44 to 50) and the lunge stops there instead of
/// shoving the player in; beyond the far edge the attack is not aimed at that
/// target.
const LUNGE_NEAR_UNITS: i32 = 56;
const LUNGE_FAR_UNITS: i32 = 112;
/// Lunge speed, Q8 units per tick: 1.5 units a tick for a light attack and 2
/// for a heavy one, so a full-length lunge covers up to roughly 14 to 22 units
/// of the gap before it reaches the near edge.
const LIGHT_LUNGE_SPEED_Q8: i32 = 384;
const HEAVY_LUNGE_SPEED_Q8: i32 = 512;
/// The lunge runs for this many source frames up to and including the first
/// active frame.
const LIGHT_LUNGE_LEAD_FRAMES: u32 = 8;
const HEAVY_LUNGE_LEAD_FRAMES: u32 = 10;

impl Playtest {
    /// The current melee attack's first authored active frame and the frame
    /// the retained pose shows, or `None` outside a melee attack.
    fn melee_attack_frames(&self) -> Option<(u32, u32)> {
        if !matches!(
            self.anim_state,
            PlayerAnim::LightAttack
                | PlayerAnim::LightAttackFollowup
                | PlayerAnim::LightAttackFinisher
                | PlayerAnim::HeavyAttack
        ) {
            return None;
        }
        let (character, pose) = (self.character.as_ref()?, self.player_actor_pose?);
        let first = character.combat_capsule_first.to_usize();
        let action = self.anim_state.action().to_index() as u8;
        let active_start = COMBAT_CAPSULES
            .get(first..first + usize::from(character.combat_capsule_count))?
            .iter()
            .filter(|capsule| {
                capsule.flags & psx_level::combat_capsule_flags::HITBOX != 0
                    && capsule.action == action
            })
            .map(|capsule| u32::from(capsule.active_start_frame))
            .min()?;
        Some((active_start, pose.pose().phase_q12() >> 12))
    }

    /// Turn the body toward a hard-locked target while the swing winds up, at
    /// most [`TRACK_TURN_STEP_Q12`] a tick. Without a valid lock the facing is
    /// left as the attack start set it.
    pub(super) fn track_locked_target_in_windup(&mut self) {
        let Some((active_start, frame)) = self.melee_attack_frames() else {
            return;
        };
        let Some(target) = self.lock_target_position() else {
            return;
        };
        let Some(wanted) = psx_engine::yaw_to_point(self.motor.position(), target) else {
            return;
        };
        if let Some(yaw) = tracked_yaw(self.motor.yaw(), wanted, frame, active_start) {
            self.motor.face(yaw);
        }
    }

    /// Forward lunge speed (Q8 units per tick) for this tick, if a hard-locked
    /// target stands inside the lunge band and the swing is in its lunge
    /// window. See [`lunge_speed_q8`].
    pub(super) fn attack_lunge_speed_q8(&self) -> Option<i32> {
        let (active_start, frame) = self.melee_attack_frames()?;
        let target = self.lock_target_position()?;
        let distance = psx_math::int32::isqrt_i32(distance_xz_sq(self.motor.position(), target));
        lunge_speed_q8(
            self.anim_state == PlayerAnim::HeavyAttack,
            frame,
            active_start,
            distance,
        )
    }
}

/// The next facing while a swing winds up: one capped step toward `wanted`,
/// or `None` once the swing is within [`TRACK_STOP_BEFORE_ACTIVE_FRAMES`] of
/// its first active frame (the direction is committed) or already past it.
fn tracked_yaw(current: Angle, wanted: Angle, frame: u32, active_start: u32) -> Option<Angle> {
    (frame + TRACK_STOP_BEFORE_ACTIVE_FRAMES < active_start)
        .then(|| current.approach_q12(wanted, TRACK_TURN_STEP_Q12))
}

/// Lunge speed for one tick, or `None` outside the lunge. The window is the
/// last few frames up to and including the first active frame; the band is
/// the centre distance in which a gap is worth closing, so the lunge ends on
/// its own when the gap closes to the near edge.
fn lunge_speed_q8(heavy: bool, frame: u32, active_start: u32, distance: i32) -> Option<i32> {
    let (lead, speed) = if heavy {
        (HEAVY_LUNGE_LEAD_FRAMES, HEAVY_LUNGE_SPEED_Q8)
    } else {
        (LIGHT_LUNGE_LEAD_FRAMES, LIGHT_LUNGE_SPEED_Q8)
    };
    if frame > active_start || frame + lead < active_start {
        return None;
    }
    (LUNGE_NEAR_UNITS..=LUNGE_FAR_UNITS)
        .contains(&distance)
        .then_some(speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lunge_runs_only_in_its_window_and_band() {
        // Light: frames 14..=22 for a swing that goes active at 22.
        assert_eq!(lunge_speed_q8(false, 13, 22, 80), None);
        assert!(lunge_speed_q8(false, 14, 22, 80).is_some());
        assert!(lunge_speed_q8(false, 22, 22, 80).is_some());
        assert_eq!(lunge_speed_q8(false, 23, 22, 80), None);
        // Heavy leads by two more frames and goes faster.
        assert_eq!(lunge_speed_q8(true, 12, 23, 80), None);
        assert!(
            lunge_speed_q8(true, 13, 23, 80).unwrap() > lunge_speed_q8(false, 14, 22, 80).unwrap()
        );
        // Band: no lunge inside striking range or beyond the far edge.
        assert_eq!(lunge_speed_q8(false, 20, 22, LUNGE_NEAR_UNITS - 1), None);
        assert!(lunge_speed_q8(false, 20, 22, LUNGE_NEAR_UNITS).is_some());
        assert!(lunge_speed_q8(false, 20, 22, LUNGE_FAR_UNITS).is_some());
        assert_eq!(lunge_speed_q8(false, 20, 22, LUNGE_FAR_UNITS + 1), None);
    }

    #[test]
    fn the_lunge_closes_to_the_near_edge_and_stops_there() {
        let mut distance = LUNGE_FAR_UNITS * 256;
        let mut ticks = 0;
        while let Some(speed) = lunge_speed_q8(false, 20, 22, distance / 256) {
            distance -= speed;
            ticks += 1;
            assert!(ticks < 1000);
        }
        let stopped = distance / 256;
        assert!(
            (LUNGE_NEAR_UNITS - 2..LUNGE_NEAR_UNITS).contains(&stopped),
            "{stopped}"
        );
    }

    #[test]
    fn the_windup_turn_is_capped_per_tick_and_takes_the_short_way_round() {
        let current = Angle::from_q12(10);
        let wanted = Angle::from_q12(4090);
        // 12 units the short way through zero, well inside one capped step.
        assert_eq!(tracked_yaw(current, wanted, 2, 22), Some(wanted));
        let far = Angle::from_q12(2000);
        let step = tracked_yaw(current, far, 2, 22).unwrap();
        assert_eq!(step.as_q12(), 10 + TRACK_TURN_STEP_Q12);
        let around = tracked_yaw(Angle::from_q12(100), Angle::from_q12(3000), 2, 22).unwrap();
        assert_eq!(around.as_q12(), 100 - TRACK_TURN_STEP_Q12);
    }

    #[test]
    fn tracking_commits_before_the_active_frames_and_never_runs_inside_them() {
        let (a, b) = (Angle::from_q12(0), Angle::from_q12(500));
        assert!(tracked_yaw(a, b, 18, 22).is_some());
        assert!(tracked_yaw(a, b, 19, 22).is_none());
        assert!(tracked_yaw(a, b, 22, 22).is_none());
        assert!(tracked_yaw(a, b, 30, 22).is_none());
    }
}
