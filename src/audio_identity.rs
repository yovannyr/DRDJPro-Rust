//! Windows endpoint identities for guarded output profiles (not hardware serial numbers).
#[cfg(target_os = "windows")]
pub fn endpoints() -> Result<Vec<(String, String)>, String> {
    use windows::Win32::System::Com::StructuredStorage::PropVariantToString;
    use windows::Win32::{
        Devices::Properties::DEVPKEY_Device_FriendlyName, Media::Audio::*, System::Com::*,
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    unsafe {
        // COM belongs to this call/thread; RPC_E_CHANGED_MODE means an existing apartment is usable.
        let status = CoInitializeEx(None, COINIT_MULTITHREADED);
        let apartment = if status.is_ok() {
            Some(Apartment)
        } else if status.0 == 0x80010106u32 as i32 {
            None
        } else {
            return Err(status.to_string());
        };
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let collection = enumerator
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for index in 0..collection.GetCount().map_err(|e| e.to_string())? {
            let endpoint = collection.Item(index).map_err(|e| e.to_string())?;
            let id = endpoint.GetId().map_err(|e| e.to_string())?;
            let identity = id.to_string().map_err(|e| e.to_string());
            CoTaskMemFree(Some(id.0.cast()));
            let identity = identity?;
            let store = endpoint
                .OpenPropertyStore(STGM_READ)
                .map_err(|e| e.to_string())?;
            let value = store
                .GetValue(&DEVPKEY_Device_FriendlyName as *const _ as *const _)
                .map_err(|e| e.to_string())?;
            let mut buffer = [0u16; 513];
            PropVariantToString(&value, &mut buffer).map_err(|e| e.to_string())?;
            let length = buffer
                .iter()
                .position(|v| *v == 0)
                .ok_or("Invalid endpoint name")?;
            let name = String::from_utf16(&buffer[..length]).map_err(|e| e.to_string())?;
            result.push((name, identity));
        }
        // Release COM interfaces before balancing this thread's initialization.
        drop(collection);
        drop(enumerator);
        drop(apartment);
        Ok(result)
    }
}
#[cfg(not(target_os = "windows"))]
pub fn endpoints() -> Result<Vec<(String, String)>, String> {
    Ok(vec![])
}

pub fn unique_id(entries: &[(String, String)], name: &str) -> Option<String> {
    let mut matching = entries.iter().filter(|(n, _)| n == name);
    let (_, id) = matching.next()?;
    (matching.next().is_none()
        && !id.is_empty()
        && id.len() <= 2048
        && !id.chars().any(char::is_control))
    .then(|| id.clone())
}

pub fn unique_name<'a>(entries: &'a [(String, String)], endpoint_id: &str) -> Option<&'a str> {
    let mut matching = entries.iter().filter(|(_, id)| id == endpoint_id);
    let (name, id) = matching.next()?;
    (matching.next().is_none()
        && !name.is_empty()
        && unique_id(entries, name).as_deref() == Some(id.as_str()))
    .then_some(name.as_str())
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires the attached MC4000; opens its output without playback"]
    fn attached_mc4000_endpoint_matches_opened_profile() {
        let entries = super::endpoints().unwrap();
        let expected =
            super::unique_id(&entries, "Lautsprecher (MC4000)").expect("unique attached MC4000");
        let selector = crate::audio_devices::DeviceSelector {
            backend: "WASAPI".into(),
            name: "Lautsprecher (MC4000)".into(),
            endpoint_id: Some(expected.clone()),
        };
        let engine = crate::playback::DualDeckEngine::open_selected(Some(&selector)).unwrap();
        let key = engine.output_device_key().unwrap();
        assert_eq!(key.endpoint_id.as_deref(), Some(expected.as_str()));
        assert_eq!(key.channels, 4);
        assert!(!engine.snapshot(crate::playback::DeckId::A).playing);
        assert!(!engine.snapshot(crate::playback::DeckId::B).playing);
        println!("MC4000 profile matches native endpoint identity; no playback started");
    }
    #[test]
    fn endpoint_mapping_rejects_missing_duplicate_or_invalid_identity() {
        use super::unique_id;
        let entry = ("Speakers".to_string(), "endpoint-a".to_string());
        assert_eq!(
            unique_id(std::slice::from_ref(&entry), "Speakers"),
            Some("endpoint-a".into())
        );
        assert_eq!(unique_id(std::slice::from_ref(&entry), "Missing"), None);
        assert_eq!(
            unique_id(
                &[entry.clone(), (entry.0.clone(), "endpoint-b".into())],
                "Speakers"
            ),
            None
        );
        assert_eq!(
            unique_id(&[("Speakers".into(), "bad\nid".into())], "Speakers"),
            None
        );
        assert_eq!(
            super::unique_name(std::slice::from_ref(&entry), "endpoint-a"),
            Some("Speakers")
        );
        assert_eq!(
            super::unique_name(std::slice::from_ref(&entry), "missing"),
            None
        );
        assert_eq!(
            super::unique_name(
                &[entry.clone(), ("Other".into(), "endpoint-a".into())],
                "endpoint-a"
            ),
            None
        );
        assert_eq!(
            super::unique_name(
                &[entry.clone(), ("Speakers".into(), "endpoint-b".into())],
                "endpoint-a"
            ),
            None
        );
    }
}
