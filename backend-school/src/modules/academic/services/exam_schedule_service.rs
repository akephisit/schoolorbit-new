#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Timelike, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    BlockedWindow, BlockedWindowInput, ClearMismatchedExamItemsResult, CreateExamRoundRequest,
    DayRoomAssignmentView, ExamDay, ExamDayDetail, ExamDayRoomAssignmentView,
    ExamInvigilatorAssignmentSummary, ExamInvigilatorDayWorkload, ExamInvigilatorStaffOption,
    ExamInvigilatorStaffWorkload, ExamInvigilatorView, ExamInvigilatorWorkspace, ExamRound,
    ExamScheduleItemView, ExamScheduleReadiness, ExamScheduleWorkspace, ExamSessionView,
    GenerateSeatsRequest, ImportExamItemsRequest, ImportExamItemsResult, InvigilatorView,
    PersonalExamScheduleRound, PersonalExamSessionView, PlaceExamSessionRequest,
    SeatAssignmentView, UpdateExamInvigilatorsRequest, UpdateExamRoundRequest,
    UpsertDayRoomAssignmentRequest, UpsertExamDayRequest,
};

#[derive(Debug, Clone, Copy)]
pub struct WorkspaceCounts {
    pub day_count: i64,
    pub item_count: i64,
    pub unscheduled_count: i64,
    pub missing_room_assignment_count: i64,
    pub invalid_session_count: i64,
    pub missing_seat_student_count: i64,
    pub invigilator_conflict_count: i64,
}

const EXAM_SESSION_SLOT_MINUTES: u32 = 5;
const EXAM_SESSION_CLASSROOM_LOCK_NAMESPACE: i64 = 0x4558_5343_4C52_0000;
const EXAM_SESSION_ROOM_LOCK_NAMESPACE: i64 = 0x4558_5352_4F4D_0000;
const EXAM_INVIGILATOR_STAFF_LOCK_NAMESPACE: i64 = 0x4558_5349_4E56_0000;
const INVIGILATOR_STAFF_OPTION_DEFAULT_LIMIT: i64 = 40;
const INVIGILATOR_STAFF_OPTION_MAX_LIMIT: i64 = 100;

