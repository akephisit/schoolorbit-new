import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { access, readFile } from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { parse as parseYaml } from 'yaml';

const execFileAsync = promisify(execFile);
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');
const skillRoot = path.join(repoRoot, '.agents/skills/schoolorbit-development');

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

const WORKFLOW_SPEC_PATTERN =
	/^docs\/workflows\/specs\/\d{4}-\d{2}-\d{2}-[a-z0-9]+(?:-[a-z0-9]+)*-design\.md$/;
const WORKFLOW_PLAN_PATTERN =
	/^docs\/workflows\/plans\/\d{4}-\d{2}-\d{2}-[a-z0-9]+(?:-[a-z0-9]+)*\.md$/;

function isAllowedMarkdown(relativePath) {
	return (
		MARKDOWN_ALLOWLIST.includes(relativePath) ||
		relativePath === '.agents/skills/schoolorbit-development/SKILL.md' ||
		WORKFLOW_SPEC_PATTERN.test(relativePath) ||
		WORKFLOW_PLAN_PATTERN.test(relativePath)
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

function requiredSection(source, startHeading, endHeading) {
	const start = source.indexOf(startHeading);
	assert.notEqual(start, -1, `.rules must contain section: ${startHeading}`);
	const end = source.indexOf(endHeading, start + startHeading.length);
	assert.notEqual(end, -1, `.rules must contain section boundary: ${endHeading}`);
	return source.slice(start, end);
}

function assertMapping(value, label) {
	assert.ok(
		value !== null && typeof value === 'object' && !Array.isArray(value),
		`${label} must be a mapping`
	);
}

function assertNonblankString(value, label) {
	assert.equal(typeof value, 'string', `${label} must be a string`);
	assert.ok(value.trim(), `${label} must not be blank`);
}

test('project skill has valid YAML frontmatter and an instruction body', async () => {
	const source = await readFile(path.join(skillRoot, 'SKILL.md'), 'utf8');
	const frontmatter = source.match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/);
	assert.ok(frontmatter, 'SKILL.md must start with delimited YAML frontmatter');
	const metadata = parseYaml(frontmatter[1]);
	assertMapping(metadata, 'skill frontmatter');
	assert.equal(metadata.name, path.basename(skillRoot), 'skill name must match its directory');
	assertNonblankString(metadata.description, 'skill description');
	assert.ok(source.slice(frontmatter[0].length).trim(), 'skill must contain instructions');
});

test('project skill invocation metadata has valid types and references the skill', async () => {
	const metadata = parseYaml(await readFile(path.join(skillRoot, 'agents/openai.yaml'), 'utf8'));
	assertMapping(metadata, 'agent metadata');
	assertMapping(metadata.interface, 'agent interface');
	for (const field of ['display_name', 'short_description', 'default_prompt']) {
		assertNonblankString(metadata.interface[field], `interface.${field}`);
	}
	assert.match(
		metadata.interface.default_prompt,
		new RegExp(`\\$${path.basename(skillRoot)}(?![\\w-])`),
		'default prompt must reference this skill'
	);
	if (metadata.policy !== undefined) {
		assertMapping(metadata.policy, 'agent policy');
		const implicitInvocation = metadata.policy.allow_implicit_invocation;
		assert.ok(
			implicitInvocation == null || typeof implicitInvocation === 'boolean',
			'policy.allow_implicit_invocation must be a boolean when supplied, or null'
		);
	}
});

test('every implementation delegates edits and keeps independent acceptance', async () => {
	const [rules, skill, metadataSource, testing] = await Promise.all([
		readFile(path.join(repoRoot, '.rules'), 'utf8'),
		readFile(path.join(skillRoot, 'SKILL.md'), 'utf8'),
		readFile(path.join(skillRoot, 'agents/openai.yaml'), 'utf8'),
		readFile(path.join(repoRoot, 'docs/TESTING.md'), 'utf8')
	]);
	const coordination = requiredSection(
		rules,
		'### Agent roles and coordination',
		'### Reports, failure recovery and completion'
	);
	// The approved policy replaces the obsolete trivial-inline exception; these are
	// instruction guards, not proof that a host ran separate agents.
	assert.match(coordination, /Every implementation task, however small/);
	assert.match(coordination, /separate delegated writer/);
	assert.match(coordination, /separate read-only Reviewer and a separate QA\/Verifier/);
	assert.match(coordination, /neither of whom wrote the candidate/);
	assert.match(coordination, /includes text, documentation, workflow and test edits/);
	assert.match(
		coordination,
		/Senior does not write tracked\s+source, configuration, tests or documentation/
	);
	assert.match(coordination, /including workflow changes and failure repairs/);
	assert.match(coordination, /Explanatory\/read-only questions do not need writers/);
	assert.match(
		coordination,
		/Explicit newer user instructions and higher-priority host requirements/
	);
	assert.match(coordination, /subagents\s+are unavailable, report the concrete blocker/);
	assert.match(coordination, /never silently substitute inline roles/);
	assert.match(coordination, /Senior \+ one relevant writer/);
	assert.match(coordination, /then Senior \+ Reviewer \+ QA\/Verifier/);
	assert.match(coordination, /do not create a Backend writer for frontend-only work/);
	assert.match(coordination, /proportional checks/);
	for (const source of [
		rules,
		skill,
		parseYaml(metadataSource).interface.default_prompt,
		testing
	]) {
		assert.match(source, /delegated writer/);
		assert.match(source, /concrete blocker/);
		assert.doesNotMatch(
			source,
			/(?:may (?:write|implement) inline|carry out the roles inline|explicit inline exceptions|Only trivial text\/mechanical edits|nontrivial changes still receive)/i,
			'entry points must not restore size-based or unavailable-subagent inline exceptions'
		);
	}
});

test('development workflow checks feature ancestry and coordinates latest-main integration', async () => {
	const [rules, skill, testing] = await Promise.all([
		readFile(path.join(repoRoot, '.rules'), 'utf8'),
		readFile(path.join(skillRoot, 'SKILL.md'), 'utf8'),
		readFile(path.join(repoRoot, 'docs/TESTING.md'), 'utf8')
	]);
	const integration = requiredSection(
		rules,
		'### Git branches, worktrees, and integration',
		'### Agent roles and coordination'
	);
	assert.match(integration, /Before final acceptance, readiness and integration/);
	assert.match(integration, /Senior fetches `origin`/);
	assert.match(integration, /git merge-base --is-ancestor origin\/main HEAD/);
	assert.match(integration, /fresh local `main` alone does not prove/);
	assert.match(
		integration,
		/pause writers[\s\S]*acknowledgements[\s\S]*save their work[\s\S]*clean tree/
	);
	assert.match(integration, /Senior merges `origin\/main` into the feature branch/);
	assert.match(integration, /semantic conflict resolution to a writer/);
	assert.match(integration, /repeat affected review and focused\/matrix checks/);
	assert.match(
		integration,
		/push the updated candidate and re-establish the latest-main `Pipeline gate`/
	);
	assert.match(integration, /merge controller\/protection serializes integration, rechecks latest/);
	assert.match(integration, /another feature merges meanwhile, repeat/);
	assert.match(integration, /never use a blind untested GitHub update-branch action/);
	assert.match(integration, /Concurrent distinct features need separate branches and checkouts/);
	assert.match(integration, /one checkout cannot hold two branches simultaneously/);
	assert.match(integration, /only one checkout is requested, serialize feature-writing phases/);
	for (const source of [skill, testing]) {
		assert.match(source, /git merge-base --is-ancestor origin\/main HEAD/);
		assert.match(source, /semantic\s+conflict/);
		assert.match(source, /latest-main `Pipeline gate`/);
		assert.match(source, /serialize feature-writing phases/);
	}
	const coordination = requiredSection(
		rules,
		'### Agent roles and coordination',
		'### Reports, failure recovery and completion'
	);
	assert.match(coordination, /one Rust build\/verification owner at a time/);
	assert.match(coordination, /compatible Cargo target\s+sequentially/);
	assert.match(
		coordination,
		/never run concurrent Cargo processes[\s\S]*or create per-agent targets/
	);
});

test('test integrity permits evidenced corrections while preserving regression coverage', async () => {
	const [rules, skill, testing] = await Promise.all([
		readFile(path.join(repoRoot, '.rules'), 'utf8'),
		readFile(path.join(skillRoot, 'SKILL.md'), 'utf8'),
		readFile(path.join(repoRoot, 'docs/TESTING.md'), 'utf8')
	]);
	const integrity = requiredSection(
		rules,
		'### Test integrity and legitimate corrections',
		'### Selecting verification scope'
	);
	for (const boundary of [
		/approved behavior is unchanged, including a refactor/,
		/fix the production cause/,
		/approved behavior\/contract change/,
		/evidenced faulty or brittle test/,
		/environment\/fixture defect/,
		/Retain equivalent-or-stronger coverage/,
		/implementation-detail checks to the observable contract/,
		/writer records the old and new requirement/,
		/expectation, fixture or snapshot change is valid/,
		/reproducer and before\/after evidence where feasible/,
		/PR[\s\S]*approved policy intentionally changed/,
		/Reviewer independently assesses test changes/,
		/QA checks actual outcomes and regression detection/,
		/Never remove cases, weaken assertions, disable\/skip required checks, blindly bless snapshots, add retries or narrow coverage merely to get green/
	]) {
		assert.match(integrity, boundary);
	}
	assert.match(skill, /Test integrity and legitimate corrections/);
	assert.match(skill, /Reviewer independently assesses changes and QA checks actual/);
	assert.match(testing, /test-integrity-and-legitimate-corrections/);
	assert.match(
		testing,
		/approved workflow-policy change can legitimately replace an obsolete guard/
	);
});

test('development workflow routes reports through recovery and verified delivery to a summary', async () => {
	const [rules, skill, testing] = await Promise.all([
		readFile(path.join(repoRoot, '.rules'), 'utf8'),
		readFile(path.join(skillRoot, 'SKILL.md'), 'utf8'),
		readFile(path.join(repoRoot, 'docs/TESTING.md'), 'utf8')
	]);
	const completion = requiredSection(
		rules,
		'### Reports, failure recovery and completion',
		'### Local verification and GitHub delivery'
	);
	// These guards check instruction boundaries, not live agent behavior or delivery success.
	for (const boundary of [
		/Child reports are handoffs to the Senior/,
		/failed required check or blocking review finding starts a repair loop/,
		/Senior, who diagnoses the issue and assigns the correction to the responsible writer/,
		/reassign the correction when diagnosis stalls/,
		/do not repeat guesses\s+or let the Senior silently edit files itself/,
		/QA never fixes source; Reviewer acceptance and\s+QA acceptance remain separate/,
		/A failure existing on main is not by itself a reason to stop/,
		/A draft PR[^.]*is not a stopping condition/,
		/Ordinary implementation requests include PR delivery and eligible trusted-team integration/,
		/track CI and automatic merge to a verified terminal result/,
		/Do not invent a deployment when the planner selects none/,
		/concise user-facing summary/,
		/actual model\/reasoning settings or fallbacks/,
		/known causes and resolutions/,
		/remaining blockers, limitations or work/,
		/blocked handoff must not claim the task is complete/
	]) {
		assert.match(completion, boundary);
	}
	assert.match(skill, /Reports, failure recovery\s+and completion/);
	assert.match(skill, /Both report to the\s+Senior/);
	assert.match(testing, /\.\.\/\.rules#reports-failure-recovery-and-completion/);
	assert.match(testing, /Static checks alone do not establish\s+that these scenarios ran/);
});

test('Workflow Markdown is limited to dated spec and plan artifacts', () => {
	const accepted = [
		'.agents/skills/schoolorbit-development/SKILL.md',
		'docs/workflows/specs/2026-07-26-admin-auth-design.md',
		'docs/workflows/plans/2026-07-26-admin-auth.md'
	];
	const rejected = [
		'.agents/skills/another-skill/SKILL.md',
		'.agents/skills/schoolorbit-development/README.md',
		'docs/workflows/README.md',
		'docs/workflows/specs/admin-auth-design.md',
		'docs/workflows/specs/2026-07-26-admin-auth.md',
		'docs/workflows/plans/admin-auth.md',
		'docs/workflows/notes/2026-07-26-admin-auth.md',
		'docs/another-plan.md'
	];

	for (const relativePath of accepted) {
		assert.equal(isAllowedMarkdown(relativePath), true, `must allow ${relativePath}`);
	}
	for (const relativePath of rejected) {
		assert.equal(isAllowedMarkdown(relativePath), false, `must reject ${relativePath}`);
	}
});

test('repository Markdown is limited to canonical docs and workflow artifacts', async () => {
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
		'## 7. Frontend: SvelteKit 3 / Svelte 5',
		'## 9. Security, PDPA, and Logging',
		'## 11. Verification Matrix',
		'### Canonical replacements and data preservation',
		'TODO.md',
		'docs/workflows/specs/',
		'docs/workflows/plans/',
		'contracts/permissions.json',
		'npm run generate:permissions',
		'npm run check:permissions',
		'npm run test:permissions',
		'npm run generate:api-contracts',
		'npm run check:api-contracts',
		'npm run test:api-contracts',
		'podman-compose.yml` is the sole production Compose owner',
		'Persisted legacy data is migration input, not a runtime compatibility requirement.',
		'expand → migrate/backfill → reconcile → switch consumers → contract/cleanup',
		'Legacy runtime compatibility is not a default requirement.',
		'./scripts/pipeline verify --scope auto',
		'git diff --check'
	];

	for (const value of required) {
		assert.ok(rules.includes(value), `.rules must contain: ${value}`);
	}

	const pipeline = await readFile(path.join(repoRoot, 'scripts/pipeline.mjs'), 'utf8');
	const deployment = await readFile(path.join(repoRoot, 'scripts/verify_deployment.sh'), 'utf8');
	assert.match(pipeline, /'--test', 'database_execution_architecture'/);
	assert.match(pipeline, /'--test', 'static_architecture'/);
	for (const command of [
		'bats scripts/tests/installer',
		'shellcheck -x',
		'shfmt -d -i 4 -ci',
		'node --test frontend-school/tests/static/deployment-installer.test.mjs'
	]) {
		assert.ok(deployment.includes(command), `Owned deployment runner must retain: ${command}`);
	}

	const canonicalPolicy = requiredSection(
		rules,
		'### Canonical replacements and data preservation',
		'## 3. Backend: Rust, Axum, SQLx'
	);
	assert.match(canonicalPolicy, /do not silently discard values/);
	assert.match(canonicalPolicy, /owner, exact scope, removal condition, and cleanup step/);
	assert.match(canonicalPolicy, /PDPA requirements/);

	const migrationPolicy = requiredSection(
		rules,
		'## 6. Database Migrations',
		'## 7. Frontend: SvelteKit 3 / Svelte 5'
	);
	assert.match(migrationPolicy, /every database-changing phase/);
	assert.match(migrationPolicy, /blocks both consumer cutover and destructive cleanup/);
	assert.match(migrationPolicy, /never discard or guess data or expose raw rows, PII/);

	const deploymentPolicy = requiredSection(
		rules,
		'## 10. Deployment Constraints',
		'## 11. Verification Matrix'
	);
	assert.match(deploymentPolicy, /approved rollout or rollback window/);
	assert.match(deploymentPolicy, /Record this decision, the compatible schema boundary/);

	const verificationPolicy = requiredSection(
		rules,
		'## 11. Verification Matrix',
		'Static-test ownership:'
	);
	assert.match(
		verificationPolicy,
		/refusal of both consumer cutover and destructive cleanup before evidence is complete/
	);
	assert.match(verificationPolicy, /preventing removed legacy contracts and runtime fallbacks/);
});

test('development rules require candid evidence-based recommendations', async () => {
	const rules = await readFile(path.join(repoRoot, '.rules'), 'utf8');
	const workflow = requiredSection(
		rules,
		'## 1. Required Analysis Workflow',
		'## 2. Adding or Changing a Feature'
	);

	assert.match(workflow, /### Recommendation and decision quality/);
	assert.match(workflow, /evidence-based, candid recommendations/);
	assert.match(workflow, /Do not agree merely[^.]*mirror[^.]*flatter/);
	assert.match(
		workflow,
		/correctness, performance, security, maintainability, operability, and delivery risk/
	);
	assert.match(workflow, /Do not prefer a smaller diff solely[^.]*larger coherent change/);
	assert.match(workflow, /Larger scope is not inherently better/);
	assert.match(workflow, /Avoid speculative rewrites and unrelated refactors/);
	assert.match(workflow, /tradeoffs[^.]*evidence[^.]*migration or rollback[^.]*verification/);
});

test('backend school workspace rules and verification commands are canonical', async () => {
	const [rules, testing, backendReadme] = await Promise.all([
		readFile(path.join(repoRoot, '.rules'), 'utf8'),
		readFile(path.join(repoRoot, 'docs/TESTING.md'), 'utf8'),
		readFile(path.join(repoRoot, 'backend-school/README.md'), 'utf8')
	]);

	for (const source of [rules, testing]) {
		assert.match(source, /cargo check --workspace --all-targets/);
	}
	assert.match(rules, /backend-school\/crates\/school-permissions\/src\/registry\.rs/);
	assert.match(rules, /backend-school\/crates\/school-migrations\/src\/lib\.rs/);
	assert.match(rules, /cohesive owner/i);
	assert.match(rules, /compile-scope/i);
	assert.match(testing, /cargo test -p school-permissions/);
	assert.match(testing, /cargo test -p school-migrations/);
	assert.match(backendReadme, /\[Testing\]\(\.\.\/docs\/TESTING\.md\)/);
	assert.match(testing, /BACKEND_SCHOOL_TEST_BIN=seed_sandbox/);
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
	assert.doesNotMatch(operations, /SCHOOL_ROLLBACK_JWT_SECRET/);
	assert.match(operations, /thirty-day|30-day/i);
	assert.match(podmanSetup, /SESSION_HMAC_KEY/);
	assert.match(backendReadme, /SESSION_HMAC_KEY/);
	assert.doesNotMatch(todo, /\bAUTH-001\b/);

	const auth002 = todo.match(/AUTH-002[\s\S]*?(?=\n- \[[ x]\] \*\*|\n##|$)/)?.[0] ?? '';
	assert.match(auth002, /\/api\/auth\/me\/profile/);
	assert.match(auth002, /default `\/api\/auth\/me` minimization is complete/i);
});

test('operations document owns the coordinated school release contract', async () => {
	const operations = await readFile(path.join(repoRoot, 'docs/OPERATIONS.md'), 'utf8');
	const podmanSetup = await readFile(path.join(repoRoot, 'docs/PODMAN_SETUP.md'), 'utf8');

	assert.match(operations, /deploy-school-release\.yml/);
	assert.match(operations, /frontend-only/);
	assert.match(operations, /backend-only/);
	assert.match(operations, /full release/);
	assert.match(operations, /\/deployment-status/);
	assert.match(operations, /10 seconds/);
	assert.match(operations, /reload/i);
	assert.doesNotMatch(operations, /deploy-backend-school\.yml/);
	assert.doesNotMatch(operations, /deploy-all-schools\.yml/);
	assert.doesNotMatch(operations, /SCHOOL_API_KEEP_MAINTENANCE/);
	assert.match(podmanSetup, /pipeline\.yml/);
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
	assert.match(testing, /--package school-fonts/);
	assert.match(testing, /school-font-library\.spec\.ts/);
	assert.match(testing, /certificate-lifecycle\.spec\.ts/);
	assert.match(testing, /survives campaign purge/i);
});

test('canonical docs own the academic cutover boundaries and unfinished lifecycle work', async () => {
	const [testing, operations, todo] = await Promise.all([
		readFile(path.join(repoRoot, 'docs/TESTING.md'), 'utf8'),
		readFile(path.join(repoRoot, 'docs/OPERATIONS.md'), 'utf8'),
		readFile(path.join(repoRoot, 'TODO.md'), 'utf8')
	]);

	assert.match(testing, /Academic Core migration rehearsal/);
	for (const migration of ['041', '042', '043', '044', '045']) {
		assert.match(testing, new RegExp(`migration_${migration}|through ${migration}`));
	}
	assert.match(testing, /system::handlers::migration::tests/);
	assert.match(testing, /discovery does not equal execution/i);
	assert.match(testing, /PostgreSQL compatibility/i);
	assert.match(testing, /Gradebook and results cutover rehearsal/);
	assert.match(testing, /migration_060/);
	assert.match(testing, /gradebook_results_status/);

	assert.match(operations, /Academic Core Phase B cleanup and rollback boundary/);
	assert.match(operations, /academicCoreCutover[\s\S]*cleanupCompleted/);
	assert.doesNotMatch(operations, /\/internal\/academic-core\/reconcile-all/);
	assert.match(operations, /one-time preflight command[\s\S]*retired/i);
	assert.match(operations, /first accepted\s+write[\s\S]*snapshot rollback boundary/i);
	assert.match(operations, /after the first\s+write[\s\S]*do not deploy the old app/i);
	assert.match(operations, /Gradebook and results cutover/);
	assert.match(operations, /gradebookResultsCutover[\s\S]*cutoverCompleted/);

	const sch002 = todo.match(/SCH-002[\s\S]*?(?=\n- \[[ x]\] \*\*|\n##|$)/)?.[0] ?? '';
	assert.match(sch002, /completed Gradebook/);
	assert.match(sch002, /term close\/reopen/);
	assert.match(sch002, /GPA\/GPAX/);
	assert.match(sch002, /annual closure/);
	assert.match(sch002, /promotion exceptions/);
	assert.match(sch002, /Thai academic documents/);
	assert.doesNotMatch(sch002, /still owns Gradebook\/results/);
});

test('development rules own route data loading and navigation performance', async () => {
	const rules = await readFile(path.join(repoRoot, '.rules'), 'utf8');

	assert.match(rules, /### Route data loading and navigation performance/);
	assert.match(rules, /\+page\.ts.*\+layout\.ts[\s\S]*primary route data/);
	assert.match(rules, /load event's `fetch`/);
	assert.match(rules, /academic context in menu destinations/);
	assert.match(
		rules,
		/Resolve required academic year and term before normal menu preload and navigation/
	);
	assert.match(rules, /URL repair is a fallback for direct, stale, or external links/);
	assert.match(rules, /independently renderable UI region/);
	assert.match(rules, /route-wide `\/page-view`/);
	assert.match(rules, /typed in-flight/);
	assert.match(rules, /real data dependency/);
	assert.match(rules, /visible independent region reads[\s\S]*concurrently/);
	assert.match(rules, /refreshAll\(\)/);
	assert.match(rules, /cache[\s\S]*tenant[\s\S]*invalidation/);
});

test('development rules require efficient measured SQL reads', async () => {
	const rules = await readFile(path.join(repoRoot, '.rules'), 'utf8');

	assert.match(rules, /minimize database round trips/);
	assert.match(rules, /N\+1/);
	assert.match(rules, /set-based/);
	assert.match(rules, /tenant pool capacity/);
	assert.match(rules, /EXPLAIN \(ANALYZE, BUFFERS\)/);
});
