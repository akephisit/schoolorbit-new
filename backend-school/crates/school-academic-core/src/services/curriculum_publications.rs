use crate::models::{
    CurriculumDraftDiscardPreview, CurriculumEdition, CurriculumLevel, CurriculumLevelView,
    CurriculumPublication, CurriculumPublicationHistory, CurriculumStructureWorkspace,
    CurriculumViewQuery, DiscardCurriculumDraftRequest, OpenCurriculumDraftRequest, StudyProgram,
    VersionStatus,
};
use school_errors::AppError;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

const PUBLICATION_COLUMNS: &str = "p.id,p.edition_id,p.publication_no,p.previous_publication_id,p.name,p.revision_year,p.description,p.published_by,NULLIF(concat_ws(' ',u.title,u.first_name,u.last_name),'') AS publisher_name,p.published_at,p.captured_at,p.change_note,p.is_baseline,p.level_count,p.program_count,p.slot_count,p.course_count,p.activity_count";

pub(super) fn validate_draft_token(actual: Option<Uuid>, requested: Uuid) -> Result<(), AppError> {
    if actual == Some(requested) {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "ร่างหลักสูตรนี้ถูกเผยแพร่หรือเปลี่ยนแล้ว กรุณาเปิดร่างล่าสุด".into(),
        ))
    }
}

pub(super) fn validate_change_note(note: &str) -> Result<(), AppError> {
    if note.trim().is_empty() || note.chars().count() > 2_000 {
        Err(AppError::ValidationError(
            "ระบุสรุปการแก้ไขไม่เกิน 2,000 ตัวอักษร".into(),
        ))
    } else {
        Ok(())
    }
}

/// Only provider-owned table names can enter this SQL. The parameter is always bound,
/// even for a workspace read, so callers cannot accidentally mix source publications.
pub(super) fn source(table: &str, publication: Option<Uuid>) -> Result<String, AppError> {
    let snapshot = match table {
        "curriculum_levels" => "curriculum_publication_levels",
        "study_programs" => "curriculum_publication_programs",
        "curriculum_term_slots" => "curriculum_publication_slots",
        "curriculum_course_requirements" => "curriculum_publication_courses",
        "curriculum_activity_requirements" => "curriculum_publication_activities",
        _ => return Err(AppError::ValidationError("แหล่งข้อมูลหลักสูตรไม่ถูกต้อง".into())),
    };
    Ok(if publication.is_some() {
        format!("(SELECT * FROM {snapshot} WHERE publication_id=$2)")
    } else {
        format!("(SELECT * FROM {table} WHERE $2::uuid IS NULL)")
    })
}

pub async fn open_draft(
    pool: &PgPool,
    id: Uuid,
    request: OpenCurriculumDraftRequest,
) -> Result<CurriculumEdition, AppError> {
    super::parse_row_version(request.row_version)?;
    let mut tx = pool.begin().await?;
    let query = format!(
        "SELECT {} FROM curriculum_editions WHERE id=$1 FOR UPDATE",
        super::curriculum::EDITION_COLUMNS
    );
    let edition: CurriculumEdition = sqlx::query_as(sqlx::AssertSqlSafe(query))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบฉบับหลักสูตร".into()))?;
    if edition.status == VersionStatus::Archived {
        return Err(AppError::Conflict("หลักสูตรที่เก็บถาวรเปิดร่างไม่ได้".into()));
    }
    // A concurrent editor opens the same single workspace, never a second draft.
    if edition.draft_id.is_none() {
        if edition.row_version != request.row_version {
            return Err(AppError::Conflict(
                "ฉบับหลักสูตรเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด".into(),
            ));
        }
        sqlx::query("UPDATE curriculum_editions SET draft_id=gen_random_uuid(),row_version=row_version+1,updated_at=now() WHERE id=$1")
            .bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE curriculum_levels SET row_version=row_version+1,updated_at=now() WHERE edition_id=$1").bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE study_programs SET row_version=row_version+1,updated_at=now() WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1)").bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    super::curriculum::get(pool, id).await
}

