use super::super::models::*;
use crate::error::AppError;
use sqlx::{Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
struct TermReference {
    id: Uuid,
    academic_year_id: Uuid,
    sequence: i32,
    status: AcademicTermStatus,
    row_version: i64,
}

/// Read-only Core evidence. The caller supplies either a consistent read
/// transaction or the exclusive tenant transition transaction.
pub(crate) async fn read_activation_state(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<ActivationState, AppError> {
    if year.is_nil() || term.is_nil() {
        return Err(AppError::ValidationError("ระบุปีและภาคเรียนให้ถูกต้อง".into()));
    }
    let context = super::term_transitions::read_context(tx, year, term).await?;
    let years: Vec<YearLifecycleContext> = sqlx::query_as(
        "SELECT id AS academic_year_id,year,name,start_date,end_date,status,row_version FROM academic_years ORDER BY start_date,id LIMIT 501"
    ).fetch_all(&mut **tx).await?;
    let terms: Vec<TermReference> = sqlx::query_as(
        "SELECT id,academic_year_id,sequence_no AS sequence,status,row_version FROM academic_terms WHERE academic_year_id=$1 OR status IN ('active','closing') ORDER BY academic_year_id,sequence_no,id LIMIT 501"
    ).bind(year).fetch_all(&mut **tx).await?;
    if years.len() > 500 || terms.len() > 500 {
        return Err(AppError::ValidationError(
            "จำนวนปีหรือภาคเรียนเกินขอบเขตที่ตรวจพร้อมกันได้".into(),
        ));
    }
    let school_days: String =
        sqlx::query_scalar("SELECT school_days FROM academic_years WHERE id=$1")
            .bind(year)
            .fetch_one(&mut **tx)
            .await?;
    let opens_year = matches!(
        context.year_status,
        AcademicYearStatus::Planning | AcademicYearStatus::Ready
    );
    let predecessor = years
        .iter()
        .rev()
        .find(|row| row.start_date < context.year_start_date)
        .cloned();
    let mut issues = Vec::new();
    let push = |issues: &mut Vec<ActivationIssue>, code, count| {
        if count > 0 {
            issues.push(ActivationIssue { code, count });
        }
    };
    use ActivationIssueCode as Code;
    push(
        &mut issues,
        Code::YearState,
        usize::from(!opens_year && context.year_status != AcademicYearStatus::Active),
    );
    push(
        &mut issues,
        Code::TermState,
        usize::from(context.term_status != AcademicTermStatus::Ready),
    );
    push(
        &mut issues,
        Code::OtherRunningYear,
        years
            .iter()
            .filter(|row| {
                row.academic_year_id != year
                    && matches!(
                        row.status,
                        AcademicYearStatus::Active | AcademicYearStatus::Closing
                    )
            })
            .count(),
    );
    push(
        &mut issues,
        Code::OtherRunningTerm,
        terms
            .iter()
            .filter(|row| {
                row.id != term
                    && matches!(
                        row.status,
                        AcademicTermStatus::Active | AcademicTermStatus::Closing
                    )
            })
            .count(),
    );
    push(
        &mut issues,
        Code::YearOverlap,
        years
            .iter()
            .filter(|row| {
                row.academic_year_id != year
                    && row.start_date <= context.year_end_date
                    && row.end_date >= context.year_start_date
            })
            .count(),
    );
    if opens_year {
        push(
            &mut issues,
            Code::EarlierYearOpen,
            years
                .iter()
                .filter(|row| {
                    row.start_date < context.year_start_date
                        && !matches!(
                            row.status,
                            AcademicYearStatus::Closed | AcademicYearStatus::Archived
                        )
                })
                .count(),
        );
        let first = terms.iter().find(|row| {
            row.academic_year_id == year && row.status != AcademicTermStatus::Cancelled
        });
        push(
            &mut issues,
            Code::NotFirstTerm,
            usize::from(first.is_none_or(|row| row.id != term)),
        );
    } else {
        push(
            &mut issues,
            Code::EarlierTermOpen,
            terms
                .iter()
                .filter(|row| {
                    row.academic_year_id == year
                        && row.sequence < context.sequence
                        && !matches!(
                            row.status,
                            AcademicTermStatus::Closed | AcademicTermStatus::Cancelled
                        )
                })
                .count(),
        );
    }
    let allowed_days: BTreeSet<_> = school_days.split(',').collect();
    let supported_days = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
    let dates_invalid = context.term_start_date < context.year_start_date
        || context.term_start_date > context.year_end_date
        || context
            .planned_end_date
            .is_some_and(|date| date < context.term_start_date || date > context.year_end_date)
        || (context.included_in_year_result && !context.blocks_year_closure)
        || allowed_days.iter().any(|day| !supported_days.contains(day));
    push(
        &mut issues,
        Code::TermConfiguration,
        usize::from(dates_invalid),
    );
    let bell: Option<BellSchedule> = sqlx::query_as("SELECT id,academic_year_id,code,name,is_default,status,owning_organization_unit_id,row_version,created_at,updated_at FROM bell_schedules WHERE id=$1 AND academic_year_id=$2")
        .bind(context.bell_schedule_id).bind(year).fetch_optional(&mut **tx).await?;
    let periods: Vec<BellSchedulePeriod> = sqlx::query_as("SELECT id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days,is_active FROM bell_schedule_periods WHERE bell_schedule_id=$1 ORDER BY order_index,id LIMIT 1001")
        .bind(context.bell_schedule_id).fetch_all(&mut **tx).await?;
    if periods.len() > 1000 {
        return Err(AppError::ValidationError(
            "ตารางเวลามีคาบเกิน 1,000 รายการ".into(),
        ));
    }
    let active_periods: Vec<_> = periods
        .iter()
        .filter(|row| row.is_active)
        .map(|row| BellSchedulePeriodInput {
            name: row.name.clone(),
            start_time: row.start_time,
            end_time: row.end_time,
            order_index: row.order_index,
            applicable_days: row
                .applicable_days
                .as_deref()
                .map(|days| days.split(',').map(str::to_owned).collect())
                .unwrap_or_default(),
            is_active: true,
        })
        .collect();
    let bell_invalid = bell
        .as_ref()
        .is_none_or(|row| row.status == VersionStatus::Archived)
        || active_periods.is_empty()
        || super::bell_schedules::validate_periods(&active_periods).is_err()
        || active_periods
            .iter()
            .flat_map(|row| &row.applicable_days)
            .any(|day| !allowed_days.contains(day.as_str()));
    push(&mut issues, Code::BellSchedule, usize::from(bell_invalid));
    let (students, placements) = read_enrollment(tx, &context).await?;
    push(
        &mut issues,
        Code::StudentReference,
        students
            .iter()
            .filter(|row| {
                matches!(
                    row.status,
                    StudentAcademicYearStatus::Planned | StudentAcademicYearStatus::Active
                ) && !row.reference_valid
            })
            .count(),
    );
    push(
        &mut issues,
        Code::PlacementReference,
        placements.iter().filter(|row| !row.reference_valid).count(),
    );
    if opens_year {
        push(
            &mut issues,
            Code::ExistingActiveEnrollment,
            students
                .iter()
                .filter(|row| row.status == StudentAcademicYearStatus::Active)
                .count()
                + placements
                    .iter()
                    .filter(|row| row.status == HomeroomPlacementStatus::Current)
                    .count(),
        );
    }
    let mut rooms: BTreeMap<Uuid, (Option<i32>, BTreeSet<Uuid>)> = BTreeMap::new();
    let mut placement_counts: BTreeMap<Uuid, usize> = BTreeMap::new();
    for row in placements.iter().filter(|row| {
        row.start_date <= context.term_start_date
            && row
                .end_date
                .is_none_or(|date| date >= context.term_start_date)
    }) {
        let room = rooms
            .entry(row.homeroom_id)
            .or_insert((row.capacity, BTreeSet::new()));
        room.1.insert(row.student_academic_year_id);
        *placement_counts
            .entry(row.student_academic_year_id)
            .or_default() += 1;
    }
    let duplicates = placement_counts
        .values()
        .filter(|count| **count > 1)
        .count();
    if duplicates > 0 {
        if let Some(issue) = issues
            .iter_mut()
            .find(|row| row.code == Code::PlacementReference)
        {
            issue.count += duplicates;
        } else {
            push(&mut issues, Code::PlacementReference, duplicates);
        }
    }
    push(
        &mut issues,
        Code::RoomCapacity,
        rooms
            .values()
            .filter(|(capacity, occupants)| {
                capacity.is_some_and(|capacity| occupants.len() as i64 > i64::from(capacity))
            })
            .count(),
    );
    let source_checksum = crate::modules::academic::lifecycle::services::checksum(&(
        &context,
        &years,
        &terms,
        &school_days,
        &bell,
        &periods,
        &students,
        &placements,
    ))?;
    Ok(ActivationState {
        context,
        opens_year,
        predecessor,
        students,
        placements,
        issues,
        source_checksum,
    })
}

async fn read_enrollment(
    tx: &mut Transaction<'_, Postgres>,
    context: &TermLifecycleContext,
) -> Result<(Vec<ActivationStudent>, Vec<ActivationPlacement>), AppError> {
    let students: Vec<ActivationStudent> = sqlx::query_as(
        "SELECT student.id,student.student_id,student.grade_level_id,student.study_program_id,student.status,student.row_version,
         COALESCE(person.user_type='student' AND person.status='active' AND grade.is_active IS TRUE
          AND program.status='published' AND version.status='published' AND curriculum.is_active IS TRUE
          AND curriculum.grade_level_ids @> jsonb_build_array(student.grade_level_id::text)
          AND starts.start_date<=target.start_date AND (ends.end_date IS NULL OR ends.end_date>=target.end_date),false) AS reference_valid
         FROM student_academic_years student JOIN academic_years target ON target.id=student.academic_year_id
         JOIN users person ON person.id=student.student_id LEFT JOIN grade_levels grade ON grade.id=student.grade_level_id
         LEFT JOIN study_programs program ON program.id=student.study_program_id
         LEFT JOIN curriculum_versions version ON version.id=program.curriculum_version_id
         LEFT JOIN curricula curriculum ON curriculum.id=version.curriculum_id
         LEFT JOIN academic_years starts ON starts.id=version.start_academic_year_id
         LEFT JOIN academic_years ends ON ends.id=version.end_academic_year_id
         WHERE student.academic_year_id=$1 ORDER BY student.id LIMIT 10001"
    ).bind(context.academic_year_id).fetch_all(&mut **tx).await?;
    let placements: Vec<ActivationPlacement> = sqlx::query_as(
        "SELECT p.id,p.student_academic_year_id,p.homeroom_id,p.start_date,p.end_date,p.status,p.row_version,
         room.row_version AS room_row_version,room.capacity,
         COALESCE(room.academic_year_id=p.academic_year_id AND room.grade_level_id=student.grade_level_id
          AND room.study_program_id=student.study_program_id AND room.is_active IS TRUE
          AND student.status IN ('planned','active') AND p.start_date>=$3 AND p.start_date<=$4
          AND (p.end_date IS NULL OR (p.end_date>=p.start_date AND p.end_date<=$4)),false) AS reference_valid,
         (p.status='planned' AND student.status='planned' AND p.start_date<=$2 AND (p.end_date IS NULL OR p.end_date>=$2)) AS eligible
         FROM homeroom_placements p JOIN student_academic_years student ON student.id=p.student_academic_year_id AND student.academic_year_id=p.academic_year_id
         LEFT JOIN homerooms room ON room.id=p.homeroom_id
         WHERE p.academic_year_id=$1 AND p.status IN ('planned','current') ORDER BY p.id LIMIT 20001"
    ).bind(context.academic_year_id).bind(context.term_start_date).bind(context.year_start_date).bind(context.year_end_date).fetch_all(&mut **tx).await?;
    if students.len() > 10_000 || placements.len() > 20_000 {
        return Err(AppError::ValidationError(
            "ข้อมูลเปิดปีเกิน 10,000 คนหรือ 20,000 รายการจัดห้อง กรุณาติดต่อผู้ดูแลระบบ".into(),
        ));
    }
    Ok((students, placements))
}
