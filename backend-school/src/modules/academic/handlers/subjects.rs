use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::get_user_with_permissions;
use crate::modules::academic::models::curriculum::{
    AddSubjectDefaultInstructorRequest, CreateSubjectRequest, SubjectFilter,
    UpdateSubjectDefaultInstructorRoleRequest, UpdateSubjectRequest,
};
use crate::modules::academic::services::subject_service;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

pub async fn list_subject_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (_, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };
    let has_access = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_READ_ALL.to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_access {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", codes::ACADEMIC_CURRICULUM_READ_ALL) })),
        ).into_response());
    }

    let groups = subject_service::list_subject_groups(&pool).await?;
    Ok(Json(json!({ "success": true, "data": groups })).into_response())
}

pub async fn list_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<SubjectFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (user_id, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };
    let has_all = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_READ_ALL.to_string());
    let has_dept = permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_all && !has_dept {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", codes::ACADEMIC_CURRICULUM_READ_ALL) })),
        ).into_response());
    }

    let dept_group_id: Option<Uuid> = if !has_all && has_dept {
        match subject_service::get_user_subject_group_id(user_id, &pool).await {
            Some(gid) => Some(gid),
            None => {
                return Ok((
                    StatusCode::FORBIDDEN,
                    Json(json!({ "success": false, "error": "ไม่พบกลุ่มสาระที่สังกัด" })),
                ).into_response());
            }
        }
    } else { None };

    let subjects = subject_service::list_subjects(&pool, filter, dept_group_id).await?;
    Ok(Json(json!({ "success": true, "data": subjects })).into_response())
}

pub async fn create_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (user_id, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };
    let has_all = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_CREATE_ALL.to_string());
    let has_dept = permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_all && !has_dept {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", codes::ACADEMIC_CURRICULUM_CREATE_ALL) })),
        ).into_response());
    }

    if !has_all && has_dept {
        let teacher_group = subject_service::get_user_subject_group_id(user_id, &pool).await
            .ok_or_else(|| AppError::BadRequest("ไม่พบกลุ่มสาระที่สังกัด".to_string()))?;
        if payload.group_id != Some(teacher_group) {
            return Err(AppError::BadRequest("ไม่สามารถเพิ่มวิชาในกลุ่มสาระอื่นได้".to_string()));
        }
    }

    let subject = subject_service::create_subject(&pool, payload).await?;
    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": subject }))).into_response())
}

pub async fn update_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (user_id, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };
    let has_all = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_UPDATE_ALL.to_string());
    let has_dept = permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_all && !has_dept {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", codes::ACADEMIC_CURRICULUM_UPDATE_ALL) })),
        ).into_response());
    }

    if !has_all && has_dept {
        let teacher_group = subject_service::get_user_subject_group_id(user_id, &pool).await
            .ok_or_else(|| AppError::BadRequest("ไม่พบกลุ่มสาระที่สังกัด".to_string()))?;
        let subject_group = subject_service::get_subject_group_id(&pool, id).await?;
        if subject_group != Some(teacher_group) {
            return Err(AppError::BadRequest("ไม่สามารถแก้ไขวิชาในกลุ่มสาระอื่นได้".to_string()));
        }
    }

    let subject = subject_service::update_subject(&pool, id, payload).await?;
    Ok(Json(json!({ "success": true, "data": subject })).into_response())
}

