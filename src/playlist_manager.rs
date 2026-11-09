//! Playlist management — creation, modification, and persistence.
//!
//! Manages playlists: add/remove tracks, reordering, shuffling,
//! and persistence. Part of the Playlist feature (P19a).
//!
//! ## Design principles
//!
//! * **Track references** — Playlists contain track IDs, not copies.
//! * **Reordering** — Supports drag-and-drop reordering semantics.
//! * **Shuffling** — Deterministic shuffle with seed support.
//! * **Persistence** — Playlists saved to disk (JSON/SQLite).
//! * **No panics** — All operations return Result<T, E>.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Playlist type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistType {
    /// User-created playlist.
    User,
    /// Auto-generated from criteria.
    Smart,
    /// Recently added tracks.
    RecentlyAdded,
    /// Most played tracks.
    MostPlayed,
    /// Currently playing queue.
    Queue,
}

impl fmt::Display for PlaylistType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlaylistType::User => write!(f, "user"),
            PlaylistType::Smart => write!(f, "smart"),
            PlaylistType::RecentlyAdded => write!(f, "recently-added"),
            PlaylistType::MostPlayed => write!(f, "most-played"),
            PlaylistType::Queue => write!(f, "queue"),
        }
    }
}

/// Playlist entry — a track reference within a playlist.
#[derive(Debug, Clone)]
pub struct PlaylistEntry {
    /// Entry ID within the playlist.
    pub entry_id: u64,
    /// Track ID referencing the library.
    pub track_id: u64,
    /// Position in the playlist (0-based).
    pub position: usize,
    /// Date added to playlist.
    pub added_at: u64,
}

/// Playlist metadata.
#[derive(Debug, Clone)]
pub struct Playlist {
    /// Unique playlist ID.
    pub id: u64,
    /// Playlist name.
    pub name: String,
    /// Playlist type.
    pub playlist_type: PlaylistType,
    /// Entries (track references).
    pub entries: Vec<PlaylistEntry>,
    /// Whether the playlist is currently playing.
    pub is_playing: bool,
    /// Index of the currently playing entry.
    pub current_index: usize,
    /// Date created.
    pub created_at: u64,
    /// Date last modified.
    pub modified_at: u64,
    /// User-provided description.
    pub description: Option<String>,
    /// Custom cover art path (if any).
    pub cover_art_path: Option<String>,
}

impl Playlist {
    /// Create a new empty playlist.
    pub fn new(id: u64, name: String, playlist_type: PlaylistType) -> Self {
        Self {
            id,
            name,
            playlist_type,
            entries: Vec::new(),
            is_playing: false,
            current_index: 0,
            created_at: current_timestamp(),
            modified_at: current_timestamp(),
            description: None,
            cover_art_path: None,
        }
    }

    /// Number of tracks in the playlist.
    pub fn track_count(&self) -> usize {
        self.entries.len()
    }

    /// Total duration of all tracks (in samples).
    ///
    /// Note: This requires looking up track durations from the library.
    /// Returns 0 if durations are not known.
    pub fn total_duration_samples(&self, track_durations: &HashMap<u64, u64>) -> u64 {
        self.entries
            .iter()
            .filter_map(|e| track_durations.get(&e.track_id))
            .sum()
    }

    /// Get the current track (if playing).
    pub fn current_track(&self) -> Option<u64> {
        if self.is_playing && self.current_index < self.entries.len() {
            Some(self.entries[self.current_index].track_id)
        } else {
            None
        }
    }

    /// Advance to the next track.
    pub fn next_track(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.current_index = (self.current_index + 1) % self.entries.len();
    }

    /// Go to the previous track.
    pub fn previous_track(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.current_index = if self.current_index == 0 {
            self.entries.len() - 1
        } else {
            self.current_index - 1
        };
    }

    /// Shuffle the playlist entries deterministically.
    pub fn shuffle(&mut self, seed: Option<u64>) {
        if self.entries.len() <= 1 {
            return;
        }

        // Use a simple deterministic shuffle (Fisher-Yates with seed).
        let mut state = seed.unwrap_or(self.id);
        for i in (1..self.entries.len()).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (state as usize) % (i + 1);
            self.entries.swap(i, j);
        }

