import { expect, type Locator, type Page } from '@playwright/test';

// Kit revalidates same-URL navigation; each feature and layout owns whether its
// route/context policy retains mounted data and drafts.
// Exercise that browser behavior without adding a refresh control to the product.
export async function revalidatePage(page: Page) {
	await page.evaluate(() => {
		const link = document.createElement('a');
		link.href = window.location.href;
		link.dataset.testRevalidation = 'true';
		link.dataset.sveltekitPreloadData = 'off';
		link.textContent = 'Revisit current page';
		link.style.cssText = 'position:fixed;right:10px;bottom:10px;z-index:9999;background:white';
		document.body.append(link);
	});
	try {
		await page.getByRole('link', { name: 'Revisit current page', exact: true }).click();
	} finally {
		await page.locator('[data-test-revalidation]').evaluateAll((links) => {
			for (const link of links) link.remove();
		});
	}
}

export async function expectSilentBackground(page: Page, region: Locator) {
	await expect(region).toHaveAttribute('aria-busy', 'true');
	await expect(page.getByRole('status').filter({ hasText: /กำลังอัปเดต/ })).toHaveCount(0);
}
