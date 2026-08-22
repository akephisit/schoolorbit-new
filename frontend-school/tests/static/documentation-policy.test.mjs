import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { access, readFile } from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const execFileAsync = promisify(execFile);
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

const MARKDOWN_ALLOWLIST = [
	'AGENTS.md',
	'CLAUDE.md',
	'GEMINI.md',
	'README.md',
	'TODO.md',
	'backend-admin/README.md',
	'backend-school/README.md',
	'docs/OPERATIONS.md',
	'docs/PODMAN_SETUP.md',
	'docs/README.md',
	'docs/TESTING.md',
	'frontend-admin/README.md',
	'frontend-school/README.md'
].sort();

const SUPERPOWERS_SPEC_PATTERN =
	/^docs\/superpowers\/specs\/\d{4}-\d{2}-\d{2}-[a-z0-9]+(?:-[a-z0-9]+)*-design\.md$/;
const SUPERPOWERS_PLAN_PATTERN =
	/^docs\/superpowers\/plans\/\d{4}-\d{2}-\d{2}-[a-z0-9]+(?:-[a-z0-9]+)*\.md$/;

function isAllowedMarkdown(relativePath) {
	return (
		MARKDOWN_ALLOWLIST.includes(relativePath) ||
		SUPERPOWERS_SPEC_PATTERN.test(relativePath) ||
		SUPERPOWERS_PLAN_PATTERN.test(relativePath)
	);
}

async function existingRepositoryMarkdown() {
	const { stdout } = await execFileAsync(
		'git',
		['ls-files', '--cached', '--others', '--exclude-standard', '*.md'],
		{ cwd: repoRoot }
	);
	const paths = stdout
		.split(/\r?\n/)
		.map((value) => value.trim())
		.filter(Boolean);
	const existing = [];

	for (const relativePath of paths) {
		try {
			await access(path.join(repoRoot, relativePath));
			existing.push(relativePath);
		} catch {
			// A tracked file deleted in the current worktree is not active documentation.
		}
	}

	return existing.sort();
}

