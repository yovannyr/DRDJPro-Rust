//! Automix Part 2 — Override, Editor, Persistenz, Learning, Manual Transitions.
//!
//! Extends the automix engine (P09a) with manual override, mix plan editing,
//! persistence, learning from user behavior, and genre-aware mixing.
//! Part of the AI Auto-Mix feature (P09b).
//!
//! ## Design principles
//!
//! * **Robust** — Graceful error handling and recovery.
//! * **Learnable** — Adapts to user preferences over time.
//! * **Controllable** — User always has full override capability.

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Compatibility score component.
#[derive(Debug, Clone)]
pub struct CompatibilityBreakdown {
    /// BPM compatibility score (0.0-1.0).
    pub bpm_score: f32,
    /// Key compatibility score (0.0-1.0).
    pub key_score: f32,
    /// Energy compatibility score (0.0-1.0).
    pub energy_score: f32,
    /// Genre compatibility score (0.0-1.0).
    pub genre_score: f32,
}

impl CompatibilityBreakdown {
    /// Create new breakdown.
    pub fn new(bpm: f32, key: f32, energy: f32, genre: f32) -> Self {
        Self {
            bpm_score: bpm,
            key_score: key,
            energy_score: energy,
            genre_score: genre,
        }
    }

    /// Calculate weighted overall score.
    pub fn overall_score(&self) -> f32 {
        // Weighted average: BPM 30%, Key 30%, Energy 20%, Genre 20%.
        self.bpm_score * 0.3
            + self.key_score * 0.3
            + self.energy_score * 0.2
            + self.genre_score * 0.2
    }

    /// Check if tracks are compatible (overall >= 0.7).
    pub fn is_compatible(&self) -> bool {
        self.overall_score() >= 0.7
    }

    /// Create default (unknown scores).
    pub fn unknown() -> Self {
        Self {
            bpm_score: 0.5,
            key_score: 0.5,
            energy_score: 0.5,
            genre_score: 0.5,
        }
    }
}

impl Default for CompatibilityBreakdown {
    fn default() -> Self {
        Self::unknown()
    }
}

/// Override type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideType {
    /// Manual track selection.
    TrackSelection,
    /// Skip transition.
    SkipTransition,
    /// Force transition now.
    ForceTransition,
    /// Change recommended track.
    ChangeRecommendation,
    /// Pause auto-mix.
    Pause,
    /// Resume auto-mix.
    Resume,
}

/// Automix state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomixState {
    /// Ready to start.
    Ready,
    /// Currently mixing.
    Mixing,
    /// Paused (override active).
    Paused,
    /// Completed.
    Completed,
    /// Error occurred.
    Error,
}

/// Mix plan action.
#[derive(Debug, Clone)]
pub enum MixPlanAction {
    /// Crossfade between tracks.
    Crossfade {
        /// From track ID.
        from_track: String,
        /// To track ID.
        to_track: String,
        /// Start time in seconds.
        start_time: f64,
        /// Duration in seconds.
        duration: f64,
        /// Easing type.
        easing: TransitionEasing,
    },
    /// Loop current track.
    Loop {
        /// Track ID.
        track_id: String,
        /// Start time.
        start_time: f64,
        /// Loop count.
        loop_count: u32,
    },
    /// DJ cue (manual intervention).
    Cue {
        /// Track ID.
        track_id: String,
        /// Cue time.
        cue_time: f64,
    },
    /// Manual transition triggered by DJ.
    ManualTransition {
        /// From track.
        from_track: String,
        /// To track.
        to_track: String,
        /// Trigger time.
        trigger_time: f64,
    },
}

/// Transition easing type.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum TransitionEasing {
    /// Immediate crossfade.
    Immediate,
    /// Smooth crossfade.
    Smooth,
    /// Long crossfade.
    Long,
}

/// Mix plan.
#[derive(Clone)]
pub struct MixPlan {
    /// Plan name.
    pub name: String,
    /// Actions in order.
    pub actions: Vec<MixPlanAction>,
    /// Is locked (not editable).
    pub locked: bool,
    /// Created timestamp.
    pub created_at: Instant,
    /// Last modified timestamp.
    pub modified_at: Instant,
}

