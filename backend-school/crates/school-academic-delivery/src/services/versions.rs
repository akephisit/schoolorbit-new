use school_database::DbSession;
use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use school_authorization::{
    academic_resource_access_for, AcademicResourceAccess, AcademicResourceListFilter,
};
use school_errors::AppError;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::models::versions::{
    DeliveryReadinessCode, DeliveryReadinessFinding, DeliverySnapshot, DeliveryTeacherAllocation,
    DeliveryVersion, DeliveryVersionCourseMetadata, DeliveryVersionDetails, DeliveryVersionStatus,
    DeliveryVersionSummary, GroupTeacherAssignments, PublishedTeacherAssignments,
    PublishedTeacherGroupIds,
};
use crate::models::{LearningOfferingKind, LearningOfferingSnapshot, LearningTeacherRole};

#[derive(sqlx::FromRow)]
struct VersionRow {
    publication_no: Option<i64>,
    id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    source_version_id: Option<Uuid>,
    effective_from: Option<NaiveDate>,
    reference_date: NaiveDate,
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
            publication_no: row.publication_no,
            is_superseded: row
                .effective_from
                .zip(row.effective_until)
                .is_some_and(|(start, end)| end < start),
            id: row.id,
            academic_term_id: row.academic_term_id,
            academic_year_id: row.academic_year_id,
            source_version_id: row.source_version_id,
            effective_from: row.effective_from,
            reference_date: row.reference_date,
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

const VERSION_SELECT: &str = "SELECT version.publication_no,version.id,version.academic_term_id,version.academic_year_id,version.source_version_id,version.effective_from,version.status,version.row_version,version.created_by,version.published_by,version.published_at,version.created_at,version.updated_at,version.snapshot, COALESCE(version.reference_date,version.effective_from) AS reference_date, (
    SELECT min(next.effective_from)-1 FROM academic_delivery_versions next
    WHERE next.academic_term_id=version.academic_term_id AND next.status='published'
      AND next.publication_no>version.publication_no
    ) AS effective_until FROM academic_delivery_versions version";

/// Reads only the latest published graph in each requested offering's exact context.
/// Missing versions or omitted offerings return no allocations, without a date fallback.
pub async fn teacher_allocations_on_connection(
    connection: &mut sqlx::PgConnection,
    offering_ids: &[Uuid],
) -> Result<Vec<DeliveryTeacherAllocation>, AppError> {
    if offering_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(DbSession::new(connection)
        .query_as(
            "academic_delivery.services.versions.teacher_allocations_on_connection",
            r#"SELECT delivery_version_id,academic_year_id,academic_term_id,
                      learning_offering_id,learning_group_id,assignment_id,teacher_id,role
               FROM academic_delivery_latest_published_teacher_allocations
               WHERE learning_offering_id=ANY($1)
               ORDER BY learning_offering_id,learning_group_id,teacher_id,assignment_id"#,
        )
        .bind(offering_ids)
        .fetch_all()
        .await?)
}

#[derive(sqlx::FromRow)]
struct TeacherMembershipRow {
    delivery_version_id: Uuid,
    learning_group_id: Option<Uuid>,
    teacher_id: Option<Uuid>,
    role: Option<LearningTeacherRole>,
}

enum TeacherMembershipFilter<'a> {
    Groups(&'a [Uuid]),
    Teacher(Uuid),
}

