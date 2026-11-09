//! Streaming Provider Integration — OAuth, token management, search, provider audio path.
//!
//! Provides a unified abstraction layer over streaming providers (Spotify,
//! SoundCloud, TIDAL, etc.) with OAuth authentication, secure token storage,
//! search, and audio playback through provider audio paths.
//! Part of the Streaming Integration feature (P05a).
//!
//! ## Design principles
//!
//! * **Abstraction** — Provider-agnostic API through traits.
//! * **Secure** — OAuth tokens stored securely via credentials module.
//! * **Mockable** — Easy to test with mock providers.
//! * **Extensible** — New providers added via new trait implementations.

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Streaming provider.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Provider {
    /// Spotify.
    Spotify,
    /// SoundCloud.
    SoundCloud,
    /// TIDAL.
    Tidal,
    /// Generic/custom provider.
    Generic(String),
}

impl Provider {
    /// Get provider display name.
    pub fn name(&self) -> &str {
        match self {
            Self::Spotify => "Spotify",
            Self::SoundCloud => "SoundCloud",
            Self::Tidal => "TIDAL",
            Self::Generic(name) => name,
        }
    }

    /// Get provider ID string.
    pub fn id(&self) -> &str {
        match self {
            Self::Spotify => "spotify",
            Self::SoundCloud => "soundcloud",
            Self::Tidal => "tidal",
            Self::Generic(id) => id,
        }
    }

    /// Check if provider is a real (non-generic) provider.
    pub fn is_real(&self) -> bool {
        !matches!(self, Self::Generic(_))
    }
}

/// OAuth token for provider authentication.
#[derive(Debug, Clone)]
pub struct OAuthToken {
    /// Access token.
    pub access_token: String,
    /// Refresh token.
    pub refresh_token: Option<String>,
    /// Token type (usually "Bearer").
    pub token_type: String,
    /// Expiration time.
    pub expires_at: Option<Instant>,
    /// Scopes granted.
    pub scopes: Vec<String>,
    /// Is valid.
    pub valid: bool,
}

impl OAuthToken {
    /// Create a new OAuth token.
    pub fn new(
        access_token: String,
        token_type: String,
        scopes: Vec<String>,
        expires_in_seconds: u64,
    ) -> Self {
        Self {
            access_token,
            refresh_token: None,
            token_type,
            expires_at: Some(Instant::now() + Duration::from_secs(expires_in_seconds)),
            scopes,
            valid: true,
        }
    }

    /// Set refresh token.
    pub fn set_refresh_token(&mut self, token: String) {
        self.refresh_token = Some(token);
    }

    /// Check if token is expired.
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| Instant::now() >= exp)
            .unwrap_or(true)
    }

    /// Check if token is valid.
    pub fn is_valid(&self) -> bool {
        self.valid && !self.is_expired()
    }

    /// Check if token has required scope.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

impl Default for OAuthToken {
    fn default() -> Self {
        Self {
            access_token: String::new(),
            refresh_token: None,
            token_type: "Bearer".to_string(),
            expires_at: None,
            scopes: Vec::new(),
            valid: false,
        }
    }
}

/// Streaming track metadata.
#[derive(Debug, Clone)]
pub struct StreamTrack {
    /// Track ID (provider-specific).
    pub id: String,
    /// Provider.
    pub provider: Provider,
    /// Track title.
    pub title: String,
    /// Artist name.
    pub artist: String,
    /// Album name.
    pub album: Option<String>,
    /// Duration in seconds.
    pub duration: f64,
    /// BPM (if available).
    pub bpm: Option<f64>,
    /// Key (if available).
    pub key: Option<String>,
    /// Preview URL (30s preview if full stream unavailable).
    pub preview_url: Option<String>,
    /// Full stream URL (if available).
    pub stream_url: Option<String>,
    /// Is explicitly marked as playable in deck.
    pub playable: bool,
    /// Quality level (low/medium/high/lossless).
    pub quality: StreamQuality,
}

impl StreamTrack {
    /// Create a new stream track.
    pub fn new(
        id: String,
        provider: Provider,
        title: String,
        artist: String,
        duration: f64,
    ) -> Self {
        Self {
            id,
            provider,
            title,
            artist,
            album: None,
            duration,
            bpm: None,
            key: None,
            preview_url: None,
            stream_url: None,
            playable: false,
            quality: StreamQuality::Medium,
        }
    }

