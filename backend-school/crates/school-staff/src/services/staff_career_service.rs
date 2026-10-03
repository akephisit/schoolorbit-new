use crate::{audit::AuditLogBuilder, career::*};
use chrono::{NaiveDate, Utc};
use school_errors::AppError;
use serde::Deserialize;
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;

fn today() -> NaiveDate {
    Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive()
}

fn conflict() -> AppError {
    AppError::Conflict("ข้อมูลเปลี่ยนไปแล้ว กรุณาโหลดข้อมูลล่าสุดก่อนบันทึกอีกครั้ง".into())
}

// Only static, service-owned SQL aliases/expressions are passed here.
pub(crate) fn career_json_sql(
    history: &'static str,
    position: Option<&'static str>,
    is_current: &'static str,
) -> String {
    let position = position.map_or_else(|| "NULL".to_owned(), |alias| format!(
        "CASE WHEN {alias}.id IS NOT NULL THEN jsonb_build_object('id',{alias}.id,'code',{alias}.code,'name',{alias}.name,'isActive',{alias}.is_active,'isSelectable',{alias}.is_selectable) END"
    ));
    format!("CASE WHEN {history}.id IS NOT NULL THEN jsonb_build_object(
        'id',{history}.id,'staffId',{history}.user_id,
        'fact',jsonb_build_object('kind',{history}.kind,'value',CASE {history}.kind
            WHEN 'personnel_type' THEN {history}.personnel_type::text
            WHEN 'job_position' THEN {history}.job_position_id::text
            WHEN 'academic_rank' THEN {history}.academic_rank::text END),
        'jobPosition',{position},'effectiveDate',{history}.effective_date,'orderDate',{history}.order_date,
        'orderNumber',{history}.order_number,'note',{history}.note,'source',{history}.source,
        'revision',{history}.revision,'isCurrent',COALESCE(({is_current}),false),
        'createdAt',{history}.created_at,'updatedAt',{history}.updated_at) END")
}

const CURRENT_MATCH: &str = "h.id=ANY(ARRAY[info.current_personnel_type_history_id,info.current_job_position_history_id,info.current_academic_rank_history_id])";

async fn lock_staff(tx: &mut Transaction<'_, Postgres>, staff_id: Uuid) -> Result<(), AppError> {
    let found: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE id=$1 AND user_type='staff' FOR UPDATE")
            .bind(staff_id)
            .fetch_optional(&mut **tx)
            .await?;
    found.ok_or_else(|| AppError::NotFound("ไม่พบบุคลากร".into()))?;
    Ok(())
}

async fn read_entry(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    entry_id: Uuid,
) -> Result<Option<StaffCareerEntry>, AppError> {
    let expression = career_json_sql("h", Some("position"), CURRENT_MATCH);
    let sql = format!(
        "SELECT {expression} FROM staff_career_history h
        LEFT JOIN staff_info info ON info.user_id=h.user_id
        LEFT JOIN staff_job_positions position ON position.id=h.job_position_id
        WHERE h.id=$1 AND h.user_id=$2"
    );
    let row: Option<Json<StaffCareerEntry>> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(entry_id)
        .bind(staff_id)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(row.map(|Json(entry)| entry))
}

async fn current_entry(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    kind: StaffCareerKind,
) -> Result<Option<StaffCareerEntry>, AppError> {
    let expression = career_json_sql("h", Some("position"), "true");
    let sql = format!(
        "SELECT {expression} FROM staff_info info
        JOIN staff_career_history h ON h.id=CASE $2::text
            WHEN 'personnel_type' THEN info.current_personnel_type_history_id
            WHEN 'job_position' THEN info.current_job_position_history_id
            WHEN 'academic_rank' THEN info.current_academic_rank_history_id END
        LEFT JOIN staff_job_positions position ON position.id=h.job_position_id
        WHERE info.user_id=$1"
    );
    let row: Option<Json<StaffCareerEntry>> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(staff_id)
        .bind(kind.as_str())
        .fetch_optional(&mut **tx)
        .await?;
    Ok(row.map(|Json(entry)| entry))
}

