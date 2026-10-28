use school_database::DbSession;
use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use school_authorization::AcademicResourceListFilter;
use school_errors::AppError;

use super::super::models::{
    ActivityRegistrationType, ApplyRosterRequest, CreateLearningGroupRequest,
    CurriculumGroupProposal, LearningGroup, LearningGroupRow, LearningGroupStudentRow,
    LearningGroupTeacherAssignment, LearningOfferingKind, LearningOfferingStatus,
    PublishRosterRequest, ReplaceLearningGroupHomeroomsRequest,
    ReplaceLearningGroupTeachersRequest, RosterOverrideAction, RosterPreview, RosterPreviewStudent,
    RosterStatus, TeacherAssignmentInput, UpdateLearningGroupRequest,
};
use super::{
    append_audit, invalidate_group_academic_confirmations, require_writable_offering_term,
    require_writable_term, stable_hash, validate_row_version,
};

const GROUP_COLUMNS: &str = r#"
    id, learning_offering_id, academic_term_id, academic_year_id, code, name,
    description, capacity, status, roster_status, roster_published_at, row_version,
    migration_provenance <> '{}'::jsonb AS migrated, created_at, updated_at
"#;
const MAX_TERM_GROUPS: usize = 2_000;

const QUALIFIED_GROUP_COLUMNS: &str = r#"
    learning_group.id, learning_group.learning_offering_id,
    learning_group.academic_term_id, learning_group.academic_year_id,
    learning_group.code, learning_group.name, learning_group.description,
    learning_group.capacity, learning_group.status, learning_group.roster_status,
    learning_group.roster_published_at, learning_group.row_version,
    learning_group.migration_provenance <> '{}'::jsonb AS migrated,
    learning_group.created_at, learning_group.updated_at
"#;

// Multiple-room groups use their first academic room as an ordering anchor.
// Numeric tokens compare by significant digit count then digits, without casts
// or locale configuration. Persisted snapshots keep their existing order.
const GROUP_ROOM_ORDER_JOIN: &str = r#"
    LEFT JOIN LATERAL (
        SELECT room.id AS homeroom_id,
               (CASE grade.level_type WHEN 'kindergarten' THEN 100 WHEN 'primary' THEN 200
                    WHEN 'secondary' THEN 300 ELSE 400 END + grade.year) AS level_order,
               (SELECT array_agg(
                   CASE WHEN token[1] ~ '^[0-9]+$'
                       THEN '0' || lpad(length(ltrim(token[1], '0'))::text, 3, '0')
                                || ltrim(token[1], '0')
                       ELSE '1' || lower(token[1]) END COLLATE "C" ORDER BY ordinal)
                FROM regexp_matches(COALESCE(NULLIF(room.room_number, ''), room.name),
                     '[0-9]+|[^0-9]+', 'g') WITH ORDINALITY AS tokens(token, ordinal)
               ) AS natural_room
        FROM learning_group_homerooms coverage
        JOIN homerooms room ON room.id = coverage.homeroom_id
        JOIN grade_levels grade ON grade.id = room.grade_level_id
        WHERE coverage.learning_group_id = learning_group.id
        ORDER BY level_order NULLS LAST, natural_room NULLS LAST,
                 lower(room.name) COLLATE "C", room.id
        LIMIT 1
    ) room_order ON TRUE
"#;
const GROUP_ROOM_ORDER: &str = r#"room_order.homeroom_id IS NULL,
    room_order.level_order NULLS LAST, room_order.natural_room COLLATE "C" NULLS LAST,
    lower(learning_group.name) COLLATE "C", learning_group.id"#;

#[derive(Debug, sqlx::FromRow)]
struct GroupLockRow {
    id: Uuid,
    learning_offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    status: LearningOfferingStatus,
    roster_status: RosterStatus,
    row_version: i64,
    roster_source_hash: Option<String>,
    capacity: Option<i32>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct RosterSourceStudent {
    student_academic_year_id: Uuid,
    student_id: Uuid,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RosterSourceHashInput<'a> {
    learning_group_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    registration_type: Option<ActivityRegistrationType>,
    homeroom_ids: &'a [Uuid],
    candidates: &'a [RosterSourceStudent],
}

struct RosterSource {
    hash: String,
    candidates: Vec<RosterSourceStudent>,
    registration_type: Option<ActivityRegistrationType>,
}

#[derive(Debug, sqlx::FromRow)]
struct RosterDisplayRow {
    student_academic_year_id: Uuid,
    student_code: Option<String>,
    display_name: String,
    level_type: String,
    grade_year: i32,
    homeroom_name: Option<String>,
}

pub(super) struct GeneratedGroupApplyOutcome {
    pub id: Uuid,
    pub created: bool,
}

pub(super) async fn apply_curriculum_generated_group(
    transaction: &mut Transaction<'_, Postgres>,
    learning_offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    proposal: &CurriculumGroupProposal,
    effective_rooms: Option<&[Uuid]>,
) -> Result<GeneratedGroupApplyOutcome, AppError> {
    if proposal.group_key.trim().is_empty()
        || proposal.name.trim().is_empty()
        || proposal.homeroom_ids.is_empty()
    {
        return Err(AppError::ValidationError(
            "กลุ่มที่เตรียมจากหลักสูตรต้องมีรหัสและห้องประจำชั้นอย่างน้อยหนึ่งห้อง".to_string(),
        ));
    }
    let mut homeroom_ids = proposal.homeroom_ids.clone();
    homeroom_ids.sort_unstable();
    homeroom_ids.dedup();
    if homeroom_ids.len() != proposal.homeroom_ids.len() {
        return Err(AppError::ValidationError(
            "ห้องประจำชั้นซ้ำกันภายในกลุ่มที่เตรียม".to_string(),
        ));
    }

    if let Some((id, row_version)) = DbSession::new(&mut **transaction)
        .query_as::<(Uuid, i64)>(
            "academic_delivery.services.groups.apply_curriculum_generated_group",
            r#"SELECT id, row_version
           FROM learning_groups
           WHERE academic_term_id = $1
             AND learning_offering_id = $2
             AND generation_source = 'curriculum_prepare'
             AND generation_key = $3
           FOR UPDATE"#,
        )
        .bind(academic_term_id)
        .bind(learning_offering_id)
        .bind(proposal.group_key.trim())
        .fetch_optional()
        .await?
    {
        let mut existing_homeroom_ids: Vec<Uuid> = DbSession::new(&mut **transaction)
            .scalar(
                "academic_delivery.services.groups.apply_curriculum_generated_group",
                "SELECT homeroom_id FROM learning_group_homerooms \
             WHERE learning_group_id = $1 ORDER BY homeroom_id",
            )
            .bind(id)
            .fetch_all()
            .await?;
        if let Some(allowed) = effective_rooms {
            existing_homeroom_ids.retain(|room| allowed.contains(room));
        }
        existing_homeroom_ids.sort_unstable();
        if existing_homeroom_ids != homeroom_ids {
            return Err(AppError::Conflict(format!(
                "กลุ่มที่ระบบเคยเตรียมไว้ถูกปรับแต่งแล้ว (rowVersion {row_version}) กรุณาตรวจกลุ่มด้วยตนเอง"
            )));
        }
        return Ok(GeneratedGroupApplyOutcome { id, created: false });
    }

    let group_id = super::offerings::prepared_group_id(learning_offering_id, &proposal.group_key);
    let code_suffix = proposal.group_key.chars().take(10).collect::<String>();
    DbSession::new(&mut **transaction)
        .query(
            "academic_delivery.services.groups.apply_curriculum_generated_group",
            r#"INSERT INTO learning_groups (
               id, learning_offering_id, academic_term_id, academic_year_id,
               code, name, status, roster_status, generation_source, generation_key
           ) VALUES ($1, $2, $3, $4, $5, $6, 'draft', 'draft',
                     'curriculum_prepare', $7)"#,
        )
        .bind(group_id)
        .bind(learning_offering_id)
        .bind(academic_term_id)
        .bind(academic_year_id)
        .bind(format!("PREP-{}", code_suffix.to_uppercase()))
        .bind(proposal.name.trim())
        .bind(proposal.group_key.trim())
        .execute()
        .await?;
    for homeroom_id in homeroom_ids {
        DbSession::new(&mut **transaction)
            .query(
                "academic_delivery.services.groups.apply_curriculum_generated_group",
                r#"INSERT INTO learning_group_homerooms (
                   id, learning_group_id, academic_term_id, academic_year_id,
                   homeroom_id, coverage_source
               ) VALUES ($1, $2, $3, $4, $5, 'curriculum_prepare')"#,
            )
            .bind(Uuid::now_v7())
            .bind(group_id)
            .bind(academic_term_id)
            .bind(academic_year_id)
            .bind(homeroom_id)
            .execute()
            .await?;
    }
    Ok(GeneratedGroupApplyOutcome {
        id: group_id,
        created: true,
    })
}

