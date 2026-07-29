# Notification SSE Proxy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver notification SSE keepalives through nginx and Cloudflare without recurring `524` disconnects.

**Architecture:** Route the exact `/api/notifications/stream` path through the existing unbuffered SSE nginx settings. Protect the boundary with a static test that resolves the effective nginx location for the production path and verifies its streaming contract.

**Tech Stack:** nginx, Node.js test runner, GitHub Actions, Podman, Axum SSE, Cloudflare.

## Global Constraints

- Keep backend-school's authentication, event payloads, and 15-second keepalive unchanged.
- Keep frontend reconnect behavior unchanged.
- Do not modify backend-admin or frontend-admin.
- Do not change database migrations, permissions, or API contracts.
- Never print credentials, cookies, tokens, or full request URLs during production verification.
- Execute inline in the current worktree without subagents.

---

### Task 1: Exact Notification SSE Proxy

**Files:**
- Create: `frontend-school/tests/static/notification-sse-proxy.test.mjs`
- Modify: `nginx-configs/school-api.schoolorbit.app.conf`

**Interfaces:**
- Consumes: authenticated `GET /api/notifications/stream`.
- Produces: an exact nginx location with immediate unbuffered SSE delivery.
- Preserves: upstream `http://schoolorbit-backend-school:8081` and tenant CORS behavior.

- [ ] **Step 1: Add the location-resolution regression test**

Create `frontend-school/tests/static/notification-sse-proxy.test.mjs`:

```javascript
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');

function extractLocationBlocks(source) {
	const blocks = [];
	const locationPattern = /^\s*location\s+(?:(=|\^~|~\*?)\s+)?([^\s{]+)\s*\{/gm;

	for (const match of source.matchAll(locationPattern)) {
		let depth = 1;
		let cursor = match.index + match[0].length;
		while (cursor < source.length && depth > 0) {
			if (source[cursor] === '{') depth += 1;
			if (source[cursor] === '}') depth -= 1;
			cursor += 1;
		}
		assert.equal(depth, 0, `unterminated nginx location ${match[2]}`);
		blocks.push({
			modifier: match[1] ?? '',
			pattern: match[2],
			body: source.slice(match.index + match[0].length, cursor - 1)
		});
	}

	return blocks;
}

function resolveLocation(blocks, requestPath) {
	const exact = blocks.find(
		(block) => block.modifier === '=' && block.pattern === requestPath
	);
	if (exact) return exact;

	const prefixes = blocks
		.filter(
			(block) =>
				block.modifier !== '~' &&
				block.modifier !== '~*' &&
				requestPath.startsWith(block.pattern)
		)
		.sort((left, right) => right.pattern.length - left.pattern.length);
	const longestPrefix = prefixes[0];
	if (longestPrefix?.modifier === '^~') return longestPrefix;

	const regex = blocks.find(
		(block) =>
			(block.modifier === '~' || block.modifier === '~*') &&
			new RegExp(block.pattern, block.modifier === '~*' ? 'i' : '').test(requestPath)
	);
	return regex ?? longestPrefix;
}

test('notification stream resolves to the unbuffered credentialed SSE proxy', async () => {
	const source = await readFile(
		path.join(repoRoot, 'nginx-configs/school-api.schoolorbit.app.conf'),
		'utf8'
	);
	const location = resolveLocation(extractLocationBlocks(source), '/api/notifications/stream');

	assert.ok(location, 'notification stream must resolve to an nginx location');
	assert.equal(location.modifier, '=');
	assert.equal(location.pattern, '/api/notifications/stream');
	assert.match(location.body, /proxy_pass\s+http:\/\/schoolorbit-backend-school:8081;/);
	assert.match(location.body, /proxy_buffering\s+off;/);
	assert.match(location.body, /proxy_cache\s+off;/);
	assert.match(location.body, /proxy_http_version\s+1\.1;/);
	assert.match(location.body, /proxy_set_header\s+Connection\s+"";/);
	assert.match(location.body, /chunked_transfer_encoding\s+on;/);
	assert.match(location.body, /proxy_read_timeout\s+24h;/);
	assert.match(location.body, /proxy_send_timeout\s+24h;/);
	assert.match(
		location.body,
		/add_header\s+'Access-Control-Allow-Origin'\s+\$allow_origin\s+always;/
	);
	assert.match(
		location.body,
		/add_header\s+'Access-Control-Allow-Credentials'\s+'true'\s+always;/
	);
});
```

