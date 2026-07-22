use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::academic::models::activity::*;
use crate::policies::activity_access_policy;
use crate::policies::resource_access_policy::UserResourceListAccess;
use chrono::{DateTime, Utc};
use sqlx::{types::Json, FromRow, PgPool};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

fn activity_datetime_from_rfc3339(value: Option<&str>) -> Option<chrono::DateTime<Utc>> {
    value
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
}

fn optional_uuid_list_json(ids: Option<&[Uuid]>) -> Option<Json<Vec<Uuid>>> {
    ids.map(|ids| Json(ids.to_vec()))
}

fn activity_actor_scope_filter(access: UserResourceListAccess) -> Option<Uuid> {
    match access {
        UserResourceListAccess::School => None,
        UserResourceListAccess::Own(actor_user_id)
        | UserResourceListAccess::Assigned(actor_user_id)
        | UserResourceListAccess::OrganizationUnit(actor_user_id)
        | UserResourceListAccess::OrganizationTree(actor_user_id) => Some(actor_user_id),
    }
}

fn unique_activity_student_ids(student_ids: &[Uuid]) -> Vec<Uuid> {
    let mut seen = HashSet::with_capacity(student_ids.len());
    student_ids
        .iter()
        .copied()
        .filter(|student_id| seen.insert(*student_id))
        .collect()
}

