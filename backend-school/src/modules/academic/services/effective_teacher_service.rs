use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::delivery::models::{
    AcademicTermChangeActionKind, LearningTeacherRole,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectedTeacherAssignment {
    pub source_episode_id: Option<Uuid>,
    pub learning_group_id: Uuid,
    pub teacher_id: Uuid,
    pub role: LearningTeacherRole,
    pub effective_from: NaiveDate,
    pub source_row_version: Option<i64>,
}

#[derive(Debug, FromRow)]
struct StoredEpisodeRow {
    id: Uuid,
    learning_group_id: Uuid,
    teacher_id: Uuid,
    role: LearningTeacherRole,
    row_version: i64,
}

#[derive(Debug, FromRow)]
struct TeacherChangeRow {
    action_kind: AcademicTermChangeActionKind,
    learning_group_id: Uuid,
    learning_group_teacher_id: Option<Uuid>,
    teacher_id: Uuid,
    teacher_role: Option<LearningTeacherRole>,
}

pub async fn project_effective_assignments_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    requested_group_ids: &[Uuid],
) -> Result<Vec<ProjectedTeacherAssignment>, AppError> {
    let effective_from: NaiveDate =
        sqlx::query_scalar("SELECT effective_from FROM academic_term_change_sets WHERE id = $1")
            .bind(change_set_id)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบชุดการเปลี่ยนแปลงภาคเรียน".to_string()))?;
    let group_ids = requested_group_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if group_ids.is_empty() {
        return Ok(Vec::new());
    }

    let stored: Vec<StoredEpisodeRow> = sqlx::query_as(
        r#"SELECT id, learning_group_id, teacher_id, role, row_version
           FROM learning_group_teachers
           WHERE learning_group_id = ANY($1)
             AND starts_on <= $2
             AND (ends_on IS NULL OR ends_on >= $2)
           ORDER BY learning_group_id, teacher_id, starts_on, id"#,
    )
    .bind(&group_ids)
    .bind(effective_from)
    .fetch_all(&mut **transaction)
    .await?;
    let changes: Vec<TeacherChangeRow> = sqlx::query_as(
        r#"SELECT action_kind, learning_group_id, learning_group_teacher_id,
                  teacher_id, teacher_role
           FROM academic_term_change_items
           WHERE change_set_id = $1
             AND learning_group_id = ANY($2)
             AND action_kind IN (
                 'add_group_teacher',
                 'adjust_group_teacher_role',
                 'stop_group_teacher'
             )
           ORDER BY CASE action_kind
                        WHEN 'stop_group_teacher' THEN 1
                        WHEN 'adjust_group_teacher_role' THEN 2
                        ELSE 3
                    END,
                    learning_group_id, teacher_id, id"#,
    )
    .bind(change_set_id)
    .bind(&group_ids)
    .fetch_all(&mut **transaction)
    .await?;

    let mut projected = stored
        .into_iter()
        .map(|row| {
            (
                (row.learning_group_id, row.teacher_id),
                ProjectedTeacherAssignment {
                    source_episode_id: Some(row.id),
                    learning_group_id: row.learning_group_id,
                    teacher_id: row.teacher_id,
                    role: row.role,
                    effective_from,
                    source_row_version: Some(row.row_version),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for change in changes {
        let key = (change.learning_group_id, change.teacher_id);
        match change.action_kind {
            AcademicTermChangeActionKind::StopGroupTeacher => {
                let removed = projected.remove(&key).ok_or_else(|| {
                    AppError::Conflict("ช่วงการสอนที่จะหยุดไม่ครอบคลุมวันที่เริ่มใช้แล้ว".to_string())
                })?;
                if removed.source_episode_id != change.learning_group_teacher_id {
                    return Err(AppError::Conflict(
                        "ช่วงการสอนที่จะหยุดเปลี่ยนแปลงแล้ว".to_string(),
                    ));
                }
            }
            AcademicTermChangeActionKind::AdjustGroupTeacherRole => {
                let current = projected.get_mut(&key).ok_or_else(|| {
                    AppError::Conflict("ช่วงการสอนที่จะปรับบทบาทไม่ครอบคลุมวันที่เริ่มใช้แล้ว".to_string())
                })?;
                if current.source_episode_id != change.learning_group_teacher_id {
                    return Err(AppError::Conflict(
                        "ช่วงการสอนที่จะปรับบทบาทเปลี่ยนแปลงแล้ว".to_string(),
                    ));
                }
                current.role = change.teacher_role.ok_or_else(|| {
                    AppError::InternalServerError("รายการปรับบทบาทครูไม่มีบทบาทใหม่".to_string())
                })?;
            }
            AcademicTermChangeActionKind::AddGroupTeacher => {
                if projected.contains_key(&key) {
                    return Err(AppError::Conflict(
                        "รายการเพิ่มครูทับซ้อนช่วงการสอนที่มีผลในวันที่เริ่มใช้".to_string(),
                    ));
                }
                projected.insert(
                    key,
                    ProjectedTeacherAssignment {
                        source_episode_id: None,
                        learning_group_id: change.learning_group_id,
                        teacher_id: change.teacher_id,
                        role: change.teacher_role.ok_or_else(|| {
                            AppError::InternalServerError("รายการเพิ่มครูไม่มีบทบาท".to_string())
                        })?,
                        effective_from,
                        source_row_version: None,
                    },
                );
            }
            AcademicTermChangeActionKind::AddOffering
            | AcademicTermChangeActionKind::StopOffering
            | AcademicTermChangeActionKind::AdjustWeeklyPeriodTarget => {
                return Err(AppError::InternalServerError(
                    "พบชนิดรายการที่ไม่ใช่การเปลี่ยนครูใน projection".to_string(),
                ));
            }
        }
    }

    let teacher_ids = projected
        .values()
        .map(|assignment| assignment.teacher_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let active_teacher_ids = if teacher_ids.is_empty() {
        BTreeSet::new()
    } else {
        sqlx::query_scalar::<_, Uuid>(
            r#"SELECT id FROM users
               WHERE id = ANY($1) AND user_type = 'staff' AND status = 'active'
               ORDER BY id"#,
        )
        .bind(&teacher_ids)
        .fetch_all(&mut **transaction)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>()
    };
    projected.retain(|(_, teacher_id), _| active_teacher_ids.contains(teacher_id));

    Ok(projected.into_values().collect())
}

pub fn eligible_teacher_ids_for_group(
    assignments: &[ProjectedTeacherAssignment],
    learning_group_id: Uuid,
) -> BTreeSet<Uuid> {
    assignments
        .iter()
        .filter(|assignment| assignment.learning_group_id == learning_group_id)
        .map(|assignment| assignment.teacher_id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assignment(group_id: Uuid, teacher_id: Uuid) -> ProjectedTeacherAssignment {
        ProjectedTeacherAssignment {
            source_episode_id: None,
            learning_group_id: group_id,
            teacher_id,
            role: LearningTeacherRole::Primary,
            effective_from: NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
            source_row_version: None,
        }
    }

    #[test]
    fn eligible_teacher_ids_are_scoped_to_one_group_and_deduplicated() {
        let selected_group_id = Uuid::new_v4();
        let other_group_id = Uuid::new_v4();
        let selected_teacher_id = Uuid::new_v4();
        let other_teacher_id = Uuid::new_v4();
        let assignments = vec![
            assignment(selected_group_id, selected_teacher_id),
            assignment(selected_group_id, selected_teacher_id),
            assignment(other_group_id, other_teacher_id),
        ];

        assert_eq!(
            eligible_teacher_ids_for_group(&assignments, selected_group_id),
            BTreeSet::from([selected_teacher_id])
        );
    }
}
