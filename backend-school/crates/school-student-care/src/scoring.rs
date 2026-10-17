use crate::models::*;
use school_errors::AppError;
use std::collections::{BTreeMap, BTreeSet};

pub fn invalid(message: &str) -> AppError {
    AppError::ValidationError(message.into())
}
pub fn text(value: &str, maximum: usize, required: bool) -> Result<(), AppError> {
    if (required && value.trim().is_empty()) || value.chars().count() > maximum {
        return Err(invalid("ข้อความว่างหรือยาวเกินกำหนด"));
    }
    Ok(())
}
fn key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 60
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
pub fn validate_definition(d: &CareInstrumentDefinition) -> Result<(), AppError> {
    text(&d.name, 200, true)?;
    text(&d.variant, 120, true)?;
    text(&d.language, 30, true)?;
    text(&d.instructions, 10000, false)?;
    text(&d.source, 2000, true)?;
    text(&d.license_note, 2000, true)?;
    if d.minimum_age < 0 || d.maximum_age > 120 || d.maximum_age < d.minimum_age {
        return Err(invalid("ช่วงอายุไม่ถูกต้อง"));
    }
    if d.mode == CareInstrumentMode::Digital && !d.digital_rights_confirmed {
        return Err(invalid("ต้องยืนยันสิทธิ์ใช้แบบประเมินดิจิทัล"));
    }
    if d.scales.is_empty()
        || d.scales.len() > 30
        || d.questions.len() > 200
        || (d.mode == CareInstrumentMode::Digital && d.questions.is_empty())
        || (d.mode == CareInstrumentMode::ManualResult && !d.questions.is_empty())
    {
        return Err(invalid("จำนวนข้อหรือมาตรวัดไม่ถูกต้อง"));
    }
    let mut scales = BTreeSet::new();
    for s in &d.scales {
        if !key(&s.key) || !scales.insert(&s.key) || s.bands.is_empty() || s.bands.len() > 30 {
            return Err(invalid("มาตรวัดหรือช่วงคะแนนซ้ำ/ไม่ถูกต้อง"));
        }
        text(&s.label, 200, true)?;
        let mut bands = s.bands.iter().collect::<Vec<_>>();
        bands.sort_by_key(|b| b.minimum);
        let mut keys = BTreeSet::new();
        for (i, b) in bands.iter().enumerate() {
            text(&b.label, 200, true)?;
            if !key(&b.key)
                || !keys.insert(&b.key)
                || b.minimum > b.maximum
                || b.minimum < -200000
                || b.maximum > 200000
                || (i > 0 && bands[i - 1].maximum.checked_add(1) != Some(b.minimum))
            {
                return Err(invalid("ช่วงคะแนนต้องต่อเนื่อง ไม่ซ้ำ และมีขอบเขต"));
            }
        }
    }
    for s in &d.scales {
        let mut refs = BTreeSet::new();
        for source in &s.source_scale_keys {
            if source == &s.key
                || !refs.insert(source)
                || !d
                    .scales
                    .iter()
                    .any(|base| base.key == *source && base.source_scale_keys.is_empty())
            {
                return Err(invalid("มาตรรวมต้องอ้างอิงมาตรฐานโดยไม่ซ้ำหรือวนรอบ"));
            }
        }
    }
    for aggregate in d.scales.iter().filter(|s| !s.source_scale_keys.is_empty()) {
        let mut minimum = 0_i32;
        let mut maximum = 0_i32;
        for source in &aggregate.source_scale_keys {
            let base = d
                .scales
                .iter()
                .find(|s| s.key == *source)
                .ok_or_else(|| invalid("ไม่มีมาตรฐาน"))?;
            minimum = minimum
                .checked_add(
                    base.bands
                        .iter()
                        .map(|b| b.minimum)
                        .min()
                        .ok_or_else(|| invalid("ไม่มีช่วงคะแนน"))?,
                )
                .ok_or_else(|| invalid("ผลรวมเกินขอบเขต"))?;
            maximum = maximum
                .checked_add(
                    base.bands
                        .iter()
                        .map(|b| b.maximum)
                        .max()
                        .ok_or_else(|| invalid("ไม่มีช่วงคะแนน"))?,
                )
                .ok_or_else(|| invalid("ผลรวมเกินขอบเขต"))?;
        }
        if !aggregate
            .bands
            .iter()
            .any(|b| minimum >= b.minimum && minimum <= b.maximum)
            || !aggregate
                .bands
                .iter()
                .any(|b| maximum >= b.minimum && maximum <= b.maximum)
        {
            return Err(invalid("ช่วงคะแนนมาตรรวมไม่ครอบคลุมผลรวม"));
        }
    }
    let mut questions = BTreeSet::new();
    let mut totals: BTreeMap<&str, (i32, i32)> = BTreeMap::new();
    for q in &d.questions {
        if !key(&q.key)
            || !questions.insert(&q.key)
            || !d
                .scales
                .iter()
                .any(|s| s.key == q.scale_key && s.source_scale_keys.is_empty())
            || q.options.len() < 2
            || q.options.len() > 20
        {
            return Err(invalid("ข้อคำถาม/ตัวเลือกไม่ถูกต้อง"));
        }
        text(&q.text, 2000, true)?;
        let mut options = BTreeSet::new();
        for o in &q.options {
            text(&o.label, 500, true)?;
            if !key(&o.key) || !options.insert(&o.key) || !(-1000..=1000).contains(&o.points) {
                return Err(invalid("ตัวเลือกซ้ำหรือคะแนนเกินกำหนด"));
            }
        }
        let minimum = q
            .options
            .iter()
            .map(|o| o.points)
            .min()
            .ok_or_else(|| invalid("ไม่มีตัวเลือก"))?;
        let maximum = q
            .options
            .iter()
            .map(|o| o.points)
            .max()
            .ok_or_else(|| invalid("ไม่มีตัวเลือก"))?;
        let total = totals.entry(&q.scale_key).or_default();
        total.0 += minimum;
        total.1 += maximum;
    }
    if d.mode == CareInstrumentMode::Digital {
        for s in d.scales.iter().filter(|s| s.source_scale_keys.is_empty()) {
            let (min, max) = totals
                .get(s.key.as_str())
                .ok_or_else(|| invalid("มาตรวัดไม่มีข้อคำถาม"))?;
            if !s
                .bands
                .iter()
                .any(|b| b.minimum <= *min && b.maximum >= *min)
                || !s
                    .bands
                    .iter()
                    .any(|b| b.minimum <= *max && b.maximum >= *max)
            {
                return Err(invalid("ช่วงคะแนนไม่ครอบคลุมผลรวมทั้งหมด"));
            }
        }
    }
    Ok(())
}
pub fn validate_answers(
    d: &CareInstrumentDefinition,
    answers: &[CareAnswer],
    complete: bool,
) -> Result<(), AppError> {
    if answers.len() > d.questions.len() {
        return Err(invalid("คำตอบเกินจำนวนข้อ"));
    }
    let mut seen = BTreeSet::new();
    for a in answers {
        if !seen.insert(&a.question_key)
            || !d
                .questions
                .iter()
                .any(|q| q.key == a.question_key && q.options.iter().any(|o| o.key == a.option_key))
        {
            return Err(invalid("คำตอบซ้ำหรือไม่ตรงรุ่นแบบประเมิน"));
        }
    }
    if complete && answers.len() != d.questions.len() {
        return Err(invalid("ต้องตอบครบทุกข้อก่อนส่ง"));
    }
    Ok(())
}
pub fn score(
    d: &CareInstrumentDefinition,
    answers: &[CareAnswer],
    manual: &[CareManualScore],
) -> Result<(Vec<CareScaleResult>, bool), AppError> {
    validate_definition(d)?;
    let mut totals = BTreeMap::<&str, i32>::new();
    let mut urgent = false;
    match d.mode {
        CareInstrumentMode::Digital => {
            if !manual.is_empty() {
                return Err(invalid("แบบดิจิทัลไม่รับคะแนนกำหนดเอง"));
            }
            validate_answers(d, answers, true)?;
            for q in &d.questions {
                let a = answers
                    .iter()
                    .find(|a| a.question_key == q.key)
                    .ok_or_else(|| invalid("คำตอบไม่ครบ"))?;
                let o = q
                    .options
                    .iter()
                    .find(|o| o.key == a.option_key)
                    .ok_or_else(|| invalid("ไม่มีตัวเลือก"))?;
                let points = if q.reverse {
                    q.options
                        .iter()
                        .map(|o| o.points)
                        .min()
                        .ok_or_else(|| invalid("ไม่มีตัวเลือก"))?
                        + q.options
                            .iter()
                            .map(|o| o.points)
                            .max()
                            .ok_or_else(|| invalid("ไม่มีตัวเลือก"))?
                        - o.points
                } else {
                    o.points
                };
                *totals.entry(&q.scale_key).or_default() += points;
                urgent |= o.urgent;
            }
        }
        CareInstrumentMode::ManualResult => {
            if !answers.is_empty()
                || manual.len()
                    != d.scales
                        .iter()
                        .filter(|s| s.source_scale_keys.is_empty())
                        .count()
            {
                return Err(invalid("ต้องบันทึกคะแนนมาตรวัดครบถ้วน"));
            }
            for s in manual {
                if !d.scales.iter().any(|v| {
                    v.key == s.scale_key
                        && v.source_scale_keys.is_empty()
                        && v.bands
                            .iter()
                            .any(|b| s.score >= b.minimum && s.score <= b.maximum)
                }) || totals.insert(&s.scale_key, s.score).is_some()
                {
                    return Err(invalid("มาตรวัดคะแนนซ้ำหรือไม่ถูกต้อง"));
                }
            }
        }
    }
    for s in d.scales.iter().filter(|s| !s.source_scale_keys.is_empty()) {
        let mut sum = 0;
        for source in &s.source_scale_keys {
            sum = i32::checked_add(
                sum,
                *totals
                    .get(source.as_str())
                    .ok_or_else(|| invalid("คะแนนมาตรฐานไม่ครบ"))?,
            )
            .ok_or_else(|| invalid("ผลรวมเกินขอบเขต"))?;
        }
        totals.insert(&s.key, sum);
    }
    let mut results = Vec::new();
    for s in &d.scales {
        let value = *totals
            .get(s.key.as_str())
            .ok_or_else(|| invalid("ไม่มีคะแนนมาตรวัด"))?;
        let b = s
            .bands
            .iter()
            .find(|b| value >= b.minimum && value <= b.maximum)
            .ok_or_else(|| invalid("คะแนนนอกช่วงที่กำหนด"))?;
        urgent |= b.urgent;
        results.push(CareScaleResult {
            scale_key: s.key.clone(),
            score: value,
            band_key: b.key.clone(),
            band_label: b.label.clone(),
            urgent: b.urgent,
        });
    }
    Ok((results, urgent))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn definition() -> CareInstrumentDefinition {
        CareInstrumentDefinition {
            name: "Synthetic fixture".into(),
            family: CareFamily::Other,
            variant: "test".into(),
            language: "en".into(),
            respondent: CareRespondent::SelfReport,
            minimum_age: 5,
            maximum_age: 20,
            mode: CareInstrumentMode::Digital,
            instructions: String::new(),
            source: "Synthetic, nonclinical".into(),
            license_note: "Test fixture".into(),
            digital_rights_confirmed: true,
            questions: vec![CareQuestion {
                key: "q".into(),
                text: "Synthetic choice".into(),
                scale_key: "s".into(),
                reverse: false,
                options: vec![
                    CareOption {
                        key: "a".into(),
                        label: "A".into(),
                        points: 0,
                        urgent: false,
                    },
                    CareOption {
                        key: "b".into(),
                        label: "B".into(),
                        points: 2,
                        urgent: true,
                    },
                ],
            }],
            scales: vec![CareScale {
                key: "s".into(),
                label: "Test".into(),
                source_scale_keys: vec![],
                bands: vec![CareBand {
                    key: "all".into(),
                    label: "Test only".into(),
                    minimum: 0,
                    maximum: 2,
                    urgent: false,
                }],
            }],
        }
    }
    #[test]
    fn missing_is_not_zero_and_partial_is_only_a_draft() {
        let d = definition();
        assert!(validate_answers(&d, &[], false).is_ok());
        assert!(score(&d, &[], &[]).is_err());
    }
    #[test]
    fn reverse_points_and_option_urgent_are_independent() {
        let mut d = definition();
        d.questions[0].reverse = true;
        let (r, u) = score(
            &d,
            &[CareAnswer {
                question_key: "q".into(),
                option_key: "b".into(),
            }],
            &[],
        )
        .unwrap();
        assert_eq!(r[0].score, 0);
        assert!(u);
    }
    #[test]
    fn unlicensed_digital_and_incomplete_bands_are_rejected() {
        let mut d = definition();
        d.digital_rights_confirmed = false;
        assert!(validate_definition(&d).is_err());
        d.digital_rights_confirmed = true;
        d.scales[0].bands[0].maximum = 1;
        assert!(validate_definition(&d).is_err());
    }
    #[test]
    fn aggregates_exclude_unreferenced_dimensions_and_reject_overflow() {
        let mut d = definition();
        d.mode = CareInstrumentMode::ManualResult;
        d.questions.clear();
        let mut social = d.scales[0].clone();
        social.key = "social".into();
        d.scales.push(social);
        d.scales.push(CareScale {
            key: "total".into(),
            label: "Total synthetic".into(),
            source_scale_keys: vec!["s".into()],
            bands: d.scales[0].bands.clone(),
        });
        let (r, _) = score(
            &d,
            &[],
            &[
                CareManualScore {
                    scale_key: "s".into(),
                    score: 1,
                },
                CareManualScore {
                    scale_key: "social".into(),
                    score: 2,
                },
            ],
        )
        .unwrap();
        assert_eq!(r[2].score, 1);
        assert!(score(
            &d,
            &[],
            &[
                CareManualScore {
                    scale_key: "s".into(),
                    score: i32::MAX
                },
                CareManualScore {
                    scale_key: "social".into(),
                    score: 2
                }
            ]
        )
        .is_err());
        d.scales[2].source_scale_keys.push("total".into());
        assert!(validate_definition(&d).is_err());
    }
    #[test]
    fn manual_scoring_is_all_or_nothing() {
        let mut d = definition();
        d.mode = CareInstrumentMode::ManualResult;
        d.questions.clear();
        assert!(score(&d, &[], &[]).is_err());
        assert!(score(
            &d,
            &[],
            &[CareManualScore {
                scale_key: "s".into(),
                score: 3
            }]
        )
        .is_err());
        assert!(score(
            &d,
            &[],
            &[CareManualScore {
                scale_key: "s".into(),
                score: 1
            }]
        )
        .is_ok());
    }
}
