use std::collections::{HashMap, HashSet};

use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{groups, offerings::LearningOfferingSignalDescriptor, roster_tracking, versions};
use crate::models::{versions::DeliveryVersionStatus, *};

/// Internal command result; the HTTP envelope serializes only `response`.
/// Signal evidence is captured under the same offering lock as the accepted setup.
pub struct BulkGroupSetupOutcome {
    pub response: BulkGroupSetupResponse,
    pub signal: LearningOfferingSignalDescriptor,
}

fn validate_request(request: &BulkGroupSetupRequest) -> Result<(), AppError> {
    super::validate_row_version(request.delivery_version_row_version)?;
    if request.groups.is_empty() || request.groups.len() > 500 {
        return Err(AppError::ValidationError(
            "เลือกกลุ่มเรียนตั้งแต่ 1 ถึง 500 กลุ่ม".into(),
        ));
    }
    if request.primary_teacher.is_none() && request.roster_tracking.is_none() {
        return Err(AppError::ValidationError(
            "เลือกการตั้งค่าที่ต้องการเปลี่ยนอย่างน้อยหนึ่งอย่าง".into(),
        ));
    }
    let mut ids = HashSet::new();
    for target in &request.groups {
        super::validate_row_version(target.row_version)?;
        if !ids.insert(target.learning_group_id) {
            return Err(AppError::ValidationError("กลุ่มเรียนที่เลือกซ้ำกัน".into()));
        }
    }
    Ok(())
}

/// Read-only scope preview. Application uses the same provider decisions again under write locks.
pub async fn preview(
    pool: &PgPool,
    offering_id: Uuid,
    request: BulkGroupSetupRequest,
) -> Result<BulkGroupSetupPreview, AppError> {
    validate_request(&request)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.bulk_group_setup.preview",
    )
    .await?;
    DbSession::new(&mut *tx)
        .query(
            "academic_delivery.services.bulk_group_setup.preview",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY",
        )
        .execute()
        .await?;
    let result = build_preview(&mut tx, offering_id, &request).await?;
    tx.commit().await?;
    Ok(result)
}

