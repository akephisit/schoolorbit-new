use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::models::OrganizationMemberItem;
use crate::services::organization_unit_service;

fn organization_member_display_name(name: Option<String>) -> String {
    name.unwrap_or_default()
}

pub async fn list_members(
    pool: &PgPool,
    organization_unit_id: Uuid,
    include_children: bool,
) -> Result<Vec<OrganizationMemberItem>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.services.organization_member_service.list_members",
    )
    .await?;
    list_members_on_connection(&mut connection, organization_unit_id, include_children).await
}

pub async fn list_members_on_connection(
    connection: &mut sqlx::PgConnection,
    organization_unit_id: Uuid,
    include_children: bool,
) -> Result<Vec<OrganizationMemberItem>, AppError> {
    let rows = DbSession::new(connection).query("staff.services.organization_member_service.list_members_on_connection",
        r#"WITH RECURSIVE descendants AS (
            SELECT id FROM organization_units WHERE id = $1 AND is_active
            UNION
            SELECT child.id FROM organization_units child
            JOIN descendants parent ON child.parent_unit_id = parent.id
            WHERE $2 AND child.is_active AND child.unit_type <> 'subject_group'
        )
        SELECT om.user_id, om.organization_unit_id, ou.name AS organization_unit_name,
               u.display_name AS name, COALESCE(u.title, '') AS title,
               om.position_code, om.position_title, om.is_primary, om.responsibilities, om.started_at
        FROM descendants d
        JOIN organization_members om ON om.organization_unit_id = d.id
        JOIN user_display_names u ON u.id = om.user_id AND u.user_type = 'staff' AND u.status = 'active'
        JOIN organization_units ou ON ou.id = d.id
        WHERE om.started_at <= CURRENT_DATE AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)
        ORDER BY (om.organization_unit_id = $1) DESC,
            CASE om.position_code WHEN 'director' THEN 1 WHEN 'deputy_director' THEN 2
                WHEN 'head' THEN 3 WHEN 'deputy_head' THEN 4 WHEN 'coordinator' THEN 5 ELSE 6 END,
            ou.display_order, ou.name, u.first_name, u.last_name, om.user_id, om.id"#,
    )
    .bind(organization_unit_id)
    .bind(include_children)
    .fetch_all()
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| OrganizationMemberItem {
            user_id: r.get("user_id"),
            organization_unit_id: r.get("organization_unit_id"),
            organization_unit_name: r.get("organization_unit_name"),
            name: organization_member_display_name(r.get("name")),
            title: r.get("title"),
            position_code: r.get("position_code"),
            position_title: r.get("position_title"),
            is_primary: r.get("is_primary"),
            responsibilities: r.get("responsibilities"),
            started_at: r.get("started_at"),
        })
        .collect())
}

pub async fn already_member(
    pool: &PgPool,
    user_id: Uuid,
    organization_unit_id: Uuid,
) -> Result<bool, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.services.organization_member_service.already_member",
    )
    .await?;
    already_member_on_connection(&mut connection, user_id, organization_unit_id).await
}

pub async fn already_member_on_connection(
    connection: &mut sqlx::PgConnection,
    user_id: Uuid,
    organization_unit_id: Uuid,
) -> Result<bool, AppError> {
    let r: bool = DbSession::new(connection).scalar("staff.services.organization_member_service.already_member_on_connection",
        "SELECT EXISTS(
             SELECT 1 FROM organization_members
             WHERE user_id = $1 AND organization_unit_id = $2
               AND (ended_at IS NULL OR ended_at > CURRENT_DATE) AND started_at < coalesce(ended_at, 'infinity'::date)
         )",
    )
    .bind(user_id)
    .bind(organization_unit_id)
    .fetch_one()
    .await?;
    Ok(r)
}

