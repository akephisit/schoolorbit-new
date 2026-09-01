use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::delivery::models::ActivitySchedulingMode;
use crate::modules::academic::models::timetable_block::{
    TimetableBlock, TimetableBlockGroup, TimetableBlockHomeroom, TimetableBlockInstructor,
    TimetableBlockKind, TimetableBlockSummary, TimetableBlockSyncState, TimetableBlockSyncStatus,
    TimetableBlockTeacher, TimetableBlockWorkspace, TimetableBlockWorkspaceHomeroom,
    TimetableBlockWorkspaceLearningGroup, TimetableBlockWorkspaceQuery,
    TimetableBlockWorkspaceRoom, TimetableBlockWorkspaceStaff, TimetableOrdinaryDemand,
    TimetableStructuralKind, TimetableSynchronizedDemand,
};
use crate::policies::timetable_access_policy::TimetableAccessFilter;

#[derive(Debug, FromRow)]
struct BlockRow {
    id: Uuid,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    bell_schedule_id: Uuid,
    bell_schedule_period_id: Uuid,
    period_name: String,
    start_time: NaiveTime,
    end_time: NaiveTime,
    day_of_week: String,
    block_kind: String,
    scheduling_mode: Option<ActivitySchedulingMode>,
    learning_offering_id: Option<Uuid>,
    offering_code: Option<String>,
    offering_name: Option<String>,
    structural_kind: Option<String>,
    title: Option<String>,
    note: Option<String>,
    series_id: Option<Uuid>,
    row_version: i64,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct GroupRow {
    id: Uuid,
    block_id: Uuid,
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    code: String,
    name: String,
    homeroom_ids: Vec<Uuid>,
    room_id: Option<Uuid>,
    room_code: Option<String>,
    row_version: i64,
    is_active: bool,
}

#[derive(Debug, FromRow)]
struct InstructorRow {
    block_group_id: Uuid,
    teacher_id: Uuid,
    display_name: String,
    role: String,
    display_order: i32,
}

#[derive(Debug, FromRow)]
struct HomeroomRow {
    id: Uuid,
    block_id: Uuid,
    homeroom_id: Uuid,
    code: String,
    name: String,
    room_id: Option<Uuid>,
    room_code: Option<String>,
    row_version: i64,
    is_active: bool,
}

#[derive(Debug, FromRow)]
struct TeacherRow {
    id: Uuid,
    block_id: Uuid,
    teacher_id: Uuid,
    display_name: String,
    row_version: i64,
    is_active: bool,
}

#[derive(Debug, FromRow)]
struct SyncRow {
    id: Uuid,
    block_id: Uuid,
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    status: String,
    linked_block_group_id: Option<Uuid>,
    conflict_code: Option<String>,
    conflict_message: Option<String>,
    attempted_group_row_version: Option<i64>,
    row_version: i64,
}

#[derive(Debug, FromRow)]
struct WorkspaceGroupRow {
    id: Uuid,
    learning_offering_id: Uuid,
    code: String,
    name: String,
    status: String,
    roster_status: String,
    offering_kind: String,
    offering_code: String,
    offering_name: String,
    scheduling_mode: Option<ActivitySchedulingMode>,
    weekly_period_target: i32,
    homeroom_ids: Vec<Uuid>,
}

#[derive(Debug, FromRow)]
struct EligibleInstructorRow {
    learning_group_id: Uuid,
    teacher_id: Uuid,
    display_name: String,
    role: String,
    display_order: i32,
}

#[derive(Debug, FromRow)]
struct SynchronizedDemandRow {
    learning_offering_id: Uuid,
    offering_code: String,
    offering_name: String,
    required_periods: i32,
    scheduled_periods: i32,
    intended_homeroom_ids: Vec<Uuid>,
    linked_group_count: i32,
    pending_group_count: i32,
    conflict_group_count: i32,
    excluded_group_count: i32,
}

pub(crate) async fn get_workspace(
    pool: &PgPool,
    query: TimetableBlockWorkspaceQuery,
    access: &TimetableAccessFilter,
) -> Result<TimetableBlockWorkspace, AppError> {
    const MAX_GROUPS: i64 = 2_000;
    const MAX_HOMEROOMS: i64 = 500;
    const MAX_ROOMS: i64 = 2_000;
    const MAX_STAFF: i64 = 2_000;

    let version = super::timetable_version_service::get_version(
        pool,
        query.timetable_version_id,
        Utc::now().date_naive(),
    )
    .await?;
    if version.academic_year_id != query.academic_year_id
        || version.academic_term_id != query.academic_term_id
    {
        return Err(AppError::ValidationError(
            "รุ่นตารางสอนไม่อยู่ในปีการศึกษาและภาคเรียนที่เลือก".to_string(),
        ));
    }
    let mut owner_ids = access.organization_unit_ids.clone();
    owner_ids.extend(access.organization_tree_unit_ids.iter().copied());
    owner_ids.sort_unstable();
    owner_ids.dedup();
    let block_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT block.id
           FROM academic_timetable_blocks block
           LEFT JOIN learning_offerings offering ON offering.id = block.learning_offering_id
           WHERE block.timetable_version_id = $1 AND block.is_active
             AND (
                 $2
                 OR offering.owning_organization_unit_id = ANY($3)
                 OR EXISTS (
                     SELECT 1 FROM academic_timetable_block_groups block_group
                     JOIN learning_group_teachers assignment
                       ON assignment.learning_group_id = block_group.learning_group_id
                     WHERE block_group.block_id = block.id AND block_group.is_active
                       AND assignment.teacher_id = $4
                 )
                 OR EXISTS (
                     SELECT 1 FROM academic_timetable_block_teachers target
                     WHERE target.block_id = block.id AND target.is_active
                       AND target.teacher_id = $4
                 )
                 OR EXISTS (
                     SELECT 1 FROM academic_timetable_block_homerooms target
                     JOIN homeroom_advisors advisor ON advisor.homeroom_id = target.homeroom_id
                     WHERE target.block_id = block.id AND target.is_active
                       AND advisor.user_id = $4
                 )
             )
           ORDER BY block.id"#,
    )
    .bind(query.timetable_version_id)
    .bind(access.includes_school_owned)
    .bind(&owner_ids)
    .bind(access.assigned_actor_id)
    .fetch_all(pool)
    .await?;
    let blocks = get_blocks(pool, &block_ids).await?;
    let referenced_period_ids = blocks
        .iter()
        .map(|block| block.bell_schedule_period_id)
        .collect::<Vec<_>>();
    let bell_periods = sqlx::query_as(
        r#"SELECT id, bell_schedule_id, name, start_time, end_time,
                  order_index, applicable_days, is_active
           FROM bell_schedule_periods
           WHERE bell_schedule_id = $1 AND (is_active OR id = ANY($2))
           ORDER BY order_index, id"#,
    )
    .bind(version.bell_schedule_id)
    .bind(&referenced_period_ids)
    .fetch_all(pool)
    .await?;