fn entry_input(entry: &StaffCareerEntry) -> StaffCareerEntryInput {
    StaffCareerEntryInput {
        fact: entry.fact.clone(),
        effective_date: entry.effective_date,
        order_date: entry.order_date,
        order_number: entry.order_number.clone(),
        note: entry.note.clone(),
    }
}

fn fact_columns(
    fact: &StaffCareerFact,
) -> (Option<&'static str>, Option<Uuid>, Option<&'static str>) {
    match fact {
        StaffCareerFact::PersonnelType { value } => {
            (value.map(StaffPersonnelType::as_str), None, None)
        }
        StaffCareerFact::JobPosition { value } => (None, *value, None),
        StaffCareerFact::AcademicRank { value } => (
            None,
            None,
            value.map(crate::personnel::StaffAcademicRank::as_str),
        ),
    }
}

fn empty_input(input: &StaffCareerEntryInput) -> bool {
    let (personnel, position, rank) = fact_columns(&input.fact);
    personnel.is_none()
        && position.is_none()
        && rank.is_none()
        && input.effective_date.is_none()
        && input.order_date.is_none()
        && input.order_number.is_none()
        && input.note.is_none()
}

async fn validate_position(
    tx: &mut Transaction<'_, Postgres>,
    input: &StaffCareerEntryInput,
    previous: Option<&StaffCareerEntry>,
) -> Result<(), AppError> {
    if let StaffCareerFact::JobPosition { value } = input.fact {
        let existing = previous.and_then(|entry| match entry.fact {
            StaffCareerFact::JobPosition { value } => value,
            _ => None,
        });
        super::staff_info_service::validate_position(tx, value, existing).await?;
    }
    Ok(())
}

async fn validate_chronology(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    input: &StaffCareerEntryInput,
    is_current: bool,
    excluded: Option<Uuid>,
) -> Result<(), AppError> {
    let Some(date) = input.effective_date else {
        return Ok(());
    };
    let latest: Option<NaiveDate> = if is_current {
        sqlx::query_scalar("SELECT max(effective_date) FROM staff_career_history WHERE user_id=$1 AND kind=$2 AND ($3::uuid IS NULL OR id<>$3)")
            .bind(staff_id).bind(input.fact.kind().as_str()).bind(excluded).fetch_one(&mut **tx).await?
    } else {
        current_entry(tx, staff_id, input.fact.kind())
            .await?
            .and_then(|entry| entry.effective_date)
    };
    if latest.is_some_and(|existing| {
        if is_current {
            date < existing
        } else {
            date > existing
        }
    }) {
        return Err(AppError::BadRequest(
            "วันที่ประวัติขัดกับวันที่ข้อมูลปัจจุบัน กรุณาตรวจและแก้ข้อมูลปัจจุบันก่อน".into(),
        ));
    }
    Ok(())
}

async fn insert_entry(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    id: Uuid,
    actor_id: Uuid,
    input: &StaffCareerEntryInput,
) -> Result<(), AppError> {
    let (personnel, position, rank) = fact_columns(&input.fact);
    sqlx::query("INSERT INTO staff_career_history(id,user_id,kind,personnel_type,job_position_id,academic_rank,effective_date,order_date,order_number,note,source,created_by,updated_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'staff_entry',$11,$11)")
        .bind(id).bind(staff_id).bind(input.fact.kind().as_str()).bind(personnel).bind(position).bind(rank)
        .bind(input.effective_date).bind(input.order_date).bind(&input.order_number).bind(&input.note).bind(actor_id).execute(&mut **tx).await.map_err(|error| {
            if error.as_database_error().is_some_and(|database| database.constraint() == Some("staff_career_history_pkey")) { conflict() } else { error.into() }
        })?;
    Ok(())
}