pub async fn add_member(
    pool: &PgPool,
    user_id: Uuid,
    organization_unit_id: Uuid,
    position_code: &str,
    position_title: Option<String>,
    is_primary: bool,
    responsibilities: Option<String>,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    lock_member_user(&mut tx, user_id).await?;
    organization_unit_service::ensure_organization_unit_active(&mut tx, organization_unit_id)
        .await?;

    if already_member_in_transaction(&mut tx, user_id, organization_unit_id).await? {
        return Err(AppError::Conflict("บุคลากรนี้เป็นสมาชิกของหน่วยงานนี้อยู่แล้ว".into()));
    }
    if is_primary {
        clear_primary_in_domain(&mut tx, user_id, organization_unit_id, None, None).await?;
    }
    DbSession::new(&mut *tx).query("staff.services.organization_member_service.add_member",
        "INSERT INTO organization_members
            (
                user_id, organization_unit_id, position_code, position_title,
                is_primary, responsibilities, started_at
            )
         VALUES ($1, $2, $3, $4, $5, $6, CURRENT_DATE)
         ON CONFLICT(user_id, organization_unit_id, started_at) DO UPDATE SET
            position_code=EXCLUDED.position_code, position_title=EXCLUDED.position_title,
            is_primary=EXCLUDED.is_primary, responsibilities=EXCLUDED.responsibilities, ended_at=NULL",
    )
    .bind(user_id)
    .bind(organization_unit_id)
    .bind(position_code)
    .bind(position_title)
    .bind(is_primary)
    .bind(responsibilities)
    .execute()
    .await?;

    tx.commit().await?;
    Ok(())
}

pub struct UpdateMemberInput {
    pub organization_unit_id: Uuid,
    pub user_id: Uuid,
    pub position_code: String,
    pub position_title: Option<String>,
    pub is_primary: bool,
    pub responsibilities: Option<String>,
    pub new_organization_unit_id: Uuid,
}

pub async fn update_member(pool: &PgPool, input: UpdateMemberInput) -> Result<u64, AppError> {
    let mut tx = pool.begin().await?;
    lock_member_user(&mut tx, input.user_id).await?;
    if input.new_organization_unit_id != input.organization_unit_id {
        organization_unit_service::ensure_organization_unit_active(
            &mut tx,
            input.new_organization_unit_id,
        )
        .await?;
    }

    if input.new_organization_unit_id != input.organization_unit_id
        && already_member_in_transaction(&mut tx, input.user_id, input.new_organization_unit_id)
            .await?
    {
        return Err(AppError::Conflict(
            "บุคลากรนี้เป็นสมาชิกของหน่วยงานปลายทางอยู่แล้ว".into(),
        ));
    }
    let sources: Vec<(Uuid, NaiveDate, Option<NaiveDate>)> = DbSession::new(&mut *tx)
        .query_as(
            "staff.services.organization_member_service.update_member",
            "SELECT id, started_at, ended_at FROM organization_members
         WHERE user_id = $1 AND organization_unit_id = $2
           AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
           AND started_at < coalesce(ended_at, 'infinity'::date)
         FOR UPDATE",
        )
        .bind(input.user_id)
        .bind(input.organization_unit_id)
        .fetch_all()
        .await?;
    let Some((source_id, source_start, source_end)) = sources.first().copied() else {
        return Ok(0);
    };
    if sources.len() > 1 {
        return Err(AppError::Conflict(
            "มีการมอบหมายหลายช่วงเวลาในหน่วยงานเดียว กรุณาตรวจสอบก่อนแก้ไข".into(),
        ));
    }
    if input.new_organization_unit_id != input.organization_unit_id {
        // Moving changes the source from today onward, retaining any planned start/end.
        let target_start: NaiveDate = DbSession::new(&mut *tx)
            .scalar(
                "staff.services.organization_member_service.update_member",
                "SELECT greatest($1::date, CURRENT_DATE)",
            )
            .bind(source_start)
            .fetch_one()
            .await?;
        DbSession::new(&mut *tx)
            .query(
                "staff.services.organization_member_service.update_member",
                "UPDATE organization_members SET ended_at = $2 WHERE id = $1",
            )
            .bind(source_id)
            .bind(target_start)
            .execute()
            .await?;
        if input.is_primary {
            clear_primary_in_domain(
                &mut tx,
                input.user_id,
                input.new_organization_unit_id,
                Some(target_start),
                source_end,
            )
            .await?;
        }
        DbSession::new(&mut *tx)
            .query(
                "staff.services.organization_member_service.update_member",
                "INSERT INTO organization_members (
                user_id, organization_unit_id, position_code, position_title,
                is_primary, responsibilities, started_at, ended_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT(user_id, organization_unit_id, started_at) DO UPDATE SET
                position_code = EXCLUDED.position_code, position_title = EXCLUDED.position_title,
                is_primary = EXCLUDED.is_primary, responsibilities = EXCLUDED.responsibilities,
                ended_at = EXCLUDED.ended_at",
            )
            .bind(input.user_id)
            .bind(input.new_organization_unit_id)
            .bind(input.position_code)
            .bind(input.position_title)
            .bind(input.is_primary)
            .bind(input.responsibilities)
            .bind(target_start)
            .bind(source_end)
            .execute()
            .await?;
        tx.commit().await?;
        return Ok(1);
    }
    if input.is_primary {
        clear_primary_in_domain(
            &mut tx,
            input.user_id,
            input.new_organization_unit_id,
            Some(source_start),
            source_end,
        )
        .await?;
    }
    let result = DbSession::new(&mut *tx).query("staff.services.organization_member_service.update_member",
        r#"UPDATE organization_members
           SET position_code = $1,
               position_title = $2,
               is_primary = $3,
               responsibilities = $4,
               organization_unit_id = $5
           WHERE user_id = $6 AND organization_unit_id = $7
             AND (ended_at IS NULL OR ended_at > CURRENT_DATE) AND started_at < coalesce(ended_at, 'infinity'::date)"#,
    )
    .bind(input.position_code)
    .bind(input.position_title)
    .bind(input.is_primary)
    .bind(input.responsibilities)
    .bind(input.new_organization_unit_id)
    .bind(input.user_id)
    .bind(input.organization_unit_id)
    .execute()
    .await?;
    let rows_affected = result.rows_affected();
    tx.commit().await?;
    Ok(rows_affected)
}

