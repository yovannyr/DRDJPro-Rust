//! Reusable observability candidate contracts, independent of desktop startup.
//! Models and mock/stub adapters here do not imply desktop or external-service integration.

/// Contracts from the audio_taps implementation.
pub mod audio_taps {
    pub use crate::audio_taps::{AudioTap, TapManager, TapStats, TapType};
}
/// Contracts from the performance implementation.
pub mod performance {
    pub use crate::performance::{
        AlertLevel, AudioCallbackMonitor, MetricSample, MetricType, PerformanceAlert,
        PerformanceMonitor, PerformanceSnapshot, ThresholdConfig,
    };
}
/// Contracts from the performance_metrics implementation.
pub mod performance_metrics {
    pub use crate::performance_metrics::{
        AlertLevel, MetricSample, MetricStatistics, MetricType, PerformanceAlert,
        PerformanceMetrics, PerformanceReport, SyncQualityMeasurement, SyncTrend,
    };
}
/// Contracts from the sync_analysis implementation.
pub mod sync_analysis {
    pub use crate::sync_analysis::{
        DriftStatus, PhaseTracker, PhaseTrend, SyncAnalysisConfig, SyncAnalysisEngine, SyncEvent,
        SyncEventType, SyncQuality, SyncState,
    };
}
/// Contracts from the neural_sync_engine implementation.
pub mod neural_sync_engine {
    pub use crate::neural_sync_engine::{
        EnergyIndicator, EnergyProfile, EnergyRegion, EnergyTransitionScore, HarmonicMixType,
        HarmonicSuggestion, NeuralSyncEngine, PhraseAlignmentScore, PhraseCompatibility,
        PhrasePosition, PhraseType, PhraseWaveformSegment, PhraseWaveformState, SettingsFormat,
        SuggestionType, SyncError, SyncMetrics, SyncMode, SyncSettings, SyncSuggestion,
        SyncToleranceConfig, TolerancePreset,
    };
}