async fn project_current(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    id: Uuid,
    fact: &StaffCareerFact,
) -> Result<(), AppError> {
    let (personnel, position, rank) = fact_columns(fact);
    sqlx::query("UPDATE staff_info SET
        personnel_type=CASE WHEN $2='personnel_type' THEN $3 ELSE personnel_type END,
        current_personnel_type_history_id=CASE WHEN $2='personnel_type' THEN $6 ELSE current_personnel_type_history_id END,
        job_position_id=CASE WHEN $2='job_position' THEN $4 ELSE job_position_id END,
        current_job_position_history_id=CASE WHEN $2='job_position' THEN $6 ELSE current_job_position_history_id END,
        academic_rank=CASE WHEN $2='academic_rank' THEN $5 ELSE academic_rank END,
        current_academic_rank_history_id=CASE WHEN $2='academic_rank' THEN $6 ELSE current_academic_rank_history_id END WHERE user_id=$1")
        .bind(staff_id).bind(fact.kind().as_str()).bind(personnel).bind(position).bind(rank).bind(id).execute(&mut **tx).await?;
    Ok(())
}

async fn save_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor_id: Uuid,
    id: Uuid,
    action: &str,
    before: Option<&StaffCareerEntry>,
    input: &StaffCareerEntryInput,
    reason: Option<&str>,
) -> Result<(), AppError> {
    let old = serde_json::to_value(before)
        .map_err(|_| AppError::InternalServerError("ไม่สามารถเตรียมประวัติการแก้ไข".into()))?;
    let new = serde_json::to_value(input)
        .map_err(|_| AppError::InternalServerError("ไม่สามารถเตรียมประวัติการแก้ไข".into()))?;
    AuditLogBuilder::new(action, "staff_career_history")
        .user(actor_id, None, None)
        .entity(id, None)
        .old_values(old)
        .new_values(new)
        .description(reason.unwrap_or("บันทึกประวัติบุคลากร"))
        .save_in_transaction(tx)
        .await?;
    Ok(())
}

async fn correct_entry(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    actor_id: Uuid,
    before: &StaffCareerEntry,
    input: &StaffCareerEntryInput,
    reason: &str,
) -> Result<StaffCareerMutationAck, AppError> {
    if input.fact.kind() != before.fact.kind() {
        return Err(AppError::BadRequest(
            "ไม่สามารถเปลี่ยนชนิดของรายการประวัติได้".into(),
        ));
    }
    if entry_input(before) == *input {
        return Ok(StaffCareerMutationAck {
            id: before.id,
            revision: before.revision,
        });
    }
    validate_position(tx, input, Some(before)).await?;
    validate_chronology(tx, staff_id, input, before.is_current, Some(before.id)).await?;
    let (personnel, position, rank) = fact_columns(&input.fact);
    let revision: i64 = sqlx::query_scalar("UPDATE staff_career_history SET personnel_type=$2,job_position_id=$3,academic_rank=$4,effective_date=$5,order_date=$6,order_number=$7,note=$8,updated_by=$9,updated_at=NOW(),revision=revision+1 WHERE id=$1 RETURNING revision")
        .bind(before.id).bind(personnel).bind(position).bind(rank).bind(input.effective_date).bind(input.order_date)
        .bind(&input.order_number).bind(&input.note).bind(actor_id).fetch_one(&mut **tx).await?;
    if before.is_current {
        project_current(tx, staff_id, before.id, &input.fact).await?;
    }
    save_audit(
        tx,
        actor_id,
        before.id,
        "staff.career.correct",
        Some(before),
        input,
        Some(reason),
    )
    .await?;
    Ok(StaffCareerMutationAck {
        id: before.id,
        revision,
    })
}

pub async fn create_current_career(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    actor_id: Uuid,
    input: &CreateStaffCareerRequest,
) -> Result<(), AppError> {
    let today = today();
    validate_create_career(input, today)?;
    for entry in &input.entries {
        let entry = normalize_career_entry(entry, today)?;
        if empty_input(&entry) {
            continue;
        }
        if current_entry(tx, staff_id, entry.fact.kind())
            .await?
            .is_some()
        {
            return Err(conflict());
        }
        validate_position(tx, &entry, None).await?;
        let id = Uuid::new_v4();
        insert_entry(tx, staff_id, id, actor_id, &entry).await?;
        project_current(tx, staff_id, id, &entry.fact).await?;
        save_audit(
            tx,
            actor_id,
            id,
            "staff.career.create_current",
            None,
            &entry,
            None,
        )
        .await?;
    }
    Ok(())
}

