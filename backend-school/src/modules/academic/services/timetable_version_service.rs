use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::timetable_version::{
    CloneTimetableVersionRequest, TimetableVersion, TimetableVersionDisplayState,
    TimetableVersionStatus, TimetableVersionTarget,
};

#[derive(Debug, Clone, FromRow)]
struct TimetableVersionRow {
    id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    effective_from: NaiveDate,
    effective_until: Option<NaiveDate>,
    status: TimetableVersionStatus,
    source_version_id: Option<Uuid>,
    change_set_id: Option<Uuid>,
    bell_schedule_id: Uuid,
    row_version: i64,
    created_by: Option<Uuid>,
    published_by: Option<Uuid>,
    published_at: Option<chrono::DateTime<Utc>>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
struct TimetableVersionTargetRow {
    timetable_version_id: Uuid,
    learning_offering_id: Uuid,
    weekly_period_target: i32,
    standard_periods_per_week: Option<i32>,
}

#[derive(Debug, Clone, FromRow)]
struct CloneSourceRow {
    id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    status: TimetableVersionStatus,
    row_version: i64,
    term_status: String,
    term_start_date: NaiveDate,
    academic_year_end_date: NaiveDate,
    bell_schedule_id: Uuid,
}

const VERSION_SELECT: &str = r#"
    SELECT version.id,
           version.academic_term_id,
           version.academic_year_id,
           version.effective_from,
           CASE
               WHEN version.status = 'published' THEN COALESCE(
                   (
                       SELECT next_version.effective_from - 1
                       FROM academic_timetable_versions next_version
                       WHERE next_version.academic_term_id = version.academic_term_id
                         AND next_version.status = 'published'
                         AND next_version.effective_from > version.effective_from
                       ORDER BY next_version.effective_from, next_version.id
                       LIMIT 1
                   ),
                   term.closed_on
               )
               ELSE NULL
           END AS effective_until,
           version.status,
           version.source_version_id,
           version.change_set_id,
           version.bell_schedule_id,
           version.row_version,
           version.created_by,
           version.published_by,
           version.published_at,
           version.created_at,
           version.updated_at
    FROM academic_timetable_versions version
    JOIN academic_terms term ON term.id = version.academic_term_id
"#;

pub(crate) fn derive_display_state(
    effective_from: NaiveDate,
    effective_until: Option<NaiveDate>,
    today: NaiveDate,
) -> TimetableVersionDisplayState {
    if today < effective_from {
        TimetableVersionDisplayState::Upcoming
    } else if effective_until.is_some_and(|end| today > end) {
        TimetableVersionDisplayState::Historical
    } else {
        TimetableVersionDisplayState::Current
    }
}

pub async fn list_versions(
    pool: &PgPool,
    term_id: Uuid,
) -> Result<Vec<TimetableVersion>, AppError> {
    let sql = format!(
        "{VERSION_SELECT} WHERE version.academic_term_id = $1 \
         ORDER BY version.effective_from DESC, \
                  CASE version.status WHEN 'draft' THEN 0 WHEN 'published' THEN 1 ELSE 2 END, \
                  version.id"
    );
    let rows = sqlx::query_as::<_, TimetableVersionRow>(&sql)
        .bind(term_id)
        .fetch_all(pool)
        .await?;
    hydrate_versions(pool, rows, Utc::now().date_naive()).await
}

pub async fn resolve_for_date(
    pool: &PgPool,
    term_id: Uuid,
    on_date: NaiveDate,
) -> Result<TimetableVersion, AppError> {
    let version_id: Uuid = sqlx::query_scalar(
        r#"SELECT id
           FROM academic_timetable_versions
           WHERE academic_term_id = $1
             AND status = 'published'
             AND effective_from <= $2
           ORDER BY effective_from DESC, id
           LIMIT 1"#,
    )
    .bind(term_id)
    .bind(on_date)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("ไม่พบตารางเรียนที่เผยแพร่และมีผลในวันที่ {on_date}")))?;

    let sql = format!("{VERSION_SELECT} WHERE version.id = $1");
    let row = sqlx::query_as::<_, TimetableVersionRow>(&sql)
        .bind(version_id)
        .fetch_one(pool)
        .await?;
    let mut versions = hydrate_versions(pool, vec![row], on_date).await?;
    versions.pop().ok_or_else(|| {
        AppError::InternalServerError("ไม่สามารถโหลดตารางเรียนตามวันที่เลือกได้".to_string())
    })
}

pub async fn clone_draft(
    pool: &PgPool,
    actor_id: Uuid,
    source_id: Uuid,
    request: CloneTimetableVersionRequest,
) -> Result<TimetableVersion, AppError> {
    let mut transaction = pool.begin().await?;
    let new_version_id = clone_draft_in_transaction(
        &mut transaction,
        actor_id,
        source_id,
        request.source_row_version,
        request.effective_from,
        None,
    )
    .await?;
    transaction.commit().await?;
    get_version(pool, new_version_id, Utc::now().date_naive()).await
}