pub async fn delete_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (user_id, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };
    let has_all = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_DELETE_ALL.to_string());
    let has_dept = permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_all && !has_dept {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", codes::ACADEMIC_CURRICULUM_DELETE_ALL) })),
        ).into_response());
    }

    if !has_all && has_dept {
        let teacher_group = subject_service::get_user_subject_group_id(user_id, &pool).await
            .ok_or_else(|| AppError::BadRequest("ไม่พบกลุ่มสาระที่สังกัด".to_string()))?;
        let subject_group = subject_service::get_subject_group_id(&pool, id).await?;
        if subject_group != Some(teacher_group) {
            return Err(AppError::BadRequest("ไม่สามารถลบวิชาในกลุ่มสาระอื่นได้".to_string()));
        }
    }

    subject_service::delete_subject(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

/// Permission check for default instructors: read.all OR manage.department OR specified manage code.
async fn check_subject_manage(
    state: &AppState,
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    subject_id: Uuid,
    manage_code: &str,
    read_only: bool,
) -> Result<(), axum::response::Response> {
    let (user_id, permissions) = match get_user_with_permissions(headers, pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Err(resp),
    };
    let read_codes = [
        "*".to_string(),
        codes::ACADEMIC_CURRICULUM_READ_ALL.to_string(),
        codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string(),
        manage_code.to_string(),
    ];
    let has_all = permissions.contains(&"*".to_string())
        || permissions.contains(&manage_code.to_string())
        || (read_only && read_codes.iter().any(|c| permissions.contains(c)));
    let has_dept = permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_all && !has_dept {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", manage_code) })),
        ).into_response());
    }
    if !has_all && has_dept {
        let teacher_group = match subject_service::get_user_subject_group_id(user_id, pool).await {
            Some(gid) => gid,
            None => return Err((
                StatusCode::FORBIDDEN,
                Json(json!({ "success": false, "error": "ไม่พบกลุ่มสาระที่สังกัด" })),
            ).into_response()),
        };
        let subject_group = subject_service::get_subject_group_id(pool, subject_id).await.ok().flatten();
        if subject_group != Some(teacher_group) {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({ "success": false, "error": "ไม่สามารถจัดการวิชาในกลุ่มสาระอื่นได้" })),
            ).into_response());
        }
    }
    Ok(())
}

pub async fn list_subject_default_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(resp) = check_subject_manage(&state, &headers, &pool, subject_id, codes::ACADEMIC_CURRICULUM_UPDATE_ALL, true).await {
        return Ok(resp);
    }
    let rows = subject_service::list_subject_default_instructors(&pool, subject_id).await?;
    Ok(Json(json!({ "success": true, "data": rows })).into_response())
}

pub async fn add_subject_default_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<Uuid>,
    Json(body): Json<AddSubjectDefaultInstructorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(resp) = check_subject_manage(&state, &headers, &pool, subject_id, codes::ACADEMIC_CURRICULUM_UPDATE_ALL, false).await {
        return Ok(resp);
    }
    subject_service::add_subject_default_instructor(&pool, subject_id, body).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn remove_subject_default_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((subject_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(resp) = check_subject_manage(&state, &headers, &pool, subject_id, codes::ACADEMIC_CURRICULUM_UPDATE_ALL, false).await {
        return Ok(resp);
    }
    subject_service::remove_subject_default_instructor(&pool, subject_id, instructor_id).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn update_subject_default_instructor_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((subject_id, instructor_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateSubjectDefaultInstructorRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(resp) = check_subject_manage(&state, &headers, &pool, subject_id, codes::ACADEMIC_CURRICULUM_UPDATE_ALL, false).await {
        return Ok(resp);
    }
    subject_service::update_subject_default_instructor_role(&pool, subject_id, instructor_id, &body.role).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct BatchListSubjectDefaultInstructorsQuery {
    pub subject_ids: String,
}

pub async fn batch_list_subject_default_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<BatchListSubjectDefaultInstructorsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (_, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };
    let has_access = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_READ_ALL.to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_access {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", codes::ACADEMIC_CURRICULUM_READ_ALL) })),
        ).into_response());
    }

    let ids: Vec<Uuid> = query.subject_ids.split(',')
        .filter_map(|s| s.trim().parse::<Uuid>().ok())
        .collect();

    let grouped = subject_service::batch_list_subject_default_instructors(&pool, ids).await?;
    Ok(Json(json!({ "success": true, "data": grouped })).into_response())
}
