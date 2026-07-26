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
		'git diff --check'
	];

	for (const value of required) {
		assert.ok(rules.includes(value), `.rules must contain: ${value}`);
	}
});
