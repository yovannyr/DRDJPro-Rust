//! Live Streaming Part 2 — Chat, Overlays, Pause, Historie, Reconnect.
//!
//! Extends the live streaming system (P15a) with chat messaging, stream overlays,
//! pause/resume, session history, and automatic reconnection.
//! Part of the Live Streaming feature (P15b).
//!
//! ## Design principles
//!
//! * **Safe** — Chat messages are validated.
//! * **Responsive** — Reconnection is automatic.
//! * **Flexible** — Overlays are customizable.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Streaming platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamPlatform {
    /// Twitch.
    Twitch,
    /// YouTube.
    YouTube,
    /// Mixcloud.
    Mixcloud,
    /// SoundCloud.
    SoundCloud,
    /// Custom RTMP server.
    CustomRTMP,
}

impl StreamPlatform {
    /// Get platform name.
    pub fn name(&self) -> &str {
        match self {
            Self::Twitch => "Twitch",
            Self::YouTube => "YouTube",
            Self::Mixcloud => "Mixcloud",
            Self::SoundCloud => "SoundCloud",
            Self::CustomRTMP => "Custom RTMP",
        }
    }

    /// Get default port.
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Twitch => 443,
            Self::YouTube => 443,
            Self::Mixcloud => 443,
            Self::SoundCloud => 443,
            Self::CustomRTMP => 1935,
        }
    }
}

impl std::fmt::Display for StreamPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Stream overlay type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayType {
    /// Now Playing display.
    NowPlaying,
    /// Logo overlay.
    Logo,
    /// Social Media Links.
    SocialLinks,
    /// Viewer count.
    ViewerCount,
    /// Chat messages.
    ChatMessages,
    /// Custom overlay.
    Custom(String),
}

impl OverlayType {
    /// Get overlay type name.
    pub fn name(&self) -> &str {
        match self {
            Self::NowPlaying => "Now Playing",
            Self::Logo => "Logo",
            Self::SocialLinks => "Social Links",
            Self::ViewerCount => "Viewer Count",
            Self::ChatMessages => "Chat Messages",
            Self::Custom(name) => name,
        }
    }
}

/// Stream overlay configuration.
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Overlay type.
    pub overlay_type: OverlayType,
    /// X position in pixels (0 = left edge).
    pub x: u32,
    /// Y position in pixels (0 = top edge).
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Opacity (0.0 to 1.0).
    pub opacity: f32,
    /// Text color (hex).
    pub text_color: String,
    /// Background color (hex).
    pub background_color: String,
    /// Font size.
    pub font_size: u32,
    /// Is visible.
    pub is_visible: bool,
    /// Custom text.
    pub custom_text: String,
    /// Logo URL/path.
    pub logo_url: String,
}

impl OverlayConfig {
    /// Create new overlay config.
    pub fn new(overlay_type: OverlayType, x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            overlay_type,
            x,
            y,
            width,
            height,
            opacity: 1.0,
            text_color: "#FFFFFF".to_string(),
            background_color: "#000000".to_string(),
            font_size: 14,
            is_visible: true,
            custom_text: String::new(),
            logo_url: String::new(),
        }
    }

    /// Set opacity.
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    /// Set text color.
    pub fn set_text_color(&mut self, color: &str) {
        self.text_color = color.to_string();
    }

    /// Set background color.
    pub fn set_background_color(&mut self, color: &str) {
        self.background_color = color.to_string();
    }

    /// Set position.
    pub fn set_position(&mut self, x: u32, y: u32) {
        self.x = x;
        self.y = y;
    }

    /// Set size.
    pub fn set_size(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Toggle visibility.
    pub fn toggle_visibility(&mut self) {
        self.is_visible = !self.is_visible;
    }

    /// Get geometry string.
    pub fn geometry_string(&self) -> String {
        format!(
            "x={} y={} w={} h={} opacity={:.1} visible={}",
            self.x, self.y, self.width, self.height, self.opacity, self.is_visible
        )
    }

    /// Is valid?
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0 && self.opacity >= 0.0 && self.opacity <= 1.0
    }
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self::new(OverlayType::NowPlaying, 10, 10, 300, 50)
    }
}

/// Chat message.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Unique message ID.
    pub id: String,
    /// Username.
    pub username: String,
    /// Message content.
    pub content: String,
    /// Message timestamp.
    pub timestamp: Instant,
    /// Is from broadcaster.
    pub is_broadcaster: bool,
    /// Is from viewer.
    pub is_viewer: bool,
    /// Message color.
    pub color: String,
}

impl ChatMessage {
    /// Create new chat message.
    pub fn new(id: &str, username: &str, content: &str, is_broadcaster: bool) -> Self {
        Self {
            id: id.to_string(),
            username: username.to_string(),
            content: content.to_string(),
            timestamp: Instant::now(),
            is_broadcaster,
            is_viewer: !is_broadcaster,
            color: if is_broadcaster {
                "#FF0000".to_string()
            } else {
                "#00FF00".to_string()
            },
        }
    }

    /// Is valid?
    pub fn is_valid(&self) -> bool {
        !self.username.is_empty() && !self.content.is_empty() && self.content.len() <= 500
    }

    /// Get content preview.
    pub fn preview(&self, max_len: usize) -> String {
        if self.content.len() <= max_len {
            self.content.clone()
        } else {
            format!("{}...", &self.content[..max_len])
        }
    }
}

/// Chat channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatChannel {
    /// Main chat channel.
    Main,
    /// Whisper/direct message.
    Whisper(String),
}

impl ChatChannel {
    /// Get channel name.
    pub fn name(&self) -> &str {
        match self {
            Self::Main => "main",
            Self::Whisper(target) => target,
        }
    }
}

