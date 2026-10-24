use crate::settings::models::{
    SchoolAppearance, SchoolAppearanceResponse, UpdateSchoolAppearanceRequest,
};
use school_errors::AppError;
use sqlx::{types::Json, FromRow, PgPool};

#[derive(FromRow)]
struct AppearanceRow {
    appearance: Json<SchoolAppearance>,
    appearance_revision: i64,
}

impl From<AppearanceRow> for SchoolAppearanceResponse {
    fn from(row: AppearanceRow) -> Self {
        Self {
            appearance: row.appearance.0,
            revision: row.appearance_revision,
        }
    }
}

impl UpdateSchoolAppearanceRequest {
    pub fn validate_and_normalize(mut self) -> Result<Self, AppError> {
        if self.expected_revision < 0 {
            return Err(AppError::ValidationError("revision ต้องไม่ติดลบ".into()));
        }
        if let Some(primary) = &mut self.appearance.primary {
            if primary.len() != 7
                || !primary.starts_with('#')
                || !primary.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
            {
                return Err(AppError::ValidationError("สีหลักต้องอยู่ในรูปแบบ #RRGGBB".into()));
            }
            primary.make_ascii_lowercase();
        }
        Ok(self)
    }
}

/// One bounded projection, shared by authenticated settings and public first paint.
pub async fn get(pool: &PgPool) -> Result<SchoolAppearanceResponse, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "administration.get").await?;
    get_on_connection(&mut connection).await
}

pub async fn get_on_connection(
    connection: &mut sqlx::PgConnection,
) -> Result<SchoolAppearanceResponse, AppError> {
    school_database::DbSession::new(&mut *connection)
        .query_as::<AppearanceRow>(
            "administration.services.appearance.get_on_connection",
            "SELECT appearance, appearance_revision FROM school_settings LIMIT 1",
        )
        .fetch_optional()
        .await
        .map_err(database_error)?
        .map(Into::into)
        .ok_or_else(missing_settings)
}

/// Compare-and-swap changes only appearance fields; a concurrent logo save is independent.
pub async fn update(
    pool: &PgPool,
    request: UpdateSchoolAppearanceRequest,
) -> Result<SchoolAppearanceResponse, AppError> {
    let request = request.validate_and_normalize()?;
    let mut transaction =
        school_tenancy::TenantPool::begin_observed(pool, "administration.appearance.transaction")
            .await
            .map_err(database_error)?;
    let updated = school_database::DbSession::new(&mut *transaction).query_as::<AppearanceRow>("administration.services.appearance.update",
        "UPDATE school_settings SET appearance = $1, appearance_revision = appearance_revision + 1 \
         WHERE id = (SELECT id FROM school_settings LIMIT 1) AND appearance_revision = $2 \
         RETURNING appearance, appearance_revision",
    )
    .bind(Json(request.appearance))
    .bind(request.expected_revision)
    .fetch_optional()
    .await
    .map_err(database_error)?;
    let Some(updated) = updated else {
        let exists: bool = school_database::DbSession::new(&mut *transaction)
            .scalar(
                "administration.services.appearance.update",
                "SELECT EXISTS(SELECT 1 FROM school_settings)",
            )
            .fetch_one()
            .await
            .map_err(database_error)?;
        return Err(if exists {
            AppError::Conflict("รูปแบบโรงเรียนถูกแก้ไขแล้ว กรุณาโหลดข้อมูลล่าสุด".into())
        } else {
            missing_settings()
        });
    };
    transaction.commit().await.map_err(database_error)?;
    Ok(updated.into())
}

fn missing_settings() -> AppError {
    AppError::NotFound("ไม่พบการตั้งค่าโรงเรียน".into())
}

fn database_error(error: sqlx::Error) -> AppError {
    let database_code = error.as_database_error().and_then(|error| error.code());
    tracing::error!(database_code = ?database_code, "School appearance persistence failed");
    AppError::InternalServerError("บันทึกหรืออ่านรูปแบบโรงเรียนไม่สำเร็จ".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(primary: Option<&str>, revision: i64) -> UpdateSchoolAppearanceRequest {
        serde_json::from_value(json!({"appearance":{"palette":"teal","font":"kanit","density":"comfortable","primary":primary},"expectedRevision":revision})).unwrap()
    }

    #[test]
    fn validates_color_and_normalizes_without_accepting_css() {
        assert_eq!(
            request(Some("#A0b1C2"), 0)
                .validate_and_normalize()
                .unwrap()
                .appearance
                .primary
                .as_deref(),
            Some("#a0b1c2")
        );
        assert!(request(None, 0).validate_and_normalize().is_ok());
        for color in [
            "red",
            "#fff",
            "#12345678",
            "#12zz00",
            "#123456;",
            "#１２３４５６",
            " #123456",
        ] {
            assert!(
                request(Some(color), 0).validate_and_normalize().is_err(),
                "{color}"
            );
        }
        assert!(request(None, -1).validate_and_normalize().is_err());
    }

    #[test]
    fn rejects_unknown_fields_and_non_enum_values() {
        let valid = serde_json::to_value(SchoolAppearance::default()).unwrap();
        let mut missing_primary = valid.clone();
        missing_primary.as_object_mut().unwrap().remove("primary");
        assert!(serde_json::from_value::<SchoolAppearance>(missing_primary).is_err());
        for field in ["mode", "device", "version", "css"] {
            let mut value = valid.clone();
            value[field] = json!("dark");
            assert!(serde_json::from_value::<SchoolAppearance>(value).is_err());
            let mut value = json!({"appearance":valid,"expectedRevision":0});
            value[field] = json!("dark");
            assert!(serde_json::from_value::<UpdateSchoolAppearanceRequest>(value).is_err());
        }
        for field in ["palette", "font", "density"] {
            for invalid in [json!("unknown"), json!(null), json!(1), json!(true)] {
                let mut value = valid.clone();
                value[field] = invalid;
                assert!(serde_json::from_value::<SchoolAppearance>(value).is_err());
            }
        }
    }
}
