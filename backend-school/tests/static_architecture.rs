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

fn active_baseline_migration_path() -> PathBuf {
    manifest_dir().join("migrations").join("001_baseline.sql")
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
fn active_migrations_are_clean_sequential_timeline() {
    let migrations_dir = manifest_dir().join("migrations");
    let mut active_migrations = list_files(&migrations_dir, |path| {
        path.extension().and_then(|extension| extension.to_str()) == Some("sql")
    })
    .into_iter()
    .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
    .collect::<Vec<_>>();
    active_migrations.sort();

    assert_eq!(
        active_migrations.first().map(String::as_str),
        Some("001_baseline.sql")
    );

    let migration_name_pattern =
        Regex::new(r"^(\d{3})_[a-z0-9_]+\.sql$").expect("valid migration name regex");
    for (index, migration) in active_migrations.iter().enumerate() {
        let captures = migration_name_pattern
            .captures(migration)
            .unwrap_or_else(|| panic!("invalid active migration name: {migration}"));
        let version = captures[1]
            .parse::<usize>()
            .unwrap_or_else(|_| panic!("invalid migration version: {migration}"));
        assert_eq!(
            version,
            index + 1,
            "active migrations must stay sequential after the clean baseline"
        );
    }

    let legacy_dir = manifest_dir().join("migrations_legacy");
    assert!(
        legacy_dir.join("001_create_users.sql").exists()
            && legacy_dir
                .join("127_canonical_permission_code_contracts.sql")
                .exists(),
        "historical migrations should be archived under {} and must not be runtime migrations",
        repo_relative(&legacy_dir)
    );
}

#[test]
fn organization_baseline_migration_defines_canonical_school_structure() {
    let migration_path = active_baseline_migration_path();
    let source = read_source(&migration_path);

    for required_fragment in [
        "ORG-BASELINE-V1",
        "'SCHOOL'",
        "DIR-01",
        "ACAD-01",
        "STU-01",
        "PER-01",
        "BUD-01",
        "GEN-01",
        "GEN-DOC",
        "SUBJ-OC",
        "\"subject_group_id\"",
    ] {
        assert!(
            source.contains(required_fragment),
            "{} must contain `{required_fragment}`",
            repo_relative(&migration_path)
        );
    }

    assert!(
        !source.contains("SUBJ-OT") && !source.contains("department"),
        "{} must be a clean organization-unit baseline without legacy department aliases",
        repo_relative(&migration_path)
    );
}

#[test]
fn organization_permission_grant_baseline_is_deterministic() {
    let migration_path = active_baseline_migration_path();
    let source = read_source(&migration_path);

    for required_fragment in [
        "CREATE TABLE \"organization_permission_grants\"",
        "academic_curriculum.manage.organization_unit",
        "academic_curriculum.manage.organization_tree",
        "organization_work.approve.organization_unit",
        "staff_profile.read.organization_tree",
        "staff_profile.read.school",
        "staff_pii.read.school",
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
    let migration_path = active_baseline_migration_path();
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
fn operational_bins_use_central_tenant_migration_runner() {
    let bin_files = list_files(manifest_dir().join("src/bin"), |path| {
        path.extension().and_then(|extension| extension.to_str()) == Some("rs")
    });
    let direct_migrate_pattern =
        Regex::new(r#"sqlx::migrate!\s*\(\s*"\./migrations"\s*\)\s*\.run\s*\("#)
            .expect("valid regex");
    let mut violations = Vec::new();

    for file in bin_files {
        let source = strip_comments(&read_source(&file));
        if direct_migrate_pattern.is_match(&source) {
            violations.push(relative(&file));
        }
    }

    let seed_sandbox = read_source(manifest_dir().join("src/bin/seed_sandbox.rs"));
    assert_eq!(violations, Vec::<String>::new());
    assert!(
        seed_sandbox.contains("migration::run_tenant_migrations(&pool)"),
        "seed_sandbox must use the same migration runner as tenant runtime"
    );
}

#[test]
fn tenant_data_cutover_script_has_safety_guards() {
    let script = read_source(repo_root().join("scripts/cutover_tenant_data.sh"));

    for required_fragment in [
        "CUTOVER_SOURCE_DATABASE_URL",
        "CUTOVER_TARGET_DATABASE_URL",
        "CUTOVER_ALLOW_NON_TEST_TARGET",
        "CUTOVER_CONFIRM_TARGET_TRUNCATE",
        "CUTOVER_KEEP_SCHEMA",
        "migrate_tenant_schema",
        "--exclude-table=public._sqlx_migrations",
        "TRUNCATE TABLE",
        "RESTART IDENTITY CASCADE",
        "DEFERRABLE INITIALLY IMMEDIATE",
        "DISABLE TRIGGER USER",
        "ENABLE TRIGGER USER",
        "SET LOCAL search_path",
        "SET CONSTRAINTS ALL DEFERRED",
        "NOT DEFERRABLE",
        "set_config",
        "query_to_xml",
        "diff -u",
    ] {
        assert!(
            script.contains(required_fragment),
            "tenant data cutover script must contain safety/validation fragment `{required_fragment}`"
        );
    }
}

#[test]
fn clean_tenant_prepare_script_has_safety_guards() {
    let script = read_source(repo_root().join("scripts/prepare_clean_tenant_db.sh"));
    let migration_bin = read_source(manifest_dir().join("src/bin/migrate_tenant_schema.rs"));

    for required_fragment in [
        "PREPARE_CLEAN_TENANT_DATABASE_URL",
        "PREPARE_CLEAN_TENANT_SCHEMA",
        "PREPARE_CLEAN_TENANT_CONFIRM",
        "PREPARE_CLEAN_TENANT_ALLOW_NON_TEST",
        "PREPARE_CLEAN_TENANT_RESET_SCHEMA",
        "PREPARE_CLEAN_TENANT_DROP_SCHEMA_ON_EXIT",
        "MIGRATION_SCHEMA_ALLOW_PUBLIC",
        "migrate_tenant_schema",
        "_sqlx_migrations",
        "migration_max_version",
        "application_table_count",
        "permissions",
        "organization_units",
        "users",
    ] {
        assert!(
            script.contains(required_fragment),
            "clean tenant prepare script must contain safety/validation fragment `{required_fragment}`"
        );
    }

    assert!(
        migration_bin.contains("MIGRATION_SCHEMA_ALLOW_PUBLIC"),
        "migrate_tenant_schema must allow public schema only through an explicit env guard"
    );
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
fn staff_list_uses_resource_aware_access_scope() {
    let staff_handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/staff/handlers/staff.rs"),
    ));
    let staff_service = strip_comments(&read_source(
        manifest_dir().join("src/modules/staff/services/staff_service.rs"),
    ));

    assert!(staff_handler.contains("staff_access_policy::resolve_staff_profile_list_access"));
    assert!(staff_handler.contains("staff_service::list_staff(&pool, filter, access)"));
    assert!(!staff_handler.contains("actor.require_any_permission(&["));
    assert!(staff_service.contains("UserResourceListAccess"));
    assert!(staff_service.contains("push_staff_list_access_filter"));
}

#[test]
fn academic_curriculum_access_uses_resource_policy_tree_resolution() {
    let curriculum_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/curriculum_access_policy.rs"),
    ));

    assert!(curriculum_policy.contains("resource_access_policy::accessible_organization_unit_ids"));
    assert!(curriculum_policy.contains("resource_access_policy::resolve_user_resource_list_access"));
    assert!(!curriculum_policy.contains("WITH RECURSIVE"));
    assert!(!curriculum_policy.contains("JOIN organization_tree parent_tree"));
}

#[test]
fn academic_curriculum_permission_decisions_live_in_policy_layer() {
    let policies_root = read_source(manifest_dir().join("src/policies.rs"));
    let curriculum_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/curriculum_access_policy.rs"),
    ));
    let subject_handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/handlers/subjects.rs"),
    ));
    let subject_service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/subject_service.rs"),
    ));

    assert!(policies_root.contains("pub mod curriculum_access_policy;"));
    assert!(curriculum_policy.contains("resource_access_policy::accessible_organization_unit_ids"));
    assert!(curriculum_policy.contains("resource_access_policy::resolve_user_resource_list_access"));
    assert!(subject_handler.contains("curriculum_access_policy::resolve_subject_read_access"));
    assert!(subject_handler.contains("curriculum_access_policy::resolve_subject_manage_access"));
    assert!(subject_handler.contains("curriculum_access_policy::ensure_subject_manage"));
    assert!(!subject_service.contains("actor.has_permission("));
    assert!(!subject_service.contains("ResourceAccessPermissions"));
    assert!(!subject_service.contains("resource_access_policy::"));
}