fn validate_discard_context(
    token: Option<Uuid>,
    publication: Option<Uuid>,
    row_version: i64,
    request: &DiscardCurriculumDraftRequest,
) -> Result<Uuid, AppError> {
    super::parse_row_version(request.row_version)?;
    if request.content_hash.len() != 64
        || !request
            .content_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(AppError::ValidationError("โหลดข้อมูลยืนยันการลบร่างใหม่".into()));
    }
    validate_draft_token(token, request.draft_id)?;
    if row_version != request.row_version {
        return Err(AppError::Conflict(
            "ร่างหลักสูตรเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุดก่อนลบ".into(),
        ));
    }
    publication.ok_or_else(|| AppError::Conflict("ยกเลิกร่างแก้ไขได้เฉพาะฉบับที่เคยเผยแพร่แล้ว".into()))
}

async fn discard_preview_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    token: Uuid,
    row_version: i64,
) -> Result<CurriculumDraftDiscardPreview, AppError> {
    sqlx::query_as(
        "WITH graph AS (SELECT
        COALESCE((SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_levels r WHERE edition_id=$1),'[]') AS levels,
        COALESCE((SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM study_programs r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1)),'[]') AS programs,
        COALESCE((SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_term_slots r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1)),'[]') AS slots,
        COALESCE((SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_course_requirements r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1)),'[]') AS courses,
        COALESCE((SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_activity_requirements r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1)),'[]') AS activities,
        COALESCE((SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_requirement_sources r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1)),'[]') AS sources)
        SELECT $2::uuid AS draft_id,$3::bigint AS row_version,
        encode(sha256(convert_to(to_jsonb(graph)::text,'UTF8')),'hex') AS content_hash,
        jsonb_array_length(levels)::bigint AS level_count,jsonb_array_length(programs)::bigint AS program_count,
        jsonb_array_length(slots)::bigint AS slot_count,jsonb_array_length(courses)::bigint AS course_count,
        jsonb_array_length(activities)::bigint AS activity_count FROM graph"
    ).bind(id).bind(token).bind(row_version).fetch_one(&mut **tx).await.map_err(Into::into)
}

pub async fn preview_discard(
    pool: &PgPool,
    id: Uuid,
    token: Uuid,
) -> Result<CurriculumDraftDiscardPreview, AppError> {
    let mut tx = pool.begin().await?;
    let sql = format!(
        "SELECT {} FROM curriculum_editions WHERE id=$1 FOR SHARE",
        super::curriculum::EDITION_COLUMNS
    );
    let edition: CurriculumEdition = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบฉบับหลักสูตร".into()))?;
    validate_draft_token(edition.draft_id, token)?;
    if edition.current_publication_id.is_none() {
        return Err(AppError::Conflict(
            "ยกเลิกร่างแก้ไขได้เฉพาะฉบับที่เคยเผยแพร่แล้ว".into(),
        ));
    }
    let preview = discard_preview_in_transaction(&mut tx, id, token, edition.row_version).await?;
    tx.commit().await?;
    Ok(preview)
}

fn discard_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(ref database) = error {
        if database.code().as_deref() == Some("23503") {
            return AppError::Conflict("ร่างมีข้อมูลอื่นอ้างอิงอยู่ จึงยังลบไม่ได้".into());
        }
    }
    error.into()
}

