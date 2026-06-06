use crate::error::AppError;
use crate::modules::facility::models::{
    Building, CreateBuildingRequest, CreateRoomRequest, Room, RoomFilter, UpdateBuildingRequest,
    UpdateRoomRequest,
};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_buildings(pool: &PgPool) -> Result<Vec<Building>, AppError> {
    sqlx::query_as::<_, Building>("SELECT * FROM buildings ORDER BY name_th ASC")
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

pub async fn create_building(
    pool: &PgPool,
    payload: CreateBuildingRequest,
) -> Result<Building, AppError> {
    sqlx::query_as::<_, Building>(
        r#"
        INSERT INTO buildings (name_th, name_en, code, description)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(payload.name_th)
    .bind(payload.name_en)
    .bind(payload.code)
    .bind(payload.description)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update_building(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateBuildingRequest,
) -> Result<Building, AppError> {
    sqlx::query_as::<_, Building>(
        r#"
        UPDATE buildings SET
            name_th = COALESCE($1, name_th),
            name_en = COALESCE($2, name_en),
            code = COALESCE($3, code),
            description = COALESCE($4, description),
            updated_at = NOW()
        WHERE id = $5
        RETURNING *
        "#,
    )
    .bind(payload.name_th)
    .bind(payload.name_en)
    .bind(payload.code)
    .bind(payload.description)
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn delete_building(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM buildings WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_rooms(pool: &PgPool, filter: RoomFilter) -> Result<Vec<Room>, AppError> {
    let mut sql = String::from(
        r#"
        SELECT r.*, b.name_th as building_name
        FROM rooms r
        LEFT JOIN buildings b ON r.building_id = b.id
        WHERE 1=1
        "#,
    );

    let mut idx = 0u32;

    if filter.building_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND r.building_id = ${idx}"));
    }
    if filter.room_type.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND r.room_type = ${idx}"));
    }
    if let Some(ref search) = filter.search {
        if !search.is_empty() {
            idx += 1;
            sql.push_str(&format!(
                " AND (r.name_th ILIKE ${idx} OR r.code ILIKE ${idx})"
            ));
        }
    }

    sql.push_str(" ORDER BY b.code NULLS LAST, r.floor NULLS FIRST, r.code ASC");

    let mut query = sqlx::query_as::<_, Room>(&sql);
    if let Some(building_id) = filter.building_id {
        query = query.bind(building_id);
    }
    if let Some(room_type) = &filter.room_type {
        query = query.bind(room_type);
    }
    if let Some(ref search) = filter.search {
        if !search.is_empty() {
            query = query.bind(format!("%{search}%"));
        }
    }

    query.fetch_all(pool).await.map_err(AppError::from)
}

pub async fn create_room(pool: &PgPool, payload: CreateRoomRequest) -> Result<Room, AppError> {
    sqlx::query_as::<_, Room>(
        r#"
        INSERT INTO rooms (
            building_id, name_th, name_en, code, room_type, 
            capacity, floor, status, description
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *, (SELECT name_th FROM buildings WHERE id = $1) as building_name
        "#,
    )
    .bind(payload.building_id)
    .bind(payload.name_th)
    .bind(payload.name_en)
    .bind(payload.code)
    .bind(payload.room_type)
    .bind(room_capacity_or_default(payload.capacity))
    .bind(payload.floor)
    .bind(room_status_or_default(payload.status))
    .bind(payload.description)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update_room(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateRoomRequest,
) -> Result<Room, AppError> {
    sqlx::query_as::<_, Room>(
        r#"
        UPDATE rooms SET
            building_id = COALESCE($1, building_id),
            name_th = COALESCE($2, name_th),
            name_en = COALESCE($3, name_en),
            code = COALESCE($4, code),
            room_type = COALESCE($5, room_type),
            capacity = COALESCE($6, capacity),
            floor = COALESCE($7, floor),
            status = COALESCE($8, status),
            description = COALESCE($9, description),
            updated_at = NOW()
        WHERE id = $10
        RETURNING *, (SELECT name_th FROM buildings WHERE id = rooms.building_id) as building_name
        "#,
    )
    .bind(payload.building_id)
    .bind(payload.name_th)
    .bind(payload.name_en)
    .bind(payload.code)
    .bind(payload.room_type)
    .bind(payload.capacity)
    .bind(payload.floor)
    .bind(payload.status)
    .bind(payload.description)
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn delete_room(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM rooms WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

fn room_capacity_or_default(capacity: Option<i32>) -> i32 {
    capacity.unwrap_or(40)
}

fn room_status_or_default(status: Option<String>) -> String {
    status.unwrap_or_else(|| "ACTIVE".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_capacity_defaults_to_forty_when_missing() {
        assert_eq!(room_capacity_or_default(None), 40);
        assert_eq!(room_capacity_or_default(Some(12)), 12);
    }

    #[test]
    fn room_status_defaults_to_active_when_missing() {
        assert_eq!(room_status_or_default(None), "ACTIVE");
        assert_eq!(
            room_status_or_default(Some("MAINTENANCE".to_string())),
            "MAINTENANCE"
        );
    }
}