pub async fn patch_current_career(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    actor_id: Uuid,
    input: &UpdateStaffCareerRequest,
) -> Result<(), AppError> {
    let today = today();
    validate_update_career(input, today)?;
    for change in &input.changes {
        let entry = normalize_career_entry(&change.entry, today)?;
        let before = current_entry(tx, staff_id, entry.fact.kind()).await?;
        let reference = before.as_ref().map(|row| StaffCareerReference {
            id: row.id,
            revision: row.revision,
        });
        if reference != change.expected_current {
            return Err(conflict());
        }
        if let Some(before) = &before {
            if entry_input(before) == entry {
                continue;
            }
            if before.fact == entry.fact {
                let reason =
                    normalize_correction_reason(change.correction_reason.as_deref().unwrap_or(""))?;
                correct_entry(tx, staff_id, actor_id, before, &entry, &reason).await?;
                continue;
            }
        } else if empty_input(&entry) {
            continue;
        }
        validate_position(tx, &entry, before.as_ref()).await?;
        validate_chronology(tx, staff_id, &entry, true, None).await?;
        let id = Uuid::new_v4();
        insert_entry(tx, staff_id, id, actor_id, &entry).await?;
        project_current(tx, staff_id, id, &entry.fact).await?;
        save_audit(
            tx,
            actor_id,
            id,
            "staff.career.change_current",
            before.as_ref(),
            &entry,
            None,
        )
        .await?;
    }
    Ok(())
}