    let group_rows: Vec<WorkspaceGroupRow> = sqlx::query_as(
        r#"SELECT learning_group.id, learning_group.learning_offering_id,
                  learning_group.code, learning_group.name,
                  learning_group.status, learning_group.roster_status,
                  offering.kind AS offering_kind,
                  offering.code_snapshot AS offering_code,
                  offering.name_snapshot AS offering_name,
                  activity_detail.scheduling_mode,
                  target.weekly_period_target,
                  COALESCE((
                      SELECT array_agg(coverage.homeroom_id ORDER BY coverage.homeroom_id)
                      FROM learning_group_homerooms coverage
                      WHERE coverage.learning_group_id = learning_group.id
                  ), ARRAY[]::uuid[]) AS homeroom_ids
           FROM learning_groups learning_group
           JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
           LEFT JOIN activity_offering_details activity_detail
             ON activity_detail.learning_offering_id = offering.id
           JOIN academic_timetable_version_targets target
             ON target.timetable_version_id = $1
            AND target.learning_offering_id = learning_group.learning_offering_id
           WHERE learning_group.academic_term_id = $2
             AND learning_group.academic_year_id = $3
             AND ($4 OR offering.owning_organization_unit_id = ANY($5) OR EXISTS (
                 SELECT 1 FROM learning_group_teachers assignment
                 WHERE assignment.learning_group_id = learning_group.id
                   AND assignment.teacher_id = $6
             ))
           ORDER BY offering.code_snapshot, learning_group.code, learning_group.id
           LIMIT $7"#,
    )
    .bind(query.timetable_version_id)
    .bind(query.academic_term_id)
    .bind(query.academic_year_id)
    .bind(access.includes_school_owned)
    .bind(&owner_ids)
    .bind(access.assigned_actor_id)
    .bind(MAX_GROUPS + 1)
    .fetch_all(pool)
    .await?;
    if group_rows.len() > MAX_GROUPS as usize {
        return Err(AppError::ValidationError(
            "จำนวนกลุ่มเรียนในพื้นที่จัดตารางเกิน 2000 กลุ่ม".to_string(),
        ));
    }
    let group_ids = group_rows.iter().map(|group| group.id).collect::<Vec<_>>();
    let eligible_rows: Vec<EligibleInstructorRow> = if group_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            r#"SELECT assignment.learning_group_id, assignment.teacher_id,
                      coalesce(nullif(concat_ws(' ',
                          nullif(concat(coalesce(account.title, ''), account.first_name), ''),
                          nullif(account.last_name, '')
                      ), ''), account.username, account.email) AS display_name,
                      assignment.role,
                      row_number() OVER (
                          PARTITION BY assignment.learning_group_id
                          ORDER BY CASE assignment.role
                                     WHEN 'primary' THEN 1
                                     WHEN 'secondary' THEN 2
                                     ELSE 3
                                   END,
                                   assignment.starts_on, assignment.id
                      )::integer AS display_order
               FROM learning_group_teachers assignment
               JOIN users account ON account.id = assignment.teacher_id
               JOIN academic_timetable_versions version ON version.id = $2
               WHERE assignment.learning_group_id = ANY($1)
                 AND assignment.starts_on <= version.effective_from
                 AND (assignment.ends_on IS NULL OR assignment.ends_on >= version.effective_from)
                 AND account.user_type = 'staff' AND account.status = 'active'
               ORDER BY assignment.learning_group_id, display_order"#,
        )
        .bind(&group_ids)
        .bind(query.timetable_version_id)
        .fetch_all(pool)
        .await?
    };
    let mut eligible_by_group: BTreeMap<Uuid, Vec<TimetableBlockInstructor>> = BTreeMap::new();
    for row in eligible_rows {
        eligible_by_group
            .entry(row.learning_group_id)
            .or_default()
            .push(TimetableBlockInstructor {
                teacher_id: row.teacher_id,
                display_name: row.display_name,
                role: row.role,
                order_index: row.display_order,
            });
    }
    let mut scheduled_by_group = BTreeMap::<Uuid, i32>::new();
    for block in &blocks {
        for group in &block.groups {
            *scheduled_by_group
                .entry(group.learning_group_id)
                .or_default() += 1;
        }
    }
    let learning_groups = group_rows
        .iter()
        .map(|group| TimetableBlockWorkspaceLearningGroup {
            id: group.id,
            learning_offering_id: group.learning_offering_id,
            code: group.code.clone(),
            name: group.name.clone(),
            status: group.status.clone(),
            roster_status: group.roster_status.clone(),
            offering_kind: group.offering_kind.clone(),
            offering_code: group.offering_code.clone(),
            offering_name: group.offering_name.clone(),
            homeroom_ids: group.homeroom_ids.clone(),
            eligible_instructors: eligible_by_group
                .get(&group.id)
                .cloned()
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    let ordinary_demands = group_rows
        .iter()
        .filter(|group| group.scheduling_mode != Some(ActivitySchedulingMode::Synchronized))
        .map(|group| {
            let scheduled_periods = scheduled_by_group.get(&group.id).copied().unwrap_or(0);
            TimetableOrdinaryDemand {
                learning_group_id: group.id,
                learning_offering_id: group.learning_offering_id,
                offering_code: group.offering_code.clone(),
                offering_name: group.offering_name.clone(),
                required_periods: group.weekly_period_target,
                scheduled_periods,
                remaining_periods: (group.weekly_period_target - scheduled_periods).max(0),
                homeroom_ids: group.homeroom_ids.clone(),
                eligible_instructors: eligible_by_group
                    .get(&group.id)
                    .cloned()
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();

    let synchronized_rows: Vec<SynchronizedDemandRow> = sqlx::query_as(
        r#"SELECT offering.id AS learning_offering_id,
                  offering.code_snapshot AS offering_code,
                  offering.name_snapshot AS offering_name,
                  target.weekly_period_target AS required_periods,
                  count(DISTINCT block.id) FILTER (WHERE block.is_active)::integer AS scheduled_periods,
                  COALESCE(array_agg(DISTINCT reservation.homeroom_id)
                      FILTER (WHERE reservation.is_active), ARRAY[]::uuid[]) AS intended_homeroom_ids,
                  count(DISTINCT sync.learning_group_id)
                      FILTER (WHERE sync.status = 'LINKED')::integer AS linked_group_count,
                  count(DISTINCT sync.learning_group_id)
                      FILTER (WHERE sync.status IN ('WAITING_FOR_DATA', 'OUTSIDE_SCOPE'))::integer
                      AS pending_group_count,
                  count(DISTINCT sync.learning_group_id)
                      FILTER (WHERE sync.status = 'CONFLICT')::integer AS conflict_group_count,
                  count(DISTINCT sync.learning_group_id)
                      FILTER (WHERE sync.status = 'EXCLUDED')::integer AS excluded_group_count
           FROM academic_timetable_version_targets target
           JOIN learning_offerings offering ON offering.id = target.learning_offering_id
           JOIN activity_offering_details detail
             ON detail.learning_offering_id = offering.id
            AND detail.scheduling_mode = 'synchronized'
           LEFT JOIN academic_timetable_blocks block
             ON block.timetable_version_id = target.timetable_version_id
            AND block.learning_offering_id = offering.id
           LEFT JOIN academic_timetable_block_homerooms reservation
             ON reservation.block_id = block.id
           LEFT JOIN academic_timetable_block_group_sync sync ON sync.block_id = block.id
           WHERE target.timetable_version_id = $1
             AND ($2 OR offering.owning_organization_unit_id = ANY($3) OR EXISTS (
                 SELECT 1 FROM learning_groups learning_group
                 JOIN learning_group_teachers assignment
                   ON assignment.learning_group_id = learning_group.id
                 WHERE learning_group.learning_offering_id = offering.id
                   AND assignment.teacher_id = $4
             ))
           GROUP BY offering.id, offering.code_snapshot, offering.name_snapshot,
                    target.weekly_period_target
           ORDER BY offering.code_snapshot, offering.id"#,
    )
    .bind(query.timetable_version_id)
    .bind(access.includes_school_owned)
    .bind(&owner_ids)
    .bind(access.assigned_actor_id)
    .fetch_all(pool)
    .await?;
    let synchronized_demands = synchronized_rows
        .into_iter()
        .map(|row| TimetableSynchronizedDemand {
            learning_offering_id: row.learning_offering_id,
            offering_code: row.offering_code,
            offering_name: row.offering_name,
            required_periods: row.required_periods,
            scheduled_periods: row.scheduled_periods,
            intended_homeroom_ids: row.intended_homeroom_ids,
            linked_group_count: row.linked_group_count,
            pending_group_count: row.pending_group_count,
            conflict_group_count: row.conflict_group_count,
            excluded_group_count: row.excluded_group_count,
        })
        .collect::<Vec<_>>();

    let relevant_homeroom_ids = learning_groups
        .iter()
        .flat_map(|group| group.homeroom_ids.iter().copied())
        .chain(
            blocks
                .iter()
                .flat_map(|block| block.homerooms.iter().map(|target| target.homeroom_id)),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let homerooms = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            Uuid,
            String,
            i32,
            Option<String>,
            bool,
        ),
    >(
        r#"SELECT homeroom.id, homeroom.code, homeroom.name, homeroom.grade_level_id,
                  grade.level_type::text, grade.year, homeroom.room_number, homeroom.is_active
           FROM homerooms homeroom
           JOIN grade_levels grade ON grade.id = homeroom.grade_level_id
           WHERE homeroom.academic_year_id = $1
             AND (($2 AND homeroom.is_active) OR homeroom.id = ANY($3))
           ORDER BY grade.level_type, grade.year, homeroom.room_number, homeroom.code
           LIMIT $4"#,
    )
    .bind(query.academic_year_id)
    .bind(access.includes_school_owned)
    .bind(&relevant_homeroom_ids)
    .bind(MAX_HOMEROOMS + 1)
    .fetch_all(pool)
    .await?;
    if homerooms.len() > MAX_HOMEROOMS as usize {
        return Err(AppError::ValidationError(
            "จำนวนห้องประจำชั้นในพื้นที่จัดตารางเกิน 500 ห้อง".to_string(),
        ));
    }
    let homerooms = homerooms
        .into_iter()
        .map(|row| TimetableBlockWorkspaceHomeroom {
            id: row.0,
            code: row.1,
            name: row.2,
            grade_level_id: row.3,
            grade_level_type: row.4,
            grade_level_year: row.5,
            room_number: row.6,
            is_active: row.7,
        })
        .collect::<Vec<_>>();

    let referenced_room_ids = blocks
        .iter()
        .flat_map(|block| {
            block
                .groups
                .iter()
                .filter_map(|group| group.room_id)
                .chain(block.homerooms.iter().filter_map(|target| target.room_id))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let rooms = sqlx::query_as::<_, (Uuid, Option<String>, String, String)>(
        r#"SELECT id, code, name_th, status::text FROM rooms
           WHERE ($1 AND status = 'ACTIVE') OR id = ANY($2)
           ORDER BY coalesce(code, ''), name_th, id LIMIT $3"#,
    )
    .bind(access.includes_school_owned)
    .bind(&referenced_room_ids)
    .bind(MAX_ROOMS + 1)
    .fetch_all(pool)
    .await?;
    if rooms.len() > MAX_ROOMS as usize {
        return Err(AppError::ValidationError(
            "จำนวนห้องเรียนในพื้นที่จัดตารางเกิน 2000 ห้อง".to_string(),
        ));
    }
    let rooms = rooms
        .into_iter()
        .map(|row| TimetableBlockWorkspaceRoom {
            id: row.0,
            code: row.1,
            name: row.2,
            status: row.3,
        })
        .collect::<Vec<_>>();

    let staff_ids =
        learning_groups
            .iter()
            .flat_map(|group| {
                group
                    .eligible_instructors
                    .iter()
                    .map(|teacher| teacher.teacher_id)
            })
            .chain(blocks.iter().flat_map(|block| {
                block
                    .teachers
                    .iter()
                    .map(|teacher| teacher.teacher_id)
                    .chain(block.groups.iter().flat_map(|group| {
                        group.instructors.iter().map(|teacher| teacher.teacher_id)
                    }))
            }))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
    let staff = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"SELECT id, coalesce(nullif(concat_ws(' ',
                      nullif(concat(coalesce(title, ''), first_name), ''),
                      nullif(last_name, '')
                  ), ''), username, email), status::text
           FROM users WHERE id = ANY($1) AND user_type = 'staff'
           ORDER BY 2, id LIMIT $2"#,
    )
    .bind(&staff_ids)
    .bind(MAX_STAFF + 1)
    .fetch_all(pool)
    .await?;
    if staff.len() > MAX_STAFF as usize {
        return Err(AppError::ValidationError(
            "จำนวนครูในพื้นที่จัดตารางเกิน 2000 คน".to_string(),
        ));
    }
    let staff = staff
        .into_iter()
        .map(|row| TimetableBlockWorkspaceStaff {
            id: row.0,
            display_name: row.1,
            status: row.2,
        })
        .collect::<Vec<_>>();

    let summary = TimetableBlockSummary {
        block_count: blocks.len() as i32,
        ordinary_demand_count: ordinary_demands.len() as i32,
        synchronized_demand_count: synchronized_demands.len() as i32,
        linked_group_count: synchronized_demands
            .iter()
            .map(|demand| demand.linked_group_count)
            .sum(),
        waiting_group_count: synchronized_demands
            .iter()
            .map(|demand| demand.pending_group_count)
            .sum(),
        conflict_group_count: synchronized_demands
            .iter()
            .map(|demand| demand.conflict_group_count)
            .sum(),
        excluded_group_count: synchronized_demands
            .iter()
            .map(|demand| demand.excluded_group_count)
            .sum(),
    };
    Ok(TimetableBlockWorkspace {
        version,
        bell_periods,
        blocks,
        ordinary_demands,
        synchronized_demands,
        learning_groups,
        homerooms,
        rooms,
        staff,
        summary,
    })
}

pub(crate) async fn get_block(pool: &PgPool, block_id: Uuid) -> Result<TimetableBlock, AppError> {
    let mut blocks = get_blocks(pool, &[block_id]).await?;
    blocks
        .pop()
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการตารางสอน".to_string()))
}

pub(crate) async fn get_blocks(
    pool: &PgPool,
    block_ids: &[Uuid],
) -> Result<Vec<TimetableBlock>, AppError> {
    if block_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<BlockRow> = sqlx::query_as(
        r#"SELECT block.id, block.timetable_version_id, block.academic_term_id,
                  block.academic_year_id, block.bell_schedule_id,
                  block.bell_schedule_period_id, period.name AS period_name,
                  period.start_time, period.end_time, block.day_of_week,
                  block.block_kind, detail.scheduling_mode,
                  block.learning_offering_id, offering.code_snapshot AS offering_code,
                  offering.name_snapshot AS offering_name, block.structural_kind,
                  block.title, block.note, block.series_id, block.row_version,
                  block.is_active, block.created_at, block.updated_at
           FROM academic_timetable_blocks block
           JOIN bell_schedule_periods period ON period.id = block.bell_schedule_period_id
           LEFT JOIN learning_offerings offering ON offering.id = block.learning_offering_id
           LEFT JOIN activity_offering_details detail
             ON detail.learning_offering_id = block.learning_offering_id
           WHERE block.id = ANY($1)
           ORDER BY block.day_of_week, period.order_index, block.id"#,
    )
    .bind(block_ids)
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let group_rows: Vec<GroupRow> = sqlx::query_as(
        r#"SELECT target.id, target.block_id, target.learning_group_id,
                  target.learning_offering_id, learning_group.code, learning_group.name,
                  COALESCE((
                      SELECT array_agg(coverage.homeroom_id ORDER BY coverage.homeroom_id)
                      FROM learning_group_homerooms coverage
                      WHERE coverage.learning_group_id = learning_group.id
                  ), ARRAY[]::uuid[]) AS homeroom_ids,
                  target.room_id, room.code AS room_code,
                  target.row_version, target.is_active
           FROM academic_timetable_block_groups target
           JOIN learning_groups learning_group ON learning_group.id = target.learning_group_id
           LEFT JOIN rooms room ON room.id = target.room_id
           WHERE target.block_id = ANY($1) AND target.is_active
           ORDER BY target.block_id, learning_group.code, target.id"#,
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let group_ids = group_rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let instructor_rows: Vec<InstructorRow> = if group_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            r#"SELECT instructor.block_group_id,
                      instructor.instructor_id AS teacher_id,
                      concat_ws(' ',
                          nullif(concat(coalesce(account.title, ''), account.first_name), ''),
                          nullif(account.last_name, '')
                      ) AS display_name,
                      instructor.role, instructor.display_order
               FROM academic_timetable_block_group_instructors instructor
               JOIN users account ON account.id = instructor.instructor_id
               WHERE instructor.block_group_id = ANY($1)
               ORDER BY instructor.block_group_id, instructor.display_order, instructor.id"#,
        )
        .bind(&group_ids)
        .fetch_all(pool)
        .await?
    };
    let homeroom_rows: Vec<HomeroomRow> = sqlx::query_as(
        r#"SELECT target.id, target.block_id, target.homeroom_id,
                  homeroom.code, homeroom.name, target.room_id,
                  room.code AS room_code, target.row_version, target.is_active
           FROM academic_timetable_block_homerooms target
           JOIN homerooms homeroom ON homeroom.id = target.homeroom_id
           LEFT JOIN rooms room ON room.id = target.room_id
           WHERE target.block_id = ANY($1) AND target.is_active
           ORDER BY target.block_id, homeroom.code, target.id"#,
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let teacher_rows: Vec<TeacherRow> = sqlx::query_as(
        r#"SELECT target.id, target.block_id, target.teacher_id,
                  concat_ws(' ',
                      nullif(concat(coalesce(account.title, ''), account.first_name), ''),
                      nullif(account.last_name, '')
                  ) AS display_name,
                  target.row_version, target.is_active
           FROM academic_timetable_block_teachers target
           JOIN users account ON account.id = target.teacher_id
           WHERE target.block_id = ANY($1) AND target.is_active
           ORDER BY target.block_id, display_name, target.id"#,
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let sync_rows: Vec<SyncRow> = sqlx::query_as(
        r#"SELECT id, block_id, learning_group_id, learning_offering_id,
                  status, linked_block_group_id, conflict_code, conflict_message,
                  attempted_group_row_version, row_version
           FROM academic_timetable_block_group_sync
           WHERE block_id = ANY($1)
           ORDER BY block_id, learning_group_id"#,
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;

    let mut instructors_by_group: BTreeMap<Uuid, Vec<TimetableBlockInstructor>> = BTreeMap::new();
    for row in instructor_rows {
        instructors_by_group
            .entry(row.block_group_id)
            .or_default()
            .push(TimetableBlockInstructor {
                teacher_id: row.teacher_id,
                display_name: row.display_name,
                role: row.role,
                order_index: row.display_order,
            });
    }
    let mut sync_by_group = BTreeMap::new();
    let mut sync_by_block: BTreeMap<Uuid, Vec<TimetableBlockSyncState>> = BTreeMap::new();
    for row in sync_rows {
        let status = parse_sync_status(&row.status)?;
        sync_by_group.insert((row.block_id, row.learning_group_id), status);
        sync_by_block
            .entry(row.block_id)
            .or_default()
            .push(TimetableBlockSyncState {
                id: row.id,
                learning_group_id: row.learning_group_id,
                learning_offering_id: row.learning_offering_id,
                status,
                linked_block_group_id: row.linked_block_group_id,
                conflict_code: row.conflict_code,
                conflict_message: row.conflict_message,
                attempted_group_row_version: row.attempted_group_row_version,
                row_version: row.row_version,
            });
    }
    let mut groups_by_block: BTreeMap<Uuid, Vec<TimetableBlockGroup>> = BTreeMap::new();
    for row in group_rows {
        groups_by_block
            .entry(row.block_id)
            .or_default()
            .push(TimetableBlockGroup {
                id: row.id,
                learning_group_id: row.learning_group_id,
                learning_offering_id: row.learning_offering_id,
                code: row.code,
                name: row.name,
                homeroom_ids: row.homeroom_ids,
                room_id: row.room_id,
                room_code: row.room_code,
                instructors: instructors_by_group.remove(&row.id).unwrap_or_default(),
                sync_status: sync_by_group
                    .get(&(row.block_id, row.learning_group_id))
                    .copied(),
                row_version: row.row_version,
                is_active: row.is_active,
            });
    }
    let mut homerooms_by_block: BTreeMap<Uuid, Vec<TimetableBlockHomeroom>> = BTreeMap::new();
    for row in homeroom_rows {
        homerooms_by_block
            .entry(row.block_id)
            .or_default()
            .push(TimetableBlockHomeroom {
                id: row.id,
                homeroom_id: row.homeroom_id,
                code: row.code,
                name: row.name,
                room_id: row.room_id,
                room_code: row.room_code,
                row_version: row.row_version,
                is_active: row.is_active,
            });
    }
    let mut teachers_by_block: BTreeMap<Uuid, Vec<TimetableBlockTeacher>> = BTreeMap::new();
    for row in teacher_rows {
        teachers_by_block
            .entry(row.block_id)
            .or_default()
            .push(TimetableBlockTeacher {
                id: row.id,
                teacher_id: row.teacher_id,
                display_name: row.display_name,
                row_version: row.row_version,
                is_active: row.is_active,
            });
    }

    rows.into_iter()
        .map(|row| {
            Ok(TimetableBlock {
                id: row.id,
                timetable_version_id: row.timetable_version_id,
                academic_term_id: row.academic_term_id,
                academic_year_id: row.academic_year_id,
                bell_schedule_id: row.bell_schedule_id,
                bell_schedule_period_id: row.bell_schedule_period_id,
                period_name: row.period_name,
                start_time: row.start_time,
                end_time: row.end_time,
                day_of_week: row.day_of_week,
                block_kind: parse_block_kind(&row.block_kind)?,
                scheduling_mode: row.scheduling_mode,
                learning_offering_id: row.learning_offering_id,
                offering_code: row.offering_code,
                offering_name: row.offering_name,
                structural_kind: row
                    .structural_kind
                    .as_deref()
                    .map(parse_structural_kind)
                    .transpose()?,
                title: row.title,
                note: row.note,
                series_id: row.series_id,
                groups: groups_by_block.remove(&row.id).unwrap_or_default(),
                homerooms: homerooms_by_block.remove(&row.id).unwrap_or_default(),
                teachers: teachers_by_block.remove(&row.id).unwrap_or_default(),
                sync_states: sync_by_block.remove(&row.id).unwrap_or_default(),
                row_version: row.row_version,
                is_active: row.is_active,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

fn parse_block_kind(value: &str) -> Result<TimetableBlockKind, AppError> {
    match value {
        "COURSE" => Ok(TimetableBlockKind::Course),
        "ACTIVITY" => Ok(TimetableBlockKind::Activity),
        "STRUCTURAL" => Ok(TimetableBlockKind::Structural),
        _ => Err(invalid_stored_value("block_kind", value)),
    }
}

fn parse_structural_kind(value: &str) -> Result<TimetableStructuralKind, AppError> {
    match value {
        "BREAK" => Ok(TimetableStructuralKind::Break),
        "HOMEROOM" => Ok(TimetableStructuralKind::Homeroom),
        "FLAG_CEREMONY" => Ok(TimetableStructuralKind::FlagCeremony),
        "TEACHER_MEETING" => Ok(TimetableStructuralKind::TeacherMeeting),
        "ACADEMIC" => Ok(TimetableStructuralKind::Academic),
        "OTHER" => Ok(TimetableStructuralKind::Other),
        _ => Err(invalid_stored_value("structural_kind", value)),
    }
}

fn parse_sync_status(value: &str) -> Result<TimetableBlockSyncStatus, AppError> {
    match value {
        "LINKED" => Ok(TimetableBlockSyncStatus::Linked),
        "WAITING_FOR_DATA" => Ok(TimetableBlockSyncStatus::WaitingForData),
        "CONFLICT" => Ok(TimetableBlockSyncStatus::Conflict),
        "OUTSIDE_SCOPE" => Ok(TimetableBlockSyncStatus::OutsideScope),
        "EXCLUDED" => Ok(TimetableBlockSyncStatus::Excluded),
        _ => Err(invalid_stored_value("sync_status", value)),
    }
}

fn invalid_stored_value(field: &str, value: &str) -> AppError {
    AppError::InternalServerError(format!("ข้อมูลตารางสอนภายในไม่ถูกต้อง: {field}={value}"))
}
