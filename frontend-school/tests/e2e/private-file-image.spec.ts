import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__private-file-image-test';
const virtualModuleId = 'virtual:private-file-image-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubModulePrefix = '\0private-file-image-test-stub:';
const stubModules = new Map([
	[
		'$app/environment',
		'export const browser = true; export const building = false; export const dev = true;'
	],
	[
		'$app/paths',
		"export const base = ''; export const assets = ''; export const resolve = (path) => path;"
	],
	['$env/dynamic/public', 'export const env = {};'],
	['$env/static/public', "export const PUBLIC_BACKEND_URL = 'https://school-api.schoolorbit.app';"]
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
			if (stubModules.has(id)) return `${stubModulePrefix}${id}`;
		},
		load(id) {
			if (id.startsWith(stubModulePrefix)) {
				return stubModules.get(id.slice(stubModulePrefix.length));
			}
			if (id !== resolvedVirtualModuleId) return;
			return `
				import { mount } from 'svelte';
				import PrivateFileImage from '/src/lib/components/files/PrivateFileImage.svelte';
				mount(PrivateFileImage, {
					target: document.querySelector('#app'),
					props: { fileId: 'test-file', alt: 'Profile' }
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

test.beforeAll(async () => {
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
	await devServer.close();
});

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
