use std::collections::{HashMap, HashSet};

use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::policies::resource_access_policy::AcademicResourceListFilter;

use super::super::models::{
    ActivityRegistrationType, ApplyRosterRequest, CreateLearningGroupRequest,
    CurriculumGroupProposal, LearningGroup, LearningGroupRow, LearningGroupStudent,
    LearningGroupStudentRow, LearningGroupTeacherAssignment, LearningOfferingKind,
    LearningOfferingStatus, PublishRosterRequest, ReplaceLearningGroupHomeroomsRequest,
    ReplaceLearningGroupTeachersRequest, RosterOverrideAction, RosterPreview, RosterPreviewStudent,
    RosterStatus, TeacherAssignmentInput, UpdateLearningGroupRequest,
};
use super::{append_audit, require_writable_term, stable_hash, validate_row_version};

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

    if let Some((id, row_version)) = sqlx::query_as::<_, (Uuid, i64)>(
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
    .fetch_optional(&mut **transaction)
    .await?
    {
        let mut existing_homeroom_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT homeroom_id FROM learning_group_homerooms \
             WHERE learning_group_id = $1 ORDER BY homeroom_id",
        )
        .bind(id)
        .fetch_all(&mut **transaction)
        .await?;
        existing_homeroom_ids.sort_unstable();
        if existing_homeroom_ids != homeroom_ids {
            return Err(AppError::Conflict(format!(
                "กลุ่มที่ระบบเคยเตรียมไว้ถูกปรับแต่งแล้ว (rowVersion {row_version}) กรุณาตรวจกลุ่มด้วยตนเอง"
            )));
        }
        return Ok(GeneratedGroupApplyOutcome { id, created: false });
    }

    let group_id = Uuid::new_v4();
    let code_suffix = proposal.group_key.chars().take(10).collect::<String>();
    sqlx::query(
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
    .execute(&mut **transaction)
    .await?;
    for homeroom_id in homeroom_ids {
        sqlx::query(
            r#"INSERT INTO learning_group_homerooms (
                   id, learning_group_id, academic_term_id, academic_year_id,
                   homeroom_id, coverage_source
               ) VALUES ($1, $2, $3, $4, $5, 'curriculum_prepare')"#,
        )
        .bind(Uuid::new_v4())
        .bind(group_id)
        .bind(academic_term_id)
        .bind(academic_year_id)
        .bind(homeroom_id)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(GeneratedGroupApplyOutcome {
        id: group_id,
        created: true,
    })
}

pub async fn list(pool: &PgPool, offering_id: Uuid) -> Result<Vec<LearningGroup>, AppError> {
    let sql = format!(
        "SELECT {GROUP_COLUMNS} FROM learning_groups WHERE learning_offering_id = $1 \
         ORDER BY code, id LIMIT 500"
    );
    let rows: Vec<LearningGroupRow> = sqlx::query_as(&sql)
        .bind(offering_id)
        .fetch_all(pool)
        .await?;
    hydrate_many(pool, rows).await
}

pub async fn list_for_term(
    pool: &PgPool,
    academic_term_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<LearningGroup>, AppError> {
    let owner_ids = filter.allowed_organization_unit_ids();
    let sql = format!(
        "SELECT {QUALIFIED_GROUP_COLUMNS} \
         FROM learning_groups learning_group \
         JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id \
         WHERE learning_group.academic_term_id = $1 \
           AND ($2 OR offering.owning_organization_unit_id = ANY($3) OR EXISTS (\
               SELECT 1 FROM learning_groups assigned_group \
               JOIN learning_group_teachers teacher \
                 ON teacher.learning_group_id = assigned_group.id \
               WHERE assigned_group.learning_offering_id = offering.id \
                 AND teacher.teacher_id = $4\
           )) \
         ORDER BY learning_group.code, learning_group.id \
         LIMIT $5"
    );
    let rows: Vec<LearningGroupRow> = sqlx::query_as(&sql)
        .bind(academic_term_id)
        .bind(filter.includes_school_owned)
        .bind(owner_ids)
        .bind(filter.assigned_actor_id)
        .bind((MAX_TERM_GROUPS + 1) as i64)
        .fetch_all(pool)
        .await?;
    if rows.len() > MAX_TERM_GROUPS {
        return Err(AppError::ValidationError(
            "จำนวนกลุ่มเรียนในภาคเรียนเกิน 2000 กลุ่ม กรุณาแบ่งข้อมูลก่อนเปิดพื้นที่ทำงาน".to_string(),
        ));
    }
    hydrate_many(pool, rows).await
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<LearningGroup, AppError> {
    let sql = format!("SELECT {GROUP_COLUMNS} FROM learning_groups WHERE id = $1");
    let row: LearningGroupRow = sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()))?;
    hydrate(pool, row).await
}