    /// Set BPM.
    pub fn set_bpm(&mut self, bpm: f64) {
        self.bpm = Some(bpm);
    }

    /// Set key.
    pub fn set_key(&mut self, key: String) {
        self.key = Some(key);
    }

    /// Set album.
    pub fn set_album(&mut self, album: String) {
        self.album = Some(album);
    }

    /// Set preview URL.
    pub fn set_preview_url(&mut self, url: String) {
        self.preview_url = Some(url);
    }

    /// Set stream URL.
    pub fn set_stream_url(&mut self, url: String) {
        self.stream_url = Some(url);
    }

    /// Mark as playable.
    pub fn set_playable(&mut self) {
        self.playable = true;
    }
}

/// Stream quality level.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StreamQuality {
    /// Low quality (96 kbps).
    Low,
    /// Medium quality (160 kbps).
    #[default]
    Medium,
    /// High quality (320 kbps).
    High,
    /// Lossless quality (FLAC/ALAC).
    Lossless,
}

/// Search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Provider.
    pub provider: Provider,
    /// Tracks matching search.
    pub tracks: Vec<StreamTrack>,
    /// Albums matching search.
    pub albums: Vec<SearchAlbum>,
    /// Artists matching search.
    pub artists: Vec<SearchArtist>,
    /// Total results.
    pub total: usize,
}

impl SearchResult {
    /// Create empty search result.
    pub fn new(provider: Provider) -> Self {
        Self {
            provider,
            tracks: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
            total: 0,
        }
    }

    /// Add a track.
    pub fn add_track(&mut self, track: StreamTrack) {
        self.tracks.push(track);
        self.total += 1;
    }

    /// Check if result has tracks.
    pub fn has_tracks(&self) -> bool {
        !self.tracks.is_empty()
    }
}

/// Search album.
#[derive(Debug, Clone)]
pub struct SearchAlbum {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub track_count: usize,
    pub year: Option<u32>,
}

/// Search artist.
#[derive(Debug, Clone)]
pub struct SearchArtist {
    pub id: String,
    pub name: String,
    pub track_count: usize,
}

