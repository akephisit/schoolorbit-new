use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::timetable_block::{
    TimetableBlockConflict, TimetableBlockConflictType, TimetableBlockMutationKind,
    TimetableBlockPlacementCandidate, TimetableBlockPlacementPreview,
    TimetableBlockPlacementPreviewRequest, TimetableBlockPlacementSource,
    TimetableBlockPlacementState, TimetableTargetKind,
};
use sqlx::PgPool;

const VALID_DAYS: &[&str] = &["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];

pub(crate) fn normalize_day(day: &str) -> Result<String, AppError> {
    let day = day.trim().to_ascii_uppercase();
    if VALID_DAYS.contains(&day.as_str()) {
        Ok(day)
    } else {
        Err(AppError::ValidationError(
            "วันสำหรับจัดตารางสอนไม่ถูกต้อง".to_string(),
        ))
    }
}

pub(crate) fn canonical_ids(ids: &[Uuid]) -> Vec<Uuid> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    ids
}

pub(crate) fn map_write_error(error: sqlx::Error) -> AppError {
    let message = match &error {
        sqlx::Error::Database(database) => match database.message() {
            "ACADEMIC_TIMETABLE_GROUP_CONFLICT" => Some("กลุ่มเรียนนี้มีรายการในวันและคาบดังกล่าวแล้ว"),
            "ACADEMIC_TIMETABLE_HOMEROOM_CONFLICT" => Some("ห้องประจำชั้นนี้มีรายการในวันและคาบดังกล่าวแล้ว"),
            "ACADEMIC_TIMETABLE_ROOM_CONFLICT" => Some("ห้องเรียนนี้ถูกใช้ในวันและคาบดังกล่าวแล้ว"),
            "ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED" => Some("ครูมีรายการอื่นในวันและคาบดังกล่าวแล้ว"),
            "ACADEMIC_PUBLISHED_TIMETABLE_VERSION_CHILD_IMMUTABLE" => {
                Some("รุ่นตารางสอนที่เผยแพร่แล้วแก้ไขไม่ได้")
            }
            _ => None,
        },
        _ => None,
    };
    message
        .map(|message| AppError::Conflict(message.to_string()))
        .unwrap_or(AppError::DbError(error))
}

