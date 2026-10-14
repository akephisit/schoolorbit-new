//! Canonical supply raster ingestion. Only the normalized image is persisted.
use bytes::Bytes;
use image::codecs::jpeg::JpegEncoder;

use crate::{
    file_inspector::inspect_supply_image, file_processor::ImageProcessor,
    platform_types::FilePurpose,
};

const MAX_EDGE: u32 = 1600;

pub(crate) fn normalize(purpose: FilePurpose, data: &[u8]) -> Result<Bytes, ()> {
    let validated = inspect_supply_image(purpose, data).map_err(|_| ())?;
    let decoded = ImageProcessor::decode_supply_image(&validated).map_err(|_| ())?;
    let resized = if decoded.width() > MAX_EDGE || decoded.height() > MAX_EDGE {
        decoded.resize(MAX_EDGE, MAX_EDGE, image::imageops::FilterType::Triangle)
    } else {
        decoded
    };
    let encoded = if resized.color().has_alpha() {
        ImageProcessor::encode_webp(&resized).map_err(|_| ())?
    } else {
        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, 82)
            .encode_image(&resized.to_rgb8())
            .map_err(|_| ())?;
        bytes
    };
    Ok(Bytes::from(encoded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_inspector::inspect_file;
    use image::ImageFormat;
    use std::io::Cursor;
    fn normalize(data: &[u8]) -> Result<Bytes, ()> {
        super::normalize(FilePurpose::SupplyCatalogImage, data)
    }
    fn decode(data: &[u8]) -> image::DynamicImage {
        let validated = inspect_file(FilePurpose::SupplyCatalogImage, data).unwrap();
        ImageProcessor::decode_inspected_image(&validated).unwrap()
    }
    fn raster(format: ImageFormat, alpha: bool) -> Vec<u8> {
        let picture = if alpha {
            image::DynamicImage::new_rgba8(1800, 900)
        } else {
            image::DynamicImage::new_rgb8(1800, 900)
        };
        let mut output = Cursor::new(Vec::new());
        picture
            .write_to(&mut output, format)
            .expect("fixture image should encode");
        output.into_inner()
    }
    #[test]
    fn common_and_desktop_formats_become_small_metadata_free_rasters() {
        for format in [
            ImageFormat::Jpeg,
            ImageFormat::Png,
            ImageFormat::WebP,
            ImageFormat::Bmp,
            ImageFormat::Gif,
            ImageFormat::Tiff,
        ] {
            let source = raster(format, false);
            let output = normalize(&source).expect("supported raster should normalize");
            let decoded = decode(&output);
            assert_eq!((decoded.width(), decoded.height()), (1600, 800));
            assert!(matches!(
                image::guess_format(&output).unwrap(),
                ImageFormat::Jpeg | ImageFormat::WebP
            ));
            assert!(output.len() < 500_000);
        }
    }
    #[test]
    fn transparency_survives_as_webp() {
        let output = normalize(&raster(ImageFormat::Png, true)).unwrap();
        assert_eq!(image::guess_format(&output).unwrap(), ImageFormat::WebP);
        assert_eq!(decode(&output).to_rgba8().get_pixel(0, 0).0[3], 0);
    }
    #[test]
    fn camera_orientation_is_applied_and_exif_is_removed() {
        let picture = image::DynamicImage::new_rgb8(120, 80);
        let mut encoded = Cursor::new(Vec::new());
        picture.write_to(&mut encoded, ImageFormat::Jpeg).unwrap();
        let jpeg = encoded.into_inner();
        // APP1/Exif, little-endian TIFF, one orientation=6 (90 degrees clockwise) tag.
        let exif: &[u8] = &[
            b'E', b'x', b'i', b'f', 0, 0, b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0,
            0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut source = jpeg[..2].to_vec();
        source.extend_from_slice(&[0xff, 0xe1]);
        source.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        source.extend_from_slice(exif);
        source.extend_from_slice(&jpeg[2..]);
        let output = normalize(&source).unwrap();
        let decoded = decode(&output);
        assert_eq!((decoded.width(), decoded.height()), (80, 120));
        assert!(!output.windows(4).any(|window| window == b"Exif"));
    }

    #[test]
    fn malformed_unsupported_and_oversized_input_are_rejected() {
        let supported = raster(ImageFormat::Jpeg, false);
        assert!(super::normalize(FilePurpose::ProfileImage, &supported).is_err());
        assert!(super::normalize(FilePurpose::SupplyProposalSource, &supported).is_ok());
        assert!(normalize(b"not an image").is_err());
        assert!(normalize(b"<svg xmlns='http://www.w3.org/2000/svg'/>").is_err());
        assert!(normalize(&vec![0; 20 * 1024 * 1024 + 1]).is_err());
        let mut truncated = raster(ImageFormat::Png, false);
        truncated.truncate(30);
        assert!(normalize(&truncated).is_err());
    }
}
