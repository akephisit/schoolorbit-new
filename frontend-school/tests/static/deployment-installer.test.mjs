import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { access, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import test from 'node:test';

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(import.meta.dirname, '../../..');
const readRepo = (file) => readFile(path.join(repoRoot, file), 'utf8');

test('the resolved production topology has one owner and private backend ports', async () => {
	const { stdout } = await execFileAsync(
		'docker',
		[
			'compose',
			'--env-file',
			'scripts/tests/installer/fixtures/runtime.env',
			'-f',
			'podman-compose.yml',
			'config',
			'--format',
			'json'
		],
		{ cwd: repoRoot }
	);
	const topology = JSON.parse(stdout);

	for (const standalone of [
		'backend-admin/docker-compose.yml',
		'backend-school/docker-compose.yml'
	]) {
		await assert.rejects(access(path.join(repoRoot, standalone)));
	}
	assert.deepEqual(Object.keys(topology.services).sort(), [
		'backend-admin',
		'backend-school',
		'clamd',
		'nginx'
	]);
	assert.equal(topology.networks['schoolorbit-net'].name, 'schoolorbit-web');
	assert.equal(
		topology.networks['file-platform-internal'].name,
		'schoolorbit-file-platform-internal'
	);
	assert.equal(topology.networks['clamav-egress'].name, 'schoolorbit-clamav-egress');
	assert.equal(topology.volumes.clamav_signatures.name, 'schoolorbit-clamav-signatures');
	assert.equal(topology.services.nginx.depends_on, undefined);
	for (const [service, target] of [
		['backend-admin', 8080],
		['backend-school', 8081]
	]) {
		assert.deepEqual(topology.services[service].ports, [
			{
				mode: 'ingress',
				host_ip: '127.0.0.1',
				target,
				published: String(target),
				protocol: 'tcp'
			}
		]);
	}
});

test('the proxy renderer substitutes only a validated base domain', async (t) => {
	const temporary = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-nginx-'));
	t.after(() => rm(temporary, { recursive: true, force: true }));
	const output = path.join(temporary, 'school.conf');

	await execFileAsync(path.join(repoRoot, 'scripts/render_nginx_config.sh'), [
		path.join(repoRoot, 'nginx-configs/school-api.conf.template'),
		output,
		'example.test'
	]);

	const rendered = await readFile(output, 'utf8');
	assert.match(rendered, /server_name school-api\.example\.test;/);
	assert.match(rendered, /\(\[\\w-\]\+\\\.\)\?example\\\.test/);
	assert.match(rendered, /ssl_certificate \/etc\/nginx\/ssl\/schoolorbit-origin\.pem;/);
	assert.match(rendered, /ssl_certificate_key \/etc\/nginx\/ssl\/schoolorbit-origin\.key;/);
	assert.doesNotMatch(rendered, /\$\{BASE_DOMAIN(?:_REGEX)?\}/);
	assert.doesNotMatch(rendered, /schoolorbit\.app/);
});

test('the proxy renderer rejects an invalid domain without replacing its output', async (t) => {
	const temporary = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-nginx-invalid-'));
	t.after(() => rm(temporary, { recursive: true, force: true }));
	const output = path.join(temporary, 'school.conf');
	await writeFile(output, 'known-good\n');

	await assert.rejects(
		execFileAsync(path.join(repoRoot, 'scripts/render_nginx_config.sh'), [
			path.join(repoRoot, 'nginx-configs/school-api.conf.template'),
			output,
			'Example Test'
		]),
		(error) => error.code === 64 && error.stderr === 'Invalid base domain\n'
	);
	assert.equal(await readFile(output, 'utf8'), 'known-good\n');
});

test('backend workflows deploy the canonical target and verify the selected origin', async () => {
	const originRootInstaller = await readRepo(
		'scripts/lib/schoolorbit-installer/remote/install_origin_root.sh'
	);
	assert.match(
		originRootInstaller,
		/https:\/\/developers\.cloudflare\.com\/ssl\/static\/origin_ca_rsa_root\.pem/
	);
	assert.match(
		originRootInstaller,
		/91a8a5567efa6bf941162aa806b3ba476aaddf7867640e53053b35fb225a5dae/
	);

	for (const file of [
		'.github/workflows/deploy-backend-admin.yml',
		'.github/workflows/deploy-backend-school.yml'
	]) {
		const workflow = await readRepo(file);
		assert.match(workflow, /podman-compose\.yml/);
		assert.match(workflow, /scripts\/render_nginx_config\.sh/);
		assert.match(workflow, /scripts\/lib\/schoolorbit-installer\/remote\/install_origin_root\.sh/);
		assert.match(workflow, /"\$origin_root_installer" "\$origin_root"/);
		assert.match(workflow, /deployment_id/);
		assert.match(workflow, /RUNTIME_DEPLOY_ENABLED/);
		assert.match(workflow, /--resolve/);
		assert.match(workflow, /cloudflare-origin-rsa-root\.pem/);
		assert.match(workflow, /\/opt\/stack\/deployment/);
		assert.match(workflow, /podman-compose -f "\$\{runtime_compose\}\.next" --dry-run up -d/);
		assert.match(workflow, /legacy_proxy_target=\/opt\/stack\/nginx\/conf\.d\/(?:admin|school)-api\.\$\{base_domain\}\.conf/);
		assert.match(workflow, /proxy_previous_target="\$proxy_target"/);
		assert.match(workflow, /rm -f "\$proxy_previous_target"/);
		assert.match(workflow, /compose_up_quiet\(\)/);
		assert.match(workflow, /podman-compose -f "\$runtime_compose" up -d "\$@" >\/dev\/null 2>&1/);
		assert.match(workflow, /reconnect_backend_network\(\)/);
		assert.match(workflow, /validate_nginx_config_with_retry\(\)/);
		assert.ok(
			(workflow.match(/validate_nginx_config_with_retry/g) ?? []).length >= 3,
			`${file} must retry Nginx validation during activation and recovery`
		);
		assert.match(workflow, /podman network connect --alias "\$service_alias" --alias "\$container" schoolorbit-web "\$container"/);
		assert.match(workflow, /podman rm schoolorbit-nginx >\/dev\/null 2>&1 \|\| true/);
		assert.match(workflow, /timeout 180 bash/);
		assert.match(workflow, /grep -lF "server_name/);
		assert.match(workflow, /group: deploy-schoolorbit-runtime/);
		assert.equal((workflow.match(/port: \$\{\{ secrets\.SERVER_PORT \}\}/g) ?? []).length, 2);
		assert.doesNotMatch(workflow, /backend-(?:admin|school)\/docker-compose\.yml/);
		assert.doesNotMatch(workflow, /file-platform-runtime/);
		assert.doesNotMatch(workflow, /"\$\{runtime_compose\}\.next" config/);
		assert.doesNotMatch(workflow, /curl[^\n]*https:\/\/(?:admin-api|school-api)\.schoolorbit\.app/);
	}
});

test('frontend deployments keep environment values out of committed Worker configuration', async () => {
	const wrangler = JSON.parse(await readRepo('frontend-admin/wrangler.json'));
	assert.equal(wrangler.account_id, undefined);
	assert.equal(wrangler.vars, undefined);

	const admin = await readRepo('.github/workflows/deploy-frontend-admin.yml');
	assert.match(admin, /secrets:\s*\|\s*\n\s*INTERNAL_API_SECRET/);
	assert.match(admin, /vars\.BACKEND_ADMIN_URL/);
	assert.match(admin, /vars\.BACKEND_SCHOOL_URL/);
	assert.match(admin, /vars\.BASE_DOMAIN/);
	assert.match(admin, /vars\.CLOUDFLARE_ACCOUNT_ID/);
	assert.match(admin, /wrangler\.deploy\.json/);
	assert.match(admin, /FRONTEND_DEPLOY_ENABLED/);
	const adminWorkerDeploy = admin.slice(admin.indexOf('- name: Deploy frontend-admin Worker'));
	assert.match(adminWorkerDeploy, /PUBLIC_API_URL: \$\{\{ vars\.BACKEND_ADMIN_URL \}\}/);
	assert.match(adminWorkerDeploy, /BACKEND_SCHOOL_URL: \$\{\{ vars\.BACKEND_SCHOOL_URL \}\}/);

	for (const file of [
		'.github/workflows/deploy-all-schools.yml',
		'.github/workflows/deploy-school-tenant.yml'
	]) {
		const workflow = await readRepo(file);
		assert.match(workflow, /vars\.BASE_DOMAIN/);
		assert.match(workflow, /vars\.BACKEND_SCHOOL_URL/);
		assert.match(workflow, /vars\.CLOUDFLARE_ACCOUNT_ID/);
		assert.match(workflow, /jq -n/);
		assert.doesNotMatch(workflow, /\.schoolorbit\.app\/\*/);
		assert.doesNotMatch(
			workflow,
			/secrets\.(?:BACKEND_SCHOOL_URL|VAPID_PUBLIC_KEY|CLOUDFLARE_ACCOUNT_ID)/
		);
	}
});

test('runtime diagnostics expose container state without environment or application logs', async () => {
	const workflow = await readRepo('.github/workflows/runtime-diagnostics.yml');

	assert.match(workflow, /workflow_dispatch/);
	assert.match(workflow, /State\.ExitCode/);
	assert.match(workflow, /State\.OOMKilled/);
	assert.match(workflow, /NetworkSettings\.Networks/);
	assert.match(workflow, /\.Aliases/);
	assert.match(workflow, /podman port schoolorbit-nginx/);
	assert.match(workflow, /podman exec schoolorbit-nginx nginx -t/);
	assert.doesNotMatch(workflow, /Config\.Env/);
	assert.doesNotMatch(workflow, /podman logs/);
	assert.doesNotMatch(workflow, /curl[^\n]*-[^\n]*k/);
});

test('installer CI enforces shell provider topology and workflow guards', async () => {
	const workflow = await readRepo('.github/workflows/installer.yml');

	assert.match(workflow, /runs-on: ubuntu-24\.04/);
	for (const path of [
		'scripts/schoolorbit-installer',
		'scripts/lib/schoolorbit-installer/**',
		'scripts/tests/installer/**',
		'podman-compose.yml',
		'nginx-configs/**',
		'.github/workflows/**',
		'.rules',
		'docs/OPERATIONS.md',
		'docs/PODMAN_SETUP.md',
		'docs/TESTING.md'
	]) {
		assert.ok(workflow.includes(path), `installer workflow must watch ${path}`);
	}
	for (const check of [
		'shellcheck scripts/schoolorbit-installer',
		'shfmt -d -i 4 -ci scripts/schoolorbit-installer',
		'bats scripts/tests/installer',
		'node --test frontend-school/tests/static/deployment-installer.test.mjs',
		'podman-compose -f podman-compose.yml --dry-run up -d',
		'rhysd/actionlint:1.7.7'
	]) {
		assert.ok(workflow.includes(check), `installer workflow must run ${check}`);
	}
});

test('durable operations docs describe the guarded replacement VPS path', async () => {
	const [operations, setup, adminReadme] = await Promise.all([
		readRepo('docs/OPERATIONS.md'),
		readRepo('docs/PODMAN_SETUP.md'),
		readRepo('frontend-admin/README.md')
	]);

	for (const value of [
		'RUNTIME_DEPLOY_ENABLED',
		'FRONTEND_DEPLOY_ENABLED',
		'migrate-vps --resume',
		'rollback-dns --run-id',
		'CUTOVER',
		'ROLLBACK',
		'Origin CA',
		'certificate_expiry'
	]) {
		assert.ok(operations.includes(value), `operations must contain ${value}`);
	}
	assert.match(setup, /schoolorbit-installer migrate-vps/);
	assert.match(setup, /Full \(strict\)/);
	assert.match(setup, /schoolorbit-web/);
	assert.doesNotMatch(setup, /certbot/i);
	assert.match(adminReadme, /repository variables/);
	assert.match(adminReadme, /Worker secret binding/);
	assert.match(adminReadme, /never owns production credentials or URLs/);
});
