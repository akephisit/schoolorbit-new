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

/// A purpose-validated inspection bound to the exact borrowed upload payload.
/// It intentionally cannot be constructed by callers outside this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedFile<'a> {
    data: &'a [u8],
    inspection: InspectedFile,
}

impl<'a> ValidatedFile<'a> {
    pub const fn detected_content(self) -> DetectedContent {
        self.inspection.detected_content()
    }

    pub const fn dimensions(self) -> Option<(u32, u32)> {
        self.inspection.dimensions()
    }

    pub const fn canonical_extension(self) -> &'static str {
        self.inspection.canonical_extension()
    }

    pub const fn canonical_mime_type(self) -> &'static str {
        self.inspection.canonical_mime_type()
    }

    pub const fn is_image(self) -> bool {
        self.inspection.is_image()
    }

    pub(crate) fn decode_image(self) -> Result<image::DynamicImage, FileInspectionError> {
        let format =
            image_format(self.detected_content()).ok_or(FileInspectionError::MalformedContent)?;
        image::load_from_memory_with_format(self.data, format)
            .map_err(|_| FileInspectionError::MalformedContent)
    }
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
) -> Result<ValidatedFile<'_>, FileInspectionError> {
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
            Ok(ValidatedFile {
                data,
                inspection: InspectedFile {
                    detected_content,
                    width: None,
                    height: None,
                },
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

            Ok(ValidatedFile {
                data,
                inspection: InspectedFile {
                    detected_content,
                    width: Some(width),
                    height: Some(height),
                },
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
    let eof = find_last_subsequence(tail, b"%%EOF").ok_or(FileInspectionError::MalformedContent)?;
    if !tail[eof + b"%%EOF".len()..]
        .iter()
        .all(u8::is_ascii_whitespace)
    {
        return Err(FileInspectionError::MalformedContent);
    }

    let before_eof = &tail[..eof];
    let startxref = find_last_subsequence(before_eof, b"startxref")
        .ok_or(FileInspectionError::MalformedContent)?;
    let offset = parse_decimal(&before_eof[startxref + b"startxref".len()..])?;
    if offset >= data.len() {
        return Err(FileInspectionError::MalformedContent);
    }

    let target = &data[offset..];
    if target.starts_with(b"xref") {
        validate_xref_table(target)
    } else {
        validate_xref_stream(target)
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn find_last_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}

fn parse_decimal(bytes: &[u8]) -> Result<usize, FileInspectionError> {
    let bytes = trim_ascii_whitespace(bytes);
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return Err(FileInspectionError::MalformedContent);
    }
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or(FileInspectionError::MalformedContent)
}

fn validate_xref_table(target: &[u8]) -> Result<(), FileInspectionError> {
    let trailer =
        find_subsequence(target, b"trailer").ok_or(FileInspectionError::MalformedContent)?;
    let mut table = &target[b"xref".len()..trailer];
    let (_, rest) = take_decimal_token(table)?;
    let (entry_count, rest) = take_decimal_token(rest)?;
    if entry_count == 0 || entry_count > 1_000_000 {
        return Err(FileInspectionError::MalformedContent);
    }
    table = rest;
    for _ in 0..entry_count {
        let (_, rest) = take_decimal_token(table)?;
        let (_, rest) = take_decimal_token(rest)?;
        let rest = trim_leading_ascii_whitespace(rest);
        if !matches!(rest.first(), Some(b'n' | b'f')) {
            return Err(FileInspectionError::MalformedContent);
        }
        table = rest.get(1..).ok_or(FileInspectionError::MalformedContent)?;
    }

    validate_trailer_dictionary(&target[trailer + b"trailer".len()..])
}

fn validate_xref_stream(target: &[u8]) -> Result<(), FileInspectionError> {
    let (_, rest) = take_decimal_token(target)?;
    let (_, rest) = take_decimal_token(rest)?;
    let rest = trim_leading_ascii_whitespace(rest);
    let rest = rest
        .strip_prefix(b"obj")
        .ok_or(FileInspectionError::MalformedContent)?;
    let dictionary = dictionary_at_start(rest)?;
    if find_subsequence(dictionary, b"/Type /XRef").is_none()
        || find_subsequence(dictionary, b"/W").is_none()
        || find_subsequence(dictionary, b"/Length").is_none()
        || !has_indirect_root(dictionary)
    {
        return Err(FileInspectionError::MalformedContent);
    }

    let after_dictionary = &rest[dictionary.len() + 4..];
    if find_subsequence(after_dictionary, b"stream").is_none()
        || find_subsequence(after_dictionary, b"endstream").is_none()
        || find_subsequence(after_dictionary, b"endobj").is_none()
    {
        return Err(FileInspectionError::MalformedContent);
    }
    Ok(())
}

fn validate_trailer_dictionary(bytes: &[u8]) -> Result<(), FileInspectionError> {
    let dictionary = dictionary_at_start(bytes)?;
    if has_indirect_root(dictionary) {
        Ok(())
    } else {
        Err(FileInspectionError::MalformedContent)
    }
}

fn dictionary_at_start(bytes: &[u8]) -> Result<&[u8], FileInspectionError> {
    let bytes = trim_leading_ascii_whitespace(bytes);
    let body = bytes
        .strip_prefix(b"<<")
        .ok_or(FileInspectionError::MalformedContent)?;
    let end = find_subsequence(body, b">>").ok_or(FileInspectionError::MalformedContent)?;
    Ok(&body[..end])
}

fn has_indirect_root(dictionary: &[u8]) -> bool {
    let Some(root) = find_subsequence(dictionary, b"/Root") else {
        return false;
    };
    let Ok((_, rest)) = take_decimal_token(&dictionary[root + b"/Root".len()..]) else {
        return false;
    };
    let Ok((_, rest)) = take_decimal_token(rest) else {
        return false;
    };
    trim_leading_ascii_whitespace(rest).starts_with(b"R")
}

fn take_decimal_token(bytes: &[u8]) -> Result<(usize, &[u8]), FileInspectionError> {
    let bytes = trim_leading_ascii_whitespace(bytes);
    let end = bytes
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(bytes.len());
    if end == 0 {
        return Err(FileInspectionError::MalformedContent);
    }
    let value = parse_decimal(&bytes[..end])?;
    Ok((value, &bytes[end..]))
}

fn trim_leading_ascii_whitespace(bytes: &[u8]) -> &[u8] {
    let first = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    &bytes[first..]
}

fn trim_ascii_whitespace(bytes: &[u8]) -> &[u8] {
    let bytes = trim_leading_ascii_whitespace(bytes);
    let last = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map(|index| index + 1)
        .unwrap_or(0);
    &bytes[..last]
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
    use crate::utils::file_processor::ImageProcessor;

    fn encoded_image(format: ImageFormat, width: u32, height: u32) -> Vec<u8> {
        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(width, height, Rgba([4, 8, 15, 255])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), format)
            .expect("synthetic image fixture must encode");
        bytes
    }

    fn xref_table_pdf() -> Vec<u8> {
        let mut pdf = b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\nendobj\n".to_vec();
        let xref_offset = pdf.len();
        pdf.extend_from_slice(
            format!(
                "xref\n0 2\n0000000000 65535 f \n0000000009 00000 n \ntrailer\n<< /Size 2 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
            )
            .as_bytes(),
        );
        pdf
    }

    fn xref_stream_pdf() -> Vec<u8> {
        let mut pdf = b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\nendobj\n".to_vec();
        let xref_offset = pdf.len();
        pdf.extend_from_slice(
            b"2 0 obj\n<< /Type /XRef /Size 3 /Root 1 0 R /W [1 1 1] /Length 3 >>\nstream\n\0\0\0\nendstream\nendobj\n",
        );
        pdf.extend_from_slice(format!("startxref\n{xref_offset}\n%%EOF\n").as_bytes());
        pdf
    }

    #[test]
    fn detects_supported_content_from_bytes_and_returns_canonical_metadata() {
        let cases = [
            (ImageFormat::Png, DetectedContent::Png, "png"),
            (ImageFormat::Jpeg, DetectedContent::Jpeg, "jpg"),
            (ImageFormat::WebP, DetectedContent::Webp, "webp"),
        ];

        for (format, content, extension) in cases {
            let bytes = encoded_image(format, 2, 3);
            let inspection = inspect_file(FilePurpose::QuestionBankImage, &bytes)
                .expect("valid image bytes must be inspected independently of multipart metadata");

            assert_eq!(inspection.detected_content(), content);
            assert_eq!(inspection.canonical_extension(), extension);
            assert_eq!(inspection.canonical_mime_type(), content.mime_type());
            assert_eq!(inspection.dimensions(), Some((2, 3)));
        }
    }

    #[test]
    fn detects_structurally_bounded_pdf_from_bytes() {
        let pdf = xref_table_pdf();
        let inspection = inspect_file(FilePurpose::Transcript, &pdf)
            .expect("xref table, trailer root, and final startxref must be accepted");

        assert_eq!(inspection.detected_content(), DetectedContent::Pdf);
        assert_eq!(inspection.canonical_extension(), "pdf");
        assert_eq!(inspection.dimensions(), None);
    }

    #[test]
    fn rejects_spoofed_unsupported_and_truncated_content() {
        let png = encoded_image(ImageFormat::Png, 2, 2);
        let jpeg = encoded_image(ImageFormat::Jpeg, 2, 2);
        let webp = encoded_image(ImageFormat::WebP, 2, 2);
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
                b"%PDF-1.7\ntrailer /Root startxref %%EOF"
            ),
            Err(FileInspectionError::MalformedContent)
        );
        assert_eq!(
            inspect_file(FilePurpose::ProfileImage, &jpeg[..jpeg.len() - 1]),
            Err(FileInspectionError::MalformedContent)
        );
        assert_eq!(
            inspect_file(FilePurpose::ProfileImage, &webp[..webp.len() - 1]),
            Err(FileInspectionError::MalformedContent)
        );
    }

    #[test]
    fn rejects_invalid_pdf_offsets_and_accepts_a_bounded_xref_stream() {
        let mut invalid_offset = xref_table_pdf();
        let offset_start = invalid_offset
            .windows(b"startxref\n".len())
            .position(|window| window == b"startxref\n")
            .expect("fixture contains startxref")
            + b"startxref\n".len();
        invalid_offset[offset_start..offset_start + 2].copy_from_slice(b"99");

        assert_eq!(
            inspect_file(FilePurpose::Transcript, &invalid_offset),
            Err(FileInspectionError::MalformedContent)
        );
        assert!(inspect_file(FilePurpose::Transcript, &xref_stream_pdf()).is_ok());
    }

    #[test]
    fn validated_image_borrows_the_exact_inspected_payload_for_derivative_decode() {
        let safe = encoded_image(ImageFormat::Png, 2, 2);
        let validated =
            inspect_file(FilePurpose::ProfileImage, &safe).expect("safe fixture must inspect");

        assert!(ImageProcessor::decode_inspected_image(&validated).is_ok());
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