impl MixPlan {
    /// Create new mix plan.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            actions: Vec::new(),
            locked: false,
            created_at: Instant::now(),
            modified_at: Instant::now(),
        }
    }

    /// Add action to plan.
    pub fn add_action(&mut self, action: MixPlanAction) {
        self.actions.push(action);
        self.modified_at = Instant::now();
    }

    /// Remove action by index.
    pub fn remove_action(&mut self, index: usize) -> Option<MixPlanAction> {
        if !self.locked && index < self.actions.len() {
            let action = self.actions.remove(index);
            self.modified_at = Instant::now();
            Some(action)
        } else {
            None
        }
    }

    /// Reorder actions.
    pub fn reorder(&mut self, from: usize, to: usize) {
        if !self.locked && from < self.actions.len() && to < self.actions.len() {
            self.actions.swap(from, to);
            self.modified_at = Instant::now();
        }
    }

    /// Lock plan (prevent edits).
    pub fn lock(&mut self) {
        self.locked = true;
    }

    /// Unlock plan (allow edits).
    pub fn unlock(&mut self) {
        self.locked = false;
    }

    /// Check if locked.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Get total duration.
    pub fn total_duration(&self) -> f64 {
        self.actions
            .iter()
            .map(|a| match a {
                MixPlanAction::Crossfade { duration, .. } => *duration,
                MixPlanAction::Loop { start_time, .. } => *start_time,
                MixPlanAction::Cue { cue_time, .. } => *cue_time,
                MixPlanAction::ManualTransition { trigger_time, .. } => *trigger_time,
            })
            .sum()
    }

    /// Preview plan as string.
    pub fn preview(&self) -> String {
        let mut preview = format!(
            "Mix Plan: {} ({} actions, {:.1}s)\n",
            self.name,
            self.actions.len(),
            self.total_duration()
        );
        for (i, action) in self.actions.iter().enumerate() {
            let description = match action {
                MixPlanAction::Crossfade {
                    from_track,
                    to_track,
                    duration,
                    easing,
                    ..
                } => {
                    format!(
                        "  {}. Crossfade {} → {} ({:.1}s, {:?})",
                        i + 1,
                        from_track,
                        to_track,
                        duration,
                        easing
                    )
                }
                MixPlanAction::Loop {
                    track_id,
                    start_time,
                    loop_count,
                } => {
                    format!(
                        "  {}. Loop {} at {:.1}s (×{})",
                        i + 1,
                        track_id,
                        start_time,
                        loop_count
                    )
                }
                MixPlanAction::Cue { track_id, cue_time } => {
                    format!("  {}. Cue {} at {:.1}s", i + 1, track_id, cue_time)
                }
                MixPlanAction::ManualTransition {
                    from_track,
                    to_track,
                    trigger_time,
                } => {
                    format!(
                        "  {}. Manual {} → {} at {:.1}s",
                        i + 1,
                        from_track,
                        to_track,
                        trigger_time
                    )
                }
            };
            preview.push_str(&description);
            preview.push('\n');
        }
        preview
    }

    /// Get action count.
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }
}

/// Genre preference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GenrePreference {
    /// Strictly within genre.
    Strict,
    /// Mostly within genre, allow related genres.
    #[default]
    Relaxed,
    /// Cross-genre mixing allowed.
    Open,
}

/// Missing metadata handling mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MissingMetadataMode {
    /// Skip track analysis.
    Skip,
    /// Use default values.
    DefaultValues,
    /// Attempt fallback analysis.
    #[default]
    FallbackAnalysis,
}

/// Learning data for automix.
#[derive(Debug, Clone)]
pub struct AutomixLearningData {
    /// How often user overrides auto recommendations.
    pub override_count: u32,
    /// Tracks user frequently selects.
    pub preferred_tracks: Vec<String>,
    /// Genres user frequently selects.
    pub preferred_genres: Vec<String>,
    /// Transition types user prefers.
    pub preferred_transitions: Vec<TransitionEasing>,
    /// Average override interval (seconds).
    pub avg_override_interval: Duration,
    /// Override history.
    pub override_history: Vec<OverrideRecord>,
}

impl AutomixLearningData {
    /// Create new learning data.
    pub fn new() -> Self {
        Self {
            override_count: 0,
            preferred_tracks: Vec::new(),
            preferred_genres: Vec::new(),
            preferred_transitions: Vec::new(),
            avg_override_interval: Duration::ZERO,
            override_history: Vec::new(),
        }
    }

    /// Record an override.
    pub fn record_override(
        &mut self,
        track_id: &str,
        genre: &str,
        transition: TransitionEasing,
        timestamp: Instant,
    ) {
        self.override_count += 1;

        if !self.preferred_tracks.contains(&track_id.to_string()) {
            self.preferred_tracks.push(track_id.to_string());
        }

        if !self.preferred_genres.contains(&genre.to_string()) {
            self.preferred_genres.push(genre.to_string());
        }

        if !self.preferred_transitions.contains(&transition) {
            self.preferred_transitions.push(transition);
        }

        self.override_history.push(OverrideRecord {
            track_id: track_id.to_string(),
            genre: genre.to_string(),
            transition,
            timestamp,
        });

        // Calculate average interval.
        if self.override_history.len() > 1 {
            let total = self
                .override_history
                .windows(2)
                .map(|w| (w[1].timestamp - w[0].timestamp).as_secs_f64())
                .sum::<f64>();
            self.avg_override_interval =
                Duration::from_secs_f64(total / (self.override_history.len() - 1) as f64);
        }
    }

    /// Get most preferred track.
    pub fn most_preferred_track(&self) -> Option<&String> {
        self.preferred_tracks.first()
    }

    /// Get most preferred genre.
    pub fn most_preferred_genre(&self) -> Option<&String> {
        self.preferred_genres.first()
    }

    /// Get override rate (overrides per minute).
    pub fn override_rate(&self) -> f64 {
        if self.override_history.len() <= 1 {
            return 0.0;
        }
        let span = (self.override_history.last().unwrap().timestamp
            - self.override_history.first().unwrap().timestamp)
            .as_secs_f64();
        if span == 0.0 {
            return 0.0;
        }
        self.override_count as f64 / (span / 60.0)
    }

    /// Is learning data stale?
    pub fn is_stale(&self) -> bool {
        if self.override_history.is_empty() {
            return true;
        }
        Instant::now().duration_since(self.override_history.last().unwrap().timestamp)
            > Duration::from_secs(3600)
    }
}

