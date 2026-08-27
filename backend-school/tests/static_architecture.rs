use regex::Regex;
use std::collections::BTreeSet;
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

fn assert_typed_session_handlers(paths: &[&str]) {
    for path in paths {
        let source = read_source(manifest_dir().join(path));
        assert!(
            source.contains("Extension(session): Extension<AuthenticatedSession>"),
            "{path} must extract the central authenticated session"
        );
        assert!(!source.contains("actor_tenant_context(&state, &headers)"));
        assert!(!source.contains("current_user_tenant_context_from_headers"));
    }
}

#[test]
fn people_and_platform_handlers_use_typed_session_identity() {
    assert_typed_session_handlers(&[
        "src/modules/achievement/handlers.rs",
        "src/modules/files/handlers.rs",
        "src/modules/menu/handlers/admin.rs",
        "src/modules/menu/handlers/public.rs",
        "src/modules/notification/handlers.rs",
        "src/modules/parents/handlers.rs",
        "src/modules/school/handlers.rs",
        "src/modules/staff/handlers/organization_delegations.rs",
        "src/modules/staff/handlers/organization_members.rs",
        "src/modules/staff/handlers/organization_permissions.rs",
        "src/modules/staff/handlers/permissions.rs",
        "src/modules/staff/handlers/roles.rs",
        "src/modules/staff/handlers/staff.rs",
        "src/modules/staff/handlers/user_roles.rs",
        "src/modules/students/handlers.rs",
        "src/modules/students/handlers_parents.rs",
        "src/modules/system/handlers/feature_toggles.rs",
    ]);
}

#[test]
fn academic_group_a_handlers_use_typed_session_identity() {
    assert_typed_session_handlers(&[
        "src/modules/academic/core/handlers.rs",
        "src/modules/academic/handlers/assessment.rs",
    ]);
}

#[test]
fn academic_group_b_handlers_use_typed_session_identity() {
    assert_typed_session_handlers(&[
        "src/modules/academic/handlers/exam_schedule.rs",
        "src/modules/academic/handlers/timetable.rs",
        "src/modules/academic/handlers/timetable_templates.rs",
    ]);
}

#[test]
fn remaining_vertical_handlers_use_typed_session_identity() {
    assert_typed_session_handlers(&[
        "src/modules/admission/handlers/applications.rs",
        "src/modules/admission/handlers/exam_rooms.rs",
        "src/modules/admission/handlers/rounds.rs",
        "src/modules/admission/handlers/scores.rs",
        "src/modules/admission/handlers/selections.rs",
        "src/modules/calendar/handlers.rs",
        "src/modules/facility/handlers.rs",
        "src/modules/question_bank/handlers.rs",
        "src/modules/supervision/handlers.rs",
        "src/modules/work/handlers.rs",
        "src/modules/workflow/handlers.rs",
        "src/modules/lookup/handlers.rs",
    ]);
}

#[test]
fn only_auth_boundary_parses_browser_session_credentials() {
    let credential_boundaries = [
        "src/middleware/session.rs",
        "src/modules/auth/http.rs",
        "src/modules/auth/session_handlers.rs",
        "src/modules/academic/websockets.rs",
    ];
    let cookie_name_owners = [
        "src/middleware/session.rs",
        "src/modules/auth/config.rs",
        "src/modules/auth/http.rs",
        "src/modules/auth/session_handlers.rs",
        "src/modules/academic/websockets.rs",
    ];

    for file in backend_rs_files() {
        let relative = relative(&file);
        let source = read_source(&file);
        if !credential_boundaries.contains(&relative.as_str()) {
            assert!(
                !source.contains("presented_session_token("),
                "credential parse in {relative}"
            );
        }
        if !cookie_name_owners.contains(&relative.as_str()) {
            assert!(
                !source.contains("SESSION_COOKIE_NAME"),
                "cookie access in {relative}"
            );
        }
    }

    let context = read_source(manifest_dir().join("src/utils/request_context.rs"));
    assert!(!context.contains("_from_headers"));
    assert!(!context.contains("HeaderMap"));
}

#[test]
fn notification_sse_binds_stream_lifetime_to_authoritative_session() {
    let source = read_source(manifest_dir().join("src/modules/notification/handlers.rs"));
    let handler = extract_braced_block(&source, "pub async fn stream_notifications", false);

    let notification_subscribe = handler
        .find("notification_channel.subscribe()")
        .expect("SSE must subscribe to notifications before session revalidation");
    let permission_subscribe = handler
        .find("permission_event_channel.subscribe()")
        .expect("SSE must subscribe to permission events before session revalidation");
    let work_subscribe = handler
        .find("work_event_channel.subscribe()")
        .expect("SSE must subscribe to work events before session revalidation");
    let session_subscribe = handler
        .find("session_events.subscribe()")
        .expect("SSE must subscribe to session events before session revalidation");
    let immediate_revalidation = handler
        .find("session_service::revalidate(&session, Utc::now())")
        .expect("SSE must immediately revalidate its authenticated session");
    assert!(notification_subscribe < immediate_revalidation);
    assert!(permission_subscribe < immediate_revalidation);
    assert!(work_subscribe < immediate_revalidation);
    assert!(session_subscribe < immediate_revalidation);

    assert!(source.contains("Instant::now() + SESSION_REVALIDATION_INTERVAL"));
    assert!(source.contains("MissedTickBehavior::Delay"));
    assert!(source.contains("event(\"session_invalid\").data(\"{}\")"));
    assert!(source.contains("event(\"session_unavailable\")"));
    assert!(source.contains("HeaderValue::from_static(\"no\")"));
    assert!(!source.contains("SessionUnavailable(error)"));
}

#[test]
fn exam_schedule_shared_module_is_private() {
    let facade_path = manifest_dir().join("src/modules/academic/services/exam_schedule_service.rs");
    let shared_path =
        manifest_dir().join("src/modules/academic/services/exam_schedule_service/shared.rs");
    let source = read_source(facade_path);

    assert!(source.contains("mod shared;"));
    assert!(!source.contains("pub mod shared;"));
    assert!(shared_path.is_file());
}

#[test]
fn supervision_service_uses_private_child_modules() {
    let facade = read_source(manifest_dir().join("src/modules/supervision/services.rs"));
    let shared = manifest_dir().join("src/modules/supervision/services/shared.rs");

    assert!(facade.contains("mod shared;"));
    assert!(!facade.contains("pub mod shared;"));
    assert!(shared.is_file());
}

#[test]
fn supervision_service_facade_is_thin_and_preserves_public_surface() {
    let facade = read_source(manifest_dir().join("src/modules/supervision/services.rs"));
    let service_dir = manifest_dir().join("src/modules/supervision/services");

    for module in [
        "cycles",
        "evaluations",
        "observations",
        "reviews_and_reports",
        "shared",
        "templates",
    ] {
        assert!(facade.contains(&format!("mod {module};")));
        assert!(!facade.contains(&format!("pub mod {module};")));
        assert!(service_dir.join(format!("{module}.rs")).is_file());
        assert!(facade.contains(&format!("pub use {module}")));
    }

    for public_item in [
        "acknowledge_observation",
        "all_required_evaluators_submitted",
        "approve_observation",
        "approve_observation_request",
        "average_submitted_evaluator_rating",
        "cancel_observation",
        "cancel_requested_observation",
        "can_transition_observation_status",
        "can_view_observation_results",
        "certify_observation",
        "create_cycle",
        "create_template",
        "cycle_progress",
        "cycle_teacher_status",
        "evaluator_availability",
        "evaluator_conflict_status_codes",
        "get_cycle",
        "get_observation",
        "get_observation_review",
        "get_template",
        "list_cycles",
        "list_observations",
        "list_templates",
        "manager_can_edit_observation",
        "observation_timetable_options",
        "replace_observation_evaluators",
        "request_observation",
        "resolve_supervision_target_rule",
        "return_observation_request",
        "submit_my_evaluation",
        "teacher_can_edit_requested_observation",
        "update_cycle",
        "update_observation",
        "update_requested_observation",
        "update_template",
    ] {
        assert!(
            facade.contains(public_item),
            "supervision facade must re-export `{public_item}`"
        );
    }

    for forbidden in ["sqlx::", ".fetch_", ".execute(", ".begin("] {
        assert!(
            !facade.contains(forbidden),
            "supervision facade must not contain `{forbidden}`"
        );
    }

    assert!(
        facade
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()
            <= 80,
        "supervision facade must remain under 80 nonblank lines"
    );
}

#[test]
fn timetable_service_uses_only_canonical_delivery_identity() {
    let service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/timetable_service.rs"),
    ));

    for required in [
        "academic_term_id",
        "learning_group_id",
        "learning_offering_id",
        "bell_schedule_period_id",
        "row_version",
        "learning_group_teachers",
    ] {
        assert!(
            service.contains(required),
            "canonical timetable service must contain `{required}`"
        );
    }

    for forbidden in [
        "academic_semesters",
        "classroom_courses",
        "activity_slots",
        "legacy_classroom_course_id",
        "legacy_activity_slot_id",
    ] {
        assert!(
            !service.contains(forbidden),
            "canonical timetable service must not contain `{forbidden}`"
        );
    }
}

#[test]
fn calendar_service_uses_private_child_modules() {
    let facade = read_source(manifest_dir().join("src/modules/calendar/services.rs"));
    let service_dir = manifest_dir().join("src/modules/calendar/services");

    for module in [
        "categories_and_tags",
        "events",
        "notifications",
        "reminders",
        "shared",
        "visibility",
    ] {
        assert!(
            facade.contains(&format!("mod {module};")),
            "calendar facade must declare private module `{module}`"
        );
        assert!(
            !facade.contains(&format!("pub mod {module};")),
            "calendar child module `{module}` must remain private"
        );
        assert!(
            service_dir.join(format!("{module}.rs")).is_file(),
            "calendar child module `{module}` must have its own source file"
        );
    }

    for public_item in [
        "create_event",
        "list_management_events",
        "list_my_events",
        "list_child_events",
        "list_public_events",
        "resolve_event_recipient_user_ids",
        "send_event_notification",
        "process_due_reminders",
        "process_due_calendar_reminders_for_all_tenants",
    ] {
        assert!(
            facade.contains(public_item),
            "calendar facade must preserve public item `{public_item}`"
        );
    }

    for forbidden in ["sqlx::", ".fetch_", ".execute(", ".begin(", "SELECT "] {
        assert!(
            !facade.contains(forbidden),
            "calendar facade must not contain persistence fragment `{forbidden}`"
        );
    }

    let nonblank_line_count = facade
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert!(
        nonblank_line_count <= 75,
        "calendar facade must stay thin; found {nonblank_line_count} nonblank lines"
    );
}

#[test]
fn exam_schedule_service_uses_a_thin_private_module_facade() {
    let service_dir = manifest_dir().join("src/modules/academic/services/exam_schedule_service");
    let facade_path = manifest_dir().join("src/modules/academic/services/exam_schedule_service.rs");
    let source = read_source(facade_path);

    for module in [
        "invigilation",
        "published_views",
        "publishing",
        "room_assignments",
        "rounds_and_days",
        "sessions_and_conflicts",
        "shared",
        "workspace",
    ] {
        assert!(
            source.contains(&format!("mod {module};")),
            "exam schedule facade must declare private module `{module}`"
        );
        assert!(
            !source.contains(&format!("pub mod {module};")),
            "exam schedule child module `{module}` must remain private"
        );
        assert!(
            service_dir.join(format!("{module}.rs")).is_file(),
            "exam schedule child module `{module}` must have its own source file"
        );
    }

    for public_function in [
        "assign_invigilator_to_assignment",
        "clear_mismatched_exam_items",
        "create_round",
        "delete_exam_day",
        "delete_exam_session",
        "generate_seats_for_assignment",
        "get_invigilator_workspace",
        "get_workspace",
        "import_exam_items",
        "list_child_published_exam_schedule",
        "list_day_room_assignments",
        "list_invigilator_staff_options",
        "list_my_published_exam_schedule",
        "list_rounds",
        "list_staff_published_exam_schedule",
        "place_exam_session",
        "publish_round",
        "remove_invigilator_from_assignment",
        "update_assignment_invigilators",
        "update_exam_day",
        "update_round",
        "upsert_day_room_assignment",
        "upsert_exam_day",
    ] {
        assert!(
            source.contains(public_function),
            "exam schedule facade must preserve public function `{public_function}`"
        );
    }

    let nonblank_line_count = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert!(
        nonblank_line_count <= 90,
        "exam schedule facade must stay thin; found {nonblank_line_count} nonblank lines"
    );

    for forbidden in ["sqlx::query", ".begin().await", "sqlx::FromRow", "SELECT "] {
        assert!(
            !source.contains(forbidden),
            "exam schedule facade must not contain persistence workflow fragment `{forbidden}`"
        );
    }
}

#[test]
fn student_deactivation_invalidates_effective_permissions() {
    let source = read_source(manifest_dir().join("src/modules/students/handlers.rs"));
    let start = source
        .find("pub async fn delete_student")
        .expect("delete_student handler should exist");
    let body = &source[start..];

    assert!(body.contains("permission_cache.invalidate_user(&tenant, student_id)"));
    assert!(body.contains("notify_permission_changed(&tenant, student_id)"));
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

struct LexicalMask {
    structural: String,
    comments: Vec<bool>,
    literals: Vec<bool>,
}

fn mark_non_structural(structural: &mut [u8], mask: &mut [bool], start: usize, end: usize) {
    for index in start..end {
        mask[index] = true;
        if structural[index] != b'\n' {
            structural[index] = b' ';
        }
    }
}

fn raw_string_opening(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        return None;
    }

    let mut cursor = if bytes.get(start) == Some(&b'r') {
        start + 1
    } else if bytes.get(start) == Some(&b'b') && bytes.get(start + 1) == Some(&b'r') {
        start + 2
    } else {
        return None;
    };
    let mut hashes = 0;
    while bytes.get(cursor) == Some(&b'#') {
        hashes += 1;
        cursor += 1;
    }

    (bytes.get(cursor) == Some(&b'"')).then_some((cursor, hashes))
}

fn raw_string_end(bytes: &[u8], quote: usize, hashes: usize) -> usize {
    let mut cursor = quote + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"'
            && cursor + hashes < bytes.len()
            && bytes[cursor + 1..=cursor + hashes]
                .iter()
                .all(|byte| *byte == b'#')
        {
            return cursor + hashes + 1;
        }
        cursor += 1;
    }
    bytes.len()
}

fn quoted_string_opening(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) == Some(&b'"') {
        return Some(start);
    }
    if matches!(bytes.get(start), Some(b'b' | b'c')) && bytes.get(start + 1) == Some(&b'"') {
        return Some(start + 1);
    }
    None
}

fn quoted_string_end(bytes: &[u8], quote: usize) -> usize {
    let mut cursor = quote + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor = (cursor + 2).min(bytes.len()),
            b'"' => return cursor + 1,
            _ => cursor += 1,
        }
    }
    bytes.len()
}

fn character_literal_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'\'') {
        return None;
    }

    let mut cursor = start + 1;
    if bytes.get(cursor) == Some(&b'\\') {
        cursor += 1;
        match bytes.get(cursor) {
            Some(b'u') if bytes.get(cursor + 1) == Some(&b'{') => {
                cursor += 2;
                cursor += bytes[cursor..].iter().position(|byte| *byte == b'}')? + 1;
            }
            Some(b'x') => cursor = (cursor + 3).min(bytes.len()),
            Some(_) => cursor += 1,
            None => return None,
        }
    } else {
        let character = source[cursor..].chars().next()?;
        if character == '\n' || character == '\r' || character == '\'' {
            return None;
        }
        cursor += character.len_utf8();
    }

    (bytes.get(cursor) == Some(&b'\'')).then_some(cursor + 1)
}

fn lexical_mask(source: &str, hash_line_comments: bool) -> LexicalMask {
    let bytes = source.as_bytes();
    let mut structural = bytes.to_vec();
    let mut comments = vec![false; bytes.len()];
    let mut literals = vec![false; bytes.len()];
    let mut cursor = 0;

    while cursor < bytes.len() {
        if bytes.get(cursor) == Some(&b'/') && bytes.get(cursor + 1) == Some(&b'/') {
            let end = bytes[cursor..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| cursor + offset);
            mark_non_structural(&mut structural, &mut comments, cursor, end);
            cursor = end;
            continue;
        }

        if hash_line_comments && bytes.get(cursor) == Some(&b'#') {
            let end = bytes[cursor..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| cursor + offset);
            mark_non_structural(&mut structural, &mut comments, cursor, end);
            cursor = end;
            continue;
        }

        if bytes.get(cursor) == Some(&b'/') && bytes.get(cursor + 1) == Some(&b'*') {
            let mut end = cursor + 2;
            let mut depth = 1_u32;
            while end < bytes.len() && depth > 0 {
                if bytes.get(end) == Some(&b'/') && bytes.get(end + 1) == Some(&b'*') {
                    depth += 1;
                    end += 2;
                } else if bytes.get(end) == Some(&b'*') && bytes.get(end + 1) == Some(&b'/') {
                    depth -= 1;
                    end += 2;
                } else {
                    end += 1;
                }
            }
            mark_non_structural(&mut structural, &mut comments, cursor, end);
            cursor = end;
            continue;
        }

        if let Some((quote, hashes)) = raw_string_opening(bytes, cursor) {
            let end = raw_string_end(bytes, quote, hashes);
            mark_non_structural(&mut structural, &mut literals, cursor, end);
            cursor = end;
            continue;
        }

        if let Some(quote) = quoted_string_opening(bytes, cursor) {
            let end = quoted_string_end(bytes, quote);
            mark_non_structural(&mut structural, &mut literals, cursor, end);
            cursor = end;
            continue;
        }

        if let Some(end) = character_literal_end(source, cursor) {
            mark_non_structural(&mut structural, &mut literals, cursor, end);
            cursor = end;
            continue;
        }

        cursor += 1;
    }

    LexicalMask {
        structural: String::from_utf8(structural).expect("masked source remains UTF-8"),
        comments,
        literals,
    }
}

fn literal_only(source: &str) -> String {
    let lexical = lexical_mask(source, false);
    let mut bytes = source.as_bytes().to_vec();
    for (index, byte) in bytes.iter_mut().enumerate() {
        if !lexical.literals[index] && *byte != b'\n' {
            *byte = b' ';
        }
    }
    String::from_utf8(bytes).expect("literal-only source remains UTF-8")
}

fn balanced_delimiter_end(
    structural: &str,
    opening: usize,
    opening_delimiter: u8,
    closing_delimiter: u8,
) -> Option<usize> {
    let mut depth = 0_u32;
    for (offset, byte) in structural.as_bytes()[opening..].iter().enumerate() {
        if *byte == opening_delimiter {
            depth += 1;
        } else if *byte == closing_delimiter {
            depth -= 1;
            if depth == 0 {
                return Some(opening + offset);
            }
        }
    }
    None
}

fn extract_braced_block(source: &str, marker: &str, hash_line_comments: bool) -> String {
    let lexical = lexical_mask(source, hash_line_comments);
    let start = lexical
        .structural
        .find(marker)
        .unwrap_or_else(|| panic!("missing block marker: {marker}"));
    let opening = lexical.structural[start..]
        .find('{')
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("missing opening brace after marker: {marker}"));
    let closing = balanced_delimiter_end(&lexical.structural, opening, b'{', b'}')
        .unwrap_or_else(|| panic!("unterminated block marker: {marker}"));

    lexical.structural[start..=closing].to_string()
}

#[derive(Debug)]
struct MethodInvocation {
    arguments: Vec<String>,
    start_offset: usize,
    end_offset: usize,
    start_line: usize,
    end_line: usize,
}

fn source_line_at(source: &str, offset: usize) -> usize {
    source.as_bytes()[..offset]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

fn normalize_rust_expression(
    source: &str,
    lexical: &LexicalMask,
    start: usize,
    end: usize,
) -> String {
    let bytes = source.as_bytes();
    let mut normalized = Vec::with_capacity(end - start);

    for (index, byte) in bytes.iter().enumerate().take(end).skip(start) {
        if lexical.comments[index] {
            continue;
        }
        if lexical.literals[index] || !byte.is_ascii_whitespace() {
            normalized.push(*byte);
        }
    }

    String::from_utf8(normalized).expect("normalized Rust expression remains UTF-8")
}

fn split_invocation_arguments(
    source: &str,
    lexical: &LexicalMask,
    start: usize,
    end: usize,
) -> Vec<String> {
    let structural = lexical.structural.as_bytes();
    let mut arguments = Vec::new();
    let mut argument_start = start;
    let mut parentheses = 0_u32;
    let mut brackets = 0_u32;
    let mut braces = 0_u32;

    for (index, byte) in structural.iter().enumerate().take(end).skip(start) {
        match *byte {
            b'(' => parentheses += 1,
            b')' => parentheses = parentheses.saturating_sub(1),
            b'[' => brackets += 1,
            b']' => brackets = brackets.saturating_sub(1),
            b'{' => braces += 1,
            b'}' => braces = braces.saturating_sub(1),
            b',' if parentheses == 0 && brackets == 0 && braces == 0 => {
                let argument = normalize_rust_expression(source, lexical, argument_start, index);
                if !argument.is_empty() {
                    arguments.push(argument);
                }
                argument_start = index + 1;
            }
            _ => {}
        }
    }

    let argument = normalize_rust_expression(source, lexical, argument_start, end);
    if !argument.is_empty() {
        arguments.push(argument);
    }
    arguments
}

fn extract_method_invocations(
    source: &str,
    lexical: &LexicalMask,
    method: &str,
) -> Vec<MethodInvocation> {
    let marker = format!(".{method}");
    let structural = lexical.structural.as_bytes();
    let mut invocations = Vec::new();
    let mut search_offset = 0;

    while let Some(relative_start) = lexical.structural[search_offset..].find(&marker) {
        let start = search_offset + relative_start;
        let mut opening = start + marker.len();
        while structural
            .get(opening)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            opening += 1;
        }
        if structural.get(opening) != Some(&b'(') {
            search_offset = start + marker.len();
            continue;
        }

        let Some(closing) = balanced_delimiter_end(&lexical.structural, opening, b'(', b')') else {
            break;
        };
        invocations.push(MethodInvocation {
            arguments: split_invocation_arguments(source, lexical, opening + 1, closing),
            start_offset: start,
            end_offset: closing,
            start_line: source_line_at(source, start),
            end_line: source_line_at(source, closing),
        });
        search_offset = closing + 1;
    }

    invocations
}

