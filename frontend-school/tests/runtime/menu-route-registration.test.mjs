import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

async function writeRouteTree(files) {
	const root = await mkdtemp(path.join(tmpdir(), 'schoolorbit-menu-routes-'));
	for (const [relativePath, source] of Object.entries(files)) {
		const target = path.join(root, relativePath);
		await mkdir(path.dirname(target), { recursive: true });
		await writeFile(target, source, 'utf8');
	}
	return root;
}

function runMenuSync(environment) {
	return new Promise((resolve, reject) => {
		const { MENU_ROUTES_ROOT: routesRoot, ...commandEnvironment } = environment;
		const args = ['run', 'sync:menu-routes'];
		if (routesRoot) {
			args.push('--', routesRoot);
		}
		const child = spawn('npm', args, {
			cwd: projectRoot,
			env: { ...process.env, ...commandEnvironment },
			stdio: ['ignore', 'pipe', 'pipe']
		});
		let stdout = '';
		let stderr = '';
		child.stdout.on('data', (chunk) => {
			stdout += chunk;
		});
		child.stderr.on('data', (chunk) => {
			stderr += chunk;
		});
		child.on('error', reject);
		child.on('close', (code) => resolve({ code, stdout, stderr }));
	});
}

async function runSyncWithServer(t, files, response = {}) {
	const routesRoot = await writeRouteTree(files);
	t.after(() => rm(routesRoot, { recursive: true, force: true }));

	let receivedRequest;
	const server = createServer((request, serverResponse) => {
		let body = '';
		request.setEncoding('utf8');
		request.on('data', (chunk) => {
			body += chunk;
		});
		request.on('end', () => {
			receivedRequest = {
				method: request.method,
				url: request.url,
				deployKey: request.headers['x-deploy-key'],
				subdomain: request.headers['x-school-subdomain'],
				body: JSON.parse(body)
			};
			serverResponse.writeHead(response.status ?? 200, { 'content-type': 'application/json' });
			serverResponse.end(
				JSON.stringify(
					response.body ?? {
						success: true,
						registered: receivedRequest.body.routes.length,
						message: 'Synced'
					}
				)
			);
		});
	});
	await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
	t.after(() => new Promise((resolve) => server.close(resolve)));

	const address = server.address();
	assert.ok(address && typeof address === 'object');

	const result = await runMenuSync({
		PUBLIC_BACKEND_URL: `http://127.0.0.1:${address.port}`,
		DEPLOY_KEY: 'test-deploy-key',
		SUBDOMAIN: 'test-school',
		MENU_ROUTES_ROOT: routesRoot
	});

	return { ...result, receivedRequest };
}

const validRoute = `
			export const _meta = {
				menu: {
					title: 'Example service',
					icon: 'BookOpen',
					group: 'academic_foundation',
					workspace: 'academic',
					order: 15,
					permission: PERMISSIONS.MENU_READ_ALL,
					user_type: 'staff'
				}
			};
		`;

test('explicit menu synchronization sends the complete route scan after deployment', async (t) => {
	const result = await runSyncWithServer(t, {
		'src/routes/(app)/staff/example/+page.ts': validRoute
	});

	assert.equal(result.code, 0, result.stderr || result.stdout);
	assert.deepEqual(result.receivedRequest, {
		method: 'POST',
		url: '/api/admin/routes/sync',
		deployKey: 'test-deploy-key',
		subdomain: 'test-school',
		body: {
			routes: [
				{
					path: '/staff/example',
					title: 'Example service',
					icon: 'BookOpen',
					group: 'academic_foundation',
					workspace: 'academic',
					order: 15,
					permission: 'menu.read.all',
					user_type: 'staff'
				}
			],
			environment: 'production'
		}
	});
});

test('menu synchronization fails when route metadata cannot be parsed', async (t) => {
	const result = await runSyncWithServer(t, {
		'src/routes/(app)/staff/invalid/+page.ts': validRoute.replace(
			'PERMISSIONS.MENU_READ_ALL',
			'PERMISSIONS.NOT_A_REAL_PERMISSION'
		)
	});

	assert.equal(result.code, 1);
	assert.match(`${result.stdout}\n${result.stderr}`, /Failed to parse menu metadata.*\+page\.ts/s);
});

test('menu synchronization identifies a structurally incomplete metadata file', async (t) => {
	const result = await runSyncWithServer(t, {
		'src/routes/(app)/staff/incomplete/+page.ts': validRoute.replace(/;\s*$/, '')
	});

	assert.equal(result.code, 1);
	assert.match(
		`${result.stdout}\n${result.stderr}`,
		/Failed to parse menu metadata.*staff\/incomplete\/\+page\.ts/s
	);
});

test('menu synchronization rejects an empty desired route scan', async (t) => {
	const result = await runSyncWithServer(t, {});

	assert.equal(result.code, 1);
	assert.match(`${result.stdout}\n${result.stderr}`, /No menu routes found/);
});

test('menu synchronization rejects duplicate route paths', async (t) => {
	const result = await runSyncWithServer(t, {
		'src/routes/(app)/(first)/staff/example/+page.ts': validRoute,
		'src/routes/(app)/(second)/staff/example/+page.ts': validRoute
	});

	assert.equal(result.code, 1);
	assert.match(`${result.stdout}\n${result.stderr}`, /Duplicate menu route path: \/staff\/example/);
});

test('menu synchronization surfaces a backend rejection', async (t) => {
	const result = await runSyncWithServer(
		t,
		{ 'src/routes/(app)/staff/example/+page.ts': validRoute },
		{ status: 500, body: { success: false, error: 'rejected' } }
	);

	assert.equal(result.code, 1);
	assert.match(`${result.stdout}\n${result.stderr}`, /Backend returned 500/);
	assert.doesNotMatch(`${result.stdout}\n${result.stderr}`, /test-deploy-key/);
});

test('menu synchronization rejects an invalid success response', async (t) => {
	const result = await runSyncWithServer(
		t,
		{ 'src/routes/(app)/staff/example/+page.ts': validRoute },
		{ body: { success: false, registered: 0, message: 'Not synchronized' } }
	);

	assert.equal(result.code, 1);
	assert.match(
		`${result.stdout}\n${result.stderr}`,
		/Backend did not confirm menu synchronization/
	);
});