pub(crate) async fn preview_placement(
    pool: &PgPool,
    request: TimetableBlockPlacementPreviewRequest,
) -> Result<TimetableBlockPlacementPreview, AppError> {
    let target_day = normalize_day(&request.target_day_of_week)?;
    let mut candidate = request.candidate;
    candidate.homeroom_ids = canonical_ids(&candidate.homeroom_ids);
    candidate.teacher_ids = canonical_ids(&candidate.teacher_ids);
    candidate.instructor_ids = canonical_ids(&candidate.instructor_ids);
    let source_block_id = match request.source {
        TimetableBlockPlacementSource::ExistingBlock { block_id, .. } => Some(block_id),
        _ => None,
    };
    let mut excluded_block_ids = source_block_id.into_iter().collect::<Vec<_>>();
    if let Some(target_id) = request.expected_target_block_id {
        excluded_block_ids.push(target_id);
    }
    excluded_block_ids = canonical_ids(&excluded_block_ids);

    let version_valid: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1 FROM academic_timetable_versions version
               JOIN bell_schedule_periods period
                 ON period.bell_schedule_id = version.bell_schedule_id
               WHERE version.id = $1 AND version.academic_term_id = $2
                 AND version.status = 'draft' AND period.id = $3 AND period.is_active
           )"#,
    )
    .bind(request.timetable_version_id)
    .bind(request.academic_term_id)
    .bind(request.target_bell_schedule_period_id)
    .fetch_one(pool)
    .await?;
    if !version_valid {
        return Ok(blocked_preview(
            source_block_id,
            request.expected_target_block_id,
            target_day,
            request.target_bell_schedule_period_id,
            candidate,
            TimetableBlockConflict {
                conflict_type: TimetableBlockConflictType::Version,
                code: "TIMETABLE_VERSION_NOT_EDITABLE".to_string(),
                message: "รุ่นตารางสอนไม่พร้อมแก้ไขหรือคาบไม่อยู่ในตารางเวลาที่เลือก".to_string(),
                existing_block_id: None,
                target_kind: None,
                target_id: None,
            },
        ));
    }

    let group_ids = candidate.learning_group_id.into_iter().collect::<Vec<_>>();
    let covered_homeroom_ids: Vec<Uuid> = if group_ids.is_empty() {
        candidate.homeroom_ids.clone()
    } else {
        let mut ids: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT homeroom_id FROM learning_group_homerooms
               WHERE learning_group_id = ANY($1)"#,
        )
        .bind(&group_ids)
        .fetch_all(pool)
        .await?;
        ids.extend(candidate.homeroom_ids.iter().copied());
        canonical_ids(&ids)
    };
    let mut teacher_ids = candidate.teacher_ids.clone();
    teacher_ids.extend(candidate.instructor_ids.iter().copied());
    teacher_ids = canonical_ids(&teacher_ids);
    let room_ids = candidate.room_id.into_iter().collect::<Vec<_>>();

    let conflicts: Vec<(String, Uuid, Uuid)> = sqlx::query_as(
        r#"WITH occupied AS (
               SELECT 'learning_group'::text AS kind, block.id AS block_id,
                      target.learning_group_id AS target_id
               FROM academic_timetable_blocks block
               JOIN academic_timetable_block_groups target ON target.block_id = block.id
               WHERE block.timetable_version_id = $1 AND block.day_of_week = $2
                 AND block.bell_schedule_period_id = $3
                 AND block.is_active AND target.is_active
                 AND target.learning_group_id = ANY($4)
               UNION ALL
               SELECT 'homeroom', block.id, target.homeroom_id
               FROM academic_timetable_blocks block
               JOIN academic_timetable_block_homerooms target ON target.block_id = block.id
               WHERE block.timetable_version_id = $1 AND block.day_of_week = $2
                 AND block.bell_schedule_period_id = $3
                 AND block.is_active AND target.is_active
                 AND target.homeroom_id = ANY($5)
               UNION ALL
               SELECT 'homeroom', block.id, coverage.homeroom_id
               FROM academic_timetable_blocks block
               JOIN academic_timetable_block_groups target ON target.block_id = block.id
               JOIN learning_group_homerooms coverage
                 ON coverage.learning_group_id = target.learning_group_id
               WHERE block.timetable_version_id = $1 AND block.day_of_week = $2
                 AND block.bell_schedule_period_id = $3
                 AND block.is_active AND target.is_active
                 AND coverage.homeroom_id = ANY($5)
               UNION ALL
               SELECT 'teacher', block.id, target.teacher_id
               FROM academic_timetable_blocks block
               JOIN academic_timetable_block_teachers target ON target.block_id = block.id
               WHERE block.timetable_version_id = $1 AND block.day_of_week = $2
                 AND block.bell_schedule_period_id = $3
                 AND block.is_active AND target.is_active
                 AND target.teacher_id = ANY($6)
               UNION ALL
               SELECT 'teacher', block.id, instructor.instructor_id
               FROM academic_timetable_blocks block
               JOIN academic_timetable_block_groups target ON target.block_id = block.id
               JOIN academic_timetable_block_group_instructors instructor
                 ON instructor.block_group_id = target.id
               WHERE block.timetable_version_id = $1 AND block.day_of_week = $2
                 AND block.bell_schedule_period_id = $3
                 AND block.is_active AND target.is_active
                 AND instructor.instructor_id = ANY($6)
               UNION ALL
               SELECT 'room', block.id, target.room_id
               FROM academic_timetable_blocks block
               JOIN academic_timetable_block_groups target ON target.block_id = block.id
               WHERE block.timetable_version_id = $1 AND block.day_of_week = $2
                 AND block.bell_schedule_period_id = $3
                 AND block.is_active AND target.is_active
                 AND target.room_id = ANY($7)
               UNION ALL
               SELECT 'room', block.id, target.room_id
               FROM academic_timetable_blocks block
               JOIN academic_timetable_block_homerooms target ON target.block_id = block.id
               WHERE block.timetable_version_id = $1 AND block.day_of_week = $2
                 AND block.bell_schedule_period_id = $3
                 AND block.is_active AND target.is_active
                 AND target.room_id = ANY($7)
           )
           SELECT DISTINCT kind, block_id, target_id
           FROM occupied
           WHERE NOT (block_id = ANY($8))
           ORDER BY kind, block_id, target_id"#,
    )
    .bind(request.timetable_version_id)
    .bind(&target_day)
    .bind(request.target_bell_schedule_period_id)
    .bind(&group_ids)
    .bind(&covered_homeroom_ids)
    .bind(&teacher_ids)
    .bind(&room_ids)
    .bind(&excluded_block_ids)
    .fetch_all(pool)
    .await?;
    let conflicts = conflicts
        .into_iter()
        .map(|(kind, block_id, target_id)| conflict_from_row(&kind, block_id, target_id))
        .collect::<Result<Vec<_>, _>>()?;
    let state = if !conflicts.is_empty() {
        TimetableBlockPlacementState::Blocked
    } else if request.expected_target_block_id.is_some() {
        TimetableBlockPlacementState::Swap
    } else if source_block_id.is_some() {
        TimetableBlockPlacementState::Move
    } else {
        TimetableBlockPlacementState::Source
    };
    let mutation = match state {
        TimetableBlockPlacementState::Source => Some(TimetableBlockMutationKind::Create),
        TimetableBlockPlacementState::Move => Some(TimetableBlockMutationKind::Move),
        TimetableBlockPlacementState::Swap => Some(TimetableBlockMutationKind::Swap),
        TimetableBlockPlacementState::Blocked => None,
    };
    Ok(TimetableBlockPlacementPreview {
        state,
        source_block_id,
        target_block_id: request.expected_target_block_id,
        target_day_of_week: target_day,
        target_bell_schedule_period_id: request.target_bell_schedule_period_id,
        normalized_candidate: candidate,
        conflicts,
        mutation,
    })
}