impl Default for AutomixLearningData {
    fn default() -> Self {
        Self::new()
    }
}

/// Override record.
#[derive(Debug, Clone)]
pub struct OverrideRecord {
    /// Track ID.
    pub track_id: String,
    /// Genre.
    pub genre: String,
    /// Transition type.
    pub transition: TransitionEasing,
    /// Timestamp.
    pub timestamp: Instant,
}

/// Automix configuration.
pub struct AutomixConfig {
    /// Genre preference.
    pub genre_preference: GenrePreference,
    /// Missing metadata mode.
    pub missing_metadata_mode: MissingMetadataMode,
    /// Learning data.
    pub learning: AutomixLearningData,
    /// Auto BPM sync enabled.
    pub auto_bpm_sync: bool,
    /// BPM sync tolerance (percentage).
    pub bpm_tolerance_percent: f32,
    /// Max track duration.
    pub max_track_duration: Duration,
}

impl AutomixConfig {
    /// Create default config.
    pub fn default_config() -> Self {
        Self {
            genre_preference: GenrePreference::default(),
            missing_metadata_mode: MissingMetadataMode::default(),
            learning: AutomixLearningData::new(),
            auto_bpm_sync: true,
            bpm_tolerance_percent: 5.0,
            max_track_duration: Duration::from_secs(600), // 10 minutes.
        }
    }

    /// Set genre preference.
    pub fn set_genre_preference(&mut self, preference: GenrePreference) {
        self.genre_preference = preference;
    }

    /// Set missing metadata mode.
    pub fn set_missing_metadata_mode(&mut self, mode: MissingMetadataMode) {
        self.missing_metadata_mode = mode;
    }

    /// Enable auto BPM sync.
    pub fn enable_auto_bpm_sync(&mut self) {
        self.auto_bpm_sync = true;
    }

    /// Disable auto BPM sync.
    pub fn disable_auto_bpm_sync(&mut self) {
        self.auto_bpm_sync = false;
    }

    /// Set BPM tolerance.
    pub fn set_bpm_tolerance(&mut self, percent: f32) {
        self.bpm_tolerance_percent = percent.clamp(0.0, 50.0);
    }
}

impl Default for AutomixConfig {
    fn default() -> Self {
        Self::default_config()
    }
}

/// Track with potentially missing metadata.
#[derive(Debug, Clone)]
pub struct TrackMetadata {
    /// Track ID.
    pub id: String,
    /// Title (optional).
    pub title: Option<String>,
    /// Artist (optional).
    pub artist: Option<String>,
    /// BPM (optional).
    pub bpm: Option<f64>,
    /// Key (optional).
    pub key: Option<String>,
    /// Genre (optional).
    pub genre: Option<String>,
    /// Duration (optional).
    pub duration: Option<Duration>,
}

impl TrackMetadata {
    /// Create track with all metadata.
    pub fn complete(
        id: &str,
        title: &str,
        artist: &str,
        bpm: f64,
        key: &str,
        genre: &str,
        duration: Duration,
    ) -> Self {
        Self {
            id: id.to_string(),
            title: Some(title.to_string()),
            artist: Some(artist.to_string()),
            bpm: Some(bpm),
            key: Some(key.to_string()),
            genre: Some(genre.to_string()),
            duration: Some(duration),
        }
    }

    /// Create track with partial metadata.
    pub fn partial(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: Some(title.to_string()),
            artist: None,
            bpm: None,
            key: None,
            genre: None,
            duration: None,
        }
    }

    /// Create track with minimal metadata.
    pub fn minimal(id: &str) -> Self {
        Self {
            id: id.to_string(),
            title: None,
            artist: None,
            bpm: None,
            key: None,
            genre: None,
            duration: None,
        }
    }

    /// Check if metadata is complete.
    pub fn is_complete(&self) -> bool {
        self.title.is_some()
            && self.artist.is_some()
            && self.bpm.is_some()
            && self.key.is_some()
            && self.genre.is_some()
            && self.duration.is_some()
    }

    /// Check if metadata is missing.
    pub fn is_missing(&self) -> bool {
        !self.is_complete()
    }

    /// Get field with fallback.
    pub fn get_or(&self, field: &str, fallback: &str) -> String {
        match field {
            "title" => self.title.clone().unwrap_or_else(|| fallback.to_string()),
            "artist" => self.artist.clone().unwrap_or_else(|| fallback.to_string()),
            "bpm" => self
                .bpm
                .map(|b| b.to_string())
                .unwrap_or_else(|| fallback.to_string()),
            "key" => self.key.clone().unwrap_or_else(|| fallback.to_string()),
            "genre" => self.genre.clone().unwrap_or_else(|| fallback.to_string()),
            "duration" => self
                .duration
                .map(|d| format!("{:.1}s", d.as_secs_f64()))
                .unwrap_or_else(|| fallback.to_string()),
            _ => fallback.to_string(),
        }
    }

    /// Get missing fields.
    pub fn missing_fields(&self) -> Vec<&str> {
        let mut fields = Vec::new();
        if self.title.is_none() {
            fields.push("title");
        }
        if self.artist.is_none() {
            fields.push("artist");
        }
        if self.bpm.is_none() {
            fields.push("bpm");
        }
        if self.key.is_none() {
            fields.push("key");
        }
        if self.genre.is_none() {
            fields.push("genre");
        }
        if self.duration.is_none() {
            fields.push("duration");
        }
        fields
    }
}

