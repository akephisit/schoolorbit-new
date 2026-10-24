//! Parse production module graphs rather than guessing test ownership from filenames.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use syn::parse::Parser;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, Item, Meta, Token};

fn cfg_enabled(meta: &Meta, api_docs: bool) -> bool {
    match meta {
        Meta::Path(path) => !path.is_ident("test"),
        Meta::NameValue(value) if value.path.is_ident("feature") => {
            matches!(&value.value, Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(feature) if api_docs && feature.value() == "api-docs"))
        }
        Meta::List(list) => {
            let entries = list
                .parse_args_with(syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated)
                .expect("valid cfg list");
            if list.path.is_ident("not") {
                assert_eq!(entries.len(), 1);
                !cfg_enabled(entries.first().unwrap(), api_docs)
            } else if list.path.is_ident("any") {
                entries.iter().any(|entry| cfg_enabled(entry, api_docs))
            } else if list.path.is_ident("all") {
                entries.iter().all(|entry| cfg_enabled(entry, api_docs))
            } else {
                true
            }
        }
        _ => true,
    }
}

fn active(attributes: &[Attribute], api_docs: bool) -> bool {
    attributes.iter().all(|attribute| {
        !attribute.path().is_ident("cfg")
            || cfg_enabled(
                &attribute.parse_args::<Meta>().expect("valid cfg"),
                api_docs,
            )
    })
}

fn item_attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

fn execution_method(name: &str) -> bool {
    matches!(
        name,
        "execute"
            | "execute_many"
            | "fetch"
            | "fetch_many"
            | "fetch_one"
            | "fetch_optional"
            | "fetch_all"
    )
}

// Bootstrap/schema inspection cannot depend on tenancy (tenancy itself owns migrations).
// These are function boundaries, not exemptions for all runtime code in an owner.
fn native_exception(file: &Path, function: &str) -> bool {
    let file = file.to_string_lossy().replace('\\', "/");
    (file.ends_with("crates/school-migrations/src/lib.rs")
        && matches!(
            function,
            "run_tenant_migrations" | "verify_tenant_schema" | "grant_runtime_privileges"
        ))
        || (file.ends_with("src/system/handlers/migration.rs") && function == "get_current_version")
        || (file.ends_with("crates/school-database/src/lib.rs") && execution_method(function))
}

fn type_contains(ty: &syn::Type, names: &[&str]) -> bool {
    struct Find<'a> {
        names: &'a [&'a str],
        found: bool,
    }
    impl<'ast> Visit<'ast> for Find<'_> {
        fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
            self.found |= path
                .path
                .segments
                .iter()
                .any(|part| self.names.iter().any(|name| part.ident == *name));
            visit::visit_type_path(self, path);
        }
    }
    let mut find = Find {
        names,
        found: false,
    };
    find.visit_type(ty);
    find.found
}

fn binding_names(pattern: &syn::Pat) -> BTreeSet<String> {
    struct Find(BTreeSet<String>);
    impl<'ast> Visit<'ast> for Find {
        fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
            self.0.insert(pattern.ident.to_string());
        }
    }
    let mut find = Find(BTreeSet::new());
    find.visit_pat(pattern);
    find.0
}

fn borrowed_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Path(path) if path.path.segments.len() == 1 => {
            path.path.get_ident().map(ToString::to_string)
        }
        Expr::Reference(reference) => borrowed_name(&reference.expr),
        Expr::Paren(paren) => borrowed_name(&paren.expr),
        Expr::Unary(unary) => borrowed_name(&unary.expr),
        Expr::MethodCall(call) if call.method == "clone" => borrowed_name(&call.receiver),
        _ => None,
    }
}

