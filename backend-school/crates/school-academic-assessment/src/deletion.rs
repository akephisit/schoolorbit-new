//! Exact current Delivery cleanup; app composition removes official Results first.
use school_academic_delivery::models::current::DeliveryDeletionScope;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, Postgres, Transaction};

const GROUP_TABLES: &[&str] = &[
    "learning_group_phase_confirmations",
    "learning_group_result_confirmations",
    "learning_group_evaluation_confirmations",
    "academic_activity_result_confirmations",
    "learning_group_student_scores",
    "learning_group_student_evaluations",
    "learning_group_result_overrides",
    "academic_activity_evaluations",
    "learning_group_score_items",
];
#[derive(Debug, Clone)]
pub struct AssessmentDeletionImpact {
    pub exam_items: i64,
    pub scores: i64,
    pub source_digest: String,
}

pub async fn preview_on_connection(
    conn: &mut PgConnection,
    scope: &DeliveryDeletionScope,
) -> Result<AssessmentDeletionImpact, AppError> {
    let mut digests = Vec::new();
    let mut scores = 0;
    for &table in GROUP_TABLES {
        let sql=format!("SELECT count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY row.id),'[]')::text,'UTF8')),'hex') FROM {table} row WHERE row.academic_year_id=$1 AND row.academic_term_id=$2 AND row.learning_group_id=ANY($3)");
        let (count, digest): (i64, String) = DbSession::new(conn)
            .query_as(
                "academic_assessment.deletion.group_impact",
                sqlx::AssertSqlSafe(sql),
            )
            .bind(scope.academic_year_id)
            .bind(scope.academic_term_id)
            .bind(&scope.learning_group_ids)
            .fetch_one()
            .await?;
        if count > 200_000 {
            return Err(AppError::ConflictWithCode {
                code: "ASSESSMENT_DELETION_TOO_LARGE",
                message: "ข้อมูลคะแนนและการประเมินที่เลือกมากเกินขอบเขตที่ตรวจสอบได้ กรุณาติดต่อผู้ดูแล".into(),
            });
        }
        if matches!(
            table,
            "learning_group_student_scores"
                | "learning_group_student_evaluations"
                | "academic_activity_evaluations"
        ) {
            scores += count;
        }
        digests.push(digest);
    }
    let (exam_items,exam_digest):(i64,String)=DbSession::new(conn).query_as("academic_assessment.deletion.exam_impact",r#"
 WITH items AS (SELECT * FROM academic_exam_schedule_items WHERE academic_year_id=$1 AND academic_term_id=$2 AND learning_offering_id=$3 AND($5 OR learning_group_id=ANY($4))),
 evidence AS (SELECT 'item' kind,to_jsonb(i) value FROM items i UNION ALL SELECT 'session',to_jsonb(s) FROM academic_exam_sessions s JOIN items i ON i.id=s.exam_schedule_item_id)
 SELECT (SELECT count(*) FROM items),encode(sha256(convert_to(COALESCE((SELECT jsonb_agg(value ORDER BY kind,value::text) FROM evidence),'[]')::text,'UTF8')),'hex')
 "#).bind(scope.academic_year_id).bind(scope.academic_term_id).bind(scope.learning_offering_id).bind(&scope.learning_group_ids).bind(scope.delete_offering).fetch_one().await?;
    digests.push(exam_digest);
    let plan_digest:String=DbSession::new(conn).scalar("academic_assessment.deletion.plan_impact",r#"SELECT encode(sha256(convert_to(COALESCE(jsonb_agg(jsonb_build_object('plan',to_jsonb(plan),'phases',(SELECT jsonb_agg(to_jsonb(phase) ORDER BY phase.id) FROM course_assessment_phases phase WHERE phase.plan_id=plan.id)) ORDER BY plan.id),'[]')::text,'UTF8')),'hex') FROM course_assessment_plans plan WHERE plan.learning_offering_id=$1"#).bind(scope.learning_offering_id).fetch_one().await?;
    digests.push(plan_digest);
    use sha2::{Digest, Sha256};
    Ok(AssessmentDeletionImpact {
        exam_items,
        scores,
        source_digest: hex::encode(Sha256::digest(digests.join(":"))),
    })
}

pub async fn delete_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    scope: &DeliveryDeletionScope,
) -> Result<(), AppError> {
    DbSession::new(&mut **tx).query("academic_assessment.deletion.exam_items","DELETE FROM academic_exam_schedule_items WHERE academic_year_id=$1 AND academic_term_id=$2 AND learning_offering_id=$3 AND($5 OR learning_group_id=ANY($4))").bind(scope.academic_year_id).bind(scope.academic_term_id).bind(scope.learning_offering_id).bind(&scope.learning_group_ids).bind(scope.delete_offering).execute().await?;
    for &table in GROUP_TABLES {
        let sql=format!("DELETE FROM {table} WHERE academic_year_id=$1 AND academic_term_id=$2 AND learning_group_id=ANY($3)");
        DbSession::new(&mut **tx)
            .query(
                "academic_assessment.deletion.group_children",
                sqlx::AssertSqlSafe(sql),
            )
            .bind(scope.academic_year_id)
            .bind(scope.academic_term_id)
            .bind(&scope.learning_group_ids)
            .execute()
            .await?;
    }
    if scope.delete_offering {
        DbSession::new(&mut **tx).query("academic_assessment.deletion.plans","DELETE FROM course_assessment_plans WHERE learning_offering_id=$1 AND academic_year_id=$2 AND academic_term_id=$3").bind(scope.learning_offering_id).bind(scope.academic_year_id).bind(scope.academic_term_id).execute().await?;
    }
    Ok(())
}