async fn teacher_memberships_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    filter: TeacherMembershipFilter<'_>,
) -> Result<Vec<TeacherMembershipRow>, AppError> {
    let (group_ids, teacher_id) = match filter {
        TeacherMembershipFilter::Groups(ids) => (Some(ids), None),
        TeacherMembershipFilter::Teacher(id) => (None, Some(id)),
    };
    let rows: Vec<TeacherMembershipRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.versions.teacher_memberships_on_connection",
            r#"WITH latest AS (
                   SELECT id FROM academic_delivery_versions
                   WHERE academic_year_id=$1 AND academic_term_id=$2 AND status='published'
                   ORDER BY publication_no DESC LIMIT 1
               )
               SELECT latest.id AS delivery_version_id,
                      membership.learning_group_id,membership.teacher_id,membership.role
               FROM latest
               LEFT JOIN LATERAL (
                   SELECT DISTINCT allocation.learning_group_id,allocation.teacher_id,allocation.role
                   FROM academic_delivery_latest_published_teacher_allocations allocation
                   JOIN users account ON account.id=allocation.teacher_id AND account.status='active'
                   WHERE allocation.delivery_version_id=latest.id
                     AND allocation.academic_year_id=$1 AND allocation.academic_term_id=$2
                     AND ($3::uuid[] IS NULL OR allocation.learning_group_id=ANY($3))
                     AND ($4::uuid IS NULL OR allocation.teacher_id=$4)
               ) membership ON true
               ORDER BY membership.learning_group_id,membership.teacher_id,membership.role"#,
        )
        .bind(academic_year_id)
        .bind(academic_term_id)
        .bind(group_ids)
        .bind(teacher_id)
        .fetch_all()
        .await?;
    if rows.iter().any(|row| {
        !matches!(
            (row.learning_group_id, row.teacher_id, row.role),
            (None, None, None) | (Some(_), Some(_), Some(_))
        )
    }) {
        return Err(AppError::InternalServerError(
            "Published teacher allocation is incomplete".into(),
        ));
    }
    Ok(rows)
}

/// Resolves source availability even for an empty group request. Missing group
/// records mean no active assignment in that exact context's latest published graph.
pub async fn group_teacher_assignments_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    group_ids: &[Uuid],
) -> Result<PublishedTeacherAssignments, AppError> {
    let group_ids = group_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let rows = teacher_memberships_on_connection(
        connection,
        academic_year_id,
        academic_term_id,
        TeacherMembershipFilter::Groups(&group_ids),
    )
    .await?;
    let delivery_version_id = rows.first().map(|row| row.delivery_version_id);
    let mut groups: BTreeMap<Uuid, (BTreeSet<Uuid>, BTreeSet<Uuid>)> = BTreeMap::new();
    for row in rows {
        match (row.learning_group_id, row.teacher_id, row.role) {
            (None, None, None) => {}
            (Some(group), Some(teacher), Some(role)) => {
                let (teachers, primaries) = groups.entry(group).or_default();
                teachers.insert(teacher);
                if role == LearningTeacherRole::Primary {
                    primaries.insert(teacher);
                }
            }
            _ => {
                return Err(AppError::InternalServerError(
                    "Published teacher allocation is incomplete".into(),
                ))
            }
        }
    }
    Ok(PublishedTeacherAssignments {
        delivery_version_id,
        groups: groups
            .into_iter()
            .map(
                |(learning_group_id, (teachers, primaries))| GroupTeacherAssignments {
                    learning_group_id,
                    teacher_ids: teachers.into_iter().collect(),
                    primary_teacher_ids: primaries.into_iter().collect(),
                },
            )
            .collect(),
    })
}

/// Assigned discovery is scoped to one actor and exact year/term before consumers
/// apply their resource filters and limits. No historical episode fallback is used.
pub async fn assigned_group_ids_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    teacher_id: Uuid,
) -> Result<PublishedTeacherGroupIds, AppError> {
    let rows = teacher_memberships_on_connection(
        connection,
        academic_year_id,
        academic_term_id,
        TeacherMembershipFilter::Teacher(teacher_id),
    )
    .await?;
    Ok(PublishedTeacherGroupIds {
        delivery_version_id: rows.first().map(|row| row.delivery_version_id),
        learning_group_ids: rows
            .into_iter()
            .filter_map(|row| row.learning_group_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    })
}

pub async fn list_versions(
    pool: &PgPool,
    term_id: Uuid,
    access: &AcademicResourceListFilter,
) -> Result<Vec<DeliveryVersionSummary>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.versions.list_versions",
    )
    .await?;
    list_versions_on_connection(&mut connection, term_id, access).await
}

