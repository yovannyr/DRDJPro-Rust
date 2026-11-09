//! Reusable presentation candidate contracts, independent of desktop startup.
//! Models and mock/stub adapters here do not imply desktop or external-service integration.

/// Contracts from the ui_framework implementation.
pub mod ui_framework {
    pub use crate::ui_framework::layouts::{default_dual_deck, full_analysis, single_deck};
    pub use crate::ui_framework::{
        UiBackend, UiElementState, UiElementType, UiFramework, UiLayout, UiTheme,
    };
}
/// Contracts from the waveform_overlay implementation.
pub mod waveform_overlay {
    pub use crate::waveform_overlay::{AnimationEvent, AnimationEventKind};
    pub use crate::waveform_overlay::{
        DeckWaveformConfig, MultiDeckWaveformState, OverlayAnimationController,
        WaveformColorScheme, WaveformOverlay, ZoomLevel,
    };
}