async fn bulk_insert_activity_group_members(
    pool: &PgPool,
    group_id: Uuid,
    student_ids: &[Uuid],
) -> Result<usize, AppError> {
    if student_ids.is_empty() {
        return Ok(0);
    }

    let result = sqlx::query(
        "INSERT INTO activity_group_members (activity_group_id, student_id)
         SELECT $1, student_id
         FROM UNNEST($2::uuid[]) AS rows(student_id)
         ON CONFLICT DO NOTHING",
    )
    .bind(group_id)
    .bind(student_ids)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(result.rows_affected() as usize)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SlotClassroomAssignmentBulkRow {
    classroom_id: Uuid,
    instructor_id: Uuid,
}

fn slot_classroom_assignment_bulk_rows(
    assignments: &[UpsertSlotClassroomAssignmentRequest],
) -> Vec<SlotClassroomAssignmentBulkRow> {
    let mut rows: Vec<SlotClassroomAssignmentBulkRow> = Vec::with_capacity(assignments.len());
    let mut index_by_classroom = HashMap::with_capacity(assignments.len());

    for assignment in assignments {
        let row = SlotClassroomAssignmentBulkRow {
            classroom_id: assignment.classroom_id,
            instructor_id: assignment.instructor_id,
        };
        if let Some(index) = index_by_classroom.get(&assignment.classroom_id).copied() {
            rows[index] = row;
        } else {
            index_by_classroom.insert(assignment.classroom_id, rows.len());
            rows.push(row);
        }
    }

    rows
}

#[derive(Debug, FromRow)]
struct ActivitySlotRow {
    id: Uuid,
    activity_catalog_id: Uuid,
    semester_id: Uuid,
    registration_type: String,
    teacher_reg_open: bool,
    student_reg_open: bool,
    student_reg_start: Option<DateTime<Utc>>,
    student_reg_end: Option<DateTime<Utc>>,
    created_by: Option<Uuid>,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    name: Option<String>,
    description: Option<String>,
    activity_type: Option<String>,
    periods_per_week: Option<i32>,
    scheduling_mode: Option<String>,
    allowed_grade_level_ids: Option<Json<Vec<Uuid>>>,
    semester_name: Option<String>,
    group_count: Option<i64>,
    total_members: Option<i64>,
    classroom_ids: Option<Vec<Uuid>>,
}

impl From<ActivitySlotRow> for ActivitySlot {
    fn from(row: ActivitySlotRow) -> Self {
        Self {
            id: row.id,
            activity_catalog_id: row.activity_catalog_id,
            semester_id: row.semester_id,
            registration_type: row.registration_type,
            teacher_reg_open: row.teacher_reg_open,
            student_reg_open: row.student_reg_open,
            student_reg_start: row.student_reg_start,
            student_reg_end: row.student_reg_end,
            created_by: row.created_by,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
            name: row.name,
            description: row.description,
            activity_type: row.activity_type,
            periods_per_week: row.periods_per_week,
            scheduling_mode: row.scheduling_mode,
            allowed_grade_level_ids: row.allowed_grade_level_ids.map(|Json(ids)| ids),
            semester_name: row.semester_name,
            group_count: row.group_count,
            total_members: row.total_members,
            classroom_ids: row.classroom_ids,
        }
    }
}

#[derive(Debug, FromRow)]
struct ActivityGroupRow {
    id: Uuid,
    slot_id: Option<Uuid>,
    name: String,
    description: Option<String>,
    instructor_id: Option<Uuid>,
    max_capacity: Option<i32>,
    registration_open: bool,
    allowed_classroom_ids: Option<Json<Vec<Uuid>>>,
    created_by: Option<Uuid>,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    instructor_name: Option<String>,
    member_count: Option<i64>,
    slot_name: Option<String>,
    activity_type: Option<String>,
    semester_name: Option<String>,
}

impl From<ActivityGroupRow> for ActivityGroup {
    fn from(row: ActivityGroupRow) -> Self {
        Self {
            id: row.id,
            slot_id: row.slot_id,
            name: row.name,
            description: row.description,
            instructor_id: row.instructor_id,
            max_capacity: row.max_capacity,
            registration_open: row.registration_open,
            allowed_classroom_ids: row.allowed_classroom_ids.map(|Json(ids)| ids),
            created_by: row.created_by,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
            instructor_name: row.instructor_name,
            member_count: row.member_count,
            slot_name: row.slot_name,
            activity_type: row.activity_type,
            semester_name: row.semester_name,
        }
    }
}

// ============================================
// Activity Slots
// ============================================

pub async fn list_slots(
    pool: &PgPool,
    filter: ActivitySlotFilter,
    access: UserResourceListAccess,
) -> Result<Vec<ActivitySlot>, AppError> {
    let mut sql = String::from(
        r#"SELECT
            s.*,
            ac.name AS name,
            ac.description AS description,
            ac.activity_type AS activity_type,
            ac.periods_per_week AS periods_per_week,
            ac.scheduling_mode AS scheduling_mode,
            ac.grade_level_ids AS allowed_grade_level_ids,
            sem.name AS semester_name,
            COUNT(DISTINCT ag.id) AS group_count,
            COUNT(DISTINCT agm.id) AS total_members,
            COALESCE(
                (SELECT array_agg(classroom_id) FROM activity_slot_classrooms WHERE slot_id = s.id),
                '{}'::uuid[]
            ) AS classroom_ids
        FROM activity_slots s
        JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
        LEFT JOIN academic_semesters sem ON sem.id = s.semester_id
        LEFT JOIN activity_groups ag ON ag.slot_id = s.id AND ag.is_active = true
        LEFT JOIN activity_group_members agm ON agm.activity_group_id = ag.id
        WHERE s.is_active = true"#,
    );

    let mut idx = 0u32;
    if filter.semester_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND s.semester_id = ${idx}"));
    }
    if filter.activity_type.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND ac.activity_type = ${idx}"));
    }
    if filter.teacher_reg_open.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND s.teacher_reg_open = ${idx}"));
    }
    if filter.student_reg_open.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND s.student_reg_open = ${idx}"));
    }
    let scoped_actor_user_id = activity_actor_scope_filter(access);
    if scoped_actor_user_id.is_some() {
        idx += 1;
        sql.push_str(&format!(
            r#" AND (
                s.teacher_reg_open = true
                OR EXISTS (
                    SELECT 1
                    FROM activity_slot_instructors asi_scope
                    WHERE asi_scope.slot_id = s.id
                      AND asi_scope.user_id = ${idx}
                )
                OR EXISTS (
                    SELECT 1
                    FROM activity_groups ag_scope
                    WHERE ag_scope.slot_id = s.id
                      AND ag_scope.is_active = true
                      AND (
                          ag_scope.instructor_id = ${idx}
                          OR EXISTS (
                              SELECT 1
                              FROM activity_group_instructors agi_scope
                              WHERE agi_scope.activity_group_id = ag_scope.id
                                AND agi_scope.instructor_id = ${idx}
                          )
                      )
                )
            )"#
        ));
    }

    sql.push_str(" GROUP BY s.id, ac.id, sem.name ORDER BY ac.activity_type, ac.name");

    let mut q = sqlx::query_as::<_, ActivitySlotRow>(&sql);
    if let Some(v) = filter.semester_id {
        q = q.bind(v);
    }
    if let Some(ref v) = filter.activity_type {
        q = q.bind(v);
    }
    if let Some(v) = filter.teacher_reg_open {
        q = q.bind(v);
    }
    if let Some(v) = filter.student_reg_open {
        q = q.bind(v);
    }
    if let Some(actor_user_id) = scoped_actor_user_id {
        q = q.bind(actor_user_id);
    }

    let rows = q.fetch_all(pool).await.map_err(|e| {
        tracing::error!("list_activity_slots error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(rows.into_iter().map(ActivitySlot::from).collect())
}

pub async fn update_slot(
    pool: &PgPool,
    id: Uuid,
    body: UpdateActivitySlotRequest,
) -> Result<ActivitySlot, AppError> {
    sqlx::query_as::<_, ActivitySlotRow>(
        r#"WITH upd AS (
            UPDATE activity_slots SET
                registration_type = COALESCE($2, registration_type),
                teacher_reg_open = COALESCE($3, teacher_reg_open),
                student_reg_open = COALESCE($4, student_reg_open),
                student_reg_start = COALESCE($5, student_reg_start),
                student_reg_end = COALESCE($6, student_reg_end),
                is_active = COALESCE($7, is_active),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
        )
        SELECT upd.*,
            ac.name AS name,
            ac.description AS description,
            ac.activity_type AS activity_type,
            ac.periods_per_week AS periods_per_week,
            ac.scheduling_mode AS scheduling_mode,
            ac.grade_level_ids AS allowed_grade_level_ids,
            NULL::TEXT AS semester_name,
            NULL::BIGINT AS group_count,
            NULL::BIGINT AS total_members,
            COALESCE(
                (SELECT array_agg(classroom_id) FROM activity_slot_classrooms WHERE slot_id = upd.id),
                '{}'::uuid[]
            ) AS classroom_ids
        FROM upd
        JOIN activity_catalog ac ON ac.id = upd.activity_catalog_id"#,
    )
    .bind(id)
    .bind(&body.registration_type)
    .bind(body.teacher_reg_open)
    .bind(body.student_reg_open)
    .bind(activity_datetime_from_rfc3339(body.student_reg_start.as_deref()))
    .bind(activity_datetime_from_rfc3339(body.student_reg_end.as_deref()))
    .bind(body.is_active)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("update_activity_slot error: {e}");
        AppError::NotFound("ไม่พบช่องกิจกรรม".to_string())
    })
    .map(ActivitySlot::from)
}