pub async fn create(
    pool: &PgPool,
    actor_user_id: Uuid,
    offering_id: Uuid,
    request: CreateLearningGroupRequest,
) -> Result<LearningGroup, AppError> {
    validate_group_fields(&request.code, &request.name, request.capacity)?;
    let mut transaction = pool.begin().await?;
    let (term_id, year_id, offering_status): (Uuid, Uuid, LearningOfferingStatus) = sqlx::query_as(
        "SELECT academic_term_id, academic_year_id, status \
             FROM learning_offerings WHERE id = $1 FOR UPDATE",
    )
    .bind(offering_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายการเปิดสอน".to_string()))?;
    if matches!(
        offering_status,
        LearningOfferingStatus::Cancelled | LearningOfferingStatus::Closed
    ) {
        return Err(AppError::Conflict("รายการเปิดสอนปิดแล้ว".to_string()));
    }
    require_writable_term(&mut transaction, term_id, false).await?;
    ensure_unique_group_code(&mut transaction, offering_id, None, &request.code).await?;
    validate_preferred_rooms(&mut transaction, &request.preferred_room_ids).await?;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO learning_groups (
               id, learning_offering_id, academic_term_id, academic_year_id,
               code, name, description, capacity, status, roster_status
           ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'draft', 'draft')"#,
    )
    .bind(id)
    .bind(offering_id)
    .bind(term_id)
    .bind(year_id)
    .bind(request.code.trim().to_uppercase())
    .bind(request.name.trim())
    .bind(request.description)
    .bind(request.capacity)
    .execute(&mut *transaction)
    .await?;
    replace_preferred_rooms_in_transaction(
        &mut transaction,
        id,
        term_id,
        year_id,
        &request.preferred_room_ids,
    )
    .await?;
    crate::modules::academic::services::timetable_block_sync::retry_sync_for_group_in_tx(
        &mut transaction,
        id,
        actor_user_id,
    )
    .await?;
    transaction.commit().await?;
    append_audit(
        pool,
        "learning_group.created",
        "learning_group",
        id,
        year_id,
        term_id,
        actor_user_id,
        serde_json::json!({ "learningOfferingId": offering_id }),
    )
    .await?;
    get(pool, id).await
}

pub async fn update(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: UpdateLearningGroupRequest,
) -> Result<LearningGroup, AppError> {
    validate_row_version(request.row_version)?;
    validate_group_fields(&request.code, &request.name, request.capacity)?;
    let mut transaction = pool.begin().await?;
    let group = lock_group(&mut transaction, id).await?;
    require_mutable_group(&group, request.row_version, false)?;
    require_writable_term(&mut transaction, group.academic_term_id, false).await?;
    ensure_unique_group_code(
        &mut transaction,
        group.learning_offering_id,
        Some(id),
        &request.code,
    )
    .await?;
    validate_preferred_rooms(&mut transaction, &request.preferred_room_ids).await?;
    sqlx::query(
        "UPDATE learning_groups SET code = $1, name = $2, description = $3, capacity = $4, \
         row_version = row_version + 1, updated_at = now() WHERE id = $5",
    )
    .bind(request.code.trim().to_uppercase())
    .bind(request.name.trim())
    .bind(request.description)
    .bind(request.capacity)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    replace_preferred_rooms_in_transaction(
        &mut transaction,
        id,
        group.academic_term_id,
        group.academic_year_id,
        &request.preferred_room_ids,
    )
    .await?;
    crate::modules::academic::services::timetable_block_sync::retry_sync_for_group_in_tx(
        &mut transaction,
        id,
        actor_user_id,
    )
    .await?;
    transaction.commit().await?;
    append_group_audit(pool, actor_user_id, &group, "learning_group.updated").await?;
    get(pool, id).await
}

