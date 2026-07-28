use std::io::Cursor;

use image::{ImageFormat, ImageReader};

use super::{
    platform_types::{DetectedContent, FilePurpose},
    purpose_registry::{purpose_definition, ContentLimits},
};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PDF_HEADER_PREFIX: &[u8] = b"%PDF-";
const PDF_STRUCTURE_WINDOW_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectedFile {
    detected_content: DetectedContent,
    width: Option<u32>,
    height: Option<u32>,
}

impl InspectedFile {
    pub const fn detected_content(self) -> DetectedContent {
        self.detected_content
    }

    pub const fn dimensions(self) -> Option<(u32, u32)> {
        match (self.width, self.height) {
            (Some(width), Some(height)) => Some((width, height)),
            _ => None,
        }
    }

    pub const fn canonical_extension(self) -> &'static str {
        self.detected_content.canonical_extension()
    }

    pub const fn canonical_mime_type(self) -> &'static str {
        self.detected_content.mime_type()
    }

    #[allow(dead_code)] // Task 6 consumes this staged inspection contract before derivatives.
    pub const fn is_image(self) -> bool {
        matches!(
            self.detected_content,
            DetectedContent::Jpeg | DetectedContent::Png | DetectedContent::Webp
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileInspectionError {
    ByteLimitExceeded,
    UnsupportedContent,
    ContentNotAllowed,
    MalformedContent,
    DimensionLimitExceeded,
    DecodedPixelLimitExceeded,
}

/// Identifies content from bytes and validates it against the server-owned purpose policy.
/// Submitted multipart MIME types and filenames are deliberately not accepted by this boundary.
pub fn inspect_file(
    purpose: FilePurpose,
    data: &[u8],
) -> Result<InspectedFile, FileInspectionError> {
    let definition =
        purpose_definition(purpose).map_err(|_| FileInspectionError::UnsupportedContent)?;
    if data.len() as u64 > definition.limits.max_bytes {
        return Err(FileInspectionError::ByteLimitExceeded);
    }

    let detected_content = detect_content(data)?;
    if !definition.allowed_content.contains(&detected_content) {
        return Err(FileInspectionError::ContentNotAllowed);
    }

    match detected_content {
        DetectedContent::Pdf => {
            validate_pdf_structure(data)?;
            Ok(InspectedFile {
                detected_content,
                width: None,
                height: None,
            })
        }
        DetectedContent::Png | DetectedContent::Jpeg | DetectedContent::Webp => {
            validate_image_container(data, detected_content)?;
            let (width, height) = image_dimensions(data, detected_content)?;
            validate_image_limits(width, height, definition.limits)?;

            // Fully decode only after the registry's byte, dimension, and pixel limits hold.
            image::load_from_memory_with_format(
                data,
                image_format(detected_content).ok_or(FileInspectionError::MalformedContent)?,
            )
            .map_err(|_| FileInspectionError::MalformedContent)?;

            Ok(InspectedFile {
                detected_content,
                width: Some(width),
                height: Some(height),
            })
        }
    }
}

pub fn validate_image_limits(
    width: u32,
    height: u32,
    limits: ContentLimits,
) -> Result<(), FileInspectionError> {
    if width == 0 || height == 0 {
        return Err(FileInspectionError::MalformedContent);
    }
    if limits.max_width.is_some_and(|maximum| width > maximum)
        || limits.max_height.is_some_and(|maximum| height > maximum)
    {
        return Err(FileInspectionError::DimensionLimitExceeded);
    }

    let decoded_pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(FileInspectionError::DecodedPixelLimitExceeded)?;
    if limits
        .max_decoded_pixels
        .is_some_and(|maximum| decoded_pixels > maximum)
    {
        return Err(FileInspectionError::DecodedPixelLimitExceeded);
    }

    Ok(())
}

fn detect_content(data: &[u8]) -> Result<DetectedContent, FileInspectionError> {
    if data.starts_with(PNG_SIGNATURE) {
        return Ok(DetectedContent::Png);
    }
    if data.starts_with(&[0xff, 0xd8]) {
        return Ok(DetectedContent::Jpeg);
    }
    if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return Ok(DetectedContent::Webp);
    }
    if data.starts_with(PDF_HEADER_PREFIX) {
        return Ok(DetectedContent::Pdf);
    }

    Err(FileInspectionError::UnsupportedContent)
}

fn validate_image_container(
    data: &[u8],
    detected_content: DetectedContent,
) -> Result<(), FileInspectionError> {
    let valid = match detected_content {
        DetectedContent::Png => {
            data.len() >= 33
                && data.starts_with(PNG_SIGNATURE)
                && data[8..12] == [0, 0, 0, 13]
                && &data[12..16] == b"IHDR"
                && data.ends_with(b"IEND\xaeB`\x82")
        }
        DetectedContent::Jpeg => {
            data.len() >= 4
                && data.starts_with(&[0xff, 0xd8])
                && data.ends_with(&[0xff, 0xd9])
                && data[2] == 0xff
                && data[3] != 0x00
                && data[3] != 0xff
        }
        DetectedContent::Webp => {
            data.len() >= 16
                && data.starts_with(b"RIFF")
                && &data[8..12] == b"WEBP"
                && usize::try_from(u32::from_le_bytes([data[4], data[5], data[6], data[7]]))
                    .ok()
                    .is_some_and(|declared_size| declared_size == data.len().saturating_sub(8))
                && matches!(&data[12..16], b"VP8 " | b"VP8L" | b"VP8X")
        }
        DetectedContent::Pdf => false,
    };

    if valid {
        Ok(())
    } else {
        Err(FileInspectionError::MalformedContent)
    }
}

fn image_dimensions(
    data: &[u8],
    detected_content: DetectedContent,
) -> Result<(u32, u32), FileInspectionError> {
    ImageReader::with_format(
        Cursor::new(data),
        image_format(detected_content).ok_or(FileInspectionError::MalformedContent)?,
    )
    .into_dimensions()
    .map_err(|_| FileInspectionError::MalformedContent)
}

pub(crate) const fn image_format(detected_content: DetectedContent) -> Option<ImageFormat> {
    match detected_content {
        DetectedContent::Png => Some(ImageFormat::Png),
        DetectedContent::Jpeg => Some(ImageFormat::Jpeg),
        DetectedContent::Webp => Some(ImageFormat::WebP),
        DetectedContent::Pdf => None,
    }
}

fn validate_pdf_structure(data: &[u8]) -> Result<(), FileInspectionError> {
    let header_is_valid = data
        .get(PDF_HEADER_PREFIX.len()..PDF_HEADER_PREFIX.len() + 3)
        .is_some_and(|version| {
            version[0].is_ascii_digit() && version[1] == b'.' && version[2].is_ascii_digit()
        });
    if !header_is_valid {
        return Err(FileInspectionError::MalformedContent);
    }

    let tail_start = data.len().saturating_sub(PDF_STRUCTURE_WINDOW_BYTES);
    let tail = &data[tail_start..];
    let trailer =
        find_subsequence(tail, b"trailer").ok_or(FileInspectionError::MalformedContent)?;
    let root = find_subsequence(&tail[trailer..], b"/Root")
        .ok_or(FileInspectionError::MalformedContent)?;
    let startxref = find_subsequence(&tail[trailer + root..], b"startxref")
        .ok_or(FileInspectionError::MalformedContent)?;
    let eof = find_subsequence(&tail[trailer + root + startxref..], b"%%EOF")
        .ok_or(FileInspectionError::MalformedContent)?;
    let after_eof = &tail[trailer + root + startxref + eof + b"%%EOF".len()..];

    if after_eof.iter().all(u8::is_ascii_whitespace) {
        Ok(())
    } else {
        Err(FileInspectionError::MalformedContent)
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

    use crate::modules::files::{
        file_inspector::{inspect_file, validate_image_limits, FileInspectionError},
        platform_types::{DetectedContent, FilePurpose},
        purpose_registry::ContentLimits,
    };

    fn encoded_image(format: ImageFormat, width: u32, height: u32) -> Vec<u8> {
        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(width, height, Rgba([4, 8, 15, 255])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), format)
            .expect("synthetic image fixture must encode");
        bytes
    }

    #[test]
    fn detects_supported_content_from_bytes_and_returns_canonical_metadata() {
        let cases = [
            (ImageFormat::Png, DetectedContent::Png, "png"),
            (ImageFormat::Jpeg, DetectedContent::Jpeg, "jpg"),
            (ImageFormat::WebP, DetectedContent::Webp, "webp"),
        ];

        for (format, content, extension) in cases {
            let inspection =
                inspect_file(FilePurpose::QuestionBankImage, &encoded_image(format, 2, 3)).expect(
                    "valid image bytes must be inspected independently of multipart metadata",
                );

            assert_eq!(inspection.detected_content(), content);
            assert_eq!(inspection.canonical_extension(), extension);
            assert_eq!(inspection.canonical_mime_type(), content.mime_type());
            assert_eq!(inspection.dimensions(), Some((2, 3)));
        }
    }

    #[test]
    fn detects_structurally_bounded_pdf_from_bytes() {
        let inspection = inspect_file(
            FilePurpose::Transcript,
            b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R >>\nstartxref\n0\n%%EOF\n",
        )
        .expect("bounded PDF header, root trailer, and EOF must be accepted");

        assert_eq!(inspection.detected_content(), DetectedContent::Pdf);
        assert_eq!(inspection.canonical_extension(), "pdf");
        assert_eq!(inspection.dimensions(), None);
    }

    #[test]
    fn rejects_spoofed_unsupported_and_truncated_content() {
        let png = encoded_image(ImageFormat::Png, 2, 2);
        assert_eq!(
            inspect_file(FilePurpose::Transcript, &png),
            Err(FileInspectionError::ContentNotAllowed)
        );
        assert_eq!(
            inspect_file(FilePurpose::ProfileImage, b"not an upload"),
            Err(FileInspectionError::UnsupportedContent)
        );
        assert_eq!(
            inspect_file(FilePurpose::ProfileImage, &png[..png.len() - 1]),
            Err(FileInspectionError::MalformedContent)
        );
        assert_eq!(
            inspect_file(
                FilePurpose::Transcript,
                b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\n"
            ),
            Err(FileInspectionError::MalformedContent)
        );
    }

    #[test]
    fn enforces_registry_byte_dimension_and_pixel_limits_before_processing() {
        let oversized = vec![0_u8; 2 * 1024 * 1024 + 1];
        assert_eq!(
            inspect_file(FilePurpose::SchoolLogo, &oversized),
            Err(FileInspectionError::ByteLimitExceeded)
        );

        assert_eq!(
            inspect_file(
                FilePurpose::SchoolLogo,
                &encoded_image(ImageFormat::Png, 2049, 1),
            ),
            Err(FileInspectionError::DimensionLimitExceeded)
        );

        assert_eq!(
            inspect_file(
                FilePurpose::SchoolLogo,
                &encoded_image(ImageFormat::Png, 2048, 2049),
            ),
            Err(FileInspectionError::DimensionLimitExceeded)
        );

        assert_eq!(
            validate_image_limits(
                4,
                3,
                ContentLimits {
                    max_bytes: 1024,
                    max_width: Some(4),
                    max_height: Some(4),
                    max_decoded_pixels: Some(10),
                },
            ),
            Err(FileInspectionError::DecodedPixelLimitExceeded)
        );
    }
}
