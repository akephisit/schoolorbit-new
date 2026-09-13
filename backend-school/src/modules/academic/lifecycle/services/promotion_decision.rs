use super::super::models::{
    PromotionDecisionInput, PromotionDecisionOutcome as Outcome, PromotionRecommendation,
    PromotionSuccessOutcome,
};
use crate::error::AppError;
use uuid::Uuid;

pub(crate) fn validate_decision(
    input: &PromotionDecisionInput,
    source_grade: Uuid,
    recommendation: &PromotionRecommendation,
) -> Result<(), AppError> {
    if source_grade.is_nil()
        || [
            input.target_grade_level_id,
            input.target_study_program_id,
            input.target_homeroom_id,
        ]
        .into_iter()
        .flatten()
        .any(|id| id.is_nil())
    {
        return Err(AppError::ValidationError(
            "ข้อมูลชั้น แผน หรือห้องเรียนไม่ถูกต้อง".into(),
        ));
    }
    for text in [&input.reason, &input.condition].into_iter().flatten() {
        validate_text(text)?;
    }
    match input.outcome {
        Outcome::Promote | Outcome::Repeat | Outcome::Conditional => {
            if input.target_grade_level_id.is_none() || input.target_study_program_id.is_none() {
                return Err(AppError::ValidationError("ระบุชั้นและแผนปลายทางให้ครบ".into()));
            }
            let same_grade = input.target_grade_level_id == Some(source_grade);
            if (input.outcome == Outcome::Repeat && !same_grade)
                || (input.outcome == Outcome::Promote && same_grade)
            {
                return Err(AppError::ValidationError(
                    "การซ้ำชั้นต้องเป็นชั้นเดิม ส่วนการเลื่อนชั้นต้องเป็นชั้นใหม่".into(),
                ));
            }
        }
        Outcome::Graduate | Outcome::TransferOut | Outcome::Hold => {
            if input.target_grade_level_id.is_some()
                || input.target_study_program_id.is_some()
                || input.target_homeroom_id.is_some()
            {
                return Err(AppError::ValidationError(
                    "ผลนี้ไม่สร้างรายการนักเรียนปีใหม่ จึงไม่ระบุปลายทาง".into(),
                ));
            }
        }
    }
    if (input.outcome == Outcome::Conditional) != input.condition.is_some() {
        return Err(AppError::ValidationError(
            "ระบุเงื่อนไขเฉพาะผลเลื่อนชั้นแบบมีเงื่อนไข".into(),
        ));
    }
    let suggested = match recommendation.suggested_outcome {
        Some(PromotionSuccessOutcome::Promote) => Some(Outcome::Promote),
        Some(PromotionSuccessOutcome::Graduate) => Some(Outcome::Graduate),
        None => None,
    };
    let follows_recommendation = recommendation.findings.is_empty()
        && suggested == Some(input.outcome)
        && input.target_grade_level_id == recommendation.target_grade_level_id
        && input.target_study_program_id == recommendation.target_study_program_id;
    if !follows_recommendation && input.reason.is_none() {
        return Err(AppError::ValidationError(
            "ระบุเหตุผลเมื่อพักรอหรือเลือกผลต่างจากคำแนะนำ".into(),
        ));
    }
    Ok(())
}

fn validate_text(text: &str) -> Result<(), AppError> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 1000 {
        return Err(AppError::ValidationError(
            "เหตุผลและเงื่อนไขต้องยาว 1–1000 ตัวอักษร".into(),
        ));
    }
    let mut digits = 0u8;
    for character in text.chars() {
        if character.is_numeric() {
            digits += 1;
            if digits >= 13 {
                return Err(AppError::ValidationError(
                    "ห้ามระบุเลขประจำตัวประชาชนในเหตุผลหรือเงื่อนไข".into(),
                ));
            }
        } else if character.is_alphabetic() {
            digits = 0;
        }
    }
    Ok(())
}

pub(crate) fn validate_reason(text: &str) -> Result<(), AppError> {
    validate_text(text)
}

#[cfg(test)]
mod tests {
    use super::super::super::models::{
        PromotionDecisionOutcome as Outcome, PromotionRecommendationFinding,
        PromotionSuccessOutcome,
    };
    use super::*;

    fn fixture() -> (Uuid, PromotionRecommendation, PromotionDecisionInput) {
        let source = Uuid::from_u128(1);
        let recommendation = PromotionRecommendation {
            suggested_outcome: Some(PromotionSuccessOutcome::Promote),
            target_grade_level_id: Some(Uuid::from_u128(2)),
            target_study_program_id: Some(Uuid::from_u128(3)),
            findings: vec![],
        };
        let input = PromotionDecisionInput {
            outcome: Outcome::Promote,
            target_grade_level_id: Some(Uuid::from_u128(2)),
            target_study_program_id: Some(Uuid::from_u128(3)),
            target_homeroom_id: None,
            reason: None,
            condition: None,
        };
        (source, recommendation, input)
    }

