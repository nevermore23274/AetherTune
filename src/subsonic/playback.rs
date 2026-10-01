use crate::audio::player::Player;

use super::client::SubsonicClient;
use super::models::Song;

/// "Previous" restarts the current track instead of going back once
/// playback is further in than this, like most music players.
const RESTART_THRESHOLD_SECS: f64 = 3.0;

/// A queue of Subsonic songs being played through the shared mpv player.
/// mpv owns the actual playlist and advances through it on its own; this
/// keeps the matching song metadata (mpv only knows URLs) and the
/// Subsonic-specific control rules.
pub struct SubsonicQueue {
    songs: Vec<Song>,
}

#[derive(Debug, PartialEq)]
enum PreviousAction {
    RestartTrack,
    PreviousTrack,
}

impl SubsonicQueue {
    /// Starts playing `songs` from index `start`, replacing whatever the
    /// player was doing (radio included). Returns None if `start` is out of
    /// range or mpv couldn't be launched.
    pub fn play(
        player: &mut Player,
        client: &SubsonicClient,
        songs: Vec<Song>,
        start: usize,
        volume: u32,
    ) -> Option<Self> {
        // Stream URLs carry a salted auth token; they're handed straight to
        // mpv and never stored anywhere else.
        let urls: Vec<String> = songs.iter().map(|song| client.stream_url(&song.id)).collect();
        if player.play_playlist(&urls, start, volume) {
            Some(Self { songs })
        } else {
            None
        }
    }

    pub fn songs(&self) -> &[Song] {
        &self.songs
    }

    /// Index of the playing song, once mpv has started it.
    pub fn current_index(&self, player: &Player) -> Option<usize> {
        player.playback.playlist_pos.filter(|i| *i < self.songs.len())
    }

    pub fn current(&self, player: &Player) -> Option<&Song> {
        self.current_index(player).map(|i| &self.songs[i])
    }

    /// Track length: mpv's once it knows, else the server's metadata.
    pub fn current_duration(&self, player: &Player) -> Option<f64> {
        player
            .playback
            .duration
            .or_else(|| self.current(player).and_then(|s| s.duration).map(f64::from))
    }

    /// Adds a song to the end of the queue without interrupting playback.
    pub fn append(&mut self, player: &mut Player, client: &SubsonicClient, song: Song) {
        player.append(&client.stream_url(&song.id));
        self.songs.push(song);
    }

    pub fn next(&self, player: &mut Player) {
        player.playlist_next();
    }

    pub fn previous(&self, player: &mut Player) {
        let action = previous_action(self.current_index(player), player.playback.time_pos);
        match action {
            PreviousAction::RestartTrack => player.seek_to(0.0),
            PreviousAction::PreviousTrack => player.playlist_prev(),
        }
    }

    /// True once the last song has finished playing.
    pub fn is_finished(&self, player: &Player) -> bool {
        player.is_playlist() && player.playback.finished()
    }
}

fn previous_action(index: Option<usize>, time_pos: Option<f64>) -> PreviousAction {
    let past_threshold = time_pos.is_some_and(|t| t > RESTART_THRESHOLD_SECS);
    // On the first track there's nothing to go back to, so restart it
    if past_threshold || index == Some(0) {
        PreviousAction::RestartTrack
    } else {
        PreviousAction::PreviousTrack
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previous_restarts_when_into_the_track() {
        assert_eq!(previous_action(Some(3), Some(42.0)), PreviousAction::RestartTrack);
    }

    #[test]
    fn previous_goes_back_near_the_start() {
        assert_eq!(previous_action(Some(3), Some(1.2)), PreviousAction::PreviousTrack);
        // Position not known yet (track just changed)
        assert_eq!(previous_action(Some(3), None), PreviousAction::PreviousTrack);
    }

    #[test]
    fn previous_on_first_track_restarts() {
        assert_eq!(previous_action(Some(0), Some(0.5)), PreviousAction::RestartTrack);
    }
}