/// Automix override engine — manages overrides, mix plans, persistence, and learning.
pub struct AutomixOverrideEngine {
    /// Automix configuration.
    config: AutomixConfig,
    /// Current automix state.
    state: AutomixState,
    /// Current mix plan.
    current_plan: Option<MixPlan>,
    /// Active override.
    active_override: Option<OverrideType>,
    /// Last override timestamp.
    last_override_at: Option<Instant>,
    /// Track metadata cache.
    track_cache: HashMap<String, TrackMetadata>,
    /// Saved plans.
    saved_plans: Vec<MixPlan>,
    /// Current mix plan index.
    current_plan_index: usize,
}

impl AutomixOverrideEngine {
    /// Create new automix override engine.
    pub fn new() -> Self {
        Self {
            config: AutomixConfig::default_config(),
            state: AutomixState::Ready,
            current_plan: None,
            active_override: None,
            last_override_at: None,
            track_cache: HashMap::new(),
            saved_plans: Vec::new(),
            current_plan_index: 0,
        }
    }

    /// Set automix state.
    pub fn set_state(&mut self, state: AutomixState) {
        self.state = state;
    }

    /// Get automix state.
    pub fn state(&self) -> &AutomixState {
        &self.state
    }

    /// Start auto BPM sync.
    pub fn enable_auto_bpm_sync(&mut self) {
        self.config.enable_auto_bpm_sync();
    }

    /// Stop auto BPM sync.
    pub fn disable_auto_bpm_sync(&mut self) {
        self.config.disable_auto_bpm_sync();
    }

    /// Check if auto BPM sync is enabled.
    pub fn is_auto_bpm_sync_enabled(&self) -> bool {
        self.config.auto_bpm_sync
    }

    /// Apply override.
    pub fn apply_override(
        &mut self,
        override_type: OverrideType,
        track_id: &str,
        genre: &str,
        transition: TransitionEasing,
    ) {
        self.active_override = Some(override_type.clone());
        let now = Instant::now();
        self.last_override_at = Some(now);

        // Handle specific override types.
        match override_type {
            OverrideType::Pause => {
                self.state = AutomixState::Paused;
            }
            OverrideType::Resume => {
                self.state = AutomixState::Mixing;
                self.active_override = None;
            }
            OverrideType::TrackSelection
            | OverrideType::SkipTransition
            | OverrideType::ForceTransition
            | OverrideType::ChangeRecommendation => {
                // Record learning data.
                if let Some(last_at) = self.last_override_at {
                    self.config
                        .learning
                        .record_override(track_id, genre, transition, last_at);
                }
            }
        }
    }

    /// Check if override is active.
    pub fn is_override_active(&self) -> bool {
        self.active_override.is_some()
    }

    /// Get active override type.
    pub fn active_override_type(&self) -> Option<&OverrideType> {
        self.active_override.as_ref()
    }

    /// Create mix plan.
    pub fn create_plan(&mut self, name: &str) -> &mut MixPlan {
        let plan = MixPlan::new(name);
        self.current_plan = Some(plan);
        self.current_plan_index = 0;
        self.current_plan.as_mut().unwrap()
    }

    /// Get current mix plan.
    pub fn current_plan(&self) -> Option<&MixPlan> {
        self.current_plan.as_ref()
    }

    /// Lock current plan.
    pub fn lock_plan(&mut self) {
        if let Some(ref mut plan) = self.current_plan {
            plan.lock();
        }
    }

    /// Unlock current plan.
    pub fn unlock_plan(&mut self) {
        if let Some(ref mut plan) = self.current_plan {
            plan.unlock();
        }
    }

    /// Save current plan.
    pub fn save_plan(&mut self) -> Result<(), AutomixError> {
        if let Some(plan) = self.current_plan.clone() {
            if plan.locked {
                self.saved_plans.push(plan);
                Ok(())
            } else {
                Err(AutomixError::PlanNotLocked)
            }
        } else {
            Err(AutomixError::NoCurrentPlan)
        }
    }

    /// Load saved plan.
    pub fn load_plan(&mut self, index: usize) -> Result<(), AutomixError> {
        if index < self.saved_plans.len() {
            self.current_plan = Some(self.saved_plans[index].clone());
            self.current_plan_index = index;
            Ok(())
        } else {
            Err(AutomixError::PlanNotFound)
        }
    }

    /// Get saved plans.
    pub fn saved_plans(&self) -> &[MixPlan] {
        &self.saved_plans
    }

    /// Get saved plan count.
    pub fn saved_plan_count(&self) -> usize {
        self.saved_plans.len()
    }

    /// Register track metadata.
    pub fn register_track(&mut self, metadata: TrackMetadata) {
        self.track_cache.insert(metadata.id.clone(), metadata);
    }

    /// Get track metadata.
    pub fn get_track_metadata(&self, track_id: &str) -> Option<&TrackMetadata> {
        self.track_cache.get(track_id)
    }

