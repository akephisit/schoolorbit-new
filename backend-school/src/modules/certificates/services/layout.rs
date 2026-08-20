use std::collections::BTreeSet;

use crate::modules::certificates::{
    models::{
        CertificateBuiltInFont, CertificateElement, CertificateFontSource, CertificateFontStyle,
        CertificateLayoutV1, ElementFrame, PageGeometry,
    },
    services::import_validation::{
        classify_header, normalize_name_for_match, referenced_variables, HeaderClass,
        RENDERABLE_STANDARD_VARIABLES, RESERVED_RENDER_VARIABLES,
    },
};

const POINTS_PER_MM: f64 = 72.0 / 25.4;
const PAPER_TOLERANCE_POINTS: f64 = POINTS_PER_MM;
const GEOMETRY_EPSILON: f64 = 0.01;
const ASPECT_RATIO_RELATIVE_TOLERANCE: f64 = 0.001;
const MAX_ELEMENTS: usize = 500;

#[derive(Clone, Copy)]
struct BuiltInFontVariant {
    family: &'static str,
    weight: u16,
    style: CertificateFontStyle,
    asset_path: &'static str,
}

const BUILT_IN_FONT_VARIANTS: [BuiltInFontVariant; 2] = [
    BuiltInFontVariant {
        family: "Sarabun",
        weight: 400,
        style: CertificateFontStyle::Normal,
        asset_path: "/fonts/Sarabun-Regular.ttf",
    },
    BuiltInFontVariant {
        family: "Sarabun",
        weight: 700,
        style: CertificateFontStyle::Normal,
        asset_path: "/fonts/Sarabun-Bold.ttf",
    },
];

pub(super) fn built_in_fonts() -> Vec<CertificateBuiltInFont> {
    BUILT_IN_FONT_VARIANTS
        .iter()
        .map(|variant| CertificateBuiltInFont {
            family: variant.family.to_string(),
            weight: variant.weight,
            style: variant.style,
            asset_path: variant.asset_path.to_string(),
        })
        .collect()
}

