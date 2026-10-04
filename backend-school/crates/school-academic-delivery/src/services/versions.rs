use std::collections::BTreeSet;

use chrono::NaiveDate;
use school_authorization::{
    academic_resource_access_for, AcademicResourceAccess, AcademicResourceListFilter,
};
use school_errors::AppError;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::models::versions::{
    DeliveryReadinessCode, DeliveryReadinessFinding, DeliverySnapshot, DeliveryVersion,
    DeliveryVersionStatus, DeliveryVersionSummary,
};
use crate::models::{LearningOfferingKind, LearningOfferingSnapshot, LearningTeacherRole};

#[derive(sqlx::FromRow)]
struct VersionRow {
    id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    source_version_id: Option<Uuid>,
    effective_from: NaiveDate,
    effective_until: Option<NaiveDate>,
    status: DeliveryVersionStatus,
    row_version: i64,
    created_by: Option<Uuid>,
    published_by: Option<Uuid>,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    snapshot: sqlx::types::Json<DeliverySnapshot>,
}

impl From<VersionRow> for DeliveryVersion {
    fn from(row: VersionRow) -> Self {
        Self {
            id: row.id,
            academic_term_id: row.academic_term_id,
            academic_year_id: row.academic_year_id,
            source_version_id: row.source_version_id,
            effective_from: row.effective_from,
            effective_until: row.effective_until,
            status: row.status,
            row_version: row.row_version,
            created_by: row.created_by,
            published_by: row.published_by,
            published_at: row.published_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            snapshot: row.snapshot.0,
        }
    }
}

const VERSION_SELECT: &str = "SELECT version.*, (
    SELECT min(next.effective_from)-1 FROM academic_delivery_versions next
    WHERE next.academic_term_id=version.academic_term_id AND next.status='published'
      AND next.effective_from>version.effective_from
    ) AS effective_until FROM academic_delivery_versions version";

pub async fn list_versions(
    pool: &PgPool,
    term_id: Uuid,
    access: &AcademicResourceListFilter,
) -> Result<Vec<DeliveryVersionSummary>, AppError> {
    Ok(sqlx::query_as(
        "WITH visible_versions AS (SELECT original.*,COALESCE((SELECT jsonb_agg(offering) FROM jsonb_array_elements(original.snapshot->'offerings') offering
        WHERE $2 OR (offering->>'owningOrganizationUnitId')::uuid=ANY($3) OR EXISTS(
            SELECT 1 FROM jsonb_array_elements(offering->'groups') source_group,jsonb_array_elements(source_group->'teachers') teacher
            WHERE (teacher->>'teacherId')::uuid=$4)),'[]'::jsonb) AS visible_offerings
        FROM academic_delivery_versions original WHERE original.academic_term_id=$1)
        SELECT version.id,version.academic_term_id,version.academic_year_id,version.source_version_id,
        (SELECT revision.id FROM academic_term_change_sets revision WHERE revision.target_delivery_version_id=version.id
            ORDER BY revision.created_at DESC,revision.id LIMIT 1) AS change_set_id,
        version.effective_from,(SELECT min(next.effective_from)-1 FROM academic_delivery_versions next
            WHERE next.academic_term_id=version.academic_term_id AND next.status='published'
                AND next.effective_from>version.effective_from) AS effective_until,
        version.status,version.row_version,version.updated_at,
        jsonb_array_length(version.visible_offerings)::bigint AS offering_count,
        (SELECT count(*) FROM jsonb_array_elements(version.visible_offerings) offering,
            jsonb_array_elements(offering->'groups') source_group) AS group_count,
        (SELECT count(*) FROM jsonb_array_elements(version.visible_offerings) offering,
            jsonb_array_elements(offering->'groups') source_group,
            jsonb_array_elements(source_group->'teachers') teacher) AS teacher_assignment_count
        FROM visible_versions version
        ORDER BY version.effective_from DESC,version.created_at DESC,version.id"
    )
        .bind(term_id)
        .bind(access.includes_school_owned)
        .bind(access.allowed_organization_unit_ids())
        .bind(access.assigned_actor_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get_version(pool: &PgPool, id: Uuid) -> Result<DeliveryVersion, AppError> {
    let query = format!("{VERSION_SELECT} WHERE version.id=$1");
    Ok(sqlx::query_as::<_, VersionRow>(sqlx::AssertSqlSafe(query))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นเปิดสอน".into()))?
        .into())
}

/// Bounded source graphs for timetable list hydration; the consumer does not
/// read or reconstruct delivery-owned snapshots with its own SQL.
pub async fn snapshots<'e>(
    executor: impl sqlx::Executor<'e, Database = Postgres>,
    ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, DeliverySnapshot>, AppError> {
    let rows: Vec<(Uuid, sqlx::types::Json<DeliverySnapshot>)> = sqlx::query_as(
        "SELECT id,snapshot FROM academic_delivery_versions WHERE id=ANY($1) AND status='published'",
    )
    .bind(ids)
    .fetch_all(executor)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, snapshot)| (id, snapshot.0))
        .collect())
}

