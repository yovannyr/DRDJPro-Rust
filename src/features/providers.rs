//! Reusable providers candidate contracts, independent of desktop startup.
//! Models and mock/stub adapters here do not imply desktop or external-service integration.

/// Contracts from the streaming_provider implementation.
pub mod streaming_provider {
    pub use crate::streaming_provider::{
        AudioPath, MockStreamProvider, OAuthToken, Playlist, Provider, SearchAlbum, SearchArtist,
        SearchResult, StreamError, StreamProvider, StreamQuality, StreamStatus, StreamTrack,
        StreamingEngine,
    };
}
/// Contracts from the streaming_integration implementation.
pub mod streaming_integration {
    pub use crate::streaming_integration::{
        PlaylistSyncResult, StreamCache, StreamCacheEntry, StreamingIntegration, SyncCategory,
        SyncDirection, SyncQueueItem, SyncQueueStatus,
    };
}
/// Contracts from the streaming_p05b_extension implementation.
pub mod streaming_p05b_extension {
    pub use crate::streaming_p05b_extension::{
        AdaptiveBitrateManager, BandwidthLevel, CrossMatchResult, MultiServiceManager,
        OfflineManager, OfflineQueueItem, OfflineState, ParallelSearchResult, PlaylistCrossMatcher,
        ProviderQualityMetrics, QualityMonitor, StreamState, StreamingP05bExtension, TokenEvent,
        TokenManager,
    };
}
/// Contracts from the credentials implementation.
pub mod credentials {
    pub use crate::credentials::{Manager, Vault, CREDENTIAL_FORMAT_VERSION};
}
/// Contracts from the cloud_sync implementation.
pub mod cloud_sync {
    pub use crate::cloud_sync::{
        AccountInfo, DeviceId, DeviceInfo, DeviceManager, DeviceStatus, SelectionRules,
        SyncCategory, SyncOperation, SyncQueueEntry,
    };
}
/// Contracts from the cloud_sync_extension implementation.
pub mod cloud_sync_extension {
    pub use crate::cloud_sync_extension::{
        ConflictResolver, ConflictStrategy, OfflineQueue, SelectionRules, SyncCategory,
        SyncConflict, SyncError, SyncOperation, SyncQueueEntry, SyncReportFormatter,
        SyncSessionSummary, SyncSummaryCalculator,
    };
}
/// Contracts from the live_streaming implementation.
pub mod live_streaming {
    pub use crate::live_streaming::{
        ChatChannel, ChatMessage, ConnectionQuality, LiveStreamingEngine, OverlayConfig,
        OverlayType, StreamPlatform, StreamSession, StreamSettings, StreamState, StreamingError,
    };
}
/// Contracts from the update implementation.
pub mod update {
    pub use crate::update::{
        Config, Release, ReleaseChannel, UpdateCheckResult, UpdateChecker, UpdateNotification,
        UpdateTimer, APP_VERSION, DEFAULT_CHECK_INTERVAL, GITHUB_REPO, MIN_CHECK_GAP,
    };
}
/// Contracts from the update_engine implementation.
pub mod update_engine {
    pub use crate::update_engine::{
        Config, DownloadProgress, DownloadResult, DownloadState, InstallStatus, InstallerConfig,
        Release, ReleaseChannel, StubInstaller, UpdateCheckResult, UpdateEngine, UpdateLogEntry,
        UpdateLogSeverity, UpdateNotification, APP_VERSION, DEFAULT_CHECK_INTERVAL, GITHUB_REPO,
        MIN_CHECK_GAP,
    };
}
