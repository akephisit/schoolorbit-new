# Podman Setup Guide Restoration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore a safe, current Thai Podman installation guide as the explicitly approved twelfth permanent Markdown document.

**Architecture:** `docs/PODMAN_SETUP.md` explains server bootstrap while executable topology remains owned by `podman-compose.yml`, Nginx files, and deployment workflows. The documentation policy is expanded from 11 to 12 files, and the existing link/allowlist guard prevents the restored guide from drifting out of the canonical set.

**Tech Stack:** Markdown, Node.js built-in test runner, GitHub Actions, Podman, podman-compose, Cockpit, Nginx

## Global Constraints

- The guide is Thai-language and targets Debian/Ubuntu servers.
- Do not restore the old guide verbatim.
- Do not duplicate complete Compose or Nginx configuration in Markdown.
- Do not recommend Cockpit root login.
- Do not include real secrets or production credentials.
- Derive commands, paths, ports, environment names, health endpoints, and container names from current tracked executable sources.
- Keep `ENCRYPTION_KEY` and `BLIND_INDEX_KEY` stable after encrypted data exists.
- The final repository contains exactly 12 tracked Markdown files.
- The temporary design and implementation plan are deleted before final verification.

---

### Task 1: Expand the Canonical Documentation Policy

**Files:**

- Modify: `.rules`
- Modify: `frontend-school/tests/static/documentation-policy.test.mjs`
- Delete: `docs/superpowers/specs/2026-07-26-podman-setup-restoration-design.md`
- Delete: `docs/superpowers/plans/2026-07-26-podman-setup-restoration.md`

**Interfaces:**

- Consumes: the current 11-path `MARKDOWN_ALLOWLIST` and `.rules` documentation ownership policy.
- Produces: a 12-path policy whose only missing document during the red test is `docs/PODMAN_SETUP.md`.

- [ ] **Step 1: Re-read the executable deployment sources**

Run:

```bash
sed -n '1,180p' podman-compose.yml
sed -n '1,180p' .github/workflows/deploy-backend-admin.yml
sed -n '1,180p' .github/workflows/deploy-backend-school.yml
sed -n '1,240p' nginx-configs/school-api.schoolorbit.app.conf
sed -n '1,180p' backend-admin/nginx.conf.example
```

Expected: confirm `/opt/stack`, container names, `schoolorbit-net`, ports `8080`/`8081`, `/ready`, and current proxy references before documenting them.

- [ ] **Step 2: Remove the temporary spec and plan**

Use `apply_patch` to delete exactly:

```text
docs/superpowers/specs/2026-07-26-podman-setup-restoration-design.md
docs/superpowers/plans/2026-07-26-podman-setup-restoration.md
```

Expected: the active Markdown inventory returns to the current 11 permanent files.

- [ ] **Step 3: Write the failing allowlist change**

Add this entry to `MARKDOWN_ALLOWLIST` in `frontend-school/tests/static/documentation-policy.test.mjs`:

```js
'docs/PODMAN_SETUP.md',
```

Update the test name to:

```js
test('tracked Markdown is limited to the approved canonical documentation set', async () => {
```

In `.rules`:

- change “Keep only these Markdown entry points” to “Keep only these 12 Markdown entry points”;
- add `docs/PODMAN_SETUP.md` beside the other `docs/` paths;
- change the proposed-exception wording from “twelfth Markdown file” to “thirteenth Markdown file”.

- [ ] **Step 4: Run the policy test and verify RED**

Run:

```bash
cd frontend-school
npm run check:docs
```

Expected:

- the allowlist test fails because expected `docs/PODMAN_SETUP.md` does not exist;
- canonical local-link and `.rules` workflow tests pass;
- no temporary spec/plan appears in the actual Markdown list.

---

### Task 2: Restore the Current Podman Installation Guide

**Files:**

- Create: `docs/PODMAN_SETUP.md`
- Modify: `docs/README.md`
- Modify: `docs/OPERATIONS.md`
- Test: `frontend-school/tests/static/documentation-policy.test.mjs`

**Interfaces:**

- Consumes: `podman-compose.yml`, `.env.example`, `backend-admin/.env.example`, `backend-school/.env.example`, Nginx references, deployment workflows, and the 12-path policy from Task 1.
- Produces: a permanent Thai operator guide linked from the canonical documentation index and operations guide.