/// Stream state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StreamState {
    /// Not streaming.
    #[default]
    Idle,
    /// Connecting to platform.
    Connecting,
    /// Streaming.
    Streaming,
    /// Paused.
    Paused,
    /// Reconnecting.
    Reconnecting,
    /// Error.
    Error(String),
}

impl StreamState {
    /// Is active (streaming or paused)?
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Streaming | Self::Paused)
    }

    /// Is streaming (not paused)?
    pub fn is_streaming(&self) -> bool {
        matches!(self, Self::Streaming)
    }

    /// Is error?
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

/// Stream session metadata.
#[derive(Debug, Clone)]
pub struct StreamSession {
    /// Unique session ID.
    pub id: String,
    /// Platform.
    pub platform: StreamPlatform,
    /// Stream title.
    pub title: String,
    /// Stream description.
    pub description: String,
    /// Stream category/game.
    pub category: String,
    /// Stream tags.
    pub tags: Vec<String>,
    /// Start time.
    pub started_at: Instant,
    /// End time.
    pub ended_at: Option<Instant>,
    /// Peak viewers.
    pub peak_viewers: u32,
    /// Average bitrate (kbps).
    pub average_bitrate: u32,
    /// Dropped frames count.
    pub dropped_frames: u32,
    /// Total bytes sent.
    pub total_bytes_sent: u64,
    /// Chat messages sent.
    pub chat_messages_sent: u32,
    /// Is archived.
    pub is_archived: bool,
}

impl StreamSession {
    /// Create new stream session.
    pub fn new(id: &str, platform: StreamPlatform, title: &str) -> Self {
        Self {
            id: id.to_string(),
            platform: platform.clone(),
            title: title.to_string(),
            description: String::new(),
            category: String::new(),
            tags: Vec::new(),
            started_at: Instant::now(),
            ended_at: None,
            peak_viewers: 0,
            average_bitrate: 6000, // 6000 kbps.
            dropped_frames: 0,
            total_bytes_sent: 0,
            chat_messages_sent: 0,
            is_archived: false,
        }
    }

    /// End session.
    pub fn end_session(&mut self) {
        self.ended_at = Some(Instant::now());
        self.is_archived = true;
    }

    /// Get session duration.
    pub fn duration(&self) -> Duration {
        let end = self.ended_at.unwrap_or_else(Instant::now);
        end.duration_since(self.started_at)
    }

    /// Get duration string.
    pub fn duration_string(&self) -> String {
        let secs = self.duration().as_secs();
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        let secs = secs % 60;

        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, mins, secs)
        } else {
            format!("{:02}:{:02}", mins, secs)
        }
    }

    /// Update peak viewers.
    pub fn update_viewers(&mut self, current: u32) {
        if current > self.peak_viewers {
            self.peak_viewers = current;
        }
    }

    /// Update bitrate.
    pub fn update_bitrate(&mut self, bitrate: u32) {
        // Simple running average.
        self.average_bitrate = (self.average_bitrate + bitrate) / 2;
    }

    /// Increment dropped frames.
    pub fn increment_dropped_frames(&mut self, count: u32) {
        self.dropped_frames += count;
    }

    /// Increment chat messages.
    pub fn increment_chat_messages(&mut self, count: u32) {
        self.chat_messages_sent += count;
    }

    /// Update bytes sent.
    pub fn update_bytes_sent(&mut self, bytes: u64) {
        self.total_bytes_sent += bytes;
    }

    /// Get bytes sent string.
    pub fn bytes_sent_string(&self) -> String {
        let gb = self.total_bytes_sent as f64 / (1024.0 * 1024.0 * 1024.0);
        if gb >= 1.0 {
            format!("{:.1} GB", gb)
        } else {
            let mb = self.total_bytes_sent as f64 / (1024.0 * 1024.0);
            format!("{:.1} MB", mb)
        }
    }

    /// Set description.
    pub fn set_description(&mut self, description: &str) {
        self.description = description.to_string();
    }

    /// Set category.
    pub fn set_category(&mut self, category: &str) {
        self.category = category.to_string();
    }

    /// Add tag.
    pub fn add_tag(&mut self, tag: &str) {
        if !self.tags.contains(&tag.to_string()) {
            self.tags.push(tag.to_string());
        }
    }

    /// Remove tag.
    pub fn remove_tag(&mut self, tag: &str) -> bool {
        let pos = self.tags.iter().position(|t| t == tag);
        if let Some(idx) = pos {
            self.tags.remove(idx);
            true
        } else {
            false
        }
    }

    /// Get all tags.
    pub fn all_tags(&self) -> Vec<&str> {
        self.tags.iter().map(|t| t.as_str()).collect()
    }

    /// Get category string.
    pub fn category_string(&self) -> String {
        if self.category.is_empty() {
            "No category".to_string()
        } else {
            self.category.clone()
        }
    }

    /// Get tags string.
    pub fn tags_string(&self) -> String {
        self.tags.join(", ")
    }
}

/// Connection quality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionQuality {
    /// Excellent (no issues).
    Excellent,
    /// Good (minor issues).
    Good,
    /// Fair (noticeable issues).
    Fair,
    /// Poor (severe issues).
    Poor,
    /// Disconnected.
    Disconnected,
}

impl ConnectionQuality {
    /// Get quality level (0-100).
    pub fn level(&self) -> u8 {
        match self {
            Self::Excellent => 100,
            Self::Good => 75,
            Self::Fair => 50,
            Self::Poor => 25,
            Self::Disconnected => 0,
        }
    }

    /// Get quality color.
    pub fn color(&self) -> &str {
        match self {
            Self::Excellent => "#00FF00",
            Self::Good => "#88FF00",
            Self::Fair => "#FFFF00",
            Self::Poor => "#FF8800",
            Self::Disconnected => "#FF0000",
        }
    }

