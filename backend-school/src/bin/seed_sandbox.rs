use chrono::{Datelike, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{env, error::Error, time::Duration};
use uuid::Uuid;

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
    active_semester_id: Uuid,
    grade_level_id: Uuid,
    study_plan_version_id: Uuid,
    classroom_id: Uuid,
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
        sqlx::migrate!("./migrations").run(&pool).await?;
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
    println!("  active semester id: {}", summary.active_semester_id);
    println!("  grade level id: {}", summary.grade_level_id);
    println!("  study plan version id: {}", summary.study_plan_version_id);
    println!("  classroom id: {}", summary.classroom_id);

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
    let active_semester_id =
        upsert_semesters(&mut tx, config.academic_year, academic_year_id).await?;
    let grade_level_id = upsert_grade_level(&mut tx).await?;
    ensure_year_grade_level(&mut tx, academic_year_id, grade_level_id).await?;
    let study_plan_version_id = upsert_study_plan_version(
        &mut tx,
        academic_year_id,
        grade_level_id,
        config.academic_year,
    )
    .await?;
    let classroom_id = upsert_classroom(
        &mut tx,
        academic_year_id,
        grade_level_id,
        study_plan_version_id,
        config.academic_year,
    )
    .await?;
    ensure_classroom_advisor(&mut tx, classroom_id, admin_user_id).await?;
    ensure_student_info(&mut tx, student_user_id).await?;
    ensure_student_enrollment(&mut tx, student_user_id, classroom_id).await?;

    tx.commit().await?;

    Ok(SeedSummary {
        admin_user_id,
        student_user_id,
        parent_user_id,
        academic_year_id,
        active_semester_id,
        grade_level_id,
        study_plan_version_id,
        classroom_id,
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

    sqlx::query("UPDATE academic_years SET is_active = false WHERE is_active = true")
        .execute(&mut **tx)
        .await?;

    let academic_year_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO academic_years (year, name, start_date, end_date, is_active, school_days, metadata)
        VALUES ($1, $2, $3, $4, true, 'MON,TUE,WED,THU,FRI',
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (year) DO UPDATE SET
            name = EXCLUDED.name,
            start_date = EXCLUDED.start_date,
            end_date = EXCLUDED.end_date,
            is_active = true,
            school_days = EXCLUDED.school_days,
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

async fn upsert_semesters(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    academic_year: i32,
    academic_year_id: Uuid,
) -> SeedResult<Uuid> {
    let start_year = academic_year - 543;
    let term_1_start = NaiveDate::from_ymd_opt(start_year, 5, 16).ok_or("invalid term date")?;
    let term_1_end = NaiveDate::from_ymd_opt(start_year, 10, 10).ok_or("invalid term date")?;
    let term_2_start = NaiveDate::from_ymd_opt(start_year, 10, 20).ok_or("invalid term date")?;
    let term_2_end = NaiveDate::from_ymd_opt(start_year + 1, 3, 31).ok_or("invalid term date")?;

    sqlx::query("UPDATE academic_semesters SET is_active = false WHERE is_active = true")
        .execute(&mut **tx)
        .await?;

    let active_semester_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO academic_semesters (academic_year_id, term, name, start_date, end_date, is_active, metadata)
        VALUES ($1, '1', 'ภาคเรียนที่ 1', $2, $3, true,
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (academic_year_id, term) DO UPDATE SET
            name = EXCLUDED.name,
            start_date = EXCLUDED.start_date,
            end_date = EXCLUDED.end_date,
            is_active = true,
            metadata = COALESCE(academic_semesters.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(academic_year_id)
    .bind(term_1_start)
    .bind(term_1_end)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO academic_semesters (academic_year_id, term, name, start_date, end_date, is_active, metadata)
        VALUES ($1, '2', 'ภาคเรียนที่ 2', $2, $3, false,
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (academic_year_id, term) DO UPDATE SET
            name = EXCLUDED.name,
            start_date = EXCLUDED.start_date,
            end_date = EXCLUDED.end_date,
            metadata = COALESCE(academic_semesters.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        "#,
    )
    .bind(academic_year_id)
    .bind(term_2_start)
    .bind(term_2_end)
    .execute(&mut **tx)
    .await?;

    Ok(active_semester_id)
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

async fn upsert_study_plan_version(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
    academic_year: i32,
) -> SeedResult<Uuid> {
    let grade_ids = json!([grade_level_id]);
    let study_plan_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO study_plans (code, name_th, name_en, description, grade_level_ids, is_active)
        VALUES ('SBX-GEN', 'Sandbox General', 'Sandbox General',
                'Minimal sandbox fixture for smoke and E2E tests', $1, true)
        ON CONFLICT (code) DO UPDATE SET
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
    let version_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO study_plan_versions (
            study_plan_id, version_name, start_academic_year_id, description, is_active
        )
        VALUES ($1, $2, $3, 'Seeded sandbox curriculum version', true)
        ON CONFLICT (study_plan_id, version_name) DO UPDATE SET
            start_academic_year_id = EXCLUDED.start_academic_year_id,
            description = EXCLUDED.description,
            is_active = true,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(study_plan_id)
    .bind(version_name)
    .bind(academic_year_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(version_id)
}

async fn upsert_classroom(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
    study_plan_version_id: Uuid,
    academic_year: i32,
) -> SeedResult<Uuid> {
    let short_year = academic_year % 100;
    let classroom_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO class_rooms (
            code, name, academic_year_id, grade_level_id, room_number,
            study_plan_version_id, capacity, is_active, metadata
        )
        VALUES ($1, 'ม.1/1', $2, $3, '1', $4, 40, true,
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (academic_year_id, grade_level_id, room_number) DO UPDATE SET
            code = EXCLUDED.code,
            name = EXCLUDED.name,
            study_plan_version_id = EXCLUDED.study_plan_version_id,
            capacity = EXCLUDED.capacity,
            is_active = true,
            metadata = COALESCE(class_rooms.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(format!("{}-M1-1", short_year))
    .bind(academic_year_id)
    .bind(grade_level_id)
    .bind(study_plan_version_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(classroom_id)
}

async fn ensure_classroom_advisor(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    classroom_id: Uuid,
    user_id: Uuid,
) -> SeedResult<()> {
    sqlx::query("DELETE FROM classroom_advisors WHERE classroom_id = $1 AND role = 'primary'")
        .bind(classroom_id)
        .execute(&mut **tx)
        .await?;

    sqlx::query(
        r#"
        INSERT INTO classroom_advisors (classroom_id, user_id, role)
        VALUES ($1, $2, 'primary')
        ON CONFLICT (classroom_id, user_id) DO UPDATE SET role = 'primary'
        "#,
    )
    .bind(classroom_id)
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

async fn ensure_student_enrollment(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    student_user_id: Uuid,
    classroom_id: Uuid,
) -> SeedResult<()> {
    sqlx::query(
        "UPDATE student_class_enrollments SET status = 'transferred', updated_at = NOW()
         WHERE student_id = $1 AND class_room_id <> $2 AND status = 'active'",
    )
    .bind(student_user_id)
    .bind(classroom_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO student_class_enrollments (
            student_id, class_room_id, enrollment_date, status, enrollment_type, class_number, metadata
        )
        VALUES ($1, $2, CURRENT_DATE, 'active', 'regular', 1,
                jsonb_build_object('seed', 'sandbox', 'managed_by', 'seed_sandbox'))
        ON CONFLICT (student_id, class_room_id) DO UPDATE SET
            enrollment_date = EXCLUDED.enrollment_date,
            status = 'active',
            enrollment_type = EXCLUDED.enrollment_type,
            class_number = EXCLUDED.class_number,
            metadata = COALESCE(student_class_enrollments.metadata, '{}'::jsonb) || EXCLUDED.metadata,
            updated_at = NOW()
        "#,
    )
    .bind(student_user_id)
    .bind(classroom_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}
