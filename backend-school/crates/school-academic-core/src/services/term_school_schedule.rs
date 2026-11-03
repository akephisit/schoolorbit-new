use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{bell_schedules, lifecycle_guard, parse_row_version, school_days, years_terms};
use crate::models::*;

enum Target {
    New(CreateAcademicTermCopySchoolScheduleRequest),
    Existing(Uuid, CopyAcademicTermSchoolScheduleRequest),
}

pub async fn create(
    pool: &PgPool,
    actor: Uuid,
    request: CreateAcademicTermCopySchoolScheduleRequest,
) -> Result<AcademicTermSchoolScheduleCopyResponse, AppError> {
    copy(pool, actor, Target::New(request)).await
}

pub async fn copy_into(
    pool: &PgPool,
    actor: Uuid,
    term: Uuid,
    request: CopyAcademicTermSchoolScheduleRequest,
) -> Result<AcademicTermSchoolScheduleCopyResponse, AppError> {
    copy(pool, actor, Target::Existing(term, request)).await
}

async fn copy(
    pool: &PgPool,
    actor: Uuid,
    target: Target,
) -> Result<AcademicTermSchoolScheduleCopyResponse, AppError> {
    let (source_id, source_version, schedule_version, requested_days) = match &target {
        Target::New(request) => (
            request.source_term_id,
            request.source_term_row_version,
            request.source_bell_schedule_row_version,
            &request.school_days,
        ),
        Target::Existing(_, request) => (
            request.source_term_id,
            request.source_term_row_version,
            request.source_bell_schedule_row_version,
            &request.school_days,
        ),
    };
    parse_row_version(source_version)?;
    parse_row_version(schedule_version)?;
    let target_days = school_days::normalize(requested_days)?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "academic_core.term_school_schedule.copy")
            .await?;
    lifecycle_guard::lock_transition_shared(&mut tx).await?;
    let source_year = term_year(&mut tx, source_id).await?;
    let target_year = match &target {
        Target::New(request) => request.academic_year_id,
        Target::Existing(id, request) => {
            parse_row_version(request.row_version)?;
            if *id == source_id {
                return Err(AppError::ValidationError(
                    "ภาคเรียนต้นฉบับและปลายทางต้องเป็นคนละภาคเรียน".into(),
                ));
            }
            term_year(&mut tx, *id).await?
        }
    };
    // Both parent rows are acquired before any term or schedule lock, using the
    // same order as standalone schedule cloning and lifecycle configuration.
    let _: Vec<Uuid> = DbSession::new(&mut *tx)
        .scalar(
            "academic_core.term_school_schedule.parents",
            "SELECT id FROM academic_years WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(vec![source_year, target_year])
        .fetch_all()
        .await?;
    let mut term_ids = vec![source_id];
    if let Target::Existing(id, _) = &target {
        term_ids.push(*id);
    }
    let _: Vec<Uuid> = DbSession::new(&mut *tx)
        .scalar(
            "academic_core.term_school_schedule.terms",
            "SELECT id FROM academic_terms WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(term_ids)
        .fetch_all()
        .await?;
    let source = years_terms::get_term_on_connection(&mut tx, source_id).await?;
    if source.row_version != source_version {
        return Err(AppError::Conflict(
            "ภาคเรียนต้นฉบับเปลี่ยนแล้ว กรุณาโหลดใหม่ก่อนทำสำเนา".into(),
        ));
    }
    let existing = if let Target::Existing(id, request) = &target {
        let current = years_terms::get_term_on_connection(&mut tx, *id).await?;
        if current.row_version != request.row_version {
            return Err(AppError::Conflict(
                "ภาคเรียนปลายทางเปลี่ยนแล้ว กรุณาโหลดใหม่".into(),
            ));
        }
        if !bell_schedules::term_schedule_is_assignable(&mut tx, *id).await? {
            return Err(AppError::Conflict(
                "ทำสำเนาตารางคาบได้เฉพาะภาคเรียนที่ยังไม่เคยเปิดและไม่มีรุ่นตารางสอนหรือคาบอ้างอิง".into(),
            ));
        }
        Some(current)
    } else {
        None
    };
    // Resolve and validate the exact source pair before creating any new rows.
    let copy_source = bell_schedules::capture_copy_source(
        &mut tx,
        source.bell_schedule_id,
        source.academic_year_id,
        schedule_version,
    )
    .await?;
    copy_period_inputs(copy_source.periods(), &target_days)?;
    let target_name = match &target {
        Target::New(request) => {
            let sequence: i32 = DbSession::new(&mut *tx).scalar("academic_core.term_school_schedule.sequence",
                "SELECT COALESCE(max(sequence_no),0)+1 FROM academic_terms WHERE academic_year_id=$1")
                .bind(target_year).fetch_one().await?;
            years_terms::derive_term_identity(
                request.term_type,
                sequence,
                request.custom_name.as_deref(),
            )?
            .name
        }
        Target::Existing(_, _) => {
            existing
                .as_ref()
                .map(|term| term.name.clone())
                .ok_or_else(|| {
                    AppError::InternalServerError("ไม่พบข้อมูลภาคเรียนปลายทางที่ตรวจแล้ว".into())
                })?
        }
    };
    if matches!(&target, Target::New(_)) {
        let status = lifecycle_guard::require_year_write_exclusive(&mut tx, target_year).await?;
        if !matches!(
            status,
            AcademicYearStatus::Planning | AcademicYearStatus::Active
        ) {
            return Err(AppError::Conflict(
                "สร้างภาคเรียนได้เฉพาะปีที่กำลังวางแผนหรือกำลังใช้งาน".into(),
            ));
        }
    } else {
        bell_schedules::require_setup_year(&mut tx, target_year).await?;
    }
    let mut schedule = bell_schedules::clone_captured_schedule(
        &mut tx,
        actor,
        &copy_source,
        CloneBellScheduleRequest {
            academic_year_id: target_year,
            name: format!("ตารางคาบ {target_name}"),
            owning_organization_unit_id: None,
            row_version: schedule_version,
        },
    )
    .await?;
    let term = match target {
        Target::New(request) => {
            years_terms::create_term_in_transaction(
                &mut tx,
                actor,
                CreateAcademicTermRequest {
                    academic_year_id: request.academic_year_id,
                    term_type: request.term_type,
                    custom_name: request.custom_name,
                    included_in_year_result: request.included_in_year_result,
                    blocks_year_closure: request.blocks_year_closure,
                    bell_schedule_id: schedule.id,
                    school_days: target_days,
                },
            )
            .await?
        }
        Target::Existing(id, request) => {
            let current = existing.ok_or_else(|| {
                AppError::InternalServerError("ไม่พบข้อมูลภาคเรียนปลายทางที่ตรวจแล้ว".into())
            })?;
            years_terms::update_term_in_transaction(
                &mut tx,
                actor,
                id,
                UpdateAcademicTermRequest {
                    term_type: current.term_type,
                    custom_name: Some(current.name),
                    included_in_year_result: current.included_in_year_result,
                    blocks_year_closure: current.blocks_year_closure,
                    bell_schedule_id: schedule.id,
                    school_days: target_days,
                    row_version: request.row_version,
                },
            )
            .await?
        }
    };
    years_terms::append_audit(&mut tx, "academic_term.school_schedule_copied", "academic_term", term.id,
        Some(target_year), Some(term.id), actor,
        serde_json::json!({"sourceTermId":source.id,"sourceTermRowVersion":source_version,"sourceBellScheduleId":source.bell_schedule_id,"sourceBellScheduleRowVersion":schedule_version,"targetBellScheduleId":schedule.id})).await?;
    // Include the new assignment when computing exact returned capabilities.
    bell_schedules::attach_editability(&mut tx, std::slice::from_mut(&mut schedule)).await?;
    tx.commit().await?;
    Ok(AcademicTermSchoolScheduleCopyResponse {
        term,
        bell_schedule: schedule,
        can_edit_school_days: true,
        can_assign_bell_schedule: true,
    })
}

async fn term_year(tx: &mut Transaction<'_, Postgres>, term: Uuid) -> Result<Uuid, AppError> {
    DbSession::new(&mut **tx)
        .scalar(
            "academic_core.term_school_schedule.term_year",
            "SELECT academic_year_id FROM academic_terms WHERE id=$1",
        )
        .bind(term)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))
}

