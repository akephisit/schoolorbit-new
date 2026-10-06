use super::super::models::{
    CopyStudyProgramRequest, CreateCurriculumLevelRequest, CreateCurriculumRequest,
    CreateStudyProgramRequest, CurriculumEdition, CurriculumLevel, PublishVersionRequest,
    StudyProgram, StudyProgramOption, UpdateCurriculumLevelRequest, UpdateCurriculumRequest,
    UpdateStudyProgramRequest, VersionStatus,
};
use super::{ensure_draft_version, parse_row_version};
use school_authorization::AcademicResourceListFilter;
use school_errors::AppError;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

const EDITION_COLUMNS: &str = "id,name,revision_year,description,status,is_active,published_at,row_version,migration_provenance <> '{}'::jsonb AS migrated,created_at,updated_at";
const LEVEL_COLUMNS: &str = "l.id,l.edition_id,l.code,l.name_th,l.name_en,l.description,ARRAY(SELECT jsonb_array_elements_text(l.grade_level_ids)::uuid) AS grade_level_ids,l.is_active,l.row_version,l.migration_provenance <> '{}'::jsonb AS migrated,l.created_at,l.updated_at,e.name AS edition_name,e.revision_year,e.status";
const PROGRAM_COLUMNS: &str = "id,curriculum_level_id,code,name_th,name_en,is_default,status,row_version,created_at,updated_at";
const MAX_LEVELS: i64 = 500;
const MAX_PROGRAM_OPTIONS: i64 = 2000;
const MAX_COPIED_REQUIREMENTS: i64 = 50_000;

fn require_school_filter(filter: &AcademicResourceListFilter) -> Result<(), AppError> {
    if filter.includes_school_owned {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงหลักสูตรของโรงเรียน".into()))
    }
}
fn validate_name(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() || name.chars().count() > 200 {
        Err(AppError::ValidationError("ระบุชื่อไม่เกิน 200 ตัวอักษร".into()))
    } else {
        Ok(())
    }
}
fn validate_revision_year(year: i32) -> Result<(), AppError> {
    if (2400..=2999).contains(&year) {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "ระบุปีปรับปรุงหลักสูตรเป็นพุทธศักราชระหว่าง 2400–2999".into(),
        ))
    }
}
fn internal_code(prefix: &str, id: Uuid) -> String {
    format!("{prefix}-{}", id.simple())
}

