import { expect, test, type Page } from '@playwright/test';
import { compile } from 'svelte/compiler';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__private-file-image-test';
const virtualModuleId = 'virtual:private-file-image-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const parentModuleId = 'virtual:private-file-image-parent';
const stubModulePrefix = '\0private-file-image-test-stub:';
const stubModules = new Map([
	[
		'$app/env',
		'export const browser = true; export const building = false; export const dev = true;'
	],
	[
		'$app/paths',
		"export const base = ''; export const assets = ''; export const resolve = (path) => path;"
	],
	['$env/dynamic/public', 'export const env = {};'],
	[
		'$app/env/public',
		"export const PUBLIC_BACKEND_URL = 'https://school-api.schoolorbit.app'; export const PUBLIC_SCHOOL_SUBDOMAIN = '';"
	]
]);
const png = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
	'base64'
);

function harnessPlugin(): Plugin {
	return {
		name: 'private-file-image-test-harness',
		enforce: 'pre',
		resolveId(id) {
			if (id === virtualModuleId) return resolvedVirtualModuleId;
			if (id === parentModuleId) return `\0${parentModuleId}`;
			if (stubModules.has(id)) return `${stubModulePrefix}${id}`;
		},
		load(id) {
			if (id === `\0${parentModuleId}`)
				return compile(
					`
				<script>
				import PrivateFileImage from '/src/lib/components/files/PrivateFileImage.svelte';
				import Roster from '/src/lib/components/academic/timetable/TimetableCollaborators.svelte';
				let params = $state({ fileId: 'test-file', resourceId: undefined });
				let shown = $state(true);
				let users = $state(Array.from({length: 4}, (_, index) => ({userId: 'user-'+index, name: 'Person '+index, color: '#123456', profileImageFileId: 'test-file-'+index, tabs: [{connectionId: 'tab-'+index, canJump: true, contextLabel: 'Board', activityLabel: 'Here'}]})));
				const roster = new URLSearchParams(location.search).get('mode') === 'roster';
				$effect(() => {
					const update = (event) => {
						const action = event.detail;
						if (action.type === 'refresh') { params = {...params}; users = users.map(user => ({...user, tabs: user.tabs.map(tab => ({...tab, activityLabel: String(action.index)}))})); }
						if (action.type === 'file') params = {...params, fileId: action.value};
						if (action.type === 'resource') params = {...params, resourceId: action.value};
						if (action.type === 'unmount') shown = false;
						if (action.type === 'join') users = [...users, {...users[0], userId: 'new-user', name: 'New person', profileImageFileId: 'new-file'}];
						if (action.type === 'leave') users = users.filter(user => user.userId !== 'user-0');
					};
					window.addEventListener('private-image-test', update);
					return () => window.removeEventListener('private-image-test', update);
				});
				</script>
				{#if shown}{#if roster}<Roster {users} onJump={() => {}} />{:else}<PrivateFileImage fileId={params.fileId} resourceId={params.resourceId} alt="Profile" />{/if}{/if}
			`,
					{
						filename: path.join(frontendRoot, 'src/PrivateFileImageTest.svelte'),
						generate: 'client',
						dev: true
					}
				).js.code;
			if (id.startsWith(stubModulePrefix)) {
				return stubModules.get(id.slice(stubModulePrefix.length));
			}
			if (id !== resolvedVirtualModuleId) return;
			return `
				import { mount } from 'svelte';
				import Parent from '${parentModuleId}';
				mount(Parent, {
					target: document.querySelector('#app'),
				});
			`;
		},
		configureServer(server) {
			server.middlewares.use((request, response, next) => {
				const pathname = new URL(request.url ?? '/', 'http://test').pathname;
				if (pathname !== harnessPath) return next();
				response.setHeader('Content-Type', 'text/html; charset=utf-8');
				response.end(
					`<div id="app"></div><script type="module" src="/@id/${virtualModuleId}"></script>`
				);
			});
		}
	};
}

let devServer: ViteDevServer;
let baseUrl: string;
const previousBackendUrl = process.env.PUBLIC_BACKEND_URL;

test.beforeAll(async () => {
	process.env.PUBLIC_BACKEND_URL = 'https://school-api.schoolorbit.app';
	devServer = await createServer({
		root: frontendRoot,
		logLevel: 'silent',
		plugins: [harnessPlugin()],
		server: { host: '127.0.0.1', port: 0 }
	});
	await devServer.listen();
	const address = devServer.httpServer?.address();
	if (!address || typeof address === 'string') throw new Error('Vite test server did not start');
	baseUrl = `http://127.0.0.1:${address.port}`;
});

test.afterAll(async () => {
	await devServer?.close();
	if (previousBackendUrl === undefined) delete process.env.PUBLIC_BACKEND_URL;
	else process.env.PUBLIC_BACKEND_URL = previousBackendUrl;
});

