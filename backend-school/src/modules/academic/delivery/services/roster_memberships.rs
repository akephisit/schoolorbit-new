use chrono::NaiveDate;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::delivery::models::{
    AddDatedRosterMembershipRequest, DatedRosterMembership, LearningOfferingStatus,
    MembershipStatus, RemoveDatedRosterMembershipRequest, RosterStatus,
};

use super::{append_audit, require_writable_term, validate_row_version};

#[derive(Debug, FromRow)]
struct MembershipRow {
    id: Uuid,
    learning_group_id: Uuid,
    student_academic_year_id: Uuid,
    student_id: Uuid,
    student_code: Option<String>,
    display_name: String,
    membership_status: MembershipStatus,
    roster_source: String,
    joined_at: NaiveDate,
    left_at: Option<NaiveDate>,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    row_version: i64,
}

#[derive(Debug, FromRow)]
struct GroupContext {
    id: Uuid,
    learning_offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    status: LearningOfferingStatus,
    roster_status: RosterStatus,
    row_version: i64,
    offering_status: LearningOfferingStatus,
    starts_on: NaiveDate,
    ends_on: Option<NaiveDate>,
    academic_year_start: NaiveDate,
    academic_year_end: NaiveDate,
}

const MEMBERSHIP_SELECT: &str = r#"
    SELECT membership.id, membership.learning_group_id,
           membership.student_academic_year_id, membership.student_id,
           info.student_id AS student_code,
           concat_ws(' ', nullif(btrim(student.title), ''),
                        student.first_name, student.last_name) AS display_name,
           membership.membership_status, membership.roster_source,
           membership.joined_at, membership.left_at, membership.published_at,
           membership.row_version
    FROM learning_group_students membership
    JOIN users student ON student.id = membership.student_id
    LEFT JOIN student_info info ON info.user_id = student.id
"#;

impl From<MembershipRow> for DatedRosterMembership {
    fn from(row: MembershipRow) -> Self {
        Self {
            id: row.id,
            learning_group_id: row.learning_group_id,
            student_academic_year_id: row.student_academic_year_id,
            student_id: row.student_id,
            student_code: row.student_code,
            display_name: row.display_name,
            membership_status: row.membership_status,
            roster_source: row.roster_source,
            joined_at: row.joined_at,
            left_at: row.left_at,
            published_at: row.published_at,
            row_version: row.row_version,
        }
    }
}

