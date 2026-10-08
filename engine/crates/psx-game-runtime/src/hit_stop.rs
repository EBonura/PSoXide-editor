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
/// Ticks the struck body stays flashed, counted from the connect. Two frames
/// of the 30 Hz render cadence, so the flash is seen on every cadence.
pub const FLASH_TICKS: u8 = 4;

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

/// Flash strength, Q8 (256 = the full flash colour), for `remaining` ticks of
/// an active flash: full for the first half, half for the second.
pub const fn flash_strength_q8(remaining: u8) -> u16 {
    match remaining {
        0 => 0,
        1 | 2 => 112,
        _ => 224,
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

    #[test]
    fn the_flash_fits_inside_the_shortest_freeze_and_fades() {
        assert!(FLASH_TICKS <= LIGHT_TICKS);
        assert_eq!(flash_strength_q8(0), 0);
        assert!(flash_strength_q8(FLASH_TICKS) > flash_strength_q8(1));
    }
}
