use image::{imageops::FilterType, ImageFormat};
use std::io::Cursor;
use tracing::info;

/// Image processing utilities
pub struct ImageProcessor;

impl ImageProcessor {
    /// Resize an image to fit within max dimensions while maintaining aspect ratio
    ///
    /// # Arguments
    /// * `data` - Original image data
    /// * `max_width` - Maximum width
    /// * `max_height` - Maximum height
    ///
    /// # Returns
    /// Resized image data as Vec<u8>
    pub fn resize_image(
        data: &[u8],
        max_width: u32,
        max_height: u32,
    ) -> Result<Vec<u8>, String> {
        info!("Resizing image to max {}x{}", max_width, max_height);
        
        // Load image
        let img = image::load_from_memory(data)
            .map_err(|e| format!("Failed to load image: {}", e))?;
        
        // Calculate new dimensions maintaining aspect ratio
        let width = img.width();
        let height = img.height();
        let (new_width, new_height) = if width > max_width || height > max_height {
            let ratio = (max_width as f32 / width as f32)
                .min(max_height as f32 / height as f32);
            ((width as f32 * ratio) as u32, (height as f32 * ratio) as u32)
        } else {
            (width, height)
        };
        
        if new_width == width && new_height == height {
            info!("Image already within size limits, no resize needed");
            return Ok(data.to_vec());
        }
        
        // Resize
        let resized = img.resize(new_width, new_height, FilterType::Lanczos3);
        
        // Encode back to JPEG
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        
        resized
            .write_to(&mut cursor, ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to encode resized image: {}", e))?;
        
        info!(
            "Image resized from {}x{} to {}x{}",
            width, height, new_width, new_height
        );
        
        Ok(buffer)
    }
    
    /// Create a thumbnail from an image
    ///
    /// # Arguments
    /// * `data` - Original image data
    /// * `size` - Thumbnail size (will be square)
    ///
    /// # Returns
    /// Thumbnail image data as Vec<u8>
    pub fn create_thumbnail(data: &[u8], size: u32) -> Result<Vec<u8>, String> {
        info!("Creating {}x{} thumbnail", size, size);
        
        let img = image::load_from_memory(data)
            .map_err(|e| format!("Failed to load image for thumbnail: {}", e))?;
        
        // Create square thumbnail by cropping to center
        let thumbnail = img.resize_to_fill(size, size, FilterType::Lanczos3);
        
        // Encode as JPEG
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        
        thumbnail
            .write_to(&mut cursor, ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to encode thumbnail: {}", e))?;
        
        info!("Thumbnail created successfully");
        
        Ok(buffer)
    }
    
    /// Get image dimensions
    pub fn get_dimensions(data: &[u8]) -> Result<(u32, u32), String> {
        let img = image::load_from_memory(data)
            .map_err(|e| format!("Failed to load image: {}", e))?;
        
        Ok((img.width(), img.height()))
    }
    
    /// Validate if data is a valid image
    pub fn is_valid_image(data: &[u8]) -> bool {
        image::load_from_memory(data).is_ok()
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