fn has_matching_following_invocation(
    invalidation: &MethodInvocation,
    notifications: &[MethodInvocation],
) -> bool {
    notifications.iter().any(|notification| {
        notification.start_offset > invalidation.end_offset
            && notification.start_line <= invalidation.end_line + 3
            && notification.arguments == invalidation.arguments
    })
}

fn permission_invalidation_violations(relative_path: &str, source: &str) -> Vec<String> {
    let lexical = lexical_mask(source, false);
    let user_notifications =
        extract_method_invocations(source, &lexical, "notify_permission_changed");
    let tenant_notifications =
        extract_method_invocations(source, &lexical, "notify_all_permissions_changed");
    let mut violations = Vec::new();

    for invalidation in extract_method_invocations(source, &lexical, "invalidate_tenant") {
        if !has_matching_following_invocation(&invalidation, &tenant_notifications) {
            violations.push(format!(
                "{relative_path}:{}: tenant invalidation must emit tenant permission_changed with matching tenant",
                invalidation.start_line
            ));
        }
    }

    for invalidation in extract_method_invocations(source, &lexical, "invalidate_user") {
        if !has_matching_following_invocation(&invalidation, &user_notifications) {
            violations.push(format!(
                "{relative_path}:{}: user invalidation must emit tenant permission_changed with matching tenant and user",
                invalidation.start_line
            ));
        }
    }

    violations
}

fn backend_rs_files() -> Vec<PathBuf> {
    list_files(manifest_dir().join("src"), |path| {
        path.extension().is_some_and(|ext| ext == "rs") && !is_rust_test_module(path)
    })
}

fn module_rs_files() -> Vec<PathBuf> {
    list_files(manifest_dir().join("src/modules"), |path| {
        path.extension().is_some_and(|ext| ext == "rs") && !is_rust_test_module(path)
    })
}

fn is_rust_test_module(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem == "tests" || stem.ends_with("_tests"))
}

fn strip_cfg_test_modules(source: &str) -> String {
    let lexical = lexical_mask(source, false);
    let structural = lexical.structural.as_str();
    let marker = "#[cfg(test)]";
    let mut runtime = source.as_bytes().to_vec();
    let mut cursor = 0;

    while let Some(offset) = structural[cursor..].find(marker) {
        let start = cursor + offset;
        let after_marker = start + marker.len();
        let Some(opening_offset) = structural[after_marker..].find('{') else {
            break;
        };
        let opening = after_marker + opening_offset;
        if !structural[after_marker..opening].contains("mod tests") {
            cursor = after_marker;
            continue;
        }
        let closing = balanced_delimiter_end(structural, opening, b'{', b'}')
            .expect("cfg(test) module must have balanced braces");
        for byte in &mut runtime[start..=closing] {
            if *byte != b'\n' && *byte != b'\r' {
                *byte = b' ';
            }
        }
        cursor = closing + 1;
    }

    String::from_utf8(runtime).expect("masking cfg(test) modules preserves UTF-8")
}

fn module_handler_files() -> Vec<PathBuf> {
    let modules_dir = manifest_dir().join("src/modules");
    list_files(&modules_dir, |path| {
        if path.extension().is_none_or(|ext| ext != "rs") {
            return false;
        }
        if is_rust_test_module(path) {
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
        if is_rust_test_module(path) {
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
    let source = strip_comments(source);
    let logic_declaration =
        Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:async\s+)?fn|struct|enum|const)\s+")
            .expect("valid service logic declaration regex");

    let has_module_or_reexport = source.lines().map(str::trim).any(|line| {
        line.starts_with("mod ") || line.starts_with("pub mod ") || line.starts_with("pub use ")
    });

    has_module_or_reexport && !logic_declaration.is_match(&source)
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
fn auth_session_migration_is_forward_only_and_hash_only() {
    let migration = read_source(manifest_dir().join("migrations/034_auth_sessions.sql"));

    for required in [
        "CREATE TABLE auth_sessions",
        "current_token_hash BYTEA NOT NULL",
        "previous_token_hash BYTEA",
        "CREATE UNIQUE INDEX auth_sessions_current_token_hash_key",
        "CREATE UNIQUE INDEX auth_sessions_previous_token_hash_key",
        "CREATE TABLE auth_login_throttles",
        "PRIMARY KEY (bucket_kind, bucket_hash)",
    ] {
        assert!(migration.contains(required), "missing `{required}`");
    }

    for forbidden in [
        "raw_token",
        "csrf_token",
        "username TEXT",
        "ip_address",
        "user_agent",
    ] {
        assert!(
            !migration.contains(forbidden),
            "forbidden persisted field `{forbidden}`"
        );
    }
}

#[test]
fn certificate_migration_keeps_issued_records_restrictive_and_permanent() {
    let migration_path = manifest_dir().join("migrations/035_certificate_issuance.sql");
    let migration = read_source(&migration_path);
    let certificate_reference = Regex::new(
        r"(?i)REFERENCES\s+certificates\s*\(\s*id\s*\)\s+ON\s+DELETE\s+(?P<action>[A-Z_]+)",
    )
    .expect("valid certificate reference regex");
    let delete_actions = certificate_reference
        .captures_iter(&migration)
        .map(|captures| {
            captures
                .name("action")
                .expect("delete action")
                .as_str()
                .to_ascii_uppercase()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        delete_actions.len(),
        4,
        "candidate and replacement links must remain explicit in {}",
        repo_relative(&migration_path)
    );
    assert!(
        delete_actions.iter().all(|action| action == "RESTRICT"),
        "no foreign key may cascade into issued certificates: {delete_actions:?}"
    );
    assert!(migration.contains("CREATE FUNCTION prevent_certificate_delete()"));
    assert!(migration.contains("CREATE FUNCTION enforce_certificate_snapshot_immutability()"));
}

#[test]
fn certificate_campaign_hard_delete_exists_only_in_the_guarded_forward_migration() {
    let original_path = manifest_dir().join("migrations/035_certificate_issuance.sql");
    let original = read_source(&original_path);
    assert!(original.contains("CREATE FUNCTION prevent_certificate_delete()"));
    assert!(original.contains("RAISE EXCEPTION 'issued certificates cannot be deleted'"));

    let purge_path = manifest_dir().join("migrations/039_certificate_campaign_purge.sql");
    let purge = read_source(&purge_path);
    for required in [
        "certificate_campaign_purge_guard_allows",
        "certificate_file_purge_guard_allows",
        "finalize_certificate_campaign_purge",
        "current_setting('schoolorbit.certificate_purge_campaign_id'",
        "DELETE FROM files AS file",
    ] {
        assert!(
            purge.contains(required),
            "guarded purge migration is missing `{required}`"
        );
    }

    let hard_delete =
        Regex::new(r"(?i)DELETE\s+FROM\s+files\b").expect("valid File Platform hard-delete regex");
    let migration_hard_deletes = list_files(manifest_dir().join("migrations"), |path| {
        path.extension().and_then(|extension| extension.to_str()) == Some("sql")
    })
    .into_iter()
    .filter(|path| hard_delete.is_match(&strip_comments(&read_source(path))))
    .map(|path| relative(&path))
    .collect::<Vec<_>>();
    assert_eq!(
        migration_hard_deletes,
        vec!["migrations/039_certificate_campaign_purge.sql"],
        "File Platform metadata may be hard-deleted only by migration 039's guarded finalizer"
    );

    for path in backend_rs_files() {
        let source = strip_comments(&read_source(&path));
        assert!(
            !hard_delete.is_match(&source),
            "application Rust must not hard-delete File Platform metadata: {}",
            relative(&path)
        );
    }
}

#[test]
fn certificate_runtime_keeps_handlers_thin_proofs_private_and_renders_ephemeral() {
    let handlers_path = manifest_dir().join("src/modules/certificates/handlers.rs");
    let handlers = strip_comments(&read_source(&handlers_path));
    let certificate_services = list_files(
        manifest_dir().join("src/modules/certificates/services"),
        |path| {
            path.extension().is_some_and(|extension| extension == "rs")
                && !is_rust_test_module(path)
        },
    )
    .into_iter()
    .map(read_source)
    .collect::<Vec<_>>()
    .join("\n");
    let certificate_migrations = list_files(manifest_dir().join("migrations"), |path| {
        path.extension().and_then(|extension| extension.to_str()) == Some("sql")
    })
    .into_iter()
    .map(read_source)
    .collect::<Vec<_>>()
    .join("\n");

    assert!(handlers.contains("permissions::registry::codes"));
    for forbidden in [
        "sqlx::query",
        ".fetch_",
        ".execute(",
        ".begin(",
        "QueryBuilder",
        "PgPool",
    ] {
        assert!(
            !handlers.contains(forbidden),
            "certificate handlers must delegate database work; found `{forbidden}` in {}",
            repo_relative(&handlers_path)
        );
    }

    for handler_name in [
        "verify_certificate_manually",
        "verify_certificate_by_qr",
        "create_public_certificate_render_manifest",
    ] {
        let handler =
            extract_braced_block(&handlers, &format!("pub async fn {handler_name}"), false);
        assert!(
            handler.contains("tenant_context(&state, &headers).await?"),
            "public handler `{handler_name}` must resolve only public tenant context"
        );
        assert!(
            !handler.contains("AuthenticatedSession")
                && !handler.contains("actor_tenant_context_from_session"),
            "public handler `{handler_name}` must not cross the authenticated actor boundary"
        );
    }

    let sql_max =
        Regex::new(r#"(?is)\"[^\"]*\bMAX\s*\("#).expect("valid certificate SQL MAX regex");
    assert!(!sql_max.is_match(&literal_only(
        r#"let label = "counter"; let value = left.max(right);"#
    )));
    assert!(sql_max.is_match(&literal_only(
        r#"let query = "SELECT MAX(sequence) FROM certificates";"#
    )));
    assert!(
        !sql_max.is_match(&literal_only(&certificate_services)),
        "certificate numbering must use locked counters, never SQL MAX(...)"
    );

    let sensitive_candidate_boundary = [
        extract_braced_block(
            &handlers,
            "pub async fn import_certificate_candidates",
            false,
        ),
        read_source(manifest_dir().join("src/modules/certificates/services/import_validation.rs")),
        read_source(manifest_dir().join("src/modules/certificates/services/candidate_service.rs")),
    ]
    .join("\n");
    let sensitive_candidate_structure = lexical_mask(&sensitive_candidate_boundary, false);
    let log_macro =
        Regex::new(r"(?:(?:tracing|log)::)?(?:trace|debug|info|warn|error)!|(?:e?println|dbg)!")
            .expect("valid certificate logging regex");
    assert!(
        !log_macro.is_match(&sensitive_candidate_structure.structural),
        "certificate import rows and candidate request bodies must never be logged"
    );

    let raw_permission = Regex::new(
        r#"\"certificate\.(?:read|create|update|delete|submit|issue|revoke|download)\.[^\"]+\""#,
    )
    .expect("valid raw certificate permission regex");
    let cfg_test_fixture = r#"
const BEFORE: &str = "certificate.read.school";
#[cfg(test)]
mod tests {
    const ALLOWED: &str = "certificate.read.own";
}
const AFTER: &str = "certificate.issue.school";
"#;
    let stripped_fixture = strip_cfg_test_modules(cfg_test_fixture);
    assert_eq!(raw_permission.find_iter(&stripped_fixture).count(), 2);
    assert!(!stripped_fixture.contains("certificate.read.own"));
    for file in backend_rs_files() {
        if relative(&file) == "src/permissions/registry_generated.rs" {
            continue;
        }
        let source = read_source(&file);
        let runtime_source = strip_cfg_test_modules(&source);
        assert!(
            !raw_permission.is_match(&runtime_source),
            "runtime certificate permissions must use generated constants: {}",
            relative(&file)
        );
    }

    let migration = read_source(manifest_dir().join("migrations/035_certificate_issuance.sql"));
    let certificate_table = migration
        .split("CREATE TABLE certificates (")
        .nth(1)
        .and_then(|tail| tail.split("\n);").next())
        .expect("certificate table definition");
    assert!(certificate_table.contains("qr_proof_encrypted TEXT NOT NULL"));
    assert!(certificate_table.contains("qr_proof_hash CHAR(64) NOT NULL"));
    let certificate_persistence_statement = Regex::new(
        r"(?is)(?:CREATE\s+TABLE\s+certificates\s*\(|ALTER\s+TABLE\s+certificates\b).*?;",
    )
    .expect("valid certificate persistence statement regex");
    let certificate_statements = certificate_persistence_statement
        .find_iter(&certificate_migrations)
        .map(|statement| statement.as_str().to_ascii_lowercase())
        .collect::<Vec<_>>();
    assert!(
        !certificate_statements.is_empty(),
        "certificate persistence statements must remain migration-owned"
    );
    for statement in &certificate_statements {
        for forbidden in [
            "proof_plaintext",
            "plaintext_proof",
            "generated_pdf",
            "rendered_pdf",
            "pdf_file_id",
            "output_file_id",
            "storage_key",
            "object_key",
            "download_url",
        ] {
            assert!(
                !statement.contains(forbidden),
                "certificates must not persist `{forbidden}`"
            );
        }
    }
    let plaintext_proof_column =
        Regex::new(r"(?im)(?:^\s*(?:qr_)?proof\s+|\bADD\s+COLUMN\s+(?:qr_)?proof\s+)")
            .expect("valid plaintext proof column regex");
    for statement in &certificate_statements {
        assert!(
            !plaintext_proof_column.is_match(statement),
            "certificate proofs may be persisted only as ciphertext plus a domain-separated hash"
        );
    }
    let template_history =
        Regex::new(r"(?i)CREATE\s+TABLE\s+certificate_template_(?:history|histories|versions?)\b")
            .expect("valid certificate template history regex");
    assert!(
        !template_history.is_match(&certificate_migrations),
        "certificate layouts intentionally update in place; no template history table is allowed"
    );

    let file_purposes = read_source(manifest_dir().join("src/modules/files/platform_types.rs"));
    for required in [
        "CertificateTemplateBackground",
        "CertificateTemplateImage",
        "SchoolFont",
    ] {
        assert!(file_purposes.contains(required));
    }
    for forbidden in [
        "CertificateTemplateFont",
        "CertificatePdf",
        "CertificateOutput",
        "certificate_pdf",
        "certificate_output",
        "certificate_generated",
    ] {
        assert!(
            !file_purposes.contains(forbidden),
            "generated certificate PDFs must remain browser-created and ephemeral"
        );
    }
}

#[test]
fn exam_invigilator_conflict_migration_drops_day_staff_unique_constraint() {
    let creation_migration = read_source(
        manifest_dir()
            .join("migrations")
            .join("019_academic_exam_schedule.sql"),
    );
    let conflict_migration = read_source(
        manifest_dir()
            .join("migrations")
            .join("020_academic_exam_invigilator_live_range_conflicts.sql"),
    );

    assert!(
        creation_migration.contains("UNIQUE (exam_day_id, staff_id)"),
        "migration 019 should remain immutable and document the original day/staff uniqueness"
    );
    assert!(
        creation_migration.contains("UNIQUE (day_room_assignment_id, staff_id)"),
        "room-assignment/staff uniqueness must remain part of the original schema"
    );
    assert!(conflict_migration.contains(
        "DROP CONSTRAINT IF EXISTS academic_exam_day_invigilators_exam_day_id_staff_id_key"
    ));
    assert!(conflict_migration
        .contains("CREATE INDEX IF NOT EXISTS idx_academic_exam_day_invigilators_exam_day_staff"));
    assert!(
        conflict_migration.contains("ON academic_exam_day_invigilators (exam_day_id, staff_id)")
    );
    assert!(
        !conflict_migration.contains("UNIQUE INDEX"),
        "staff/day lookup index must not be unique"
    );
}

#[test]
fn exam_day_order_migration_drops_sort_order_column() {
    let migration = read_source(
        manifest_dir()
            .join("migrations")
            .join("021_academic_exam_day_drop_sort_order.sql"),
    );

    assert!(migration
        .contains("DROP CONSTRAINT IF EXISTS academic_exam_days_exam_round_id_sort_order_key"));
    assert!(migration.contains("DROP COLUMN IF EXISTS sort_order"));
}

#[test]
fn exam_round_kind_migration_adds_midterm_final_contract() {
    let migration = read_source(
        manifest_dir()
            .join("migrations")
            .join("022_academic_exam_round_exam_kind.sql"),
    );

    assert!(
        migration.contains("ADD COLUMN exam_kind TEXT NOT NULL DEFAULT 'midterm'"),
        "exam rounds need a non-null default kind for existing tenants"
    );
    assert!(
        migration.contains("exam_kind IN ('midterm', 'final')"),
        "exam round kind must stay limited to the supported assessment categories"
    );
}

#[test]
fn academic_exam_days_use_date_ordering_without_sort_order_contract() {
    let model = read_source(
        manifest_dir()
            .join("src")
            .join("modules")
            .join("academic")
            .join("models")
            .join("exam_schedule.rs"),
    );
    let service_dir = manifest_dir().join("src/modules/academic/services");
    let service = [
        service_dir.join("exam_schedule_service.rs"),
        service_dir.join("exam_schedule_service/rounds_and_days.rs"),
        service_dir.join("exam_schedule_service/workspace.rs"),
    ]
    .into_iter()
    .map(read_source)
    .collect::<Vec<_>>()
    .join("\n");

    assert!(!model.contains("sort_order"));
    assert!(!service.contains("sort_order"));
    assert!(service.contains("ORDER BY exam_date ASC, start_time ASC, id ASC"));
    assert!(service.contains("ORDER BY day.exam_date,"));
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
    let backend_registry =
        read_source(manifest_dir().join("src/permissions/registry_generated.rs"));
    let frontend_registry = read_source(
        repo_root()
            .join("frontend-school")
            .join("src/lib/permissions/registry.generated.ts"),
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
fn academic_assessment_plans_are_offering_and_term_scoped() {
    let migration_path = manifest_dir()
        .join("migrations")
        .join("043_academic_consumer_cutover.sql");
    let migration = read_source(&migration_path);
    let service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/assessment_service.rs"),
    ));
    let models = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/models/assessment.rs"),
    ));

    for required_fragment in [
        "ADD COLUMN learning_offering_id UUID",
        "ADD COLUMN academic_year_id UUID",
        "FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)",
        "UNIQUE (learning_offering_id)",
    ] {
        assert!(
            migration.contains(required_fragment),
            "{} must contain `{required_fragment}`",
            repo_relative(&migration_path)
        );
    }

    assert!(
        service.contains("FROM learning_offerings offering")
            && service.contains(
                "LEFT JOIN course_assessment_plans plan ON plan.learning_offering_id = offering.id"
            )
            && service.contains("offering.academic_term_id = $1"),
        "assessment list must use offering identity within an explicit academic term"
    );
    assert!(
        !service.contains("classroom_course_id") && !service.contains("academic_semester_id"),
        "assessment service must not use legacy course or semester identity"
    );
    assert!(models.contains("pub offering_id: Uuid"));
    assert!(models.contains("pub academic_term_id: Uuid"));
    assert!(models.contains("pub academic_year_id: Uuid"));
}

#[test]
fn academic_assessment_supports_resource_and_assigned_group_scope() {
    let service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/assessment_service.rs"),
    ));
    let models = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/models/assessment.rs"),
    ));
    let backend_registry = strip_comments(&read_source(
        manifest_dir().join("src/permissions/registry_generated.rs"),
    ));

    assert!(
        backend_registry.contains("academic_assessment.read.organization_unit"),
        "assessment organization-unit read permission must remain registered"
    );
    assert!(
        service.contains("AcademicResourceListFilter")
            && service.contains("allowed_organization_unit_ids")
            && service.contains("offering.owning_organization_unit_id = ANY")
            && service.contains("learning_group_teachers"),
        "assessment list must combine resource ownership and assigned learning-group scope"
    );
    assert!(
        models.contains("pub learning_group_ids: Vec<Uuid>"),
        "assessment summaries must expose their canonical learning groups"
    );
}

#[test]
fn academic_assessment_teacher_scope_uses_learning_group_teachers() {
    let service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/assessment_service.rs"),
    ));

    assert!(
        service.contains("JOIN learning_group_teachers teacher")
            && service.contains("teacher.teacher_id = $3"),
        "assessment teacher filtering must use canonical learning-group teachers"
    );
    assert!(
        !service.contains("primary_instructor_id")
            && !service.contains("classroom_course_instructors"),
        "assessment teacher filtering must not retain legacy primary-instructor identity"
    );
}

#[test]
fn academic_assessment_save_persists_saved_status() {
    let migration_path = manifest_dir()
        .join("migrations")
        .join("016_academic_assessment_saved_status.sql");
    let migration = read_source(&migration_path);
    let service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/assessment_service.rs"),
    ));

    assert!(
        migration.contains("'saved'"),
        "{} must add the saved assessment status to the database constraint",
        repo_relative(&migration_path)
    );
    assert!(
        service.contains("SET status = 'saved', submitted_at = NULL, submitted_by = NULL"),
        "saving an assessment plan should persist saved status"
    );
    assert!(
        !service.contains("CASE WHEN status = 'submitted' THEN status ELSE 'draft' END"),
        "assessment save must not keep submitted status when a plan is edited and saved"
    );
}

#[test]
fn academic_assessment_list_has_stable_offering_order() {
    let service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/assessment_service.rs"),
    ));

    assert!(
        service.contains("ORDER BY offering.code_snapshot, offering.id"),
        "assessment list must have deterministic canonical offering order"
    );
    assert!(
        !service.contains("sort_actor_id") && !service.contains("classroom_room_number"),
        "assessment ordering must not infer legacy classroom context"
    );
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
fn staff_dashboard_endpoint_is_staff_scoped_and_aggregate_only() {
    let routes = strip_comments(&read_source(manifest_dir().join("src/app.rs")));
    let staff_handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/staff/handlers/staff.rs"),
    ));
    let dashboard_service = strip_comments(&read_source(
        manifest_dir().join("src/modules/staff/services/dashboard_service.rs"),
    ));

    assert!(routes.contains("\"/api/staff/dashboard\""));
    assert!(routes.contains("get(modules::staff::handlers::staff::get_staff_dashboard)"));
    assert!(
        routes.find("\"/api/staff/dashboard\"") < routes.find("\"/api/staff/{id}\""),
        "dashboard route must be registered before /api/staff/{{id}}"
    );

    assert!(staff_handler.contains("dashboard_service::ensure_active_staff_user"));
    assert!(staff_handler.contains("Query(query): Query<StaffDashboardQuery>"));
    assert!(staff_handler
        .contains("dashboard_service::get_staff_dashboard(&pool, query.academic_year_id)"));
    assert!(staff_handler.contains("ApiResponse::ok(data)"));
    assert!(!staff_handler.contains("actor.require_permission(codes::STAFF_READ_ALL)"));

    assert!(dashboard_service.contains("COUNT(*)"));
    assert!(dashboard_service.contains("user_type = 'staff'"));
    assert!(dashboard_service.contains("user_type = 'student'"));
    assert!(dashboard_service.contains("FROM homerooms"));
    assert!(dashboard_service.contains("academic_year_id = $1"));
    assert!(!dashboard_service.contains("class_rooms"));

    for forbidden in [
        "national_id",
        "phone",
        "email",
        "first_name",
        "last_name",
        "staff_service::list_staff",
        "student_service::list_students",
    ] {
        assert!(
            !dashboard_service.contains(forbidden),
            "staff dashboard aggregate service must not expose or select `{forbidden}`"
        );
    }
}