fn conflict_from_row(
    kind: &str,
    existing_block_id: Uuid,
    target_id: Uuid,
) -> Result<TimetableBlockConflict, AppError> {
    let (conflict_type, code, message, target_kind) = match kind {
        "learning_group" => (
            TimetableBlockConflictType::LearningGroup,
            "TIMETABLE_GROUP_CONFLICT",
            "กลุ่มเรียนมีคาบอื่นในช่วงเวลานี้",
            Some(TimetableTargetKind::Group),
        ),
        "homeroom" => (
            TimetableBlockConflictType::Homeroom,
            "TIMETABLE_HOMEROOM_CONFLICT",
            "ห้องประจำชั้นมีคาบอื่นในช่วงเวลานี้",
            Some(TimetableTargetKind::Homeroom),
        ),
        "teacher" => (
            TimetableBlockConflictType::Teacher,
            "TIMETABLE_TEACHER_CONFLICT",
            "ครูมีคาบอื่นในช่วงเวลานี้",
            Some(TimetableTargetKind::Teacher),
        ),
        "room" => (
            TimetableBlockConflictType::Room,
            "TIMETABLE_ROOM_CONFLICT",
            "ห้องเรียนถูกใช้ในช่วงเวลานี้",
            None,
        ),
        _ => {
            return Err(AppError::InternalServerError(
                "ชนิดความขัดแย้งของตารางสอนไม่ถูกต้อง".to_string(),
            ))
        }
    };
    Ok(TimetableBlockConflict {
        conflict_type,
        code: code.to_string(),
        message: message.to_string(),
        existing_block_id: Some(existing_block_id),
        target_kind,
        target_id: Some(target_id),
    })
}

fn blocked_preview(
    source_block_id: Option<Uuid>,
    target_block_id: Option<Uuid>,
    target_day_of_week: String,
    target_bell_schedule_period_id: Uuid,
    normalized_candidate: TimetableBlockPlacementCandidate,
    conflict: TimetableBlockConflict,
) -> TimetableBlockPlacementPreview {
    TimetableBlockPlacementPreview {
        state: TimetableBlockPlacementState::Blocked,
        source_block_id,
        target_block_id,
        target_day_of_week,
        target_bell_schedule_period_id,
        normalized_candidate,
        conflicts: vec![conflict],
        mutation: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_days_and_target_ids() {
        assert_eq!(normalize_day(" mon ").unwrap(), "MON");
        assert!(normalize_day("holiday").is_err());
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        assert_eq!(canonical_ids(&[second, first, second]), {
            let mut expected = vec![first, second];
            expected.sort_unstable();
            expected
        });
    }
}
