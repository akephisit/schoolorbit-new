use super::invalidate_group_academic_confirmations;
use crate::models::{LearningGroupRosterTracking, RosterTrackingMode, UpdateRosterTrackingRequest};
use chrono::NaiveDate;
use school_academic_core::ports::PlacementRosterPort;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

pub struct RoomRosterTracking;
#[async_trait::async_trait]
impl PlacementRosterPort for RoomRosterTracking {
    async fn reconcile(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: Uuid,
        ids: &[Uuid],
    ) -> Result<(), AppError> {
        sync_students(tx, actor, ids).await
    }
}

const TRACKING_COLUMNS: &str = r#"COALESCE(config.mode,'manual') AS mode, config.effective_from,
    g.row_version AS group_row_version, o.starts_on AS starts_on,
    o.ends_on AS ends_on"#;
const TRACKING_FROM: &str = r#"FROM learning_groups g JOIN academic_terms t ON t.id=g.academic_term_id
    JOIN learning_offerings o ON o.id=g.learning_offering_id
    JOIN academic_years y ON y.id=g.academic_year_id
    LEFT JOIN learning_group_roster_tracking config ON config.learning_group_id=g.id"#;

#[derive(sqlx::FromRow)]
struct TrackingRow {
    learning_group_id: Uuid,
    #[sqlx(flatten)]
    tracking: LearningGroupRosterTracking,
}

pub(super) async fn get_many_on_connection(
    connection: &mut sqlx::PgConnection,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, LearningGroupRosterTracking>, AppError> {
    let sql = format!(
        "SELECT g.id AS learning_group_id,{TRACKING_COLUMNS} {TRACKING_FROM} WHERE g.id=ANY($1)"
    );
    let rows: Vec<TrackingRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.roster_tracking.get_many_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(ids)
        .fetch_all()
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.learning_group_id, row.tracking))
        .collect())
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<LearningGroupRosterTracking, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.roster_tracking.get",
    )
    .await?;
    get_on_connection(&mut connection, id).await
}