pub(crate) async fn clone_draft_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    actor_id: Uuid,
    source_id: Uuid,
    source_row_version: i64,
    effective_from: NaiveDate,
    change_set_id: Option<Uuid>,
) -> Result<Uuid, AppError> {
    if source_row_version <= 0 {
        return Err(AppError::ValidationError(
            "sourceRowVersion ต้องมากกว่าศูนย์".to_string(),
        ));
    }

    let source: CloneSourceRow = sqlx::query_as(
        r#"SELECT source.id,
                  source.academic_term_id,
                  source.academic_year_id,
                  source.status,
                  source.row_version,
                  term.status AS term_status,
                  term.start_date AS term_start_date,
                  year.end_date AS academic_year_end_date,
                  term.bell_schedule_id
           FROM academic_timetable_versions source
           JOIN academic_terms term ON term.id = source.academic_term_id
           JOIN academic_years year ON year.id = source.academic_year_id
           WHERE source.id = $1
           FOR UPDATE OF source, term"#,
    )
    .bind(source_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางเรียนต้นทาง".to_string()))?;

    if source.status != TimetableVersionStatus::Published {
        return Err(AppError::Conflict(
            "สร้างแบบร่างใหม่ได้จากรุ่นตารางเรียนที่เผยแพร่แล้วเท่านั้น".to_string(),
        ));
    }
    if source.row_version != source_row_version {
        return Err(AppError::Conflict(format!(
            "รุ่นตารางเรียนต้นทางถูกแก้ไขแล้ว (expected {}, actual {})",
            source_row_version, source.row_version
        )));
    }
    if matches!(
        source.term_status.as_str(),
        "closing" | "closed" | "cancelled"
    ) {
        return Err(AppError::Conflict(
            "ภาคเรียนนี้ปิดรับการสร้างรุ่นตารางเรียนใหม่แล้ว".to_string(),
        ));
    }
    if effective_from < source.term_start_date || effective_from > source.academic_year_end_date {
        return Err(AppError::ValidationError(
            "วันที่เริ่มใช้ตารางต้องอยู่ตั้งแต่วันเปิดภาคเรียนถึงวันสิ้นสุดปีการศึกษา".to_string(),
        ));
    }

    let duplicate: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1
               FROM academic_timetable_versions
               WHERE academic_term_id = $1
                 AND effective_from = $2
                 AND status IN ('draft', 'published')
           )"#,
    )
    .bind(source.academic_term_id)
    .bind(effective_from)
    .fetch_one(&mut **transaction)
    .await?;
    if duplicate {
        return Err(AppError::Conflict(
            "มีรุ่นตารางเรียนแบบร่างหรือเผยแพร่ในวันที่นี้แล้ว".to_string(),
        ));
    }

    let new_version_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_timetable_versions (
               id, academic_term_id, academic_year_id, effective_from, status,
               source_version_id, change_set_id, bell_schedule_id, created_by
           ) VALUES ($1, $2, $3, $4, 'draft', $5, $6, $7, $8)"#,
    )
    .bind(new_version_id)
    .bind(source.academic_term_id)
    .bind(source.academic_year_id)
    .bind(effective_from)
    .bind(source.id)
    .bind(change_set_id)
    .bind(source.bell_schedule_id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_clone_write_error)?;

    sqlx::query(
        r#"INSERT INTO academic_timetable_version_targets (
               timetable_version_id, learning_offering_id, academic_term_id,
               academic_year_id, weekly_period_target, migration_provenance
           )
           SELECT $1, target.learning_offering_id, target.academic_term_id,
                  target.academic_year_id, target.weekly_period_target,
                  target.migration_provenance || jsonb_build_object(
                      'clonedFromVersionId', $2::text
                  )
           FROM academic_timetable_version_targets target
           WHERE target.timetable_version_id = $2"#,
    )
    .bind(new_version_id)
    .bind(source.id)
    .execute(&mut **transaction)
    .await?;

    sqlx::query(
        r#"WITH source_entries AS MATERIALIZED (
               SELECT entry.*,
                      gen_random_uuid() AS new_entry_id,
                      CASE
                          WHEN entry.batch_id IS NULL THEN NULL
                          ELSE uuid_generate_v5(
                              $1,
                              'batch:' || entry.batch_id::text
                          )
                      END AS new_batch_id
               FROM academic_timetable_entries entry
               WHERE entry.timetable_version_id = $2
                 AND entry.is_active
           ), inserted_entries AS (
               INSERT INTO academic_timetable_entries (
                   id, day_of_week, bell_schedule_period_id, room_id, note,
                   is_active, created_by, updated_by, entry_type, title,
                   homeroom_id, academic_term_id, batch_id, academic_year_id,
                   learning_offering_id, learning_group_id, bell_schedule_id,
                   migration_provenance, row_version, timetable_version_id,
                   created_at, updated_at
               )
               SELECT source.new_entry_id,
                      source.day_of_week,
                      source.bell_schedule_period_id,
                      source.room_id,
                      source.note,
                      true,
                      $3,
                      $3,
                      source.entry_type,
                      source.title,
                      source.homeroom_id,
                      source.academic_term_id,
                      source.new_batch_id,
                      source.academic_year_id,
                      source.learning_offering_id,
                      source.learning_group_id,
                      source.bell_schedule_id,
                      source.migration_provenance || jsonb_build_object(
                          'clonedFromEntryId', source.id::text,
                          'sourceVersionId', $2::text
                      ),
                      1,
                      $1,
                      now(),
                      now()
               FROM source_entries source
               RETURNING id
           )
           INSERT INTO timetable_entry_instructors (
               id, entry_id, instructor_id, role
           )
           SELECT gen_random_uuid(),
                  source.new_entry_id,
                  instructor.instructor_id,
                  instructor.role
           FROM source_entries source
           JOIN inserted_entries inserted ON inserted.id = source.new_entry_id
           JOIN timetable_entry_instructors instructor ON instructor.entry_id = source.id"#,
    )
    .bind(new_version_id)
    .bind(source.id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    Ok(new_version_id)
}

