//! Shared production UI contract for the desktop executable and integration tests.
//! Startup, controllers and audio composition remain in the executable.
slint::include_modules!();

mod energy;
mod harmonie_wheel;
mod harmony_wheel_ui;
mod key_analysis_extension;
mod spectrogram;
mod spectrogram_export;

/// Reusable offline analysis and presentation models.
///
/// These contracts are independent of desktop startup. Exposing a model here
/// does not mean it is connected to the desktop controller or audio callback.
pub mod analysis {
    /// EBU R128 analysis of caller-supplied interleaved PCM, outside audio callbacks.
    pub mod energy {
        pub use crate::energy::{analyze_energy, Config, EnergyAnalysis};
    }
    /// Camelot keys, compatibility and user corrections shared by analysis clients.
    pub mod keys {
        pub use crate::harmonie_wheel::{
            CamelotKey, HarmonieWheel, KeyAnalysis, KeyCompatibility, KeyCompatibilityResult,
            KeyMode,
        };
    }
    /// Per-track confidence, analysis history and manual override ownership.
    pub mod corrections {
        pub use crate::key_analysis_extension::{
            KeyAnalysisEngine, KeyAnalysisWithConfidence, KeyChangeEvent, KeyChangeSource,
            KeyConfidenceInterval, KeyHistoryEntry, KeyOverride,
        };
    }
    /// Framework-independent models for an optional harmony-wheel view.
    pub mod wheel {
        pub use crate::harmony_wheel_ui::{
            Chromagram, CompatibilityStage, DisplayMode, HarmonyWheelState, HarmonyWheelUI,
            KeyCorrection, MusicalKey, TrackInfo,
        };
    }
    /// Offline FFT generation, configuration, results and errors.
    pub mod spectrum {
        pub use crate::spectrogram::{
            ColorMap, ExportFormat, FftBin, FftSize, FrameEnergy, HopSize, Spectrogram,
            SpectrogramConfig, SpectrogramError, SpectrogramFrame, SpectrogramGenerator,
        };
    }
    /// Spectrogram export and navigation models; no native window is created.
    pub mod export {
        pub use crate::spectrogram_export::{
            ColorMap, ExportStatus, SpectrogramExport, SpectrogramExportEngine,
            SpectrogramExportFormat, SpectrogramView, SpectrumData,
        };
    }
}

mod equalizer_config;
/// Shared audio parameters owned once by the library and reused by desktop DSP.
pub mod audio_contracts {
    pub use crate::equalizer_config::EqSettings;
}

#[cfg(test)]
mod test_alloc;

mod automix;

mod automix_override;

mod mastering;

mod mastering_extension;

mod mastering_v2;

mod remix_engine;

mod slice_remixer;

mod mixer_channel;

mod mixer_channel_extension;

#[path = "features/mixing.rs"]
pub mod mixing;

mod library_manager;

mod playlist_manager;

mod smart_playlist;

mod beat_grid_engine;

mod beat_grid_ui;

mod tap_tempo;

mod track_analysis;

mod track_prep_engine;

#[path = "features/preparation.rs"]
pub mod preparation;

mod mix_session;

mod mix_history_analytics;

mod session_analytics;

mod recording_catalog;

#[path = "features/sessions.rs"]
pub mod sessions;

mod streaming_provider;

mod streaming_integration;

mod streaming_p05b_extension;

mod credentials;

mod cloud_sync;

mod cloud_sync_extension;

mod live_streaming;

mod update;

mod update_engine;

#[path = "features/providers.rs"]
pub mod providers;

mod stems;

mod stem_cue_export;

mod stem_export_streaming;

mod video_engine;

mod video_recording;

#[path = "features/media.rs"]
pub mod media;

mod command_scheduler;

mod deck_model;

mod midi_controller;

mod midi_preset_manager;

mod performance_pads;

mod dvs;

mod dvs_recording;

mod audio_output;

#[path = "features/control.rs"]
pub mod control;

mod audio_taps;

mod performance;

mod performance_metrics;

mod sync_analysis;

mod neural_sync_engine;

#[path = "features/observability.rs"]
pub mod observability;

mod ui_framework;

mod waveform_overlay;

#[path = "features/presentation.rs"]
pub mod presentation;

mod atomic_float;
mod harmony;

/// Chroma-based key detection on caller-supplied PCM.
pub mod tonal_analysis {
    pub use crate::harmony::{analyze_pcm, KeyAnalysis};
}
