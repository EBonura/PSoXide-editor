//! Hit-stop: a short freeze of the two actors involved in a melee connect.
//!
//! Counts are 60 Hz simulation ticks and cover only the attacker and the
//! struck actor; the world, projectiles and renderer keep running. Every value
//! is a starting point from general action-game practice (a few frames for a
//! light hit, roughly twice that for a heavy one, a little more when poise
//! breaks, the most on a kill). The Bloodborne notes carry no hit-stop data.
//!
//! The counts are even because the NPC tick runs at 30 Hz and consumes two
//! simulation ticks per step, so an even count freezes an enemy for exactly as
//! long as the player.

/// A light hit that does not break poise.
pub const LIGHT_TICKS: u8 = 4;
/// A light hit that breaks poise.
pub const LIGHT_BREAK_TICKS: u8 = 6;
/// A heavy hit that does not break poise.
pub const HEAVY_TICKS: u8 = 8;
/// A heavy hit that breaks poise.
pub const HEAVY_BREAK_TICKS: u8 = 10;
/// The blow that defeats an actor.
pub const KILL_TICKS: u8 = 12;

/// Freeze length for one melee connect.
pub const fn ticks_for(heavy: bool, poise_broken: bool, killed: bool) -> u8 {
    if killed {
        KILL_TICKS
    } else if heavy {
        if poise_broken {
            HEAVY_BREAK_TICKS
        } else {
            HEAVY_TICKS
        }
    } else if poise_broken {
        LIGHT_BREAK_TICKS
    } else {
        LIGHT_TICKS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heavier_outcomes_freeze_longer_and_all_counts_are_even() {
        assert!(ticks_for(false, false, false) < ticks_for(false, true, false));
        assert!(ticks_for(false, true, false) < ticks_for(true, false, false));
        assert!(ticks_for(true, false, false) < ticks_for(true, true, false));
        assert!(ticks_for(true, true, false) < ticks_for(false, false, true));
        for (heavy, broke, killed) in [
            (false, false, false),
            (false, true, false),
            (true, false, false),
            (true, true, false),
            (false, false, true),
            (true, true, true),
        ] {
            assert_eq!(ticks_for(heavy, broke, killed) % 2, 0);
        }
    }
}
