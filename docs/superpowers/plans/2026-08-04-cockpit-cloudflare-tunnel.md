# Cockpit over Cloudflare Tunnel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a resumable, secret-safe Cockpit management surface at `https://server.schoolorbit.app` through Cloudflare Tunnel for the current VPS and every future VPS migration.

**Architecture:** A focused provider owns Tunnel, ingress, connector, and management-CNAME state. A privileged remote script owns Cockpit, cloudflared, loopback binding, and the `schoolorbit` password. Migration and a standalone `configure-cockpit` command call the same management phases, while application DNS and management DNS retain separate failure and rollback decisions.

**Tech Stack:** Bash 4.4+, Bats, Cloudflare v4 API, cloudflared 2026.7.3, Cockpit/Cockpit Podman, systemd, rootless Podman, jq, curl, GitHub Actions.

## Global Constraints

- Read `.rules` and run its complete change-type matrix.
- Do not add Cloudflare Access, OTP, identity-provider, or account-member authentication.
- Do not open inbound TCP 9090; Cockpit listens only on `127.0.0.1:9090`.
- Login uses `schoolorbit`; keep `root` in `/etc/cockpit/disallowed-users`.
- Require at least 10 characters for `SCHOOLORBIT_SERVER_PASSWORD`; uniqueness and greater length remain recommended.
- Never put the bootstrap token, Tunnel token, or `SCHOOLORBIT_SERVER_PASSWORD` in command arguments, logs, checkpoints, or Git.
- Store the Tunnel token in a root-owned mode-0600 file and use cloudflared `--token-file`.
- Support Debian and Ubuntu on amd64 and arm64.
- Pin cloudflared 2026.7.3: amd64 SHA-256 `049777d30f9bf93da6df8bbe31383460eb2aa51a832c6551824d56f9fcc55974`; arm64 SHA-256 `d3ea7d22dd337b465da33d6bc1c4b3cfd381407447a2a7d29542c19783430db3`.
- Keep the previous management CNAME and Tunnel recoverable until rollback closes explicitly.
- Execute inline; the operator previously chose inline execution instead of subagents.

## File Structure

- Create `scripts/lib/schoolorbit-installer/cloudflare_tunnel.sh`: Tunnel API, ingress, connector, and CNAME state.
- Create `scripts/lib/schoolorbit-installer/remote/configure_cockpit.sh`: privileged idempotent host configuration from stdin JSON.
- Create `scripts/tests/installer/cockpit_provider.bats` and `cockpit_remote.bats`: focused provider and host tests.
- Modify `config.sh`, `state.sh`, `vps.sh`, `phases.sh`, `verification.sh`, and the installer entry point.
- Modify `remote/bootstrap.sh` to install Cockpit and pinned cloudflared.
- Modify installer Bats suites and deployment static guards.
- Modify `.env.example`, `.rules`, `docs/TESTING.md`, `docs/OPERATIONS.md`, and `docs/PODMAN_SETUP.md`.

---

### Task 1: Restore the Existing Permission Contract Baseline

**Files:**
- Modify: `backend-school/tests/static_architecture.rs:4422`

**Interfaces:**
- Consumes: `.github/workflows/deploy-backend-school.yml` portable Compose validation.
- Produces: a green architecture baseline before feature work.

- [ ] **Step 1: Reproduce RED**

```bash
cd backend-school
cargo test --test static_architecture deployment_and_smoke_checks_use_backend_readiness -- --exact
```

Expected: FAIL because the assertion still searches for Compose `config`.

- [ ] **Step 2: Replace only the stale assertion**

```rust
assert!(school_deploy.contains(
    r#"podman-compose -f "${runtime_compose}.next" --dry-run up -d backend-school"#
));
```

- [ ] **Step 3: Verify GREEN and commit**

```bash
cd backend-school
cargo test --test static_architecture deployment_and_smoke_checks_use_backend_readiness -- --exact
cd ..
git add backend-school/tests/static_architecture.rs
git commit -m "test: align deployment readiness contract"
```

---

### Task 2: Add Cockpit Command and Secret Contracts

**Files:**
- Modify: `scripts/lib/schoolorbit-installer/config.sh`
- Modify: `scripts/lib/schoolorbit-installer/state.sh`
- Modify: `scripts/schoolorbit-installer`
- Modify: `.env.example`
- Test: `scripts/tests/installer/config_state.bats`

**Interfaces:**
- Produces: `_parse_cockpit_args`, `load_cockpit_inputs`, `SO_COCKPIT_RESUME_RUN_ID`, `SO_COCKPIT_ROLLBACK_RUN_ID`, and sanitized management metadata.