pub async fn list(pool: &PgPool, offering_id: Uuid) -> Result<Vec<LearningGroup>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.groups.list",
    )
    .await?;
    list_on_connection(&mut connection, offering_id).await
}

pub async fn list_on_connection(
    connection: &mut sqlx::PgConnection,
    offering_id: Uuid,
) -> Result<Vec<LearningGroup>, AppError> {
    let sql = format!(
        "SELECT {QUALIFIED_GROUP_COLUMNS} FROM learning_groups learning_group \
         {GROUP_ROOM_ORDER_JOIN} WHERE learning_group.learning_offering_id = $1 \
         ORDER BY {GROUP_ROOM_ORDER} LIMIT 500"
    );
    let rows: Vec<LearningGroupRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.groups.list_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(offering_id)
        .fetch_all()
        .await?;
    hydrate_many(connection, rows).await
}

/// Batch hydration preserves the authoritative academic-room list order.
pub(super) async fn get_many_on_connection(
    connection: &mut sqlx::PgConnection,
    ids: &[Uuid],
) -> Result<Vec<LearningGroup>, AppError> {
    let sql = format!(
        "SELECT {QUALIFIED_GROUP_COLUMNS} FROM learning_groups learning_group \
        {GROUP_ROOM_ORDER_JOIN} WHERE learning_group.id=ANY($1) ORDER BY {GROUP_ROOM_ORDER}"
    );
    let rows = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.groups.get_many_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(ids)
        .fetch_all()
        .await?;
    hydrate_many(connection, rows).await
}

pub async fn list_for_term(
    pool: &PgPool,
    academic_term_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<LearningGroup>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.groups.list_for_term",
    )
    .await?;
    list_for_term_on_connection(&mut connection, academic_term_id, filter).await
}

pub async fn list_for_term_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_term_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<LearningGroup>, AppError> {
    let owner_ids = filter.allowed_organization_unit_ids();
    let sql = format!(
        "SELECT {QUALIFIED_GROUP_COLUMNS} \
         FROM learning_groups learning_group \
         JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id \
         {GROUP_ROOM_ORDER_JOIN} \
         WHERE learning_group.academic_term_id = $1 \
           AND ($2 OR offering.owning_organization_unit_id = ANY($3) OR EXISTS (\
               SELECT 1 FROM learning_groups assigned_group \
               JOIN learning_group_teachers teacher \
                 ON teacher.learning_group_id = assigned_group.id \
               WHERE assigned_group.learning_offering_id = offering.id \
                 AND teacher.teacher_id = $4\
           )) \
         ORDER BY {GROUP_ROOM_ORDER} \
         LIMIT $5"
    );
    let rows: Vec<LearningGroupRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.groups.list_for_term_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(academic_term_id)
        .bind(filter.includes_school_owned)
        .bind(owner_ids)
        .bind(filter.assigned_actor_id)
        .bind((MAX_TERM_GROUPS + 1) as i64)
        .fetch_all()
        .await?;
    if rows.len() > MAX_TERM_GROUPS {
        return Err(AppError::ValidationError(
            "จำนวนกลุ่มเรียนในภาคเรียนเกิน 2000 กลุ่ม กรุณาแบ่งข้อมูลก่อนเปิดพื้นที่ทำงาน".to_string(),
        ));
    }
    hydrate_many(connection, rows).await
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<LearningGroup, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.groups.get",
    )
    .await?;
    get_on_connection(&mut connection, id).await
}

pub async fn get_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<LearningGroup, AppError> {
    let sql = format!("SELECT {GROUP_COLUMNS} FROM learning_groups WHERE id = $1");
    let row: LearningGroupRow = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.groups.get_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()))?;
    hydrate(connection, row).await
}

/// Reviewed manual activation creates one target-room group in the offering transaction.
pub(super) async fn create_target_homeroom_groups(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    term_id: Uuid,
    year_id: Uuid,
) -> Result<(), AppError> {
    let rooms: Vec<(Uuid, String)> = DbSession::new(&mut **transaction).query_as("academic_delivery.services.groups.create_target_homeroom_groups",
        "SELECT DISTINCT room.id, room.name FROM homerooms room \
         JOIN learning_offering_targets target ON target.learning_offering_id = $1 \
         AND (target.homeroom_id = room.id OR (target.target_kind = 'grade_program' \
         AND target.grade_level_id = room.grade_level_id AND target.study_program_id = room.study_program_id)) \
         WHERE room.academic_year_id = $2 AND room.is_active ORDER BY room.id LIMIT 501",
    ).bind(offering_id).bind(year_id).fetch_all().await?;
    if rooms.is_empty() || rooms.len() > 500 {
        return Err(AppError::ValidationError(
            "ต้องมีห้องเป้าหมาย 1 ถึง 500 ห้องสำหรับเปิดพร้อมจัดกลุ่ม".into(),
        ));
    }
    let room_ids: Vec<Uuid> = rooms.iter().map(|room| room.0).collect();
    let group_ids: Vec<Uuid> = rooms.iter().map(|_| Uuid::now_v7()).collect();
    let names: Vec<String> = rooms.into_iter().map(|room| room.1).collect();
    DbSession::new(&mut **transaction).query("academic_delivery.services.groups.create_target_homeroom_groups", "INSERT INTO learning_groups (id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) \
        SELECT group_id,$1,$2,$3,'ROOM-' || group_id::text,name,'draft','draft' \
        FROM unnest($4::uuid[],$5::text[]) AS input(group_id,name)")
        .bind(offering_id).bind(term_id).bind(year_id).bind(&group_ids).bind(&names)
        .execute().await?;
    DbSession::new(&mut **transaction).query("academic_delivery.services.groups.create_target_homeroom_groups", "INSERT INTO learning_group_homerooms (id,learning_group_id,academic_term_id,academic_year_id,homeroom_id,coverage_source) \
        SELECT gen_random_uuid(),group_id,$1,$2,room_id,'manual' \
        FROM unnest($3::uuid[],$4::uuid[]) AS input(group_id,room_id)")
        .bind(term_id).bind(year_id).bind(&group_ids).bind(&room_ids).execute().await?;
    Ok(())
}

