# SchoolOrbit Hybrid VPS Installer Design

**Status:** Approved during brainstorming on 2026-08-02

**Initial command:** `./scripts/schoolorbit-installer migrate-vps`

## Purpose

SchoolOrbit currently has tracked production configuration and GitHub Actions workflows, but moving the backend to a replacement VPS still requires several manual operations across GitHub, Cloudflare, SSH, Podman, Nginx, and deployment verification. Some deployment paths also have overlapping ownership: `podman-compose.yml` is documented as the production topology, while the backend-school workflow uploads a second Compose definition that owns the same backend-school and clamd container names.

The first installer release provides one operator command that prepares and verifies a replacement VPS, deploys the existing system through GitHub Actions, and performs a confirmed Cloudflare DNS cutover. It preserves the existing GitHub repository, Cloudflare account, Neon databases, R2 buckets, API hostnames, and tenant data.

## Scope

### Goals

- Run from a trusted Linux or WSL administrator workstation.
- Support the project-supported Ubuntu LTS and Debian stable releases by detecting `/etc/os-release` on the target.
- Use `schoolorbit.app` as the default base domain while accepting an explicit `BASE_DOMAIN`.
- Prepare a new VPS through SSH without interrupting the old VPS.
- Configure the required GitHub repository variables and secrets without putting secret values in command arguments or logs.
- Keep GitHub Actions as the build and deployment engine for backend images and Cloudflare frontends.
- Normalize production Compose ownership before relying on it for a clean VPS.
- Deploy backend-admin, backend-school, frontend-admin, and tenant frontends.
- Issue and install a Cloudflare Origin Certificate for the two API origins.
- Verify the replacement origin before changing public DNS.
- Require an explicit operator confirmation immediately before DNS cutover.
- Preserve enough non-secret state to resume a failed run and offer a confirmed DNS rollback.
- Keep the old VPS available after a successful cutover.

### Non-goals

- Creating or replacing Neon projects, tenant databases, R2 buckets, or the Cloudflare account.
- Copying tenant data or changing the SQLx migration timeline.
- Deleting, shutting down, or modifying the old VPS automatically.
- Automatically rolling DNS back without operator confirmation.
- Supporting native Windows PowerShell in the first release; Windows users run the installer through WSL.
- Implementing `fresh-install` in the first release. Its future implementation will reuse the same modules and state model.
- Acting as a general-purpose infrastructure-as-code platform.

## Architectural Decisions

### Hybrid orchestration

The trusted administrator workstation owns orchestration. It performs preflight checks, accepts secrets, calls GitHub and Cloudflare APIs, bootstraps the VPS through SSH, dispatches GitHub Actions, monitors their results, verifies the target, and controls cutover.

GitHub Actions remains responsible for reproducible builds and deployments:

- build and publish backend images to GHCR;
- deploy backend-admin and backend-school to the selected VPS;
- build and deploy frontend-admin to Cloudflare Workers;
- build and deploy tenant frontend Workers.

The target VPS owns runtime processes and runtime secrets. Neon and R2 remain external dependencies. Cloudflare owns public DNS, proxying, Workers, and the API origin certificate trust boundary.

### Bash implementation

The installer is a Bash entry point with focused library modules. Bash is available in the supported administrator environments and avoids introducing a separate installer runtime. The entry point requires Bash 4.4 or newer and uses explicit prerequisites such as `gh`, `ssh`, `curl`, `jq`, and `openssl`.

The code is divided by responsibility:

- `preflight`: local tools, authentication, supported target OS, SSH, and API permission checks;
- `configuration`: flags, environment input, hidden prompts, normalization, and validation;
- `state`: run identity, checkpoints, configuration fingerprints, and resume behavior;
- `github`: repository variables/secrets, workflow dispatch, monitoring, and failure summaries;
- `cloudflare`: zone discovery, origin certificate issuance, DNS snapshots, cutover, and rollback;
- `vps`: bootstrap, service user, rootless Podman, filesystem permissions, runtime environment, and TLS material;
- `verification`: Compose/Nginx validation, readiness, direct-origin checks, and smoke tests;
- `ui`: sanitized progress output, plans, confirmations, and final handoff.