pub async fn list_versions_on_connection(
    connection: &mut sqlx::PgConnection,
    term_id: Uuid,
    access: &AcademicResourceListFilter,
) -> Result<Vec<DeliveryVersionSummary>, AppError> {
    Ok(DbSession::new(connection).query_as("academic_delivery.services.versions.list_versions_on_connection",
        "WITH visible_versions AS (SELECT original.*,COALESCE((SELECT jsonb_agg(offering) FROM jsonb_array_elements(original.snapshot->'offerings') offering
        WHERE $2 OR (offering->>'owningOrganizationUnitId')::uuid=ANY($3) OR EXISTS(
            SELECT 1 FROM jsonb_array_elements(offering->'groups') source_group,jsonb_array_elements(source_group->'teachers') teacher
            WHERE (teacher->>'teacherId')::uuid=$4)),'[]'::jsonb) AS visible_offerings
        FROM academic_delivery_versions original WHERE original.academic_term_id=$1)
        SELECT version.id,version.academic_term_id,version.academic_year_id,version.source_version_id,
        (SELECT revision.id FROM academic_term_change_sets revision WHERE revision.target_delivery_version_id=version.id
            ORDER BY revision.created_at DESC,revision.id LIMIT 1) AS change_set_id,
        version.effective_from,COALESCE(version.reference_date,version.effective_from) AS reference_date,(SELECT min(next.effective_from)-1 FROM academic_delivery_versions next
            WHERE next.academic_term_id=version.academic_term_id AND next.status='published'
                AND next.publication_no>version.publication_no) AS effective_until,
        version.publication_no,version.published_at,version.published_by,
        COALESCE((SELECT min(next.effective_from) FROM academic_delivery_versions next
            WHERE next.academic_term_id=version.academic_term_id AND next.status='published'
              AND next.publication_no>version.publication_no)<=version.effective_from,false) AS is_superseded,
        version.status,version.row_version,version.updated_at,
        jsonb_array_length(version.visible_offerings)::bigint AS offering_count,
        (SELECT count(*) FROM jsonb_array_elements(version.visible_offerings) offering,
            jsonb_array_elements(offering->'groups') source_group) AS group_count,
        (SELECT count(*) FROM jsonb_array_elements(version.visible_offerings) offering,
            jsonb_array_elements(offering->'groups') source_group,
            jsonb_array_elements(source_group->'teachers') teacher) AS teacher_assignment_count
        FROM visible_versions version
        ORDER BY version.publication_no DESC NULLS FIRST,version.created_at DESC,version.id"
    )
        .bind(term_id)
        .bind(access.includes_school_owned)
        .bind(access.allowed_organization_unit_ids())
        .bind(access.assigned_actor_id)
        .fetch_all()
        .await?)
}

pub async fn get_version(pool: &PgPool, id: Uuid) -> Result<DeliveryVersion, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.versions.get_version",
    )
    .await?;
    get_version_on_connection(&mut connection, id).await
}

pub async fn get_version_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<DeliveryVersion, AppError> {
    let query = format!("{VERSION_SELECT} WHERE version.id=$1");
    Ok(DbSession::new(connection)
        .query_as::<VersionRow>(
            "academic_delivery.services.versions.get_version_on_connection",
            sqlx::AssertSqlSafe(query),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นเปิดสอน".into()))?
        .into())
}

/// Bounded source graphs for timetable list hydration; the consumer does not
/// read or reconstruct delivery-owned snapshots with its own SQL.
pub async fn snapshots(
    pool: &PgPool,
    ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, DeliverySnapshot>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.versions.snapshots",
    )
    .await?;
    snapshots_on_connection(&mut connection, ids).await
}
pub async fn snapshots_on_connection(
    connection: &mut sqlx::PgConnection,
    ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, DeliverySnapshot>, AppError> {
    let rows: Vec<(Uuid, sqlx::types::Json<DeliverySnapshot>)> = DbSession::new(connection).query_as("academic_delivery.services.versions.snapshots_on_connection",
        "SELECT id,snapshot FROM academic_delivery_versions WHERE id=ANY($1) AND status='published'",
    )
    .bind(ids)
    .fetch_all()
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
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.versions.visible_version",
    )
    .await?;
    visible_version_on_connection(&mut connection, id, access).await
}

