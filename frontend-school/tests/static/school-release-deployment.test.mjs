import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { access, mkdtemp, mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { parse as parseYaml } from 'yaml';
import { readWorkflowSource } from '../helpers/workflow-source.mjs';

const repoRoot = path.resolve(import.meta.dirname, '../../..');

test('Admin uploads code with secrets and promotes only its recorded Worker version', async () => {
	const workflow = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/deploy-frontend-admin.yml'), 'utf8')
	);
	const steps = workflow.jobs.deploy.steps;
	const prepare = steps.find((step) => step.name === 'Prepare versioned Worker secrets');
	const upload = steps.find(
		(step) => step.name === 'Upload frontend-admin Worker with its secrets'
	);
	const record = steps.find((step) => step.id === 'uploaded-version');
	const promote = steps.find(
		(step) => step.name === 'Promote uploaded frontend-admin Worker and apply routes'
	);
	const cleanup = steps.find((step) => step.name === 'Remove temporary Worker secrets');
	assert.match(
		upload.with.command,
		/^versions upload .*--secrets-file \.wrangler\/admin-release-secrets\.json/
	);
	assert.equal(
		upload.env.WRANGLER_OUTPUT_FILE_PATH,
		'${{ github.workspace }}/frontend-admin/.wrangler/admin-release-upload.ndjson'
	);
	assert.equal(upload.env.PUBLIC_API_URL, '${{ vars.BACKEND_ADMIN_URL }}');
	assert.equal(upload.env.BACKEND_SCHOOL_URL, '${{ vars.BACKEND_SCHOOL_URL }}');
	assert.match(
		promote.with.command,
		/--version-id \$\{\{ steps\.uploaded-version\.outputs\.id \}\} --percentage 100 --yes/
	);
	assert.match(promote.with.command, /\ntriggers deploy --config wrangler\.deploy\.json/);
	assert.equal(cleanup.if, 'always()');
	assert.ok(steps.indexOf(prepare) < steps.indexOf(upload));
	assert.ok(steps.indexOf(upload) < steps.indexOf(record));
	assert.ok(steps.indexOf(record) < steps.indexOf(promote));
	for (const step of steps) {
		assert.ok(!step.with?.secrets, 'legacy secret bulk must not run');
		assert.doesNotMatch(step.with?.command || '', /secret bulk/);
	}
	const temp = await mkdtemp(path.join(os.tmpdir(), 'admin-version-'));
	const output = path.join(temp, 'output');
	const run = (step, extra = {}) =>
		spawnSync('bash', ['-eu', '-c', step.run], {
			cwd: temp,
			env: { ...process.env, GITHUB_OUTPUT: output, ...extra },
			encoding: 'utf8'
		});
	try {
		assert.notEqual(run(prepare, { INTERNAL_API_SECRET: '' }).status, 0);
		await mkdir(path.join(temp, '.wrangler'), { recursive: true });
		const fakeSecret = 'disposable-test-binding';
		const prepared = run(prepare, { INTERNAL_API_SECRET: fakeSecret });
		assert.equal(prepared.status, 0, prepared.stderr);
		assert.doesNotMatch(prepared.stdout + prepared.stderr, /disposable-test-binding/);
		assert.equal(
			(await stat(path.join(temp, '.wrangler/admin-release-secrets.json'))).mode & 0o777,
			0o600
		);
		assert.equal(
			JSON.parse(await readFile(path.join(temp, '.wrangler/admin-release-secrets.json'), 'utf8'))
				.INTERNAL_API_SECRET,
			fakeSecret
		);
		await writeFile(
			path.join(temp, 'wrangler.deploy.json'),
			JSON.stringify({ name: 'schoolorbit-frontend-admin' })
		);
		const valid = {
			type: 'version-upload',
			version: 1,
			worker_name: 'schoolorbit-frontend-admin',
			version_id: '12345678-abcd-1234-abcd-123456789abc'
		};
		for (const entries of [
			[],
			[valid, valid],
			[{ ...valid, worker_name: 'other-worker' }],
			[{ ...valid, version: 2 }],
			[{ ...valid, version_id: 'latest' }],
			[{ ...valid, version_id: 'id\ninjected=value' }]
		]) {
			await writeFile(
				path.join(temp, '.wrangler/admin-release-upload.ndjson'),
				entries.map((entry) => JSON.stringify(entry)).join('\n')
			);
			assert.notEqual(run(record).status, 0, JSON.stringify(entries));
		}
		await writeFile(
			path.join(temp, '.wrangler/admin-release-upload.ndjson'),
			JSON.stringify(valid)
		);
		const recorded = run(record);
		assert.equal(recorded.status, 0, recorded.stderr);
		assert.equal(await readFile(output, 'utf8'), `id=${valid.version_id}\n`);
		assert.equal(run(cleanup).status, 0);
		await assert.rejects(access(path.join(temp, '.wrangler/admin-release-secrets.json')));
		await assert.rejects(access(path.join(temp, '.wrangler/admin-release-upload.ndjson')));
	} finally {
		await rm(temp, { recursive: true, force: true });
	}
});