test.beforeEach(async ({ page }) => {
	await page.addInitScript(() => {
		Object.defineProperty(globalThis, '__sveltekit_dev', {
			value: { env: {} },
			configurable: true,
			writable: true
		});
		const create = URL.createObjectURL.bind(URL),
			revoke = URL.revokeObjectURL.bind(URL);
		let created = 0,
			revoked = 0;
		URL.createObjectURL = (blob) => {
			document.documentElement.dataset.createdUrls = String(++created);
			return create(blob);
		};
		URL.revokeObjectURL = (url) => {
			document.documentElement.dataset.revokedUrls = String(++revoked);
			revoke(url);
		};
	});
});

async function action(page: Page, type: string, value?: string, index?: number) {
	await page.evaluate(
		(detail) => window.dispatchEvent(new CustomEvent('private-image-test', { detail })),
		{ type, value, index }
	);
	await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));
}
async function metrics(page: Page) {
	return page.evaluate(() => ({
		created: Number(document.documentElement.dataset.createdUrls ?? 0),
		revoked: Number(document.documentElement.dataset.revokedUrls ?? 0)
	}));
}
function gate() {
	let release!: () => void;
	const wait = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { wait, release };
}
async function delivery(
	page: Page,
	held?: { phase: 'grant' | 'blob'; wait: Promise<void> },
	failure = false
) {
	const grants: string[] = [],
		blobs: string[] = [];
	await page.route('https://school-api.schoolorbit.app/api/files/**/download*', async (route) => {
		const id = new URL(route.request().url()).pathname.split('/').at(-2)!;
		grants.push(route.request().url());
		if (id === 'test-file' && held?.phase === 'grant') await held.wait;
		await route.fulfill({
			status: failure && id === 'test-file' ? 400 : 200,
			contentType: 'application/json',
			headers: {
				'Access-Control-Allow-Origin': baseUrl,
				'Access-Control-Allow-Credentials': 'true'
			},
			body: JSON.stringify(
				failure && id === 'test-file'
					? { success: false, error: 'denied' }
					: {
							success: true,
							data: { url: `${baseUrl}/__blob/${id}.png`, expiresAt: '2099-01-01T00:00:00Z' }
						}
			)
		});
	});
	await page.route(`${baseUrl}/__blob/**`, async (route) => {
		blobs.push(route.request().url());
		if (
			new URL(route.request().url()).pathname.endsWith('/test-file.png') &&
			held?.phase === 'blob'
		)
			await held.wait;
		await route.fulfill({ status: 200, contentType: 'image/png', body: png });
	});
	return { grants, blobs };
}

test('keeps a private image hidden until the downloaded blob loads', async ({ page }) => {
	let releaseGrant = () => {};
	const heldGrant = new Promise<void>((resolve) => {
		releaseGrant = resolve;
	});
	let markGrantRequested = () => {};
	const grantRequested = new Promise<void>((resolve) => {
		markGrantRequested = resolve;
	});

	await page.route(
		'https://school-api.schoolorbit.app/api/files/test-file/download',
		async (route) => {
			markGrantRequested();
			await heldGrant;
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				headers: {
					'Access-Control-Allow-Origin': baseUrl,
					'Access-Control-Allow-Credentials': 'true'
				},
				body: JSON.stringify({
					success: true,
					data: {
						url: `${baseUrl}/__private-file-image.png`,
						expiresAt: '2099-01-01T00:00:00Z'
					}
				})
			});
		}
	);
	await page.route(`${baseUrl}/__private-file-image.png`, (route) =>
		route.fulfill({ status: 200, contentType: 'image/png', body: png })
	);

	await page.goto(`${baseUrl}${harnessPath}`);
	await grantRequested;
	const image = page.locator('img[alt="Profile"]');

	try {
		await expect(image).toHaveCSS('visibility', 'hidden');
	} finally {
		releaseGrant();
	}

	await expect(image).toHaveCSS('visibility', 'visible');
	await expect
		.poll(() => image.evaluate((node) => (node as HTMLImageElement).naturalWidth))
		.toBe(1);
});

test('fresh parent objects preserve an image URL while real file/resource changes and unmount revoke only their own URL', async ({
	page
}) => {
	const requests = await delivery(page);
	await page.goto(`${baseUrl}${harnessPath}`);
	const image = page.locator('img[alt="Profile"]');
	await expect(image).toHaveCSS('visibility', 'visible');
	const original = await image.elementHandle(),
		src = await image.getAttribute('src');
	for (let index = 0; index < 10; index++) await action(page, 'refresh', undefined, index);
	await expect(image).toHaveAttribute('src', src!);
	expect(await image.evaluate((node, previous) => node === previous, original)).toBe(true);
	expect(requests.grants).toHaveLength(1);
	expect(requests.blobs).toHaveLength(1);
	expect(await metrics(page)).toEqual({ created: 1, revoked: 0 });
	await action(page, 'file', 'second-file');
	await expect(image).not.toHaveAttribute('src', src!);
	await expect(image).toHaveCSS('visibility', 'visible');
	const second = await image.getAttribute('src');
	await action(page, 'resource', 'resource-two');
	await expect(image).not.toHaveAttribute('src', second!);
	await expect(image).toHaveCSS('visibility', 'visible');
	expect(requests.grants).toHaveLength(3);
	expect(requests.blobs).toHaveLength(3);
	expect(new URL(requests.grants.at(-1)!).searchParams.get('resource_id')).toBe('resource-two');
	expect(await metrics(page)).toEqual({ created: 3, revoked: 2 });
	await action(page, 'unmount');
	await expect(image).toHaveCount(0);
	expect(await metrics(page)).toEqual({ created: 3, revoked: 3 });
});