pub async fn create(
    pool: &PgPool,
    actor_user_id: Uuid,
    offering_id: Uuid,
    request: CreateLearningGroupRequest,
) -> Result<LearningGroup, AppError> {
    validate_group_fields(&request.name, request.capacity)?;
    let id = Uuid::now_v7();
    let internal_code = internal_group_code(&request.name)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.groups.create",
    )
    .await?;
    require_writable_offering_term(&mut transaction, offering_id).await?;
    let (term_id, year_id, offering_status): (Uuid, Uuid, LearningOfferingStatus) =
        DbSession::new(&mut *transaction)
            .query_as(
                "academic_delivery.services.groups.create",
                "SELECT academic_term_id, academic_year_id, status \
             FROM learning_offerings WHERE id = $1 FOR UPDATE",
            )
            .bind(offering_id)
            .fetch_optional()
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบรายการเปิดสอน".to_string()))?;
    if matches!(
        offering_status,
        LearningOfferingStatus::Cancelled | LearningOfferingStatus::Closed
    ) {
        return Err(AppError::Conflict("รายการเปิดสอนปิดแล้ว".to_string()));
    }
    ensure_unique_internal_group_code(&mut transaction, offering_id, &internal_code).await?;
    validate_preferred_rooms(&mut transaction, &request.preferred_room_ids).await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_delivery.services.groups.create",
            r#"INSERT INTO learning_groups (
               id, learning_offering_id, academic_term_id, academic_year_id,
               code, name, description, capacity, status, roster_status
           ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'draft', 'draft')"#,
        )
        .bind(id)
        .bind(offering_id)
        .bind(term_id)
        .bind(year_id)
        .bind(internal_code)
        .bind(request.name.trim())
        .bind(request.description)
        .bind(request.capacity)
        .execute()
        .await?;
    replace_preferred_rooms_in_transaction(
        &mut transaction,
        id,
        term_id,
        year_id,
        &request.preferred_room_ids,
    )
    .await?;
    super::versions::refresh_drafts_for_offering(&mut transaction, offering_id).await?;
    transaction.commit().await?;
    let mut completion = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.groups.create.completion",
    )
    .await?;
    append_audit(
        &mut completion,
        "learning_group.created",
        "learning_group",
        id,
        year_id,
        term_id,
        actor_user_id,
        serde_json::json!({ "learningOfferingId": offering_id }),
    )
    .await?;
    get_on_connection(&mut completion, id).await
}

pub async fn update(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: UpdateLearningGroupRequest,
) -> Result<LearningGroup, AppError> {
    validate_row_version(request.row_version)?;
    validate_group_fields(&request.name, request.capacity)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.groups.update",
    )
    .await?;
    let term_id = find_group_term(&mut transaction, id).await?;
    require_writable_term(&mut transaction, term_id, false).await?;
    let group = lock_offering_then_group(&mut transaction, id, term_id).await?;
    require_mutable_group(&group, request.row_version, false)?;
    validate_preferred_rooms(&mut transaction, &request.preferred_room_ids).await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_delivery.services.groups.update",
            "UPDATE learning_groups SET name = $1, description = $2, capacity = $3, \
         row_version = row_version + 1, updated_at = now() WHERE id = $4",
        )
        .bind(request.name.trim())
        .bind(request.description)
        .bind(request.capacity)
        .bind(id)
        .execute()
        .await?;
    replace_preferred_rooms_in_transaction(
        &mut transaction,
        id,
        group.academic_term_id,
        group.academic_year_id,
        &request.preferred_room_ids,
    )
    .await?;
    super::versions::refresh_drafts_for_offering(&mut transaction, group.learning_offering_id)
        .await?;
    transaction.commit().await?;
    let mut completion = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.groups.update.completion",
    )
    .await?;
    append_group_audit(
        &mut completion,
        actor_user_id,
        &group,
        "learning_group.updated",
    )
    .await?;
    get_on_connection(&mut completion, id).await
}

pub async fn replace_teachers(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: ReplaceLearningGroupTeachersRequest,
) -> Result<LearningGroup, AppError> {
    validate_row_version(request.row_version)?;
    let teacher_ids = unique_teacher_ids(&request.teachers)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.groups.replace_teachers",
    )
    .await?;
    let academic_term_id = find_group_term(&mut transaction, id).await?;
    require_writable_term(&mut transaction, academic_term_id, false).await?;
    let group = lock_offering_then_group(&mut transaction, id, academic_term_id).await?;
    require_mutable_group(&group, request.row_version, false)?;
    require_draft_group_teachers(&group)?;
    if !teacher_ids.is_empty() {
        let count: i64 = DbSession::new(&mut *transaction)
            .scalar(
                "academic_delivery.services.groups.replace_teachers",
                "SELECT count(*) FROM users WHERE id = ANY($1) \
             AND user_type = 'staff' AND status = 'active'",
            )
            .bind(&teacher_ids)
            .fetch_one()
            .await?;
        if count != teacher_ids.len() as i64 {
            return Err(AppError::ValidationError(
                "ครูผู้สอนต้องเป็นบุคลากรที่ใช้งานอยู่".to_string(),
            ));
        }
    }
    let offering_starts_on: chrono::NaiveDate = DbSession::new(&mut *transaction)
        .scalar(
            "academic_delivery.services.groups.replace_teachers",
            "SELECT starts_on FROM learning_offerings WHERE id = $1",
        )
        .bind(group.learning_offering_id)
        .fetch_one()
        .await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_delivery.services.groups.replace_teachers",
            "DELETE FROM learning_group_teachers WHERE learning_group_id = $1",
        )
        .bind(id)
        .execute()
        .await?;
    for teacher in request.teachers {
        DbSession::new(&mut *transaction)
            .query(
                "academic_delivery.services.groups.replace_teachers",
                r#"INSERT INTO learning_group_teachers (
                   id, learning_group_id, academic_term_id, academic_year_id,
                   teacher_id, role, starts_on, created_by, updated_by
               ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)"#,
            )
            .bind(Uuid::now_v7())
            .bind(id)
            .bind(group.academic_term_id)
            .bind(group.academic_year_id)
            .bind(teacher.teacher_id)
            .bind(teacher.role)
            .bind(offering_starts_on)
            .bind(actor_user_id)
            .execute()
            .await?;
    }
    increment_group_revision(&mut transaction, id).await?;
    invalidate_group_academic_confirmations(&mut transaction, &[id]).await?;
    super::versions::refresh_drafts_for_offering(&mut transaction, group.learning_offering_id)
        .await?;
    transaction.commit().await?;
    let mut completion = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.groups.replace_teachers.completion",
    )
    .await?;
    append_group_audit(
        &mut completion,
        actor_user_id,
        &group,
        "learning_group.teachers_replaced",
    )
    .await?;
    get_on_connection(&mut completion, id).await
}