/// Restore the released graph and physically remove all unpublished additions in one
/// owner-locked transaction. Neither publication history nor actual delivery is written.
pub async fn discard_draft(
    pool: &PgPool,
    id: Uuid,
    request: DiscardCurriculumDraftRequest,
) -> Result<CurriculumEdition, AppError> {
    let mut tx = pool.begin().await?;
    let sql = format!(
        "SELECT {} FROM curriculum_editions WHERE id=$1 FOR UPDATE",
        super::curriculum::EDITION_COLUMNS
    );
    let edition: CurriculumEdition = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบฉบับหลักสูตร".into()))?;
    let publication = validate_discard_context(
        edition.draft_id,
        edition.current_publication_id,
        edition.row_version,
        &request,
    )?;
    let preview =
        discard_preview_in_transaction(&mut tx, id, request.draft_id, edition.row_version).await?;
    if preview.content_hash != request.content_hash {
        return Err(AppError::Conflict(
            "มีการแก้ไขร่างหลังเปิดหน้าต่างยืนยัน กรุณาตรวจสอบและยืนยันใหม่".into(),
        ));
    }
    for table in [
        "curriculum_course_requirements",
        "curriculum_activity_requirements",
        "curriculum_term_slots",
    ] {
        let sql=format!("DELETE FROM {table} WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1)");
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(discard_error)?;
    }
    sqlx::query("DELETE FROM curriculum_requirement_sources s WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1) AND NOT EXISTS(SELECT 1 FROM curriculum_publication_courses WHERE id=s.id) AND NOT EXISTS(SELECT 1 FROM curriculum_publication_activities WHERE id=s.id) AND NOT EXISTS(SELECT 1 FROM course_offering_details WHERE curriculum_course_requirement_id=s.id) AND NOT EXISTS(SELECT 1 FROM activity_offering_details WHERE curriculum_activity_requirement_id=s.id)")
        .bind(id).execute(&mut *tx).await.map_err(discard_error)?;
    sqlx::query("DELETE FROM study_programs w WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1) AND NOT EXISTS(SELECT 1 FROM curriculum_publication_programs p WHERE p.publication_id=$2 AND p.id=w.id)")
        .bind(id).bind(publication).execute(&mut *tx).await.map_err(discard_error)?;
    sqlx::query("DELETE FROM curriculum_levels w WHERE edition_id=$1 AND NOT EXISTS(SELECT 1 FROM curriculum_publication_levels p WHERE p.publication_id=$2 AND p.id=w.id)")
        .bind(id).bind(publication).execute(&mut *tx).await.map_err(discard_error)?;
    // Codes and the default flag may have been swapped during editing. Vacate their
    // unique keys inside this transaction before restoring the published values.
    sqlx::query(
        "UPDATE curriculum_levels SET code='discard-'||gen_random_uuid()::text WHERE edition_id=$1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE study_programs SET code='discard-'||gen_random_uuid()::text,is_default=false WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=$1)")
        .bind(id).execute(&mut *tx).await?;
    for (table,snapshot,columns) in [
        ("curriculum_levels","curriculum_publication_levels","id,description,created_at,updated_at,row_version,migration_provenance,edition_id,code,name_th,name_en,grade_level_ids,is_active"),
        ("study_programs","curriculum_publication_programs","id,curriculum_level_id,code,name_th,name_en,is_default,status,row_version,created_at,updated_at,migration_provenance"),
        ("curriculum_term_slots","curriculum_publication_slots","id,curriculum_level_id,sequence,term_type,type_occurrence,name,row_version,created_at,updated_at"),
        ("curriculum_course_requirements","curriculum_publication_courses","id,curriculum_level_id,grade_level_id,subject_version_id,display_order,metadata,created_at,updated_at,study_program_id,requirement_kind,row_version,term_slot_id"),
        ("curriculum_activity_requirements","curriculum_publication_activities","id,curriculum_level_id,display_order,created_at,updated_at,activity_version_id,grade_level_id,study_program_id,requirement_kind,row_version,term_slot_id"),
    ] {
        let selected=columns.split(',').map(|column| match column {
            "row_version"=>"p.row_version+1".to_owned(),
            "updated_at"=>"now()".to_owned(),
            _=>format!("p.{column}"),
        }).collect::<Vec<_>>().join(",");
        let updated=columns.split(',').filter(|column| *column!="id").map(|column| match column {
            "row_version"=>"row_version=GREATEST(workspace.row_version+1,EXCLUDED.row_version)".to_owned(),
            _=>format!("{column}=EXCLUDED.{column}"),
        }).collect::<Vec<_>>().join(",");
        let sql=format!("INSERT INTO {table} AS workspace({columns}) SELECT {selected} FROM {snapshot} p WHERE publication_id=$1 ON CONFLICT(id) DO UPDATE SET {updated}");
        sqlx::query(sqlx::AssertSqlSafe(sql)).bind(publication).execute(&mut *tx).await.map_err(discard_error)?;
    }
    // The database guard independently requires the entire workspace to match the
    // publication before permitting this token to close without a new publication.
    sqlx::query("UPDATE curriculum_editions SET draft_id=NULL,row_version=row_version+1,updated_at=now() WHERE id=$1")
        .bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    super::curriculum::get(pool, id).await
}