#[test]
fn daily_teaching_overview_endpoint_is_read_only_and_pii_safe() {
    let routes = strip_comments(&read_source(manifest_dir().join("src/modules/academic.rs")));
    let handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/handlers/timetable.rs"),
    ));
    let service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/daily_teaching_service.rs"),
    ));
    let registry = read_source(manifest_dir().join("src/permissions/registry_generated.rs"));
    let daily_handler =
        extract_braced_block(&handler, "pub async fn daily_teaching_overview", false);

    assert!(routes.contains("\"/timetable/daily-teaching\""));
    assert!(routes.contains("get(handlers::timetable::daily_teaching_overview)"));
    assert!(
        routes.find("\"/timetable/daily-teaching\"") < routes.find("\"/timetable/{id}\""),
        "daily teaching route must be registered before /timetable/{{id}}"
    );

    assert!(daily_handler.contains("actor_tenant_context_from_session(&state, &session).await?"));
    assert!(daily_handler.contains("LEARNING_OFFERING_READ_SCHOOL"));
    assert!(!daily_handler.contains("LEARNING_OFFERING_MANAGE_SCHOOL"));
    assert!(daily_handler.contains("daily_teaching_service::get_daily_teaching_overview"));
    assert!(daily_handler.contains("ApiResponse::ok(overview)"));

    assert!(service.contains("#[serde(rename_all = \"camelCase\")]"));
    assert!(service.contains("DailyTeachingOverview"));
    assert!(service.contains("timetable_entry_instructors"));
    assert!(service.contains("academic_terms"));
    assert!(service.contains("bell_schedule_periods"));
    assert!(service.contains("learning_group_teachers"));
    assert!(service.contains("course_offering_details"));
    assert!(service.contains("activity_offering_details"));
    assert!(service.contains("subject_versions"));
    assert!(service.contains("activity_versions"));
    assert!(registry.contains("learning_offering.read.school"));

    for legacy in ["academic_semesters", "classroom_courses", "activity_slots"] {
        assert!(
            !service.contains(legacy),
            "daily teaching service must not use legacy `{legacy}` identity"
        );
    }

    for forbidden in [
        "national_id",
        "phone",
        "email",
        "username",
        "address",
        "student_",
    ] {
        assert!(
            !service.contains(forbidden),
            "daily teaching service must not expose or select `{forbidden}`"
        );
    }
}

#[test]
fn academic_core_handlers_are_thin_authorized_and_signal_only_after_mutation() {
    let handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/core/handlers.rs"),
    ));
    let context_service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/core/services/context.rs"),
    ));

    assert!(handlers.contains("actor_tenant_context_from_session(&state, &session).await?"));
    assert!(!handlers.contains("sqlx::query"));
    assert!(!handlers.contains(".fetch_"));
    assert!(!handlers.contains(".execute("));
    assert!(!handlers.contains(".begin("));

    let context_handler =
        extract_braced_block(&handlers, "pub async fn list_context_options", false);
    assert!(context_handler.contains("ACADEMIC_CONTEXT_READ_SCHOOL"));
    assert!(context_handler.contains("context::list_options(&pool).await?"));
    assert!(!context_handler.contains("signal_core_changed"));
    for forbidden in ["UPDATE ", "INSERT ", "DELETE "] {
        assert!(
            !context_service.contains(forbidden),
            "context options must remain read-only: {forbidden}"
        );
    }

    for (handler_name, permission) in [
        ("create_year", "ACADEMIC_YEAR_MANAGE_SCHOOL"),
        ("update_year", "ACADEMIC_YEAR_MANAGE_SCHOOL"),
        ("create_term", "ACADEMIC_TERM_MANAGE_SCHOOL"),
        ("update_term", "ACADEMIC_TERM_MANAGE_SCHOOL"),
        ("delete_term", "ACADEMIC_TERM_MANAGE_SCHOOL"),
        ("create_bell_schedule", "ACADEMIC_TERM_MANAGE_SCHOOL"),
        ("update_bell_schedule", "ACADEMIC_TERM_MANAGE_SCHOOL"),
        (
            "replace_bell_schedule_periods",
            "ACADEMIC_TERM_MANAGE_SCHOOL",
        ),
        ("replace_grade_progressions", "ACADEMIC_YEAR_MANAGE_SCHOOL"),
        ("create_homeroom", "HOMEROOM_MANAGE_SCHOOL"),
        ("update_homeroom", "HOMEROOM_MANAGE_SCHOOL"),
        ("replace_homeroom_advisors", "HOMEROOM_MANAGE_SCHOOL"),
        ("create_student_year", "STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL"),
        ("update_student_year", "STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL"),
        ("create_placement", "STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL"),
        ("transfer_placement", "STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL"),
    ] {
        let handler =
            extract_braced_block(&handlers, &format!("pub async fn {handler_name}"), false);
        assert!(handler.contains("actor_tenant_context_from_session(&state, &session).await?"));
        assert!(
            handler.contains(&format!("actor.require_permission(codes::{permission})?")),
            "{handler_name} must require {permission}"
        );
        assert!(
            handler.contains("signal_core_changed"),
            "{handler_name} must emit a bounded invalidation after success"
        );
    }
}

#[test]
fn academic_core_curriculum_handlers_enforce_resource_policy_contract() {
    let handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/core/handlers.rs"),
    ));
    let cases = [
        ("get_curriculum", "CurriculumAction::Read"),
        ("update_curriculum", "CurriculumAction::Manage"),
        ("list_curriculum_versions", "CurriculumAction::Read"),
        ("create_curriculum_version", "CurriculumAction::Manage"),
        ("get_curriculum_version", "CurriculumAction::Read"),
        ("update_curriculum_version", "CurriculumAction::Manage"),
        ("publish_curriculum_version", "CurriculumAction::Manage"),
        ("list_study_programs", "CurriculumAction::Read"),
        ("create_study_program", "CurriculumAction::Manage"),
        ("get_study_program", "CurriculumAction::Read"),
        ("update_study_program", "CurriculumAction::Manage"),
        ("list_program_requirements", "CurriculumAction::Read"),
        ("replace_program_requirements", "CurriculumAction::Manage"),
    ];

    for (handler_name, action) in cases {
        let handler =
            extract_braced_block(&handlers, &format!("pub async fn {handler_name}"), false);
        assert!(handler.contains("actor_tenant_context_from_session(&state, &session).await?"));
        assert!(
            handler.contains("require_academic_curriculum_access") && handler.contains(action),
            "{handler_name} must use the curriculum resource policy with {action}"
        );
    }

    let list_handler = extract_braced_block(&handlers, "pub async fn list_curricula", false);
    let create_handler = extract_braced_block(&handlers, "pub async fn create_curriculum", false);
    assert!(list_handler.contains("require_academic_curriculum_list_access"));
    assert!(list_handler.contains("CurriculumAction::Read"));
    assert!(create_handler.contains("require_academic_curriculum_list_access"));
    assert!(create_handler.contains("CurriculumAction::Manage"));
    assert!(create_handler.contains("owner_allowed"));
}

#[test]
fn academic_core_catalog_handlers_enforce_resource_policy_contract() {
    let handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/core/handlers.rs"),
    ));
    let cases = [
        ("get_catalog_subject", "CatalogAction::Read"),
        ("update_catalog_subject", "CatalogAction::Manage"),
        ("list_subject_versions", "CatalogAction::Read"),
        ("create_subject_version", "CatalogAction::Manage"),
        ("get_subject_version", "CatalogAction::Read"),
        ("update_subject_version", "CatalogAction::Manage"),
        ("publish_subject_version", "CatalogAction::Manage"),
        ("get_catalog_activity", "CatalogAction::Read"),
        ("update_catalog_activity", "CatalogAction::Manage"),
        ("list_activity_versions", "CatalogAction::Read"),
        ("create_activity_version", "CatalogAction::Manage"),
        ("get_activity_version", "CatalogAction::Read"),
        ("update_activity_version", "CatalogAction::Manage"),
        ("publish_activity_version", "CatalogAction::Manage"),
    ];

    for (handler_name, action) in cases {
        let handler =
            extract_braced_block(&handlers, &format!("pub async fn {handler_name}("), false);
        assert!(handler.contains("actor_tenant_context_from_session(&state, &session).await?"));
        assert!(
            handler.contains("require_academic_catalog_access") && handler.contains(action),
            "{handler_name} must use the catalog resource policy with {action}"
        );
    }

    let list_subjects =
        extract_braced_block(&handlers, "pub async fn list_catalog_subjects", false);
    let create_activity =
        extract_braced_block(&handlers, "pub async fn create_catalog_activity", false);
    assert!(list_subjects.contains("require_academic_catalog_list_access"));
    assert!(list_subjects.contains("CatalogAction::Read"));
    assert!(create_activity.contains("require_academic_catalog_list_access"));
    assert!(create_activity.contains("CatalogAction::Manage"));
    assert!(create_activity.contains("owner_allowed"));
}

#[test]
fn academic_exam_schedule_routes_are_registered_and_authorized() {
    fn handler_body<'a>(source: &'a str, handler_name: &str) -> &'a str {
        let marker = format!("pub async fn {handler_name}");
        let start = source
            .find(&marker)
            .unwrap_or_else(|| panic!("missing handler {handler_name}"));
        let after_start = &source[start..];
        let end = after_start[marker.len()..]
            .find("pub async fn ")
            .map(|offset| marker.len() + offset)
            .unwrap_or(after_start.len());

        &after_start[..end]
    }

    fn app_route_snippet<'a>(source: &'a str, route: &str) -> &'a str {
        let start = source
            .find(route)
            .unwrap_or_else(|| panic!("missing app route {route}"));
        source[start..]
            .split("\n        .route(")
            .next()
            .unwrap_or_else(|| panic!("missing app route snippet {route}"))
    }

    fn assert_handler_permission(source: &str, handler_name: &str, permission: &str) {
        let body = handler_body(source, handler_name);
        let expected_check = format!("actor.require_permission(codes::{permission})");
        assert!(
            body.contains(&expected_check),
            "{handler_name} must require {permission}"
        );

        for other_permission in [
            "ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL",
            "ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL",
            "ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL",
        ] {
            if other_permission != permission {
                assert!(
                    !body.contains(other_permission),
                    "{handler_name} must not require {other_permission}"
                );
            }
        }
    }

    let academic_handler_root = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/handlers.rs"),
    ));
    let academic_routes =
        strip_comments(&read_source(manifest_dir().join("src/modules/academic.rs")));
    let app_routes = strip_comments(&read_source(manifest_dir().join("src/app.rs")));
    let parent_handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/parents/handlers.rs"),
    ));
    let parent_services = strip_comments(&read_source(
        manifest_dir().join("src/modules/parents/services.rs"),
    ));
    let exam_service = strip_comments(&read_source(
        manifest_dir()
            .join("src/modules/academic/services/exam_schedule_service/published_views.rs"),
    ));
    let exam_handler_path = manifest_dir().join("src/modules/academic/handlers/exam_schedule.rs");

    assert!(
        academic_handler_root.contains("pub mod exam_schedule;"),
        "academic handlers root must export the exam_schedule handler module"
    );
    assert!(
        exam_handler_path.exists(),
        "academic exam schedule handlers must live in src/modules/academic/handlers/exam_schedule.rs"
    );

    let exam_handler = strip_comments(&read_source(exam_handler_path));

    for route in [
        "\"/exam-schedules\"",
        "\"/exam-schedules/{round_id}\"",
        "\"/exam-schedules/{round_id}/import-items\"",
        "\"/exam-schedules/{round_id}/clear-mismatched-items\"",
        "\"/exam-schedules/{round_id}/days\"",
        "\"/exam-schedules/days/{exam_day_id}\"",
        "\"/exam-schedules/days/{exam_day_id}/room-assignments\"",
        "\"/exam-schedules/room-assignments/{assignment_id}/seats\"",
        "\"/exam-schedules/sessions\"",
        "\"/exam-schedules/sessions/{session_id}\"",
        "\"/exam-schedules/{round_id}/invigilators\"",
        "\"/exam-schedules/{round_id}/invigilator-staff-options\"",
        "\"/exam-schedules/room-assignments/{assignment_id}/invigilators\"",
        "\"/exam-schedules/{round_id}/publish\"",
    ] {
        assert!(
            academic_routes.contains(route),
            "missing academic route {route}"
        );
    }

    for handler_ref in [
        "get(handlers::exam_schedule::list_rounds)",
        "post(handlers::exam_schedule::create_round)",
        "get(handlers::exam_schedule::get_workspace)",
        ".patch(handlers::exam_schedule::update_round)",
        "post(handlers::exam_schedule::import_items)",
        "post(handlers::exam_schedule::clear_mismatched_items)",
        "post(handlers::exam_schedule::upsert_day)",
        "patch(handlers::exam_schedule::update_day)",
        "delete(handlers::exam_schedule::delete_day)",
        "get(handlers::exam_schedule::list_day_room_assignments)",
        "post(handlers::exam_schedule::upsert_day_room_assignment)",
        "post(handlers::exam_schedule::generate_seats)",
        "post(handlers::exam_schedule::place_session)",
        "delete(handlers::exam_schedule::delete_session)",
        "get(handlers::exam_schedule::get_invigilator_workspace)",
        "get(handlers::exam_schedule::get_invigilator_staff_options)",
        "put(handlers::exam_schedule::update_assignment_invigilators)",
        "post(handlers::exam_schedule::publish_round)",
    ] {
        assert!(
            academic_routes.contains(handler_ref),
            "missing academic handler registration {handler_ref}"
        );
    }

    let self_route = app_route_snippet(&app_routes, "\"/api/me/exam-schedules\"");
    assert!(self_route.contains("exam_schedule::list_my_exam_schedule"));

    let staff_route = app_route_snippet(&app_routes, "\"/api/staff/exam-schedules\"");
    assert!(staff_route.contains("exam_schedule::list_staff_exam_schedule"));

    let parent_route = app_route_snippet(
        &app_routes,
        "\"/api/parent/students/{student_id}/exam-schedules\"",
    );
    assert!(parent_route.contains("parents::handlers::get_child_exam_schedule"));
    assert_eq!(
        app_routes
            .matches("from_fn_with_state(runtime, session_middleware)")
            .count(),
        1
    );

    for read_handler in [
        "list_rounds",
        "get_workspace",
        "list_day_room_assignments",
        "get_invigilator_workspace",
    ] {
        assert_handler_permission(
            &exam_handler,
            read_handler,
            "ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL",
        );
    }

    for manage_handler in [
        "create_round",
        "update_round",
        "import_items",
        "clear_mismatched_items",
        "upsert_day",
        "delete_day",
        "upsert_day_room_assignment",
        "generate_seats",
        "place_session",
        "delete_session",
        "get_invigilator_staff_options",
        "update_assignment_invigilators",
    ] {
        assert_handler_permission(
            &exam_handler,
            manage_handler,
            "ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL",
        );
    }

    assert_handler_permission(
        &exam_handler,
        "publish_round",
        "ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL",
    );

    let self_handler = handler_body(&exam_handler, "list_my_exam_schedule");
    assert!(self_handler.contains("list_my_published_exam_schedule"));
    assert!(
        !self_handler.contains("ACADEMIC_EXAM_SCHEDULE_"),
        "self exam schedule route must not require academic permissions"
    );

    let staff_handler = handler_body(&exam_handler, "list_staff_exam_schedule");
    assert!(staff_handler.contains("list_staff_published_exam_schedule"));
    assert!(
        !staff_handler.contains("ACADEMIC_EXAM_SCHEDULE_"),
        "staff published exam schedule route must not require academic permissions"
    );
    assert!(exam_service.contains("ensure_active_staff_user_for_exam_schedule"));
    assert!(exam_service.contains("list_published_exam_schedule_for_staff"));

    assert!(parent_handlers.contains("pub async fn get_child_exam_schedule"));
    assert!(parent_services.contains("pub async fn get_child_exam_schedule"));
    assert!(parent_services.contains("list_child_published_exam_schedule"));
}

#[test]
fn academic_curriculum_access_uses_resource_policy_tree_resolution() {
    let curriculum_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/academic_curriculum_access_policy.rs"),
    ));

    assert!(curriculum_policy.contains("resolve_academic_resource_list_filter"));
    assert!(curriculum_policy.contains("academic_resource_access_for"));
    assert!(!curriculum_policy.contains("WITH RECURSIVE"));
    assert!(!curriculum_policy.contains("JOIN organization_tree parent_tree"));
}

#[test]
fn academic_curriculum_permission_decisions_live_in_policy_layer() {
    let policies_root = read_source(manifest_dir().join("src/policies.rs"));
    let catalog_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/academic_catalog_access_policy.rs"),
    ));
    let curriculum_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/academic_curriculum_access_policy.rs"),
    ));
    let core_handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/core/handlers.rs"),
    ));
    let catalog_service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/core/services/catalog.rs"),
    ));

    assert!(policies_root.contains("pub mod academic_catalog_access_policy;"));
    assert!(policies_root.contains("pub mod academic_curriculum_access_policy;"));
    assert!(catalog_policy.contains("academic_catalog_list_access"));
    assert!(catalog_policy.contains("academic_catalog_access"));
    assert!(curriculum_policy.contains("academic_curriculum_list_access"));
    assert!(curriculum_policy.contains("academic_curriculum_access"));
    assert!(
        core_handler.contains("academic_catalog_access_policy::require_academic_catalog_access")
    );
    assert!(core_handler
        .contains("academic_curriculum_access_policy::require_academic_curriculum_access"));
    assert!(!catalog_service.contains("actor.has_permission("));
    assert!(!catalog_service.contains("ResourceAccessPermissions"));
}

#[test]
fn academic_core_resource_policies_preserve_independent_scopes() {
    let policies_root = read_source(manifest_dir().join("src/policies.rs"));
    let shared_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/resource_access_policy.rs"),
    ));
    let catalog_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/academic_catalog_access_policy.rs"),
    ));
    let curriculum_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/academic_curriculum_access_policy.rs"),
    ));
    let offering_policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/learning_offering_access_policy.rs"),
    ));

    for module in [
        "pub mod academic_catalog_access_policy;",
        "pub mod academic_curriculum_access_policy;",
        "pub mod learning_offering_access_policy;",
    ] {
        assert!(
            policies_root.contains(module),
            "missing policy module: {module}"
        );
    }

    for policy in [&catalog_policy, &curriculum_policy, &offering_policy] {
        assert!(policy.contains("resolve_academic_resource_list_filter"));
        assert!(policy.contains("academic_resource_access_for"));
    }

    assert!(shared_policy.contains("accessible_exact_units_for_permission"));
    assert!(shared_policy.contains("accessible_tree_units_for_permission"));
    assert!(shared_policy.contains("p.is_active = true"));
    assert!(offering_policy.contains("JOIN learning_group_teachers teacher"));
    assert!(offering_policy.contains("teacher.teacher_id = $2"));
}

#[test]
fn academic_subject_catalog_uses_versioned_stable_identity() {
    let catalog = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/core/services/catalog.rs"),
    ));

    for required in [
        "pub async fn list_subjects",
        "pub async fn create_subject",
        "pub async fn update_subject",
        "pub async fn list_subject_versions",
        "pub async fn create_subject_version",
        "pub async fn publish_subject_version",
        "subject_default_instructors",
    ] {
        assert!(
            catalog.contains(required),
            "canonical subject catalog must contain `{required}`"
        );
    }
    assert!(!catalog.contains("subjects.default_instructor_id"));
}

#[test]
fn academic_subject_default_instructors_live_in_catalog_junction() {
    let catalog = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/core/services/catalog.rs"),
    ));
    let drop_migration = read_source(
        manifest_dir()
            .join("migrations")
            .join("017_drop_subject_default_instructor_id.sql"),
    );

    assert!(catalog.contains("list_subject_default_teachers"));
    assert!(catalog.contains("subject_default_instructors"));
    assert!(!catalog.contains("subjects.default_instructor_id"));

    assert!(drop_migration.contains("DROP TRIGGER IF EXISTS subject_sync_junction ON subjects"));
    assert!(drop_migration.contains("DROP FUNCTION IF EXISTS refresh_subject_default_instructor"));
    assert!(drop_migration.contains("DROP FUNCTION IF EXISTS trg_subject_sync_junction"));
    assert!(drop_migration.contains("DROP FUNCTION IF EXISTS trg_sdi_sync_primary"));
    assert!(drop_migration.contains("INSERT INTO subject_default_instructors"));
    assert!(drop_migration.contains("WHERE default_instructor_id IS NOT NULL"));
    assert!(drop_migration.contains("ON CONFLICT (subject_id, instructor_id)"));
    assert!(drop_migration.contains("DROP COLUMN IF EXISTS default_instructor_id"));
    assert!(drop_migration.contains("CREATE TRIGGER sdi_enforce_single_primary"));
    assert!(drop_migration.contains("trg_sdi_enforce_single_primary"));
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
    let backend_registry =
        read_source(manifest_dir().join("src/permissions/registry_generated.rs"));
    let frontend_registry = read_source(
        repo_root()
            .join("frontend-school")
            .join("src/lib/permissions/registry.generated.ts"),
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
fn activity_offerings_use_the_shared_delivery_resource_policy() {
    let policies_root = read_source(manifest_dir().join("src/policies.rs"));
    let handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery/handlers.rs"),
    ));
    let offerings = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery/services/offerings.rs"),
    ));

    assert!(policies_root.contains("pub mod learning_offering_access_policy;"));
    assert!(
        handlers.contains("learning_offering_access_policy::require_learning_offering_list_access")
    );
    assert!(handlers.contains("require_learning_offering_access"));
    assert!(offerings.contains("activity_offering_details"));
    assert!(offerings.contains("activity_versions"));
    assert!(!handlers.contains("activity_access_policy"));
    assert!(!offerings.contains("activity_slots"));
}

