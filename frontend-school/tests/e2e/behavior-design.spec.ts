import { expect, test } from '@playwright/test';
import { PERMISSIONS } from '../../src/lib/permissions/registry';
import {
	mockBehavior,
	behaviorPath,
	behaviorStudent,
	behaviorTerm
} from './fixtures/behavior-route-data';

const screens = [
	{ name: 'overview', path: behaviorPath(), title: 'พฤติกรรมนักเรียน' },
	{ name: 'record', path: behaviorPath('/new'), title: 'บันทึกพฤติกรรม' },
	{
		name: 'summary',
		path: `${behaviorPath()}&studentId=${behaviorStudent}`,
		title: 'พฤติกรรมนักเรียน'
	},
	{ name: 'catalog', path: behaviorPath('/settings/items'), title: 'รายการพฤติกรรม' },
	{ name: 'criteria', path: behaviorPath('/settings/attendance'), title: 'เกณฑ์คะแนนจากเช็กชื่อ' },
	{ name: 'policy', path: behaviorPath('/settings'), title: 'นโยบายคะแนนและรอบรีเซ็ต' },
	{ name: 'cleanup', path: behaviorPath('/cleanup'), title: 'รีเซ็ตและล้างข้อมูลคะแนน' },
	{
		name: 'student',
		path: `/student/behavior?academicTermId=${behaviorTerm}`,
		title: 'พฤติกรรมและคะแนน',
		role: 'student' as const
	},
	{
		name: 'parent',
		path: `/parent/behavior?academicTermId=${behaviorTerm}&studentId=${behaviorStudent}`,
		title: 'พฤติกรรมและคะแนน',
		role: 'parent' as const
	}
];

for (const width of [375, 1440]) {
	test.describe(`behavior layout at ${width}px`, () => {
		test.use({ hasTouch: width === 375 });
		for (const theme of ['light', 'dark'] as const) {
			for (const screen of screens) {
				test(`${screen.name} keeps its work area readable at ${width}px in ${theme}`, async ({
					page
				}) => {
					await page.setViewportSize({ width, height: 900 });
					await page.emulateMedia({ colorScheme: theme });
					await page.addInitScript((value) => localStorage.setItem('theme', value), theme);
					await mockBehavior(page, {
						userType: screen.role,
						...(screen.role ? { permissions: [PERMISSIONS.BEHAVIOR_READ_OWN] } : {})
					});
					await page.goto(screen.path);
					await expect(
						page.getByRole('heading', { name: screen.title, level: 1, exact: true })
					).toBeVisible();
					const content =
						screen.name === 'record'
							? page.getByRole('heading', { name: 'นักเรียน', exact: true })
							: screen.name === 'cleanup'
								? page.getByRole('heading', { name: 'ขอบเขตข้อมูล', exact: true })
								: screen.name === 'policy'
									? page.getByRole('heading', { name: 'การเชื่อมคะแนนและรอบรีเซ็ต', exact: true })
									: screen.name === 'catalog'
										? page.getByRole('button', { name: 'บันทึกรายการ', exact: true })
										: screen.name === 'criteria'
											? page.getByRole('button', { name: 'บันทึกเกณฑ์', exact: true })
											: page.getByRole('heading', { name: 'ประวัติรายการคะแนน', exact: true });
					await expect(content).toBeVisible();
					expect(
						await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)
					).toBe(true);
					await expect
						.poll(() => page.locator('html').evaluate((node) => node.classList.contains('dark')))
						.toBe(theme === 'dark');
					await page.screenshot({
						path: `/tmp/behavior-design-${screen.name}-${width}-${theme}.png`,
						fullPage: true
					});
					await content.scrollIntoViewIfNeeded();
					await page.screenshot({
						path: `/tmp/behavior-design-${screen.name}-${width}-${theme}-work.png`,
						fullPage: true
					});
				});
			}
		}
	});
}

test('settings navigation loads only the selected section and retains term context', async ({
	page
}) => {
	const api = await mockBehavior(page);
	await page.goto(behaviorPath('/settings/items'));
	await expect(page.getByRole('button', { name: 'บันทึกรายการ', exact: true })).toBeVisible();
	expect(api.reads).toContain('/api/behavior/items');
	expect(api.reads).not.toContain('/api/behavior/settings');
	expect(api.reads).not.toContain('/api/behavior/rules');
	expect(api.reads).not.toContain('/api/behavior/options');
	api.reads.length = 0;
	await page.getByRole('link', { name: 'คะแนนจากเช็กชื่อ', exact: true }).click();
	await expect(page.getByRole('button', { name: 'บันทึกเกณฑ์', exact: true })).toBeVisible();
	await expect(page).toHaveURL(new RegExp(`/settings/attendance[?]academicTermId=${behaviorTerm}`));
	expect(api.reads).toContain('/api/behavior/rules');
	expect(api.reads).toContain('/api/behavior/options');
	expect(api.reads).not.toContain('/api/behavior/settings');
	expect(api.reads).not.toContain('/api/behavior/items');
	api.reads.length = 0;
	await page.getByRole('link', { name: 'นโยบายและรอบรีเซ็ต', exact: true }).click();
	await expect(page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true })).toBeVisible();
	expect(api.reads).toContain('/api/behavior/settings');
	expect(api.reads).not.toContain('/api/behavior/rules');
	expect(api.reads).not.toContain('/api/behavior/items');
});