pub async fn replace_teachers(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: ReplaceLearningGroupTeachersRequest,
) -> Result<LearningGroup, AppError> {
    validate_row_version(request.row_version)?;
    let teacher_ids = unique_teacher_ids(&request.teachers)?;
    let mut transaction = pool.begin().await?;
    let group = lock_group(&mut transaction, id).await?;
    require_mutable_group(&group, request.row_version, false)?;
    require_draft_group_teachers(&group)?;
    require_writable_term(&mut transaction, group.academic_term_id, false).await?;
    if !teacher_ids.is_empty() {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM users WHERE id = ANY($1) \
             AND user_type = 'staff' AND status = 'active'",
        )
        .bind(&teacher_ids)
        .fetch_one(&mut *transaction)
        .await?;
        if count != teacher_ids.len() as i64 {
            return Err(AppError::ValidationError(
                "ครูผู้สอนต้องเป็นบุคลากรที่ใช้งานอยู่".to_string(),
            ));
        }
    }
    let offering_starts_on: chrono::NaiveDate =
        sqlx::query_scalar("SELECT starts_on FROM learning_offerings WHERE id = $1")
            .bind(group.learning_offering_id)
            .fetch_one(&mut *transaction)
            .await?;
    sqlx::query("DELETE FROM learning_group_teachers WHERE learning_group_id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    for teacher in request.teachers {
        sqlx::query(
            r#"INSERT INTO learning_group_teachers (
                   id, learning_group_id, academic_term_id, academic_year_id,
                   teacher_id, role, starts_on, created_by, updated_by
               ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)"#,
        )
        .bind(Uuid::new_v4())
        .bind(id)
        .bind(group.academic_term_id)
        .bind(group.academic_year_id)
        .bind(teacher.teacher_id)
        .bind(teacher.role)
        .bind(offering_starts_on)
        .bind(actor_user_id)
        .execute(&mut *transaction)
        .await?;
    }
    increment_group_revision(&mut transaction, id).await?;
    crate::modules::academic::services::timetable_block_sync::retry_sync_for_group_in_tx(
        &mut transaction,
        id,
        actor_user_id,
    )
    .await?;
    transaction.commit().await?;
    append_group_audit(
        pool,
        actor_user_id,
        &group,
        "learning_group.teachers_replaced",
    )
    .await?;
    get(pool, id).await
}

pub async fn replace_homerooms(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: ReplaceLearningGroupHomeroomsRequest,
) -> Result<LearningGroup, AppError> {
    validate_row_version(request.row_version)?;
    let homeroom_ids = unique_ids(&request.homeroom_ids, "ห้องเรียนซ้ำกัน")?;
    let mut transaction = pool.begin().await?;
    let group = lock_group(&mut transaction, id).await?;
    require_mutable_group(&group, request.row_version, true)?;
    require_writable_term(&mut transaction, group.academic_term_id, false).await?;
    validate_homeroom_coverage(&mut transaction, &group, &homeroom_ids).await?;
    sqlx::query("DELETE FROM learning_group_homerooms WHERE learning_group_id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    for homeroom_id in homeroom_ids {
        sqlx::query(
            r#"INSERT INTO learning_group_homerooms (
                   id, learning_group_id, academic_term_id, academic_year_id,
                   homeroom_id, coverage_source
               ) VALUES ($1, $2, $3, $4, $5, 'manual')"#,
        )
        .bind(Uuid::new_v4())
        .bind(id)
        .bind(group.academic_term_id)
        .bind(group.academic_year_id)
        .bind(homeroom_id)
        .execute(&mut *transaction)
        .await?;
    }
    sqlx::query(
        "UPDATE learning_groups SET roster_source_hash = NULL, \
         row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    crate::modules::academic::services::timetable_block_sync::retry_sync_for_group_in_tx(
        &mut transaction,
        id,
        actor_user_id,
    )
    .await?;
    transaction.commit().await?;
    append_group_audit(
        pool,
        actor_user_id,
        &group,
        "learning_group.homerooms_replaced",
    )
    .await?;
    get(pool, id).await
}