pub async fn remove_member(
    pool: &PgPool,
    organization_unit_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    if !try_lock_member_user(&mut tx, user_id).await? {
        return Ok(());
    }
    DbSession::new(&mut *tx).query("staff.services.organization_member_service.remove_member",
        "UPDATE organization_members SET ended_at = greatest(started_at, CURRENT_DATE)
         WHERE user_id = $1 AND organization_unit_id = $2
           AND (ended_at IS NULL OR ended_at > CURRENT_DATE) AND started_at < coalesce(ended_at, 'infinity'::date)",
    )
    .bind(user_id)
    .bind(organization_unit_id)
    .execute()
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn try_lock_member_user(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<bool, AppError> {
    let found: Option<Uuid> = DbSession::new(&mut **tx)
        .scalar(
            "staff.services.organization_member_service.try_lock_member_user",
            "SELECT id FROM users WHERE id=$1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional()
        .await?;
    Ok(found.is_some())
}

pub(super) async fn lock_member_user(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<(), AppError> {
    if !try_lock_member_user(tx, user_id).await? {
        return Err(AppError::NotFound("ไม่พบบุคลากร".into()));
    }
    Ok(())
}

async fn already_member_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    unit_id: Uuid,
) -> Result<bool, AppError> {
    Ok(DbSession::new(&mut **tx)
        .scalar(
            "staff.services.organization_member_service.already_member_in_transaction",
            r#"SELECT EXISTS (
        SELECT 1 FROM organization_members WHERE user_id = $1 AND organization_unit_id = $2
          AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
          AND started_at < coalesce(ended_at, 'infinity'::date)
    )"#,
        )
        .bind(user_id)
        .bind(unit_id)
        .fetch_one()
        .await?)
}

async fn clear_primary_in_domain(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    unit_id: Uuid,
    started_at: Option<NaiveDate>,
    ended_at: Option<NaiveDate>,
) -> Result<(), AppError> {
    DbSession::new(&mut **tx).query("staff.services.organization_member_service.clear_primary_in_domain", r#"UPDATE organization_members m SET is_primary = false
        FROM organization_units existing_unit, organization_units selected_unit
        WHERE m.organization_unit_id = existing_unit.id AND selected_unit.id = $2
          AND m.user_id = $1 AND m.is_primary IS TRUE
          AND (m.ended_at IS NULL OR m.ended_at > CURRENT_DATE)
          AND m.started_at < coalesce(m.ended_at, 'infinity'::date)
          AND m.started_at < coalesce($4::date, 'infinity'::date)
          AND coalesce($3::date, CURRENT_DATE) < coalesce(m.ended_at, 'infinity'::date)
          AND (existing_unit.unit_type = 'subject_group') = (selected_unit.unit_type = 'subject_group')"#)
        .bind(user_id).bind(unit_id).bind(started_at).bind(ended_at).execute().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organization_member_display_name_defaults_missing_names_to_empty_string() {
        assert_eq!(organization_member_display_name(None), "");
        assert_eq!(
            organization_member_display_name(Some("ครูสมหญิง ใจดี".to_string())),
            "ครูสมหญิง ใจดี"
        );
    }
}