test('cleanup choices retain one selection on repeated activation', async ({ page }) => {
	await mockBehavior(page);
	await page.goto(behaviorPath('/cleanup'));
	const method = page.getByRole('radio', { name: 'ล้างรายการ', exact: true });
	await method.click();
	await method.click();
	await expect(method).toBeChecked();
	const archive = page.getByRole('radio', {
		name: 'ย้ายคะแนนไปประวัติและล้างรายละเอียด',
		exact: true
	});
	await archive.click();
	await archive.click();
	await expect(archive).toBeChecked();
});

for (const section of ['/settings/items', '/settings/attendance']) {
	test(`reader cannot open management section ${section}`, async ({ page }) => {
		const api = await mockBehavior(page, { permissions: [PERMISSIONS.BEHAVIOR_READ_SCHOOL] });
		await page.goto(behaviorPath(section));
		await expect(page).toHaveURL(/\/403(?:[?]|$)/);
		expect(api.reads).not.toContain('/api/behavior/items');
		expect(api.reads).not.toContain('/api/behavior/rules');
		expect(api.reads).not.toContain('/api/behavior/options');
	});
}

for (const width of [375, 1440]) {
	for (const hasTouch of [false, true]) {
		test.describe(`control sizing at ${width}px with ${hasTouch ? 'touch' : 'mouse'}`, () => {
			test.use({ viewport: { width, height: 900 }, hasTouch });
			for (const theme of ['light', 'dark'] as const) {
				test(`normal sizing and calendar targets work in ${theme}`, async ({ page }) => {
					await page.emulateMedia({ colorScheme: theme });
					const api = await mockBehavior(page);
					await page.goto(behaviorPath('/new'));
					expect(await page.evaluate(() => matchMedia('(any-pointer: coarse)').matches)).toBe(
						hasTouch
					);
					const choice = page.getByRole('radio', { name: 'ความดี', exact: true });
					const save = page.getByRole('button', { name: 'บันทึกพฤติกรรม', exact: true });
					const select = page.getByRole('button', { name: 'รายการพฤติกรรม', exact: true });
					const now = page.getByRole('button', { name: 'ใช้วันเวลาปัจจุบัน', exact: true });
					await expect(save).toBeVisible();
					for (const control of [choice, save, select, now]) {
						const box = (await control.boundingBox())!;
						if (hasTouch) {
							expect(box.height).toBeGreaterThanOrEqual(44);
							expect(box.width).toBeGreaterThanOrEqual(44);
						} else {
							expect(box.height).toBeLessThan(44);
							expect(box.height).toBeGreaterThanOrEqual(24);
						}
					}
					if (hasTouch) {
						await choice.tap();
						await choice.tap();
						await select.tap();
					} else {
						await choice.click();
						await choice.click();
						await select.click();
					}
					await expect(choice).toBeChecked();
					const option = page.getByRole('option', {
						name: 'ช่วยงานโรงเรียน · 5 คะแนน',
						exact: true
					});
					await expect(option).toBeVisible();
					await page.locator('[data-slot=select-content]').evaluate(async (element) => {
						await Promise.all(
							element.getAnimations({ subtree: true }).map((animation) => animation.finished)
						);
					});
					const optionBox = (await option.boundingBox())!;
					if (hasTouch) {
						expect(optionBox.height).toBeGreaterThanOrEqual(44);
						await option.tap();
					} else {
						expect(optionBox.height).toBeLessThan(44);
						await option.click();
					}
					const date = page.getByRole('button', { name: 'เลือกวันที่', exact: true });
					if (hasTouch) await date.tap();
					else await date.click();
					const cells = page.locator('[data-bits-day]');
					await expect(cells.first()).toBeVisible();
					await page.locator('[data-slot=popover-content]').evaluate(async (element) => {
						await Promise.all(
							element.getAnimations({ subtree: true }).map((animation) => animation.finished)
						);
					});
					const boxes = await cells.evaluateAll((nodes) =>
						nodes.map((node) => {
							const rect = node.getBoundingClientRect();
							return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
						})
					);
					for (const box of boxes) {
						if (hasTouch) {
							expect(box.width).toBeGreaterThanOrEqual(44);
							expect(box.height).toBeGreaterThanOrEqual(44);
						} else expect(box.height).toBeLessThan(44);
						expect(box.x).toBeGreaterThanOrEqual(0);
						expect(box.x + box.width).toBeLessThanOrEqual(width);
					}
					for (let i = 0; i < boxes.length; i++) {
						for (let j = i + 1; j < boxes.length; j++) {
							const a = boxes[i],
								b = boxes[j];
							const overlaps =
								Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x) > 0.5 &&
								Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y) > 0.5;
							expect(overlaps).toBe(false);
						}
					}
					await page.keyboard.press('Escape');
					expect(api.batches).toHaveLength(0);
					await page.screenshot({
						path: `/tmp/behavior-density-${width}-${hasTouch ? 'touch' : 'mouse'}-${theme}.png`,
						fullPage: true
					});
					if (hasTouch) {
						await page.goto(behaviorPath());
						const selectedEntry = page.getByRole('checkbox', {
							name: 'เลือกยกเลิก นักเรียน หนึ่ง ช่วยงานโรงเรียน',
							exact: true
						});
						const target = selectedEntry.locator('..');
						await expect(target).toBeVisible();
						const box = (await target.boundingBox())!;
						expect(box.width).toBeGreaterThanOrEqual(44);
						expect(box.height).toBeGreaterThanOrEqual(44);
						await target.tap();
						await expect(selectedEntry).toBeChecked();
						expect(api.cancellations).toHaveLength(0);
					}
				});
			}
		});
	}
}
