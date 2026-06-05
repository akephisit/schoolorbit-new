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
fn foundation_handlers_delegate_database_work_to_services() {
    let mut handler_files = list_files(manifest_dir().join("src/modules/staff/handlers"), |path| {
        path.extension().is_some_and(|ext| ext == "rs")
    });

    for relative_path in [
        "src/modules/achievement/handlers.rs",
        "src/modules/consent/handlers.rs",
        "src/modules/files/handlers.rs",
        "src/modules/lookup/handlers.rs",
        "src/modules/notification/handlers.rs",
        "src/modules/parents/handlers.rs",
        "src/modules/school/handlers.rs",
        "src/modules/students/handlers.rs",
        "src/modules/students/handlers_parents.rs",
    ] {
        handler_files.push(manifest_dir().join(relative_path));
    }

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

    for file in handler_files {
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
            && Regex::new(r"\.pool_manager\s*\.get_pool\s*\(")
                .expect("valid regex")
                .is_match(&source)
        {
            violations.push(format!(
                "{file_name}: use utils::tenant resolver instead of pool_manager.get_pool"
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}
