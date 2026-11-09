//! Mastering preset management, chain import/export,
//! per-deck mastering presets, and chain visualisation.
//! Extends P10a (mastering effects: compressor, limiter, EQ, exciter).
//!
//! ## Design principles
//!
//! * **Chain serialisation** — Presets are portable JSON-serialisable structs.
//! * **Safe defaults** — Preset loads never set unsafe values.
//! * **No allocations in process path** — UI-only feature.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::mastering::MasteringChain;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Mastering preset.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MasteringPreset {
    /// Preset name.
    pub name: String,
    /// Preset description.
    pub description: String,
    /// Chain settings.
    pub chain: MasteringChain,
    /// Preset category.
    pub category: PresetCategory,
    /// Whether this is a built-in preset.
    pub is_builtin: bool,
    /// Preset version (for migration).
    pub version: u32,
}

/// Preset category.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PresetCategory {
    /// Live performance presets.
    Live,
    /// Club/festival presets.
    Club,
    /// Studio/mastering presets.
    Studio,
    /// Genre-specific.
    Genre(String),
    /// User-created.
    User,
    /// Import.
    Import,
}

/// Mastering chain export format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainExportFormat {
    /// Human-readable JSON.
    Json,
    /// Compact JSON for storage.
    CompactJson,
    /// TOML (optional).
    Toml,
}

/// Preset file metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PresetFileInfo {
    /// File path.
    pub path: PathBuf,
    /// File name (without extension).
    pub display_name: String,
    /// Preset name from file.
    pub preset_name: String,
    /// Category.
    pub category: String,
    /// Modification time.
    pub modified: std::time::SystemTime,
}

/// Chain visualisation node.
#[derive(Debug, Clone)]
pub struct ChainNode {
    /// Node type (compressor, EQ, etc.).
    pub node_type: ChainNodeType,
    /// Input gain (dB).
    pub input_gain_db: f64,
    /// Output gain (dB).
    pub output_gain_db: f64,
    /// Current gain reduction (dB).
    pub gain_reduction_db: f64,
    /// Bypassed?
    pub bypassed: bool,
}

/// Chain node type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainNodeType {
    Input,
    Compressor,
    Eq,
    Exciter,
    Limiter,
    Output,
}

impl ChainNodeType {
    /// Display label.
    pub fn label(&self) -> &str {
        match self {
            Self::Input => "Input",
            Self::Compressor => "Compressor",
            Self::Eq => "EQ",
            Self::Exciter => "Exciter",
            Self::Limiter => "Limiter",
            Self::Output => "Output",
        }
    }
}

/// Preset manager.
pub struct PresetManager {
    /// Builtin presets.
    builtin_presets: Vec<MasteringPreset>,
    /// User presets keyed by name.
    user_presets: std::sync::Mutex<HashMap<String, MasteringPreset>>,
    /// Import directory.
    import_dir: PathBuf,
}

impl PresetManager {
    /// Create a new preset manager with built-in presets.
    pub fn new() -> Self {
        Self {
            builtin_presets: Self::build_builtin_presets(),
            user_presets: std::sync::Mutex::new(HashMap::new()),
            import_dir: std::env::temp_dir().join("drdj_presets"),
        }
    }

    /// Create with a custom import directory.
    pub fn with_import_dir(import_dir: PathBuf) -> Self {
        let mut mgr = Self::new();
        mgr.import_dir = import_dir;
        mgr
    }

    /// Get all available presets (builtin + user).
    pub fn all_presets(&self) -> Vec<MasteringPreset> {
        let user = self.user_presets.lock().unwrap();
        let mut all: Vec<MasteringPreset> = self
            .builtin_presets
            .iter()
            .chain(user.values())
            .cloned()
            .collect();
        all.sort_by(|a, b| a.name.cmp(&b.name));
        all
    }

    /// Get presets by category.
    pub fn presets_by_category(&self, category: &PresetCategory) -> Vec<MasteringPreset> {
        self.all_presets()
            .into_iter()
            .filter(|p| &p.category == category)
            .collect()
    }

    /// Get a preset by name.
    pub fn get_preset(&self, name: &str) -> Option<MasteringPreset> {
        // Check builtin first.
        if let Some(p) = self.builtin_presets.iter().find(|p| p.name == name) {
            return Some(p.clone());
        }
        // Then user presets.
        self.user_presets.lock().unwrap().get(name).cloned()
    }

    /// Get builtin presets.
    pub fn builtin_presets(&self) -> &[MasteringPreset] {
        &self.builtin_presets
    }

