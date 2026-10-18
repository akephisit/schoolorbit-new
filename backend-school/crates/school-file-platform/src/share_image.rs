//! Fixed social banners. Only bundled fonts and server-resolved, inspected crest bytes enter here.
//! Kanit comes from google/fonts ofl/kanit; its SIL OFL is bundled in assets.
use bytes::Bytes;
use image::{DynamicImage, ImageFormat, RgbaImage};
use std::{io::Cursor, sync::OnceLock};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::{
    file_inspector::inspect_file, platform_service::FilePlatformError, platform_types::FilePurpose,
    ImageProcessor,
};

const WIDTH: u32 = 1200;
const HEIGHT: u32 = 630;
const INK: [u8; 4] = [22, 46, 70, 255];
const TEAL: [u8; 4] = [35, 124, 155, 255];
const REGULAR: &[u8] = include_bytes!("../assets/Kanit-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../assets/Kanit-SemiBold.ttf");
const SUBTITLE: &str = "ระบบจัดการโรงเรียนครบวงจร";

fn font(bold: bool) -> Result<&'static rustybuzz::Face<'static>, FilePlatformError> {
    static REGULAR_FACE: OnceLock<Option<rustybuzz::Face<'static>>> = OnceLock::new();
    static BOLD_FACE: OnceLock<Option<rustybuzz::Face<'static>>> = OnceLock::new();
    let (cell, bytes) = if bold {
        (&BOLD_FACE, BOLD)
    } else {
        (&REGULAR_FACE, REGULAR)
    };
    cell.get_or_init(|| rustybuzz::Face::from_slice(bytes, 0))
        .as_ref()
        .ok_or(FilePlatformError::InspectionRejected)
}

fn shape(text: &str, bold: bool) -> Result<rustybuzz::GlyphBuffer, FilePlatformError> {
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    Ok(rustybuzz::shape(font(bold)?, &[], buffer))
}

fn text_width(text: &str, size: f32, bold: bool) -> Result<f32, FilePlatformError> {
    Ok(shape(text, bold)?
        .glyph_positions()
        .iter()
        .map(|p| p.x_advance as f32)
        .sum::<f32>()
        * size
        / font(bold)?.units_per_em() as f32)
}

/// Split only at shaped cluster boundaries, so Thai vowels and tone marks stay with their base.
fn name_lines(name: &str, size: f32) -> Result<Vec<String>, FilePlatformError> {
    let mut lines = Vec::new();
    let mut remaining = name.trim();
    while !remaining.is_empty() {
        let remaining_width = text_width(remaining, size, true)?;
        if remaining_width <= 650.0 {
            lines.push(remaining.to_owned());
            break;
        }
        let line_count = (remaining_width / 650.0).ceil();
        let target_width = (remaining_width / line_count).min(645.0);
        let shaped = shape(remaining, true)?;
        let scale = size / font(true)?.units_per_em() as f32;
        let mut width = 0.0;
        let mut end = 0;
        let mut previous_cluster = 0;
        for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
            if info.cluster != previous_cluster {
                if width > target_width {
                    break;
                }
                end = info.cluster as usize;
                previous_cluster = info.cluster;
            }
            width += position.x_advance as f32 * scale;
        }
        if end == 0 {
            return Err(FilePlatformError::InspectionRejected);
        }
        let end = remaining[..end]
            .rfind(char::is_whitespace)
            .filter(|&space| space > end / 2)
            .unwrap_or(end);
        lines.push(remaining[..end].trim().to_owned());
        remaining = remaining[end..].trim_start();
    }
    Ok(lines)
}

fn fit_name(name: &str) -> Result<(Vec<String>, f32), FilePlatformError> {
    for size in (2..=60).rev() {
        let lines = name_lines(name, size as f32)?;
        if lines.len() <= 3 {
            let mut fits = true;
            for line in &lines {
                fits &= text_width(line, size as f32, true)? <= 650.0;
            }
            if fits {
                return Ok((lines, size as f32));
            }
        }
    }
    Err(FilePlatformError::InspectionRejected)
}

fn paint(color: [u8; 4]) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;
    paint
}

struct Outline(PathBuilder);
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.0.close();
    }
}