    /// Get quality message.
    pub fn message(&self) -> &str {
        match self {
            Self::Excellent => "Excellent connection quality.",
            Self::Good => "Good connection quality.",
            Self::Fair => "Fair connection quality. Some viewers may experience issues.",
            Self::Poor => "Poor connection quality. Consider lowering bitrate.",
            Self::Disconnected => "Disconnected. Reconnecting...",
        }
    }
}

/// Stream settings.
#[derive(Debug, Clone)]
pub struct StreamSettings {
    /// Platform.
    pub platform: StreamPlatform,
    /// Stream key.
    pub stream_key: String,
    /// Server URL.
    pub server_url: String,
    /// Video bitrate (kbps).
    pub video_bitrate: u32,
    /// Audio bitrate (kbps).
    pub audio_bitrate: u32,
    /// Resolution width.
    pub width: u32,
    /// Resolution height.
    pub height: u32,
    /// Frames per second.
    pub fps: u32,
    /// Stream title.
    pub title: String,
    /// Stream description.
    pub description: String,
    /// Stream category.
    pub category: String,
    /// Stream tags.
    pub tags: Vec<String>,
    /// Auto-reconnect.
    pub auto_reconnect: bool,
    /// Reconnect timeout (seconds).
    pub reconnect_timeout: u64,
    /// Maximum reconnect attempts.
    pub max_reconnect_attempts: u32,
    /// Current reconnect attempts.
    pub current_reconnect_attempts: u32,
}

impl StreamSettings {
    /// Create new stream settings.
    pub fn new(platform: StreamPlatform, stream_key: &str) -> Self {
        Self {
            platform,
            stream_key: stream_key.to_string(),
            server_url: String::new(),
            video_bitrate: 6000,
            audio_bitrate: 320,
            width: 1920,
            height: 1080,
            fps: 60,
            title: String::new(),
            description: String::new(),
            category: String::new(),
            tags: Vec::new(),
            auto_reconnect: true,
            reconnect_timeout: 5,
            max_reconnect_attempts: 10,
            current_reconnect_attempts: 0,
        }
    }

    /// Set server URL.
    pub fn set_server_url(&mut self, url: &str) {
        self.server_url = url.to_string();
    }

    /// Set bitrate.
    pub fn set_bitrate(&mut self, video: u32, audio: u32) {
        self.video_bitrate = video;
        self.audio_bitrate = audio;
    }

    /// Set resolution.
    pub fn set_resolution(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Set FPS.
    pub fn set_fps(&mut self, fps: u32) {
        self.fps = fps.clamp(24, 120);
    }

    /// Set title.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    /// Set description.
    pub fn set_description(&mut self, description: &str) {
        self.description = description.to_string();
    }

    /// Set category.
    pub fn set_category(&mut self, category: &str) {
        self.category = category.to_string();
    }

    /// Add tag.
    pub fn add_tag(&mut self, tag: &str) {
        if !self.tags.contains(&tag.to_string()) {
            self.tags.push(tag.to_string());
        }
    }

    /// Set auto-reconnect.
    pub fn set_auto_reconnect(&mut self, enabled: bool) {
        self.auto_reconnect = enabled;
    }

    /// Set reconnect timeout.
    pub fn set_reconnect_timeout(&mut self, seconds: u64) {
        self.reconnect_timeout = seconds;
    }

    /// Set max reconnect attempts.
    pub fn set_max_reconnect_attempts(&mut self, attempts: u32) {
        self.max_reconnect_attempts = attempts;
    }

    /// Reset reconnect attempts.
    pub fn reset_reconnect_attempts(&mut self) {
        self.current_reconnect_attempts = 0;
    }

    /// Increment reconnect attempts.
    pub fn increment_reconnect_attempts(&mut self) -> u32 {
        self.current_reconnect_attempts += 1;
        self.current_reconnect_attempts
    }

    /// Has exceeded max reconnect attempts?
    pub fn has_exceeded_max_reconnect(&self) -> bool {
        self.current_reconnect_attempts >= self.max_reconnect_attempts
    }

    /// Resolution string.
    pub fn resolution_string(&self) -> String {
        format!("{}x{}@{}", self.width, self.height, self.fps)
    }

    /// Bitrate string.
    pub fn bitrate_string(&self) -> String {
        format!("{} kbps / {} kbps", self.video_bitrate, self.audio_bitrate)
    }
}

/// Live streaming engine.
pub struct LiveStreamingEngine {
    /// Stream state.
    state: StreamState,
    /// Settings.
    settings: Option<StreamSettings>,
    /// Active overlays.
    overlays: Vec<OverlayConfig>,
    /// Chat messages (incoming).
    incoming_chat: VecDeque<ChatMessage>,
    /// Chat messages (outgoing).
    outgoing_chat: VecDeque<ChatMessage>,
    /// Chat history.
    chat_history: VecDeque<ChatMessage>,
    /// Chat message counter.
    chat_counter: u32,
    /// Stream sessions.
    sessions: Vec<StreamSession>,
    /// Current session.
    current_session: Option<StreamSession>,
    /// Session counter.
    session_counter: u32,
    /// Connection quality.
    connection_quality: ConnectionQuality,
    /// Last quality update.
    last_quality_update: Instant,
    /// Viewer count.
    viewer_count: u32,
    /// Reconnection timer.
    reconnect_timer: Option<Instant>,
    /// Auto-reconnect enabled.
    auto_reconnect: bool,
    /// Max reconnect attempts.
    max_reconnect_attempts: u32,
    /// Reconnect attempts.
    reconnect_attempts: u32,
}

impl LiveStreamingEngine {
    /// Create new live streaming engine.
    pub fn new() -> Self {
        Self {
            state: StreamState::Idle,
            settings: None,
            overlays: Vec::new(),
            incoming_chat: VecDeque::new(),
            outgoing_chat: VecDeque::new(),
            chat_history: VecDeque::new(),
            chat_counter: 0,
            sessions: Vec::new(),
            current_session: None,
            session_counter: 0,
            connection_quality: ConnectionQuality::Disconnected,
            last_quality_update: Instant::now(),
            viewer_count: 0,
            reconnect_timer: None,
            auto_reconnect: true,
            max_reconnect_attempts: 10,
            reconnect_attempts: 0,
        }
    }

