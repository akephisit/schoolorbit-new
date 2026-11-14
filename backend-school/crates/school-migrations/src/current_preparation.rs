//! Connection-local admission evidence for the single current-operations cutover.
//! Expectations constrain the actual migration; they never choose its date/source.
use chrono::NaiveDate;
use school_database::DbSession;
use sqlx::{
    migrate::{MigrateError, Migrator},
    PgConnection,
};

#[derive(Clone, Debug)]
pub struct CurrentPreparationExpectation {
    pub cutover_date: NaiveDate,
    pub selection_fingerprint: [u8; 32],
}

pub(super) enum PreparationPolicy<'a> {
    Ordinary,
    Operational(Option<&'a CurrentPreparationExpectation>),
    FreshOrCurrent,
}

pub(super) async fn verified_prefix(
    connection: &mut PgConnection,
    migrations: &Migrator,
) -> Result<i64, MigrateError> {
    let exists: bool = DbSession::new(&mut *connection)
        .scalar(
            "migrations.preparation.history_exists",
            "SELECT to_regclass(format('%I._sqlx_migrations',current_schema())) IS NOT NULL",
        )
        .fetch_one()
        .await?;
    if !exists {
        return Ok(0);
    }
    let applied: Vec<(i64, bool, Vec<u8>)> = DbSession::new(connection)
        .query_as(
            "migrations.preparation.history",
            "SELECT version,success,checksum FROM _sqlx_migrations ORDER BY version",
        )
        .fetch_all()
        .await?;
    for (index, (version, success, checksum)) in applied.iter().enumerate() {
        if !success {
            return Err(MigrateError::Dirty(*version));
        }
        let expected = migrations
            .iter()
            .find(|m| m.version == *version)
            .ok_or(MigrateError::VersionMissing(*version))?;
        if expected.checksum.as_ref() != checksum.as_slice() {
            return Err(MigrateError::VersionMismatch(*version));
        }
        if *version != index as i64 + 1 {
            return Err(MigrateError::Source(
                std::io::Error::other("Canonical migration history is not a contiguous prefix")
                    .into(),
            ));
        }
    }
    Ok(applied.last().map_or(0, |r| r.0))
}

async fn empty_initialization(connection: &mut PgConnection) -> Result<bool, sqlx::Error> {
    // Canonical seed/settings rows are allowed. Every identity and scoped-content
    // anchor must be absent; provisioning inserts its first user only after migration.
    for table in [
        "users",
        "academic_years",
        "academic_semesters",
        "subjects",
        "activities",
        "subject_catalog",
        "activity_catalog",
        "curriculum_editions",
        "curriculum_versions",
        "learning_offerings",
        "learning_groups",
        "academic_timetable_versions",
        "academic_timetables",
        "attendance_sessions",
        "files",
        "admission_applications",
        "budget_projects",
        "calendar_events",
    ] {
        let exists: bool = DbSession::new(&mut *connection)
            .scalar(
                "migrations.preparation.initialization_relation",
                "SELECT to_regclass(format('%I.%I',current_schema(),$1::text)) IS NOT NULL",
            )
            .bind(table)
            .fetch_one()
            .await?;
        if exists {
            let query = format!("SELECT EXISTS(SELECT 1 FROM \"{table}\")");
            let populated: bool = DbSession::new(&mut *connection)
                .scalar(
                    "migrations.preparation.initialization_empty",
                    sqlx::AssertSqlSafe(query),
                )
                .fetch_one()
                .await?;
            if populated {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(super) async fn install(
    connection: &mut PgConnection,
    policy: PreparationPolicy<'_>,
    migrations: &Migrator,
) -> Result<bool, MigrateError> {
    let expected = match policy {
        PreparationPolicy::Ordinary => return Ok(false),
        PreparationPolicy::FreshOrCurrent => {
            let prefix = verified_prefix(&mut *connection, migrations).await?;
            if prefix >= 157 || empty_initialization(&mut *connection).await? {
                return Ok(false);
            }
            return Err(MigrateError::Source(
                std::io::Error::other(
                    "Populated tenant initialization requires current preparation",
                )
                .into(),
            ));
        }
        PreparationPolicy::Operational(expected) => expected,
    };
    DbSession::new(&mut *connection).query("migrations.preparation.install","CREATE TEMP TABLE academic_current_preparation_expected(singleton boolean PRIMARY KEY CHECK(singleton),required boolean NOT NULL CHECK(required),cutover_date date,selection_fingerprint bytea CHECK(selection_fingerprint IS NULL OR octet_length(selection_fingerprint)=32),CHECK((cutover_date IS NULL)=(selection_fingerprint IS NULL))) ON COMMIT PRESERVE ROWS").execute().await?;
    DbSession::new(connection).query("migrations.preparation.expectation","INSERT INTO pg_temp.academic_current_preparation_expected(singleton,required,cutover_date,selection_fingerprint) VALUES(true,true,$1,$2)")
        .bind(expected.map(|e|e.cutover_date)).bind(expected.map(|e|e.selection_fingerprint.as_slice())).execute().await?;
    Ok(true)
}

pub(super) async fn remove(connection: &mut PgConnection) -> Result<(), sqlx::Error> {
    DbSession::new(connection)
        .query(
            "migrations.preparation.remove",
            "DROP TABLE pg_temp.academic_current_preparation_expected",
        )
        .execute()
        .await?;
    Ok(())
}
