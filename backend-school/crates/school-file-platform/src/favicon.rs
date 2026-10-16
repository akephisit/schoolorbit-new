use bytes::Bytes;
use image::{DynamicImage, ImageFormat, RgbaImage};
use std::io::Cursor;

use crate::{
    file_inspector::inspect_file, file_processor::ImageProcessor,
    platform_service::FilePlatformError, platform_types::FilePurpose,
};

/// Reinspect stored crest bytes before decoding, then preserve the complete crest on a square.
pub(crate) fn render_png(source: &[u8]) -> Result<Bytes, FilePlatformError> {
    let inspected = inspect_file(FilePurpose::SchoolLogo, source)?;
    let decoded = ImageProcessor::decode_inspected_image(&inspected)
        .map_err(|_| FilePlatformError::InspectionRejected)?;
    let resized = decoded
        .resize(256, 256, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let mut square = RgbaImage::new(256, 256);
    image::imageops::overlay(
        &mut square,
        &resized,
        i64::from((256 - resized.width()) / 2),
        i64::from((256 - resized.height()) / 2),
    );
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(square)
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|_| FilePlatformError::InspectionRejected)?;
    Ok(Bytes::from(encoded.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn square_png_preserves_portrait_landscape_and_square_crests() {
        for (width, height, format) in [
            (120, 240, ImageFormat::Png),
            (240, 120, ImageFormat::Jpeg),
            (192, 192, ImageFormat::WebP),
        ] {
            let source = RgbaImage::from_pixel(width, height, Rgba([20, 80, 120, 255]));
            let mut bytes = Cursor::new(Vec::new());
            let source = DynamicImage::ImageRgba8(source);
            if format == ImageFormat::Jpeg {
                source.to_rgb8().write_to(&mut bytes, format).unwrap();
            } else {
                source.write_to(&mut bytes, format).unwrap();
            }
            let png = render_png(bytes.get_ref()).unwrap();
            assert_eq!(image::guess_format(&png).unwrap(), ImageFormat::Png);
            let actual = image::load_from_memory(&png).unwrap().to_rgba8();
            assert_eq!(actual.dimensions(), (256, 256));
            assert_eq!(actual.get_pixel(128, 128).0[3], 255);
            if width != height {
                assert_eq!(actual.get_pixel(0, 0).0[3], 0);
            }
        }
    }

    #[test]
    fn favicon_refuses_uninspected_or_non_image_bytes() {
        assert_eq!(
            render_png(b"not an image"),
            Err(FilePlatformError::InspectionRejected)
        );
    }
}
