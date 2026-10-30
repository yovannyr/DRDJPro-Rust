//! Guarded output profiles: backend/name/channel shape, not a hardware serial number.
use serde::{Deserialize, Serialize};
pub const MAX_ROUTES_BYTES: usize = 65536;
pub const MAX_OUTPUT_PROFILES: usize = 32;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputDeviceKey {
    pub backend: String,
    pub name: String,
    pub channels: u16,
}
impl OutputDeviceKey {
    pub fn validate(&self) -> Result<(), String> {
        if self.backend.is_empty()
            || self.backend.len() > 32
            || self.name.is_empty()
            || self.name.len() > 512
            || self.channels == 0
            || self
                .backend
                .chars()
                .chain(self.name.chars())
                .any(char::is_control)
        {
            return Err("Invalid audio device profile key".into());
        }
        Ok(())
    }
    pub fn unique(backend: String, name: String, channels: u16, matches: usize) -> Option<Self> {
        let key = Self {
            backend,
            name,
            channels,
        };
        (matches == 1 && key.validate().is_ok()).then_some(key)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitorPreferences {
    pub gain: f32,
    pub mix: f32,
    pub cues: [bool; 2],
}
impl MonitorPreferences {
    pub fn settings(self) -> crate::output_routing::MonitorSettings {
        crate::output_routing::MonitorSettings {
            gain: self.gain,
            mix: self.mix,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputRoute {
    pub device: OutputDeviceKey,
    pub pair: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor: Option<MonitorPreferences>,
}
impl OutputRoute {
    pub fn validate(&self) -> Result<(), String> {
        self.device.validate()?;
        if let Some(monitor) = self.monitor {
            monitor.settings().validate()?;
            if self.device.channels < 4
                || (self.pair == 1 && (monitor.mix > 0. || monitor.cues.into_iter().any(|v| v)))
            {
                return Err(
                    "Saved monitor profile conflicts with the output format or master route".into(),
                );
            }
        }
        if self.pair != 0 && u32::from(self.pair) * 2 + 1 >= u32::from(self.device.channels) {
            return Err("Saved audio output pair is unavailable".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputRoutes {
    pub version: u8,
    pub routes: Vec<OutputRoute>,
}
impl Default for OutputRoutes {
    fn default() -> Self {
        Self {
            version: 1,
            routes: vec![],
        }
    }
}
impl OutputRoutes {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.routes.len() > MAX_OUTPUT_PROFILES {
            return Err("Unsupported or excessive output profiles".into());
        }
        for (index, route) in self.routes.iter().enumerate() {
            route.validate()?;
            if self.routes[..index]
                .iter()
                .any(|r| r.device == route.device)
            {
                return Err("Duplicate audio device profile".into());
            }
        }
        Ok(())
    }
    pub fn pair_for(&self, key: &OutputDeviceKey) -> Option<u16> {
        self.routes
            .iter()
            .find(|r| &r.device == key)
            .map(|r| r.pair)
    }
    pub fn update(&mut self, route: OutputRoute) -> Result<(), String> {
        route.validate()?;
        if let Some(old) = self.routes.iter_mut().find(|r| r.device == route.device) {
            *old = route;
        } else {
            if self.routes.len() == MAX_OUTPUT_PROFILES {
                return Err("Audio device profile limit reached".into());
            }
            self.routes.push(route);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monitor_profiles_are_optional_strict_and_validate_routing_conflicts() {
        let old = r#"{"version":1,"routes":[{"device":{"backend":"Test","name":"MC4000","channels":4},"pair":0}]}"#;
        let mut profiles: OutputRoutes = serde_json::from_str(old).unwrap();
        profiles.validate().unwrap();
        assert_eq!(profiles.routes[0].monitor, None);
        let settings = MonitorPreferences {
            gain: 0.4,
            mix: 0.75,
            cues: [true, false],
        };
        profiles.routes[0].monitor = Some(settings);
        profiles.validate().unwrap();
        let saved = serde_json::to_string(&profiles).unwrap();
        assert_eq!(
            serde_json::from_str::<OutputRoutes>(&saved).unwrap(),
            profiles
        );
        profiles.routes[0].pair = 1;
        assert!(profiles.validate().is_err());
        profiles.routes[0].pair = 0;
        profiles.routes[0].monitor.as_mut().unwrap().gain = f32::NAN;
        assert!(profiles.validate().is_err());
        assert!(serde_json::from_str::<MonitorPreferences>(
            r#"{"gain":1,"mix":0,"cues":[false,false],"future":1}"#
        )
        .is_err());
    }
    #[test]
    fn profiles_match_exact_device_shape_and_reject_ambiguity_or_invalid_data() {
        let key = OutputDeviceKey::unique("WASAPI".into(), "MC4000".into(), 4, 1).unwrap();
        assert!(OutputDeviceKey::unique("WASAPI".into(), "MC4000".into(), 4, 2).is_none());
        let mut profiles = OutputRoutes::default();
        profiles
            .update(OutputRoute {
                device: key.clone(),
                pair: 1,

                monitor: None,
            })
            .unwrap();
        assert_eq!(profiles.pair_for(&key), Some(1));
        for other in [
            OutputDeviceKey {
                name: "Other".into(),
                ..key.clone()
            },
            OutputDeviceKey {
                backend: "Other".into(),
                ..key.clone()
            },
            OutputDeviceKey {
                channels: 2,
                ..key.clone()
            },
        ] {
            assert_eq!(profiles.pair_for(&other), None);
        }
        assert!(profiles
            .update(OutputRoute {
                device: key.clone(),
                pair: 2,
                monitor: None,
            })
            .is_err());
        let mut duplicate = profiles.clone();
        duplicate.routes.push(duplicate.routes[0].clone());
        assert!(duplicate.validate().is_err());
        profiles
            .update(OutputRoute {
                device: key,
                pair: 0,

                monitor: None,
            })
            .unwrap();
        assert_eq!(profiles.routes.len(), 1);
        for i in 1..32 {
            profiles
                .update(OutputRoute {
                    device: OutputDeviceKey {
                        backend: "WASAPI".into(),
                        name: format!("Device{i}"),
                        channels: 2,
                    },
                    pair: 0,

                    monitor: None,
                })
                .unwrap();
        }
        assert!(profiles
            .update(OutputRoute {
                device: OutputDeviceKey {
                    backend: "WASAPI".into(),
                    name: "overflow".into(),
                    channels: 2
                },
                pair: 0,
                monitor: None,
            })
            .is_err());
        profiles.version = 2;
        assert!(profiles.validate().is_err());
        assert!(
            serde_json::from_str::<OutputRoutes>(r#"{"version":1,"routes":[],"extra":true}"#)
                .is_err()
        );
    }
}