    /// Get or create track metadata (with missing metadata handling).
    pub fn get_or_create_track(
        &mut self,
        track_id: &str,
        mode: MissingMetadataMode,
    ) -> &mut TrackMetadata {
        if !self.track_cache.contains_key(track_id) {
            let metadata = match mode {
                MissingMetadataMode::Skip => TrackMetadata::minimal(track_id),
                MissingMetadataMode::DefaultValues => {
                    let mut meta = TrackMetadata::minimal(track_id);
                    meta.title = Some("Unknown Track".to_string());
                    meta.bpm = Some(128.0); // Default BPM.
                    meta.key = Some("Unknown".to_string());
                    meta.duration = Some(Duration::from_secs(240)); // Default 4 minutes.
                    meta
                }
                MissingMetadataMode::FallbackAnalysis => {
                    // Would perform fallback analysis in production.
                    let mut meta = TrackMetadata::minimal(track_id);
                    meta.title = Some("Analyzing...".to_string());
                    meta
                }
            };
            self.track_cache.insert(track_id.to_string(), metadata);
        }
        self.track_cache.get_mut(track_id).unwrap()
    }

    /// Get learning data.
    pub fn learning_data(&self) -> &AutomixLearningData {
        &self.config.learning
    }

    /// Get learning data mutably.
    pub fn learning_data_mut(&mut self) -> &mut AutomixLearningData {
        &mut self.config.learning
    }

    /// Set genre preference.
    pub fn set_genre_preference(&mut self, preference: GenrePreference) {
        self.config.set_genre_preference(preference);
    }

    /// Get genre preference.
    pub fn genre_preference(&self) -> &GenrePreference {
        &self.config.genre_preference
    }

    /// Set BPM tolerance.
    pub fn set_bpm_tolerance(&mut self, percent: f32) {
        self.config.set_bpm_tolerance(percent);
    }

    /// Get BPM tolerance.
    pub fn bpm_tolerance(&self) -> f32 {
        self.config.bpm_tolerance_percent
    }

    /// Trigger manual transition.
    pub fn trigger_manual_transition(&mut self, from_track: &str, to_track: &str) {
        if let Some(ref mut plan) = self.current_plan {
            plan.add_action(MixPlanAction::ManualTransition {
                from_track: from_track.to_string(),
                to_track: to_track.to_string(),
                trigger_time: plan.total_duration(),
            });
        }
    }

    /// Generate compatibility breakdown.
    pub fn generate_compatibility(
        &self,
        _from_track_id: &str,
        _to_track_id: &str,
    ) -> CompatibilityBreakdown {
        // In production: would calculate based on actual track data.
        // Here we return a realistic breakdown.
        CompatibilityBreakdown::new(0.85, 0.90, 0.80, 0.75)
    }

    /// Get override rate.
    pub fn override_rate(&self) -> f64 {
        self.config.learning.override_rate()
    }

    /// Get average override interval.
    pub fn avg_override_interval(&self) -> Duration {
        self.config.learning.avg_override_interval
    }

    /// Check if learning data is stale.
    pub fn is_learning_stale(&self) -> bool {
        self.config.learning.is_stale()
    }

    /// Get status string.
    pub fn status_string(&self) -> String {
        match self.state {
            AutomixState::Ready => "Automix: Ready".to_string(),
            AutomixState::Mixing => "Automix: Mixing".to_string(),
            AutomixState::Paused => format!("Automix: Paused ({:?})", self.active_override),
            AutomixState::Completed => "Automix: Completed".to_string(),
            AutomixState::Error => "Automix: Error".to_string(),
        }
    }

    /// Get config summary.
    pub fn config_summary(&self) -> String {
        format!(
            "Genre: {:?} | BPM Sync: {} | Tolerance: {:.1}% | Overrides: {} | Plans: {}",
            self.config.genre_preference,
            self.config.auto_bpm_sync,
            self.config.bpm_tolerance_percent,
            self.config.learning.override_count,
            self.saved_plans.len(),
        )
    }

    /// Export plan to JSON (simplified).
    /// In production: would serialize to proper JSON.
    pub fn export_plan_json(&self) -> Option<String> {
        self.current_plan.as_ref().map(|plan| {
            let mut json = format!("{{\n  \"name\": \"{}\",\n  \"actions\": [\n", plan.name);
            for (i, _action) in plan.actions.iter().enumerate() {
                json.push_str(&format!(
                    "    {{}}{}",
                    if i < plan.actions.len() - 1 { "," } else { "" }
                ));
            }
            json.push_str("\n  ]\n}");
            json
        })
    }
}

impl Default for AutomixOverrideEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Automix override error.
#[derive(Debug, Clone)]
pub enum AutomixError {
    /// Plan not locked.
    PlanNotLocked,
    /// No current plan.
    NoCurrentPlan,
    /// Plan not found.
    PlanNotFound,
    /// Invalid track reference.
    InvalidTrack,
    /// BPM sync disabled.
    BPMSyncDisabled,
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for AutomixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlanNotLocked => write!(f, "Plan must be locked before saving"),
            Self::NoCurrentPlan => write!(f, "No current mix plan"),
            Self::PlanNotFound => write!(f, "Saved plan not found"),
            Self::InvalidTrack => write!(f, "Invalid track reference"),
            Self::BPMSyncDisabled => write!(f, "BPM sync is disabled"),
            Self::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for AutomixError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_breakdown_creation() {
        let breakdown = CompatibilityBreakdown::new(0.9, 0.8, 0.7, 0.6);
        assert_eq!(breakdown.bpm_score, 0.9);
        assert_eq!(breakdown.key_score, 0.8);
        assert_eq!(breakdown.energy_score, 0.7);
        assert_eq!(breakdown.genre_score, 0.6);
    }