pub async fn list(
    pool: &PgPool,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<CurriculumEdition>, AppError> {
    require_school_filter(filter)?;
    let sql=format!("SELECT {EDITION_COLUMNS} FROM curriculum_editions ORDER BY revision_year DESC NULLS LAST,created_at DESC,id LIMIT 501");
    let rows: Vec<CurriculumEdition> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .fetch_all(pool)
        .await?;
    if rows.len() > 500 {
        return Err(AppError::ValidationError(
            "จำนวนฉบับหลักสูตรเกิน 500 ฉบับ".into(),
        ));
    }
    Ok(rows)
}
pub async fn get(pool: &PgPool, id: Uuid) -> Result<CurriculumEdition, AppError> {
    let sql = format!("SELECT {EDITION_COLUMNS} FROM curriculum_editions WHERE id=$1");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบฉบับหลักสูตร".into()))
}
pub async fn create(
    pool: &PgPool,
    request: CreateCurriculumRequest,
) -> Result<CurriculumEdition, AppError> {
    validate_name(&request.name)?;
    validate_revision_year(request.revision_year)?;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO curriculum_editions(id,name,revision_year,description) VALUES($1,$2,$3,$4)",
    )
    .bind(id)
    .bind(request.name.trim())
    .bind(request.revision_year)
    .bind(request.description)
    .execute(pool)
    .await?;
    get(pool, id).await
}
pub async fn update(
    pool: &PgPool,
    id: Uuid,
    request: UpdateCurriculumRequest,
) -> Result<CurriculumEdition, AppError> {
    validate_name(&request.name)?;
    validate_revision_year(request.revision_year)?;
    parse_row_version(request.row_version)?;
    let result=sqlx::query("UPDATE curriculum_editions SET name=$1,revision_year=$2,description=$3,row_version=row_version+1,updated_at=now() WHERE id=$4 AND row_version=$5 AND status='draft'")
        .bind(request.name.trim()).bind(request.revision_year).bind(request.description).bind(id).bind(request.row_version).execute(pool).await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict("ฉบับหลักสูตรถูกแก้ไขหรือเผยแพร่แล้ว".into()));
    }
    get(pool, id).await
}
pub async fn get_level(pool: &PgPool, id: Uuid) -> Result<CurriculumLevel, AppError> {
    let sql=format!("SELECT {LEVEL_COLUMNS} FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id WHERE l.id=$1");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบระดับการศึกษาในฉบับหลักสูตร".into()))
}
pub async fn list_levels(
    pool: &PgPool,
    edition_id: Uuid,
) -> Result<Vec<CurriculumLevel>, AppError> {
    get(pool, edition_id).await?;
    let sql=format!("SELECT {LEVEL_COLUMNS} FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id WHERE l.edition_id=$1 ORDER BY l.created_at,l.id LIMIT $2");
    let rows: Vec<CurriculumLevel> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(edition_id)
        .bind(MAX_LEVELS + 1)
        .fetch_all(pool)
        .await?;
    if rows.len() as i64 > MAX_LEVELS {
        return Err(AppError::ValidationError(
            "จำนวนระดับการศึกษาเกิน 500 รายการ".into(),
        ));
    }
    Ok(rows)
}
async fn require_draft_edition(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(), AppError> {
    let status: VersionStatus =
        sqlx::query_scalar("SELECT status FROM curriculum_editions WHERE id=$1 FOR SHARE")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบฉบับหลักสูตร".into()))?;
    ensure_draft_version(status)
}
pub(super) async fn require_draft_level(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(), AppError> {
    let edition_id: Uuid =
        sqlx::query_scalar("SELECT edition_id FROM curriculum_levels WHERE id=$1")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบระดับการศึกษา".into()))?;
    require_draft_edition(tx, edition_id).await?;
    sqlx::query("SELECT id FROM curriculum_levels WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
    Ok(())
}
async fn validate_grade_levels(pool: &PgPool, ids: &[Uuid]) -> Result<(), AppError> {
    let unique = ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if ids.is_empty() || unique.len() != ids.len() {
        return Err(AppError::ValidationError(
            "เลือกระดับชั้นอย่างน้อยหนึ่งชั้นและไม่ซ้ำกัน".into(),
        ));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM grade_levels WHERE id=ANY($1) AND is_active")
            .bind(ids)
            .fetch_one(pool)
            .await?;
    if count != ids.len() as i64 {
        return Err(AppError::ValidationError(
            "ระดับชั้นที่เลือกไม่ถูกต้องหรือปิดใช้งานแล้ว".into(),
        ));
    }
    Ok(())
}
pub async fn create_level(
    pool: &PgPool,
    edition_id: Uuid,
    request: CreateCurriculumLevelRequest,
) -> Result<CurriculumLevel, AppError> {
    validate_name(&request.name_th)?;
    validate_grade_levels(pool, &request.grade_level_ids).await?;
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    require_draft_edition(&mut tx, edition_id).await?;
    sqlx::query("INSERT INTO curriculum_levels(id,edition_id,code,name_th,description,grade_level_ids,is_active) VALUES($1,$2,$3,$4,$5,$6,true)")
        .bind(id).bind(edition_id).bind(internal_code("LEVEL",id)).bind(request.name_th.trim()).bind(request.description).bind(sqlx::types::Json(request.grade_level_ids)).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO curriculum_term_slots(id,curriculum_level_id,sequence,term_type,type_occurrence,name) SELECT gen_random_uuid(),$1,n,'regular',n,'ภาคเรียนที่ '||n::text FROM generate_series(1,2) n")
        .bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    get_level(pool, id).await
}
pub async fn update_level(
    pool: &PgPool,
    id: Uuid,
    request: UpdateCurriculumLevelRequest,
) -> Result<CurriculumLevel, AppError> {
    validate_name(&request.name_th)?;
    validate_grade_levels(pool, &request.grade_level_ids).await?;
    parse_row_version(request.row_version)?;
    let mut tx = pool.begin().await?;
    require_draft_level(&mut tx, id).await?;
    let uncovered:i64=sqlx::query_scalar("SELECT count(*) FROM (SELECT grade_level_id FROM curriculum_course_requirements WHERE curriculum_level_id=$1 UNION SELECT grade_level_id FROM curriculum_activity_requirements WHERE curriculum_level_id=$1) r WHERE NOT grade_level_id=ANY($2)").bind(id).bind(&request.grade_level_ids).fetch_one(&mut *tx).await?;
    if uncovered > 0 {
        return Err(AppError::ValidationError(
            "ระดับชั้นที่เลือกต้องครอบคลุมรายวิชาและกิจกรรมที่มีอยู่".into(),
        ));
    }
    let result=sqlx::query("UPDATE curriculum_levels SET name_th=$1,description=$2,grade_level_ids=$3,row_version=row_version+1,updated_at=now() WHERE id=$4 AND row_version=$5")
        .bind(request.name_th.trim()).bind(request.description).bind(sqlx::types::Json(request.grade_level_ids)).bind(id).bind(request.row_version).execute(&mut *tx).await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict("ระดับการศึกษาถูกแก้ไขโดยผู้ใช้อื่น".into()));
    }
    tx.commit().await?;
    get_level(pool, id).await
}
pub async fn publish(
    pool: &PgPool,
    id: Uuid,
    request: PublishVersionRequest,
) -> Result<CurriculumEdition, AppError> {
    parse_row_version(request.row_version)?;
    let mut tx = pool.begin().await?;
    let sql = format!("SELECT {EDITION_COLUMNS} FROM curriculum_editions WHERE id=$1 FOR UPDATE");
    let edition: CurriculumEdition = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบฉบับหลักสูตร".into()))?;
    ensure_draft_version(edition.status)?;
    if edition.row_version != request.row_version {
        return Err(AppError::Conflict("ฉบับหลักสูตรถูกแก้ไขโดยผู้ใช้อื่น".into()));
    }
    validate_revision_year(
        edition
            .revision_year
            .ok_or_else(|| AppError::ValidationError("กำหนดปีปรับปรุงก่อนเผยแพร่".into()))?,
    )?;
    let sql=format!("SELECT {LEVEL_COLUMNS} FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id WHERE l.edition_id=$1 FOR UPDATE OF l");
    let levels: Vec<CurriculumLevel> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
    if levels.is_empty() {
        return Err(AppError::ValidationError("เพิ่มระดับการศึกษาก่อนเผยแพร่".into()));
    }
    for level in &levels {
        if level.grade_level_ids.is_empty() {
            return Err(AppError::ValidationError(
                "กำหนดชั้นที่ครอบคลุมให้ทุกระดับก่อนเผยแพร่".into(),
            ));
        }
        validate_publishable(&mut tx, level).await?;
    }
    sqlx::query("UPDATE study_programs SET status='published',row_version=row_version+1,updated_at=now() WHERE curriculum_level_id IN (SELECT id FROM curriculum_levels WHERE edition_id=$1) AND status='draft'").bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE curriculum_editions SET status='published',published_at=now(),row_version=row_version+1,updated_at=now() WHERE id=$1").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    get(pool, id).await
}
pub async fn list_programs(pool: &PgPool, level_id: Uuid) -> Result<Vec<StudyProgram>, AppError> {
    get_level(pool, level_id).await?;
    list_programs_for_level(pool, level_id).await
}
pub(super) async fn list_programs_for_level(
    pool: &PgPool,
    level_id: Uuid,
) -> Result<Vec<StudyProgram>, AppError> {
    let sql=format!("SELECT {PROGRAM_COLUMNS} FROM study_programs WHERE curriculum_level_id=$1 ORDER BY is_default DESC,code,id");
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(level_id)
        .fetch_all(pool)
        .await?)
}
pub async fn get_program(pool: &PgPool, id: Uuid) -> Result<StudyProgram, AppError> {
    let sql = format!("SELECT {PROGRAM_COLUMNS} FROM study_programs WHERE id=$1");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบแผนการเรียน".into()))
}
async fn clear_default(
    tx: &mut Transaction<'_, Postgres>,
    level_id: Uuid,
    except: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query("UPDATE study_programs SET is_default=false,row_version=row_version+1,updated_at=now() WHERE curriculum_level_id=$1 AND is_default AND ($2::uuid IS NULL OR id<>$2)").bind(level_id).bind(except).execute(&mut **tx).await?;
    Ok(())
}
pub async fn create_program(
    pool: &PgPool,
    level_id: Uuid,
    request: CreateStudyProgramRequest,
) -> Result<StudyProgram, AppError> {
    validate_name(&request.name_th)?;
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    require_draft_level(&mut tx, level_id).await?;
    if request.is_default {
        clear_default(&mut tx, level_id, None).await?;
    }
    sqlx::query("INSERT INTO study_programs(id,curriculum_level_id,code,name_th,is_default,status) VALUES($1,$2,$3,$4,$5,'draft')")
        .bind(id).bind(level_id).bind(internal_code("PLAN",id)).bind(request.name_th.trim()).bind(request.is_default).execute(&mut *tx).await?;
    tx.commit().await?;
    get_program(pool, id).await
}
pub async fn update_program(
    pool: &PgPool,
    id: Uuid,
    request: UpdateStudyProgramRequest,
) -> Result<StudyProgram, AppError> {
    validate_name(&request.name_th)?;
    parse_row_version(request.row_version)?;
    let mut tx = pool.begin().await?;
    let level_id: Uuid =
        sqlx::query_scalar("SELECT curriculum_level_id FROM study_programs WHERE id=$1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบแผนการเรียน".into()))?;
    require_draft_level(&mut tx, level_id).await?;
    let program: StudyProgram = {
        let sql = format!("SELECT {PROGRAM_COLUMNS} FROM study_programs WHERE id=$1 FOR UPDATE");
        sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบแผนการเรียน".into()))?
    };
    ensure_draft_version(program.status)?;
    if program.row_version != request.row_version {
        return Err(AppError::Conflict("แผนการเรียนถูกแก้ไขโดยผู้ใช้อื่น".into()));
    }
    if request.is_default {
        clear_default(&mut tx, program.curriculum_level_id, Some(id)).await?;
    }
    sqlx::query("UPDATE study_programs SET name_th=$1,is_default=$2,row_version=row_version+1,updated_at=now() WHERE id=$3").bind(request.name_th.trim()).bind(request.is_default).bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    get_program(pool, id).await
}
pub async fn list_study_program_options_for_year(
    pool: &PgPool,
    academic_year_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<StudyProgramOption>, AppError> {
    require_school_filter(filter)?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_years WHERE id=$1)")
            .bind(academic_year_id)
            .fetch_one(pool)
            .await?;
    if !exists {
        return Err(AppError::NotFound("ไม่พบปีการศึกษา".into()));
    }
    let options:Vec<StudyProgramOption>=sqlx::query_as(r#"SELECT p.id,p.code,p.name_th AS name,e.id AS edition_id,e.name AS edition_name,e.revision_year,l.id AS curriculum_level_id,l.name_th AS level_name,
        ARRAY(SELECT DISTINCT grade_level_id FROM (SELECT grade_level_id FROM curriculum_course_requirements WHERE study_program_id=p.id UNION SELECT grade_level_id FROM curriculum_activity_requirements WHERE study_program_id=p.id) r ORDER BY grade_level_id) AS grade_level_ids
        FROM study_programs p JOIN curriculum_levels l ON l.id=p.curriculum_level_id JOIN curriculum_editions e ON e.id=l.edition_id
        WHERE p.status='published' AND e.status='published' AND e.is_active AND l.is_active
        ORDER BY e.revision_year DESC NULLS LAST,e.created_at DESC,l.name_th,p.is_default DESC,p.name_th,p.id LIMIT $1"#).bind(MAX_PROGRAM_OPTIONS+1).fetch_all(pool).await?;
    if options.len() as i64 > MAX_PROGRAM_OPTIONS {
        return Err(AppError::ValidationError(
            "จำนวนแผนการเรียนเกิน 2,000 รายการ".into(),
        ));
    }
    Ok(options)
}

async fn validate_publishable(
    transaction: &mut Transaction<'_, Postgres>,
    version: &CurriculumLevel,
) -> Result<(), AppError> {
    let term_slot_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM curriculum_term_slots WHERE curriculum_level_id = $1",
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if term_slot_count == 0 {
        return Err(AppError::ValidationError(
            "ต้องกำหนดภาคเรียนในโครงสร้างหลักสูตรก่อนเผยแพร่".to_string(),
        ));
    }
    let program_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM study_programs WHERE curriculum_level_id = $1 AND status <> 'archived'",
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if program_count == 0 {
        return Err(AppError::ValidationError(
            "ต้องมีแผนการเรียนอย่างน้อยหนึ่งรายการก่อนเผยแพร่".to_string(),
        ));
    }
    let default_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM study_programs WHERE curriculum_level_id = $1 AND is_default AND status <> 'archived'",
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if default_count != 1 {
        return Err(AppError::ValidationError(
            "ต้องมีแผนการเรียนเริ่มต้นหนึ่งรายการพอดี".to_string(),
        ));
    }
    let empty_programs: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*) FROM study_programs program
        WHERE program.curriculum_level_id = $1 AND program.status <> 'archived'
          AND NOT EXISTS (SELECT 1 FROM curriculum_course_requirements course WHERE course.study_program_id = program.id)
          AND NOT EXISTS (SELECT 1 FROM curriculum_activity_requirements activity WHERE activity.study_program_id = program.id)
        "#,
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if empty_programs != 0 {
        return Err(AppError::ValidationError(
            "ทุกแผนการเรียนต้องมีข้อกำหนดอย่างน้อยหนึ่งรายการ".to_string(),
        ));
    }
    let unpublished_resources: i64 = sqlx::query_scalar(
        r#"
        SELECT
          (SELECT count(*) FROM curriculum_course_requirements requirement
           JOIN subject_versions subject ON subject.id = requirement.subject_version_id
           WHERE requirement.curriculum_level_id = $1 AND subject.status <> 'published')
        + (SELECT count(*) FROM curriculum_activity_requirements requirement
           JOIN activity_versions activity ON activity.id = requirement.activity_version_id
           WHERE requirement.curriculum_level_id = $1 AND activity.status <> 'published')
        "#,
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if unpublished_resources != 0 {
        return Err(AppError::ValidationError(
            "ข้อกำหนดอ้างอิงเวอร์ชันที่ยังไม่เผยแพร่".to_string(),
        ));
    }
    let incomplete_course_metrics: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM curriculum_course_requirements requirement
           JOIN subject_versions version ON version.id = requirement.subject_version_id
           WHERE requirement.curriculum_level_id = $1
             AND (version.periods_per_week IS NULL
                  OR version.periods_per_week <= 0
                  OR version.credit IS NULL
                  OR version.credit <= 0
                  OR version.hours_per_semester IS NULL
                  OR version.hours_per_semester <= 0)"#,
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if incomplete_course_metrics != 0 {
        return Err(AppError::ValidationError(
            "รายวิชาในโครงสร้างต้องมีหน่วยกิต จำนวนคาบ และชั่วโมงรวมจากบัญชีรายวิชาให้ครบและมากกว่า 0"
                .to_string(),
        ));
    }
    let incomplete_activity_metrics: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM curriculum_activity_requirements requirement
           JOIN activity_versions version ON version.id = requirement.activity_version_id
           WHERE requirement.curriculum_level_id = $1
             AND (version.hours_per_week IS NULL
                  OR version.hours_per_week <= 0
                  OR version.hours_per_term IS NULL
                  OR version.hours_per_term <= 0)"#,
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if incomplete_activity_metrics != 0 {
        return Err(AppError::ValidationError(
            "กิจกรรมในโครงสร้างต้องมีชั่วโมงต่อสัปดาห์และชั่วโมงรวมต่อภาคเรียนให้ครบและมากกว่า 0".to_string(),
        ));
    }
    Ok(())
}

pub async fn copy_program(
    pool: &PgPool,
    level_id: Uuid,
    request: CopyStudyProgramRequest,
) -> Result<StudyProgram, AppError> {
    parse_row_version(request.source_row_version)?;
    parse_row_version(request.destination_row_version)?;
    let mut tx = pool.begin().await?;
    require_draft_level(&mut tx, level_id).await?;
    let (covered, revision): (sqlx::types::Json<Vec<Uuid>>, i64) = sqlx::query_as(
        "SELECT grade_level_ids,row_version FROM curriculum_levels WHERE id=$1 FOR UPDATE",
    )
    .bind(level_id)
    .fetch_one(&mut *tx)
    .await?;
    if revision != request.destination_row_version {
        return Err(AppError::Conflict(
            "ระดับการศึกษาปลายทางถูกแก้ไขแล้ว กรุณาโหลดใหม่".into(),
        ));
    }
    let source: StudyProgram = {
        let sql = format!("SELECT {PROGRAM_COLUMNS} FROM study_programs WHERE id=$1 FOR SHARE");
        sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(request.source_program_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบแผนต้นทาง".into()))?
    };
    let source_status:VersionStatus=sqlx::query_scalar("SELECT e.status FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id WHERE l.id=$1 FOR SHARE OF e")
        .bind(source.curriculum_level_id).fetch_one(&mut *tx).await?;
    if source_status != VersionStatus::Published || source.status != VersionStatus::Published {
        return Err(AppError::Conflict("คัดลอกได้เฉพาะแผนจากฉบับที่เผยแพร่แล้ว".into()));
    }
    if source.row_version != request.source_row_version {
        return Err(AppError::Conflict("แผนต้นทางถูกแก้ไขแล้ว กรุณาโหลดใหม่".into()));
    }
    let (total,uncovered):(i64,i64)=sqlx::query_as("SELECT count(*),count(*) FILTER(WHERE NOT grade_level_id=ANY($2)) FROM (SELECT grade_level_id FROM curriculum_course_requirements WHERE study_program_id=$1 UNION ALL SELECT grade_level_id FROM curriculum_activity_requirements WHERE study_program_id=$1) r")
        .bind(source.id).bind(&covered.0).fetch_one(&mut *tx).await?;
    if total > MAX_COPIED_REQUIREMENTS || uncovered > 0 {
        return Err(AppError::ValidationError(
            "แผนต้นทางมีข้อมูลมากเกินกำหนดหรือมีชั้นที่ระดับปลายทางไม่ครอบคลุม".into(),
        ));
    }
    let name = request.name_th.as_deref().unwrap_or(&source.name_th).trim();
    validate_name(name)?;
    sqlx::query(r#"INSERT INTO curriculum_term_slots(id,curriculum_level_id,sequence,term_type,type_occurrence,name)
        SELECT gen_random_uuid(),$1,(SELECT COALESCE(max(sequence),0) FROM curriculum_term_slots WHERE curriculum_level_id=$1)+row_number() OVER(ORDER BY s.sequence,s.id)::integer,s.term_type,s.type_occurrence,s.name
        FROM curriculum_term_slots s WHERE s.curriculum_level_id=$2
        AND EXISTS(SELECT 1 FROM (SELECT term_slot_id FROM curriculum_course_requirements WHERE study_program_id=$3 UNION SELECT term_slot_id FROM curriculum_activity_requirements WHERE study_program_id=$3) used WHERE used.term_slot_id=s.id)
        AND NOT EXISTS(SELECT 1 FROM curriculum_term_slots t WHERE t.curriculum_level_id=$1 AND t.term_type=s.term_type AND t.type_occurrence=s.type_occurrence)"#)
        .bind(level_id).bind(source.curriculum_level_id).bind(source.id).execute(&mut *tx).await?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO study_programs(id,curriculum_level_id,code,name_th,name_en,is_default,status) SELECT $1,$2,$3,$4,$5,NOT EXISTS(SELECT 1 FROM study_programs WHERE curriculum_level_id=$2 AND is_default),'draft'")
        .bind(id).bind(level_id).bind(internal_code("PLAN",id)).bind(name).bind(&source.name_en).execute(&mut *tx).await?;
    sqlx::query(r#"INSERT INTO curriculum_course_requirements(id,curriculum_level_id,study_program_id,subject_version_id,grade_level_id,term_slot_id,requirement_kind,display_order)
        SELECT gen_random_uuid(),$1,$2,r.subject_version_id,r.grade_level_id,target.id,r.requirement_kind,r.display_order
        FROM curriculum_course_requirements r JOIN curriculum_term_slots original ON original.id=r.term_slot_id
        JOIN curriculum_term_slots target ON target.curriculum_level_id=$1 AND target.term_type=original.term_type AND target.type_occurrence=original.type_occurrence
        WHERE r.study_program_id=$3"#).bind(level_id).bind(id).bind(source.id).execute(&mut *tx).await?;
    sqlx::query(r#"INSERT INTO curriculum_activity_requirements(id,curriculum_level_id,study_program_id,activity_version_id,grade_level_id,term_slot_id,requirement_kind,display_order)
        SELECT gen_random_uuid(),$1,$2,r.activity_version_id,r.grade_level_id,target.id,r.requirement_kind,r.display_order
        FROM curriculum_activity_requirements r JOIN curriculum_term_slots original ON original.id=r.term_slot_id
        JOIN curriculum_term_slots target ON target.curriculum_level_id=$1 AND target.term_type=original.term_type AND target.type_occurrence=original.type_occurrence
        WHERE r.study_program_id=$3"#).bind(level_id).bind(id).bind(source.id).execute(&mut *tx).await?;
    let copied:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM curriculum_course_requirements WHERE study_program_id=$1)+(SELECT count(*) FROM curriculum_activity_requirements WHERE study_program_id=$1)").bind(id).fetch_one(&mut *tx).await?;
    if copied != total {
        return Err(AppError::Conflict(
            "คัดลอกข้อมูลได้ไม่ครบ กรุณาตรวจภาคเรียนต้นทางและปลายทาง".into(),
        ));
    }
    sqlx::query(
        "UPDATE curriculum_levels SET row_version=row_version+1,updated_at=now() WHERE id=$1",
    )
    .bind(level_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    get_program(pool, id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edition_inputs_and_generated_codes_are_bounded_and_stable() {
        assert!(validate_name("   ").is_err());
        assert!(validate_revision_year(2399).is_err());
        assert!(validate_revision_year(2569).is_ok());
        let id = Uuid::from_u128(42);
        assert_eq!(internal_code("LEVEL", id), format!("LEVEL-{}", id.simple()));
    }
}