async fn build_preview(
    connection: &mut PgConnection,
    offering_id: Uuid,
    request: &BulkGroupSetupRequest,
) -> Result<BulkGroupSetupPreview, AppError> {
    let version =
        versions::get_version_on_connection(connection, request.delivery_version_id).await?;
    if version.status != DeliveryVersionStatus::Draft
        || version.row_version != request.delivery_version_row_version
    {
        return Err(AppError::Conflict(
            "รุ่นเปิดสอนเปลี่ยนแล้วหรือไม่ใช่แบบร่าง กรุณาโหลดใหม่".into(),
        ));
    }
    let offering = version
        .snapshot
        .offerings
        .iter()
        .find(|offering| offering.id == offering_id)
        .ok_or_else(|| AppError::ValidationError("รายการเปิดสอนไม่อยู่ในรุ่นที่เลือก".into()))?;
    let writable: bool = DbSession::new(connection).scalar("academic_delivery.services.bulk_group_setup.build_preview", "SELECT y.status NOT IN ('closed','archived') AND t.status NOT IN ('closed','cancelled') FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1 AND y.id=$2")
        .bind(version.academic_term_id).bind(version.academic_year_id).fetch_one().await?;
    if !writable {
        return Err(AppError::Conflict("ปีหรือภาคเรียนนี้ปิดแล้วหรือยกเลิกแล้ว".into()));
    }
    if let Some(teacher) = &request.primary_teacher {
        let active: bool = DbSession::new(connection).scalar("academic_delivery.services.bulk_group_setup.build_preview", "SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND user_type='staff' AND status='active')")
            .bind(teacher.teacher_id).fetch_one().await?;
        if !active {
            return Err(AppError::ValidationError(
                "ครูผู้สอนต้องเป็นบุคลากรที่ใช้งานอยู่".into(),
            ));
        }
    }
    let selected: HashMap<_, _> = request
        .groups
        .iter()
        .map(|target| (target.learning_group_id, target.row_version))
        .collect();
    if selected
        .keys()
        .any(|id| !offering.groups.iter().any(|group| group.id == *id))
    {
        return Err(AppError::ValidationError(
            "กลุ่มเรียนไม่อยู่ในรายการเปิดสอนและรุ่นที่เลือก".into(),
        ));
    }
    let ids: Vec<_> = selected.keys().copied().collect();
    let loaded = groups::get_many_on_connection(connection, &ids).await?;
    if loaded.len() != ids.len() {
        return Err(AppError::NotFound("ไม่พบกลุ่มเรียนบางกลุ่ม".into()));
    }
    let mut tracking_by_group = roster_tracking::get_many_on_connection(connection, &ids).await?;
    let mut tracking_conflicts = if let Some(input) = &request.roster_tracking {
        roster_tracking::configuration_conflicts(
            connection,
            offering_id,
            &tracking_by_group,
            input.mode,
            input.effective_from,
        )
        .await?
    } else {
        HashMap::new()
    };
    let mut targets = Vec::with_capacity(request.groups.len());
    for group in loaded {
        if group.learning_offering_id != offering_id
            || group.academic_term_id != version.academic_term_id
            || group.academic_year_id != version.academic_year_id
        {
            return Err(AppError::ValidationError(
                "กลุ่มเรียนไม่อยู่ในรายการเปิดสอนและภาคเรียนที่เลือก".into(),
            ));
        }
        if selected.get(&group.id) != Some(&group.row_version) {
            return Err(AppError::Conflict("กลุ่มเรียนถูกแก้ไขแล้ว กรุณาโหลดใหม่".into()));
        }
        let tracking = tracking_by_group
            .remove(&group.id)
            .ok_or_else(|| AppError::NotFound("ไม่พบการตั้งค่ารายชื่อของกลุ่มเรียน".into()))?;
        let mut blocked_reason = if group.teachers_locked
            || group.status != LearningOfferingStatus::Draft
            || group.roster_status != RosterStatus::Draft
        {
            Some("ตั้งค่าเป็นชุดได้เฉพาะกลุ่มและรายชื่อแบบร่าง".into())
        } else {
            None
        };
        let primary_teacher_action =
            teacher_action(&group.teacher_assignments, request.primary_teacher.as_ref());
        if let Some(teacher) = &request.primary_teacher {
            if matches!(
                primary_teacher_action,
                BulkGroupSetupTeacherAction::Set | BulkGroupSetupTeacherAction::Replace
            ) && group.teacher_assignments.iter().any(|assignment| {
                assignment.teacher_id == teacher.teacher_id
                    && assignment.role != LearningTeacherRole::Primary
            }) {
                blocked_reason = Some("ครูที่เลือกเป็นครูร่วมสอนหรือผู้ช่วยอยู่แล้ว กรุณาปรับเฉพาะกลุ่ม".into());
            }
        }
        let tracking_changed = request.roster_tracking.as_ref().is_some_and(|input| {
            tracking.mode != input.mode || tracking.effective_from != input.effective_from
        });
        match tracking_conflicts.remove(&group.id) {
            Some(AppError::ValidationError(reason) | AppError::Conflict(reason)) => {
                blocked_reason = Some(reason);
            }
            Some(error) => return Err(error),
            None => {}
        }
        targets.push(BulkGroupSetupPreviewTarget {
            group,
            roster_tracking: tracking,
            primary_teacher_action,
            roster_tracking_changed: tracking_changed,
            blocked_reason,
        });
    }
    let blocked_count = targets
        .iter()
        .filter(|target| target.blocked_reason.is_some())
        .count();
    let changed_count = targets
        .iter()
        .filter(|target| target.blocked_reason.is_none() && changes(target))
        .count();
    let skipped_count = targets.len() - blocked_count - changed_count;
    Ok(BulkGroupSetupPreview {
        delivery_version_row_version: version.row_version,
        targets,
        changed_count,
        skipped_count,
        blocked_count,
    })
}