pub async fn visible_version(
    pool: &PgPool,
    id: Uuid,
    access: &AcademicResourceListFilter,
) -> Result<DeliveryVersion, AppError> {
    let mut version = get_version(pool, id).await?;
    version.snapshot.offerings.retain(|offering| {
        let assigned = access.assigned_actor_id.is_some_and(|actor| {
            offering.groups.iter().any(|group| {
                group
                    .teachers
                    .iter()
                    .any(|teacher| teacher.teacher_id == actor)
            })
        });
        academic_resource_access_for(access, Some(offering.owning_organization_unit_id), assigned)
            != AcademicResourceAccess::None
    });
    Ok(version)
}

/// Timetable owns placements and consumes this provider-owned immutable source.
// Published graphs are immutable. Read-only preparation previews use this same
// provider; writers serialize the term before reading publication intervals.
pub async fn published_source(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    term_id: Uuid,
) -> Result<DeliveryVersion, AppError> {
    let query = format!("{VERSION_SELECT} WHERE version.id=$1");
    let version: DeliveryVersion = sqlx::query_as::<_, VersionRow>(sqlx::AssertSqlSafe(query))
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นเปิดสอน".into()))?
        .into();
    if version.academic_term_id != term_id || version.status != DeliveryVersionStatus::Published {
        return Err(AppError::ValidationError(
            "ตารางสอนต้องอ้างอิงรุ่นเปิดสอนที่เผยแพร่แล้วในภาคเรียนเดียวกัน".into(),
        ));
    }
    Ok(version)
}

pub async fn latest_published_id(
    tx: &mut Transaction<'_, Postgres>,
    term_id: Uuid,
) -> Result<Uuid, AppError> {
    sqlx::query_scalar("SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC, published_at DESC, id DESC LIMIT 1")
        .bind(term_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::ValidationError("กรุณาเผยแพร่รุ่นเปิดสอนก่อนแก้ตารางสอน".into()))
}

pub async fn latest_published_source(
    tx: &mut Transaction<'_, Postgres>,
    term_id: Uuid,
) -> Result<Option<DeliveryVersion>, AppError> {
    let id: Option<Uuid>=sqlx::query_scalar("SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,published_at DESC,id DESC LIMIT 1")
        .bind(term_id).fetch_optional(&mut **tx).await?;
    match id {
        Some(id) => Ok(Some(published_source(tx, id, term_id).await?)),
        None => Ok(None),
    }
}

pub async fn published_source_evidence(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    term_id: Uuid,
) -> Result<String, AppError> {
    super::stable_hash(&published_source(tx, id, term_id).await?)
}

pub async fn latest_published_for_term(
    pool: &PgPool,
    term_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    Ok(sqlx::query_scalar("SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,published_at DESC,id DESC LIMIT 1")
        .bind(term_id).fetch_optional(pool).await?)
}

pub(super) async fn refresh_revision_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
) -> Result<(), AppError> {
    let updated = sqlx::query("UPDATE academic_delivery_versions version SET snapshot=academic_delivery_revision_snapshot($1),
        row_version=version.row_version+1,updated_at=now()
        FROM academic_term_change_sets revision WHERE revision.id=$1 AND revision.target_delivery_version_id=version.id
            AND revision.status='draft' AND version.status='draft'")
        .bind(change_set_id).execute(&mut **tx).await?;
    if updated.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "รุ่นเปิดสอนแบบร่างเปลี่ยนไป กรุณาโหลดข้อมูลใหม่".into(),
        ));
    }
    Ok(())
}