- [ ] **Step 1: Write failing CLI and password tests**

```bash
@test "configure-cockpit accepts dry-run and resume forms" {
    parse_args configure-cockpit --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test --dry-run
    [ "$SO_COMMAND" = configure-cockpit ]
    [ "$SO_DRY_RUN" = true ]
    parse_args configure-cockpit --resume cockpit-run-1
    [ "$SO_COCKPIT_RESUME_RUN_ID" = cockpit-run-1 ]
}

@test "cockpit password requires ten characters" {
    export SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN=bootstrap-token-value
    export SCHOOLORBIT_SERVER_PASSWORD=123456789
    run load_cockpit_inputs
    [ "$status" -eq 64 ]
    SCHOOLORBIT_SERVER_PASSWORD=1234567890
    load_cockpit_inputs
}
```

- [ ] **Step 2: Run RED**

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats scripts/tests/installer/config_state.bats
```

- [ ] **Step 3: Implement parsing and focused loading**

Add:

```bash
declare -ga SO_REQUIRED_COCKPIT_SECRETS=(
    SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN
    SCHOOLORBIT_SERVER_PASSWORD
)
SO_COCKPIT_RESUME_RUN_ID=
SO_COCKPIT_ROLLBACK_RUN_ID=
```

`configure-cockpit` accepts the validated connection options, `--dry-run`, `--secrets-stdin`, or exclusive `--resume RUN_ID`. `rollback-cockpit` accepts only `--run-id RUN_ID`. Extract `_load_named_secrets`: full migration adds `SCHOOLORBIT_SERVER_PASSWORD`; standalone setup loads only `SO_REQUIRED_COCKPIT_SECRETS`.

Extend validation:

```bash
SCHOOLORBIT_SERVER_PASSWORD)
    minimum=10
    ;;
```

Reject secret-like command options before reading values.

- [ ] **Step 4: Extend sanitized state and help**

Allow only:

```text
management_hostname management_dns_snapshot management_record_id
management_record_existed management_tunnel_id management_tunnel_name
```

Add the three command forms to installer help. Add only a non-production marker to `.env.example`.

- [ ] **Step 5: Verify GREEN and commit**

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats scripts/tests/installer/config_state.bats
git add scripts/lib/schoolorbit-installer/config.sh scripts/lib/schoolorbit-installer/state.sh \
    scripts/schoolorbit-installer scripts/tests/installer/config_state.bats .env.example
git commit -m "feat: define cockpit installer contracts"
```

---

### Task 3: Implement the Cloudflare Tunnel Provider

**Files:**
- Create: `scripts/lib/schoolorbit-installer/cloudflare_tunnel.sh`
- Create: `scripts/tests/installer/cockpit_provider.bats`
- Create: `scripts/tests/installer/fixtures/cloudflare-cockpit-*.json`
- Modify: `scripts/schoolorbit-installer`

**Interfaces:**
- Consumes: `_cf_request`, `SO_CF_ACCOUNT_ID`, `SO_CF_ZONE_ID`, `SO_CONFIG`, `SO_RUN_ID`, and `retry`.
- Produces: `cf_cockpit_preflight`, `cf_cockpit_snapshot`, `cf_cockpit_provision_tunnel`, `cf_cockpit_get_token`, `cf_cockpit_wait_connector`, `cf_cockpit_publish`, `cf_cockpit_restore_dns`, and `cf_cockpit_restore_checkpoint`.

- [ ] **Step 1: Write failing provider tests**

```bash
@test "cockpit preflight accepts zero or one exact CNAME" {
    cf_cockpit_preflight
    [ "$SO_CF_COCKPIT_HOSTNAME" = server.schoolorbit.app ]
}

@test "tunnel ingress has Cockpit and a 404 catch-all" {
    cf_cockpit_provision_tunnel
    jq -e '.config.ingress == [
      {"hostname":"server.schoolorbit.app","service":"http://127.0.0.1:9090","originRequest":{}},
      {"service":"http_status:404"}
    ]' "$CAPTURED_REQUEST_BODY"
}

@test "new CNAME rollback deletes only the run-owned record" {
    cf_cockpit_snapshot
    cf_cockpit_publish
    cf_cockpit_restore_dns
    grep -Fq 'DELETE /zones/zone-123/dns_records/cockpit-record-1' "$FAKE_COMMAND_LOG"
}
```

Also cover A/AAAA/duplicate rejection, existing-CNAME restoration, checkpoint drift, wrong connector origin IP, token redaction, and no Tunnel deletion.