    /// Configure stream settings.
    pub fn configure(&mut self, mut settings: StreamSettings) -> Result<(), StreamingError> {
        if self.state.is_active() {
            return Err(StreamingError::Streaming(
                "Cannot configure while streaming".to_string(),
            ));
        }

        if settings.stream_key.is_empty() {
            return Err(StreamingError::ConfigError(
                "Stream key is required".to_string(),
            ));
        }

        settings.reset_reconnect_attempts();
        self.settings = Some(settings);
        Ok(())
    }

    /// Start stream.
    pub fn start_stream(&mut self, title: &str) -> Result<&mut StreamSession, StreamingError> {
        if self.state.is_active() {
            return Err(StreamingError::Streaming("Already streaming".to_string()));
        }

        if self.settings.is_none() {
            return Err(StreamingError::ConfigError(
                "No stream settings configured".to_string(),
            ));
        }

        let settings = self.settings.as_ref().unwrap();

        // Create session.
        self.session_counter += 1;
        let session_id = format!("session_{}", self.session_counter);
        let session = StreamSession::new(&session_id, settings.platform.clone(), title);

        self.current_session = Some(session);
        self.state = StreamState::Connecting;

        // Simulate connection.
        self.state = StreamState::Streaming;
        self.connection_quality = ConnectionQuality::Excellent;

        Ok(self.current_session.as_mut().unwrap())
    }

    /// Pause stream.
    pub fn pause_stream(&mut self) -> Result<(), StreamingError> {
        if let Some(ref session) = self.current_session {
            if !session.is_archived {
                self.state = StreamState::Paused;
                Ok(())
            } else {
                Err(StreamingError::Streaming("Session ended".to_string()))
            }
        } else {
            Err(StreamingError::NotStreaming)
        }
    }

    /// Resume stream.
    pub fn resume_stream(&mut self) -> Result<(), StreamingError> {
        if self.state == StreamState::Paused {
            self.state = StreamState::Streaming;
            Ok(())
        } else {
            Err(StreamingError::NotPaused)
        }
    }

    /// Stop stream.
    pub fn stop_stream(&mut self) -> Result<(), StreamingError> {
        if let Some(ref mut session) = self.current_session {
            session.end_session();
            self.sessions.push(session.clone());
        }

        self.current_session = None;
        self.state = StreamState::Idle;
        self.reconnect_attempts = 0;
        self.reconnect_timer = None;

        Ok(())
    }

    /// Send chat message.
    pub fn send_chat(
        &mut self,
        username: &str,
        content: &str,
    ) -> Result<ChatMessage, StreamingError> {
        if !self.state.is_active() {
            return Err(StreamingError::NotStreaming);
        }

        if content.is_empty() || content.len() > 500 {
            return Err(StreamingError::ChatError(
                "Invalid message content".to_string(),
            ));
        }

        if username.is_empty() {
            return Err(StreamingError::ChatError(
                "Username is required".to_string(),
            ));
        }

        self.chat_counter += 1;
        let message_id = format!("chat_{}", self.chat_counter);
        let message = ChatMessage::new(&message_id, username, content, true);

        if let Some(ref mut session) = self.current_session {
            session.increment_chat_messages(1);
        }

        self.outgoing_chat.push_back(message.clone());
        self.chat_history.push_back(message.clone());

        // Keep chat history at max 1000 messages.
        if self.chat_history.len() > 1000 {
            self.chat_history.pop_front();
        }

        Ok(message)
    }

    /// Receive chat message.
    pub fn receive_chat(&mut self, username: &str, content: &str) {
        self.chat_counter += 1;
        let message_id = format!("chat_recv_{}", self.chat_counter);
        let message = ChatMessage::new(&message_id, username, content, false);

        self.incoming_chat.push_back(message.clone());
        self.chat_history.push_back(message);

        // Keep chat history at max 1000 messages.
        if self.chat_history.len() > 1000 {
            self.chat_history.pop_front();
        }
    }

    /// Get incoming messages.
    pub fn incoming_messages(&self, count: usize) -> Vec<&ChatMessage> {
        self.incoming_chat.iter().take(count).collect()
    }

    /// Clear incoming messages.
    pub fn clear_incoming(&mut self) {
        self.incoming_chat.clear();
    }

    /// Add overlay.
    pub fn add_overlay(&mut self, overlay: OverlayConfig) -> Result<(), StreamingError> {
        if !overlay.is_valid() {
            return Err(StreamingError::OverlayError(
                "Invalid overlay configuration".to_string(),
            ));
        }

        self.overlays.push(overlay);
        Ok(())
    }

    /// Remove overlay.
    pub fn remove_overlay(&mut self, index: usize) -> Result<OverlayConfig, StreamingError> {
        if index < self.overlays.len() {
            Ok(self.overlays.remove(index))
        } else {
            Err(StreamingError::OverlayError(
                "Overlay not found".to_string(),
            ))
        }
    }

    /// Get overlay at index.
    pub fn get_overlay(&self, index: usize) -> Option<&OverlayConfig> {
        self.overlays.get(index)
    }

    /// Get all overlays.
    pub fn all_overlays(&self) -> &[OverlayConfig] {
        &self.overlays
    }

    /// Update overlay position.
    pub fn update_overlay_position(
        &mut self,
        index: usize,
        x: u32,
        y: u32,
    ) -> Result<(), StreamingError> {
        if let Some(overlay) = self.overlays.get_mut(index) {
            overlay.set_position(x, y);
            Ok(())
        } else {
            Err(StreamingError::OverlayError(
                "Overlay not found".to_string(),
            ))
        }
    }