/// Resource edits refresh only opening drafts which already contain the resource.
/// Published snapshots and all timetable placements remain immutable here.
pub(super) async fn refresh_drafts_for_offering(
    tx: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
) -> Result<(), AppError> {
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT revision.id FROM academic_term_change_sets revision
        JOIN academic_delivery_versions version ON version.id=revision.target_delivery_version_id
        WHERE revision.status='draft' AND version.status='draft' AND EXISTS(
            SELECT 1 FROM jsonb_array_elements(version.snapshot->'offerings') offering WHERE (offering->>'id')::uuid=$1)
        ORDER BY revision.id FOR UPDATE OF revision,version")
        .bind(offering_id).fetch_all(&mut **tx).await?;
    for id in ids {
        refresh_revision_snapshot(tx, id).await?;
    }
    Ok(())
}

/// Include an explicitly selected offering in an opening draft, never in a
/// timetable. The stable registry ID is also recorded in the delivery journal.
pub(super) async fn include_offering(
    tx: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
    offering_id: Uuid,
    actor_id: Uuid,
) -> Result<(), AppError> {
    let (revision_id, term_id, year_id, date, already_included): (Uuid, Uuid, Uuid, NaiveDate, bool) = sqlx::query_as(
        "SELECT revision.id,version.academic_term_id,version.academic_year_id,version.effective_from,
        EXISTS(SELECT 1 FROM jsonb_array_elements(version.snapshot->'offerings') offering WHERE (offering->>'id')::uuid=$2)
        FROM academic_delivery_versions version JOIN academic_term_change_sets revision ON revision.target_delivery_version_id=version.id
        WHERE version.id=$1 AND version.status='draft' AND revision.status='draft' FOR UPDATE OF version,revision",
    ).bind(version_id).bind(offering_id).fetch_optional(&mut **tx).await?
        .ok_or_else(|| AppError::Conflict("เพิ่มรายการได้เฉพาะรุ่นเปิดสอนแบบร่าง".into()))?;
    if already_included {
        refresh_revision_snapshot(tx, revision_id).await?;
        return Ok(());
    }
    let target: i32 = sqlx::query_scalar("SELECT COALESCE(subject.periods_per_week,activity.periods_per_week)
        FROM learning_offerings offering LEFT JOIN course_offering_details course ON course.learning_offering_id=offering.id
        LEFT JOIN subject_versions subject ON subject.id=course.subject_version_id
        LEFT JOIN activity_offering_details detail ON detail.learning_offering_id=offering.id
        LEFT JOIN activity_versions activity ON activity.id=detail.activity_version_id
        WHERE offering.id=$1 AND offering.academic_term_id=$2 AND offering.academic_year_id=$3 AND offering.status IN('draft','published')
        AND (offering.starts_on IS NULL OR offering.starts_on<=$4) AND (offering.ends_on IS NULL OR offering.ends_on>=$4)")
        .bind(offering_id).bind(term_id).bind(year_id).bind(date).fetch_optional(&mut **tx).await?
        .ok_or_else(|| AppError::ValidationError("รายการเปิดสอนไม่ตรงกับรุ่นหรือวันที่ที่เลือก".into()))?;
    if target <= 0 {
        return Err(AppError::ValidationError("กรุณาระบุจำนวนคาบมากกว่าศูนย์".into()));
    }
    sqlx::query("INSERT INTO academic_term_change_items(id,change_set_id,academic_term_id,academic_year_id,action_kind,learning_offering_id,weekly_period_target,created_by)
        VALUES(gen_random_uuid(),$1,$2,$3,'add_offering',$4,$5,$6)")
        .bind(revision_id).bind(term_id).bind(year_id).bind(offering_id).bind(target).bind(actor_id).execute(&mut **tx).await?;
    sqlx::query("UPDATE academic_term_change_sets SET row_version=row_version+1,updated_at=now() WHERE id=$1")
        .bind(revision_id).execute(&mut **tx).await?;
    refresh_revision_snapshot(tx, revision_id).await
}

/// Roster publication has its own lifecycle; never infer it from opening status.
pub async fn roster_statuses(
    pool: &PgPool,
    group_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, crate::models::RosterStatus>, AppError> {
    Ok(
        sqlx::query_as("SELECT id,roster_status FROM learning_groups WHERE id=ANY($1)")
            .bind(group_ids)
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect(),
    )
}

pub fn contains_date(version: &DeliveryVersion, date: NaiveDate) -> bool {
    version.status == DeliveryVersionStatus::Published
        && date >= version.effective_from
        && version.effective_until.is_none_or(|last| date <= last)
}

/// This check deliberately has no timetable input: delivery publishes before scheduling.
pub fn readiness(snapshot: &DeliverySnapshot) -> Vec<DeliveryReadinessFinding> {
    let mut findings = Vec::new();
    let mut offering_ids = BTreeSet::new();
    let mut group_ids = BTreeSet::new();
    for offering in &snapshot.offerings {
        let mut record = |code, learning_group_id| {
            findings.push(DeliveryReadinessFinding {
                code,
                learning_offering_id: offering.id,
                learning_group_id,
            });
        };
        if !offering_ids.insert(offering.id) {
            record(DeliveryReadinessCode::DuplicateOffering, None);
        }
        if offering.weekly_period_target <= 0 {
            record(DeliveryReadinessCode::InvalidWeeklyTarget, None);
        }
        if !matches!(
            (offering.kind, &offering.catalog),
            (
                LearningOfferingKind::Course,
                LearningOfferingSnapshot::Course(_)
            ) | (
                LearningOfferingKind::Activity,
                LearningOfferingSnapshot::Activity(_)
            )
        ) {
            record(DeliveryReadinessCode::CatalogKindMismatch, None);
        }
        if offering.targets.is_empty() {
            record(DeliveryReadinessCode::MissingTargets, None);
        }
        if offering.groups.is_empty() {
            record(DeliveryReadinessCode::MissingGroups, None);
        }
        for group in &offering.groups {
            if !group_ids.insert(group.id) {
                record(DeliveryReadinessCode::DuplicateGroup, Some(group.id));
            }
            let mut teacher_ids = BTreeSet::new();
            if !group
                .teachers
                .iter()
                .any(|teacher| teacher.role == LearningTeacherRole::Primary)
            {
                record(DeliveryReadinessCode::MissingPrimaryTeacher, Some(group.id));
            }
            for teacher in &group.teachers {
                if !teacher_ids.insert(teacher.teacher_id) {
                    record(DeliveryReadinessCode::DuplicateTeacher, Some(group.id));
                }
            }
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::versions::{
        DeliveryVersionGroup, DeliveryVersionOffering, DeliveryVersionTeacher,
    };
    use crate::models::{CourseOfferingSnapshot, LearningOfferingTarget, OfferingTargetKind};

    fn ready_snapshot() -> DeliverySnapshot {
        DeliverySnapshot {
            offerings: vec![DeliveryVersionOffering {
                id: Uuid::from_u128(1),
                kind: LearningOfferingKind::Course,
                code: "ท101".into(),
                name: "ภาษาไทย".into(),
                owning_organization_unit_id: Uuid::from_u128(2),
                source_requirement_kind: None,
                source_requirement_id: None,
                weekly_period_target: 3,
                homeroom_ids: vec![Uuid::from_u128(6)],
                catalog: LearningOfferingSnapshot::Course(CourseOfferingSnapshot {
                    subject_version_id: Uuid::from_u128(3),
                    subject_id: Uuid::from_u128(4),
                    curriculum_course_requirement_id: None,
                    credit: "1.5".into(),
                    hours: None,
                    standard_periods_per_week: 3,
                    assessment_total_score: "100".into(),
                }),
                targets: vec![LearningOfferingTarget {
                    id: Uuid::from_u128(5),
                    target_kind: OfferingTargetKind::Homeroom,
                    homeroom_id: Some(Uuid::from_u128(6)),
                    grade_level_id: Uuid::from_u128(7),
                    study_program_id: Uuid::from_u128(8),
                }],
                groups: vec![DeliveryVersionGroup {
                    id: Uuid::from_u128(9),
                    code: "1".into(),
                    name: "ม.1/1".into(),
                    description: None,
                    capacity: None,
                    homeroom_ids: vec![Uuid::from_u128(6)],
                    preferred_room_ids: vec![],
                    teachers: vec![DeliveryVersionTeacher {
                        display_name: "ครูตัวอย่าง".into(),
                        assignment_id: Uuid::from_u128(10),
                        teacher_id: Uuid::from_u128(11),
                        role: LearningTeacherRole::Primary,
                    }],
                }],
            }],
        }
    }

    #[test]
    fn delivery_readiness_does_not_require_placed_lessons() {
        assert!(readiness(&ready_snapshot()).is_empty());
    }

    #[test]
    fn invalid_graph_reports_resource_ids_without_guessing_replacements() {
        let mut snapshot = ready_snapshot();
        snapshot.offerings[0].weekly_period_target = 0;
        snapshot.offerings[0].groups[0].teachers.clear();
        let findings = readiness(&snapshot);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].code, DeliveryReadinessCode::InvalidWeeklyTarget);
        assert_eq!(
            findings[1].code,
            DeliveryReadinessCode::MissingPrimaryTeacher
        );
        assert_eq!(findings[1].learning_group_id, Some(Uuid::from_u128(9)));
    }

    #[test]
    fn snapshot_round_trip_preserves_catalog_group_and_teacher_identity() {
        let snapshot = ready_snapshot();
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        let restored: DeliverySnapshot = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            super::super::stable_hash(&snapshot).unwrap(),
            super::super::stable_hash(&restored).unwrap()
        );
        assert_eq!(
            restored.offerings[0].groups[0].teachers[0].assignment_id,
            Uuid::from_u128(10)
        );
        assert!(String::from_utf8(bytes).unwrap().contains("ภาษาไทย"));
    }

    #[test]
    fn duplicated_group_identity_is_rejected_across_offerings() {
        let mut snapshot = ready_snapshot();
        let mut other = snapshot.offerings[0].clone();
        other.id = Uuid::from_u128(12);
        snapshot.offerings.push(other);
        assert_eq!(
            readiness(&snapshot)[0].code,
            DeliveryReadinessCode::DuplicateGroup
        );
    }
}

pub const DELIVERY_TIMETABLE_CUTOVER_VERSION: i64 = 88;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryCutoverCheck {
    pub code: String,
    pub count: i64,
    pub passed: bool,
}

#[derive(Debug)]
pub struct DeliveryCutoverAudit {
    pub completed: bool,
    pub checks: Vec<DeliveryCutoverCheck>,
}

#[derive(sqlx::FromRow)]
struct DeliveryAuditRow {
    migration_version: i32,
    passed: bool,
    cutover_completed: bool,
    checks: sqlx::types::Json<Vec<DeliveryCutoverCheck>>,
}

/// Deployment reads bounded aggregate evidence through the delivery owner.
/// Missing or incomplete reconciliation keeps the coordinated release closed.
pub async fn read_cutover_audit(
    pool: &sqlx::PgPool,
) -> Result<DeliveryCutoverAudit, school_errors::AppError> {
    let rows: Vec<DeliveryAuditRow> = sqlx::query_as("SELECT migration_version,passed,cutover_completed,checks FROM academic_delivery_version_migration_audit WHERE migration_version IN (87,88) ORDER BY migration_version")
        .fetch_all(pool).await?;
    let completed = complete_cutover_evidence(&rows);
    Ok(DeliveryCutoverAudit {
        completed,
        checks: rows.into_iter().flat_map(|row| row.checks.0).collect(),
    })
}

fn complete_cutover_evidence(rows: &[DeliveryAuditRow]) -> bool {
    rows.len() == 2
        && rows.iter().map(|row| row.migration_version).eq([87, 88])
        && rows.iter().all(|row| {
            row.passed
                && row.cutover_completed
                && row.checks.len() == 15
                && row
                    .checks
                    .iter()
                    .all(|check| check.passed && check.count >= 0)
                && row
                    .checks
                    .iter()
                    .map(|check| &check.code)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    == 15
        })
}

#[cfg(test)]
mod cutover_tests {
    use super::*;
    fn evidence(version: i32) -> DeliveryAuditRow {
        DeliveryAuditRow {
            migration_version: version,
            passed: true,
            cutover_completed: true,
            checks: sqlx::types::Json(
                (0..15)
                    .map(|index| DeliveryCutoverCheck {
                        code: format!("resource-{index}"),
                        count: 1,
                        passed: true,
                    })
                    .collect(),
            ),
        }
    }
    #[test]
    fn both_complete_distinct_migration_audits_are_required() {
        assert!(complete_cutover_evidence(&[evidence(87), evidence(88)]));
        assert!(!complete_cutover_evidence(&[evidence(88)]));
        let mut rows = [evidence(87), evidence(88)];
        rows[0].cutover_completed = false;
        assert!(!complete_cutover_evidence(&rows));
        rows[0].cutover_completed = true;
        rows[1].checks.0[0].passed = false;
        assert!(!complete_cutover_evidence(&rows));
        rows[1].checks.0[0].passed = true;
        rows[1].checks.0.pop();
        assert!(!complete_cutover_evidence(&rows));
    }
}