pub async fn replace_homerooms(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: ReplaceLearningGroupHomeroomsRequest,
) -> Result<LearningGroup, AppError> {
    validate_row_version(request.row_version)?;
    let homeroom_ids = unique_ids(&request.homeroom_ids, "ห้องเรียนซ้ำกัน")?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.groups.replace_homerooms",
    )
    .await?;
    let term_id = find_group_term(&mut transaction, id).await?;
    require_writable_term(&mut transaction, term_id, false).await?;
    let group = lock_offering_then_group(&mut transaction, id, term_id).await?;
    require_mutable_group(&group, request.row_version, true)?;
    validate_homeroom_coverage(&mut transaction, &group, &homeroom_ids).await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_delivery.services.groups.replace_homerooms",
            "DELETE FROM learning_group_homerooms WHERE learning_group_id = $1",
        )
        .bind(id)
        .execute()
        .await?;
    for homeroom_id in homeroom_ids {
        DbSession::new(&mut *transaction)
            .query(
                "academic_delivery.services.groups.replace_homerooms",
                r#"INSERT INTO learning_group_homerooms (
                   id, learning_group_id, academic_term_id, academic_year_id,
                   homeroom_id, coverage_source
               ) VALUES ($1, $2, $3, $4, $5, 'manual')"#,
            )
            .bind(Uuid::now_v7())
            .bind(id)
            .bind(group.academic_term_id)
            .bind(group.academic_year_id)
            .bind(homeroom_id)
            .execute()
            .await?;
    }
    DbSession::new(&mut *transaction)
        .query(
            "academic_delivery.services.groups.replace_homerooms",
            "UPDATE learning_groups SET roster_source_hash = NULL, \
         row_version = row_version + 1, updated_at = now() WHERE id = $1",
        )
        .bind(id)
        .execute()
        .await?;
    super::versions::refresh_drafts_for_offering(&mut transaction, group.learning_offering_id)
        .await?;
    transaction.commit().await?;
    let mut completion = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.groups.replace_homerooms.completion",
    )
    .await?;
    append_group_audit(
        &mut completion,
        actor_user_id,
        &group,
        "learning_group.homerooms_replaced",
    )
    .await?;
    get_on_connection(&mut completion, id).await
}

pub async fn preview_roster(pool: &PgPool, group_id: Uuid) -> Result<RosterPreview, AppError> {
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.groups.preview_roster",
    )
    .await?;
    let group = lock_group(&mut transaction, group_id).await?;
    let source = roster_source(&mut transaction, &group).await?;
    let current = current_roster_students(&mut transaction, group_id).await?;
    let mut preview = build_roster_preview(group_id, source, current);
    enrich_roster_preview(&mut transaction, &mut preview).await?;
    transaction.commit().await?;
    Ok(preview)
}

pub async fn apply_roster(
    pool: &PgPool,
    actor_user_id: Uuid,
    group_id: Uuid,
    request: ApplyRosterRequest,
) -> Result<LearningGroup, AppError> {
    validate_row_version(request.row_version)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.groups.apply_roster",
    )
    .await?;
    let academic_term_id = find_group_term(&mut transaction, group_id).await?;
    let term = require_writable_term(&mut transaction, academic_term_id, false).await?;
    let group = lock_offering_then_group(&mut transaction, group_id, academic_term_id).await?;
    require_mutable_group(&group, request.row_version, true)?;
    let source = roster_source(&mut transaction, &group).await?;
    if source.hash != request.source_hash {
        return Err(AppError::Conflict(
            "รายชื่อนักเรียนต้นทางเปลี่ยนไป กรุณาโหลด preview ใหม่".to_string(),
        ));
    }
    if !request.overrides.is_empty() {
        super::roster_tracking::require_manual(&mut transaction, group_id).await?;
    }
    let automatic: bool = DbSession::new(&mut *transaction).scalar("academic_delivery.services.groups.apply_roster", "SELECT EXISTS(SELECT 1 FROM learning_group_roster_tracking WHERE learning_group_id=$1 AND mode='homeroom')")
        .bind(group_id).fetch_one().await?;
    if automatic {
        super::roster_tracking::sync_group(&mut transaction, actor_user_id, group_id, None).await?;
    } else {
        let current = current_roster_students(&mut transaction, group_id).await?;
        let mut desired: HashMap<Uuid, (Uuid, &str)> = HashMap::new();
        if source.registration_type != Some(ActivityRegistrationType::SelfRegistration) {
            for candidate in &source.candidates {
                desired.insert(
                    candidate.student_academic_year_id,
                    (candidate.student_id, "placement"),
                );
            }
        } else {
            for current_student in &current {
                desired.insert(
                    current_student.student_academic_year_id,
                    (current_student.student_id, "self_registration"),
                );
            }
        }
        let mut override_ids = HashSet::new();
        for roster_override in request.overrides {
            if !override_ids.insert(roster_override.student_academic_year_id) {
                return Err(AppError::ValidationError(
                    "นักเรียนในรายการ override ซ้ำกัน".to_string(),
                ));
            }
            match roster_override.action {
                RosterOverrideAction::Add => {
                    let student_id = validate_manual_student_year(
                        &mut transaction,
                        roster_override.student_academic_year_id,
                        group.academic_year_id,
                    )
                    .await?;
                    desired.insert(
                        roster_override.student_academic_year_id,
                        (student_id, "manual_add"),
                    );
                }
                RosterOverrideAction::Remove => {
                    desired.remove(&roster_override.student_academic_year_id);
                }
            }
        }
        for current_student in current {
            if !desired.contains_key(&current_student.student_academic_year_id) {
                DbSession::new(&mut *transaction)
                    .query(
                        "academic_delivery.services.groups.apply_roster",
                        "UPDATE learning_group_students SET membership_status = 'removed', \
                 left_at = GREATEST(joined_at, $1), row_version = row_version + 1, \
                 updated_at = now() WHERE id = $2 AND membership_status = 'active'",
                    )
                    .bind(term.start_date)
                    .bind(current_student.id)
                    .execute()
                    .await?;
            }
        }
        for (student_year_id, (student_id, roster_source)) in desired {
            let active_exists: bool = DbSession::new(&mut *transaction)
                .scalar(
                    "academic_delivery.services.groups.apply_roster",
                    "SELECT EXISTS (SELECT 1 FROM learning_group_students \
             WHERE learning_group_id = $1 AND student_academic_year_id = $2 \
               AND membership_status = 'active')",
                )
                .bind(group_id)
                .bind(student_year_id)
                .fetch_one()
                .await?;
            if !active_exists {
                DbSession::new(&mut *transaction)
                    .query(
                        "academic_delivery.services.groups.apply_roster",
                        r#"INSERT INTO learning_group_students (
                       id, learning_group_id, academic_term_id, academic_year_id,
                       student_academic_year_id, student_id, membership_status,
                       roster_source, joined_at
                   ) VALUES ($1, $2, $3, $4, $5, $6, 'active', $7, $8)"#,
                    )
                    .bind(Uuid::now_v7())
                    .bind(group_id)
                    .bind(group.academic_term_id)
                    .bind(group.academic_year_id)
                    .bind(student_year_id)
                    .bind(student_id)
                    .bind(roster_source)
                    .bind(term.start_date)
                    .execute()
                    .await?;
            }
        }
    }
    DbSession::new(&mut *transaction)
        .query(
            "academic_delivery.services.groups.apply_roster",
            "UPDATE learning_groups SET roster_status = 'draft', roster_source_hash = $1, \
         roster_published_at = NULL, roster_publish_idempotency_key = NULL, \
         row_version = row_version + 1, updated_at = now() WHERE id = $2",
        )
        .bind(&source.hash)
        .bind(group_id)
        .execute()
        .await?;
    invalidate_group_academic_confirmations(&mut transaction, &[group_id]).await?;
    transaction.commit().await?;
    let mut completion = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.groups.apply_roster.completion",
    )
    .await?;
    append_group_audit(
        &mut completion,
        actor_user_id,
        &group,
        "learning_group.roster_applied",
    )
    .await?;
    get_on_connection(&mut completion, group_id).await
}

