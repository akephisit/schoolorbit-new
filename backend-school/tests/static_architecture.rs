use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    manifest_dir()
        .parent()
        .expect("backend-school should live under the repository root")
        .to_path_buf()
}

fn collect_files(
    dir: &Path,
    predicate: &dyn Fn(&Path) -> bool,
    files: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_files(&path, predicate, files)?;
        } else if predicate(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn list_files(dir: impl AsRef<Path>, predicate: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(dir.as_ref(), &predicate, &mut files)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.as_ref().display()));
    files
}

fn relative(path: &Path) -> String {
    path.strip_prefix(manifest_dir())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn repo_relative(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn read_source(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path.as_ref())
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.as_ref().display()))
}

fn strip_comments(source: &str) -> String {
    let mut stripped = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(char) = chars.next() {
        if in_line_comment {
            if char == '\n' {
                in_line_comment = false;
                stripped.push('\n');
            }
            continue;
        }

        if in_block_comment {
            if char == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            }
            continue;
        }

        if char == '/' && chars.peek() == Some(&'/') {
            chars.next();
            in_line_comment = true;
            continue;
        }

        if char == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            continue;
        }

        stripped.push(char);
    }

    stripped
}

fn backend_rs_files() -> Vec<PathBuf> {
    list_files(manifest_dir().join("src"), |path| {
        path.extension().is_some_and(|ext| ext == "rs")
    })
}

fn module_rs_files() -> Vec<PathBuf> {
    list_files(manifest_dir().join("src/modules"), |path| {
        path.extension().is_some_and(|ext| ext == "rs")
    })
}

fn module_handler_files() -> Vec<PathBuf> {
    let modules_dir = manifest_dir().join("src/modules");
    list_files(&modules_dir, |path| {
        if path.extension().is_none_or(|ext| ext != "rs") {
            return false;
        }

        let Ok(relative_path) = path.strip_prefix(&modules_dir) else {
            return false;
        };
        let path_text = relative_path.to_string_lossy().replace('\\', "/");

        path_text.ends_with("/handlers.rs") || path_text.contains("/handlers/")
    })
}

fn module_service_files() -> Vec<PathBuf> {
    let modules_dir = manifest_dir().join("src/modules");
    list_files(&modules_dir, |path| {
        if path.extension().is_none_or(|ext| ext != "rs") {
            return false;
        }

        let Ok(relative_path) = path.strip_prefix(&modules_dir) else {
            return false;
        };
        let path_text = relative_path.to_string_lossy().replace('\\', "/");

        path_text.ends_with("/services.rs") || path_text.contains("/services/")
    })
}

fn is_reexport_only_service_file(source: &str) -> bool {
    strip_comments(source)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .all(|line| {
            line.starts_with("pub mod ")
                || line.starts_with("pub use ")
                || line.starts_with("#[")
                || line == ";"
        })
}

#[test]
fn rust_module_roots_use_rust_2018_style() {
    let legacy_module_roots = list_files(manifest_dir().join("src"), |path| {
        path.file_name().is_some_and(|name| name == "mod.rs")
    });

    assert!(
        legacy_module_roots.is_empty(),
        "module roots should use foo.rs + foo/ children instead of mod.rs: {:?}",
        legacy_module_roots
            .iter()
            .map(|path| relative(path))
            .collect::<Vec<_>>()
    );
}