#[test]
fn activity_delivery_routes_use_offerings_and_learning_groups() {
    let routes = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery.rs"),
    ));

    assert!(routes.contains("\"/offerings\""));
    assert!(routes.contains("\"/offerings/{id}/groups\""));
    assert!(routes.contains("\"/learning-groups/{id}\""));
    assert!(routes.contains("\"/learning-groups/{id}/roster\""));
    assert!(!routes.contains("activity-slots"));
    assert!(!routes.contains("/activities"));
}

#[test]
fn activity_delivery_requests_use_strict_typed_snapshots() {
    let models = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery/models.rs"),
    ));

    for required in [
        "pub struct CreateActivityOfferingRequest",
        "pub struct ActivityAttendanceRequirement",
        "pub struct ActivityPassCriteria",
        "pub struct ActivityOfferingSnapshot",
        "ActivityRegistrationType",
        "ActivitySchedulingMode",
    ] {
        assert!(models.contains(required));
    }
    assert!(models.contains("deny_unknown_fields"));
    assert!(!models.contains("activity_slot_id"));
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

        let family_tests_path = file
            .parent()
            .expect("service source should have a parent directory")
            .join("tests.rs");
        let has_focused_family_tests =
            family_tests_path.is_file() && read_source(&family_tests_path).contains("#[test]");
        let characterization_tests_path = if file
            .parent()
            .and_then(|parent| parent.file_name())
            .is_some_and(|name| name == "services")
        {
            file.parent()
                .and_then(Path::parent)
                .map(|parent| parent.join("services_tests.rs"))
        } else {
            file.parent()
                .and_then(|parent| {
                    parent.file_name().map(|name| {
                        parent
                            .parent()
                            .map(|root| root.join(format!("{}_tests.rs", name.to_string_lossy())))
                    })
                })
                .flatten()
        };
        let has_characterization_tests = characterization_tests_path.is_some_and(|path| {
            path.is_file() && {
                let tests = read_source(path);
                tests.contains("#[test]") || tests.contains("#[tokio::test]")
            }
        });

        if !source.contains("#[cfg(test)]")
            && !has_focused_family_tests
            && !has_characterization_tests
        {
            violations.push(format!(
                "{}: service logic files must include focused #[cfg(test)] coverage in-file or in a colocated tests.rs",
                relative(&file)
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

fn mask_export_openapi_cli_block(source: &str) -> String {
    let marker = r#"if command_args.first().map(String::as_str) == Some("export-openapi")"#;
    let lexical = lexical_mask(source, false);
    let Some(start) = source.match_indices(marker).find_map(|(start, _)| {
        (!lexical.comments[start] && !lexical.literals[start]).then_some(start)
    }) else {
        return source.to_owned();
    };
    let opening = lexical.structural[start..]
        .find('{')
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("missing opening brace after {marker}"));
    let closing = balanced_delimiter_end(&lexical.structural, opening, b'{', b'}')
        .unwrap_or_else(|| panic!("unterminated {marker} block"));
    let mut masked = source.as_bytes().to_vec();

    for byte in &mut masked[start..=closing] {
        if *byte != b'\n' {
            *byte = b' ';
        }
    }

    String::from_utf8(masked).expect("masked Rust source remains UTF-8")
}

fn structured_logging_violations(file_name: &str, source: &str) -> Vec<String> {
    if file_name.starts_with("src/bin/") {
        return Vec::new();
    }

    let source = if file_name == "src/main.rs" {
        mask_export_openapi_cli_block(source)
    } else {
        source.to_owned()
    };
    let structural = lexical_mask(&source, false).structural;
    let mut violations = Vec::new();

    for macro_name in ["println", "eprintln"] {
        let invocation = Regex::new(&format!(r"\b{}\s*!\s*\(", regex::escape(macro_name)))
            .expect("plain output macro pattern should compile");
        if invocation.is_match(&structural) {
            violations.push(format!(
                "{file_name}: use tracing macros instead of {macro_name}! in runtime code"
            ));
        }
    }

    violations
}

#[test]
fn backend_runtime_uses_structured_logging_instead_of_plain_stdout_stderr() {
    let mut violations = Vec::new();

    for file in backend_rs_files() {
        let file_name = relative(&file);
        violations.extend(structured_logging_violations(
            &file_name,
            &read_source(&file),
        ));
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
            "src/modules/admission/models/rounds.rs",
            "scoring_subject_ids: serde_json::Value",
        ),
        (
            "src/modules/consent/models.rs",
            "data_categories: serde_json::Value",
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
fn permission_registry_wraps_generated_contract() {
    let wrapper = read_source(manifest_dir().join("src/permissions/registry.rs"));

    assert!(wrapper.contains("include!(\"registry_generated.rs\")"));
    assert!(!wrapper.contains("pub const STAFF_READ_ALL"));
    assert!(!wrapper.contains("pub const ALL_PERMISSIONS"));
}

#[test]
fn permission_registry_codes_match_declared_module_action_scope() {
    let registry = read_source(manifest_dir().join("src/permissions/registry_generated.rs"));
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
    let registry = read_source(manifest_dir().join("src/permissions/registry_generated.rs"));
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
        "download",
        "enroll",
        "evaluate",
        "execute",
        "issue",
        "manage",
        "manage_members",
        "publish",
        "read",
        "remove",
        "request",
        "revoke",
        "scores",
        "submit",
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
                "{}: use load_actor_context/load_actor_context_for_session and actor.require_* helpers",
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

fn authorization_handlers_from_router(router: &str) -> Vec<String> {
    let handler_pattern = Regex::new(
        r"\bmodules::(?P<handler>(?:auth::(?:handlers|session_handlers)|staff::handlers::(?:roles|permissions|user_roles|organization_permissions|organization_delegations|organization_members))::[A-Za-z_][A-Za-z0-9_]*)\b",
    )
    .expect("valid authorization handler regex");
    let router = strip_comments(router);
    let mut handlers = handler_pattern
        .captures_iter(&router)
        .map(|captures| {
            format!(
                "crate::modules::{}",
                captures.name("handler").expect("handler capture").as_str()
            )
        })
        .collect::<Vec<_>>();
    handlers.sort();
    handlers.dedup();
    handlers
}

fn openapi_path_registry(contract: &str) -> &str {
    let paths_start = contract
        .find("paths(")
        .expect("OpenAPI contract must declare paths");
    let components_offset = contract[paths_start..]
        .find("components(schemas(")
        .expect("OpenAPI paths must precede component schemas");
    &contract[paths_start..paths_start + components_offset]
}

fn authorization_handlers_missing_from_contract(router: &str, contract: &str) -> Vec<String> {
    let path_registry = openapi_path_registry(contract);

    authorization_handlers_from_router(router)
        .into_iter()
        .filter(|handler| !path_registry.contains(handler))
        .collect()
}

fn direct_route_path_before_handler(source: &str, handler_offset: usize) -> Option<&str> {
    let route_start = source[..handler_offset].rfind(".route(")?;
    let route_prefix = &source[route_start..handler_offset];
    let quote_start = route_prefix.find('"')? + 1;
    let path_tail = &route_prefix[quote_start..];
    let quote_end = path_tail.find('"')?;
    Some(&path_tail[..quote_end])
}

fn is_read_oriented_direct_path(path: &str) -> bool {
    path == "/api/menu/user"
        || path.starts_with("/api/admin/features")
        || path.starts_with("/api/admin/menu/")
        || path.starts_with("/api/lookup/")
        || path == "/api/staff"
        || path.starts_with("/api/staff/")
        || path == "/api/student/profile"
        || path.starts_with("/api/parent/")
        || path.starts_with("/api/me/")
        || path == "/api/public/calendar/events"
        || path.starts_with("/api/school/")
        || (path.starts_with("/api/notifications") && path != "/api/notifications/stream")
}

fn read_oriented_handlers_from_routers(main_router: &str, calendar_router: &str) -> Vec<String> {
    let direct_handler_pattern =
        Regex::new(r"\bget\(\s*modules::(?P<handler>[A-Za-z_][A-Za-z0-9_:]*)\s*\)")
            .expect("valid direct GET handler regex");
    let nested_calendar_pattern =
        Regex::new(r"\bget\(\s*handlers::(?P<handler>[A-Za-z_][A-Za-z0-9_]*)\s*\)")
            .expect("valid nested calendar GET handler regex");
    let main_router = strip_comments(main_router);
    let calendar_router = strip_comments(calendar_router);

    let mut handlers = direct_handler_pattern
        .captures_iter(&main_router)
        .filter_map(|captures| {
            let matched = captures.get(0).expect("direct handler match");
            let path = direct_route_path_before_handler(&main_router, matched.start())?;
            if !is_read_oriented_direct_path(path) {
                return None;
            }
            Some(format!(
                "crate::modules::{}",
                captures.name("handler").expect("handler capture").as_str()
            ))
        })
        .chain(
            nested_calendar_pattern
                .captures_iter(&calendar_router)
                .map(|captures| {
                    format!(
                        "crate::modules::calendar::handlers::{}",
                        captures.name("handler").expect("handler capture").as_str()
                    )
                }),
        )
        .collect::<Vec<_>>();
    handlers.sort();
    handlers.dedup();
    handlers
}

fn read_oriented_handlers_missing_from_contract(
    main_router: &str,
    calendar_router: &str,
    contract: &str,
) -> Vec<String> {
    let path_registry = openapi_path_registry(contract);
    read_oriented_handlers_from_routers(main_router, calendar_router)
        .into_iter()
        .filter(|handler| !path_registry.contains(handler))
        .collect()
}

#[test]
fn authorization_handlers_are_registered_in_the_openapi_document() {
    let router = read_source(manifest_dir().join("src/app.rs"));
    let contract = read_source(manifest_dir().join("src/api_contract.rs"));

    assert!(
        authorization_handlers_from_router(&router).len() >= 32,
        "authorization router parser must find the current phase inventory"
    );

    let handlers = authorization_handlers_from_router(&router);
    for handler in [
        "crate::modules::staff::handlers::roles::deactivate_role",
        "crate::modules::staff::handlers::roles::deactivate_organization_unit",
    ] {
        assert!(
            handlers.iter().any(|registered| registered == handler),
            "missing runtime deactivation handler: {handler}"
        );
    }

    assert_eq!(
        authorization_handlers_missing_from_contract(&router, &contract),
        Vec::<String>::new()
    );
}

#[test]
fn authorization_openapi_guard_detects_a_new_router_handler() {
    let router = r#"
        .route("/api/auth/login", post(modules::auth::handlers::login))
        .route("/api/auth/refresh", post(modules::auth::handlers::refresh))
    "#;
    let contract = r#"
        #[openapi(
            paths(crate::modules::auth::handlers::login),
            components(schemas(ApiErrorResponse))
        )]
    "#;

    assert_eq!(
        authorization_handlers_missing_from_contract(router, contract),
        vec!["crate::modules::auth::handlers::refresh".to_string()]
    );
}

#[test]
fn read_oriented_handlers_are_registered_in_the_openapi_document() {
    let main_router = read_source(manifest_dir().join("src/app.rs"));
    let calendar_router = read_source(manifest_dir().join("src/modules/calendar.rs"));
    let contract = read_source(manifest_dir().join("src/api_contract.rs"));

    assert_eq!(
        read_oriented_handlers_from_routers(&main_router, &calendar_router).len(),
        43,
        "read-oriented router inventory must stay aligned with the 43-operation rollout"
    );
    assert_eq!(
        read_oriented_handlers_missing_from_contract(&main_router, &calendar_router, &contract),
        Vec::<String>::new()
    );
}

#[test]
fn read_oriented_openapi_guard_detects_a_removed_router_registration() {
    let complete_router = r#"
        .route(
            "/api/lookup/staff",
            get(modules::lookup::handlers::lookup_staff),
        )
        .route(
            "/api/lookup/students",
            get(modules::lookup::handlers::lookup_students),
        )
    "#;
    let missing_route_router = r#"
        .route(
            "/api/lookup/staff",
            get(modules::lookup::handlers::lookup_staff),
        )
    "#;

    assert_eq!(
        read_oriented_handlers_from_routers(complete_router, "").len(),
        2
    );
    assert_eq!(
        read_oriented_handlers_from_routers(missing_route_router, "").len(),
        1,
        "removing a registered read route must reduce the derived inventory"
    );
}

#[test]
fn read_oriented_openapi_guard_detects_direct_and_nested_router_handlers() {
    let main_router = r#"
        .route(
            "/api/lookup/staff",
            get(modules::lookup::handlers::lookup_staff),
        )
        .nest("/api/calendar", modules::calendar::calendar_routes())
    "#;
    let calendar_router = r#"
        Router::new().route(
            "/events",
            get(handlers::list_calendar_events).post(handlers::create_calendar_event),
        )
    "#;
    let contract = r#"
        #[openapi(
            paths(crate::modules::lookup::handlers::lookup_staff),
            components(schemas(ApiErrorResponse))
        )]
    "#;

    assert_eq!(
        read_oriented_handlers_missing_from_contract(main_router, calendar_router, contract),
        vec!["crate::modules::calendar::handlers::list_calendar_events".to_string()]
    );
}

#[test]
fn module_handlers_use_actor_context_instead_of_raw_permission_lists() {
    let raw_permission_lookup =
        Regex::new(r"\bget_cached_user_permissions\b|\bpermission_matches\s*\(")
            .expect("valid regex");
    let mut violations = Vec::new();

    for file in module_rs_files() {
        if matches!(
            relative(&file).as_str(),
            "src/modules/auth/handlers.rs" | "src/modules/auth/session_service.rs"
        ) {
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
    let session_service = read_source(manifest_dir().join("src/modules/auth/session_service.rs"));
    let login_snapshot =
        extract_braced_block(&session_service, "async fn load_login_snapshot", false);
    let current_snapshot = extract_braced_block(
        &session_service,
        "async fn load_active_shell_snapshot",
        false,
    );

    assert!(login_snapshot.contains("get_cached_user_permissions"));
    assert!(current_snapshot.contains("get_cached_user_permissions"));
    assert!(!auth_handler.contains("permission_delegations"));
    assert!(!auth_handler.contains("department_permissions dp"));
    assert!(!auth_handler.contains("JOIN role_permissions"));
    assert!(!session_service.contains("permission_delegations"));
    assert!(!session_service.contains("department_permissions dp"));
    assert!(!session_service.contains("JOIN role_permissions"));
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
    let admin_menu_handler = read_source(manifest_dir().join("src/modules/menu/handlers/admin.rs"));
    let public_menu_service =
        read_source(manifest_dir().join("src/modules/menu/services/public_menu_service.rs"));
    let public_menu_handler =
        read_source(manifest_dir().join("src/modules/menu/handlers/public.rs"));
    let route_registration_service = read_source(
        manifest_dir().join("src/modules/system/services/route_registration_service.rs"),
    );
    let route_migration =
        read_source(manifest_dir().join("migrations/029_configurable_menu_workspaces.sql"));

    assert!(menu_models.contains("pub workspace: Option<String>"));
    assert!(menu_models.contains("pub struct MenuWorkspace"));
    assert!(menu_models.contains("pub workspace_code: String"));
    assert!(menu_models.contains("pub workspace_name: String"));
    assert!(menu_models.contains("pub workspace_order: i32"));
    assert!(menu_models.contains("#[serde(rename_all = \"camelCase\")]"));
    assert!(public_menu_service.contains("mg.workspace_code"));
    assert!(public_menu_service.contains("JOIN menu_workspaces mw"));
    assert!(public_menu_service.contains("workspace_code: row.group_workspace_code"));
    assert!(public_menu_service.contains("workspace_name: row.workspace_name"));
    assert!(public_menu_handler.contains("public_menu_service::group_and_filter_menu"));
    assert!(route_registration_service.contains("route_workspace_code("));
    assert!(route_registration_service.contains("ensure_route_navigation_defaults("));
    assert!(!route_registration_service
        .contains("UPDATE menu_groups SET workspace_code = $1 WHERE code = $2"));
    assert!(!route_registration_service.contains("name = EXCLUDED.name"));
    assert!(!route_registration_service.contains("icon = EXCLUDED.icon"));
    assert!(route_registration_service.contains("path = EXCLUDED.path"));
    assert!(
        route_registration_service.contains("required_permission = EXCLUDED.required_permission")
    );
    assert!(route_migration.contains("CREATE TABLE menu_workspaces"));
    assert!(route_migration.contains("workspace_code"));
    for permission in [
        "codes::MENU_READ_ALL",
        "codes::MENU_CREATE_ALL",
        "codes::MENU_UPDATE_ALL",
        "codes::MENU_DELETE_ALL",
    ] {
        assert!(admin_menu_handler.contains(permission));
    }
    assert!(!admin_menu_handler.contains("has_module_permission"));
    assert!(!public_menu_service.contains("feature_toggles"));
    assert!(!public_menu_handler.contains("feature_toggles"));
}

#[test]
fn braced_block_extractor_ignores_comment_contents() {
    let source = r#"
        fn guarded() {
            // forbidden_call(); }
            actual_call();
        }

        fn after_guarded() { forbidden_call(); }
    "#;
    let block = extract_braced_block(source, "fn guarded()", false);

    assert!(block.contains("actual_call();"), "block: {block}");
    assert!(!block.contains("forbidden_call();"), "block: {block}");
}

#[test]
fn braced_block_extractor_ignores_string_contents_and_excludes_following_code() {
    let source = r#"
        fn guarded() {
            let fake = "forbidden_call(); }";
            let raw = r"forbidden_call(); }";
            actual_call();
        }

        fn after_guarded() { forbidden_call(); }
    "#;
    let block = extract_braced_block(source, "fn guarded()", false);

    assert!(block.contains("actual_call();"), "block: {block}");
    assert!(!block.contains("forbidden_call();"), "block: {block}");
}

#[test]
fn structured_logging_guard_scopes_the_main_cli_exception() {
    let cli_only = r#"
        async fn main() {
            let command_args = env::args().skip(1).collect::<Vec<_>>();
            if command_args.first().map(String::as_str) == Some("export-openapi") {
                eprintln!("allowed CLI diagnostic");
                return;
            }
        }
    "#;
    let source = r#"
        async fn main() {
            let command_args = env::args().skip(1).collect::<Vec<_>>();
            if command_args.first().map(String::as_str) == Some("export-openapi") {
                eprintln!("allowed CLI diagnostic");
                return;
            }

            println!("runtime stdout");
            eprintln!("runtime stderr");
        }
    "#;

    assert_eq!(
        structured_logging_violations("src/main.rs", cli_only),
        Vec::<String>::new()
    );
    assert_eq!(
        structured_logging_violations("src/main.rs", source),
        vec![
            "src/main.rs: use tracing macros instead of println! in runtime code".to_string(),
            "src/main.rs: use tracing macros instead of eprintln! in runtime code".to_string(),
        ]
    );
}

#[test]
fn structured_logging_guard_ignores_cli_marker_decoys() {
    let source = r##"
        async fn main() {
            // if command_args.first().map(String::as_str) == Some("export-openapi") {
            let _decoy = r#"if command_args.first().map(String::as_str) == Some("export-openapi") {"#;
            eprintln!("runtime stderr must remain visible to the guard");

            let command_args = env::args().skip(1).collect::<Vec<_>>();
            if command_args.first().map(String::as_str) == Some("export-openapi") {
                eprintln!("allowed CLI diagnostic");
                return;
            }
        }
    "##;

    assert_eq!(
        structured_logging_violations("src/main.rs", source),
        vec!["src/main.rs: use tracing macros instead of eprintln! in runtime code".to_string()]
    );
}

#[test]
fn structured_logging_guard_detects_spaced_macro_tokens() {
    let source = r#"
        async fn main() {
            println ! ("runtime stdout");
            eprintln /* diagnostic */ ! ("runtime stderr");
        }
    "#;

    assert_eq!(
        structured_logging_violations("src/main.rs", source),
        vec![
            "src/main.rs: use tracing macros instead of println! in runtime code".to_string(),
            "src/main.rs: use tracing macros instead of eprintln! in runtime code".to_string(),
        ]
    );
}

#[test]
fn timetable_websocket_handler_orders_session_auth_before_room_state() {
    let source = read_source(manifest_dir().join("src/modules/academic/websockets.rs"));
    let params = extract_braced_block(&source, "pub struct WsParams", false);

    assert!(params.contains("pub academic_term_id: Uuid"));
    assert!(params.contains("pub school_subdomain: Option<String>"));
    assert!(!params.contains("user_id"));
    assert!(!params.contains("name:"));
    assert!(!params.contains("school_key"));

    let handler = extract_braced_block(&source, "pub async fn timetable_websocket_handler", false);
    assert!(handler.contains("parse_realtime_tenant_hint("));
    assert!(handler.contains("session_events.subscribe()"));
    assert!(handler.contains("permission_event_channel.subscribe()"));
    assert!(handler.contains("SessionMaintenanceMode::TouchOnly"));
    assert!(handler.contains("session_service::authenticate("));
    assert!(handler.contains("session_service::revalidate("));
    assert!(handler.contains("actor_tenant_context_from_session(&state, &authenticated)"));
    assert!(handler.contains("authorize_socket("));
    let permission_subscribe = handler
        .find("permission_event_channel.subscribe()")
        .expect("WebSocket must subscribe to permission changes before authentication");
    let session_subscribe = handler
        .find("session_events.subscribe()")
        .expect("WebSocket must subscribe to session changes before authentication");
    let authenticate = handler
        .find("session_service::authenticate(")
        .expect("WebSocket must authenticate through the central tenant context");
    let authorize = handler
        .find("authorize_socket(")
        .expect("WebSocket must authorize timetable access before upgrade");
    assert!(
        permission_subscribe < authenticate
            && session_subscribe < authenticate
            && authenticate < authorize,
        "session and permission subscriptions must precede authentication and authorization"
    );
    let immediate_revalidation = handler
        .find("session_service::revalidate(")
        .expect("WebSocket must immediately revalidate the authenticated session");
    assert!(authorize < immediate_revalidation);
    assert!(handler.contains("permission_event_receiver"));

    let socket_loop = extract_braced_block(&source, "async fn handle_socket", false);
    assert!(socket_loop.contains("sanitize_client_event("));
    assert!(socket_loop.contains("permission_event_receiver.recv()"));
    assert!(socket_loop.contains("permission_event_decision("));
    assert!(socket_loop.contains("session_event_receiver.recv()"));
    assert!(socket_loop.contains("session_event_decision("));
    assert!(socket_loop.contains("queued_session_decision("));

    let queued_permission_drain = socket_loop
        .find("initialize_socket_if_permissions_current(")
        .expect("queued permission changes must be drained through the production initializer");
    for operation in [
        "get_or_create_room(",
        "join_room(",
        "get_state_snapshot(",
        "TimetableEvent::StateSync",
        "TimetableEvent::UserJoined",
    ] {
        let operation_offset = socket_loop
            .find(operation)
            .unwrap_or_else(|| panic!("missing socket initialization operation: {operation}"));
        assert!(
            queued_permission_drain < operation_offset,
            "queued permission drain must precede {operation}"
        );
    }

    let select = socket_loop
        .find("tokio::select!")
        .expect("socket lifecycle must use one select loop");
    let biased = socket_loop[select..]
        .find("biased;")
        .map(|offset| select + offset)
        .expect("permission revocation must win when multiple socket branches are ready");
    let session_branch = socket_loop[select..]
        .find("session_change = session_event_receiver.recv()")
        .map(|offset| select + offset)
        .expect("select loop must receive session revocations");
    let permission_branch = socket_loop[select..]
        .find("permission_change = permission_event_receiver.recv()")
        .map(|offset| select + offset)
        .expect("select loop must receive permission changes");
    let heartbeat_branch = socket_loop[select..]
        .find("_ = heartbeat.tick()")
        .map(|offset| select + offset)
        .expect("socket must run the delayed heartbeat");
    let incoming_branch = socket_loop[select..]
        .find("incoming = socket.next()")
        .map(|offset| select + offset)
        .expect("select loop must receive client frames");
    let room_broadcast_branch = socket_loop[select..]
        .find("broadcast = rx.recv()")
        .map(|offset| select + offset)
        .expect("select loop must receive room events");
    assert!(
        select < biased
            && biased < session_branch
            && session_branch < permission_branch
            && permission_branch < heartbeat_branch
            && heartbeat_branch < incoming_branch
            && incoming_branch < room_broadcast_branch,
        "biased select must prioritize revocation and due revalidation before socket and room input"
    );

    let heartbeat_revalidation = socket_loop[heartbeat_branch..]
        .find("session_service::revalidate(")
        .map(|offset| heartbeat_branch + offset)
        .expect("heartbeat must revalidate the authoritative session");
    let heartbeat_ping = socket_loop[heartbeat_branch..]
        .find("Message::Ping")
        .map(|offset| heartbeat_branch + offset)
        .expect("heartbeat must send a ping after revalidation");
    assert!(heartbeat_revalidation < heartbeat_ping);
}

#[test]
fn timetable_websocket_authorization_authenticates_active_user_before_permissions() {
    let service = read_source(
        manifest_dir().join("src/modules/academic/services/timetable_realtime_service.rs"),
    );
    let authorize = extract_braced_block(&service, "pub async fn authorize_socket", false);
    let active_user_lookup = authorize
        .find("sqlx::query_as::<_, RealtimeUser>")
        .expect("socket authorization must authenticate an active user");
    let permission_check = authorize
        .find("socket_permission(actor)")
        .expect("socket authorization must evaluate timetable permissions");
    let term_lookup = authorize
        .find("sqlx::query_scalar::<_, bool>")
        .expect("socket authorization must verify the selected academic term");
    assert!(service.contains("FROM users WHERE id = $1 AND status = 'active'"));
    assert!(service.contains("FROM academic_terms WHERE id = $1"));

    assert!(
        active_user_lookup < permission_check && permission_check < term_lookup,
        "active-user authentication must precede permission and term authorization"
    );

    let app = read_source(manifest_dir().join("src/app.rs"));
    assert!(!app.contains("WebSocket Route (No standard middleware auth, uses Query Params)"));
    assert!(app.contains(
        "WebSocket authentication runs in the handler; query selects academic term only"
    ));
}

#[test]
fn auth_me_rejects_inactive_users_before_loading_roles_or_permissions() {
    let handler_source = read_source(manifest_dir().join("src/modules/auth/session_handlers.rs"));
    let handler = extract_braced_block(&handler_source, "pub async fn me", false);
    let session_service = read_source(manifest_dir().join("src/modules/auth/session_service.rs"));
    let load_current =
        extract_braced_block(&session_service, "pub async fn load_current_user", false);
    let snapshot = extract_braced_block(
        &session_service,
        "async fn load_active_shell_snapshot",
        false,
    );
    let auth_services = read_source(manifest_dir().join("src/modules/auth/services.rs"));
    let active_lookup = extract_braced_block(
        &auth_services,
        "pub async fn find_active_user_shell_by_id",
        false,
    );

    assert!(handler.contains("session_service::load_current_user(&context, &session).await?"));
    let verify_active = load_current
        .find("find_active_user_shell_by_id")
        .expect("auth me must load only an active user");
    let load_snapshot = load_current
        .find("load_active_shell_snapshot")
        .expect("auth me must hydrate the response after active-user validation");
    assert!(active_lookup.contains("sqlx::query_as::<_, ActiveUserShell>"));
    assert!(auth_services.contains("WHERE id = $1 AND status = 'active'"));
    assert!(snapshot.contains("get_primary_role_name"));
    assert!(snapshot.contains("get_cached_user_permissions"));

    assert!(
        verify_active < load_snapshot,
        "auth me must reject an inactive user before loading roles or permissions"
    );
}

#[test]
fn deleting_staff_revokes_cached_and_active_permissions_after_soft_delete() {
    let source = read_source(manifest_dir().join("src/modules/staff/handlers/staff.rs"));
    let handler = extract_braced_block(&source, "pub async fn delete_staff", false);
    let tenant = handler
        .find("let tenant = context.tenant.subdomain.clone()")
        .expect("delete_staff must retain the tenant before moving the pool");
    let pool = handler
        .find("let pool = context.tenant.pool")
        .expect("delete_staff must use the resolved tenant pool");
    let soft_delete = handler
        .find("staff_service::soft_delete_staff(&pool, staff_id).await?")
        .expect("delete_staff must soft-delete the user");
    let invalidate = handler
        .find("state.permission_cache.invalidate_user(&tenant, staff_id)")
        .expect("delete_staff must invalidate the deleted user's cached permissions");
    let notify = handler
        .find("state.notify_permission_changed(&tenant, staff_id)")
        .expect("delete_staff must revoke the deleted user's active permission sessions");

    assert!(tenant < pool);
    assert!(soft_delete < invalidate && invalidate < notify);
}

#[test]
fn timetable_websocket_proxy_does_not_log_query_identity() {
    let nginx = read_source(repo_root().join("nginx-configs/school-api.conf.template"));
    let websocket_location = extract_braced_block(&nginx, "location /ws/ {", true);
    let access_log_directives = websocket_location
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("access_log "))
        .collect::<Vec<_>>();

    assert_eq!(access_log_directives, vec!["access_log off;"]);
    for forbidden in ["$request", "$args", "$query_string", "$request_uri"] {
        assert!(
            !websocket_location.contains(forbidden),
            "WebSocket access logging must not include {forbidden}"
        );
    }

    let testing = read_source(repo_root().join("docs/TESTING.md"));
    assert!(testing.contains("legacy query identity"));
    for legacy_identity in ["`user_id`", "`name`", "`school_key`"] {
        assert!(
            testing.contains(legacy_identity),
            "rollout log checklist must name {legacy_identity}"
        );
    }
}

#[test]
fn permission_cache_and_process_events_are_tenant_explicit() {
    let cache = read_source(manifest_dir().join("src/db/permission_cache.rs"));
    let events = read_source(manifest_dir().join("src/modules/notification/events.rs"));
    let main = read_source(manifest_dir().join("src/main.rs"));

    assert!(cache.contains("TenantUserKey"));
    assert!(cache.contains("PermissionCacheRevision"));
    assert!(cache.contains("snapshot_revision"));
    assert!(cache.contains("fill_if_current"));
    assert!(
        Regex::new(r"invalidate_user\s*\(\s*&self,\s*tenant:\s*&str")
            .unwrap()
            .is_match(&cache)
    );
    assert!(
        Regex::new(r"invalidate_tenant\s*\(\s*&self,\s*tenant:\s*&str")
            .unwrap()
            .is_match(&cache)
    );
    assert!(!cache.contains("clear_all"));
    assert!(events.contains("pub tenant: String"));
    assert!(
        Regex::new(r"notify_permission_changed\s*\(\s*&self,\s*tenant:\s*&str")
            .unwrap()
            .is_match(&main)
    );
    assert!(
        Regex::new(r"notify_work_items_changed\s*\(\s*&self,\s*tenant:\s*&str")
            .unwrap()
            .is_match(&main)
    );

    let permission_middleware = read_source(manifest_dir().join("src/middleware/permission.rs"));
    let load = extract_braced_block(
        &permission_middleware,
        "pub async fn get_cached_user_permissions",
        false,
    );
    let revision_snapshot = load
        .find("cache.snapshot_revision(tenant, user_id)")
        .expect("permission load must snapshot cache revision before fetching");
    let database_fetch = load
        .find("fetch_user_permissions(user_id, pool).await")
        .expect("permission load must fetch permissions on a cache miss");
    let guarded_fill = load
        .find("cache.fill_if_current(")
        .expect("permission load must conditionally fill only the captured revision");
    assert!(
        revision_snapshot < database_fetch && database_fetch < guarded_fill,
        "revision snapshot must surround the in-flight permission fetch"
    );
}

#[test]
fn feature_modules_do_not_parse_jwt_directly() {
    for path in list_files(manifest_dir().join("src/modules"), |path| {
        path.extension().and_then(|extension| extension.to_str()) == Some("rs")
    }) {
        let source = read_source(&path);
        assert!(
            !source.contains("JwtService::verify_token"),
            "duplicate JWT verification in {}",
            relative(&path)
        );
    }
}

#[test]
fn browser_auth_uses_one_session_boundary_and_no_jwt_runtime() {
    let main = read_source(manifest_dir().join("src/main.rs"));
    let app = read_source(manifest_dir().join("src/app.rs"));
    assert!(!main.contains("auth_middleware"));
    assert_eq!(
        app.matches("from_fn_with_state(runtime, session_middleware)")
            .count(),
        1
    );
    assert!(!manifest_dir().join("src/utils/jwt.rs").exists());
    assert!(!manifest_dir().join("src/middleware/auth.rs").exists());
    assert!(!app.contains("Authorization"));
    assert!(!read_source(manifest_dir().join("Cargo.toml")).contains("jsonwebtoken"));
}

#[test]
fn public_and_protected_routes_are_explicitly_partitioned() {
    let app = read_source(manifest_dir().join("src/app.rs"));
    assert!(app.contains("public_routes()"));
    assert!(app.contains("protected_routes()"));
    assert!(app.contains("admission_public_routes()"));
    assert!(app.contains("admission_staff_routes()"));
    assert!(app.contains("/internal/migrate-all"));
    assert!(app.contains("/api/admin/routes/sync"));
    assert_eq!(
        app.matches("DefaultBodyLimit::max(AUTH_JSON_BODY_LIMIT)")
            .count(),
        2
    );
    assert!(app.contains("DefaultBodyLimit::max(APPLICATION_BODY_LIMIT)"));
}

#[test]
fn permission_cache_invalidations_notify_active_clients() {
    let mut violations = Vec::new();

    for file in backend_rs_files() {
        if relative(&file) == "src/db/permission_cache.rs" {
            continue;
        }

        violations.extend(permission_invalidation_violations(
            &relative(&file),
            &read_source(&file),
        ));
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn permission_invalidation_guard_rejects_mismatched_notification_arguments() {
    let violations = permission_invalidation_violations(
        "src/modules/example.rs",
        r#"
            alternate_cache.invalidate_user(&tenant_a, user_a);
            state.notify_permission_changed(&tenant_b, user_a);

            alternate_cache.invalidate_user(&tenant_a, user_a);
            state.notify_permission_changed(&tenant_a, user_b);

            alternate_cache.invalidate_tenant(&tenant_a);
            state.notify_all_permissions_changed(&tenant_b);
        "#,
    );

    assert_eq!(violations.len(), 3, "violations: {violations:#?}");
}

#[test]
fn permission_invalidation_guard_parses_multiline_and_nested_arguments() {
    let mismatched = permission_invalidation_violations(
        "src/modules/multiline_mismatch.rs",
        r#"
            alternate_cache.invalidate_user(
                tenant_for(&tenant_a, region("north,west")),
                user_for((user_a, fallback(user_b, user_c))),
            );
            state.notify_permission_changed(
                tenant_for(&tenant_b, region("north,west")),
                user_for((user_a, fallback(user_b, user_c))),
            );

            alternate_cache.invalidate_user(
                tenant_for(&tenant_a, region("north,west")),
                user_for((user_a, fallback(user_b, user_c))),
            );
            state.notify_permission_changed(
                tenant_for(&tenant_a, region("north,west")),
                user_for((user_b, fallback(user_a, user_c))),
            );
        "#,
    );
    assert_eq!(mismatched.len(), 2, "violations: {mismatched:#?}");

    let matching = permission_invalidation_violations(
        "src/modules/multiline_match.rs",
        r#"
            alternate_cache.invalidate_user(
                tenant_for(&tenant_a, region("north,west")),
                user_for((user_a, fallback(user_b, user_c))),
            );
            state.notify_permission_changed(
                tenant_for(&tenant_a, region("north,west")),
                user_for((user_a, fallback(user_b, user_c))),
            );

            alternate_cache.invalidate_tenant(
                tenant_for(&tenant_a, region("north,west")),
            );
            state.notify_all_permissions_changed(
                tenant_for(&tenant_a, region("north,west")),
            );
        "#,
    );
    assert!(matching.is_empty(), "violations: {matching:#?}");
}

#[test]
fn permission_invalidation_guard_ignores_comment_and_string_decoys() {
    let violations = permission_invalidation_violations(
        "src/modules/decoys.rs",
        r##"
            let normal = ".invalidate_user(&tenant_a, user_a);";
            let raw = r#".invalidate_tenant(&tenant_a);"#;
            // alternate_cache.invalidate_user(&tenant_a, user_a);
            /* alternate_cache.invalidate_tenant(&tenant_a); */
        "##,
    );

    assert!(violations.is_empty(), "violations: {violations:#?}");
}

#[test]
fn permission_change_sse_supports_user_targeted_and_broadcast_invalidation() {
    let event_source = read_source(manifest_dir().join("src/modules/notification/events.rs"));
    let notification_handler =
        read_source(manifest_dir().join("src/modules/notification/handlers.rs"));
    let app_state = read_source(manifest_dir().join("src/main.rs"));

    for expected in [
        "pub fn for_user(tenant: &str, user_id: Uuid)",
        "pub fn for_all_users(tenant: &str)",
        "pub fn applies_to(&self, tenant: &str, user_id: Uuid) -> bool",
    ] {
        assert!(event_source.contains(expected), "missing {expected}");
    }

    for expected in [
        "permission_event_channel.subscribe()",
        "event.applies_to(&tenant, user_id)",
        ".event(\"permission_changed\")",
    ] {
        assert!(
            notification_handler.contains(expected),
            "missing {expected}"
        );
    }

    assert!(
        app_state.contains("notify_permission_changed(&self, tenant: &str, target_user_id: Uuid)")
    );
    assert!(app_state.contains("notify_all_permissions_changed(&self, tenant: &str)"));
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
        "pub fn work_items_changed(tenant: &str)",
        "pub fn workflow_window_changed(tenant: &str)",
        "pub fn applies_to(&self, tenant: &str) -> bool",
    ] {
        assert!(event_source.contains(expected), "missing {expected}");
    }

    for expected in [
        "work_event_channel.subscribe()",
        "event.applies_to(&tenant)",
        "event.event_name()",
        "NotificationStreamEvent::WorkChanged(event.event_name())",
        ".event(event_name)",
    ] {
        assert!(
            notification_handler.contains(expected),
            "missing {expected}"
        );
    }

    assert!(
        Regex::new(r"notify_work_items_changed\s*\(\s*&self,\s*tenant:\s*&str")
            .unwrap()
            .is_match(&app_state)
    );
    assert!(
        Regex::new(r"notify_workflow_window_changed\s*\(\s*&self,\s*tenant:\s*&str")
            .unwrap()
            .is_match(&app_state)
    );

    assert!(work_handler.contains("state.notify_work_items_changed(&context.tenant.subdomain)"));
    assert!(workflow_handler
        .contains("state.notify_workflow_window_changed(&context.tenant.subdomain)"));
}

#[test]
fn teaching_supervision_registry_and_module_are_registered() {
    let registry = read_source(manifest_dir().join("src/permissions/registry_generated.rs"));
    let modules = read_source(manifest_dir().join("src/modules.rs"));

    for expected in [
        "SUPERVISION_READ_OWN",
        "SUPERVISION_READ_ASSIGNED",
        "SUPERVISION_READ_ORGANIZATION_UNIT",
        "SUPERVISION_READ_ORGANIZATION_TREE",
        "SUPERVISION_READ_SCHOOL",
        "SUPERVISION_REQUEST_OWN",
        "SUPERVISION_MANAGE_ORGANIZATION_UNIT",
        "SUPERVISION_MANAGE_ORGANIZATION_TREE",
        "SUPERVISION_MANAGE_SCHOOL",
        "SUPERVISION_EVALUATE_ASSIGNED",
        "SUPERVISION_APPROVE_SCHOOL",
        "supervision.read.own",
        "supervision.manage.organization_unit",
        "supervision.manage.organization_tree",
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
fn teaching_supervision_subject_group_management_permissions_are_seeded() {
    let migration = read_source(
        manifest_dir()
            .join("migrations/007_teaching_supervision_scoped_management_permissions.sql"),
    );

    for expected in [
        "supervision.manage.organization_unit",
        "supervision.manage.organization_tree",
        "organization_permission_grants",
        "unit_type = 'subject_group'",
        "position_code",
        "'head'",
        "'deputy_head'",
        "ON CONFLICT DO NOTHING",
    ] {
        assert!(
            migration.contains(expected),
            "missing supervision scoped management seed {expected}"
        );
    }
}

#[test]
fn teaching_supervision_academic_affairs_approval_permission_is_seeded() {
    let migration = read_source(
        manifest_dir().join("migrations/010_supervision_academic_affairs_approval_grant.sql"),
    );

    for expected in [
        "supervision.approve.school",
        "organization_permission_grants",
        "code = 'ACAD-01'",
        "'head'",
        "'deputy_head'",
        "ON CONFLICT DO NOTHING",
    ] {
        assert!(
            migration.contains(expected),
            "missing supervision academic affairs approval seed {expected}"
        );
    }
}

#[test]
fn teaching_supervision_default_staff_permissions_are_seeded() {
    let migration = read_source(
        manifest_dir().join("migrations/006_teaching_supervision_default_permissions.sql"),
    );

    for expected in [
        "supervision.read.own",
        "supervision.request.own",
        "supervision.read.assigned",
        "supervision.evaluate.assigned",
        "role_permissions",
        "organization_permission_grants",
        "ON CONFLICT DO NOTHING",
    ] {
        assert!(
            migration.contains(expected),
            "missing supervision default permission seed {expected}"
        );
    }
}

#[test]
fn teaching_supervision_handlers_use_request_context_and_services() {
    let handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/supervision/handlers.rs"),
    ));

    assert!(handler.contains("actor_tenant_context_from_session"));
    assert!(handler.contains("ApiResponse::ok"));
    assert!(handler.contains("supervision_access_policy"));
    assert!(handler.contains("require_observation_management_access"));
    assert!(handler.contains("services::"));
    assert!(!handler.contains("sqlx::query"));
    assert!(!handler.contains(".fetch_"));
    assert!(!handler.contains(".execute("));
}

#[test]
fn teaching_supervision_observation_detail_actions_are_registered() {
    let handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/supervision/handlers.rs"),
    ));
    let models = strip_comments(&read_source(
        manifest_dir().join("src/modules/supervision/models.rs"),
    ));
    let service = strip_comments(
        &[
            "src/modules/supervision/services/shared.rs",
            "src/modules/supervision/services/observations.rs",
            "src/modules/supervision/services/evaluations.rs",
        ]
        .into_iter()
        .map(|path| read_source(manifest_dir().join(path)))
        .collect::<String>(),
    );

    for expected in [
        "patch(update_observation)",
        "put(replace_observation_evaluators)",
        "post(cancel_observation)",
    ] {
        assert!(
            handler.contains(expected),
            "missing supervision observation detail route/action {expected}"
        );
    }

    for expected in [
        "UpdateSupervisionObservationRequest",
        "ReplaceObservationEvaluatorsRequest",
        "CancelObservationRequest",
        "SupervisionAction",
        "pub actions: Vec<SupervisionAction>",
    ] {
        assert!(
            models.contains(expected),
            "missing supervision observation detail DTO {expected}"
        );
    }

    for expected in [
        "manager_can_edit_observation",
        "replace_observation_evaluators",
        "cancel_observation",
        "normalize_evaluator_replacement",
        "load_observation_actions",
    ] {
        assert!(
            service.contains(expected),
            "missing supervision observation detail service helper {expected}"
        );
    }
}

#[test]
fn teaching_supervision_services_use_bulk_mutations_for_multi_row_writes() {
    let service = strip_comments(
        &[
            "src/modules/supervision/services/templates.rs",
            "src/modules/supervision/services/evaluations.rs",
        ]
        .into_iter()
        .map(|path| read_source(manifest_dir().join(path)))
        .collect::<String>(),
    );

    for expected in [
        "build_template_section_bulk_rows",
        "bulk_insert_template_sections",
        "bulk_insert_template_items",
        "load_evaluation_item_specs",
        "bulk_upsert_evaluation_responses",
        "dedupe_evaluation_responses",
    ] {
        assert!(
            service.contains(expected),
            "supervision service should keep multi-row mutation helper {expected}"
        );
    }

    assert!(
        !service.contains("for response in input.responses"),
        "supervision evaluation responses should not be saved one database call per response"
    );
}

#[test]
fn mutation_performance_foundation_services_use_bulk_helpers() {
    let checked = [
        (
            "src/modules/admission/services/score_service.rs",
            [
                "upsert_application_scores",
                "score_entries_to_bulk_rows",
                "bulk_update_scores",
            ]
            .as_slice(),
            ["for entry in scores"].as_slice(),
        ),
        (
            "src/modules/admission/services/exam_room_service.rs",
            ["insert_exam_seat_assignments"].as_slice(),
            [
                "for (app_id, rid, seat, eid) in &new_assignments",
                "for (app_id, rid, seat, eid) in &assignments",
            ]
            .as_slice(),
        ),
        (
            "src/modules/admission/services/application_service.rs",
            [
                "student_id_assignment_rows",
                "bulk_update_assigned_student_ids",
            ]
            .as_slice(),
            ["for student in &students"].as_slice(),
        ),
        (
            "src/modules/menu/services/menu_service.rs",
            ["reorder_menu_groups"].as_slice(),
            ["for (id, display_order) in &groups"].as_slice(),
        ),
        (
            "src/modules/supervision/services/evaluations.rs",
            ["insert_supervision_evaluators"].as_slice(),
            ["for evaluator in input.evaluators"].as_slice(),
        ),
        (
            "src/modules/supervision/services/cycles.rs",
            ["Failed to insert supervision cycle targets"].as_slice(),
            ["Failed to insert supervision cycle target:"].as_slice(),
        ),
        (
            "src/modules/supervision/services/templates.rs",
            ["Failed to insert supervision template steps"].as_slice(),
            ["Failed to insert supervision template step:"].as_slice(),
        ),
        (
            "src/modules/staff/services/organization_permission_service.rs",
            ["bulk_insert_organization_permission_grants"].as_slice(),
            ["for grant in unique_permission_grants"].as_slice(),
        ),
        (
            "src/modules/staff/services/role_service.rs",
            ["insert_role_permissions"].as_slice(),
            ["for perm_id in perm_ids"].as_slice(),
        ),
        (
            "src/modules/staff/services/staff_service.rs",
            [
                "insert_user_roles",
                "insert_organization_memberships",
                "organization_assignments_to_bulk_rows",
            ]
            .as_slice(),
            [
                "for role_id in &payload.role_ids",
                "for role_id in role_ids",
                "for assignment in organization_assignments",
            ]
            .as_slice(),
        ),
        (
            "src/modules/work/services.rs",
            ["insert_work_item_assignees"].as_slice(),
            ["for assignee in assignees"].as_slice(),
        ),
    ];

    for (path, required_helpers, rejected_patterns) in checked {
        let source = strip_comments(&read_source(manifest_dir().join(path)));
        for helper in required_helpers {
            assert!(
                source.contains(helper),
                "{path}: missing bulk mutation helper {helper}"
            );
        }
        for pattern in rejected_patterns {
            assert!(
                !source.contains(pattern),
                "{path}: replace per-row mutation loop `{pattern}` with a bulk helper"
            );
        }
    }
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

#[test]
fn backend_school_registers_separate_liveness_and_readiness_routes() {
    let main = read_source(repo_root().join("backend-school/src/main.rs"));
    let app = read_source(repo_root().join("backend-school/src/app.rs"));
    let health =
        read_source(repo_root().join("backend-school/src/modules/system/handlers/health.rs"));
    let health_route = Regex::new(r#"\.route\(\s*"/health","#).expect("valid health regex");
    let ready_route = Regex::new(r#"\.route\(\s*"/ready","#).expect("valid ready regex");

    assert!(health_route.is_match(&app));
    assert!(ready_route.is_match(&app));
    assert!(app.contains("handlers::health::health_check"));
    assert!(app.contains("handlers::health::readiness_check"));
    assert!(main.contains("GET  /ready"));
    assert!(health.contains("state.admin_client.check_readiness()"));
    assert!(health.contains("state.file_platform.check_readiness()"));
    assert!(!health.contains("get_pool("));
    assert!(!health.contains("PgPool"));
}

#[test]
fn school_session_runtime_is_deployment_owned() {
    let main = read_source(repo_root().join("backend-school/src/main.rs"));
    let session_config = main
        .find("modules::auth::config::SessionConfig::from_env()")
        .expect("backend-school must initialize session configuration");
    let app = main
        .find("app::build_app(state.clone())")
        .expect("backend-school must build the application");
    assert!(session_config < app);

    for file in ["docker-compose.yml", "podman-compose.yml"] {
        let compose = read_source(repo_root().join(file));
        let admin_start = compose
            .find("  backend-admin:")
            .expect("compose must define backend-admin");
        let school_start = compose
            .find("  backend-school:")
            .expect("compose must define backend-school");
        let school_end = compose[school_start..]
            .find("\n  clamd:")
            .map(|offset| school_start + offset)
            .expect("compose must define clamd after backend-school");
        let admin = &compose[admin_start..school_start];
        let school = &compose[school_start..school_end];

        assert!(admin.contains("${JWT_SECRET"));
        assert!(!admin.contains("${SCHOOL_ROLLBACK_JWT_SECRET"));
        assert!(school.contains("${SCHOOL_ROLLBACK_JWT_SECRET"));
        assert!(!school.contains("${JWT_SECRET"));
        assert!(school.contains("${SESSION_HMAC_KEY"));
        assert!(school.contains("${BASE_DOMAIN"));
        assert!(school.contains("TRUSTED_PROXY_CIDRS"));
        assert!(school.contains("SCHOOL_ALLOWED_DEV_ORIGINS"));
    }
}

#[test]
fn recurring_healthchecks_use_liveness_while_deployment_and_smoke_use_readiness() {
    let docker_compose = read_source(repo_root().join("docker-compose.yml"));
    let podman_compose = read_source(repo_root().join("podman-compose.yml"));
    let school_deploy =
        read_source(repo_root().join(".github/workflows/deploy-backend-school.yml"));
    let frontend_deploy = read_source(repo_root().join(".github/workflows/deploy-all-schools.yml"));
    let admin_deploy = read_source(repo_root().join(".github/workflows/deploy-backend-admin.yml"));
    let smoke = read_source(repo_root().join("scripts/smoke_test.sh"));

    for compose in [&docker_compose, &podman_compose] {
        assert!(compose.contains("http://localhost:8080/health"));
        assert!(compose.contains("http://localhost:8081/health"));
        assert!(!compose.contains("http://localhost:8080/ready"));
        assert!(!compose.contains("http://localhost:8081/ready"));
        assert!(compose.contains("BACKEND_ADMIN_REQUEST_TIMEOUT_MS"));
        assert!(compose.contains("BACKEND_ADMIN_RETRY_MAX_ATTEMPTS"));
        assert!(compose.contains("BACKEND_ADMIN_RETRY_BASE_DELAY_MS"));
        assert!(compose.contains("docker.io/clamav/clamav-debian:1.5.3"));
    }
    assert!(school_deploy.contains("docker.io/amazon/aws-cli:2.36.9"));
    assert!(school_deploy.contains("docker.io/clamav/clamav-debian:1.5.3"));
    assert!(!repo_root()
        .join("backend-school/docker-compose.yml")
        .exists());
    assert!(!school_deploy.contains("list-buckets"));
    assert!(school_deploy.contains(r#"r2_cli s3api head-bucket --bucket "$public_bucket""#));
    assert!(school_deploy.contains(r#"r2_cli s3api head-bucket --bucket "$private_bucket""#));
    assert!(school_deploy.contains("put-bucket-cors"));
    assert!(school_deploy.contains("get-bucket-cors"));
    assert!(school_deploy.contains(r#"private_cors_origin="https://*.${base_domain}""#));
    assert!(school_deploy.contains(r#"AllowedMethods:["GET","HEAD"]"#));
    assert!(school_deploy
        .contains(r#"podman-compose -f "${runtime_compose}.next" --dry-run up -d backend-school"#));
    assert!(school_deploy
        .contains(r#"podman-compose -f "$runtime_compose" --dry-run up -d clamd backend-school"#));
    assert!(school_deploy.contains("http://127.0.0.1:8081/ready"));
    assert!(admin_deploy.contains("http://127.0.0.1:8080/ready"));
    assert!(school_deploy.contains(r#"--resolve "${school_host}:443:127.0.0.1""#));
    assert!(admin_deploy.contains(r#"--resolve "${admin_host}:443:127.0.0.1""#));
    assert!(school_deploy.contains("cloudflare-origin-rsa-root.pem"));
    assert!(admin_deploy.contains("cloudflare-origin-rsa-root.pem"));
    assert!(school_deploy.contains("seq 1 36"));
    assert!(admin_deploy.contains("seq 1 12"));
    assert!(school_deploy.contains("timeout 180 bash -c"));
    assert!(admin_deploy.contains("timeout 180 bash -c"));
    assert!(frontend_deploy.contains("BACKEND_SCHOOL_URL: ${{ vars.BACKEND_SCHOOL_URL }}"));
    assert!(frontend_deploy.contains("${BACKEND_SCHOOL_URL%/}/ready"));
    assert!(frontend_deploy.contains(r#".filePlatform == "ready""#));
    assert!(smoke.contains("$SMOKE_ADMIN_API_URL/ready"));
    assert!(smoke.contains("$SMOKE_API_URL/ready"));
}

#[test]
fn scheduled_jobs_use_explicit_bangkok_timezone() {
    let main = read_source(&repo_root().join("backend-school/src/main.rs"));
    let scheduling = read_source(&repo_root().join("backend-school/src/scheduling.rs"));

    assert!(!main.contains("Job::new_async("));
    assert!(scheduling.contains("Job::new_async_tz"));
    assert!(scheduling.contains("chrono_tz::Asia::Bangkok"));
    assert!(scheduling.contains("0 0 * * * *"));
    assert!(!scheduling.contains("0 */5 * * * *"));
}

#[test]
fn scheduled_jobs_never_trigger_lazy_tenant_migrations() {
    let main = read_source(repo_root().join("backend-school/src/main.rs"));
    let reminders =
        read_source(repo_root().join("backend-school/src/modules/calendar/services/reminders.rs"));

    let non_migrating_call = ".get_pool_without_migrations(&db_url, &school.subdomain)";
    assert!(main.contains(non_migrating_call));
    assert!(reminders.contains(non_migrating_call));
    assert!(!main.contains(".get_pool(&db_url, &school.subdomain)"));
    assert!(!reminders.contains(".get_pool(&db_url, &school.subdomain)"));
}

#[test]
fn backend_school_deploy_can_finish_in_maintenance_mode() {
    let deploy = read_source(repo_root().join(".github/workflows/deploy-backend-school.yml"));

    assert!(deploy.contains(
        "SCHOOL_API_KEEP_MAINTENANCE: ${{ vars.SCHOOL_API_KEEP_MAINTENANCE || 'true' }}"
    ));
    assert!(deploy.contains("envs: SCHOOL_API_KEEP_MAINTENANCE"));
    assert!(deploy.contains("keep_school_api_maintenance=${SCHOOL_API_KEEP_MAINTENANCE:-true}"));
    assert!(deploy
        .contains("School API remains in maintenance until the authenticated smoke completes"));
    assert!(deploy.contains(".academicCoreCutover.migrationVersion == 45"));
    assert!(deploy.contains(".academicCoreCutover.status == \"cleanupCompleted\""));
    assert!(deploy.contains(".academicCoreCutover.passed == true"));
    assert!(deploy.contains("all(.academicCoreCutover.checks[]; .passed == true)"));
    assert!(!deploy.contains("academic_core_phase_a_reconcile"));
    assert!(!deploy.contains("ACADEMIC_CORE_PHASE_A_RECONCILE"));
    assert!(!deploy.contains("/internal/academic-core/reconcile-all"));
}

#[test]
fn academic_core_smoke_is_private_authenticated_read_only_and_precedes_go_live() {
    let deploy = read_source(repo_root().join(".github/workflows/deploy-backend-school.yml"));
    let smoke = read_source(repo_root().join("scripts/smoke_test.sh"));

    assert!(deploy.contains("academic_core_cleanup_smoke:"));
    assert!(deploy.contains("academic_core_smoke_subdomain:"));
    assert!(deploy.contains("Validate Academic Core authenticated smoke inputs"));
    assert!(deploy.contains("Run Academic Core authenticated smoke"));
    assert!(deploy.contains("Open School API after authenticated smoke"));
    assert!(deploy.contains("SMOKE_USERNAME: ${{ secrets.SMOKE_USERNAME }}"));
    assert!(deploy.contains("SMOKE_PASSWORD: ${{ secrets.SMOKE_PASSWORD }}"));
    assert!(deploy.contains("SMOKE_API_URL=http://localhost:8081"));
    assert!(deploy.contains("SMOKE_ACADEMIC_CONTEXT=true"));
    assert!(deploy.contains("SMOKE_DIRECT_BACKEND=true"));
    assert!(deploy.contains("vars.SCHOOL_API_KEEP_MAINTENANCE || 'true'"));
    assert!(deploy.contains("restore_maintenance"));
    assert!(
        deploy.find("Run Academic Core authenticated smoke")
            < deploy.find("Open School API after authenticated smoke"),
        "the normal proxy may open only after authenticated smoke"
    );
    assert!(smoke.contains("SMOKE_DIRECT_BACKEND"));
    assert!(smoke.contains("expect_cors_header"));
    assert!(smoke.contains(r#"if [[ $SMOKE_DIRECT_BACKEND == false ]]; then"#));

    let academic_start = smoke
        .find("# Academic Core maintenance read-only smoke start")
        .expect("academic smoke start marker");
    let academic_end = smoke
        .find("# Academic Core maintenance read-only smoke end")
        .expect("academic smoke end marker");
    let academic_smoke = &smoke[academic_start..academic_end];

    for path in [
        "/api/academic/context/options",
        "/api/academic/years",
        "/api/academic/terms?academicYearId=",
        "/api/academic/offerings?academicTermId=",
        "/api/academic/assessments/plans?academicTermId=",
        "/api/academic/timetable?academicTermId=",
        "/api/academic/exam-schedules?academicTermId=",
        "/api/supervision/cycles?academicYearId=",
        "/api/supervision/observations?academicYearId=",
        "/api/admission/rounds?academicYearId=",
        "/api/staff/dashboard?academicYearId=",
    ] {
        assert!(
            academic_smoke.contains(path),
            "missing read-only smoke path: {path}"
        );
    }
    for method in ["POST", "PUT", "PATCH", "DELETE"] {
        assert!(
            !academic_smoke.contains(&format!(" {method} ")),
            "academic smoke must not use {method}"
        );
    }
}

#[test]
fn learning_group_teacher_endpoint_accepts_a_typed_put_body() {
    let routes = read_source(manifest_dir().join("src/modules/academic/delivery.rs"));
    let handlers = read_source(manifest_dir().join("src/modules/academic/delivery/handlers.rs"));
    let models = read_source(manifest_dir().join("src/modules/academic/delivery/models.rs"));

    assert!(routes.contains("\"/learning-groups/{id}/teachers\""));
    assert!(routes.contains("put(handlers::replace_group_teachers)"));
    assert!(handlers.contains("Json(request): Json<ReplaceLearningGroupTeachersRequest>"));
    assert!(models.contains("pub struct ReplaceLearningGroupTeachersRequest"));
    assert!(models.contains("pub teachers: Vec<TeacherAssignmentInput>"));
}

#[test]
fn learning_delivery_handlers_enforce_policy_and_service_boundaries() {
    let handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery/handlers.rs"),
    ));

    for handler_name in [
        "list_offerings",
        "create_offering",
        "update_offering",
        "publish_offering",
        "list_groups",
        "create_group",
        "replace_group_teachers",
        "apply_group_roster",
        "publish_group_roster",
    ] {
        let body = extract_braced_block(&handlers, &format!("pub async fn {handler_name}"), false);
        assert!(
            body.contains("actor_tenant_context_from_session(&state, &session).await?"),
            "{handler_name} must load actor and tenant through the shared request context"
        );
        assert!(
            body.contains("learning_offering_access_policy")
                || body.contains("require_group_access"),
            "{handler_name} must authorize through the learning-offering policy"
        );
        for forbidden_db_call in ["sqlx::query", ".execute(", ".fetch_"] {
            assert!(
                !body.contains(forbidden_db_call),
                "{handler_name} must delegate database work to the service layer"
            );
        }
    }
}

#[test]
fn learning_group_team_changes_are_versioned_and_serialized() {
    let service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery/services/groups.rs"),
    ));

    assert!(
        service.contains("pub async fn replace_teachers")
            && service.contains("let group = lock_group(&mut transaction, id).await?")
            && service.contains("require_mutable_group(&group, request.row_version, false)?")
            && service.contains("DELETE FROM learning_group_teachers")
            && service.contains("INSERT INTO learning_group_teachers"),
        "teaching-team replacement must lock and version the canonical learning group"
    );
    assert!(!service.contains("classroom_courses"));
}

#[test]
fn calendar_schema_routes_and_permissions_are_registered() {
    let migration = read_source(manifest_dir().join("migrations/018_school_calendar.sql"));
    let tags_migration = read_source(manifest_dir().join("migrations/026_calendar_event_tags.sql"));
    let backend_registry =
        read_source(manifest_dir().join("src/permissions/registry_generated.rs"));
    let frontend_registry = read_source(
        repo_root()
            .join("frontend-school")
            .join("src/lib/permissions/registry.generated.ts"),
    );
    let modules_root = read_source(manifest_dir().join("src/modules.rs"));
    let main_source = read_source(manifest_dir().join("src/main.rs"));
    let app = strip_comments(&read_source(manifest_dir().join("src/app.rs")));

    for required in [
        "CREATE TABLE calendar_categories",
        "CREATE TABLE calendar_events",
        "CREATE TABLE calendar_event_targets",
        "CREATE TABLE calendar_event_reminders",
        "days_before INTEGER NOT NULL",
        "remind_on DATE NOT NULL",
        "CONSTRAINT calendar_event_targets_single_scope CHECK (grade_level_id IS NULL OR class_room_id IS NULL)",
        "CREATE UNIQUE INDEX idx_calendar_event_targets_unique_global",
        "ON calendar_event_targets (event_id, audience_type)",
        "WHERE grade_level_id IS NULL AND class_room_id IS NULL",
        "CREATE UNIQUE INDEX idx_calendar_event_targets_unique_grade",
        "ON calendar_event_targets (event_id, audience_type, grade_level_id)",
        "WHERE grade_level_id IS NOT NULL AND class_room_id IS NULL",
        "CREATE UNIQUE INDEX idx_calendar_event_targets_unique_class",
        "ON calendar_event_targets (event_id, audience_type, class_room_id)",
        "WHERE class_room_id IS NOT NULL AND grade_level_id IS NULL",
        "CREATE TRIGGER update_calendar_event_targets_updated_at",
        "CREATE TRIGGER update_calendar_event_reminders_updated_at",
        "calendar.read.school",
        "calendar.manage.school",
    ] {
        assert!(
            migration.contains(required),
            "calendar migration must contain `{required}`"
        );
    }

    for source in [&backend_registry, &frontend_registry] {
        assert!(source.contains("calendar.read.school"));
        assert!(source.contains("calendar.manage.school"));
    }

    assert!(modules_root.contains("pub mod calendar;"));
    assert!(app.contains("\"/api/calendar\""));
    assert!(app.contains("modules::calendar::calendar_routes()"));
    assert!(app.contains("\"/api/me/calendar/events\""));
    assert!(app.contains("\"/api/parent/students/{student_id}/calendar/events\""));
    assert!(app.contains("\"/api/public/calendar/events\""));
    assert!(main_source.contains("process_due_calendar_reminders_for_all_tenants"));

    for required in [
        "CREATE TABLE calendar_tags",
        "CREATE TABLE calendar_event_tags",
        "REFERENCES calendar_events(id) ON DELETE CASCADE",
        "REFERENCES calendar_tags(id) ON DELETE CASCADE",
        "PRIMARY KEY (event_id, tag_id)",
        "idx_calendar_tags_name_unique",
    ] {
        assert!(
            tags_migration.contains(required),
            "calendar tags migration must contain `{required}`"
        );
    }

    assert!(migration.contains("REFERENCES calendar_categories(id) ON DELETE SET NULL"));
}

#[test]
fn calendar_handlers_stay_thin_and_services_own_sql() {
    let models = strip_comments(&read_source(
        manifest_dir().join("src/modules/calendar/models.rs"),
    ));
    let handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/calendar/handlers.rs"),
    ));
    let services = strip_comments(
        &[
            "src/modules/calendar/services.rs",
            "src/modules/calendar/services/categories_and_tags.rs",
            "src/modules/calendar/services/events.rs",
            "src/modules/calendar/services/notifications.rs",
            "src/modules/calendar/services/reminders.rs",
            "src/modules/calendar/services/shared.rs",
            "src/modules/calendar/services/visibility.rs",
        ]
        .into_iter()
        .map(|path| read_source(manifest_dir().join(path)))
        .collect::<Vec<_>>()
        .join("\n"),
    );
    let routes = strip_comments(&read_source(manifest_dir().join("src/modules/calendar.rs")));
    let parent_handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/parents/handlers.rs"),
    ));
    let parent_services = strip_comments(&read_source(
        manifest_dir().join("src/modules/parents/services.rs"),
    ));

    assert!(handlers.contains("actor_tenant_context_from_session(&state, &session).await?"));
    assert!(handlers.contains("codes::CALENDAR_READ_SCHOOL"));
    assert!(handlers.contains("codes::CALENDAR_MANAGE_SCHOOL"));
    assert!(!handlers.contains("sqlx::query"));
    assert!(!handlers.contains(".fetch_"));
    assert!(!handlers.contains(".execute("));
    assert!(!handlers.contains(".begin("));
    assert!(!handlers.contains("QueryBuilder"));
    assert!(!handlers.contains("PgPool"));
    assert!(models.contains("pub struct CalendarPublicEvent"));
    assert!(services.contains("Result<Vec<CalendarPublicEvent>, AppError>"));
    assert!(services.contains("sqlx::query"));
    assert!(services.contains("CalendarEvent"));
    assert!(services.contains("resolve_event_recipient_user_ids"));
    assert!(services.contains("process_due_reminders"));
    assert!(services.contains("DELETE FROM calendar_categories WHERE id = $1"));
    assert!(services.contains("DELETE FROM calendar_tags WHERE id = $1"));
    assert!(services.contains("replace_event_tags"));
    assert!(routes.contains("\"/tags\""));
    assert!(routes.contains("\"/tags/{id}\""));
    assert!(parent_handlers.contains("get_child_calendar_events"));
    assert!(!parent_handlers.contains("sqlx::query"));
    assert!(!parent_handlers.contains(".fetch_"));
    assert!(!parent_handlers.contains(".execute("));
    assert!(!parent_handlers.contains(".begin("));
    assert!(!parent_handlers.contains("QueryBuilder"));
    assert!(!parent_handlers.contains("PgPool"));
    assert!(parent_services.contains("JOIN users u ON u.id = student_parents.student_user_id"));
    assert!(parent_services.contains("u.user_type = 'student'"));
    assert!(parent_services.contains("u.status = 'active'"));
}

#[test]
fn question_bank_authorization_lives_in_policy_and_supports_team_teaching() {
    let policies_root = strip_comments(&read_source(manifest_dir().join("src/policies.rs")));
    let policy = strip_comments(&read_source(
        manifest_dir().join("src/policies/question_bank_access_policy.rs"),
    ));
    let handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/question_bank/handlers.rs"),
    ));
    let services = strip_comments(&read_source(
        manifest_dir().join("src/modules/question_bank/services.rs"),
    ));

    assert!(policies_root.contains("pub mod question_bank_access_policy;"));
    assert!(handlers.contains("question_bank_access_policy::resolve_access"));
    assert!(services.contains("question_bank_access_policy::require_question_read_access"));
    assert!(services.contains("question_bank_access_policy::require_question_manage_access"));
    assert!(services.contains("question_bank_access_policy::require_subject_create_access"));
    assert!(policy.contains("FROM course_offering_details"));
    assert!(policy.contains("JOIN learning_groups"));
    assert!(policy.contains("JOIN learning_group_teachers"));
    assert!(policy.contains("detail.subject_id = $1"));
    assert!(policy.contains("teacher.teacher_id = $2"));
    assert!(!policy.contains("classroom_course_instructors"));
    assert!(!handlers.contains("actor.has_permission("));
    assert!(!handlers.contains("actor.require_permission("));
    assert!(!services.contains("actor.has_permission("));
    assert!(!services.contains("actor.require_permission("));
}

#[test]
fn question_bank_subject_contract_and_temporary_file_lifecycle_are_explicit() {
    let migration = read_source(
        manifest_dir()
            .join("migrations")
            .join("024_question_bank_subject_contract_and_search.sql"),
    );
    let registry = strip_comments(&read_source(
        manifest_dir().join("src/modules/files/purpose_registry.rs"),
    ));
    let repository = strip_comments(&read_source(
        manifest_dir().join("src/modules/files/repository.rs"),
    ));
    let cleaner = strip_comments(&read_source(manifest_dir().join("src/services/cleaner.rs")));
    let routes = strip_comments(&read_source(
        manifest_dir().join("src/modules/question_bank.rs"),
    ));

    assert!(migration.contains("CHECK (subject_id IS NOT NULL) NOT VALID"));
    assert!(migration.contains("academic_question_bank_questions_subject_id_fkey"));
    assert!(migration.contains("ON DELETE RESTRICT"));
    assert!(routes.contains(".route(\"/options\", get(handlers::list_options))"));
    assert!(registry.contains("FilePurpose::QuestionBankImage"));
    assert!(registry.contains("retention_class: RetentionClass::Temporary"));
    assert!(repository.contains("INTERVAL '24 hours'"));
    assert!(cleaner.contains("request_expired_file_deletions"));
    assert!(cleaner.contains("retention_class = 'temporary'"));
}

#[test]
fn question_bank_rich_content_is_versioned_typed_and_searchable() {
    let migration = read_source(
        manifest_dir()
            .join("migrations")
            .join("025_question_bank_rich_document.sql"),
    );
    let models = strip_comments(&read_source(
        manifest_dir().join("src/modules/question_bank/models.rs"),
    ));
    let services = strip_comments(&read_source(
        manifest_dir().join("src/modules/question_bank/services.rs"),
    ));

    assert!(models.contains("pub struct RichContent"));
    assert!(models.contains("pub schema_version: u16"));
    assert!(models.contains("pub document: RichDocument"));
    assert!(models.contains("pub stem_content: Json<RichContent>"));
    assert!(models.contains("pub content: Json<RichContent>"));
    assert!(!models.contains("serde_json::Value"));
    assert!(migration.contains("ADD COLUMN search_text TEXT NOT NULL"));
    assert!(migration.contains("idx_question_bank_questions_search_trgm"));
    assert!(services.contains("q.search_text ILIKE"));
    assert!(!services.contains("q.stem_content::text ILIKE"));
}

#[test]
fn role_and_organization_system_flags_are_migration_owned() {
    let migration = read_source(
        manifest_dir()
            .join("migrations")
            .join("027_role_organization_system_flags.sql"),
    );
    let normalized = migration.split_whitespace().collect::<Vec<_>>().join(" ");

    assert!(normalized.contains("ALTER TABLE roles ADD COLUMN is_system"));
    assert!(normalized.contains("ALTER TABLE organization_units ADD COLUMN is_system"));
    assert!(normalized.contains("WHERE code = 'ADMIN'"));
    assert!(normalized.contains("WHERE code = 'SCHOOL'"));

    let models = read_source(manifest_dir().join("src/modules/staff/models.rs"));
    for response_model in ["pub struct Role {", "pub struct OrganizationUnit {"] {
        let definition = extract_braced_block(&models, response_model, false);
        assert!(
            definition.contains("pub is_system: bool"),
            "{response_model} must expose the protected-record flag"
        );
    }

    for request_model in [
        "pub struct CreateRoleRequest {",
        "pub struct UpdateRoleRequest {",
        "pub struct CreateOrganizationUnitRequest {",
        "pub struct UpdateOrganizationUnitRequest {",
    ] {
        let definition = extract_braced_block(&models, request_model, false);
        assert!(
            !definition.contains("is_system"),
            "{request_model} must not allow clients to change the protected-record flag"
        );
    }
}

#[test]
fn inactive_authorization_sources_are_filtered() {
    let permissions = read_source(manifest_dir().join("src/middleware/permission.rs"));
    let normalized_permissions = permissions.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalized_permissions.contains("JOIN roles r ON ur.role_id = r.id AND r.is_active = true")
    );
    assert!(normalized_permissions.contains(
        "JOIN organization_units ou ON om.organization_unit_id = ou.id AND ou.is_active = true"
    ));
    assert!(normalized_permissions.contains(
        "LEFT JOIN organization_units delegated_ou ON delegated_ou.id = opd.organization_unit_id"
    ));
    assert!(normalized_permissions
        .contains("opd.organization_unit_id IS NULL OR delegated_ou.is_active = true"));

    let auth = read_source(manifest_dir().join("src/modules/auth/services.rs"));
    let normalized_auth = auth.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(normalized_auth.contains("AND r.is_active = true"));

    let user_roles =
        read_source(manifest_dir().join("src/modules/staff/services/user_role_service.rs"));
    let normalized_user_roles = user_roles.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(normalized_user_roles.contains(
        "JOIN roles r ON ur.role_id = r.id AND r.is_active = true JOIN role_permissions"
    ));

    let resource_policy =
        read_source(manifest_dir().join("src/policies/resource_access_policy.rs"));
    assert!(
        resource_policy
            .matches("active_unit.is_active = true")
            .count()
            >= 4
    );

    let staff_service =
        read_source(manifest_dir().join("src/modules/staff/services/staff_service.rs"));
    assert!(staff_service.contains("active_actor_unit.is_active = true"));
    assert!(staff_service.contains("active_root.is_active = true"));
}

#[test]
fn auto_scheduler_backend_and_schema_are_removed_without_deleting_timetable_entries() {
    let router = read_source(manifest_dir().join("src/modules/academic.rs"));
    for removed in [
        "/scheduling/auto-schedule",
        "/scheduling/jobs",
        "/instructor-preferences",
        "/instructor-rooms",
        "/timetable/locked-slots",
        "/scheduling/instructors",
        "/scheduling/subjects",
        "/scheduling/settings",
        "/scheduling/classroom-courses",
        "/scheduling/rooms",
        "/scheduling/configuration",
    ] {
        assert!(
            !router.contains(removed),
            "removed auto-scheduler route must not remain: {removed}"
        );
    }

    for removed in [
        "src/modules/academic/handlers/scheduling.rs",
        "src/modules/academic/handlers/scheduling_config.rs",
        "src/modules/academic/models/scheduling.rs",
        "src/modules/academic/models/scheduling_config.rs",
        "src/modules/academic/services/scheduler.rs",
        "src/modules/academic/services/scheduler",
        "src/modules/academic/services/scheduler_data.rs",
        "src/modules/academic/services/scheduling_service.rs",
        "src/modules/academic/services/scheduling_config_service.rs",
        "src/modules/academic/services/scheduling_config_service_tests.rs",
    ] {
        assert!(
            !manifest_dir().join(removed).exists(),
            "removed auto-scheduler module must not remain: {removed}"
        );
    }

    let migration = read_source(manifest_dir().join("migrations/028_remove_auto_scheduler.sql"));
    let normalized = migration
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    assert!(normalized.contains("drop column scheduler_job_id"));
    assert!(normalized.contains("drop table timetable_scheduling_jobs"));
    assert!(!normalized.contains("delete from academic_timetable_entries"));
    assert!(!normalized.contains("truncate academic_timetable_entries"));
    assert!(!normalized.contains("drop table academic_timetable_entries"));
}

#[test]
fn file_platform_blocks_new_provider_coupling_and_locator_responses() {
    let tenant_object_prefix = Regex::new(r#"(?s)format!\s*\(\s*"school-[^"\n]*"#).unwrap();
    let response_struct = Regex::new(
        r"(?s)(?:pub\s+)?struct\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\{(?P<body>.*?)\n\}",
    )
    .unwrap();
    let response_field =
        Regex::new(r"(?m)^\s*(?:pub\s+)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*:").unwrap();
    let mut violations = BTreeSet::new();

    for file in backend_rs_files() {
        let file_name = relative(&file);
        if file_name == "src/modules/files/r2_storage_provider.rs" {
            continue;
        }

        let source = strip_comments(&read_source(&file));
        if source.contains("R2Client") || source.contains("services::r2_client") {
            violations.insert(format!("{file_name}: direct R2 client use"));
        }
        if tenant_object_prefix.is_match(&source) {
            violations.insert(format!("{file_name}: constructs a tenant object prefix"));
        }

        for response in response_struct.captures_iter(&source) {
            let response_name = response.name("name").unwrap().as_str();
            let is_file_response = response_name.ends_with("Response")
                || response_name.ends_with("Data")
                || response_name.ends_with("Dto")
                || matches!(response_name, "QuestionFile" | "ApplicationDocument");
            if !is_file_response {
                continue;
            }

            let body = response.name("body").unwrap().as_str();
            for field in response_field.captures_iter(body) {
                let field_name = field.name("name").unwrap().as_str();
                let is_storage_locator = matches!(
                    field_name,
                    "storage_path" | "thumbnail_path" | "bucket" | "object_key"
                );
                let is_provider_url = field_name == "url" || field_name.ends_with("_url");
                let is_file_platform_delivery_url = file_name == "src/modules/files/models.rs"
                    && matches!(
                        response_name,
                        "FileDownloadGrantResponse" | "PublicFileDeliveryResponse"
                    )
                    && field_name == "url";
                if (is_storage_locator || is_provider_url) && !is_file_platform_delivery_url {
                    violations.insert(format!("{file_name}: {response_name} exposes {field_name}"));
                }
            }
        }
    }

    assert_eq!(
        violations,
        BTreeSet::new(),
        "business modules must use File Platform IDs without provider coupling or API locators"
    );
}

#[test]
fn file_platform_domain_relationships_own_attachment_and_lifecycle_deletion() {
    let migration = strip_comments(&read_source(
        manifest_dir().join("migrations/031_file_platform_domain_references.sql"),
    ));
    assert!(migration.contains("profile_image_file_id UUID REFERENCES files(id)"));
    assert!(migration.contains("image_file_id UUID REFERENCES files(id)"));

    let file_handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/files/handlers.rs"),
    ));
    assert!(
        file_handlers.contains("authorize_school_font_delete_guard"),
        "school-font cleanup must hold its staging authorization through lifecycle deletion"
    );

    for relative_path in [
        "src/modules/school/services.rs",
        "src/modules/admission/services/application_service.rs",
        "src/modules/question_bank/services.rs",
        "src/modules/auth/services.rs",
        "src/modules/staff/services/staff_service.rs",
        "src/modules/achievement/services.rs",
    ] {
        let source = strip_comments(&read_source(manifest_dir().join(relative_path)));
        assert!(
            source.contains("retention_class = 'standard'"),
            "{relative_path} must finalize temporary uploads in its relationship transaction"
        );
        assert!(
            !source.contains("DELETE FROM files"),
            "{relative_path} must not bypass File Platform lifecycle deletion"
        );
    }

    for relative_path in [
        "src/modules/school/handlers.rs",
        "src/modules/admission/handlers/applications.rs",
        "src/modules/admission/handlers/portal.rs",
        "src/modules/admission/handlers/rounds.rs",
        "src/modules/question_bank/handlers.rs",
        "src/modules/auth/handlers.rs",
        "src/modules/staff/handlers/staff.rs",
        "src/modules/achievement/handlers.rs",
    ] {
        let source = strip_comments(&read_source(manifest_dir().join(relative_path)));
        assert!(
            source.contains("request_deletions"),
            "{relative_path} must route detached/replaced files through durable deletion"
        );
    }
}

#[test]
fn file_platform_runtime_uses_only_canonical_schema_columns() {
    for relative_path in [
        "src/services/cleaner.rs",
        "src/modules/files/repository.rs",
        "src/modules/school/services.rs",
        "src/modules/admission/services/application_service.rs",
        "src/modules/admission/services/portal_service.rs",
        "src/modules/question_bank/services.rs",
        "src/modules/auth/services.rs",
        "src/modules/staff/services/staff_service.rs",
        "src/modules/achievement/services.rs",
    ] {
        let source = strip_comments(&read_source(manifest_dir().join(relative_path)));
        for legacy_pattern in [
            "is_temporary",
            "profile_image_url",
            "image_path",
            "legacy_file_type",
            "f.original_filename",
            "f.file_size",
            "f.mime_type",
            "files.user_id",
            "SELECT id, user_id, purpose_code",
        ] {
            assert!(
                !source.contains(legacy_pattern),
                "{relative_path} still depends on removed File Platform compatibility field {legacy_pattern}"
            );
        }
    }
}

#[test]
fn file_platform_object_keys_can_only_be_constructed_by_the_purpose_registry() {
    let platform_types = read_source(manifest_dir().join("src/modules/files/platform_types.rs"));
    let registry_path = manifest_dir().join("src/modules/files/purpose_registry.rs");
    let registry = read_source(&registry_path);

    assert!(
        !platform_types.contains("struct ObjectKey"),
        "platform types must not expose raw object-key construction"
    );
    assert!(
        registry.contains("pub struct ObjectKey(String, StorageClass);"),
        "purpose registry must own the private raw object-key constructor and storage class"
    );

    let storage_provider =
        read_source(manifest_dir().join("src/modules/files/storage_provider.rs"));
    assert!(
        !storage_provider.contains("pub storage_class: StorageClass"),
        "stored objects must derive storage class from registry-created object keys"
    );
    assert!(
        storage_provider.contains("self.object_key.storage_class()"),
        "stored objects must retain the storage class carried by their object key"
    );

    for file in backend_rs_files() {
        if file == registry_path {
            continue;
        }
        assert!(
            !read_source(&file).contains("ObjectKey::new"),
            "{} bypasses purpose-registry object-key construction",
            relative(&file)
        );
    }
}

#[test]
fn file_platform_derivatives_require_the_validated_payload_boundary() {
    let inspector = read_source(manifest_dir().join("src/modules/files/file_inspector.rs"));
    let processor = read_source(manifest_dir().join("src/utils/file_processor.rs"));
    let platform_service =
        read_source(manifest_dir().join("src/modules/files/platform_service.rs"));

    assert!(
        inspector.contains("pub struct ValidatedFile<'a>"),
        "inspection must bind validation metadata to a borrowed payload"
    );
    assert!(
        processor.contains("pub fn decode_inspected_image(")
            && processor.contains("validated: &ValidatedFile"),
        "derivative decoding must receive the validated payload instead of separate bytes and metadata"
    );
    assert!(
        platform_service.contains("inspect_file(")
            && platform_service.contains("ImageProcessor::decode_inspected_image("),
        "File Platform derivative paths must create and consume a validated payload"
    );

    for file in backend_rs_files() {
        let file_name = relative(&file);
        if file_name == "src/modules/files/file_inspector.rs" {
            continue;
        }
        let source = strip_comments(&read_source(&file));
        assert!(
            !source.contains("image::load_from_memory") && !source.contains("ImageReader"),
            "{file_name} bypasses the validated image decoder boundary"
        );
    }
}

#[test]
fn academic_core_one_time_cutover_tools_are_retired_after_cleanup() {
    assert!(!manifest_dir()
        .join("src/bin/preflight_academic_core.rs")
        .exists());
    assert!(!manifest_dir()
        .join("src/modules/academic/cutover_preflight.rs")
        .exists());

    let academic = read_source(manifest_dir().join("src/modules/academic.rs"));
    assert!(academic.contains("#[cfg(test)]\npub mod cutover_test_preflight;"));
    assert!(academic.contains("#[cfg(test)]\npub mod cutover_test_support;"));

    let app = read_source(manifest_dir().join("src/app.rs"));
    let reconciliation = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/reconciliation.rs"),
    ));
    assert!(!app.contains("/internal/academic-core/reconcile-all"));
    assert!(reconciliation.contains("read_academic_core_cleanup_audit"));
    assert!(reconciliation.contains("academic-core-v1-cleanup"));
    assert!(!reconciliation.contains("academic_core_entity_map"));
}

#[test]
fn academic_core_041_migration_uses_stable_identity_and_non_pii_audits() {
    let migration = strip_comments(&read_source(
        manifest_dir().join("migrations/041_academic_core_catalog.sql"),
    ));

    assert!(migration.contains("5c33b984-10df-58db-bf80-62dbc4a03d1b"));
    assert!(migration.contains("CREATE OR REPLACE FUNCTION academic_normalize_identity"));
    assert!(migration.contains("CREATE OR REPLACE FUNCTION academic_assert_version_range"));
    assert!(migration.contains("CREATE TABLE academic_core_cutover_audits"));
    assert!(migration.contains("'academic-core-v1'"));
    assert!(migration.contains("sha256("));
    assert!(!migration.contains("national_id"));
    assert!(!migration.contains("DROP TABLE"));
    assert!(!migration.contains("CREATE EXTENSION"));
}

#[test]
fn academic_core_042_migration_enforces_delivery_context_without_pii_audits() {
    let migration = strip_comments(&read_source(
        manifest_dir().join("migrations/042_academic_delivery_backfill.sql"),
    ));

    for required in [
        "CREATE TABLE student_academic_years",
        "CREATE TABLE homeroom_placements",
        "CREATE TABLE learning_offerings",
        "CREATE TABLE course_offering_details",
        "CREATE TABLE activity_offering_details",
        "CREATE TABLE learning_groups",
        "CREATE TABLE learning_group_students",
        "CREATE TABLE learning_results",
        "CREATE TABLE academic_core_entity_map",
        "learning_groups_offering_context_fkey",
        "learning_group_homerooms_homeroom_context_fkey",
        "ACADEMIC_CORE_OFFERING_SUBTYPE_MISMATCH",
        "ACADEMIC_CORE_PUBLISHED_OFFERING_IMMUTABLE",
        "5c33b984-10df-58db-bf80-62dbc4a03d1b",
        "'academic-core-v1'",
        "sha256(",
    ] {
        assert!(
            migration.contains(required),
            "migration 042 must retain the delivery invariant: {required}"
        );
    }

    assert!(!migration.contains("national_id"));
    assert!(!migration.contains("DROP TABLE"));
    assert!(!migration.contains("CREATE EXTENSION"));
    assert!(!migration.contains("REAL"));
    assert!(!migration.contains("DOUBLE PRECISION"));
}

#[test]
fn academic_core_043_migration_cuts_over_consumers_and_permissions_without_pii() {
    let migration = strip_comments(&read_source(
        manifest_dir().join("migrations/043_academic_consumer_cutover.sql"),
    ));

    for required in [
        "ALTER TABLE academic_assessment_plans RENAME TO course_assessment_plans",
        "course_assessment_plans_offering_context_fkey",
        "academic_timetable_entries_group_context_fkey",
        "CREATE OR REPLACE FUNCTION check_entry_move_no_instructor_conflict",
        "CREATE OR REPLACE FUNCTION check_instructor_no_double_book",
        "ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED",
        "academic_exam_schedule_items_plan_offering_context_fkey",
        "supervision_cycles_term_context_fkey",
        "ALTER TABLE permissions ADD COLUMN is_active",
        "academic_context.read.school",
        "learning_offering.manage.assigned",
        "academic_permission_cutover_map",
        "permission-context-delegation:",
        "ACADEMIC_CORE_043_PERMISSION_MAPPING_UNRESOLVED",
        "ACADEMIC_CORE_043_PERMISSION_PRINCIPAL_MISMATCH",
        "'academic-core-v1'",
        "sha256(",
    ] {
        assert!(
            migration.contains(required),
            "migration 043 must retain the consumer cutover invariant: {required}"
        );
    }

    assert!(!migration.contains("national_id"));
    assert!(!migration.contains("DROP TABLE"));
    assert!(!migration.contains("CREATE EXTENSION"));
    assert!(!migration.contains("REAL"));
    assert!(!migration.contains("DOUBLE PRECISION"));
}

#[test]
fn academic_core_044_exposes_the_clean_runtime_contract_and_removes_compatibility_columns() {
    let migration = strip_comments(&read_source(
        manifest_dir().join("migrations/044_academic_runtime_contract.sql"),
    ));

    for required in [
        "ALTER COLUMN legacy_term DROP NOT NULL",
        "ALTER COLUMN academic_year_id DROP NOT NULL",
        "subject_groups_row_version_check",
        "ADD COLUMN archived_at TIMESTAMPTZ",
        "study_programs_one_default_per_version",
        "curriculum_course_requirements_program_resource_key",
        "curriculum_activity_requirements_program_resource_key",
        "CREATE TABLE grade_level_progression_sets",
        "ON DELETE SET NULL",
        "DROP COLUMN legacy_classroom_course_id",
        "DROP COLUMN legacy_activity_slot_id",
        "DROP COLUMN legacy_period_id",
    ] {
        assert!(
            migration.contains(required),
            "migration 044 must retain the runtime invariant: {required}"
        );
    }

    assert!(!migration.contains("national_id"));
    assert!(!migration.contains("DROP TABLE"));
    assert!(!migration.contains("REAL"));
    assert!(!migration.contains("DOUBLE PRECISION"));
}

#[test]
fn academic_core_045_fails_closed_then_removes_the_exact_legacy_manifest() {
    let migration = strip_comments(&read_source(
        manifest_dir().join("migrations/045_academic_core_legacy_cleanup.sql"),
    ));

    for required in [
        "IN ACCESS EXCLUSIVE MODE",
        "ACADEMIC_CORE_045_RECONCILIATION_MARKER_MISSING",
        "ACADEMIC_CORE_045_RECONCILIATION_MARKER_INVALID",
        "ACADEMIC_CORE_045_MARKER_CHECKSUM_MISMATCH",
        "ACADEMIC_CORE_045_RECONCILIATION_STALE",
        "ACADEMIC_CORE_045_RECONCILIATION_FAILED",
        "ACADEMIC_CORE_045_PERMISSION_RECONCILIATION_FAILED",
        "DELETE FROM role_permissions",
        "DELETE FROM organization_permission_grants",
        "DELETE FROM organization_permission_delegations",
        "DELETE FROM permissions",
        "DROP TABLE activity_group_members",
        "DROP TABLE activity_group_instructors",
        "DROP TABLE activity_groups",
        "DROP TABLE activity_slot_classroom_assignments",
        "DROP TABLE activity_slot_classrooms",
        "DROP TABLE activity_slot_instructors",
        "DROP TABLE activity_slots",
        "DROP TABLE classroom_course_instructors",
        "DROP TABLE classroom_courses",
        "DROP TABLE student_class_enrollments",
        "DROP TABLE IF EXISTS classroom_course_preferred_rooms",
        "DROP TABLE academic_core_entity_map",
        "ALTER TABLE academic_years DROP COLUMN is_active",
        "ALTER TABLE academic_terms DROP COLUMN is_active, DROP COLUMN legacy_term",
        "ALTER TABLE grade_levels DROP COLUMN next_grade_level_id",
        "ALTER TABLE homerooms DROP COLUMN legacy_curriculum_version_id",
        "ALTER TABLE bell_schedule_periods DROP COLUMN academic_year_id",
        "ACADEMIC_CORE_045_CLEANUP_MANIFEST_REMAINS",
        "ACADEMIC_CORE_045_TARGET_MANIFEST_MISSING",
        "academic-core-v1-cleanup",
    ] {
        assert!(
            migration.contains(required),
            "migration 045 must retain the cleanup invariant: {required}"
        );
    }

    assert!(
        migration.find("ACADEMIC_CORE_045_PERMISSION_RECONCILIATION_FAILED")
            < migration.find("DELETE FROM role_permissions"),
        "legacy permission equivalence must be proven before grants are deleted"
    );
    assert!(!migration.contains("national_id"));
    assert!(!migration.contains("CREATE EXTENSION"));
    assert!(!migration.contains("REAL"));
    assert!(!migration.contains("DOUBLE PRECISION"));
    assert!(!migration.contains("DROP TABLE CASCADE"));
}

#[test]
fn academic_runtime_cannot_query_phase_b_legacy_schema() {
    let allowed_test_only_files = BTreeSet::from([
        "src/modules/academic/cutover_test_preflight.rs",
        "src/modules/academic/cutover_test_support.rs",
    ]);
    let forbidden = [
        "student_class_enrollments",
        "classroom_courses",
        "classroom_course_instructors",
        "classroom_course_preferred_rooms",
        "activity_slots",
        "activity_slot_classrooms",
        "activity_slot_classroom_assignments",
        "activity_slot_instructors",
        "activity_groups",
        "activity_group_instructors",
        "activity_group_members",
        "academic_core_entity_map",
        "next_grade_level_id",
        "legacy_curriculum_version_id",
        "legacy_term",
        "academic_semester_id",
        "legacy_classroom_course_id",
        "legacy_activity_slot_id",
        "study_plan_id",
        "class_room_id",
    ];

    let mut violations = Vec::new();
    for path in backend_rs_files() {
        let relative_path = relative(&path);
        if allowed_test_only_files.contains(relative_path.as_str()) {
            continue;
        }
        let source = strip_comments(&read_source(&path));
        for token in forbidden {
            if source.contains(token) {
                violations.push(format!("{relative_path}: legacy runtime token `{token}`"));
            }
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn academic_core_registers_only_clean_replacement_routes() {
    let core_routes = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/core.rs"),
    ));
    let aggregate_routes =
        strip_comments(&read_source(manifest_dir().join("src/modules/academic.rs")));

    for required in [
        "/context/options",
        "/years",
        "/terms",
        "/bell-schedules",
        "/grade-progressions",
        "/catalog/subjects",
        "/catalog/activities",
        "/curricula",
        "/study-programs/{id}/requirements",
        "/homerooms",
        "/student-years",
        "/placements/{id}/transfer",
    ] {
        assert!(
            core_routes.contains(required),
            "missing clean Academic Core route: {required}"
        );
    }

    assert!(aggregate_routes.contains("core::routes().merge("));
    for removed in [
        "\"/structure\"",
        "\"/levels\"",
        "\"/semesters\"",
        "\"/classrooms\"",
        "\"/enrollments\"",
        "\"/subjects\"",
        "\"/study-plans\"",
        "\"/periods\"",
        "\"/planning/courses\"",
        "\"/planning/classrooms/",
        "\"/activity-slots\"",
        "\"/activities\"",
    ] {
        assert!(
            !aggregate_routes.contains(removed),
            "removed legacy route is still registered: {removed}"
        );
    }
}

#[test]
fn learning_delivery_handlers_are_thin_policy_owned_and_signal_after_mutation() {
    let handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery/handlers.rs"),
    ));

    assert!(handlers.contains("actor_tenant_context_from_session(&state, &session).await?"));
    assert!(!handlers.contains("sqlx::query"));
    assert!(!handlers.contains(".fetch_"));
    assert!(!handlers.contains(".execute("));
    assert!(!handlers.contains(".begin("));

    for (handler_name, action) in [
        ("list_offerings", "OfferingAction::Read"),
        ("create_offering", "OfferingAction::Manage"),
        (
            "preview_offerings_from_curriculum",
            "OfferingAction::Manage",
        ),
        ("apply_offerings_from_curriculum", "OfferingAction::Manage"),
    ] {
        let handler =
            extract_braced_block(&handlers, &format!("pub async fn {handler_name}"), false);
        assert!(
            handler.contains("require_learning_offering_list_access"),
            "{handler_name} must use list access"
        );
        assert!(
            handler.contains(action),
            "{handler_name} must require {action}"
        );
    }

    for (handler_name, action) in [
        ("get_offering", "OfferingAction::Read"),
        ("update_offering", "OfferingAction::Manage"),
        ("publish_offering", "OfferingAction::Manage"),
        ("list_groups", "OfferingAction::Read"),
        ("create_group", "OfferingAction::Manage"),
    ] {
        let signature = if handler_name == "list_groups" {
            "pub async fn list_groups(".to_string()
        } else {
            format!("pub async fn {handler_name}")
        };
        let handler = extract_braced_block(&handlers, &signature, false);
        assert!(
            handler.contains("require_learning_offering_access"),
            "{handler_name} must use resource access"
        );
        assert!(
            handler.contains(action),
            "{handler_name} must require {action}"
        );
    }

    for (handler_name, action) in [
        ("get_group", "OfferingAction::Read"),
        ("update_group", "OfferingAction::Manage"),
        ("list_group_homerooms", "OfferingAction::Read"),
        ("replace_group_homerooms", "OfferingAction::Manage"),
        ("list_group_teachers", "OfferingAction::Read"),
        ("replace_group_teachers", "OfferingAction::Manage"),
        ("preview_group_roster", "OfferingAction::Manage"),
        ("apply_group_roster", "OfferingAction::Manage"),
        ("publish_group_roster", "OfferingAction::Manage"),
    ] {
        let handler =
            extract_braced_block(&handlers, &format!("pub async fn {handler_name}"), false);
        assert!(handler.contains("require_learning_group_access"));
        assert!(handler.contains(action));
    }

    for handler_name in [
        "create_offering",
        "apply_offerings_from_curriculum",
        "update_offering",
        "publish_offering",
        "create_group",
        "update_group",
        "replace_group_homerooms",
        "replace_group_teachers",
        "apply_group_roster",
        "publish_group_roster",
    ] {
        let handler =
            extract_braced_block(&handlers, &format!("pub async fn {handler_name}"), false);
        assert!(
            handler.contains("signal_delivery_changed") || handler.contains("signal_group_changed"),
            "{handler_name} must emit a bounded delivery invalidation after success"
        );
    }
}

#[test]
fn learning_delivery_registers_the_canonical_offering_group_and_roster_routes() {
    let routes = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery.rs"),
    ));
    let aggregate_routes =
        strip_comments(&read_source(manifest_dir().join("src/modules/academic.rs")));

    for required in [
        "\"/offerings\"",
        "\"/offerings/preview-from-curriculum\"",
        "\"/offerings/apply-from-curriculum\"",
        "\"/offerings/{id}\"",
        "\"/offerings/{id}/publish\"",
        "\"/offerings/{id}/groups\"",
        "\"/learning-groups/{id}\"",
        "\"/learning-groups/{id}/homerooms\"",
        "\"/learning-groups/{id}/teachers\"",
        "\"/learning-groups/{id}/roster\"",
        "\"/learning-groups/{id}/roster/publish\"",
    ] {
        assert!(
            routes.contains(required),
            "missing delivery route: {required}"
        );
    }
    assert!(aggregate_routes.contains("delivery::routes()"));
}

#[test]
fn learning_delivery_contract_is_strictly_tagged_idempotent_and_pii_safe() {
    let models = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery/models.rs"),
    ));
    let websockets = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/websockets.rs"),
    ));
    let signal = extract_braced_block(&websockets, "LearningDeliveryChanged", false);

    assert!(models.contains("#[serde(tag = \"kind\", rename_all = \"snake_case\")]"));
    assert!(models.contains("Course(CreateCourseOfferingRequest)"));
    assert!(models.contains("Activity(CreateActivityOfferingRequest)"));
    assert!(models.contains("pub struct PublishRosterRequest"));
    assert!(models.contains("pub idempotency_key: Uuid"));

    for required in [
        "academic_term_id: Uuid",
        "learning_offering_id: Uuid",
        "learning_group_id: Option<Uuid>",
        "revision: i64",
    ] {
        assert!(
            signal.contains(required),
            "delivery signal missing {required}"
        );
    }
    for forbidden in ["student", "roster", "national_id", "email", "phone"] {
        assert!(
            !signal.contains(forbidden),
            "delivery signal must not contain {forbidden}"
        );
    }
}

#[test]
fn academic_runtime_contract_supports_delivery_idempotency_without_plaintext_pii() {
    let migration = strip_comments(&read_source(
        manifest_dir().join("migrations/044_academic_runtime_contract.sql"),
    ));

    for required in [
        "learning_offerings_publish_idempotency_key",
        "learning_offering_targets_grade_program_key",
        "roster_source_hash CHAR(64)",
        "learning_groups_roster_publish_idempotency_key",
        "CREATE TABLE learning_delivery_apply_runs",
        "request_hash CHAR(64)",
        "source_hash CHAR(64)",
        "legacyGradingPolicy",
        "legacyAttendanceRequirement",
        "'policyCode', 'legacy_migrated'",
        "'requireTeacherConfirmation', true",
    ] {
        assert!(
            migration.contains(required),
            "delivery invariant missing: {required}"
        );
    }
    for forbidden in ["national_id", "student_name", "email", "phone"] {
        assert!(!migration.contains(forbidden));
    }
}

#[test]
fn converted_academic_consumers_cannot_reintroduce_legacy_runtime_identity() {
    let mut runtime_files = vec![
        manifest_dir().join("src/modules/academic.rs"),
        manifest_dir().join("src/modules/academic/handlers/assessment.rs"),
        manifest_dir().join("src/modules/academic/handlers/exam_schedule.rs"),
        manifest_dir().join("src/modules/academic/handlers/timetable.rs"),
        manifest_dir().join("src/modules/academic/handlers/timetable_templates.rs"),
        manifest_dir().join("src/modules/academic/models/assessment.rs"),
        manifest_dir().join("src/modules/academic/models/exam_schedule.rs"),
        manifest_dir().join("src/modules/academic/models/timetable.rs"),
        manifest_dir().join("src/modules/academic/services/assessment_service.rs"),
        manifest_dir().join("src/modules/academic/services/daily_teaching_service.rs"),
        manifest_dir().join("src/modules/academic/services/timetable_realtime_service.rs"),
        manifest_dir().join("src/modules/academic/services/timetable_service.rs"),
        manifest_dir().join("src/modules/academic/services/timetable_template_service.rs"),
        manifest_dir().join("src/modules/academic/websockets.rs"),
    ];
    runtime_files.extend(list_files(
        manifest_dir().join("src/modules/academic/services/exam_schedule_service"),
        |path| {
            path.extension().and_then(|extension| extension.to_str()) == Some("rs")
                && path.file_name().and_then(|name| name.to_str()) != Some("tests.rs")
        },
    ));

    let forbidden = [
        "academic_semesters",
        "class_rooms",
        "classroom_courses",
        "student_class_enrollments",
        "activity_catalog",
        "study_plans",
        "study_plan_versions",
        "academic_assessment_plans",
        "academic_semester_id",
        "classroom_course_id",
        "class_room_id",
    ];
    let inferred_context = Regex::new(
        r"(?is)(academic_(?:years|terms)[^;]{0,400}is_active\s*=\s*true|is_active\s*=\s*true[^;]{0,400}academic_(?:years|terms))",
    )
    .unwrap();

    for path in runtime_files {
        let source = strip_comments(&read_source(&path));
        let path = relative(&path);
        for token in forbidden {
            assert!(
                !source.contains(token),
                "converted academic runtime {path} reintroduced `{token}`"
            );
        }
        assert!(
            !inferred_context.is_match(&source),
            "converted academic runtime {path} inferred an active year or term inside SQL"
        );
    }
}

#[test]
fn cross_module_academic_consumers_use_only_canonical_runtime_identity() {
    let mut runtime_files = vec![
        manifest_dir().join("src/bin/seed_sandbox.rs"),
        manifest_dir().join("src/modules/admission/models/rounds.rs"),
        manifest_dir().join("src/modules/admission/services/application_service.rs"),
        manifest_dir().join("src/modules/admission/services/portal_service.rs"),
        manifest_dir().join("src/modules/admission/services/round_service.rs"),
        manifest_dir().join("src/modules/admission/services/selection_service.rs"),
        manifest_dir().join("src/modules/academic/reconciliation.rs"),
        manifest_dir().join("src/modules/calendar/models.rs"),
        manifest_dir().join("src/modules/calendar/services/events.rs"),
        manifest_dir().join("src/modules/calendar/services/notifications.rs"),
        manifest_dir().join("src/modules/calendar/services/shared.rs"),
        manifest_dir().join("src/modules/calendar/services/visibility.rs"),
        manifest_dir().join("src/modules/lookup/handlers.rs"),
        manifest_dir().join("src/modules/lookup/models.rs"),
        manifest_dir().join("src/modules/lookup/services.rs"),
        manifest_dir().join("src/modules/parents/handlers.rs"),
        manifest_dir().join("src/modules/parents/models.rs"),
        manifest_dir().join("src/modules/parents/services.rs"),
        manifest_dir().join("src/modules/question_bank/models.rs"),
        manifest_dir().join("src/modules/question_bank/services.rs"),
        manifest_dir().join("src/modules/staff/handlers/staff.rs"),
        manifest_dir().join("src/modules/staff/models.rs"),
        manifest_dir().join("src/modules/staff/services/dashboard_service.rs"),
        manifest_dir().join("src/modules/staff/services/staff_service.rs"),
        manifest_dir().join("src/modules/students/handlers.rs"),
        manifest_dir().join("src/modules/students/models.rs"),
        manifest_dir().join("src/modules/students/services.rs"),
        manifest_dir().join("src/modules/supervision/handlers.rs"),
        manifest_dir().join("src/modules/supervision/models.rs"),
        manifest_dir().join("src/policies/question_bank_access_policy.rs"),
        manifest_dir().join("src/policies/student_access_policy.rs"),
    ];
    runtime_files.extend(list_files(
        manifest_dir().join("src/modules/supervision/services"),
        |path| path.extension().and_then(|extension| extension.to_str()) == Some("rs"),
    ));

    let forbidden = [
        "academic_semesters",
        "class_rooms",
        "classroom_courses",
        "student_class_enrollments",
        "activity_catalog",
        "study_plans",
        "study_plan_versions",
        "academic_assessment_plans",
        "academic_semester_id",
        "classroom_course_id",
        "class_room_id",
        "study_plan_id",
    ];
    let inferred_context = Regex::new(
        r"(?is)(academic_(?:years|terms)[^;]{0,400}is_active\s*=\s*true|is_active\s*=\s*true[^;]{0,400}academic_(?:years|terms))",
    )
    .unwrap();

    let mut violations = Vec::new();
    for path in runtime_files {
        let source = strip_comments(&read_source(&path));
        let path = relative(&path);
        for token in forbidden {
            if source.contains(token) {
                violations.push(format!("{path}: legacy runtime token `{token}`"));
            }
        }
        if inferred_context.is_match(&source) {
            violations.push(format!(
                "{path}: active academic context must be selected by the caller"
            ));
        }
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn academic_list_hydration_does_not_issue_one_query_per_response_item() {
    let offerings = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery/services/offerings.rs"),
    ));
    let timetable = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/timetable_service.rs"),
    ));
    let offering_n_plus_one =
        Regex::new(r"(?s)for\s+row\s+in\s+rows\s*\{.{0,200}hydrate\(pool,\s*row\)\.await").unwrap();
    let timetable_n_plus_one =
        Regex::new(r"(?s)for\s+row\s+in\s+rows\s*\{.{0,200}hydrate_row\(pool,\s*row\)\.await")
            .unwrap();

    assert!(
        !offering_n_plus_one.is_match(&offerings),
        "learning-offering list hydration must fetch related rows in batches"
    );
    assert!(
        !timetable_n_plus_one.is_match(&timetable),
        "timetable list hydration must fetch instructors in batches"
    );
}

#[test]
fn learning_group_collection_hydrators_are_set_based() {
    let groups = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery/services/groups.rs"),
    ));
    let list_n_plus_one =
        Regex::new(r"(?s)for\s+row\s+in\s+rows\s*\{.{0,240}hydrate\(pool,\s*row\)\.await").unwrap();

    assert!(
        !list_n_plus_one.is_match(&groups),
        "learning-group collection reads must not call the single-row hydrator in a row loop"
    );
    assert!(
        groups.matches("hydrate_many(pool, rows).await").count() >= 2,
        "nested and term-scoped learning-group lists must share the bulk hydrator"
    );
    assert!(
        groups.matches("learning_group_id = ANY($1)").count() >= 3,
        "learning-group teachers, homerooms, and preferred rooms must load by the parent ID set"
    );
}

