import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { access, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { parse as parseYaml } from 'yaml';
import { readWorkflowSource } from '../helpers/workflow-source.mjs';

const repoRoot = path.resolve(import.meta.dirname, '../../..');

test('Admin configuration uses exact apex and canonical-www zone routes and retires the former alias after promotion', async (t) => {
	const workflow = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/deploy-frontend-admin.yml'), 'utf8')
	);
	const step = workflow.jobs.deploy.steps.find(
		(item) => item.name === 'Create environment-specific deployment configuration'
	);
	const temporary = await mkdtemp(path.join(os.tmpdir(), 'admin-apex-route-'));
	t.after(() => rm(temporary, { recursive: true, force: true }));
	await writeFile(
		path.join(temporary, 'wrangler.json'),
		JSON.stringify({ name: 'schoolorbit-frontend-admin', build: { command: 'never-run' } })
	);
	const result = spawnSync('bash', ['-c', step.run], {
		cwd: temporary,
		encoding: 'utf8',
		env: {
			...process.env,
			BASE_DOMAIN: 'example.test',
			PUBLIC_API_URL: 'https://admin-api.example.test'
		}
	});
	assert.equal(result.status, 0, result.stderr);
	const config = JSON.parse(await readFile(path.join(temporary, 'wrangler.deploy.json'), 'utf8'));
	assert.deepEqual(config.routes, [
		{ pattern: 'example.test/*', zone_name: 'example.test' },
		{ pattern: 'www.example.test/*', zone_name: 'example.test' }
	]);
	assert.equal(config.build, undefined);
	assert.deepEqual(config.vars, { PUBLIC_API_URL: 'https://admin-api.example.test' });
	const steps = workflow.jobs.deploy.steps;
	const promote = steps.findIndex(
		(item) => item.name === 'Promote uploaded frontend-admin Worker and apply routes'
	);
	const retire = steps.findIndex((item) =>
		item.run?.endsWith('pipeline-admin-worker.mjs retire-alias')
	);
	const verify = steps.findIndex(
		(item) => item.name === 'Verify the exact Admin Worker deployment and retain its receipt'
	);
	assert.ok(promote >= 0 && promote < retire && retire < verify);
	assert.equal(steps[retire].env.BASE_DOMAIN, '${{ vars.BASE_DOMAIN }}');
	assert.equal(steps[retire].env.CLOUDFLARE_API_TOKEN, '${{ secrets.CLOUDFLARE_API_TOKEN }}');
	const release = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/release.yml'), 'utf8')
	);
	assert.ok(
		release.jobs['private-preflight'].steps.some((item) =>
			item.run?.endsWith('pipeline-admin-worker.mjs check-alias')
		)
	);
});