- [ ] **Step 2: Run the focused test and confirm RED**

Run:

```bash
cd frontend-school
node --test tests/static/notification-sse-proxy.test.mjs
```

Expected: FAIL because `/api/notifications/stream` resolves to the ordinary `/`
location rather than an exact unbuffered SSE location.

- [ ] **Step 3: Replace the stale nginx SSE location**

In `nginx-configs/school-api.schoolorbit.app.conf`, change only the location
selector:

```nginx
location = /api/notifications/stream {
```

Keep the existing proxy, streaming, timeout, forwarded-header, CORS, and OPTIONS
directives inside the block unchanged.

- [ ] **Step 4: Run the focused test and confirm GREEN**

Run:

```bash
cd frontend-school
node --test tests/static/notification-sse-proxy.test.mjs
```

Expected: PASS with the production notification path resolving to the exact SSE
location.

- [ ] **Step 5: Run the frontend verification matrix**

Run:

```bash
cd frontend-school
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Expected: lint passes, Svelte Check reports 0 errors and 0 warnings, and all
static tests pass.

- [ ] **Step 6: Commit the tested proxy fix**

```bash
git add frontend-school/tests/static/notification-sse-proxy.test.mjs \
  nginx-configs/school-api.schoolorbit.app.conf
git commit -m "fix: stream notifications through nginx"
```

---

### Task 2: Deploy and Verify the Realtime Path

**Files:**
- Deploy: `.github/workflows/deploy-backend-school.yml`
- Verify: `scripts/smoke_test.sh`

**Interfaces:**
- Consumes: the exact notification SSE location from Task 1.
- Produces: a validated and reloaded production nginx configuration.

- [ ] **Step 1: Review and push the completed change**

Run:

```bash
git diff HEAD~1 --check
git diff HEAD~1 --stat
git status --short
git push origin main
```

Expected: no whitespace errors, a clean worktree after the commit, and
`origin/main` advances to the tested commit.

- [ ] **Step 2: Monitor the backend deployment**

Find the run for the pushed commit:

```bash
gh run list --workflow deploy-backend-school.yml --limit 5 \
  --json databaseId,headSha,status,conclusion,url
```

Watch it with:

```bash
gh run watch <run-id> --exit-status
```

Expected: build, backend readiness, `nginx -t`, configuration installation, and
nginx reload all succeed.

- [ ] **Step 3: Run the authenticated production smoke test**

Run with the ignored local smoke environment:

```bash
SMOKE_ENV_FILE=.env.smoke.local \
SMOKE_SUBDOMAIN=snwsb \
SMOKE_ORIGIN=https://snwsb.schoolorbit.app \
SMOKE_TENANT_URL=https://snwsb.schoolorbit.app \
SMOKE_TIMEOUT_SECONDS=60 \
scripts/smoke_test.sh
```

Expected: health, readiness, CORS, login, and authenticated checks pass without
printing credentials.

- [ ] **Step 4: Verify multiple production SSE keepalives**

Use an in-memory authenticated Node fetch. Abort after receiving two stream
chunks or after 35 seconds. Report only:

```text
status=200
allowOrigin=https://snwsb.schoolorbit.app
contentType=text/event-stream
chunkCount>=2
firstChunkMs<=16000
```

Never print the cookie or the full request URL.

- [ ] **Step 5: Confirm final repository and deployment state**

Run:

```bash
git fetch origin main
git diff --check
git status --short
git rev-parse HEAD
git rev-parse origin/main
```

Expected: clean worktree and identical local/remote commit hashes.