#[test]
fn student_profile_access_uses_resource_policy_and_separate_pii_scope() {
    let policies_root = read_source(manifest_dir().join("src/policies.rs"));
    let student_handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/students/handlers.rs"),
    ));
    let student_service = strip_comments(&read_source(
        manifest_dir().join("src/modules/students/services.rs"),
    ));
    let backend_registry = read_source(manifest_dir().join("src/permissions/registry.rs"));
    let frontend_registry = read_source(
        repo_root()
            .join("frontend-school")
            .join("src/lib/permissions/registry.ts"),
    );

    assert!(policies_root.contains("pub mod student_access_policy;"));
    assert!(student_handler.contains("student_access_policy::can_read_student_profile"));
    assert!(student_handler.contains("student_access_policy::can_read_student_pii"));
    assert!(student_handler.contains("student_access_policy::resolve_student_list_access"));
    assert!(!student_handler.contains("actor.require_permission(codes::STUDENT_READ"));
    assert!(student_service.contains("UserResourceListAccess"));
    assert!(student_service.contains("include_pii: bool"));
    assert!(student_service.contains("hide_student_pii_fields"));

    for source in [&backend_registry, &frontend_registry] {
        assert!(source.contains("student.read.school"));
        assert!(source.contains("student.read.assigned"));
        assert!(source.contains("student_pii.read.own"));
        assert!(source.contains("student_pii.read.assigned"));
        assert!(source.contains("student_pii.read.school"));
    }
}