fn copy_period_inputs(
    periods: &[BellSchedulePeriod],
    target_days: &[String],
) -> Result<Vec<BellSchedulePeriodInput>, AppError> {
    if periods.len() > 1_000 || !periods.iter().any(|period| period.is_active) {
        return Err(AppError::ValidationError(
            "ตารางคาบต้นฉบับต้องมีคาบที่เปิดใช้งานอย่างน้อยหนึ่งคาบและมีคาบรวมไม่เกิน 1,000 คาบ".into(),
        ));
    }
    let inputs = periods
        .iter()
        .map(|period| {
            Ok(BellSchedulePeriodInput {
                name: period.name.clone(),
                start_time: period.start_time,
                end_time: period.end_time,
                order_index: period.order_index,
                applicable_days: Some(school_days::effective_period_days(
                    period.applicable_days.as_deref(),
                    target_days,
                )?),
                is_active: period.is_active,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    bell_schedules::validate_periods(&inputs)?;
    Ok(inputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn period(days: Option<&str>) -> BellSchedulePeriod {
        BellSchedulePeriod {
            id: Uuid::now_v7(),
            bell_schedule_id: Uuid::now_v7(),
            name: Some("Preserved name".into()),
            start_time: "08:00:00".parse().unwrap(),
            end_time: "09:00:00".parse().unwrap(),
            order_index: 1,
            applicable_days: days.map(str::to_owned),
            is_active: true,
        }
    }

    #[test]
    fn term_school_copy_validation_resolves_wildcard_without_rewriting_source_properties() {
        let source = period(None);
        let target = vec!["MON".into(), "SAT".into()];
        let inputs = copy_period_inputs(std::slice::from_ref(&source), &target).unwrap();
        let copied = &inputs[0];
        assert_eq!(copied.applicable_days, Some(target));
        assert_eq!(copied.name, source.name);
        assert_eq!(
            (
                copied.start_time,
                copied.end_time,
                copied.order_index,
                copied.is_active
            ),
            (
                source.start_time,
                source.end_time,
                source.order_index,
                source.is_active
            )
        );
        assert_eq!(source.applicable_days, None);
        let mut overlapping = period(Some("MON"));
        overlapping.order_index = 2;
        assert!(
            copy_period_inputs(&[source, overlapping], &["MON".into()]).is_err(),
            "NULL overlaps an explicit period on selected term school days"
        );
    }

    #[test]
    fn term_school_copy_validation_refuses_unusable_or_incompatible_sources() {
        let target = vec!["MON".into()];
        assert!(copy_period_inputs(&[], &target).is_err());
        let mut inactive = period(None);
        inactive.is_active = false;
        assert!(copy_period_inputs(&[inactive], &target).is_err());
        for invalid in ["SAT", "", "{MON,TUE}"] {
            assert!(copy_period_inputs(&[period(Some(invalid))], &target).is_err());
        }
        assert!(copy_period_inputs(&vec![period(None); 1001], &target).is_err());
    }
}
