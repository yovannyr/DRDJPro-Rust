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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputRoute {
    pub device: OutputDeviceKey,
    pub pair: u16,
}
impl OutputRoute {
    pub fn validate(&self) -> Result<(), String> {
        self.device.validate()?;
        if self.pair != 0 && u32::from(self.pair) * 2 + 1 >= u32::from(self.device.channels) {
            return Err("Saved audio output pair is unavailable".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    fn profiles_match_exact_device_shape_and_reject_ambiguity_or_invalid_data() {
        let key = OutputDeviceKey::unique("WASAPI".into(), "MC4000".into(), 4, 1).unwrap();
        assert!(OutputDeviceKey::unique("WASAPI".into(), "MC4000".into(), 4, 2).is_none());
        let mut profiles = OutputRoutes::default();
        profiles
            .update(OutputRoute {
                device: key.clone(),
                pair: 1,
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
                pair: 2
            })
            .is_err());
        let mut duplicate = profiles.clone();
        duplicate.routes.push(duplicate.routes[0].clone());
        assert!(duplicate.validate().is_err());
        profiles
            .update(OutputRoute {
                device: key,
                pair: 0,
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
                pair: 0
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
