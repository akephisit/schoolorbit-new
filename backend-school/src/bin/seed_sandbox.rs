use chrono::{Datelike, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{env, error::Error, time::Duration};
use uuid::Uuid;

#[path = "../permissions/registry.rs"]
pub mod permission_registry;

pub mod permissions {
    pub use crate::permission_registry as registry;
}

#[path = "../utils/permission_sync.rs"]
pub mod permission_sync;

pub mod utils {
    pub use crate::permission_sync;
}

#[path = "../db/migration.rs"]
pub mod migration;

#[cfg(test)]
#[path = "../modules/academic/cutover_test_preflight.rs"]
mod cutover_test_preflight;

#[cfg(test)]
#[path = "../modules/academic/cutover_test_support.rs"]
mod cutover_test_support;

type SeedResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug)]
struct SeedConfig {
    subdomain: String,
    database_url: Option<String>,
    admin_api_url: Option<String>,
    internal_api_secret: Option<String>,
    seed_password: String,
    admin_username: String,
    student_password: String,
    parent_password: String,
    academic_year: i32,
    allow_non_sandbox: bool,
    run_migrations: bool,
}

#[derive(Debug, Deserialize)]
struct SchoolInfo {
    subdomain: String,
    status: String,
    db_connection_string: Option<String>,
}

#[derive(Debug)]
struct SeedSummary {
    admin_user_id: Uuid,
    student_user_id: Uuid,
    parent_user_id: Uuid,
    academic_year_id: Uuid,
    active_term_id: Uuid,
    grade_level_id: Uuid,
    curriculum_version_id: Uuid,
    study_program_id: Uuid,
    homeroom_id: Uuid,
}

#[tokio::main]
async fn main() -> SeedResult<()> {
    load_env_files()?;

    let config = SeedConfig::from_env()?;
    config.validate_scope()?;

    let database_url = match &config.database_url {
        Some(url) => url.clone(),
        None => fetch_database_url(&config).await?,
    };

    if !config.allow_non_sandbox && !database_url.to_ascii_lowercase().contains("sandbox") {
        return Err(
            "Refusing to seed a database URL that does not look like sandbox. Set SANDBOX_ALLOW_NON_SANDBOX=1 to override."
                .into(),
        );
    }

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(30))
        .connect(&database_url)
        .await?;

    if config.run_migrations {
        println!("Running tenant migrations...");
        migration::run_tenant_migrations(&pool).await?;
    }

    let summary = seed_database(&pool, &config).await?;

    println!("Sandbox seed completed.");
    println!("  subdomain: {}", config.subdomain);
    println!("  admin username: {}", config.admin_username);
    println!("  admin user id: {}", summary.admin_user_id);
    println!("  student username: SBX0001");
    println!("  student user id: {}", summary.student_user_id);
    println!("  parent username: P0001");
    println!("  parent user id: {}", summary.parent_user_id);
    println!("  academic year: {}", config.academic_year);
    println!("  academic year id: {}", summary.academic_year_id);
    println!("  active term id: {}", summary.active_term_id);
    println!("  grade level id: {}", summary.grade_level_id);
    println!("  curriculum version id: {}", summary.curriculum_version_id);
    println!("  study program id: {}", summary.study_program_id);
    println!("  homeroom id: {}", summary.homeroom_id);

    Ok(())
}