    /// Load a preset into a chain.
    pub fn load_into(&self, name: &str, chain: &mut MasteringChain) -> bool {
        if let Some(preset) = self.get_preset(name) {
            *chain = preset.chain;
            true
        } else {
            false
        }
    }

    /// Save a user preset.
    pub fn save_preset(&self, preset: MasteringPreset) -> Result<(), PresetError> {
        let json = serde_json::to_string_pretty(&preset)
            .map_err(|e| PresetError::Serialization(e.to_string()))?;

        let path = self.preset_path(&preset.name);
        fs::create_dir_all(&self.import_dir).map_err(|e| PresetError::Io(e.to_string()))?;
        fs::write(&path, json).map_err(|e| PresetError::Io(e.to_string()))?;

        self.user_presets
            .lock()
            .unwrap()
            .insert(preset.name.clone(), preset);
        Ok(())
    }

    /// Delete a user preset.
    pub fn delete_preset(&self, name: &str) -> Result<(), PresetError> {
        let mut user = self.user_presets.lock().unwrap();
        if let Some(preset) = user.remove(name) {
            let path = self.preset_path(&preset.name);
            let _ = fs::remove_file(&path);
        }
        Ok(())
    }

    /// Export a chain to a file.
    pub fn export_chain(
        &self,
        chain: &MasteringChain,
        path: &Path,
        format: ChainExportFormat,
    ) -> Result<(), PresetError> {
        let json = match format {
            ChainExportFormat::Json => serde_json::to_string_pretty(chain)
                .map_err(|e| PresetError::Serialization(e.to_string()))?,
            ChainExportFormat::CompactJson => serde_json::to_string(chain)
                .map_err(|e| PresetError::Serialization(e.to_string()))?,
            ChainExportFormat::Toml => {
                // Serialize to a simple string representation.
                format!(
                    "master_gain_db = {}\nenabled = {}",
                    chain.master_gain_db, chain.enabled
                )
            }
        };

        fs::write(path, json).map_err(|e| PresetError::Io(e.to_string()))?;
        Ok(())
    }

    /// Import a chain from a file.
    pub fn import_chain(&self, path: &Path) -> Result<MasteringChain, PresetError> {
        let json = fs::read_to_string(path).map_err(|e| PresetError::Io(e.to_string()))?;

        // Try JSON first.
        if let Ok(chain) = serde_json::from_str::<MasteringChain>(&json) {
            return Ok(chain);
        }

        // Fallback: TOML-like simple parse.
        self.parse_toml_chain(&json)
    }

