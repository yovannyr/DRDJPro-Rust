use std::path::Path;
use std::io::Cursor;

use image::{ImageReader, ImageFormat, Limits};

/// Maximum allowed input size (4 MiB).
const MAX_INPUT_SIZE: usize = 4 * 1024 * 1024;

/// Maximum allowed width or height for the decoded cover.
const MAX_DIMENSION: u32 = 1024;

/// Maximum memory allocation for image decoding (32 MiB, best-effort).
const MAX_ALLOC: u64 = 32 * 1024 * 1024;

/// Compute a dominant ARGB color from RGBA pixel data.
///
/// Samples pixels at a regular interval to keep the computation cheap,
/// returns 0x00000000 (fully transparent) when data is empty or sampling fails.
pub(crate) fn dominant_color(data: &[u8]) -> u32 {
    if data.is_empty() || data.len() < 4 {
        return 0x00000000;
    }

    // Sample every 64th pixel to keep it fast even for large images.
    let step = 64usize;
    let len = data.len() / 16; // number of pixels
    let count = (len + step - 1) / step;
    let mut r: u32 = 0;
    let mut g: u32 = 0;
    let mut b: u32 = 0;
    let mut a: u32 = 0;

    for i in (0..len).step_by(step) {
        let ri = data[i * 4];
        let gi = data[i * 4 + 1];
        let bi = data[i * 4 + 2];
        let ai = data[i * 4 + 3];
        r += ri as u32;
        g += gi as u32;
        b += bi as u32;
        a += ai as u32;
    }

    let r = (r / count as u32) as u8;
    let g = (g / count as u32) as u8;
    let b = (b / count as u32) as u8;
    let a = (a / count as u32) as u8;

    // Return as ARGB (matches Slint color format)
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Decode embedded cover-art data into (width, height, RGBA pixel buffer).
///
/// # Limits
/// - Input must not exceed 4 MiB.
/// - Only PNG and JPEG are accepted (format is guessed from magic bytes).
/// - Decoded image width and height must not exceed 1024 px each.
/// - The image crate allocation limit is set to 32 MiB (best-effort).
///
/// # Errors
/// Returns a descriptive error string for empty, corrupt, oversized,
/// or otherwise unsupported input - never panics.
pub(crate) fn decode(data: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if data.is_empty() {
        return Err("cover data is empty".into());
    }
    if data.len() > MAX_INPUT_SIZE {
        return Err("cover data exceeds 4 MiB limit".into());
    }

    // Step 1: Guess format via with_guessed_format().
    let guessed = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("failed to guess image format: {e}"))?;

    // Step 2: Extract the detected format (returns Option).
    let format = guessed
        .format()
        .ok_or_else(|| "could not determine image format".to_string())?;

    match format {
        ImageFormat::Png | ImageFormat::Jpeg => {}
        other => {
            return Err(format!(
                "cover format {:?} is not supported (only PNG/JPEG)",
                other
            ))
        }
    }

    // Step 3: Configure strict limits before decoding.
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_ALLOC);

    // Step 4: Apply limits (returns ()) then decode.
    let img = {
        let cursor = Cursor::new(data);
        let mut reader = ImageReader::with_format(cursor, format);
        reader.limits(limits);
        reader.decode().map_err(|e| {
            format!("failed to decode cover image: {e}")
        })?
    };

    let width = img.width();
    let height = img.height();

    Ok((width, height, img.into_rgba8().into_raw()))
}

