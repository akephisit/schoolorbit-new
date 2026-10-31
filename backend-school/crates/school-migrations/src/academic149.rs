//! Finite 147 -> 149 envelope for the protocol owned by migration 149.
use school_database::DbSession;
use sqlx::{migrate::MigrateError, migrate::Migrator, Connection, PgConnection};

const FINALIZE: &str = "-- ACADEMIC_CONTEXT_149_FINALIZE\n";

fn source_error(reason: &'static str) -> MigrateError {
    MigrateError::Source(std::io::Error::other(reason).into())
}

async fn verified_prefix(
    connection: &mut PgConnection,
    migrations: &Migrator,
) -> Result<i64, MigrateError> {
    let exists: bool = DbSession::new(connection)
        .scalar(
            "migrations.academic149.history_exists",
            "SELECT to_regclass(format('%I._sqlx_migrations',current_schema())) IS NOT NULL",
        )
        .fetch_one()
        .await?;
    if !exists {
        return Ok(0);
    }
    let applied: Vec<(i64, bool, Vec<u8>)> = DbSession::new(connection)
        .query_as(
            "migrations.academic149.history",
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
            .find(|migration| migration.version == *version)
            .ok_or(MigrateError::VersionMissing(*version))?;
        if checksum.as_slice() != expected.checksum.as_ref() {
            return Err(MigrateError::VersionMismatch(*version));
        }
        if *version != index as i64 + 1 {
            return Err(source_error(
                "Canonical migration history is not a contiguous prefix",
            ));
        }
    }
    Ok(applied.last().map_or(0, |row| row.0))
}

async fn refuse_staging(connection: &mut PgConnection) -> Result<(), MigrateError> {
    let staging:bool=DbSession::new(connection).scalar(
        "migrations.academic149.staging_absent",
        "SELECT EXISTS(SELECT 1 FROM pg_attribute WHERE attrelid='learning_delivery_apply_runs'::regclass AND NOT attisdropped AND starts_with(attname,'academic149_')) OR EXISTS(SELECT 1 FROM pg_class relation WHERE relation.relnamespace IN(current_schema()::regnamespace,pg_my_temp_schema()) AND starts_with(relation.relname,'academic149_')) OR EXISTS(SELECT 1 FROM pg_proc function_record WHERE function_record.pronamespace IN(current_schema()::regnamespace,pg_my_temp_schema()) AND starts_with(function_record.proname,'academic149_')) OR EXISTS(SELECT 1 FROM pg_type type_record WHERE type_record.typnamespace IN(current_schema()::regnamespace,pg_my_temp_schema()) AND starts_with(type_record.typname,'academic149_'))",
    ).fetch_one().await?;
    if staging {
        Err(source_error(
            "Academic receipt transition staging already exists",
        ))
    } else {
        Ok(())
    }
}

/// Private canonical runner core; callers supply only their compiled migrator.
/// Native SQLx migrations nest savepoints inside this real outer transaction.
pub(super) async fn run_transition(
    connection: &mut PgConnection,
    migrations: &Migrator,
) -> Result<(), MigrateError> {
    let initial_version = verified_prefix(connection, migrations).await?;
    if initial_version >= 149 {
        refuse_staging(connection).await?;
        // The finite exception ends at 149; later/no_tx migrations stay native.
        return migrations.run_direct(None, connection, false).await;
    }
    // Native SQLx validates the timeline and brings fresh/older prefixes to 147.
    // A gap must already have refused above, rather than replaying an old script.
    // SQLx exposes this native entry to avoid the generic Acquire HRTB/Send issue.
    migrations
        .run_direct(Some(147), &mut *connection, false)
        .await?;
    let mut transaction = connection.begin().await?;
    let result=async {
        // Revalidate only after owning the history and complete receipt/output cohort.
        sqlx::raw_sql("LOCK TABLE _sqlx_migrations,learning_delivery_apply_runs,learning_offerings IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *transaction).await.map_err(|error|MigrateError::ExecuteMigration(error,149))?;
        let version=verified_prefix(&mut transaction,migrations).await?;
        if version<147 {
            return Err(source_error("Academic receipt transition requires canonical prefix 147"));
        }
        refuse_staging(&mut transaction).await?;
        if version>=149 {
            // Another canonical owner completed while this connection waited.
            return Ok(false);
        }
        if version<149 {
            let migration=migrations.iter().find(|migration|migration.version==149)
                .ok_or(MigrateError::VersionNotPresent(149))?;
            if migration.no_tx || migrations.iter().any(|item|item.version==148 && item.no_tx) {
                return Err(source_error("Academic receipt migration requires native transactions"));
            }
            let script=migration.sql.as_str();
            let (prepare,finalize)=script.split_once(FINALIZE)
                .ok_or_else(||source_error("Academic receipt migration has no fixed phase boundary"))?;
            if prepare.is_empty() || finalize.is_empty() || finalize.contains(FINALIZE) {
                return Err(source_error("Academic receipt migration has an invalid phase boundary"));
            }
            // This is the exact prefix of the compiled, checksummed 149 source.
            // No caller SQL, settings or externally supplied fragment is accepted.
            sqlx::raw_sql(sqlx::AssertSqlSafe(prepare))
                .execute(&mut *transaction).await
                .map_err(|error|MigrateError::ExecuteMigration(error,149))?;
        }
        migrations.run_direct(Some(149),&mut *transaction,false).await?;
        let final_version=verified_prefix(&mut transaction,migrations).await?;
        if final_version!=149 {
            return Err(source_error("Academic receipt transition did not reach its compiled version"));
        }
        refuse_staging(&mut transaction).await?;
        Ok(true)
    }.await;
    match result {
        Ok(true) => transaction
            .commit()
            .await
            .map_err(|error| MigrateError::ExecuteMigration(error, 149))?,
        Ok(false) => transaction.rollback().await?,
        Err(primary) => {
            // Secondary cleanup must never replace the native primary cause.
            let _ = transaction.rollback().await;
            return Err(primary);
        }
    };
    // A failed COMMIT can be ambiguous. No retry or rollback outcome is inferred;
    // the operator reconciles committed history/evidence on a fresh connection.
    migrations.run_direct(None, connection, false).await
}
