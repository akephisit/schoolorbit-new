import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { access, readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { parse as parseYaml } from 'yaml';

const repoRoot = path.resolve(import.meta.dirname, '../../..');
const workflowPath = path.join(repoRoot, '.github/workflows/deploy-school-release.yml');

test('every production mutation requires release CI while builds stay parallel', async () => {
	const workflow = parseYaml(await readFile(workflowPath, 'utf8'));
	for (const name of ['deploy-backend', 'promote-frontends', 'accept-release']) {
		assert.ok(workflow.jobs[name].needs.includes('verify-release-ci'), name);
		assert.match(workflow.jobs[name].if, /needs\.verify-release-ci\.result == 'success'/);
	}
	for (const name of ['build-backend', 'stage-frontends']) {
		assert.ok(!JSON.stringify(workflow.jobs[name].needs).includes('verify-release-ci'));
		assert.doesNotMatch(workflow.jobs[name].if, /verify-release-ci/);
	}
	const gate = workflow.jobs['verify-release-ci'];
	assert.equal(gate.permissions.actions, 'read');
	assert.equal(gate.permissions.contents, 'read');
	assert.match(gate.if, /already_deployed != 'true'/);
	assert.match(workflow.jobs['accept-release'].if, /already_deployed == 'true'/);
	assert.ok(gate['timeout-minutes'] > 30);
	const push = gate.steps.find(
		(step) => step.name === 'Verify trusted push CI on this release SHA'
	);
	assert.equal(push.env.GH_TOKEN, '${{ github.token }}');
	assert.equal(push.env.PUSH_BASE_SHA, '${{ github.event.before }}');
	for (const [job, file] of [
		['manual-api-ci', 'api-contract.yml'],
		['manual-permission-ci', 'permission-contract.yml'],
		['manual-installer-ci', 'installer.yml']
	]) {
		assert.equal(workflow.jobs[job].uses, `./.github/workflows/${file}`);
		assert.match(workflow.jobs[job].if, /workflow_dispatch/);
		const definition = parseYaml(
			await readFile(path.join(repoRoot, '.github/workflows', file), 'utf8')
		);
		assert.ok(Object.hasOwn(definition.on, 'workflow_call'));
	}
	const summary = workflow.jobs['release-summary'].steps[0];
	assert.equal(summary.env.CI_RESULT, '${{ needs.verify-release-ci.result }}');
	assert.match(summary.run, /active release was preserved/);
});

test('CI refusal blocks backend-only, frontend-only and full production paths', async () => {
	const workflow = parseYaml(await readFile(workflowPath, 'utf8'));
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
						outputs: { needs_backend: backend, needs_frontend: frontend, already_deployed: 'false' }
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

test('manual CI failure blocks release for every required reusable workflow', async () => {
	const workflow = parseYaml(await readFile(workflowPath, 'utf8'));
	const step = workflow.jobs['verify-release-ci'].steps.find(
		(step) => step.name === 'Verify manual release CI results'
	);
	for (const key of ['API_RESULT', 'PERMISSION_RESULT', 'INSTALLER_RESULT']) {
		for (const status of ['failure', 'cancelled', 'skipped']) {
			const result = spawnSync('bash', ['-c', step.run], {
				env: {
					...process.env,
					API_RESULT: 'success',
					PERMISSION_RESULT: 'success',
					INSTALLER_RESULT: 'success',
					[key]: status,
					GITHUB_STEP_SUMMARY: '/dev/null',
					GITHUB_SHA: 'a'.repeat(40)
				},
				encoding: 'utf8'
			});
			assert.notEqual(result.status, 0, `${key}: ${status}`);
		}
	}
	assert.equal(
		spawnSync('bash', ['-c', step.run], {
			env: {
				...process.env,
				API_RESULT: 'success',
				PERMISSION_RESULT: 'success',
				INSTALLER_RESULT: 'success',
				GITHUB_STEP_SUMMARY: '/dev/null',
				GITHUB_SHA: 'a'.repeat(40)
			}
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
	const source = await readFile(workflowPath, 'utf8');
	const resolver = await readFile(
		path.join(repoRoot, 'scripts/resolve_school_release_scope.sh'),
		'utf8'
	);
	const workflow = parseYaml(source);

	assert.equal(workflow.name, 'Deploy School Release');
	assert.equal(workflow.concurrency.group, 'school-production-release');
	assert.equal(workflow.concurrency['cancel-in-progress'], false);
	assert.equal(workflow.jobs['resolve-scope'].steps[0].with['fetch-depth'], 0);
	assert.equal(workflow.jobs['resolve-scope'].permissions.actions, 'read');
	assert.deepEqual(workflow.on.workflow_dispatch.inputs.release_scope.options, [
		'auto',
		'frontend',
		'backend',
		'full'
	]);
	assert.match(source, /RUNTIME_DEPLOY_ENABLED/);
	assert.match(source, /FRONTEND_DEPLOY_ENABLED/);
	assert.match(source, /\^\[0-9a-f\]\{40\}\$/);
	assert.match(resolver, /scope=frontend/);
	assert.match(resolver, /scope=backend/);
	assert.match(resolver, /scope=full/);
	assert.match(source, /artifacts\?name=school-release-state/);
	assert.match(source, /frontendAcceptedSha/);
	assert.match(source, /backendAcceptedSha/);
	assert.match(source, /force_full/);
	assert.match(source, /\.updated_at > \$accepted_at/);
	assert.match(source, /scripts\/resolve_school_release_scope\.sh/);
	assert.doesNotMatch(source, /BEFORE_SHA/);
});

test('an accepted run rerun becomes a no-op and restores its accepted state', async () => {
	const workflow = parseYaml(await readFile(workflowPath, 'utf8'));
	const resolve = workflow.jobs['resolve-scope'];
	const replay = resolve.steps.find(
		(step) => step.name === 'Detect a previously accepted run attempt'
	);
	const scope = resolve.steps.find((step) => step.name === 'Resolve and validate release scope');
	const acceptedState = workflow.jobs['accept-release'].steps.find(
		(step) => step.name === 'Create accepted release state'
	);
	const persistedState = workflow.jobs['accept-release'].steps.find(
		(step) => step.name === 'Persist accepted release state'
	);
	const releaseSummary = workflow.jobs['release-summary'].steps.find(
		(step) => step.name === 'Report bounded release outcome'
	);

	assert.equal(resolve.outputs.already_deployed, '${{ steps.replay.outputs.already_deployed }}');
	assert.equal(resolve.outputs.accepted_attempt, '${{ steps.replay.outputs.accepted_attempt }}');
	assert.equal(resolve.outputs.restore_state, '${{ steps.replay.outputs.restore_state }}');
	assert.equal(replay.env.GH_TOKEN, '${{ github.token }}');
	assert.match(replay.run, /jobs\?filter=all&per_page=100/);
	assert.match(replay.run, /actions\/runs\/\$\{GITHUB_RUN_ID\}/);
	assert.match(replay.run, /actions\/workflows\/\$\{workflow_id\}\/runs/);
	assert.match(replay.run, /created=>=/);
	assert.match(replay.run, /jq -s '\.'/);
	assert.match(replay.run, /resolve_school_release_replay\.mjs/);
	assert.equal(scope.env.ALREADY_DEPLOYED, '${{ steps.replay.outputs.already_deployed }}');
	assert.equal(scope.env.REPLAY_SCOPE, '${{ steps.replay.outputs.accepted_scope }}');
	assert.match(scope.run, /if \[ "\$ALREADY_DEPLOYED" = true \]/);
	assert.match(scope.run, /needs_frontend=false/);
	assert.match(scope.run, /needs_backend=false/);
	assert.equal(
		acceptedState.env.ALREADY_DEPLOYED,
		'${{ needs.resolve-scope.outputs.already_deployed }}'
	);
	assert.equal(
		acceptedState.env.REPLAY_FRONTEND,
		'${{ needs.resolve-scope.outputs.accepted_frontend }}'
	);
	assert.equal(
		acceptedState.env.REPLAY_BACKEND,
		'${{ needs.resolve-scope.outputs.accepted_backend }}'
	);
	assert.match(acceptedState.run, /if \[ "\$ALREADY_DEPLOYED" = true \]/);
	assert.match(String(persistedState.if), /restore_state == 'true'/);
	assert.equal(
		releaseSummary.env.ALREADY_DEPLOYED,
		'${{ needs.resolve-scope.outputs.already_deployed }}'
	);
	assert.match(releaseSummary.run, /already accepted in run attempt/);
});

test('full preparation stages every Worker before backend maintenance can begin', async () => {
	const source = await readFile(workflowPath, 'utf8');
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
	const source = await readFile(workflowPath, 'utf8');
	const readinessVerifier = await readFile(
		path.join(repoRoot, 'frontend-school/scripts/verify-tenant-worker-readiness.mjs'),
		'utf8'
	);
	const workflow = parseYaml(source);
	const promote = workflow.jobs['promote-frontends'];
	const accept = workflow.jobs['accept-release'];

	assert.match(JSON.stringify(promote.needs), /deploy-backend/);
	assert.match(String(promote.if), /deploy-backend\.result == 'success'/);
	assert.match(source, /actions\/download-artifact@v7/);
	assert.match(source, /\.candidateVersionId/);
	assert.match(source, /versions deploy --version-id/);
	assert.match(source, /--percentage 100/);
	assert.doesNotMatch(source, /versions deploy --version-tag/);
	assert.match(source, /Promoted tenant Worker did not activate the recorded candidate/);
	assert.match(readinessVerifier, /_app\/immutable/);
	assert.match(readinessVerifier, /schoolorbitAppMounted/);
	assert.match(source, /wrangler triggers deploy --config wrangler-triggers\.json/);
	assert.match(source, /\{name:\$name,routes:/);
	assert.match(source, /verify-tenant-worker-readiness\.mjs/);
	assert.match(source, /npm run sync:menu-routes/);
	assert.match(source, /127\.0\.0\.1:18081/);
	assert.match(JSON.stringify(accept.needs), /promote-frontends/);
	assert.match(String(accept.if), /promote-frontends\.result == 'success'/);
	assert.match(source, /Open accepted School API release/);
	assert.match(source, /restore_maintenance/);
});

test('frontend promotion waits for complete tenant assets and application mount readiness', async () => {
	const workflow = parseYaml(await readFile(workflowPath, 'utf8'));
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
	const workflow = parseYaml(await readFile(workflowPath, 'utf8'));
	const acceptance = workflow.jobs['accept-release'].steps.find(
		(step) => step.name === 'Open accepted School API release'
	);

	assert.match(acceptance.with.script, /SMOKE_CA_CERT="\$origin_root"/);
});

test('backend runtime selects the candidate SHA without advancing latest before acceptance', async () => {
	const source = await readFile(workflowPath, 'utf8');
	const compose = await readFile(path.join(repoRoot, 'podman-compose.yml'), 'utf8');
	const acceptance = source.indexOf('Open accepted School API release');
	const localLatest = source.indexOf(
		'podman tag "${backend_image}:${{ needs.resolve-scope.outputs.release_id }}" "${backend_image}:latest"'
	);

	assert.match(
		compose,
		/\$\{BACKEND_SCHOOL_IMAGE_REFERENCE:-ghcr\.io\/akephisit\/schoolorbit-backend-school:latest\}/
	);
	assert.match(source, /export BACKEND_SCHOOL_IMAGE_REFERENCE="\$backend_school_image_reference"/);
	assert.match(
		source,
		/backend_image_digest="\$\{\{ needs\.build-backend\.outputs\.image_digest \}\}"/
	);
	assert.ok(localLatest > acceptance);
	assert.equal(
		source.indexOf('podman tag "${backend_image}:${{ github.sha }}" "${backend_image}:latest"'),
		-1
	);
	assert.match(source, /if: steps\.existing-backend-image\.outputs\.exists != 'true'/);
	assert.match(source, /Resolve immutable backend image digest/);
	assert.match(source, /manifest unknown/);
	assert.match(source, /Could not determine whether the immutable backend image exists/);
});

test('release acceptance requires successful scope resolution', async () => {
	const workflow = parseYaml(await readFile(workflowPath, 'utf8'));
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
	await assert.rejects(access(path.join(repoRoot, '.github/workflows/deploy-backend-school.yml')));
	await assert.rejects(access(path.join(repoRoot, '.github/workflows/deploy-all-schools.yml')));
	await access(path.join(repoRoot, '.github/workflows/deploy-school-tenant.yml'));

	const tenant = await readFile(
		path.join(repoRoot, '.github/workflows/deploy-school-tenant.yml'),
		'utf8'
	);
	const tenantWorkflow = parseYaml(tenant);
	assert.equal(tenantWorkflow.concurrency.group, 'school-production-release');
	assert.equal(tenantWorkflow.concurrency['cancel-in-progress'], false);
	assert.match(tenant, /workflow_dispatch/);
	assert.doesNotMatch(tenant, /^\s*push:/m);
});