fn resource_initializer(expression: &Expr) -> bool {
    match expression {
        Expr::Await(value) => resource_initializer(&value.base),
        Expr::Try(value) => resource_initializer(&value.expr),
        Expr::Call(call) => {
            matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(|part| matches!(part.ident.to_string().as_str(), "observed_checkout_pool" | "begin_observed")))
        }
        Expr::MethodCall(call)
            if matches!(
                call.method.to_string().as_str(),
                "map_err" | "expect" | "unwrap"
            ) =>
        {
            resource_initializer(&call.receiver)
        }
        Expr::MethodCall(call) => matches!(
            call.method.to_string().as_str(),
            "checkout" | "acquire" | "begin"
        ),
        _ => false,
    }
}

// The lifecycle bootstrap cannot depend on tenancy without creating a cycle.
fn native_pool_boundary_exception(file: &Path, function: &str) -> bool {
    let path = file.to_string_lossy().replace('\\', "/");
    (path.ends_with("crates/school-migrations/src/lib.rs")
        && matches!(function, "sync_permissions" | "grant_runtime_privileges"))
        || (path.ends_with("crates/school-tenancy/src/tenant_pool.rs")
            && matches!(function, "observed_checkout_pool" | "begin_observed"))
        || (path.ends_with("crates/school-tenancy/src/pool_manager.rs")
            && function == "create_pool")
}

// Existing domain work entries own explicit transaction policy or intentionally
// split DB preparation from CPU/network work. Pure query/hydration helpers do not
// belong here: lend their caller's connection instead.
fn private_work_entry(file: &Path, function: &str) -> bool {
    let path = file.to_string_lossy().replace('\\', "/");
    let entries: &[(&str, &[&str])] = &[
        // Supply detail explicitly owns one repeatable-read/read-only metadata+history snapshot.
        (
            "school-administration/src/supply_catalog/services.rs",
            &["read_item"],
        ),
        // Assessment factories return guard+scope after authoritative locked validation.
        (
            "school-academic-assessment/src/gradebook/services.rs",
            &["begin_scope"],
        ),
        (
            "school-academic-assessment/src/learner_evaluation/services.rs",
            &["begin_subject"],
        ),
        // Atomic term/year activation is a domain transaction owner.
        (
            "school-academic-lifecycle/src/services/term_activation.rs",
            &["activate_term"],
        ),
        // Notification completion is the token-fenced acknowledgement work phase.
        ("school-notifications/src/dispatch.rs", &["finish_delivery"]),
        // Observation status owns locks+mutation+commit, then a fresh response read.
        (
            "school-supervision/src/services/observations.rs",
            &["set_observation_status"],
        ),
        (
            "school-academic-core/src/services/catalog.rs",
            &["replace_default_teachers"],
        ),
        (
            "school-academic-core/src/services/homeroom_roster.rs",
            &["read_transaction", "write_roster"],
        ),
        (
            "school-academic-lifecycle/src/services/promotion_execution.rs",
            &["record_failure", "finish_batch"],
        ),
        (
            "school-academic-results/src/services/activities.rs",
            &["begin_activity"],
        ),
        (
            "school-academic-results/src/services/course_preparation.rs",
            &["begin_course"],
        ),
        (
            "school-admission/src/services/application_service.rs",
            &["prepare_enrollment_password"],
        ),
        (
            "school-attendance/src/services/faces.rs",
            &["scan_context", "accept_scan"],
        ),
        (
            "school-calendar/src/delivery/reminders.rs",
            &[
                "fetch_due_reminder_candidates",
                "process_due_reminder_candidate",
            ],
        ),
        (
            "school-certificates/src/services/candidate_service.rs",
            &["create_single_candidate", "bulk_update_inner"],
        ),
        (
            "school-certificates/src/services/purge_service.rs",
            &["advance_one"],
        ),
        (
            "school-certificates/src/services/render_service.rs",
            &["issued_manifest_inner"],
        ),
        (
            "school-workflow/src/correspondence.rs",
            &["explicit_read", "act_impl"],
        ),
        ("school-workflow/src/document_export.rs", &["read_snapshot"]),
    ];
    entries
        .iter()
        .any(|(suffix, functions)| path.ends_with(suffix) && functions.contains(&function))
}