pub async fn list_students(
    pool: &PgPool,
    group_id: Uuid,
) -> Result<Vec<LearningGroupStudent>, AppError> {
    get(pool, group_id).await?;
    let rows: Vec<LearningGroupStudentRow> = sqlx::query_as(
        r#"SELECT id, learning_group_id, student_academic_year_id, student_id,
                  membership_status, roster_source, joined_at, left_at,
                  published_at, row_version
           FROM learning_group_students WHERE learning_group_id = $1
           ORDER BY student_id, joined_at, id"#,
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn preview_roster(pool: &PgPool, group_id: Uuid) -> Result<RosterPreview, AppError> {
    let mut transaction = pool.begin().await?;
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
    let mut transaction = pool.begin().await?;
    let group = lock_group(&mut transaction, group_id).await?;
    require_mutable_group(&group, request.row_version, true)?;
    let term = require_writable_term(&mut transaction, group.academic_term_id, false).await?;
    let source = roster_source(&mut transaction, &group).await?;
    if source.hash != request.source_hash {
        return Err(AppError::Conflict(
            "รายชื่อนักเรียนต้นทางเปลี่ยนไป กรุณาโหลด preview ใหม่".to_string(),
        ));
    }
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
            sqlx::query(
                "UPDATE learning_group_students SET membership_status = 'removed', \
                 left_at = GREATEST(joined_at, $1), row_version = row_version + 1, \
                 updated_at = now() WHERE id = $2 AND membership_status = 'active'",
            )
            .bind(term.start_date)
            .bind(current_student.id)
            .execute(&mut *transaction)
            .await?;
        }
    }
    for (student_year_id, (student_id, roster_source)) in desired {
        let active_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM learning_group_students \
             WHERE learning_group_id = $1 AND student_academic_year_id = $2 \
               AND membership_status = 'active')",
        )
        .bind(group_id)
        .bind(student_year_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !active_exists {
            sqlx::query(
                r#"INSERT INTO learning_group_students (
                       id, learning_group_id, academic_term_id, academic_year_id,
                       student_academic_year_id, student_id, membership_status,
                       roster_source, joined_at
                   ) VALUES ($1, $2, $3, $4, $5, $6, 'active', $7, $8)"#,
            )
            .bind(Uuid::new_v4())
            .bind(group_id)
            .bind(group.academic_term_id)
            .bind(group.academic_year_id)
            .bind(student_year_id)
            .bind(student_id)
            .bind(roster_source)
            .bind(term.start_date)
            .execute(&mut *transaction)
            .await?;
        }
    }
    sqlx::query(
        "UPDATE learning_groups SET roster_status = 'draft', roster_source_hash = $1, \
         roster_published_at = NULL, roster_publish_idempotency_key = NULL, \
         row_version = row_version + 1, updated_at = now() WHERE id = $2",
    )
    .bind(&source.hash)
    .bind(group_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    append_group_audit(pool, actor_user_id, &group, "learning_group.roster_applied").await?;
    get(pool, group_id).await
}

pub async fn publish_roster(
    pool: &PgPool,
    actor_user_id: Uuid,
    group_id: Uuid,
    request: PublishRosterRequest,
) -> Result<LearningGroup, AppError> {
    validate_row_version(request.row_version)?;
    if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM learning_groups WHERE roster_publish_idempotency_key = $1",
    )
    .bind(request.idempotency_key)
    .fetch_optional(pool)
    .await?
    {
        if existing_id == group_id {
            return get(pool, group_id).await;
        }
        return Err(AppError::Conflict(
            "idempotencyKey ถูกใช้กับกลุ่มอื่นแล้ว".to_string(),
        ));
    }
    let mut transaction = pool.begin().await?;
    let group = lock_group(&mut transaction, group_id).await?;
    require_mutable_group(&group, request.row_version, false)?;
    if group.roster_status != RosterStatus::Draft {
        return Err(AppError::Conflict(
            "roster นี้เผยแพร่แล้ว หากต้องการเปลี่ยนรายชื่อต้องสร้างฉบับ draft ใหม่".to_string(),
        ));
    }
    require_writable_term(&mut transaction, group.academic_term_id, false).await?;
    let offering_status: LearningOfferingStatus =
        sqlx::query_scalar("SELECT status FROM learning_offerings WHERE id = $1 FOR SHARE")
            .bind(group.learning_offering_id)
            .fetch_one(&mut *transaction)
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
    sqlx::query(
        "UPDATE learning_group_students SET published_at = now(), updated_at = now() \
         WHERE learning_group_id = $1 AND membership_status = 'active'",
    )
    .bind(group_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE learning_groups SET roster_status = 'published', roster_published_at = now(), \
         roster_publish_idempotency_key = $1, row_version = row_version + 1, \
         updated_at = now() WHERE id = $2",
    )
    .bind(request.idempotency_key)
    .bind(group_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    append_group_audit(
        pool,
        actor_user_id,
        &group,
        "learning_group.roster_published",
    )
    .await?;
    get(pool, group_id).await
}