#[test]
fn academic_delivery_and_timetable_collection_reads_are_set_based() {
    let offerings = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery/services/offerings.rs"),
    ));
    let delivery_handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/delivery/handlers.rs"),
    ));
    let timetable = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/timetable_service.rs"),
    ));
    let templates = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/timetable_template_service.rs"),
    ));

    let preview = extract_braced_block(&offerings, "async fn build_curriculum_preview", false);
    assert!(
        preview.contains("load_existing_preview_offerings"),
        "curriculum preview must preload existing course and activity offerings once"
    );
    assert!(
        !Regex::new(r"(?s)for\s+row\s+in\s+rows\s*\{.*?fetch_optional")
            .unwrap()
            .is_match(&preview),
        "curriculum preview must not query an existing offering per requirement"
    );

    let apply_handler = extract_braced_block(
        &delivery_handlers,
        "pub async fn apply_offerings_from_curriculum",
        false,
    );
    assert!(apply_handler.contains("signal_descriptors"));
    assert!(!Regex::new(r"(?s)for\s+offering_id.*?offerings::get")
        .unwrap()
        .is_match(&apply_handler));

    for function_name in ["pub async fn create_batch", "pub async fn deactivate_batch"] {
        let body = extract_braced_block(&timetable, function_name, false);
        assert!(body.contains("get_entries(pool"), "{function_name}");
        assert!(!Regex::new(r"(?s)for\s+.*?get_entry\(pool")
            .unwrap()
            .is_match(&body));
    }
    let clear_timetable = extract_braced_block(&templates, "pub async fn clear_timetable", false);
    assert!(clear_timetable.contains("get_entries(pool"));
    assert!(!Regex::new(r"(?s)for\s+id\s+in\s+ids.*?get_entry\(pool")
        .unwrap()
        .is_match(&clear_timetable));

    let occupancy = extract_braced_block(&timetable, "pub async fn occupancy", false);
    assert!(occupancy.contains("load_relationship_indexes"));
    assert!(
        !Regex::new(r"(?s)for\s+entry\s+in\s+entries.*?effective_(?:homerooms|instructors)")
            .unwrap()
            .is_match(&occupancy)
    );

    let validate_moves = extract_braced_block(&timetable, "pub async fn validate_moves", false);
    assert!(validate_moves.contains("load_relationship_indexes"));
    assert!(
        !Regex::new(r"(?s)for\s+day\s+in\s+VALID_DAYS.*?find_conflicts\(")
            .unwrap()
            .is_match(&validate_moves)
    );
}

