use crate::api_response::{ApiErrorResponse, ApiResponse, EmptyData, UuidIdData};
use crate::modules::auth::models::{
    ChangePasswordRequest, LoginData, LoginRequest, ProfileResponse, UpdateProfileRequest,
    UserResponse,
};
use crate::modules::staff::models::{
    AssignRoleRequest, CreateOrganizationUnitRequest, CreateRoleRequest,
    OrganizationPermissionGrantInput, OrganizationUnit, Permission, Role,
    UpdateOrganizationPermissionsRequest, UpdateOrganizationUnitRequest, UpdateRoleRequest,
    UserRoleAssignmentResponse,
};
use crate::modules::staff::services::organization_permission_service::OrganizationPermissionGrant;
use serde_json::Value;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::modules::auth::handlers::login,
        crate::modules::auth::handlers::logout,
        crate::modules::auth::handlers::me,
        crate::modules::auth::handlers::get_profile,
        crate::modules::auth::handlers::update_profile,
        crate::modules::auth::handlers::change_password,
        crate::modules::staff::handlers::roles::list_roles,
        crate::modules::staff::handlers::roles::get_role,
        crate::modules::staff::handlers::roles::create_role,
        crate::modules::staff::handlers::roles::update_role,
        crate::modules::staff::handlers::permissions::list_permissions,
        crate::modules::staff::handlers::permissions::list_permissions_by_module,
        crate::modules::staff::handlers::user_roles::get_user_roles,
        crate::modules::staff::handlers::user_roles::assign_user_role,
        crate::modules::staff::handlers::user_roles::remove_user_role,
        crate::modules::staff::handlers::user_roles::get_user_permissions,
        crate::modules::staff::handlers::roles::list_organization_units,
        crate::modules::staff::handlers::roles::get_organization_unit,
        crate::modules::staff::handlers::roles::create_organization_unit,
        crate::modules::staff::handlers::roles::update_organization_unit,
        crate::modules::staff::handlers::organization_permissions::get_organization_permissions,
        crate::modules::staff::handlers::organization_permissions::update_organization_permissions
    ),
    components(schemas(
        UserResponse,
        LoginRequest,
        LoginData,
        ProfileResponse,
        UpdateProfileRequest,
        ChangePasswordRequest,
        ApiResponse<LoginData>,
        ApiResponse<ProfileResponse>,
        ApiResponse<UserResponse>,
        EmptyData,
        ApiResponse<EmptyData>,
        UuidIdData,
        ApiResponse<UuidIdData>,
        Role,
        CreateRoleRequest,
        UpdateRoleRequest,
        Permission,
        AssignRoleRequest,
        UserRoleAssignmentResponse,
        ApiResponse<Vec<Role>>,
        ApiResponse<Role>,
        ApiResponse<Vec<Permission>>,
        ApiResponse<std::collections::HashMap<String, Vec<Permission>>>,
        ApiResponse<Vec<UserRoleAssignmentResponse>>,
        ApiResponse<Vec<String>>,
        OrganizationUnit,
        CreateOrganizationUnitRequest,
        UpdateOrganizationUnitRequest,
        OrganizationPermissionGrantInput,
        UpdateOrganizationPermissionsRequest,
        OrganizationPermissionGrant,
        ApiResponse<Vec<OrganizationUnit>>,
        ApiResponse<OrganizationUnit>,
        ApiResponse<Vec<OrganizationPermissionGrant>>,
        ApiErrorResponse
    )),
    tags(
        (name = "auth", description = "Authentication and current-user operations"),
        (name = "roles", description = "Role assignment and role administration"),
        (name = "permissions", description = "Permission discovery and effective permissions"),
        (name = "organization", description = "Organization units and scoped access")
    )
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

    fn assert_operations(document: &Value, expected: &[(&str, &str, &str)]) {
        for (path, method, operation_id) in expected {
            assert_eq!(
                document["paths"][path][method]["operationId"], *operation_id,
                "missing or incorrect {method} {path}"
            );
        }
    }

    #[test]
    fn documents_current_user_operation_and_envelopes() {
        let document = school_api_value().expect("document should serialize");
        let operation = &document["paths"]["/api/auth/me"]["get"];
        let success_response =
            &operation["responses"]["200"]["content"]["application/json"]["schema"];
        let error_response =
            &operation["responses"]["401"]["content"]["application/json"]["schema"];

        assert_eq!(operation["operationId"], "getCurrentUser");
        assert_eq!(
            success_response["$ref"],
            "#/components/schemas/ApiResponse_UserResponse"
        );
        assert_eq!(
            error_response["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );

        let success_schema = &document["components"]["schemas"]["ApiResponse_UserResponse"];
        assert_eq!(required(success_schema), vec!["data", "success"]);
        assert_eq!(success_schema["properties"]["success"]["type"], "boolean");
        assert_eq!(
            success_schema["properties"]["data"],
            document["components"]["schemas"]["UserResponse"]
        );

        let error_schema = &document["components"]["schemas"]["ApiErrorResponse"];
        assert_eq!(required(error_schema), vec!["error", "success"]);
        assert_eq!(error_schema["properties"]["success"]["type"], "boolean");
        assert_eq!(error_schema["properties"]["error"]["type"], "string");
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

    #[test]
    fn documents_shared_empty_and_uuid_identifier_envelopes() {
        let document = school_api_value().expect("document should serialize");
        let schemas = &document["components"]["schemas"];

        let empty_envelope = &schemas["ApiResponse_EmptyData"];
        assert_eq!(required(empty_envelope), vec!["data", "success"]);
        assert_eq!(empty_envelope["properties"]["data"], schemas["EmptyData"]);
        assert_eq!(
            schemas["EmptyData"]["type"], "object",
            "empty responses must generate an object DTO"
        );

        let id_envelope = &schemas["ApiResponse_UuidIdData"];
        assert_eq!(required(id_envelope), vec!["data", "success"]);
        assert_eq!(required(&schemas["UuidIdData"]), vec!["id"]);
        assert_eq!(schemas["UuidIdData"]["properties"]["id"]["format"], "uuid");
    }

    #[test]
    fn documents_auth_operations_and_transport_shapes() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/auth/login", "post", "login"),
                ("/api/auth/logout", "post", "logout"),
                ("/api/auth/me", "get", "getCurrentUser"),
                ("/api/auth/me/profile", "get", "getCurrentUserProfile"),
                ("/api/auth/me/profile", "put", "updateCurrentUserProfile"),
                (
                    "/api/auth/me/change-password",
                    "post",
                    "changeCurrentUserPassword",
                ),
            ],
        );

        let schemas = &document["components"]["schemas"];
        let login = &schemas["LoginRequest"];
        assert_eq!(required(login), vec!["password", "username"]);
        assert!(login["properties"].get("rememberMe").is_some());
        assert!(login["properties"].get("remember_me").is_none());
        assert_eq!(
            document["paths"]["/api/auth/login"]["post"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_LoginData"
        );

        let profile = &schemas["ProfileResponse"];
        for field in [
            "nationalId",
            "title",
            "nickname",
            "email",
            "phone",
            "emergencyContact",
            "lineId",
            "dateOfBirth",
            "gender",
            "address",
            "profileImageUrl",
            "hiredDate",
        ] {
            assert!(
                required(profile).contains(&field),
                "{field} must be required"
            );
            assert!(
                contains_null(&profile["properties"][field]),
                "{field} must accept null"
            );
        }
        assert!(!required(profile).contains(&"primaryRoleName"));
        assert!(!contains_null(&profile["properties"]["primaryRoleName"]));

        let update = &schemas["UpdateProfileRequest"]["properties"];
        assert!(update.get("emergencyContact").is_some());
        assert!(update.get("dateOfBirth").is_some());
        assert!(update.get("profileImageUrl").is_some());
        let change = &schemas["ChangePasswordRequest"];
        assert_eq!(required(change), vec!["currentPassword", "newPassword"]);
    }

    #[test]
    fn documents_role_permission_and_user_role_operations() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/roles", "get", "listRoles"),
                ("/api/roles/{id}", "get", "getRole"),
                ("/api/roles", "post", "createRole"),
                ("/api/roles/{id}", "put", "updateRole"),
                ("/api/permissions", "get", "listPermissions"),
                ("/api/permissions/modules", "get", "listPermissionsByModule"),
                ("/api/users/{id}/roles", "get", "getUserRoles"),
                ("/api/users/{id}/roles", "post", "assignUserRole"),
                (
                    "/api/users/{id}/roles/{role_id}",
                    "delete",
                    "removeUserRole",
                ),
                (
                    "/api/users/{id}/permissions",
                    "get",
                    "listUserEffectivePermissions",
                ),
            ],
        );

        assert!(document["paths"]["/api/roles/{id}"]["delete"].is_null());
        assert_eq!(
            document["paths"]["/api/roles"]["post"]["responses"]["201"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_UuidIdData"
        );
        assert_eq!(
            document["paths"]["/api/roles/{id}"]["put"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_EmptyData"
        );

        let schemas = &document["components"]["schemas"];
        let role = &schemas["Role"];
        for field in ["name_en", "description"] {
            assert!(required(role).contains(&field));
            assert!(contains_null(&role["properties"][field]));
        }
        assert_eq!(schemas["Permission"]["properties"]["id"]["format"], "uuid");

        let assignment = &schemas["UserRoleAssignmentResponse"];
        for field in ["organization_unit_id", "ended_at", "notes"] {
            assert!(required(assignment).contains(&field));
            assert!(contains_null(&assignment["properties"][field]));
        }
        assert!(
            document["paths"]["/api/permissions/modules"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]
                .is_object()
        );
    }

    #[test]
    fn documents_organization_unit_and_permission_grant_operations() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/organization/units", "get", "listOrganizationUnits"),
                ("/api/organization/units/{id}", "get", "getOrganizationUnit"),
                ("/api/organization/units", "post", "createOrganizationUnit"),
                (
                    "/api/organization/units/{id}",
                    "put",
                    "updateOrganizationUnit",
                ),
                (
                    "/api/organization/units/{id}/permissions",
                    "get",
                    "getOrganizationPermissions",
                ),
                (
                    "/api/organization/units/{id}/permissions",
                    "put",
                    "updateOrganizationPermissions",
                ),
            ],
        );

        assert!(document["paths"]["/api/organization/units/{id}"]["delete"].is_null());
        assert_eq!(
            document["paths"]["/api/organization/units"]["post"]["responses"]["201"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_UuidIdData"
        );

        let schemas = &document["components"]["schemas"];
        let unit = &schemas["OrganizationUnit"];
        for field in [
            "name_en",
            "description",
            "parent_unit_id",
            "phone",
            "email",
            "location",
            "subject_group_id",
        ] {
            assert!(required(unit).contains(&field));
            assert!(contains_null(&unit["properties"][field]));
        }

        let grant = &schemas["OrganizationPermissionGrant"];
        assert!(required(grant).contains(&"position_code"));
        assert!(contains_null(&grant["properties"]["position_code"]));
    }
}
