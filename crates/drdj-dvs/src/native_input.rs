use crate::{InputProducer, InputStream};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, Stream,
};

/// Native formats explicitly supported by the bounded conversion path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputFormat {
    F32,
    I16,
    U16,
}
/// Current default-host selection. Duplicate OS names are rejected, never guessed.
/// The first stereo pair (channels 1/2) is used; no channels are mixed together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputDevice {
    pub name: String,
    pub channels: u16,
    pub rate: u32,
    pub format: InputFormat,
}
/// Supported input metadata plus per-device errors; one broken input does not
/// hide the remaining devices. Unsupported formats are listed as diagnostics.
#[derive(Clone, Debug, Default)]
pub struct InputInventory {
    pub devices: Vec<InputDevice>,
    pub errors: Vec<String>,
}
fn describe(name: String, config: &cpal::SupportedStreamConfig) -> Option<InputDevice> {
    let format = match config.sample_format() {
        SampleFormat::F32 => InputFormat::F32,
        SampleFormat::I16 => InputFormat::I16,
        SampleFormat::U16 => InputFormat::U16,
        _ => return None,
    };
    if !(2..=32).contains(&config.channels()) || !(8000..=192000).contains(&config.sample_rate().0)
    {
        return None;
    }
    Some(InputDevice {
        name,
        channels: config.channels(),
        rate: config.sample_rate().0,
        format,
    })
}
/// Read input metadata only. Enumeration must be called on a control worker.
/// No streams are opened or started by this function.
pub fn enumerate_inputs() -> Result<InputInventory, String> {
    let mut inventory = InputInventory::default();
    for device in cpal::default_host()
        .input_devices()
        .map_err(|error| error.to_string())?
    {
        match (device.name(), device.default_input_config()) {
            (Ok(name), Ok(config)) => match describe(name.clone(), &config) {
                Some(device) => inventory.devices.push(device),
                None => inventory
                    .errors
                    .push(format!("{name}: unsupported input format/channels/rate")),
            },
            (name, config) => inventory
                .errors
                .push(format!("Input metadata unavailable: {name:?}, {config:?}")),
        }
    }
    Ok(inventory)
}
pub(crate) fn open(selected: InputDevice, mut producer: InputProducer) -> Result<Stream, String> {
    let mut matches = Vec::new();
    for device in cpal::default_host()
        .input_devices()
        .map_err(|error| error.to_string())?
    {
        if device.name().map_err(|error| error.to_string())? == selected.name {
            matches.push(device);
        }
    }
    if matches.len() != 1 {
        return Err("Input selection missing or ambiguous; refresh devices".into());
    }
    let device = matches.pop().ok_or("Input not found")?;
    let config = device
        .default_input_config()
        .map_err(|error| error.to_string())?;
    if describe(selected.name.clone(), &config).as_ref() != Some(&selected) {
        return Err("Input configuration changed; refresh devices".into());
    }
    let channels = usize::from(selected.channels);
    let failure = producer.failure();
    let errors = move |_: cpal::StreamError| failure.signal();
    let stream = match selected.format {
        InputFormat::F32 => device.build_input_stream(
            &config.into(),
            move |pcm: &[f32], _| {
                producer.interleaved(pcm, channels, |sample| {
                    sample
                        .is_finite()
                        .then(|| (sample.clamp(-1.0, 1.0) * 32767.0) as i16)
                });
            },
            errors,
            None,
        ),
        InputFormat::I16 => device.build_input_stream(
            &config.into(),
            move |pcm: &[i16], _| {
                producer.interleaved(pcm, channels, Some);
            },
            errors,
            None,
        ),
        InputFormat::U16 => device.build_input_stream(
            &config.into(),
            move |pcm: &[u16], _| {
                producer.interleaved(pcm, channels, |sample| {
                    Some((i32::from(sample) - 32768) as i16)
                });
            },
            errors,
            None,
        ),
    }
    .map_err(|error| error.to_string())?;
    Ok(stream)
}
impl InputStream for Stream {
    fn start(&mut self) -> Result<(), String> {
        self.play().map_err(|error| error.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_observed_config_determines_decoder_and_stream_shape() {
        let config = cpal::SupportedStreamConfig::new(
            4,
            cpal::SampleRate(48000),
            cpal::SupportedBufferSize::Unknown,
            SampleFormat::F32,
        );
        let device = describe("fixture".into(), &config).unwrap();
        assert_eq!(device.channels, 4);
        assert_eq!(device.rate, 48000);
        assert_eq!(device.format, InputFormat::F32);
        for (channels, rate, format) in [
            (1, 48000, SampleFormat::I16),
            (2, 4000, SampleFormat::F32),
            (2, 48000, SampleFormat::F64),
        ] {
            assert!(describe(
                "unsupported".into(),
                &cpal::SupportedStreamConfig::new(
                    channels,
                    cpal::SampleRate(rate),
                    cpal::SupportedBufferSize::Unknown,
                    format
                )
            )
            .is_none());
        }
    }
}
