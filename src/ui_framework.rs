//! UI Framework Abstraction Layer — Cross-platform UI operations.
//!
//! Provides a unified UI abstraction over Slint (primary) and potential
//! alternative frameworks. Supports common DJ application UI operations
//! like deck rendering, waveform display, and mixer controls.
//! Part of the Session Analytics feature (P25a).
//!
//! ## Design principles
//!
//! * **Abstraction** — Decouple application logic from specific UI framework.
//! * **Performance** — Minimize UI thread blocking.
//! * **Cross-platform** — Consistent behavior on Windows/macOS/Linux.
//! * **Testable** — UI components testable without actual rendering.

use std::time::Duration;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Supported UI backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiBackend {
    /// Slint UI framework.
    #[default]
    Slint,
    /// Placeholder for other frameworks.
    /// Future: GTK, Qt, etc.
    /// Not yet implemented.
    Future,
}

/// UI theme.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UiTheme {
    /// Dark theme (default for DJ apps).
    #[default]
    Dark,
    /// Light theme.
    Light,
    /// Custom theme.
    Custom {
        primary: String,
        secondary: String,
        background: String,
    },
}

/// UI element type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiElementType {
    /// Deck view (waveform, transport, controls).
    Deck,
    /// Mixer channel.
    MixerChannel,
    /// Waveform display.
    Waveform,
    /// Spectrogram display.
    Spectrogram,
    /// Beat grid display.
    BeatGrid,
    /// Track list.
    TrackList,
    /// Library browser.
    LibraryBrowser,
    /// Effect rack.
    EffectRack,
    /// BPM/Tempo display.
    BpmDisplay,
    /// Energy meter.
    EnergyMeter,
    /// Custom element.
    Custom(String),
}

/// UI element state.
#[derive(Debug, Clone)]
pub struct UiElementState {
    /// Element type.
    pub element_type: UiElementType,
    /// Visible.
    pub visible: bool,
    /// Enabled.
    pub enabled: bool,
    /// Position (x, y).
    pub position: (f64, f64),
    /// Size (width, height).
    pub size: (f64, f64),
    /// Custom properties.
    pub properties: std::collections::HashMap<String, String>,
}

impl UiElementState {
    /// Create a new UI element state.
    pub fn new(element_type: UiElementType) -> Self {
        Self {
            element_type,
            visible: true,
            enabled: true,
            position: (0.0, 0.0),
            size: (0.0, 0.0),
            properties: std::collections::HashMap::new(),
        }
    }

    /// Set element size.
    pub fn set_size(&mut self, width: f64, height: f64) {
        self.size = (width, height);
    }

    /// Set element position.
    pub fn set_position(&mut self, x: f64, y: f64) {
        self.position = (x, y);
    }

    /// Set visibility.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Set enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set a property.
    pub fn set_property(&mut self, key: String, value: String) {
        self.properties.insert(key, value);
    }
}

/// UI layout configuration.
#[derive(Debug, Clone)]
pub struct UiLayout {
    /// Layout name.
    pub name: String,
    /// Elements in the layout.
    pub elements: Vec<UiElementState>,
    /// Layout mode (grid, freeform, etc.).
    pub layout_mode: String,
    /// Grid cell size.
    pub grid_cell_size: (f64, f64),
}

impl UiLayout {
    /// Create a new UI layout.
    pub fn new(name: String) -> Self {
        Self {
            name,
            elements: Vec::new(),
            layout_mode: "grid".to_string(),
            grid_cell_size: (200.0, 400.0),
        }
    }

    /// Add element to layout.
    pub fn add_element(&mut self, element: UiElementState) {
        self.elements.push(element);
    }

    /// Remove element from layout.
    pub fn remove_element(&mut self, element_type: &UiElementType) -> Option<UiElementState> {
        let idx = self
            .elements
            .iter()
            .position(|e| &e.element_type == element_type);
        idx.map(|i| self.elements.remove(i))
    }

    /// Get element by type.
    pub fn get_element(&self, element_type: &UiElementType) -> Option<&UiElementState> {
        self.elements
            .iter()
            .find(|e| &e.element_type == element_type)
    }

    /// Get element count.
    pub fn element_count(&self) -> usize {
        self.elements.len()
    }
}

/// Preset layouts for common DJ setups.
pub mod layouts {
    use super::*;