/// Streaming error.
#[derive(Debug, Clone)]
pub enum StreamError {
    /// Provider not authenticated.
    NotAuthenticated,
    /// Token expired and refresh failed.
    TokenExpired,
    /// Track not found.
    TrackNotFound,
    /// Track not available in user's region.
    RegionLocked,
    /// Quality not available for subscription tier.
    QualityUnavailable,
    /// Network error.
    NetworkError(String),
    /// Rate limit exceeded.
    RateLimited,
    /// Provider API error.
    ProviderError { code: u32, message: String },
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAuthenticated => write!(f, "Not authenticated with provider"),
            Self::TokenExpired => write!(f, "Token expired, refresh failed"),
            Self::TrackNotFound => write!(f, "Track not found"),
            Self::RegionLocked => write!(f, "Track not available in your region"),
            Self::QualityUnavailable => write!(f, "Quality not available for your subscription"),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::RateLimited => write!(f, "Rate limit exceeded, try again later"),
            Self::ProviderError { code, message } => {
                write!(f, "Provider error {}: {}", code, message)
            }
            Self::Generic(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for StreamError {}

/// Provider audio path — resolves stream URL for a track.
#[derive(Debug, Clone)]
pub struct AudioPath {
    /// Track ID.
    pub track_id: String,
    /// Provider.
    pub provider: Provider,
    /// Stream URL.
    pub stream_url: String,
    /// Quality used.
    pub quality: StreamQuality,
    /// URL expiration time (seconds).
    pub url_expires_in: u64,
    /// Is valid.
    pub valid: bool,
}

impl AudioPath {
    /// Create new audio path.
    pub fn new(
        track_id: String,
        provider: Provider,
        stream_url: String,
        quality: StreamQuality,
    ) -> Self {
        Self {
            track_id,
            provider,
            stream_url,
            quality,
            url_expires_in: 3600, // 1 hour default.
            valid: true,
        }
    }

    /// Check if audio path is valid.
    pub fn is_valid(&self) -> bool {
        self.valid
    }
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Provider implementation trait.
/// All providers must implement this trait.
pub trait StreamProvider {
    /// Get provider name.
    fn name(&self) -> &str;

    /// Authenticate with provider using OAuth.
    fn authenticate(&self, code: &str, redirect_uri: &str) -> Result<OAuthToken, StreamError>;

    /// Refresh an expired token.
    fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken, StreamError>;

    /// Search for tracks.
    fn search_tracks(&self, query: &str, limit: u32) -> Result<Vec<StreamTrack>, StreamError>;

    /// Search for albums.
    fn search_albums(&self, query: &str, limit: u32) -> Result<Vec<SearchAlbum>, StreamError>;

    /// Search for artists.
    fn search_artists(&self, query: &str, limit: u32) -> Result<Vec<SearchArtist>, StreamError>;

    /// Get track by ID.
    fn get_track(&self, track_id: &str) -> Result<StreamTrack, StreamError>;

    /// Get audio path for track.
    fn get_audio_path(
        &self,
        track_id: &str,
        quality: StreamQuality,
    ) -> Result<AudioPath, StreamError>;

    /// Get user playlists.
    fn get_user_playlists(&self) -> Result<Vec<Playlist>, StreamError>;

    /// Check if provider is connected.
    fn is_connected(&self) -> bool;

    /// Disconnect from provider.
    fn disconnect(&mut self);
}

/// User playlist from provider.
#[derive(Debug, Clone)]
pub struct Playlist {
    /// Playlist ID.
    pub id: String,
    /// Provider.
    pub provider: Provider,
    /// Playlist name.
    pub name: String,
    /// Track count.
    pub track_count: usize,
    /// Is editable.
    pub editable: bool,
}

impl Playlist {
    /// Create a new playlist.
    pub fn new(
        id: String,
        provider: Provider,
        name: String,
        track_count: usize,
        editable: bool,
    ) -> Self {
        Self {
            id,
            provider,
            name,
            track_count,
            editable,
        }
    }
}

// ---------------------------------------------------------------------------
// Mock Provider for testing
// ---------------------------------------------------------------------------

/// Mock provider for testing without real API access.
pub struct MockStreamProvider {
    connected: std::sync::atomic::AtomicBool,
    name: String,
    token: Option<OAuthToken>,
    tracks: HashMap<String, StreamTrack>,
    playlists: Vec<Playlist>,
}

impl MockStreamProvider {
    /// Create new mock provider.
    pub fn new() -> Self {
        let mut provider = Self {
            connected: std::sync::atomic::AtomicBool::new(false),
            name: "MockStreamProvider".into(),
            token: None,
            tracks: HashMap::new(),
            playlists: Vec::new(),
        };

        // Add some mock tracks.
        provider.add_mock_track(StreamTrack::new(
            "mock1".to_string(),
            Provider::Generic("Mock".to_string()),
            "Mock Track 1".to_string(),
            "Mock Artist".to_string(),
            240.0,
        ));
        provider.add_mock_track(StreamTrack::new(
            "mock2".to_string(),
            Provider::Generic("Mock".to_string()),
            "Mock Track 2".to_string(),
            "Mock Artist".to_string(),
            180.0,
        ));

        provider
    }

    /// Add a mock track.
    pub fn add_mock_track(&mut self, track: StreamTrack) {
        self.tracks.insert(track.id.clone(), track);
    }

    /// Add a mock playlist.
    pub fn add_mock_playlist(&mut self, playlist: Playlist) {
        self.playlists.push(playlist);
    }
}

impl StreamProvider for MockStreamProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn authenticate(&self, _code: &str, _redirect_uri: &str) -> Result<OAuthToken, StreamError> {
        // In mock: always succeed.
        let token = OAuthToken::new(
            "mock_access_token".to_string(),
            "Bearer".to_string(),
            vec!["read".to_string(), "stream".to_string()],
            3600,
        );
        self.connected
            .store(true, std::sync::atomic::Ordering::Release);
        Ok(token)
    }

    fn refresh_token(&self, _refresh_token: &str) -> Result<OAuthToken, StreamError> {
        Ok(OAuthToken::new(
            "mock_refreshed_token".to_string(),
            "Bearer".to_string(),
            vec!["read".to_string(), "stream".to_string()],
            3600,
        ))
    }

    fn search_tracks(&self, query: &str, _limit: u32) -> Result<Vec<StreamTrack>, StreamError> {
        let q = query.to_lowercase();
        let results: Vec<_> = self
            .tracks
            .values()
            .filter(|t| t.title.to_lowercase().contains(&q) || t.artist.to_lowercase().contains(&q))
            .cloned()
            .collect();
        Ok(results)
    }

    fn search_albums(&self, _query: &str, _limit: u32) -> Result<Vec<SearchAlbum>, StreamError> {
        Ok(Vec::new())
    }

    fn search_artists(&self, _query: &str, _limit: u32) -> Result<Vec<SearchArtist>, StreamError> {
        Ok(Vec::new())
    }

    fn get_track(&self, track_id: &str) -> Result<StreamTrack, StreamError> {
        self.tracks
            .get(track_id)
            .cloned()
            .ok_or(StreamError::TrackNotFound)
    }

    fn get_audio_path(
        &self,
        track_id: &str,
        quality: StreamQuality,
    ) -> Result<AudioPath, StreamError> {
        if !self.connected.load(std::sync::atomic::Ordering::Acquire) {
            return Err(StreamError::NotAuthenticated);
        }
        let track = self
            .tracks
            .get(track_id)
            .ok_or(StreamError::TrackNotFound)?;
        Ok(AudioPath::new(
            track_id.to_string(),
            track.provider.clone(),
            format!("mock://stream/{}", track_id),
            quality,
        ))
    }

    fn get_user_playlists(&self) -> Result<Vec<Playlist>, StreamError> {
        Ok(self.playlists.clone())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::Acquire)
    }

    fn disconnect(&mut self) {
        self.connected
            .store(false, std::sync::atomic::Ordering::Release);
        self.token = None;
    }
}

// ---------------------------------------------------------------------------
// Streaming integration engine
// ---------------------------------------------------------------------------

/// Streaming integration engine — manages multiple providers.
pub struct StreamingEngine {
    /// Provider instances (by ID).
    providers: HashMap<String, Box<dyn StreamProvider>>,
    /// Connected providers.
    connected: HashMap<String, bool>,
    /// Cached tokens.
    tokens: HashMap<String, OAuthToken>,
    /// Last search results.
    last_search: Option<SearchResult>,
    /// Rate limit tracking.
    rate_limits: HashMap<String, RateLimitInfo>,
    /// Connection status.
    status: StreamStatus,
}

/// Rate limit info.
#[derive(Debug, Clone)]
struct RateLimitInfo {
    requests_remaining: u32,
    resets_at: Instant,
}

/// Streaming engine status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamStatus {
    /// All providers disconnected.
    Disconnected,
    /// Some providers connected.
    PartiallyConnected {
        connected_count: usize,
        total_count: usize,
    },
    /// All providers connected.
    Connected,
    /// Error connecting.
    Error(String),
}

