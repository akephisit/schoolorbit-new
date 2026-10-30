use school_database::DbSession;
use uuid::Uuid;

use crate::models::timetable_block::{
    TimetableBlockConflict, TimetableBlockConflictType, TimetableBlockMutationKind,
    TimetableBlockPlacementCandidate, TimetableBlockPlacementPreview,
    TimetableBlockPlacementPreviewRequest, TimetableBlockPlacementSource,
    TimetableBlockPlacementState, TimetableTargetKind,
};
use school_errors::AppError;

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
    connection: &mut sqlx::PgConnection,
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

    let version_valid: bool = DbSession::new(connection)
        .scalar(
            "academic_timetable.services.timetable_block_conflicts.preview_placement",
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
        .fetch_one()
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

    let source_block = if let TimetableBlockPlacementSource::ExistingBlock {
        block_id,
        row_version,
    } = &request.source
    {
        load_placement_block(
            connection,
            *block_id,
            request.timetable_version_id,
            request.academic_term_id,
        )
        .await?
        .filter(|block| block.row_version == *row_version)
    } else {
        None
    };
    let target_block = if let Some(block_id) = request.expected_target_block_id {
        load_placement_block(
            connection,
            block_id,
            request.timetable_version_id,
            request.academic_term_id,
        )
        .await?
        .filter(|block| {
            Some(block.row_version) == request.expected_target_row_version
                && block.day_of_week == target_day
                && block.bell_schedule_period_id == request.target_bell_schedule_period_id
                && source_block_id != Some(block_id)
        })
    } else {
        None
    };
    if (source_block_id.is_some() && source_block.is_none())
        || (request.expected_target_block_id.is_some()
            && (source_block.is_none() || target_block.is_none()))
        || (request.expected_target_block_id.is_none()
            && request.expected_target_row_version.is_some())
    {
        return Ok(blocked_preview(
            source_block_id,
            request.expected_target_block_id,
            target_day,
            request.target_bell_schedule_period_id,
            candidate,
            TimetableBlockConflict {
                conflict_type: TimetableBlockConflictType::StaleBlock,
                code: "TIMETABLE_STALE_BLOCK".to_string(),
                message: "รายการต้นทางหรือรายการที่จะสลับเปลี่ยนแปลงแล้ว กรุณาโหลดตารางสอนใหม่".to_string(),
                existing_block_id: None,
                target_kind: None,
                target_id: None,
            },
        ));
    }
    let mut excluded_block_ids = source_block_id.into_iter().collect::<Vec<_>>();
    excluded_block_ids.extend(request.expected_target_block_id);
    let reservations = if let Some(block) = &source_block {
        block.reservations()
    } else {
        candidate_reservations(connection, request.timetable_version_id, &candidate).await?
    };
    let mut conflicts = placement_conflicts(
        connection,
        request.timetable_version_id,
        &target_day,
        request.target_bell_schedule_period_id,
        &reservations,
        &excluded_block_ids,
    )
    .await?;
    if let (Some(source), Some(target)) = (&source_block, &target_block) {
        let mut return_conflicts = placement_conflicts(
            connection,
            request.timetable_version_id,
            &source.day_of_week,
            source.bell_schedule_period_id,
            &target.reservations(),
            &excluded_block_ids,
        )
        .await?;
        for conflict in &mut return_conflicts {
            conflict.code = conflict
                .code
                .replacen("TIMETABLE_", "TIMETABLE_SWAP_RETURN_", 1);
            conflict.message = format!("คาบที่สลับกลับ: {}", conflict.message);
        }
        conflicts.extend(return_conflicts);
    }
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

struct PlacementReservations {
    group_ids: Vec<Uuid>,
    homeroom_ids: Vec<Uuid>,
    teacher_ids: Vec<Uuid>,
    room_ids: Vec<Uuid>,
}

#[derive(sqlx::FromRow)]
struct PlacementBlock {
    day_of_week: String,
    bell_schedule_period_id: Uuid,
    row_version: i64,
    group_ids: Vec<Uuid>,
    homeroom_ids: Vec<Uuid>,
    teacher_ids: Vec<Uuid>,
    room_ids: Vec<Uuid>,
}

impl PlacementBlock {
    fn reservations(&self) -> PlacementReservations {
        PlacementReservations {
            group_ids: self.group_ids.clone(),
            homeroom_ids: self.homeroom_ids.clone(),
            teacher_ids: self.teacher_ids.clone(),
            room_ids: self.room_ids.clone(),
        }
    }
}

async fn load_placement_block(
    connection: &mut sqlx::PgConnection,
    block_id: Uuid,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
) -> Result<Option<PlacementBlock>, AppError> {
    Ok(DbSession::new(connection)
        .query_as(
            "academic_timetable.services.timetable_block_conflicts.load_placement_block",
            r#"SELECT block.day_of_week, block.bell_schedule_period_id, block.row_version,
            ARRAY(SELECT learning_group_id FROM academic_timetable_block_groups
                  WHERE block_id = block.id AND is_active) AS group_ids,
            ARRAY(SELECT homeroom_id FROM academic_timetable_block_homerooms
                  WHERE block_id = block.id AND is_active
                  UNION SELECT unnest(homeroom_ids) FROM academic_timetable_block_groups
                  WHERE block_id = block.id AND is_active) AS homeroom_ids,
            ARRAY(SELECT teacher_id FROM academic_timetable_block_teachers
                  WHERE block_id = block.id AND is_active
                  UNION SELECT instructor.instructor_id
                  FROM academic_timetable_block_groups target
                  JOIN academic_timetable_block_group_instructors instructor
                    ON instructor.block_group_id = target.id
                  WHERE target.block_id = block.id AND target.is_active) AS teacher_ids,
            ARRAY(SELECT room_id FROM academic_timetable_block_groups
                  WHERE block_id = block.id AND is_active AND room_id IS NOT NULL
                  UNION SELECT room_id FROM academic_timetable_block_homerooms
                  WHERE block_id = block.id AND is_active AND room_id IS NOT NULL) AS room_ids
           FROM academic_timetable_blocks block
           WHERE block.id = $1 AND block.timetable_version_id = $2
             AND block.academic_term_id = $3 AND block.is_active"#,
        )
        .bind(block_id)
        .bind(timetable_version_id)
        .bind(academic_term_id)
        .fetch_optional()
        .await?)
}

async fn candidate_reservations(
    connection: &mut sqlx::PgConnection,
    timetable_version_id: Uuid,
    candidate: &TimetableBlockPlacementCandidate,
) -> Result<PlacementReservations, AppError> {
    let group_ids = candidate.learning_group_id.into_iter().collect::<Vec<_>>();
    let covered_homeroom_ids: Vec<Uuid> = if group_ids.is_empty() {
        candidate.homeroom_ids.clone()
    } else {
        let mut ids: Vec<Uuid> = {
            let source_id: Uuid = DbSession::new(connection)
                .scalar(
                    "academic_timetable.services.timetable_block_conflicts.preview_placement",
                    "SELECT delivery_version_id FROM academic_timetable_versions WHERE id=$1",
                )
                .bind(timetable_version_id)
                .fetch_one()
                .await?;
            let source = school_academic_delivery::services::versions::get_version_on_connection(
                connection, source_id,
            )
            .await?;
            source
                .snapshot
                .offerings
                .iter()
                .flat_map(|offering| offering.groups.iter())
                .filter(|group| group_ids.contains(&group.id))
                .flat_map(|group| group.homeroom_ids.iter().copied())
                .collect()
        };
        ids.extend(candidate.homeroom_ids.iter().copied());
        canonical_ids(&ids)
    };
    let mut teacher_ids = candidate.teacher_ids.clone();
    teacher_ids.extend(candidate.instructor_ids.iter().copied());
    teacher_ids = canonical_ids(&teacher_ids);
    let room_ids = candidate.room_id.into_iter().collect::<Vec<_>>();

    Ok(PlacementReservations {
        group_ids,
        homeroom_ids: covered_homeroom_ids,
        teacher_ids,
        room_ids,
    })
}

async fn placement_conflicts(
    connection: &mut sqlx::PgConnection,
    timetable_version_id: Uuid,
    day: &str,
    period_id: Uuid,
    reservations: &PlacementReservations,
    excluded_block_ids: &[Uuid],
) -> Result<Vec<TimetableBlockConflict>, AppError> {
    let conflicts: Vec<(String, Uuid, Uuid)> = DbSession::new(connection)
        .query_as(
            "academic_timetable.services.timetable_block_conflicts.preview_placement",
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
               CROSS JOIN LATERAL unnest(target.homeroom_ids) coverage(homeroom_id)
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
        .bind(timetable_version_id)
        .bind(day)
        .bind(period_id)
        .bind(&reservations.group_ids)
        .bind(&reservations.homeroom_ids)
        .bind(&reservations.teacher_ids)
        .bind(&reservations.room_ids)
        .bind(excluded_block_ids)
        .fetch_all()
        .await?;
    conflicts
        .into_iter()
        .map(|(kind, block_id, target_id)| conflict_from_row(&kind, block_id, target_id))
        .collect::<Result<Vec<_>, _>>()
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