test('reconciled hashes leave a secret-free job and missing transport blocks maintenance', async () => {
	const release = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/release.yml'), 'utf8')
	);
	const publicJob = release.jobs.preflight;
	const privateJob = release.jobs['private-preflight'];
	assert.deepEqual(publicJob.outputs, {
		plan: '${{ steps.reconcile.outputs.plan }}',
		deploy: '${{ steps.reconcile.outputs.deploy }}'
	});
	assert.doesNotMatch(
		JSON.stringify(publicJob),
		/secrets\.|appleboy\/|CLOUDFLARE_API_TOKEN|ADMIN_BOOTSTRAP/
	);
	assert.equal(publicJob.steps.at(-1).run, 'node scripts/release_preflight.mjs');
	assert.equal(privateJob.outputs, undefined);
	assert.equal(privateJob.needs, 'preflight');
	assert.deepEqual(release.jobs.maintenance.needs, ['preflight', 'private-preflight']);
	assert.match(release.jobs.maintenance.if, /needs.private-preflight.result == 'success'/);
	const validation = privateJob.steps[2];
	assert.equal(validation.name, 'Validate reconciled plan transport before private checks');
	assert.equal(validation.env.PLAN, '${{ needs.preflight.outputs.plan }}');
	const plan = {
		schemaVersion: 1,
		sha: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repoRoot, encoding: 'utf8' }).trim(),
		tree: execFileSync('git', ['rev-parse', 'HEAD^{tree}'], {
			cwd: repoRoot,
			encoding: 'utf8'
		}).trim(),
		deploy: ['backend-admin'],
		hashes: { 'backend-admin': '22'.repeat(32) }
	};
	for (const candidate of [
		plan,
		'',
		null,
		{ ...plan, schemaVersion: 2 },
		{ ...plan, sha: '0'.repeat(40) },
		{ ...plan, tree: '0'.repeat(40) },
		{ ...plan, deploy: [] },
		{ ...plan, deploy: ['backend-admin', 'backend-admin'] },
		{ ...plan, deploy: ['unknown'] },
		{ ...plan, hashes: {} },
		{ ...plan, hashes: { 'backend-admin': '***' } }
	]) {
		const result = spawnSync('bash', ['-c', validation.run], {
			cwd: repoRoot,
			encoding: 'utf8',
			env: {
				...process.env,
				PLAN: candidate === '' ? '' : JSON.stringify(candidate),
				DEPLOY: JSON.stringify(plan.deploy),
				GITHUB_SHA: plan.sha
			}
		});
		assert.equal(result.status === 0, candidate === plan, result.stderr);
	}
	const mismatched = spawnSync('bash', ['-c', validation.run], {
		cwd: repoRoot,
		encoding: 'utf8',
		env: {
			...process.env,
			PLAN: JSON.stringify(plan),
			DEPLOY: '["frontend-admin"]',
			GITHUB_SHA: plan.sha
		}
	});
	assert.notEqual(mismatched.status, 0);
	for (const result of ['failure', 'skipped', 'cancelled']) {
		const enabled = Function(
			'needs',
			`return (${release.jobs.maintenance.if.replace(/needs\.([a-z-]+)/g, 'needs["$1"]')})`
		)({
			preflight: { result: 'success', outputs: { deploy: '["backend-admin"]' } },
			'private-preflight': { result }
		});
		assert.equal(enabled, false, result);
	}
});

test('Admin deploy resolves its image artifact locally instead of crossing a secret-bearing job output', async () => {
	const release = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/release.yml'), 'utf8')
	);
	const backend = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/deploy-backend-admin.yml'), 'utf8')
	);
	assert.equal(release.jobs.preflight.outputs.admin_digest, undefined);
	assert.equal(
		release.jobs['admin-backend'].with.input_hash,
		"${{ fromJson(needs.preflight.outputs.plan).hashes['backend-admin'] }}"
	);
	assert.equal(backend.on.workflow_call.inputs.input_hash.required, true);
	assert.equal(backend.on.workflow_call.inputs.image_digest, undefined);
	const steps = backend.jobs.deploy.steps;
	const download = steps.findIndex((step) => step.with?.name === 'prepared-backend-admin');
	const validation = steps.findIndex((step) => step.id === 'manifest');
	const deploy = steps.findIndex((step) => step.uses?.startsWith('appleboy/ssh-action@'));
	assert.ok(download >= 0 && download < validation && validation < deploy);
	assert.equal(steps[validation].env.INPUT_HASH, '${{ inputs.input_hash }}');
	assert.equal(steps[deploy].env.BACKEND_IMAGE_DIGEST, '${{ steps.manifest.outputs.digest }}');
});