async fn get_version(
    pool: &PgPool,
    version_id: Uuid,
    display_date: NaiveDate,
) -> Result<TimetableVersion, AppError> {
    let sql = format!("{VERSION_SELECT} WHERE version.id = $1");
    let row = sqlx::query_as::<_, TimetableVersionRow>(&sql)
        .bind(version_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางเรียน".to_string()))?;
    let mut versions = hydrate_versions(pool, vec![row], display_date).await?;
    versions
        .pop()
        .ok_or_else(|| AppError::InternalServerError("ไม่สามารถโหลดรุ่นตารางเรียนได้".to_string()))
}

async fn hydrate_versions(
    pool: &PgPool,
    rows: Vec<TimetableVersionRow>,
    display_date: NaiveDate,
) -> Result<Vec<TimetableVersion>, AppError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let version_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let target_rows: Vec<TimetableVersionTargetRow> = sqlx::query_as(
        r#"SELECT target.timetable_version_id,
                  target.learning_offering_id,
                  target.weekly_period_target,
                  subject_version.periods_per_week AS standard_periods_per_week
           FROM academic_timetable_version_targets target
           LEFT JOIN course_offering_details course_detail
             ON course_detail.learning_offering_id = target.learning_offering_id
           LEFT JOIN subject_versions subject_version
             ON subject_version.id = course_detail.subject_version_id
           WHERE target.timetable_version_id = ANY($1)
           ORDER BY target.timetable_version_id, target.learning_offering_id"#,
    )
    .bind(&version_ids)
    .fetch_all(pool)
    .await?;
    let mut targets_by_version: HashMap<Uuid, Vec<TimetableVersionTarget>> = HashMap::new();
    for target in target_rows {
        targets_by_version
            .entry(target.timetable_version_id)
            .or_default()
            .push(TimetableVersionTarget {
                timetable_version_id: target.timetable_version_id,
                learning_offering_id: target.learning_offering_id,
                weekly_period_target: target.weekly_period_target,
                standard_periods_per_week: target.standard_periods_per_week,
            });
    }

    Ok(rows
        .into_iter()
        .map(|row| {
            let display_state = (row.status == TimetableVersionStatus::Published).then(|| {
                derive_display_state(row.effective_from, row.effective_until, display_date)
            });
            TimetableVersion {
                id: row.id,
                academic_term_id: row.academic_term_id,
                academic_year_id: row.academic_year_id,
                effective_from: row.effective_from,
                effective_until: row.effective_until,
                status: row.status,
                display_state,
                source_version_id: row.source_version_id,
                change_set_id: row.change_set_id,
                bell_schedule_id: row.bell_schedule_id,
                row_version: row.row_version,
                created_by: row.created_by,
                published_by: row.published_by,
                published_at: row.published_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
                targets: targets_by_version.remove(&row.id).unwrap_or_default(),
            }
        })
        .collect())
}

fn map_clone_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("academic_timetable_versions_live_effective_key") {
            return AppError::Conflict("มีรุ่นตารางเรียนแบบร่างหรือเผยแพร่ในวันที่นี้แล้ว".to_string());
        }
    }
    AppError::DbError(error)
}

#[cfg(test)]
mod tests {
    use super::derive_display_state;
    use crate::modules::academic::models::timetable_version::TimetableVersionDisplayState;
    use chrono::NaiveDate;

    #[test]
    fn display_state_uses_the_effective_interval() {
        let today = NaiveDate::from_ymd_opt(2027, 6, 15).unwrap();

        assert_eq!(
            derive_display_state(NaiveDate::from_ymd_opt(2027, 7, 1).unwrap(), None, today,),
            TimetableVersionDisplayState::Upcoming
        );
        assert_eq!(
            derive_display_state(
                NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
                Some(NaiveDate::from_ymd_opt(2027, 6, 30).unwrap()),
                today,
            ),
            TimetableVersionDisplayState::Current
        );
        assert_eq!(
            derive_display_state(
                NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
                Some(NaiveDate::from_ymd_opt(2027, 6, 14).unwrap()),
                today,
            ),
            TimetableVersionDisplayState::Historical
        );
    }
}