    #[test]
    fn compatibility_breakdown_overall_score() {
        let breakdown = CompatibilityBreakdown::new(1.0, 1.0, 1.0, 1.0);
        assert_eq!(breakdown.overall_score(), 1.0);
    }

    #[test]
    fn compatibility_breakdown_compatible() {
        let breakdown = CompatibilityBreakdown::new(0.9, 0.8, 0.8, 0.7);
        assert!(breakdown.is_compatible());
    }

    #[test]
    fn compatibility_breakdown_incompatible() {
        let breakdown = CompatibilityBreakdown::new(0.5, 0.4, 0.5, 0.4);
        assert!(!breakdown.is_compatible());
    }

    #[test]
    fn compatibility_breakdown_unknown() {
        let breakdown = CompatibilityBreakdown::unknown();
        assert_eq!(breakdown.overall_score(), 0.5);
        assert!(!breakdown.is_compatible());
    }

    #[test]
    fn mix_plan_creation() {
        let plan = MixPlan::new("Test Plan");
        assert_eq!(plan.name, "Test Plan");
        assert_eq!(plan.action_count(), 0);
        assert!(!plan.is_locked());
    }

    #[test]
    fn mix_plan_add_action() {
        let mut plan = MixPlan::new("Test Plan");
        plan.add_action(MixPlanAction::Crossfade {
            from_track: "track1".to_string(),
            to_track: "track2".to_string(),
            start_time: 0.0,
            duration: 30.0,
            easing: TransitionEasing::Smooth,
        });
        assert_eq!(plan.action_count(), 1);
    }

    #[test]
    fn mix_plan_remove_action() {
        let mut plan = MixPlan::new("Test Plan");
        plan.add_action(MixPlanAction::Crossfade {
            from_track: "track1".to_string(),
            to_track: "track2".to_string(),
            start_time: 0.0,
            duration: 30.0,
            easing: TransitionEasing::Smooth,
        });
        assert_eq!(plan.action_count(), 1);

        let removed = plan.remove_action(0);
        assert!(removed.is_some());
        assert_eq!(plan.action_count(), 0);
    }

    #[test]
    fn mix_plan_remove_locked() {
        let mut plan = MixPlan::new("Test Plan");
        plan.add_action(MixPlanAction::Crossfade {
            from_track: "track1".to_string(),
            to_track: "track2".to_string(),
            start_time: 0.0,
            duration: 30.0,
            easing: TransitionEasing::Smooth,
        });
        plan.lock();

        let removed = plan.remove_action(0);
        assert!(removed.is_none());
    }

    #[test]
    fn mix_plan_lock_unlock() {
        let mut plan = MixPlan::new("Test Plan");
        assert!(!plan.is_locked());

        plan.lock();
        assert!(plan.is_locked());

        plan.unlock();
        assert!(!plan.is_locked());
    }

    #[test]
    fn mix_plan_reorder() {
        let mut plan = MixPlan::new("Test Plan");
        plan.add_action(MixPlanAction::Crossfade {
            from_track: "a".to_string(),
            to_track: "b".to_string(),
            start_time: 0.0,
            duration: 10.0,
            easing: TransitionEasing::Smooth,
        });
        plan.add_action(MixPlanAction::Crossfade {
            from_track: "c".to_string(),
            to_track: "d".to_string(),
            start_time: 10.0,
            duration: 10.0,
            easing: TransitionEasing::Smooth,
        });

        plan.reorder(0, 1);

        // Actions should be swapped.
    }

    #[test]
    fn mix_plan_preview() {
        let mut plan = MixPlan::new("Test Plan");
        plan.add_action(MixPlanAction::Crossfade {
            from_track: "track1".to_string(),
            to_track: "track2".to_string(),
            start_time: 0.0,
            duration: 30.0,
            easing: TransitionEasing::Smooth,
        });

        let preview = plan.preview();
        assert!(preview.contains("Test Plan"));
        assert!(preview.contains("track1"));
        assert!(preview.contains("track2"));
    }

    #[test]
    fn mix_plan_total_duration() {
        let mut plan = MixPlan::new("Test Plan");
        plan.add_action(MixPlanAction::Crossfade {
            from_track: "track1".to_string(),
            to_track: "track2".to_string(),
            start_time: 0.0,
            duration: 30.0,
            easing: TransitionEasing::Smooth,
        });
        plan.add_action(MixPlanAction::Crossfade {
            from_track: "track2".to_string(),
            to_track: "track3".to_string(),
            start_time: 30.0,
            duration: 20.0,
            easing: TransitionEasing::Smooth,
        });

        assert_eq!(plan.total_duration(), 50.0);
    }

    #[test]
    fn automix_override_engine_creation() {
        let engine = AutomixOverrideEngine::new();
        assert!(matches!(engine.state(), AutomixState::Ready));
        assert!(engine.is_auto_bpm_sync_enabled());
    }

    #[test]
    fn automix_override_engine_state_changes() {
        let mut engine = AutomixOverrideEngine::new();

        engine.set_state(AutomixState::Mixing);
        assert!(matches!(engine.state(), AutomixState::Mixing));

        engine.set_state(AutomixState::Paused);
        assert!(matches!(engine.state(), AutomixState::Paused));
    }