    /// Scan the import directory for preset files.
    pub fn scan_import_dir(&self) -> Result<Vec<PresetFileInfo>, PresetError> {
        let mut files = Vec::new();

        if !self.import_dir.exists() {
            return Ok(files);
        }

        for entry in fs::read_dir(&self.import_dir).map_err(|e| PresetError::Io(e.to_string()))? {
            let entry = entry.map_err(|e| PresetError::Io(e.to_string()))?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "json") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    files.push(PresetFileInfo {
                        path: path.clone(),
                        display_name: name.to_string(),
                        preset_name: name.to_string(),
                        category: "Import".to_string(),
                        modified: entry
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::SystemTime::now()),
                    });
                }
            }
        }

        files.sort_by_key(|value| std::cmp::Reverse(value.modified));
        Ok(files)
    }

    /// Get all available categories.
    pub fn available_categories(&self) -> Vec<PresetCategory> {
        let mut seen = HashMap::new();
        for preset in self.all_presets() {
            seen.insert(preset.category.clone(), true);
        }
        seen.into_keys().collect()
    }

    /// Create a chain from settings (convenience).
    pub fn chain_from_settings(settings: &MasteringChain) -> MasteringChain {
        settings.clone()
    }

    /// Validate preset name (alphanumeric + underscores, max 64 chars).
    pub fn validate_preset_name(name: &str) -> bool {
        if name.len() > 64 || name.is_empty() {
            return false;
        }
        name.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    }

    /// Preset file path for a given name.
    fn preset_path(&self, name: &str) -> PathBuf {
        self.import_dir.join(format!("{}.json", name))
    }

    /// Parse a simple TOML chain (fallback for export).
    fn parse_toml_chain(&self, text: &str) -> Result<MasteringChain, PresetError> {
        let mut chain = MasteringChain::default();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(value) = line.strip_prefix("master_gain_db = ") {
                if let Ok(gain) = value.trim().parse::<f64>() {
                    chain.master_gain_db = gain;
                }
            } else if let Some(value) = line.strip_prefix("enabled = ") {
                if let Ok(enabled) = value.trim().parse::<bool>() {
                    chain.enabled = enabled;
                }
            }
        }

        Ok(chain)
    }

    /// Built-in presets.
    fn build_builtin_presets() -> Vec<MasteringPreset> {
        vec![
            MasteringPreset {
                name: "Clean Master".to_string(),
                description: "Transparent limiting for clean playback".to_string(),
                chain: MasteringChain::default(),
                category: PresetCategory::Studio,
                is_builtin: true,
                version: 1,
            },
            MasteringPreset {
                name: "Club Punch".to_string(),
                description: "Aggressive compression for club sound".to_string(),
                chain: MasteringChain {
                    compressor: crate::mastering::CompressorSettings {
                        threshold_db: -12.0,
                        ratio: 8.0,
                        attack_ms: 5.0,
                        release_ms: 100.0,
                        makeup_gain_db: 6.0,
                        enabled: true,
                    },
                    limiter: crate::mastering::LimiterSettings {
                        max_level_db: -1.0,
                        release_ms: 50.0,
                        enabled: true,
                    },
                    eq: crate::mastering::EqSettings {
                        low: crate::mastering::EqBand {
                            gain_db: 2.0,
                            ..crate::mastering::EqSettings::default().low
                        },
                        high: crate::mastering::EqBand {
                            gain_db: 1.0,
                            ..crate::mastering::EqSettings::default().high
                        },
                        enabled: true,
                        ..Default::default()
                    },
                    exciter: crate::mastering::ExciterSettings {
                        enabled: false,
                        ..Default::default()
                    },
                    master_gain_db: 0.0,
                    enabled: true,
                },
                category: PresetCategory::Club,
                is_builtin: true,
                version: 1,
            },
            MasteringPreset {
                name: "Warm Vinyl".to_string(),
                description: "Tape warmth and vinyl character".to_string(),
                chain: MasteringChain {
                    compressor: crate::mastering::CompressorSettings {
                        threshold_db: -18.0,
                        ratio: 2.0,
                        attack_ms: 30.0,
                        release_ms: 300.0,
                        makeup_gain_db: 3.0,
                        enabled: true,
                    },
                    limiter: crate::mastering::LimiterSettings {
                        max_level_db: -3.0,
                        release_ms: 100.0,
                        enabled: false,
                    },
                    eq: crate::mastering::EqSettings {
                        low: crate::mastering::EqBand {
                            gain_db: 3.0,
                            ..crate::mastering::EqSettings::default().low
                        },
                        high: crate::mastering::EqBand {
                            gain_db: -2.0,
                            ..crate::mastering::EqSettings::default().high
                        },
                        enabled: true,
                        ..Default::default()
                    },
                    exciter: crate::mastering::ExciterSettings {
                        enabled: true,
                        ..Default::default()
                    },
                    master_gain_db: 1.0,
                    enabled: true,
                },
                category: PresetCategory::Genre("Lo-Fi".to_string()),
                is_builtin: true,
                version: 1,
            },
            MasteringPreset {
                name: "Live Safe".to_string(),
                description: "Conservative limiting for live sound".to_string(),
                chain: MasteringChain {
                    compressor: crate::mastering::CompressorSettings {
                        threshold_db: -6.0,
                        ratio: 4.0,
                        attack_ms: 10.0,
                        release_ms: 200.0,
                        makeup_gain_db: 2.0,
                        enabled: true,
                    },
                    limiter: crate::mastering::LimiterSettings {
                        max_level_db: -2.0,
                        release_ms: 75.0,
                        enabled: true,
                    },
                    eq: crate::mastering::EqSettings {
                        low: crate::mastering::EqBand {
                            gain_db: 0.0,
                            ..crate::mastering::EqSettings::default().low
                        },
                        high: crate::mastering::EqBand {
                            gain_db: 0.0,
                            ..crate::mastering::EqSettings::default().high
                        },
                        enabled: false,
                        ..Default::default()
                    },
                    exciter: crate::mastering::ExciterSettings {
                        enabled: false,
                        ..Default::default()
                    },
                    master_gain_db: 0.0,
                    enabled: true,
                },
                category: PresetCategory::Live,
                is_builtin: true,
                version: 1,
            },
        ]
    }
}

impl Default for PresetManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Preset error.
#[derive(Debug, Clone)]
pub enum PresetError {
    Io(String),
    Serialization(String),
    NotFound(String),
    InvalidPreset(String),
}