- [ ] **Step 1: Create the guide with current ownership boundaries**

Create `docs/PODMAN_SETUP.md` with these headings:

```markdown
# คู่มือติดตั้ง SchoolOrbit ด้วย Podman

## ขอบเขตและโครงสร้างระบบ
## 1. เตรียม Server
## 2. ติดตั้ง Podman และเครื่องมือ
## 3. เตรียม `/opt/stack`
## 4. เตรียม Environment Variables
## 5. เตรียม Nginx, DNS และ TLS
## 6. เริ่ม Backend Services
## 7. ตรวจสอบหลังติดตั้ง
## 8. เชื่อมต่อ GitHub Deployment
## 9. อัปเดตและย้อนกลับ
## 10. แก้ปัญหาเบื้องต้น
## เอกสารที่เกี่ยวข้อง
```

Required content:

- supported Debian/Ubuntu assumption and a normal `sudo` user;
- installation of `podman`, `podman-compose`, `cockpit`, `cockpit-podman`, `git`, `curl`, and `ca-certificates`;
- optional Cockpit access on port `9090` without enabling root login;
- `/opt/stack` ownership and repository checkout/update;
- production `.env` groups matching current Compose variables without sample secret values;
- `podman-compose -f podman-compose.yml config` before `up -d`;
- use of `podman-compose.yml` rather than an embedded Compose copy;
- Nginx references and requirement that `schoolorbit-nginx` join `schoolorbit-net`;
- DNS/TLS prerequisites without claiming Cloudflare proxy mode is universally fixed;
- `/health` liveness and `/ready` deployment gating;
- `podman ps`, `podman network inspect schoolorbit-net`, and bounded log commands;
- workflow expectation that the repo/compose file is under `/opt/stack`;
- safe image pull/recreate/rollback guidance that never deletes volumes or databases;
- explicit warning to preserve `ENCRYPTION_KEY` and `BLIND_INDEX_KEY`;
- links to `podman-compose.yml`, Nginx references, `OPERATIONS.md`, and `TESTING.md`.

- [ ] **Step 2: Link the guide from canonical documents**

Add to `docs/README.md`:

```markdown
- [Podman server setup](./PODMAN_SETUP.md)
```

Add a sentence in `docs/OPERATIONS.md` under `## Runtime Topology`:

```markdown
For first-time production server bootstrap, follow [Podman server setup](./PODMAN_SETUP.md).
```

- [ ] **Step 3: Run the policy test and verify GREEN**

Run:

```bash
cd frontend-school
npm run check:docs
```

Expected: 3 tests pass and 0 fail.

- [ ] **Step 4: Verify current names and reject stale instructions**

Run from the repository root:

```bash
rg -n \
  "podman-compose.yml|/opt/stack|schoolorbit-net|schoolorbit-nginx|/health|/ready|ENCRYPTION_KEY|BLIND_INDEX_KEY" \
  docs/PODMAN_SETUP.md

rg -n -i \
  "root login|disallowed-users|ADMIN_DATABASE_URL|pgcrypto|ALTER ROLE|178 unique operations" \
  docs/PODMAN_SETUP.md
```

Expected:

- the first command finds every current operational invariant;
- the second command returns no matches.

- [ ] **Step 5: Run affected static verification**

Run:

```bash
cd frontend-school
npm run check:docs
npm run test:static
```

Expected: documentation policy and frontend static suites pass.

- [ ] **Step 6: Verify the final inventory and diff**

Run from the repository root:

```bash
git ls-files '*.md' | while IFS= read -r file; do
  if [ -f "$file" ]; then printf '%s\n' "$file"; fi
done | sort

git diff --check
git status --short
```

Expected:

- exactly 12 Markdown paths;
- `docs/PODMAN_SETUP.md` is the only new permanent Markdown path;
- no temporary spec/plan remains;
- no whitespace errors.

- [ ] **Step 7: Commit the restoration**

```bash
git add \
  .rules \
  docs/PODMAN_SETUP.md \
  docs/README.md \
  docs/OPERATIONS.md \
  frontend-school/tests/static/documentation-policy.test.mjs
git add -u \
  docs/superpowers/specs/2026-07-26-podman-setup-restoration-design.md \
  docs/superpowers/plans/2026-07-26-podman-setup-restoration.md
git commit -m "docs: restore current Podman setup guide"
```
