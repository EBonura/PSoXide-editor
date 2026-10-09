//! The charged arch shot: hold fire on an arch, release to fire a stronger bolt.
//!
//! The charge exists only while the player is attached to an arch with the
//! Zenith weapon aimed. A press arms it; a release within the tap window fires
//! an ordinary shot at release (the press itself no longer fires on an arch);
//! a hold that outlasts the tap builds the charge, and releasing at full fires
//! the empowered bolt. Every constant lives in `psx_game_runtime::hook_points`.
use super::*;
use psx_game_runtime::hook_points::ChargeInput;

impl Playtest {
    /// Arch fire input for this tick. Queues the shot request exactly as the
    /// ground path does, so the ordinary start-of-shot conditions still apply.
    pub(super) fn update_arch_charge(&mut self, ctx: &Ctx, now: SimTick, action_locked: bool) {
        // Only the release tick may carry a charged shot into `begin_shot`.
        self.ranged_ready.charged_next = false;
        let step = self.hook_charge.step(ChargeInput {
            can_hold: self.hook_attached.is_some()
                && self.player_stance.active() == VitalityChannelId::Two
                && self.ranged_ready.can_fire()
                && !action_locked
                && self.motor.action().is_idle()
                && !self.ranged_ready.firing(now.as_u32())
                && self.combat_flow.can_shoot(),
            pressed: ctx.just_pressed(ACTIVE_HEAVY_ATTACK_BUTTON),
            held: ctx.is_held(ACTIVE_HEAVY_ATTACK_BUTTON),
            may_charge: self.combat_flow.can_shoot_charged(),
        });
        if step.began_charging {
            self.queue_gameplay_sfx(LevelGameplaySfxEvent::ProjectileCharge);
            telemetry::debug_log("arch charge:start");
        }
        if step.became_ready {
            self.queue_gameplay_sfx(LevelGameplaySfxEvent::StanceSwapReady);
            telemetry::debug_log("arch charge:full");
        }
        if step.charged_shot {
            self.ranged_ready.charged_next = self.combat_flow.can_shoot_charged();
        }
        if step.ordinary_shot || step.charged_shot {
            self.attack_buffer.request(5, now.as_u32());
        }
    }

    /// Tint strength (Q8) the player's body carries while the charge builds:
    /// a rising glow, then a steady pulse once the charge is full, so a full
    /// charge reads without looking at the HUD.
    pub(super) fn arch_charge_glow_q8(&self, now: SimTick) -> u16 {
        if self.hook_charge.ready() {
            return if (now.as_u32() / 6).is_multiple_of(2) {
                144
            } else {
                64
            };
        }
        (u32::from(self.hook_charge.progress_q12()) * 64 / 4096) as u16
    }
}