- [ ] **Step 2: Run RED**

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats scripts/tests/installer/cockpit_provider.bats
```

- [ ] **Step 3: Implement provider state and preflight**

```bash
SO_CF_COCKPIT_HOSTNAME=
SO_CF_COCKPIT_RECORD_ID=
SO_CF_COCKPIT_RECORD_EXISTED=false
SO_CF_COCKPIT_DNS_SNAPSHOT=null
SO_CF_COCKPIT_TUNNEL_ID=
SO_CF_COCKPIT_TUNNEL_NAME=
```

Accept no record or one proxied CNAME named `server.${SO_CONFIG[base_domain]}`. List non-deleted Tunnels to verify authorization. Reject every ambiguous shape before mutation.

- [ ] **Step 4: Create and configure a distinct Tunnel**

Name it `schoolorbit-cockpit-${SO_RUN_ID}`. POST `{"name":NAME,"config_src":"cloudflare"}` to `/accounts/$SO_CF_ACCOUNT_ID/cfd_tunnel`, validate its UUID, then PUT exactly the Cockpit ingress plus `http_status:404`.

- [ ] **Step 5: Handle token, connector, DNS, and rollback**

Read `/token` through a mode-0600 response file into `SO_SECRETS[SCHOOLORBIT_COCKPIT_TUNNEL_TOKEN]`, then delete the response. Poll `/connections` until an active connection has `origin_ip == SO_CONFIG[target]`.

Create or patch a proxied CNAME to `${SO_CF_COCKPIT_TUNNEL_ID}.cfargotunnel.com` only after drift comparison. Rollback deletes only a run-created record or restores the exact snapshotted record. Never delete a Tunnel automatically.

- [ ] **Step 6: Verify GREEN and commit**

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats scripts/tests/installer/cockpit_provider.bats
shellcheck scripts/lib/schoolorbit-installer/cloudflare_tunnel.sh
shfmt -d -i 4 -ci scripts/lib/schoolorbit-installer/cloudflare_tunnel.sh
git add scripts/lib/schoolorbit-installer/cloudflare_tunnel.sh scripts/schoolorbit-installer \
    scripts/tests/installer/cockpit_provider.bats scripts/tests/installer/fixtures/cloudflare-cockpit-*.json
git commit -m "feat: provision cockpit Cloudflare tunnels"
```

---

### Task 4: Configure Cockpit and cloudflared on the VPS

**Files:**
- Create: `scripts/lib/schoolorbit-installer/remote/configure_cockpit.sh`
- Create: `scripts/tests/installer/cockpit_remote.bats`
- Modify: `scripts/lib/schoolorbit-installer/remote/bootstrap.sh`
- Modify: `scripts/lib/schoolorbit-installer/vps.sh`
- Test: `scripts/tests/installer/vps.bats`

**Interfaces:**
- Consumes: the server password, Tunnel token, `server_user`, base domain, `_vps_ssh`, and `_vps_privileged_prefix`.
- Produces: `vps_configure_cockpit`, `vps_reverify_cockpit`, and idempotent root-owned configuration.

- [ ] **Step 1: Write failing package and host tests**

```bash
@test "remote cockpit binds loopback and uses token-file" {
    run_configure_cockpit_fixture
    grep -Fxq 'ListenStream=127.0.0.1:9090' "$ROOT/etc/systemd/system/cockpit.socket.d/listen.conf"
    grep -Fxq root "$ROOT/etc/cockpit/disallowed-users"
    grep -Fq 'Origins = https://server.schoolorbit.app' "$ROOT/etc/cockpit/cockpit.conf"
    grep -Fq -- '--token-file /etc/cloudflared/schoolorbit-cockpit.token' \
        "$ROOT/etc/systemd/system/schoolorbit-cloudflared.service"
    [ "$(stat -c %a "$ROOT/etc/cloudflared/schoolorbit-cockpit.token")" = 600 ]
}
```

Add pinned amd64/arm64 digest, stdin-secret, non-public-listener, fresh-session, and run-twice idempotency cases.

