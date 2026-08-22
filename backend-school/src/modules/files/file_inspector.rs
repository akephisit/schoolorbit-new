use std::io::Cursor;

use image::{ImageFormat, ImageReader};

use super::{
    platform_types::{
        DetectedContent, FileInspectionMetadata, FilePurpose, FontInspectionStyle, PdfPageBox,
    },
    purpose_registry::{purpose_definition, ContentLimits},
};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PDF_HEADER_PREFIX: &[u8] = b"%PDF-";
const PDF_STRUCTURE_WINDOW_BYTES: usize = 64 * 1024;
const MAX_PDF_OBJECTS: u32 = 1_000_000;

#[derive(Clone, Debug, PartialEq)]
pub struct InspectedFile {
    detected_content: DetectedContent,
    metadata: FileInspectionMetadata,
}

/// A purpose-validated inspection bound to the exact borrowed upload payload.
/// It intentionally cannot be constructed by callers outside this module.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedFile<'a> {
    data: &'a [u8],
    inspection: InspectedFile,
}

impl<'a> ValidatedFile<'a> {
    pub const fn detected_content(&self) -> DetectedContent {
        self.inspection.detected_content()
    }

    pub const fn dimensions(&self) -> Option<(u32, u32)> {
        self.inspection.dimensions()
    }

    pub const fn canonical_extension(&self) -> &'static str {
        self.inspection.canonical_extension()
    }

    pub const fn canonical_mime_type(&self) -> &'static str {
        self.inspection.canonical_mime_type()
    }

    pub const fn is_image(&self) -> bool {
        self.inspection.is_image()
    }

    pub const fn metadata(&self) -> &FileInspectionMetadata {
        &self.inspection.metadata
    }

    pub(crate) fn decode_image(&self) -> Result<image::DynamicImage, FileInspectionError> {
        let format =
            image_format(self.detected_content()).ok_or(FileInspectionError::MalformedContent)?;
        image::load_from_memory_with_format(self.data, format)
            .map_err(|_| FileInspectionError::MalformedContent)
    }
}

impl InspectedFile {
    pub const fn detected_content(&self) -> DetectedContent {
        self.detected_content
    }

    pub const fn dimensions(&self) -> Option<(u32, u32)> {
        match &self.metadata {
            FileInspectionMetadata::Image {
                width_px,
                height_px,
            } => Some((*width_px, *height_px)),
            _ => None,
        }
    }

    pub const fn canonical_extension(&self) -> &'static str {
        self.detected_content.canonical_extension()
    }

    pub const fn canonical_mime_type(&self) -> &'static str {
        self.detected_content.mime_type()
    }

    #[allow(dead_code)] // Task 6 consumes this staged inspection contract before derivatives.
    pub const fn is_image(&self) -> bool {
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
    EncryptedPdfNotAllowed,
    PageCountNotAllowed,
    PageDimensionsNotAllowed,
    PageRotationNotAllowed,
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
            if purpose == FilePurpose::CertificateTemplateBackground
                && pdf_declares_encryption(data)
            {
                return Err(FileInspectionError::EncryptedPdfNotAllowed);
            }
            let document = validate_pdf_structure(data)?;
            let metadata = if purpose == FilePurpose::CertificateTemplateBackground {
                inspect_certificate_background(&document)?
            } else {
                FileInspectionMetadata::Unknown
            };
            Ok(ValidatedFile {
                data,
                inspection: InspectedFile {
                    detected_content,
                    metadata,
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
                    metadata: FileInspectionMetadata::Image {
                        width_px: width,
                        height_px: height,
                    },
                },
            })
        }
        DetectedContent::Ttf | DetectedContent::Otf => {
            let metadata = inspect_font(data)?;
            Ok(ValidatedFile {
                data,
                inspection: InspectedFile {
                    detected_content,
                    metadata,
                },
            })
        }
    }
}

fn pdf_declares_encryption(data: &[u8]) -> bool {
    data.windows(b"/Encrypt".len())
        .any(|window| window == b"/Encrypt")
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
    if data.starts_with(b"\x00\x01\x00\x00") {
        return Ok(DetectedContent::Ttf);
    }
    if data.starts_with(b"OTTO") {
        return Ok(DetectedContent::Otf);
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
        DetectedContent::Pdf | DetectedContent::Ttf | DetectedContent::Otf => false,
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
        DetectedContent::Pdf | DetectedContent::Ttf | DetectedContent::Otf => None,
    }
}