impl SeedConfig {
    fn from_env() -> SeedResult<Self> {
        let subdomain = env_or("SANDBOX_SUBDOMAIN", "sandbox");
        let academic_year = match env::var("SANDBOX_ACADEMIC_YEAR") {
            Ok(value) => value.parse::<i32>()?,
            Err(_) => default_thai_academic_year(),
        };

        let seed_password = env::var("SANDBOX_SEED_PASSWORD")
            .or_else(|_| env::var("SMOKE_PASSWORD"))
            .or_else(|_| env::var("E2E_PASSWORD"))
            .map_err(|_| {
                "Set SANDBOX_SEED_PASSWORD, SMOKE_PASSWORD, or E2E_PASSWORD before seeding sandbox"
            })?;

        let admin_username = env::var("SANDBOX_ADMIN_USERNAME")
            .or_else(|_| env::var("SMOKE_USERNAME"))
            .unwrap_or_else(|_| "T0001".to_string());

        Ok(Self {
            subdomain,
            database_url: env::var("SANDBOX_DATABASE_URL").ok(),
            admin_api_url: env::var("SANDBOX_ADMIN_API_URL")
                .or_else(|_| env::var("SMOKE_ADMIN_API_URL"))
                .or_else(|_| env::var("BACKEND_ADMIN_URL"))
                .ok(),
            internal_api_secret: env::var("INTERNAL_API_SECRET").ok(),
            student_password: env::var("SANDBOX_STUDENT_PASSWORD")
                .unwrap_or_else(|_| seed_password.clone()),
            parent_password: env::var("SANDBOX_PARENT_PASSWORD")
                .unwrap_or_else(|_| seed_password.clone()),
            seed_password,
            admin_username,
            academic_year,
            allow_non_sandbox: env_bool("SANDBOX_ALLOW_NON_SANDBOX"),
            run_migrations: !env_bool("SANDBOX_SKIP_MIGRATIONS"),
        })
    }

    fn validate_scope(&self) -> SeedResult<()> {
        if !self.allow_non_sandbox && self.subdomain != "sandbox" {
            return Err(
                "Refusing to seed a non-sandbox subdomain. Set SANDBOX_ALLOW_NON_SANDBOX=1 to override."
                    .into(),
            );
        }
        Ok(())
    }
}

fn load_env_files() -> SeedResult<()> {
    load_optional_env_file("../.env")?;
    load_optional_env_file(".env")?;

    Ok(())
}

fn load_optional_env_file(path: &str) -> SeedResult<()> {
    match dotenvy::from_filename(path) {
        Ok(_) => Ok(()),
        Err(error) if error.not_found() => Ok(()),
        Err(error) => Err(format!("Failed to load {path}: {error}").into()),
    }
}

fn env_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

fn env_bool(name: &str) -> bool {
    matches!(
        env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn default_thai_academic_year() -> i32 {
    let today = Utc::now().date_naive();
    let gregorian_year = today.year();
    if today.month() >= 5 {
        gregorian_year + 543
    } else {
        gregorian_year + 542
    }
}

async fn fetch_database_url(config: &SeedConfig) -> SeedResult<String> {
    let admin_api_url = config.admin_api_url.as_deref().ok_or(
        "Set SANDBOX_DATABASE_URL, or set SANDBOX_ADMIN_API_URL/BACKEND_ADMIN_URL with INTERNAL_API_SECRET",
    )?;
    let internal_api_secret = config
        .internal_api_secret
        .as_deref()
        .ok_or("Set INTERNAL_API_SECRET when resolving sandbox database URL from backend-admin")?;

    let url = format!(
        "{}/internal/schools/{}",
        admin_api_url.trim_end_matches('/'),
        config.subdomain
    );

    let response = reqwest::Client::new()
        .get(url)
        .header("X-Internal-Secret", internal_api_secret)
        .header("X-Internal-Caller", "seed-sandbox")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!(
            "backend-admin returned {} while resolving sandbox database URL",
            response.status()
        )
        .into());
    }

    let school = response.json::<SchoolInfo>().await?;
    if school.subdomain != config.subdomain {
        return Err(format!(
            "backend-admin returned subdomain '{}' but expected '{}'",
            school.subdomain, config.subdomain
        )
        .into());
    }
    if school.status != "active" && school.status != "provisioning" {
        return Err(format!("school '{}' is not active", school.subdomain).into());
    }

    school
        .db_connection_string
        .ok_or_else(|| "backend-admin returned no db_connection_string".into())
}