    /// Update overlay size.
    pub fn update_overlay_size(
        &mut self,
        index: usize,
        width: u32,
        height: u32,
    ) -> Result<(), StreamingError> {
        if let Some(overlay) = self.overlays.get_mut(index) {
            overlay.set_size(width, height);
            Ok(())
        } else {
            Err(StreamingError::OverlayError(
                "Overlay not found".to_string(),
            ))
        }
    }

    /// Toggle overlay visibility.
    pub fn toggle_overlay_visibility(&mut self, index: usize) -> Result<(), StreamingError> {
        if let Some(overlay) = self.overlays.get_mut(index) {
            overlay.toggle_visibility();
            Ok(())
        } else {
            Err(StreamingError::OverlayError(
                "Overlay not found".to_string(),
            ))
        }
    }

    /// Get active overlays (visible only).
    pub fn active_overlays(&self) -> Vec<&OverlayConfig> {
        self.overlays.iter().filter(|o| o.is_visible).collect()
    }

    /// Get overlay count.
    pub fn overlay_count(&self) -> usize {
        self.overlays.len()
    }

    /// Update connection quality.
    pub fn update_connection_quality(&mut self, quality: ConnectionQuality) {
        self.connection_quality = quality;
        self.last_quality_update = Instant::now();
    }

    /// Get connection quality.
    pub fn connection_quality(&self) -> ConnectionQuality {
        self.connection_quality.clone()
    }

    /// Check if connection is degrading (Fair or worse).
    pub fn is_connection_degraded(&self) -> bool {
        matches!(
            self.connection_quality,
            ConnectionQuality::Fair | ConnectionQuality::Poor | ConnectionQuality::Disconnected
        )
    }

    /// Update viewer count.
    pub fn update_viewer_count(&mut self, count: u32) {
        self.viewer_count = count;
        if let Some(ref mut session) = self.current_session {
            session.update_viewers(count);
        }
    }

    /// Get viewer count.
    pub fn viewer_count(&self) -> u32 {
        self.viewer_count
    }

    /// Auto-reconnect.
    pub fn auto_reconnect(&mut self) -> Result<(), StreamingError> {
        if !self.auto_reconnect {
            return Err(StreamingError::ReconnectError(
                "Auto-reconnect disabled".to_string(),
            ));
        }

        if self.reconnect_attempts >= self.max_reconnect_attempts {
            return Err(StreamingError::ReconnectError(
                "Max reconnect attempts exceeded".to_string(),
            ));
        }

        if !self.state.is_active() {
            return Err(StreamingError::NotStreaming);
        }

        self.state = StreamState::Reconnecting;
        self.reconnect_attempts += 1;

        // Simulate reconnection.
        self.state = StreamState::Streaming;
        self.connection_quality = ConnectionQuality::Good;

        Ok(())
    }

    /// Get reconnect attempts.
    pub fn reconnect_attempts(&self) -> u32 {
        self.reconnect_attempts
    }

    /// Set max reconnect attempts.
    pub fn set_max_reconnect_attempts(&mut self, attempts: u32) {
        self.max_reconnect_attempts = attempts;
    }

    /// Toggle auto-reconnect.
    pub fn toggle_auto_reconnect(&mut self) {
        self.auto_reconnect = !self.auto_reconnect;
    }

    /// Get stream state.
    pub fn stream_state(&self) -> &StreamState {
        &self.state
    }

    /// Get current session.
    pub fn current_session(&self) -> Option<&StreamSession> {
        self.current_session.as_ref()
    }

    /// Get all sessions.
    pub fn all_sessions(&self) -> &[StreamSession] {
        &self.sessions
    }

    /// Get session count.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get chat history.
    pub fn chat_history(&self, count: usize) -> Vec<&ChatMessage> {
        self.chat_history.iter().rev().take(count).collect()
    }

    /// Get chat history count.
    pub fn chat_history_count(&self) -> usize {
        self.chat_history.len()
    }

    /// Clear chat history.
    pub fn clear_chat_history(&mut self) {
        self.chat_history.clear();
        self.incoming_chat.clear();
        self.outgoing_chat.clear();
    }

    /// Get settings.
    pub fn settings(&self) -> Option<&StreamSettings> {
        self.settings.as_ref()
    }

    /// Get settings string.
    pub fn settings_string(&self) -> String {
        if let Some(ref settings) = self.settings {
            format!(
                "Platform: {} | Resolution: {} | Bitrate: {} | Reconnect: {}",
                settings.platform.name(),
                settings.resolution_string(),
                settings.bitrate_string(),
                if settings.auto_reconnect {
                    "Enabled"
                } else {
                    "Disabled"
                }
            )
        } else {
            "No settings configured".to_string()
        }
    }