fn teacher_action(
    assignments: &[LearningGroupTeacherAssignment],
    input: Option<&BulkGroupSetupPrimaryTeacher>,
) -> BulkGroupSetupTeacherAction {
    let Some(input) = input else {
        return BulkGroupSetupTeacherAction::Unchanged;
    };
    let primaries: Vec<_> = assignments
        .iter()
        .filter(|assignment| assignment.role == LearningTeacherRole::Primary)
        .collect();
    if primaries.len() == 1 && primaries[0].teacher_id == input.teacher_id {
        BulkGroupSetupTeacherAction::Unchanged
    } else if primaries.is_empty() {
        BulkGroupSetupTeacherAction::Set
    } else if input.replace_existing_primary {
        BulkGroupSetupTeacherAction::Replace
    } else {
        BulkGroupSetupTeacherAction::PreserveExisting
    }
}

fn changes(target: &BulkGroupSetupPreviewTarget) -> bool {
    matches!(
        target.primary_teacher_action,
        BulkGroupSetupTeacherAction::Set | BulkGroupSetupTeacherAction::Replace
    ) || target.roster_tracking_changed
}

pub async fn apply(
    pool: &PgPool,
    actor: Uuid,
    offering_id: Uuid,
    request: BulkGroupSetupRequest,
) -> Result<BulkGroupSetupOutcome, AppError> {
    validate_request(&request)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.bulk_group_setup.apply",
    )
    .await?;
    // Initial exclusive term mode avoids lock upgrades and serializes publication and draft refresh.
    let term_id: Uuid = DbSession::new(&mut *tx)
        .scalar(
            "academic_delivery.services.bulk_group_setup.apply",
            "SELECT academic_term_id FROM learning_offerings WHERE id=$1",
        )
        .bind(offering_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการเปิดสอน".into()))?;
    super::require_writable_term(&mut tx, term_id, true).await?;
    let (starts_on, offering_term_id, offering_row_version): (chrono::NaiveDate, Uuid, i64) = DbSession::new(&mut *tx).query_as("academic_delivery.services.bulk_group_setup.apply", "SELECT starts_on,academic_term_id,row_version FROM learning_offerings WHERE id=$1 AND academic_term_id=$2 AND status IN ('draft','published') FOR UPDATE")
        .bind(offering_id).bind(term_id).fetch_optional().await?.ok_or_else(|| AppError::Conflict("รายการเปิดสอนไม่พร้อมแก้ไข".into()))?;
    // Verify scope before acquiring any requested group/version locks, then recheck revisions under those locks.
    let scope_version =
        versions::get_version_on_connection(&mut tx, request.delivery_version_id).await?;
    let scope_offering = scope_version
        .snapshot
        .offerings
        .iter()
        .find(|offering| offering.id == offering_id);
    if scope_version.academic_term_id != term_id
        || scope_offering.is_none()
        || request.groups.iter().any(|target| {
            !scope_offering.is_some_and(|offering| {
                offering
                    .groups
                    .iter()
                    .any(|group| group.id == target.learning_group_id)
            })
        })
    {
        return Err(AppError::ValidationError(
            "กลุ่มเรียนและรุ่นต้องอยู่ในรายการเปิดสอนและภาคเรียนที่เลือก".into(),
        ));
    }
    let ids: Vec<_> = request
        .groups
        .iter()
        .map(|target| target.learning_group_id)
        .collect();
    let locked: Vec<Uuid> = DbSession::new(&mut *tx).scalar("academic_delivery.services.bulk_group_setup.apply", "SELECT id FROM learning_groups WHERE id=ANY($1) AND learning_offering_id=$2 AND academic_term_id=$3 ORDER BY id FOR UPDATE")
        .bind(&ids).bind(offering_id).bind(term_id).fetch_all().await?;
    if locked.len() != ids.len() {
        return Err(AppError::ValidationError(
            "กลุ่มเรียนบางกลุ่มไม่อยู่ในรายการเปิดสอนที่เลือก".into(),
        ));
    }
    DbSession::new(&mut *tx).query("academic_delivery.services.bulk_group_setup.apply", "SELECT id FROM academic_delivery_versions WHERE id=$1 AND academic_term_id=$2 AND EXISTS(SELECT 1 FROM jsonb_array_elements(snapshot->'offerings') offering WHERE (offering->>'id')::uuid=$3) FOR UPDATE")
        .bind(request.delivery_version_id).bind(term_id).bind(offering_id).execute().await?;
    let preview = build_preview(&mut tx, offering_id, &request).await?;
    if preview.blocked_count > 0 {
        return Err(AppError::Conflict(
            preview
                .targets
                .iter()
                .find_map(|target| target.blocked_reason.clone())
                .unwrap_or_else(|| "ตรวจการตั้งค่ากลุ่มเรียนก่อนบันทึก".into()),
        ));
    }
    if preview.changed_count == 0 {
        return Err(AppError::Conflict("ไม่มีกลุ่มเรียนที่ต้องเปลี่ยนการตั้งค่า".into()));
    }
    let changed_ids: Vec<_> = preview
        .targets
        .iter()
        .filter(|target| changes(target))
        .map(|target| target.group.id)
        .collect();
    let teacher_changed: Vec<_> = preview
        .targets
        .iter()
        .filter(|target| {
            matches!(
                target.primary_teacher_action,
                BulkGroupSetupTeacherAction::Set | BulkGroupSetupTeacherAction::Replace
            )
        })
        .map(|target| target.group.id)
        .collect();
    let tracking_changed: Vec<_> = preview
        .targets
        .iter()
        .filter(|target| target.roster_tracking_changed)
        .map(|target| target.group.id)
        .collect();
    if let Some(teacher) = &request.primary_teacher {
        if !teacher_changed.is_empty() {
            // Only draft primary assignments are replaced; other roles retain episode identities.
            DbSession::new(&mut *tx).query("academic_delivery.services.bulk_group_setup.apply", "DELETE FROM learning_group_teachers WHERE learning_group_id=ANY($1) AND role='primary'")
                .bind(&teacher_changed).execute().await?;
            DbSession::new(&mut *tx).query("academic_delivery.services.bulk_group_setup.apply", "INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on,created_by,updated_by) SELECT uuidv7(),g.id,g.academic_term_id,g.academic_year_id,$2,'primary',$3,$4,$4 FROM learning_groups g WHERE g.id=ANY($1)")
                .bind(&teacher_changed).bind(teacher.teacher_id).bind(starts_on).bind(actor).execute().await?;
            super::invalidate_group_academic_confirmations(&mut tx, &teacher_changed).await?;
        }
    }
    if let Some(input) = &request.roster_tracking {
        if !tracking_changed.is_empty() {
            roster_tracking::save_configurations(
                &mut tx,
                &tracking_changed,
                input.mode,
                input.effective_from,
            )
            .await?;
        }
    }
    DbSession::new(&mut *tx).query("academic_delivery.services.bulk_group_setup.apply", "UPDATE learning_groups SET row_version=row_version+1,updated_at=now() WHERE id=ANY($1)")
        .bind(&changed_ids).execute().await?;
    // One bounded audit record captures the explicit atomic scope in the same transaction.
    super::append_audit(&mut tx, "learning_group.bulk_setup", "learning_offering", offering_id, scope_version.academic_year_id, term_id, actor,
        serde_json::json!({"deliveryVersionId":request.delivery_version_id,"selectedGroupIds":ids,"changedGroupIds":changed_ids,"primaryTeacherGroupIds":teacher_changed,"rosterTrackingGroupIds":tracking_changed})).await?;
    if !teacher_changed.is_empty() {
        versions::refresh_drafts_for_offering(&mut tx, offering_id).await?;
    }
    let delivery_version_row_version =
        versions::get_version_on_connection(&mut tx, request.delivery_version_id)
            .await?
            .row_version;
    let fresh_groups = groups::get_many_on_connection(&mut tx, &ids).await?;
    let mut tracking_by_group = roster_tracking::get_many_on_connection(&mut tx, &ids).await?;
    let mut tracking = Vec::with_capacity(ids.len());
    for group in &fresh_groups {
        tracking.push(BulkGroupSetupRosterTracking {
            learning_group_id: group.id,
            tracking: tracking_by_group
                .remove(&group.id)
                .ok_or_else(|| AppError::NotFound("ไม่พบการตั้งค่ารายชื่อของกลุ่มเรียน".into()))?,
        });
    }
    let outcome = BulkGroupSetupOutcome {
        response: BulkGroupSetupResponse {
            delivery_version_row_version,
            groups: fresh_groups,
            roster_tracking: tracking,
        },
        signal: LearningOfferingSignalDescriptor {
            learning_offering_id: offering_id,
            academic_term_id: offering_term_id,
            row_version: offering_row_version,
        },
    };
    tx.commit().await?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> BulkGroupSetupRequest {
        BulkGroupSetupRequest {
            delivery_version_id: Uuid::now_v7(),
            delivery_version_row_version: 1,
            groups: vec![BulkGroupSetupTarget {
                learning_group_id: Uuid::now_v7(),
                row_version: 1,
            }],
            primary_teacher: Some(BulkGroupSetupPrimaryTeacher {
                teacher_id: Uuid::now_v7(),
                replace_existing_primary: false,
            }),
            roster_tracking: None,
        }
    }

    #[test]
    fn request_validation_refuses_empty_missing_operation_stale_shapes_and_duplicates() {
        for case in 0..7 {
            let mut input = request();
            match case {
                0 => input.groups.clear(),
                1 => input.primary_teacher = None,
                2 => input.delivery_version_row_version = 0,
                3 => input.delivery_version_row_version = -1,
                4 => input.groups[0].row_version = 0,
                5 => input.groups[0].row_version = -1,
                _ => input.groups.push(input.groups[0].clone()),
            }
            assert!(
                matches!(validate_request(&input), Err(AppError::ValidationError(_))),
                "invalid request case {case}"
            );
        }
    }

    #[test]
    fn request_validation_accepts_500_targets_and_rejects_501() {
        let mut input = request();
        input.groups = (1..=500)
            .map(|id| BulkGroupSetupTarget {
                learning_group_id: Uuid::from_u128(id),
                row_version: 1,
            })
            .collect();
        assert!(validate_request(&input).is_ok());
        input.groups.push(BulkGroupSetupTarget {
            learning_group_id: Uuid::from_u128(501),
            row_version: 1,
        });
        assert!(matches!(
            validate_request(&input),
            Err(AppError::ValidationError(_))
        ));
    }

    fn assignment(teacher_id: Uuid, role: LearningTeacherRole) -> LearningGroupTeacherAssignment {
        LearningGroupTeacherAssignment {
            id: Uuid::now_v7(),
            teacher_id,
            display_name: "Synthetic teacher".into(),
            role,
            starts_on: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            ends_on: None,
            row_version: 1,
        }
    }

    #[test]
    fn teacher_actions_fill_missing_preserve_defaults_and_replace_explicitly() {
        let selected = Uuid::now_v7();
        let other = Uuid::now_v7();
        let mut input = BulkGroupSetupPrimaryTeacher {
            teacher_id: selected,
            replace_existing_primary: false,
        };
        let secondary_only = vec![assignment(other, LearningTeacherRole::Secondary)];
        assert_eq!(
            teacher_action(&secondary_only, None),
            BulkGroupSetupTeacherAction::Unchanged
        );
        assert_eq!(
            teacher_action(&secondary_only, Some(&input)),
            BulkGroupSetupTeacherAction::Set
        );
        let existing = vec![
            assignment(other, LearningTeacherRole::Primary),
            assignment(selected, LearningTeacherRole::Assistant),
        ];
        assert_eq!(
            teacher_action(&existing, Some(&input)),
            BulkGroupSetupTeacherAction::PreserveExisting
        );
        let same = vec![assignment(selected, LearningTeacherRole::Primary)];
        assert_eq!(
            teacher_action(&same, Some(&input)),
            BulkGroupSetupTeacherAction::Unchanged
        );
        input.replace_existing_primary = true;
        assert_eq!(
            teacher_action(&existing, Some(&input)),
            BulkGroupSetupTeacherAction::Replace
        );
    }
}
