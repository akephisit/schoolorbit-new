import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { mountApiCalls } from '../helpers/route-startup-policy.mjs';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const appRoutes = path.join(projectRoot, 'src/routes/(app)');
const inventoryPath = path.join(projectRoot, 'tests/fixtures/route-data-loading-inventory.json');

async function pageFiles(directory) {
	const files = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) files.push(...(await pageFiles(fullPath)));
		else if (entry.name === '+page.svelte') files.push(fullPath);
	}
	return files;
}

async function sourceFiles(directory) {
	const files = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) files.push(...(await sourceFiles(fullPath)));
		else if (/\.(?:svelte|ts)$/.test(entry.name)) files.push(fullPath);
	}
	return files;
}

test('primary reads never start from page mount; browser subscriptions have reviewed owners', async () => {
	const inventory = JSON.parse(await readFile(inventoryPath, 'utf8'));
	const records = new Map(inventory.routes.map((record) => [record.route, record]));
	for (const file of await pageFiles(appRoutes)) {
		const source = await readFile(file, 'utf8');
		const route = path.relative(appRoutes, path.dirname(file)).split(path.sep).join('/');
		const record = records.get(route);
		const calls = mountApiCalls(source);
		const review = record?.browserMount;
		if (/\bonMount\b/.test(source) && /\$lib\/api\//.test(source)) {
			assert.ok(review?.reason?.trim(), `${route}: browser mount needs an explicit review reason`);
			assert.ok(review?.tests?.length, `${route}: browser mount needs a focused test owner`);
			for (const testFile of review.tests) await readFile(path.join(projectRoot, testFile), 'utf8');
		}
		assert.deepEqual(
			calls.filter((call) => !call.startsWith('subscribe:')),
			[],
			`${route}: primary mount API read`
		);
		assert.deepEqual(calls, review?.eventReads ?? [], `${route}: unreviewed subscription API read`);
	}
	const panel = await readFile(
		path.join(projectRoot, 'src/lib/features/session-security/SessionSecurityPanel.svelte'),
		'utf8'
	);
	assert.deepEqual(
		mountApiCalls(panel),
		[],
		'route-owned account list must not restart from its child mount'
	);
});

test('startup inspection follows aliases and local helpers without banning interaction reads', () => {
	const imports = `<script lang="ts">import {onMount as mounted} from 'svelte'; import {list as read} from '$lib/api/feature';`;
	assert.deepEqual(
		mountApiCalls(`${imports} function primary(){return read()} mounted(primary);</script>`),
		['read']
	);
	assert.deepEqual(
		mountApiCalls(`${imports} const primary=()=>read(); mounted(()=>primary());</script>`),
		['read']
	);
	assert.deepEqual(
		mountApiCalls(
			`${imports} function onClick(){return read()} mounted(()=>window.focus());</script>`
		),
		[]
	);
	assert.deepEqual(mountApiCalls(`${imports} mounted(()=>store.subscribe(()=>read()));</script>`), [
		'subscribe:read'
	]);
});

test('API contract contains no route-wide page-view endpoint', async () => {
	const contract = JSON.parse(
		await readFile(path.join(projectRoot, '../contracts/openapi/school-api.json'), 'utf8')
	);
	assert.deepEqual(
		Object.keys(contract.paths).filter((apiPath) => apiPath.endsWith('/page-view')),
		[]
	);
});

test('the application keeps safe hover data preload enabled', async () => {
	const app = await readFile(path.join(projectRoot, 'src/app.html'), 'utf8');
	assert.match(app, /<body[^>]*data-sveltekit-preload-data="hover"/);
});

test('route and component code uses focused invalidation', async () => {
	const allowedInvalidateAllOwners = new Set([]);
	const offenders = [];
	for (const file of await sourceFiles(path.join(projectRoot, 'src'))) {
		const relative = path.relative(projectRoot, file);
		if (
			/\binvalidateAll\s*\(/.test(await readFile(file, 'utf8')) &&
			!allowedInvalidateAllOwners.has(relative)
		) {
			offenders.push(relative);
		}
	}
	assert.deepEqual(offenders.sort(), []);
});