pub async fn delete_slot(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM activity_slots WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("delete_activity_slot error: {e}");
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    Ok(())
}

// ============================================
// Activity Groups
// ============================================

pub async fn list_groups(
    pool: &PgPool,
    filter: ActivityGroupFilter,
    access: UserResourceListAccess,
) -> Result<Vec<ActivityGroup>, AppError> {
    let mut sql = String::from(
        r#"SELECT
            ag.*,
            u.first_name || ' ' || u.last_name AS instructor_name,
            COUNT(agm.id) AS member_count,
            ac.name AS slot_name,
            ac.activity_type,
            sem.name AS semester_name
        FROM activity_groups ag
        LEFT JOIN users u ON u.id = ag.instructor_id
        LEFT JOIN activity_group_members agm ON agm.activity_group_id = ag.id
        LEFT JOIN activity_slots s ON s.id = ag.slot_id
        LEFT JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
        LEFT JOIN academic_semesters sem ON sem.id = s.semester_id
        WHERE ag.is_active = true"#,
    );

    let mut idx = 0u32;
    if filter.slot_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND ag.slot_id = ${idx}"));
    }
    if filter.semester_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND s.semester_id = ${idx}"));
    }
    if filter.activity_type.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND ac.activity_type = ${idx}"));
    }
    if filter.instructor_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND ag.instructor_id = ${idx}"));
    }
    if filter.registration_open.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND ag.registration_open = ${idx}"));
    }
    if let Some(ref search) = filter.search {
        if !search.is_empty() {
            idx += 1;
            sql.push_str(&format!(" AND ag.name ILIKE ${idx}"));
        }
    }
    let scoped_actor_user_id = activity_actor_scope_filter(access);
    if scoped_actor_user_id.is_some() {
        idx += 1;
        sql.push_str(&format!(
            r#" AND (
                ag.instructor_id = ${idx}
                OR EXISTS (
                    SELECT 1
                    FROM activity_group_instructors agi_scope
                    WHERE agi_scope.activity_group_id = ag.id
                      AND agi_scope.instructor_id = ${idx}
                )
            )"#
        ));
    }

    sql.push_str(" GROUP BY ag.id, u.first_name, u.last_name, ac.name, ac.activity_type, sem.name ORDER BY ac.activity_type, ag.name");

    let mut q = sqlx::query_as::<_, ActivityGroupRow>(&sql);
    if let Some(v) = filter.slot_id {
        q = q.bind(v);
    }
    if let Some(v) = filter.semester_id {
        q = q.bind(v);
    }
    if let Some(ref v) = filter.activity_type {
        q = q.bind(v);
    }
    if let Some(v) = filter.instructor_id {
        q = q.bind(v);
    }
    if let Some(v) = filter.registration_open {
        q = q.bind(v);
    }
    if let Some(ref search) = filter.search {
        if !search.is_empty() {
            q = q.bind(format!("%{search}%"));
        }
    }
    if let Some(actor_user_id) = scoped_actor_user_id {
        q = q.bind(actor_user_id);
    }

    let rows = q.fetch_all(pool).await.map_err(|e| {
        tracing::error!("list_activity_groups error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(rows.into_iter().map(ActivityGroup::from).collect())
}

/// Outcome ของ create_group — ให้ caller รู้ว่า slot ปิดอยู่หรือครูไม่อยู่ในรายชื่อ
pub enum CreateGroupOutcome {
    Created(Box<ActivityGroup>),
    SlotClosed,
    InstructorNotInSlot,
}

pub async fn create_group(
    pool: &PgPool,
    actor: &ActorContext,
    body: CreateActivityGroupRequest,
) -> Result<CreateGroupOutcome, AppError> {
    let has_manage_all = activity_access_policy::can_manage_all_activity(actor);
    let effective_instructor_id = if has_manage_all {
        body.instructor_id
    } else {
        Some(body.instructor_id.unwrap_or(actor.user_id))
    };

    if let Some(instructor_id) = effective_instructor_id {
        activity_access_policy::can_create_activity_group_for(actor, instructor_id)?;
    } else if !has_manage_all {
        return Err(AppError::Forbidden("ไม่มีสิทธิ์สร้างกิจกรรมนี้".to_string()));
    }

    // Check slot is open
    let slot_open: Option<(bool,)> = sqlx::query_as(
        "SELECT teacher_reg_open FROM activity_slots WHERE id = $1 AND is_active = true",
    )
    .bind(body.slot_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    match slot_open {
        None => return Err(AppError::NotFound("ไม่พบช่องกิจกรรม".to_string())),
        Some((false,)) if !has_manage_all => return Ok(CreateGroupOutcome::SlotClosed),
        _ => {}
    }

    // Instructor must be in slot (ยกเว้น admin)
    if let Some(instructor_id) = effective_instructor_id {
        if !has_manage_all {
            let in_slot: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM activity_slot_instructors WHERE slot_id = $1 AND user_id = $2)",
            )
            .bind(body.slot_id)
            .bind(instructor_id)
            .fetch_one(pool)
            .await
            .unwrap_or(false);
            if !in_slot {
                return Ok(CreateGroupOutcome::InstructorNotInSlot);
            }
        }
    }

    let allowed = optional_uuid_list_json(body.allowed_classroom_ids.as_deref());

    let row: ActivityGroupRow = sqlx::query_as(
        r#"INSERT INTO activity_groups
            (slot_id, name, description, instructor_id, max_capacity, allowed_classroom_ids)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING *, NULL::TEXT AS instructor_name, NULL::BIGINT AS member_count,
                     NULL::TEXT AS slot_name, NULL::TEXT AS activity_type, NULL::TEXT AS semester_name"#,
    )
    .bind(body.slot_id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(effective_instructor_id)
    .bind(body.max_capacity)
    .bind(&allowed)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("create_activity_group error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(CreateGroupOutcome::Created(Box::new(ActivityGroup::from(
        row,
    ))))
}