fn validate_pdf_structure(data: &[u8]) -> Result<lopdf::Document, FileInspectionError> {
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
        Ok(document)
    } else {
        Err(FileInspectionError::MalformedContent)
    }
}

fn inspect_certificate_background(
    document: &lopdf::Document,
) -> Result<FileInspectionMetadata, FileInspectionError> {
    if document.is_encrypted() {
        return Err(FileInspectionError::EncryptedPdfNotAllowed);
    }

    let pages = document.get_pages();
    if pages.len() != 1 {
        return Err(FileInspectionError::PageCountNotAllowed);
    }
    let page_id = *pages
        .values()
        .next()
        .ok_or(FileInspectionError::PageCountNotAllowed)?;
    let media_object = inherited_page_value(document, page_id, b"MediaBox")?
        .ok_or(FileInspectionError::MalformedContent)?;
    let media_box = parse_pdf_page_box(document, media_object)?;
    let crop_box = match inherited_page_value(document, page_id, b"CropBox")? {
        Some(value) => parse_pdf_page_box(document, value)?,
        None => media_box,
    };
    let rotation = match inherited_page_value(document, page_id, b"Rotate")? {
        Some(value) => object_integer(document, value)?,
        None => 0,
    };
    let normalized_rotation = ((rotation % 360) + 360) % 360;
    if normalized_rotation % 90 != 0 {
        return Err(FileInspectionError::PageRotationNotAllowed);
    }
    let rotation = i16::try_from(normalized_rotation)
        .map_err(|_| FileInspectionError::PageRotationNotAllowed)?;

    validate_certificate_page_boxes(crop_box, media_box)?;

    Ok(FileInspectionMetadata::Pdf {
        page_count: 1,
        crop_box,
        media_box,
        rotation,
    })
}

fn inherited_page_value<'a>(
    document: &'a lopdf::Document,
    page_id: lopdf::ObjectId,
    key: &[u8],
) -> Result<Option<&'a lopdf::Object>, FileInspectionError> {
    let mut current_id = page_id;
    for _ in 0..64 {
        let dictionary = document
            .get_object(current_id)
            .and_then(lopdf::Object::as_dict)
            .map_err(|_| FileInspectionError::MalformedContent)?;
        if let Ok(value) = dictionary.get(key) {
            return document
                .dereference(value)
                .map(|(_, value)| Some(value))
                .map_err(|_| FileInspectionError::MalformedContent);
        }
        let Ok(parent) = dictionary.get(b"Parent") else {
            return Ok(None);
        };
        current_id = parent
            .as_reference()
            .map_err(|_| FileInspectionError::MalformedContent)?;
    }
    Err(FileInspectionError::MalformedContent)
}

fn parse_pdf_page_box(
    document: &lopdf::Document,
    object: &lopdf::Object,
) -> Result<PdfPageBox, FileInspectionError> {
    let values = object
        .as_array()
        .map_err(|_| FileInspectionError::MalformedContent)?;
    if values.len() != 4 {
        return Err(FileInspectionError::MalformedContent);
    }
    let lower_x = object_number(document, &values[0])?;
    let lower_y = object_number(document, &values[1])?;
    let upper_x = object_number(document, &values[2])?;
    let upper_y = object_number(document, &values[3])?;
    let page_box = PdfPageBox::new(lower_x, lower_y, upper_x - lower_x, upper_y - lower_y);
    if ![page_box.x, page_box.y, page_box.width, page_box.height]
        .into_iter()
        .all(f64::is_finite)
        || page_box.width <= 0.0
        || page_box.height <= 0.0
    {
        return Err(FileInspectionError::PageDimensionsNotAllowed);
    }
    Ok(page_box)
}

fn object_number(
    document: &lopdf::Document,
    object: &lopdf::Object,
) -> Result<f64, FileInspectionError> {
    let (_, object) = document
        .dereference(object)
        .map_err(|_| FileInspectionError::MalformedContent)?;
    match object {
        lopdf::Object::Integer(value) => Ok(*value as f64),
        lopdf::Object::Real(value) => Ok(f64::from(*value)),
        _ => Err(FileInspectionError::MalformedContent),
    }
}

fn object_integer(
    document: &lopdf::Document,
    object: &lopdf::Object,
) -> Result<i64, FileInspectionError> {
    let (_, object) = document
        .dereference(object)
        .map_err(|_| FileInspectionError::MalformedContent)?;
    object
        .as_i64()
        .map_err(|_| FileInspectionError::MalformedContent)
}

