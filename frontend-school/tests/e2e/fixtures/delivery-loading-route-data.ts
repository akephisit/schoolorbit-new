import type { Page } from '@playwright/test';
import {
	mockDeliveryNavigation,
	navigationDelivery,
	navigationWorkspace
} from './delivery-navigation-route-data';

type Region = 'homerooms' | 'offerings';
type DeliveryFrame = { ready: boolean; skeleton: boolean };

declare global {
	interface Window {
		deliveryFrames?: DeliveryFrame[];
	}
}

export function deferred() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { promise, release };
}

export async function mockDeliveryLoading(page: Page, options: { manager?: boolean } = {}) {
	const api = await mockDeliveryNavigation(page, options);
	const counts = { homerooms: 0, offerings: 0 };
	const gates = new Map<string, ReturnType<typeof deferred>>();
	await page.route(
		(url) =>
			url.pathname === '/api/academic/delivery/homerooms' ||
			url.pathname === '/api/academic/delivery/workspace',
		async (route) => {
			if (route.request().method() !== 'GET') {
				await route.fallback();
				return;
			}
			const url = new URL(route.request().url());
			const region: Region = url.pathname.endsWith('/homerooms') ? 'homerooms' : 'offerings';
			const key = `${region}:${++counts[region]}`;
			await gates.get(key)?.promise;
			const data = region === 'homerooms' ? navigationWorkspace() : navigationDelivery();
			await route.fulfill({ json: { success: true, data } });
		}
	);
	return {
		...api,
		counts,
		hold(region: Region, read = counts[region] + 1) {
			const gate = deferred();
			gates.set(`${region}:${read}`, gate);
			return gate;
		}
	};
}

export async function addDeliveryLink(
	page: Page,
	href: string,
	preload: 'hover' | 'tap' | 'false' = 'false'
) {
	await page.evaluate(
		({ href, preload }) => {
			const link = document.createElement('a');
			link.href = href;
			link.textContent = 'Open delivery fixture';
			link.dataset.sveltekitPreloadData = preload;
			link.style.cssText = 'position:fixed;right:10px;bottom:10px;z-index:9999;background:white';
			document.body.append(link);
		},
		{ href, preload }
	);
	return page.getByRole('link', { name: 'Open delivery fixture', exact: true });
}

export async function recordDeliveryFrames(page: Page) {
	await page.addInitScript(() => {
		const frames: DeliveryFrame[] = [];
		window.deliveryFrames = frames;
		function inspect() {
			const region = document.querySelector('section[aria-label="มุมมองรายห้อง"]');
			if (region)
				frames.push({
					ready: region.hasAttribute('data-testid'),
					skeleton: !!region.querySelector('[data-slot="skeleton"]')
				});
			requestAnimationFrame(inspect);
		}
		requestAnimationFrame(inspect);
	});
}

export async function deliveryFrames(page: Page) {
	return page.evaluate(() => {
		if (!window.deliveryFrames) throw new Error('Delivery frame recording was not initialized');
		return window.deliveryFrames;
	});
}