Modules communicate through validated values and stable function return codes. They do not read one another's internal variables or duplicate API calls.

### Canonical production topology

`podman-compose.yml` becomes the only Compose owner of backend-admin, backend-school, clamd, their volumes, and their networks. Production networks and volumes receive explicit names so their identity does not depend on the working directory or Compose project inference.

The backend deployment workflows upload and validate the canonical Compose file, then recreate only the requested service and its required dependency. They must not restart unrelated services. The duplicate `backend-school/docker-compose.yml` runtime path is removed after the canonical workflow is active.

Topology replacement uses a staged file on the VPS:

1. upload to a temporary path;
2. run `podman-compose config` against the target runtime environment;
3. verify required service, network, and volume names;
4. atomically replace `/opt/stack/podman-compose.yml`;
5. recreate only the requested services;
6. gate success on `/ready`.

This topology change also updates the durable ownership statements in `.rules`, `docs/OPERATIONS.md`, and `docs/PODMAN_SETUP.md` where required.

### Frontend-admin deployment ownership

A dedicated frontend-admin workflow is added. Public URLs and the base domain come from GitHub repository variables. Cloudflare credentials and `INTERNAL_API_SECRET` remain secrets. `INTERNAL_API_SECRET` is installed as a Cloudflare Worker secret binding and is removed from committed Wrangler variables.

The workflow generates or supplies environment-specific Wrangler configuration during deployment. The committed configuration must not contain an account identifier, production-only URL, tenant identifier, or secret placeholder that could be mistaken for an operative credential.

## Operator Interface

The normal entry point is:

```text
./scripts/schoolorbit-installer migrate-vps \
  --repository OWNER/REPOSITORY \
  --target NEW_VPS_IP \
  --base-domain schoolorbit.app
```

`--base-domain` defaults to `schoolorbit.app`. The command remains interactive for hidden secret input and the cutover confirmation. A `--dry-run` option performs discovery, validation, and change planning without mutation. A stopped run is continued with:

```text
./scripts/schoolorbit-installer migrate-vps --resume RUN_ID
```

Secrets are accepted from the current process environment, standard input, or a hidden prompt. A secret is never accepted as a command-line value because process listings and shell history may expose it. The installer identifies missing values by name without printing their content.

The first release accepts existing runtime credentials; it does not automatically read `/opt/stack/.env` from the old VPS. An operator or secret manager may pipe values to standard input. This keeps authority to read the old server separate from authority to create the replacement.

## Configuration Ownership

Configuration is separated by sensitivity and consumer:

- GitHub repository variables hold non-secret deployment values such as the base domain and public API URLs.
- GitHub repository secrets hold deployment credentials such as the Cloudflare token, target SSH private key, and application secrets required by workflows.
- `/opt/stack/.env` holds backend runtime URLs and secrets, with mode `0600` and ownership restricted to the runtime service user.
- Cloudflare Worker secret bindings hold server-side frontend secrets.
- The local checkpoint holds only non-secret identifiers and observations.

The installer creates a dedicated deployment SSH key when one is not supplied. Transient key material uses a private temporary directory, is passed to GitHub through standard input, and is removed when configuration finishes. The public key is authorized only for the deployment service user. Existing credentials may be supplied instead.

The Cloudflare token must be limited to the selected account and zone with only the permissions needed for Workers, DNS Write, and Zone SSL and Certificates Edit. Runtime R2 credentials are limited to the existing SchoolOrbit buckets. The VPS never receives the administrator's GitHub or Cloudflare bootstrap credentials.

## Migration State Machine

Each run stores state under:

```text
~/.local/state/schoolorbit-installer/RUN_ID.json
```

The state directory is created under `umask 077`. State contains the run ID, repository, target IP, base domain, completed phase names, GitHub workflow run IDs, non-secret Cloudflare resource and certificate IDs, certificate expiry, the original DNS record values, the intended DNS values, and a fingerprint of non-secret configuration. It never contains tokens, passwords, private keys, database URLs, application keys, cookies, certificate bodies, or the contents of `/opt/stack/.env`.

The phases are:

1. **Preflight** — perform read-only validation of local tools, GitHub authentication, Cloudflare permissions, target SSH access, and target OS support.
2. **Input validation** — collect required secrets and reject missing, placeholder, malformed, or conflicting values without printing them.
3. **Snapshot** — record current API DNS records, proxy state, TTL, Worker routes, old origin IP, and relevant non-secret GitHub settings.
4. **Bootstrap** — create the runtime user and directories, install rootless Podman and required host packages, configure linger/firewall policy, and write the runtime environment atomically.
5. **TLS preparation** — generate the private key and CSR locally, request a 5,475-day Cloudflare Origin Certificate for `admin-api.BASE_DOMAIN` and `school-api.BASE_DOMAIN`, stream the certificate and private key to root-owned files on the target, and validate the key/certificate pair.
6. **Deploy** — update GitHub settings through standard input, dispatch backend workflows in dependency order, deploy frontend-admin, and deploy tenant frontends.
7. **Origin verification** — validate Compose and Nginx, check both `/ready` endpoints directly against the target IP with the intended hostnames, verify TLS against the Cloudflare Origin CA, and run pre-cutover API checks.
8. **Cutover gate** — display an exact DNS diff, old and new origin IPs, proxy state, completed verification, and the rollback target. Continue only after an explicit confirmation entered at this gate.
9. **DNS cutover** — submit only the two approved API record patches through one Cloudflare DNS batch transaction, require them to remain Cloudflare Proxied, and poll both records through propagation.
10. **Post-cutover verification** — verify public readiness, service identity, login, APIs, File Platform upload/download, private image delivery, SSE/CORS, frontend-admin, and tenant frontend behavior.
11. **Handoff** — print a sanitized summary, checkpoint location, rollback command, and instruction to retain the old VPS until an operator separately decommissions it.

A checkpoint is written only after the phase's verification passes. Resume recomputes the configuration fingerprint and rechecks external resources before skipping a completed phase.

## TLS and DNS

The first release standardizes both API records as Cloudflare Proxied. The installer generates a private key and CSR locally, then requests a 5,475-day Cloudflare Origin Certificate containing only the two API hostnames. Transient local key material uses the same private temporary-storage and cleanup rules as the deployment SSH key and is never written to a checkpoint or log. On the VPS, the key is mode `0600` and readable only by the component that supplies it to Nginx.

Pre-cutover checks connect directly to the new IP while sending the future hostname. TLS is verified with the pinned Cloudflare Origin CA trust certificate; verification must not use `--insecure` or disable hostname checking.

Immediately before cutover, the installer re-reads both DNS records and compares their identifiers, content, proxy mode, and modification state with the snapshot. Any drift stops the run. Cutover uses Cloudflare's DNS batch endpoint so both patches execute in one database transaction; DNS propagation is not atomic, so the installer polls and reports each hostname independently until both resolve through Cloudflare. The two backends use the same existing databases and object storage, making a brief mixed-origin propagation window safe. Rollback restores the snapshotted record content, TTL, and proxy state through a second confirmed batch request.

## Deployment Ordering

The deployment order is intentionally strict:

1. canonical topology and Nginx definitions are present and validate on the target;
2. backend-admin is deployed and passes target-origin `/ready` and identity checks;
3. backend-school is deployed and passes readiness checks for backend-admin, R2, and clamd;
4. frontend-admin is deployed with Worker variables and secret bindings;
5. active tenant frontends are deployed through the existing all-schools workflow;
6. direct-origin verification passes;
7. the operator approves DNS cutover;
8. public smoke and browser checks pass.

Backend workflows must verify the selected target directly. They must not treat a request to the still-public old API hostname as evidence that the new VPS is ready.

## Failure Handling and Recovery

Every mutating operation follows `plan → apply → verify`. Operations are idempotent: an already-correct package, directory, GitHub setting, DNS record, certificate, or runtime resource is verified and reused rather than duplicated.

Transient network failures use bounded retries with backoff. Authentication, authorization, invalid configuration, unsupported OS, failed workflow, and resource drift errors stop immediately. GitHub workflow failures report the workflow run, job or step where available, and a log URL without copying secret-bearing output into installer state.