fn is_supported_built_in_font(family: &str, weight: u16, style: CertificateFontStyle) -> bool {
    BUILT_IN_FONT_VARIANTS.iter().any(|variant| {
        variant.family == family && variant.weight == weight && variant.style == style
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaperKind {
    A4,
    A5,
    Letter,
    Custom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaperOrientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackgroundLayoutAction {
    Preserve,
    Scale,
    Reset,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayoutValidationError {
    InvalidSchemaVersion,
    TooManyElements,
    MultipleQrElements,
    DuplicateElementId,
    InvalidFrame,
    ElementOutsidePage,
    InvalidRotation,
    InvalidText,
    InvalidFont,
    InvalidColor,
    InvalidShadow,
    InvalidImageAspectRatio,
    ImageAspectRatioMismatch,
    UnknownVariable(String),
    InvalidSafeMargin,
    GeometryMismatch,
}

pub fn recognize_paper(width_points: f64, height_points: f64) -> (PaperKind, PaperOrientation) {
    let orientation = if width_points <= height_points {
        PaperOrientation::Portrait
    } else {
        PaperOrientation::Landscape
    };
    let (short_side, long_side) = if width_points <= height_points {
        (width_points, height_points)
    } else {
        (height_points, width_points)
    };
    let paper = [
        (PaperKind::A4, 210.0, 297.0),
        (PaperKind::A5, 148.0, 210.0),
        (PaperKind::Letter, 215.9, 279.4),
    ]
    .into_iter()
    .find(|(_, expected_short_mm, expected_long_mm)| {
        (short_side - expected_short_mm * POINTS_PER_MM).abs() <= PAPER_TOLERANCE_POINTS
            && (long_side - expected_long_mm * POINTS_PER_MM).abs() <= PAPER_TOLERANCE_POINTS
    })
    .map_or(PaperKind::Custom, |(paper, _, _)| paper);
    (paper, orientation)
}

pub fn paper_label(width_points: f64, height_points: f64) -> String {
    let (paper, orientation) = recognize_paper(width_points, height_points);
    let orientation = match orientation {
        PaperOrientation::Portrait => "แนวตั้ง",
        PaperOrientation::Landscape => "แนวนอน",
    };
    match paper {
        PaperKind::A4 => format!("A4 {orientation}"),
        PaperKind::A5 => format!("A5 {orientation}"),
        PaperKind::Letter => format!("Letter {orientation}"),
        PaperKind::Custom => format!(
            "ขนาดกำหนดเอง {:.1} × {:.1} มม. {orientation}",
            width_points / POINTS_PER_MM,
            height_points / POINTS_PER_MM,
        ),
    }
}

pub fn validate_safe_margin(
    safe_margin_points: f64,
    page_width: f64,
    page_height: f64,
) -> Result<(), LayoutValidationError> {
    if !safe_margin_points.is_finite()
        || safe_margin_points < 0.0
        || !page_width.is_finite()
        || !page_height.is_finite()
        || page_width <= 0.0
        || page_height <= 0.0
        || safe_margin_points * 2.0 >= page_width.min(page_height)
    {
        return Err(LayoutValidationError::InvalidSafeMargin);
    }
    Ok(())
}

pub fn validate_layout(
    layout: &CertificateLayoutV1,
    page: PageGeometry,
    custom_variables: &[String],
) -> Result<(), LayoutValidationError> {
    if layout.schema_version != 1 {
        return Err(LayoutValidationError::InvalidSchemaVersion);
    }
    if layout.elements.len() > MAX_ELEMENTS {
        return Err(LayoutValidationError::TooManyElements);
    }
    let allowed_variables = RENDERABLE_STANDARD_VARIABLES
        .into_iter()
        .chain(RESERVED_RENDER_VARIABLES)
        .map(normalize_name_for_match)
        .chain(
            custom_variables
                .iter()
                .filter_map(|variable| match classify_header(variable) {
                    HeaderClass::Custom(variable) => Some(normalize_name_for_match(&variable)),
                    _ => None,
                }),
        )
        .collect::<BTreeSet<_>>();
    let (page_width, page_height) = page.displayed_size();
    let mut ids = BTreeSet::new();
    let mut has_qr = false;

    for element in &layout.elements {
        if !ids.insert(element.id()) {
            return Err(LayoutValidationError::DuplicateElementId);
        }
        validate_frame(element.frame(), page_width, page_height)?;
        if !element.rotation().is_finite() {
            return Err(LayoutValidationError::InvalidRotation);
        }
        validate_rotated_frame(element.frame(), element.rotation(), page_width, page_height)?;

        match element {
            CertificateElement::Text(text) => {
                if text.content.chars().count() > 10_000 {
                    return Err(LayoutValidationError::InvalidText);
                }
                if text.font_family.trim().is_empty()
                    || text.font_family.chars().count() > 200
                    || !(100..=900).contains(&text.font_weight)
                    || text.font_weight % 100 != 0
                    || !text.font_size.is_finite()
                    || !text.min_font_size.is_finite()
                    || text.font_size <= 0.0
                    || text.min_font_size <= 0.0
                    || text.min_font_size > text.font_size
                    || !text.line_height.is_finite()
                    || !(0.5..=5.0).contains(&text.line_height)
                {
                    return Err(LayoutValidationError::InvalidFont);
                }
                if matches!(text.font_source, CertificateFontSource::BuiltIn)
                    && !is_supported_built_in_font(
                        &text.font_family,
                        text.font_weight,
                        text.font_style,
                    )
                {
                    return Err(LayoutValidationError::InvalidFont);
                }
                if !valid_color(&text.color) {
                    return Err(LayoutValidationError::InvalidColor);
                }
                if let Some(shadow) = &text.shadow {
                    if !shadow.offset_x.is_finite()
                        || !shadow.offset_y.is_finite()
                        || !shadow.blur.is_finite()
                        || shadow.blur < 0.0
                        || !valid_color(&shadow.color)
                    {
                        return Err(LayoutValidationError::InvalidShadow);
                    }
                }
                let variables = referenced_variables(&text.content)
                    .map_err(|_| LayoutValidationError::InvalidText)?;
                for variable in variables {
                    if !allowed_variables.contains(&normalize_name_for_match(&variable)) {
                        return Err(LayoutValidationError::UnknownVariable(variable));
                    }
                }
            }
            CertificateElement::Qr(_) => {
                if has_qr {
                    return Err(LayoutValidationError::MultipleQrElements);
                }
                has_qr = true;
            }
            CertificateElement::Image(image) => {
                let aspect_ratio = image.aspect_ratio;
                if !aspect_ratio.is_finite() || aspect_ratio <= 0.0 {
                    return Err(LayoutValidationError::InvalidImageAspectRatio);
                }
                if image.lock_aspect_ratio {
                    let frame_ratio = image.frame.width / image.frame.height;
                    let tolerance = aspect_ratio.abs().max(frame_ratio.abs()).max(1.0)
                        * ASPECT_RATIO_RELATIVE_TOLERANCE;
                    if (frame_ratio - aspect_ratio).abs() > tolerance {
                        return Err(LayoutValidationError::ImageAspectRatioMismatch);
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn adapt_layout_for_background(
    layout: &CertificateLayoutV1,
    old_page: PageGeometry,
    new_page: PageGeometry,
    action: BackgroundLayoutAction,
) -> Result<CertificateLayoutV1, LayoutValidationError> {
    match action {
        BackgroundLayoutAction::Reset => Ok(CertificateLayoutV1::default()),
        BackgroundLayoutAction::Preserve => {
            if geometries_are_equivalent(old_page, new_page) {
                Ok(layout.clone())
            } else {
                Err(LayoutValidationError::GeometryMismatch)
            }
        }
        BackgroundLayoutAction::Scale => {
            let (old_width, old_height) = old_page.displayed_size();
            let (new_width, new_height) = new_page.displayed_size();
            let scale_x = new_width / old_width;
            let scale_y = new_height / old_height;
            let uniform_scale = scale_x.min(scale_y);
            if !scale_x.is_finite()
                || !scale_y.is_finite()
                || !uniform_scale.is_finite()
                || scale_x <= 0.0
                || scale_y <= 0.0
            {
                return Err(LayoutValidationError::GeometryMismatch);
            }

            let mut scaled = layout.clone();
            for element in &mut scaled.elements {
                let frame = element.frame_mut();
                frame.x *= scale_x;
                frame.y *= scale_y;
                match element {
                    CertificateElement::Text(text) => {
                        text.frame.width *= scale_x;
                        text.frame.height *= scale_y;
                        text.font_size *= uniform_scale;
                        text.min_font_size *= uniform_scale;
                        if let Some(shadow) = &mut text.shadow {
                            shadow.offset_x *= uniform_scale;
                            shadow.offset_y *= uniform_scale;
                            shadow.blur *= uniform_scale;
                        }
                    }
                    CertificateElement::Image(image) => {
                        image.frame.width *= uniform_scale;
                        image.frame.height *= uniform_scale;
                    }
                    CertificateElement::Qr(qr) => {
                        qr.frame.width *= uniform_scale;
                        qr.frame.height *= uniform_scale;
                    }
                }
            }
            Ok(scaled)
        }
    }
}

pub fn geometries_are_equivalent(left: PageGeometry, right: PageGeometry) -> bool {
    let (left_width, left_height) = left.source_size();
    let (right_width, right_height) = right.source_size();
    left.rotation() == right.rotation()
        && (left_width - right_width).abs() <= GEOMETRY_EPSILON
        && (left_height - right_height).abs() <= GEOMETRY_EPSILON
}

fn validate_frame(
    frame: ElementFrame,
    page_width: f64,
    page_height: f64,
) -> Result<(), LayoutValidationError> {
    if ![frame.x, frame.y, frame.width, frame.height]
        .into_iter()
        .all(f64::is_finite)
        || frame.x < 0.0
        || frame.y < 0.0
        || frame.width <= 0.0
        || frame.height <= 0.0
    {
        return Err(LayoutValidationError::InvalidFrame);
    }
    if frame.x + frame.width > page_width + GEOMETRY_EPSILON
        || frame.y + frame.height > page_height + GEOMETRY_EPSILON
    {
        return Err(LayoutValidationError::ElementOutsidePage);
    }
    Ok(())
}

fn validate_rotated_frame(
    frame: ElementFrame,
    rotation_degrees: f64,
    page_width: f64,
    page_height: f64,
) -> Result<(), LayoutValidationError> {
    let radians = rotation_degrees.to_radians();
    let extent_x =
        radians.cos().abs() * frame.width / 2.0 + radians.sin().abs() * frame.height / 2.0;
    let extent_y =
        radians.sin().abs() * frame.width / 2.0 + radians.cos().abs() * frame.height / 2.0;
    let center_x = frame.x + frame.width / 2.0;
    let center_y = frame.y + frame.height / 2.0;
    if center_x - extent_x < -GEOMETRY_EPSILON
        || center_y - extent_y < -GEOMETRY_EPSILON
        || center_x + extent_x > page_width + GEOMETRY_EPSILON
        || center_y + extent_y > page_height + GEOMETRY_EPSILON
    {
        return Err(LayoutValidationError::ElementOutsidePage);
    }
    Ok(())
}

fn valid_color(color: &str) -> bool {
    matches!(color.len(), 7 | 9)
        && color.starts_with('#')
        && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::modules::certificates::{
        models::{
            CertificateElement, CertificateFontStyle, CertificateLayoutV1, ElementFrame,
            ImageElement, PageGeometry, QrElement, TextAlignment, TextElement,
        },
        services::layout::{
            adapt_layout_for_background, paper_label, recognize_paper, validate_layout,
            validate_safe_margin, BackgroundLayoutAction, LayoutValidationError, PaperKind,
            PaperOrientation,
        },
    };

    fn text_layout() -> CertificateLayoutV1 {
        CertificateLayoutV1 {
            schema_version: 1,
            elements: vec![CertificateElement::Text(TextElement {
                id: Uuid::nil(),
                content: "มอบให้ {ชื่อ}".into(),
                frame: ElementFrame {
                    x: 10.0,
                    y: 20.0,
                    width: 100.0,
                    height: 40.0,
                },
                rotation: 0.0,
                font_source: Default::default(),
                font_family: "Sarabun".into(),
                font_weight: 400,
                font_style: CertificateFontStyle::Normal,
                font_size: 24.0,
                min_font_size: 12.0,
                color: "#112233".into(),
                alignment: TextAlignment::Center,
                line_height: 1.2,
                auto_shrink: true,
                shadow: None,
            })],
        }
    }

    #[test]
    fn recognizes_standard_paper_within_one_millimeter_and_validates_margin() {
        let points_per_mm = 72.0 / 25.4;
        assert_eq!(
            recognize_paper(210.8 * points_per_mm, 296.2 * points_per_mm),
            (PaperKind::A4, PaperOrientation::Portrait)
        );
        assert_eq!(
            recognize_paper(297.0 * points_per_mm, 210.0 * points_per_mm),
            (PaperKind::A4, PaperOrientation::Landscape)
        );
        assert!(validate_safe_margin(10.0 * points_per_mm, 595.28, 841.89).is_ok());
        assert!(validate_safe_margin(300.0, 500.0, 500.0).is_err());
        assert_eq!(
            paper_label(123.0 * points_per_mm, 234.0 * points_per_mm),
            "ขนาดกำหนดเอง 123.0 × 234.0 มม. แนวตั้ง"
        );
    }

    #[test]
    fn rejects_unsupported_built_in_font_family_weight_and_style() {
        let page = PageGeometry::new(200.0, 100.0, 0).unwrap();
        for (family, weight, style) in [
            ("Unknown Thai", 400, CertificateFontStyle::Normal),
            ("Sarabun", 500, CertificateFontStyle::Normal),
            ("Sarabun", 400, CertificateFontStyle::Italic),
        ] {
            let mut layout = text_layout();
            let CertificateElement::Text(text) = &mut layout.elements[0] else {
                unreachable!("test layout must contain text");
            };
            text.font_family = family.to_string();
            text.font_weight = weight;
            text.font_style = style;
            assert_eq!(
                validate_layout(&layout, page, &["ชื่อ".into()]),
                Err(LayoutValidationError::InvalidFont),
                "unsupported built-in tuple {family}/{weight}/{} must fail",
                style.as_str()
            );
        }
    }

    #[test]
    fn validates_bounds_and_scales_or_resets_in_displayed_coordinates() {
        let layout = text_layout();
        let old = PageGeometry::new(200.0, 100.0, 0).unwrap();
        let new = PageGeometry::new(100.0, 400.0, 90).unwrap();
        let scaled =
            adapt_layout_for_background(&layout, old, new, BackgroundLayoutAction::Scale).unwrap();
        let CertificateElement::Text(text) = &scaled.elements[0] else {
            panic!("expected text")
        };
        assert_eq!(text.frame.x, 20.0);
        assert_eq!(text.frame.y, 20.0);
        assert_eq!(text.frame.width, 200.0);
        assert_eq!(text.frame.height, 40.0);
        assert_eq!(text.font_size, 24.0);
        assert!(validate_layout(&scaled, new, &["ชื่อ".into()]).is_ok());

        assert_eq!(
            adapt_layout_for_background(&layout, old, new, BackgroundLayoutAction::Reset,).unwrap(),
            CertificateLayoutV1::default()
        );
    }

    #[test]
    fn displayed_dimensions_follow_all_right_angle_page_rotations() {
        for (rotation, expected) in [
            (0, (200.0, 100.0)),
            (90, (100.0, 200.0)),
            (180, (200.0, 100.0)),
            (270, (100.0, 200.0)),
        ] {
            assert_eq!(
                PageGeometry::new(200.0, 100.0, rotation)
                    .unwrap()
                    .displayed_size(),
                expected
            );
        }
    }

    #[test]
    fn scales_and_resets_deterministically_at_every_page_rotation() {
        for rotation in [0, 90, 180, 270] {
            let old = PageGeometry::new(200.0, 100.0, rotation).unwrap();
            let new = PageGeometry::new(400.0, 200.0, rotation).unwrap();
            let scaled = adapt_layout_for_background(
                &text_layout(),
                old,
                new,
                BackgroundLayoutAction::Scale,
            )
            .unwrap();
            let CertificateElement::Text(text) = &scaled.elements[0] else {
                panic!("expected text")
            };
            assert_eq!(text.frame.x, 20.0);
            assert_eq!(text.frame.y, 40.0);
            assert_eq!(text.font_size, 48.0);
            assert_eq!(
                adapt_layout_for_background(
                    &text_layout(),
                    old,
                    new,
                    BackgroundLayoutAction::Reset,
                )
                .unwrap(),
                CertificateLayoutV1::default()
            );
        }
    }

    #[test]
    fn image_and_qr_sizes_use_uniform_scaling_without_distortion() {
        let layout = CertificateLayoutV1 {
            schema_version: 1,
            elements: vec![
                CertificateElement::Image(ImageElement {
                    id: Uuid::new_v4(),
                    frame: ElementFrame {
                        x: 10.0,
                        y: 10.0,
                        width: 80.0,
                        height: 40.0,
                    },
                    rotation: 0.0,
                    asset_id: Uuid::new_v4(),
                    lock_aspect_ratio: true,
                    aspect_ratio: 2.0,
                }),
                CertificateElement::Qr(QrElement {
                    id: Uuid::new_v4(),
                    frame: ElementFrame {
                        x: 10.0,
                        y: 10.0,
                        width: 30.0,
                        height: 30.0,
                    },
                    rotation: 0.0,
                }),
            ],
        };
        let scaled = adapt_layout_for_background(
            &layout,
            PageGeometry::new(200.0, 100.0, 0).unwrap(),
            PageGeometry::new(400.0, 100.0, 0).unwrap(),
            BackgroundLayoutAction::Scale,
        )
        .unwrap();
        assert_eq!(scaled.elements[0].frame().width, 80.0);
        assert_eq!(scaled.elements[0].frame().height, 40.0);
        assert_eq!(scaled.elements[1].frame().width, 30.0);
        assert_eq!(scaled.elements[1].frame().height, 30.0);
        assert_eq!(scaled.elements[0].frame().x, 20.0);
    }

    #[test]
    fn layout_requires_explicit_font_style_and_image_aspect_contract() {
        let complete = serde_json::json!({
            "schemaVersion": 1,
            "elements": [
                {
                    "type": "text",
                    "id": Uuid::new_v4(),
                    "content": "มอบให้ {ชื่อ}",
                    "frame": {"x": 10.0, "y": 10.0, "width": 90.0, "height": 30.0},
                    "rotation": 0.0,
                    "fontSource": {"type": "built_in"},
                    "fontFamily": "Sarabun",
                    "fontWeight": 400,
                    "fontStyle": "normal",
                    "fontSize": 24.0,
                    "minFontSize": 12.0,
                    "color": "#112233",
                    "alignment": "center",
                    "lineHeight": 1.2,
                    "autoShrink": true,
                    "shadow": null
                },
                {
                    "type": "image",
                    "id": Uuid::new_v4(),
                    "frame": {"x": 110.0, "y": 10.0, "width": 80.0, "height": 40.0},
                    "rotation": 0.0,
                    "assetId": Uuid::new_v4(),
                    "lockAspectRatio": true,
                    "aspectRatio": 2.0
                }
            ]
        });

        for (element_index, field) in [(0, "fontStyle"), (1, "lockAspectRatio"), (1, "aspectRatio")]
        {
            let mut incomplete = complete.clone();
            incomplete["elements"][element_index]
                .as_object_mut()
                .expect("element fixture must be an object")
                .remove(field);
            assert!(
                serde_json::from_value::<CertificateLayoutV1>(incomplete).is_err(),
                "missing {field} must be rejected"
            );
        }

        let layout: CertificateLayoutV1 = serde_json::from_value(complete).unwrap();

        let CertificateElement::Text(text) = &layout.elements[0] else {
            panic!("expected text")
        };
        assert_eq!(text.font_style, CertificateFontStyle::Normal);
        let CertificateElement::Image(image) = &layout.elements[1] else {
            panic!("expected image")
        };
        assert!(image.lock_aspect_ratio);
        assert_eq!(image.aspect_ratio, 2.0);
    }

    #[test]
    fn validates_image_aspect_ratio_and_locked_frame_consistency() {
        let page = PageGeometry::new(200.0, 100.0, 0).unwrap();
        let mut layout = CertificateLayoutV1 {
            schema_version: 1,
            elements: vec![CertificateElement::Image(ImageElement {
                id: Uuid::new_v4(),
                frame: ElementFrame {
                    x: 10.0,
                    y: 10.0,
                    width: 80.0,
                    height: 40.0,
                },
                rotation: 0.0,
                asset_id: Uuid::new_v4(),
                lock_aspect_ratio: true,
                aspect_ratio: 2.0,
            })],
        };
        assert!(validate_layout(&layout, page, &[]).is_ok());

        if let CertificateElement::Image(image) = &mut layout.elements[0] {
            image.aspect_ratio = 0.0;
        }
        assert_eq!(
            validate_layout(&layout, page, &[]),
            Err(LayoutValidationError::InvalidImageAspectRatio)
        );
        if let CertificateElement::Image(image) = &mut layout.elements[0] {
            image.aspect_ratio = 1.0;
        }
        assert_eq!(
            validate_layout(&layout, page, &[]),
            Err(LayoutValidationError::ImageAspectRatioMismatch)
        );
        if let CertificateElement::Image(image) = &mut layout.elements[0] {
            image.lock_aspect_ratio = false;
        }
        assert!(validate_layout(&layout, page, &[]).is_ok());
    }

    #[test]
    fn rejects_more_than_one_qr_element() {
        let qr = |x| {
            CertificateElement::Qr(QrElement {
                id: Uuid::new_v4(),
                frame: ElementFrame {
                    x,
                    y: 10.0,
                    width: 30.0,
                    height: 30.0,
                },
                rotation: 0.0,
            })
        };
        let layout = CertificateLayoutV1 {
            schema_version: 1,
            elements: vec![qr(10.0), qr(50.0)],
        };

        assert_eq!(
            validate_layout(&layout, PageGeometry::new(200.0, 100.0, 0).unwrap(), &[]),
            Err(LayoutValidationError::MultipleQrElements)
        );
    }

    #[test]
    fn rejects_unknown_variables_nonfinite_rotation_and_page_overflow() {
        let page = PageGeometry::new(200.0, 100.0, 0).unwrap();
        let mut layout = text_layout();
        if let CertificateElement::Text(text) = &mut layout.elements[0] {
            text.content = "{ตัวแปรที่ไม่มี}".into();
        }
        assert!(matches!(
            validate_layout(&layout, page, &[]),
            Err(LayoutValidationError::UnknownVariable(_))
        ));
        if let CertificateElement::Text(text) = &mut layout.elements[0] {
            text.content = "ข้อความ".into();
            text.rotation = f64::INFINITY;
        }
        assert_eq!(
            validate_layout(&layout, page, &[]),
            Err(LayoutValidationError::InvalidRotation)
        );
        if let CertificateElement::Text(text) = &mut layout.elements[0] {
            text.rotation = 0.0;
            text.frame.x = 150.0;
        }
        assert_eq!(
            validate_layout(&layout, page, &[]),
            Err(LayoutValidationError::ElementOutsidePage)
        );

        if let CertificateElement::Text(text) = &mut layout.elements[0] {
            text.frame.x = 0.0;
            text.frame.y = 0.0;
            text.frame.width = 100.0;
            text.frame.height = 20.0;
            text.rotation = 45.0;
        }
        assert_eq!(
            validate_layout(&layout, page, &[]),
            Err(LayoutValidationError::ElementOutsidePage)
        );

        if let CertificateElement::Text(text) = &mut layout.elements[0] {
            text.frame.x = 10.0;
            text.frame.y = 20.0;
            text.frame.width = 100.0;
            text.frame.height = 40.0;
            text.rotation = 0.0;
            text.content = "{รหัสนักเรียน}".into();
        }
        assert!(matches!(
            validate_layout(&layout, page, &["รหัสนักเรียน".into()]),
            Err(LayoutValidationError::UnknownVariable(_))
        ));
    }

    #[test]
    fn layout_json_is_tagged_and_denies_unknown_fields() {
        let value = serde_json::to_value(text_layout()).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["elements"][0]["type"], "text");

        let mut unexpected = value;
        unexpected["elements"][0]["html"] = serde_json::json!("<b>unsafe field</b>");
        assert!(serde_json::from_value::<CertificateLayoutV1>(unexpected).is_err());
    }

    #[test]
    fn preserve_requires_the_same_source_geometry_and_rotation() {
        let layout = text_layout();
        let source = PageGeometry::new(200.0, 100.0, 0).unwrap();
        assert_eq!(
            adapt_layout_for_background(
                &layout,
                source,
                PageGeometry::new(200.005, 99.995, 0).unwrap(),
                BackgroundLayoutAction::Preserve,
            )
            .unwrap(),
            layout
        );
        assert_eq!(
            adapt_layout_for_background(
                &layout,
                source,
                PageGeometry::new(200.0, 100.0, 90).unwrap(),
                BackgroundLayoutAction::Preserve,
            ),
            Err(LayoutValidationError::GeometryMismatch)
        );
    }
}
