//! Photograph import. The stored file is a PNG with the EXIF orientation
//! already applied. GPS stays out of the file. The caller keeps latitude and
//! longitude only when the person asked for them.

use std::io::Cursor;
use std::path::Path;

use exif::{In, Reader, Tag};
use image::DynamicImage;

const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug)]
pub struct PreparedPhoto {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub filename: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

pub fn prepare_photo(bytes: &[u8], filename: &str) -> Result<PreparedPhoto, String> {
    if bytes.is_empty() {
        return Err("photo-empty".into());
    }
    if bytes.len() > MAX_BYTES {
        return Err("photo-large".into());
    }
    classify(bytes)?;
    let located = read_location(bytes);
    let decoded = image::load_from_memory(bytes).map_err(|_| "photo-decode".to_string())?;
    let oriented = orient(decoded, read_orientation(bytes));
    let width = oriented.width();
    let height = oriented.height();
    if width == 0 || height == 0 {
        return Err("photo-decode".into());
    }
    let mut png = Vec::new();
    oriented
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|_| "photo-decode".to_string())?;
    if png.len() > MAX_BYTES || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("photo-large".into());
    }
    Ok(PreparedPhoto {
        png,
        width,
        height,
        filename: photo_filename(filename),
        latitude: located.0,
        longitude: located.1,
    })
}

pub fn raster_from_bytes(bytes: &[u8]) -> Result<holdover_core::Raster, String> {
    let image = image::load_from_memory(bytes).map_err(|_| "photo-decode".to_string())?;
    let gray = image.to_luma8();
    Ok(holdover_core::Raster {
        width: gray.width(),
        height: gray.height(),
        gray: gray.into_raw(),
    })
}

fn classify(bytes: &[u8]) -> Result<(), String> {
    if is_heic(bytes) {
        return Err("photo-heic".into());
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) || bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(());
    }
    Err("photo-type".into())
}

fn is_heic(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || &bytes[4..8] != b"ftyp" {
        return false;
    }
    let end = bytes.len().min(64);
    let brands = &bytes[8..end];
    const CODES: [&[u8; 4]; 6] = [b"heic", b"heif", b"mif1", b"heix", b"heim", b"heis"];
    brands
        .windows(4)
        .any(|window| CODES.iter().any(|code| window == code.as_slice()))
}

fn read_orientation(bytes: &[u8]) -> u32 {
    let Ok(exif) = Reader::new().read_from_container(&mut Cursor::new(bytes)) else {
        return 1;
    };
    exif.get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
        .unwrap_or(1)
}

fn read_location(bytes: &[u8]) -> (Option<f64>, Option<f64>) {
    let Ok(exif) = Reader::new().read_from_container(&mut Cursor::new(bytes)) else {
        return (None, None);
    };
    (
        gps_degrees(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef),
        gps_degrees(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef),
    )
}

fn gps_degrees(exif: &exif::Exif, tag: Tag, reference: Tag) -> Option<f64> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    let coords = match &field.value {
        exif::Value::Rational(items) if items.len() >= 3 => items,
        _ => return None,
    };
    let degrees = coords[0].to_f64() + coords[1].to_f64() / 60.0 + coords[2].to_f64() / 3600.0;
    if !degrees.is_finite() {
        return None;
    }
    let text = exif
        .get_field(reference, In::PRIMARY)
        .map(|item| item.display_value().to_string())
        .unwrap_or_default();
    let negative = text.contains('S') || text.contains('W');
    Some(if negative { -degrees } else { degrees })
}

fn orient(image: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.fliph().rotate270(),
        6 => image.rotate90(),
        7 => image.fliph().rotate90(),
        8 => image.rotate270(),
        _ => image,
    }
}