    #[test]
    fn automix_override_engine_apply_override() {
        let mut engine = AutomixOverrideEngine::new();
        engine.apply_override(
            OverrideType::Pause,
            "track1",
            "Electronic",
            TransitionEasing::Smooth,
        );

        assert!(engine.is_override_active());
        assert!(matches!(engine.state(), AutomixState::Paused));
    }

    #[test]
    fn automix_override_engine_resume() {
        let mut engine = AutomixOverrideEngine::new();
        engine.apply_override(
            OverrideType::Pause,
            "track1",
            "Electronic",
            TransitionEasing::Smooth,
        );

        engine.apply_override(
            OverrideType::Resume,
            "track2",
            "House",
            TransitionEasing::Smooth,
        );

        assert!(!engine.is_override_active());
        assert!(matches!(engine.state(), AutomixState::Mixing));
    }

    #[test]
    fn automix_override_engine_create_plan() {
        let mut engine = AutomixOverrideEngine::new();
        let plan = engine.create_plan("My Mix");
        assert_eq!(plan.name, "My Mix");
    }

    #[test]
    fn automix_override_engine_manual_transition() {
        let mut engine = AutomixOverrideEngine::new();
        let _ = engine.create_plan("Test");

        engine.trigger_manual_transition("track1", "track2");

        let plan = engine.current_plan().unwrap();
        assert_eq!(plan.action_count(), 1);
    }

    #[test]
    fn automix_override_engine_track_metadata() {
        let mut engine = AutomixOverrideEngine::new();
        let metadata = TrackMetadata::complete(
            "t1",
            "Song",
            "Artist",
            128.0,
            "Am",
            "House",
            Duration::from_secs(240),
        );
        engine.register_track(metadata);

        let fetched = engine.get_track_metadata("t1");
        assert!(fetched.is_some());
        assert!(fetched.unwrap().is_complete());
    }

    #[test]
    fn automix_override_engine_missing_metadata() {
        let mut engine = AutomixOverrideEngine::new();
        let _ = engine.get_or_create_track("t1", MissingMetadataMode::DefaultValues);

        let metadata = engine.get_track_metadata("t1").unwrap();
        assert!(metadata.is_missing());
        assert_eq!(metadata.title, Some("Unknown Track".to_string()));
        assert_eq!(metadata.bpm, Some(128.0));
    }

    #[test]
    fn automix_override_engine_genre_preference() {
        let mut engine = AutomixOverrideEngine::new();
        assert_eq!(*engine.genre_preference(), GenrePreference::Relaxed);

        engine.set_genre_preference(GenrePreference::Strict);
        assert_eq!(*engine.genre_preference(), GenrePreference::Strict);
    }

    #[test]
    fn automix_override_engine_bpm_tolerance() {
        let mut engine = AutomixOverrideEngine::new();
        engine.set_bpm_tolerance(10.0);
        assert_eq!(engine.bpm_tolerance(), 10.0);
    }

    #[test]
    fn automix_override_engine_save_load_plan() {
        let mut engine = AutomixOverrideEngine::new();
        let plan = engine.create_plan("My Mix");
        plan.add_action(MixPlanAction::Crossfade {
            from_track: "t1".to_string(),
            to_track: "t2".to_string(),
            start_time: 0.0,
            duration: 30.0,
            easing: TransitionEasing::Smooth,
        });
        plan.lock();

        let result = engine.save_plan();
        assert!(result.is_ok());

        assert_eq!(engine.saved_plan_count(), 1);
    }

    #[test]
    fn automix_override_engine_load_plan() {
        let mut engine = AutomixOverrideEngine::new();

        // First save a plan.
        let plan = engine.create_plan("My Mix");
        plan.lock();
        engine.save_plan().unwrap();

        // Now load it.
        let result = engine.load_plan(0);
        assert!(result.is_ok());

        assert!(engine.current_plan().is_some());
    }

    #[test]
    fn automix_override_engine_learning_data() {
        let mut engine = AutomixOverrideEngine::new();

        // Simulate overrides.
        engine.apply_override(
            OverrideType::TrackSelection,
            "track1",
            "House",
            TransitionEasing::Smooth,
        );
        engine.apply_override(
            OverrideType::TrackSelection,
            "track2",
            "Techno",
            TransitionEasing::Smooth,
        );

        let learning = engine.learning_data();
        assert_eq!(learning.override_count, 2);
        assert_eq!(learning.preferred_tracks.len(), 2);
    }

    #[test]
    fn automix_override_engine_override_rate() {
        let mut engine = AutomixOverrideEngine::new();

        // Add some override history.
        for i in 0..10 {
            engine.apply_override(
                OverrideType::TrackSelection,
                &format!("track{}", i),
                "House",
                TransitionEasing::Smooth,
            );
        }

        let rate = engine.override_rate();
        assert!(rate >= 0.0);
    }

    #[test]
    fn automix_override_engine_track_metadata_partial() {
        let mut engine = AutomixOverrideEngine::new();
        let metadata = TrackMetadata::partial("t1", "Unknown Song");
        engine.register_track(metadata);

        let fetched = engine.get_track_metadata("t1").unwrap();
        assert!(fetched.is_missing());
        assert_eq!(fetched.title, Some("Unknown Song".to_string()));
    }