fn validate_certificate_page_boxes(
    crop_box: PdfPageBox,
    media_box: PdfPageBox,
) -> Result<(), FileInspectionError> {
    const POINTS_PER_MM: f64 = 72.0 / 25.4;
    const MIN_SIDE_POINTS: f64 = 25.0 * POINTS_PER_MM;
    const MAX_SIDE_POINTS: f64 = 600.0 * POINTS_PER_MM;
    const MAX_AREA_SQUARE_POINTS: f64 = 250_000.0 * POINTS_PER_MM * POINTS_PER_MM;
    const BOX_EPSILON: f64 = 0.01;

    if !(MIN_SIDE_POINTS..=MAX_SIDE_POINTS).contains(&crop_box.width)
        || !(MIN_SIDE_POINTS..=MAX_SIDE_POINTS).contains(&crop_box.height)
        || crop_box.width * crop_box.height > MAX_AREA_SQUARE_POINTS
        || crop_box.x < media_box.x - BOX_EPSILON
        || crop_box.y < media_box.y - BOX_EPSILON
        || crop_box.x + crop_box.width > media_box.x + media_box.width + BOX_EPSILON
        || crop_box.y + crop_box.height > media_box.y + media_box.height + BOX_EPSILON
    {
        return Err(FileInspectionError::PageDimensionsNotAllowed);
    }
    Ok(())
}

