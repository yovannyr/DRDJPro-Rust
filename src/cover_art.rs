use std::io::Cursor;
use std::path::Path;

use image::{ImageFormat, ImageReader, Limits};

/// Maximum allowed input size (4 MiB).
const MAX_INPUT_SIZE: usize = 4 * 1024 * 1024;

/// Maximum allowed width or height for the decoded cover.
const MAX_DIMENSION: u32 = 4096;
/// All consumers share this bounded thumbnail rather than full-size pixels.
pub(crate) const THUMBNAIL_SIZE: u32 = 128;

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
    let len = data.len() / 4; // number of RGBA pixels
    let count = len.div_ceil(step);
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
/// - Source dimensions must not exceed 4096 px; decoding is allocation-limited.
/// - Output fits within 128 × 128 px without distortion or upscaling.
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
        reader
            .decode()
            .map_err(|e| format!("failed to decode cover image: {e}"))?
    };

    let img = if img.width() > THUMBNAIL_SIZE || img.height() > THUMBNAIL_SIZE {
        img.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE)
    } else {
        img
    };
    let width = img.width();
    let height = img.height();

    Ok((width, height, img.into_rgba8().into_raw()))
}

/// Read cover art from an audio file, cancelling cooperatively.
///
/// Requests metadata/visual limits from media_input::probe; not every upstream
/// reader enforces those options. The output/input checks in `decode` do not
/// establish a hard bound on upstream metadata-parser allocations.
/// Prefers front-cover images, falls back to the first available image.
/// Returns Ok(None) when no cover image is present, Err on real failures.
///
/// # Cancellation
/// Checks cancel before probing, between probe-and-decode, and after decode.
///
/// # Limitation
/// Readers honoring limit_visual_bytes may omit oversized visuals.
/// Other readers ignore that option and may allocate large tag/frame
/// payloads before this function can reject an excessive encoded picture.
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

    // ID3 metadata can belong to the probe; MP4/FLAC artwork can belong to
    // the format reader. Borrow both sets without cloning compressed images.
    let metadata = probed.metadata.get();
    let format_metadata = probed.format.metadata();
    let visuals: Vec<_> = metadata
        .as_ref()
        .and_then(|m| m.current())
        .into_iter()
        .chain(format_metadata.current())
        .flat_map(|revision| revision.visuals())
        .collect();
    for front in [true, false] {
        for cover in visuals.iter().filter(|cover| {
            (cover.usage == Some(symphonia::core::meta::StandardVisualKey::FrontCover)) == front
        }) {
            cancel.check()?;
            match decode(&cover.data) {
                Ok(result) => {
                    cancel.check()?;
                    return Ok(Some(result));
                }
                Err(error) => last_err = Some(format!("failed to decode cover image: {error}")),
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
pub(crate) mod tests {
    use super::*;

    #[test]
    fn dominant_color_samples_the_whole_rgba_buffer() {
        let mut pixels = [255, 0, 0, 255].repeat(64);
        pixels.extend([0, 0, 255, 255].repeat(64));
        assert_eq!(dominant_color(&pixels), 0xff7f007f);
        assert_eq!(dominant_color(&[]), 0);
    }

    pub(crate) fn embedded_cover_file() -> crate::test_support::TempFile {
        let mut bytes = Vec::new();
        image::RgbaImage::from_pixel(4, 2, image::Rgba([220, 30, 40, 255]))
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        crate::test_support::TempFile::new("mp3", &tagged_mp3_with_cover("Visible Cover", &bytes))
    }

    #[test]
    fn flac_picture_from_format_metadata_is_extracted() {
        let original = std::fs::read("tests/fixtures/format-flac.flac").unwrap();
        let mut bytes = original.clone();
        let mut offset = 4;
        loop {
            let last = bytes[offset] & 0x80 != 0;
            let len = ((bytes[offset + 1] as usize) << 16)
                | ((bytes[offset + 2] as usize) << 8)
                | bytes[offset + 3] as usize;
            if last {
                bytes[offset] &= 0x7f;
                offset += 4 + len;
                break;
            }
            offset += 4 + len;
        }
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(4, 2, image::Rgba([220, 30, 40, 255]))
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        let mut picture = Vec::new();
        picture.extend(3u32.to_be_bytes());
        picture.extend(9u32.to_be_bytes());
        picture.extend(b"image/png");
        for value in [0u32, 4, 2, 32, 0, png.len() as u32] {
            picture.extend(value.to_be_bytes());
        }
        picture.extend(png);
        bytes.truncate(offset);
        bytes.push(0x86);
        bytes.extend(&((picture.len() as u32).to_be_bytes())[1..]);
        bytes.extend(picture);
        bytes.extend(&original[offset..]);
        let file = crate::test_support::TempFile::new("flac", &bytes);
        let (w, h, pixels) = read_cancellable(file.path(), &Default::default())
            .unwrap()
            .unwrap();
        assert_eq!((w, h), (4, 2));
        assert_eq!(&pixels[..4], &[220, 30, 40, 255]);
    }

    #[test]
    fn thumbnail_retains_color_and_bounds_portrait_landscape_and_oversize() {
        for (w, h) in [(2048, 1024), (1024, 2048), (4097, 1)] {
            let mut bytes = Vec::new();
            image::RgbaImage::from_pixel(w, h, image::Rgba([20, 80, 180, 255]))
                .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
                .unwrap();
            if w > MAX_DIMENSION {
                assert!(decode(&bytes).is_err());
                continue;
            }
            let (tw, th, pixels) = decode(&bytes).unwrap();
            assert!(tw <= 128 && th <= 128);
            assert_eq!(tw * h, th * w);
            assert_eq!(pixels.len(), (tw * th * 4) as usize);
            assert_eq!(dominant_color(&pixels), 0xff1450b4);
        }
    }

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
    fn larger_art_is_downscaled_and_keeps_aspect_ratio() {
        let data = gen_1025x1_png();
        let result = decode(&data);
        let (w, h, pixels) = result.unwrap();
        assert_eq!((w, h), (128, 1));
        assert_eq!(pixels.len(), 128 * 4);
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
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52,
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

        assert!(
            result.is_ok(),
            "read_cancellable should succeed: {:?}",
            result
        );
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
