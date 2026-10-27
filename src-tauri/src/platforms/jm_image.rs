//! Undo JM's page-image scrambling before the bytes are cached.
//!
//! JM cuts every page into horizontal bands and stores them bottom-up. The
//! band count comes from the photo id and file name; the threshold comes from
//! the album's `scramble_id`. Reversing it is a pure row permutation, but the
//! image has to be decoded and re-encoded because the bands are not aligned to
//! JPEG/WebP blocks.

use image::RgbImage;
use md5::{Digest, Md5};

use crate::error::{AppError, Result};

const SCRAMBLE_268850: i64 = 268_850;
const SCRAMBLE_421926: i64 = 421_926;
/// A decoded page is ~900 KiB as PNG and ~200 KiB as JPEG at this quality.
/// The cache is disposable, but the smaller footprint matters for a chapter.
const JPEG_QUALITY: u8 = 92;

/// Number of bands the page was cut into. `0` means the album predates the
/// scrambling threshold and the bytes can be stored as downloaded.
pub(crate) fn segment_count(scramble_id: i64, photo_id: &str, filename: &str) -> u32 {
    let Ok(photo_id) = photo_id.parse::<i64>() else {
        return 0;
    };
    if photo_id < scramble_id {
        return 0;
    }
    if photo_id < SCRAMBLE_268850 {
        return 10;
    }

    let modulus = if photo_id < SCRAMBLE_421926 { 10 } else { 8 };
    let digest = md5_hex(&format!("{photo_id}{filename}"));
    // Upstream hashes the *hex digest* and uses its last character, not the
    // last byte of the digest; the two produce different band counts.
    let last = digest.as_bytes()[digest.len() - 1] as u32;
    (last % modulus) * 2 + 2
}

/// Decode, reorder and re-encode one page image.
pub(crate) fn descramble(bytes: &[u8], segments: u32) -> Result<Vec<u8>> {
    if segments == 0 {
        return Ok(bytes.to_vec());
    }

    let decoded = image::load_from_memory(bytes)
        .map_err(|error| AppError::Unexpected(format!("failed to decode a jm page: {error}")))?;
    let reordered = reorder(&decoded.to_rgb8(), segments);

    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, JPEG_QUALITY)
        .encode_image(&reordered)
        .map_err(|error| AppError::Unexpected(format!("failed to encode a jm page: {error}")))?;

    Ok(encoded)
}

/// Move the bottom band to the top, then every band above it in order. Band
/// `0` also carries the remainder rows, which is why its height differs.
fn reorder(image: &RgbImage, segments: u32) -> RgbImage {
    let (width, height) = image.dimensions();
    if segments == 0 || height < segments || width == 0 {
        return image.clone();
    }

    let row_bytes = width as usize * 3;
    let source = image.as_raw();
    let mut output = vec![0u8; row_bytes * height as usize];

    let remainder = height % segments;
    let band = height / segments;

    for index in 0..segments {
        let source_y = height - band * (index + 1) - remainder;
        let rows = if index == 0 { band + remainder } else { band };
        let mut target_y = band * index;
        if index != 0 {
            target_y += remainder;
        }

        let start = source_y as usize * row_bytes;
        let length = rows as usize * row_bytes;
        let target = target_y as usize * row_bytes;
        output[target..target + length].copy_from_slice(&source[start..start + length]);
    }

    RgbImage::from_raw(width, height, output)
        .expect("the reordered buffer keeps the source dimensions")
}

fn md5_hex(input: &str) -> String {
    hex::encode(Md5::digest(input.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn striped(height: u32) -> RgbImage {
        ImageBuffer::from_fn(1, height, |_, y| Rgb([y as u8, 0, 0]))
    }

    fn rows(image: &RgbImage) -> Vec<u8> {
        image.pixels().map(|pixel| pixel[0]).collect()
    }

    #[test]
    fn segment_count_follows_the_album_thresholds() {
        // Before the threshold: stored unscrambled.
        assert_eq!(segment_count(220_980, "150000", "00001"), 0);
        // Old albums: always ten bands.
        assert_eq!(segment_count(220_980, "250000", "00001"), 10);
        // Known vector: 350235 / 00001.
        assert_eq!(segment_count(220_980, "350235", "00001"), 10);
        // Newer albums hash the name into an even band count.
        let newer = segment_count(220_980, "500000", "00001");
        assert!((2..=16).contains(&newer));
        assert_eq!(newer % 2, 0);

        assert_eq!(segment_count(220_980, "not-a-number", "00001"), 0);
    }

    #[test]
    fn reorders_bands_bottom_up() {
        let reordered = reorder(&striped(10), 5);
        assert_eq!(rows(&reordered), vec![8, 9, 6, 7, 4, 5, 2, 3, 0, 1]);
    }

    #[test]
    fn the_first_band_absorbs_the_remainder() {
        let reordered = reorder(&striped(11), 5);
        assert_eq!(rows(&reordered), vec![8, 9, 10, 6, 7, 4, 5, 2, 3, 0, 1]);
    }

    #[test]
    fn keeps_images_that_are_shorter_than_the_band_count() {
        let image = striped(3);
        assert_eq!(rows(&reorder(&image, 10)), rows(&image));
    }

    #[test]
    fn descramble_is_a_noop_without_bands() {
        assert_eq!(descramble(b"raw-bytes", 0).unwrap(), b"raw-bytes");
    }

    #[test]
    fn descramble_re_encodes_an_image_with_the_same_dimensions() {
        let source = striped(20);
        let mut png = Vec::new();
        source
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();

        let decoded = descramble(&png, 10).unwrap();
        let result = image::load_from_memory(&decoded).unwrap();
        assert_eq!((result.width(), result.height()), (1, 20));
    }

    #[test]
    fn descramble_reports_broken_payloads() {
        let error = descramble(b"not an image", 10).unwrap_err();
        assert_eq!(error.code(), crate::error::ErrorCode::Unexpected);
    }
}