- [ ] **Step 2: Run RED**

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats scripts/tests/installer/cockpit_remote.bats scripts/tests/installer/vps.bats
```

- [ ] **Step 3: Install pinned packages**

Keep apt ownership for `cockpit` and `cockpit-podman`. Map `dpkg --print-architecture` to the exact cloudflared URL and digest in Global Constraints. Download to `mktemp`, verify SHA-256, install with `dpkg -i`, and reject other architectures before download. Skip only when version is exactly 2026.7.3.

- [ ] **Step 4: Implement stdin-only remote configuration**

Read one JSON object containing `server_user`, `server_password`, `management_hostname`, and `tunnel_token`. Require EUID 0 and validate every field. Change the password only through:

```bash
printf '%s:%s\n' "$server_user" "$server_password" | chpasswd
```

Atomically install Cockpit config:

```ini
[WebService]
Origins = https://server.schoolorbit.app
ProtocolHeader = X-Forwarded-Proto
ForwardedForHeader = X-Forwarded-For
LoginTo = false
```

Install the loopback socket override, keep root disallowed, write the token file mode 0600, and install `schoolorbit-cloudflared.service` with:

```ini
ExecStart=/usr/bin/cloudflared --no-autoupdate tunnel run --token-file /etc/cloudflared/schoolorbit-cockpit.token
Restart=on-failure
RestartSec=5s
```

Enable both units, verify `/ping`, and reject wildcard IPv4/IPv6 9090 listeners.

- [ ] **Step 5: Stream script and payload in separate SSH calls**

First atomically install the tracked script through SSH stdin. Then send jq-generated secret JSON to that installed script through a second SSH stdin stream. `vps_reverify_cockpit` uses a fresh session to verify units, loopback, `/ping`, modes, and no public 9090 firewall allow.

- [ ] **Step 6: Verify GREEN and commit**

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats scripts/tests/installer/cockpit_remote.bats scripts/tests/installer/vps.bats
shellcheck scripts/lib/schoolorbit-installer/remote/bootstrap.sh \
    scripts/lib/schoolorbit-installer/remote/configure_cockpit.sh scripts/lib/schoolorbit-installer/vps.sh
shfmt -d -i 4 -ci scripts/lib/schoolorbit-installer/remote/bootstrap.sh \
    scripts/lib/schoolorbit-installer/remote/configure_cockpit.sh scripts/lib/schoolorbit-installer/vps.sh
git add scripts/lib/schoolorbit-installer/remote/bootstrap.sh \
    scripts/lib/schoolorbit-installer/remote/configure_cockpit.sh scripts/lib/schoolorbit-installer/vps.sh \
    scripts/tests/installer/cockpit_remote.bats scripts/tests/installer/vps.bats
git commit -m "feat: configure loopback cockpit runtime"
```

---

### Task 5: Add Resumable Management Orchestration

**Files:**
- Modify: `scripts/lib/schoolorbit-installer/phases.sh`
- Modify: `scripts/lib/schoolorbit-installer/verification.sh`
- Modify: `scripts/schoolorbit-installer`
- Test: `scripts/tests/installer/orchestration.bats`

**Interfaces:**
- Consumes: Tasks 2–4 command, state, provider, and VPS functions.
- Produces: shared management phases, standalone setup/resume, public verification, and `rollback-cockpit`.

- [ ] **Step 1: Write failing phase and failure tests**

```bash
@test "migration publishes cockpit only after public application verification" {
    install_orchestration_fakes
    schoolorbit_main migrate-vps --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test
    expected='preflight input snapshot bootstrap tls deploy origin-verify cutover-gate dns-cutover public-verify management-provision management-publish handoff'
    [ "$(tr '\n' ' ' <"$PHASE_LOG" | sed 's/ $//')" = "$expected" ]
}

@test "standalone cockpit setup never dispatches or changes API DNS" {
    install_orchestration_fakes
    schoolorbit_main configure-cockpit --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test
    ! grep -Eq 'workflow run|cutover-batch-applied' "$FAKE_COMMAND_LOG"
}

@test "management failure reports only management rollback" {
    install_orchestration_fakes
    export FAKE_MANAGEMENT_VERIFY_FAILURE=1
    run schoolorbit_main configure-cockpit --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test
    [ "$status" -ne 0 ]
    [[ $output == *'rollback-cockpit --run-id'* ]]
    [[ $output != *'rollback-dns --run-id'* ]]
}
```

Also cover dry-run mutation absence, resume revalidation, pre-publication failure, existing-CNAME restore, initial-CNAME deletion, and full migration rollback restoring management DNS.

