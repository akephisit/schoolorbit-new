use std::io::Cursor;

use image::{ImageFormat, ImageReader};

use super::{
    platform_types::{DetectedContent, FilePurpose},
    purpose_registry::{purpose_definition, ContentLimits},
};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PDF_HEADER_PREFIX: &[u8] = b"%PDF-";
const PDF_STRUCTURE_WINDOW_BYTES: usize = 64 * 1024;
const MAX_PDF_OBJECTS: u32 = 1_000_000;

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

    let document =
        lopdf::Document::load_mem(data).map_err(|_| FileInspectionError::MalformedContent)?;
    validate_pdf_cross_references(&document, data.len())?;

    let root_id = document
        .trailer
        .get(b"Root")
        .and_then(lopdf::Object::as_reference)
        .map_err(|_| FileInspectionError::MalformedContent)?;
    let root = document
        .get_object(root_id)
        .and_then(lopdf::Object::as_dict)
        .map_err(|_| FileInspectionError::MalformedContent)?;
    let root_type = root
        .get(b"Type")
        .and_then(lopdf::Object::as_name)
        .map_err(|_| FileInspectionError::MalformedContent)?;
    if root_type != b"Catalog" {
        return Err(FileInspectionError::MalformedContent);
    }

    let pages_id = root
        .get(b"Pages")
        .and_then(lopdf::Object::as_reference)
        .map_err(|_| FileInspectionError::MalformedContent)?;
    let pages = document
        .get_object(pages_id)
        .and_then(lopdf::Object::as_dict)
        .map_err(|_| FileInspectionError::MalformedContent)?;
    if pages
        .get(b"Type")
        .and_then(lopdf::Object::as_name)
        .is_ok_and(|object_type| object_type == b"Pages")
    {
        Ok(())
    } else {
        Err(FileInspectionError::MalformedContent)
    }
}

fn find_last_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}

fn validate_pdf_cross_references(
    document: &lopdf::Document,
    source_len: usize,
) -> Result<(), FileInspectionError> {
    let declared_size = document
        .trailer
        .get(b"Size")
        .and_then(lopdf::Object::as_i64)
        .ok()
        .and_then(|size| u32::try_from(size).ok())
        .filter(|size| *size > 0 && *size <= MAX_PDF_OBJECTS)
        .ok_or(FileInspectionError::MalformedContent)?;
    if declared_size != document.reference_table.size {
        return Err(FileInspectionError::MalformedContent);
    }

    for (object_number, entry) in &document.reference_table.entries {
        match entry {
            lopdf::xref::XrefEntry::Normal { offset, generation } => {
                if usize::try_from(*offset)
                    .ok()
                    .is_none_or(|offset| offset >= source_len)
                    || !document
                        .objects
                        .contains_key(&(*object_number, *generation))
                {
                    return Err(FileInspectionError::MalformedContent);
                }
            }
            lopdf::xref::XrefEntry::Compressed { container, .. } => {
                if !document.objects.contains_key(&(*object_number, 0))
                    || document
                        .get_object((*container, 0))
                        .and_then(lopdf::Object::as_stream)
                        .is_err()
                {
                    return Err(FileInspectionError::MalformedContent);
                }
            }
            lopdf::xref::XrefEntry::Free | lopdf::xref::XrefEntry::UnusableFree => {}
        }
    }

    Ok(())
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
        let mut pdf = b"%PDF-1.7\n".to_vec();
        let catalog_offset = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        let pages_offset = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Count 0 /Kids [] >>\nendobj\n");
        let xref_offset = pdf.len();
        pdf.extend_from_slice(
            format!(
                "xref\n0 3\n0000000000 65535 f \n{catalog_offset:010} 00000 n \n{pages_offset:010} 00000 n \ntrailer\n<< /Size 3 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
            )
            .as_bytes(),
        );
        pdf
    }

    fn xref_stream_entry(entry_type: u8, offset: usize, generation: u16) -> [u8; 7] {
        let offset =
            u32::try_from(offset).expect("synthetic PDF offsets must fit the four-byte xref field");
        let mut entry = [0_u8; 7];
        entry[0] = entry_type;
        entry[1..5].copy_from_slice(&offset.to_be_bytes());
        entry[5..7].copy_from_slice(&generation.to_be_bytes());
        entry
    }

    fn xref_stream_pdf() -> Vec<u8> {
        let mut pdf = b"%PDF-1.7\n".to_vec();
        let catalog_offset = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        let pages_offset = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Count 0 /Kids [] >>\nendobj\n");
        let xref_offset = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /XRef /Size 4 /Root 1 0 R /W [1 4 2] /Index [0 4] /Length 28 >>\nstream\n",
        );
        pdf.extend_from_slice(&xref_stream_entry(0, 0, u16::MAX));
        pdf.extend_from_slice(&xref_stream_entry(1, catalog_offset, 0));
        pdf.extend_from_slice(&xref_stream_entry(1, pages_offset, 0));
        pdf.extend_from_slice(&xref_stream_entry(1, xref_offset, 0));
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
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
    fn rejects_invalid_pdf_references_and_accepts_a_valid_xref_stream() {
        let mut invalid_object_offset = xref_table_pdf();
        let first_in_use_entry = invalid_object_offset
            .windows(b" 00000 n ".len())
            .position(|window| window == b" 00000 n ")
            .expect("fixture contains an in-use xref entry")
            - 10;
        invalid_object_offset[first_in_use_entry..first_in_use_entry + 10]
            .copy_from_slice(b"0009999999");

        assert_eq!(
            inspect_file(FilePurpose::Transcript, &invalid_object_offset),
            Err(FileInspectionError::MalformedContent)
        );

        let mut missing_root = xref_table_pdf();
        let root = missing_root
            .windows(b"/Root 1 0 R".len())
            .position(|window| window == b"/Root 1 0 R")
            .expect("fixture contains trailer root");
        missing_root[root..root + b"/Root 1 0 R".len()].copy_from_slice(b"/Root 9 0 R");
        assert_eq!(
            inspect_file(FilePurpose::Transcript, &missing_root),
            Err(FileInspectionError::MalformedContent)
        );

        let mut inconsistent_stream = xref_stream_pdf();
        let length = inconsistent_stream
            .windows(b"/Length 28".len())
            .position(|window| window == b"/Length 28")
            .expect("fixture contains xref stream length");
        inconsistent_stream[length..length + b"/Length 28".len()].copy_from_slice(b"/Length 27");
        assert_eq!(
            inspect_file(FilePurpose::Transcript, &inconsistent_stream),
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
