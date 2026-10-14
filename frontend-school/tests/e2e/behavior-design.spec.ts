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
				expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
					true
				);
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
	const method = page.getByRole('radio', { name: /ล้างรายการ ลบรายละเอียดจริง/ });
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
