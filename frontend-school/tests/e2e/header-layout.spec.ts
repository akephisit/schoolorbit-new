import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
let devServer: ViteDevServer;
let baseUrl: string;
const headerPath = '/__header-layout';
const entry = 'virtual:app-header-layout';

function headerHarness(): Plugin {
	return {
		name: 'app-header-layout-test',
		enforce: 'pre',
		resolveId(id) {
			if (id === '$env/dynamic/public') return '\0header-env';
			if (id === entry) return `\0${entry}`;
		},
		load(id) {
			if (id === '\0header-env') return 'export const env = {};';
			if (id !== `\0${entry}`) return;
			return `import { mount } from 'svelte';
			import '/src/routes/layout.css';
			import Header from '/tests/e2e/fixtures/AppHeaderHarness.svelte';
			mount(Header, { target: document.querySelector('#app') });`;
		},
		configureServer(server) {
			server.middlewares.use((request, response, next) => {
				if (new URL(request.url ?? '/', 'http://test').pathname !== headerPath) return next();
				response.setHeader('Content-Type', 'text/html; charset=utf-8');
				response.end(
					`<!doctype html><html lang="th"><head><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><div id="app"></div><script type="module" src="/@id/${entry}"></script></body></html>`
				);
			});
		}
	};
}

test.describe.configure({ mode: 'serial' });
test.use({ serviceWorkers: 'block' });
test.beforeAll(async () => {
	devServer = await createServer({
		root: frontendRoot,
		cacheDir: path.resolve(frontendRoot, 'node_modules/.vite-header-layout-test'),
		logLevel: 'silent',
		plugins: [headerHarness()],
		server: { host: '127.0.0.1', port: 0 }
	});
	await devServer.listen();
	const address = devServer.httpServer?.address();
	if (!address || typeof address === 'string') throw new Error('Header fixture did not start');
	baseUrl = `http://127.0.0.1:${address.port}`;
});
test.afterAll(async () => {
	await devServer?.close();
});
for (const width of [375, 1625]) {
	test(`the app header omits search and retains its controls at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 888 });
		await page.goto(`${baseUrl}${headerPath}`);
		await expect(page.getByTestId('header-fixture')).toBeVisible();
		await expect(page.locator('header input')).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'Search', exact: true })).toHaveCount(0);
		await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
		await expect(page.locator('html')).toHaveClass(/dark/);
		await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
		if (width < 1024) {
			await page.getByRole('button', { name: 'Open Menu' }).click();
			await expect(page.getByTestId('header-fixture')).toHaveText('เมนูเปิดแล้ว');
		}
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
		await page.screenshot({ path: `/tmp/schoolorbit-app-header-${width}.png`, fullPage: true });
	});
}