    /// Default dual-deck layout.
    pub fn default_dual_deck() -> UiLayout {
        let mut layout = UiLayout::new("Dual Deck".to_string());

        layout.add_element(UiElementState::new(UiElementType::Deck));
        layout.elements.last_mut().unwrap().set_position(0.0, 0.0);
        layout.elements.last_mut().unwrap().set_size(600.0, 800.0);

        layout.add_element(UiElementState::new(UiElementType::Deck));
        layout.elements.last_mut().unwrap().set_position(600.0, 0.0);
        layout.elements.last_mut().unwrap().set_size(600.0, 800.0);

        layout.add_element(UiElementState::new(UiElementType::MixerChannel));
        layout.elements.last_mut().unwrap().set_position(0.0, 800.0);
        layout.elements.last_mut().unwrap().set_size(1200.0, 200.0);

        layout
    }

    /// Single deck layout.
    pub fn single_deck() -> UiLayout {
        let mut layout = UiLayout::new("Single Deck".to_string());

        layout.add_element(UiElementState::new(UiElementType::Deck));
        layout.elements.last_mut().unwrap().set_size(1200.0, 800.0);

        layout.add_element(UiElementState::new(UiElementType::MixerChannel));
        layout.elements.last_mut().unwrap().set_position(0.0, 800.0);
        layout.elements.last_mut().unwrap().set_size(1200.0, 200.0);

        layout
    }

    /// Full analysis layout.
    pub fn full_analysis() -> UiLayout {
        let mut layout = UiLayout::new("Full Analysis".to_string());

        layout.add_element(UiElementState::new(UiElementType::Deck));
        layout.elements.last_mut().unwrap().set_size(400.0, 600.0);

        layout.add_element(UiElementState::new(UiElementType::Deck));
        layout.elements.last_mut().unwrap().set_position(400.0, 0.0);
        layout.elements.last_mut().unwrap().set_size(400.0, 600.0);

        layout.add_element(UiElementState::new(UiElementType::Waveform));
        layout.elements.last_mut().unwrap().set_position(800.0, 0.0);
        layout.elements.last_mut().unwrap().set_size(400.0, 300.0);

        layout.add_element(UiElementState::new(UiElementType::Spectrogram));
        layout
            .elements
            .last_mut()
            .unwrap()
            .set_position(800.0, 300.0);
        layout.elements.last_mut().unwrap().set_size(400.0, 300.0);

        layout.add_element(UiElementState::new(UiElementType::MixerChannel));
        layout.elements.last_mut().unwrap().set_position(0.0, 600.0);
        layout.elements.last_mut().unwrap().set_size(800.0, 200.0);

        layout
    }
}

/// UI framework abstraction.
pub struct UiFramework {
    /// Current backend.
    backend: UiBackend,
    /// Current theme.
    theme: UiTheme,
    /// Current layout.
    current_layout: Option<UiLayout>,
    /// Refresh rate.
    refresh_rate: Duration,
    /// FPS target.
    fps_target: u32,
    /// Render count.
    render_count: u64,
}

impl UiFramework {
    /// Create a new UI framework.
    pub fn new() -> Self {
        Self {
            backend: UiBackend::default(),
            theme: UiTheme::default(),
            current_layout: None,
            refresh_rate: Duration::from_millis(16), // ~60 FPS
            fps_target: 60,
            render_count: 0,
        }
    }

    /// Set the UI backend.
    pub fn set_backend(&mut self, backend: UiBackend) {
        self.backend = backend;
    }

    /// Get the current backend.
    pub fn backend(&self) -> UiBackend {
        self.backend
    }

    /// Set the UI theme.
    pub fn set_theme(&mut self, theme: UiTheme) {
        self.theme = theme;
    }

    /// Get the current theme.
    pub fn theme(&self) -> &UiTheme {
        &self.theme
    }

    /// Set the current layout.
    pub fn set_layout(&mut self, layout: UiLayout) {
        self.current_layout = Some(layout);
    }

    /// Get the current layout.
    pub fn current_layout(&self) -> Option<&UiLayout> {
        self.current_layout.as_ref()
    }

    /// Set refresh rate.
    pub fn set_refresh_rate(&mut self, rate: Duration) {
        self.refresh_rate = rate;
    }

    /// Get refresh rate.
    pub fn refresh_rate(&self) -> Duration {
        self.refresh_rate
    }

