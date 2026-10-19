//! The drive side of XA music: one [`psx_io::cd::xa::Player`] and the few
//! calls [`crate::music`] makes on it.
//!
//! Music is a read of the song's file, so the drive cannot read data while a
//! song plays. The game never asks it to: [`crate::music::MusicPlayer`] stops
//! the song (`stop`) before a scene's data load and starts it again afterwards.
//!
//! On the host there is no drive. Under `cfg(test)` a fake records the calls
//! and answers polls from a script, so the player's timing is testable.

use psx_level::LevelMusicTrack;

/// Sectors the drive reads per second of music at single speed.
pub(crate) const SECTORS_PER_SECOND: u32 = 75;

/// Sectors into a song.
pub(crate) type SongOffset = u32;

/// What [`poll`] saw.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(target_arch = "mips", test)), allow(dead_code))]
pub(crate) enum Poll {
    /// No song is playing.
    Idle,
    /// A song is playing, or its seek has not landed yet.
    Playing,
    /// The song reached its end and the drive restarted it from the top.
    Looped,
    /// The song reached its end and the drive was paused.
    Finished,
}

#[cfg(target_arch = "mips")]
mod drive {
    use super::*;
    use core::cell::UnsafeCell;
    use psx_io::cd::xa::{DriveSpeed, Event, File, Player};

    struct State {
        player: Option<Player>,
        start_offset: SongOffset,
    }

    struct Cell(UnsafeCell<State>);
    // SAFETY: the game runs on one core and no interrupt handler touches this.
    unsafe impl Sync for Cell {}
    static STATE: Cell = Cell(UnsafeCell::new(State {
        player: None,
        start_offset: 0,
    }));

    fn state() -> &'static mut State {
        // SAFETY: single core, no re-entrancy: every caller returns before
        // another call into this module can start.
        unsafe { &mut *STATE.0.get() }
    }

    fn player() -> &'static mut Player {
        state().player.get_or_insert_with(|| {
            // SAFETY: the engine has no other owner of the controller token;
            // the data readers use the SDK's polled free functions, and music
            // is stopped before they run.
            let mut player = Player::new(unsafe { psx_io::periph::Cd::steal() });
            // The drive's own mixer has to pass the song to the SPU.
            player.set_volume(0x80, 0x80);
            player
        })
    }

    pub(crate) fn play(track: &LevelMusicTrack, offset: SongOffset, looping: bool) -> bool {
        let offset = offset.min(track.span_sectors.saturating_sub(1));
        state().start_offset = offset;
        // A song that starts part way in is a file that starts there: the
        // player ends it at the file's span, which is the song's own end.
        let file = File::new(
            track.file_lba + offset,
            track.span_sectors - offset + psx_hw::cd::xa::END_GUARD_SECTORS,
            track.file_number,
            DriveSpeed::Single,
        );
        let player = player();
        // A song already streaming is paused first; an idle player sends the
        // drive nothing.
        player.stop();
        player.play(file.song(track.channel), looping).is_ok()
    }

    pub(crate) fn poll() -> Poll {
        match player().poll() {
            Event::Idle => Poll::Idle,
            Event::Playing => Poll::Playing,
            Event::Looped => Poll::Looped,
            Event::Finished => Poll::Finished,
        }
    }

    pub(crate) fn head() -> Option<SongOffset> {
        let offset = state().start_offset;
        let player = player();
        player.is_streaming().then(|| {
            let sectors = (u64::from(player.elapsed_millis()) * u64::from(SECTORS_PER_SECOND))
                .div_ceil(1000) as u32;
            offset.saturating_add(sectors)
        })
    }

    pub(crate) fn stop() {
        if let Some(player) = state().player.as_mut() {
            player.stop();
            // A pause the drive refused during the seek of a start is retried
            // by one more poll, so a data read never meets a drive that is
            // still reading the song.
            let _ = player.poll();
        }
    }
}

#[cfg(all(not(target_arch = "mips"), not(test)))]
mod drive {
    use super::*;

    pub(crate) fn play(_: &LevelMusicTrack, _: SongOffset, _: bool) -> bool {
        true
    }
    pub(crate) fn poll() -> Poll {
        Poll::Idle
    }
    pub(crate) fn head() -> Option<SongOffset> {
        None
    }
    pub(crate) fn stop() {}
}

#[cfg(test)]
mod drive {
    extern crate std;
    use super::*;
    use core::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::vec::Vec;