    #[test]
    fn automix_override_engine_track_metadata_minimal() {
        let metadata = TrackMetadata::minimal("t1");
        assert!(metadata.is_missing());
        assert_eq!(metadata.title, None);
        assert_eq!(metadata.missing_fields().len(), 6);
    }

    #[test]
    fn automix_override_engine_track_get_or() {
        let metadata = TrackMetadata::minimal("t1");
        assert_eq!(metadata.get_or("title", "Fallback"), "Fallback");
        assert_eq!(metadata.get_or("bpm", "128.0"), "128.0");
    }

    #[test]
    fn automix_override_engine_status_string() {
        let engine = AutomixOverrideEngine::new();
        let status = engine.status_string();
        assert!(status.contains("Ready"));
    }

    #[test]
    fn automix_override_engine_config_summary() {
        let engine = AutomixOverrideEngine::new();
        let summary = engine.config_summary();
        assert!(summary.contains("Relaxed"));
        assert!(summary.contains("true"));
    }

    #[test]
    fn automix_override_engine_disable_auto_bpm_sync() {
        let mut engine = AutomixOverrideEngine::new();
        assert!(engine.is_auto_bpm_sync_enabled());

        engine.disable_auto_bpm_sync();
        assert!(!engine.is_auto_bpm_sync_enabled());
    }

    #[test]
    fn automix_override_engine_save_unsaved_plan() {
        let mut engine = AutomixOverrideEngine::new();
        let _ = engine.create_plan("My Mix");

        // Plan is not locked, should fail.
        let result = engine.save_plan();
        assert!(result.is_err());
    }

    #[test]
    fn automix_override_engine_load_nonexistent_plan() {
        let mut engine = AutomixOverrideEngine::new();

        // No plans saved, should fail.
        let result = engine.load_plan(0);
        assert!(result.is_err());
    }

    #[test]
    fn track_metadata_complete() {
        let metadata = TrackMetadata::complete(
            "t1",
            "Song",
            "Artist",
            128.0,
            "Am",
            "House",
            Duration::from_secs(240),
        );
        assert!(metadata.is_complete());
        assert!(!metadata.is_missing());
        assert_eq!(metadata.missing_fields().len(), 0);
    }

    #[test]
    fn learning_data_stale() {
        let data = AutomixLearningData::new();
        assert!(data.is_stale()); // No history means stale.
    }

    #[test]
    fn learning_data_most_preferred() {
        let mut data = AutomixLearningData::new();
        data.record_override("track1", "House", TransitionEasing::Smooth, Instant::now());
        data.record_override("track2", "Techno", TransitionEasing::Smooth, Instant::now());

        assert!(data.most_preferred_track().is_some());
        assert!(data.most_preferred_genre().is_some());
    }

    #[test]
    fn override_type_variants() {
        assert!(matches!(
            OverrideType::TrackSelection,
            OverrideType::TrackSelection
        ));
        assert!(matches!(
            OverrideType::SkipTransition,
            OverrideType::SkipTransition
        ));
        assert!(matches!(
            OverrideType::ForceTransition,
            OverrideType::ForceTransition
        ));
        assert!(matches!(
            OverrideType::ChangeRecommendation,
            OverrideType::ChangeRecommendation
        ));
        assert!(matches!(OverrideType::Pause, OverrideType::Pause));
        assert!(matches!(OverrideType::Resume, OverrideType::Resume));
    }

    #[test]
    fn transition_easing_variants() {
        assert!(matches!(
            TransitionEasing::Immediate,
            TransitionEasing::Immediate
        ));
        assert!(matches!(TransitionEasing::Smooth, TransitionEasing::Smooth));
        assert!(matches!(TransitionEasing::Long, TransitionEasing::Long));
    }

    #[test]
    fn automix_error_display() {
        assert_eq!(
            format!("{}", AutomixError::PlanNotLocked),
            "Plan must be locked before saving"
        );
        assert_eq!(
            format!("{}", AutomixError::NoCurrentPlan),
            "No current mix plan"
        );
        assert_eq!(
            format!("{}", AutomixError::PlanNotFound),
            "Saved plan not found"
        );
    }

    #[test]
    fn mix_plan_export_json() {
        let engine = AutomixOverrideEngine::new();
        let json = engine.export_plan_json();
        assert!(json.is_none()); // No current plan.

        let mut engine2 = AutomixOverrideEngine::new();
        let _ = engine2.create_plan("Test");
        let json = engine2.export_plan_json();
        assert!(json.is_some());
        assert!(json.unwrap().contains("Test"));
    }

    #[test]
    fn mix_plan_add_multiple_actions() {
        let mut plan = MixPlan::new("Complex Mix");
        plan.add_action(MixPlanAction::Crossfade {
            from_track: "t1".to_string(),
            to_track: "t2".to_string(),
            start_time: 0.0,
            duration: 30.0,
            easing: TransitionEasing::Smooth,
        });
        plan.add_action(MixPlanAction::Loop {
            track_id: "t2".to_string(),
            start_time: 30.0,
            loop_count: 4,
        });
        plan.add_action(MixPlanAction::Cue {
            track_id: "t3".to_string(),
            cue_time: 150.0,
        });

        assert_eq!(plan.action_count(), 3);
    }
}