pub async fn publish_roster(
    pool: &PgPool,
    actor_user_id: Uuid,
    group_id: Uuid,
    request: PublishRosterRequest,
) -> Result<LearningGroup, AppError> {
    validate_row_version(request.row_version)?;
    let mut preflight = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.groups.publish_roster.preflight",
    )
    .await?;
    if let Some(existing_id) = DbSession::new(&mut preflight)
        .scalar::<Uuid>(
            "academic_delivery.services.groups.publish_roster.preflight",
            "SELECT id FROM learning_groups WHERE roster_publish_idempotency_key = $1",
        )
        .bind(request.idempotency_key)
        .fetch_optional()
        .await?
    {
        if existing_id == group_id {
            return get_on_connection(&mut preflight, group_id).await;
        }
        return Err(AppError::Conflict(
            "idempotencyKey ถูกใช้กับกลุ่มอื่นแล้ว".to_string(),
        ));
    }
    drop(preflight);
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.groups.publish_roster",
    )
    .await?;
    let academic_term_id = find_group_term(&mut transaction, group_id).await?;
    require_writable_term(&mut transaction, academic_term_id, false).await?;
    let group = lock_offering_then_group(&mut transaction, group_id, academic_term_id).await?;
    require_mutable_group(&group, request.row_version, false)?;
    if group.roster_status != RosterStatus::Draft {
        return Err(AppError::Conflict(
            "roster นี้เผยแพร่แล้ว หากต้องการเปลี่ยนรายชื่อต้องสร้างฉบับ draft ใหม่".to_string(),
        ));
    }
    let offering_status: LearningOfferingStatus = DbSession::new(&mut *transaction)
        .scalar(
            "academic_delivery.services.groups.publish_roster",
            "SELECT status FROM learning_offerings WHERE id = $1",
        )
        .bind(group.learning_offering_id)
        .fetch_one()
        .await?;
    if offering_status != LearningOfferingStatus::Published {
        return Err(AppError::Conflict(
            "ต้องเผยแพร่รายการเปิดสอนก่อนเผยแพร่ roster".to_string(),
        ));
    }
    let source = roster_source(&mut transaction, &group).await?;
    if group.roster_source_hash.as_deref().map(str::trim_end) != Some(source.hash.as_str()) {
        return Err(AppError::Conflict(
            "รายชื่อนักเรียนต้นทางเปลี่ยนไป กรุณาสร้าง roster ใหม่".to_string(),
        ));
    }
    validate_publishable_roster(&mut transaction, &group).await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_delivery.services.groups.publish_roster",
            "UPDATE learning_group_students SET published_at = now(), updated_at = now() \
         WHERE learning_group_id = $1 AND membership_status = 'active'",
        )
        .bind(group_id)
        .execute()
        .await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_delivery.services.groups.publish_roster",
            "UPDATE learning_groups SET roster_status = 'published', roster_published_at = now(), \
         roster_publish_idempotency_key = $1, row_version = row_version + 1, \
         updated_at = now() WHERE id = $2",
        )
        .bind(request.idempotency_key)
        .bind(group_id)
        .execute()
        .await?;
    super::roster_tracking::sync_group(&mut transaction, actor_user_id, group_id, None).await?;
    transaction.commit().await?;
    let mut completion = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.groups.publish_roster.completion",
    )
    .await?;
    append_group_audit(
        &mut completion,
        actor_user_id,
        &group,
        "learning_group.roster_published",
    )
    .await?;
    get_on_connection(&mut completion, group_id).await
}

async fn hydrate(
    connection: &mut sqlx::PgConnection,
    row: LearningGroupRow,
) -> Result<LearningGroup, AppError> {
    hydrate_many(connection, vec![row])
        .await?
        .pop()
        .ok_or_else(|| AppError::InternalServerError("ไม่สามารถประกอบข้อมูลกลุ่มเรียนได้".to_string()))
}

