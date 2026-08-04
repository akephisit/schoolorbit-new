import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const testDirectory = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(testDirectory, '../../..');
const discoveryScript = path.join(repoRoot, 'scripts/discover_school_tenants.sh');

test('pre-cutover frontend deployment pins API traffic to the selected origin', async () => {
	const workflow = await readFile(
		path.join(repoRoot, '.github/workflows/deploy-all-schools.yml'),
		'utf8'
	);
	const originRouting = await readFile(
		path.join(repoRoot, 'scripts/lib/schoolorbit-installer/configure_pre_cutover_origin.sh'),
		'utf8'
	);

	assert.match(workflow, /target_origin_ip:/);
	assert.match(workflow, /scripts\/lib\/schoolorbit-installer\/configure_pre_cutover_origin\.sh/);
	assert.match(workflow, /--resolve "\$school_origin:443:\$TARGET_ORIGIN_IP"/);
	assert.match(
		workflow,
		/CURL_CA_BUNDLE: \$\{\{ steps\.origin-routing\.outputs\.origin_ca_root \}\}/
	);
	assert.match(
		workflow,
		/NODE_EXTRA_CA_CERTS: \$\{\{ steps\.origin-routing\.outputs\.origin_ca_root \}\}/
	);
	assert.match(
		originRouting,
		/https:\/\/developers\.cloudflare\.com\/ssl\/static\/origin_ca_rsa_root\.pem/
	);
	assert.match(originRouting, /91a8a5567efa6bf941162aa806b3ba476aaddf7867640e53053b35fb225a5dae/);
	assert.match(originRouting, /admin-api\.\$base_domain/);
	assert.match(originRouting, /school-api\.\$base_domain/);
});

async function runDiscovery(handler, environment = {}) {
	const server = createServer(handler);
	await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));

	const address = server.address();
	assert.ok(address && typeof address !== 'string');

	const temporaryDirectory = await mkdtemp(path.join(tmpdir(), 'schoolorbit-tenant-discovery-'));
	const githubOutput = path.join(temporaryDirectory, 'github-output');

	try {
		const child = spawn('bash', [discoveryScript], {
			env: {
				...process.env,
				BACKEND_ADMIN_URL: `http://127.0.0.1:${address.port}`,
				INTERNAL_API_SECRET: 'test-internal-secret',
				GITHUB_OUTPUT: githubOutput,
				TENANT_DISCOVERY_MAX_ATTEMPTS: '1',
				TENANT_DISCOVERY_RETRY_DELAY_SECONDS: '0',
				...environment
			},
			stdio: ['ignore', 'pipe', 'pipe']
		});

		let stdout = '';
		let stderr = '';
		child.stdout.setEncoding('utf8');
		child.stderr.setEncoding('utf8');
		child.stdout.on('data', (chunk) => {
			stdout += chunk;
		});
		child.stderr.on('data', (chunk) => {
			stderr += chunk;
		});

		const exitCode = await new Promise((resolve, reject) => {
			child.once('error', reject);
			child.once('close', resolve);
		});

		let output = '';
		try {
			output = await readFile(githubOutput, 'utf8');
		} catch (error) {
			if (error?.code !== 'ENOENT') throw error;
		}

		return { exitCode, output, stdout, stderr };
	} finally {
		await new Promise((resolve, reject) => {
			server.close((error) => (error ? reject(error) : resolve()));
		});
		await rm(temporaryDirectory, { recursive: true, force: true });
	}
}

test('tenant discovery uses the authenticated Backend Admin school listing', async () => {
	let observedRequest;
	const result = await runDiscovery((request, response) => {
		observedRequest = {
			url: request.url,
			secret: request.headers['x-internal-secret'],
			caller: request.headers['x-internal-caller']
		};
		response.writeHead(200, { 'content-type': 'application/json' });
		response.end(
			JSON.stringify({
				schools: [
					{ id: '0e297ca4-0809-4aab-a03f-1915045257b8', subdomain: 'snwsb', status: 'active' },
					{ id: '4398d7c2-7788-4cb1-86d4-4336fbaeddbf', subdomain: 'sandbox', status: 'active' }
				],
				total: 2
			})
		);
	});

	assert.equal(result.exitCode, 0, result.stderr);
	assert.deepEqual(observedRequest, {
		url: '/internal/schools?status=active',
		secret: 'test-internal-secret',
		caller: 'deploy-all-schools'
	});
	assert.equal(result.output, 'schools=[{"subdomain":"snwsb"},{"subdomain":"sandbox"}]\n');
});

test('tenant discovery fails without publishing an empty matrix when the API fails', async () => {
	const result = await runDiscovery((_request, response) => {
		response.writeHead(404);
		response.end();
	});

	assert.notEqual(result.exitCode, 0);
	assert.equal(result.output, '');
	assert.match(result.stderr, /status 404/);
});

test('tenant discovery retries a rollout 404 before publishing the matrix', async () => {
	let attempts = 0;
	const result = await runDiscovery(
		(_request, response) => {
			attempts += 1;
			if (attempts < 3) {
				response.writeHead(404);
				response.end();
				return;
			}

			response.writeHead(200, { 'content-type': 'application/json' });
			response.end(
				JSON.stringify({
					schools: [
						{
							id: '0e297ca4-0809-4aab-a03f-1915045257b8',
							subdomain: 'snwsb',
							status: 'active'
						}
					],
					total: 1
				})
			);
		},
		{ TENANT_DISCOVERY_MAX_ATTEMPTS: '3' }
	);

	assert.equal(result.exitCode, 0, result.stderr);
	assert.equal(attempts, 3);
	assert.equal(result.output, 'schools=[{"subdomain":"snwsb"}]\n');
});