pub async fn update_group(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    body: UpdateActivityGroupRequest,
) -> Result<ActivityGroup, AppError> {
    activity_access_policy::can_manage_activity_group(pool, actor, id).await?;
    if !activity_access_policy::can_manage_all_activity(actor) {
        if let Some(instructor_id) = body.instructor_id {
            activity_access_policy::can_create_activity_group_for(actor, instructor_id)?;
        }
    }

    let allowed = optional_uuid_list_json(body.allowed_classroom_ids.as_deref());

    sqlx::query_as::<_, ActivityGroupRow>(
        r#"UPDATE activity_groups SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            instructor_id = COALESCE($4, instructor_id),
            max_capacity = COALESCE($5, max_capacity),
            registration_open = COALESCE($6, registration_open),
            is_active = COALESCE($7, is_active),
            allowed_classroom_ids = COALESCE($8, allowed_classroom_ids),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *, NULL::TEXT AS instructor_name, NULL::BIGINT AS member_count,
                  NULL::TEXT AS slot_name, NULL::TEXT AS activity_type, NULL::TEXT AS semester_name"#,
    )
    .bind(id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(body.instructor_id)
    .bind(body.max_capacity)
    .bind(body.registration_open)
    .bind(body.is_active)
    .bind(&allowed)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("update_activity_group error: {e}");
        AppError::NotFound("ไม่พบกลุ่มกิจกรรม".to_string())
    })
    .map(ActivityGroup::from)
}

pub async fn delete_group(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM activity_groups WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("delete_activity_group error: {e}");
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    Ok(())
}