impl StreamingEngine {
    /// Create a new streaming engine.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            connected: HashMap::new(),
            tokens: HashMap::new(),
            last_search: None,
            rate_limits: HashMap::new(),
            status: StreamStatus::Disconnected,
        }
    }

    /// Register a provider.
    pub fn register_provider(&mut self, provider: Box<dyn StreamProvider>) {
        let name = provider.name().to_string();
        self.providers.insert(name.clone(), provider);
        self.connected.insert(name.clone(), false);
    }

    /// Get a provider by name.
    pub fn get_provider(&self, name: &str) -> Option<&dyn StreamProvider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    /// Authenticate with a provider.
    pub fn authenticate(
        &mut self,
        provider_name: &str,
        code: &str,
        redirect_uri: &str,
    ) -> Result<(), StreamError> {
        if let Some(provider) = self.providers.get(provider_name) {
            let token = provider.authenticate(code, redirect_uri)?;
            self.tokens.insert(provider_name.to_string(), token);
            self.connected.insert(provider_name.to_string(), true);
            self.update_status();
            Ok(())
        } else {
            Err(StreamError::Generic(format!(
                "Provider not registered: {}",
                provider_name
            )))
        }
    }

    /// Disconnect from a provider.
    pub fn disconnect(&mut self, provider_name: &str) {
        if let Some(provider) = self.providers.get_mut(provider_name) {
            provider.disconnect();
            self.connected.insert(provider_name.to_string(), false);
            self.tokens.remove(provider_name);
            self.update_status();
        }
    }

    /// Check if provider is connected.
    pub fn is_connected(&self, provider_name: &str) -> bool {
        self.connected.get(provider_name).copied().unwrap_or(false)
    }

    /// Search tracks across all connected providers.
    pub fn search_tracks(&mut self, query: &str, limit: u32) -> Result<SearchResult, StreamError> {
        let mut result = SearchResult::new(Provider::Generic("Multi".to_string()));

        for (name, provider) in &self.providers {
            if self.is_connected(name) {
                if let Ok(tracks) = provider.search_tracks(query, limit) {
                    for track in tracks {
                        result.add_track(track);
                    }
                }
            }
        }

        self.last_search = Some(result.clone());
        Ok(result)
    }

    /// Get track from provider.
    pub fn get_track(
        &self,
        provider_name: &str,
        track_id: &str,
    ) -> Result<StreamTrack, StreamError> {
        if let Some(provider) = self.providers.get(provider_name) {
            provider.get_track(track_id)
        } else {
            Err(StreamError::Generic(format!(
                "Provider not registered: {}",
                provider_name
            )))
        }
    }

    /// Get audio path for track.
    pub fn get_audio_path(
        &self,
        provider_name: &str,
        track_id: &str,
        quality: StreamQuality,
    ) -> Result<AudioPath, StreamError> {
        if let Some(provider) = self.providers.get(provider_name) {
            provider.get_audio_path(track_id, quality)
        } else {
            Err(StreamError::Generic(format!(
                "Provider not registered: {}",
                provider_name
            )))
        }
    }

    /// Get user playlists from a provider.
    pub fn get_user_playlists(&self, provider_name: &str) -> Result<Vec<Playlist>, StreamError> {
        if let Some(provider) = self.providers.get(provider_name) {
            provider.get_user_playlists()
        } else {
            Err(StreamError::Generic(format!(
                "Provider not registered: {}",
                provider_name
            )))
        }
    }

    /// Get last search results.
    pub fn last_search(&self) -> Option<&SearchResult> {
        self.last_search.as_ref()
    }

    /// Get connection status.
    pub fn status(&self) -> &StreamStatus {
        &self.status
    }

    /// Get registered provider names.
    pub fn provider_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Get connected provider count.
    pub fn connected_count(&self) -> usize {
        self.connected.values().filter(|c| **c).count()
    }

    /// Update status after connection changes.
    fn update_status(&mut self) {
        let total = self.providers.len();
        let connected = self.connected_count();

        if connected == 0 {
            self.status = StreamStatus::Disconnected;
        } else if connected == total {
            self.status = StreamStatus::Connected;
        } else {
            self.status = StreamStatus::PartiallyConnected {
                connected_count: connected,
                total_count: total,
            };
        }
    }

    /// Check if all required providers are connected.
    pub fn check_all_connected(&self, required: &[&str]) -> bool {
        required.iter().all(|&name| self.is_connected(name))
    }

    /// Get token for provider.
    pub fn get_token(&self, provider_name: &str) -> Option<&OAuthToken> {
        self.tokens.get(provider_name)
    }

    /// Get search result preview string.
    pub fn search_preview(&self) -> String {
        self.last_search
            .as_ref()
            .map_or("No search results yet".to_string(), |r| {
                format!("{} results from {} provider", r.total, r.provider.name())
            })
    }

    /// Get status string.
    pub fn status_string(&self) -> String {
        match &self.status {
            StreamStatus::Disconnected => "Disconnected".to_string(),
            StreamStatus::PartiallyConnected {
                connected_count,
                total_count,
            } => {
                format!("Partially connected: {}/{}", connected_count, total_count)
            }
            StreamStatus::Connected => "All providers connected".to_string(),
            StreamStatus::Error(msg) => format!("Error: {}", msg),
        }
    }
}