fn photo_filename(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|item| item.to_str())
        .unwrap_or("photo");
    let stem = base.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(base);
    let clean: String = stem
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .take(80)
        .collect();
    let stem = if clean.is_empty() { "photo" } else { &clean };
    format!("{stem}.png")
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};

    fn jpeg_wide() -> Vec<u8> {
        let image = RgbImage::from_pixel(8, 4, Rgb([240, 240, 240]));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .unwrap();
        bytes
    }

    fn exif_orientation_3_and_gps() -> Vec<u8> {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&42u16.to_le_bytes());
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&2u16.to_le_bytes());
        push_entry(&mut tiff, 0x0112, 3, 1, 6);
        push_entry(&mut tiff, 0x8825, 4, 1, 38);
        tiff.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(tiff.len(), 38);
        tiff.extend_from_slice(&4u16.to_le_bytes());
        push_entry(&mut tiff, 0x0001, 2, 2, u32::from_le_bytes(*b"N\0\0\0"));
        push_entry(&mut tiff, 0x0002, 5, 3, 92);
        push_entry(&mut tiff, 0x0003, 2, 2, u32::from_le_bytes(*b"E\0\0\0"));
        push_entry(&mut tiff, 0x0004, 5, 3, 116);
        tiff.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(tiff.len(), 92);
        for (num, den) in [(59u32, 1u32), (20, 1), (0, 1), (18, 1), (3, 1), (30, 1)] {
            tiff.extend_from_slice(&num.to_le_bytes());
            tiff.extend_from_slice(&den.to_le_bytes());
        }
        let mut app = b"Exif\0\0".to_vec();
        app.extend_from_slice(&tiff);
        let length = (app.len() + 2) as u16;
        let mut segment = vec![0xFF, 0xE1];
        segment.extend_from_slice(&length.to_be_bytes());
        segment.extend_from_slice(&app);
        segment
    }

    fn push_entry(out: &mut Vec<u8>, tag: u16, kind: u16, count: u32, value: u32) {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn jpeg_with_exif() -> Vec<u8> {
        let jpeg = jpeg_wide();
        let mut out = vec![0xFF, 0xD8];
        out.extend_from_slice(&exif_orientation_3_and_gps());
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    #[test]
    fn jpeg_gps_is_read_and_stripped_and_orientation_is_applied() {
        let prepared = prepare_photo(&jpeg_with_exif(), "../range/shot.jpg").unwrap();
        assert_eq!(prepared.filename, "shot.png");
        let lat = prepared.latitude.unwrap();
        let lon = prepared.longitude.unwrap();
        assert!((lat - (59.0 + 20.0 / 60.0)).abs() < 1e-6, "{lat}");
        assert!(
            (lon - (18.0 + 3.0 / 60.0 + 30.0 / 3600.0)).abs() < 1e-6,
            "{lon}"
        );
        assert!(Reader::new()
            .read_from_container(&mut Cursor::new(prepared.png.as_slice()))
            .is_err());
        assert_eq!(prepared.width, 4, "orientation 6 rotates a wide frame");
        assert_eq!(prepared.height, 8);
    }

    #[test]
    fn heic_and_unknown_bytes_are_refused() {
        let heic = b"\x00\x00\x00\x18ftypheic\x00\x00\x00\x00mif1";
        assert_eq!(prepare_photo(heic, "a.heic").unwrap_err(), "photo-heic");
        assert_eq!(
            prepare_photo(b"not a photo", "a.txt").unwrap_err(),
            "photo-type"
        );
        assert_eq!(prepare_photo(b"", "a.jpg").unwrap_err(), "photo-empty");
    }

    #[test]
    fn a_plain_png_round_trips_without_a_location() {
        let mut image = RgbaImage::from_pixel(3, 2, Rgba([240, 240, 240, 255]));
        image.put_pixel(1, 1, Rgba([0, 0, 0, 255]));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        let prepared = prepare_photo(&bytes, "card.png").unwrap();
        assert!(prepared.latitude.is_none());
        assert_eq!(prepared.width, 3);
        assert_eq!(prepared.height, 2);
        assert!(prepared.png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }
}