#[derive(Debug, sqlx::FromRow)]
struct ExamDayGradeLevelRow {
    exam_day_id: Uuid,
    grade_level_id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct BlockedWindowRow {
    exam_day_id: Uuid,
    id: Uuid,
    label: String,
    start_time: NaiveTime,
    end_time: NaiveTime,
}

#[derive(Debug, sqlx::FromRow)]
struct ExamDayRoomAssignmentRow {
    id: Uuid,
    exam_day_id: Uuid,
    classroom_id: Uuid,
    room_id: Uuid,
    capacity_override: Option<i32>,
    classroom_name: Option<String>,
    room_name: Option<String>,
    room_capacity: Option<i32>,
}

#[derive(Debug, sqlx::FromRow)]
struct ExamSessionRow {
    id: Uuid,
    exam_schedule_item_id: Uuid,
    exam_round_id: Uuid,
    exam_day_id: Uuid,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
    academic_semester_id: Uuid,
    assessment_category_id: Uuid,
    assessment_plan_id: Uuid,
    classroom_course_id: Uuid,
    classroom_id: Uuid,
    subject_id: Uuid,
    grade_level_id: Uuid,
    duration_minutes: i32,
    imported_at: DateTime<Utc>,
    exam_date: Option<NaiveDate>,
    assessment_category_name: Option<String>,
    subject_code: Option<String>,
    subject_name_th: Option<String>,
    subject_name_en: Option<String>,
    subject_group_id: Option<Uuid>,
    subject_group_name: Option<String>,
    subject_group_display_order: Option<i32>,
    subject_type: Option<String>,
    classroom_name: Option<String>,
    grade_level_name: Option<String>,
    grade_level_type: Option<String>,
    grade_level_year: Option<i32>,
    day_room_assignment_id: Option<Uuid>,
    room_id: Option<Uuid>,
    room_name: Option<String>,
    building_name: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct DayRoomAssignmentViewRow {
    id: Uuid,
    exam_day_id: Uuid,
    classroom_id: Uuid,
    classroom_name: String,
    room_id: Uuid,
    room_name: String,
    building_name: Option<String>,
    room_capacity: Option<i32>,
    capacity_override: Option<i32>,
    seats_generated: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct InvigilatorViewRow {
    day_room_assignment_id: Uuid,
    staff_id: Uuid,
    display_name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct InvigilatorAssignmentSummaryRow {
    assignment_id: Uuid,
    exam_day_id: Uuid,
    classroom_id: Uuid,
    classroom_name: String,
    room_id: Uuid,
    room_name: String,
    session_minutes: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct InvigilatorSessionWindowRow {
    assignment_id: Uuid,
    exam_day_id: Uuid,
    staff_id: Uuid,
    staff_name: String,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
}

#[derive(Debug, sqlx::FromRow)]
struct ExamDayContext {
    exam_round_id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct ClassroomAssignmentContext {
    classroom_id: Uuid,
    classroom_name: String,
    grade_level_id: Uuid,
    is_active: Option<bool>,
}

#[derive(Debug, sqlx::FromRow)]
struct RoomAssignmentContext {
    room_id: Uuid,
    capacity: i32,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct SeatAssignmentContext {
    assignment_id: Uuid,
    exam_round_id: Uuid,
    classroom_id: Uuid,
    capacity_override: Option<i32>,
    room_capacity: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct ExamScheduleItemPlacementContext {
    id: Uuid,
    exam_round_id: Uuid,
    academic_semester_id: Uuid,
    assessment_category_id: Uuid,
    assessment_plan_id: Uuid,
    classroom_course_id: Uuid,
    classroom_id: Uuid,
    subject_id: Uuid,
    grade_level_id: Uuid,
    duration_minutes: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct ExamDayPlacementContext {
    id: Uuid,
    exam_round_id: Uuid,
    start_time: NaiveTime,
    end_time: NaiveTime,
}

#[derive(Debug, sqlx::FromRow)]
struct DayRoomAssignmentPlacementContext {
    id: Uuid,
    room_id: Uuid,
}

#[derive(Debug, Clone)]
struct CandidateRoomSession {
    session_id: Option<Uuid>,
    room_id: Uuid,
    exam_day_id: Uuid,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
}

#[derive(Debug, sqlx::FromRow)]
struct PersonalExamSessionRow {
    round_id: Uuid,
    round_name: String,
    academic_semester_id: Uuid,
    published_at: Option<DateTime<Utc>>,
    exam_date: NaiveDate,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
    subject_name: String,
    assessment_category_name: String,
    classroom_name: String,
    room_name: String,
    building_name: Option<String>,
    seat_number: Option<String>,
}

struct NormalizedUpdateRoundRequest {
    name: Option<String>,
    description: Option<String>,
    exam_kind: Option<String>,
}

impl PersonalExamSessionRow {
    fn into_session_view(self) -> PersonalExamSessionView {
        PersonalExamSessionView {
            exam_date: self.exam_date,
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            subject_name: self.subject_name,
            assessment_category_name: self.assessment_category_name,
            classroom_name: self.classroom_name,
            room_name: self.room_name,
            building_name: self.building_name,
            seat_number: self.seat_number,
        }
    }
}

impl ExamDayRoomAssignmentRow {
    fn into_view(self, invigilators: Vec<ExamInvigilatorView>) -> ExamDayRoomAssignmentView {
        ExamDayRoomAssignmentView {
            id: self.id,
            exam_day_id: self.exam_day_id,
            classroom_id: self.classroom_id,
            room_id: self.room_id,
            capacity_override: self.capacity_override,
            classroom_name: self.classroom_name,
            room_name: self.room_name,
            room_capacity: self.room_capacity,
            invigilators,
        }
    }
}

impl ExamSessionRow {
    fn into_view(self, invigilators: Vec<ExamInvigilatorView>) -> ExamSessionView {
        ExamSessionView {
            id: self.id,
            exam_schedule_item_id: self.exam_schedule_item_id,
            exam_round_id: self.exam_round_id,
            exam_day_id: self.exam_day_id,
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            academic_semester_id: self.academic_semester_id,
            assessment_category_id: self.assessment_category_id,
            assessment_plan_id: self.assessment_plan_id,
            classroom_course_id: self.classroom_course_id,
            classroom_id: self.classroom_id,
            subject_id: self.subject_id,
            grade_level_id: self.grade_level_id,
            duration_minutes: self.duration_minutes,
            imported_at: self.imported_at,
            exam_date: self.exam_date,
            assessment_category_name: self.assessment_category_name,
            subject_code: self.subject_code,
            subject_name_th: self.subject_name_th,
            subject_name_en: self.subject_name_en,
            subject_group_id: self.subject_group_id,
            subject_group_name: self.subject_group_name,
            subject_group_display_order: self.subject_group_display_order,
            subject_type: self.subject_type,
            classroom_name: self.classroom_name,
            grade_level_name: self.grade_level_name,
            grade_level_type: self.grade_level_type,
            grade_level_year: self.grade_level_year,
            room_id: self.room_id,
            room_name: self.room_name,
            building_name: self.building_name,
            invigilators,
        }
    }
}

impl DayRoomAssignmentViewRow {
    fn into_view(self, invigilators: Vec<InvigilatorView>) -> DayRoomAssignmentView {
        DayRoomAssignmentView {
            id: self.id,
            exam_day_id: self.exam_day_id,
            classroom_id: self.classroom_id,
            classroom_name: self.classroom_name,
            room_id: self.room_id,
            room_name: self.room_name,
            building_name: self.building_name,
            room_capacity: self.room_capacity,
            capacity_override: self.capacity_override,
            invigilators,
            seats_generated: self.seats_generated,
        }
    }
}

impl InvigilatorViewRow {
    fn into_view(self) -> InvigilatorView {
        InvigilatorView {
            staff_id: self.staff_id,
            display_name: self.display_name,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SessionValidationError {
    InvalidDuration,
    EndTimeOverflow,
    StartTimeOutsideSlot,
    BeforeDayStart,
    AfterDayEnd,
    BlockedWindow(String),
}

pub fn add_minutes(start: NaiveTime, minutes: i32) -> Result<NaiveTime, SessionValidationError> {
    if minutes <= 0 {
        return Err(SessionValidationError::InvalidDuration);
    }

    let (end_time, day_delta) = start.overflowing_add_signed(Duration::minutes(i64::from(minutes)));
    if day_delta == 0 {
        Ok(end_time)
    } else {
        Err(SessionValidationError::EndTimeOverflow)
    }
}

pub fn time_ranges_overlap(
    left_start: NaiveTime,
    left_end: NaiveTime,
    right_start: NaiveTime,
    right_end: NaiveTime,
) -> bool {
    left_start < right_end && right_start < left_end
}

fn is_exam_session_start_on_slot(starts_at: NaiveTime) -> bool {
    starts_at.num_seconds_from_midnight() % (EXAM_SESSION_SLOT_MINUTES * 60) == 0
}

#[derive(Debug, Clone)]
pub struct CandidateSession {
    pub session_id: Option<Uuid>,
    pub classroom_id: Uuid,
    pub exam_day_id: Uuid,
    pub starts_at: NaiveTime,
    pub ends_at: NaiveTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvigilatorSessionWindow {
    pub assignment_id: Uuid,
    pub exam_day_id: Uuid,
    pub staff_id: Uuid,
    pub starts_at: NaiveTime,
    pub ends_at: NaiveTime,
}

pub fn invigilator_workload_minutes(windows: &[InvigilatorSessionWindow]) -> i32 {
    windows
        .iter()
        .map(|window| minutes_between_times(window.starts_at, window.ends_at))
        .sum()
}

fn minutes_between_times(starts_at: NaiveTime, ends_at: NaiveTime) -> i32 {
    let start_minutes = starts_at.num_seconds_from_midnight() / 60;
    let end_minutes = ends_at.num_seconds_from_midnight() / 60;
    end_minutes.saturating_sub(start_minutes) as i32
}

pub fn has_invigilator_time_conflict(
    candidate_assignment_id: Uuid,
    candidate_windows: &[InvigilatorSessionWindow],
    existing_windows: &[InvigilatorSessionWindow],
) -> bool {
    candidate_windows.iter().any(|candidate| {
        existing_windows.iter().any(|existing| {
            existing.assignment_id != candidate_assignment_id
                && existing.staff_id == candidate.staff_id
                && existing.exam_day_id == candidate.exam_day_id
                && time_ranges_overlap(
                    candidate.starts_at,
                    candidate.ends_at,
                    existing.starts_at,
                    existing.ends_at,
                )
        })
    })
}

pub fn has_same_classroom_conflict(
    candidate: &CandidateSession,
    existing: &[CandidateSession],
) -> bool {
    existing.iter().any(|item| {
        item.exam_day_id == candidate.exam_day_id
            && item.classroom_id == candidate.classroom_id
            && item.session_id != candidate.session_id
            && time_ranges_overlap(
                candidate.starts_at,
                candidate.ends_at,
                item.starts_at,
                item.ends_at,
            )
    })
}

fn has_same_room_conflict(
    candidate: &CandidateRoomSession,
    existing: &[CandidateRoomSession],
) -> bool {
    existing.iter().any(|item| {
        item.exam_day_id == candidate.exam_day_id
            && item.room_id == candidate.room_id
            && item.session_id != candidate.session_id
            && time_ranges_overlap(
                candidate.starts_at,
                candidate.ends_at,
                item.starts_at,
                item.ends_at,
            )
    })
}

fn exam_session_conflict_lock_key(namespace: i64, exam_day_id: Uuid, resource_id: Uuid) -> i64 {
    let day_bytes = exam_day_id.as_bytes();
    let resource_bytes = resource_id.as_bytes();
    let mut key_bytes = [0_u8; 8];
    for index in 0..8 {
        key_bytes[index] = day_bytes[index]
            ^ day_bytes[index + 8]
            ^ resource_bytes[index]
            ^ resource_bytes[index + 8];
    }
    i64::from_be_bytes(key_bytes) ^ namespace
}

fn exam_session_conflict_lock_keys(
    exam_day_id: Uuid,
    classroom_id: Uuid,
    room_id: Uuid,
) -> [i64; 2] {
    let mut keys = [
        exam_session_conflict_lock_key(
            EXAM_SESSION_CLASSROOM_LOCK_NAMESPACE,
            exam_day_id,
            classroom_id,
        ),
        exam_session_conflict_lock_key(EXAM_SESSION_ROOM_LOCK_NAMESPACE, exam_day_id, room_id),
    ];
    keys.sort_unstable();
    keys
}

async fn lock_exam_session_conflict_scope(
    tx: &mut Transaction<'_, Postgres>,
    exam_day_id: Uuid,
    classroom_id: Uuid,
    room_id: Uuid,
) -> Result<(), AppError> {
    for lock_key in exam_session_conflict_lock_keys(exam_day_id, classroom_id, room_id) {
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

fn exam_invigilator_staff_lock_keys(exam_day_id: Uuid, staff_ids: &[Uuid]) -> Vec<i64> {
    let mut keys = unique_uuids(staff_ids.to_vec())
        .into_iter()
        .map(|staff_id| {
            exam_session_conflict_lock_key(
                EXAM_INVIGILATOR_STAFF_LOCK_NAMESPACE,
                exam_day_id,
                staff_id,
            )
        })
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

async fn lock_exam_invigilator_staff_conflict_scope(
    tx: &mut Transaction<'_, Postgres>,
    exam_day_id: Uuid,
    staff_ids: &[Uuid],
) -> Result<(), AppError> {
    if staff_ids.is_empty() {
        return Ok(());
    }

    for lock_key in exam_invigilator_staff_lock_keys(exam_day_id, staff_ids) {
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

pub fn validate_session_window(
    starts_at: NaiveTime,
    duration_minutes: i32,
    day_start: NaiveTime,
    day_end: NaiveTime,
    blocked_windows: &[BlockedWindow],
) -> Result<NaiveTime, SessionValidationError> {
    let ends_at = add_minutes(starts_at, duration_minutes)?;
    if !is_exam_session_start_on_slot(starts_at) {
        return Err(SessionValidationError::StartTimeOutsideSlot);
    }
    if starts_at < day_start {
        return Err(SessionValidationError::BeforeDayStart);
    }
    if ends_at > day_end {
        return Err(SessionValidationError::AfterDayEnd);
    }
    for blocked in blocked_windows {
        if time_ranges_overlap(starts_at, ends_at, blocked.start_time, blocked.end_time) {
            return Err(SessionValidationError::BlockedWindow(blocked.label.clone()));
        }
    }
    Ok(ends_at)
}

pub fn validation_error_to_app_error(error: SessionValidationError) -> AppError {
    match error {
        SessionValidationError::InvalidDuration => {
            AppError::BadRequest("Exam duration must be greater than zero".into())
        }
        SessionValidationError::EndTimeOverflow => {
            AppError::BadRequest("Exam end time is outside the valid day range".into())
        }
        SessionValidationError::StartTimeOutsideSlot => AppError::BadRequest(format!(
            "Exam start time must align to a {EXAM_SESSION_SLOT_MINUTES}-minute slot"
        )),
        SessionValidationError::BeforeDayStart => {
            AppError::BadRequest("Exam starts before the exam day begins".into())
        }
        SessionValidationError::AfterDayEnd => {
            AppError::BadRequest("Exam ends after the exam day ends".into())
        }
        SessionValidationError::BlockedWindow(label) => {
            AppError::BadRequest(format!("Exam overlaps blocked window: {label}"))
        }
    }
}

pub fn build_readiness(counts: WorkspaceCounts) -> ExamScheduleReadiness {
    let mut blockers = Vec::new();
    if counts.day_count == 0 {
        blockers.push("Add at least one exam day".to_string());
    }
    if counts.item_count == 0 {
        blockers.push("Import in-timetable assessment categories".to_string());
    }
    if counts.unscheduled_count > 0 {
        blockers.push(format!(
            "Schedule {} remaining unscheduled exam item(s)",
            counts.unscheduled_count
        ));
    }
    if counts.missing_room_assignment_count > 0 {
        blockers.push(format!(
            "Assign rooms for {} classroom-day group(s)",
            counts.missing_room_assignment_count
        ));
    }
    if counts.invalid_session_count > 0 {
        blockers.push(format!(
            "Fix {} scheduled exam session(s) that no longer fit the exam day",
            counts.invalid_session_count
        ));
    }
    if counts.missing_seat_student_count > 0 {
        blockers.push(format!(
            "Generate seats for {} active student(s) in scheduled classroom-day group(s)",
            counts.missing_seat_student_count
        ));
    }
    if counts.invigilator_conflict_count > 0 {
        blockers.push(format!(
            "Fix {} overlapping invigilator supervision assignment(s)",
            counts.invigilator_conflict_count
        ));
    }
    ExamScheduleReadiness {
        can_publish: blockers.is_empty(),
        blockers,
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SeatStudent {
    pub student_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatAssignmentDraft {
    pub student_id: Uuid,
    pub seat_number: String,
}

pub fn build_default_seat_assignments(students: &[SeatStudent]) -> Vec<SeatAssignmentDraft> {
    students
        .iter()
        .enumerate()
        .map(|(index, student)| SeatAssignmentDraft {
            student_id: student.student_id,
            seat_number: format!("{:02}", index + 1),
        })
        .collect()
}

pub fn validate_seat_generation_capacity(
    active_student_count: usize,
    effective_capacity: i32,
) -> Result<(), AppError> {
    if effective_capacity <= 0 {
        return Err(AppError::BadRequest(
            "Room capacity must be greater than zero".to_string(),
        ));
    }
    if active_student_count > effective_capacity as usize {
        return Err(AppError::BadRequest(format!(
            "Classroom has {active_student_count} active student(s), which exceeds the room capacity of {effective_capacity}"
        )));
    }
    Ok(())
}

pub async fn list_rounds(
    pool: &PgPool,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<ExamRound>, AppError> {
    let rows = sqlx::query_as::<_, ExamRound>(
        r#"
        SELECT id,
               academic_semester_id,
               name,
               description,
               exam_kind,
               status,
               published_at,
               created_at,
               updated_at
        FROM academic_exam_rounds
        WHERE ($1::uuid IS NULL OR academic_semester_id = $1)
        ORDER BY created_at DESC, name ASC
        "#,
    )
    .bind(academic_semester_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn create_round(
    pool: &PgPool,
    request: CreateExamRoundRequest,
    actor_user_id: Uuid,
) -> Result<ExamRound, AppError> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "Exam round name is required".to_string(),
        ));
    }
    let exam_kind = normalize_exam_kind(request.exam_kind.as_deref())?;

    let row = sqlx::query_as::<_, ExamRound>(
        r#"
        INSERT INTO academic_exam_rounds (
            academic_semester_id,
            name,
            description,
            exam_kind,
            created_by,
            updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $5)
        RETURNING id,
                  academic_semester_id,
                  name,
                  description,
                  exam_kind,
                  status,
                  published_at,
                  created_at,
                  updated_at
        "#,
    )
    .bind(request.academic_semester_id)
    .bind(name)
    .bind(request.description)
    .bind(exam_kind)
    .bind(actor_user_id)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn update_round(
    pool: &PgPool,
    round_id: Uuid,
    request: UpdateExamRoundRequest,
    actor_user_id: Uuid,
) -> Result<ExamRound, AppError> {
    let normalized = normalize_update_round_request(request)?;

    let mut tx = pool.begin().await?;
    mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;

    let row = sqlx::query_as::<_, ExamRound>(
        r#"
        UPDATE academic_exam_rounds
        SET name = COALESCE($2, name),
            description = COALESCE($3, description),
            exam_kind = COALESCE($4, exam_kind),
            updated_by = $5,
            updated_at = now()
        WHERE id = $1
        RETURNING id,
                  academic_semester_id,
                  name,
                  description,
                  exam_kind,
                  status,
                  published_at,
                  created_at,
                  updated_at
        "#,
    )
    .bind(round_id)
    .bind(normalized.name)
    .bind(normalized.description)
    .bind(normalized.exam_kind)
    .bind(actor_user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam round not found".to_string()))?;

    tx.commit().await?;
    Ok(row)
}

pub async fn upsert_exam_day(
    pool: &PgPool,
    round_id: Uuid,
    request: UpsertExamDayRequest,
) -> Result<ExamDayDetail, AppError> {
    validate_exam_day_window(request.start_time, request.end_time)?;
    let blocked_windows = normalize_blocked_windows(
        request.start_time,
        request.end_time,
        request.blocked_windows,
    )?;
    let grade_level_ids = unique_uuids(request.grade_level_ids);

    let mut tx = pool.begin().await?;
    let round_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM academic_exam_rounds
            WHERE id = $1
        )
        "#,
    )
    .bind(round_id)
    .fetch_one(&mut *tx)
    .await?;

    if !round_exists {
        return Err(AppError::NotFound("Exam round not found".to_string()));
    }

    let day = sqlx::query_as::<_, ExamDay>(
        r#"
        INSERT INTO academic_exam_days (
            exam_round_id,
            exam_date,
            label,
            start_time,
            end_time
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (exam_round_id, exam_date)
        DO UPDATE SET
            label = EXCLUDED.label,
            start_time = EXCLUDED.start_time,
            end_time = EXCLUDED.end_time,
            updated_at = now()
        RETURNING id,
                  exam_round_id,
                  exam_date,
                  label,
                  start_time,
                  end_time
        "#,
    )
    .bind(round_id)
    .bind(request.exam_date)
    .bind(request.label)
    .bind(request.start_time)
    .bind(request.end_time)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM academic_exam_day_grade_levels WHERE exam_day_id = $1")
        .bind(day.id)
        .execute(&mut *tx)
        .await?;

    if !grade_level_ids.is_empty() {
        sqlx::query(
            r#"
            INSERT INTO academic_exam_day_grade_levels (exam_day_id, grade_level_id)
            SELECT $1, grade_level_id
            FROM unnest($2::uuid[]) AS grade_level_id
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(day.id)
        .bind(&grade_level_ids)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("DELETE FROM academic_exam_day_blocked_windows WHERE exam_day_id = $1")
        .bind(day.id)
        .execute(&mut *tx)
        .await?;

    if !blocked_windows.is_empty() {
        let labels: Vec<String> = blocked_windows
            .iter()
            .map(|window| window.label.clone())
            .collect();
        let start_times: Vec<NaiveTime> = blocked_windows
            .iter()
            .map(|window| window.start_time)
            .collect();
        let end_times: Vec<NaiveTime> = blocked_windows
            .iter()
            .map(|window| window.end_time)
            .collect();

        sqlx::query(
            r#"
            INSERT INTO academic_exam_day_blocked_windows (
                exam_day_id,
                label,
                start_time,
                end_time
            )
            SELECT $1, label, start_time, end_time
            FROM unnest($2::text[], $3::time[], $4::time[])
                AS blocked_window(label, start_time, end_time)
            "#,
        )
        .bind(day.id)
        .bind(&labels)
        .bind(&start_times)
        .bind(&end_times)
        .execute(&mut *tx)
        .await?;
    }

    mark_round_draft_after_mutation(&mut tx, round_id, None).await?;
    tx.commit().await?;

    fetch_exam_day_detail(pool, day.id).await
}

pub async fn delete_exam_day(pool: &PgPool, exam_day_id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    let round_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        DELETE FROM academic_exam_days
        WHERE id = $1
        RETURNING exam_round_id
        "#,
    )
    .bind(exam_day_id)
    .fetch_optional(&mut *tx)
    .await?;

    let round_id = round_id.ok_or_else(|| AppError::NotFound("Exam day not found".to_string()))?;

    mark_round_draft_after_mutation(&mut tx, round_id, None).await?;
    tx.commit().await?;

    Ok(())
}

pub async fn get_workspace(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<ExamScheduleWorkspace, AppError> {
    let round = fetch_round(pool, round_id).await?;
    let days = fetch_exam_day_details_for_round(pool, round_id).await?;
    let unscheduled_items = fetch_unscheduled_items(pool, round_id).await?;
    let scheduled_sessions = fetch_scheduled_sessions(pool, round_id).await?;
    let counts = fetch_workspace_counts(pool, round_id).await?;

    Ok(ExamScheduleWorkspace {
        round,
        days,
        unscheduled_items,
        scheduled_sessions,
        readiness: build_readiness(counts),
    })
}

pub async fn get_invigilator_workspace(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<ExamInvigilatorWorkspace, AppError> {
    fetch_round(pool, round_id).await?;
    let assignments = fetch_invigilator_assignment_summaries(pool, round_id).await?;
    let assignment_ids: Vec<Uuid> = assignments.iter().map(|item| item.assignment_id).collect();
    let mut invigilators_by_assignment =
        fetch_invigilator_views_by_assignment_ids(pool, &assignment_ids).await?;
    let staff_workloads = fetch_invigilator_staff_workloads(pool, round_id).await?;

    Ok(ExamInvigilatorWorkspace {
        round_id,
        assignments: assignments
            .into_iter()
            .map(|row| ExamInvigilatorAssignmentSummary {
                assignment_id: row.assignment_id,
                exam_day_id: row.exam_day_id,
                classroom_id: row.classroom_id,
                classroom_name: row.classroom_name,
                room_id: row.room_id,
                room_name: row.room_name,
                session_minutes: row.session_minutes,
                invigilators: invigilators_by_assignment
                    .remove(&row.assignment_id)
                    .unwrap_or_default(),
            })
            .collect(),
        staff_workloads,
    })
}

pub async fn list_invigilator_staff_options(
    pool: &PgPool,
    round_id: Uuid,
    search: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<ExamInvigilatorStaffOption>, AppError> {
    fetch_round(pool, round_id).await?;
    let search_pattern = invigilator_staff_option_search_pattern(search);
    let limit = invigilator_staff_option_limit(limit);

    sqlx::query_as::<_, ExamInvigilatorStaffOption>(
        r#"
        SELECT user_account.id AS staff_id,
               COALESCE(
                   NULLIF(
                       concat_ws(' ', user_account.title, user_account.first_name, user_account.last_name),
                       ''
                   ),
                   user_account.id::TEXT
               ) AS display_name
        FROM users user_account
        WHERE user_account.user_type = 'staff'
          AND user_account.status = 'active'
          AND (
              $1::TEXT IS NULL
              OR user_account.first_name ILIKE $1
              OR user_account.last_name ILIKE $1
              OR concat_ws(' ', user_account.title, user_account.first_name, user_account.last_name) ILIKE $1
          )
        ORDER BY user_account.first_name, user_account.last_name, user_account.id
        LIMIT $2
        "#,
    )
    .bind(search_pattern)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub async fn import_exam_items(
    pool: &PgPool,
    round_id: Uuid,
    request: ImportExamItemsRequest,
    actor_user_id: Uuid,
) -> Result<ImportExamItemsResult, AppError> {
    let grade_level_ids = request.grade_level_ids.map(unique_uuids);
    let mut tx = pool.begin().await?;

    let round_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM academic_exam_rounds
            WHERE id = $1
        )
        "#,
    )
    .bind(round_id)
    .fetch_one(&mut *tx)
    .await?;

    if !round_exists {
        return Err(AppError::NotFound("Exam round not found".to_string()));
    }

    let skipped_existing_count: i64 = sqlx::query_scalar(
        r#"
        WITH round_context AS (
          SELECT id AS exam_round_id, academic_semester_id, exam_kind
          FROM academic_exam_rounds
          WHERE id = $1
        ),
        source_items AS (
          SELECT
            rc.exam_round_id,
            c.id AS assessment_category_id,
            cr.id AS classroom_id
          FROM round_context rc
          JOIN academic_assessment_plans ap
            ON ap.academic_semester_id = rc.academic_semester_id
          JOIN academic_assessment_categories c
            ON c.plan_id = ap.id
          JOIN classroom_courses cc
            ON cc.academic_semester_id = rc.academic_semester_id
           AND cc.subject_id = ap.subject_id
          JOIN class_rooms cr
            ON cr.id = cc.classroom_id
          WHERE c.exam_mode = 'in_timetable'
            AND c.code = rc.exam_kind
            AND c.exam_duration_minutes IS NOT NULL
            AND cr.is_active = true
            AND ($2::uuid[] IS NULL OR cr.grade_level_id = ANY($2))
        )
        SELECT COUNT(*)::BIGINT
        FROM source_items source
        WHERE EXISTS (
            SELECT 1
            FROM academic_exam_schedule_items item
            WHERE item.exam_round_id = source.exam_round_id
              AND item.assessment_category_id = source.assessment_category_id
              AND item.classroom_id = source.classroom_id
        )
        "#,
    )
    .bind(round_id)
    .bind(grade_level_ids.clone())
    .fetch_one(&mut *tx)
    .await?;

    let skipped_missing_duration_count: i64 = sqlx::query_scalar(
        r#"
        WITH round_context AS (
          SELECT id AS exam_round_id, academic_semester_id, exam_kind
          FROM academic_exam_rounds
          WHERE id = $1
        )
        SELECT COUNT(*)::BIGINT
        FROM round_context rc
        JOIN academic_assessment_plans ap
          ON ap.academic_semester_id = rc.academic_semester_id
        JOIN academic_assessment_categories c
          ON c.plan_id = ap.id
        JOIN classroom_courses cc
          ON cc.academic_semester_id = rc.academic_semester_id
         AND cc.subject_id = ap.subject_id
        JOIN class_rooms cr
          ON cr.id = cc.classroom_id
        WHERE c.exam_mode = 'in_timetable'
          AND c.code = rc.exam_kind
          AND c.exam_duration_minutes IS NULL
          AND cr.is_active = true
          AND ($2::uuid[] IS NULL OR cr.grade_level_id = ANY($2))
        "#,
    )
    .bind(round_id)
    .bind(grade_level_ids.clone())
    .fetch_one(&mut *tx)
    .await?;

    let insert_result = sqlx::query(
        r#"
        WITH round_context AS (
          SELECT id AS exam_round_id, academic_semester_id, exam_kind
          FROM academic_exam_rounds
          WHERE id = $1
        ),
        source_items AS (
          SELECT
            rc.exam_round_id,
            rc.academic_semester_id,
            c.id AS assessment_category_id,
            ap.id AS assessment_plan_id,
            cc.id AS classroom_course_id,
            cr.id AS classroom_id,
            ap.subject_id,
            cr.grade_level_id,
            c.exam_duration_minutes AS duration_minutes
          FROM round_context rc
          JOIN academic_assessment_plans ap
            ON ap.academic_semester_id = rc.academic_semester_id
          JOIN academic_assessment_categories c
            ON c.plan_id = ap.id
          JOIN classroom_courses cc
            ON cc.academic_semester_id = rc.academic_semester_id
           AND cc.subject_id = ap.subject_id
          JOIN class_rooms cr
            ON cr.id = cc.classroom_id
          WHERE c.exam_mode = 'in_timetable'
            AND c.code = rc.exam_kind
            AND c.exam_duration_minutes IS NOT NULL
            AND cr.is_active = true
            AND ($2::uuid[] IS NULL OR cr.grade_level_id = ANY($2))
        )
        INSERT INTO academic_exam_schedule_items (
          exam_round_id,
          academic_semester_id,
          assessment_category_id,
          assessment_plan_id,
          classroom_course_id,
          classroom_id,
          subject_id,
          grade_level_id,
          duration_minutes
        )
        SELECT
          exam_round_id,
          academic_semester_id,
          assessment_category_id,
          assessment_plan_id,
          classroom_course_id,
          classroom_id,
          subject_id,
          grade_level_id,
          duration_minutes
        FROM source_items
        ON CONFLICT (exam_round_id, assessment_category_id, classroom_id) DO NOTHING
        "#,
    )
    .bind(round_id)
    .bind(grade_level_ids)
    .execute(&mut *tx)
    .await?;

    mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;
    tx.commit().await?;

    Ok(ImportExamItemsResult {
        inserted_count: insert_result.rows_affected() as i64,
        skipped_existing_count,
        skipped_missing_duration_count,
    })
}

pub async fn clear_mismatched_exam_items(
    pool: &PgPool,
    round_id: Uuid,
    actor_user_id: Uuid,
) -> Result<ClearMismatchedExamItemsResult, AppError> {
    let mut tx = pool.begin().await?;

    let _round_status: String = sqlx::query_scalar(
        r#"
        SELECT status
        FROM academic_exam_rounds
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(round_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam round not found".to_string()))?;

    let delete_result = sqlx::query(
        r#"
        WITH round_context AS (
          SELECT id AS exam_round_id, exam_kind
          FROM academic_exam_rounds
          WHERE id = $1
        )
        DELETE FROM academic_exam_schedule_items item
        USING academic_assessment_categories c, round_context rc
        WHERE item.exam_round_id = rc.exam_round_id
          AND item.assessment_category_id = c.id
          AND c.code IS DISTINCT FROM rc.exam_kind
        "#,
    )
    .bind(round_id)
    .execute(&mut *tx)
    .await?;

    mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;
    tx.commit().await?;

    Ok(ClearMismatchedExamItemsResult {
        deleted_count: delete_result.rows_affected() as i64,
    })
}

pub async fn list_day_room_assignments(
    pool: &PgPool,
    exam_day_id: Uuid,
) -> Result<Vec<DayRoomAssignmentView>, AppError> {
    let day_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM academic_exam_days
            WHERE id = $1
        )
        "#,
    )
    .bind(exam_day_id)
    .fetch_one(pool)
    .await?;

    if !day_exists {
        return Err(AppError::NotFound("Exam day not found".to_string()));
    }

    fetch_day_room_assignment_views_for_day(pool, exam_day_id).await
}

pub async fn upsert_day_room_assignment(
    pool: &PgPool,
    exam_day_id: Uuid,
    request: UpsertDayRoomAssignmentRequest,
    actor_user_id: Uuid,
) -> Result<DayRoomAssignmentView, AppError> {
    let invigilator_staff_ids = request
        .invigilator_staff_ids
        .as_ref()
        .map(|ids| validate_unique_invigilator_staff_ids(ids.clone()))
        .transpose()?;
    let capacity_override = validate_capacity_override(request.capacity_override)?;

    let mut tx = pool.begin().await?;
    let day_context = fetch_exam_day_context_for_update(&mut tx, exam_day_id).await?;
    let classroom = fetch_classroom_assignment_context(&mut tx, request.classroom_id).await?;
    if classroom.is_active != Some(true) {
        return Err(AppError::BadRequest(
            "Classroom must be active before assigning an exam room".to_string(),
        ));
    }
    validate_day_allows_grade_level(&mut tx, exam_day_id, classroom.grade_level_id).await?;

    let room = fetch_room_assignment_context(&mut tx, request.room_id).await?;
    if room.status != "ACTIVE" {
        return Err(AppError::BadRequest(
            "Room must be ACTIVE before assigning it to an exam day".to_string(),
        ));
    }

    let effective_capacity = capacity_override.unwrap_or(room.capacity);
    let active_student_count =
        count_active_classroom_students(&mut tx, request.classroom_id).await?;
    if active_student_count > i64::from(effective_capacity) {
        return Err(AppError::BadRequest(format!(
            "Classroom has {active_student_count} active student(s), which exceeds the room capacity of {effective_capacity}"
        )));
    }

    let assignment_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO academic_exam_day_room_assignments (
            exam_day_id,
            classroom_id,
            room_id,
            capacity_override,
            created_by,
            updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $5)
        ON CONFLICT (exam_day_id, classroom_id)
        DO UPDATE SET
            room_id = EXCLUDED.room_id,
            capacity_override = EXCLUDED.capacity_override,
            updated_by = EXCLUDED.updated_by,
            updated_at = now()
        RETURNING id
        "#,
    )
    .bind(exam_day_id)
    .bind(request.classroom_id)
    .bind(request.room_id)
    .bind(capacity_override)
    .bind(actor_user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_day_room_assignment_write_error)?;

    if let Some(invigilator_staff_ids) = invigilator_staff_ids {
        lock_exam_invigilator_staff_conflict_scope(&mut tx, exam_day_id, &invigilator_staff_ids)
            .await?;
        validate_invigilator_time_conflicts(
            &mut tx,
            day_context.exam_round_id,
            assignment_id,
            &invigilator_staff_ids,
        )
        .await?;
        replace_assignment_invigilators_in_tx(
            &mut tx,
            day_context.exam_round_id,
            exam_day_id,
            assignment_id,
            &invigilator_staff_ids,
        )
        .await?;
    }

    mark_round_draft_after_mutation(&mut tx, day_context.exam_round_id, Some(actor_user_id))
        .await?;
    tx.commit().await?;

    fetch_day_room_assignment_view(pool, assignment_id).await
}

pub async fn update_assignment_invigilators(
    pool: &PgPool,
    assignment_id: Uuid,
    request: UpdateExamInvigilatorsRequest,
    actor_user_id: Uuid,
) -> Result<DayRoomAssignmentView, AppError> {
    let invigilator_staff_ids =
        validate_unique_invigilator_staff_ids(request.invigilator_staff_ids)?;
    let mut tx = pool.begin().await?;
    let context = fetch_seat_assignment_context(&mut tx, assignment_id).await?;
    let exam_day_id: Uuid = sqlx::query_scalar(
        "SELECT exam_day_id FROM academic_exam_day_room_assignments WHERE id = $1",
    )
    .bind(assignment_id)
    .fetch_one(&mut *tx)
    .await?;

    lock_exam_invigilator_staff_conflict_scope(&mut tx, exam_day_id, &invigilator_staff_ids)
        .await?;
    validate_invigilator_time_conflicts(
        &mut tx,
        context.exam_round_id,
        assignment_id,
        &invigilator_staff_ids,
    )
    .await?;
    replace_assignment_invigilators_in_tx(
        &mut tx,
        context.exam_round_id,
        exam_day_id,
        assignment_id,
        &invigilator_staff_ids,
    )
    .await?;
    mark_round_draft_after_mutation(&mut tx, context.exam_round_id, Some(actor_user_id)).await?;
    tx.commit().await?;

    fetch_day_room_assignment_view(pool, assignment_id).await
}

async fn replace_assignment_invigilators_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    _round_id: Uuid,
    exam_day_id: Uuid,
    assignment_id: Uuid,
    invigilator_staff_ids: &[Uuid],
) -> Result<(), AppError> {
    validate_active_staff_users(tx, invigilator_staff_ids).await?;

    sqlx::query(
        r#"
        DELETE FROM academic_exam_day_invigilators
        WHERE day_room_assignment_id = $1
        "#,
    )
    .bind(assignment_id)
    .execute(&mut **tx)
    .await?;

    if invigilator_staff_ids.is_empty() {
        return Ok(());
    }

    sqlx::query(
        r#"
        INSERT INTO academic_exam_day_invigilators (
            exam_day_id,
            day_room_assignment_id,
            staff_id
        )
        SELECT $1, $2, staff_id
        FROM unnest($3::uuid[]) AS staff_id
        "#,
    )
    .bind(exam_day_id)
    .bind(assignment_id)
    .bind(invigilator_staff_ids)
    .execute(&mut **tx)
    .await
    .map_err(map_day_room_assignment_write_error)?;

    Ok(())
}

pub async fn generate_seats_for_assignment(
    pool: &PgPool,
    assignment_id: Uuid,
    request: GenerateSeatsRequest,
    actor_user_id: Uuid,
) -> Result<Vec<SeatAssignmentView>, AppError> {
    let mut tx = pool.begin().await?;
    let assignment_context = fetch_seat_assignment_context(&mut tx, assignment_id).await?;

    let existing_seats = fetch_seat_assignments_for_assignment(&mut tx, assignment_id).await?;
    if !request.regenerate && !existing_seats.is_empty() {
        tx.commit().await?;
        return Ok(existing_seats);
    }

    let students = fetch_ordered_seat_students(&mut tx, assignment_context.classroom_id).await?;
    let effective_capacity = assignment_context
        .capacity_override
        .unwrap_or(assignment_context.room_capacity);
    validate_seat_generation_capacity(students.len(), effective_capacity)?;

    let mut wrote_seats = false;
    if request.regenerate {
        sqlx::query(
            r#"
            DELETE FROM academic_exam_seat_assignments
            WHERE day_room_assignment_id = $1
            "#,
        )
        .bind(assignment_id)
        .execute(&mut *tx)
        .await?;
        wrote_seats = true;
    }

    let seat_drafts = build_default_seat_assignments(&students);

    if !seat_drafts.is_empty() {
        let student_ids: Vec<Uuid> = seat_drafts
            .iter()
            .map(|assignment| assignment.student_id)
            .collect();
        let seat_numbers: Vec<String> = seat_drafts
            .iter()
            .map(|assignment| assignment.seat_number.clone())
            .collect();

        sqlx::query(
            r#"
            INSERT INTO academic_exam_seat_assignments (
                day_room_assignment_id,
                student_id,
                seat_number
            )
            SELECT $1, student_id, seat_number
            FROM unnest($2::uuid[], $3::text[]) AS seat(student_id, seat_number)
            "#,
        )
        .bind(assignment_context.assignment_id)
        .bind(&student_ids)
        .bind(&seat_numbers)
        .execute(&mut *tx)
        .await?;
        wrote_seats = true;
    }

    if wrote_seats {
        mark_round_draft_after_mutation(
            &mut tx,
            assignment_context.exam_round_id,
            Some(actor_user_id),
        )
        .await?;
    }

    let seats = fetch_seat_assignments_for_assignment(&mut tx, assignment_id).await?;
    tx.commit().await?;

    Ok(seats)
}

pub async fn place_exam_session(
    pool: &PgPool,
    request: PlaceExamSessionRequest,
    actor_user_id: Uuid,
) -> Result<ExamSessionView, AppError> {
    let mut tx = pool.begin().await?;

    let item =
        fetch_schedule_item_placement_context(&mut tx, request.exam_schedule_item_id).await?;
    let day = fetch_exam_day_placement_context(&mut tx, request.exam_day_id).await?;
    if day.exam_round_id != item.exam_round_id {
        return Err(AppError::BadRequest(
            "Exam day belongs to a different exam round".to_string(),
        ));
    }

    validate_day_allows_grade_level(&mut tx, day.id, item.grade_level_id).await?;
    let blocked_windows = fetch_blocked_windows_for_day_for_placement(&mut tx, day.id).await?;
    let ends_at = validate_session_window(
        request.starts_at,
        item.duration_minutes,
        day.start_time,
        day.end_time,
        &blocked_windows,
    )
    .map_err(validation_error_to_app_error)?;

    let day_room_assignment =
        fetch_day_room_assignment_placement_context(&mut tx, day.id, item.classroom_id).await?;
    lock_exam_session_conflict_scope(
        &mut tx,
        day.id,
        item.classroom_id,
        day_room_assignment.room_id,
    )
    .await?;
    let existing_session_id =
        fetch_existing_session_id_for_item(&mut tx, request.exam_schedule_item_id).await?;

    let candidate = CandidateSession {
        session_id: existing_session_id,
        classroom_id: item.classroom_id,
        exam_day_id: day.id,
        starts_at: request.starts_at,
        ends_at,
    };
    let existing_classroom_sessions = fetch_candidate_sessions_for_day(&mut tx, day.id).await?;
    if has_same_classroom_conflict(&candidate, &existing_classroom_sessions) {
        return Err(AppError::BadRequest(
            "Classroom already has an exam session during this time".to_string(),
        ));
    }

    let room_candidate = CandidateRoomSession {
        session_id: existing_session_id,
        room_id: day_room_assignment.room_id,
        exam_day_id: day.id,
        starts_at: request.starts_at,
        ends_at,
    };
    let existing_room_sessions = fetch_candidate_room_sessions_for_day(&mut tx, day.id).await?;
    if has_same_room_conflict(&room_candidate, &existing_room_sessions) {
        return Err(AppError::BadRequest(
            "Room already has an exam session during this time".to_string(),
        ));
    }

    let invigilator_staff_ids =
        fetch_invigilator_staff_ids_for_assignment(&mut tx, day_room_assignment.id).await?;
    lock_exam_invigilator_staff_conflict_scope(&mut tx, day.id, &invigilator_staff_ids).await?;
    validate_invigilator_candidate_session_conflicts(
        &mut tx,
        item.exam_round_id,
        day_room_assignment.id,
        day.id,
        request.starts_at,
        ends_at,
        &invigilator_staff_ids,
    )
    .await?;

    let session_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO academic_exam_sessions (
            exam_schedule_item_id,
            exam_round_id,
            exam_day_id,
            starts_at,
            ends_at,
            created_by,
            updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $6)
        ON CONFLICT (exam_schedule_item_id)
        DO UPDATE SET
            exam_round_id = EXCLUDED.exam_round_id,
            exam_day_id = EXCLUDED.exam_day_id,
            starts_at = EXCLUDED.starts_at,
            ends_at = EXCLUDED.ends_at,
            updated_by = EXCLUDED.updated_by,
            updated_at = now()
        RETURNING id
        "#,
    )
    .bind(item.id)
    .bind(item.exam_round_id)
    .bind(day.id)
    .bind(request.starts_at)
    .bind(ends_at)
    .bind(actor_user_id)
    .fetch_one(&mut *tx)
    .await?;

    mark_round_draft_after_mutation(&mut tx, item.exam_round_id, Some(actor_user_id)).await?;
    tx.commit().await?;

    fetch_exam_session_view(pool, session_id).await
}

pub async fn delete_exam_session(
    pool: &PgPool,
    session_id: Uuid,
    actor_user_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    let round_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT exam_round_id
        FROM academic_exam_sessions
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam session not found".to_string()))?;

    sqlx::query("DELETE FROM academic_exam_sessions WHERE id = $1")
        .bind(session_id)
        .execute(&mut *tx)
        .await?;

    mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;
    tx.commit().await?;

    Ok(())
}

pub async fn publish_round(
    pool: &PgPool,
    round_id: Uuid,
    actor_user_id: Uuid,
) -> Result<ExamRound, AppError> {
    let mut tx = pool.begin().await?;

    let _locked_round_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT id
        FROM academic_exam_rounds
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(round_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam round not found".to_string()))?;

    let readiness = build_readiness(fetch_workspace_counts_in_tx(&mut tx, round_id).await?);
    if !readiness.can_publish {
        return Err(AppError::BadRequest(format!(
            "Exam round is not ready to publish: {}",
            readiness.blockers.join("; ")
        )));
    }

    let round = sqlx::query_as::<_, ExamRound>(
        r#"
        UPDATE academic_exam_rounds
        SET status = 'published',
            published_at = now(),
            published_by = $2,
            updated_by = $2,
            updated_at = now()
        WHERE id = $1
        RETURNING id,
                  academic_semester_id,
                  name,
                  description,
                  exam_kind,
                  status,
                  published_at,
                  created_at,
                  updated_at
        "#,
    )
    .bind(round_id)
    .bind(actor_user_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(round)
}

async fn fetch_workspace_counts_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    round_id: Uuid,
) -> Result<WorkspaceCounts, AppError> {
    let row: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(WORKSPACE_COUNTS_SQL)
        .bind(round_id)
        .fetch_one(&mut **tx)
        .await?;

    Ok(workspace_counts_from_row(row))
}

fn workspace_counts_from_row(
    (
        day_count,
        item_count,
        unscheduled_count,
        missing_room_assignment_count,
        invalid_session_count,
        missing_seat_student_count,
        invigilator_conflict_count,
    ): (i64, i64, i64, i64, i64, i64, i64),
) -> WorkspaceCounts {
    WorkspaceCounts {
        day_count,
        item_count,
        unscheduled_count,
        missing_room_assignment_count,
        invalid_session_count,
        missing_seat_student_count,
        invigilator_conflict_count,
    }
}

const WORKSPACE_COUNTS_SQL: &str = r#"
        SELECT (
                   SELECT COUNT(*)::BIGINT
                   FROM academic_exam_days day
                   WHERE day.exam_round_id = $1
               ) AS day_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM academic_exam_schedule_items item
                   WHERE item.exam_round_id = $1
               ) AS item_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM academic_exam_schedule_items item
                   WHERE item.exam_round_id = $1
                     AND NOT EXISTS (
                         SELECT 1
                         FROM academic_exam_sessions session
                         WHERE session.exam_schedule_item_id = item.id
                     )
               ) AS unscheduled_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM (
                       SELECT DISTINCT session.exam_day_id,
                                       item.classroom_id
                       FROM academic_exam_sessions session
                       JOIN academic_exam_schedule_items item
                         ON item.id = session.exam_schedule_item_id
                        AND item.exam_round_id = session.exam_round_id
                       LEFT JOIN academic_exam_day_room_assignments assignment
                         ON assignment.exam_day_id = session.exam_day_id
                        AND assignment.classroom_id = item.classroom_id
                       WHERE session.exam_round_id = $1
                         AND assignment.id IS NULL
                   ) missing_room_assignments
               ) AS missing_room_assignment_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM (
                       SELECT session.id
                       FROM academic_exam_sessions session
                       JOIN academic_exam_schedule_items item
                         ON item.id = session.exam_schedule_item_id
                        AND item.exam_round_id = session.exam_round_id
                       JOIN academic_exam_days day
                         ON day.id = session.exam_day_id
                        AND day.exam_round_id = session.exam_round_id
                       WHERE session.exam_round_id = $1
                         AND (
                             session.starts_at < day.start_time
                             OR session.ends_at > day.end_time
                             OR (EXTRACT(EPOCH FROM session.starts_at)::BIGINT % 900) <> 0
                             OR EXISTS (
                                 SELECT 1
                                 FROM academic_exam_day_blocked_windows blocked
                                 WHERE blocked.exam_day_id = session.exam_day_id
                                   AND session.starts_at < blocked.end_time
                                   AND blocked.start_time < session.ends_at
                             )
                             OR (
                                 EXISTS (
                                     SELECT 1
                                     FROM academic_exam_day_grade_levels scope
                                     WHERE scope.exam_day_id = session.exam_day_id
                                 )
                                 AND NOT EXISTS (
                                     SELECT 1
                                     FROM academic_exam_day_grade_levels scope
                                     WHERE scope.exam_day_id = session.exam_day_id
                                       AND scope.grade_level_id = item.grade_level_id
                                 )
                             )
                         )
                   ) invalid_sessions
               ) AS invalid_session_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM (
                       SELECT DISTINCT assignment.id AS day_room_assignment_id,
                                       enrollment.student_id
                       FROM academic_exam_sessions session
                       JOIN academic_exam_schedule_items item
                         ON item.id = session.exam_schedule_item_id
                        AND item.exam_round_id = session.exam_round_id
                       JOIN academic_exam_day_room_assignments assignment
                         ON assignment.exam_day_id = session.exam_day_id
                        AND assignment.classroom_id = item.classroom_id
                       JOIN student_class_enrollments enrollment
                         ON enrollment.class_room_id = item.classroom_id
                        AND enrollment.status = 'active'
                       JOIN users user_account
                         ON user_account.id = enrollment.student_id
                        AND user_account.user_type = 'student'
                        AND user_account.status = 'active'
                       LEFT JOIN academic_exam_seat_assignments seat
                         ON seat.day_room_assignment_id = assignment.id
                        AND seat.student_id = enrollment.student_id
                       WHERE session.exam_round_id = $1
                         AND seat.student_id IS NULL
                   ) missing_seat_students
               ) AS missing_seat_student_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM academic_exam_day_invigilators left_invigilator
                   JOIN academic_exam_day_invigilators right_invigilator
                     ON right_invigilator.staff_id = left_invigilator.staff_id
                    AND right_invigilator.exam_day_id = left_invigilator.exam_day_id
                    AND right_invigilator.day_room_assignment_id <> left_invigilator.day_room_assignment_id
                    AND right_invigilator.id > left_invigilator.id
                   JOIN academic_exam_day_room_assignments left_assignment
                     ON left_assignment.id = left_invigilator.day_room_assignment_id
                    AND left_assignment.exam_day_id = left_invigilator.exam_day_id
                   JOIN academic_exam_day_room_assignments right_assignment
                     ON right_assignment.id = right_invigilator.day_room_assignment_id
                    AND right_assignment.exam_day_id = right_invigilator.exam_day_id
                   JOIN academic_exam_days day
                     ON day.id = left_invigilator.exam_day_id
                   JOIN academic_exam_sessions left_session
                     ON left_session.exam_day_id = left_assignment.exam_day_id
                    AND left_session.exam_round_id = day.exam_round_id
                   JOIN academic_exam_schedule_items left_item
                     ON left_item.id = left_session.exam_schedule_item_id
                    AND left_item.classroom_id = left_assignment.classroom_id
                   JOIN academic_exam_sessions right_session
                     ON right_session.exam_day_id = right_assignment.exam_day_id
                    AND right_session.exam_round_id = day.exam_round_id
                   JOIN academic_exam_schedule_items right_item
                     ON right_item.id = right_session.exam_schedule_item_id
                    AND right_item.classroom_id = right_assignment.classroom_id
                   WHERE day.exam_round_id = $1
                     AND left_session.starts_at < right_session.ends_at
                     AND right_session.starts_at < left_session.ends_at
               ) AS invigilator_conflict_count
        "#;

pub async fn list_my_published_exam_schedule(
    pool: &PgPool,
    user_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError> {
    ensure_active_student_user(pool, user_id).await?;
    list_published_exam_schedule_for_student(pool, user_id, academic_semester_id).await
}

pub async fn list_child_published_exam_schedule(
    pool: &PgPool,
    parent_user_id: Uuid,
    student_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError> {
    ensure_parent_user_for_exam_schedule(pool, parent_user_id).await?;
    ensure_parent_student_link_for_exam_schedule(pool, parent_user_id, student_id).await?;
    list_published_exam_schedule_for_student(pool, student_id, academic_semester_id).await
}

async fn mark_round_draft_after_mutation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    round_id: Uuid,
    actor_user_id: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE academic_exam_rounds
        SET status = 'draft',
            published_at = NULL,
            published_by = NULL,
            updated_by = COALESCE($2, updated_by),
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(round_id)
    .bind(actor_user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn fetch_schedule_item_placement_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_schedule_item_id: Uuid,
) -> Result<ExamScheduleItemPlacementContext, AppError> {
    sqlx::query_as::<_, ExamScheduleItemPlacementContext>(
        r#"
        SELECT item.id,
               item.exam_round_id,
               item.academic_semester_id,
               item.assessment_category_id,
               item.assessment_plan_id,
               item.classroom_course_id,
               item.classroom_id,
               item.subject_id,
               item.grade_level_id,
               item.duration_minutes
        FROM academic_exam_schedule_items item
        JOIN academic_exam_rounds round ON round.id = item.exam_round_id
        WHERE item.id = $1
        FOR UPDATE OF item
        "#,
    )
    .bind(exam_schedule_item_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam schedule item not found".to_string()))
}

async fn fetch_exam_day_placement_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
) -> Result<ExamDayPlacementContext, AppError> {
    sqlx::query_as::<_, ExamDayPlacementContext>(
        r#"
        SELECT id,
               exam_round_id,
               start_time,
               end_time
        FROM academic_exam_days
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(exam_day_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam day not found".to_string()))
}

async fn fetch_blocked_windows_for_day_for_placement(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
) -> Result<Vec<BlockedWindow>, AppError> {
    sqlx::query_as::<_, BlockedWindow>(
        r#"
        SELECT id,
               label,
               start_time,
               end_time
        FROM academic_exam_day_blocked_windows
        WHERE exam_day_id = $1
        ORDER BY start_time, end_time, label, id
        "#,
    )
    .bind(exam_day_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn fetch_day_room_assignment_placement_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
    classroom_id: Uuid,
) -> Result<DayRoomAssignmentPlacementContext, AppError> {
    sqlx::query_as::<_, DayRoomAssignmentPlacementContext>(
        r#"
        SELECT id,
               room_id
        FROM academic_exam_day_room_assignments
        WHERE exam_day_id = $1
          AND classroom_id = $2
        FOR UPDATE
        "#,
    )
    .bind(exam_day_id)
    .bind(classroom_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(
            "Assign an exam room for this classroom before placing the session".to_string(),
        )
    })
}

async fn fetch_existing_session_id_for_item(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_schedule_item_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    sqlx::query_scalar(
        r#"
        SELECT id
        FROM academic_exam_sessions
        WHERE exam_schedule_item_id = $1
        "#,
    )
    .bind(exam_schedule_item_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn fetch_candidate_sessions_for_day(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
) -> Result<Vec<CandidateSession>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, NaiveTime, NaiveTime)>(
        r#"
        SELECT session.id,
               item.classroom_id,
               session.exam_day_id,
               session.starts_at,
               session.ends_at
        FROM academic_exam_sessions session
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.exam_round_id = session.exam_round_id
        WHERE session.exam_day_id = $1
        "#,
    )
    .bind(exam_day_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(session_id, classroom_id, exam_day_id, starts_at, ends_at)| CandidateSession {
                session_id: Some(session_id),
                classroom_id,
                exam_day_id,
                starts_at,
                ends_at,
            },
        )
        .collect())
}

async fn fetch_candidate_room_sessions_for_day(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
) -> Result<Vec<CandidateRoomSession>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, NaiveTime, NaiveTime)>(
        r#"
        SELECT session.id,
               assignment.room_id,
               session.exam_day_id,
               session.starts_at,
               session.ends_at
        FROM academic_exam_sessions session
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.exam_round_id = session.exam_round_id
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        WHERE session.exam_day_id = $1
        "#,
    )
    .bind(exam_day_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(session_id, room_id, exam_day_id, starts_at, ends_at)| CandidateRoomSession {
                session_id: Some(session_id),
                room_id,
                exam_day_id,
                starts_at,
                ends_at,
            },
        )
        .collect())
}

async fn fetch_exam_session_view(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<ExamSessionView, AppError> {
    let rows = sqlx::query_as::<_, ExamSessionRow>(
        r#"
        SELECT session.id,
               session.exam_schedule_item_id,
               session.exam_round_id,
               session.exam_day_id,
               session.starts_at,
               session.ends_at,
               item.academic_semester_id,
               item.assessment_category_id,
               item.assessment_plan_id,
               item.classroom_course_id,
               item.classroom_id,
               item.subject_id,
               item.grade_level_id,
               item.duration_minutes,
               item.imported_at,
               day.exam_date AS exam_date,
               category.name AS assessment_category_name,
               subject.code AS subject_code,
               subject.name_th AS subject_name_th,
               subject.name_en AS subject_name_en,
               subject.group_id AS subject_group_id,
               subject_group.name_th AS subject_group_name,
               subject_group.display_order AS subject_group_display_order,
               subject.type AS subject_type,
               classroom.name AS classroom_name,
               CASE grade_level.level_type
                   WHEN 'kindergarten' THEN CONCAT('อ.', grade_level.year)
                   WHEN 'primary' THEN CONCAT('ป.', grade_level.year)
                   WHEN 'secondary' THEN CONCAT('ม.', grade_level.year)
                   ELSE CONCAT('?.', grade_level.year)
               END AS grade_level_name,
               grade_level.level_type AS grade_level_type,
               grade_level.year AS grade_level_year,
               assignment.id AS day_room_assignment_id,
               assignment.room_id AS room_id,
               room.name_th AS room_name,
               building.name_th AS building_name
        FROM academic_exam_sessions session
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.exam_round_id = session.exam_round_id
        JOIN academic_exam_days day
          ON day.id = session.exam_day_id
         AND day.exam_round_id = session.exam_round_id
        JOIN academic_assessment_categories category
          ON category.id = item.assessment_category_id
        JOIN subjects subject ON subject.id = item.subject_id
        LEFT JOIN subject_groups subject_group ON subject_group.id = subject.group_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        JOIN grade_levels grade_level ON grade_level.id = item.grade_level_id
        LEFT JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        LEFT JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE session.id = $1
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    let assignment_ids: Vec<Uuid> = rows
        .iter()
        .filter_map(|row| row.day_room_assignment_id)
        .collect();
    let invigilators_by_assignment =
        fetch_invigilators_by_assignment_ids(pool, &assignment_ids).await?;

    rows.into_iter()
        .map(|row| {
            let invigilators = invigilators_for_assignment(
                row.day_room_assignment_id,
                &invigilators_by_assignment,
            );
            row.into_view(invigilators)
        })
        .next()
        .ok_or_else(|| AppError::NotFound("Exam session not found".to_string()))
}

async fn ensure_active_student_user(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let user_row: Option<(String, String)> =
        sqlx::query_as("SELECT user_type, status FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

    match user_row
        .as_ref()
        .map(|(user_type, status)| (user_type.as_str(), status.as_str()))
    {
        Some(("student", "active")) => Ok(()),
        Some(_) => Err(AppError::Forbidden(
            "Only active students can view personal exam schedules".to_string(),
        )),
        None => Err(AppError::AuthError("Please sign in".to_string())),
    }
}

async fn ensure_parent_user_for_exam_schedule(
    pool: &PgPool,
    parent_user_id: Uuid,
) -> Result<(), AppError> {
    let user_type: Option<String> = sqlx::query_scalar("SELECT user_type FROM users WHERE id = $1")
        .bind(parent_user_id)
        .fetch_optional(pool)
        .await?;

    match user_type.as_deref() {
        Some("parent") => Ok(()),
        Some(_) => Err(AppError::Forbidden("เฉพาะผู้ปกครองเท่านั้น".to_string())),
        None => Err(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string())),
    }
}

async fn ensure_parent_student_link_for_exam_schedule(
    pool: &PgPool,
    parent_user_id: Uuid,
    student_id: Uuid,
) -> Result<(), AppError> {
    let is_linked: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM student_parents
            JOIN users user_account ON user_account.id = student_parents.student_user_id
            WHERE student_parents.parent_user_id = $1
              AND student_parents.student_user_id = $2
              AND user_account.user_type = 'student'
              AND user_account.status = 'active'
        )
        "#,
    )
    .bind(parent_user_id)
    .bind(student_id)
    .fetch_one(pool)
    .await?;

    if !is_linked {
        return Err(AppError::Forbidden(
            "คุณไม่มีสิทธิ์เข้าถึงข้อมูลนักเรียนคนนี้".to_string(),
        ));
    }

    Ok(())
}

async fn list_published_exam_schedule_for_student(
    pool: &PgPool,
    student_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError> {
    let rows = sqlx::query_as::<_, PersonalExamSessionRow>(
        r#"
        SELECT round.id AS round_id,
               round.name AS round_name,
               round.academic_semester_id,
               round.published_at,
               day.exam_date,
               session.starts_at,
               session.ends_at,
               COALESCE(NULLIF(subject.name_th, ''), NULLIF(subject.name_en, ''), subject.code)
                   AS subject_name,
               category.name AS assessment_category_name,
               classroom.name AS classroom_name,
               room.name_th AS room_name,
               building.name_th AS building_name,
               seat.seat_number
        FROM student_class_enrollments enrollment
        JOIN users student_user
          ON student_user.id = enrollment.student_id
         AND student_user.user_type = 'student'
         AND student_user.status = 'active'
        JOIN academic_exam_schedule_items item
          ON item.classroom_id = enrollment.class_room_id
        JOIN academic_exam_rounds round
          ON round.id = item.exam_round_id
         AND round.academic_semester_id = item.academic_semester_id
        JOIN academic_exam_sessions session
          ON session.exam_schedule_item_id = item.id
         AND session.exam_round_id = item.exam_round_id
        JOIN academic_exam_days day
          ON day.id = session.exam_day_id
         AND day.exam_round_id = session.exam_round_id
        JOIN academic_assessment_categories category
          ON category.id = item.assessment_category_id
        JOIN subjects subject ON subject.id = item.subject_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        LEFT JOIN academic_exam_seat_assignments seat
          ON seat.day_room_assignment_id = assignment.id
         AND seat.student_id = enrollment.student_id
        WHERE enrollment.student_id = $1
          AND enrollment.status = 'active'
          AND round.status = 'published'
          AND ($2::uuid IS NULL OR round.academic_semester_id = $2)
        ORDER BY round.published_at DESC NULLS LAST,
                 round.name,
                 day.exam_date,
                 session.starts_at,
                 classroom.name,
                 subject.code,
                 category.display_order,
                 category.name,
                 session.id
        "#,
    )
    .bind(student_id)
    .bind(academic_semester_id)
    .fetch_all(pool)
    .await?;

    Ok(group_personal_exam_schedule_rows(rows))
}

fn group_personal_exam_schedule_rows(
    rows: Vec<PersonalExamSessionRow>,
) -> Vec<PersonalExamScheduleRound> {
    let mut rounds = Vec::new();
    let mut round_indexes = HashMap::new();

    for row in rows {
        let round_id = row.round_id;
        let round_index = match round_indexes.get(&round_id) {
            Some(index) => *index,
            None => {
                let index = rounds.len();
                rounds.push(PersonalExamScheduleRound {
                    round_id,
                    round_name: row.round_name.clone(),
                    academic_semester_id: row.academic_semester_id,
                    published_at: row.published_at,
                    sessions: Vec::new(),
                });
                round_indexes.insert(round_id, index);
                index
            }
        };

        rounds[round_index].sessions.push(row.into_session_view());
    }

    rounds
}

fn validate_unique_invigilator_staff_ids(ids: Vec<Uuid>) -> Result<Vec<Uuid>, AppError> {
    let mut seen = HashSet::new();
    for id in &ids {
        if !seen.insert(*id) {
            return Err(AppError::BadRequest(
                "Duplicate invigilator staff ids are not allowed".to_string(),
            ));
        }
    }
    Ok(ids)
}

fn validate_capacity_override(capacity_override: Option<i32>) -> Result<Option<i32>, AppError> {
    if matches!(capacity_override, Some(value) if value <= 0) {
        return Err(AppError::BadRequest(
            "Capacity override must be greater than zero".to_string(),
        ));
    }
    Ok(capacity_override)
}

async fn fetch_exam_day_context_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
) -> Result<ExamDayContext, AppError> {
    sqlx::query_as::<_, ExamDayContext>(
        r#"
        SELECT exam_round_id
        FROM academic_exam_days
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(exam_day_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam day not found".to_string()))
}

async fn fetch_classroom_assignment_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    classroom_id: Uuid,
) -> Result<ClassroomAssignmentContext, AppError> {
    sqlx::query_as::<_, ClassroomAssignmentContext>(
        r#"
        SELECT id AS classroom_id,
               name AS classroom_name,
               grade_level_id,
               is_active
        FROM class_rooms
        WHERE id = $1
        "#,
    )
    .bind(classroom_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Classroom not found".to_string()))
}

async fn fetch_room_assignment_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    room_id: Uuid,
) -> Result<RoomAssignmentContext, AppError> {
    sqlx::query_as::<_, RoomAssignmentContext>(
        r#"
        SELECT id AS room_id,
               capacity,
               status
        FROM rooms
        WHERE id = $1
        "#,
    )
    .bind(room_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Room not found".to_string()))
}

async fn validate_day_allows_grade_level(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
    grade_level_id: Uuid,
) -> Result<(), AppError> {
    let scoped_grade_level_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT grade_level_id
        FROM academic_exam_day_grade_levels
        WHERE exam_day_id = $1
        ORDER BY grade_level_id
        "#,
    )
    .bind(exam_day_id)
    .fetch_all(&mut **tx)
    .await?;

    if !grade_level_allowed_by_day_scope(grade_level_id, &scoped_grade_level_ids) {
        return Err(AppError::BadRequest(
            "Classroom grade level is not allowed on this exam day".to_string(),
        ));
    }
    Ok(())
}

fn grade_level_allowed_by_day_scope(grade_level_id: Uuid, scoped_grade_level_ids: &[Uuid]) -> bool {
    scoped_grade_level_ids.is_empty() || scoped_grade_level_ids.contains(&grade_level_id)
}

async fn count_active_classroom_students(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    classroom_id: Uuid,
) -> Result<i64, AppError> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM student_class_enrollments enrollment
        WHERE enrollment.class_room_id = $1
          AND enrollment.status = 'active'
        "#,
    )
    .bind(classroom_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn validate_active_staff_users(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    staff_ids: &[Uuid],
) -> Result<(), AppError> {
    if staff_ids.is_empty() {
        return Ok(());
    }

    let invalid_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM unnest($1::uuid[]) AS requested(staff_id)
        LEFT JOIN users user_account
          ON user_account.id = requested.staff_id
         AND user_account.user_type = 'staff'
         AND user_account.status = 'active'
        WHERE user_account.id IS NULL
        "#,
    )
    .bind(staff_ids)
    .fetch_one(&mut **tx)
    .await?;

    if invalid_count > 0 {
        return Err(AppError::BadRequest(
            "Every invigilator must be an active staff user".to_string(),
        ));
    }
    Ok(())
}

async fn fetch_assignment_session_windows(
    tx: &mut Transaction<'_, Postgres>,
    assignment_id: Uuid,
    staff_ids: &[Uuid],
) -> Result<Vec<InvigilatorSessionWindow>, AppError> {
    if staff_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, InvigilatorSessionWindowRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               assignment.exam_day_id,
               staff.staff_id,
               '' AS staff_name,
               session.starts_at,
               session.ends_at
        FROM academic_exam_day_room_assignments assignment
        JOIN unnest($2::uuid[]) AS staff(staff_id) ON TRUE
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN academic_exam_sessions session
          ON session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.classroom_id = assignment.classroom_id
        WHERE assignment.id = $1
        ORDER BY session.starts_at, staff.staff_id
        "#,
    )
    .bind(assignment_id)
    .bind(staff_ids)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| InvigilatorSessionWindow {
            assignment_id: row.assignment_id,
            exam_day_id: row.exam_day_id,
            staff_id: row.staff_id,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
        })
        .collect())
}

async fn fetch_existing_invigilator_session_windows(
    tx: &mut Transaction<'_, Postgres>,
    round_id: Uuid,
    staff_ids: &[Uuid],
) -> Result<Vec<InvigilatorSessionWindow>, AppError> {
    if staff_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, InvigilatorSessionWindowRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               assignment.exam_day_id,
               invigilator.staff_id,
               '' AS staff_name,
               session.starts_at,
               session.ends_at
        FROM academic_exam_day_invigilators invigilator
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.id = invigilator.day_room_assignment_id
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN academic_exam_sessions session
          ON session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.classroom_id = assignment.classroom_id
        WHERE day.exam_round_id = $1
          AND invigilator.staff_id = ANY($2)
        ORDER BY assignment.exam_day_id, session.starts_at, invigilator.staff_id
        "#,
    )
    .bind(round_id)
    .bind(staff_ids)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| InvigilatorSessionWindow {
            assignment_id: row.assignment_id,
            exam_day_id: row.exam_day_id,
            staff_id: row.staff_id,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
        })
        .collect())
}

async fn validate_invigilator_time_conflicts(
    tx: &mut Transaction<'_, Postgres>,
    round_id: Uuid,
    assignment_id: Uuid,
    staff_ids: &[Uuid],
) -> Result<(), AppError> {
    if staff_ids.is_empty() {
        return Ok(());
    }

    let candidate_windows = fetch_assignment_session_windows(tx, assignment_id, staff_ids).await?;
    if candidate_windows.is_empty() {
        return Ok(());
    }

    let existing_windows =
        fetch_existing_invigilator_session_windows(tx, round_id, staff_ids).await?;
    if has_invigilator_time_conflict(assignment_id, &candidate_windows, &existing_windows) {
        return Err(AppError::BadRequest(
            "Invigilator has an overlapping exam supervision assignment".to_string(),
        ));
    }

    Ok(())
}

fn build_invigilator_candidate_session_windows(
    assignment_id: Uuid,
    exam_day_id: Uuid,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
    staff_ids: &[Uuid],
) -> Vec<InvigilatorSessionWindow> {
    staff_ids
        .iter()
        .map(|staff_id| InvigilatorSessionWindow {
            assignment_id,
            exam_day_id,
            staff_id: *staff_id,
            starts_at,
            ends_at,
        })
        .collect()
}

async fn fetch_invigilator_staff_ids_for_assignment(
    tx: &mut Transaction<'_, Postgres>,
    assignment_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    sqlx::query_scalar(
        r#"
        SELECT staff_id
        FROM academic_exam_day_invigilators
        WHERE day_room_assignment_id = $1
        ORDER BY staff_id
        "#,
    )
    .bind(assignment_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn validate_invigilator_candidate_session_conflicts(
    tx: &mut Transaction<'_, Postgres>,
    round_id: Uuid,
    assignment_id: Uuid,
    exam_day_id: Uuid,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
    staff_ids: &[Uuid],
) -> Result<(), AppError> {
    if staff_ids.is_empty() {
        return Ok(());
    }

    let candidate_windows = build_invigilator_candidate_session_windows(
        assignment_id,
        exam_day_id,
        starts_at,
        ends_at,
        staff_ids,
    );
    let existing_windows =
        fetch_existing_invigilator_session_windows(tx, round_id, staff_ids).await?;
    if has_invigilator_time_conflict(assignment_id, &candidate_windows, &existing_windows) {
        return Err(AppError::BadRequest(
            "Invigilator has an overlapping exam supervision assignment".to_string(),
        ));
    }

    Ok(())
}

async fn fetch_day_room_assignment_views_for_day(
    pool: &PgPool,
    exam_day_id: Uuid,
) -> Result<Vec<DayRoomAssignmentView>, AppError> {
    let rows = sqlx::query_as::<_, DayRoomAssignmentViewRow>(
        r#"
        SELECT assignment.id,
               assignment.exam_day_id,
               assignment.classroom_id,
               classroom.name AS classroom_name,
               assignment.room_id,
               room.name_th AS room_name,
               building.name_th AS building_name,
               room.capacity AS room_capacity,
               assignment.capacity_override,
               EXISTS (
                   SELECT 1
                   FROM academic_exam_seat_assignments seat
                   WHERE seat.day_room_assignment_id = assignment.id
               ) AS seats_generated
        FROM academic_exam_day_room_assignments assignment
        JOIN class_rooms classroom ON classroom.id = assignment.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE assignment.exam_day_id = $1
        ORDER BY classroom.name, room.name_th, assignment.id
        "#,
    )
    .bind(exam_day_id)
    .fetch_all(pool)
    .await?;

    hydrate_day_room_assignment_views(pool, rows).await
}

async fn fetch_day_room_assignment_view(
    pool: &PgPool,
    assignment_id: Uuid,
) -> Result<DayRoomAssignmentView, AppError> {
    let rows = sqlx::query_as::<_, DayRoomAssignmentViewRow>(
        r#"
        SELECT assignment.id,
               assignment.exam_day_id,
               assignment.classroom_id,
               classroom.name AS classroom_name,
               assignment.room_id,
               room.name_th AS room_name,
               building.name_th AS building_name,
               room.capacity AS room_capacity,
               assignment.capacity_override,
               EXISTS (
                   SELECT 1
                   FROM academic_exam_seat_assignments seat
                   WHERE seat.day_room_assignment_id = assignment.id
               ) AS seats_generated
        FROM academic_exam_day_room_assignments assignment
        JOIN class_rooms classroom ON classroom.id = assignment.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE assignment.id = $1
        "#,
    )
    .bind(assignment_id)
    .fetch_all(pool)
    .await?;

    let mut views = hydrate_day_room_assignment_views(pool, rows).await?;
    views
        .pop()
        .ok_or_else(|| AppError::NotFound("Exam room assignment not found".to_string()))
}

async fn hydrate_day_room_assignment_views(
    pool: &PgPool,
    rows: Vec<DayRoomAssignmentViewRow>,
) -> Result<Vec<DayRoomAssignmentView>, AppError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let assignment_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let mut invigilators_by_assignment =
        fetch_invigilator_views_by_assignment_ids(pool, &assignment_ids).await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let invigilators = invigilators_by_assignment
                .remove(&row.id)
                .unwrap_or_default();
            row.into_view(invigilators)
        })
        .collect())
}

async fn fetch_invigilator_assignment_summaries(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<InvigilatorAssignmentSummaryRow>, AppError> {
    sqlx::query_as::<_, InvigilatorAssignmentSummaryRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               day.id AS exam_day_id,
               assignment.classroom_id,
               classroom.name AS classroom_name,
               assignment.room_id,
               room.name_th AS room_name,
               COALESCE(SUM(EXTRACT(EPOCH FROM (session.ends_at - session.starts_at)) / 60), 0)::INT
                   AS session_minutes
        FROM academic_exam_day_room_assignments assignment
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN class_rooms classroom ON classroom.id = assignment.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN academic_exam_schedule_items item
          ON item.exam_round_id = day.exam_round_id
         AND item.classroom_id = assignment.classroom_id
        LEFT JOIN academic_exam_sessions session
          ON session.exam_schedule_item_id = item.id
         AND session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        WHERE day.exam_round_id = $1
        GROUP BY assignment.id, day.id, assignment.classroom_id, classroom.name, assignment.room_id, room.name_th
        ORDER BY day.exam_date, day.start_time, day.id, classroom.name, room.name_th, assignment.id
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn fetch_invigilator_staff_workloads(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ExamInvigilatorStaffWorkload>, AppError> {
    let rows = sqlx::query_as::<_, InvigilatorSessionWindowRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               assignment.exam_day_id,
               invigilator.staff_id,
               concat_ws(' ', user_account.title, user_account.first_name, user_account.last_name)
                   AS staff_name,
               session.starts_at,
               session.ends_at
        FROM academic_exam_day_invigilators invigilator
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.id = invigilator.day_room_assignment_id
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN users user_account ON user_account.id = invigilator.staff_id
        JOIN academic_exam_sessions session
          ON session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.classroom_id = assignment.classroom_id
        WHERE day.exam_round_id = $1
        ORDER BY staff_name, day.exam_date, day.start_time, day.id, session.starts_at, assignment.id
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await?;

    Ok(build_invigilator_staff_workloads(rows))
}

#[derive(Debug, Default)]
struct StaffWorkloadAccumulator {
    staff_name: String,
    day_minutes: BTreeMap<Uuid, i32>,
    day_assignments: BTreeMap<Uuid, BTreeSet<Uuid>>,
    assignments: BTreeSet<Uuid>,
}

fn build_invigilator_staff_workloads(
    rows: Vec<InvigilatorSessionWindowRow>,
) -> Vec<ExamInvigilatorStaffWorkload> {
    let mut by_staff: BTreeMap<Uuid, StaffWorkloadAccumulator> = BTreeMap::new();

    for row in rows {
        let minutes = minutes_between_times(row.starts_at, row.ends_at);
        let accumulator =
            by_staff
                .entry(row.staff_id)
                .or_insert_with(|| StaffWorkloadAccumulator {
                    staff_name: row.staff_name.clone(),
                    ..Default::default()
                });

        *accumulator.day_minutes.entry(row.exam_day_id).or_insert(0) += minutes;
        accumulator
            .day_assignments
            .entry(row.exam_day_id)
            .or_default()
            .insert(row.assignment_id);
        accumulator.assignments.insert(row.assignment_id);
    }

    by_staff
        .into_iter()
        .map(|(staff_id, accumulator)| {
            let days = accumulator
                .day_minutes
                .iter()
                .map(|(exam_day_id, minutes)| ExamInvigilatorDayWorkload {
                    exam_day_id: *exam_day_id,
                    minutes: *minutes,
                    assignment_count: accumulator
                        .day_assignments
                        .get(exam_day_id)
                        .map(|assignment_ids| assignment_ids.len() as i32)
                        .unwrap_or(0),
                })
                .collect::<Vec<_>>();

            ExamInvigilatorStaffWorkload {
                staff_id,
                staff_name: accumulator.staff_name,
                total_minutes: days.iter().map(|day| day.minutes).sum(),
                assigned_day_count: days.len() as i32,
                assignment_count: accumulator.assignments.len() as i32,
                days,
            }
        })
        .collect()
}

async fn fetch_invigilator_views_by_assignment_ids(
    pool: &PgPool,
    assignment_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<InvigilatorView>>, AppError> {
    let assignment_ids = unique_uuids(assignment_ids.to_vec());
    if assignment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, InvigilatorViewRow>(
        r#"
        SELECT invigilator.day_room_assignment_id,
               invigilator.staff_id,
               concat_ws(' ', user_account.title, user_account.first_name, user_account.last_name)
                   AS display_name
        FROM academic_exam_day_invigilators invigilator
        JOIN users user_account ON user_account.id = invigilator.staff_id
        WHERE invigilator.day_room_assignment_id = ANY($1)
        ORDER BY invigilator.day_room_assignment_id,
                 user_account.first_name,
                 user_account.last_name,
                 invigilator.staff_id
        "#,
    )
    .bind(&assignment_ids)
    .fetch_all(pool)
    .await?;

    let mut grouped = HashMap::new();
    for row in rows {
        grouped
            .entry(row.day_room_assignment_id)
            .or_insert_with(Vec::new)
            .push(row.into_view());
    }
    Ok(grouped)
}

async fn fetch_seat_assignment_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    assignment_id: Uuid,
) -> Result<SeatAssignmentContext, AppError> {
    sqlx::query_as::<_, SeatAssignmentContext>(
        r#"
        SELECT assignment.id AS assignment_id,
               exam_day.exam_round_id,
               assignment.classroom_id,
               assignment.capacity_override,
               room.capacity AS room_capacity
        FROM academic_exam_day_room_assignments assignment
        JOIN academic_exam_days exam_day ON exam_day.id = assignment.exam_day_id
        JOIN rooms room ON room.id = assignment.room_id
        WHERE assignment.id = $1
        FOR UPDATE OF assignment
        "#,
    )
    .bind(assignment_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam room assignment not found".to_string()))
}

async fn fetch_seat_assignments_for_assignment(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    assignment_id: Uuid,
) -> Result<Vec<SeatAssignmentView>, AppError> {
    sqlx::query_as::<_, SeatAssignmentView>(
        r#"
        SELECT seat.id,
               seat.day_room_assignment_id,
               seat.student_id,
               concat_ws(' ', user_account.title, user_account.first_name, user_account.last_name)
                   AS student_name,
               seat.seat_number
        FROM academic_exam_seat_assignments seat
        JOIN users user_account ON user_account.id = seat.student_id
        WHERE seat.day_room_assignment_id = $1
        ORDER BY length(seat.seat_number), seat.seat_number, seat.id
        "#,
    )
    .bind(assignment_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn fetch_ordered_seat_students(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    classroom_id: Uuid,
) -> Result<Vec<SeatStudent>, AppError> {
    sqlx::query_as::<_, SeatStudent>(
        r#"
        SELECT user_account.id AS student_id
        FROM student_class_enrollments enrollment
        JOIN users user_account
          ON user_account.id = enrollment.student_id
         AND user_account.user_type = 'student'
         AND user_account.status = 'active'
        LEFT JOIN student_info ON student_info.user_id = user_account.id
        WHERE enrollment.class_room_id = $1
          AND enrollment.status = 'active'
        ORDER BY enrollment.class_number ASC NULLS LAST,
                 student_info.student_id ASC NULLS LAST,
                 user_account.id ASC
        "#,
    )
    .bind(classroom_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)
}

fn map_day_room_assignment_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_error) = &error {
        let code = db_error.code().unwrap_or_default();
        if code == "23505" {
            let constraint = db_error.constraint().unwrap_or_default();
            if constraint.contains("exam_day_id_room_id") {
                return AppError::BadRequest(
                    "Room is already assigned to another classroom on this exam day".to_string(),
                );
            }
            if constraint.contains("day_room_assignment_id_staff_id") {
                return AppError::BadRequest(
                    "Duplicate invigilator for this room assignment".to_string(),
                );
            }
            return AppError::BadRequest(
                "Exam room assignment conflicts with existing schedule data".to_string(),
            );
        }
    }
    AppError::from(error)
}

fn invigilator_staff_option_limit(limit: Option<i64>) -> i64 {
    limit
        .unwrap_or(INVIGILATOR_STAFF_OPTION_DEFAULT_LIMIT)
        .clamp(1, INVIGILATOR_STAFF_OPTION_MAX_LIMIT)
}

fn invigilator_staff_option_search_pattern(search: Option<String>) -> Option<String> {
    let trimmed = search?.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(format!("%{trimmed}%"))
    }
}

fn validate_exam_day_window(start_time: NaiveTime, end_time: NaiveTime) -> Result<(), AppError> {
    if start_time >= end_time {
        return Err(AppError::BadRequest(
            "Exam day start time must be before end time".to_string(),
        ));
    }
    Ok(())
}

fn normalize_exam_kind(value: Option<&str>) -> Result<String, AppError> {
    let normalized = value.unwrap_or("midterm").trim();
    match normalized {
        "midterm" | "final" => Ok(normalized.to_string()),
        _ => Err(AppError::BadRequest(
            "Exam round kind must be midterm or final".to_string(),
        )),
    }
}

fn normalize_update_round_request(
    request: UpdateExamRoundRequest,
) -> Result<NormalizedUpdateRoundRequest, AppError> {
    if request.name.is_none() && request.description.is_none() && request.exam_kind.is_none() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    let name = match request.name {
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                return Err(AppError::BadRequest(
                    "Exam round name is required".to_string(),
                ));
            }
            Some(trimmed)
        }
        None => None,
    };

    Ok(NormalizedUpdateRoundRequest {
        name,
        description: request.description,
        exam_kind: match request.exam_kind {
            Some(value) => Some(normalize_exam_kind(Some(&value))?),
            None => None,
        },
    })
}

fn normalize_blocked_windows(
    day_start_time: NaiveTime,
    day_end_time: NaiveTime,
    blocked_windows: Vec<BlockedWindowInput>,
) -> Result<Vec<BlockedWindowInput>, AppError> {
    let mut normalized = Vec::with_capacity(blocked_windows.len());
    for window in blocked_windows {
        if window.start_time >= window.end_time {
            return Err(AppError::BadRequest(
                "Blocked window start time must be before end time".to_string(),
            ));
        }
        if window.start_time < day_start_time || window.end_time > day_end_time {
            return Err(AppError::BadRequest(
                "Blocked windows must be within the exam day".to_string(),
            ));
        }
        let label = window.label.trim().to_string();
        if label.is_empty() {
            return Err(AppError::BadRequest(
                "Blocked window label is required".to_string(),
            ));
        }
        normalized.push(BlockedWindowInput {
            label,
            start_time: window.start_time,
            end_time: window.end_time,
        });
    }
    Ok(normalized)
}

fn unique_uuids(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

async fn fetch_round(pool: &PgPool, round_id: Uuid) -> Result<ExamRound, AppError> {
    sqlx::query_as::<_, ExamRound>(
        r#"
        SELECT id,
               academic_semester_id,
               name,
               description,
               exam_kind,
               status,
               published_at,
               created_at,
               updated_at
        FROM academic_exam_rounds
        WHERE id = $1
        "#,
    )
    .bind(round_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam round not found".to_string()))
}

async fn fetch_exam_day_detail(
    pool: &PgPool,
    exam_day_id: Uuid,
) -> Result<ExamDayDetail, AppError> {
    let day = sqlx::query_as::<_, ExamDay>(
        r#"
        SELECT id,
               exam_round_id,
               exam_date,
               label,
               start_time,
               end_time
        FROM academic_exam_days
        WHERE id = $1
        "#,
    )
    .bind(exam_day_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam day not found".to_string()))?;

    let mut details = hydrate_exam_day_details(pool, vec![day]).await?;
    details
        .pop()
        .ok_or_else(|| AppError::NotFound("Exam day not found".to_string()))
}

async fn fetch_exam_day_details_for_round(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ExamDayDetail>, AppError> {
    let days = sqlx::query_as::<_, ExamDay>(
        r#"
        SELECT id,
               exam_round_id,
               exam_date,
               label,
               start_time,
               end_time
        FROM academic_exam_days
        WHERE exam_round_id = $1
        ORDER BY exam_date ASC, start_time ASC, id ASC
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await?;

    hydrate_exam_day_details(pool, days).await
}

async fn hydrate_exam_day_details(
    pool: &PgPool,
    days: Vec<ExamDay>,
) -> Result<Vec<ExamDayDetail>, AppError> {
    if days.is_empty() {
        return Ok(Vec::new());
    }

    let day_ids: Vec<Uuid> = days.iter().map(|day| day.id).collect();

    let grade_rows = sqlx::query_as::<_, ExamDayGradeLevelRow>(
        r#"
        SELECT scope.exam_day_id,
               scope.grade_level_id
        FROM academic_exam_day_grade_levels scope
        JOIN grade_levels grade_level ON grade_level.id = scope.grade_level_id
        WHERE scope.exam_day_id = ANY($1)
        ORDER BY scope.exam_day_id,
                 CASE grade_level.level_type
                     WHEN 'kindergarten' THEN 1
                     WHEN 'primary' THEN 2
                     WHEN 'secondary' THEN 3
                     ELSE 4
                 END,
                 grade_level.year,
                 scope.grade_level_id
        "#,
    )
    .bind(&day_ids)
    .fetch_all(pool)
    .await?;

    let blocked_rows = sqlx::query_as::<_, BlockedWindowRow>(
        r#"
        SELECT exam_day_id,
               id,
               label,
               start_time,
               end_time
        FROM academic_exam_day_blocked_windows
        WHERE exam_day_id = ANY($1)
        ORDER BY exam_day_id, start_time, end_time, label, id
        "#,
    )
    .bind(&day_ids)
    .fetch_all(pool)
    .await?;

    let assignment_rows = sqlx::query_as::<_, ExamDayRoomAssignmentRow>(
        r#"
        SELECT assignment.id,
               assignment.exam_day_id,
               assignment.classroom_id,
               assignment.room_id,
               assignment.capacity_override,
               classroom.name AS classroom_name,
               room.name_th AS room_name,
               room.capacity AS room_capacity
        FROM academic_exam_day_room_assignments assignment
        JOIN class_rooms classroom ON classroom.id = assignment.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        WHERE assignment.exam_day_id = ANY($1)
        ORDER BY assignment.exam_day_id, classroom.name, room.name_th, assignment.id
        "#,
    )
    .bind(&day_ids)
    .fetch_all(pool)
    .await?;

    let assignment_ids: Vec<Uuid> = assignment_rows
        .iter()
        .map(|assignment| assignment.id)
        .collect();
    let mut invigilators_by_assignment =
        fetch_invigilators_by_assignment_ids(pool, &assignment_ids).await?;

    let mut grade_ids_by_day: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for row in grade_rows {
        grade_ids_by_day
            .entry(row.exam_day_id)
            .or_default()
            .push(row.grade_level_id);
    }

    let mut blocked_windows_by_day: HashMap<Uuid, Vec<BlockedWindow>> = HashMap::new();
    for row in blocked_rows {
        blocked_windows_by_day
            .entry(row.exam_day_id)
            .or_default()
            .push(BlockedWindow {
                id: Some(row.id),
                label: row.label,
                start_time: row.start_time,
                end_time: row.end_time,
            });
    }

    let mut assignments_by_day: HashMap<Uuid, Vec<ExamDayRoomAssignmentView>> = HashMap::new();
    for row in assignment_rows {
        let invigilators = invigilators_by_assignment
            .remove(&row.id)
            .unwrap_or_default();
        assignments_by_day
            .entry(row.exam_day_id)
            .or_default()
            .push(row.into_view(invigilators));
    }

    Ok(days
        .into_iter()
        .map(|day| {
            let day_id = day.id;
            ExamDayDetail {
                id: day.id,
                exam_round_id: day.exam_round_id,
                exam_date: day.exam_date,
                label: day.label,
                start_time: day.start_time,
                end_time: day.end_time,
                grade_level_ids: grade_ids_by_day.remove(&day_id).unwrap_or_default(),
                blocked_windows: blocked_windows_by_day.remove(&day_id).unwrap_or_default(),
                room_assignments: assignments_by_day.remove(&day_id).unwrap_or_default(),
            }
        })
        .collect())
}

async fn fetch_invigilators_by_assignment_ids(
    pool: &PgPool,
    assignment_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<ExamInvigilatorView>>, AppError> {
    let assignment_ids = unique_uuids(assignment_ids.to_vec());
    if assignment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, ExamInvigilatorView>(
        r#"
        SELECT invigilator.id,
               invigilator.exam_day_id,
               invigilator.day_room_assignment_id,
               invigilator.staff_id,
               NULLIF(
                   concat_ws(' ', user_account.title, user_account.first_name, user_account.last_name),
                   ''
               ) AS staff_name,
               invigilator.role_label
        FROM academic_exam_day_invigilators invigilator
        JOIN users user_account ON user_account.id = invigilator.staff_id
        WHERE invigilator.day_room_assignment_id = ANY($1)
        ORDER BY invigilator.day_room_assignment_id,
                 invigilator.role_label NULLS LAST,
                 user_account.first_name,
                 user_account.last_name,
                 invigilator.id
        "#,
    )
    .bind(&assignment_ids)
    .fetch_all(pool)
    .await?;

    let mut grouped = HashMap::new();
    for row in rows {
        grouped
            .entry(row.day_room_assignment_id)
            .or_insert_with(Vec::new)
            .push(row);
    }
    Ok(grouped)
}

fn invigilators_for_assignment(
    assignment_id: Option<Uuid>,
    invigilators_by_assignment: &HashMap<Uuid, Vec<ExamInvigilatorView>>,
) -> Vec<ExamInvigilatorView> {
    assignment_id
        .and_then(|assignment_id| invigilators_by_assignment.get(&assignment_id).cloned())
        .unwrap_or_default()
}

async fn fetch_unscheduled_items(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ExamScheduleItemView>, AppError> {
    sqlx::query_as::<_, ExamScheduleItemView>(
        r#"
        SELECT item.id,
               item.exam_round_id,
               item.academic_semester_id,
               item.assessment_category_id,
               item.assessment_plan_id,
               item.classroom_course_id,
               item.classroom_id,
               item.subject_id,
               item.grade_level_id,
               item.duration_minutes,
               item.imported_at,
               category.name AS assessment_category_name,
               subject.code AS subject_code,
               subject.name_th AS subject_name_th,
               subject.name_en AS subject_name_en,
               subject.group_id AS subject_group_id,
               subject_group.name_th AS subject_group_name,
               subject_group.display_order AS subject_group_display_order,
               subject.type AS subject_type,
               classroom.name AS classroom_name,
               CASE grade_level.level_type
                   WHEN 'kindergarten' THEN CONCAT('อ.', grade_level.year)
                   WHEN 'primary' THEN CONCAT('ป.', grade_level.year)
                   WHEN 'secondary' THEN CONCAT('ม.', grade_level.year)
                   ELSE CONCAT('?.', grade_level.year)
               END AS grade_level_name,
               grade_level.level_type AS grade_level_type,
               grade_level.year AS grade_level_year
        FROM academic_exam_schedule_items item
        JOIN academic_assessment_categories category
          ON category.id = item.assessment_category_id
        JOIN subjects subject ON subject.id = item.subject_id
        LEFT JOIN subject_groups subject_group ON subject_group.id = subject.group_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        JOIN grade_levels grade_level ON grade_level.id = item.grade_level_id
        WHERE item.exam_round_id = $1
          AND NOT EXISTS (
              SELECT 1
              FROM academic_exam_sessions session
              WHERE session.exam_schedule_item_id = item.id
          )
        ORDER BY subject_group.display_order NULLS LAST,
                 subject_group.name_th NULLS LAST,
                 CASE grade_level.level_type
                     WHEN 'kindergarten' THEN 1
                     WHEN 'primary' THEN 2
                     WHEN 'secondary' THEN 3
                     ELSE 4
                 END,
                 grade_level.year,
                 CASE subject.type
                     WHEN 'BASIC' THEN 1
                     WHEN 'ADDITIONAL' THEN 2
                     WHEN 'ACTIVITY' THEN 3
                     ELSE 4
                 END,
                 subject.code,
                 classroom.room_number NULLS LAST,
                 classroom.name,
                 category.display_order,
                 category.name,
                 item.id
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn fetch_scheduled_sessions(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ExamSessionView>, AppError> {
    let rows = sqlx::query_as::<_, ExamSessionRow>(
        r#"
        SELECT session.id,
               session.exam_schedule_item_id,
               session.exam_round_id,
               session.exam_day_id,
               session.starts_at,
               session.ends_at,
               item.academic_semester_id,
               item.assessment_category_id,
               item.assessment_plan_id,
               item.classroom_course_id,
               item.classroom_id,
               item.subject_id,
               item.grade_level_id,
               item.duration_minutes,
               item.imported_at,
               day.exam_date AS exam_date,
               category.name AS assessment_category_name,
               subject.code AS subject_code,
               subject.name_th AS subject_name_th,
               subject.name_en AS subject_name_en,
               subject.group_id AS subject_group_id,
               subject_group.name_th AS subject_group_name,
               subject_group.display_order AS subject_group_display_order,
               subject.type AS subject_type,
               classroom.name AS classroom_name,
               CASE grade_level.level_type
                   WHEN 'kindergarten' THEN CONCAT('อ.', grade_level.year)
                   WHEN 'primary' THEN CONCAT('ป.', grade_level.year)
                   WHEN 'secondary' THEN CONCAT('ม.', grade_level.year)
                   ELSE CONCAT('?.', grade_level.year)
               END AS grade_level_name,
               grade_level.level_type AS grade_level_type,
               grade_level.year AS grade_level_year,
               assignment.id AS day_room_assignment_id,
               assignment.room_id AS room_id,
               room.name_th AS room_name,
               building.name_th AS building_name
        FROM academic_exam_sessions session
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.exam_round_id = session.exam_round_id
        JOIN academic_exam_days day
          ON day.id = session.exam_day_id
         AND day.exam_round_id = session.exam_round_id
        JOIN academic_assessment_categories category
          ON category.id = item.assessment_category_id
        JOIN subjects subject ON subject.id = item.subject_id
        LEFT JOIN subject_groups subject_group ON subject_group.id = subject.group_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        JOIN grade_levels grade_level ON grade_level.id = item.grade_level_id
        LEFT JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        LEFT JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE session.exam_round_id = $1
        ORDER BY day.exam_date,
                 day.start_time,
                 day.id,
                 session.starts_at,
                 classroom.name,
                 subject.code,
                 category.display_order,
                 category.name,
                 session.id
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await?;

    let assignment_ids: Vec<Uuid> = rows
        .iter()
        .filter_map(|row| row.day_room_assignment_id)
        .collect();
    let invigilators_by_assignment =
        fetch_invigilators_by_assignment_ids(pool, &assignment_ids).await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let invigilators = invigilators_for_assignment(
                row.day_room_assignment_id,
                &invigilators_by_assignment,
            );
            row.into_view(invigilators)
        })
        .collect())
}

async fn fetch_workspace_counts(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<WorkspaceCounts, AppError> {
    let row: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(WORKSPACE_COUNTS_SQL)
        .bind(round_id)
        .fetch_one(pool)
        .await?;

    Ok(workspace_counts_from_row(row))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn t(value: &str) -> NaiveTime {
        NaiveTime::parse_from_str(value, "%H:%M").unwrap()
    }

    #[test]
    fn room_assignment_payload_without_invigilators_preserves_existing_staff() {
        let request = serde_json::json!({
            "classroomId": Uuid::from_u128(1),
            "roomId": Uuid::from_u128(2),
            "capacityOverride": null
        });

        let parsed: UpsertDayRoomAssignmentRequest = serde_json::from_value(request).unwrap();

        assert_eq!(parsed.invigilator_staff_ids, None);
    }

    #[test]
    fn room_assignment_payload_with_invigilators_remains_backwards_compatible() {
        let staff_id = Uuid::from_u128(3);
        let request = serde_json::json!({
            "classroomId": Uuid::from_u128(1),
            "roomId": Uuid::from_u128(2),
            "capacityOverride": null,
            "invigilatorStaffIds": [staff_id]
        });

        let parsed: UpsertDayRoomAssignmentRequest = serde_json::from_value(request).unwrap();

        assert_eq!(parsed.invigilator_staff_ids, Some(vec![staff_id]));
    }

    #[test]
    fn invigilator_staff_option_limit_uses_bounded_default() {
        assert_eq!(invigilator_staff_option_limit(None), 40);
        assert_eq!(invigilator_staff_option_limit(Some(0)), 1);
        assert_eq!(invigilator_staff_option_limit(Some(250)), 100);
        assert_eq!(invigilator_staff_option_limit(Some(24)), 24);
    }

    #[test]
    fn invigilator_staff_option_search_pattern_trims_empty_values() {
        assert_eq!(invigilator_staff_option_search_pattern(None), None);
        assert_eq!(
            invigilator_staff_option_search_pattern(Some("   ".to_string())),
            None
        );
        assert_eq!(
            invigilator_staff_option_search_pattern(Some("  Kru A  ".to_string())),
            Some("%Kru A%".to_string())
        );
    }

    #[test]
    fn computes_end_time_from_duration() {
        assert_eq!(add_minutes(t("08:30"), 90).unwrap(), t("10:00"));
    }

    #[test]
    fn rejects_zero_duration() {
        assert_eq!(
            add_minutes(t("08:30"), 0),
            Err(SessionValidationError::InvalidDuration)
        );
    }

    #[test]
    fn rejects_negative_duration() {
        assert_eq!(
            add_minutes(t("08:30"), -30),
            Err(SessionValidationError::InvalidDuration)
        );
    }

    #[test]
    fn rejects_end_time_overflow() {
        assert_eq!(
            add_minutes(t("23:30"), 60),
            Err(SessionValidationError::EndTimeOverflow)
        );
    }

    #[test]
    fn detects_half_open_time_overlap() {
        assert!(time_ranges_overlap(
            t("08:30"),
            t("10:00"),
            t("09:59"),
            t("11:00")
        ));
        assert!(!time_ranges_overlap(
            t("08:30"),
            t("10:00"),
            t("10:00"),
            t("11:00")
        ));
    }

    #[test]
    fn detects_classroom_time_conflict() {
        let candidate = CandidateSession {
            session_id: None,
            classroom_id: Uuid::nil(),
            exam_day_id: Uuid::nil(),
            starts_at: t("09:00"),
            ends_at: t("10:00"),
        };
        let existing = vec![CandidateSession {
            session_id: Some(Uuid::max()),
            classroom_id: Uuid::nil(),
            exam_day_id: Uuid::nil(),
            starts_at: t("09:30"),
            ends_at: t("10:30"),
        }];
        assert!(has_same_classroom_conflict(&candidate, &existing));
    }

    #[test]
    fn invigilator_workload_sums_session_minutes_without_gaps() {
        let assignment_id = Uuid::from_u128(1);
        let staff_id = Uuid::from_u128(2);
        let windows = vec![
            InvigilatorSessionWindow {
                assignment_id,
                exam_day_id: Uuid::from_u128(10),
                staff_id,
                starts_at: t("08:30"),
                ends_at: t("09:30"),
            },
            InvigilatorSessionWindow {
                assignment_id,
                exam_day_id: Uuid::from_u128(10),
                staff_id,
                starts_at: t("10:00"),
                ends_at: t("11:30"),
            },
        ];

        let minutes = invigilator_workload_minutes(&windows);

        assert_eq!(minutes, 150);
    }

    #[test]
    fn invigilator_staff_workloads_group_by_staff_and_day() {
        let staff_id = Uuid::from_u128(7);
        let exam_day_id = Uuid::from_u128(10);
        let rows = vec![
            InvigilatorSessionWindowRow {
                assignment_id: Uuid::from_u128(1),
                exam_day_id,
                staff_id,
                staff_name: "Teacher One".to_string(),
                starts_at: t("08:30"),
                ends_at: t("09:30"),
            },
            InvigilatorSessionWindowRow {
                assignment_id: Uuid::from_u128(2),
                exam_day_id,
                staff_id,
                staff_name: "Teacher One".to_string(),
                starts_at: t("10:00"),
                ends_at: t("11:30"),
            },
        ];

        let workloads = build_invigilator_staff_workloads(rows);

        assert_eq!(workloads.len(), 1);
        assert_eq!(workloads[0].staff_id, staff_id);
        assert_eq!(workloads[0].staff_name, "Teacher One");
        assert_eq!(workloads[0].total_minutes, 150);
        assert_eq!(workloads[0].assigned_day_count, 1);
        assert_eq!(workloads[0].assignment_count, 2);
        assert_eq!(workloads[0].days.len(), 1);
        assert_eq!(workloads[0].days[0].exam_day_id, exam_day_id);
        assert_eq!(workloads[0].days[0].minutes, 150);
        assert_eq!(workloads[0].days[0].assignment_count, 2);
    }

    #[test]
    fn invigilator_conflict_rejects_overlapping_live_session_ranges() {
        let staff_id = Uuid::from_u128(7);
        let candidate = vec![InvigilatorSessionWindow {
            assignment_id: Uuid::from_u128(1),
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("08:30"),
            ends_at: t("09:30"),
        }];
        let existing = vec![InvigilatorSessionWindow {
            assignment_id: Uuid::from_u128(2),
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("09:00"),
            ends_at: t("10:00"),
        }];

        assert!(has_invigilator_time_conflict(
            Uuid::from_u128(1),
            &candidate,
            &existing
        ));
    }

    #[test]
    fn invigilator_conflict_allows_non_overlapping_same_day_assignments() {
        let staff_id = Uuid::from_u128(7);
        let candidate = vec![InvigilatorSessionWindow {
            assignment_id: Uuid::from_u128(1),
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("08:30"),
            ends_at: t("09:30"),
        }];
        let existing = vec![InvigilatorSessionWindow {
            assignment_id: Uuid::from_u128(2),
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("09:30"),
            ends_at: t("10:30"),
        }];

        assert!(!has_invigilator_time_conflict(
            Uuid::from_u128(1),
            &candidate,
            &existing
        ));
    }

    #[test]
    fn exam_invigilator_staff_lock_keys_are_sorted_deduped_and_stable() {
        let exam_day_id = Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap();
        let staff_a = Uuid::parse_str("11121314-1516-1718-191a-1b1c1d1e1f20").unwrap();
        let staff_b = Uuid::parse_str("21222324-2526-2728-292a-2b2c2d2e2f30").unwrap();

        let keys = exam_invigilator_staff_lock_keys(exam_day_id, &[staff_b, staff_a, staff_a]);

        assert_eq!(
            keys,
            exam_invigilator_staff_lock_keys(exam_day_id, &[staff_a, staff_b])
        );
        assert_eq!(keys.len(), 2);
        assert!(keys[0] < keys[1]);
    }

    #[test]
    fn update_assignment_invigilators_locks_staff_scope_before_conflict_validation() {
        let source = include_str!("exam_schedule_service.rs");
        let update_start = source
            .find("pub async fn update_assignment_invigilators")
            .unwrap();
        let update_body = &source[update_start..];
        let lock_position = update_body
            .find("lock_exam_invigilator_staff_conflict_scope")
            .unwrap();
        let validation_position = update_body
            .find("validate_invigilator_time_conflicts")
            .unwrap();

        assert!(lock_position < validation_position);
    }

    #[test]
    fn upsert_day_room_assignment_locks_optional_invigilator_scope_before_conflict_validation() {
        let source = include_str!("exam_schedule_service.rs");
        let upsert_start = source
            .find("pub async fn upsert_day_room_assignment")
            .unwrap();
        let upsert_tail = &source[upsert_start..];
        let update_start = upsert_tail
            .find("pub async fn update_assignment_invigilators")
            .unwrap();
        let upsert_body = &upsert_tail[..update_start];
        let lock_position = upsert_body
            .find("lock_exam_invigilator_staff_conflict_scope")
            .unwrap();
        let validation_position = upsert_body
            .find("validate_invigilator_time_conflicts")
            .unwrap();

        assert!(lock_position < validation_position);
    }

    #[test]
    fn place_exam_session_locks_and_validates_invigilators_before_insert() {
        let source = include_str!("exam_schedule_service.rs");
        let placement_start = source.find("pub async fn place_exam_session").unwrap();
        let placement_body = &source[placement_start..];
        let lock_position = placement_body
            .find("lock_exam_invigilator_staff_conflict_scope")
            .unwrap();
        let validation_position = placement_body
            .find("validate_invigilator_candidate_session_conflicts")
            .unwrap();
        let insert_position = placement_body
            .find("INSERT INTO academic_exam_sessions")
            .unwrap();

        assert!(lock_position < validation_position);
        assert!(validation_position < insert_position);
    }

    #[test]
    fn builds_invigilator_candidate_session_windows_for_each_staff_member() {
        let assignment_id = Uuid::from_u128(1);
        let exam_day_id = Uuid::from_u128(2);
        let staff_a = Uuid::from_u128(3);
        let staff_b = Uuid::from_u128(4);

        let windows = build_invigilator_candidate_session_windows(
            assignment_id,
            exam_day_id,
            t("08:30"),
            t("09:30"),
            &[staff_a, staff_b],
        );

        assert_eq!(
            windows,
            vec![
                InvigilatorSessionWindow {
                    assignment_id,
                    exam_day_id,
                    staff_id: staff_a,
                    starts_at: t("08:30"),
                    ends_at: t("09:30"),
                },
                InvigilatorSessionWindow {
                    assignment_id,
                    exam_day_id,
                    staff_id: staff_b,
                    starts_at: t("08:30"),
                    ends_at: t("09:30"),
                },
            ]
        );
    }

    #[test]
    fn get_invigilator_workspace_checks_round_before_assignment_queries() {
        let source = include_str!("exam_schedule_service.rs");
        let workspace_start = source
            .find("pub async fn get_invigilator_workspace")
            .unwrap();
        let workspace_body = &source[workspace_start..];
        let round_position = workspace_body
            .find("fetch_round(pool, round_id).await?")
            .unwrap();
        let assignments_position = workspace_body
            .find("fetch_invigilator_assignment_summaries")
            .unwrap();

        assert!(round_position < assignments_position);
    }

    #[test]
    fn import_exam_items_filters_source_categories_by_round_kind() {
        let source = include_str!("exam_schedule_service.rs");
        let import_start = source.find("pub async fn import_exam_items").unwrap();
        let import_tail = &source[import_start..];
        let next_function_start = import_tail
            .find("pub async fn list_day_room_assignments")
            .unwrap();
        let import_body = &import_tail[..next_function_start];

        assert!(import_body.contains("exam_kind"));
        assert_eq!(
            import_body.matches("c.code = rc.exam_kind").count(),
            3,
            "existing, missing-duration, and insert source queries must filter by round kind"
        );
    }

    #[test]
    fn clear_mismatched_exam_items_deletes_only_items_outside_round_kind() {
        let source = include_str!("exam_schedule_service.rs");
        let clear_start = source
            .find("pub async fn clear_mismatched_exam_items")
            .unwrap();
        let clear_tail = &source[clear_start..];
        let next_function_start = clear_tail
            .find("pub async fn list_day_room_assignments")
            .unwrap();
        let clear_body = &clear_tail[..next_function_start];

        assert!(clear_body.contains("SELECT status"));
        assert!(clear_body.contains("FOR UPDATE"));
        assert!(clear_body.contains("DELETE FROM academic_exam_schedule_items"));
        assert!(clear_body.contains("USING academic_assessment_categories c"));
        assert!(clear_body.contains("round_context rc"));
        assert!(clear_body.contains("item.assessment_category_id = c.id"));
        assert!(clear_body.contains("c.code IS DISTINCT FROM rc.exam_kind"));
        assert!(clear_body.contains("mark_round_draft_after_mutation"));
    }

    #[test]
    fn publish_round_locks_round_before_readiness_check() {
        let source = include_str!("exam_schedule_service.rs");
        let publish_start = source.find("pub async fn publish_round").unwrap();
        let publish_body = &source[publish_start..];
        let tx_position = publish_body
            .find("let mut tx = pool.begin().await?")
            .unwrap();
        let lock_position = publish_body.find("FOR UPDATE").unwrap();
        let readiness_position = publish_body.find("fetch_workspace_counts_in_tx").unwrap();
        let update_position = publish_body.find("UPDATE academic_exam_rounds").unwrap();

        assert!(tx_position < lock_position);
        assert!(lock_position < readiness_position);
        assert!(readiness_position < update_position);
    }

    #[test]
    fn placement_locks_conflict_scope_before_conflict_queries() {
        let source = include_str!("exam_schedule_service.rs");
        let placement_start = source.find("pub async fn place_exam_session").unwrap();
        let placement_body = &source[placement_start..];
        let lock_position = placement_body
            .find("lock_exam_session_conflict_scope")
            .unwrap();
        let classroom_conflict_query_position = placement_body
            .find("fetch_candidate_sessions_for_day")
            .unwrap();
        let room_conflict_query_position = placement_body
            .find("fetch_candidate_room_sessions_for_day")
            .unwrap();

        assert!(lock_position < classroom_conflict_query_position);
        assert!(lock_position < room_conflict_query_position);
    }

    #[test]
    fn exam_session_conflict_lock_keys_are_sorted_and_scoped() {
        let exam_day_id = Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap();
        let classroom_id = Uuid::parse_str("11121314-1516-1718-191a-1b1c1d1e1f20").unwrap();
        let room_id = Uuid::parse_str("21222324-2526-2728-292a-2b2c2d2e2f30").unwrap();

        let keys = exam_session_conflict_lock_keys(exam_day_id, classroom_id, room_id);

        assert_eq!(
            keys,
            exam_session_conflict_lock_keys(exam_day_id, classroom_id, room_id)
        );
        assert!(keys[0] < keys[1]);
    }

    #[test]
    fn rejects_placement_outside_day_window() {
        let outcome = validate_session_window(
            t("08:00"),
            120,
            t("08:30"),
            t("16:00"),
            &[BlockedWindow {
                id: None,
                label: "Lunch".to_string(),
                start_time: t("12:00"),
                end_time: t("13:00"),
            }],
        );
        assert!(matches!(
            outcome,
            Err(SessionValidationError::BeforeDayStart)
        ));
    }

    #[test]
    fn rejects_placement_after_day_end() {
        let outcome = validate_session_window(t("15:30"), 60, t("08:30"), t("16:00"), &[]);
        assert!(matches!(outcome, Err(SessionValidationError::AfterDayEnd)));
    }

    #[test]
    fn rejects_placement_across_blocked_window() {
        let outcome = validate_session_window(
            t("11:30"),
            90,
            t("08:30"),
            t("16:00"),
            &[BlockedWindow {
                id: None,
                label: "Lunch".to_string(),
                start_time: t("12:00"),
                end_time: t("13:00"),
            }],
        );
        assert!(matches!(
            outcome,
            Err(SessionValidationError::BlockedWindow(_))
        ));
    }

    #[test]
    fn accepts_placement_start_time_on_5_minute_slot() {
        let outcome = validate_session_window(t("08:35"), 60, t("08:30"), t("16:00"), &[]);

        assert!(outcome.is_ok());
    }

    #[test]
    fn rejects_placement_start_time_outside_5_minute_slot() {
        let outcome = validate_session_window(t("08:37"), 60, t("08:30"), t("16:00"), &[]);

        assert!(outcome.is_err());
    }

    #[test]
    fn empty_day_grade_scope_allows_any_grade_level() {
        assert!(grade_level_allowed_by_day_scope(Uuid::nil(), &[]));
    }

    #[test]
    fn explicit_day_grade_scope_rejects_removed_grade_level() {
        assert!(!grade_level_allowed_by_day_scope(
            Uuid::from_u128(1),
            &[Uuid::from_u128(2)]
        ));
    }

    #[test]
    fn readiness_sql_rechecks_sessions_after_day_window_changes() {
        assert!(WORKSPACE_COUNTS_SQL.contains("invalid_session_count"));
        assert!(WORKSPACE_COUNTS_SQL.contains("session.starts_at < day.start_time"));
        assert!(WORKSPACE_COUNTS_SQL.contains("session.ends_at > day.end_time"));
    }

    #[test]
    fn readiness_sql_rechecks_sessions_after_blocked_window_changes() {
        assert!(WORKSPACE_COUNTS_SQL.contains("academic_exam_day_blocked_windows blocked"));
        assert!(WORKSPACE_COUNTS_SQL.contains("session.starts_at < blocked.end_time"));
        assert!(WORKSPACE_COUNTS_SQL.contains("blocked.start_time < session.ends_at"));
    }

    #[test]
    fn readiness_sql_rechecks_sessions_after_grade_scope_changes() {
        assert!(WORKSPACE_COUNTS_SQL.contains("academic_exam_day_grade_levels scope"));
        assert!(WORKSPACE_COUNTS_SQL.contains("scope.grade_level_id = item.grade_level_id"));
    }

    #[test]
    fn readiness_sql_requires_seats_for_every_active_student() {
        assert!(WORKSPACE_COUNTS_SQL.contains("missing_seat_student_count"));
        assert!(WORKSPACE_COUNTS_SQL.contains("student_class_enrollments enrollment"));
        assert!(WORKSPACE_COUNTS_SQL.contains("seat.student_id IS NULL"));
    }

    #[test]
    fn readiness_sql_counts_invigilator_live_range_conflicts() {
        assert!(WORKSPACE_COUNTS_SQL.contains("invigilator_conflict_count"));
        assert!(WORKSPACE_COUNTS_SQL.contains("academic_exam_day_invigilators"));
        assert!(WORKSPACE_COUNTS_SQL.contains("left_session.starts_at < right_session.ends_at"));
        assert!(WORKSPACE_COUNTS_SQL.contains("right_session.starts_at < left_session.ends_at"));
    }

    #[test]
    fn day_staff_unique_error_mapping_is_removed_after_live_range_migration() {
        let source = include_str!("exam_schedule_service.rs");
        let mapping_start = source
            .find("fn map_day_room_assignment_write_error")
            .unwrap();
        let mapping_body = &source[mapping_start..];
        let mapping_end = mapping_body.find("fn validate_exam_day_window").unwrap();

        assert!(!mapping_body[..mapping_end].contains("exam_day_id_staff_id"));
    }

    #[test]
    fn readiness_requires_days_items_rooms_and_sessions() {
        let readiness = build_readiness(WorkspaceCounts {
            day_count: 0,
            item_count: 4,
            unscheduled_count: 4,
            missing_room_assignment_count: 2,
            invalid_session_count: 0,
            missing_seat_student_count: 2,
            invigilator_conflict_count: 0,
        });
        assert!(!readiness.can_publish);
        assert!(readiness
            .blockers
            .iter()
            .any(|value| value.contains("exam day")));
        assert!(readiness
            .blockers
            .iter()
            .any(|value| value.contains("unscheduled")));
    }

    #[test]
    fn readiness_reports_missing_active_student_seats() {
        let readiness = build_readiness(WorkspaceCounts {
            day_count: 1,
            item_count: 4,
            unscheduled_count: 0,
            missing_room_assignment_count: 0,
            invalid_session_count: 0,
            missing_seat_student_count: 3,
            invigilator_conflict_count: 0,
        });

        assert!(!readiness.can_publish);
        assert!(readiness
            .blockers
            .iter()
            .any(|value| value.contains("active student")));
    }

    #[test]
    fn readiness_reports_invalid_scheduled_sessions() {
        let readiness = build_readiness(WorkspaceCounts {
            day_count: 1,
            item_count: 4,
            unscheduled_count: 0,
            missing_room_assignment_count: 0,
            invalid_session_count: 2,
            missing_seat_student_count: 0,
            invigilator_conflict_count: 0,
        });

        assert!(!readiness.can_publish);
        assert!(readiness
            .blockers
            .iter()
            .any(|value| value.contains("no longer fit")));
    }

    #[test]
    fn readiness_reports_invigilator_live_range_conflicts() {
        let readiness = build_readiness(WorkspaceCounts {
            day_count: 1,
            item_count: 4,
            unscheduled_count: 0,
            missing_room_assignment_count: 0,
            invalid_session_count: 0,
            missing_seat_student_count: 0,
            invigilator_conflict_count: 2,
        });

        assert!(!readiness.can_publish);
        assert!(readiness
            .blockers
            .iter()
            .any(|value| value.contains("overlapping invigilator")));
    }

    #[test]
    fn rejects_round_update_without_supplied_fields() {
        let result = normalize_update_round_request(UpdateExamRoundRequest {
            name: None,
            description: None,
            exam_kind: None,
        });

        assert!(matches!(
            result,
            Err(AppError::BadRequest(message)) if message.contains("No fields")
        ));
    }

    #[test]
    fn normalizes_supported_exam_round_kinds() {
        assert_eq!(normalize_exam_kind(None).unwrap(), "midterm");
        assert_eq!(normalize_exam_kind(Some(" final ")).unwrap(), "final");

        assert!(matches!(
            normalize_exam_kind(Some("quiz")),
            Err(AppError::BadRequest(message)) if message.contains("midterm or final")
        ));
    }

    #[test]
    fn rejects_blocked_windows_outside_exam_day_range() {
        let result = normalize_blocked_windows(
            t("08:30"),
            t("16:00"),
            vec![BlockedWindowInput {
                label: "Before school".to_string(),
                start_time: t("08:00"),
                end_time: t("08:45"),
            }],
        );

        assert!(matches!(
            result,
            Err(AppError::BadRequest(message)) if message.contains("within the exam day")
        ));
    }

    #[test]
    fn shared_assignment_invigilators_are_reused_for_each_session() {
        let assignment_id = Uuid::from_u128(1);
        let invigilator = ExamInvigilatorView {
            id: Uuid::from_u128(2),
            exam_day_id: Uuid::from_u128(3),
            day_room_assignment_id: assignment_id,
            staff_id: Uuid::from_u128(4),
            staff_name: Some("Exam Staff".to_string()),
            role_label: Some("Lead".to_string()),
        };
        let invigilators_by_assignment =
            HashMap::from([(assignment_id, vec![invigilator.clone()])]);

        let first = invigilators_for_assignment(Some(assignment_id), &invigilators_by_assignment);
        let second = invigilators_for_assignment(Some(assignment_id), &invigilators_by_assignment);

        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_eq!(first[0].id, invigilator.id);
        assert_eq!(second[0].id, invigilator.id);
        assert!(invigilators_by_assignment.contains_key(&assignment_id));
    }

    #[test]
    fn generates_padded_seat_numbers_in_input_order() {
        let students = vec![
            SeatStudent {
                student_id: Uuid::nil(),
            },
            SeatStudent {
                student_id: Uuid::max(),
            },
        ];
        let seats = build_default_seat_assignments(&students);
        assert_eq!(seats[0].seat_number, "01");
        assert_eq!(seats[1].seat_number, "02");
    }

    #[test]
    fn rejects_seat_generation_when_student_count_exceeds_capacity() {
        let result = validate_seat_generation_capacity(41, 40);

        assert!(matches!(
            result,
            Err(AppError::BadRequest(message)) if message.contains("exceeds")
        ));
    }

    #[test]
    fn rejects_seat_generation_when_effective_capacity_is_not_positive() {
        let result = validate_seat_generation_capacity(0, 0);

        assert!(matches!(
            result,
            Err(AppError::BadRequest(message)) if message.contains("greater than zero")
        ));
    }
}