    /// Get status string.
    pub fn status_string(&self) -> String {
        let viewers = self.viewer_count;
        let quality = self.connection_quality();
        format!(
            "{} | Viewers: {} | Quality: {} | Attempts: {}/{}",
            self.state.name(),
            viewers,
            quality.message(),
            self.reconnect_attempts,
            self.max_reconnect_attempts
        )
    }
}

impl StreamState {
    /// Get state name.
    pub fn name(&self) -> &str {
        match self {
            Self::Idle => "Idle",
            Self::Connecting => "Connecting",
            Self::Streaming => "Streaming",
            Self::Paused => "Paused",
            Self::Reconnecting => "Reconnecting",
            Self::Error(msg) => msg,
        }
    }
}

impl Default for LiveStreamingEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Streaming error.
#[derive(Debug, Clone)]
pub enum StreamingError {
    /// Config error.
    ConfigError(String),
    /// Streaming error.
    Streaming(String),
    /// Not streaming.
    NotStreaming,
    /// Not paused.
    NotPaused,
    /// Chat error.
    ChatError(String),
    /// Overlay error.
    OverlayError(String),
    /// Reconnect error.
    ReconnectError(String),
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for StreamingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigError(msg) => write!(f, "Config error: {}", msg),
            Self::Streaming(msg) => write!(f, "Streaming error: {}", msg),
            Self::NotStreaming => write!(f, "Not streaming"),
            Self::NotPaused => write!(f, "Not paused"),
            Self::ChatError(msg) => write!(f, "Chat error: {}", msg),
            Self::OverlayError(msg) => write!(f, "Overlay error: {}", msg),
            Self::ReconnectError(msg) => write!(f, "Reconnect error: {}", msg),
            Self::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for StreamingError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_engine_creation() {
        let engine = LiveStreamingEngine::new();
        assert_eq!(engine.stream_state(), &StreamState::Idle);
        assert!(engine.settings().is_none());
    }