    #[test]
    fn promotion_decision_accepts_exact_recommendation_but_requires_reason_for_overrides() {
        let (source, recommendation, input) = fixture();
        assert!(validate_decision(&input, source, &recommendation).is_ok());
        let mut changed = input.clone();
        changed.target_study_program_id = Some(Uuid::from_u128(4));
        assert!(validate_decision(&changed, source, &recommendation).is_err());
        changed.reason = Some("เลือกแผนตามความประสงค์ของนักเรียน".into());
        assert!(validate_decision(&changed, source, &recommendation).is_ok());
        let mut missing = recommendation.clone();
        missing.suggested_outcome = None;
        missing.findings = vec![PromotionRecommendationFinding::PolicyRuleMissing];
        assert!(validate_decision(&input, source, &missing).is_err());
        assert!(validate_decision(&changed, source, &missing).is_ok());
        let mut contradictory = recommendation.clone();
        contradictory.findings = vec![PromotionRecommendationFinding::InsufficientEarnedCredits];
        assert!(validate_decision(&input, source, &contradictory).is_err());
    }

    #[test]
    fn promotion_decision_enforces_grade_direction_and_requires_complete_destination() {
        let (source, recommendation, input) = fixture();
        let mut repeat = input.clone();
        repeat.outcome = Outcome::Repeat;
        repeat.reason = Some("พิจารณาให้ซ้ำชั้น".into());
        assert!(validate_decision(&repeat, source, &recommendation).is_err());
        repeat.target_grade_level_id = Some(source);
        assert!(validate_decision(&repeat, source, &recommendation).is_ok());
        repeat.outcome = Outcome::Promote;
        assert!(validate_decision(&repeat, source, &recommendation).is_err());
        for field in 0..4 {
            let mut bad = input.clone();
            match field {
                0 => bad.target_grade_level_id = None,
                1 => bad.target_study_program_id = None,
                2 => bad.target_homeroom_id = Some(Uuid::nil()),
                _ => bad.target_grade_level_id = Some(Uuid::nil()),
            }
            assert!(validate_decision(&bad, source, &recommendation).is_err());
        }
        let mut placed = input;
        placed.target_homeroom_id = Some(Uuid::from_u128(8));
        assert!(validate_decision(&placed, source, &recommendation).is_ok());
        assert!(validate_decision(&placed, Uuid::nil(), &recommendation).is_err());
    }

    #[test]
    fn promotion_decision_terminal_outcomes_never_carry_target_enrollment() {
        let (source, recommendation, mut input) = fixture();
        for outcome in [Outcome::Graduate, Outcome::TransferOut, Outcome::Hold] {
            input.outcome = outcome;
            input.reason = Some("ฝ่ายวิชาการพิจารณาแล้ว".into());
            input.target_grade_level_id = Some(Uuid::from_u128(2));
            input.target_study_program_id = Some(Uuid::from_u128(3));
            assert!(validate_decision(&input, source, &recommendation).is_err());
            input.target_grade_level_id = None;
            input.target_study_program_id = None;
            assert!(validate_decision(&input, source, &recommendation).is_ok());
            input.target_homeroom_id = Some(Uuid::from_u128(8));
            assert!(validate_decision(&input, source, &recommendation).is_err());
            input.target_homeroom_id = None;
            input.reason = None;
            assert!(validate_decision(&input, source, &recommendation).is_err());
        }
        input.outcome = Outcome::Graduate;
        let graduate = PromotionRecommendation {
            suggested_outcome: Some(PromotionSuccessOutcome::Graduate),
            target_grade_level_id: None,
            target_study_program_id: None,
            findings: vec![],
        };
        assert!(validate_decision(&input, source, &graduate).is_ok());
    }

    #[test]
    fn promotion_decision_conditional_requires_explicit_condition_and_reason() {
        let (source, recommendation, mut input) = fixture();
        input.outcome = Outcome::Conditional;
        input.reason = Some("อนุญาตให้เรียนต่อโดยมีเงื่อนไข".into());
        assert!(validate_decision(&input, source, &recommendation).is_err());
        input.condition = Some("ให้ผ่านกิจกรรมที่ค้างก่อนสิ้นเดือน".into());
        assert!(validate_decision(&input, source, &recommendation).is_ok());
        input.reason = None;
        assert!(validate_decision(&input, source, &recommendation).is_err());
        input.outcome = Outcome::Promote;
        assert!(validate_decision(&input, source, &recommendation).is_err());
    }

    #[test]
    fn promotion_decision_rejects_blank_overlong_or_identifier_like_free_text() {
        let (source, recommendation, input) = fixture();
        for text in [
            " ".into(),
            "ก".repeat(1001),
            "1234567890123".into(),
            "1-2345-67890-12-3".into(),
            "๑๒๓๔๕๖๗๘๙๐๑๒๓".into(),
        ] {
            let mut bad = input.clone();
            bad.reason = Some(text.clone());
            assert!(validate_decision(&bad, source, &recommendation).is_err());
            bad.outcome = Outcome::Conditional;
            bad.reason = Some("พิจารณาเป็นรายคน".into());
            bad.condition = Some(text);
            assert!(validate_decision(&bad, source, &recommendation).is_err());
        }
        let mut boundary = input;
        boundary.reason = Some("ก".repeat(1000));
        assert!(validate_decision(&boundary, source, &recommendation).is_ok());
    }
}
