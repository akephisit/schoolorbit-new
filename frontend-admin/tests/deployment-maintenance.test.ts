import assert from 'node:assert/strict';
import test from 'node:test';
import {
	deploymentIsInMaintenance,
	maintenanceResponse
} from '../src/lib/server/deployment-maintenance.ts';

test('maintenance requires a successful explicit status response; unavailable probes do not invent maintenance', async () => {
	for (const status of ['ready', 'maintenance']) {
		const result = await deploymentIsInMaintenance(
			'https://api.example.invalid',
			async (url, options) => {
				assert.equal(String(url), 'https://api.example.invalid/deployment-status');
				assert.equal(options?.cache, 'no-store');
				assert.ok(options?.signal);
				return Response.json({ status });
			}
		);
		assert.equal(result, status === 'maintenance');
	}
	assert.equal(
		await deploymentIsInMaintenance('https://api.example.invalid', async () => {
			throw new Error('offline');
		}),
		false
	);
	assert.equal(
		await deploymentIsInMaintenance(
			'https://api.example.invalid',
			async () => new Response('not JSON')
		),
		false
	);
	assert.equal(
		await deploymentIsInMaintenance('https://api.example.invalid', async () =>
			Response.json({ status: 'maintenance' }, { status: 503 })
		),
		false
	);
});

test('the maintenance response is uncached and reloads only when the status becomes ready', async () => {
	const response = maintenanceResponse('https://api.example.invalid');
	assert.equal(response.status, 503);
	assert.equal(response.headers.get('Cache-Control'), 'no-store');
	assert.equal(response.headers.get('Retry-After'), '10');
	const html = await response.text();
	assert.match(html, /กำลังปรับปรุงระบบ/);
	assert.match(html, /status==='ready'/);
	assert.match(html, /document.hidden/);
});
