import { expect, test, type Locator, type Page, type TestInfo } from '@playwright/test';
import {
	curriculumCatalogFixture,
	curriculumCatalogPath,
	installCurriculumCatalog
} from './fixtures/curriculum-catalog';

test.use({ serviceWorkers: 'block', timezoneId: 'Asia/Bangkok' });

const matrix = (page: Page) => page.getByTestId('curriculum-matrix');
const target = (page: Page) => matrix(page).locator('[data-requirement-id]').first();
const addPanel = (page: Page) => page.getByRole('dialog', { name: 'เพิ่มรายวิชา', exact: true });
const editPanel = (page: Page) => page.getByRole('dialog', { name: 'แก้ไขเงื่อนไข', exact: true });
const removePanel = (page: Page) =>
	page.getByRole('alertdialog', { name: 'นำรายการออก', exact: true });
const discardPanel = (page: Page) =>
	page.getByRole('alertdialog', { name: 'ละทิ้งการเปลี่ยนแปลง?', exact: true });
type Entry = 'mouse' | 'keyboard' | 'touch';

function scrollableFixture() {
	const fixture = curriculumCatalogFixture({ longCatalog: true });
	fixture.workspace.requirements.push(
		...Array.from({ length: 28 }, (_, index) => ({
			...fixture.original,
			id: `scroll-requirement-${index}`,
			catalogId: `scroll-catalog-${index}`,
			catalogVersionId: `scroll-version-${index}`,
			code: `ท${String(index).padStart(3, '0')}`,
			name: `รายวิชาสังเคราะห์สำหรับเลื่อนตาราง ${index}`,
			displayOrder: index + 2,
			hasNewerCurrentVersion: false
		}))
	);
	return fixture;
}

async function locks(page: Page) {
	return page.evaluate(() => {
		const body = getComputedStyle(document.body);
		return {
			pointerEvents: body.pointerEvents,
			overflow: body.overflow,
			overflowX: body.overflowX,
			overflowY: body.overflowY,
			inert: document.querySelectorAll('[inert]').length,
			mainHidden: !!document.querySelector('main')?.closest('[aria-hidden="true"], [inert]'),
			overlays: document.querySelectorAll(
				'[data-slot="sheet-overlay"], [data-slot="dialog-overlay"], [data-slot="alert-dialog-overlay"]'
			).length
		};
	});
}

async function openAction(page: Page, action = 'เพิ่มรายวิชา…', entry: Entry = 'mouse') {
	const cell = target(page);
	if (entry === 'touch') await cell.getByRole('button', { name: 'เปิดเมนูรายการ' }).tap();
	else if (entry === 'keyboard') {
		await cell.focus();
		await cell.press('Shift+F10');
	} else await cell.click({ button: 'right' });
	const item = page.getByRole('menuitem', { name: action, exact: true });
	if (entry === 'keyboard') await item.press('Enter');
	else if (entry === 'touch') await item.tap();
	else await item.click();
	const name = action === 'นำรายการนี้ออก…' ? 'นำรายการออก' : action.slice(0, -1);
	await expect(
		page.getByRole(action === 'นำรายการนี้ออก…' ? 'alertdialog' : 'dialog', {
			name,
			exact: true
		})
	).toBeVisible();
	await expect
		.poll(
			() => page.evaluate(() => getComputedStyle(document.body).pointerEvents),
			'open action must block background pointers'
		)
		.toBe('none');
}

async function closePanel(page: Page, panel: Locator, method: 'cancel' | 'escape' | 'outside') {
	if (method === 'cancel') await panel.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	else if (method === 'escape') await page.keyboard.press('Escape');
	else {
		await page.locator('[data-slot="sheet-overlay"], [data-slot="dialog-overlay"]').click({
			position: { x: 8, y: 80 }
		});
	}
}

