use image::{imageops::FilterType, DynamicImage, ImageFormat};
use std::io::Cursor;
use tracing::info;

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

    /// Resizes a purpose-validated image while preserving its bounded decode boundary.
    pub fn resize_validated_image(
        validated: &ValidatedFile<'_>,
        max_width: u32,
        max_height: u32,
    ) -> Result<DynamicImage, String> {
        info!("Resizing image to max {}x{}", max_width, max_height);

        let img = Self::decode_inspected_image(validated)?;

        // Calculate new dimensions maintaining aspect ratio
        let width = img.width();
        let height = img.height();
        let (new_width, new_height) = if width > max_width || height > max_height {
            let ratio = (max_width as f32 / width as f32).min(max_height as f32 / height as f32);
            (
                (width as f32 * ratio) as u32,
                (height as f32 * ratio) as u32,
            )
        } else {
            (width, height)
        };

        if new_width == width && new_height == height {
            info!("Image already within size limits, no resize needed");
            return Ok(img);
        }

        let resized = img.resize(new_width, new_height, FilterType::Lanczos3);

        info!(
            "Image resized from {}x{} to {}x{}",
            width, height, new_width, new_height
        );

        Ok(resized)
    }

    pub fn encode_jpeg(image: &DynamicImage) -> Result<Vec<u8>, String> {
        let mut buffer = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Jpeg)
            .map_err(|_| "Validated image could not be encoded".to_string())?;
        Ok(buffer)
    }

    /// Creates a thumbnail from a decoded image that came through the validated boundary.
    pub fn create_thumbnail_from_image(image: &DynamicImage, size: u32) -> Result<Vec<u8>, String> {
        info!("Creating {}x{} thumbnail", size, size);

        let thumbnail = image.resize_to_fill(size, size, FilterType::Lanczos3);
        let buffer = Self::encode_jpeg(&thumbnail)?;

        info!("Thumbnail created successfully");

        Ok(buffer)
    }
}

/// File validation utilities
pub struct FileValidator;

impl FileValidator {
    /// Validate file size
    pub fn validate_size(size: usize, max_size_mb: u64) -> Result<(), String> {
        let max_bytes = max_size_mb * 1024 * 1024;
        if size > max_bytes as usize {
            return Err(format!(
                "File size ({} MB) exceeds maximum allowed size ({} MB)",
                size / 1024 / 1024,
                max_size_mb
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_size() {
        assert!(FileValidator::validate_size(1024 * 1024, 5).is_ok()); // 1 MB < 5 MB
        assert!(FileValidator::validate_size(10 * 1024 * 1024, 5).is_err()); // 10 MB > 5 MB
    }
}