    /// Set target FPS.
    pub fn set_fps_target(&mut self, fps: u32) {
        self.fps_target = fps;
        // Update refresh rate to match FPS target.
        self.refresh_rate = Duration::from_nanos(1_000_000_000 / fps as u64);
    }

    /// Get target FPS.
    pub fn fps_target(&self) -> u32 {
        self.fps_target
    }

    /// Increment render count.
    pub fn increment_render_count(&mut self) {
        self.render_count += 1;
    }

    /// Get render count.
    pub fn render_count(&self) -> u64 {
        self.render_count
    }

    /// Initialize the UI framework.
    pub fn initialize(&mut self) -> bool {
        // In production: initialize Slint or other backend.
        // For testing: return true.
        true
    }

    /// Start the UI event loop.
    /// Returns after `duration` of simulated time.
    pub fn start_event_loop(&mut self, duration: Duration) -> u64 {
        if !self.initialize() {
            return 0;
        }

        let interval_nanos = self.refresh_rate.as_nanos();
        let duration_nanos = duration.as_nanos();
        let frames = duration_nanos / interval_nanos;

        for _ in 0..frames {
            self.render();
        }

        frames as u64
    }

    /// Render the current frame.
    pub fn render(&mut self) {
        self.render_count += 1;
        // In production: render current layout to screen.
    }

    /// Get layout preview (simulated).
    pub fn get_layout_preview(&self) -> Option<String> {
        self.current_layout.as_ref().map(|layout| {
            format!(
                "Layout '{}' with {} elements, mode: {}",
                layout.name,
                layout.element_count(),
                layout.layout_mode
            )
        })
    }

    /// Validate layout.
    pub fn validate_layout(&self) -> bool {
        match &self.current_layout {
            None => false,
            Some(layout) => {
                // Must have at least one deck.
                layout
                    .elements
                    .iter()
                    .any(|e| matches!(e.element_type, UiElementType::Deck))
            }
        }
    }

    /// Clone layout.
    pub fn clone_layout(&self, name: &str) -> Option<UiLayout> {
        self.current_layout.as_ref().map(|layout| {
            let mut new_layout = UiLayout::new(name.to_string());
            for element in &layout.elements {
                let cloned = element.clone();
                new_layout.add_element(cloned);
            }
            new_layout
        })
    }
}

impl Default for UiFramework {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_framework_creation() {
        let framework = UiFramework::new();
        assert_eq!(framework.backend(), UiBackend::Slint);
        assert_eq!(framework.fps_target(), 60);
        assert_eq!(framework.render_count(), 0);
    }

    #[test]
    fn ui_framework_set_backend() {
        let mut framework = UiFramework::new();
        framework.set_backend(UiBackend::Future);
        assert_eq!(framework.backend(), UiBackend::Future);
    }

    #[test]
    fn ui_framework_set_theme() {
        let mut framework = UiFramework::new();
        framework.set_theme(UiTheme::Light);
        assert!(matches!(framework.theme(), UiTheme::Light));

        framework.set_theme(UiTheme::Custom {
            primary: "#ff0000".to_string(),
            secondary: "#00ff00".to_string(),
            background: "#000000".to_string(),
        });
        assert!(matches!(framework.theme(), UiTheme::Custom { .. }));
    }

    #[test]
    fn ui_framework_initialize() {
        let mut framework = UiFramework::new();
        assert!(framework.initialize());
    }

    #[test]
    fn ui_framework_set_layout() {
        let mut framework = UiFramework::new();
        let layout = layouts::default_dual_deck();
        framework.set_layout(layout.clone());

        assert!(framework.current_layout().is_some());
        assert_eq!(framework.current_layout().unwrap().name, "Dual Deck");
    }

    #[test]
    fn ui_framework_validate_layout() {
        let mut framework = UiFramework::new();

        // No layout — should fail.
        assert!(!framework.validate_layout());

        // Valid dual-deck layout.
        let layout = layouts::default_dual_deck();
        framework.set_layout(layout);
        assert!(framework.validate_layout());
    }

    #[test]
    fn ui_framework_layout_preview() {
        let mut framework = UiFramework::new();
        let layout = layouts::single_deck();
        framework.set_layout(layout);

        let preview = framework.get_layout_preview();
        assert!(preview.is_some());
        assert!(preview.unwrap().contains("Single Deck"));
    }

