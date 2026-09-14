import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { access, readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { parse as parseYaml } from 'yaml';

const repoRoot = path.resolve(import.meta.dirname, '../../..');
const workflowPath = path.join(repoRoot, '.github/workflows/deploy-school-release.yml');

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
	assert.doesNotMatch(result.stdout, /--version-tag/);
});

test('frontend promotion is gated by backend acceptance and ready waits for every tenant', async () => {
	const source = await readFile(workflowPath, 'utf8');
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
	assert.match(source, /_app\/immutable/);
	assert.match(source, /schoolorbitAppMounted/);
	assert.match(source, /wrangler triggers deploy --config wrangler-triggers\.json/);
	assert.match(source, /\{name:\$name,routes:/);
	assert.match(source, /url_effective/);
	assert.match(source, /new URL\(process\.argv\[1\], process\.argv\[2\]\)/);
	assert.match(source, /npm run sync:menu-routes/);
	assert.match(source, /127\.0\.0\.1:18081/);
	assert.match(JSON.stringify(accept.needs), /promote-frontends/);
	assert.match(String(accept.if), /promote-frontends\.result == 'success'/);
	assert.match(source, /Open accepted School API release/);
	assert.match(source, /restore_maintenance/);
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