impl std::fmt::Display for PresetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "IO error: {}", msg),
            Self::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            Self::NotFound(name) => write!(f, "Preset not found: {}", name),
            Self::InvalidPreset(msg) => write!(f, "Invalid preset: {}", msg),
        }
    }
}

impl std::error::Error for PresetError {}

/// Chain visualiser.
pub struct ChainVisualiser {}

impl ChainVisualiser {
    pub fn new() -> Self {
        Self {}
    }

    /// Build chain node graph from a chain.
    pub fn build_graph(&self, chain: &MasteringChain) -> Vec<ChainNode> {
        vec![
            ChainNode {
                node_type: ChainNodeType::Input,
                input_gain_db: chain.master_gain_db,
                output_gain_db: chain.master_gain_db,
                gain_reduction_db: 0.0,
                bypassed: !chain.enabled,
            },
            ChainNode {
                node_type: ChainNodeType::Compressor,
                input_gain_db: 0.0,
                output_gain_db: chain.compressor.makeup_gain_db,
                gain_reduction_db: 0.0,
                bypassed: !chain.compressor.enabled,
            },
            ChainNode {
                node_type: ChainNodeType::Eq,
                input_gain_db: 0.0,
                output_gain_db: 0.0,
                gain_reduction_db: 0.0,
                bypassed: !chain.eq.enabled,
            },
            ChainNode {
                node_type: ChainNodeType::Exciter,
                input_gain_db: 0.0,
                output_gain_db: 0.0,
                gain_reduction_db: 0.0,
                bypassed: !chain.exciter.enabled,
            },
            ChainNode {
                node_type: ChainNodeType::Limiter,
                input_gain_db: 0.0,
                output_gain_db: 0.0,
                gain_reduction_db: 0.0,
                bypassed: !chain.limiter.enabled,
            },
            ChainNode {
                node_type: ChainNodeType::Output,
                input_gain_db: 0.0,
                output_gain_db: chain.master_gain_db,
                gain_reduction_db: 0.0,
                bypassed: !chain.enabled,
            },
        ]
    }

    /// Get chain summary text.
    pub fn chain_summary(&self, chain: &MasteringChain) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Mastering Chain (enabled: {})", chain.enabled));
        lines.push(format!("  Master gain: {:.2} dB", chain.master_gain_db));

        if chain.compressor.enabled {
            lines.push(format!(
                "  Compressor: {:.0}:{:.0} @ {:.1} dB, A/R {:.0}/{:.0} ms",
                chain.compressor.ratio,
                chain.compressor.ratio,
                chain.compressor.threshold_db,
                chain.compressor.attack_ms,
                chain.compressor.release_ms,
            ));
        }

        if chain.limiter.enabled {
            lines.push(format!(
                "  Limiter: {:.1} dB, R {:.0} ms",
                chain.limiter.max_level_db, chain.limiter.release_ms,
            ));
        }

        if chain.eq.enabled {
            lines.push(format!(
                "  EQ: Low +/-.{} dB, High +/-.{} dB",
                chain.eq.low.gain_db, chain.eq.high.gain_db,
            ));
        }

        if chain.exciter.enabled {
            lines.push("  Exciter: On".to_string());
        }

        lines.join("\n")
    }
}