test('every production mutation requires release CI while builds stay parallel', async () => {
	const pipeline = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/pipeline.yml'), 'utf8')
	);
	const release = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/release.yml'), 'utf8')
	);
	assert.ok(pipeline.jobs.release.needs.includes('gate'));
	assert.ok(!pipeline.jobs.prepare.needs.includes('gate'));
	assert.equal(pipeline.jobs.release.concurrency.group, 'schoolorbit-production');
	assert.equal(pipeline.jobs.release.concurrency['cancel-in-progress'], false);
	assert.ok(release.jobs.maintenance.needs.includes('preflight'));
	for (const name of ['admin-backend', 'admin-frontend', 'school'])
		assert.ok(release.jobs[name].needs.includes('maintenance'));
	const implementation = parseYaml(
		await readWorkflowSource(repoRoot, '.github/workflows/deploy-school-release.yml')
	);
	for (const name of ['deploy-backend', 'promote-frontends', 'accept-release']) {
		assert.ok(implementation.jobs[name].needs.includes('verify-release-ci'));
		assert.match(implementation.jobs[name].if, /verify-release-ci.result == 'success'/);
	}
});

test('CI refusal blocks backend-only, frontend-only and full production paths', async () => {
	const workflow = parseYaml(
		await readWorkflowSource(repoRoot, '.github/workflows/deploy-school-release.yml')
	);
	for (const [backend, frontend] of [
		['true', 'false'],
		['false', 'true'],
		['true', 'true']
	]) {
		for (const ci of ['success', 'failure', 'cancelled', 'skipped']) {
			const needs = Object.fromEntries(
				Object.keys(workflow.jobs).map((id) => [
					id,
					{
						result: 'success',
						outputs: { needs_backend: backend, needs_frontend: frontend }
					}
				])
			);
			needs['verify-release-ci'].result = ci;
			if (backend === 'false') needs['deploy-backend'].result = 'skipped';
			if (frontend === 'false') needs['stage-frontends'].result = 'skipped';
			for (const job of ['deploy-backend', 'promote-frontends', 'accept-release']) {
				const expression = workflow.jobs[job].if
					.replace(/^\s*\$\{\{|\}\}\s*$/g, '')
					.replace(/needs\.([a-z-]+)/g, 'needs["$1"]');
				const enabled = Function('needs', 'always', `return (${expression})`)(needs, () => true);
				const selected =
					job === 'deploy-backend'
						? backend === 'true'
						: job === 'promote-frontends'
							? frontend === 'true'
							: true;
				assert.equal(
					enabled,
					ci === 'success' && selected,
					`${job}: backend=${backend}, frontend=${frontend}, ci=${ci}`
				);
			}
		}
	}
});