fn contains_query(body: &syn::Block) -> bool {
    struct Query(bool);
    impl<'ast> Visit<'ast> for Query {
        fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
            self.0 |= execution_method(&call.method.to_string());
            visit::visit_expr_method_call(self, call);
        }
    }
    let mut query = Query(false);
    query.visit_block(body);
    query.0
}

fn executor_aliases(file: &syn::File) -> BTreeSet<String> {
    struct Aliases(BTreeSet<String>);
    impl<'ast> Visit<'ast> for Aliases {
        fn visit_use_rename(&mut self, item: &'ast syn::UseRename) {
            if matches!(item.ident.to_string().as_str(), "Executor" | "PgExecutor") {
                self.0.insert(item.rename.to_string());
            }
        }
    }
    let mut aliases = Aliases(BTreeSet::from(["Executor".into(), "PgExecutor".into()]));
    aliases.visit_file(file);
    aliases.0
}

struct Scanner<'a> {
    file: PathBuf,
    module_dir: PathBuf,
    function: String,
    api_docs: bool,
    problems: &'a mut BTreeSet<String>,
    pools: BTreeSet<String>,
    resources: BTreeSet<String>,
    executor_aliases: BTreeSet<String>,
}

impl Scanner<'_> {
    fn problem(&mut self, message: &str) {
        self.problems.insert(format!(
            "{}::{}: {message}",
            self.file.display(),
            self.function
        ));
    }

    fn excepted(&self) -> bool {
        native_exception(&self.file, &self.function)
    }

    fn enter_parameters(&mut self, signature: &syn::Signature) {
        self.pools.clear();
        self.resources.clear();
        for argument in &signature.inputs {
            if let syn::FnArg::Typed(argument) = argument {
                if type_contains(&argument.ty, &["PgPool", "TenantPool", "Pool"]) {
                    self.pools.extend(binding_names(&argument.pat));
                }
                if type_contains(
                    &argument.ty,
                    &[
                        "PgConnection",
                        "Transaction",
                        "ObservedTransaction",
                        "ObservedConnection",
                    ],
                ) {
                    self.resources.extend(binding_names(&argument.pat));
                }
            }
        }
    }

    fn pool_argument(&self, expression: &Expr) -> bool {
        borrowed_name(expression).is_some_and(|name| self.pools.contains(&name))
    }

    fn signature(&mut self, signature: &syn::Signature) {
        if !signature.ident.to_string().ends_with("_on_connection") {
            return;
        }
        struct PoolType(bool);
        impl<'ast> Visit<'ast> for PoolType {
            fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
                self.0 |= path.path.segments.iter().any(|part| {
                    matches!(
                        part.ident.to_string().as_str(),
                        "PgPool" | "Pool" | "TenantPool"
                    )
                });
                visit::visit_type_path(self, path);
            }
        }
        let mut types = PoolType(false);
        for argument in &signature.inputs {
            if let syn::FnArg::Typed(argument) = argument {
                types.visit_type(&argument.ty);
            }
        }
        if types.0 {
            self.problem("connection helper accepts a pool capability");
        }
    }
}