#[test]
fn achievement_access_uses_resource_policy_and_no_plain_stderr_logging() {
    let policies_root = read_source(manifest_dir().join("src/policies.rs"));
    let achievement_service = strip_comments(&read_source(
        manifest_dir().join("src/modules/achievement/services.rs"),
    ));

    assert!(policies_root.contains("pub mod achievement_access_policy;"));
    assert!(
        achievement_service.contains("achievement_access_policy::resolve_achievement_list_access")
    );
    assert!(achievement_service.contains("achievement_access_policy::can_create_achievement_for"));
    assert!(achievement_service.contains("achievement_access_policy::can_update_achievement"));
    assert!(achievement_service.contains("achievement_access_policy::can_delete_achievement"));
    assert!(achievement_service.contains("UserResourceListAccess"));
    assert!(!achievement_service.contains("actor.has_permission(codes::ACHIEVEMENT"));
    assert!(!achievement_service.contains("eprintln!"));
}

#[test]
fn activity_manage_own_uses_resource_policy_for_group_access() {
    let policies_root = read_source(manifest_dir().join("src/policies.rs"));
    let activity_handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/handlers/activity.rs"),
    ));
    let activity_service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/activity_service.rs"),
    ));

    assert!(policies_root.contains("pub mod activity_access_policy;"));
    assert!(activity_handler.contains("activity_access_policy::resolve_activity_list_access"));
    assert!(activity_handler.contains("activity_service::list_slots(&pool, filter, access)"));
    assert!(activity_handler.contains("activity_service::list_groups(&pool, filter, access)"));
    assert!(activity_handler.contains("activity_service::create_group(&pool, &actor, body)"));
    assert!(activity_handler.contains("activity_service::update_group(&pool, &actor, id, body)"));
    assert!(activity_handler.contains("activity_service::add_group_instructor"));
    assert!(activity_handler.contains("activity_service::remove_group_instructor"));
    assert!(!activity_handler.contains("actor.has_permission(codes::ACTIVITY_MANAGE"));

    assert!(activity_service.contains("activity_access_policy::can_manage_activity_group"));
    assert!(activity_service.contains("activity_access_policy::can_create_activity_group_for"));
    assert!(activity_service.contains("UserResourceListAccess"));
    assert!(!activity_service.contains("actor.has_permission(codes::ACTIVITY_MANAGE"));
}