    #[test]
    fn ui_framework_set_fps_target() {
        let mut framework = UiFramework::new();
        framework.set_fps_target(30);
        assert_eq!(framework.fps_target(), 30);
        assert_eq!(
            framework.refresh_rate(),
            Duration::from_nanos(1_000_000_000 / 30)
        );
    }

    #[test]
    fn ui_framework_render_count() {
        let mut framework = UiFramework::new();
        assert_eq!(framework.render_count(), 0);

        framework.render();
        assert_eq!(framework.render_count(), 1);

        framework.render();
        assert_eq!(framework.render_count(), 2);
    }

    #[test]
    fn ui_framework_clone_layout() {
        let mut framework = UiFramework::new();
        let original = layouts::full_analysis();
        framework.set_layout(original);

        let cloned = framework.clone_layout("Analysis Copy");
        assert!(cloned.is_some());
        assert_eq!(cloned.unwrap().name, "Analysis Copy");
    }

    #[test]
    fn ui_element_state_creation() {
        let state = UiElementState::new(UiElementType::Deck);
        assert!(state.visible);
        assert!(state.enabled);
        assert_eq!(state.size, (0.0, 0.0));
    }

    #[test]
    fn ui_element_state_setters() {
        let mut state = UiElementState::new(UiElementType::Deck);
        state.set_size(600.0, 800.0);
        assert_eq!(state.size, (600.0, 800.0));

        state.set_position(100.0, 200.0);
        assert_eq!(state.position, (100.0, 200.0));

        state.set_visible(false);
        assert!(!state.visible);

        state.set_enabled(false);
        assert!(!state.enabled);

        state.set_property("key".to_string(), "value".to_string());
        assert_eq!(state.properties.get("key").unwrap(), "value");
    }

    #[test]
    fn ui_layout_creation() {
        let layout = UiLayout::new("Test".to_string());
        assert_eq!(layout.name, "Test");
        assert_eq!(layout.element_count(), 0);
    }

    #[test]
    fn ui_layout_add_element() {
        let mut layout = UiLayout::new("Test".to_string());
        layout.add_element(UiElementState::new(UiElementType::Deck));
        layout.add_element(UiElementState::new(UiElementType::Deck));

        assert_eq!(layout.element_count(), 2);
    }

    #[test]
    fn ui_layout_get_element() {
        let mut layout = UiLayout::new("Test".to_string());
        layout.add_element(UiElementState::new(UiElementType::Deck));
        layout.add_element(UiElementState::new(UiElementType::MixerChannel));

        assert!(layout.get_element(&UiElementType::Deck).is_some());
        assert!(layout.get_element(&UiElementType::MixerChannel).is_some());
        assert!(layout.get_element(&UiElementType::Waveform).is_none());
    }

    #[test]
    fn ui_layout_remove_element() {
        let mut layout = UiLayout::new("Test".to_string());
        layout.add_element(UiElementState::new(UiElementType::Deck));
        layout.add_element(UiElementState::new(UiElementType::MixerChannel));

        let removed = layout.remove_element(&UiElementType::Deck);
        assert!(removed.is_some());
        assert_eq!(layout.element_count(), 1);
    }

    #[test]
    fn ui_preset_default_dual_deck() {
        let layout = layouts::default_dual_deck();
        assert_eq!(layout.name, "Dual Deck");
        assert_eq!(layout.element_count(), 3); // 2 decks + 1 mixer
    }

    #[test]
    fn ui_preset_single_deck() {
        let layout = layouts::single_deck();
        assert_eq!(layout.name, "Single Deck");
        assert_eq!(layout.element_count(), 2); // 1 deck + 1 mixer
    }

    #[test]
    fn ui_preset_full_analysis() {
        let layout = layouts::full_analysis();
        assert_eq!(layout.name, "Full Analysis");
        assert_eq!(layout.element_count(), 5);
    }

    #[test]
    fn ui_event_loop_simulation() {
        let mut framework = UiFramework::new();

        // Run for 100ms at 60 FPS (~16ms per frame).
        let frames = framework.start_event_loop(Duration::from_millis(100));

        // Should render approximately 6 frames.
        assert!(frames >= 5);
        assert!(frames <= 10);
        assert_eq!(framework.render_count(), frames);
    }

    #[test]
    fn ui_backend_default() {
        assert_eq!(UiBackend::default(), UiBackend::Slint);
    }

    #[test]
    fn ui_theme_default() {
        assert!(matches!(UiTheme::default(), UiTheme::Dark));
    }
}