- [ ] **Step 2: Run RED**

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats scripts/tests/installer/orchestration.bats
```

- [ ] **Step 3: Implement shared management phases**

Define:

```text
management-snapshot  -> checkpoint DNS/Tunnel state
management-provision -> create/configure Tunnel, configure VPS, wait for target connector
management-publish   -> drift-check, publish CNAME, verify public Cockpit
management-handoff   -> print sanitized hostname/Tunnel/rollback summary
```

`migrate-vps` takes the management snapshot with the provider snapshot, then runs provision and publish after `public-verify` and before `handoff`. `configure-cockpit` runs only preflight, input, management-snapshot, bootstrap, provision, publish, and management-handoff.

- [ ] **Step 4: Implement public verification**

`verify_public_cockpit` requires HTTPS through `server.${base_domain}`, the Cockpit login marker or documented login redirect, no Cloudflare Access redirect, and no response from `${target}:9090`. Do not submit credentials in automated requests.

- [ ] **Step 5: Implement explicit management rollback**

`rollback-cockpit --run-id RUN_ID` reloads only the bootstrap token, shows the reverse diff, and requires:

```text
ROLLBACK COCKPIT server.schoolorbit.app
```

It restores or removes only the management CNAME and retains both Tunnels/VPSs. Full `rollback-dns` invokes the same primitive only when its migration checkpoint contains management metadata.

- [ ] **Step 6: Verify GREEN and commit**

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats scripts/tests/installer/orchestration.bats
git add scripts/lib/schoolorbit-installer/phases.sh scripts/lib/schoolorbit-installer/verification.sh \
    scripts/schoolorbit-installer scripts/tests/installer/orchestration.bats
git commit -m "feat: orchestrate cockpit management cutover"
```

---

### Task 6: Add Durable Documentation and Static Guards

**Files:**
- Modify: `.rules`
- Modify: `docs/TESTING.md`
- Modify: `docs/OPERATIONS.md`
- Modify: `docs/PODMAN_SETUP.md`
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs`

**Interfaces:**
- Consumes: final command names, phase names, paths, and verification behavior.
- Produces: canonical operator instructions and cross-stack regression guards.

- [ ] **Step 1: Write failing static guards**

```javascript
assert.match(installer, /cloudflare_tunnel\.sh/);
assert.match(cockpitRemote, /ListenStream=127\.0\.0\.1:9090/);
assert.match(cockpitRemote, /--token-file/);
assert.doesNotMatch(compose, /9090\s*:/);
assert.doesNotMatch(bootstrap, /ufw allow 9090/);
```

Also require the pinned release/digests, root prohibition, direct login hostname, rollback command, and focused Bats files in the documented matrix.

- [ ] **Step 2: Run RED**

```bash
node --test frontend-school/tests/static/deployment-installer.test.mjs
```

- [ ] **Step 3: Update canonical docs**

Document command/resume/rollback forms; `SCHOOLORBIT_SERVER_PASSWORD`; direct `schoolorbit` login; why root does not own the production Podman namespace; public-login residual risk; Tunnel/CNAME retention; current/future VPS checks; and the expanded test matrix.

- [ ] **Step 4: Verify GREEN and commit**

```bash
node --test frontend-school/tests/static/deployment-installer.test.mjs
node --test frontend-school/tests/static/documentation-policy.test.mjs
git add .rules docs/TESTING.md docs/OPERATIONS.md docs/PODMAN_SETUP.md \
    frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "docs: document cockpit tunnel operations"
```

---

### Task 7: Run Full Local and GitHub Verification

**Files:**
- Modify only files required by failures caused by Tasks 1–6.

**Interfaces:**
- Consumes: complete implementation.
- Produces: fresh evidence before production mutation.

- [ ] **Step 1: Run shell checks**

```bash
shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh \
    scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh \
    scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
```

- [ ] **Step 2: Run installer, static, Compose, and Actions checks**

```bash
bats scripts/tests/installer
node --test frontend-school/tests/static/deployment-installer.test.mjs
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
    podman-compose -f podman-compose.yml --dry-run up -d >/dev/null
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
```

- [ ] **Step 3: Run backend verification**

```bash
cd backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
cd ..
```

- [ ] **Step 4: Inspect diff and secrets**

```bash
git diff --check origin/main...HEAD
git status --short
git grep -n -E 'SCHOOLORBIT_SERVER_PASSWORD|SCHOOLORBIT_COCKPIT_TUNNEL_TOKEN'
```

Review every match. Only variable names, safe markers, tests, and runtime lookups may appear.

- [ ] **Step 5: Push and monitor every triggered workflow**

```bash
git push origin main
gh run list --repo akephisit/schoolorbit-new --commit "$(git rev-parse HEAD)" \
    --json databaseId,workflowName,status,conclusion,url