impl<'ast> Visit<'ast> for Scanner<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if active(item_attributes(item), self.api_docs) {
            visit::visit_item(self, item);
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let previous = std::mem::replace(&mut self.function, item.sig.ident.to_string());
        let pools = self.pools.clone();
        let resources = self.resources.clone();
        self.enter_parameters(&item.sig);
        self.signature(&item.sig);
        if !matches!(item.vis, syn::Visibility::Public(_))
            && !self.pools.is_empty()
            && contains_query(&item.block)
            && !self.excepted()
            && !private_work_entry(&self.file, &self.function)
        {
            self.problem(
                "private query helper retains a pool capability; accept the caller's connection",
            );
        }
        visit::visit_item_fn(self, item);
        self.function = previous;
        self.pools = pools;
        self.resources = resources;
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if !active(&item.attrs, self.api_docs) {
            return;
        }
        let previous = std::mem::replace(&mut self.function, item.sig.ident.to_string());
        let pools = self.pools.clone();
        let resources = self.resources.clone();
        self.enter_parameters(&item.sig);
        self.signature(&item.sig);
        visit::visit_impl_item_fn(self, item);
        self.function = previous;
        self.pools = pools;
        self.resources = resources;
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        if !active(&item.attrs, self.api_docs) {
            return;
        }
        let previous = std::mem::replace(&mut self.function, item.sig.ident.to_string());
        let pools = self.pools.clone();
        let resources = self.resources.clone();
        self.enter_parameters(&item.sig);
        self.signature(&item.sig);
        visit::visit_trait_item_fn(self, item);
        self.function = previous;
        self.pools = pools;
        self.resources = resources;
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if let Some((_, items)) = &item.content {
            let previous = self.module_dir.clone();
            self.module_dir.push(item.ident.to_string());
            for child in items {
                self.visit_item(child);
            }
            self.module_dir = previous;
            return;
        }
        let explicit_path = item.attrs.iter().find_map(|attribute| {
            if !attribute.path().is_ident("path") {
                return None;
            }
            match &attribute.meta {
                Meta::NameValue(value) => match &value.value {
                    Expr::Lit(value) => match &value.lit {
                        syn::Lit::Str(path) => Some(path.value()),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            }
        });
        let path = explicit_path
            .map(|path| self.file.parent().unwrap().join(path))
            .unwrap_or_else(|| {
                let direct = self.module_dir.join(format!("{}.rs", item.ident));
                if direct.exists() {
                    direct
                } else {
                    self.module_dir.join(item.ident.to_string()).join("mod.rs")
                }
            });
        scan_file(&path, self.api_docs, self.problems);
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        let resources = self.resources.clone();
        let pools = self.pools.clone();
        visit::visit_block(self, block);
        self.resources = resources;
        self.pools = pools;
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        visit::visit_local(self, local);
        if let Some(initializer) = &local.init {
            if self.pool_argument(&initializer.expr) {
                self.pools.extend(binding_names(&local.pat));
            }
            if resource_initializer(&initializer.expr) {
                self.resources.extend(binding_names(&local.pat));
            }
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let method = call.method.to_string();
        if !self.excepted() && execution_method(&method) && !call.args.is_empty() {
            // Password admission executes a CPU closure. Visit its body as usual,
            // so a closure cannot hide a native query execution.
            let cpu_closure = method == "execute"
                && call
                    .args
                    .iter()
                    .any(|argument| matches!(argument, Expr::Closure(_)));
            if !cpu_closure {
                self.problem(
                    "native execution accepts an executor; use connection-bound DbSession",
                );
            }
        }
        if self.function.ends_with("_on_connection") && method == "acquire" {
            self.problem("connection helper acquires another connection");
        }
        if matches!(method.as_str(), "begin" | "acquire")
            && self.pool_argument(&call.receiver)
            && !native_pool_boundary_exception(&self.file, &self.function)
        {
            self.problem("native pool admission bypasses observed work boundary");
        }
        visit::visit_expr_method_call(self, call);
        if matches!(
            method.as_str(),
            "commit" | "rollback" | "return_to_pool" | "close"
        ) {
            if let Some(name) = borrowed_name(&call.receiver) {
                self.resources.remove(&name);
            }
        }
    }

    fn visit_trait_bound(&mut self, bound: &'ast syn::TraitBound) {
        if !self.excepted()
            && bound
                .path
                .segments
                .iter()
                .any(|part| self.executor_aliases.contains(&part.ident.to_string()))
        {
            self.problem("generic SQLx Executor capability bypasses a borrowed connection");
        }
        visit::visit_trait_bound(self, bound);
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        let executor = expression.path.segments.iter().any(|part| {
            self.executor_aliases.contains(&part.ident.to_string())
                || matches!(part.ident.to_string().as_str(), "sqlx" | "PgConnection")
        });
        if !self.excepted()
            && executor
            && expression
                .path
                .segments
                .last()
                .is_some_and(|part| execution_method(&part.ident.to_string()))
        {
            self.problem("native Executor function capability bypasses DbSession");
        }
        visit::visit_expr_path(self, expression);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let Expr::Path(path) = &*call.func {
            if let Some(name) = path.path.segments.last() {
                if !self.excepted()
                    && execution_method(&name.ident.to_string())
                    && path.path.segments.iter().any(|part| {
                        self.executor_aliases.contains(&part.ident.to_string())
                            || matches!(
                                part.ident.to_string().as_str(),
                                "sqlx" | "Executor" | "PgExecutor" | "PgConnection" | "Connection"
                            )
                    })
                {
                    self.problem("executor-style function execution bypasses DbSession");
                }
                if self.function.ends_with("_on_connection")
                    && matches!(
                        name.ident.to_string().as_str(),
                        "observed_checkout_pool" | "begin_observed"
                    )
                {
                    self.problem("connection helper opens a new pool work boundary");
                }
            }
        }
        let path_name = if let Expr::Path(path) = &*call.func {
            path.path.segments.last().map(|part| part.ident.to_string())
        } else {
            None
        };
        let transparent_repository_factory = matches!(&*call.func, Expr::Path(path) if path.path.segments.iter().any(|part| matches!(part.ident.to_string().as_str(), "SqlFileRepository" | "FileRepository")) && path.path.segments.last().is_some_and(|part| part.ident == "new"));
        // These functions consume the caller's owned transaction; their only pool
        // use before commit constructs an inert FileRepository. Final reads occur
        // after consuming commit. Keep the exact owning port scopes visible here.
        let consuming_transaction_port = matches!(
            path_name.as_deref(),
            Some("purge_in_transaction" | "reset_in_transaction")
        ) && self
            .file
            .to_string_lossy()
            .contains("crates/school-attendance/")
            && call.args.iter().any(|argument| {
                borrowed_name(argument).is_some_and(|name| self.resources.contains(&name))
            });
        let borrowed_transaction_factory = path_name.as_deref() == Some("prepare_delete")
            && self
                .file
                .to_string_lossy()
                .contains("school-administration");
        if !self.resources.is_empty()
            && call
                .args
                .iter()
                .any(|argument| self.pool_argument(argument))
            && !transparent_repository_factory
            && !borrowed_transaction_factory
            && !consuming_transaction_port
        {
            self.problem(&format!("pool helper {} called while a connection or transaction is held; propagate its connection", path_name.as_deref().unwrap_or("<call>")));
        }
        if path_name.as_deref() == Some("drop") {
            if let Some(name) = call.args.first().and_then(borrowed_name) {
                self.resources.remove(&name);
            }
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let name = mac
            .path
            .segments
            .last()
            .map(|name| name.ident.to_string())
            .unwrap_or_default();
        if matches!(name.as_str(), "join" | "try_join") {
            let parser = syn::punctuated::Punctuated::<Expr, Token![,]>::parse_terminated;
            // Tokio's optional scheduling modifier precedes the expression list.
            let tokens = mac.tokens.to_string();
            let tokens = tokens
                .strip_prefix("biased ;")
                .or_else(|| tokens.strip_prefix("fair ;"))
                .unwrap_or(&tokens);
            match parser.parse_str(tokens) {
                Ok(expressions) => {
                    for expression in &expressions {
                        self.visit_expr(expression);
                    }
                }
                Err(_) => self.problem("cannot inspect async join expressions"),
            }
        } else {
            let tokens = mac.tokens.to_string();
            if !self.excepted()
                && (tokens.contains("sqlx :: query")
                    || [
                        "execute",
                        "execute_many",
                        "fetch",
                        "fetch_many",
                        "fetch_one",
                        "fetch_optional",
                        "fetch_all",
                    ]
                    .iter()
                    .any(|method| tokens.contains(&format!(". {method} ("))))
            {
                self.problem(
                    "opaque macro contains database execution; expose an inspectable expression",
                );
            }
        }
    }
}

fn scan_file(path: &Path, api_docs: bool, problems: &mut BTreeSet<String>) {
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let syntax =
        syn::parse_file(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    if !active(&syntax.attrs, api_docs) {
        return;
    }
    let parent = path.parent().unwrap();
    let stem = path.file_stem().unwrap().to_str().unwrap();
    let module_dir = if matches!(stem, "lib" | "main" | "mod") {
        parent.to_path_buf()
    } else {
        parent.join(stem)
    };
    Scanner {
        file: path.to_path_buf(),
        module_dir,
        function: String::new(),
        api_docs,
        problems,
        pools: BTreeSet::new(),
        resources: BTreeSet::new(),
        executor_aliases: executor_aliases(&syntax),
    }
    .visit_file(&syntax);
}

#[test]
fn production_database_execution_uses_bound_sessions() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut roots = vec![root.join("src/main.rs")];
    for crate_dir in fs::read_dir(root.join("crates")).unwrap() {
        let directory = crate_dir.unwrap().path();
        let manifest = fs::read_to_string(directory.join("Cargo.toml")).unwrap();
        let normal = manifest
            .split("[dependencies]")
            .nth(1)
            .unwrap_or("")
            .split("[dev-dependencies]")
            .next()
            .unwrap_or("");
        assert!(
            !normal
                .lines()
                .any(|line| line.trim_start().starts_with("school-test-db =")),
            "fixture owner must remain dev-only: {}",
            directory.display()
        );
        if directory
            .file_name()
            .is_some_and(|name| name == "school-test-db")
        {
            continue;
        }
        let source = directory.join("src/lib.rs");
        if source.exists() {
            roots.push(source);
        }
    }
    for bin in fs::read_dir(root.join("src/bin")).unwrap() {
        let path = bin.unwrap().path();
        let source = if path.is_dir() {
            path.join("main.rs")
        } else {
            path
        };
        if source
            .extension()
            .is_some_and(|extension| extension == "rs")
            && source.exists()
        {
            roots.push(source);
        }
    }
    let mut problems = BTreeSet::new();
    for api_docs in [false, true] {
        for source in &roots {
            scan_file(source, api_docs, &mut problems);
        }
    }
    assert!(
        problems.is_empty(),
        "{}",
        problems.into_iter().collect::<Vec<_>>().join("\n")
    );
}

fn inspect_snippet(source: &str) -> BTreeSet<String> {
    let mut problems = BTreeSet::new();
    let syntax = syn::parse_file(source).unwrap();
    Scanner {
        file: PathBuf::from("runtime.rs"),
        module_dir: PathBuf::new(),
        function: String::new(),
        api_docs: false,
        problems: &mut problems,
        pools: BTreeSet::new(),
        resources: BTreeSet::new(),
        executor_aliases: executor_aliases(&syntax),
    }
    .visit_file(&syntax);
    problems
}

#[test]
fn guard_rejects_native_alias_builder_and_opaque_macro_execution() {
    for source in [
        "async fn bad(pool: &PgPool) { query_alias(SQL).fetch_all(pool).await; }",
        "async fn bad(pool: &PgPool) { builder.build().execute(pool).await; }",
        "async fn bad(pool: &PgPool) { tokio::try_join!(q.fetch_one(pool), r.fetch_all(pool)); }",
        "async fn bad(pool: &PgPool) { tokio::join!(async { q.fetch_one(pool).await }, r.fetch_all(pool)); }",
        "async fn bad(pool: &PgPool) { opaque!(q.fetch_one(pool)); }",
        "async fn bad(pool: &PgPool) { Executor::fetch_all(pool, SQL).await; }",
        "async fn bad(pool: &PgPool) { let execute = Executor::fetch_all; execute(pool, SQL).await; }",
        "use sqlx::Executor as Run; async fn bad(pool: &PgPool) { Run::fetch_all(pool, SQL).await; }",
        "use sqlx::Executor as Run; async fn bad<E: Run>(pool: E) { E::fetch_all(pool, SQL).await; }",
        "async fn bad(pool: &PgPool) { let alias = pool; alias.begin().await; }",
        "async fn bad_on_connection(pool: &PgPool) {}",
        "async fn hydrate_templates(pool: &PgPool, rows: Rows) { let mut connection = TenantPool::observed_checkout_pool(pool, OP).await?; DbSession::new(&mut connection).query(OP, SQL).fetch_all().await; }",
        "async fn bad(pool: &PgPool) { pool.begin().await; }",
        "async fn bad(pool: &PgPool) { pool.acquire().await; }",
        "async fn bad(pool: &PgPool) { let connection = TenantPool::observed_checkout_pool(pool, OP).await?; hydrate_templates(pool, rows).await; }",
        "async fn bad(pool: &PgPool, tx: &mut Transaction) { hydrate_templates(pool, rows).await; }",
        "async fn bad_on_connection(connection: &mut PgConnection) { TenantPool::observed_checkout_pool(hidden(), OP).await; }",
        "#[cfg(not(feature = \"integration-test-support\"))] async fn normal(pool: &PgPool) { q.fetch_one(pool).await; }",
    ] { assert!(!inspect_snippet(source).is_empty(), "guard accepted: {source}"); }
}

#[test]
fn guard_accepts_bound_macro_queries_and_excludes_only_test_cfg() {
    for source in [
        "async fn good(connection: &mut PgConnection) { DbSession::new(connection).query(OP, SQL).fetch_all().await; }",
        "async fn good(a: &mut PgConnection, b: &mut PgConnection) { tokio::try_join!(DbSession::new(a).query(OP, SQL).fetch_one(), DbSession::new(b).query(OP, SQL).fetch_all()); }",
        "#[cfg(test)] mod tests { async fn fixture(pool: &PgPool) { q.fetch_one(pool).await; } }",
        "#[cfg(feature = \"integration-test-support\")] async fn fixture(pool: &PgPool) { q.fetch_one(pool).await; }",
        "async fn cpu() { worker.execute(move || bcrypt::hash(password)).await; }",
        "async fn cpu() { worker.execute(PasswordOperation::Hash, move || bcrypt::hash(password)).await; }",
        "async fn good(pool: &PgPool) { let connection = TenantPool::observed_checkout_pool(pool, OP).await?; drop(connection); helper(pool).await; }",
        "async fn good(pool: &PgPool) { let ids = { let connection = TenantPool::observed_checkout_pool(pool, OP).await?; query(connection).await }; helper(pool).await; }",
    ] { assert!(inspect_snippet(source).is_empty(), "guard rejected: {source}"); }
}

#[test]
fn guard_follows_external_cfg_modules_without_test_filename_exemptions() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "school-database-guard-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("lib.rs"), r#"#[cfg(not(feature = "integration-test-support"))] mod runtime_tests; #[cfg(test)] mod absent_fixture;"#).unwrap();
    fs::write(
        directory.join("runtime_tests.rs"),
        "async fn normal(pool: &PgPool) { q.fetch_one(pool).await; }",
    )
    .unwrap();
    let mut problems = BTreeSet::new();
    for api_docs in [false, true] {
        scan_file(&directory.join("lib.rs"), api_docs, &mut problems);
    }
    fs::remove_dir_all(directory).unwrap();
    assert!(problems
        .iter()
        .any(|problem| problem.contains("runtime_tests.rs::normal")));
}