pub async fn list_memberships(
    pool: &PgPool,
    group_id: Uuid,
) -> Result<Vec<DatedRosterMembership>, AppError> {
    let group_exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM learning_groups WHERE id = $1)")
            .bind(group_id)
            .fetch_one(pool)
            .await?;
    if !group_exists {
        return Err(AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()));
    }
    let query = format!(
        "{MEMBERSHIP_SELECT} WHERE membership.learning_group_id = $1 \
         ORDER BY student.first_name, student.last_name, membership.joined_at, membership.id"
    );
    let rows = sqlx::query_as::<_, MembershipRow>(&query)
        .bind(group_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn add_membership(
    pool: &PgPool,
    actor_user_id: Uuid,
    group_id: Uuid,
    request: AddDatedRosterMembershipRequest,
) -> Result<DatedRosterMembership, AppError> {
    validate_row_version(request.group_row_version)?;
    let mut transaction = pool.begin().await?;
    let academic_term_id = find_group_term(&mut transaction, group_id).await?;
    require_writable_term(&mut transaction, academic_term_id, true).await?;
    let group = lock_group_context(&mut transaction, group_id).await?;
    require_published_roster(&group, request.group_row_version)?;

    let (student_id, student_year_status): (Uuid, String) = sqlx::query_as(
        r#"SELECT student_id, status
           FROM student_academic_years
           WHERE id = $1 AND academic_year_id = $2
           FOR SHARE"#,
    )
    .bind(request.student_academic_year_id)
    .bind(group.academic_year_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::ValidationError("นักเรียนไม่ได้อยู่ในปีการศึกษาของกลุ่มเรียนนี้".to_string()))?;
    if !matches!(student_year_status.as_str(), "active" | "planned") {
        return Err(AppError::ValidationError(
            "สถานะนักเรียนในปีการศึกษานี้ไม่พร้อมเข้ากลุ่มเรียน".to_string(),
        ));
    }
    validate_membership_date(&group, request.joined_at)?;

    let overlaps: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1
               FROM learning_group_students membership
               WHERE membership.learning_group_id = $1
                 AND membership.student_id = $2
                 AND daterange(membership.joined_at, membership.left_at, '[]')
                     && daterange($3, NULL, '[]')
           )"#,
    )
    .bind(group_id)
    .bind(student_id)
    .bind(request.joined_at)
    .fetch_one(&mut *transaction)
    .await?;
    if overlaps {
        return Err(AppError::Conflict(
            "วันที่เริ่มใหม่ซ้อนกับประวัติการอยู่ในกลุ่มเดิม ต้องเริ่มหลังวันสิ้นสุดเดิมอย่างน้อย 1 วัน".to_string(),
        ));
    }

    let membership_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO learning_group_students (
               id, learning_group_id, academic_term_id, academic_year_id,
               student_academic_year_id, student_id, membership_status,
               roster_source, joined_at, published_at
           ) VALUES ($1, $2, $3, $4, $5, $6, 'active',
                     'operational_change', $7, now())"#,
    )
    .bind(membership_id)
    .bind(group.id)
    .bind(group.academic_term_id)
    .bind(group.academic_year_id)
    .bind(request.student_academic_year_id)
    .bind(student_id)
    .bind(request.joined_at)
    .execute(&mut *transaction)
    .await
    .map_err(map_membership_overlap)?;
    increment_group_revision(&mut transaction, group.id).await?;
    transaction.commit().await?;

    append_audit(
        pool,
        "learning_group.membership_added",
        "learning_group_student",
        membership_id,
        group.academic_year_id,
        group.academic_term_id,
        actor_user_id,
        serde_json::json!({
            "learningGroupId": group.id,
            "learningOfferingId": group.learning_offering_id,
            "joinedAt": request.joined_at,
            "groupRowVersion": request.group_row_version,
        }),
    )
    .await?;
    get_membership(pool, membership_id).await
}

