import { expect, test, type Locator, type Page, type TestInfo } from '@playwright/test';
import { ids } from './helpers/curriculum-fixtures';
import {
	curriculumCatalogFixture,
	curriculumCatalogPath,
	installCurriculumCatalog
} from './fixtures/curriculum-catalog';

test.use({ serviceWorkers: 'block', timezoneId: 'Asia/Bangkok' });

const matrix = (page: Page) => page.getByTestId('curriculum-matrix');
const target = (page: Page) =>
	matrix(page).locator(`[data-requirement-id="${ids.requirement}"]`).first();
const addPanel = (page: Page) => page.getByRole('dialog', { name: 'เพิ่มรายวิชา', exact: true });

async function locks(page: Page) {
	return page.evaluate(() => {
		const body = getComputedStyle(document.body);
		return {
			pointerEvents: body.pointerEvents,
			overflow: body.overflow,
			overflowX: body.overflowX,
			overflowY: body.overflowY,
			inert: document.querySelectorAll('[inert]').length,
			hidden: document.querySelectorAll('[aria-hidden="true"]').length,
			overlays: document.querySelectorAll(
				'[data-slot="sheet-overlay"], [data-slot="dialog-overlay"], [data-slot="alert-dialog-overlay"]'
			).length
		};
	});
}

async function openAction(page: Page, action = 'เพิ่มรายวิชา…') {
	await target(page).click({ button: 'right' });
	await page.getByRole('menuitem', { name: action, exact: true }).click();
}

async function recovered(
	page: Page,
	baseline: Awaited<ReturnType<typeof locks>>,
	testInfo: TestInfo,
	label: string
) {
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
	const tab = page.getByRole('tab', { name: 'เอกสารรายแผน', exact: true });
	await tab.scrollIntoViewIfNeeded();
	const hit = await tab.evaluate((element) => {
		const rect = element.getBoundingClientRect();
		const top = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2);
		return {
			bounds: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
			isTarget: top === element || !!top && element.contains(top),
			top: top?.outerHTML.slice(0, 500),
			active: document.activeElement?.outerHTML.slice(0, 500),
			bodyStyle: document.body.getAttribute('style'),
			menus: [...document.querySelectorAll('[role="menu"]')].map((menu) => ({
				state: menu.getAttribute('data-state'),
				html: menu.outerHTML.slice(0, 700)
			}))
		};
	});
	await testInfo.attach(`${label}-recovery`, {
		body: JSON.stringify({ baseline, after: await locks(page), hit }, null, 2),
		contentType: 'application/json'
	});
	await expect.poll(() => locks(page), `${label} must restore background locks`).toEqual(baseline);
	await expect(page.getByRole('menu')).toHaveCount(0);
	expect(hit.isTarget, `${label} must expose the background tab to hit testing`).toBe(true);
	await tab.click();
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	await tab.press('ArrowLeft');
	await expect(page.getByRole('tab', { name: 'เทียบทุกแผน', exact: true })).toBeFocused();
	await page.keyboard.press('Enter');
	await expect(page.getByRole('tab', { name: 'เทียบทุกแผน', exact: true })).toHaveAttribute(
		'aria-selected',
		'true'
	);
}

test('clean add cancellation restores background interaction and body locks', async ({ page }, testInfo) => {
	const fixture = curriculumCatalogFixture();
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await expect(target(page)).toBeVisible();
	const baseline = await locks(page);
	await openAction(page);
	await expect(addPanel(page)).toBeVisible();
	await addPanel(page).getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await recovered(page, baseline, testInfo, 'cancel-add');
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
	expect(state.unexpected).toEqual([]);
});