#[test]
fn organization_delegation_handlers_use_policy_layer_for_authorization() {
    let policies_root = read_source(manifest_dir().join("src/policies.rs"));
    let organization_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/organization_access_policy.rs"),
    ));
    let delegation_handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/staff/handlers/organization_delegations.rs"),
    ));

    assert!(policies_root.contains("pub mod organization_access_policy;"));
    assert!(organization_policy.contains("ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT"));
    assert!(organization_policy.contains("is_organization_unit_leader"));
    assert!(organization_policy.contains("can_revoke_organization_delegation"));
    assert!(
        delegation_handler.contains("organization_access_policy::can_approve_organization_work")
    );
    assert!(delegation_handler
        .contains("organization_access_policy::can_revoke_organization_delegation"));
    assert!(!delegation_handler.contains("actor.has_permission("));
}

#[test]
fn organization_delegation_authorizing_positions_are_explicit() {
    let delegation_service = strip_comments(&read_source(
        manifest_dir().join("src/modules/staff/services/organization_delegation_service.rs"),
    ));

    assert!(delegation_service
        .contains("position_code IN ('director', 'deputy_director', 'head', 'deputy_head')"));
    assert!(!delegation_service.contains(
        "position_code IN ('director', 'deputy_director', 'head', 'deputy_head', 'coordinator'"
    ));
    assert!(!delegation_service.contains(
        "position_code IN ('director', 'deputy_director', 'head', 'deputy_head', 'member'"
    ));
}

#[test]
fn organization_delegatable_permissions_are_unique_across_position_grants() {
    let delegation_service = strip_comments(&read_source(
        manifest_dir().join("src/modules/staff/services/organization_delegation_service.rs"),
    ));

    assert!(
        delegation_service.contains("GROUP BY p.id")
            || delegation_service.contains("DISTINCT ON (p.id)")
            || delegation_service.contains("SELECT DISTINCT p.id"),
        "delegatable permissions must collapse position-scoped organization grants to one row per permission"
    );
}

