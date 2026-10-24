use school_database::DbSession;
use sqlx::{PgConnection, Postgres, QueryBuilder, Row};

#[derive(sqlx::FromRow)]
struct SelectedRow {
    value: i32,
}

async fn compiled_surface(
    connection: &mut PgConnection,
    borrowed: &str,
) -> Result<(), sqlx::Error> {
    let mut session = DbSession::new(connection);
    session
        .query("probe.execute", "SELECT $1::text")
        .bind(borrowed)
        .execute()
        .await?;
    let row = session
        .query("probe.row", "SELECT 1 AS value")
        .fetch_one()
        .await?;
    let _: i32 = row.try_get("value")?;
    let result = session
        .query_as::<SelectedRow>("probe.typed", "SELECT $1::int AS value")
        .bind(3_i32)
        .fetch_one()
        .await?;
    assert_eq!(result.value, 3);
    let _: Option<(String,)> = session
        .query_as("probe.optional", "SELECT $1::text WHERE false")
        .bind(borrowed)
        .fetch_optional()
        .await?;
    let _: Vec<SelectedRow> = session
        .query_as("probe.all", "SELECT 1 AS value")
        .fetch_all()
        .await?;
    let _: i32 = session
        .scalar("probe.scalar", "SELECT $1::int")
        .bind(1_i32)
        .fetch_one()
        .await?;
    let _: Option<i32> = session
        .scalar("probe.scalar_optional", "SELECT 1 WHERE false")
        .fetch_optional()
        .await?;
    let _: Vec<i32> = session
        .scalar("probe.scalar_all", "SELECT 1")
        .fetch_all()
        .await?;
    let sql = String::from("SELECT 1");
    let _ = session
        .query("probe.reviewed_dynamic", sqlx::AssertSqlSafe(sql))
        .fetch_optional()
        .await?;
    let mut builder = QueryBuilder::<Postgres>::new("SELECT ");
    builder.push_bind(7_i32);
    let _: i32 = session
        .prepared_scalar("probe.builder_scalar", builder.build_query_scalar())
        .fetch_one()
        .await?;
    builder.reset();
    builder.push("8 AS value");
    let _: SelectedRow = session
        .prepared_query_as("probe.builder_typed", builder.build_query_as())
        .fetch_one()
        .await?;
    builder.reset();
    builder.push("9");
    session
        .prepared_query("probe.builder_raw", builder.build())
        .fetch_all()
        .await?;
    Ok(())
}

#[test]
fn public_api_compiles_with_borrowed_binds_and_native_builders() {
    let _probe = compiled_surface;
}