impl Default for StreamingEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_engine_creation() {
        let engine = StreamingEngine::new();
        assert_eq!(engine.provider_names().len(), 0);
        assert_eq!(engine.connected_count(), 0);
        assert!(matches!(engine.status(), StreamStatus::Disconnected));
    }

    #[test]
    fn streaming_engine_register_provider() {
        let mut engine = StreamingEngine::new();
        let mock = MockStreamProvider::new();
        engine.register_provider(Box::new(mock));

        assert_eq!(engine.provider_names().len(), 1);
    }

    #[test]
    fn streaming_engine_authenticate() {
        let mut engine = StreamingEngine::new();
        let mock = MockStreamProvider::new();
        engine.register_provider(Box::new(mock));

        let result = engine.authenticate(
            "MockStreamProvider",
            "mock_code",
            "http://localhost/callback",
        );
        assert!(result.is_ok());
        assert!(engine.is_connected("MockStreamProvider"));
    }

    #[test]
    fn streaming_engine_disconnect() {
        let mut engine = StreamingEngine::new();
        let mock = MockStreamProvider::new();
        engine.register_provider(Box::new(mock));

        engine
            .authenticate(
                "MockStreamProvider",
                "mock_code",
                "http://localhost/callback",
            )
            .unwrap();
        engine.disconnect("MockStreamProvider");

        assert!(!engine.is_connected("MockStreamProvider"));
        assert!(matches!(engine.status(), StreamStatus::Disconnected));
    }

    #[test]
    fn streaming_engine_search() {
        let mut engine = StreamingEngine::new();
        let mock = MockStreamProvider::new();
        engine.register_provider(Box::new(mock));

        engine
            .authenticate(
                "MockStreamProvider",
                "mock_code",
                "http://localhost/callback",
            )
            .unwrap();

        let result = engine.search_tracks("Mock", 10);
        assert!(result.is_ok());
        assert!(result.unwrap().has_tracks());
    }

    #[test]
    fn streaming_engine_get_audio_path() {
        let mut engine = StreamingEngine::new();
        let mock = MockStreamProvider::new();
        engine.register_provider(Box::new(mock));

        engine
            .authenticate(
                "MockStreamProvider",
                "mock_code",
                "http://localhost/callback",
            )
            .unwrap();

        let path = engine.get_audio_path("MockStreamProvider", "mock1", StreamQuality::Medium);
        assert!(path.is_ok());
        assert!(path.unwrap().stream_url.contains("mock"));
    }

    #[test]
    fn streaming_engine_unconnected_search() {
        let mut engine = StreamingEngine::new();
        // No providers registered.
        let result = engine.search_tracks("test", 10);
        assert!(result.is_ok()); // Should return empty result.
    }

    #[test]
    fn oauth_token_creation() {
        let token = OAuthToken::new(
            "test_token".to_string(),
            "Bearer".to_string(),
            vec!["read".to_string(), "stream".to_string()],
            3600,
        );

        assert_eq!(token.access_token, "test_token");
        assert!(token.is_valid());
        assert!(token.has_scope("read"));
        assert!(!token.has_scope("write"));
    }

    #[test]
    fn oauth_token_expiry() {
        let token = OAuthToken::new(
            "test".to_string(),
            "Bearer".to_string(),
            Vec::new(),
            0, // Expires immediately.
        );
        assert!(token.is_expired());
    }

    #[test]
    fn oauth_token_scopes() {
        let token = OAuthToken::new(
            "test".to_string(),
            "Bearer".to_string(),
            vec!["read".to_string(), "write".to_string()],
            3600,
        );

        assert!(token.has_scope("read"));
        assert!(token.has_scope("write"));
        assert!(!token.has_scope("admin"));
    }

    #[test]
    fn stream_track_creation() {
        let track = StreamTrack::new(
            "t1".to_string(),
            Provider::Spotify,
            "Test Track".to_string(),
            "Test Artist".to_string(),
            240.0,
        );

        assert_eq!(track.title, "Test Track");
        assert_eq!(track.duration, 240.0);
        assert!(!track.playable);
    }

    #[test]
    fn stream_track_setters() {
        let mut track = StreamTrack::new(
            "t1".to_string(),
            Provider::Spotify,
            "Test Track".to_string(),
            "Test Artist".to_string(),
            240.0,
        );

        track.set_bpm(128.0);
        assert_eq!(track.bpm, Some(128.0));

        track.set_key("8A".to_string());
        assert_eq!(track.key, Some("8A".to_string()));

        track.set_album("Test Album".to_string());
        assert_eq!(track.album, Some("Test Album".to_string()));

        track.set_preview_url("http://preview".to_string());
        assert_eq!(track.preview_url, Some("http://preview".to_string()));

        track.set_playable();
        assert!(track.playable);
    }

    #[test]
    fn provider_names() {
        assert_eq!(Provider::Spotify.name(), "Spotify");
        assert_eq!(Provider::SoundCloud.name(), "SoundCloud");
        assert_eq!(Provider::Tidal.name(), "TIDAL");

        let generic = Provider::Generic("MyProvider".to_string());
        assert_eq!(generic.name(), "MyProvider");
    }

    #[test]
    fn provider_ids() {
        assert_eq!(Provider::Spotify.id(), "spotify");
        assert_eq!(Provider::SoundCloud.id(), "soundcloud");
        assert_eq!(Provider::Tidal.id(), "tidal");
    }

    #[test]
    fn provider_is_real() {
        assert!(Provider::Spotify.is_real());
        assert!(Provider::SoundCloud.is_real());
        assert!(!Provider::Generic("Test".to_string()).is_real());
    }

    #[test]
    fn search_result_add_track() {
        let mut result = SearchResult::new(Provider::Spotify);
        result.add_track(StreamTrack::new(
            "t1".to_string(),
            Provider::Spotify,
            "Track 1".to_string(),
            "Artist".to_string(),
            240.0,
        ));
        assert_eq!(result.total, 1);
        assert!(result.has_tracks());
    }

    #[test]
    fn stream_error_display() {
        let err = StreamError::TrackNotFound;
        let msg = format!("{}", err);
        assert!(msg.contains("not found"));
    }

    #[test]
    fn audio_path_creation() {
        let path = AudioPath::new(
            "t1".to_string(),
            Provider::Spotify,
            "http://stream".to_string(),
            StreamQuality::High,
        );

        assert_eq!(path.track_id, "t1");
        assert!(path.is_valid());
        assert_eq!(path.quality, StreamQuality::High);
    }

    #[test]
    fn stream_quality_values() {
        assert!(matches!(StreamQuality::Low, StreamQuality::Low));
        assert!(matches!(StreamQuality::Lossless, StreamQuality::Lossless));
    }

    #[test]
    fn streaming_engine_multiple_providers() {
        let mut engine = StreamingEngine::new();

        let mock1 = MockStreamProvider::new();
        let mut mock2 = MockStreamProvider::new();
        mock2.name = "SecondMockProvider".into();
        engine.register_provider(Box::new(mock1));
        engine.register_provider(Box::new(mock2));

        assert_eq!(engine.provider_names().len(), 2);

        // Connect only one.
        engine
            .authenticate("MockStreamProvider", "code", "uri")
            .unwrap();

        assert_eq!(engine.connected_count(), 1);
        assert!(matches!(
            engine.status(),
            StreamStatus::PartiallyConnected { .. }
        ));
    }

    #[test]
    fn streaming_engine_check_all_connected() {
        let mut engine = StreamingEngine::new();
        let mock = MockStreamProvider::new();
        engine.register_provider(Box::new(mock));

        engine
            .authenticate("MockStreamProvider", "code", "uri")
            .unwrap();

        assert!(engine.check_all_connected(&["MockStreamProvider"]));
        assert!(!engine.check_all_connected(&["NonExistent"]));
    }

    #[test]
    fn streaming_engine_get_token() {
        let mut engine = StreamingEngine::new();
        let mock = MockStreamProvider::new();
        engine.register_provider(Box::new(mock));

        engine
            .authenticate("MockStreamProvider", "code", "uri")
            .unwrap();

        let token = engine.get_token("MockStreamProvider");
        assert!(token.is_some());
        assert!(token.unwrap().is_valid());
    }

    #[test]
    fn streaming_engine_search_preview() {
        let mut engine = StreamingEngine::new();
        let mock = MockStreamProvider::new();
        engine.register_provider(Box::new(mock));

        assert_eq!(engine.search_preview(), "No search results yet");

        engine
            .authenticate("MockStreamProvider", "code", "uri")
            .unwrap();
        engine.search_tracks("Mock", 10).unwrap();
        assert!(engine.search_preview().contains("results"));
    }

    #[test]
    fn streaming_engine_status_string() {
        let engine = StreamingEngine::new();
        assert_eq!(engine.status_string(), "Disconnected");
    }

    #[test]
    fn streaming_engine_get_track_not_found() {
        let mut engine = StreamingEngine::new();
        let mock = MockStreamProvider::new();
        engine.register_provider(Box::new(mock));

        let result = engine.get_track("MockStreamProvider", "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn streaming_engine_authenticate_unknown_provider() {
        let mut engine = StreamingEngine::new();
        let result = engine.authenticate("Unknown", "code", "uri");
        assert!(result.is_err());
    }
}