function localLinkTargets(source) {
	const targets = [];
	const pattern = /!?\[[^\]]*]\(([^)]+)\)/g;

	for (const match of source.matchAll(pattern)) {
		const rawTarget = match[1].trim().replace(/^<|>$/g, '');
		const target = rawTarget.split(/\s+["']/)[0];
		if (!target || target.startsWith('#') || /^(?:https?:|mailto:|tel:)/i.test(target)) {
			continue;
		}
		targets.push(decodeURIComponent(target.split('#')[0]));
	}

	return targets;
}

test('Superpowers Markdown is limited to dated spec and plan artifacts', () => {
	const accepted = [
		'docs/superpowers/specs/2026-07-26-admin-auth-design.md',
		'docs/superpowers/plans/2026-07-26-admin-auth.md'
	];
	const rejected = [
		'docs/superpowers/README.md',
		'docs/superpowers/specs/admin-auth-design.md',
		'docs/superpowers/specs/2026-07-26-admin-auth.md',
		'docs/superpowers/plans/admin-auth.md',
		'docs/superpowers/notes/2026-07-26-admin-auth.md',
		'docs/another-plan.md'
	];

	for (const relativePath of accepted) {
		assert.equal(isAllowedMarkdown(relativePath), true, `must allow ${relativePath}`);
	}
	for (const relativePath of rejected) {
		assert.equal(isAllowedMarkdown(relativePath), false, `must reject ${relativePath}`);
	}
});

test('repository Markdown is limited to canonical docs and Superpowers artifacts', async () => {
	const existing = await existingRepositoryMarkdown();
	const missingCanonical = MARKDOWN_ALLOWLIST.filter(
		(relativePath) => !existing.includes(relativePath)
	);
	const unexpected = existing.filter((relativePath) => !isAllowedMarkdown(relativePath));

	assert.deepEqual(missingCanonical, []);
	assert.deepEqual(unexpected, []);
});

test('canonical Markdown local links resolve', async () => {
	const broken = [];

	for (const relativePath of await existingRepositoryMarkdown()) {
		const source = await readFile(path.join(repoRoot, relativePath), 'utf8');
		for (const target of localLinkTargets(source)) {
			const resolved = path.resolve(repoRoot, path.dirname(relativePath), target);
			try {
				await access(resolved);
			} catch {
				broken.push(`${relativePath} -> ${target}`);
			}
		}
	}

	assert.deepEqual(broken, []);
});

test('project rules own durable development and verification workflows', async () => {
	const rules = await readFile(path.join(repoRoot, '.rules'), 'utf8');
	const required = [
		'## 0. Rule Ownership and Documentation Policy',
		'## 1. Required Analysis Workflow',
		'## 2. Adding or Changing a Feature',
		'## 4. Permissions and Resource Authorization',
		'## 5. API Contracts',
		'## 6. Database Migrations',
		'## 7. Frontend: SvelteKit 5',
		'## 9. Security, PDPA, and Logging',
		'## 11. Verification Matrix',
		'TODO.md',
		'docs/superpowers/specs/',
		'docs/superpowers/plans/',
		'contracts/permissions.json',
		'npm run generate:permissions',
		'npm run check:permissions',
		'npm run test:permissions',
		'npm run generate:api-contracts',
		'npm run check:api-contracts',
		'npm run test:api-contracts',
		'cargo test --test static_architecture',
		'podman-compose.yml` is the sole production Compose owner',
		'bats scripts/tests/installer',
		'shellcheck scripts/schoolorbit-installer',
		'shfmt -d -i 4 -ci scripts/schoolorbit-installer',
		'node --test frontend-school/tests/static/deployment-installer.test.mjs',
		'git diff --check'
	];

	for (const value of required) {
		assert.ok(rules.includes(value), `.rules must contain: ${value}`);
	}
});

test('canonical docs own the school session and cutover contract', async () => {
	const [rules, testing, operations, podmanSetup, backendReadme, todo] = await Promise.all([
		readFile(path.join(repoRoot, '.rules'), 'utf8'),
		readFile(path.join(repoRoot, 'docs/TESTING.md'), 'utf8'),
		readFile(path.join(repoRoot, 'docs/OPERATIONS.md'), 'utf8'),
		readFile(path.join(repoRoot, 'docs/PODMAN_SETUP.md'), 'utf8'),
		readFile(path.join(repoRoot, 'backend-school/README.md'), 'utf8'),
		readFile(path.join(repoRoot, 'TODO.md'), 'utf8')
	]);

	assert.match(rules, /AuthenticatedSession/);
	assert.match(rules, /__Host-schoolorbit_session/);
	assert.match(rules, /SESSION_HMAC_KEY/);
	assert.doesNotMatch(rules, /current_user_tenant_context_from_claims/);
	assert.match(testing, /X-CSRF-Token/);
	assert.match(testing, /session-security\.spec\.ts/);
	assert.match(operations, /SCHOOL_ROLLBACK_JWT_SECRET/);
	assert.match(operations, /thirty-day|30-day/i);
	assert.match(podmanSetup, /SESSION_HMAC_KEY/);
	assert.match(backendReadme, /SESSION_HMAC_KEY/);
	assert.doesNotMatch(todo, /\bAUTH-001\b/);

	const auth002 = todo.match(/AUTH-002[\s\S]*?(?=\n- \[[ x]\] \*\*|\n##|$)/)?.[0] ?? '';
	assert.match(auth002, /\/api\/auth\/me\/profile/);
	assert.match(auth002, /default `\/api\/auth\/me` minimization is complete/i);
});

test('canonical docs own the school-font rollout and lifecycle contract', async () => {
	const [operations, testing] = await Promise.all([
		readFile(path.join(repoRoot, 'docs/OPERATIONS.md'), 'utf8'),
		readFile(path.join(repoRoot, 'docs/TESTING.md'), 'utf8')
	]);

	assert.match(operations, /040_school_font_library\.sql/);
	assert.match(operations, /legacy certificate template fonts must be empty/i);
	assert.match(operations, /school_font[\s\S]*private/i);
	assert.match(operations, /fix forward|fail-forward/i);
	assert.match(operations, /reference-safe|reference count/i);
	assert.match(operations, /reconciler/i);
	assert.match(testing, /modules::school_fonts/);
	assert.match(testing, /school-font-library\.spec\.ts/);
	assert.match(testing, /certificate-lifecycle\.spec\.ts/);
	assert.match(testing, /survives campaign purge/i);
});
