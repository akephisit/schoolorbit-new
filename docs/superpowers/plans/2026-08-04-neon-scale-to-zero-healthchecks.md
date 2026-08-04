# Neon Scale-to-Zero Healthchecks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop repository-owned recurring healthchecks from keeping Neon active while preserving dependency-aware deployment and smoke-test gates.

**Architecture:** Recurring Docker and Podman container probes use the existing dependency-free `/health` endpoints. The existing `/ready` endpoints remain unchanged and continue to own database, control-plane, R2, and clamd validation for deployments, smoke tests, and intentional diagnostics.

**Tech Stack:** Rust/Axum static architecture tests, Docker Compose, Podman Compose, GitHub Actions, Bash/Bats, canonical operations documentation

## Global Constraints

- `/health` is dependency-free process liveness; `/ready` is dependency readiness.
- Both local and production Compose must use `/health` for recurring backend probes every 30 seconds.
- Deployment workflows and smoke tests must continue to use `/ready`.
- External uptime monitors must use `/health` to avoid recreating the Neon keep-awake behavior.
- File Platform reconciliation at the top of every hour and calendar reminders at 07:00 Asia/Bangkok remain unchanged.
- The accepted first-request cold start is approximately 1-5 seconds after Neon becomes idle.
- Do not change health/readiness response contracts, migrations, permissions, generated API contracts, schedulers, or SQLx pool settings.
- Do not trigger a production deployment without separate user authorization.
- Never print or commit database URLs, Neon credentials, or other secrets.

---

### Task 1: Separate Recurring Liveness from Release Readiness

**Files:**
- Modify: `backend-school/tests/static_architecture.rs:4390-4440`
- Modify: `docker-compose.yml:49-51,116-118`
- Modify: `podman-compose.yml:41-43,110-112`
- Modify: `.rules:225-230`
- Modify: `docs/OPERATIONS.md:34-41`

**Interfaces:**
- Consumes: existing `GET /health` liveness handlers and `GET /ready` readiness handlers in both backends.
- Produces: Compose healthchecks that call `/health`; deployment workflows and smoke tests that remain guarded by `/ready`; a static test that enforces this split.

- [ ] **Step 1: Write the failing static architecture guard**

Rename `deployment_and_smoke_checks_use_backend_readiness` and replace only the Compose assertions so the test reads:

```rust
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
```

- [ ] **Step 2: Run the focused test and verify RED**

Run from `backend-school`:

```bash
cargo test --test static_architecture recurring_healthchecks_use_liveness_while_deployment_and_smoke_use_readiness -- --exact
```

Expected: FAIL because both Compose definitions still contain `http://localhost:8080/ready` and `http://localhost:8081/ready` instead of `/health`.

- [ ] **Step 3: Make the minimal Compose changes**

In both `docker-compose.yml` and `podman-compose.yml`, change only the two backend healthcheck URLs:

```yaml
backend-admin:
  healthcheck:
    test: ["CMD", "curl", "-f", "http://localhost:8080/health"]

backend-school:
  healthcheck:
    test: ["CMD", "curl", "-f", "http://localhost:8081/health"]
```

Preserve each file's existing YAML list-spacing style. Do not change the 30-second interval, timeout, retries, start period, `depends_on`, clamd healthcheck, environment, network, or volume configuration.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run from `backend-school`:

```bash
cargo test --test static_architecture recurring_healthchecks_use_liveness_while_deployment_and_smoke_use_readiness -- --exact
```

Expected: PASS, including the unchanged assertions that deployment workflows and smoke tests still call `/ready`.

- [ ] **Step 5: Update the authoritative runtime rule**

Replace the existing health/readiness bullet in `.rules` with:

```markdown
- `/health` is dependency-free process liveness and owns recurring Docker/Podman and external uptime-monitor healthchecks. `/ready` verifies dependencies and is reserved for deployment, smoke, and intentional readiness checks so routine probes do not keep Neon active.
```

- [ ] **Step 6: Update the operations contract**

Replace the paragraph below the `/health` and `/ready` bullets in `docs/OPERATIONS.md` with:

```markdown
Recurring Compose healthchecks use `/health` so process monitoring does not wake Neon or probe external dependencies. Backend deployment workflows and smoke tests use `/ready`; backend-school readiness verifies its backend-admin control-plane connection without waking every tenant database. External uptime monitors must use `/health`, because polling `/ready` would keep the admin Neon compute active. A dependency failure must fail the deployment readiness gate, while a live process remains diagnosable through `/health`.
```

- [ ] **Step 7: Run focused backend verification**

Run from `backend-admin`:

```bash
cargo fmt --all -- --check
cargo test health
cargo check
```

Expected: the liveness and readiness unit tests pass, including the assertion that `/health` has no database status.

Then run from `backend-school`:

```bash
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
```

Expected: all commands exit 0 with no test failures or compiler errors.

- [ ] **Step 8: Run the production-topology verification matrix**

Run from the repository root:

```bash
shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
bats scripts/tests/installer
node --test frontend-school/tests/static/deployment-installer.test.mjs
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
  podman-compose -f podman-compose.yml --dry-run up -d >/dev/null
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
```

Expected: every command exits 0. Do not replace an unavailable command with a weaker check; report the exact unavailable dependency or failure.

- [ ] **Step 9: Review the complete change**

Run from the repository root:

```bash
git diff --check
git diff -- backend-school/tests/static_architecture.rs docker-compose.yml \
  podman-compose.yml .rules docs/OPERATIONS.md
git status --short
```

Expected: no whitespace errors; the diff contains only the test guard, four healthcheck URL replacements, and the approved rule/documentation updates. The design and plan documents remain separate workflow artifacts.

- [ ] **Step 10: Commit the implementation**

```bash
git add backend-school/tests/static_architecture.rs docker-compose.yml podman-compose.yml \
  .rules docs/OPERATIONS.md
git commit -m "fix: allow Neon compute to scale to zero"
```

Expected: one focused implementation commit with no secrets or generated artifacts.

## Operational Handoff

Production validation requires separate deployment authorization. After an authorized deployment:

1. Inspect both backend container health commands and confirm they end in `/health`.
2. Call each public `/ready` endpoint once and confirm the release dependencies are ready.
3. Ensure external uptime monitors do not poll `/ready`.
4. Observe Neon after the configured inactivity delay. With no users or scheduled work, the recurring `SELECT 1` pattern must disappear and compute should reach idle/zero.
5. Expect short activity at the top of each hour and at 07:00 Asia/Bangkok from the retained scheduled jobs.
