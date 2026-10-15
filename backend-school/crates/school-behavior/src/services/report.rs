use super::shared::{current_cycle, today};
use crate::{
    access::{require_student_read, BehaviorAccess},
    models::*,
    rules::{invalid, pagination, totals, validate_range},
};
use school_errors::AppError;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

fn report_filter(
    access: BehaviorAccess,
    q: &BehaviorReportQuery,
    prefix: &str,
) -> QueryBuilder<Postgres> {
    let mut b = QueryBuilder::new(prefix.to_owned());
    b.push(" FROM behavior_entries e JOIN behavior_daily_totals t ON t.id=e.total_id JOIN behavior_cycles c ON c.id=t.cycle_id JOIN academic_terms term ON term.id=t.academic_term_id JOIN academic_years year ON year.id=term.academic_year_id JOIN users u ON u.id=t.student_id LEFT JOIN student_info si ON si.user_id=u.id LEFT JOIN homerooms h ON h.id=t.homeroom_id LEFT JOIN behavior_batches batch ON batch.id=e.batch_id LEFT JOIN user_display_names creator ON creator.id=e.created_by WHERE t.date BETWEEN ");
    b.push_bind(q.start).push(" AND ").push_bind(q.end);
    if !access.school {
        b.push(" AND((").push_bind(access.own).push(" AND(e.created_by=").push_bind(access.actor_id)
          .push(" OR t.student_id=").push_bind(access.actor_id).push(" OR EXISTS(SELECT 1 FROM student_parents p JOIN users pu ON pu.id=p.parent_user_id AND pu.status='active' WHERE p.student_user_id=t.student_id AND p.parent_user_id=").push_bind(access.actor_id).push(")))OR(")
          .push_bind(access.assigned).push(" AND EXISTS(SELECT 1 FROM homeroom_advisors a WHERE a.homeroom_id=t.homeroom_id AND a.user_id=").push_bind(access.actor_id).push(")))");
    }
    if let Some(search) = q.search.as_ref().filter(|s| !s.trim().is_empty()) {
        let pattern = format!("%{}%", search.trim());
        b.push(" AND(concat_ws(' ',u.first_name,u.last_name)ILIKE ")
            .push_bind(pattern.clone())
            .push(" OR si.student_id ILIKE ")
            .push_bind(pattern.clone())
            .push(" OR e.item_name ILIKE ")
            .push_bind(pattern.clone())
            .push(" OR batch.detail ILIKE ")
            .push_bind(pattern)
            .push(")");
    }
    if let Some(id) = q.batch_id {
        b.push(" AND e.batch_id=").push_bind(id);
    }
    if let Some(id) = q.academic_term_id {
        b.push(" AND t.academic_term_id=").push_bind(id);
    }
    if let Some(id) = q.student_id {
        b.push(" AND t.student_id=").push_bind(id);
    }
    if let Some(id) = q.homeroom_id {
        b.push(" AND t.homeroom_id=").push_bind(id);
    }
    if let Some(id) = q.grade_level_id {
        b.push(" AND t.grade_level_id=").push_bind(id);
    }
    if let Some(side) = q.side {
        b.push(" AND t.side=").push_bind(side);
    }
    if let Some(source) = q.source {
        b.push(" AND t.source=").push_bind(source);
    }
    if q.mine.unwrap_or(false) {
        b.push(" AND e.created_by=").push_bind(access.actor_id);
    } else if let Some(actor) = q.created_by {
        b.push(" AND e.created_by=").push_bind(actor);
    }
    if let Some(cancelled) = q.cancelled {
        b.push(" AND(e.cancelled_at IS NOT NULL)=")
            .push_bind(cancelled);
    }
    b
}
pub async fn report(
    pool: &PgPool,
    access: BehaviorAccess,
    q: BehaviorReportQuery,
) -> Result<BehaviorReportPage, AppError> {
    access.require_read()?;
    validate_range(q.start, q.end)?;
    if q.search.as_ref().is_some_and(|s| s.chars().count() > 200) {
        return Err(invalid("คำค้นยาวเกินไป"));
    }
    let (offset, limit) = pagination(q.offset, q.limit)?;
    let mut connection = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *connection)
        .await?;
    let mut query=report_filter(access,&q,"SELECT e.id,t.student_id,concat_ws(' ',u.title,u.first_name,u.last_name)student_name,si.student_id student_code,t.homeroom_id,COALESCE(h.name,'ยังไม่จัดห้อง')homeroom,t.grade_level_id,t.date,t.academic_term_id,t.side,t.source,e.points,e.item_name,batch.detail,batch.occurred_at,e.batch_id,e.source_key,e.source_session_id,e.created_by,creator.display_name creator_name,e.created_at,e.cancelled_at,e.cancellation_reason,(c.state='current' AND term.status NOT IN('closed','cancelled') AND year.status NOT IN('closed','archived'))can_cancel");
    query
        .push(" ORDER BY t.date DESC,e.created_at DESC,e.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);
    let mut items: Vec<BehaviorReportEntry> =
        query.build_query_as().fetch_all(&mut *connection).await?;
    for item in &mut items {
        item.can_cancel = item.can_cancel
            && item.source == BehaviorSource::Manual
            && item.cancelled_at.is_none()
            && access.can_cancel(item.created_by);
    }
    let mut count = report_filter(access, &q, "SELECT count(*)::bigint");
    let total = count
        .build_query_scalar()
        .fetch_one(&mut *connection)
        .await?;
    connection.commit().await?;
    Ok(BehaviorReportPage { items, total })
}
pub async fn student_summary(
    pool: &PgPool,
    access: BehaviorAccess,
    student: Uuid,
    q: BehaviorSummaryQuery,
) -> Result<BehaviorStudentSummary, AppError> {
    let mut connection = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *connection)
        .await?;
    require_student_read(&mut connection, access, student).await?;
    let merit_cycle = current_cycle(&mut connection, BehaviorSide::Merit).await?;
    let deduction_cycle = current_cycle(&mut connection, BehaviorSide::Deduction).await?;
    let activation: chrono::NaiveDate =
        sqlx::query_scalar("SELECT activation_date FROM behavior_state WHERE singleton")
            .fetch_one(&mut *connection)
            .await?;
    let start = q.start.unwrap_or(activation);
    let end = q.end.unwrap_or_else(today);
    validate_range(start, end)?;
    let rows:Vec<BehaviorDailyTotal>=sqlx::query_as(r#"SELECT t.id,t.student_id,t.date,t.academic_term_id,t.grade_level_id,t.homeroom_id,t.side,t.source,t.cycle_id,t.points,t.row_version FROM behavior_daily_totals t WHERE t.student_id=$1 AND t.date BETWEEN $2 AND $3 AND($4::uuid IS NULL OR t.academic_term_id=$4)AND(
      $5 OR($6 AND(t.student_id=$7 OR EXISTS(SELECT 1 FROM student_parents p JOIN users pu ON pu.id=p.parent_user_id AND pu.status='active' WHERE p.student_user_id=t.student_id AND p.parent_user_id=$7)))
      OR($8 AND EXISTS(SELECT 1 FROM homeroom_advisors a WHERE a.homeroom_id=t.homeroom_id AND a.user_id=$7)))ORDER BY t.date DESC,t.side,t.source,t.id LIMIT 100001"#)
      .bind(student).bind(start).bind(end).bind(q.academic_term_id).bind(access.school).bind(access.own).bind(access.actor_id).bind(access.assigned).fetch_all(&mut *connection).await?;
    if rows.len() > 100000 {
        return Err(invalid("ข้อมูลสรุปเกินขนาดที่รองรับ กรุณาเลือกช่วงวันที่"));
    }
    let comparison_start = start
        .max(merit_cycle.starts_on)
        .max(deduction_cycle.starts_on);
    let comparison_end = end;
    let mut current = totals(0, 0);
    let mut history = totals(0, 0);
    let mut comparable = totals(0, 0);
    let mut history_totals = Vec::new();
    for row in rows {
        let active = row.cycle_id == merit_cycle.id || row.cycle_id == deduction_cycle.id;
        let target = if active { &mut current } else { &mut history };
        if row.side == BehaviorSide::Merit {
            target.merit += row.points;
        } else {
            target.deduction += row.points;
        }
        if active && row.date >= comparison_start {
            if row.side == BehaviorSide::Merit {
                comparable.merit += row.points;
            } else {
                comparable.deduction += row.points;
            }
        }
        if !active {
            history_totals.push(row);
        }
    }
    // Independently archived sides are aggregate buckets, not matched windows.
    let history = BehaviorCurrentTotals {
        merit: history.merit,
        deduction: history.deduction,
        net: None,
    };
    let lifetime = totals(
        current.merit + history.merit,
        current.deduction + history.deduction,
    );
    let equal_window = merit_cycle.starts_at == deduction_cycle.starts_at;
    let later_start = merit_cycle.starts_at.max(deduction_cycle.starts_at);
    let archived_comparison_cycles: Vec<_> = history_totals
        .iter()
        .filter(|row| row.date >= comparison_start)
        .map(|row| row.cycle_id)
        .collect();
    // Partial archive can move only one side out of an otherwise equal current
    // cycle. Completed reset windows ending exactly at its start do not overlap.
    let archived_overlap: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM behavior_cycles WHERE id=ANY($1) AND COALESCE(ends_at,'infinity'::timestamptz)>$2)")
        .bind(archived_comparison_cycles).bind(later_start).fetch_one(&mut *connection).await?;
    let current = BehaviorCurrentTotals {
        merit: current.merit,
        deduction: current.deduction,
        net: (equal_window && !archived_overlap).then_some(current.merit - current.deduction),
    };
    let later = merit_cycle
        .starts_at
        .max(deduction_cycle.starts_at)
        .with_timezone(&chrono_tz::Asia::Bangkok);
    let comparison_exact = !archived_overlap
        && (equal_window
            || later.time() == chrono::NaiveTime::MIN
            || comparison_start > later.date_naive());
    let comparable_current = BehaviorCurrentTotals {
        merit: comparable.merit,
        deduction: comparable.deduction,
        net: comparison_exact.then_some(comparable.merit - comparable.deduction),
    };
    connection.commit().await?;
    Ok(BehaviorStudentSummary {
        student_id: student,
        current,
        history,
        lifetime,
        comparable_current,
        comparison_exact,
        comparison_start,
        comparison_end,
        merit_cycle,
        deduction_cycle,
        history_totals,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn own_and_assigned_report_branches_are_combined() {
        let a = BehaviorAccess {
            actor_id: Uuid::new_v4(),
            own: true,
            assigned: true,
            school: false,
            create_assigned: false,
            create_school: false,
            cancel_own: false,
            cancel_school: false,
        };
        let q = BehaviorReportQuery {
            start: today(),
            end: today(),
            academic_term_id: None,
            student_id: None,
            homeroom_id: None,
            grade_level_id: None,
            side: None,
            source: None,
            search: None,
            batch_id: None,
            created_by: None,
            mine: None,
            cancelled: None,
            offset: None,
            limit: None,
        };
        let sql = report_filter(a, &q, "SELECT count(*)").sql();
        let sql = sql.as_str();
        assert!(sql.contains("OR("));
        assert!(sql.contains("student_parents"));
        assert!(sql.contains("homeroom_advisors"));
    }
}