pub async fn remove_membership(
    pool: &PgPool,
    actor_user_id: Uuid,
    group_id: Uuid,
    membership_id: Uuid,
    request: RemoveDatedRosterMembershipRequest,
) -> Result<DatedRosterMembership, AppError> {
    validate_row_version(request.group_row_version)?;
    validate_row_version(request.membership_row_version)?;
    let mut transaction = pool.begin().await?;
    let academic_term_id = find_group_term(&mut transaction, group_id).await?;
    require_writable_term(&mut transaction, academic_term_id, true).await?;
    let group = lock_group_context(&mut transaction, group_id).await?;
    require_published_roster(&group, request.group_row_version)?;
    validate_membership_date(&group, request.left_at)?;

    let (status, joined_at, row_version): (MembershipStatus, NaiveDate, i64) = sqlx::query_as(
        r#"SELECT membership_status, joined_at, row_version
           FROM learning_group_students
           WHERE id = $1 AND learning_group_id = $2
           FOR UPDATE"#,
    )
    .bind(membership_id)
    .bind(group_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบช่วงรายชื่อนักเรียน".to_string()))?;
    if status != MembershipStatus::Active {
        return Err(AppError::Conflict(
            "ช่วงรายชื่อนี้สิ้นสุดแล้ว ไม่สามารถกำหนดวันสิ้นสุดซ้ำ".to_string(),
        ));
    }
    if row_version != request.membership_row_version {
        return Err(AppError::Conflict(
            "ช่วงรายชื่อนักเรียนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    if request.left_at < joined_at {
        return Err(AppError::ValidationError(
            "วันสิ้นสุดต้องไม่ก่อนวันที่เริ่มอยู่ในกลุ่ม".to_string(),
        ));
    }

    sqlx::query(
        r#"UPDATE learning_group_students
           SET membership_status = 'ended', left_at = $1,
               row_version = row_version + 1, updated_at = now()
           WHERE id = $2"#,
    )
    .bind(request.left_at)
    .bind(membership_id)
    .execute(&mut *transaction)
    .await?;
    increment_group_revision(&mut transaction, group.id).await?;
    transaction.commit().await?;

    append_audit(
        pool,
        "learning_group.membership_ended",
        "learning_group_student",
        membership_id,
        group.academic_year_id,
        group.academic_term_id,
        actor_user_id,
        serde_json::json!({
            "learningGroupId": group.id,
            "learningOfferingId": group.learning_offering_id,
            "leftAtInclusive": request.left_at,
            "groupRowVersion": request.group_row_version,
            "membershipRowVersion": request.membership_row_version,
        }),
    )
    .await?;
    get_membership(pool, membership_id).await
}

async fn find_group_term(
    transaction: &mut Transaction<'_, Postgres>,
    group_id: Uuid,
) -> Result<Uuid, AppError> {
    sqlx::query_scalar("SELECT academic_term_id FROM learning_groups WHERE id = $1")
        .bind(group_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()))
}

async fn lock_group_context(
    transaction: &mut Transaction<'_, Postgres>,
    group_id: Uuid,
) -> Result<GroupContext, AppError> {
    sqlx::query_as(
        r#"SELECT learning_group.id, learning_group.learning_offering_id,
                  learning_group.academic_term_id, learning_group.academic_year_id,
                  learning_group.status, learning_group.roster_status,
                  learning_group.row_version, offering.status AS offering_status,
                  offering.starts_on, offering.ends_on,
                  year.start_date AS academic_year_start,
                  year.end_date AS academic_year_end
           FROM learning_groups learning_group
           JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
           JOIN academic_years year ON year.id = learning_group.academic_year_id
           WHERE learning_group.id = $1
           FOR UPDATE OF learning_group, offering"#,
    )
    .bind(group_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()))
}

fn require_published_roster(group: &GroupContext, row_version: i64) -> Result<(), AppError> {
    if group.row_version != row_version {
        return Err(AppError::Conflict("กลุ่มเรียนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()));
    }
    if group.status != LearningOfferingStatus::Published
        || group.offering_status != LearningOfferingStatus::Published
        || group.roster_status != RosterStatus::Published
    {
        return Err(AppError::Conflict(
            "การเพิ่มหรือสิ้นสุดตามวันที่ใช้ได้หลังเผยแพร่รายการ กลุ่ม และ roster แล้วเท่านั้น".to_string(),
        ));
    }
    Ok(())
}

fn validate_membership_date(group: &GroupContext, date: NaiveDate) -> Result<(), AppError> {
    if date < group.academic_year_start || date > group.academic_year_end {
        return Err(AppError::ValidationError(
            "วันที่เปลี่ยนรายชื่อต้องอยู่ในปีการศึกษาของนักเรียน".to_string(),
        ));
    }
    if date < group.starts_on || group.ends_on.is_some_and(|end| date > end) {
        return Err(AppError::ValidationError(
            "วันที่เปลี่ยนรายชื่อต้องอยู่ในช่วงที่รายการเปิดสอนใช้งาน".to_string(),
        ));
    }
    Ok(())
}

async fn increment_group_revision(
    transaction: &mut Transaction<'_, Postgres>,
    group_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE learning_groups SET row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(group_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn get_membership(
    pool: &PgPool,
    membership_id: Uuid,
) -> Result<DatedRosterMembership, AppError> {
    let query = format!("{MEMBERSHIP_SELECT} WHERE membership.id = $1");
    let row = sqlx::query_as::<_, MembershipRow>(&query)
        .bind(membership_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบช่วงรายชื่อนักเรียน".to_string()))?;
    Ok(row.into())
}

fn map_membership_overlap(error: sqlx::Error) -> AppError {
    if error
        .to_string()
        .contains("ACADEMIC_ROSTER_MEMBERSHIP_INTERVAL_OVERLAP")
    {
        AppError::Conflict(
            "วันที่เริ่มใหม่ซ้อนกับประวัติการอยู่ในกลุ่มเดิม ต้องเริ่มหลังวันสิ้นสุดเดิมอย่างน้อย 1 วัน".to_string(),
        )
    } else {
        AppError::DbError(error)
    }
}