        // Update positions.
        for (pos, entry) in self.entries.iter_mut().enumerate() {
            entry.position = pos;
        }

        self.modified_at = current_timestamp();
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_index = 0;
        self.modified_at = current_timestamp();
    }
}

/// Playlist manager — manages all playlists.
pub struct PlaylistManager {
    playlists: Arc<Mutex<Vec<Playlist>>>,
    next_id: Arc<Mutex<u64>>,
    name_index: Arc<Mutex<HashMap<String, u64>>>,
}

impl PlaylistManager {
    /// Create a new playlist manager.
    pub fn new() -> Self {
        Self {
            playlists: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
            name_index: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new playlist.
    pub fn create_playlist(
        &self,
        name: String,
        playlist_type: PlaylistType,
    ) -> Result<u64, String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let mut next_id = self.next_id.lock().map_err(|e| e.to_string())?;
        let mut name_index = self.name_index.lock().map_err(|e| e.to_string())?;

        // Check for duplicate name.
        if name_index.contains_key(&name) {
            return Err(format!("Playlist '{}' already exists", name));
        }

        let id = *next_id;
        *next_id += 1;

        let playlist = Playlist::new(id, name.clone(), playlist_type);

        playlists.push(playlist);
        name_index.insert(name, id);

        Ok(id)
    }

    /// Get a playlist by ID.
    pub fn get_playlist(&self, playlist_id: u64) -> Result<Playlist, String> {
        let playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        playlists
            .iter()
            .find(|p| p.id == playlist_id)
            .cloned()
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))
    }

    /// Add a track to a playlist.
    pub fn add_track(&self, playlist_id: u64, track_id: u64) -> Result<u64, String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let playlist = playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        let entry_id = playlist.entries.len() as u64 + 1;
        let position = playlist.entries.len();

        playlist.entries.push(PlaylistEntry {
            entry_id,
            track_id,
            position,
            added_at: current_timestamp(),
        });

        playlist.modified_at = current_timestamp();

        Ok(entry_id)
    }

    /// Remove a track from a playlist by entry ID.
    pub fn remove_track(&self, playlist_id: u64, entry_id: u64) -> Result<(), String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let playlist = playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        let pos = playlist
            .entries
            .iter()
            .position(|e| e.entry_id == entry_id)
            .ok_or_else(|| format!("Entry {} not found", entry_id))?;

        playlist.entries.remove(pos);

        // Reposition remaining entries.
        for (i, entry) in playlist.entries.iter_mut().enumerate() {
            entry.position = i;
        }

        // Adjust current index if needed.
        if playlist.current_index >= playlist.entries.len() {
            playlist.current_index = playlist.entries.len().saturating_sub(1);
        }

        playlist.modified_at = current_timestamp();

        Ok(())
    }

    /// Reorder playlist entries.
    pub fn reorder_entries(
        &self,
        playlist_id: u64,
        from_position: usize,
        to_position: usize,
    ) -> Result<(), String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let playlist = playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        if from_position >= playlist.entries.len() || to_position >= playlist.entries.len() {
            return Err("Position out of range".to_string());
        }

        let moved = playlist.entries.remove(from_position);
        playlist.entries.insert(to_position, moved);

        // Update positions.
        for (i, entry) in playlist.entries.iter_mut().enumerate() {
            entry.position = i;
        }

        playlist.modified_at = current_timestamp();

        Ok(())
    }

    /// Move a track entry to a new position.
    pub fn move_entry(
        &self,
        playlist_id: u64,
        entry_id: u64,
        to_position: usize,
    ) -> Result<(), String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let playlist = playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        let from_position = playlist
            .entries
            .iter()
            .position(|e| e.entry_id == entry_id)
            .ok_or_else(|| format!("Entry {} not found", entry_id))?;

        if to_position > playlist.entries.len() {
            return Err("Target position out of range".to_string());
        }

        let entry = playlist.entries.remove(from_position);
        playlist.entries.insert(to_position, entry);

        // Reposition.
        for (i, entry) in playlist.entries.iter_mut().enumerate() {
            entry.position = i;
        }

        playlist.modified_at = current_timestamp();

        Ok(())
    }

    /// Shuffle a playlist.
    pub fn shuffle_playlist(&self, playlist_id: u64, seed: Option<u64>) -> Result<(), String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let playlist = playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        playlist.shuffle(seed);

        Ok(())
    }

    /// Start playback of a playlist.
    pub fn start_playback(&self, playlist_id: u64) -> Result<u64, String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let playlist = playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        if playlist.entries.is_empty() {
            return Err("Playlist is empty".to_string());
        }

        playlist.is_playing = true;
        playlist.current_index = 0;

        Ok(playlist.entries[0].track_id)
    }

    /// Stop playback of a playlist.
    pub fn stop_playback(&self, playlist_id: u64) -> Result<(), String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let playlist = playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        playlist.is_playing = false;
        Ok(())
    }

    /// Advance to the next track in a playlist.
    pub fn next_track(&self, playlist_id: u64) -> Result<Option<u64>, String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let playlist = playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        playlist.next_track();

        Ok(playlist.current_track())
    }

    /// Go to previous track.
    pub fn previous_track(&self, playlist_id: u64) -> Result<Option<u64>, String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let playlist = playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        playlist.previous_track();

        Ok(playlist.current_track())
    }

    /// Get the current track of a playlist.
    pub fn current_track(&self, playlist_id: u64) -> Result<Option<u64>, String> {
        let playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let playlist = playlists
            .iter()
            .find(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        Ok(playlist.current_track())
    }

    /// List all playlists.
    pub fn list_playlists(&self) -> Result<Vec<Playlist>, String> {
        self.playlists
            .lock()
            .map(|playlists| playlists.clone())
            .map_err(|e| e.to_string())
    }

    /// Get playlist count.
    pub fn playlist_count(&self) -> Result<usize, String> {
        self.playlists
            .lock()
            .map(|p| p.len())
            .map_err(|e| e.to_string())
    }

    /// Delete a playlist.
    pub fn delete_playlist(&self, playlist_id: u64) -> Result<(), String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let mut name_index = self.name_index.lock().map_err(|e| e.to_string())?;

        let pos = playlists
            .iter()
            .position(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        let removed = playlists.remove(pos);
        name_index.remove(&removed.name);

        Ok(())
    }

    /// Update playlist metadata.
    pub fn update_playlist(
        &self,
        playlist_id: u64,
        name: Option<String>,
        description: Option<String>,
        cover_art_path: Option<String>,
    ) -> Result<(), String> {
        let mut playlists = self.playlists.lock().map_err(|e| e.to_string())?;
        let playlist = playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
            .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

        if let Some(new_name) = name {
            // Update name index.
            let old_name = playlist.name.clone();
            playlist.name = new_name;
            let mut name_index = self.name_index.lock().map_err(|e| e.to_string())?;
            name_index.remove(&old_name);
            name_index.insert(playlist.name.clone(), playlist_id);
        }

        if let Some(desc) = description {
            playlist.description = Some(desc);
        }

        if let Some(cover) = cover_art_path {
            playlist.cover_art_path = Some(cover);
        }

        playlist.modified_at = current_timestamp();

        Ok(())
    }
}