```

Wait for final conclusions. Inspect exact logs before changing code after a failure.

---

### Task 8: Configure and Verify the Current VPS

**Files:**
- Local secret input: `.env.local` only; never stage.
- Remote managed paths: `/etc/cockpit`, `/etc/cloudflared`, `/etc/systemd/system`, and package paths on `130.94.21.134`.

**Interfaces:**
- Consumes: strong `SCHOOLORBIT_SERVER_PASSWORD`, updated bootstrap token, strict known-host SSH, and verified installer.
- Produces: healthy Tunnel/CNAME and direct Cockpit login with current rootless containers.

- [ ] **Step 1: Require the operator password**

Confirm `.env.local` contains a unique value of at least 10 characters named:

```text
SCHOOLORBIT_SERVER_PASSWORD
```

Never print it or put it in a command argument.

- [ ] **Step 2: Run standalone dry-run**

Load only the bootstrap token and server password, then run:

```bash
./scripts/schoolorbit-installer configure-cockpit \
    --repository akephisit/schoolorbit-new \
    --target 130.94.21.134 \
    --base-domain schoolorbit.app \
    --dry-run
```

Expected: Tunnel authorization, management DNS shape, target OS, strict SSH, and a non-mutating plan pass.

- [ ] **Step 3: Apply standalone setup**

Run the same command without `--dry-run`. Record only the sanitized run ID, Tunnel ID, CNAME ID, and phase statuses.

- [ ] **Step 4: Verify the target**

```bash
systemctl is-active cockpit.socket schoolorbit-cloudflared.service
ss -ltnH | grep -F '127.0.0.1:9090'
curl -fsS http://127.0.0.1:9090/ping
```

Reject wildcard/public 9090. Confirm UFW still allows SSH/80/443 and denies 8080/8081.

- [ ] **Step 5: Verify Cloudflare and login**

Require connector `origin_ip == 130.94.21.134`, a proxied CNAME to the checkpointed Tunnel UUID, and direct Cockpit at `https://server.schoolorbit.app` without Cloudflare Access.

Login as `schoolorbit` and confirm Podman lists:

```text
schoolorbit-backend-admin
schoolorbit-backend-school
schoolorbit-clamd
schoolorbit-nginx
```

Keep root login disabled.

- [ ] **Step 6: Recheck production**

```bash
curl -fsS https://admin-api.schoolorbit.app/ready
curl -fsS https://school-api.schoolorbit.app/ready
curl -fsS -o /dev/null https://admin.schoolorbit.app/
curl -fsS -o /dev/null https://snwsb.schoolorbit.app/
```

Record the management rollback command and retain the previous Tunnel/VPS through the rollback window.

---

### Task 9: Lower the Operator-approved Password Compatibility Floor

**Files:**
- Modify: `scripts/tests/installer/config_state.bats`
- Modify: `scripts/tests/installer/cockpit_remote.bats`
- Modify: `scripts/lib/schoolorbit-installer/config.sh`
- Modify: `scripts/lib/schoolorbit-installer/remote/configure_cockpit.sh`
- Modify: `.env.example`
- Modify: `.rules`
- Modify: `docs/OPERATIONS.md`
- Modify: `docs/PODMAN_SETUP.md`

**Interfaces:**
- Consumes: `load_cockpit_inputs` and the exact JSON payload accepted by `configure_cockpit.sh`.
- Produces: one consistent 10-character minimum at local validation, remote validation, tests, and operator documentation.

- [x] **Step 1: Write failing 9/10-character boundary tests**

Update the local validation test to reject `123456789` and accept `1234567890`. Add a remote test that sends the same two boundary values through stdin JSON and requires nonzero/zero exit status respectively. Keep the password out of command arguments and fake command logs.

- [x] **Step 2: Run RED**

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats scripts/tests/installer/config_state.bats scripts/tests/installer/cockpit_remote.bats
```

Expected: the 10-character cases fail while both validators still require 16 characters.

- [x] **Step 3: Change both validation boundaries**

In `config.sh` use:

```bash
SCHOOLORBIT_SERVER_PASSWORD)
    minimum=10
    ;;
```

In `remote/configure_cockpit.sh` use:

```bash
server_password=$(jq -er '.server_password | strings | select(length >= 10)' <<<"$payload")
```

- [x] **Step 4: Align durable configuration and documentation**

Replace only the Cockpit password minimum from 16 to 10 in `.env.example`, `.rules`, `docs/OPERATIONS.md`, and `docs/PODMAN_SETUP.md`. Retain the requirements for a unique password and secret-only transport.

- [x] **Step 5: Run GREEN and the deployment verification matrix**

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats scripts/tests/installer
node --test frontend-school/tests/static/deployment-installer.test.mjs \
    frontend-school/tests/static/documentation-policy.test.mjs
```