async function wheelScroll(page: Page, testInfo: TestInfo, label: string) {
	const table = matrix(page).locator('[data-slot="table-container"]').first();
	const main = page.getByRole('main');
	for (const [name, container] of [
		['matrix', table],
		['page', main]
	] as const) {
		expect(
			await container.evaluate((element) => element.scrollHeight > element.clientHeight),
			`${name} fixture must be naturally scrollable`
		).toBe(true);
		// Reset only the fixture's starting position; movement below must come from a real wheel.
		await container.evaluate((element) => {
			element.scrollTop = 0;
		});
		if (name === 'matrix') await container.scrollIntoViewIfNeeded();
		const bounds = (await container.boundingBox())!;
		const mainBounds = (await main.boundingBox())!;
		await page.mouse.move(
			name === 'page' ? bounds.x + 5 : bounds.x + bounds.width / 2,
			name === 'page'
				? bounds.y + bounds.height / 2
				: (Math.max(bounds.y, mainBounds.y) +
						Math.min(bounds.y + bounds.height, mainBounds.y + mainBounds.height)) /
						2
		);
		await page.mouse.wheel(0, 400);
		await expect
			.poll(() => container.evaluate((element) => element.scrollTop), `${label}: ${name} wheel`)
			.toBeGreaterThan(0);
		await testInfo.attach(`${label}-${name}-wheel`, {
			body: JSON.stringify(
				await container.evaluate((element) => ({
					scrollTop: element.scrollTop,
					scrollHeight: element.scrollHeight,
					clientHeight: element.clientHeight
				}))
			),
			contentType: 'application/json'
		});
		await container.evaluate((element) => {
			element.scrollTop = 0;
		});
	}
}

async function recovered(
	page: Page,
	baseline: Awaited<ReturnType<typeof locks>>,
	testInfo: TestInfo,
	label: string,
	checkWheel = false,
	touch = false
) {
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
	const tab = page.getByRole('tab', { name: 'เอกสารรายแผน', exact: true });
	await tab.scrollIntoViewIfNeeded();
	const hitTest = () =>
		tab.evaluate((element) => {
			const rect = element.getBoundingClientRect();
			const top = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2);
			return {
				bounds: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
				isTarget: top === element || (!!top && element.contains(top)),
				top: top?.outerHTML.slice(0, 500),
				active: document.activeElement?.outerHTML.slice(0, 500),
				bodyStyle: document.body.getAttribute('style'),
				hiddenElements: document.querySelectorAll('[aria-hidden="true"]').length,
				menus: [...document.querySelectorAll('[role="menu"]')].map((menu) => ({
					state: menu.getAttribute('data-state'),
					html: menu.outerHTML.slice(0, 700)
				}))
			};
		});
	await testInfo.attach(`${label}-recovery`, {
		body: JSON.stringify({ baseline, after: await locks(page), hit: await hitTest() }, null, 2),
		contentType: 'application/json'
	});
	await expect.poll(() => locks(page), `${label} must restore background locks`).toEqual(baseline);
	await expect(page.getByRole('menu')).toHaveCount(0);
	await expect(page.getByRole('listbox')).toHaveCount(0);
	expect((await hitTest()).isTarget, `${label} must expose the background tab to hit testing`).toBe(
		true
	);
	if (touch) await tab.tap();
	else await tab.click();
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	await tab.press('ArrowLeft');
	await expect(page.getByRole('tab', { name: 'เทียบทุกแผน', exact: true })).toBeFocused();
	await page.keyboard.press('Enter');
	await expect(page.getByRole('tab', { name: 'เทียบทุกแผน', exact: true })).toHaveAttribute(
		'aria-selected',
		'true'
	);
	const secondTerm = page.getByRole('checkbox', { name: 'ภาคเรียนที่ 2', exact: true });
	await secondTerm.uncheck();
	await expect(matrix(page).locator('thead').first()).not.toContainText('ภาคเรียนที่ 2');
	await secondTerm.check();
	await expect(matrix(page).locator('thead').first()).toContainText('ภาคเรียนที่ 2');
	if (checkWheel) await wheelScroll(page, testInfo, label);
}