test('selected verification or artifact failure blocks the stable gate for every release scope', async () => {
	const pipeline = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/pipeline.yml'), 'utf8')
	);
	const script = pipeline.jobs.gate.steps[0].run;
	const good = {
		...process.env,
		PLAN_RESULT: 'success',
		VERIFY_RESULT: 'success',
		PREPARE_RESULT: 'success',
		CANDIDATE: 'false',
		BUILDS: '["backend-school"]'
	};
	assert.equal(spawnSync('bash', ['-eu', '-c', script], { env: good }).status, 0);
	for (const key of ['PLAN_RESULT', 'VERIFY_RESULT', 'PREPARE_RESULT'])
		for (const status of ['failure', 'cancelled', 'skipped', 'timed_out', '']) {
			assert.notEqual(
				spawnSync('bash', ['-eu', '-c', script], { env: { ...good, [key]: status } }).status,
				0,
				`${key}: ${status}`
			);
		}
	assert.equal(
		spawnSync('bash', ['-eu', '-c', script], {
			env: { ...good, CANDIDATE: 'true', PREPARE_RESULT: 'skipped' }
		}).status,
		0
	);
});

test('all Wrangler actions use Node 24 runtime release and the tracked CLI lock', async () => {
	for (const file of [
		'deploy-school-release.yml',
		'deploy-school-tenant.yml',
		'deploy-frontend-admin.yml'
	]) {
		const workflow = parseYaml(
			await readFile(path.join(repoRoot, '.github/workflows', file), 'utf8')
		);
		for (const job of Object.values(workflow.jobs)) {
			for (const step of job.steps ?? []) {
				if (!step.uses?.startsWith('cloudflare/wrangler-action@')) continue;
				assert.equal(step.uses, 'cloudflare/wrangler-action@v4.1.3');
				assert.equal(step.with.wranglerVersion, '${{ steps.wrangler-version.outputs.version }}');
				const version = job.steps.find((item) => item.id === 'wrangler-version');
				assert.match(version.run, /package-lock\.json/);
				assert.match(version.run, /node_modules\/wrangler/);
			}
		}
	}
});

test('one serialized workflow owns every school production release scope', async () => {
	const pipeline = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/pipeline.yml'), 'utf8')
	);
	const operations = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/operations.yml'), 'utf8')
	);
	assert.equal(
		pipeline.jobs.release.concurrency.group,
		operations.jobs.provision.concurrency.group
	);
	for (const file of [
		'deploy-school-release',
		'deploy-backend-admin',
		'deploy-frontend-admin',
		'deploy-school-tenant'
	]) {
		const d = parseYaml(
			await readFile(path.join(repoRoot, `.github/workflows/${file}.yml`), 'utf8')
		);
		assert.deepEqual(Object.keys(d.on), ['workflow_call']);
	}
	const preflight = await readFile(path.join(repoRoot, 'scripts/release_preflight.mjs'), 'utf8');
	assert.match(preflight, /run_attempt/);
	assert.match(preflight, /Pipeline gate/);
	assert.match(preflight, /newer main/);
	assert.match(preflight, /readAcceptedState/);
});

test('accepted baselines and successful attempt provenance govern no-op replay', async () => {
	const state = await readFile(path.join(repoRoot, 'scripts/lib/pipeline-state.mjs'), 'utf8');
	const planner = await readFile(path.join(repoRoot, 'scripts/lib/pipeline-policy.mjs'), 'utf8');
	assert.match(state, /attempts\/\$\{state.attempt\}\/jobs/);
	assert.match(state, /acceptance\[0\].conclusion !== 'success'/);
	assert.match(planner, /components\?\.\[part\]\?\.sha/);
	assert.match(planner, /inputHash/);
	assert.match(planner, /Divergent baseline/);
	const source = await readFile(path.join(repoRoot, '.github/workflows/pipeline.yml'), 'utf8');
	assert.match(source, /outputs.deploy != '\[\]'/);
});

