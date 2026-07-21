use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::modules::auth::models::UserResponse;
use serde_json::Value;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(crate::modules::auth::handlers::me),
    components(schemas(
        UserResponse,
        ApiResponse<UserResponse>,
        ApiErrorResponse
    )),
    tags((name = "auth", description = "Authentication and current-user operations"))
)]
struct SchoolApiDoc;

fn sort_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut entries = std::mem::take(map).into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (_, child) in &mut entries {
                sort_json(child);
            }
            map.extend(entries);
        }
        Value::Array(values) => values.iter_mut().for_each(sort_json),
        _ => {}
    }
}

pub fn school_api_value() -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(SchoolApiDoc::openapi())?;
    sort_json(&mut value);
    Ok(value)
}

pub fn render_school_api() -> Result<String, serde_json::Error> {
    let mut output = serde_json::to_string_pretty(&school_api_value()?)?;
    output.push('\n');
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{render_school_api, school_api_value};
    use serde_json::Value;

    fn required(schema: &Value) -> Vec<&str> {
        let mut fields = schema["required"]
            .as_array()
            .expect("required must be an array")
            .iter()
            .map(|value| value.as_str().expect("required entry must be a string"))
            .collect::<Vec<_>>();
        fields.sort_unstable();
        fields
    }

    fn contains_null(schema: &Value) -> bool {
        match schema {
            Value::String(value) => value == "null",
            Value::Array(values) => values.iter().any(contains_null),
            Value::Object(values) => values.values().any(contains_null),
            _ => false,
        }
    }

    #[test]
    fn documents_current_user_operation_and_envelopes() {
        let document = school_api_value().expect("document should serialize");
        let operation = &document["paths"]["/api/auth/me"]["get"];

        assert_eq!(operation["operationId"], "getCurrentUser");
        assert!(operation["responses"]["200"]["content"]["application/json"]["schema"].is_object());
        assert!(operation["responses"]["401"]["content"]["application/json"]["schema"].is_object());
    }

    #[test]
    fn current_user_schema_matches_serde() {
        let document = school_api_value().expect("document should serialize");
        let schema = &document["components"]["schemas"]["UserResponse"];

        assert_eq!(
            required(schema),
            vec![
                "createdAt",
                "email",
                "firstName",
                "id",
                "lastName",
                "nationalId",
                "phone",
                "profileImageUrl",
                "status",
                "userType",
                "username",
            ]
        );

        let properties = schema["properties"]
            .as_object()
            .expect("properties must exist");
        assert_eq!(properties["id"]["format"], "uuid");
        assert_eq!(properties["createdAt"]["format"], "date-time");

        for field in ["nationalId", "email", "phone", "profileImageUrl"] {
            assert!(
                contains_null(&properties[field]),
                "{field} must accept null"
            );
        }

        for field in ["primaryRoleName", "permissions"] {
            assert!(!required(schema).contains(&field));
            assert!(
                !contains_null(&properties[field]),
                "{field} is omitted, not null"
            );
        }
    }

    #[test]
    fn render_is_deterministic_and_newline_terminated() {
        let first = render_school_api().expect("first render");
        let second = render_school_api().expect("second render");

        assert_eq!(first, second);
        assert!(first.ends_with('\n'));
    }
}