test('four-plus roster members retain visible avatar identity through presence replacements and legitimate joins/leaves', async ({
	page
}) => {
	const requests = await delivery(page);
	await page.goto(`${baseUrl}${harnessPath}?mode=roster`);
	const avatars = page.locator('[data-timetable-roster] img');
	await expect(avatars).toHaveCount(3);
	for (const image of await avatars.all()) await expect(image).toHaveCSS('visibility', 'visible');
	const nodes = await avatars.elementHandles(),
		sources = await avatars.evaluateAll((images) =>
			images.map((image) => image.getAttribute('src'))
		);
	for (let index = 0; index < 10; index++) await action(page, 'refresh', undefined, index);
	expect(
		await avatars.evaluateAll((images) => images.map((image) => image.getAttribute('src')))
	).toEqual(sources);
	for (let index = 0; index < nodes.length; index++)
		expect(
			await avatars.nth(index).evaluate((node, previous) => node === previous, nodes[index])
		).toBe(true);
	expect(requests.grants).toHaveLength(3);
	expect(requests.blobs).toHaveLength(3);
	expect(await metrics(page)).toEqual({ created: 3, revoked: 0 });
	await action(page, 'join');
	expect(requests.grants).toHaveLength(3);
	await action(page, 'leave');
	await expect(page.locator('[data-timetable-collaborator="user-3"] img')).toHaveCSS(
		'visibility',
		'visible'
	);
	expect(requests.grants).toHaveLength(4);
	expect(requests.blobs).toHaveLength(4);
	expect(await metrics(page)).toEqual({ created: 4, revoked: 1 });
});

for (const phase of ['grant', 'blob'] as const) {
	test(`a delayed old ${phase} cannot overwrite a newer file or allocate a stale object URL`, async ({
		page
	}) => {
		const held = gate();
		const requests = await delivery(page, { phase, wait: held.wait });
		try {
			await page.goto(`${baseUrl}${harnessPath}`);
			await expect
				.poll(() => (phase === 'grant' ? requests.grants.length : requests.blobs.length))
				.toBe(1);
			await action(page, 'file', 'second-file');
			const image = page.locator('img[alt="Profile"]');
			await expect(image).toHaveCSS('visibility', 'visible');
			const current = await image.getAttribute('src');
			held.release();
			await action(page, 'refresh');
			await expect(image).toHaveAttribute('src', current!);
			expect(await metrics(page)).toEqual({ created: 1, revoked: 0 });
			expect(requests.blobs).toHaveLength(phase === 'grant' ? 1 : 2);
			await action(page, 'unmount');
			expect(await metrics(page)).toEqual({ created: 1, revoked: 1 });
		} finally {
			held.release();
		}
	});
}

test('unmount during a pending grant never downloads or reveals its stale image', async ({
	page
}) => {
	const held = gate();
	const requests = await delivery(page, { phase: 'grant', wait: held.wait });
	try {
		await page.goto(`${baseUrl}${harnessPath}`);
		await expect.poll(() => requests.grants.length).toBe(1);
		await action(page, 'unmount');
		held.release();
		await action(page, 'refresh');
		await expect(page.locator('img')).toHaveCount(0);
		expect(requests.blobs).toHaveLength(0);
		expect(await metrics(page)).toEqual({ created: 0, revoked: 0 });
	} finally {
		held.release();
	}
});

test('download errors stay hidden without mouse-driven retries and a real new file can recover', async ({
	page
}) => {
	const requests = await delivery(page, undefined, true);
	const errors: string[] = [];
	page.on('console', (message) => {
		if (message.type() === 'error' && message.text().includes('Unable to load private file image'))
			errors.push(message.text());
	});
	await page.goto(`${baseUrl}${harnessPath}`);
	await expect.poll(() => errors.length).toBe(1);
	await expect(page.locator('img')).toHaveCSS('visibility', 'hidden');
	await action(page, 'refresh');
	expect(requests.grants).toHaveLength(1);
	await action(page, 'file', 'second-file');
	await expect(page.locator('img')).toHaveCSS('visibility', 'visible');
	expect(requests.grants).toHaveLength(2);
	expect(requests.blobs).toHaveLength(1);
});