#[test]
fn backend_runtime_uses_organization_units_not_department_tables() {
    let legacy_organization_runtime_patterns = Regex::new(
        r"\bdepartments\b|\bdepartment_members\b|\bdepartment_permissions\b|\bpermission_delegations\b|\bdepartment_id\b|\bparent_department_id\b|\bis_primary_department\b|/api/departments|/api/lookup/departments",
    )
    .expect("valid regex");
    let mut violations = Vec::new();

    for file in backend_rs_files() {
        let source = read_source(&file);
        if legacy_organization_runtime_patterns.is_match(&source) {
            violations.push(relative(&file));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn backend_permission_contracts_use_organization_units_not_department_names() {
    let legacy_permission_patterns = Regex::new(
        r#""[^"]*(?:dept_work|\.department)[^"]*"|\bDEPT_WORK_[A-Z0-9_]*\b|\bACADEMIC_CURRICULUM_MANAGE_DEPT\b"#,
    )
    .expect("valid regex");
    let mut violations = Vec::new();

    for file in backend_rs_files() {
        let source = read_source(&file);
        if legacy_permission_patterns.is_match(&source) {
            violations.push(relative(&file));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn organization_baseline_migration_defines_canonical_school_structure() {
    let migration_path = manifest_dir()
        .join("migrations")
        .join("123_reset_organization_unit_baseline.sql");
    let source = read_source(&migration_path);

    for required_fragment in [
        "ORG-BASELINE-V1",
        "DIR-01",
        "ACAD-01",
        "PER-01",
        "BUD-01",
        "GEN-01",
        "SUBJ-OC",
        "FROM subject_groups sg",
        "ou.code = 'SUBJ-' || sg.code",
        "DELETE FROM organization_units",
        "to_regclass('public.department_menu_access')",
        "tmp_org_baseline_menu_refs",
        "code = 'SUBJ-OT'",
        "Legacy organization unit code SUBJ-OT remains",
    ] {
        assert!(
            source.contains(required_fragment),
            "{} must contain `{required_fragment}`",
            repo_relative(&migration_path)
        );
    }
}

#[test]
fn organization_permission_grant_baseline_is_deterministic() {
    let migration_path = manifest_dir()
        .join("migrations")
        .join("124_normalize_organization_permission_grants.sql");
    let source = read_source(&migration_path);

    for required_fragment in [
        "ORG-GRANTS-BASELINE-V1",
        "DELETE FROM organization_permission_grants",
        "academic_curriculum.manage.organization_unit",
        "organization_work.approve.organization_unit",
        "staff_profile.read.organization_tree",
        "staff_profile.read.school",
        "staff_pii.read.school",
        "SUBJ-%",
        "SCHOOL",
        "director",
        "deputy_director",
        "deputy_head",
    ] {
        assert!(
            source.contains(required_fragment),
            "{} must contain `{required_fragment}`",
            repo_relative(&migration_path)
        );
    }
}

#[test]
fn effective_permissions_do_not_inherit_child_organization_grants() {
    let permission_middleware = read_source(manifest_dir().join("src/middleware/permission.rs"));

    assert!(
        !permission_middleware.contains("Parent-leader inheritance"),
        "effective permissions must come from explicit role, membership grant, or delegation only"
    );
    assert!(
        !permission_middleware.contains("JOIN organization_units child"),
        "parent organization leaders must not implicitly inherit child organization grants"
    );
    assert!(
        !permission_middleware.contains("child.parent_unit_id = om.organization_unit_id"),
        "use explicit organization_tree policies instead of hidden child-grant inheritance"
    );
}

#[test]
fn academic_curriculum_tree_scope_is_explicitly_registered() {
    let backend_registry = read_source(manifest_dir().join("src/permissions/registry.rs"));
    let frontend_registry = read_source(
        repo_root()
            .join("frontend-school")
            .join("src/lib/permissions/registry.ts"),
    );
    let migration_path = manifest_dir()
        .join("migrations")
        .join("125_curriculum_organization_tree_permissions.sql");
    let migration = read_source(&migration_path);

    for source in [&backend_registry, &frontend_registry, &migration] {
        assert!(
            source.contains("academic_curriculum.read.organization_tree"),
            "curriculum tree read permission must be registered across backend/frontend/migration"
        );
        assert!(
            source.contains("academic_curriculum.manage.organization_tree"),
            "curriculum tree manage permission must be registered across backend/frontend/migration"
        );
    }
}

#[test]
fn lookup_models_expose_reference_data_only() {
    let lookup_models = strip_comments(&read_source(
        manifest_dir().join("src/modules/lookup/models.rs"),
    ));
    let forbidden_lookup_fields =
        Regex::new(r"\b(?:username|national_id|phone|email|address|line_id)\s*:")
            .expect("valid regex");

    assert!(
        !forbidden_lookup_fields.is_match(&lookup_models),
        "lookup DTOs must stay minimal reference data; move sensitive or account fields behind workflow-specific endpoints"
    );
}

#[test]
fn staff_profile_handler_uses_scoped_access_policy_and_pii_flag() {
    let staff_handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/staff/handlers/staff.rs"),
    ));

    assert!(staff_handler.contains("staff_access_policy::can_read_staff_profile"));
    assert!(staff_handler.contains("staff_access_policy::can_read_staff_pii"));
    assert!(staff_handler.contains("get_staff_profile(&pool, staff_id, include_pii)"));
    assert!(!staff_handler.contains("actor.require_permission(codes::STAFF_READ_ALL)?;"));
}

#[test]
fn foundation_handlers_delegate_database_work_to_services() {
    let direct_database_patterns = [
        "sqlx::query",
        "sqlx::query_as",
        "sqlx::query_scalar",
        ".fetch_one(",
        ".fetch_all(",
        ".fetch_optional(",
        ".execute(",
        ".begin(",
    ];
    let mut violations = Vec::new();

    for file in module_handler_files() {
        let file_name = relative(&file);
        if file_name == "src/modules/system/handlers/migration.rs" {
            continue;
        }

        let source = fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", file.display()));
        let source = strip_comments(&source);

        if direct_database_patterns
            .iter()
            .any(|pattern| source.contains(pattern))
        {
            violations.push(format!(
                "{}: move database work into services",
                relative(&file)
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn foundation_handlers_do_not_own_database_row_or_pool_types() {
    let database_type_patterns = ["sqlx::FromRow", "use sqlx::PgPool", "&sqlx::PgPool"];
    let mut violations = Vec::new();

    for file in module_handler_files() {
        let file_name = relative(&file);
        if file_name == "src/modules/system/handlers/migration.rs" {
            continue;
        }

        let source = strip_comments(&read_source(&file));

        for pattern in database_type_patterns {
            if source.contains(pattern) {
                violations.push(format!(
                    "{}: move database row/pool types into models or services ({pattern})",
                    relative(&file)
                ));
            }
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn module_handlers_use_central_api_response_envelope() {
    let legacy_envelope_patterns = [
        "Json(json!({ \"success\"",
        "Json(serde_json::json!({ \"success\"",
        "JsonResponse(serde_json::json!({ \"success\"",
        "json!({ \"success\"",
        "serde_json::json!({ \"success\"",
        "ApiResponse::ok(serde_json::json!",
        "ApiResponse::ok(json!",
        "ApiResponse::with_message(serde_json::json!",
        "ApiResponse::with_message(json!",
        "struct ApiResponse",
        "ApiResponse::success(",
    ];
    let mut violations = Vec::new();

    for file in module_handler_files() {
        let source = strip_comments(&read_source(&file));

        for pattern in legacy_envelope_patterns {
            if source.contains(pattern) {
                violations.push(format!(
                    "{}: use crate::api_response::ApiResponse instead of local/ad-hoc envelopes ({pattern})",
                    relative(&file)
                ));
            }
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn module_json_handlers_do_not_return_no_content_for_empty_mutations() {
    let mut violations = Vec::new();

    for file in module_handler_files() {
        let source = strip_comments(&read_source(&file));
        if source.contains("StatusCode::NO_CONTENT") {
            violations.push(format!(
                "{}: JSON mutations should return ApiResponse::empty() instead of 204 No Content",
                relative(&file)
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn module_service_logic_has_focused_unit_tests() {
    let mut violations = Vec::new();

    for file in module_service_files() {
        let source = read_source(&file);
        if is_reexport_only_service_file(&source) {
            continue;
        }

        if !source.contains("#[cfg(test)]") {
            violations.push(format!(
                "{}: service logic files must include focused #[cfg(test)] coverage",
                relative(&file)
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn module_services_do_not_return_raw_json_values_for_api_contracts() {
    let raw_json_result_patterns = [
        Regex::new(r"Result\s*<\s*serde_json::Value\s*,\s*AppError\s*>").expect("valid regex"),
        Regex::new(r"Result\s*<\s*Vec\s*<\s*serde_json::Value\s*>\s*,\s*AppError\s*>")
            .expect("valid regex"),
    ];
    let mut violations = Vec::new();

    for file in module_service_files() {
        let source = strip_comments(&read_source(&file));

        for pattern in &raw_json_result_patterns {
            if pattern.is_match(&source) {
                violations.push(format!(
                    "{}: return a typed DTO/outcome instead of raw serde_json::Value",
                    relative(&file)
                ));
            }
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn module_handlers_use_typed_api_dtos_instead_of_raw_json_values() {
    let raw_json_patterns = ["serde_json::Value", "use serde_json::Value"];
    let mut violations = Vec::new();

    for file in module_handler_files() {
        let source = strip_comments(&read_source(&file));

        for pattern in raw_json_patterns {
            if source.contains(pattern) {
                violations.push(format!(
                    "{}: use typed request/response DTOs in handlers instead of {pattern}",
                    relative(&file)
                ));
            }
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn known_shape_jsonb_api_arrays_use_typed_boundaries() {
    let forbidden_fields = [
        (
            "src/modules/academic/models/activity.rs",
            "allowed_grade_level_ids: Option<serde_json::Value",
        ),
        (
            "src/modules/academic/models/activity.rs",
            "allowed_classroom_ids: Option<serde_json::Value",
        ),
        (
            "src/modules/admission/models/rounds.rs",
            "scoring_subject_ids: serde_json::Value",
        ),
        (
            "src/modules/consent/models.rs",
            "data_categories: serde_json::Value",
        ),
    ];
    let legacy_value_helpers = Regex::new(
        r"fn\s+\w*(?:uuid|ids|categories)\w*_json\s*\([^)]*\)\s*->\s*(?:Option\s*<\s*)?serde_json::Value",
    )
    .expect("valid regex");
    let mut violations = Vec::new();

    for (relative_path, pattern) in forbidden_fields {
        let source = strip_comments(&read_source(manifest_dir().join(relative_path)));
        if source.contains(pattern) {
            violations.push(format!(
                "{relative_path}: known-shape JSONB arrays should expose Vec<T> at the API boundary"
            ));
        }
    }

    for file in module_service_files() {
        let source = strip_comments(&read_source(&file));
        if legacy_value_helpers.is_match(&source) {
            violations.push(format!(
                "{}: known-shape JSONB helper should return sqlx::types::Json<Vec<T>>, not serde_json::Value",
                relative(&file)
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn migrated_utility_handlers_use_shared_request_context() {
    let direct_context_patterns = [
        "resolve_tenant_pool",
        "resolve_tenant_context",
        "resolve_tenant_context_by_subdomain",
        "load_actor_context",
        "load_actor_context_or_error",
        "extract_user_id",
        "Uuid::parse_str(&claims.sub",
    ];
    let local_helper_pattern =
        Regex::new(r"\b(?:get_pool|get_db_pool|tenant_pool_by_subdomain|user_id_from_claims)\s*\(")
            .expect("valid regex");
    let mut violations = Vec::new();

    for file in module_handler_files() {
        let file_name = relative(&file);
        if file_name == "src/modules/system/handlers/migration.rs" {
            continue;
        }

        let source = strip_comments(&read_source(&file));

        for pattern in direct_context_patterns {
            if source.contains(pattern) {
                violations.push(format!(
                    "{}: use utils::request_context instead of {pattern}",
                    relative(&file)
                ));
            }
        }

        if local_helper_pattern.is_match(&source) {
            violations.push(format!(
                "{}: use shared request context helpers instead of local pool/user helpers",
                relative(&file)
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn permission_checks_use_registry_constants() {
    let call_with_permission_literal = Regex::new(
        r#"(?s)\b(?:has_permission|has_any_permission|has_all_permissions|require_permission|require_any_permission|require_all_permissions)\s*\([^;]*?"[a-z_]+(?:\.[a-z_]+){0,2}""#,
    )
    .expect("valid regex");
    let mut violations = Vec::new();

    for file in backend_rs_files() {
        let source = strip_comments(&read_source(&file));
        for matched in call_with_permission_literal.find_iter(&source) {
            let call = matched.as_str();
            if call.contains("codes::") {
                continue;
            }
            violations.push(format!(
                "{}: {}",
                relative(&file),
                call.split_whitespace().collect::<Vec<_>>().join(" ")
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn permission_handlers_use_actor_context_loader_apis_only() {
    let legacy_permission_helpers = Regex::new(
        r"\b(?:check_permission|check_any_permission|check_all_permissions|check_user_permission|get_actor_context|get_actor_context_or_error)\b",
    )
    .expect("valid regex");
    let mut violations = Vec::new();

    for file in backend_rs_files() {
        let source = strip_comments(&read_source(&file));
        if legacy_permission_helpers.is_match(&source) {
            violations.push(format!(
                "{}: use load_actor_context/load_actor_context_or_error and actor.require_* helpers",
                relative(&file)
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn permissions_do_not_use_legacy_user_permissions_resolver() {
    let mut violations = Vec::new();

    for file in backend_rs_files() {
        let source = read_source(&file);
        if source.contains("UserPermissions") || source.contains("get_user_with_permissions") {
            violations.push(relative(&file));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn module_handlers_use_actor_context_instead_of_raw_permission_lists() {
    let raw_permission_lookup =
        Regex::new(r"\bget_cached_user_permissions\b|\bpermission_matches\s*\(")
            .expect("valid regex");
    let mut violations = Vec::new();

    for file in module_rs_files() {
        if relative(&file) == "src/modules/auth/handlers.rs" {
            continue;
        }

        let source = read_source(&file);
        if raw_permission_lookup.is_match(&source) {
            violations.push(relative(&file));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn auth_responses_use_shared_effective_permission_resolver() {
    let auth_handler = read_source(manifest_dir().join("src/modules/auth/handlers.rs"));

    assert!(auth_handler.contains("get_cached_user_permissions"));
    assert!(!auth_handler.contains("permission_delegations"));
    assert!(!auth_handler.contains("department_permissions dp"));
    assert!(!auth_handler.contains("JOIN role_permissions"));
}

#[test]
fn menu_and_feature_handlers_do_not_parse_auth_or_query_permissions_directly() {
    let checked_files = [
        "src/modules/menu/handlers/admin.rs",
        "src/modules/menu/services/menu_service.rs",
        "src/modules/system/handlers/feature_toggles.rs",
    ];
    let forbidden_patterns = Regex::new(
        r"\bJwtService\b|\bfield_encryption\b|JOIN role_permissions|permission_delegations",
    )
    .expect("valid regex");
    let mut violations = Vec::new();

    for relative_path in checked_files {
        let source = read_source(manifest_dir().join(relative_path));
        if forbidden_patterns.is_match(&source) {
            violations.push(relative_path.to_string());
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn permission_cache_invalidations_notify_active_clients() {
    let mut violations = Vec::new();

    for file in backend_rs_files() {
        let source = strip_comments(&read_source(&file));
        let lines = source.lines().collect::<Vec<_>>();

        for (index, line) in lines.iter().enumerate() {
            let next_lines = lines
                .iter()
                .skip(index + 1)
                .take(3)
                .copied()
                .collect::<Vec<_>>()
                .join("\n");

            if line.contains("permission_cache.clear_all()")
                && !next_lines.contains("notify_all_permissions_changed()")
            {
                violations.push(format!(
                    "{}:{}: clear_all must emit permission_changed",
                    relative(&file),
                    index + 1
                ));
            }

            if line.contains("permission_cache.invalidate(")
                && !next_lines.contains("notify_permission_changed(")
            {
                violations.push(format!(
                    "{}:{}: invalidate must emit permission_changed",
                    relative(&file),
                    index + 1
                ));
            }
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn permission_change_sse_supports_user_targeted_and_broadcast_invalidation() {
    let event_source = read_source(manifest_dir().join("src/modules/notification/events.rs"));
    let notification_handler =
        read_source(manifest_dir().join("src/modules/notification/handlers.rs"));
    let app_state = read_source(manifest_dir().join("src/main.rs"));

    for expected in [
        "pub fn for_user(user_id: Uuid)",
        "pub fn for_all_users()",
        "pub fn applies_to(&self, user_id: Uuid) -> bool",
    ] {
        assert!(event_source.contains(expected), "missing {expected}");
    }

    for expected in [
        "permission_event_channel.subscribe()",
        "event.applies_to(user_id)",
        ".event(\"permission_changed\")",
    ] {
        assert!(
            notification_handler.contains(expected),
            "missing {expected}"
        );
    }

    assert!(app_state.contains("notify_permission_changed(&self, target_user_id: Uuid)"));
    assert!(app_state.contains("notify_all_permissions_changed(&self)"));
}

#[test]
fn internal_api_secrets_use_constant_time_comparison_and_caller_headers() {
    let checked_files = [
        repo_root().join("backend-school/src/middleware/internal_auth.rs"),
        repo_root().join("backend-admin/src/handlers/internal.rs"),
    ];

    for file in checked_files {
        let source = read_source(&file);
        assert!(
            source.contains("ConstantTimeEq"),
            "{} must use ConstantTimeEq",
            repo_relative(&file)
        );
        assert!(
            source.contains("X-Internal-Caller"),
            "{} must use X-Internal-Caller",
            repo_relative(&file)
        );
        assert!(
            source.contains("INTERNAL_API_SECRET_"),
            "{} must support caller-specific secrets",
            repo_relative(&file)
        );
        assert!(
            !source.contains("!= internal_secret"),
            "{} must not use naive secret comparison",
            repo_relative(&file)
        );
        assert!(
            !source.contains("== internal_secret"),
            "{} must not use naive secret comparison",
            repo_relative(&file)
        );
    }

    let backend_school_client =
        read_source(repo_root().join("backend-school/src/db/admin_client.rs"));
    let backend_admin_client =
        read_source(repo_root().join("backend-admin/src/clients/backend_school_client.rs"));

    assert!(backend_school_client.contains("X-Internal-Caller"));
    assert!(backend_school_client.contains("backend-school"));
    assert!(backend_admin_client.contains("X-Internal-Caller"));
    assert!(backend_admin_client.contains("backend-admin"));
}

#[test]
fn module_handlers_resolve_tenant_pools_through_the_central_resolver() {
    let mut violations = Vec::new();
    let pool_manager_get_pool =
        Regex::new(r"\.pool_manager\s*\.get_pool\s*\(").expect("valid regex");

    for file in module_rs_files() {
        let source = read_source(&file);
        let file_name = relative(&file);

        if source.contains("get_school_database_url") {
            violations.push(format!(
                "{file_name}: use utils::tenant resolver instead of get_school_database_url"
            ));
        }

        if source.contains("PgPool::connect(") {
            violations.push(format!(
                "{file_name}: use AppState PoolManager via utils::tenant resolver"
            ));
        }

        if file_name != "src/modules/system/handlers/migration.rs"
            && pool_manager_get_pool.is_match(&source)
        {
            violations.push(format!(
                "{file_name}: use utils::tenant resolver instead of pool_manager.get_pool"
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}