async fn hydrate_many(
    connection: &mut sqlx::PgConnection,
    rows: Vec<LearningGroupRow>,
) -> Result<Vec<LearningGroup>, AppError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let group_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let teacher_rows: Vec<GroupTeacherRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.groups.hydrate_many",
            r#"SELECT teacher.learning_group_id, teacher.id, teacher.teacher_id,
                  user_account.display_name AS display_name,
                  teacher.role, teacher.starts_on, teacher.ends_on, teacher.row_version
           FROM learning_group_teachers teacher
           JOIN user_display_names user_account ON user_account.id = teacher.teacher_id
           WHERE teacher.learning_group_id = ANY($1)
           ORDER BY teacher.learning_group_id,
                    CASE teacher.role
                        WHEN 'primary' THEN 1
                        WHEN 'secondary' THEN 2
                        ELSE 3
                    END,
                    teacher.starts_on, teacher.id"#,
        )
        .bind(&group_ids)
        .fetch_all()
        .await?;
    let homeroom_rows: Vec<GroupHomeroomRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.groups.hydrate_many",
            "SELECT learning_group_id, homeroom_id FROM learning_group_homerooms \
         WHERE learning_group_id = ANY($1) \
         ORDER BY learning_group_id, homeroom_id",
        )
        .bind(&group_ids)
        .fetch_all()
        .await?;
    let preferred_room_rows: Vec<GroupPreferredRoomRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.groups.hydrate_many",
            "SELECT learning_group_id, room_id FROM learning_group_preferred_rooms \
         WHERE learning_group_id = ANY($1) \
         ORDER BY learning_group_id, rank, room_id",
        )
        .bind(&group_ids)
        .fetch_all()
        .await?;

    let mut teachers_by_group: HashMap<Uuid, Vec<LearningGroupTeacherAssignment>> = HashMap::new();
    for teacher in teacher_rows {
        teachers_by_group
            .entry(teacher.learning_group_id)
            .or_default()
            .push(teacher.into());
    }
    let mut homerooms_by_group: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for homeroom in homeroom_rows {
        homerooms_by_group
            .entry(homeroom.learning_group_id)
            .or_default()
            .push(homeroom.homeroom_id);
    }
    let mut preferred_rooms_by_group: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for room in preferred_room_rows {
        preferred_rooms_by_group
            .entry(room.learning_group_id)
            .or_default()
            .push(room.room_id);
    }

    Ok(rows
        .into_iter()
        .map(|row| {
            let teachers_locked = row.status != LearningOfferingStatus::Draft;
            LearningGroup {
                id: row.id,
                learning_offering_id: row.learning_offering_id,
                academic_term_id: row.academic_term_id,
                academic_year_id: row.academic_year_id,
                code: row.code,
                name: row.name,
                description: row.description,
                capacity: row.capacity,
                status: row.status,
                teachers_locked,
                roster_status: row.roster_status,
                roster_published_at: row.roster_published_at,
                row_version: row.row_version,
                migrated: row.migrated,
                created_at: row.created_at,
                updated_at: row.updated_at,
                teacher_assignments: teachers_by_group.remove(&row.id).unwrap_or_default(),
                homeroom_ids: homerooms_by_group.remove(&row.id).unwrap_or_default(),
                preferred_room_ids: preferred_rooms_by_group.remove(&row.id).unwrap_or_default(),
            }
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct GroupTeacherRow {
    learning_group_id: Uuid,
    id: Uuid,
    teacher_id: Uuid,
    display_name: String,
    role: super::super::models::LearningTeacherRole,
    starts_on: chrono::NaiveDate,
    ends_on: Option<chrono::NaiveDate>,
    row_version: i64,
}

impl From<GroupTeacherRow> for LearningGroupTeacherAssignment {
    fn from(row: GroupTeacherRow) -> Self {
        Self {
            id: row.id,
            teacher_id: row.teacher_id,
            display_name: row.display_name,
            role: row.role,
            starts_on: row.starts_on,
            ends_on: row.ends_on,
            row_version: row.row_version,
        }
    }
}

#[derive(sqlx::FromRow)]
struct GroupHomeroomRow {
    learning_group_id: Uuid,
    homeroom_id: Uuid,
}

#[derive(sqlx::FromRow)]
struct GroupPreferredRoomRow {
    learning_group_id: Uuid,
    room_id: Uuid,
}

async fn lock_group(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<GroupLockRow, AppError> {
    DbSession::new(&mut **transaction)
        .query_as(
            "academic_delivery.services.groups.lock_group",
            "SELECT id, learning_offering_id, academic_term_id, academic_year_id, \
         status, roster_status, row_version, roster_source_hash::text, capacity \
         FROM learning_groups WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()))
}

async fn lock_offering_then_group(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    academic_term_id: Uuid,
) -> Result<GroupLockRow, AppError> {
    let offering_id: Uuid = DbSession::new(&mut **transaction).scalar("academic_delivery.services.groups.lock_offering_then_group",
        "SELECT learning_offering_id FROM learning_groups WHERE id = $1 AND academic_term_id = $2",
    )
    .bind(id)
    .bind(academic_term_id)
    .fetch_optional()
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()))?;
    DbSession::new(&mut **transaction)
        .query(
            "academic_delivery.services.groups.lock_offering_then_group",
            "SELECT id FROM learning_offerings WHERE id = $1 FOR UPDATE",
        )
        .bind(offering_id)
        .execute()
        .await?;
    let group = lock_group(transaction, id).await?;
    if group.learning_offering_id != offering_id || group.academic_term_id != academic_term_id {
        return Err(AppError::Conflict(
            "กลุ่มเรียนเปลี่ยนรายการเปิดสอนระหว่างทำรายการ".to_string(),
        ));
    }
    Ok(group)
}

async fn find_group_term(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Uuid, AppError> {
    DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.groups.find_group_term",
            "SELECT academic_term_id FROM learning_groups WHERE id = $1",
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()))
}

fn require_mutable_group(
    group: &GroupLockRow,
    expected_row_version: i64,
    require_draft_roster: bool,
) -> Result<(), AppError> {
    if group.status == LearningOfferingStatus::Closed || group.roster_status == RosterStatus::Closed
    {
        return Err(AppError::Conflict("กลุ่มเรียนปิดแล้ว".to_string()));
    }
    if require_draft_roster && group.roster_status != RosterStatus::Draft {
        return Err(AppError::Conflict(
            "roster ที่เผยแพร่แล้วต้องปิดก่อนจึงจะแก้ไขความครอบคลุมได้".to_string(),
        ));
    }
    if group.row_version != expected_row_version {
        return Err(AppError::Conflict("กลุ่มเรียนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()));
    }
    Ok(())
}

fn require_draft_group_teachers(group: &GroupLockRow) -> Result<(), AppError> {
    if group.status != LearningOfferingStatus::Draft {
        Err(AppError::Conflict(
            "เผยแพร่กลุ่มเรียนแล้ว ไม่สามารถเปลี่ยนครูผู้สอนได้".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn internal_group_code(name: &str) -> Result<String, AppError> {
    let normalized_name = name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let hash = stable_hash(&normalized_name)?;
    Ok(format!("GROUP-{}", &hash[..24]))
}

fn validate_group_fields(name: &str, capacity: Option<i32>) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::ValidationError("ชื่อกลุ่มเรียนห้ามว่าง".to_string()));
    }
    if capacity.is_some_and(|value| value <= 0) {
        return Err(AppError::ValidationError("ความจุต้องมากกว่าศูนย์".to_string()));
    }
    Ok(())
}

async fn ensure_unique_internal_group_code(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    code: &str,
) -> Result<(), AppError> {
    let duplicate: bool = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.groups.ensure_unique_internal_group_code",
            "SELECT EXISTS (SELECT 1 FROM learning_groups \
         WHERE learning_offering_id = $1 AND code = $2)",
        )
        .bind(offering_id)
        .bind(code)
        .fetch_one()
        .await?;
    if duplicate {
        Err(AppError::Conflict("ชื่อกลุ่มเรียนซ้ำในรายการเปิดสอนนี้".to_string()))
    } else {
        Ok(())
    }
}

fn unique_teacher_ids(teachers: &[TeacherAssignmentInput]) -> Result<Vec<Uuid>, AppError> {
    let mut ids = Vec::with_capacity(teachers.len());
    let mut unique = HashSet::new();
    for teacher in teachers {
        if !unique.insert(teacher.teacher_id) {
            return Err(AppError::ValidationError("ครูผู้สอนซ้ำกัน".to_string()));
        }
        ids.push(teacher.teacher_id);
    }
    Ok(ids)
}

fn unique_ids(ids: &[Uuid], message: &str) -> Result<Vec<Uuid>, AppError> {
    let mut unique = ids.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.len() != ids.len() {
        Err(AppError::ValidationError(message.to_string()))
    } else {
        Ok(unique)
    }
}

async fn validate_homeroom_coverage(
    transaction: &mut Transaction<'_, Postgres>,
    group: &GroupLockRow,
    homeroom_ids: &[Uuid],
) -> Result<(), AppError> {
    for homeroom_id in homeroom_ids {
        let valid: bool = DbSession::new(&mut **transaction)
            .scalar(
                "academic_delivery.services.groups.validate_homeroom_coverage",
                r#"SELECT EXISTS (
                   SELECT 1 FROM homerooms homeroom
                   WHERE homeroom.id = $1 AND homeroom.academic_year_id = $2
                     AND EXISTS (
                         SELECT 1 FROM learning_offering_targets target
                         WHERE target.learning_offering_id = $3
                           AND (
                               (target.target_kind = 'homeroom'
                                AND target.homeroom_id = homeroom.id)
                               OR (target.target_kind = 'grade_program'
                                   AND target.grade_level_id = homeroom.grade_level_id
                                   AND target.study_program_id = homeroom.study_program_id)
                           )
                     )
               )"#,
            )
            .bind(homeroom_id)
            .bind(group.academic_year_id)
            .bind(group.learning_offering_id)
            .fetch_one()
            .await?;
        if !valid {
            return Err(AppError::ValidationError(
                "ห้องเรียนไม่ตรงปีหรือไม่อยู่ในเป้าหมายของรายการเปิดสอน".to_string(),
            ));
        }
    }
    Ok(())
}

async fn validate_preferred_rooms(
    transaction: &mut Transaction<'_, Postgres>,
    room_ids: &[Uuid],
) -> Result<(), AppError> {
    let room_ids = unique_ids(room_ids, "ห้องที่ต้องการใช้ซ้ำกัน")?;
    if room_ids.is_empty() {
        return Ok(());
    }
    let count: i64 = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.groups.validate_preferred_rooms",
            "SELECT count(*) FROM rooms WHERE id = ANY($1) AND status = 'ACTIVE'",
        )
        .bind(&room_ids)
        .fetch_one()
        .await?;
    if count != room_ids.len() as i64 {
        return Err(AppError::ValidationError(
            "ห้องที่ต้องการใช้ต้องเป็นห้องที่เปิดใช้งาน".to_string(),
        ));
    }
    Ok(())
}

async fn replace_preferred_rooms_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    group_id: Uuid,
    term_id: Uuid,
    year_id: Uuid,
    room_ids: &[Uuid],
) -> Result<(), AppError> {
    DbSession::new(&mut **transaction)
        .query(
            "academic_delivery.services.groups.replace_preferred_rooms_in_transaction",
            "DELETE FROM learning_group_preferred_rooms WHERE learning_group_id = $1",
        )
        .bind(group_id)
        .execute()
        .await?;
    for (index, room_id) in room_ids.iter().enumerate() {
        DbSession::new(&mut **transaction)
            .query(
                "academic_delivery.services.groups.replace_preferred_rooms_in_transaction",
                r#"INSERT INTO learning_group_preferred_rooms (
                   learning_group_id, academic_term_id, academic_year_id, room_id, rank
               ) VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(group_id)
            .bind(term_id)
            .bind(year_id)
            .bind(room_id)
            .bind(
                i32::try_from(index + 1).map_err(|_| {
                    AppError::ValidationError("จำนวนห้องที่ต้องการใช้มากเกินไป".to_string())
                })?,
            )
            .execute()
            .await?;
    }
    Ok(())
}

async fn increment_group_revision(
    transaction: &mut Transaction<'_, Postgres>,
    group_id: Uuid,
) -> Result<(), AppError> {
    DbSession::new(&mut **transaction).query("academic_delivery.services.groups.increment_group_revision",
        "UPDATE learning_groups SET row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(group_id)
    .execute()
    .await?;
    Ok(())
}

async fn roster_source(
    transaction: &mut Transaction<'_, Postgres>,
    group: &GroupLockRow,
) -> Result<RosterSource, AppError> {
    let mut homeroom_ids: Vec<Uuid> = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.groups.roster_source",
            "SELECT homeroom_id FROM learning_group_homerooms \
         WHERE learning_group_id = $1 ORDER BY homeroom_id",
        )
        .bind(group.id)
        .fetch_all()
        .await?;
    homeroom_ids.sort_unstable();
    let mut candidates: Vec<RosterSourceStudent> = DbSession::new(&mut **transaction)
        .query_as(
            "academic_delivery.services.groups.roster_source",
            r#"SELECT DISTINCT student_year.id AS student_academic_year_id,
                  student_year.student_id
           FROM homeroom_placements placement
           JOIN student_academic_years student_year
             ON student_year.id = placement.student_academic_year_id
           JOIN learning_group_homerooms coverage
             ON coverage.homeroom_id = placement.homeroom_id
            AND coverage.learning_group_id = $1
           WHERE student_year.academic_year_id = $2
             AND student_year.status IN ('planned', 'active')
             AND placement.status = 'current'
             AND placement.end_date IS NULL
           ORDER BY student_academic_year_id, student_id"#,
        )
        .bind(group.id)
        .bind(group.academic_year_id)
        .fetch_all()
        .await?;
    candidates.sort();
    let (kind, registration_type): (LearningOfferingKind, Option<ActivityRegistrationType>) =
        DbSession::new(&mut **transaction)
            .query_as(
                "academic_delivery.services.groups.roster_source",
                r#"SELECT offering.kind,
                      CASE WHEN offering.kind = 'activity'
                           THEN detail.registration_type ELSE NULL END
               FROM learning_offerings offering
               LEFT JOIN activity_offering_details detail
                 ON detail.learning_offering_id = offering.id
               WHERE offering.id = $1"#,
            )
            .bind(group.learning_offering_id)
            .fetch_one()
            .await?;
    let registration_type = match kind {
        LearningOfferingKind::Course => None,
        LearningOfferingKind::Activity => registration_type,
    };
    let hash = stable_hash(&RosterSourceHashInput {
        learning_group_id: group.id,
        academic_term_id: group.academic_term_id,
        academic_year_id: group.academic_year_id,
        registration_type,
        homeroom_ids: &homeroom_ids,
        candidates: &candidates,
    })?;
    Ok(RosterSource {
        hash,
        candidates,
        registration_type,
    })
}

async fn current_roster_students(
    transaction: &mut Transaction<'_, Postgres>,
    group_id: Uuid,
) -> Result<Vec<LearningGroupStudentRow>, AppError> {
    Ok(DbSession::new(&mut **transaction)
        .query_as(
            "academic_delivery.services.groups.current_roster_students",
            r#"SELECT id, learning_group_id, student_academic_year_id, student_id,
                  membership_status, roster_source, joined_at, left_at,
                  published_at, row_version
           FROM learning_group_students
           WHERE learning_group_id = $1 AND membership_status = 'active'
           ORDER BY student_academic_year_id, id"#,
        )
        .bind(group_id)
        .fetch_all()
        .await?)
}

fn build_roster_preview(
    group_id: Uuid,
    source: RosterSource,
    current: Vec<LearningGroupStudentRow>,
) -> RosterPreview {
    let candidate_map: HashMap<Uuid, Uuid> = source
        .candidates
        .iter()
        .map(|value| (value.student_academic_year_id, value.student_id))
        .collect();
    let current_map: HashMap<Uuid, Uuid> = current
        .iter()
        .map(|value| (value.student_academic_year_id, value.student_id))
        .collect();
    let self_registration =
        source.registration_type == Some(ActivityRegistrationType::SelfRegistration);
    let students_by_year: BTreeMap<Uuid, Uuid> = current_map
        .iter()
        .chain(candidate_map.iter())
        .map(|(&year_id, &student_id)| (year_id, student_id))
        .collect();
    let mut students = Vec::with_capacity(students_by_year.len());
    for (student_year_id, student_id) in students_by_year {
        let currently_active = current_map.contains_key(&student_year_id);
        let proposed_active = if self_registration {
            currently_active
        } else {
            candidate_map.contains_key(&student_year_id)
        };
        students.push(RosterPreviewStudent {
            student_academic_year_id: student_year_id,
            student_id,
            student_code: None,
            display_name: String::new(),
            grade_level_name: String::new(),
            homeroom_name: None,
            proposed_active,
            currently_active,
            conflict_reason: None,
        });
    }
    RosterPreview {
        learning_group_id: group_id,
        source_hash: source.hash,
        added: students
            .iter()
            .filter(|student| student.proposed_active && !student.currently_active)
            .count(),
        removed: students
            .iter()
            .filter(|student| !student.proposed_active && student.currently_active)
            .count(),
        retained: students
            .iter()
            .filter(|student| student.proposed_active && student.currently_active)
            .count(),
        conflicts: students
            .iter()
            .filter(|student| student.conflict_reason.is_some())
            .count(),
        students,
    }
}

async fn enrich_roster_preview(
    transaction: &mut Transaction<'_, Postgres>,
    preview: &mut RosterPreview,
) -> Result<(), AppError> {
    if preview.students.is_empty() {
        return Ok(());
    }
    let student_year_ids: Vec<Uuid> = preview
        .students
        .iter()
        .map(|student| student.student_academic_year_id)
        .collect();
    let rows: Vec<RosterDisplayRow> = DbSession::new(&mut **transaction)
        .query_as(
            "academic_delivery.services.groups.enrich_roster_preview",
            r#"
        SELECT student_year.id AS student_academic_year_id,
               info.student_id AS student_code,
               student.display_name AS display_name,
               grade.level_type, grade.year AS grade_year,
               homeroom.name AS homeroom_name
        FROM student_academic_years student_year
        JOIN user_display_names student ON student.id = student_year.student_id
        LEFT JOIN student_info info ON info.user_id = student.id
        JOIN grade_levels grade ON grade.id = student_year.grade_level_id
        LEFT JOIN LATERAL (
            SELECT placement.homeroom_id
            FROM homeroom_placements placement
            WHERE placement.student_academic_year_id = student_year.id
              AND placement.academic_year_id = student_year.academic_year_id
              AND placement.status IN ('current', 'planned')
              AND placement.end_date IS NULL
            ORDER BY (placement.status = 'current') DESC,
                     placement.start_date DESC, placement.id
            LIMIT 1
        ) placement ON true
        LEFT JOIN homerooms homeroom ON homeroom.id = placement.homeroom_id
        WHERE student_year.id = ANY($1)
        ORDER BY student_year.id
        "#,
        )
        .bind(&student_year_ids)
        .fetch_all()
        .await?;
    if rows.len() != student_year_ids.len() {
        return Err(AppError::InternalServerError(
            "ไม่สามารถแสดงข้อมูลนักเรียนใน roster ได้ครบถ้วน".to_string(),
        ));
    }
    let mut display_by_student_year: HashMap<Uuid, RosterDisplayRow> = rows
        .into_iter()
        .map(|row| (row.student_academic_year_id, row))
        .collect();
    for student in &mut preview.students {
        let display = display_by_student_year
            .remove(&student.student_academic_year_id)
            .ok_or_else(|| {
                AppError::InternalServerError("ไม่สามารถจับคู่ข้อมูลนักเรียนใน roster ได้".to_string())
            })?;
        student.student_code = display.student_code;
        student.display_name = display.display_name;
        student.grade_level_name = grade_level_short_name(&display.level_type, display.grade_year);
        student.homeroom_name = display.homeroom_name;
    }
    Ok(())
}

fn grade_level_short_name(level_type: &str, year: i32) -> String {
    match level_type {
        "kindergarten" => format!("อ.{year}"),
        "primary" => format!("ป.{year}"),
        "secondary" => format!("ม.{year}"),
        _ => format!("ระดับ {year}"),
    }
}

async fn validate_manual_student_year(
    transaction: &mut Transaction<'_, Postgres>,
    student_year_id: Uuid,
    academic_year_id: Uuid,
) -> Result<Uuid, AppError> {
    DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.groups.validate_manual_student_year",
            "SELECT student_id FROM student_academic_years WHERE id = $1 \
         AND academic_year_id = $2 AND status IN ('planned', 'active')",
        )
        .bind(student_year_id)
        .bind(academic_year_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::ValidationError("นักเรียนที่เพิ่มต้องอยู่ในปีการศึกษาเดียวกัน".to_string()))
}