test('clean add cancellation restores background interaction and body locks', async ({
	page
}, testInfo) => {
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

test('repeated add cancel, Escape and outside closing restore matrix and page wheel scrolling', async ({
	page
}, testInfo) => {
	test.setTimeout(60_000);
	const fixture = scrollableFixture();
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await expect(target(page)).toBeVisible();
	const baseline = await locks(page);
	await wheelScroll(page, testInfo, 'before-opening');
	for (const [index, method] of ['cancel', 'escape', 'outside', 'cancel'].entries()) {
		await openAction(page, index === 3 ? 'เพิ่มกิจกรรม…' : 'เพิ่มรายวิชา…');
		const panel = page.getByRole('dialog', {
			name: index === 3 ? 'เพิ่มกิจกรรม' : 'เพิ่มรายวิชา',
			exact: true
		});
		await expect(panel).toBeVisible();
		await closePanel(page, panel, method as 'cancel' | 'escape' | 'outside');
		await recovered(page, baseline, testInfo, `add-${index}-${method}`, true);
	}
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
	expect(state.unexpected).toEqual([]);
});

test('nested catalog and condition selections retain dirty drafts, then discard or save without locks', async ({
	page
}, testInfo) => {
	test.setTimeout(60_000);
	const fixture = scrollableFixture();
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await expect(target(page)).toBeVisible();
	const baseline = await locks(page);
	await openAction(page, 'เพิ่มรายวิชา…', 'keyboard');
	const panel = addPanel(page);
	await panel.getByRole('combobox', { name: 'หมวดทะเบียน', exact: true }).click();
	await page.getByRole('option', { name: 'ทุกหมวดรายวิชา', exact: true }).click();
	await panel.getByRole('combobox', { name: 'เงื่อนไข', exact: true }).click();
	await page.getByRole('option', { name: 'เพิ่มเติม', exact: true }).click();
	const choice = panel.getByRole('checkbox', { name: /^เลือก ว20001 / });
	await choice.check();
	await closePanel(page, panel, 'escape');
	await expect(discardPanel(page)).toBeVisible();
	await discardPanel(page).getByRole('button', { name: 'แก้ไขต่อ', exact: true }).click();
	await expect(choice).toBeChecked();
	await expect(panel.getByRole('combobox', { name: 'เงื่อนไข', exact: true })).toHaveText(
		'เพิ่มเติม'
	);
	await panel.getByRole('combobox', { name: 'หมวดทะเบียน', exact: true }).click();
	await page.keyboard.press('Escape');
	await expect(panel).toBeVisible();
	await expect(discardPanel(page)).toHaveCount(0);
	await closePanel(page, panel, 'outside');
	await discardPanel(page)
		.getByRole('button', { name: 'ละทิ้งการเปลี่ยนแปลง', exact: true })
		.click();
	await recovered(page, baseline, testInfo, 'nested-dirty-discard', true);
	expect(state.writes).toEqual([]);
	await openAction(page);
	await addPanel(page)
		.getByRole('checkbox', { name: /^เลือก ว20001 / })
		.check();
	await addPanel(page).getByRole('button', { name: 'เพิ่ม 1 รายการ', exact: true }).click();
	await recovered(page, baseline, testInfo, 'add-save', true);
	expect(state.writes).toHaveLength(1);
	expect(state.errors).toEqual([]);
	expect(state.unexpected).toEqual([]);
});

test('edit cancel, Escape, outside, dirty discard and save restore the same background controls', async ({
	page
}, testInfo) => {
	test.setTimeout(60_000);
	const fixture = scrollableFixture();
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await expect(target(page)).toBeVisible();
	const baseline = await locks(page);
	for (const method of ['cancel', 'escape', 'outside'] as const) {
		await openAction(page, 'แก้ไขเงื่อนไข…');
		await closePanel(page, editPanel(page), method);
		await recovered(page, baseline, testInfo, `edit-${method}`, true);
	}
	await openAction(page, 'แก้ไขเงื่อนไข…');
	await editPanel(page).getByRole('combobox', { name: 'เงื่อนไข', exact: true }).click();
	await page.getByRole('option', { name: 'เลือก', exact: true }).click();
	await closePanel(page, editPanel(page), 'cancel');
	await discardPanel(page).getByRole('button', { name: 'แก้ไขต่อ', exact: true }).click();
	await expect(editPanel(page).getByRole('combobox', { name: 'เงื่อนไข', exact: true })).toHaveText(
		'เลือก'
	);
	await closePanel(page, editPanel(page), 'escape');
	await discardPanel(page)
		.getByRole('button', { name: 'ละทิ้งการเปลี่ยนแปลง', exact: true })
		.click();
	await recovered(page, baseline, testInfo, 'edit-discard', true);
	expect(state.writes).toEqual([]);
	await openAction(page, 'แก้ไขเงื่อนไข…', 'keyboard');
	await editPanel(page).getByRole('combobox', { name: 'เงื่อนไข', exact: true }).click();
	await page.getByRole('option', { name: 'เพิ่มเติม', exact: true }).click();
	await editPanel(page).getByRole('button', { name: 'บันทึก', exact: true }).click();
	await recovered(page, baseline, testInfo, 'edit-save', true);
	expect(state.writes).toHaveLength(1);
	expect(state.errors).toEqual([]);
	expect(state.unexpected).toEqual([]);
});

test('remove cancel, Escape and confirmed removal restore clicks, keyboard access and scrolling', async ({
	page
}, testInfo) => {
	test.setTimeout(60_000);
	const fixture = scrollableFixture();
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await expect(target(page)).toBeVisible();
	const baseline = await locks(page);
	for (const method of ['cancel', 'escape'] as const) {
		await openAction(page, 'นำรายการนี้ออก…');
		await expect(removePanel(page)).toBeVisible();
		await closePanel(page, removePanel(page), method);
		await recovered(page, baseline, testInfo, `remove-${method}`, true);
	}
	expect(state.writes).toEqual([]);
	await openAction(page, 'นำรายการนี้ออก…', 'keyboard');
	await removePanel(page).getByRole('button', { name: 'นำออกจากร่างนี้', exact: true }).click();
	await recovered(page, baseline, testInfo, 'remove-save', true);
	expect(state.writes).toHaveLength(1);
	expect(fixture.workspace.requirements).toHaveLength(28);
	expect(state.errors).toEqual([]);
	expect(state.unexpected).toEqual([]);
});

test.describe('touch overlay recovery', () => {
	test.use({ hasTouch: true, viewport: { width: 390, height: 900 } });
	test('touch menu add and remove recover after repeated cancellation and removal', async ({
		page
	}, testInfo) => {
		test.setTimeout(60_000);
		const fixture = scrollableFixture();
		const state = await installCurriculumCatalog(page, fixture);
		await page.goto(curriculumCatalogPath);
		await expect(target(page)).toBeVisible();
		const baseline = await locks(page);
		for (let index = 0; index < 2; index++) {
			await openAction(page, 'เพิ่มรายวิชา…', 'touch');
			await addPanel(page).getByRole('button', { name: 'ยกเลิก', exact: true }).tap();
			await recovered(page, baseline, testInfo, `touch-add-${index}`, true, true);
			await openAction(page, 'นำรายการนี้ออก…', 'touch');
			await removePanel(page).getByRole('button', { name: 'ยกเลิก', exact: true }).tap();
			await recovered(page, baseline, testInfo, `touch-remove-${index}`, true, true);
		}
		await openAction(page, 'นำรายการนี้ออก…', 'touch');
		await removePanel(page).getByRole('button', { name: 'นำออกจากร่างนี้', exact: true }).tap();
		await recovered(page, baseline, testInfo, 'touch-remove-save', true, true);
		expect(state.writes).toHaveLength(1);
		expect(state.errors).toEqual([]);
		expect(state.unexpected).toEqual([]);
	});
});