Also run ShellCheck, shfmt, `git diff --check`, and every applicable installer/documentation check from `.rules`.

- [ ] **Step 6: Commit, push, verify CI, and resume current-VPS setup**

```bash
git add .env.example .rules docs/OPERATIONS.md docs/PODMAN_SETUP.md \
    docs/superpowers/plans/2026-08-04-cockpit-cloudflare-tunnel.md \
    scripts/lib/schoolorbit-installer/config.sh \
    scripts/lib/schoolorbit-installer/remote/configure_cockpit.sh \
    scripts/tests/installer/config_state.bats \
    scripts/tests/installer/cockpit_remote.bats
git commit -m "fix: support cockpit password compatibility floor"
git push origin main
```

After CI passes, load `.env.local`, run standalone `--dry-run`, apply `configure-cockpit`, and complete Task 8 live verification without printing the password.

---

### Task 10: Expose the Existing Rootless Podman Namespace to Cockpit

**Files:**
- Modify: `scripts/lib/schoolorbit-installer/remote/bootstrap.sh`
- Modify: `scripts/lib/schoolorbit-installer/vps.sh`
- Modify: `scripts/tests/installer/cockpit_remote.bats`
- Modify: `scripts/tests/installer/vps.bats`
- Modify: `.rules`
- Modify: `docs/OPERATIONS.md`
- Modify: `docs/PODMAN_SETUP.md`

**Interfaces:**
- Consumes: the existing `schoolorbit` account, its linger-enabled user manager, `podman.socket`, and root-privileged bootstrap execution.
- Produces: `enable_server_user_podman_socket USER`, an active `/run/user/UID/podman/podman.sock`, and resume verification that fails closed when Cockpit cannot reach the rootless API.

- [ ] **Step 1: Write failing host-boundary tests**

Add a Bats case that fakes `id`, `getent`, root `systemctl`, and `runuser`. Invoke the desired helper twice and require both calls to leave the fake user socket active without invoking `podman stop`, `podman rm`, or `podman create`:

```bash
@test "bootstrap enables the server user's Podman API socket idempotently" {
    export USER_PODMAN_STATE="$TEST_ROOT/user-podman-active"
    make_fake_command id '
case "$*" in
    "-u schoolorbit") printf "%s\n" 1000 ;;
    *) exit 1 ;;
esac
'
    make_fake_command getent '
[ "$*" = "passwd schoolorbit" ]
printf "%s\n" "schoolorbit:x:1000:1000::/home/schoolorbit:/bin/bash"
'
    make_fake_command systemctl '
[ "$*" = "start user@1000.service" ]
printf "systemctl %s\n" "$*" >>"$FAKE_COMMAND_LOG"
'
    make_fake_command runuser '
printf "runuser %s\n" "$*" >>"$FAKE_COMMAND_LOG"
case "$*" in
    *"systemctl --user enable --now podman.socket") touch "$USER_PODMAN_STATE" ;;
    *"systemctl --user is-active --quiet podman.socket") [ -f "$USER_PODMAN_STATE" ] ;;
    *) exit 1 ;;
esac
'

    run env PATH="$FAKE_BIN:$ORIGINAL_PATH" \
        FAKE_COMMAND_LOG="$FAKE_COMMAND_LOG" USER_PODMAN_STATE="$USER_PODMAN_STATE" \
        bash -c 'source "$1"; enable_server_user_podman_socket schoolorbit; enable_server_user_podman_socket schoolorbit' \
        _ "$BOOTSTRAP_SCRIPT"

    [ "$status" -eq 0 ]
    [ -f "$USER_PODMAN_STATE" ]
    ! grep -Eq 'podman (stop|rm|create)' "$FAKE_COMMAND_LOG"
}
```

Extend the VPS checkpoint test to require the remote script to check `podman.socket`, the per-user Unix socket, and `podman --remote` connectivity.

- [ ] **Step 2: Run RED**

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats --filter "Podman API socket|Cockpit bootstrap revalidation" \
    scripts/tests/installer/cockpit_remote.bats scripts/tests/installer/vps.bats