Before DNS cutover, a failure leaves the old production path unchanged and reports the resume command. After cutover, a failed smoke check presents the exact observed failures and offers a confirmed DNS rollback. It does not delete containers, volumes, databases, buckets, Workers, certificates, or either VPS.

If resume detects that a snapshotted external resource changed outside the run, it exits with a configuration-drift error. The operator must start a new plan or explicitly restore the expected external state; the installer never overwrites unknown changes.

All shell paths keep `set -x` disabled. Output redaction covers tokens, authorization headers, cookies, private keys, database URLs, encryption keys, blind-index keys, internal secrets, deployment keys, and R2 credentials.

## Verification Strategy

### Static checks

- Run `shellcheck` and the chosen shell formatter over installer code.
- Validate GitHub workflow syntax and action references.
- Run `podman-compose config` against a fixture environment.
- Run Nginx configuration validation for the rendered base domain.
- Run repository documentation-policy checks after adding and later removing workflow artifacts.

### Unit tests

Bats tests isolate modules behind fake `gh`, `curl`, `ssh`, `openssl`, and Compose commands. They cover:

- supported and unsupported OS detection;
- required input and placeholder rejection;
- secret redaction;
- checkpoint serialization without secret values;
- idempotent reruns;
- retry classification and limits;
- workflow success and failure parsing;
- DNS diff generation and drift detection;
- cutover and rollback confirmation gates;
- resume fingerprint validation.

### Integration tests

Integration tests exercise bootstrap behavior for Ubuntu LTS and Debian stable environments, including a second run against an already-prepared target and an interrupted/resumed run. Cloudflare and GitHub mutations use dedicated test resources or API fakes; production resources are not integration-test fixtures.

### Migration acceptance

A disposable VPS acceptance run must demonstrate:

- a complete dry run without mutation;
- installation and deployment to the new origin while the old origin remains public;
- direct TLS, Nginx, backend identity, and readiness verification;
- successful frontend-admin and tenant deployments;
- an intentional pre-cutover failure followed by resume;
- explicit DNS cutover and public smoke tests;
- login, API, file upload/download, private profile image loading, and notification SSE/CORS behavior;
- a DNS rollback drill using the saved checkpoint;
- no secrets in command output, workflow annotations, checkpoint files, or test artifacts.

## Acceptance Criteria

The design is complete when implementation satisfies all of the following:

1. One documented command orchestrates replacement-VPS migration from a trusted Linux/WSL workstation.
2. Ubuntu LTS and Debian stable targets pass the same preflight, bootstrap, resume, and verification behavior.
3. `podman-compose.yml` is the sole production Compose owner for backend-admin, backend-school, and clamd.
4. Frontend-admin deploys through GitHub Actions without a committed `INTERNAL_API_SECRET` or hard-coded Cloudflare account.
5. The new VPS passes direct-origin TLS and readiness checks before DNS changes.
6. DNS cannot change without a final explicit operator confirmation and a no-drift comparison.
7. A second run creates no duplicate resources, and an interrupted run resumes from verified checkpoints.
8. Failure before cutover leaves public production on the old VPS; failure after cutover offers a confirmed rollback.
9. Checkpoints, logs, workflow output, and committed files contain no secret values.
10. The installer never removes the old VPS or changes existing Neon databases, tenant data, or R2 object contents.

## External Constraints

- [Cloudflare Origin CA](https://developers.cloudflare.com/ssl/origin-configuration/origin-ca/) defines the API-token permission, hostname coverage, trust root, and supported certificate lifetime used by this design.
- [Cloudflare DNS batch records](https://developers.cloudflare.com/api/resources/dns/subresources/records/methods/batch/) executes record mutations in one database transaction but explicitly does not guarantee atomic propagation, which is why the installer polls both API hostnames independently.

## Future Extension

`fresh-install` will be a separate command that reuses preflight, configuration, GitHub, Cloudflare, VPS, deployment, verification, and state modules. Its separate design must define creation and lifecycle ownership for Neon, R2, initial tenant data, and first administrator credentials. Those responsibilities are deliberately absent from `migrate-vps`.