    /// A drive command the fake saw.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub(crate) enum HostEvent {
        Play {
            lba: u32,
            span: u32,
            offset: u32,
            looping: bool,
        },
        Stop,
    }

    std::thread_local! {
        static EVENTS: RefCell<Vec<HostEvent>> = const { RefCell::new(Vec::new()) };
        static POLLS: RefCell<VecDeque<Poll>> = const { RefCell::new(VecDeque::new()) };
        static PLAYING: Cell<bool> = const { Cell::new(false) };
        static PLAY_FAILS: Cell<bool> = const { Cell::new(false) };
        static POSITION: Cell<Option<u32>> = const { Cell::new(None) };
        static START_OFFSET: Cell<u32> = const { Cell::new(0) };
    }

    /// What the fake recorded so far.
    pub(crate) fn host_events() -> Vec<HostEvent> {
        EVENTS.with(|events| events.borrow().clone())
    }

    /// Forget what the fake recorded.
    pub(crate) fn host_clear_events() {
        EVENTS.with(|events| events.borrow_mut().clear());
    }

    /// Answer the next polls from `polls`, then `Playing` for ever.
    pub(crate) fn host_script_polls(polls: &[Poll]) {
        POLLS.with(|queue| *queue.borrow_mut() = polls.iter().copied().collect());
    }

    /// Make every start fail, as a drive that refuses a command does.
    pub(crate) fn host_set_play_fails(fails: bool) {
        PLAY_FAILS.with(|flag| flag.set(fails));
    }

    /// Where the head is, in sectors from the start of the current play.
    pub(crate) fn host_set_position(position: Option<u32>) {
        POSITION.with(|slot| slot.set(position));
    }

    pub(crate) fn play(track: &LevelMusicTrack, offset: SongOffset, looping: bool) -> bool {
        let offset = offset.min(track.span_sectors.saturating_sub(1));
        EVENTS.with(|events| {
            events.borrow_mut().push(HostEvent::Play {
                lba: track.file_lba,
                span: track.span_sectors,
                offset,
                looping,
            });
        });
        if PLAY_FAILS.with(Cell::get) {
            return false;
        }
        START_OFFSET.with(|slot| slot.set(offset));
        PLAYING.with(|flag| flag.set(true));
        POSITION.with(|slot| slot.set(Some(0)));
        true
    }

    pub(crate) fn poll() -> Poll {
        if !PLAYING.with(Cell::get) {
            return Poll::Idle;
        }
        let poll = POLLS
            .with(|queue| queue.borrow_mut().pop_front())
            .unwrap_or(Poll::Playing);
        match poll {
            Poll::Finished => PLAYING.with(|flag| flag.set(false)),
            Poll::Looped => POSITION.with(|slot| slot.set(Some(0))),
            _ => {}
        }
        poll
    }

    pub(crate) fn head() -> Option<SongOffset> {
        let offset = START_OFFSET.with(Cell::get);
        POSITION.with(Cell::get).map(|head| offset + head)
    }

    pub(crate) fn stop() {
        if PLAYING.with(|flag| flag.replace(false)) {
            EVENTS.with(|events| events.borrow_mut().push(HostEvent::Stop));
        }
    }
}

/// Start `track` and stream it from `offset` sectors in. With `looping` the
/// drive restarts it from the song's first sector by itself when [`poll`] sees
/// the end. Blocks for the five drive commands of a start (a few milliseconds
/// on a warm drive). `false` when the drive refused a command; the caller
/// tries again later.
pub(crate) fn play(track: &LevelMusicTrack, offset: SongOffset, looping: bool) -> bool {
    drive::play(track, offset, looping)
}

/// Call now and then while a song plays: notices the end of the song and loops
/// or stops. On a console it asks the drive where the head is, one polled
/// command.
pub(crate) fn poll() -> Poll {
    drive::poll()
}

/// How far into the song the head was at the last poll, in sectors from the
/// song's own start, or `None` while the seek of a start has not landed.
pub(crate) fn head() -> Option<SongOffset> {
    drive::head()
}

/// Pause the drive so data can be read. Does nothing when no song plays.
pub(crate) fn stop() {
    drive::stop();
}

#[cfg(test)]
pub(crate) use drive::{
    host_clear_events, host_events, host_script_polls, host_set_play_fails, host_set_position,
    HostEvent,
};
