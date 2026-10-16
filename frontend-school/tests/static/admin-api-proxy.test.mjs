import { readWorkflowSource } from '../helpers/workflow-source.mjs';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { renderProxy } from './helpers/render-proxy.mjs';

const testDirectory = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(testDirectory, '../../..');

async function readRepoFile(relativePath) {
	return relativePath.startsWith('.github/workflows/')
		? readWorkflowSource(repoRoot, relativePath)
		: readFile(path.join(repoRoot, relativePath), 'utf8');
}

test('the tracked admin API proxy routes only to backend-admin', async () => {
	const proxy = await renderProxy('nginx-configs/admin-api.conf.template');

	assert.match(proxy, /server_name admin-api\.example\.test;/);
	assert.match(proxy, /proxy_pass http:\/\/schoolorbit-backend-admin:8080;/);
	assert.doesNotMatch(proxy, /schoolorbit-backend-school|proxy_pass http:\/\/localhost/);
	assert.match(proxy, /X-Internal-Caller/);
	assert.match(proxy, /access_log off/);
});

test('Admin ready and maintenance CORS permit the apex without the retired alias or school Origins', async () => {
	for (const file of [
		'nginx-configs/admin-api.conf.template',
		'nginx-configs/admin-api.maintenance.conf.template'
	]) {
		const proxy = await renderProxy(file);
		assert.match(proxy, /"https:\/\/example\.test" \$http_origin;/);
		assert.doesNotMatch(proxy, /"https:\/\/admin\.example\.test"|\[\\w-\]/);
		const start = proxy.indexOf('server_name admin.example.test;');
		assert.ok(start >= 0);
		const retired = proxy.slice(start, proxy.indexOf('\n}', start));
		assert.match(retired, /return 410;/);
		assert.match(retired, /Cache-Control "no-store" always/);
		assert.doesNotMatch(retired, /proxy_pass|return 30[1278]/);
	}
});

test('backend-admin deployment installs and verifies the tracked proxy fail-closed', async () => {
	const workflow = await readRepoFile('.github/workflows/deploy-backend-admin.yml');

	assert.match(workflow, /nginx-configs\/admin-api\.conf\.template/);
	assert.match(workflow, /scripts\/render_nginx_config\.sh/);
	assert.match(workflow, /podman-compose\.yml/);
	assert.match(workflow, /podman exec schoolorbit-nginx nginx -t/);
	assert.match(workflow, /podman exec schoolorbit-nginx nginx -s reload/);
	assert.match(workflow, /SchoolOrbit Backend Admin/);
	assert.match(workflow, /proxy_backup/);
	assert.match(workflow, /restore_proxy/);
	assert.match(workflow, /cloudflare-origin-rsa-root\.pem/);
	assert.match(workflow, /--resolve "\$\{admin_host\}:443:127\.0\.0\.1"/);
	assert.match(workflow, /direct_admin_ok=false/);
	assert.match(workflow, /for attempt in \$\(seq 1 12\)/);
	assert.doesNotMatch(workflow, /nginx -s reload \|\| true/);
	assert.doesNotMatch(workflow, /https:\/\/admin-api\.schoolorbit\.app\//);
});
