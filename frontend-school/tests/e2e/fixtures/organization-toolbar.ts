import { expect, type Page } from '@playwright/test';

export async function expectCompactOrganizationToolbar(
	page: Page,
	kind: 'organization' | 'learning-area',
	selected = false
) {
	const tabs = page.getByRole('navigation', { name: 'ประเภทโครงสร้าง', exact: true });
	const toolbar = page.getByTestId(`${kind}-toolbar`);
	const create = toolbar.getByRole('button', {
		name: kind === 'organization' ? 'เพิ่มหน่วยงาน' : 'เพิ่มกลุ่มสาระ',
		exact: true
	});
	await expect(create).toBeVisible();
	await expect(page.getByRole('button', { name: /^รีเฟรช(โครงสร้าง|กลุ่มสาระ)$/ })).toHaveCount(0);
	const description = await page.locator('main header p').boundingBox();
	const tabsBox = await tabs.boundingBox();
	const toolbarBox = await toolbar.boundingBox();
	const createBox = await create.boundingBox();
	if (!description || !tabsBox || !toolbarBox || !createBox)
		throw new Error('Expected visible organization controls');
	expect(tabsBox.y - (description.y + description.height)).toBeLessThanOrEqual(32);
	expect(toolbarBox.y - (tabsBox.y + tabsBox.height)).toBeLessThanOrEqual(20);
	expect(createBox.y).toBeGreaterThanOrEqual(tabsBox.y + tabsBox.height);
	if (selected && page.viewportSize()!.width < 1024) {
		const back = toolbar.getByRole('button', { name: /^กลับไปเลือก/ });
		const backBox = await back.boundingBox();
		if (!backBox) throw new Error('Expected visible return control');
		expect(Math.abs(createBox.y - backBox.y)).toBeLessThan(1);
	}
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
}