async fn validate_publishable_roster(
    transaction: &mut Transaction<'_, Postgres>,
    group: &GroupLockRow,
) -> Result<(), AppError> {
    let active_count: i64 = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.groups.validate_publishable_roster",
            "SELECT count(*) FROM learning_group_students \
         WHERE learning_group_id = $1 AND membership_status = 'active'",
        )
        .bind(group.id)
        .fetch_one()
        .await?;
    let activity_capacity: Option<i32> = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.groups.validate_publishable_roster",
            "SELECT capacity FROM activity_offering_details WHERE learning_offering_id = $1",
        )
        .bind(group.learning_offering_id)
        .fetch_optional()
        .await?
        .flatten();
    let effective_capacity = match (group.capacity, activity_capacity) {
        (Some(group_capacity), Some(activity_capacity)) => {
            Some(group_capacity.min(activity_capacity))
        }
        (Some(capacity), None) | (None, Some(capacity)) => Some(capacity),
        (None, None) => None,
    };
    if effective_capacity.is_some_and(|capacity| active_count > i64::from(capacity)) {
        return Err(AppError::ValidationError(
            "จำนวนนักเรียนเกินความจุของกลุ่มหรือกิจกรรม".to_string(),
        ));
    }
    let invalid_count: i64 = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.groups.validate_publishable_roster",
            r#"SELECT count(*) FROM learning_group_students member
           JOIN student_academic_years student_year
             ON student_year.id = member.student_academic_year_id
           WHERE member.learning_group_id = $1 AND member.membership_status = 'active'
             AND (student_year.academic_year_id <> $2
                  OR student_year.status NOT IN ('planned', 'active'))"#,
        )
        .bind(group.id)
        .bind(group.academic_year_id)
        .fetch_one()
        .await?;
    if invalid_count != 0 {
        return Err(AppError::ValidationError(
            "roster มีนักเรียนที่ไม่อยู่ในปีการศึกษานี้".to_string(),
        ));
    }
    Ok(())
}