async fn seed_database(pool: &PgPool, config: &SeedConfig) -> SeedResult<SeedSummary> {
    let mut tx = pool.begin().await?;

    let admin_user_id = upsert_admin_user(&mut tx, config).await?;
    let parent_user_id = upsert_parent_user(&mut tx, config).await?;
    let student_user_id = upsert_student_user(&mut tx, config).await?;
    ensure_student_parent_link(&mut tx, student_user_id, parent_user_id).await?;

    let academic_year_id = upsert_academic_year(&mut tx, config.academic_year).await?;
    let active_term_id = upsert_terms(&mut tx, config.academic_year, academic_year_id).await?;
    let grade_level_id = upsert_grade_level(&mut tx).await?;
    ensure_year_grade_level(&mut tx, academic_year_id, grade_level_id).await?;
    let (curriculum_version_id, study_program_id) = upsert_curriculum_program(
        &mut tx,
        academic_year_id,
        grade_level_id,
        config.academic_year,
    )
    .await?;
    let homeroom_id = upsert_homeroom(
        &mut tx,
        academic_year_id,
        grade_level_id,
        study_program_id,
        config.academic_year,
    )
    .await?;
    ensure_homeroom_advisor(&mut tx, homeroom_id, admin_user_id).await?;
    ensure_student_info(&mut tx, student_user_id).await?;
    ensure_student_year_placement(
        &mut tx,
        student_user_id,
        academic_year_id,
        grade_level_id,
        study_program_id,
        homeroom_id,
    )
    .await?;

    tx.commit().await?;

    Ok(SeedSummary {
        admin_user_id,
        student_user_id,
        parent_user_id,
        academic_year_id,
        active_term_id,
        grade_level_id,
        curriculum_version_id,
        study_program_id,
        homeroom_id,
    })
}

