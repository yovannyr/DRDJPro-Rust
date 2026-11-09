//! Reusable mixing candidate contracts, independent of desktop startup.
//! Models and mock/stub adapters here do not imply desktop or external-service integration.

/// Contracts from the automix implementation.
pub mod automix {
    pub use crate::automix::{
        AutomixConfig, AutomixEngine, AutomixMode, AutomixRecommendation, CrossfadeConfig,
        CrossfadeCurve, CrossfadeState, TrackInfo, TransitionScore,
    };
}
/// Contracts from the automix_override implementation.
pub mod automix_override {
    pub use crate::automix_override::{
        AutomixConfig, AutomixError, AutomixLearningData, AutomixOverrideEngine, AutomixState,
        CompatibilityBreakdown, GenrePreference, MissingMetadataMode, MixPlan, MixPlanAction,
        OverrideRecord, OverrideType, TrackMetadata, TransitionEasing,
    };
}
/// Contracts from the mastering implementation.
pub mod mastering {
    pub use crate::mastering::{
        CompressorSettings, EqBand, EqBandType, EqSettings, ExciterSettings, LimiterSettings,
        MasteringChain, MasteringEngine,
    };
}
/// Contracts from the mastering_extension implementation.
pub mod mastering_extension {
    pub use crate::mastering_extension::{
        ChainExportFormat, ChainNode, ChainNodeType, ChainVisualiser, MasteringPreset,
        PresetCategory, PresetError, PresetFileInfo, PresetManager,
    };
}
/// Contracts from the mastering_v2 implementation.
pub mod mastering_v2 {
    pub use crate::mastering_v2::{
        FrequencyBand, GainReductionReading, GainStagingMode, LUFSReading, LevelMeterReading,
        MasteringChain, MasteringEngineV2, MasteringError, MasteringPreset, MeteringData,
        ProcessingStage, SpectrumData, TruePeakReading, CLIPPING_THRESHOLD, LUFS_MAX, LUFS_MIN,
        TRUE_PEAK_LIMIT,
    };
}
/// Contracts from the remix_engine implementation.
pub mod remix_engine {
    pub use crate::remix_engine::{
        BarCount, BeatCount, PerformanceRecording, PhraseCount, RemixEngine, Slice, SliceMode,
        SlicePattern, SliceTrigger,
    };
}
/// Contracts from the slice_remixer implementation.
pub mod slice_remixer {
    pub use crate::slice_remixer::{
        ArrangementEntry, RemixPattern, Slice, SliceEdit, SliceRemixerEngine,
    };
}
/// Contracts from the mixer_channel implementation.
pub mod mixer_channel {
    pub use crate::mixer_channel::{
        ChannelRole, ChannelSettings, ChannelState, CueMode, LimiterState, MasterChannelState,
        MeterReading, MeterType, MixerEngine, MixerError,
    };
}
/// Contracts from the mixer_channel_extension implementation.
pub mod mixer_channel_extension {
    pub use crate::mixer_channel_extension::{
        ChannelGroup, ChannelSettingChange, ChannelUndoStack, EqPreset, FadeCurve, FadeTransition,
        MixerChannelExtension,
    };
}