test('both Admin image consumers reject stale or malformed artifact identities before remote deployment', async (t) => {
	const temporary = await mkdtemp(path.join(os.tmpdir(), 'admin-image-artifact-'));
	t.after(() => rm(temporary, { recursive: true, force: true }));
	await mkdir(path.join(temporary, 'prepared'));
	const manifest = {
		schemaVersion: 1,
		component: 'backend-admin',
		sha: 'a'.repeat(40),
		inputHash: 'b'.repeat(64),
		digest: 'sha256:' + '22'.repeat(32)
	};
	for (const [file, job] of [
		['release.yml', 'private-preflight'],
		['deploy-backend-admin.yml', 'deploy']
	]) {
		const workflow = parseYaml(
			await readFile(path.join(repoRoot, '.github/workflows', file), 'utf8')
		);
		const script = workflow.jobs[job].steps.find((step) => step.id === 'manifest').run;
		for (const patch of [
			{},
			{ schemaVersion: 2 },
			{ component: 'backend-school' },
			{ sha: 'c'.repeat(40) },
			{ inputHash: 'd'.repeat(64) },
			{ digest: 'latest' },
			{ digest: null }
		]) {
			await writeFile(
				path.join(temporary, 'prepared/backend-admin.json'),
				JSON.stringify({ ...manifest, ...patch })
			);
			const output = path.join(temporary, 'output');
			await writeFile(output, '');
			const result = spawnSync('bash', ['-c', script], {
				encoding: 'utf8',
				env: {
					...process.env,
					RUNNER_TEMP: temporary,
					GITHUB_SHA: manifest.sha,
					INPUT_HASH: manifest.inputHash,
					GITHUB_OUTPUT: output
				}
			});
			if (Object.keys(patch).length === 0) {
				assert.equal(result.status, 0, result.stderr);
				assert.equal(await readFile(output, 'utf8'), `digest=${manifest.digest}\n`);
			} else {
				assert.notEqual(result.status, 0, `${file}: ${JSON.stringify(patch)}`);
				assert.equal(await readFile(output, 'utf8'), '');
			}
		}
	}
});

test('direct noninteractive Wrangler API commands receive credentials independently of action inputs', async () => {
	let directCommands = 0;
	for (const file of [
		'deploy-frontend-admin.yml',
		'deploy-school-release.yml',
		'deploy-school-tenant.yml'
	]) {
		const workflow = parseYaml(
			await readFile(path.join(repoRoot, '.github/workflows', file), 'utf8')
		);
		for (const [jobName, job] of Object.entries(workflow.jobs)) {
			for (const step of job.steps || []) {
				if (!/\bnpx\s+wrangler\s+(?:versions|deployments|triggers|deploy)\b/.test(step.run || ''))
					continue;
				directCommands++;
				const env = { ...workflow.env, ...job.env, ...step.env };
				assert.equal(
					env.CLOUDFLARE_API_TOKEN,
					'${{ secrets.CLOUDFLARE_API_TOKEN }}',
					`${file} ${jobName} ${step.name}`
				);
				assert.equal(
					env.CLOUDFLARE_ACCOUNT_ID,
					'${{ vars.CLOUDFLARE_ACCOUNT_ID }}',
					`${file} ${jobName} ${step.name}`
				);
			}
		}
	}
	assert.ok(directCommands > 5, 'the guard must cover actual deployment commands');
});

test('Admin has no internal credentials and promotes only its recorded Worker version', async () => {
	const workflow = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/deploy-frontend-admin.yml'), 'utf8')
	);
	const steps = workflow.jobs.deploy.steps;
	const prepare = steps.find((step) => step.name === 'Prepare Worker upload receipt');
	const upload = steps.find((step) => step.name === 'Upload frontend-admin Worker');
	const record = steps.find((step) => step.id === 'uploaded-version');
	const promote = steps.find(
		(step) => step.name === 'Promote uploaded frontend-admin Worker and apply routes'
	);
	const cleanup = steps.find((step) => step.name === 'Remove temporary upload receipt');
	assert.match(upload.with.command, /^versions upload --config wrangler\.deploy\.json --tag/);
	assert.equal(
		upload.env.WRANGLER_OUTPUT_FILE_PATH,
		'${{ github.workspace }}/frontend-admin/.wrangler/admin-release-upload.ndjson'
	);
	assert.equal(upload.env.PUBLIC_API_URL, '${{ vars.BACKEND_ADMIN_URL }}');
	assert.equal(upload.env.BACKEND_SCHOOL_URL, undefined);
	assert.doesNotMatch(JSON.stringify(workflow), /INTERNAL_API_SECRET|secrets-file/);
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
		const prepared = run(prepare);
		assert.equal(prepared.status, 0, prepared.stderr);
		await assert.rejects(access(path.join(temp, '.wrangler/admin-release-secrets.json')));
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

test('planning or selected artifact failure blocks the stable gate for every release scope', async () => {
	const pipeline = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/pipeline.yml'), 'utf8')
	);
	const script = pipeline.jobs.gate.steps[0].run;
	const good = {
		...process.env,
		PLAN_RESULT: 'success',
		PREPARE_RESULT: 'success',
		BUILDS: '["backend-school"]'
	};
	assert.equal(spawnSync('bash', ['-eu', '-c', script], { env: good }).status, 0);
	for (const key of ['PLAN_RESULT', 'PREPARE_RESULT'])
		for (const status of ['failure', 'cancelled', 'skipped', 'timed_out', '']) {
			assert.notEqual(
				spawnSync('bash', ['-eu', '-c', script], { env: { ...good, [key]: status } }).status,
				0,
				`${key}: ${status}`
			);
		}
	assert.equal(
		spawnSync('bash', ['-eu', '-c', script], {
			env: { ...good, BUILDS: '[]', PREPARE_RESULT: 'skipped' }
		}).status,
		0
	);
});