pub async fn get_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<LearningGroupRosterTracking, AppError> {
    DbSession::new(connection)
        .query_as(
            "academic_delivery.services.roster_tracking.get_on_connection",
            sqlx::AssertSqlSafe(format!(
                "SELECT {TRACKING_COLUMNS} {TRACKING_FROM} WHERE g.id=$1"
            )),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".into()))
}

fn validate(
    mode: RosterTrackingMode,
    date: Option<NaiveDate>,
    starts: NaiveDate,
    ends: Option<NaiveDate>,
    today: NaiveDate,
) -> Result<(), AppError> {
    match (mode, date) {
        (RosterTrackingMode::Manual, None) => Ok(()),
        (RosterTrackingMode::Homeroom, Some(date))
            if date >= today && date >= starts && ends.is_none_or(|end| date <= end) =>
        {
            Ok(())
        }
        _ => Err(AppError::ValidationError(
            "เลือกวันเริ่มติดตามตั้งแต่วันนี้ ภายในช่วงเปิดสอนของกลุ่ม".into(),
        )),
    }
}

/// Shared authoritative validation for tracking policy changes. This never rebuilds memberships.
pub(super) async fn validate_configuration(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
    offering: Uuid,
    mode: RosterTrackingMode,
    effective_from: Option<NaiveDate>,
) -> Result<(), AppError> {
    let contexts = get_many_on_connection(connection, &[id]).await?;
    if contexts.is_empty() {
        return Err(AppError::NotFound("ไม่พบกลุ่มเรียน".into()));
    }
    let conflicts =
        configuration_conflicts(connection, offering, &contexts, mode, effective_from).await?;
    if let Some(error) = conflicts.into_values().next() {
        return Err(error);
    }
    Ok(())
}

/// Evaluate the same date/source/member policy for a bounded set without per-group queries.
pub(super) async fn configuration_conflicts(
    connection: &mut sqlx::PgConnection,
    offering: Uuid,
    contexts: &HashMap<Uuid, LearningGroupRosterTracking>,
    mode: RosterTrackingMode,
    effective_from: Option<NaiveDate>,
) -> Result<HashMap<Uuid, AppError>, AppError> {
    let today: NaiveDate = DbSession::new(connection)
        .scalar(
            "academic_delivery.services.roster_tracking.configuration_conflicts",
            "SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date",
        )
        .fetch_one()
        .await?;
    let mut conflicts = HashMap::new();
    for (id, context) in contexts {
        if let Err(AppError::ValidationError(reason)) = validate(
            mode,
            effective_from,
            context.starts_on,
            context.ends_on,
            today,
        ) {
            conflicts.insert(*id, AppError::ValidationError(reason));
        }
    }
    if mode == RosterTrackingMode::Homeroom {
        let ids: Vec<Uuid> = contexts.keys().copied().collect();
        let rows: Vec<(Uuid, bool, bool)> = DbSession::new(connection).query_as("academic_delivery.services.roster_tracking.configuration_conflicts", r#"
            SELECT g.id,
                EXISTS(SELECT 1 FROM learning_group_homerooms WHERE learning_group_id=g.id)
                AND NOT EXISTS(SELECT 1 FROM activity_offering_details WHERE learning_offering_id=$2 AND registration_type='self') AS valid_source,
                EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=g.id AND m.membership_status='active'
                    AND NOT EXISTS(SELECT 1 FROM homeroom_placements p
                        JOIN learning_group_homerooms c ON c.homeroom_id=p.homeroom_id AND c.learning_group_id=g.id
                        JOIN student_academic_years sy ON sy.id=p.student_academic_year_id AND sy.status IN ('active','planned')
                        JOIN academic_years y ON y.id=sy.academic_year_id
                        JOIN users u ON u.id=sy.student_id AND u.status='active'
                        WHERE p.student_academic_year_id=m.student_academic_year_id
                            AND (p.status='current' OR (p.status='planned' AND y.status IN ('planning','ready')))
                            AND p.end_date IS NULL AND (o.ends_on IS NULL OR p.start_date<=o.ends_on))) AS extra_members
            FROM learning_groups g JOIN academic_terms t ON t.id=g.academic_term_id
            JOIN academic_years ay ON ay.id=g.academic_year_id JOIN learning_offerings o ON o.id=g.learning_offering_id
            WHERE g.id=ANY($1) AND o.id=$2
        "#).bind(&ids).bind(offering).fetch_all().await?;
        for (id, valid_source, extra_members) in rows {
            if !valid_source {
                conflicts.insert(
                    id,
                    AppError::ValidationError(
                        "กำหนดห้องต้นทางก่อน และใช้จัดรายชื่อเองสำหรับกิจกรรมสมัครด้วยตนเอง".into(),
                    ),
                );
            } else if extra_members {
                conflicts.insert(
                    id,
                    AppError::Conflict(
                        "รายชื่อปัจจุบันไม่ตรงกับนักเรียนที่พร้อมเรียนในห้องต้นทาง กรุณาตรวจรายชื่อก่อนเปิดติดตามอัตโนมัติ"
                            .into(),
                    ),
                );
            }
        }
    }
    Ok(conflicts)
}

pub(super) async fn save_configurations(
    connection: &mut sqlx::PgConnection,
    ids: &[Uuid],
    mode: RosterTrackingMode,
    effective_from: Option<NaiveDate>,
) -> Result<(), AppError> {
    DbSession::new(connection).query("academic_delivery.services.roster_tracking.save_configurations", "INSERT INTO learning_group_roster_tracking(learning_group_id,mode,effective_from) SELECT id,$2,$3 FROM unnest($1::uuid[]) id ON CONFLICT(learning_group_id) DO UPDATE SET mode=EXCLUDED.mode,effective_from=EXCLUDED.effective_from")
        .bind(ids).bind(mode).bind(effective_from).execute().await?;
    Ok(())
}

pub async fn update(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    request: UpdateRosterTrackingRequest,
) -> Result<LearningGroupRosterTracking, AppError> {
    super::validate_row_version(request.row_version)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.roster_tracking.update",
    )
    .await?;
    let (year,term,offering): (Uuid,Uuid,Uuid) = DbSession::new(&mut *tx).query_as("academic_delivery.services.roster_tracking.update", "SELECT academic_year_id,academic_term_id,learning_offering_id FROM learning_groups WHERE id=$1").bind(id).fetch_one().await?;
    school_academic_core::services::lifecycle_guard::require_year_write_exclusive(&mut tx, year)
        .await?;
    super::ensure_writable_term(&mut tx, term, false).await?;
    DbSession::new(&mut *tx)
        .query(
            "academic_delivery.services.roster_tracking.update",
            "SELECT id FROM learning_offerings WHERE id=$1 FOR UPDATE",
        )
        .bind(offering)
        .execute()
        .await?;
    let (version, status, roster): (i64, String, String) = DbSession::new(&mut *tx)
        .query_as(
            "academic_delivery.services.roster_tracking.update",
            "SELECT row_version,status,roster_status FROM learning_groups WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .fetch_one()
        .await?;
    if version != request.row_version
        || ["closed", "cancelled"].contains(&status.as_str())
        || roster == "closed"
    {
        return Err(AppError::Conflict(
            "กลุ่มเรียนเปลี่ยนแล้วหรือปิดใช้งาน กรุณาโหลดใหม่".into(),
        ));
    }
    validate_configuration(&mut tx, id, offering, request.mode, request.effective_from).await?;
    save_configurations(&mut tx, &[id], request.mode, request.effective_from).await?;
    if request.mode == RosterTrackingMode::Homeroom && roster == "published" {
        sync_group(&mut tx, actor, id, None).await?;
    }
    DbSession::new(&mut *tx)
        .query(
            "academic_delivery.services.roster_tracking.update",
            "UPDATE learning_groups SET row_version=row_version+1,updated_at=now() WHERE id=$1",
        )
        .bind(id)
        .execute()
        .await?;
    super::append_audit(
        &mut *tx,
        "learning_group.roster_tracking_changed",
        "learning_group",
        id,
        year,
        term,
        actor,
        serde_json::json!({"mode":request.mode,"effectiveFrom":request.effective_from}),
    )
    .await?;
    tx.commit().await?;
    get(pool, id).await
}

pub async fn require_manual(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<(), AppError> {
    let automatic: bool=DbSession::new(&mut **tx).scalar("academic_delivery.services.roster_tracking.require_manual", "SELECT EXISTS(SELECT 1 FROM learning_group_roster_tracking WHERE learning_group_id=$1 AND mode='homeroom')").bind(id).fetch_one().await?;
    if automatic {
        Err(AppError::Conflict(
            "กลุ่มนี้ติดตามห้องอัตโนมัติ ให้จัดนักเรียนจากห้องต้นทาง หรือเปลี่ยนเป็นจัดรายชื่อเองก่อน".into(),
        ))
    } else {
        Ok(())
    }
}

pub async fn sync_students(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    ids: &[Uuid],
) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    let groups: Vec<Uuid>=DbSession::new(&mut **tx).scalar("academic_delivery.services.roster_tracking.sync_students", r#"SELECT g.id FROM learning_groups g
        JOIN learning_group_roster_tracking config ON config.learning_group_id=g.id AND config.mode='homeroom'
        JOIN academic_terms t ON t.id=g.academic_term_id JOIN academic_years y ON y.id=g.academic_year_id
        JOIN learning_offerings o ON o.id=g.learning_offering_id
        WHERE g.roster_status='published' AND g.status NOT IN ('closed','cancelled') AND o.status='published'
        AND t.status NOT IN ('closed','cancelled') AND y.status NOT IN ('closed','archived')
        AND (EXISTS(SELECT 1 FROM homeroom_placements p JOIN learning_group_homerooms c ON c.homeroom_id=p.homeroom_id AND c.learning_group_id=g.id WHERE p.student_academic_year_id=ANY($1))
            OR EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=g.id AND m.student_academic_year_id=ANY($1)))
        ORDER BY o.id,g.id FOR UPDATE OF o,g"#).bind(ids).fetch_all().await?;
    for id in groups {
        sync_group(tx, actor, id, Some(ids)).await?;
    }
    Ok(())
}

pub async fn sync_group(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    id: Uuid,
    ids: Option<&[Uuid]>,
) -> Result<(), AppError> {
    let enabled: bool=DbSession::new(&mut **tx).scalar("academic_delivery.services.roster_tracking.sync_group", "SELECT EXISTS(SELECT 1 FROM learning_group_roster_tracking WHERE learning_group_id=$1 AND mode='homeroom')").bind(id).fetch_one().await?;
    if !enabled {
        return Ok(());
    }
    let rows: Vec<(Uuid,Uuid,NaiveDate,Uuid)>=DbSession::new(&mut **tx).query_as("academic_delivery.services.roster_tracking.sync_group", r#"SELECT sy.id,sy.student_id,MAX(GREATEST(p.start_date,config.effective_from,o.starts_on)) AS joined,p.homeroom_id
        FROM learning_groups g JOIN learning_group_roster_tracking config ON config.learning_group_id=g.id
        JOIN academic_terms t ON t.id=g.academic_term_id JOIN learning_offerings o ON o.id=g.learning_offering_id
        JOIN academic_years y ON y.id=g.academic_year_id
        JOIN learning_group_homerooms c ON c.learning_group_id=g.id JOIN homeroom_placements p ON p.homeroom_id=c.homeroom_id
        JOIN student_academic_years sy ON sy.id=p.student_academic_year_id AND sy.academic_year_id=g.academic_year_id
        JOIN users u ON u.id=sy.student_id AND u.status='active'
        WHERE g.id=$1 AND (p.status='current' OR (p.status='planned' AND y.status IN ('planning','ready'))) AND p.end_date IS NULL AND sy.status IN ('active','planned')
            AND ($2::uuid[] IS NULL OR sy.id=ANY($2))
            AND (o.ends_on IS NULL OR GREATEST(p.start_date,config.effective_from,o.starts_on)<=o.ends_on)
        GROUP BY sy.id,sy.student_id,p.homeroom_id"#).bind(id).bind(ids).fetch_all().await?;
    let desired: Vec<_> = rows.iter().map(|row| row.0).collect();
    let old: Vec<(Uuid,Uuid,NaiveDate,NaiveDate)>=DbSession::new(&mut **tx).query_as("academic_delivery.services.roster_tracking.sync_group", r#"SELECT m.id,m.student_academic_year_id,m.joined_at,
        LEAST(COALESCE((SELECT MAX(p.end_date) FROM homeroom_placements p JOIN learning_group_homerooms c ON c.homeroom_id=p.homeroom_id AND c.learning_group_id=m.learning_group_id WHERE p.student_academic_year_id=m.student_academic_year_id AND p.status='ended' AND p.end_date>=m.joined_at AND sy.status IN ('active','planned')), (now() AT TIME ZONE 'Asia/Bangkok')::date),o.ends_on)
        FROM learning_group_students m JOIN learning_groups g ON g.id=m.learning_group_id
        JOIN student_academic_years sy ON sy.id=m.student_academic_year_id
        JOIN learning_offerings o ON o.id=g.learning_offering_id JOIN academic_terms t ON t.id=g.academic_term_id
        JOIN academic_years y ON y.id=g.academic_year_id WHERE m.learning_group_id=$1 AND m.membership_status='active'
            AND ($2::uuid[] IS NULL OR m.student_academic_year_id=ANY($2)) AND NOT(m.student_academic_year_id=ANY($3)) FOR UPDATE OF m"#).bind(id).bind(ids).bind(&desired).fetch_all().await?;
    let old_ids: Vec<_> = old.iter().map(|row| row.0).collect();
    let left_dates: Vec<_> = old.iter().map(|row| row.3).collect();
    let ended = DbSession::new(&mut **tx)
        .query(
            "academic_delivery.services.roster_tracking.sync_group",
            r#"UPDATE learning_group_students m SET
        membership_status=CASE WHEN changes.left_on < m.joined_at THEN 'removed' ELSE 'ended' END,
        left_at=GREATEST(m.joined_at,changes.left_on),row_version=m.row_version+1,updated_at=now()
        FROM unnest($1::uuid[],$2::date[]) changes(id,left_on) WHERE m.id=changes.id"#,
        )
        .bind(&old_ids)
        .bind(&left_dates)
        .execute()
        .await?
        .rows_affected();
    // Keep raw desired candidates above: a denied new intake is not a departure.
    let today: NaiveDate = DbSession::new(&mut **tx)
        .scalar(
            "academic_delivery.services.roster_tracking.sync_group",
            "SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date",
        )
        .fetch_one()
        .await?;
    let dates: Vec<_> = rows
        .iter()
        .map(|row| row.2.max(today))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let constraints_by_date =
        super::audiences::dated_group_constraints_for_dates_on_connection(&mut **tx, &[id], &dates)
            .await?;
    let mut eligible = Vec::with_capacity(rows.len());
    for row in rows {
        let scopes = constraints_by_date.get(&row.2.max(today)).ok_or_else(|| {
            AppError::InternalServerError("ไม่พบขอบเขตเปิดสอนสำหรับวันที่ตรวจสอบ".into())
        })?;
        if scopes
            .constraint_for_group(id)
            .is_none_or(|scope| scope.allows_homeroom(id, Some(row.3)))
        {
            eligible.push(row);
        }
    }
    let rows = eligible;
    let student_years: Vec<_> = rows.iter().map(|row| row.0).collect();
    let students: Vec<_> = rows.iter().map(|row| row.1).collect();
    let joined_dates: Vec<_> = rows.iter().map(|row| row.2).collect();
    let overlaps: bool = DbSession::new(&mut **tx).scalar("academic_delivery.services.roster_tracking.sync_group", r#"SELECT EXISTS(
        SELECT 1 FROM unnest($1::uuid[],$2::date[]) candidate(student_id,joined_at)
        JOIN learning_group_students m ON m.learning_group_id=$3 AND m.student_id=candidate.student_id
        WHERE m.membership_status<>'removed'
          AND daterange(m.joined_at,m.left_at,'[]') && daterange(candidate.joined_at,NULL,'[]')
          AND NOT EXISTS(SELECT 1 FROM learning_group_students active
            WHERE active.learning_group_id=$3 AND active.student_id=candidate.student_id AND active.membership_status='active'))"#)
        .bind(&students).bind(&joined_dates).bind(id).fetch_one().await?;
    if overlaps {
        return Err(AppError::Conflict(
            "วันที่จัดห้องซ้อนกับประวัติกลุ่มเรียนเดิม กรุณาตรวจสอบวันที่".into(),
        ));
    }
    let inserted = DbSession::new(&mut **tx).query("academic_delivery.services.roster_tracking.sync_group", r#"INSERT INTO learning_group_students(id,learning_group_id,academic_term_id,academic_year_id,student_academic_year_id,student_id,membership_status,roster_source,joined_at,published_at)
        SELECT gen_random_uuid(),g.id,g.academic_term_id,g.academic_year_id,candidate.student_year,candidate.student_id,
            'active','homeroom_tracking',candidate.joined_at,CASE WHEN g.roster_status='published' THEN now() END
        FROM learning_groups g CROSS JOIN unnest($2::uuid[],$3::uuid[],$4::date[]) candidate(student_year,student_id,joined_at)
        WHERE g.id=$1 AND NOT EXISTS(SELECT 1 FROM learning_group_students m
            WHERE m.learning_group_id=g.id AND m.student_id=candidate.student_id AND m.membership_status='active')"#)
        .bind(id).bind(&student_years).bind(&students).bind(&joined_dates).execute().await?.rows_affected();
    let changed = ended + inserted;
    let exceeded: bool=DbSession::new(&mut **tx).scalar("academic_delivery.services.roster_tracking.sync_group", "SELECT LEAST(g.capacity,a.capacity) IS NOT NULL AND (SELECT count(*) FROM learning_group_students WHERE learning_group_id=$1 AND membership_status='active')>LEAST(g.capacity,a.capacity) FROM learning_groups g LEFT JOIN activity_offering_details a ON a.learning_offering_id=g.learning_offering_id WHERE g.id=$1").bind(id).fetch_one().await?;
    if exceeded {
        return Err(AppError::Conflict(
            "นักเรียนเกินความจุกลุ่มวิชาที่ติดตามห้อง กรุณาปรับความจุกลุ่มก่อน".into(),
        ));
    }
    if changed > 0 {
        DbSession::new(&mut **tx)
            .query(
                "academic_delivery.services.roster_tracking.sync_group",
                "UPDATE learning_groups SET row_version=row_version+1,updated_at=now() WHERE id=$1",
            )
            .bind(id)
            .execute()
            .await?;
        invalidate_group_academic_confirmations(tx, &[id]).await?;
        let (year, term): (Uuid, Uuid) = DbSession::new(&mut **tx)
            .query_as(
                "academic_delivery.services.roster_tracking.sync_group",
                "SELECT academic_year_id,academic_term_id FROM learning_groups WHERE id=$1",
            )
            .bind(id)
            .fetch_one()
            .await?;
        super::append_audit(
            &mut **tx,
            "learning_group.homeroom_roster_tracked",
            "learning_group",
            id,
            year,
            term,
            actor,
            serde_json::json!({"changedMemberships":changed}),
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tracking_is_explicit_and_cannot_rewrite_past_dates() {
        let starts = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        let ends = Some(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap());
        let today = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        assert!(validate(RosterTrackingMode::Manual, None, starts, ends, today).is_ok());
        assert!(validate(
            RosterTrackingMode::Homeroom,
            Some(today),
            starts,
            ends,
            today
        )
        .is_ok());
        assert!(validate(
            RosterTrackingMode::Homeroom,
            Some(starts),
            starts,
            ends,
            today
        )
        .is_err());
        assert!(validate(
            RosterTrackingMode::Homeroom,
            Some(today + chrono::Duration::days(400)),
            starts,
            None,
            today
        )
        .is_ok());
        assert!(validate(RosterTrackingMode::Homeroom, None, starts, ends, today).is_err());
        assert!(validate(RosterTrackingMode::Manual, Some(today), starts, ends, today).is_err());
    }
}
