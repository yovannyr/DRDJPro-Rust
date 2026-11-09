//! Reusable control candidate contracts, independent of desktop startup.
//! Models and mock/stub adapters here do not imply desktop or external-service integration.

/// Contracts from the command_scheduler implementation.
pub mod command_scheduler {
    pub use crate::command_scheduler::{
        Command, CommandId, Config, ScheduleResult, Scheduler, SchedulerStatsSnapshot,
        TimestampedCommand, TransportAction,
    };
}
/// Contracts from the deck_model implementation.
pub mod deck_model {
    pub use crate::deck_model::{
        DeckManager, DeckState, DeckTransport, MixerState, MAX_DECKS, MAX_HOT_CUES, MAX_SAVED_LOOPS,
    };
}
/// Contracts from the midi_controller implementation.
pub mod midi_controller {
    pub use crate::midi_controller::{
        MidiAction, MidiDevice, MidiEqBand, MidiManager, MidiMapping, MidiMessage, MidiMessageType,
        MidiPreset,
    };
}
/// Contracts from the midi_preset_manager implementation.
pub mod midi_preset_manager {
    pub use crate::midi_preset_manager::{
        ControllerState, MidiControllerType, MidiPreset, MidiPresetManager, ParameterMapping,
        PresetValue, TargetType,
    };
}
/// Contracts from the performance_pads implementation.
pub mod performance_pads {
    pub use crate::performance_pads::{
        FeedbackType, PadColor, PadConfig, PadError, PadFeedback, PadMode, PerformancePadEngine,
        SamplerSound, SavedLoop, SlicedRegion, TriggerVelocity, VelocityLevel, BEAT_JUMP_DISTANCES,
    };
}
/// Contracts from the dvs implementation.
pub mod dvs {
    pub use crate::dvs::{
        CalibrationResult, DvsConfig, DvsDeck, DvsDeviceState, DvsManager, ScratchType,
        TimecodeFormat, TimecodeFrame,
    };
}
/// Contracts from the dvs_recording implementation.
pub mod dvs_recording {
    pub use crate::dvs_recording::{
        CalibrationState, DvsConfig, DvsError, DvsMode, DvsRecordingEngine, DvsRecordingMetrics,
        PitchRange, PlatterState, PlaybackDirection, RecoveryState, SignalQuality,
        TrackingAccuracy, VinylRPM,
    };
}
/// Contracts from the audio_output implementation.
pub mod audio_output {
    pub use crate::audio_output::{
        AudioDevice, AudioFormat, AudioOutputEngine, LatencyMetrics, OutputEngineState,
        OutputRouting,
    };
}
