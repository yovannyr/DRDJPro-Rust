//! Reusable preparation candidate contracts, independent of desktop startup.
//! Models and mock/stub adapters here do not imply desktop or external-service integration.

/// Contracts from the library_manager implementation.
pub mod library_manager {
    pub use crate::library_manager::{
        AudioFormat, LibraryManager, LibrarySearchQuery, LibrarySortOption, LibraryStats,
        LibraryTrack,
    };
}
/// Contracts from the playlist_manager implementation.
pub mod playlist_manager {
    pub use crate::playlist_manager::{Playlist, PlaylistEntry, PlaylistManager, PlaylistType};
}
/// Contracts from the smart_playlist implementation.
pub mod smart_playlist {
    pub use crate::smart_playlist::{
        BpmMode, DeletedPlaylist, EnergyFlowPattern, EnergyLevel, KeyCompatibility, KeyMode,
        MockTrack, PlaylistAnalysis, PlaylistCriteria, PlaylistType, PlaylistViewMode,
        SmartPlaylist, SmartPlaylistEngine, TrackScore, TransitionPrediction,
        TransitionQualitySummary,
    };
}
/// Contracts from the beat_grid_engine implementation.
pub mod beat_grid_engine {
    pub use crate::beat_grid_engine::{
        BeatEvent, BeatGrid, BeatGridEngine, BeatTime, BeatType, CorrectionType, DeckPhaseState,
        GridCorrection, PhaseAnalyzer, PhaseRelationship,
    };
}
/// Contracts from the beat_grid_ui implementation.
pub mod beat_grid_ui {
    pub use crate::beat_grid_ui::{
        BeatGridUI, CorrectionHandle, GridRenderConfig, PhaseIndicator, SyncIndicator,
        VisibleRange, VisualBar, VisualBeat, ZoomLevel,
    };
}
/// Contracts from the tap_tempo implementation.
pub mod tap_tempo {
    pub use crate::tap_tempo::{
        BatchAnalysisJob, BatchAnalysisScheduler, BatchStatus, TapTempoManager, TapTempoResult,
        TrackAnalysisResult,
    };
}
/// Contracts from the track_analysis implementation.
pub mod track_analysis {
    pub use crate::track_analysis::{
        AnalysisConfig, AnalysisError, AnalysisState, AudioFormat, BatchJob, BeatGridMarker,
        CuePoint, CuePointType, TrackAnalysis, TrackAnalysisManager, TrackMetadata,
    };
}
/// Contracts from the track_prep_engine implementation.
pub mod track_prep_engine {
    pub use crate::track_prep_engine::{
        BatchAnalysisJob, BatchJobStatus, CueJump, CueJumpType, KeyDetection, KeyMode, MetaField,
        MetadataEdit, MusicalKey, Note, OverlayType, PrepError, PrepStatus, ReAnalysisConfig,
        SaveFormat, TrackPrepEngine, WaveformOverlay,
    };
}