async fn hydrate(pool: &PgPool, row: LearningGroupRow) -> Result<LearningGroup, AppError> {
    hydrate_many(pool, vec![row])
        .await?
        .pop()
        .ok_or_else(|| AppError::InternalServerError("ไม่สามารถประกอบข้อมูลกลุ่มเรียนได้".to_string()))
}

async fn hydrate_many(
    pool: &PgPool,
    rows: Vec<LearningGroupRow>,
) -> Result<Vec<LearningGroup>, AppError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let group_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let teacher_rows: Vec<GroupTeacherRow> = sqlx::query_as(
        r#"SELECT teacher.learning_group_id, teacher.id, teacher.teacher_id,
                  concat_ws(' ',
                      nullif(concat(coalesce(user_account.title, ''), user_account.first_name), ''),
                      nullif(user_account.last_name, '')) AS display_name,
                  teacher.role, teacher.starts_on, teacher.ends_on, teacher.row_version
           FROM learning_group_teachers teacher
           JOIN users user_account ON user_account.id = teacher.teacher_id
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
    .fetch_all(pool)
    .await?;
    let homeroom_rows: Vec<GroupHomeroomRow> = sqlx::query_as(
        "SELECT learning_group_id, homeroom_id FROM learning_group_homerooms \
         WHERE learning_group_id = ANY($1) \
         ORDER BY learning_group_id, homeroom_id",
    )
    .bind(&group_ids)
    .fetch_all(pool)
    .await?;
    let preferred_room_rows: Vec<GroupPreferredRoomRow> = sqlx::query_as(
        "SELECT learning_group_id, room_id FROM learning_group_preferred_rooms \
         WHERE learning_group_id = ANY($1) \
         ORDER BY learning_group_id, rank, room_id",
    )
    .bind(&group_ids)
    .fetch_all(pool)
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
    sqlx::query_as(
        "SELECT id, learning_offering_id, academic_term_id, academic_year_id, \
         status, roster_status, row_version, roster_source_hash::text, capacity \
         FROM learning_groups WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut **transaction)
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

fn validate_group_fields(code: &str, name: &str, capacity: Option<i32>) -> Result<(), AppError> {
    if code.trim().is_empty() || name.trim().is_empty() {
        return Err(AppError::ValidationError(
            "รหัสและชื่อกลุ่มเรียนห้ามว่าง".to_string(),
        ));
    }
    if capacity.is_some_and(|value| value <= 0) {
        return Err(AppError::ValidationError("ความจุต้องมากกว่าศูนย์".to_string()));
    }
    Ok(())
}

async fn ensure_unique_group_code(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    except_id: Option<Uuid>,
    code: &str,
) -> Result<(), AppError> {
    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM learning_groups \
         WHERE learning_offering_id = $1 AND lower(btrim(code)) = lower(btrim($2)) \
           AND ($3::uuid IS NULL OR id <> $3))",
    )
    .bind(offering_id)
    .bind(code)
    .bind(except_id)
    .fetch_one(&mut **transaction)
    .await?;
    if duplicate {
        Err(AppError::Conflict(
            "รหัสกลุ่มเรียนซ้ำในรายการเปิดสอนนี้".to_string(),
        ))
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
        let valid: bool = sqlx::query_scalar(
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
        .fetch_one(&mut **transaction)
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
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM rooms WHERE id = ANY($1) AND status = 'ACTIVE'")
            .bind(&room_ids)
            .fetch_one(&mut **transaction)
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
    sqlx::query("DELETE FROM learning_group_preferred_rooms WHERE learning_group_id = $1")
        .bind(group_id)
        .execute(&mut **transaction)
        .await?;
    for (index, room_id) in room_ids.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO learning_group_preferred_rooms (
                   learning_group_id, academic_term_id, academic_year_id, room_id, rank
               ) VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(group_id)
        .bind(term_id)
        .bind(year_id)
        .bind(room_id)
        .bind(
            i32::try_from(index + 1)
                .map_err(|_| AppError::ValidationError("จำนวนห้องที่ต้องการใช้มากเกินไป".to_string()))?,
        )
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn increment_group_revision(
    transaction: &mut Transaction<'_, Postgres>,
    group_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE learning_groups SET row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(group_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn roster_source(
    transaction: &mut Transaction<'_, Postgres>,
    group: &GroupLockRow,
) -> Result<RosterSource, AppError> {
    let mut homeroom_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT homeroom_id FROM learning_group_homerooms \
         WHERE learning_group_id = $1 ORDER BY homeroom_id",
    )
    .bind(group.id)
    .fetch_all(&mut **transaction)
    .await?;
    homeroom_ids.sort_unstable();
    let mut candidates: Vec<RosterSourceStudent> = sqlx::query_as(
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
    .fetch_all(&mut **transaction)
    .await?;
    candidates.sort();
    let (kind, registration_type): (LearningOfferingKind, Option<ActivityRegistrationType>) =
        sqlx::query_as(
            r#"SELECT offering.kind,
                      CASE WHEN offering.kind = 'activity'
                           THEN detail.registration_type ELSE NULL END
               FROM learning_offerings offering
               LEFT JOIN activity_offering_details detail
                 ON detail.learning_offering_id = offering.id
               WHERE offering.id = $1"#,
        )
        .bind(group.learning_offering_id)
        .fetch_one(&mut **transaction)
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
    Ok(sqlx::query_as(
        r#"SELECT id, learning_group_id, student_academic_year_id, student_id,
                  membership_status, roster_source, joined_at, left_at,
                  published_at, row_version
           FROM learning_group_students
           WHERE learning_group_id = $1 AND membership_status = 'active'
           ORDER BY student_academic_year_id, id"#,
    )
    .bind(group_id)
    .fetch_all(&mut **transaction)
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
    let mut ids: Vec<Uuid> = candidate_map
        .keys()
        .chain(current_map.keys())
        .copied()
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let mut students = Vec::with_capacity(ids.len());
    for student_year_id in ids {
        let currently_active = current_map.contains_key(&student_year_id);
        let proposed_active = if self_registration {
            currently_active
        } else {
            candidate_map.contains_key(&student_year_id)
        };
        let student_id = candidate_map
            .get(&student_year_id)
            .or_else(|| current_map.get(&student_year_id))
            .copied()
            .expect("roster preview union must resolve student id");
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
    let rows: Vec<RosterDisplayRow> = sqlx::query_as(
        r#"
        SELECT student_year.id AS student_academic_year_id,
               info.student_id AS student_code,
               concat_ws(' ', nullif(btrim(student.title), ''),
                                student.first_name, student.last_name) AS display_name,
               grade.level_type, grade.year AS grade_year,
               homeroom.name AS homeroom_name
        FROM student_academic_years student_year
        JOIN users student ON student.id = student_year.student_id
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
    .fetch_all(&mut **transaction)
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
    sqlx::query_scalar(
        "SELECT student_id FROM student_academic_years WHERE id = $1 \
         AND academic_year_id = $2 AND status IN ('planned', 'active')",
    )
    .bind(student_year_id)
    .bind(academic_year_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::ValidationError("นักเรียนที่เพิ่มต้องอยู่ในปีการศึกษาเดียวกัน".to_string()))
}

async fn validate_publishable_roster(
    transaction: &mut Transaction<'_, Postgres>,
    group: &GroupLockRow,
) -> Result<(), AppError> {
    let active_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_group_students \
         WHERE learning_group_id = $1 AND membership_status = 'active'",
    )
    .bind(group.id)
    .fetch_one(&mut **transaction)
    .await?;
    let activity_capacity: Option<i32> = sqlx::query_scalar(
        "SELECT capacity FROM activity_offering_details WHERE learning_offering_id = $1",
    )
    .bind(group.learning_offering_id)
    .fetch_optional(&mut **transaction)
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
    let invalid_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM learning_group_students member
           JOIN student_academic_years student_year
             ON student_year.id = member.student_academic_year_id
           WHERE member.learning_group_id = $1 AND member.membership_status = 'active'
             AND (student_year.academic_year_id <> $2
                  OR student_year.status NOT IN ('planned', 'active'))"#,
    )
    .bind(group.id)
    .bind(group.academic_year_id)
    .fetch_one(&mut **transaction)
    .await?;
    if invalid_count != 0 {
        return Err(AppError::ValidationError(
            "roster มีนักเรียนที่ไม่อยู่ในปีการศึกษานี้".to_string(),
        ));
    }
    Ok(())
}

async fn append_group_audit(
    pool: &PgPool,
    actor_user_id: Uuid,
    group: &GroupLockRow,
    event_code: &str,
) -> Result<(), AppError> {
    append_audit(
        pool,
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
