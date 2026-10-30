//! Guarded device selection for the next application start.
use rodio::cpal::{
    self,
    traits::{DeviceTrait, HostTrait},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceSelector {
    pub backend: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_id: Option<String>,
}
impl DeviceSelector {
    pub fn validate(&self) -> Result<(), String> {
        if self.backend.is_empty()
            || self.backend.len() > 32
            || self.name.is_empty()
            || self.name.len() > 512
            || self
                .backend
                .chars()
                .chain(self.name.chars())
                .any(char::is_control)
            || self.endpoint_id.as_ref().is_some_and(|id| {
                id.is_empty() || id.len() > 2048 || id.chars().any(char::is_control)
            })
        {
            return Err("Invalid selected audio device".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct DeviceChoice {
    pub selector: DeviceSelector,
    pub channels: u16,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartupSelection {
    pub version: u8,
    pub device: Option<DeviceSelector>,
}
impl Default for StartupSelection {
    fn default() -> Self {
        Self {
            version: 1,
            device: None,
        }
    }
}
impl StartupSelection {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported startup audio preference version".into());
        }
        if let Some(device) = &self.device {
            device.validate()?;
        }
        Ok(())
    }
}
pub fn enumerate() -> Result<Vec<DeviceChoice>, String> {
    let host = cpal::default_host();
    #[cfg(target_os = "windows")]
    let endpoints = if host.id().name() == "WASAPI" {
        Some(crate::audio_identity::endpoints()?)
    } else {
        None
    };
    let mut choices = Vec::new();
    for device in host.output_devices().map_err(|e| e.to_string())? {
        let Ok(name) = device.name() else { continue };
        let Ok(config) = device.default_output_config() else {
            continue;
        };
        let selector = DeviceSelector {
            backend: host.id().name().into(),
            endpoint_id: {
                #[cfg(target_os = "windows")]
                {
                    endpoints
                        .as_deref()
                        .and_then(|entries| crate::audio_identity::unique_id(entries, &name))
                }
                #[cfg(not(target_os = "windows"))]
                {
                    None
                }
            },
            name,
        };
        if selector.validate().is_ok()
            && (1..=crate::mixer_dsp::MAX_CHANNELS).contains(&config.channels())
        {
            choices.push(DeviceChoice {
                selector,
                channels: config.channels(),
            });
        }
    }
    Ok(choices)
}
pub fn unique_choice(choices: &[DeviceChoice], selector: &DeviceSelector) -> Result<(), String> {
    selector.validate()?;
    match choices
        .iter()
        .filter(|choice| {
            choice.selector.backend == selector.backend
                && match selector.endpoint_id.as_ref() {
                    Some(id) => choice.selector.endpoint_id.as_ref() == Some(id),
                    None => choice.selector.name == selector.name,
                }
        })
        .count()
    {
        1 => Ok(()),
        0 => Err("Selected audio device is no longer available; refresh the list".into()),
        _ => Err("Audio device name is ambiguous; selection was not saved".into()),
    }
}
pub fn validate_new_startup_choice(selector: &DeviceSelector) -> Result<(), String> {
    selector.validate()?;
    if selector.backend == "WASAPI" && selector.endpoint_id.is_none() {
        return Err(
            "Windows could not identify this output uniquely; select System default or another output"
                .into(),
        );
    }
    Ok(())
}
pub fn resolve(host: &cpal::Host, selector: &DeviceSelector) -> Result<cpal::Device, String> {
    selector.validate()?;
    if host.id().name() != selector.backend {
        return Err("Selected audio backend is unavailable".into());
    }
    let name = match selector.endpoint_id.as_deref() {
        Some(endpoint_id) if selector.backend == "WASAPI" => {
            let endpoints = crate::audio_identity::endpoints()?;
            crate::audio_identity::unique_name(&endpoints, endpoint_id)
                .ok_or("Selected audio endpoint is unavailable or ambiguous")?
                .to_owned()
        }
        Some(_) => return Err("Stable endpoint selection is unavailable on this backend".into()),
        None => selector.name.clone(),
    };
    let mut found = None;
    for device in host.output_devices().map_err(|e| e.to_string())? {
        if device.name().is_ok_and(|device_name| device_name == name) {
            if found.is_some() {
                return Err("Selected audio endpoint maps to an ambiguous device name".into());
            }
            found = Some(device);
        }
    }
    found.ok_or_else(|| "Selected audio device is unavailable".into())
}
pub fn open_preferred<T, E: std::fmt::Display>(
    selection: &StartupSelection,
    mut open: impl FnMut(Option<&DeviceSelector>) -> Result<T, E>,
) -> Result<(T, Option<String>), E> {
    match open(selection.device.as_ref()) {
        Ok(value) => Ok((value, None)),
        Err(error) if selection.device.is_some() => {
            let warning=format!("Selected device could not open: {error}; system default in use. Saved selection preserved.");
            open(None).map(|value| (value, Some(warning)))
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferred_open_reports_fallback_and_keeps_the_requested_identity() {
        let selection = StartupSelection {
            version: 1,
            device: Some(DeviceSelector {
                backend: "Test".into(),
                name: "Missing".into(),
                endpoint_id: Some("stable-id".into()),
            }),
        };
        let mut requests = Vec::new();
        let (opened, warning) = open_preferred(&selection, |device| {
            requests.push(device.cloned());
            if device.is_some() {
                Err("unavailable")
            } else {
                Ok("default")
            }
        })
        .unwrap();
        assert_eq!(opened, "default");
        assert!(warning.unwrap().contains("Saved selection preserved"));
        assert_eq!(requests, vec![selection.device.clone(), None]);
        assert_eq!(
            selection.device.as_ref().unwrap().endpoint_id.as_deref(),
            Some("stable-id")
        );
        let mut count = 0;
        assert!(open_preferred::<(), _>(&StartupSelection::default(), |_| {
            count += 1;
            Err("no outputs")
        })
        .is_err());
        assert_eq!(count, 1);
    }
    #[test]
    fn selection_is_strict_and_names_must_resolve_uniquely() {
        let selector = DeviceSelector {
            backend: "Test".into(),
            name: "MC4000".into(),
            endpoint_id: None,
        };
        let choice = DeviceChoice {
            selector: selector.clone(),
            channels: 4,
        };
        unique_choice(std::slice::from_ref(&choice), &selector).unwrap();
        assert!(unique_choice(&[], &selector).is_err());
        assert!(unique_choice(&[choice.clone(), choice], &selector).is_err());
        assert!(DeviceSelector {
            backend: "Test".into(),
            name: "bad\nname".into(),
            endpoint_id: None,
        }
        .validate()
        .is_err());
        assert!(StartupSelection {
            version: 2,
            device: None
        }
        .validate()
        .is_err());
        assert!(serde_json::from_str::<StartupSelection>(
            r#"{"version":1,"device":null,"future":true}"#
        )
        .is_err());
    }

    #[test]
    fn endpoint_selection_matches_exact_id_and_legacy_names_only_when_unique() {
        let make_choice = |name: &str, endpoint_id: Option<&str>| DeviceChoice {
            selector: DeviceSelector {
                backend: "WASAPI".into(),
                name: name.into(),
                endpoint_id: endpoint_id.map(str::to_owned),
            },
            channels: 4,
        };
        let choices = [
            make_choice("MC4000", Some("endpoint-a")),
            make_choice("MC4000", Some("endpoint-b")),
            make_choice("Speakers", Some("endpoint-c")),
        ];
        let first = DeviceSelector {
            backend: "WASAPI".into(),
            name: "MC4000".into(),
            endpoint_id: Some("endpoint-a".into()),
        };
        unique_choice(&choices, &first).unwrap();
        let renamed = DeviceSelector {
            name: "Old MC4000 name".into(),
            ..first.clone()
        };
        unique_choice(&choices, &renamed).unwrap();
        let legacy = DeviceSelector {
            backend: "WASAPI".into(),
            name: "MC4000".into(),
            endpoint_id: None,
        };
        assert!(unique_choice(&choices, &legacy).is_err());
        let unique_legacy = DeviceSelector {
            backend: "WASAPI".into(),
            name: "Speakers".into(),
            endpoint_id: None,
        };
        unique_choice(&choices, &unique_legacy).unwrap();
        validate_new_startup_choice(&choices[0].selector).unwrap();
        assert!(validate_new_startup_choice(&choices[2].selector).is_ok());
        assert!(validate_new_startup_choice(&unique_legacy).is_err());

        let old_json = r#"{"version":1,"device":{"backend":"WASAPI","name":"MC4000"}}"#;
        let old: StartupSelection = serde_json::from_str(old_json).unwrap();
        assert_eq!(old.device.unwrap().endpoint_id, None);
        let current: StartupSelection = serde_json::from_str(
            r#"{"version":1,"device":{"backend":"WASAPI","name":"Old name","endpoint_id":"endpoint-a"}}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<StartupSelection>(&serde_json::to_string(&current).unwrap())
                .unwrap(),
            current
        );
        assert!(DeviceSelector {
            backend: "WASAPI".into(),
            name: "MC4000".into(),
            endpoint_id: Some("bad\nidentity".into()),
        }
        .validate()
        .is_err());
    }
}