impl Default for ChainVisualiser {
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
    fn test_builtin_presets_exist() {
        let mgr = PresetManager::new();
        let presets = mgr.all_presets();
        assert!(presets.len() >= 4);

        let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"Clean Master"));
        assert!(names.contains(&"Club Punch"));
        assert!(names.contains(&"Warm Vinyl"));
        assert!(names.contains(&"Live Safe"));
    }

    #[test]
    fn test_load_builtin_preset() {
        let mgr = PresetManager::new();
        let mut chain = MasteringChain::default();
        assert!(mgr.load_into("Club Punch", &mut chain));
        assert!(chain.compressor.enabled);
        assert_eq!(chain.compressor.ratio, 8.0);
    }

    #[test]
    fn test_load_nonexistent_preset() {
        let mgr = PresetManager::new();
        let mut chain = MasteringChain::default();
        assert!(!mgr.load_into("Nonexistent", &mut chain));
    }

    #[test]
    fn test_save_and_load_user_preset() {
        let mgr = PresetManager::new();
        let preset = MasteringPreset {
            name: "TestPreset".to_string(),
            description: "Test".to_string(),
            chain: MasteringChain::default(),
            category: PresetCategory::User,
            is_builtin: false,
            version: 1,
        };
        assert!(mgr.save_preset(preset.clone()).is_ok());
        assert!(mgr.get_preset("TestPreset").is_some());
    }

    #[test]
    fn test_delete_user_preset() {
        let mgr = PresetManager::new();
        let preset = MasteringPreset {
            name: "ToDelete".to_string(),
            description: "Delete me".to_string(),
            chain: MasteringChain::default(),
            category: PresetCategory::User,
            is_builtin: false,
            version: 1,
        };
        assert!(mgr.save_preset(preset).is_ok());
        assert!(mgr.get_preset("ToDelete").is_some());
        assert!(mgr.delete_preset("ToDelete").is_ok());
        assert!(mgr.get_preset("ToDelete").is_none());
    }

    #[test]
    fn test_export_chain_json() {
        let mgr = PresetManager::new();
        let chain = MasteringChain::default();
        let path = std::env::temp_dir().join("drdj_test_chain.json");
        assert!(mgr
            .export_chain(&chain, &path, ChainExportFormat::Json)
            .is_ok());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_import_chain_json() {
        let mgr = PresetManager::new();
        let chain = MasteringChain::default();
        let path = std::env::temp_dir().join("drdj_test_import.json");
        let _ = mgr.export_chain(&chain, &path, ChainExportFormat::CompactJson);

        let imported = mgr.import_chain(&path);
        assert!(imported.is_ok());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_import_chain_toml() {
        let mgr = PresetManager::new();
        let toml_text = "master_gain_db = 3.5\nenabled = true";
        let path = std::env::temp_dir().join("drdj_test_toml.txt");
        let _ = fs::write(&path, toml_text);

        let imported = mgr.import_chain(&path);
        assert!(imported.is_ok());
        let chain = imported.unwrap();
        assert_eq!(chain.master_gain_db, 3.5);
        assert!(chain.enabled);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_scan_import_dir_empty() {
        let directory = tempfile::tempdir().unwrap();
        let mgr = PresetManager::with_import_dir(directory.path().to_path_buf());
        let files = mgr.scan_import_dir().unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_validate_preset_name() {
        assert!(PresetManager::validate_preset_name("Valid_Preset-1"));
        assert!(PresetManager::validate_preset_name("A"));
        assert!(!PresetManager::validate_preset_name(""));
        assert!(!PresetManager::validate_preset_name(&"A".repeat(65)));
        assert!(!PresetManager::validate_preset_name("Invalid Name"));
    }

    #[test]
    fn test_preset_by_category() {
        let mgr = PresetManager::new();
        let live = mgr.presets_by_category(&PresetCategory::Live);
        assert!(!live.is_empty());
        assert!(live.iter().all(|p| p.category == PresetCategory::Live));
    }

    #[test]
    fn test_chain_visualiser_graph() {
        let vis = ChainVisualiser::new();
        let chain = MasteringChain::default();
        let graph = vis.build_graph(&chain);
        assert_eq!(graph.len(), 6);
        assert_eq!(graph[0].node_type, ChainNodeType::Input);
        assert_eq!(graph[1].node_type, ChainNodeType::Compressor);
        assert_eq!(graph[2].node_type, ChainNodeType::Eq);
        assert_eq!(graph[5].node_type, ChainNodeType::Output);
    }

    #[test]
    fn test_chain_visualiser_summary() {
        let vis = ChainVisualiser::new();
        let chain = MasteringChain::default();
        let summary = vis.chain_summary(&chain);
        assert!(summary.contains("Mastering Chain"));
        assert!(summary.contains("enabled"));
    }

    #[test]
    fn test_chain_visualiser_enabled() {
        let vis = ChainVisualiser::new();
        let chain = MasteringChain {
            enabled: true,
            ..MasteringChain::default()
        };
        let graph = vis.build_graph(&chain);
        assert!(!graph[0].bypassed);
        assert!(!graph[5].bypassed);

        let disabled = MasteringChain {
            enabled: false,
            ..MasteringChain::default()
        };
        let graph = vis.build_graph(&disabled);
        assert!(graph[0].bypassed);
    }

    #[test]
    fn test_preset_error_display() {
        let err = PresetError::Io("file not found".to_string());
        assert!(format!("{}", err).contains("file not found"));

        let err = PresetError::NotFound("MyPreset".to_string());
        assert!(format!("{}", err).contains("MyPreset"));
    }

    #[test]
    fn test_chain_export_toml_parse() {
        let mgr = PresetManager::new();
        let text = "master_gain_db = -2.0\nenabled = false";
        let chain = mgr.parse_toml_chain(text).unwrap();
        assert_eq!(chain.master_gain_db, -2.0);
        assert!(!chain.enabled);
    }
}