```

Expected: FAIL because `enable_server_user_podman_socket` does not exist and checkpoint revalidation does not inspect the user socket.

- [ ] **Step 3: Implement the idempotent user socket lifecycle**

Add this boundary to `remote/bootstrap.sh` and call it after `loginctl enable-linger`:

```bash
enable_server_user_podman_socket() {
    local server_user=${1:?Server user is required} server_uid server_home runtime_directory
    server_uid=$(id -u "$server_user")
    server_home=$(getent passwd "$server_user" | awk -F: 'NR == 1 { print $6 }')
    [[ -n $server_home ]] || return 78
    runtime_directory="/run/user/$server_uid"

    systemctl start "user@${server_uid}.service"
    runuser -u "$server_user" -- env \
        HOME="$server_home" \
        XDG_RUNTIME_DIR="$runtime_directory" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_directory/bus" \
        systemctl --user enable --now podman.socket
    runuser -u "$server_user" -- env \
        HOME="$server_home" \
        XDG_RUNTIME_DIR="$runtime_directory" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_directory/bus" \
        systemctl --user is-active --quiet podman.socket
}
```

Do not invoke Compose or any mutating Podman command in this helper.

- [ ] **Step 4: Strengthen fresh-session revalidation**

In `vps_reverify_cockpit_bootstrap`, derive the UID and home again, then require:

```bash
runtime_directory="/run/user/$(id -u "$server_user")"
socket="$runtime_directory/podman/podman.sock"
runuser -u "$server_user" -- env \
    HOME="$(getent passwd "$server_user" | awk -F: 'NR == 1 { print $6 }')" \
    XDG_RUNTIME_DIR="$runtime_directory" \
    DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_directory/bus" \
    systemctl --user is-active --quiet podman.socket
test -S "$socket"
runuser -u "$server_user" -- env \
    HOME="$(getent passwd "$server_user" | awk -F: 'NR == 1 { print $6 }')" \
    XDG_RUNTIME_DIR="$runtime_directory" \
    podman --remote --url "unix://$socket" info >/dev/null
```

- [ ] **Step 5: Document the durable boundary and verify GREEN**

Require the `schoolorbit` user socket in `.rules`. Document its systemd unit, runtime path, and read-only diagnostics in `docs/OPERATIONS.md` and `docs/PODMAN_SETUP.md`. Then run:

```bash
PATH=/tmp/schoolorbit-installer-jq-1.7.1-amd64:/tmp/schoolorbit-bats-core-v1.11.1/bin:$PATH \
    bats scripts/tests/installer
shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh \
    scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh \
    scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
node --test frontend-school/tests/static/deployment-installer.test.mjs
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
    podman-compose -f podman-compose.yml --dry-run up -d >/dev/null
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
```

- [ ] **Step 6: Commit, push, and verify CI**

```bash
git add .rules docs/OPERATIONS.md docs/PODMAN_SETUP.md \
    docs/superpowers/plans/2026-08-04-cockpit-cloudflare-tunnel.md \
    scripts/lib/schoolorbit-installer/remote/bootstrap.sh \
    scripts/lib/schoolorbit-installer/vps.sh \
    scripts/tests/installer/cockpit_remote.bats scripts/tests/installer/vps.bats
git commit -m "fix: expose rootless podman to cockpit"
git push origin main
```

Wait for every workflow triggered by the commit to complete successfully before live mutation.

- [ ] **Step 7: Repair and verify the current VPS without restarting containers**

Before mutation, record the four `schoolorbit` container IDs and start timestamps. Start `user@1000.service`, enable and start `podman.socket` inside the `schoolorbit` user manager, then require:

```bash
systemctl is-active user@1000.service
runuser -u schoolorbit -- env XDG_RUNTIME_DIR=/run/user/1000 \
    systemctl --user is-active podman.socket
test -S /run/user/1000/podman/podman.sock
runuser -u schoolorbit -- env HOME=/home/schoolorbit XDG_RUNTIME_DIR=/run/user/1000 \
    podman --remote --url unix:///run/user/1000/podman/podman.sock ps
```

Compare the container IDs and start timestamps with the pre-mutation snapshot; all four must be unchanged. Refresh Cockpit Podman and verify it lists the same four names. Recheck Cockpit HTTPS, both API readiness endpoints, and that public TCP 9090 remains unreachable.

---

## Plan Self-review

- Spec coverage: Tasks 2–8 cover public Cockpit, no Access, loopback 9090, `schoolorbit`, root prohibition, secret transport, distinct Tunnels, resume, rollback, current VPS, future migrations, documentation, and live verification. Task 9 covers the operator-approved 10-character compatibility floor at both trust boundaries. Task 10 covers the rootless user Podman API socket, installer idempotency, fresh-session revalidation, and live no-restart repair.
- Placeholder scan: hostnames, variables, commands, functions, version, hashes, and confirmation phrase are concrete; no deferred implementation markers remain.
- Interface consistency: Task 2 supplies command/state contracts; Tasks 3–4 produce provider/VPS functions; Task 5 composes them; Tasks 6–8 verify and operate the same names.