#[test]
fn staff_access_policy_uses_resource_access_foundation() {
    let policies_root = read_source(manifest_dir().join("src/policies.rs"));
    let staff_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/staff_access_policy.rs"),
    ));
    let resource_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/resource_access_policy.rs"),
    ));

    assert!(policies_root.contains("pub mod resource_access_policy;"));
    assert!(staff_policy.contains("resource_access_policy::ResourceAccessPermissions"));
    assert!(staff_policy.contains("resource_access_policy::require_user_resource_access"));
    assert!(staff_policy.contains("resource_access_policy::can_access_direct_resource"));
    assert!(!staff_policy.contains("WITH RECURSIVE"));
    assert!(!staff_policy.contains("FROM organization_members"));

    for required_type in [
        "pub enum ResourceAccessScope",
        "pub struct ResourceAccessPermissions",
        "pub struct ResourceAccessTarget",
        "pub async fn require_user_resource_access",
        "pub fn can_access_direct_resource",
    ] {
        assert!(
            resource_policy.contains(required_type),
            "resource access foundation must define `{required_type}`"
        );
    }
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
fn backend_runtime_uses_structured_logging_instead_of_plain_stdout_stderr() {
    let mut violations = Vec::new();

    for file in backend_rs_files() {
        let file_name = relative(&file);
        if file_name.starts_with("src/bin/") {
            continue;
        }

        let source = strip_comments(&read_source(&file));
        for pattern in ["println!", "eprintln!"] {
            if source.contains(pattern) {
                violations.push(format!(
                    "{}: use tracing macros instead of {pattern} in runtime code",
                    file_name
                ));
            }
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
        (
            "src/modules/academic/services/study_plan_service.rs",
            "grade_level_ids: Option<serde_json::Value",
        ),
        (
            "src/modules/academic/services/study_plan_service.rs",
            "catalog_grade_level_ids: Option<serde_json::Value",
        ),
        (
            "src/modules/academic/services/timetable_template_service.rs",
            "grade_level_ids: serde_json::Value",
        ),
        (
            "src/modules/academic/services/timetable_template_service.rs",
            "classroom_ids: serde_json::Value",
        ),
        (
            "src/modules/academic/services/timetable_template_service.rs",
            "instructor_ids: serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduling_service.rs",
            "hard_unavailable_slots: serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduling_service.rs",
            "preferred_slots: serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduling_service.rs",
            "preferred_days: serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduling_service.rs",
            "avoid_days: serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduling_service.rs",
            "for_subjects: serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduling_service.rs",
            "scope_ids: serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduling_service.rs",
            "period_ids: serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduling_service.rs",
            "classroom_ids: serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduling_service.rs",
            "config: serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduling_service.rs",
            "failed_courses: serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduling_config_service.rs",
            "use serde_json::Value",
        ),
        (
            "src/modules/academic/services/scheduler_data.rs",
            "Option<serde_json::Value>",
        ),
        (
            "src/modules/academic/models.rs",
            "advisors: serde_json::Value",
        ),
        (
            "src/modules/admission/models/rounds.rs",
            "selection_settings: Option<serde_json::Value",
        ),
        (
            "src/modules/admission/models/rounds.rs",
            "subjects_by_track: Option<serde_json::Value",
        ),
        (
            "src/modules/admission/models/rounds.rs",
            "method_by_track: Option<serde_json::Value",
        ),
        (
            "src/modules/admission/services/portal_service.rs",
            "selection_settings: Option<serde_json::Value",
        ),
        (
            "src/modules/admission/models/applications.rs",
            "parent_status: Option<serde_json::Value",
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
fn remaining_raw_json_values_are_explicit_dynamic_payloads() {
    let allowed_dynamic_value_patterns = [
        (
            "src/modules/academic/models.rs",
            "pub metadata: Option<serde_json::Value>",
        ),
        (
            "src/modules/academic/models/course_planning.rs",
            "pub settings: serde_json::Value",
        ),
        (
            "src/modules/academic/models/course_planning.rs",
            "pub settings: Option<serde_json::Value>",
        ),
        (
            "src/modules/academic/models/study_plans.rs",
            "pub metadata: serde_json::Value",
        ),
        (
            "src/modules/academic/websockets.rs",
            "entry: serde_json::Value",
        ),
        (
            "src/modules/academic/websockets.rs",
            "entry_a: serde_json::Value",
        ),
        (
            "src/modules/academic/websockets.rs",
            "entry_b: serde_json::Value",
        ),
        (
            "src/modules/academic/websockets.rs",
            "target: Option<serde_json::Value>",
        ),
        (
            "src/modules/admission/models/applications.rs",
            "pub metadata: serde_json::Value",
        ),
        (
            "src/modules/admission/models/applications.rs",
            "pub form_data: serde_json::Value",
        ),
        (
            "src/modules/admission/models/applications.rs",
            "pub form_data: Option<serde_json::Value>",
        ),
        (
            "src/modules/admission/models/rounds.rs",
            "pub report_config: Option<serde_json::Value>",
        ),
        (
            "src/modules/admission/services/application_service.rs",
            "pub form_data: Option<serde_json::Value>",
        ),
        (
            "src/modules/admission/services/application_service.rs",
            "let form_data: Option<serde_json::Value>",
        ),
        (
            "src/modules/auth/models.rs",
            "pub metadata: serde_json::Value",
        ),
        (
            "src/modules/consent/models.rs",
            "pub metadata: serde_json::Value",
        ),
        (
            "src/modules/consent/services.rs",
            "metadata: serde_json::Value",
        ),
    ];
    let mut violations = Vec::new();

    for file in module_rs_files() {
        let file_name = relative(&file);
        let mut source = strip_comments(&read_source(&file));
        if !source.contains("serde_json::Value") {
            continue;
        }

        for (allowed_file, allowed_pattern) in allowed_dynamic_value_patterns {
            if file_name == allowed_file {
                source = source.replace(allowed_pattern, "");
            }
        }

        if source.contains("serde_json::Value") {
            violations.push(format!(
                "{file_name}: raw serde_json::Value must be typed or explicitly allowlisted as dynamic metadata/form/config/WebSocket payload"
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
fn permission_registry_codes_match_declared_module_action_scope() {
    let registry = read_source(manifest_dir().join("src/permissions/registry.rs"));
    let permission_const_pattern =
        Regex::new(r#"pub const (?P<constant>[A-Z0-9_]+):\s*&str\s*=\s*"(?P<code>[^"]+)";"#)
            .expect("valid regex");
    let permission_def_pattern = Regex::new(
        r#"(?s)PermissionDef\s*\{\s*code:\s*codes::(?P<constant>[A-Z0-9_]+).*?module:\s*"(?P<module>[^"]+)".*?action:\s*"(?P<action>[^"]+)".*?scope:\s*"(?P<scope>[^"]+)""#,
    )
    .expect("valid regex");
    let permission_codes = permission_const_pattern
        .captures_iter(&registry)
        .map(|captures| {
            (
                captures
                    .name("constant")
                    .expect("permission constant")
                    .as_str()
                    .to_string(),
                captures
                    .name("code")
                    .expect("permission code")
                    .as_str()
                    .to_string(),
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let mut violations = Vec::new();

    for captures in permission_def_pattern.captures_iter(&registry) {
        let constant = captures
            .name("constant")
            .expect("permission constant")
            .as_str();
        let module = captures.name("module").expect("permission module").as_str();
        let action = captures.name("action").expect("permission action").as_str();
        let scope = captures.name("scope").expect("permission scope").as_str();

        if constant == "WILDCARD" {
            continue;
        }

        let expected_constant = format!(
            "{}_{}_{}",
            module.to_ascii_uppercase(),
            action.to_ascii_uppercase(),
            scope.to_ascii_uppercase()
        );
        let expected_code = format!("{module}.{action}.{scope}");

        if constant != expected_constant {
            violations.push(format!(
                "codes::{constant} should be named codes::{expected_constant} for {module}.{action}.{scope}"
            ));
        }

        if permission_codes.get(constant).map(String::as_str) != Some(expected_code.as_str()) {
            violations.push(format!(
                "codes::{constant} should be `{expected_code}` to match its PermissionDef fields"
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn permission_registry_uses_canonical_action_and_scope_vocabulary() {
    let registry = read_source(manifest_dir().join("src/permissions/registry.rs"));
    let permission_def_pattern = Regex::new(
        r#"(?s)PermissionDef\s*\{.*?code:\s*codes::(?P<constant>[A-Z0-9_]+).*?action:\s*"(?P<action>[^"]+)".*?scope:\s*"(?P<scope>[^"]+)""#,
    )
    .expect("valid regex");
    let allowed_actions = [
        "all",
        "approve",
        "assign",
        "create",
        "delete",
        "enroll",
        "evaluate",
        "execute",
        "manage",
        "manage_members",
        "read",
        "remove",
        "request",
        "scores",
        "update",
        "verify",
    ];
    let allowed_scopes = [
        "all",
        "assigned",
        "global",
        "organization_tree",
        "organization_unit",
        "own",
        "school",
    ];
    let mut violations = Vec::new();

    for captures in permission_def_pattern.captures_iter(&registry) {
        let constant = captures
            .name("constant")
            .expect("permission constant")
            .as_str();
        let action = captures.name("action").expect("permission action").as_str();
        let scope = captures.name("scope").expect("permission scope").as_str();

        if !allowed_actions.contains(&action) {
            violations.push(format!(
                "codes::{constant} uses unsupported action `{action}`"
            ));
        }
        if !allowed_scopes.contains(&scope) {
            violations.push(format!(
                "codes::{constant} uses unsupported scope `{scope}`"
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
fn menu_workspace_contract_is_explicit_and_permission_based() {
    let menu_models = read_source(manifest_dir().join("src/modules/menu/models.rs"));
    let public_menu_service =
        read_source(manifest_dir().join("src/modules/menu/services/public_menu_service.rs"));
    let public_menu_handler =
        read_source(manifest_dir().join("src/modules/menu/handlers/public.rs"));
    let route_registration_service = read_source(
        manifest_dir().join("src/modules/system/services/route_registration_service.rs"),
    );
    let route_migration =
        read_source(manifest_dir().join("migrations/002_menu_workspace_code.sql"));

    assert!(menu_models.contains("pub workspace: Option<String>"));
    assert!(menu_models.contains("pub workspace_code: String"));
    assert!(menu_models.contains("#[serde(rename_all = \"camelCase\")]"));
    assert!(public_menu_service.contains("mg.workspace_code"));
    assert!(public_menu_handler.contains("workspace_code: group_workspace_code"));
    assert!(route_registration_service.contains("route_workspace_code("));
    assert!(route_migration.contains("workspace_code"));
    assert!(!public_menu_service.contains("feature_toggles"));
    assert!(!public_menu_handler.contains("feature_toggles"));
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
fn work_change_sse_supports_work_item_and_window_refresh_signals() {
    let event_source = read_source(manifest_dir().join("src/modules/notification/events.rs"));
    let notification_handler =
        read_source(manifest_dir().join("src/modules/notification/handlers.rs"));
    let app_state = read_source(manifest_dir().join("src/main.rs"));
    let work_handler = read_source(manifest_dir().join("src/modules/work/handlers.rs"));
    let workflow_handler = read_source(manifest_dir().join("src/modules/workflow/handlers.rs"));

    for expected in [
        "pub enum WorkChangeKind",
        "WorkItemsChanged",
        "WorkflowWindowChanged",
        "pub struct WorkChangeEvent",
    ] {
        assert!(event_source.contains(expected), "missing {expected}");
    }

    for expected in [
        "work_event_channel.subscribe()",
        "event.event_name()",
        ".event(event.event_name())",
    ] {
        assert!(
            notification_handler.contains(expected),
            "missing {expected}"
        );
    }

    for expected in [
        "notify_work_items_changed(&self)",
        "notify_workflow_window_changed(&self)",
    ] {
        assert!(app_state.contains(expected), "missing {expected}");
    }

    assert!(work_handler.contains("state.notify_work_items_changed()"));
    assert!(workflow_handler.contains("state.notify_workflow_window_changed()"));
}

#[test]
fn teaching_supervision_registry_and_module_are_registered() {
    let registry = read_source(manifest_dir().join("src/permissions/registry.rs"));
    let modules = read_source(manifest_dir().join("src/modules.rs"));

    for expected in [
        "SUPERVISION_READ_OWN",
        "SUPERVISION_READ_ASSIGNED",
        "SUPERVISION_READ_ORGANIZATION_UNIT",
        "SUPERVISION_READ_ORGANIZATION_TREE",
        "SUPERVISION_READ_SCHOOL",
        "SUPERVISION_REQUEST_OWN",
        "SUPERVISION_MANAGE_SCHOOL",
        "SUPERVISION_EVALUATE_ASSIGNED",
        "SUPERVISION_APPROVE_SCHOOL",
        "supervision.read.own",
        "supervision.approve.school",
    ] {
        assert!(
            registry.contains(expected),
            "missing supervision registry entry {expected}"
        );
    }

    assert!(modules.contains("pub mod supervision;"));
}

#[test]
fn teaching_supervision_handlers_use_request_context_and_services() {
    let handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/supervision/handlers.rs"),
    ));

    assert!(handler.contains("actor_tenant_context"));
    assert!(handler.contains("ApiResponse::ok"));
    assert!(handler.contains("supervision_access_policy"));
    assert!(handler.contains("services::"));
    assert!(!handler.contains("sqlx::query"));
    assert!(!handler.contains(".fetch_"));
    assert!(!handler.contains(".execute("));
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
