//! Hit-stop for the two actors of a melee connect.
//!
//! The struck and the striking actor freeze for a few ticks; the world,
//! projectiles, particles and the renderer keep running. Entities hold their
//! whole step (`GameEntities::begin_hit_stop`). The player holds its animation
//! clock: every frozen tick moves each tick-stamped animation field forward by
//! one, so `now - start` stays constant and the pose, the attack lock and the
//! combat windows resume exactly where they stopped. Everything is a pure
//! function of the connect tick and the fixed counts in
//! `psx_game_runtime::hit_stop`, so replays stay deterministic.
use super::*;
use psx_game_runtime::hit_stop;

impl Playtest {
    /// Freeze the player for `ticks` and, when `struck`, flash its body.
    ///
    /// Only a player inside a locked action (an attack, or the reaction that
    /// just started) is frozen: an idle body has no clock to hold. A player on
    /// an arch, mid-hook, in a motor action or dying is never frozen, because
    /// those states own the body and the tether timer.
    pub(super) fn begin_player_hit_stop(&mut self, ticks: u8, struck: bool, now: SimTick) {
        if struck {
            self.player_hit_flash = hit_stop::FLASH_TICKS;
        }
        if self.hook_attached.is_some()
            || self.hook_travel.is_some()
            || self.hazard_death_ticks_remaining != 0
            || self.anim_lock_until_tick <= now
            || !self.motor.action().is_idle()
        {
            return;
        }
        self.player_hit_stop = self.player_hit_stop.max(ticks);
        self.player_hit_stop_action = self.anim_state.action().to_index() as u8;
    }

    /// Apply the freeze for one connect: the attacker and the struck actor
    /// both hold for `ticks`, and the struck one flashes.
    pub(super) fn hit_stop_player_strikes(&mut self, entity: usize, ticks: u8, now: SimTick) {
        self.game_entities.begin_hit_stop(entity, ticks, true);
        self.begin_player_hit_stop(ticks, false, now);
    }

    /// Enemy `entity` struck the player.
    pub(super) fn hit_stop_enemy_strikes(&mut self, entity: usize, ticks: u8, now: SimTick) {
        self.game_entities.begin_hit_stop(entity, ticks, false);
        self.begin_player_hit_stop(ticks, true, now);
    }

    /// Advance the player's freeze by one tick. Returns true while the player
    /// is frozen this tick. Call once per tick before anything reads the
    /// animation stamps.
    pub(super) fn step_player_hit_stop(&mut self, now: SimTick) -> bool {
        if self.player_hit_stop == 0 {
            return false;
        }
        let same_action = self.anim_state.action().to_index() as u8 == self.player_hit_stop_action;
        if !same_action
            || self.anim_lock_until_tick <= now
            || self.hook_attached.is_some()
            || self.hook_travel.is_some()
            || self.hazard_death_ticks_remaining != 0
        {
            // A dodge cancel, a new hit reaction, death or the end of the lock
            // ends the freeze: nothing is owed to a clock that no longer runs.
            self.player_hit_stop = 0;
            return false;
        }
        self.player_hit_stop -= 1;
        let advance = |tick: SimTick| SimTick::from_u32(tick.as_u32().wrapping_add(1));
        self.anim_start_tick = advance(self.anim_start_tick);
        self.anim_lock_until_tick = advance(self.anim_lock_until_tick);
        if let Some((from, local, started)) = self.anim_blend_from {
            self.anim_blend_from = Some((from, local, advance(started)));
        }
        self.authored_attack.shift_source(1);
        self.authored_dodge.shift_source(1);
        true
    }

    /// Per-tick countdown of the entity freezes and of both body flashes.
    /// Runs at the end of the gameplay layer, after the entity step.
    pub(super) fn tick_hit_feel_counters(&mut self) {
        self.game_entities.tick_hit_stops();
        self.player_hit_flash = self.player_hit_flash.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> std::boxed::Box<Playtest> {
        // The production validity initializer on heap storage: the scene holds
        // non-zero Option niches and is too large for the test stack.
        let layout = std::alloc::Layout::new::<Playtest>();
        let raw = unsafe { std::alloc::alloc_zeroed(layout) as *mut Playtest };
        assert!(!raw.is_null());
        unsafe {
            Playtest::init_zeroed(raw);
            std::boxed::Box::from_raw(raw)
        }
    }

    fn attacking(scene: &mut Playtest) {
        scene.anim_state = PlayerAnim::LightAttack;
        scene.anim_start_tick = SimTick::from_u32(100);
        scene.anim_lock_until_tick = SimTick::from_u32(150);
    }

    #[test]
    fn a_frozen_player_holds_its_clock_and_resumes_exactly() {
        let mut scene = scene();
        attacking(&mut scene);
        scene.begin_player_hit_stop(4, true, SimTick::from_u32(110));
        assert_eq!(scene.player_hit_flash, hit_stop::FLASH_TICKS);
        let elapsed = |scene: &Playtest, tick: u32| tick - scene.anim_start_tick.as_u32();
        for tick in 111..=114 {
            assert!(scene.step_player_hit_stop(SimTick::from_u32(tick)));
            assert_eq!(
                elapsed(&scene, tick),
                10,
                "tick {tick} holds the connect pose"
            );
        }
        assert!(!scene.step_player_hit_stop(SimTick::from_u32(115)));
        assert_eq!(elapsed(&scene, 115), 11, "the clock resumes one tick later");
        assert_eq!(
            scene.anim_lock_until_tick.as_u32(),
            154,
            "the lock was held too"
        );
    }

    #[test]
    fn the_freeze_never_takes_an_unlocked_arched_or_dodging_player() {
        let mut scene = scene();
        scene.begin_player_hit_stop(4, false, SimTick::from_u32(110));
        assert_eq!(
            scene.player_hit_stop, 0,
            "an idle player has no clock to hold"
        );
        attacking(&mut scene);
        scene.hook_attached = Some(0);
        scene.begin_player_hit_stop(4, true, SimTick::from_u32(110));
        assert_eq!(scene.player_hit_stop, 0, "the arch owns the body");
        assert_eq!(
            scene.player_hit_flash,
            hit_stop::FLASH_TICKS,
            "it still flashes"
        );
    }

    #[test]
    fn a_new_action_or_an_expired_lock_ends_the_freeze_early() {
        let mut scene = scene();
        attacking(&mut scene);
        scene.begin_player_hit_stop(8, false, SimTick::from_u32(110));
        assert!(scene.step_player_hit_stop(SimTick::from_u32(111)));
        scene.anim_state = PlayerAnim::Roll;
        assert!(!scene.step_player_hit_stop(SimTick::from_u32(112)));
        assert_eq!(scene.player_hit_stop, 0);
        attacking(&mut scene);
        scene.begin_player_hit_stop(8, false, SimTick::from_u32(110));
        scene.anim_lock_until_tick = SimTick::from_u32(111);
        assert!(!scene.step_player_hit_stop(SimTick::from_u32(111)));
        assert_eq!(scene.player_hit_stop, 0);
    }
}