fn text(
    canvas: &mut Pixmap,
    value: &str,
    x: f32,
    baseline: f32,
    size: f32,
    bold: bool,
    color: [u8; 4],
) -> Result<(), FilePlatformError> {
    let face = font(bold)?;
    let scale = size / face.units_per_em() as f32;
    let shaped = shape(value, bold)?;
    let mut pen = x;
    for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
        let mut outline = Outline(PathBuilder::new());
        face.outline_glyph(ttf_parser::GlyphId(info.glyph_id as u16), &mut outline);
        if let Some(path) = outline.0.finish() {
            canvas.fill_path(
                &path,
                &paint(color),
                FillRule::Winding,
                Transform::from_row(
                    scale,
                    0.0,
                    0.0,
                    -scale,
                    pen + position.x_offset as f32 * scale,
                    baseline - position.y_offset as f32 * scale,
                ),
                None,
            );
        }
        pen += position.x_advance as f32 * scale;
    }
    Ok(())
}

fn circle(canvas: &mut Pixmap, x: f32, y: f32, radius: f32, color: [u8; 4]) {
    if let Some(path) = PathBuilder::from_circle(x, y, radius) {
        canvas.fill_path(
            &path,
            &paint(color),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn curve(canvas: &mut Pixmap, points: [f32; 8], width: f32, color: [u8; 4]) {
    let mut path = PathBuilder::new();
    path.move_to(points[0], points[1]);
    path.cubic_to(
        points[2], points[3], points[4], points[5], points[6], points[7],
    );
    if let Some(path) = path.finish() {
        canvas.stroke_path(
            &path,
            &paint(color),
            &Stroke {
                width,
                ..Stroke::default()
            },
            Transform::identity(),
            None,
        );
    }
}

/// Geometry matches the Admin Orbit mark, whose original Lucide ISC notice is bundled in assets.
fn orbit_mark(canvas: &mut Pixmap, x: f32, y: f32, size: f32) {
    let scale = size / 40.0;
    circle(canvas, x + size / 2.0, y + size / 2.0, size / 2.0, TEAL);
    let transform = Transform::from_row(scale, 0.0, 0.0, scale, x + 8.0 * scale, y + 8.0 * scale);
    let mut path = PathBuilder::new();
    // Cubic equivalents of the source SVG's two circular arcs.
    path.move_to(20.341, 6.484);
    path.cubic_to(
        22.546067, 9.816975, 22.555002, 14.142772, 20.363722, 17.484828,
    );
    path.cubic_to(18.172442, 20.826883, 14.201768, 22.543377, 10.266, 21.85);
    path.move_to(3.659, 17.516);
    path.cubic_to(1.453948, 14.182123, 1.446007, 9.855404, 3.638808, 6.513456);
    path.cubic_to(5.831608, 3.171507, 9.803872, 1.456378, 13.74, 2.152);
    for (cx, cy, radius) in [(12.0, 12.0, 3.0), (19.0, 5.0, 2.0), (5.0, 19.0, 2.0)] {
        path.push_circle(cx, cy, radius);
    }
    if let Some(path) = path.finish() {
        canvas.stroke_path(
            &path,
            &paint([255; 4]),
            &Stroke {
                width: 1.6,
                line_cap: tiny_skia::LineCap::Round,
                line_join: tiny_skia::LineJoin::Round,
                ..Stroke::default()
            },
            transform,
            None,
        );
    }
}

fn school_display_name(name: &str) -> String {
    let name = name.trim();
    name.strip_prefix("โรงเรียน")
        .unwrap_or(name)
        .trim()
        .to_owned()
}

pub(crate) fn render_png(
    crest: Option<&[u8]>,
    name: &str,
    domain: &str,
) -> Result<Bytes, FilePlatformError> {
    if name.len() > 4096 || domain.len() > 253 {
        return Err(FilePlatformError::InspectionRejected);
    }
    let mut canvas = Pixmap::new(WIDTH, HEIGHT).ok_or(FilePlatformError::InspectionRejected)?;
    canvas.fill(tiny_skia::Color::WHITE);
    for y in 0..HEIGHT {
        let t = y as f32 / HEIGHT as f32;
        for x in 0..WIDTH {
            let offset = ((y * WIDTH + x) * 4) as usize;
            canvas.data_mut()[offset..offset + 4].copy_from_slice(&[
                (252.0 - 14.0 * t) as u8,
                (254.0 - 8.0 * t) as u8,
                255,
                255,
            ]);
        }
    }
    curve(
        &mut canvas,
        [-90.0, 510.0, 200.0, 100.0, 840.0, 620.0, 1300.0, 120.0],
        2.0,
        [190, 221, 239, 150],
    );
    curve(
        &mut canvas,
        [-100.0, 560.0, 250.0, 220.0, 780.0, 740.0, 1280.0, 220.0],
        1.5,
        [209, 231, 244, 180],
    );
    curve(
        &mut canvas,
        [1220.0, 280.0, 1080.0, 410.0, 1040.0, 550.0, 1250.0, 660.0],
        2.5,
        [139, 192, 223, 150],
    );
    curve(
        &mut canvas,
        [1260.0, 350.0, 1090.0, 465.0, 1100.0, 600.0, 1190.0, 680.0],
        1.5,
        [168, 208, 232, 170],
    );
    curve(
        &mut canvas,
        [-40.0, 620.0, 250.0, 510.0, 580.0, 700.0, 870.0, 650.0],
        2.0,
        [174, 210, 233, 160],
    );
    circle(&mut canvas, 120.0, 88.0, 5.0, [137, 191, 215, 160]);
    circle(&mut canvas, 1110.0, 124.0, 7.0, [137, 191, 215, 120]);
    circle(&mut canvas, 263.0, 306.0, 202.0, [227, 240, 249, 170]);
    circle(&mut canvas, 263.0, 302.0, 190.0, [255, 255, 255, 255]);
    if crest.is_none() {
        orbit_mark(&mut canvas, 123.0, 162.0, 280.0);
    }
    let display_name = school_display_name(name);
    let missing_name = display_name.is_empty();
    let name = if missing_name {
        "โรงเรียน"
    } else {
        &display_name
    };
    let (lines, size) = fit_name(name)?;
    let line_height = size * 1.25;
    let first_baseline = 295.0 - (lines.len().saturating_sub(1) as f32 * line_height) / 2.0;
    if !missing_name {
        text(
            &mut canvas,
            "โรงเรียน",
            488.0,
            first_baseline - 74.0,
            42.0,
            false,
            INK,
        )?;
    }
    for (index, line) in lines.iter().enumerate() {
        text(
            &mut canvas,
            line,
            486.0,
            first_baseline + index as f32 * line_height,
            size,
            true,
            INK,
        )?;
    }
    let subtitle_y =
        (first_baseline + (lines.len().saturating_sub(1) as f32 * line_height) + 66.0).max(360.0);
    text(
        &mut canvas,
        SUBTITLE,
        489.0,
        subtitle_y,
        30.0,
        false,
        [64, 102, 126, 255],
    )?;
    let domain_size = (590.0 / text_width(domain, 24.0, false)?).min(1.0) * 24.0;
    text(
        &mut canvas,
        domain,
        490.0,
        subtitle_y + 47.0,
        domain_size,
        false,
        [87, 130, 154, 255],
    )?;
    orbit_mark(&mut canvas, 895.0, 535.0, 42.0);
    text(&mut canvas, "School", 948.0, 565.0, 28.0, true, INK)?;
    let orbit_x = 948.0 + text_width("School", 28.0, true)?;
    text(&mut canvas, "Orbit", orbit_x, 565.0, 28.0, true, TEAL)?;
    let mut image = RgbaImage::from_raw(WIDTH, HEIGHT, canvas.take())
        .ok_or(FilePlatformError::InspectionRejected)?;
    if let Some(source) = crest {
        let inspected = inspect_file(FilePurpose::SchoolLogo, source)?;
        let decoded = ImageProcessor::decode_inspected_image(&inspected)
            .map_err(|_| FilePlatformError::InspectionRejected)?;
        let contained = decoded
            .resize(326, 326, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        image::imageops::overlay(
            &mut image,
            &contained,
            100 + i64::from((326 - contained.width()) / 2),
            139 + i64::from((326 - contained.height()) / 2),
        );
    }
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|_| FilePlatformError::InspectionRejected)?;
    Ok(Bytes::from(encoded.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn crest(width: u32, height: u32) -> Vec<u8> {
        let image = RgbaImage::from_fn(width, height, |x, y| {
            if x < width / 2 && y < height / 2 {
                Rgba([240, 40, 40, 255])
            } else if x >= width / 2 && y < height / 2 {
                Rgba([40, 180, 40, 255])
            } else if x < width / 2 {
                Rgba([40, 40, 230, 255])
            } else {
                Rgba([220, 150, 20, 255])
            }
        });
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn school_share_banner_is_png_and_contains_whole_portrait_and_landscape_original() {
        for (width, height) in [(160, 320), (320, 160), (320, 320)] {
            let png = render_png(
                Some(&crest(width, height)),
                "โรงเรียนตัวอย่างเพื่อการเรียนรู้",
                "sample.example.test",
            )
            .unwrap();
            assert_eq!(image::guess_format(&png).unwrap(), ImageFormat::Png);
            let image = image::load_from_memory(&png).unwrap().to_rgba8();
            assert_eq!(image.dimensions(), (1200, 630));
            let scale = 326.0 / width.max(height) as f32;
            let (w, h) = (
                (width as f32 * scale).round() as u32,
                (height as f32 * scale).round() as u32,
            );
            let (left, top) = (100 + (326 - w) / 2, 139 + (326 - h) / 2);
            // All four original corners remain visible, including wide and tall sources.
            for (x, y, expected) in [
                (left + 4, top + 4, [240, 40, 40, 255]),
                (left + w - 5, top + 4, [40, 180, 40, 255]),
                (left + 4, top + h - 5, [40, 40, 230, 255]),
                (left + w - 5, top + h - 5, [220, 150, 20, 255]),
            ] {
                assert_eq!(image.get_pixel(x, y).0, expected);
            }
        }
    }

    #[test]
    fn school_share_thai_marks_are_shaped_and_long_names_fit_in_three_lines() {
        let shaped = shape("โรงเรียนรู้ กิ้ กุ้", true).unwrap();
        assert!(shaped.glyph_infos().iter().all(|glyph| glyph.glyph_id != 0));
        assert!(shaped
            .glyph_positions()
            .iter()
            .any(|position| position.x_offset != 0 || position.y_offset != 0));
        for name in ["โรงเรียนทดสอบ", "โรงเรียนสาธิตเพื่อพัฒนาคุณภาพการศึกษาและการเรียนรู้อย่างสร้างสรรค์แห่งชุมชนตัวอย่าง".repeat(3).as_str(), "A Very Long International Demonstration School & Learning Centre <Example> \"ชื่อทดสอบ\""] {
            let (lines, size) = fit_name(name).unwrap();
            assert!(lines.len() <= 3);
            for line in &lines { assert!(text_width(line, size, true).unwrap() <= 650.0); }
            assert_eq!(lines.join("").replace(' ', ""), name.replace(' ', ""));
            assert!(size * 1.35 * lines.len() as f32 <= 243.0);
            let png = render_png(None, name, "sample.example.test").unwrap();
            let image = image::load_from_memory(&png).unwrap().to_rgba8();
            // Actual glyph outlines, including Thai, are rasterized in the name rectangle.
            assert!(image.enumerate_pixels().any(|(x, y, pixel)| x >= 486 && x < 1140 && y >= 145 && y < 340 && pixel.0[0] < 50 && pixel.0[1] < 75));
        }
    }

    #[test]
    fn school_share_literal_text_and_missing_branding_are_safe() {
        assert_eq!(school_display_name("ตัวอย่าง"), "ตัวอย่าง");
        assert_eq!(school_display_name("โรงเรียนตัวอย่าง"), "ตัวอย่าง");
        let png = render_png(None, "<script>&\" โรงเรียนทดสอบ", "sample.example.test").unwrap();
        assert_eq!(image::load_from_memory(&png).unwrap().width(), 1200);
        let fallback = render_png(None, "  ", "sample.example.test").unwrap();
        assert_eq!(
            fallback,
            render_png(None, "โรงเรียน", "sample.example.test").unwrap()
        );
        assert_eq!(
            render_png(Some(b"not an image"), "โรงเรียน", "sample.example.test"),
            Err(FilePlatformError::InspectionRejected)
        );
        assert!(render_png(None, &"x".repeat(4097), "sample.example.test").is_err());
    }

    #[test]
    fn school_share_optional_visual_fixtures() {
        let Ok(directory) = std::env::var("SCHOOL_SHARE_FIXTURE_DIR") else {
            return;
        };
        std::fs::create_dir_all(&directory).unwrap();
        let crest = std::env::var("SCHOOL_SHARE_FIXTURE_CREST")
            .ok()
            .map(|path| std::fs::read(path).unwrap());
        for (filename, name) in [
            ("school-share.png", "ตัวอย่างเพื่อการเรียนรู้".to_owned()),
            (
                "school-share-long.png",
                "โรงเรียนสาธิตเพื่อพัฒนาคุณภาพการศึกษาและการเรียนรู้อย่างสร้างสรรค์แห่งชุมชนตัวอย่าง".repeat(2),
            ),
        ] {
            let png = render_png(crest.as_deref(), &name, "sample.example.test").unwrap();
            std::fs::write(std::path::Path::new(&directory).join(filename), png).unwrap();
        }
        if let Ok(name) = std::env::var("SCHOOL_SHARE_FIXTURE_NAME") {
            std::fs::write(
                std::path::Path::new(&directory).join("school-share-actual-name.png"),
                render_png(crest.as_deref(), &name, "sample.example.test").unwrap(),
            )
            .unwrap();
        }
        std::fs::write(
            std::path::Path::new(&directory).join("school-share-fallback.png"),
            render_png(None, "โรงเรียนตัวอย่างเพื่อการเรียนรู้", "sample.example.test").unwrap(),
        )
        .unwrap();
    }
}