#[test]
fn supervision_collection_hydrators_are_set_based() {
    let templates = strip_comments(&read_source(
        manifest_dir().join("src/modules/supervision/services/templates.rs"),
    ));
    let observations = strip_comments(&read_source(
        manifest_dir().join("src/modules/supervision/services/observations.rs"),
    ));
    let template_list = extract_braced_block(&templates, "pub async fn list_templates", false);
    let observation_list =
        extract_braced_block(&observations, "pub async fn list_observations", false);

    assert!(
        !Regex::new(r"(?s)for\s+row\s+in\s+rows\s*\{.*?get_template\(pool")
            .unwrap()
            .is_match(&template_list),
        "supervision template lists must not call the detail hydrator per parent row"
    );
    assert!(
        template_list.contains("hydrate_templates(pool, rows).await"),
        "supervision template lists must share the set-based template hydrator"
    );
    assert!(
        !Regex::new(r"(?s)for\s+row\s+in\s+rows\s*\{.*?observation_from_row\(pool")
            .unwrap()
            .is_match(&observation_list),
        "supervision observation lists must not call the detail hydrator per parent row"
    );
    assert!(
        observation_list.contains("hydrate_observations(pool, rows).await"),
        "supervision observation lists must share the set-based observation hydrator"
    );
}
