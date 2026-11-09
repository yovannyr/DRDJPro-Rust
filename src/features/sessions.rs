//! Reusable sessions candidate contracts, independent of desktop startup.
//! Models and mock/stub adapters here do not imply desktop or external-service integration.

/// Contracts from the mix_session implementation.
pub mod mix_session {
    pub use crate::mix_session::{
        DeckState, EqBand, MixSession, PlayStatus, SessionEvent, TransitionType,
    };
}
/// Contracts from the mix_history_analytics implementation.
pub mod mix_history_analytics {
    pub use crate::mix_history_analytics::{
        AutoSessionState, CrossfaderPosition, DeleteMode, ExportBuilder, ExportFormat,
        ExportResult, MixHistoryAnalytics, OverallStatistics, PerformanceMetrics,
        PersistenceConfig, QualityScore, ReplaySnapshot, SessionFilter, SessionRecord,
        SessionReplay, SessionReplayBuilder, SortDirection, SortKey, TrackInfo, TransitionRecord,
        TransitionType,
    };
}
/// Contracts from the session_analytics implementation.
pub mod session_analytics {
    pub use crate::session_analytics::{
        EnergyFlow, GenreStats, KeyCompatibilityAnalysis, MixingPattern, SessionAnalytics,
        SessionAnalyticsEngine, TrackUsage, TransitionAnalysis,
    };
}
/// Contracts from the recording_catalog implementation.
pub mod recording_catalog {
    pub use crate::recording_catalog::{
        RecordingCatalog, RecordingEntry, RecordingError, RecordingFormat, RecordingMetadata,
        RecordingStatus, SortColumn, SortOrder, StorageWarningLevel,
    };
}