    #[test]
    fn streaming_engine_configure() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "test_key");
        assert!(engine.configure(settings).is_ok());
        assert!(engine.settings().is_some());
    }

    #[test]
    fn streaming_engine_no_key() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "");
        assert!(engine.configure(settings).is_err());
    }

    #[test]
    fn streaming_engine_start_stop() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "test_key");
        engine.configure(settings).unwrap();

        let session = engine.start_stream("Test Stream");
        assert!(session.is_ok());
        assert!(engine.stream_state().is_streaming());

        assert!(engine.stop_stream().is_ok());
        assert_eq!(engine.stream_state(), &StreamState::Idle);
    }

    #[test]
    fn streaming_engine_pause_resume() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "test_key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        assert!(engine.pause_stream().is_ok());
        assert!(*engine.stream_state() == StreamState::Paused);

        assert!(engine.resume_stream().is_ok());
        assert!(engine.stream_state().is_streaming());
    }

    #[test]
    fn streaming_engine_pause_without_streaming() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "test_key");
        engine.configure(settings).unwrap();

        assert!(engine.pause_stream().is_err());
    }

    #[test]
    fn streaming_engine_resume_without_pause() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "test_key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        assert!(engine.resume_stream().is_err());
    }

    #[test]
    fn streaming_engine_send_chat() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "test_key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        let msg = engine.send_chat("DJ", "Hello world!");
        assert!(msg.is_ok());
        assert_eq!(msg.unwrap().username, "DJ");
    }

    #[test]
    fn streaming_engine_send_chat_empty() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "test_key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        assert!(engine.send_chat("DJ", "").is_err());
    }

    #[test]
    fn streaming_engine_send_chat_no_stream() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "test_key");
        engine.configure(settings).unwrap();

        assert!(engine.send_chat("DJ", "Hello").is_err());
    }

    #[test]
    fn streaming_engine_receive_chat() {
        let mut engine = LiveStreamingEngine::new();
        engine.receive_chat("Viewer1", "Hey!");
        let msgs = engine.incoming_messages(10);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].username, "Viewer1");
    }

    #[test]
    fn streaming_engine_clear_incoming() {
        let mut engine = LiveStreamingEngine::new();
        engine.receive_chat("Viewer1", "Hey!");
        engine.clear_incoming();
        assert!(engine.incoming_messages(10).is_empty());
    }

    #[test]
    fn streaming_engine_add_overlay() {
        let mut engine = LiveStreamingEngine::new();
        let overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 10, 200, 40);
        assert!(engine.add_overlay(overlay).is_ok());
        assert_eq!(engine.overlay_count(), 1);
    }

    #[test]
    fn streaming_engine_remove_overlay() {
        let mut engine = LiveStreamingEngine::new();
        let overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 10, 200, 40);
        engine.add_overlay(overlay).unwrap();

        let removed = engine.remove_overlay(0);
        assert!(removed.is_ok());
        assert_eq!(engine.overlay_count(), 0);
    }

    #[test]
    fn streaming_engine_toggle_overlay() {
        let mut engine = LiveStreamingEngine::new();
        let overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 10, 200, 40);
        engine.add_overlay(overlay).unwrap();

        engine.toggle_overlay_visibility(0).unwrap();
        assert!(!engine.all_overlays()[0].is_visible);
    }

    #[test]
    fn streaming_engine_active_overlays() {
        let mut engine = LiveStreamingEngine::new();
        let overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 10, 200, 40);
        engine.add_overlay(overlay).unwrap();

        let active = engine.active_overlays();
        assert_eq!(active.len(), 1);

        engine.toggle_overlay_visibility(0).unwrap();
        let active = engine.active_overlays();
        assert_eq!(active.len(), 0);
    }

    #[test]
    fn streaming_engine_invalid_overlay() {
        let mut engine = LiveStreamingEngine::new();
        let mut overlay = OverlayConfig::new(OverlayType::NowPlaying, 0, 0, 0, 0);
        overlay.set_opacity(1.5); // Invalid.

        assert!(engine.add_overlay(overlay).is_err());
    }

    #[test]
    fn streaming_engine_connection_quality() {
        let mut engine = LiveStreamingEngine::new();
        engine.update_connection_quality(ConnectionQuality::Excellent);
        assert_eq!(engine.connection_quality().level(), 100);

        engine.update_connection_quality(ConnectionQuality::Poor);
        assert!(engine.is_connection_degraded());
    }

    #[test]
    fn streaming_engine_viewer_count() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        engine.update_viewer_count(100);
        assert_eq!(engine.viewer_count(), 100);

        engine.update_viewer_count(150);
        assert_eq!(engine.viewer_count(), 150);
    }

    #[test]
    fn streaming_engine_auto_reconnect() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        assert!(engine.auto_reconnect().is_ok());
        assert_eq!(engine.reconnect_attempts(), 1);
    }

    #[test]
    fn streaming_engine_reconnect_max_attempts() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        engine.set_max_reconnect_attempts(2);

        let _ = engine.auto_reconnect();
        let _ = engine.auto_reconnect();
        assert!(engine.auto_reconnect().is_err());
    }

    #[test]
    fn streaming_engine_reconnect_disabled() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        engine.toggle_auto_reconnect();
        assert!(engine.auto_reconnect().is_err());
    }

    #[test]
    fn streaming_engine_session_history() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();
        engine.stop_stream().unwrap();

        assert_eq!(engine.session_count(), 1);
        let session = engine.all_sessions().first().unwrap();
        assert!(session.is_archived);
    }

    #[test]
    fn streaming_engine_chat_history() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        let _ = engine.send_chat("DJ", "Hello");
        engine.receive_chat("Viewer1", "Hey!");

        let history = engine.chat_history(100);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn streaming_engine_clear_chat_history() {
        let mut engine = LiveStreamingEngine::new();
        engine.receive_chat("V1", "Hi");
        engine.clear_chat_history();
        assert_eq!(engine.chat_history_count(), 0);
    }

    #[test]
    fn streaming_engine_multiple_sessions() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();

        engine.start_stream("Stream 1").unwrap();
        engine.stop_stream().unwrap();
        engine.start_stream("Stream 2").unwrap();
        engine.stop_stream().unwrap();

        assert_eq!(engine.session_count(), 2);
    }

    #[test]
    fn stream_settings_bitrate() {
        let mut settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        settings.set_bitrate(8000, 256);
        assert!(settings.bitrate_string().contains("8000"));
    }

    #[test]
    fn stream_settings_resolution() {
        let mut settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        settings.set_resolution(1280, 720);
        assert!(settings.resolution_string().contains("1280x720"));
    }

    #[test]
    fn stream_settings_fps_clamp() {
        let mut settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        settings.set_fps(200);
        assert_eq!(settings.fps, 120); // Capped at 120.

        settings.set_fps(10);
        assert_eq!(settings.fps, 24); // Minimum is 24.
    }

    #[test]
    fn stream_settings_reset_reconnect() {
        let mut settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        settings.increment_reconnect_attempts();
        assert_eq!(settings.current_reconnect_attempts, 1);

        settings.reset_reconnect_attempts();
        assert_eq!(settings.current_reconnect_attempts, 0);
    }

    #[test]
    fn overlay_config_position() {
        let mut overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 10, 200, 40);
        overlay.set_position(100, 200);
        assert_eq!(overlay.x, 100);
        assert_eq!(overlay.y, 200);
    }

    #[test]
    fn overlay_config_size() {
        let mut overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 10, 200, 40);
        overlay.set_size(300, 60);
        assert_eq!(overlay.width, 300);
        assert_eq!(overlay.height, 60);
    }

    #[test]
    fn overlay_config_opacity() {
        let mut overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 10, 200, 40);
        overlay.set_opacity(0.5);
        assert!((overlay.opacity - 0.5).abs() < f32::EPSILON);

        overlay.set_opacity(1.5); // Should clamp.
        assert!((overlay.opacity - 1.0).abs() < f32::EPSILON);

        overlay.set_opacity(-0.5); // Should clamp.
        assert!((overlay.opacity).abs() < f32::EPSILON);
    }

    #[test]
    fn overlay_config_toggle_visibility() {
        let mut overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 10, 200, 40);
        assert!(overlay.is_visible);

        overlay.toggle_visibility();
        assert!(!overlay.is_visible);

        overlay.toggle_visibility();
        assert!(overlay.is_visible);
    }

    #[test]
    fn overlay_config_geometry_string() {
        let overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 20, 100, 50);
        let geo = overlay.geometry_string();
        assert!(geo.contains("x=10"));
        assert!(geo.contains("y=20"));
    }

    #[test]
    fn overlay_config_invalid() {
        let overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 10, 0, 50);
        assert!(!overlay.is_valid());
    }

    #[test]
    fn chat_message_valid() {
        let msg = ChatMessage::new("1", "DJ", "Hello", true);
        assert!(msg.is_valid());

        let empty = ChatMessage::new("2", "", "Hello", false);
        assert!(!empty.is_valid());

        let empty_user = ChatMessage::new("3", "User", "", false);
        assert!(!empty_user.is_valid());
    }

    #[test]
    fn chat_message_preview() {
        let msg = ChatMessage::new(
            "1",
            "DJ",
            "This is a long message that should be truncated",
            true,
        );
        let preview = msg.preview(10);
        assert!(preview.len() < 30);
    }

    #[test]
    fn chat_message_broadcaster_color() {
        let msg = ChatMessage::new("1", "DJ", "Hello", true);
        assert_eq!(msg.color, "#FF0000");
    }

    #[test]
    fn stream_session_duration_string() {
        let mut session = StreamSession::new("s1", StreamPlatform::Twitch, "Test");
        session.end_session();
        let dur = session.duration_string();
        assert!(!dur.is_empty());
    }

    #[test]
    fn stream_session_viewers() {
        let mut session = StreamSession::new("s1", StreamPlatform::Twitch, "Test");
        session.update_viewers(100);
        session.update_viewers(50); // Should not lower peak.
        assert_eq!(session.peak_viewers, 100);
    }

    #[test]
    fn stream_session_tags() {
        let mut session = StreamSession::new("s1", StreamPlatform::Twitch, "Test");
        session.add_tag("live");
        session.add_tag("electronic");
        session.add_tag("live"); // Duplicate.

        let tags = session.all_tags();
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn stream_session_remove_tag() {
        let mut session = StreamSession::new("s1", StreamPlatform::Twitch, "Test");
        session.add_tag("live");
        assert!(session.remove_tag("live"));
        assert!(!session.remove_tag("live")); // Already removed.
    }

    #[test]
    fn connection_quality_level() {
        assert_eq!(ConnectionQuality::Excellent.level(), 100);
        assert_eq!(ConnectionQuality::Good.level(), 75);
        assert_eq!(ConnectionQuality::Fair.level(), 50);
        assert_eq!(ConnectionQuality::Poor.level(), 25);
        assert_eq!(ConnectionQuality::Disconnected.level(), 0);
    }

    #[test]
    fn connection_quality_color() {
        assert_eq!(ConnectionQuality::Excellent.color(), "#00FF00");
        assert_eq!(ConnectionQuality::Good.color(), "#88FF00");
        assert_eq!(ConnectionQuality::Fair.color(), "#FFFF00");
        assert_eq!(ConnectionQuality::Poor.color(), "#FF8800");
        assert_eq!(ConnectionQuality::Disconnected.color(), "#FF0000");
    }

    #[test]
    fn stream_platform_name() {
        assert_eq!(StreamPlatform::Twitch.name(), "Twitch");
        assert_eq!(StreamPlatform::YouTube.name(), "YouTube");
        assert_eq!(StreamPlatform::CustomRTMP.name(), "Custom RTMP");
    }

    #[test]
    fn stream_platform_default_port() {
        assert_eq!(StreamPlatform::Twitch.default_port(), 443);
        assert_eq!(StreamPlatform::CustomRTMP.default_port(), 1935);
    }

    #[test]
    fn overlay_type_name() {
        assert_eq!(OverlayType::NowPlaying.name(), "Now Playing");
        assert_eq!(OverlayType::Logo.name(), "Logo");
        assert_eq!(OverlayType::ChatMessages.name(), "Chat Messages");
    }

    #[test]
    fn stream_session_bytes_sent_string() {
        let mut session = StreamSession::new("s1", StreamPlatform::Twitch, "Test");
        session.update_bytes_sent(1024 * 1024 * 100); // 100 MB.
        assert!(session.bytes_sent_string().contains("MB"));
    }

    #[test]
    fn streaming_engine_status_string() {
        let engine = LiveStreamingEngine::new();
        let status = engine.status_string();
        assert!(status.contains("Idle"));
    }

    #[test]
    fn streaming_engine_settings_string() {
        let mut engine = LiveStreamingEngine::new();
        assert_eq!(engine.settings_string(), "No settings configured");

        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        assert!(engine.settings_string().contains("Twitch"));
    }

    #[test]
    fn streaming_engine_update_overlay_position() {
        let mut engine = LiveStreamingEngine::new();
        let overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 10, 200, 40);
        engine.add_overlay(overlay).unwrap();

        assert!(engine.update_overlay_position(0, 100, 200).is_ok());
        assert_eq!(engine.get_overlay(0).unwrap().x, 100);
    }

    #[test]
    fn streaming_engine_update_overlay_size() {
        let mut engine = LiveStreamingEngine::new();
        let overlay = OverlayConfig::new(OverlayType::NowPlaying, 10, 10, 200, 40);
        engine.add_overlay(overlay).unwrap();

        assert!(engine.update_overlay_size(0, 300, 60).is_ok());
        assert_eq!(engine.get_overlay(0).unwrap().width, 300);
    }

    #[test]
    fn streaming_engine_overlay_not_found() {
        let mut engine = LiveStreamingEngine::new();
        assert!(engine.remove_overlay(0).is_err());
        assert!(engine.update_overlay_position(0, 10, 10).is_err());
    }

    #[test]
    fn streaming_engine_chat_long_message() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        let long_msg = "a".repeat(501);
        assert!(engine.send_chat("DJ", &long_msg).is_err());
    }

    #[test]
    fn streaming_engine_chat_empty_username() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        assert!(engine.send_chat("", "Hello").is_err());
    }

    #[test]
    fn streaming_error_display() {
        assert_eq!(
            format!("{}", StreamingError::ConfigError("test".to_string())),
            "Config error: test"
        );
        assert_eq!(format!("{}", StreamingError::NotStreaming), "Not streaming");
    }

    #[test]
    fn stream_session_category_string() {
        let mut session = StreamSession::new("s1", StreamPlatform::Twitch, "Test");
        assert_eq!(session.category_string(), "No category");

        session.set_category("DJ Set");
        assert_eq!(session.category_string(), "DJ Set");
    }

    #[test]
    fn stream_session_tags_string() {
        let mut session = StreamSession::new("s1", StreamPlatform::Twitch, "Test");
        session.add_tag("live");
        session.add_tag("electronic");
        assert!(session.tags_string().contains("live"));
    }

    #[test]
    fn streaming_engine_config_while_streaming() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        let new_settings = StreamSettings::new(StreamPlatform::YouTube, "new_key");
        assert!(engine.configure(new_settings).is_err());
    }

    #[test]
    fn stream_settings_max_reconnect() {
        let mut settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        settings.max_reconnect_attempts = 2;
        settings.current_reconnect_attempts = 2;

        assert!(settings.has_exceeded_max_reconnect());
    }

    #[test]
    fn streaming_engine_stop_ends_session() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        let before_count = engine.session_count();
        engine.stop_stream().unwrap();
        assert_eq!(engine.session_count(), before_count + 1);
    }

    #[test]
    fn streaming_engine_chat_message_count_tracking() {
        let mut engine = LiveStreamingEngine::new();
        let settings = StreamSettings::new(StreamPlatform::Twitch, "key");
        engine.configure(settings).unwrap();
        engine.start_stream("Test").unwrap();

        let _ = engine.send_chat("DJ", "Hello");
        let _ = engine.send_chat("DJ", "World");

        let session = engine.current_session().unwrap();
        assert_eq!(session.chat_messages_sent, 2);
    }
}