async fn append_group_audit(
    connection: &mut sqlx::PgConnection,
    actor_user_id: Uuid,
    group: &GroupLockRow,
    event_code: &str,
) -> Result<(), AppError> {
    append_audit(
        connection,
        event_code,
        "learning_group",
        group.id,
        group.academic_year_id,
        group.academic_term_id,
        actor_user_id,
        serde_json::json!({ "learningOfferingId": group.learning_offering_id }),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview_current(year: u128, student: u128) -> LearningGroupStudentRow {
        LearningGroupStudentRow {
            id: Uuid::new_v4(),
            learning_group_id: Uuid::new_v4(),
            student_academic_year_id: Uuid::from_u128(year),
            student_id: Uuid::from_u128(student),
            membership_status: crate::models::MembershipStatus::Active,
            roster_source: "manual".into(),
            joined_at: chrono::NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            left_at: None,
            published_at: None,
            row_version: 1,
        }
    }

    #[test]
    fn roster_preview_union_preserves_all_identities_candidate_precedence_and_order() {
        for registration_type in [
            Some(ActivityRegistrationType::Assigned),
            Some(ActivityRegistrationType::SelfRegistration),
        ] {
            let preview = build_roster_preview(
                Uuid::new_v4(),
                RosterSource {
                    hash: "source".into(),
                    candidates: vec![
                        RosterSourceStudent {
                            student_academic_year_id: Uuid::from_u128(3),
                            student_id: Uuid::from_u128(30),
                        },
                        RosterSourceStudent {
                            student_academic_year_id: Uuid::from_u128(2),
                            student_id: Uuid::from_u128(20),
                        },
                    ],
                    registration_type,
                },
                vec![preview_current(2, 99), preview_current(1, 10)],
            );
            assert_eq!(preview.source_hash, "source");
            assert_eq!(
                preview
                    .students
                    .iter()
                    .map(|s| (s.student_academic_year_id, s.student_id))
                    .collect::<Vec<_>>(),
                vec![
                    (Uuid::from_u128(1), Uuid::from_u128(10)),
                    (Uuid::from_u128(2), Uuid::from_u128(20)),
                    (Uuid::from_u128(3), Uuid::from_u128(30))
                ]
            );
            assert_eq!(
                preview.retained,
                if registration_type == Some(ActivityRegistrationType::SelfRegistration) {
                    2
                } else {
                    1
                }
            );
            assert_eq!(
                (preview.added, preview.removed),
                if registration_type == Some(ActivityRegistrationType::SelfRegistration) {
                    (0, 0)
                } else {
                    (1, 1)
                }
            );
            assert_eq!(preview.conflicts, 0);
        }
    }

    #[test]
    fn roster_preview_handles_empty_and_current_only_unions_without_dropping_students() {
        let source = || RosterSource {
            hash: "source".into(),
            candidates: Vec::new(),
            registration_type: None,
        };
        assert!(build_roster_preview(Uuid::new_v4(), source(), Vec::new())
            .students
            .is_empty());
        let preview = build_roster_preview(Uuid::new_v4(), source(), vec![preview_current(1, 10)]);
        assert_eq!(preview.students.len(), 1);
        assert_eq!(preview.students[0].student_id, Uuid::from_u128(10));
        assert!(preview.students[0].currently_active);
        assert!(!preview.students[0].proposed_active);
        assert_eq!(preview.removed, 1);
    }

    #[test]
    fn internal_group_code_is_stable_across_terms_without_user_input() {
        let first = internal_group_code("  กลุ่ม ห้อง 1 ").expect("code must be generated");
        let same_name = internal_group_code("กลุ่ม ห้อง 1").expect("code must be generated");
        let other_name = internal_group_code("กลุ่ม ห้อง 2").expect("code must be generated");

        assert_eq!(first, same_name);
        assert_ne!(first, other_name);
        assert!(first.starts_with("GROUP-"));
    }
}