pub async fn resolve_view(
    pool: &PgPool,
    edition_id: Uuid,
    query: &CurriculumViewQuery,
) -> Result<Option<Uuid>, AppError> {
    if query.draft_id.is_some() && query.publication_id.is_some() {
        return Err(AppError::ValidationError(
            "เลือกดูร่างหรือประวัติการเผยแพร่เพียงอย่างเดียว".into(),
        ));
    }
    let edition = super::curriculum::get(pool, edition_id).await?;
    if let Some(draft) = query.draft_id {
        validate_draft_token(edition.draft_id, draft)?;
        return Ok(None);
    }
    if let Some(publication) = query.publication_id {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM curriculum_publications WHERE id=$1 AND edition_id=$2)",
        )
        .bind(publication)
        .bind(edition_id)
        .fetch_one(pool)
        .await?;
        if !exists {
            return Err(AppError::NotFound("ไม่พบประวัติการเผยแพร่ในฉบับนี้".into()));
        }
        return Ok(Some(publication));
    }
    Ok(edition.current_publication_id)
}

async fn level_edition(pool: &PgPool, level_id: Uuid) -> Result<Uuid, AppError> {
    sqlx::query_scalar("SELECT edition_id FROM curriculum_levels WHERE id=$1")
        .bind(level_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบระดับการศึกษา".into()))
}

pub async fn read_levels(
    pool: &PgPool,
    edition_id: Uuid,
    query: &CurriculumViewQuery,
) -> Result<Vec<CurriculumLevelView>, AppError> {
    let publication = resolve_view(pool, edition_id, query).await?;
    let table = source("curriculum_levels", publication)?;
    let columns = if publication.is_some() {
        super::curriculum::LEVEL_COLUMNS.replace("CASE WHEN e.draft_id IS NOT NULL THEN 'draft' ELSE e.status END AS status,e.draft_id,NULL::uuid AS publication_id", "'published'::text AS status,NULL::uuid AS draft_id,l.publication_id")
    } else {
        super::curriculum::LEVEL_COLUMNS.to_owned()
    };
    let sql = format!("SELECT {columns} FROM {table} l JOIN curriculum_editions e ON e.id=l.edition_id WHERE l.edition_id=$1 ORDER BY l.created_at,l.id LIMIT 501");
    let levels: Vec<CurriculumLevel> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(edition_id)
        .bind(publication)
        .fetch_all(pool)
        .await?;
    if levels.len() > 500 {
        return Err(AppError::ValidationError(
            "จำนวนระดับการศึกษาเกิน 500 รายการ".into(),
        ));
    }
    Ok(levels
        .into_iter()
        .map(|level| CurriculumLevelView { level })
        .collect())
}

pub async fn read_level(
    pool: &PgPool,
    level_id: Uuid,
    query: &CurriculumViewQuery,
) -> Result<CurriculumLevel, AppError> {
    let publication = resolve_view(pool, level_edition(pool, level_id).await?, query).await?;
    super::curriculum::level_from_publication(pool, level_id, publication).await
}

pub async fn read_workspace(
    pool: &PgPool,
    level_id: Uuid,
    query: &CurriculumViewQuery,
) -> Result<CurriculumStructureWorkspace, AppError> {
    let edition_id = level_edition(pool, level_id).await?;
    let publication = resolve_view(pool, edition_id, query).await?;
    let workspace =
        super::curriculum_structure::get_workspace_from_publication(pool, level_id, publication)
            .await?;
    if query.draft_id.is_some() {
        resolve_view(pool, edition_id, query).await?;
    }
    Ok(workspace)
}

pub async fn read_programs(
    pool: &PgPool,
    level_id: Uuid,
    query: &CurriculumViewQuery,
) -> Result<Vec<StudyProgram>, AppError> {
    let publication = resolve_view(pool, level_edition(pool, level_id).await?, query).await?;
    super::curriculum::level_from_publication(pool, level_id, publication).await?;
    super::curriculum::programs_from_publication(pool, level_id, publication).await
}

pub async fn read_program(
    pool: &PgPool,
    id: Uuid,
    query: &CurriculumViewQuery,
) -> Result<StudyProgram, AppError> {
    let level_id: Uuid =
        sqlx::query_scalar("SELECT curriculum_level_id FROM study_programs WHERE id=$1")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบแผนการเรียน".into()))?;
    let publication = resolve_view(pool, level_edition(pool, level_id).await?, query).await?;
    let table = source("study_programs", publication)?;
    let sql = format!(
        "SELECT {} FROM {table} WHERE id=$1",
        super::curriculum::PROGRAM_COLUMNS
    );
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(publication)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบแผนการเรียนในชุดหลักสูตรที่เลือก".into()))
}

pub async fn list_publications(
    pool: &PgPool,
    edition_id: Uuid,
) -> Result<Vec<CurriculumPublication>, AppError> {
    super::curriculum::get(pool, edition_id).await?;
    let sql = format!("SELECT {PUBLICATION_COLUMNS} FROM curriculum_publications p LEFT JOIN users u ON u.id=p.published_by WHERE edition_id=$1 ORDER BY publication_no DESC LIMIT 501");
    let rows: Vec<CurriculumPublication> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(edition_id)
        .fetch_all(pool)
        .await?;
    if rows.len() > 500 {
        return Err(AppError::ValidationError(
            "ประวัติการเผยแพร่เกิน 500 ครั้ง".into(),
        ));
    }
    Ok(rows)
}

pub async fn history(
    pool: &PgPool,
    edition_id: Uuid,
    publication_id: Uuid,
) -> Result<CurriculumPublicationHistory, AppError> {
    let sql = format!("SELECT {PUBLICATION_COLUMNS} FROM curriculum_publications p LEFT JOIN users u ON u.id=p.published_by WHERE edition_id=$1 AND p.id=$2");
    let publication: CurriculumPublication = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(edition_id)
        .bind(publication_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบประวัติการเผยแพร่ในฉบับนี้".into()))?;
    let changes = super::curriculum_publication_changes::compare(
        pool,
        publication.previous_publication_id,
        publication.id,
    )
    .await?;
    Ok(CurriculumPublicationHistory {
        publication,
        changes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discard_requires_current_amendment_and_exact_revision() {
        let token = Uuid::new_v4();
        let publication = Uuid::new_v4();
        let request = DiscardCurriculumDraftRequest {
            draft_id: token,
            row_version: 3,
            content_hash: "0".repeat(64),
        };
        assert_eq!(
            validate_discard_context(Some(token), Some(publication), 3, &request).unwrap(),
            publication
        );
        assert!(validate_discard_context(Some(token), None, 3, &request).is_err());
        assert!(validate_discard_context(None, Some(publication), 3, &request).is_err());
        assert!(
            validate_discard_context(Some(Uuid::new_v4()), Some(publication), 3, &request).is_err()
        );
        assert!(validate_discard_context(Some(token), Some(publication), 4, &request).is_err());
    }
    #[test]
    fn draft_tokens_and_change_notes_fail_closed() {
        let token = Uuid::new_v4();
        assert!(validate_draft_token(Some(token), token).is_ok());
        assert!(validate_draft_token(None, token).is_err());
        assert!(validate_draft_token(Some(Uuid::new_v4()), token).is_err());
        assert!(validate_change_note("เพิ่มรายวิชาภาคเรียนที่ 2").is_ok());
        assert!(validate_change_note("  ").is_err());
        assert!(validate_change_note(&"ก".repeat(2001)).is_err());
    }
    #[test]
    fn only_canonical_sources_enter_queries() {
        assert!(source("users", None).is_err());
        assert!(source("curriculum_levels", Some(Uuid::new_v4()))
            .unwrap()
            .contains("publication_id=$2"));
        assert!(source("curriculum_levels", None)
            .unwrap()
            .contains("$2::uuid IS NULL"));
    }
}