pub async fn append_staff_career_history(
    pool: &PgPool,
    staff_id: Uuid,
    actor_id: Uuid,
    input: CreateStaffCareerHistoryRequest,
) -> Result<StaffCareerMutationAck, AppError> {
    let entry = normalize_career_entry(&input.entry, today())?;
    let mut tx = pool.begin().await?;
    lock_staff(&mut tx, staff_id).await?;
    let existing_owner: Option<Uuid> =
        sqlx::query_scalar("SELECT user_id FROM staff_career_history WHERE id=$1")
            .bind(input.id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(owner) = existing_owner {
        if owner != staff_id {
            return Err(conflict());
        }
        let previous = read_entry(&mut tx, staff_id, input.id)
            .await?
            .ok_or_else(conflict)?;
        if previous.source != StaffCareerSource::StaffEntry || entry_input(&previous) != entry {
            return Err(conflict());
        }
        tx.commit().await?;
        return Ok(StaffCareerMutationAck {
            id: previous.id,
            revision: previous.revision,
        });
    }
    validate_position(&mut tx, &entry, None).await?;
    validate_chronology(&mut tx, staff_id, &entry, false, None).await?;
    insert_entry(&mut tx, staff_id, input.id, actor_id, &entry).await?;
    save_audit(
        &mut tx,
        actor_id,
        input.id,
        "staff.career.append_history",
        None,
        &entry,
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(StaffCareerMutationAck {
        id: input.id,
        revision: 1,
    })
}

pub async fn correct_staff_career_history(
    pool: &PgPool,
    staff_id: Uuid,
    entry_id: Uuid,
    actor_id: Uuid,
    input: CorrectStaffCareerHistoryRequest,
) -> Result<StaffCareerMutationAck, AppError> {
    let entry = normalize_career_entry(&input.entry, today())?;
    let reason = normalize_correction_reason(&input.reason)?;
    let mut tx = pool.begin().await?;
    lock_staff(&mut tx, staff_id).await?;
    let before = read_entry(&mut tx, staff_id, entry_id)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการประวัติ".into()))?;
    if before.revision != input.expected_revision || before.is_current != input.expected_is_current
    {
        return Err(conflict());
    }
    let ack = correct_entry(&mut tx, staff_id, actor_id, &before, &entry, &reason).await?;
    tx.commit().await?;
    Ok(ack)
}

#[derive(Deserialize)]
struct HistoryRead {
    items: Vec<StaffCareerEntry>,
    current: StaffCareerCurrent,
}

pub(super) fn history_read_sql() -> String {
    let entry = career_json_sql("h", Some("position"), CURRENT_MATCH);
    format!("WITH target AS (SELECT id FROM users WHERE id=$1 AND user_type='staff'),
        anchor AS (SELECT effective_date,created_at,id FROM staff_career_history WHERE user_id=$1 AND id=$2),
        page_rows AS MATERIALIZED (SELECT h.id,h.user_id,h.kind,h.personnel_type,h.job_position_id,h.academic_rank,
            h.effective_date,h.order_date,h.order_number,h.note,h.source,h.revision,h.created_at,h.updated_at
            FROM staff_career_history h LEFT JOIN anchor ON true
            WHERE h.user_id=$1 AND EXISTS(SELECT 1 FROM target) AND ($2::uuid IS NULL
            OR (anchor.effective_date IS NOT NULL AND (h.effective_date<anchor.effective_date OR h.effective_date IS NULL))
            OR (h.effective_date IS NOT DISTINCT FROM anchor.effective_date AND (h.created_at,h.id)<(anchor.created_at,anchor.id)))
            ORDER BY h.effective_date DESC NULLS LAST,h.created_at DESC,h.id DESC LIMIT $3),
        page AS (SELECT h.effective_date,h.created_at,h.id,{entry} AS entry FROM page_rows h
            LEFT JOIN staff_info info ON info.user_id=h.user_id
            LEFT JOIN staff_job_positions position ON position.id=h.job_position_id),
        current_entries AS (SELECT h.kind,{entry} AS entry FROM staff_career_history h
            JOIN target ON target.id=h.user_id JOIN staff_info info ON info.user_id=h.user_id
            LEFT JOIN staff_job_positions position ON position.id=h.job_position_id WHERE {CURRENT_MATCH})
        SELECT EXISTS(SELECT 1 FROM target),($2::uuid IS NULL OR EXISTS(SELECT 1 FROM anchor)),
            jsonb_build_object('items',COALESCE((SELECT jsonb_agg(entry ORDER BY effective_date DESC NULLS LAST,created_at DESC,id DESC) FROM page),'[]'::jsonb),
            'current',jsonb_build_object(
                'personnelType',(SELECT entry FROM current_entries WHERE kind='personnel_type'),
                'jobPosition',(SELECT entry FROM current_entries WHERE kind='job_position'),
                'academicRank',(SELECT entry FROM current_entries WHERE kind='academic_rank')))")
}

pub async fn list_staff_career_history(
    pool: &PgPool,
    staff_id: Uuid,
    query: StaffCareerHistoryQuery,
) -> Result<StaffCareerHistoryPage, AppError> {
    let size = query.page_size.unwrap_or(20);
    if !(1..=50).contains(&size) {
        return Err(AppError::BadRequest(
            "จำนวนรายการต้องอยู่ระหว่าง 1 ถึง 50".into(),
        ));
    }
    let sql = history_read_sql();
    let (exists, cursor_valid, Json(mut data)): (bool, bool, Json<HistoryRead>) =
        sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(staff_id)
            .bind(query.cursor)
            .bind(size + 1)
            .fetch_one(pool)
            .await?;
    if !exists {
        return Err(AppError::NotFound("ไม่พบบุคลากร".into()));
    }
    if !cursor_valid {
        return Err(AppError::BadRequest(
            "ตำแหน่งหน้ารายการไม่ถูกต้อง กรุณาโหลดประวัติใหม่".into(),
        ));
    }
    let next_cursor = if data.items.len() > size as usize {
        data.items.truncate(size as usize);
        data.items.last().map(|entry| entry.id)
    } else {
        None
    };
    Ok(StaffCareerHistoryPage {
        items: data.items,
        current: data.current,
        next_cursor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_career_input_does_not_fabricate_history() {
        let input = StaffCareerEntryInput {
            fact: StaffCareerFact::AcademicRank { value: None },
            effective_date: None,
            order_date: None,
            order_number: None,
            note: None,
        };
        assert!(empty_input(&input));
        let dated = StaffCareerEntryInput {
            effective_date: NaiveDate::from_ymd_opt(2024, 2, 29),
            ..input
        };
        assert!(!empty_input(&dated));
    }
}