test('PR planning rejects a conflict-free stale branch until its developer updates it', async (t) => {
	const workflow = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/pipeline.yml'), 'utf8')
	);
	const body = workflow.jobs.plan.steps.find((step) => step.id === 'plan').run;
	const script = body.slice(0, body.indexOf('cat "$RUNNER_TEMP/plan.json"'));
	for (const updated of [false, true]) {
		const root = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-pr-base-'));
		t.after(() => rm(root, { recursive: true, force: true }));
		const git = (...args) => execFileSync('git', args, { cwd: root, stdio: 'pipe' });
		git('init', '-q', '-b', 'main');
		git('config', 'user.name', 'Fixture');
		git('config', 'user.email', 'fixture@example.invalid');
		await mkdir(path.join(root, 'scripts'));
		await writeFile(
			path.join(root, 'scripts/pipeline'),
			'#!/bin/sh\ntouch "$RUNNER_TEMP/planned"\n',
			{
				mode: 0o755
			}
		);
		git('add', '.');
		git('commit', '-qm', 'base');
		git('checkout', '-qb', 'feature');
		await writeFile(path.join(root, 'feature'), 'feature');
		git('add', '.');
		git('commit', '-qm', 'feature');
		git('checkout', '-q', 'main');
		await writeFile(path.join(root, 'main'), 'advance');
		git('add', '.');
		git('commit', '-qm', 'advance main');
		if (updated) {
			git('checkout', '-q', 'feature');
			git('merge', '--no-ff', '-qm', 'developer updates branch', 'main');
			git('checkout', '-q', 'main');
		}
		git('merge', '--no-ff', '-qm', 'GitHub merge candidate', 'feature');
		const result = spawnSync('bash', ['-c', script], {
			cwd: root,
			env: { ...process.env, EVENT: 'pull_request', PR_NUMBER: '7', RUNNER_TEMP: root },
			encoding: 'utf8'
		});
		assert.equal(result.status, updated ? 0 : 1, result.stderr);
		if (updated) await access(path.join(root, 'planned'));
		else await assert.rejects(access(path.join(root, 'planned')));
	}
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

test('independent Workers run in parallel after Admin backend, but any failure blocks global acceptance', async () => {
	const parent = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/release.yml'), 'utf8')
	);
	assert.deepEqual(parent.jobs.school.needs, ['preflight', 'maintenance', 'admin-backend']);
	assert.deepEqual(parent.jobs['admin-frontend'].needs, [
		'preflight',
		'maintenance',
		'admin-backend'
	]);
	const evaluate = (job, needs, deploy) => {
		needs.preflight.outputs = { plan: JSON.stringify({ deploy }) };
		return Function(
			'needs',
			'inputs',
			'always',
			'contains',
			'fromJson',
			`return (${parent.jobs[job].if.replace(/needs\.([a-z-]+)/g, 'needs["$1"]')})`
		)(
			needs,
			{ plan: JSON.stringify({ deploy }) },
			() => true,
			(list, item) => list.includes(item),
			JSON.parse
		);
	};
	for (const admin of ['success', 'skipped', 'failure', 'cancelled']) {
		const needs = Object.fromEntries(
			[
				'preflight',
				'private-preflight',
				'maintenance',
				'admin-backend',
				'admin-frontend',
				'school'
			].map((id) => [id, { result: 'success' }])
		);
		needs['admin-backend'].result = admin;
		assert.equal(
			evaluate('school', needs, ['backend-school']),
			['success', 'skipped'].includes(admin)
		);
		assert.equal(
			evaluate('admin-frontend', needs, ['frontend-admin']),
			['success', 'skipped'].includes(admin)
		);
	}
	for (const failed of ['maintenance', 'admin-backend', 'admin-frontend', 'school']) {
		const needs = Object.fromEntries(
			[
				'preflight',
				'private-preflight',
				'maintenance',
				'admin-backend',
				'admin-frontend',
				'school'
			].map((id) => [id, { result: 'success' }])
		);
		needs[failed].result = 'failure';
		assert.equal(evaluate('accept', needs, ['frontend-admin', 'backend-school']), false, failed);
	}
});

