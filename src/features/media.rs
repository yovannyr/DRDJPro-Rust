//! Reusable media candidate contracts, independent of desktop startup.
//! Models and mock/stub adapters here do not imply desktop or external-service integration.

/// Contracts from the stems implementation.
pub mod stems {
    pub use crate::stems::{
        SeparationMethod, Stem, StemCache, StemCacheEntry, StemError, StemMixState, StemRouting,
        StemSeparation, StemSeparator, VoiceCategory,
    };
}
/// Contracts from the stem_cue_export implementation.
pub mod stem_cue_export {
    pub use crate::stem_cue_export::{
        BatchExtractionJob, BatchStatus, ExportStatus, StemCacheEntry, StemCacheManager,
        StemCueExportEngine, StemCuePoint, StemExportError, StemExportFormat, StemExportJob,
        StemPlaybackState, StemQuality, StemWaveform,
    };
}
/// Contracts from the stem_export_streaming implementation.
pub mod stem_export_streaming {
    pub use crate::stem_export_streaming::{
        ExportFormat, OutputMode, StemBundle, StemExportConfig, StemExportManager,
        StemExportResult, StemMixManager, StemMixSnapshot, StemMixTemplate, StemStreamingManager,
        StemStreamingSession,
    };
}
/// Contracts from the video_engine implementation.
pub mod video_engine {
    pub use crate::video_engine::{
        FrameRateCalculator, OverlayElement, OverlayKind, PlaybackState, PlaybackStats,
        ScalingMode, SyncState, VideoCodec, VideoConfig, VideoEngine, VideoError, VideoFormat,
        VideoFrame, VideoMetadata, VideoResolution,
    };
}
/// Contracts from the video_recording implementation.
pub mod video_recording {
    pub use crate::video_recording::{
        AspectRatio, AudioSyncMode, ChromaKeyColor, ChromaKeySettings, EasingFunction, PiPPosition,
        RecordingConfig, ResolutionPreset, SlideDirection, SplitOrientation, TransitionConfig,
        TransitionType, VideoError, VideoLayout, VideoRecordingEngine, WipeDirection,
        ZoomDirection,
    };
}