pub async fn visible_version_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
    access: &AcademicResourceListFilter,
) -> Result<DeliveryVersion, AppError> {
    let mut version = get_version_on_connection(connection, id).await?;
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

pub async fn visible_version_details(
    pool: &PgPool,
    id: Uuid,
    access: &AcademicResourceListFilter,
) -> Result<DeliveryVersionDetails, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.versions.visible_version_details",
    )
    .await?;
    visible_version_details_on_connection(&mut connection, id, access).await
}

pub async fn visible_version_details_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
    access: &AcademicResourceListFilter,
) -> Result<DeliveryVersionDetails, AppError> {
    let version = visible_version_on_connection(connection, id, access).await?;
    let subject_version_ids = version
        .snapshot
        .offerings
        .iter()
        .filter_map(|offering| match &offering.catalog {
            LearningOfferingSnapshot::Course(course) => Some(course.subject_version_id),
            LearningOfferingSnapshot::Activity(_) => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let course_metadata =
        school_academic_core::services::catalog::subject_groups_for_versions_on_connection(
            connection,
            &subject_version_ids,
        )
        .await?
        .into_iter()
        .map(|metadata| DeliveryVersionCourseMetadata {
            subject_version_id: metadata.subject_version_id,
            subject_group_id: metadata.subject_group_id,
            subject_group_name: metadata.name,
        })
        .collect();
    Ok(DeliveryVersionDetails {
        version,
        course_metadata,
    })
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
    let version: DeliveryVersion = DbSession::new(&mut **tx)
        .query_as::<VersionRow>(
            "academic_delivery.services.versions.published_source",
            sqlx::AssertSqlSafe(query),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นเปิดสอน".into()))?
        .into();
    if version.academic_term_id != term_id
        || version.status != DeliveryVersionStatus::Published
        || version.effective_from.is_none()
    {
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
    DbSession::new(&mut **tx).scalar("academic_delivery.services.versions.latest_published_id", "SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY publication_no DESC LIMIT 1")
        .bind(term_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::ValidationError("กรุณาเผยแพร่รุ่นเปิดสอนก่อนแก้ตารางสอน".into()))
}

pub async fn latest_published_source(
    tx: &mut Transaction<'_, Postgres>,
    term_id: Uuid,
) -> Result<Option<DeliveryVersion>, AppError> {
    let id: Option<Uuid>=DbSession::new(&mut **tx).scalar("academic_delivery.services.versions.latest_published_source", "SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY publication_no DESC LIMIT 1")
        .bind(term_id).fetch_optional().await?;
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
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.versions.latest_published_for_term",
    )
    .await?;
    latest_published_for_term_on_connection(&mut connection, term_id).await
}

pub async fn latest_published_for_term_on_connection(
    connection: &mut sqlx::PgConnection,
    term_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    Ok(DbSession::new(connection).scalar("academic_delivery.services.versions.latest_published_for_term_on_connection", "SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY publication_no DESC LIMIT 1")
        .bind(term_id).fetch_optional().await?)
}

pub(super) async fn refresh_revision_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
) -> Result<(), AppError> {
    let updated = DbSession::new(&mut **tx).query("academic_delivery.services.versions.refresh_revision_snapshot", "UPDATE academic_delivery_versions version SET snapshot=academic_delivery_revision_snapshot($1),
        row_version=version.row_version+1,updated_at=now()
        FROM academic_term_change_sets revision WHERE revision.id=$1 AND revision.target_delivery_version_id=version.id
            AND revision.status='draft' AND version.status='draft'")
        .bind(change_set_id).execute().await?;
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
    let ids: Vec<Uuid> = DbSession::new(&mut **tx).scalar("academic_delivery.services.versions.refresh_drafts_for_offering", "SELECT revision.id FROM academic_term_change_sets revision
        JOIN academic_delivery_versions version ON version.id=revision.target_delivery_version_id
        WHERE revision.status='draft' AND version.status='draft' AND EXISTS(
            SELECT 1 FROM jsonb_array_elements(version.snapshot->'offerings') offering WHERE (offering->>'id')::uuid=$1)
        ORDER BY revision.id FOR UPDATE OF revision,version")
        .bind(offering_id).fetch_all().await?;
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
    let (revision_id, term_id, year_id, date, already_included): (Uuid, Uuid, Uuid, NaiveDate, bool) = DbSession::new(&mut **tx).query_as("academic_delivery.services.versions.include_offering",
        "SELECT revision.id,version.academic_term_id,version.academic_year_id,COALESCE(version.effective_from,version.reference_date),
        EXISTS(SELECT 1 FROM jsonb_array_elements(version.snapshot->'offerings') offering WHERE (offering->>'id')::uuid=$2)
        FROM academic_delivery_versions version JOIN academic_term_change_sets revision ON revision.target_delivery_version_id=version.id
        WHERE version.id=$1 AND version.status='draft' AND revision.status='draft' FOR UPDATE OF version,revision",
    ).bind(version_id).bind(offering_id).fetch_optional().await?
        .ok_or_else(|| AppError::Conflict("เพิ่มรายการได้เฉพาะรุ่นเปิดสอนแบบร่าง".into()))?;
    // Explicit inclusion is the only path that clears this version's offering exclusion.
    DbSession::new(&mut **tx).query("academic_delivery.services.versions.include_offering",
        "UPDATE academic_term_change_sets SET excluded_offering_ids=array_remove(excluded_offering_ids,$2)
         WHERE id=$1 AND $2=ANY(excluded_offering_ids)")
        .bind(revision_id).bind(offering_id).execute().await?;
    if already_included {
        refresh_revision_snapshot(tx, revision_id).await?;
        return Ok(());
    }
    // An explicit plan inclusion may reactivate a registry offering after a gap.
    // Its previous plan's absence is not an eligibility condition for this write.
    let target: i32 = DbSession::new(&mut **tx).scalar("academic_delivery.services.versions.include_offering", "SELECT COALESCE(subject.periods_per_week,activity.periods_per_week)
        FROM learning_offerings offering LEFT JOIN course_offering_details course ON course.learning_offering_id=offering.id
        LEFT JOIN subject_versions subject ON subject.id=course.subject_version_id
        LEFT JOIN activity_offering_details detail ON detail.learning_offering_id=offering.id
        LEFT JOIN activity_versions activity ON activity.id=detail.activity_version_id
        WHERE offering.id=$1 AND offering.academic_term_id=$2 AND offering.academic_year_id=$3 AND offering.status IN('draft','published')
        AND (offering.status='draft' OR COALESCE(subject.effective_from,activity.effective_from)<=$4)")
        .bind(offering_id).bind(term_id).bind(year_id).bind(date).fetch_optional().await?
        .ok_or_else(|| AppError::ValidationError("รายการเปิดสอนไม่ตรงกับรุ่นหรือวันที่ที่เลือก".into()))?;
    if !valid_weekly_period_target(target) {
        return Err(AppError::ValidationError(
            "จำนวนคาบที่จัดจริงต่อสัปดาห์ต้องเป็นศูนย์หรือมากกว่า".into(),
        ));
    }
    DbSession::new(&mut **tx).query("academic_delivery.services.versions.include_offering", "INSERT INTO academic_term_change_items(id,change_set_id,academic_term_id,academic_year_id,action_kind,learning_offering_id,weekly_period_target,created_by)
        SELECT gen_random_uuid(),$1,$2,$3,'add_offering',$4,$5,$6
        WHERE NOT EXISTS(SELECT 1 FROM academic_term_change_sets revision
            JOIN academic_delivery_versions base ON base.id=revision.base_delivery_version_id,
            jsonb_array_elements(base.snapshot->'offerings') entry
            WHERE revision.id=$1 AND (entry->>'id')::uuid=$4)
        AND NOT EXISTS(SELECT 1 FROM academic_term_change_items WHERE change_set_id=$1
            AND learning_offering_id=$4 AND action_kind='add_offering')")
        .bind(revision_id).bind(term_id).bind(year_id).bind(offering_id).bind(target).bind(actor_id).execute().await?;
    DbSession::new(&mut **tx).query("academic_delivery.services.versions.include_offering", "UPDATE academic_term_change_sets SET row_version=row_version+1,updated_at=now() WHERE id=$1")
        .bind(revision_id).execute().await?;
    refresh_revision_snapshot(tx, revision_id).await
}

/// Roster publication has its own lifecycle; never infer it from opening status.
pub async fn roster_statuses(
    pool: &PgPool,
    group_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, crate::models::RosterStatus>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.versions.roster_statuses",
    )
    .await?;
    roster_statuses_on_connection(&mut connection, group_ids).await
}

pub async fn roster_statuses_on_connection(
    connection: &mut sqlx::PgConnection,
    group_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, crate::models::RosterStatus>, AppError> {
    Ok(DbSession::new(connection)
        .query_as(
            "academic_delivery.services.versions.roster_statuses_on_connection",
            "SELECT id,roster_status FROM learning_groups WHERE id=ANY($1)",
        )
        .bind(group_ids)
        .fetch_all()
        .await?
        .into_iter()
        .collect())
}

pub fn contains_date(version: &DeliveryVersion, date: NaiveDate) -> bool {
    version.status == DeliveryVersionStatus::Published
        && version.effective_from.is_some_and(|start| date >= start)
        && version.effective_until.is_none_or(|last| date <= last)
}

pub(crate) fn valid_weekly_period_target(value: i32) -> bool {
    value >= 0
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
        if !valid_weekly_period_target(offering.weekly_period_target) {
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
        let central = matches!(&offering.catalog, LearningOfferingSnapshot::Activity(activity) if activity.scheduling_mode == crate::models::ActivitySchedulingMode::Synchronized);
        if offering.groups.is_empty() && !central {
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
                curriculum_sources: Vec::new(),
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
    fn zero_course_target_keeps_curriculum_and_delivery_requirements() {
        let mut snapshot = ready_snapshot();
        snapshot.offerings[0].weekly_period_target = 0;
        assert!(readiness(&snapshot).is_empty());
        let LearningOfferingSnapshot::Course(catalog) = &snapshot.offerings[0].catalog else {
            panic!("course catalog preserved");
        };
        assert_eq!(catalog.standard_periods_per_week, 3);
        assert_eq!(catalog.credit, "1.5");
        snapshot.offerings[0].groups[0].teachers.clear();
        assert_eq!(
            readiness(&snapshot)[0].code,
            DeliveryReadinessCode::MissingPrimaryTeacher
        );
    }

    #[test]
    fn invalid_graph_reports_resource_ids_without_guessing_replacements() {
        let mut snapshot = ready_snapshot();
        snapshot.offerings[0].weekly_period_target = -1;
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

    fn central_snapshot() -> DeliverySnapshot {
        let mut snapshot = ready_snapshot();
        let offering = &mut snapshot.offerings[0];
        offering.kind = LearningOfferingKind::Activity;
        offering.catalog =
            LearningOfferingSnapshot::Activity(crate::models::ActivityOfferingSnapshot {
                activity_version_id: Uuid::from_u128(3),
                activity_id: Uuid::from_u128(4),
                curriculum_activity_requirement_id: None,
                registration_type: crate::models::ActivityRegistrationType::Assigned,
                scheduling_mode: crate::models::ActivitySchedulingMode::Synchronized,
                hours: "1".into(),
                capacity: None,
                attendance_requirement: crate::models::ActivityAttendanceRequirement {
                    minimum_percent: Some("80".into()),
                    required_sessions: None,
                },
                pass_criteria: crate::models::ActivityPassCriteria {
                    require_attendance: true,
                    require_teacher_confirmation: true,
                    outcomes: vec!["pass".into(), "fail".into()],
                },
            });
        offering.groups.clear();
        snapshot
    }

    #[test]
    fn central_activity_publishes_before_groups_but_keeps_target_and_count_validation() {
        let mut snapshot = central_snapshot();
        assert!(readiness(&snapshot).is_empty());
        snapshot.offerings[0].weekly_period_target = 0;
        assert!(readiness(&snapshot).is_empty());
        snapshot.offerings[0].weekly_period_target = -1;
        snapshot.offerings[0].targets.clear();
        let codes: Vec<_> = readiness(&snapshot)
            .into_iter()
            .map(|finding| finding.code)
            .collect();
        assert_eq!(
            codes,
            vec![
                DeliveryReadinessCode::InvalidWeeklyTarget,
                DeliveryReadinessCode::MissingTargets
            ]
        );
    }

    #[test]
    fn zero_activity_targets_preserve_requirements_and_reject_negative_in_both_modes() {
        for mode in [
            crate::models::ActivitySchedulingMode::Independent,
            crate::models::ActivitySchedulingMode::Synchronized,
        ] {
            let mut snapshot = central_snapshot();
            snapshot.offerings[0].groups = ready_snapshot().offerings[0].groups.clone();
            if let LearningOfferingSnapshot::Activity(activity) = &mut snapshot.offerings[0].catalog
            {
                activity.scheduling_mode = mode;
            }
            let catalog = serde_json::to_value(&snapshot.offerings[0].catalog).unwrap();
            snapshot.offerings[0].weekly_period_target = 0;
            assert!(readiness(&snapshot).is_empty());
            assert_eq!(
                serde_json::to_value(&snapshot.offerings[0].catalog).unwrap(),
                catalog
            );
            snapshot.offerings[0].weekly_period_target = -1;
            assert_eq!(
                readiness(&snapshot)[0].code,
                DeliveryReadinessCode::InvalidWeeklyTarget
            );
            snapshot.offerings[0].weekly_period_target = 0;
            snapshot.offerings[0].groups[0].teachers.clear();
            assert_eq!(
                readiness(&snapshot)[0].code,
                DeliveryReadinessCode::MissingPrimaryTeacher
            );
            snapshot.offerings[0].targets.clear();
            assert!(readiness(&snapshot)
                .iter()
                .any(|finding| finding.code == DeliveryReadinessCode::MissingTargets));
            snapshot.offerings[0].groups.clear();
            assert_eq!(
                readiness(&snapshot)
                    .iter()
                    .any(|finding| finding.code == DeliveryReadinessCode::MissingGroups),
                mode == crate::models::ActivitySchedulingMode::Independent
            );
        }
    }

    #[test]
    fn independent_activities_and_courses_still_require_groups_and_existing_groups_require_primary_teachers(
    ) {
        let mut independent = central_snapshot();
        if let LearningOfferingSnapshot::Activity(activity) = &mut independent.offerings[0].catalog
        {
            activity.scheduling_mode = crate::models::ActivitySchedulingMode::Independent;
        }
        assert_eq!(
            readiness(&independent)[0].code,
            DeliveryReadinessCode::MissingGroups
        );
        let mut course = ready_snapshot();
        course.offerings[0].groups.clear();
        assert_eq!(
            readiness(&course)[0].code,
            DeliveryReadinessCode::MissingGroups
        );
        let mut central = central_snapshot();
        central.offerings[0].groups = ready_snapshot().offerings[0].groups.clone();
        central.offerings[0].groups[0].teachers.clear();
        assert_eq!(
            readiness(&central)[0].code,
            DeliveryReadinessCode::MissingPrimaryTeacher
        );
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
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.versions.read_cutover_audit",
    )
    .await?;
    read_cutover_audit_on_connection(&mut connection).await
}

pub async fn read_cutover_audit_on_connection(
    connection: &mut sqlx::PgConnection,
) -> Result<DeliveryCutoverAudit, school_errors::AppError> {
    let rows: Vec<DeliveryAuditRow> = DbSession::new(connection).query_as("academic_delivery.services.versions.read_cutover_audit_on_connection", "SELECT migration_version,passed,cutover_completed,checks FROM academic_delivery_version_migration_audit WHERE migration_version IN (87,88) ORDER BY migration_version")
        .fetch_all().await?;
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