/// Read cover art from an audio file, cancelling cooperatively.
///
/// Uses media_input::probe with bounded metadata and visual byte limits.
/// Prefers front-cover images, falls back to the first available image.
/// Returns Ok(None) when no cover image is present, Err on real failures.
///
/// # Cancellation
/// Checks cancel before probing, between probe-and-decode, and after decode.
///
/// # Limitation
/// Symphonia silently ignores images that would exceed limit_visual_bytes.
/// If the probe returns warnings about tag/image truncation, a valid cover
/// may simply not have been parsed. The caller should surface such warnings.
pub(crate) fn read_cancellable(
    path: &Path,
    cancel: &crate::cancellation::Cancellation,
) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
    cancel.check()?;

    let options = symphonia::core::meta::MetadataOptions {
        limit_metadata_bytes: symphonia::core::meta::Limit::Maximum(512 * 1024),
        limit_visual_bytes: symphonia::core::meta::Limit::Maximum(MAX_INPUT_SIZE),
    };

    let (mut probed, warning) = crate::media_input::probe(path, &options)
        .map_err(|e| format!("probe failed for {:?}: {e}", path))?;

    cancel.check()?;

    if let Some(w) = &warning {
        eprintln!("cover_art: probe warning for {:?}: {}", path, w);
    }

    let mut last_err: Option<String> = None;

    // Iterate through available metadata revisions (up to revision 1).
    if let Some(metadata) = probed.metadata.get() {
        if let Some(revision) = metadata.current() {
            let visuals: Vec<_> = revision.visuals().to_vec();
            if !visuals.is_empty() {
                // Prefer FrontCover, fallback to first image.
                let cover = visuals
                    .iter()
                    .find(|img| img.usage == Some(symphonia::core::meta::StandardVisualKey::FrontCover))
                    .or_else(|| visuals.first());

                if let Some(cover) = cover {
                    if cover.data.is_empty() {
                        last_err = Some("cover image data is empty".into());
                    } else {
                        cancel.check()?;

                        match decode(&cover.data) {
                            Ok(result) => return Ok(Some(result)),
                            Err(e) => {
                                last_err = Some(format!("failed to decode cover image: {e}"));
                            }
                        }
                    }
                }
            }
        }
    }

    cancel.check()?;

    // No cover art found. If we had decode failures, surface them via Err.
    if let Some(e) = last_err {
        Err(e)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen_2x2_png() -> Vec<u8> {
        let mut buf = Vec::new();
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = image::ImageBuffer::new(2, 2);
        img.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    fn gen_2x2_jpeg() -> Vec<u8> {
        let mut buf = Vec::new();
        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> = image::ImageBuffer::new(2, 2);
        img.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Jpeg)
            .unwrap();
        buf
    }

    fn gen_1025x1_png() -> Vec<u8> {
        let mut buf = Vec::new();
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = image::ImageBuffer::new(1025, 1);
        img.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn test_decode_2x2_png() {
        let data = gen_2x2_png();
        let (w, h, pixels) = decode(&data).unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(pixels.len(), (2 * 2 * 4) as usize);
    }

    #[test]
    fn test_decode_2x2_jpeg() {
        let data = gen_2x2_jpeg();
        let (w, h, pixels) = decode(&data).unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(pixels.len(), (2 * 2 * 4) as usize);
    }

    #[test]
    fn test_decode_empty() {
        let result = decode(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_decode_corrupt() {
        let result = decode(&[0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_unsupported() {
        let result = decode(b"NOT AN IMAGE AT ALL");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_4mib_plus_1() {
        let data = vec![0u8; MAX_INPUT_SIZE + 1];
        let result = decode(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds"));
    }

    #[test]
    fn test_decode_1025px_width() {
        let data = gen_1025x1_png();
        let result = decode(&data);
        assert!(result.is_err());
    }

    // --- ID3v2.3 APIC-frame helpers ---

    /// Encode a size as a 4-byte synchsafe integer (ID3v2.3).
    /// Each byte carries 7 bits of data (highest bit always 0).
    fn size_v23(value: usize) -> [u8; 4] {
        [
            ((value >> 21) & 0x7F) as u8,
            ((value >> 14) & 0x7F) as u8,
            ((value >> 7) & 0x7F) as u8,
            (value & 0x7F) as u8,
        ]
    }

    fn apic_payload(png_data: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.push(0); // encoding UTF-8
        payload.push(0); // MIME type none
        payload.push(3); // picture type 3 = Front Cover
        payload.extend_from_slice(b"cover\0"); // null-terminated description
        payload.extend_from_slice(png_data); // image data
        payload
    }

    fn tagged_mp3_with_cover(title: &str, cover_data: &[u8]) -> Vec<u8> {
        let mp3_payload = std::fs::read(crate::test_support::fixture()).unwrap();
        let mut tags = Vec::new();

        fn make_text_frame_v23(id: &[u8], value: &str, out: &mut Vec<u8>) {
            let mut payload = vec![3];
            payload.extend_from_slice(value.as_bytes());
            out.extend_from_slice(id);
            out.extend_from_slice(&size_v23(payload.len()));
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(&payload);
        }

        make_text_frame_v23(b"TIT2", title, &mut tags);
        make_text_frame_v23(b"TPE1", "Cover Test Artist", &mut tags);
        make_text_frame_v23(b"TALB", "Cover Album", &mut tags);

        let apic = apic_payload(cover_data);
        tags.extend_from_slice(b"APIC");
        tags.extend_from_slice(&size_v23(apic.len()));
        tags.extend_from_slice(&[0, 0]);
        tags.extend_from_slice(&apic);

        let mut bytes = b"ID3\x03\x03\x00".to_vec();
        bytes.extend_from_slice(&size_v23(tags.len()));
        bytes.extend_from_slice(&tags);
        bytes.extend_from_slice(&mp3_payload);
        bytes
    }

    fn tagged_mp3_with_corrupt_cover(title: &str) -> Vec<u8> {
        let corrupt_png: Vec<u8> = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        ]
        .to_vec();
        tagged_mp3_with_cover(title, &corrupt_png)
    }

    fn tagged_mp3_no_cover(title: &str) -> Vec<u8> {
        let mp3_payload = std::fs::read(crate::test_support::fixture()).unwrap();
        let mut tags = Vec::new();

        let mut make_text_frame = |id: &[u8], value: &str| {
            let mut payload = vec![3];
            payload.extend_from_slice(value.as_bytes());
            tags.extend_from_slice(id);
            tags.extend_from_slice(&size_v23(payload.len()));
            tags.extend_from_slice(&[0, 0]);
            tags.extend_from_slice(&payload);
        };

        make_text_frame(b"TIT2", title);
        make_text_frame(b"TPE1", "No Cover Artist");
        make_text_frame(b"TALB", "No Cover Album");

        let mut bytes = b"ID3\x03\x03\x00".to_vec();
        bytes.extend_from_slice(&size_v23(tags.len()));
        bytes.extend_from_slice(&tags);
        bytes.extend_from_slice(&mp3_payload);
        bytes
    }

    fn tagged_mp3_with_oversized_cover(title: &str) -> Vec<u8> {
        let oversized_png: Vec<u8> = vec![0x89; MAX_INPUT_SIZE + 1];
        tagged_mp3_with_cover(title, &oversized_png)
    }

    #[test]
    fn test_read_cancellable_embedded_cover() {
        let cover_data = gen_2x2_png();
        let mp3_bytes = tagged_mp3_with_cover("Embedded Cover Track", &cover_data);
        let file = crate::test_support::TempFile::new("mp3", &mp3_bytes);

        let cancel = crate::cancellation::Cancellation::default();
        let result = read_cancellable(file.path(), &cancel);

        assert!(result.is_ok(), "read_cancellable should succeed: {:?}", result);
        let cover = result.unwrap();
        assert!(cover.is_some(), "should find embedded cover art");

        let (w, h, pixels) = cover.unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(pixels.len(), 16);
    }

    #[test]
    fn test_read_cancellable_no_cover_returns_none() {
        let mp3_bytes = tagged_mp3_no_cover("No Cover Track");
        let file = crate::test_support::TempFile::new("mp3", &mp3_bytes);

        let cancel = crate::cancellation::Cancellation::default();
        let result = read_cancellable(file.path(), &cancel);

        assert!(result.is_ok(), "should not error: {:?}", result);
        assert!(
            result.unwrap().is_none(),
            "should return None when no cover is present"
        );
    }

    #[test]
    fn test_read_cancellable_corrupt_cover_returns_err() {
        let mp3_bytes = tagged_mp3_with_corrupt_cover("Corrupt Cover Track");
        let file = crate::test_support::TempFile::new("mp3", &mp3_bytes);

        let cancel = crate::cancellation::Cancellation::default();
        let result = read_cancellable(file.path(), &cancel);

        assert!(result.is_err(), "should return Err for corrupt cover");
    }

    #[test]
    fn test_read_cancellable_oversized_cover_returns_err() {
        let mp3_bytes = tagged_mp3_with_oversized_cover("Oversized Cover Track");
        let file = crate::test_support::TempFile::new("mp3", &mp3_bytes);

        let cancel = crate::cancellation::Cancellation::default();
        let result = read_cancellable(file.path(), &cancel);

        assert!(result.is_err(), "should return Err for oversized cover");
    }

    #[test]
    fn test_read_cancellable_missing_file_returns_err() {
        let cancel = crate::cancellation::Cancellation::default();
        let result = read_cancellable(Path::new("nonexistent-audio.mp3"), &cancel);

        assert!(result.is_err(), "should return Err for missing file");
    }

    #[test]
    fn test_read_cancellable_cancelled_before_probe() {
        let mp3_bytes = gen_2x2_png();
        let file = crate::test_support::TempFile::new("mp3", &mp3_bytes);

        let cancel = crate::cancellation::Cancellation::default();
        cancel.request();

        let result = read_cancellable(file.path(), &cancel);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cancelled"));
    }
}

