use image::{DynamicImage, ImageFormat};
use std::io::Cursor;

use crate::modules::files::file_inspector::ValidatedFile;

/// Image processing utilities
pub struct ImageProcessor;

impl ImageProcessor {
    /// Decodes only the exact payload borrowed by a successful purpose-bound inspection.
    pub fn decode_inspected_image(validated: &ValidatedFile<'_>) -> Result<DynamicImage, String> {
        if !validated.is_image() {
            return Err("Inspected content is not an image".to_string());
        }

        validated
            .decode_image()
            .map_err(|_| "Inspected image could not be decoded".to_string())
    }

    pub fn encode_webp(image: &DynamicImage) -> Result<Vec<u8>, String> {
        let mut buffer = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::WebP)
            .map_err(|_| "Validated image could not be encoded".to_string())?;
        Ok(buffer)
    }
}
