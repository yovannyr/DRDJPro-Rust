//! Guarded named-device selection for the next application start.
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
    let mut choices = Vec::new();
    for device in host.output_devices().map_err(|e| e.to_string())? {
        let Ok(name) = device.name() else { continue };
        let Ok(config) = device.default_output_config() else {
            continue;
        };
        let selector = DeviceSelector {
            backend: host.id().name().into(),
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
        .filter(|choice| &choice.selector == selector)
        .count()
    {
        1 => Ok(()),
        0 => Err("Selected audio device is no longer available; refresh the list".into()),
        _ => Err("Audio device name is ambiguous; selection was not saved".into()),
    }
}
pub fn resolve(host: &cpal::Host, selector: &DeviceSelector) -> Result<cpal::Device, String> {
    selector.validate()?;
    if host.id().name() != selector.backend {
        return Err("Selected audio backend is unavailable".into());
    }
    let mut found = None;
    for device in host.output_devices().map_err(|e| e.to_string())? {
        if device.name().is_ok_and(|name| name == selector.name) {
            if found.is_some() {
                return Err("Selected audio device name is ambiguous".into());
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
            name: "bad\nname".into()
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
}