// ============================================
// Members
// ============================================

pub async fn list_members(
    pool: &PgPool,
    group_id: Uuid,
) -> Result<Vec<ActivityGroupMember>, AppError> {
    sqlx::query_as(
        r#"SELECT
            agm.*,
            u.first_name || ' ' || u.last_name AS student_name,
            si.student_id AS student_code,
            cr.name AS classroom_name,
            CASE gl.level_type
                WHEN 'kindergarten' THEN 'อ.' || gl.year
                WHEN 'primary'      THEN 'ป.' || gl.year
                WHEN 'secondary'    THEN 'ม.' || gl.year
                ELSE gl.level_type || gl.year::TEXT
            END AS grade_level_name
        FROM activity_group_members agm
        JOIN users u ON u.id = agm.student_id
        LEFT JOIN student_info si ON si.user_id = agm.student_id
        LEFT JOIN student_class_enrollments se ON se.student_id = agm.student_id AND se.status = 'active'
        LEFT JOIN class_rooms cr ON cr.id = se.class_room_id
        LEFT JOIN grade_levels gl ON gl.id = cr.grade_level_id
        WHERE agm.activity_group_id = $1
        ORDER BY cr.name, u.first_name"#,
    )
    .bind(group_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("list_members error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })
}

/// AddMembersOutcome — caller รู้ว่า over capacity หรือสำเร็จกี่คน
pub enum AddMembersOutcome {
    Inserted(usize),
    OverCapacity(i32),
}

pub async fn add_members(
    pool: &PgPool,
    group_id: Uuid,
    student_ids: Vec<Uuid>,
) -> Result<AddMembersOutcome, AppError> {
    let (current_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM activity_group_members WHERE activity_group_id = $1")
            .bind(group_id)
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let (max_cap,): (Option<i32>,) =
        sqlx::query_as("SELECT max_capacity FROM activity_groups WHERE id = $1")
            .bind(group_id)
            .fetch_one(pool)
            .await
            .map_err(|_| AppError::NotFound("ไม่พบกลุ่มกิจกรรม".to_string()))?;

    let student_ids = unique_activity_student_ids(&student_ids);

    if let Some(cap) = max_cap {
        if current_count + student_ids.len() as i64 > cap as i64 {
            return Ok(AddMembersOutcome::OverCapacity(cap));
        }
    }

    let inserted = bulk_insert_activity_group_members(pool, group_id, &student_ids).await?;
    Ok(AddMembersOutcome::Inserted(inserted))
}

pub async fn my_enrollments(pool: &PgPool, user_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    sqlx::query_scalar("SELECT activity_group_id FROM activity_group_members WHERE student_id = $1")
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))
}

/// Outcome ของ self_enroll — caller format error message ตามสถานะ
pub enum SelfEnrollOutcome {
    Enrolled,
    AlreadyEnrolled,
    NotSelfRegistrationType,
    NotOpen,
    Full,
    ClassroomNotAllowed,
}

pub async fn self_enroll(
    pool: &PgPool,
    group_id: Uuid,
    user_id: Uuid,
) -> Result<SelfEnrollOutcome, AppError> {
    let row: Option<(bool, Option<i32>, String)> = sqlx::query_as(
        r#"SELECT s.student_reg_open, ag.max_capacity, s.registration_type
           FROM activity_groups ag
           JOIN activity_slots s ON s.id = ag.slot_id
           WHERE ag.id = $1 AND ag.is_active = true"#,
    )
    .bind(group_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let (open, cap, reg_type) =
        row.ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มกิจกรรม".to_string()))?;

    if reg_type != "self" {
        return Ok(SelfEnrollOutcome::NotSelfRegistrationType);
    }
    if !open {
        return Ok(SelfEnrollOutcome::NotOpen);
    }

    if let Some(max) = cap {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM activity_group_members WHERE activity_group_id = $1",
        )
        .bind(group_id)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        if count >= max as i64 {
            return Ok(SelfEnrollOutcome::Full);
        }
    }

    let student_classroom: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT sce.class_room_id FROM student_class_enrollments sce
           WHERE sce.student_id = $1 AND sce.status = 'active'
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if let Some(classroom_id) = student_classroom {
        let is_allowed: bool = sqlx::query_scalar(
            r#"SELECT CASE
                   WHEN ag.allowed_classroom_ids IS NOT NULL
                     THEN ag.allowed_classroom_ids ? $2::text
                   ELSE EXISTS(
                     SELECT 1 FROM activity_slot_classrooms asc2
                     WHERE asc2.slot_id = ag.slot_id AND asc2.classroom_id = $2
                   )
               END
               FROM activity_groups ag
               WHERE ag.id = $1"#,
        )
        .bind(group_id)
        .bind(classroom_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?
        .unwrap_or(false);

        if !is_allowed {
            return Ok(SelfEnrollOutcome::ClassroomNotAllowed);
        }
    }

    let result = sqlx::query(
        "INSERT INTO activity_group_members (activity_group_id, student_id, enrolled_by)
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(group_id)
    .bind(user_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if result.rows_affected() > 0 {
        Ok(SelfEnrollOutcome::Enrolled)
    } else {
        Ok(SelfEnrollOutcome::AlreadyEnrolled)
    }
}

pub async fn self_unenroll(pool: &PgPool, group_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        "DELETE FROM activity_group_members WHERE activity_group_id = $1 AND student_id = $2",
    )
    .bind(group_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn remove_member(
    pool: &PgPool,
    group_id: Uuid,
    student_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "DELETE FROM activity_group_members WHERE activity_group_id = $1 AND student_id = $2",
    )
    .bind(group_id)
    .bind(student_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn update_member_result(
    pool: &PgPool,
    member_id: Uuid,
    result: &str,
) -> Result<(), AppError> {
    if result != "pass" && result != "fail" {
        return Err(AppError::BadRequest(
            "result ต้องเป็น pass หรือ fail".to_string(),
        ));
    }
    sqlx::query("UPDATE activity_group_members SET result = $1 WHERE id = $2")
        .bind(result)
        .bind(member_id)
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

// ============================================
// Group Instructors
// ============================================

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct InstructorInfo {
    pub id: Uuid,
    pub instructor_id: Uuid,
    pub role: String,
    pub instructor_name: Option<String>,
}

pub async fn list_group_instructors(
    pool: &PgPool,
    group_id: Uuid,
) -> Result<Vec<InstructorInfo>, AppError> {
    sqlx::query_as(
        r#"SELECT agi.id, agi.instructor_id, agi.role,
                  u.first_name || ' ' || u.last_name AS instructor_name
           FROM activity_group_instructors agi
           JOIN users u ON u.id = agi.instructor_id
           WHERE agi.activity_group_id = $1
           ORDER BY CASE agi.role WHEN 'primary' THEN 1 ELSE 2 END"#,
    )
    .bind(group_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn add_group_instructor(
    pool: &PgPool,
    actor: &ActorContext,
    group_id: Uuid,
    instructor_id: Uuid,
    role: &str,
) -> Result<(), AppError> {
    activity_access_policy::can_manage_activity_group(pool, actor, group_id).await?;

    sqlx::query(
        "INSERT INTO activity_group_instructors (activity_group_id, instructor_id, role)
         VALUES ($1, $2, $3) ON CONFLICT (activity_group_id, instructor_id) DO UPDATE SET role = $3",
    )
    .bind(group_id)
    .bind(instructor_id)
    .bind(role)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn remove_group_instructor(
    pool: &PgPool,
    actor: &ActorContext,
    group_id: Uuid,
    instructor_id: Uuid,
) -> Result<(), AppError> {
    activity_access_policy::can_manage_activity_group(pool, actor, group_id).await?;

    sqlx::query(
        "DELETE FROM activity_group_instructors WHERE activity_group_id = $1 AND instructor_id = $2",
    )
    .bind(group_id)
    .bind(instructor_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

// ============================================
// Slot Instructors
// ============================================

#[derive(Debug, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct SlotInstructorInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub instructor_name: Option<String>,
}

pub async fn list_slot_instructors(
    pool: &PgPool,
    slot_id: Uuid,
) -> Result<Vec<SlotInstructorInfo>, AppError> {
    sqlx::query_as(
        r#"SELECT asi.id, asi.user_id,
                  u.first_name || ' ' || u.last_name AS instructor_name
           FROM activity_slot_instructors asi
           JOIN users u ON u.id = asi.user_id
           WHERE asi.slot_id = $1
           ORDER BY u.first_name"#,
    )
    .bind(slot_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

/// Add slot instructor + propagate ไป timetable_entry_instructors
pub async fn add_slot_instructor(
    pool: &PgPool,
    slot_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        "INSERT INTO activity_slot_instructors (slot_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(slot_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        r#"INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
           SELECT te.id, $2, 'primary'
           FROM academic_timetable_entries te
           WHERE te.activity_slot_id = $1
             AND NOT EXISTS (
                 SELECT 1 FROM academic_timetable_entries te2
                 JOIN timetable_entry_instructors tei2 ON tei2.entry_id = te2.id
                 WHERE tei2.instructor_id = $2
                   AND te2.day_of_week = te.day_of_week
                   AND te2.period_id = te.period_id
                   AND te2.id <> te.id
             )
           ON CONFLICT (entry_id, instructor_id) DO NOTHING"#,
    )
    .bind(slot_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn add_slot_instructors_batch(
    pool: &PgPool,
    slot_id: Uuid,
    user_ids: Vec<Uuid>,
) -> Result<usize, AppError> {
    if user_ids.is_empty() {
        return Ok(0);
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        r#"INSERT INTO activity_slot_instructors (slot_id, user_id)
           SELECT $1, u.id FROM UNNEST($2::uuid[]) AS u(id)
           ON CONFLICT DO NOTHING"#,
    )
    .bind(slot_id)
    .bind(&user_ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        r#"INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
           SELECT te.id, u.id, 'primary'
           FROM academic_timetable_entries te
           CROSS JOIN UNNEST($2::uuid[]) AS u(id)
           WHERE te.activity_slot_id = $1
             AND NOT EXISTS (
                 SELECT 1 FROM academic_timetable_entries te2
                 JOIN timetable_entry_instructors tei2 ON tei2.entry_id = te2.id
                 WHERE tei2.instructor_id = u.id
                   AND te2.day_of_week = te.day_of_week
                   AND te2.period_id = te.period_id
                   AND te2.id <> te.id
             )
           ON CONFLICT (entry_id, instructor_id) DO NOTHING"#,
    )
    .bind(slot_id)
    .bind(&user_ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(user_ids.len())
}

pub async fn remove_slot_instructor(
    pool: &PgPool,
    slot_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query("DELETE FROM activity_slot_instructors WHERE slot_id = $1 AND user_id = $2")
        .bind(slot_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        r#"DELETE FROM timetable_entry_instructors tei
           USING academic_timetable_entries te
           WHERE tei.entry_id = te.id
             AND te.activity_slot_id = $1
             AND tei.instructor_id = $2"#,
    )
    .bind(slot_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn delete_slot_timetable_entries(pool: &PgPool, slot_id: Uuid) -> Result<u64, AppError> {
    let result = sqlx::query("DELETE FROM academic_timetable_entries WHERE activity_slot_id = $1")
        .bind(slot_id)
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(result.rows_affected())
}

pub async fn delete_all_slot_groups(pool: &PgPool, slot_id: Uuid) -> Result<u64, AppError> {
    let result = sqlx::query("DELETE FROM activity_groups WHERE slot_id = $1")
        .bind(slot_id)
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(result.rows_affected())
}

pub async fn remove_all_slot_instructors(pool: &PgPool, slot_id: Uuid) -> Result<u64, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let result = sqlx::query("DELETE FROM activity_slot_instructors WHERE slot_id = $1")
        .bind(slot_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        r#"DELETE FROM timetable_entry_instructors tei
           USING academic_timetable_entries te
           WHERE tei.entry_id = te.id
             AND te.activity_slot_id = $1"#,
    )
    .bind(slot_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(result.rows_affected())
}

// ============================================
// Slot Classroom Assignments
// ============================================

pub async fn list_slot_classroom_assignments(
    pool: &PgPool,
    slot_id: Uuid,
) -> Result<Vec<SlotClassroomAssignment>, AppError> {
    sqlx::query_as::<_, SlotClassroomAssignment>(
        r#"SELECT asca.*, cr.name AS classroom_name,
                  concat(u.first_name, ' ', u.last_name) AS instructor_name
           FROM activity_slot_classroom_assignments asca
           JOIN class_rooms cr ON cr.id = asca.classroom_id
           JOIN users u ON u.id = asca.instructor_id
           WHERE asca.slot_id = $1
           ORDER BY cr.name"#,
    )
    .bind(slot_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("list_slot_classroom_assignments error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })
}

pub async fn batch_upsert_slot_classroom_assignments(
    pool: &PgPool,
    slot_id: Uuid,
    body: BatchUpsertSlotClassroomAssignmentsRequest,
) -> Result<usize, AppError> {
    let assignment_rows = slot_classroom_assignment_bulk_rows(&body.assignments);
    if assignment_rows.is_empty() {
        return Ok(0);
    }

    let classroom_ids: Vec<Uuid> = assignment_rows
        .iter()
        .map(|assignment| assignment.classroom_id)
        .collect();
    let instructor_ids: Vec<Uuid> = assignment_rows
        .iter()
        .map(|assignment| assignment.instructor_id)
        .collect();

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        r#"INSERT INTO activity_slot_classroom_assignments (slot_id, classroom_id, instructor_id)
           SELECT $1, classroom_id, instructor_id
           FROM UNNEST($2::uuid[], $3::uuid[]) AS rows(classroom_id, instructor_id)
           ON CONFLICT (slot_id, classroom_id)
           DO UPDATE SET instructor_id = EXCLUDED.instructor_id"#,
    )
    .bind(slot_id)
    .bind(&classroom_ids)
    .bind(&instructor_ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("upsert_slot_classroom_assignment error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    sqlx::query(
        r#"DELETE FROM timetable_entry_instructors tei
           USING academic_timetable_entries te
           WHERE tei.entry_id = te.id
             AND te.activity_slot_id = $1
             AND te.classroom_id = ANY($2)"#,
    )
    .bind(slot_id)
    .bind(&classroom_ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        r#"WITH assignment_rows AS (
               SELECT classroom_id, instructor_id
               FROM UNNEST($2::uuid[], $3::uuid[]) AS rows(classroom_id, instructor_id)
           )
           INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
           SELECT te.id, assignment_rows.instructor_id, 'primary'
           FROM academic_timetable_entries te
           JOIN assignment_rows ON assignment_rows.classroom_id = te.classroom_id
           WHERE te.activity_slot_id = $1
           ON CONFLICT (entry_id, instructor_id) DO NOTHING"#,
    )
    .bind(slot_id)
    .bind(&classroom_ids)
    .bind(&instructor_ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(assignment_rows.len())
}

pub async fn delete_all_slot_classroom_assignments(
    pool: &PgPool,
    slot_id: Uuid,
) -> Result<u64, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let result = sqlx::query("DELETE FROM activity_slot_classroom_assignments WHERE slot_id = $1")
        .bind(slot_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        r#"DELETE FROM timetable_entry_instructors tei
           USING academic_timetable_entries te
           WHERE tei.entry_id = te.id
             AND te.activity_slot_id = $1"#,
    )
    .bind(slot_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(result.rows_affected())
}

pub async fn delete_slot_classroom_assignment(
    pool: &PgPool,
    slot_id: Uuid,
    assignment_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let classroom_id: Option<Uuid> = sqlx::query_scalar(
        "DELETE FROM activity_slot_classroom_assignments WHERE id = $1 AND slot_id = $2 RETURNING classroom_id",
    )
    .bind(assignment_id)
    .bind(slot_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if let Some(cls_id) = classroom_id {
        sqlx::query(
            r#"DELETE FROM timetable_entry_instructors tei
               USING academic_timetable_entries te
               WHERE tei.entry_id = te.id
                 AND te.activity_slot_id = $1
                 AND te.classroom_id = $2"#,
        )
        .bind(slot_id)
        .bind(cls_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_datetime_from_rfc3339_accepts_valid_values() {
        assert!(activity_datetime_from_rfc3339(Some("2026-06-06T08:30:00+07:00")).is_some());
        assert!(activity_datetime_from_rfc3339(None).is_none());
    }

    #[test]
    fn activity_datetime_from_rfc3339_ignores_invalid_values() {
        assert!(activity_datetime_from_rfc3339(Some("not-a-date")).is_none());
    }

    #[test]
    fn optional_uuid_list_json_wraps_some_values_and_preserves_none() {
        let id = Uuid::new_v4();

        assert_eq!(optional_uuid_list_json(None), None);
        assert_eq!(
            optional_uuid_list_json(Some(&[id])).map(|Json(ids)| ids),
            Some(vec![id])
        );
    }

    #[test]
    fn unique_activity_student_ids_preserves_first_seen_order() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();

        assert_eq!(
            unique_activity_student_ids(&[first, second, first]),
            vec![first, second]
        );
    }

    #[test]
    fn slot_classroom_assignment_bulk_rows_keeps_latest_classroom_assignment() {
        let classroom_id = Uuid::new_v4();
        let old_instructor_id = Uuid::new_v4();
        let new_instructor_id = Uuid::new_v4();

        assert_eq!(
            slot_classroom_assignment_bulk_rows(&[
                UpsertSlotClassroomAssignmentRequest {
                    classroom_id,
                    instructor_id: old_instructor_id,
                },
                UpsertSlotClassroomAssignmentRequest {
                    classroom_id,
                    instructor_id: new_instructor_id,
                },
            ]),
            vec![SlotClassroomAssignmentBulkRow {
                classroom_id,
                instructor_id: new_instructor_id,
            }]
        );
    }

    #[test]
    fn activity_actor_scope_filter_leaves_school_scope_unfiltered() {
        assert_eq!(
            activity_actor_scope_filter(UserResourceListAccess::School),
            None
        );
    }

    #[test]
    fn activity_actor_scope_filter_uses_actor_for_own_scope() {
        let actor_user_id = Uuid::new_v4();

        assert_eq!(
            activity_actor_scope_filter(UserResourceListAccess::Own(actor_user_id)),
            Some(actor_user_id)
        );
    }
}