test('release preparation and consumers share a pinned runner and Admin browser uses the locked subset', async () => {
	for (const file of [
		'prepare.yml',
		'release.yml',
		'deploy-backend-admin.yml',
		'deploy-frontend-admin.yml',
		'deploy-school-release.yml',
		'deploy-school-tenant.yml'
	]) {
		const workflow = parseYaml(
			await readFile(path.join(repoRoot, '.github/workflows', file), 'utf8')
		);
		for (const job of Object.values(workflow.jobs))
			if (job['runs-on']) assert.equal(job['runs-on'], 'ubuntu-24.04', file);
	}
	const admin = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/deploy-frontend-admin.yml'), 'utf8')
	);
	const steps = admin.jobs.deploy.steps;
	const tooling = steps.findIndex(
		(step) =>
			step.uses === './.github/actions/setup-deployment-tooling' &&
			step.with.component === 'frontend-school'
	);
	assert.ok(
		tooling >= 0 &&
			tooling < steps.findIndex((step) => step.uses === './.github/actions/setup-chromium')
	);
	assert.ok(
		!steps.some((step) => step.run === 'npm ci'),
		'acceptance must consume the locked deployment subset'
	);
});

test('an empty reconciled scope skips maintenance and every production branch', async () => {
	const parent = parseYaml(
		await readFile(path.join(repoRoot, '.github/workflows/release.yml'), 'utf8')
	);
	const needs = Object.fromEntries(
		[
			'preflight',
			'private-preflight',
			'maintenance',
			'admin-backend',
			'admin-frontend',
			'school'
		].map((id) => [id, { result: 'skipped' }])
	);
	needs.preflight = {
		result: 'success',
		outputs: { deploy: '[]', plan: JSON.stringify({ deploy: [] }) }
	};
	for (const job of ['maintenance', 'admin-backend', 'admin-frontend', 'school', 'accept']) {
		const expression = parent.jobs[job].if.replace(/needs\.([a-z-]+)/g, 'needs["$1"]');
		const enabled = Function(
			'needs',
			'always',
			'contains',
			'fromJson',
			`return (${expression})`
		)(
			needs,
			() => true,
			(list, item) => list.includes(item),
			JSON.parse
		);
		assert.equal(enabled, false, job);
	}
	assert.equal(parent.jobs.school.with.plan, '${{ needs.preflight.outputs.plan }}');
	for (const step of parent.jobs.accept.steps.filter((step) => step.env?.PLAN))
		assert.equal(step.env.PLAN, '${{ needs.preflight.outputs.plan }}');
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

test('release acceptance uses public trust with an isolated explicit initial migration', async () => {
	const maintenance = await readFile(
		path.join(repoRoot, 'scripts/lib/pipeline-remote/maintenance.sh'),
		'utf8'
	);
	assert.match(maintenance, /mode == enter-origin-ca-migration/);
	assert.doesNotMatch(maintenance, /--cacert "\$origin_root"/);
	assert.match(maintenance, /--resolve "\$\{host\}:443:127.0.0.1"/);
	assert.match(maintenance, /SMOKE_RESOLVE_IP=127.0.0.1/);
	assert.doesNotMatch(maintenance, /SMOKE_CA_CERT=/);
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
	assert.match(
		String(workflow.jobs['accept-release'].if),
		/verify-release-ci\.result == 'success'/
	);
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