test('School stages selected Worker versions before backend cutover', async () => {
	const source = await readWorkflowSource(repoRoot, '.github/workflows/deploy-school-release.yml');
	const workflow = parseYaml(source);
	const stage = workflow.jobs['stage-frontends'];
	const deploy = workflow.jobs['deploy-backend'];

	assert.equal(stage.strategy['fail-fast'], false);
	assert.match(JSON.stringify(stage.needs), /discover-schools/);
	assert.match(source, /versions upload --tag/);
	assert.match(source, /deployments list --json/);
	assert.match(source, /versions list --json/);
	assert.match(source, /Find reusable Worker recovery manifest/);
	assert.match(source, /Recover an inactive candidate missing only its manifest/);
	assert.match(source, /find_worker_release_candidates\.mjs/);
	assert.doesNotMatch(
		source.slice(
			source.indexOf('Recover an inactive candidate missing only its manifest'),
			source.indexOf('Upload inactive Worker version')
		),
		/wrangler versions list/
	);
	assert.match(source, /github-token: \$\{\{ github\.token \}\}/);
	assert.match(source, /Complete Worker version inventory returned an invalid candidate set/);
	assert.match(source, /\.workflow_id == \$workflow_id/);
	assert.match(source, /\.head_repository\.full_name == \$repo/);
	assert.match(source, /\.head_sha == \$release/);
	assert.match(source, /versions view "\$candidate_id" --json/);
	assert.match(JSON.stringify(deploy.needs), /stage-frontends/);
	assert.match(String(deploy.if), /stage-frontends\.result == 'success'/);
	assert.ok(
		source.indexOf('Activate School API maintenance') < source.indexOf('podman rm --force')
	);
});

test('locked Wrangler accepts exact version-ID promotion arguments', () => {
	const result = spawnSync('npx', ['--no-install', 'wrangler', 'versions', 'deploy', '--help'], {
		cwd: path.join(repoRoot, 'frontend-school'),
		encoding: 'utf8'
	});

	assert.equal(result.status, 0, result.stderr);
	assert.match(result.stdout, /--version-id/);
	assert.match(result.stdout, /--percentage/);
});

test('frontend promotion is gated by backend acceptance and ready waits for every tenant', async () => {
	const child = parseYaml(
		await readWorkflowSource(repoRoot, '.github/workflows/deploy-school-release.yml')
	);
	assert.match(child.jobs['promote-frontends'].if, /deploy-backend.result == 'success'/);
	assert.match(child.jobs['accept-release'].if, /promote-frontends.result == 'success'/);
	const parent = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/release.yml'), 'utf8')
	);
	assert.ok(parent.jobs.accept.needs.includes('school'));
	assert.ok(parent.jobs.accept.needs.includes('admin-frontend'));
	assert.match(parent.jobs.accept.if, /needs.school.result == 'success'/);
	const maintenance = await readFile(
		path.join(repoRoot, 'scripts/lib/pipeline-remote/maintenance.sh'),
		'utf8'
	);
	assert.ok(maintenance.indexOf('SMOKE_REQUIRE_AUTH=true') < maintenance.indexOf('ready\n'));
	assert.match(maintenance, /restore_maintenance/);
});

test('frontend promotion waits for complete tenant assets and application mount readiness', async () => {
	const workflow = parseYaml(
		await readWorkflowSource(repoRoot, '.github/workflows/deploy-school-release.yml')
	);
	const steps = workflow.jobs['promote-frontends'].steps;
	const chromiumIndex = steps.findIndex((step) => step.uses === './.github/actions/setup-chromium');
	const readinessIndex = steps.findIndex(
		(step) => step.name === 'Verify promoted frontend readiness'
	);

	assert.ok(chromiumIndex >= 0);
	assert.ok(readinessIndex > chromiumIndex);
	const setupSource = await readFile(
		path.join(repoRoot, '.github/actions/setup-chromium/action.yml'),
		'utf8'
	);
	const setup = parseYaml(setupSource);
	const browserCache = setup.runs.steps.find((step) => step.uses === 'actions/cache@v5');
	assert.match(browserCache.with.key, /runner\.os.*runner\.arch.*steps\.version\.outputs\.version/);
	const verify = setup.runs.steps.find((step) => step.run?.includes('chromium.launch'));
	assert.ok(verify, 'a restored browser must still launch before promotion readiness');
	assert.equal(verify.if, undefined, 'cache hits must not skip host dependency verification');
	assert.match(verify.run, /playwright install --only-shell chromium/);
	assert.match(verify.run, /playwright install-deps chromium/);
	assert.equal(
		steps[readinessIndex].env.TENANT_ORIGIN,
		'https://${{ matrix.school.subdomain }}.${{ vars.BASE_DOMAIN }}'
	);
	assert.equal(steps[readinessIndex].run.trim(), 'node scripts/verify-tenant-worker-readiness.mjs');
});

