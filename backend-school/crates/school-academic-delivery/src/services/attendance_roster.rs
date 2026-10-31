//! Historical group membership projection; current status alone is insufficient.
use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{FromRow, PgConnection, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;
#[derive(Debug, Clone, FromRow)]
pub struct AttendanceGroupStudent {
    pub learning_group_id: Uuid,
    pub student_id: Uuid,
    pub student_academic_year_id: Uuid,
}
#[derive(Debug, Clone, FromRow)]
pub struct AttendanceMembershipSpan {
    pub learning_group_id: Uuid,
    pub student_id: Uuid,
    pub joined_at: NaiveDate,
    pub left_at: Option<NaiveDate>,
}
pub async fn membership_spans(
    connection: &mut PgConnection,
    groups: &[Uuid],
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<AttendanceMembershipSpan>, AppError> {
    membership_spans_scoped(connection, groups, start, end, None).await
}

/// Resolve one student's historical membership without loading their classmates.
pub async fn membership_spans_for_student(
    connection: &mut PgConnection,
    groups: &[Uuid],
    start: NaiveDate,
    end: NaiveDate,
    student: Uuid,
) -> Result<Vec<AttendanceMembershipSpan>, AppError> {
    membership_spans_scoped(connection, groups, start, end, Some(student)).await
}

async fn membership_spans_scoped(
    connection: &mut PgConnection,
    groups: &[Uuid],
    start: NaiveDate,
    end: NaiveDate,
    student: Option<Uuid>,
) -> Result<Vec<AttendanceMembershipSpan>, AppError> {
    if groups.len() > 20000 {
        return Err(AppError::ValidationError("กลุ่มเกินขนาดที่รองรับ".into()));
    }
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT learning_group_id,student_id,joined_at,left_at FROM learning_group_students WHERE learning_group_id=ANY(",
    );
    query
        .push_bind(groups)
        .push(") AND membership_status<>'removed' AND joined_at<=")
        .push_bind(end)
        .push(" AND (left_at IS NULL OR left_at>=")
        .push_bind(start)
        .push(")");
    if let Some(student) = student {
        query.push(" AND student_id=").push_bind(student);
    }
    query.push(" ORDER BY learning_group_id,student_id LIMIT 100001");
    let rows: Vec<AttendanceMembershipSpan> = DbSession::new(&mut *connection)
        .prepared_query_as(
            "academic_delivery.services.attendance_roster.membership_spans_scoped",
            query.build_query_as(),
        )
        .fetch_all()
        .await?;
    if rows.len() > 100000 {
        return Err(AppError::ValidationError(
            "ประวัติกลุ่มเรียนเกินขนาดที่รองรับ".into(),
        ));
    }
    Ok(rows)
}
pub async fn students(
    pool: &PgPool,
    groups: &[Uuid],
    date: NaiveDate,
) -> Result<Vec<AttendanceGroupStudent>, AppError> {
    students_on_connection(
        &mut *school_tenancy::TenantPool::observed_checkout_pool(
            pool,
            "academic_delivery.services.attendance_roster.students",
        )
        .await?,
        groups,
        date,
    )
    .await
}
pub async fn students_on_connection(
    connection: &mut PgConnection,
    groups: &[Uuid],
    date: NaiveDate,
) -> Result<Vec<AttendanceGroupStudent>, AppError> {
    if groups.len() > 2000 {
        return Err(AppError::ValidationError("กลุ่มเรียนเกินขนาดที่รองรับ".into()));
    }
    let rows:Vec<AttendanceGroupStudent>=DbSession::new(&mut *connection).query_as("academic_delivery.services.attendance_roster.students_on_connection", "SELECT learning_group_id,student_id,student_academic_year_id FROM learning_group_students WHERE learning_group_id=ANY($1) AND membership_status<>'removed' AND joined_at<=$2 AND (left_at IS NULL OR left_at>=$2) ORDER BY learning_group_id,student_id LIMIT 100001").bind(groups).bind(date).fetch_all().await?;
    if rows.len() > 100000 {
        return Err(AppError::ValidationError("สมาชิกกลุ่มเกินขนาดที่รองรับ".into()));
    }
    Ok(rows)
}

#[derive(Clone, FromRow)]
struct SourceMembership {
    learning_group_id: Uuid,
    student_id: Uuid,
    student_academic_year_id: Uuid,
    academic_term_id: Uuid,
    roster_source: String,
    joined_at: NaiveDate,
    left_at: Option<NaiveDate>,
}
impl SourceMembership {
    fn span(self) -> AttendanceMembershipSpan {
        AttendanceMembershipSpan {
            learning_group_id: self.learning_group_id,
            student_id: self.student_id,
            joined_at: self.joined_at,
            left_at: self.left_at,
        }
    }
}

/// Project historical facts against the exact Delivery source pinned by a timetable.
/// `student` narrows the authoritative reads before their bounded row limits.
pub async fn pinned_membership_spans(
    connection: &mut PgConnection,
    version: Uuid,
    groups: &[Uuid],
    start: NaiveDate,
    end: NaiveDate,
    student: Option<Uuid>,
) -> Result<Vec<AttendanceMembershipSpan>, AppError> {
    Ok(
        pinned_memberships(connection, version, groups, start, end, student)
            .await?
            .into_iter()
            .map(SourceMembership::span)
            .collect(),
    )
}

pub async fn pinned_students_on_connection(
    connection: &mut PgConnection,
    version: Uuid,
    groups: &[Uuid],
    date: NaiveDate,
) -> Result<Vec<AttendanceGroupStudent>, AppError> {
    if groups.len() > 2000 {
        return Err(AppError::ValidationError("กลุ่มเรียนเกินขนาดที่รองรับ".into()));
    }
    Ok(
        pinned_memberships(connection, version, groups, date, date, None)
            .await?
            .into_iter()
            .map(|row| AttendanceGroupStudent {
                learning_group_id: row.learning_group_id,
                student_id: row.student_id,
                student_academic_year_id: row.student_academic_year_id,
            })
            .collect(),
    )
}

async fn pinned_memberships(
    connection: &mut PgConnection,
    version: Uuid,
    groups: &[Uuid],
    start: NaiveDate,
    end: NaiveDate,
    student: Option<Uuid>,
) -> Result<Vec<SourceMembership>, AppError> {
    if groups.len() > 20000 {
        return Err(AppError::ValidationError("กลุ่มเกินขนาดที่รองรับ".into()));
    }
    let constraints =
        super::audiences::pinned_group_constraints_on_connection(connection, version, groups)
            .await?;
    let rows: Vec<SourceMembership> = DbSession::new(&mut *connection).query_as(
        "academic_delivery.services.attendance_roster.pinned_memberships",
        "SELECT learning_group_id,student_id,student_academic_year_id,academic_term_id,roster_source,joined_at,left_at FROM learning_group_students WHERE learning_group_id=ANY($1) AND membership_status<>'removed' AND joined_at<=$3 AND (left_at IS NULL OR left_at>=$2) AND ($4::uuid IS NULL OR student_id=$4) ORDER BY learning_group_id,student_id LIMIT 100001",
    ).bind(groups).bind(start).bind(end).bind(student).fetch_all().await?;
    if rows.len() > 100000 {
        return Err(AppError::ValidationError(
            "ประวัติกลุ่มเรียนเกินขนาดที่รองรับ".into(),
        ));
    }
    let terms = rows
        .iter()
        .filter(|row| {
            constraints
                .constraint_for_group(row.learning_group_id)
                .is_some_and(|scope| {
                    scope.allows_group(row.learning_group_id)
                        && uses_temporal_room(&row.roster_source)
                })
        })
        .map(|row| row.academic_term_id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut placements = Vec::new();
    for term in terms {
        let mut term_placements = match student {
            Some(student) => {
                school_academic_core::services::attendance_roster::placement_spans_for_student(
                    connection, term, start, end, student,
                )
                .await?
            }
            None => {
                school_academic_core::services::attendance_roster::placement_spans(
                    connection, term, start, end,
                )
                .await?
            }
        };
        placements.append(&mut term_placements);
    }
    let mut by_year = std::collections::HashMap::<Uuid, Vec<_>>::new();
    for placement in &placements {
        by_year
            .entry(placement.student_academic_year_id)
            .or_default()
            .push(placement);
    }
    let mut projected = Vec::new();
    for row in rows {
        let Some(scope) = constraints.constraint_for_group(row.learning_group_id) else {
            projected.push(row);
            continue;
        };
        // Offering/group existence is required even for established manual facts.
        if !scope.allows_group(row.learning_group_id) {
            continue;
        }
        if !uses_temporal_room(&row.roster_source) {
            projected.push(row);
            continue;
        }
        let from = row.joined_at.max(start);
        let until = row.left_at.unwrap_or(end).min(end);
        let student_placements = by_year
            .get(&row.student_academic_year_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        // Partition at placement changes, retaining the Core provider's precedence
        // at same-day transfer boundaries before testing the pinned room scope.
        let mut boundaries = std::collections::BTreeSet::from([from]);
        for placement in student_placements {
            if placement.start_date > from && placement.start_date <= until {
                boundaries.insert(placement.start_date);
            }
            if let Some(next) = placement.end_date.and_then(|date| date.succ_opt()) {
                if next > from && next <= until {
                    boundaries.insert(next);
                }
            }
        }
        let boundaries = boundaries.into_iter().collect::<Vec<_>>();
        for (index, date) in boundaries.iter().copied().enumerate() {
            let room = student_placements
                .iter()
                .find(|placement| {
                    placement.start_date <= date && placement.end_date.is_none_or(|end| end >= date)
                })
                .map(|placement| placement.homeroom_id);
            if scope.allows_homeroom(row.learning_group_id, room) {
                let mut segment = row.clone();
                segment.joined_at = date;
                segment.left_at = Some(
                    boundaries
                        .get(index + 1)
                        .and_then(|next| next.pred_opt())
                        .unwrap_or(until),
                );
                projected.push(segment);
            }
        }
    }
    if projected.len() > 100000 {
        return Err(AppError::ValidationError(
            "ประวัติกลุ่มเรียนเกินขนาดที่รองรับ".into(),
        ));
    }
    Ok(projected)
}

fn uses_temporal_room(source: &str) -> bool {
    matches!(
        source,
        "placement" | "homeroom_tracking" | "migration_homeroom_snapshot" | "self_registration"
    )
}
