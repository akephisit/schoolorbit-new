import { expect, test } from '@playwright/test';
import { createServer as createHttpServer, type Server } from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const entry = 'virtual:file-cors-cache';
const image = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a8S8AAAAASUVORK5CYII=',
	'base64'
);
let assets: Server;
let app: ViteDevServer;
let base: string;
let assetUrl: string;
let origins: Array<string | undefined>;

function fixture(): Plugin {
	return {
		name: 'file-cors-cache-fixture',
		enforce: 'pre',
		resolveId(id) {
			if (id === entry) return `\0${entry}`;
		},
		load(id) {
			if (id !== `\0${entry}`) return;
			return `import { apiClient } from '/src/lib/api/client.ts';
			const image = document.querySelector('img');
			image.onload = () => document.querySelector('button').disabled = false;
			image.src = ${JSON.stringify(assetUrl)};
			document.querySelector('button').onclick = async () => {
				try {
					await fetch(${JSON.stringify(assetUrl)}, { mode: 'cors', credentials: 'omit', referrerPolicy: 'no-referrer' });
					document.querySelector('#baseline').textContent = 'Cached read succeeded';
				} catch { document.querySelector('#baseline').textContent = 'Cached read failed'; }
				try {
					const response = await apiClient.getExternalBlob(${JSON.stringify(assetUrl)});
					document.querySelector('[role=status]').textContent = response.success && response.data.size > 0 ? 'Download passed' : 'Download failed';
				} catch { document.querySelector('[role=status]').textContent = 'Download failed'; }
			};`;
		},
		configureServer(server) {
			server.middlewares.use((request, response, next) => {
				if (request.url !== '/__file-cors-cache') return next();
				response.setHeader('Content-Type', 'text/html');
				response.end(
					`<!doctype html><html><body><img alt="Cached branding"><button disabled>Download image bytes</button><p id="baseline"></p><p role="status"></p><script>globalThis.__sveltekit_dev={env:{PUBLIC_SCHOOL_SUBDOMAIN:""}};</script><script type="module" src="/@id/${entry}"></script></body></html>`
				);
			});
		}
	};
}

test.use({ serviceWorkers: 'block' });
test.beforeAll(async () => {
	origins = [];
	assets = createHttpServer((request, response) => {
		origins.push(request.headers.origin);
		response.setHeader('Content-Type', 'image/png');
		response.setHeader('Cache-Control', 'public, max-age=3600');
		// Public branding loaded as an image has no Origin and no Vary header.
		// This mirrors the production CDN response that caused the export failure.
		if (request.headers.origin) {
			response.setHeader('Access-Control-Allow-Origin', request.headers.origin);
			response.setHeader('Vary', 'Origin');
		}
		response.end(image);
	});
	await new Promise<void>((resolve) => assets.listen(0, '127.0.0.1', resolve));
	const address = assets.address();
	if (!address || typeof address === 'string') throw new Error('Asset fixture did not start');
	assetUrl = `http://127.0.0.1:${address.port}/logo.png`;
	process.env.PUBLIC_BACKEND_URL = 'http://127.0.0.1:9';
	process.env.PUBLIC_VAPID_KEY = 'test';
	app = await createServer({
		root,
		cacheDir: path.join(root, 'node_modules/.vite-file-cors-test'),
		logLevel: 'silent',
		plugins: [fixture()],
		server: { host: '127.0.0.1', port: 0 }
	});
	await app.listen();
	const appAddress = app.httpServer?.address();
	if (!appAddress || typeof appAddress === 'string') throw new Error('App fixture did not start');
	base = `http://127.0.0.1:${appAddress.port}`;
});
test.afterAll(async () => {
	await app?.close();
	await new Promise<void>((resolve, reject) =>
		assets?.close((error) => (error ? reject(error) : resolve()))
	);
});

test('file download bypasses an image cache response lacking CORS headers', async ({ page }) => {
	// Do not intercept requests: Playwright routing disables the HTTP cache.
	await page.goto(`${base}/__file-cors-cache`);
	await expect(page.getByRole('button', { name: 'Download image bytes' })).toBeEnabled();
	expect(origins).toEqual([undefined]);
	await page.getByRole('button', { name: 'Download image bytes' }).click();
	await expect(page.getByRole('status')).toHaveText('Download passed');
	await expect(page.locator('#baseline')).toHaveText('Cached read failed');
	expect(origins).toEqual([undefined, base]);
});