test('release acceptance trusts the pinned Origin CA only for resolved API smoke calls', async () => {
	const maintenance = await readFile(
		path.join(repoRoot, 'scripts/lib/pipeline-remote/maintenance.sh'),
		'utf8'
	);
	assert.match(maintenance, /--cacert "\$origin_root"/);
	assert.match(maintenance, /--resolve "\$\{host\}:443:127.0.0.1"/);
	assert.match(maintenance, /SMOKE_RESOLVE_IP=127.0.0.1/);
	assert.match(maintenance, /SMOKE_CA_CERT="\$origin_root"/);
	assert.match(maintenance, /SMOKE_RELEASE_PROBE_TOKEN/);
	assert.doesNotMatch(maintenance, /curl[^\n]*--insecure/);
});

test('backend runtime selects the candidate SHA without advancing latest before acceptance', async () => {
	const source = await readWorkflowSource(repoRoot, '.github/workflows/deploy-school-release.yml');
	assert.match(
		source,
		/backend_school_image_reference="\$\{backend_image\}@\$\{backend_image_digest\}"/
	);
	assert.match(source, /BACKEND_SCHOOL_IMAGE/);
	const prepare = await readFile(path.join(repoRoot, '.github/workflows/prepare.yml'), 'utf8');
	assert.doesNotMatch(prepare, /type=raw,value=latest/);
	assert.match(prepare, /input-\$\{INPUT_HASH\}/);
	const frontend = await readFile(
		path.join(repoRoot, '.github/workflows/deploy-frontend-admin.yml'),
		'utf8'
	);
	assert.match(frontend, /del\(\.build\)/);
	assert.doesNotMatch(frontend, /run: npm run build/);
});

test('release acceptance requires successful scope resolution', async () => {
	const workflow = parseYaml(
		await readWorkflowSource(repoRoot, '.github/workflows/deploy-school-release.yml')
	);
	assert.match(String(workflow.jobs['accept-release'].if), /resolve-scope\.result == 'success'/);
});

test('deployment status requests are bounded and application mount is observable', async () => {
	const statusClient = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/deployment/maintenance.ts'),
		'utf8'
	);
	const layout = await readFile(
		path.join(repoRoot, 'frontend-school/src/routes/+layout.svelte'),
		'utf8'
	);

	assert.match(statusClient, /new AbortController\(\)/);
	assert.match(statusClient, /signal: controller\.signal/);
	assert.match(statusClient, /STATUS_REQUEST_TIMEOUT_MS = 5_000/);
	assert.match(statusClient, /clearTimeout\(timeout\)/);
	assert.match(layout, /schoolorbitAppMounted = 'true'/);
});

test('superseded push workflows are removed while tenant provisioning remains', async () => {
	const files = (await import('node:fs/promises')).readdir;
	const owners = [];
	for (const file of await files(path.join(repoRoot, '.github/workflows'))) {
		const d = parseYaml(await readFile(path.join(repoRoot, '.github/workflows', file), 'utf8'));
		if (Object.keys(d.on).some((key) => key !== 'workflow_call')) owners.push(file);
	}
	assert.deepEqual(owners.sort(), [
		'maintenance.yml',
		'merge.yml',
		'operations.yml',
		'pipeline.yml'
	]);
});
