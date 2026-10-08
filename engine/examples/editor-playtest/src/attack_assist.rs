//! Locked-on melee assist: turning toward the target during the windup.
//!
//! It reads the target of a deliberate hard lock and the first active frame of
//! the current attack's authored hitbox, so it follows each clip's own timing
//! instead of a second set of per-attack numbers. It only moves the body's
//! facing; what can hit is unchanged. The values are starting points, not
//! tuned against hardware play.
use super::*;

/// Windup turn cap, Q12 yaw per 60 Hz tick (4096 is a full turn): about
/// 2.8 degrees a tick, 169 degrees a second. The enemy's tell turn is 16.
const TRACK_TURN_STEP_Q12: u16 = 32;
/// Tracking stops this many source frames before the first active frame, so
/// the swing direction commits shortly before it lands.
const TRACK_STOP_BEFORE_ACTIVE_FRAMES: u32 = 3;

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
}

/// The next facing while a swing winds up: one capped step toward `wanted`,
/// or `None` once the swing is within [`TRACK_STOP_BEFORE_ACTIVE_FRAMES`] of
/// its first active frame (the direction is committed) or already past it.
fn tracked_yaw(current: Angle, wanted: Angle, frame: u32, active_start: u32) -> Option<Angle> {
    (frame + TRACK_STOP_BEFORE_ACTIVE_FRAMES < active_start)
        .then(|| current.approach_q12(wanted, TRACK_TURN_STEP_Q12))
}

#[cfg(test)]
mod tests {
    use super::*;

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