fn inspect_font(data: &[u8]) -> Result<FileInspectionMetadata, FileInspectionError> {
    let face =
        ttf_parser::Face::parse(data, 0).map_err(|_| FileInspectionError::MalformedContent)?;
    let family_name = [
        ttf_parser::name_id::TYPOGRAPHIC_FAMILY,
        ttf_parser::name_id::FAMILY,
    ]
    .into_iter()
    .find_map(|name_id| {
        face.names()
            .into_iter()
            .filter(|name| name.name_id == name_id)
            .find_map(|name| name.to_string())
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
    });

    Ok(FileInspectionMetadata::Font {
        family_name,
        units_per_em: face.units_per_em(),
        weight: face.weight().to_number(),
        style: if face.is_italic() || face.is_oblique() {
            FontInspectionStyle::Italic
        } else {
            FontInspectionStyle::Normal
        },
        is_variable: face.is_variable(),
    })
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
    use lopdf::{dictionary, Document, Object};

    use crate::modules::files::{
        file_inspector::{inspect_file, validate_image_limits, FileInspectionError},
        platform_types::{
            DetectedContent, FileInspectionMetadata, FilePurpose, FontInspectionStyle, PdfPageBox,
        },
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

    fn font_table_record(bytes: &[u8], tag: &[u8; 4]) -> usize {
        let table_count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        (0..table_count)
            .map(|index| 12 + index * 16)
            .find(|&position| bytes.get(position..position + 4) == Some(tag.as_slice()))
            .expect("synthetic font fixture must contain the requested table")
    }

    fn italic_font_fixture() -> Vec<u8> {
        let mut bytes =
            include_bytes!("../../../../frontend-school/static/fonts/Sarabun-Regular.ttf").to_vec();
        let record = font_table_record(&bytes, b"OS/2");
        let table_offset = u32::from_be_bytes(
            bytes[record + 8..record + 12]
                .try_into()
                .expect("table offset must be four bytes"),
        ) as usize;
        let selection_offset = table_offset + 62;
        let mut selection = u16::from_be_bytes(
            bytes[selection_offset..selection_offset + 2]
                .try_into()
                .expect("selection flags must be two bytes"),
        );
        selection |= 1;
        selection &= !(1 << 6);
        bytes[selection_offset..selection_offset + 2].copy_from_slice(&selection.to_be_bytes());
        bytes
    }

    fn variable_font_fixture() -> Vec<u8> {
        let mut bytes =
            include_bytes!("../../../../frontend-school/static/fonts/Sarabun-Regular.ttf").to_vec();
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
        let table_offset = u32::try_from(bytes.len()).expect("font fixture must fit u32");
        let fvar = [
            0x00, 0x01, 0x00, 0x00, // version 1.0
            0x00, 0x10, // axes array offset
            0x00, 0x02, // count-size pairs
            0x00, 0x01, // axis count
            0x00, 0x14, // axis size
            0x00, 0x00, // instance count
            0x00, 0x08, // instance size
            b'w', b'g', b'h', b't', // weight axis
            0x00, 0x64, 0x00, 0x00, // minimum 100
            0x01, 0x90, 0x00, 0x00, // default 400
            0x03, 0x84, 0x00, 0x00, // maximum 900
            0x00, 0x00, // flags
            0x01, 0x00, // name ID 256
        ];
        bytes.extend_from_slice(&fvar);

        let record = font_table_record(&bytes, b"DSIG");
        bytes[record..record + 4].copy_from_slice(b"fvar");
        bytes[record + 8..record + 12].copy_from_slice(&table_offset.to_be_bytes());
        bytes[record + 12..record + 16].copy_from_slice(
            &u32::try_from(fvar.len())
                .expect("fvar fixture must fit u32")
                .to_be_bytes(),
        );
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

    fn pdf_box(values: [f32; 4]) -> Object {
        Object::Array(values.into_iter().map(Object::Real).collect())
    }

    fn certificate_pdf(
        crop_box: [f32; 4],
        media_box: [f32; 4],
        rotation: i64,
        page_count: usize,
        encrypted: bool,
    ) -> Vec<u8> {
        let mut document = Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let page_ids = (0..page_count)
            .map(|_| {
                document.add_object(dictionary! {
                    "Type" => "Page",
                    "Parent" => pages_id,
                })
            })
            .collect::<Vec<_>>();
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids
                .iter()
                .copied()
                .map(Object::Reference)
                .collect::<Vec<_>>(),
            "Count" => i64::try_from(page_count).expect("fixture page count must fit"),
            "CropBox" => pdf_box(crop_box),
            "MediaBox" => pdf_box(media_box),
            "Rotate" => rotation,
        };
        document.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);
        if encrypted {
            document.trailer.set(
                "ID",
                Object::Array(vec![
                    Object::String((1_u8..=16).collect(), lopdf::StringFormat::Literal),
                    Object::String((1_u8..=16).rev().collect(), lopdf::StringFormat::Literal),
                ]),
            );
            let encryption = lopdf::EncryptionVersion::V2 {
                document: &document,
                owner_password: "owner",
                user_password: "user",
                key_length: 128,
                permissions: lopdf::Permissions::all(),
            };
            let encryption_state = lopdf::EncryptionState::try_from(encryption)
                .expect("synthetic encryption state must build");
            document
                .encrypt(&encryption_state)
                .expect("synthetic certificate PDF must encrypt");
        }

        let mut bytes = Vec::new();
        document
            .save_to(&mut bytes)
            .expect("synthetic certificate PDF must encode");
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
            let bytes = encoded_image(format, 2, 3);
            let inspection = inspect_file(FilePurpose::QuestionBankImage, &bytes)
                .expect("valid image bytes must be inspected independently of multipart metadata");

            assert_eq!(inspection.detected_content(), content);
            assert_eq!(inspection.canonical_extension(), extension);
            assert_eq!(inspection.canonical_mime_type(), content.mime_type());
            assert_eq!(inspection.dimensions(), Some((2, 3)));
            assert_eq!(
                inspection.metadata(),
                &FileInspectionMetadata::Image {
                    width_px: 2,
                    height_px: 3,
                }
            );
        }
    }

    #[test]
    fn certificate_background_reads_one_page_inherited_boxes_and_rotation() {
        let pdf = certificate_pdf(
            [18.0, 24.0, 859.89, 619.28],
            [0.0, 0.0, 900.0, 650.0],
            450,
            1,
            false,
        );
        let inspected = inspect_file(FilePurpose::CertificateTemplateBackground, &pdf)
            .expect("valid one-page certificate background should inspect");

        assert_eq!(inspected.detected_content(), DetectedContent::Pdf);
        assert_eq!(
            inspected.metadata(),
            &FileInspectionMetadata::Pdf {
                page_count: 1,
                crop_box: PdfPageBox::new(18.0, 24.0, 841.89, 595.28),
                media_box: PdfPageBox::new(0.0, 0.0, 900.0, 650.0),
                rotation: 90,
            }
        );
    }

    #[test]
    fn certificate_background_rejects_encrypted_multiple_or_unsafe_pages() {
        let crop = [0.0, 0.0, 841.89, 595.28];
        let media = [0.0, 0.0, 841.89, 595.28];
        let two_page_pdf = certificate_pdf(crop, media, 0, 2, false);
        assert_eq!(
            inspect_file(FilePurpose::CertificateTemplateBackground, &two_page_pdf,),
            Err(FileInspectionError::PageCountNotAllowed)
        );
        assert!(
            inspect_file(FilePurpose::Transcript, &two_page_pdf).is_ok(),
            "existing generic PDF purposes must keep accepting multiple pages"
        );
        assert_eq!(
            inspect_file(
                FilePurpose::CertificateTemplateBackground,
                &certificate_pdf(crop, media, 0, 1, true),
            ),
            Err(FileInspectionError::EncryptedPdfNotAllowed)
        );
        assert_eq!(
            inspect_file(
                FilePurpose::CertificateTemplateBackground,
                &certificate_pdf([0.0, 0.0, 60.0, 60.0], media, 0, 1, false),
            ),
            Err(FileInspectionError::PageDimensionsNotAllowed)
        );
        assert_eq!(
            inspect_file(
                FilePurpose::CertificateTemplateBackground,
                &certificate_pdf(
                    [0.0, 0.0, 1700.0, 1700.0],
                    [0.0, 0.0, 1700.0, 1700.0],
                    0,
                    1,
                    false,
                ),
            ),
            Err(FileInspectionError::PageDimensionsNotAllowed)
        );
    }

    #[test]
    fn school_font_requires_a_valid_font_and_cannot_be_relabeled() {
        let sarabun =
            include_bytes!("../../../../frontend-school/static/fonts/Sarabun-Regular.ttf");
        let inspected = inspect_file(FilePurpose::SchoolFont, sarabun)
            .expect("built-in Sarabun must be a valid uploadable font");
        assert_eq!(inspected.detected_content(), DetectedContent::Ttf);
        match inspected.metadata() {
            FileInspectionMetadata::Font {
                family_name,
                units_per_em,
                weight,
                style,
                is_variable,
            } => {
                assert_eq!(family_name.as_deref(), Some("Sarabun"));
                assert_eq!(*units_per_em, 1000);
                assert_eq!(*weight, 400);
                assert_eq!(*style, FontInspectionStyle::Normal);
                assert!(!is_variable);
            }
            metadata => panic!("expected font metadata, got {metadata:?}"),
        }

        let bold = inspect_file(
            FilePurpose::SchoolFont,
            include_bytes!("../../../../frontend-school/static/fonts/Sarabun-Bold.ttf"),
        )
        .expect("built-in Sarabun Bold must be a valid uploadable font");
        match bold.metadata() {
            FileInspectionMetadata::Font { weight, style, .. } => {
                assert_eq!(*weight, 700);
                assert_eq!(*style, FontInspectionStyle::Normal);
            }
            metadata => panic!("expected bold font metadata, got {metadata:?}"),
        }

        assert_eq!(
            inspect_file(FilePurpose::CertificateTemplateImage, sarabun),
            Err(FileInspectionError::ContentNotAllowed)
        );
        assert_eq!(
            inspect_file(FilePurpose::SchoolFont, b"\x00\x01\x00\x00not-a-font",),
            Err(FileInspectionError::MalformedContent)
        );
        assert_eq!(
            inspect_file(FilePurpose::SchoolFont, b"OTTOnot-a-font",),
            Err(FileInspectionError::MalformedContent)
        );
        assert_eq!(
            inspect_file(FilePurpose::SchoolFont, &vec![0_u8; 5 * 1024 * 1024 + 1]),
            Err(FileInspectionError::ByteLimitExceeded)
        );
    }

    #[test]
    fn school_font_detects_italic_and_variable_faces() {
        let italic_bytes = italic_font_fixture();
        let italic = inspect_file(FilePurpose::SchoolFont, &italic_bytes)
            .expect("synthetic italic font must remain structurally valid");
        let italic_style = match italic.metadata() {
            FileInspectionMetadata::Font { style, .. } => *style,
            metadata => panic!("expected italic font metadata, got {metadata:?}"),
        };

        let variable_bytes = variable_font_fixture();
        let variable = inspect_file(FilePurpose::SchoolFont, &variable_bytes)
            .expect("synthetic variable font must remain structurally valid");
        let is_variable = match variable.metadata() {
            FileInspectionMetadata::Font { is_variable, .. } => *is_variable,
            metadata => panic!("expected variable font metadata, got {metadata:?}"),
        };

        assert_eq!(
            (italic_style, is_variable),
            (FontInspectionStyle::Italic, true)
        );
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