async fn upsert_admin_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    config: &SeedConfig,
) -> SeedResult<Uuid> {
    let password_hash = bcrypt::hash(&config.seed_password, bcrypt::DEFAULT_COST)?;
    let user_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO users (
            username, national_id, national_id_hash, email, password_hash,
            title, first_name, last_name, user_type, status, metadata
        )
        VALUES ($1, NULL, NULL, NULL, $2, 'นาย', 'Sandbox', 'Admin', 'staff', 'active',
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (username) DO UPDATE SET
            password_hash = EXCLUDED.password_hash,
            title = EXCLUDED.title,
            first_name = EXCLUDED.first_name,
            last_name = EXCLUDED.last_name,
            user_type = 'staff',
            status = 'active',
            metadata = COALESCE(users.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(&config.admin_username)
    .bind(password_hash)
    .fetch_one(&mut **tx)
    .await?;

    let admin_role_id = ensure_admin_role(tx).await?;
    ensure_user_role(tx, user_id, admin_role_id).await?;

    sqlx::query(
        r#"
        INSERT INTO staff_info (user_id, employment_type, metadata)
        VALUES ($1, 'permanent', jsonb_build_object('seed', 'sandbox'))
        ON CONFLICT (user_id) DO UPDATE SET
            employment_type = EXCLUDED.employment_type,
            metadata = COALESCE(staff_info.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(user_id)
}

async fn ensure_admin_role(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> SeedResult<Uuid> {
    let role_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO roles (code, name, name_en, description, user_type, level, is_active)
        VALUES ('ADMIN', 'ผู้ดูแลระบบ', 'System Admin', 'Seeded sandbox admin role', 'staff', 999, true)
        ON CONFLICT (code) DO UPDATE SET
            name = EXCLUDED.name,
            name_en = EXCLUDED.name_en,
            user_type = EXCLUDED.user_type,
            level = EXCLUDED.level,
            is_active = true,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .fetch_one(&mut **tx)
    .await?;

    let permission_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO permissions (code, name, module, action, scope, description)
        VALUES ('*', 'ทั้งหมด (Wildcard)', 'system', '*', 'all', 'สิทธิ์เข้าถึงทุกอย่างในระบบ')
        ON CONFLICT (code) DO UPDATE SET
            name = EXCLUDED.name,
            module = EXCLUDED.module,
            action = EXCLUDED.action,
            scope = EXCLUDED.scope,
            description = EXCLUDED.description,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        "INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(role_id)
    .bind(permission_id)
    .execute(&mut **tx)
    .await?;

    Ok(role_id)
}

async fn ensure_user_role(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    role_id: Uuid,
) -> SeedResult<()> {
    sqlx::query("UPDATE user_roles SET is_primary = false WHERE user_id = $1 AND ended_at IS NULL")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;

    sqlx::query(
        r#"
        INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
        SELECT $1, $2, true, CURRENT_DATE
        WHERE NOT EXISTS (
            SELECT 1 FROM user_roles
            WHERE user_id = $1 AND role_id = $2 AND ended_at IS NULL
        )
        "#,
    )
    .bind(user_id)
    .bind(role_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        "UPDATE user_roles SET is_primary = true WHERE user_id = $1 AND role_id = $2 AND ended_at IS NULL",
    )
    .bind(user_id)
    .bind(role_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_parent_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    config: &SeedConfig,
) -> SeedResult<Uuid> {
    let password_hash = bcrypt::hash(&config.parent_password, bcrypt::DEFAULT_COST)?;
    let user_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO users (
            username, national_id, national_id_hash, email, password_hash,
            title, first_name, last_name, phone, user_type, status, metadata
        )
        VALUES ('P0001', NULL, NULL, 'sandbox.parent@example.test', $1,
                'นาง', 'ผู้ปกครอง', 'ทดสอบ', '0800000001', 'parent', 'active',
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (username) DO UPDATE SET
            password_hash = EXCLUDED.password_hash,
            email = EXCLUDED.email,
            title = EXCLUDED.title,
            first_name = EXCLUDED.first_name,
            last_name = EXCLUDED.last_name,
            phone = EXCLUDED.phone,
            user_type = 'parent',
            status = 'active',
            metadata = COALESCE(users.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(password_hash)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO parent_info (user_id, relationship, occupation, metadata)
        VALUES ($1, 'guardian', 'Sandbox fixture', jsonb_build_object('seed', 'sandbox'))
        ON CONFLICT (user_id) DO UPDATE SET
            relationship = EXCLUDED.relationship,
            occupation = EXCLUDED.occupation,
            metadata = COALESCE(parent_info.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(user_id)
}

async fn upsert_student_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    config: &SeedConfig,
) -> SeedResult<Uuid> {
    let password_hash = bcrypt::hash(&config.student_password, bcrypt::DEFAULT_COST)?;
    let user_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO users (
            username, national_id, national_id_hash, email, password_hash,
            title, first_name, last_name, user_type, status, date_of_birth, gender, metadata
        )
        VALUES ('SBX0001', NULL, NULL, 'sandbox.student@example.test', $1,
                'เด็กชาย', 'นักเรียน', 'ทดสอบ', 'student', 'active', DATE '2013-05-01', 'male',
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (username) DO UPDATE SET
            password_hash = EXCLUDED.password_hash,
            email = EXCLUDED.email,
            title = EXCLUDED.title,
            first_name = EXCLUDED.first_name,
            last_name = EXCLUDED.last_name,
            user_type = 'student',
            status = 'active',
            date_of_birth = EXCLUDED.date_of_birth,
            gender = EXCLUDED.gender,
            metadata = COALESCE(users.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(password_hash)
    .fetch_one(&mut **tx)
    .await?;

    Ok(user_id)
}

async fn ensure_student_parent_link(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    student_user_id: Uuid,
    parent_user_id: Uuid,
) -> SeedResult<()> {
    sqlx::query(
        r#"
        INSERT INTO student_parents (student_user_id, parent_user_id, relationship, is_primary)
        VALUES ($1, $2, 'guardian', true)
        ON CONFLICT (student_user_id, parent_user_id) DO UPDATE SET
            relationship = EXCLUDED.relationship,
            is_primary = true,
            updated_at = NOW()
        "#,
    )
    .bind(student_user_id)
    .bind(parent_user_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_academic_year(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    academic_year: i32,
) -> SeedResult<Uuid> {
    let start_year = academic_year - 543;
    let start_date = NaiveDate::from_ymd_opt(start_year, 5, 16).ok_or("invalid start date")?;
    let end_date = NaiveDate::from_ymd_opt(start_year + 1, 3, 31).ok_or("invalid end date")?;

    sqlx::query(
        "UPDATE academic_years
         SET status = 'closed', row_version = row_version + 1, updated_at = NOW()
         WHERE status = 'active' AND year <> $1",
    )
    .bind(academic_year)
    .execute(&mut **tx)
    .await?;

    let academic_year_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO academic_years (year, name, start_date, end_date, school_days, status, metadata)
        VALUES ($1, $2, $3, $4, 'MON,TUE,WED,THU,FRI', 'active',
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (year) DO UPDATE SET
            name = EXCLUDED.name,
            start_date = EXCLUDED.start_date,
            end_date = EXCLUDED.end_date,
            status = 'active',
            school_days = EXCLUDED.school_days,
            row_version = academic_years.row_version + 1,
            metadata = COALESCE(academic_years.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(academic_year)
    .bind(format!("ปีการศึกษา {}", academic_year))
    .bind(start_date)
    .bind(end_date)
    .fetch_one(&mut **tx)
    .await?;

    Ok(academic_year_id)
}

async fn upsert_terms(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    academic_year: i32,
    academic_year_id: Uuid,
) -> SeedResult<Uuid> {
    let start_year = academic_year - 543;
    let term_1_start = NaiveDate::from_ymd_opt(start_year, 5, 16).ok_or("invalid term date")?;
    let term_1_end = NaiveDate::from_ymd_opt(start_year, 10, 10).ok_or("invalid term date")?;
    let term_2_start = NaiveDate::from_ymd_opt(start_year, 10, 20).ok_or("invalid term date")?;
    let term_2_end = NaiveDate::from_ymd_opt(start_year + 1, 3, 31).ok_or("invalid term date")?;

    let bell_schedule_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO bell_schedules (
            id, academic_year_id, code, name, is_default, status
        )
        VALUES ($1, $2, 'DEFAULT', 'ตารางคาบมาตรฐาน Sandbox', true, 'published')
        ON CONFLICT (academic_year_id, code) DO UPDATE SET
            name = EXCLUDED.name,
            is_default = true,
            status = 'published',
            row_version = bell_schedules.row_version + 1,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(Uuid::new_v5(
        &Uuid::parse_str("5c33b984-10df-58db-bf80-62dbc4a03d1b")?,
        format!("sandbox-bell-schedule:{academic_year_id}").as_bytes(),
    ))
    .bind(academic_year_id)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        "UPDATE academic_terms
         SET status = 'closed',
             closed_on = COALESCE(closed_on, planned_end_date, CURRENT_DATE),
             row_version = row_version + 1, updated_at = NOW()
         WHERE status = 'active' AND academic_year_id <> $1",
    )
    .bind(academic_year_id)
    .execute(&mut **tx)
    .await?;

    let active_term_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO academic_terms (
            academic_year_id, sequence_no, code, name, term_type, start_date, planned_end_date,
            included_in_year_result, blocks_year_closure, bell_schedule_id, status, metadata
        )
        VALUES ($1, 1, 'TERM-1', 'ภาคเรียนที่ 1', 'regular', $2, $3,
                true, true, $4, 'active',
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (academic_year_id, code) DO UPDATE SET
            sequence_no = 1,
            name = EXCLUDED.name,
            term_type = EXCLUDED.term_type,
            start_date = EXCLUDED.start_date,
            planned_end_date = EXCLUDED.planned_end_date,
            included_in_year_result = true,
            blocks_year_closure = true,
            bell_schedule_id = EXCLUDED.bell_schedule_id,
            status = 'active',
            row_version = academic_terms.row_version + 1,
            metadata = COALESCE(academic_terms.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(academic_year_id)
    .bind(term_1_start)
    .bind(term_1_end)
    .bind(bell_schedule_id)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO academic_terms (
            academic_year_id, sequence_no, code, name, term_type, start_date, planned_end_date,
            included_in_year_result, blocks_year_closure, bell_schedule_id, status, metadata
        )
        VALUES ($1, 2, 'TERM-2', 'ภาคเรียนที่ 2', 'regular', $2, $3,
                true, true, $4, 'ready',
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (academic_year_id, code) DO UPDATE SET
            sequence_no = 2,
            name = EXCLUDED.name,
            term_type = EXCLUDED.term_type,
            start_date = EXCLUDED.start_date,
            planned_end_date = EXCLUDED.planned_end_date,
            included_in_year_result = true,
            blocks_year_closure = true,
            bell_schedule_id = EXCLUDED.bell_schedule_id,
            status = 'ready',
            row_version = academic_terms.row_version + 1,
            metadata = COALESCE(academic_terms.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        "#,
    )
    .bind(academic_year_id)
    .bind(term_2_start)
    .bind(term_2_end)
    .bind(bell_schedule_id)
    .execute(&mut **tx)
    .await?;

    Ok(active_term_id)
}

async fn upsert_grade_level(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> SeedResult<Uuid> {
    let grade_level_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO grade_levels (level_type, year, is_active)
        VALUES ('secondary', 1, true)
        ON CONFLICT (level_type, year) DO UPDATE SET
            is_active = true,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(grade_level_id)
}

async fn ensure_year_grade_level(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
) -> SeedResult<()> {
    sqlx::query(
        "INSERT INTO academic_year_grade_levels (academic_year_id, grade_level_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(academic_year_id)
    .bind(grade_level_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_curriculum_program(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
    academic_year: i32,
) -> SeedResult<(Uuid, Uuid)> {
    let grade_ids = json!([grade_level_id]);
    let curriculum_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO curricula (
            code, identity_key, name_th, name_en, description, grade_level_ids, is_active
        )
        VALUES ('SBX-GEN', 'sbx-gen', 'Sandbox General', 'Sandbox General',
                'Minimal sandbox fixture for smoke and E2E tests', $1, true)
        ON CONFLICT (code) DO UPDATE SET
            identity_key = EXCLUDED.identity_key,
            name_th = EXCLUDED.name_th,
            name_en = EXCLUDED.name_en,
            description = EXCLUDED.description,
            grade_level_ids = EXCLUDED.grade_level_ids,
            is_active = true,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(grade_ids)
    .fetch_one(&mut **tx)
    .await?;

    let version_name = format!("Sandbox {}", academic_year);
    sqlx::query(
        r#"
        INSERT INTO curriculum_versions (
            curriculum_id, version_name, start_academic_year_id, description, is_active, status
        )
        VALUES ($1, $2, $3, 'Seeded sandbox curriculum version', true, 'draft')
        ON CONFLICT (curriculum_id, version_name) DO NOTHING
        "#,
    )
    .bind(curriculum_id)
    .bind(&version_name)
    .bind(academic_year_id)
    .execute(&mut **tx)
    .await?;

    let curriculum_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM curriculum_versions
         WHERE curriculum_id = $1 AND version_name = $2",
    )
    .bind(curriculum_id)
    .bind(&version_name)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO study_programs (
            id, curriculum_version_id, code, name_th, name_en, is_default, status
        )
        SELECT $1, $2, 'GENERAL', 'แผนการเรียนทั่วไป Sandbox',
               'Sandbox General', true, 'draft'
        WHERE NOT EXISTS (
            SELECT 1 FROM study_programs
            WHERE curriculum_version_id = $2 AND code = 'GENERAL'
        )
        ON CONFLICT (curriculum_version_id, code) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v5(
        &Uuid::parse_str("5c33b984-10df-58db-bf80-62dbc4a03d1b")?,
        format!("sandbox-study-program:{curriculum_version_id}:GENERAL").as_bytes(),
    ))
    .bind(curriculum_version_id)
    .execute(&mut **tx)
    .await?;

    let study_program_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM study_programs
         WHERE curriculum_version_id = $1 AND code = 'GENERAL'",
    )
    .bind(curriculum_version_id)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        "UPDATE study_programs
         SET status = 'published', row_version = row_version + 1, updated_at = NOW()
         WHERE id = $1 AND status = 'draft'",
    )
    .bind(study_program_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        "UPDATE curriculum_versions
         SET status = 'published', published_at = NOW(),
             row_version = row_version + 1, updated_at = NOW()
         WHERE id = $1 AND status = 'draft'",
    )
    .bind(curriculum_version_id)
    .execute(&mut **tx)
    .await?;

    Ok((curriculum_version_id, study_program_id))
}

async fn upsert_homeroom(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
    academic_year: i32,
) -> SeedResult<Uuid> {
    let short_year = academic_year % 100;
    let homeroom_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO homerooms (
            code, name, academic_year_id, grade_level_id, room_number,
            study_program_id, capacity, is_active, metadata
        )
        VALUES ($1, 'ม.1/1', $2, $3, '1', $4, 40, true,
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (academic_year_id, grade_level_id, room_number) DO UPDATE SET
            code = EXCLUDED.code,
            name = EXCLUDED.name,
            study_program_id = EXCLUDED.study_program_id,
            capacity = EXCLUDED.capacity,
            is_active = true,
            row_version = homerooms.row_version + 1,
            metadata = COALESCE(homerooms.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(format!("{}-M1-1", short_year))
    .bind(academic_year_id)
    .bind(grade_level_id)
    .bind(study_program_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(homeroom_id)
}

async fn ensure_homeroom_advisor(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    homeroom_id: Uuid,
    user_id: Uuid,
) -> SeedResult<()> {
    sqlx::query("DELETE FROM homeroom_advisors WHERE homeroom_id = $1 AND role = 'primary'")
        .bind(homeroom_id)
        .execute(&mut **tx)
        .await?;

    sqlx::query(
        r#"
        INSERT INTO homeroom_advisors (homeroom_id, user_id, role)
        VALUES ($1, $2, 'primary')
        ON CONFLICT (homeroom_id, user_id) DO UPDATE SET role = 'primary'
        "#,
    )
    .bind(homeroom_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn ensure_student_info(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    student_user_id: Uuid,
) -> SeedResult<()> {
    sqlx::query(
        r#"
        INSERT INTO student_info (user_id, student_id, student_number, enrollment_date, metadata)
        VALUES ($1, 'SBX0001', 1, CURRENT_DATE,
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (user_id) DO UPDATE SET
            student_id = EXCLUDED.student_id,
            student_number = EXCLUDED.student_number,
            enrollment_date = EXCLUDED.enrollment_date,
            metadata = COALESCE(student_info.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        "#,
    )
    .bind(student_user_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn ensure_student_year_placement(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    student_user_id: Uuid,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
    homeroom_id: Uuid,
) -> SeedResult<()> {
    let student_academic_year_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO student_academic_years (
            id, student_id, academic_year_id, grade_level_id, study_program_id,
            status, migration_provenance
        )
        VALUES ($1, $2, $3, $4, $5, 'active',
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (student_id, academic_year_id) DO UPDATE SET
            grade_level_id = EXCLUDED.grade_level_id,
            study_program_id = EXCLUDED.study_program_id,
            status = 'active',
            row_version = student_academic_years.row_version + 1,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(Uuid::new_v5(
        &Uuid::parse_str("5c33b984-10df-58db-bf80-62dbc4a03d1b")?,
        format!("sandbox-student-year:{student_user_id}:{academic_year_id}").as_bytes(),
    ))
    .bind(student_user_id)
    .bind(academic_year_id)
    .bind(grade_level_id)
    .bind(study_program_id)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        "UPDATE homeroom_placements
         SET status = 'ended', end_date = GREATEST(start_date, CURRENT_DATE),
             row_version = row_version + 1, updated_at = NOW()
         WHERE student_academic_year_id = $1
           AND homeroom_id <> $2
           AND status = 'current'",
    )
    .bind(student_academic_year_id)
    .bind(homeroom_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO homeroom_placements (
            id, student_academic_year_id, academic_year_id, homeroom_id,
            start_date, status, enrollment_type, class_number, metadata,
            migration_provenance
        )
        VALUES ($1, $2, $3, $4, CURRENT_DATE, 'current', 'regular', 1,
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'),
                jsonb_build_object('source', 'seed_sandbox'))
        ON CONFLICT (id) DO UPDATE SET
            homeroom_id = EXCLUDED.homeroom_id,
            status = 'current',
            end_date = NULL,
            enrollment_type = EXCLUDED.enrollment_type,
            class_number = EXCLUDED.class_number,
            metadata = homeroom_placements.metadata || EXCLUDED.metadata,
            row_version = homeroom_placements.row_version + 1,
            updated_at = NOW()
        "#,
    )
    .bind(Uuid::new_v5(
        &Uuid::parse_str("5c33b984-10df-58db-bf80-62dbc4a03d1b")?,
        format!("sandbox-placement:{student_user_id}:{academic_year_id}").as_bytes(),
    ))
    .bind(student_academic_year_id)
    .bind(academic_year_id)
    .bind(homeroom_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SeedConfig {
        SeedConfig {
            subdomain: "sandbox".to_string(),
            database_url: None,
            admin_api_url: None,
            internal_api_secret: None,
            seed_password: "sandbox-test-only".to_string(),
            admin_username: "T0001".to_string(),
            student_password: "sandbox-test-only".to_string(),
            parent_password: "sandbox-test-only".to_string(),
            academic_year: 2569,
            allow_non_sandbox: false,
            run_migrations: false,
        }
    }

    #[tokio::test]
    async fn canonical_seed_is_idempotent_across_student_year_and_placement() {
        let database_url =
            env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is provided by test script");
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("connect disposable database");
        cutover_test_support::apply_migrations_through(&pool, 40)
            .await
            .expect("apply pre-cutover migrations");
        cutover_test_support::seed_academic_cutover_fixture(
            &pool,
            cutover_test_support::CutoverFixture::Passing,
        )
        .await
        .expect("seed passing cutover fixture");
        cutover_test_support::apply_phase_b_runtime_migrations(&pool)
            .await
            .expect("apply academic cutover migrations");

        let config = test_config();
        let first = seed_database(&pool, &config).await.expect("first seed");
        let second = seed_database(&pool, &config).await.expect("second seed");

        assert_eq!(first.student_user_id, second.student_user_id);
        assert_eq!(first.academic_year_id, second.academic_year_id);
        assert_eq!(first.study_program_id, second.study_program_id);
        assert_eq!(first.homeroom_id, second.homeroom_id);

        let student_year_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM student_academic_years
             WHERE student_id = $1 AND academic_year_id = $2",
        )
        .bind(first.student_user_id)
        .bind(first.academic_year_id)
        .fetch_one(&pool)
        .await
        .expect("count student year");
        let placement_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM homeroom_placements placement
             JOIN student_academic_years student_year
               ON student_year.id = placement.student_academic_year_id
             WHERE student_year.student_id = $1
               AND student_year.academic_year_id = $2
               AND placement.status = 'current'",
        )
        .bind(first.student_user_id)
        .bind(first.academic_year_id)
        .fetch_one(&pool)
        .await
        .expect("count current placement");

        assert_eq!(student_year_count, 1);
        assert_eq!(placement_count, 1);
    }
}