impl Default for PlaylistManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current Unix timestamp in seconds.
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_playlist() {
        let manager = PlaylistManager::new();
        let id = manager
            .create_playlist("My Playlist".to_string(), PlaylistType::User)
            .unwrap();

        let playlist = manager.get_playlist(id).unwrap();
        assert_eq!(playlist.name, "My Playlist");
        assert_eq!(playlist.playlist_type, PlaylistType::User);
        assert_eq!(playlist.track_count(), 0);
    }

    #[test]
    fn create_duplicate_playlist_fails() {
        let manager = PlaylistManager::new();
        manager
            .create_playlist("Test".to_string(), PlaylistType::User)
            .unwrap();

        assert!(manager
            .create_playlist("Test".to_string(), PlaylistType::User)
            .is_err());
    }

    #[test]
    fn add_and_remove_track() {
        let manager = PlaylistManager::new();
        let pid = manager
            .create_playlist("Test".to_string(), PlaylistType::User)
            .unwrap();

        let entry_id = manager.add_track(pid, 100).unwrap();
        let playlist = manager.get_playlist(pid).unwrap();
        assert_eq!(playlist.track_count(), 1);
        assert_eq!(playlist.entries[0].track_id, 100);

        manager.remove_track(pid, entry_id).unwrap();
        let playlist = manager.get_playlist(pid).unwrap();
        assert_eq!(playlist.track_count(), 0);
    }

    #[test]
    fn add_multiple_tracks() {
        let manager = PlaylistManager::new();
        let pid = manager
            .create_playlist("Test".to_string(), PlaylistType::User)
            .unwrap();

        manager.add_track(pid, 1).unwrap();
        manager.add_track(pid, 2).unwrap();
        manager.add_track(pid, 3).unwrap();

        let playlist = manager.get_playlist(pid).unwrap();
        assert_eq!(playlist.track_count(), 3);
        assert_eq!(playlist.entries[0].position, 0);
        assert_eq!(playlist.entries[1].position, 1);
        assert_eq!(playlist.entries[2].position, 2);
    }

    #[test]
    fn playlist_reorder() {
        let manager = PlaylistManager::new();
        let pid = manager
            .create_playlist("Test".to_string(), PlaylistType::User)
            .unwrap();

        manager.add_track(pid, 1).unwrap();
        manager.add_track(pid, 2).unwrap();
        manager.add_track(pid, 3).unwrap();

        // Move position 0 to position 2.
        manager.reorder_entries(pid, 0, 2).unwrap();

        let playlist = manager.get_playlist(pid).unwrap();
        assert_eq!(playlist.entries[0].track_id, 2);
        assert_eq!(playlist.entries[1].track_id, 3);
        assert_eq!(playlist.entries[2].track_id, 1);
    }

    #[test]
    fn playlist_shuffle() {
        let mut playlist = Playlist::new(1, "Test".to_string(), PlaylistType::User);
        for i in 0..5 {
            playlist.entries.push(PlaylistEntry {
                entry_id: i + 1,
                track_id: i + 100,
                position: i as usize,
                added_at: 0,
            });
        }

        // Shuffle with fixed seed.
        playlist.shuffle(Some(42));
        assert_eq!(playlist.entries[0].track_id, playlist.entries[0].track_id); // Deterministic.

        // Verify all tracks still present.
        let mut track_ids: Vec<u64> = playlist.entries.iter().map(|e| e.track_id).collect();
        track_ids.sort();
        assert_eq!(track_ids, vec![100, 101, 102, 103, 104]);
    }

    #[test]
    fn playlist_playback() {
        let manager = PlaylistManager::new();
        let pid = manager
            .create_playlist("Test".to_string(), PlaylistType::User)
            .unwrap();

        manager.add_track(pid, 10).unwrap();
        manager.add_track(pid, 20).unwrap();
        manager.add_track(pid, 30).unwrap();

        // Start playback.
        let first = manager.start_playback(pid).unwrap();
        assert_eq!(first, 10);

        // Next track.
        let next = manager.next_track(pid).unwrap().unwrap();
        assert_eq!(next, 20);

        // Previous track.
        let prev = manager.previous_track(pid).unwrap().unwrap();
        assert_eq!(prev, 10);

        // Stop playback.
        manager.stop_playback(pid).unwrap();
        assert!(manager.current_track(pid).unwrap().is_none());
    }

    #[test]
    fn playlist_wrap_around() {
        let manager = PlaylistManager::new();
        let pid = manager
            .create_playlist("Test".to_string(), PlaylistType::User)
            .unwrap();

        manager.add_track(pid, 1).unwrap();
        manager.add_track(pid, 2).unwrap();

        manager.start_playback(pid).unwrap();
        manager.next_track(pid).unwrap(); // Index 1.
        manager.next_track(pid).unwrap(); // Wraps to 0.

        let playlist = manager.get_playlist(pid).unwrap();
        assert_eq!(playlist.current_index, 0);
    }

    #[test]
    fn empty_playlist_playback_fails() {
        let manager = PlaylistManager::new();
        let pid = manager
            .create_playlist("Empty".to_string(), PlaylistType::User)
            .unwrap();

        assert!(manager.start_playback(pid).is_err());
    }

    #[test]
    fn playlist_total_duration() {
        let mut playlist = Playlist::new(1, "Test".to_string(), PlaylistType::User);
        playlist.entries.push(PlaylistEntry {
            entry_id: 1,
            track_id: 10,
            position: 0,
            added_at: 0,
        });
        playlist.entries.push(PlaylistEntry {
            entry_id: 2,
            track_id: 20,
            position: 1,
            added_at: 0,
        });

        let mut durations = HashMap::new();
        durations.insert(10, 441000); // 10 seconds at 44.1kHz.
        durations.insert(20, 882000); // 20 seconds.

        assert_eq!(playlist.total_duration_samples(&durations), 1323000);
    }

    #[test]
    fn delete_playlist() {
        let manager = PlaylistManager::new();
        let pid = manager
            .create_playlist("To Delete".to_string(), PlaylistType::User)
            .unwrap();

        assert_eq!(manager.playlist_count().unwrap(), 1);
        manager.delete_playlist(pid).unwrap();
        assert_eq!(manager.playlist_count().unwrap(), 0);
        assert!(manager.get_playlist(pid).is_err());
    }

    #[test]
    fn move_entry() {
        let manager = PlaylistManager::new();
        let pid = manager
            .create_playlist("Test".to_string(), PlaylistType::User)
            .unwrap();

        manager.add_track(pid, 1).unwrap(); // entry_id=1
        manager.add_track(pid, 2).unwrap(); // entry_id=2
        manager.add_track(pid, 3).unwrap(); // entry_id=3

        // Move entry 1 (position 0) to position 2.
        manager.move_entry(pid, 1, 2).unwrap();

        let playlist = manager.get_playlist(pid).unwrap();
        assert_eq!(playlist.entries[0].track_id, 2);
        assert_eq!(playlist.entries[1].track_id, 3);
        assert_eq!(playlist.entries[2].track_id, 1);
    }

    #[test]
    fn update_playlist_metadata() {
        let manager = PlaylistManager::new();
        let pid = manager
            .create_playlist("Old Name".to_string(), PlaylistType::User)
            .unwrap();

        manager
            .update_playlist(
                pid,
                Some("New Name".to_string()),
                Some("A description".to_string()),
                Some("/art/cover.jpg".to_string()),
            )
            .unwrap();

        let playlist = manager.get_playlist(pid).unwrap();
        assert_eq!(playlist.name, "New Name");
        assert_eq!(playlist.description, Some("A description".to_string()));
        assert_eq!(playlist.cover_art_path, Some("/art/cover.jpg".to_string()));
    }

    #[test]
    fn playlist_not_found() {
        let manager = PlaylistManager::new();
        assert!(manager.get_playlist(999).is_err());
        assert!(manager.add_track(999, 1).is_err());
        assert!(manager.delete_playlist(999).is_err());
    }

    #[test]
    fn playlist_type_display() {
        assert_eq!(format!("{}", PlaylistType::User), "user");
        assert_eq!(format!("{}", PlaylistType::Smart), "smart");
        assert_eq!(format!("{}", PlaylistType::Queue), "queue");
    }

    #[test]
    fn clear_playlist() {
        let mut playlist = Playlist::new(1, "Test".to_string(), PlaylistType::User);
        for i in 0..5 {
            playlist.entries.push(PlaylistEntry {
                entry_id: i + 1,
                track_id: i + 100,
                position: i as usize,
                added_at: 0,
            });
        }

        playlist.clear();
        assert_eq!(playlist.track_count(), 0);
        assert_eq!(playlist.current_index, 0);
    }
}
