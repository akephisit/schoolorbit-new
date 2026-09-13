use super::*;
use crate::modules::academic::learner_evaluation::models::LearnerEvaluationDomain;
use sqlx::types::Json;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
struct BaselineLabels {
    term_name: String,
    criterion_name: Option<String>,
    evaluation_domain: Option<LearnerEvaluationDomain>,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
struct ResultBaseline {
    annual_revision_id: Uuid,
    academic_term_id: Uuid,
    result_id: Uuid,
    kind: &'static str,
    source_version: i64,
    #[serde(flatten)]
    labels: BaselineLabels,
}

type Baselines = BTreeMap<(Uuid, &'static str, Uuid), ResultBaseline>;
const MAX_BASELINES: usize = 100_000;

/// The caller owns authorization and a consistent read/transition transaction.
/// Corrections are compared with the exact effective versions pinned by the
/// annual snapshot, not timestamps that may predate a transaction's lock wait.
pub(crate) async fn corrections_after_annuals(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    annual_ids: &[Uuid],
) -> Result<Vec<AnnualCorrectionEvidence>, AppError> {
    if year.is_nil()
        || annual_ids.is_empty()
        || annual_ids.len() > 500
        || annual_ids.iter().any(Uuid::is_nil)
        || annual_ids.iter().collect::<BTreeSet<_>>().len() != annual_ids.len()
    {
        return Err(AppError::ValidationError(
            "ตรวจผลแก้ไขได้ครั้งละ 1–500 รุ่นผลรายปี โดยไม่ซ้ำรายการ".into(),
        ));
    }
    let annuals: Vec<(Uuid, Json<AnnualResultPreview>)> = sqlx::query_as(
        "SELECT id,snapshot FROM academic_annual_result_revisions WHERE academic_year_id=$1 AND id=ANY($2) ORDER BY id",
    ).bind(year).bind(annual_ids).fetch_all(&mut **tx).await?;
    if annuals.len() != annual_ids.len() {
        return Err(AppError::NotFound("ไม่พบรุ่นผลรายปีครบตามปีการศึกษาที่ระบุ".into()));
    }
    let mut baselines = Baselines::new();
    for (id, snapshot) in annuals {
        append_annual_baselines(&mut baselines, id, year, &snapshot)?;
    }
    if baselines.is_empty() {
        return Ok(Vec::new());
    }
    let baselines: Vec<_> = baselines.into_values().collect();
    let rows: Vec<AnnualCorrectionEvidence> = sqlx::query_as(
        "WITH baseline AS (
           SELECT * FROM jsonb_to_recordset($1) AS b(annual_revision_id uuid,academic_term_id uuid,result_id uuid,kind text,source_version bigint,term_name text,criterion_name text,evaluation_domain text)
         )
         SELECT b.annual_revision_id,b.academic_term_id,b.result_id,b.source_version AS source_effective_version,
           b.term_name,b.criterion_name,b.evaluation_domain,offering.code_snapshot AS offering_code,offering.name_snapshot AS offering_name,
           jsonb_build_object('id',c.id,'expectedEffectiveVersion',c.expected_effective_version,
             'previous', CASE b.kind
               WHEN 'course' THEN jsonb_build_object('kind','course','outcome',c.old_course_outcome,'numericGrade',c.old_numeric_grade::text)
               WHEN 'activity' THEN jsonb_build_object('kind','activity','outcome',c.old_activity_outcome)
               ELSE jsonb_build_object('kind','learner_evaluation','qualityLevel',c.old_quality_level) END,
             'corrected', CASE b.kind
               WHEN 'course' THEN jsonb_build_object('kind','course','outcome',c.new_course_outcome,'numericGrade',c.new_numeric_grade::text)
               WHEN 'activity' THEN jsonb_build_object('kind','activity','outcome',c.new_activity_outcome)
               ELSE jsonb_build_object('kind','learner_evaluation','qualityLevel',c.new_quality_level) END,
             'correctedBy',c.corrected_by,'correctedAt',c.corrected_at) AS correction
         FROM baseline b JOIN academic_result_corrections c ON
           ((b.kind='course' AND c.course_result_id=b.result_id)
             OR (b.kind='activity' AND c.activity_result_id=b.result_id)
             OR (b.kind='learner_evaluation' AND c.subject_student_evaluation_id=b.result_id))
         LEFT JOIN academic_course_results cr ON cr.id=c.course_result_id
         LEFT JOIN academic_activity_results ar ON ar.id=c.activity_result_id
         LEFT JOIN subject_term_student_evaluations er ON er.id=c.subject_student_evaluation_id
         JOIN learning_offerings offering ON offering.id=COALESCE(cr.learning_offering_id,ar.learning_offering_id,er.learning_offering_id)
         WHERE c.expected_effective_version>=b.source_version
         ORDER BY b.annual_revision_id,b.academic_term_id,b.kind,b.result_id,c.expected_effective_version,c.id
         LIMIT 10001",
    ).bind(Json(&baselines)).fetch_all(&mut **tx).await?;
    if rows.len() > 10_000 {
        return Err(AppError::ValidationError(
            "ผลแก้ไขในชุดนี้เกิน 10,000 รายการ กรุณาแบ่งชุดที่ตรวจให้เล็กลง".into(),
        ));
    }
    Ok(rows)
}

fn invalid_source() -> AppError {
    AppError::InternalServerError("ข้อมูลอ้างอิงในรุ่นผลรายปีไม่สอดคล้องกัน".into())
}

fn append_annual_baselines(
    baselines: &mut Baselines,
    annual: Uuid,
    year: Uuid,
    snapshot: &AnnualResultPreview,
) -> Result<(), AppError> {
    if snapshot.academic_year_id != year {
        return Err(invalid_source());
    }
    for term in &snapshot.terms {
        let Some(revision) = &term.revision else {
            continue;
        };
        let source = &revision.snapshot;
        if source.results.academic_year_id != year
            || source.results.academic_term_id != term.academic_term_id
            || source.results.student_academic_year_id != snapshot.student_academic_year_id
            || source.learner_evaluations.student_academic_year_id
                != snapshot.student_academic_year_id
        {
            return Err(invalid_source());
        }
        for course in &source.results.courses {
            add_baseline(
                baselines,
                annual,
                term.academic_term_id,
                "course",
                course.result_id,
                course.effective_version,
                BaselineLabels {
                    term_name: term.term_name.clone(),
                    criterion_name: None,
                    evaluation_domain: None,
                },
            )?;
        }
        for activity in &source.results.activities {
            add_baseline(
                baselines,
                annual,
                term.academic_term_id,
                "activity",
                activity.result_id,
                activity.effective_version,
                BaselineLabels {
                    term_name: term.term_name.clone(),
                    criterion_name: None,
                    evaluation_domain: None,
                },
            )?;
        }
        for criterion in source
            .learner_evaluations
            .domains
            .iter()
            .flat_map(|domain| &domain.subjects)
            .flat_map(|subject| &subject.criteria)
        {
            add_baseline(
                baselines,
                annual,
                term.academic_term_id,
                "learner_evaluation",
                Some(criterion.id),
                Some(criterion.row_version),
                BaselineLabels {
                    term_name: term.term_name.clone(),
                    criterion_name: Some(criterion.name.clone()),
                    evaluation_domain: Some(criterion.domain),
                },
            )?;
        }
    }
    Ok(())
}

fn add_baseline(
    baselines: &mut Baselines,
    annual: Uuid,
    term: Uuid,
    kind: &'static str,
    result: Option<Uuid>,
    version: Option<i64>,
    labels: BaselineLabels,
) -> Result<(), AppError> {
    let (result, version) = match (result, version) {
        (None, None) => return Ok(()),
        (Some(result), Some(version)) if !result.is_nil() && version > 0 => (result, version),
        _ => return Err(invalid_source()),
    };
    let key = (annual, kind, result);
    let value = ResultBaseline {
        annual_revision_id: annual,
        academic_term_id: term,
        kind,
        result_id: result,
        source_version: version,
        labels,
    };
    if let Some(previous) = baselines.get(&key) {
        if previous != &value {
            return Err(invalid_source());
        }
        return Ok(());
    }
    if baselines.len() >= MAX_BASELINES {
        return Err(AppError::ValidationError(
            "ข้อมูลอ้างอิงเกินขอบเขต กรุณาแบ่งชุดรุ่นผลรายปีที่ตรวจ".into(),
        ));
    }
    baselines.insert(key, value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annual_correction_baselines_reject_inconsistent_versions_and_deduplicate_identity() {
        let annual = Uuid::new_v4();
        let term = Uuid::new_v4();
        let result = Uuid::new_v4();
        let mut rows = Baselines::new();
        let labels = || BaselineLabels {
            term_name: "E2E-term".into(),
            criterion_name: None,
            evaluation_domain: None,
        };
        for _ in 0..2 {
            add_baseline(
                &mut rows,
                annual,
                term,
                "course",
                Some(result),
                Some(1),
                labels(),
            )
            .unwrap();
        }
        assert_eq!(rows.len(), 1);
        assert!(add_baseline(
            &mut rows,
            annual,
            term,
            "course",
            Some(result),
            Some(2),
            labels()
        )
        .is_err());
        assert!(add_baseline(
            &mut rows,
            annual,
            Uuid::new_v4(),
            "course",
            Some(result),
            Some(1),
            labels()
        )
        .is_err());
        for (id, version) in [
            (Some(result), None),
            (None, Some(1)),
            (Some(Uuid::nil()), Some(1)),
            (Some(result), Some(0)),
        ] {
            assert!(
                add_baseline(&mut rows, annual, term, "activity", id, version, labels()).is_err()
            );
        }
        add_baseline(&mut rows, annual, term, "activity", None, None, labels()).unwrap();
        assert_eq!(rows.len(), 1);
    }
}
