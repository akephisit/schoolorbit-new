import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

let devServer: ViteDevServer;
let baseUrl: string;

test.beforeAll(async () => {
	devServer = await createServer({
		root: frontendRoot,
		logLevel: 'silent',
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

test('applies the global 90% density without shrinking the viewport', async ({ page }) => {
	await page.setViewportSize({ width: 1920, height: 1080 });
	await page.goto(baseUrl);
	await expect(page.getByRole('heading', { name: 'SchoolOrbit', exact: true })).toBeVisible();

	const metrics = await page.evaluate(() => {
		const routeRoot = document.querySelector('.min-h-screen');
		if (!(routeRoot instanceof HTMLElement)) throw new Error('Landing route root not found');
		return {
			rootFontSize: getComputedStyle(document.documentElement).fontSize,
			routeHeight: routeRoot.getBoundingClientRect().height,
			viewportHeight: window.innerHeight,
			documentWidth: document.documentElement.scrollWidth,
			viewportWidth: window.innerWidth
		};
	});

	expect(metrics.rootFontSize).toBe('14.4px');
	expect(Math.abs(metrics.routeHeight - metrics.viewportHeight)).toBeLessThanOrEqual(1);
	expect(metrics.documentWidth).toBeLessThanOrEqual(metrics.viewportWidth + 1);
});
