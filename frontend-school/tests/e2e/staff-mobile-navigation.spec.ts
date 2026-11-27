import { expect, test, type Page, type Route } from '@playwright/test';
import type { MenuGroup } from '../../src/lib/api/menu.js';
import { homePath, id, mockStaffHome, year } from './fixtures/staff-home-route-data';

test.use({ serviceWorkers: 'block' });

const term = id(80);
const academicOptions = {
	defaultAcademicYearId: year,
	defaultAcademicTermId: term,
	years: [{ id: year, name: '2569', year: 2569, status: 'open', isVisible: true }],
	terms: [
		{
			id: term,
			academicYearId: year,
			name: 'ภาคเรียนที่ 1',
			code: '1',
			sequence: 1,
			termType: 'regular',
			status: 'open',
			isVisible: true,
			schoolDays: ['MON', 'TUE', 'WED', 'THU', 'FRI'],
			includedInYearResult: true,
			blocksYearClosure: true
		}
	]
};
const profileLabel = 'ข้อมูลบุคลากรและการติดต่อของฉัน';
const groups: MenuGroup[] = [
	{
		code: 'personal',
		name: 'บริการของบุคลากร',
		icon: 'User',
		workspaceCode: 'home',
		workspaceName: 'งานของฉัน',
		workspaceIcon: 'Home',
		workspaceOrder: 1,
		displayOrder: 1,
		items: [
			{ id: id(81), name: profileLabel, path: '/staff/profile', icon: 'User' },
			{
				id: id(82),
				name: 'ตารางสอนที่โรงเรียนกำหนด',
				path: '/staff/timetable?view=personal',
				icon: 'CalendarDays'
			}
		].map((item, index) => ({ ...item, code: item.id, displayOrder: index + 1 }))
	}
];

function fulfill(route: Route, data: unknown) {
	return route.fulfill({ json: { success: true, data } });
}

async function mockNavigation(
	page: Page,
	options: { menuGroups?: MenuGroup[]; hold?: string; optionsGate?: Promise<void> } = {}
) {
	const { optionsGate, ...menuOptions } = options;
	const api = await mockStaffHome(page, { permissions: [], menuGroups: groups, ...menuOptions });
	await page.route('**/api/academic/context/options', async (route) => {
		await optionsGate;
		return fulfill(route, academicOptions);
	});
	await page.route('**/api/me/timetable**', (route) => fulfill(route, []));
	await page.route('**/api/me/work-items', (route) => fulfill(route, { items: [] }));
	return api;
}

for (const [width, theme] of [
	[320, 'light'],
	[390, 'dark'],
	[768, 'light'],
	[1023, 'dark']
] as const) {
	test(`${width}px ${theme}: staff tabs resolve context and reserve their own space`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 844 });
		await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
		const api = await mockNavigation(page);
		await page.goto(homePath());
		const navigation = page.getByRole('navigation', { name: 'เมนูหลักบุคลากร', exact: true });
		await expect(navigation).toBeVisible();
		await expect(page.getByRole('button', { name: 'Open Menu', exact: true })).toHaveCount(0);
		const home = navigation.getByRole('link', { name: 'หน้าหลัก', exact: true });
		const work = navigation.getByRole('link', { name: 'งานของฉัน', exact: true });
		const timetable = navigation.getByRole('link', { name: 'ตารางสอน', exact: true });
		await expect(home).toHaveAttribute('href', homePath());
		await expect(home).toHaveAttribute('aria-current', 'page');
		await expect(work).toHaveAttribute('href', '/staff/work');
		await expect(timetable).toHaveAttribute(
			'href',
			`/staff/timetable?view=personal&academicYearId=${year}&academicTermId=${term}`
		);
		const services = navigation.getByRole('button', { name: 'บริการ', exact: true });
		await expect(services).toHaveAttribute('aria-expanded', 'false');
		await expect(services).not.toHaveAttribute('aria-current');
		await expect(page.locator('aside')).toHaveAttribute('inert');
		for (const control of [home, work, timetable, services]) {
			const box = await control.boundingBox();
			expect(box!.width).toBeGreaterThanOrEqual(44);
			expect(box!.height).toBeGreaterThanOrEqual(44);
		}
		const main = await page.locator('main').boundingBox();
		const bar = await navigation.boundingBox();
		expect(main!.y + main!.height).toBeLessThanOrEqual(bar!.y + 1);
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
		await timetable.click();
		await expect(timetable).toHaveAttribute('aria-current', 'page');
		await expect(home).not.toHaveAttribute('aria-current');
		await expect(page.locator('main h1')).toHaveText('ตารางสอน');
		await work.click();
		await expect(work).toHaveAttribute('aria-current', 'page');
		await expect(home).toHaveAttribute('href', homePath());
		await page.screenshot({ path: testInfo.outputPath('staff-mobile-navigation.png') });
		expect(api.count('/api/menu/user')).toBe(1);
	});
}

test('Services uses the shared filtered menu and traps focus after a collapsed desktop preference', async ({
	page
}) => {
	await page.setViewportSize({ width: 1440, height: 900 });
	const api = await mockNavigation(page);
	await page.goto('/staff/work');
	await page.getByRole('button', { name: 'Collapse sidebar', exact: true }).click();
	await page.setViewportSize({ width: 320, height: 844 });
	const navigation = page.getByRole('navigation', { name: 'เมนูหลักบุคลากร', exact: true });
	const services = navigation.getByRole('button', { name: 'บริการ', exact: true });
	await services.focus();
	await page.keyboard.press('Enter');
	const drawer = page.getByRole('dialog', { name: 'เมนูบริการ', exact: true });
	await expect(drawer).toBeVisible();
	const close = drawer.getByRole('button', { name: 'ปิดเมนูบริการ', exact: true });
	await expect(close).toBeFocused();
	const profile = drawer.getByRole('link', { name: profileLabel, exact: true });
	await expect(profile).toBeVisible();
	await expect(profile).toHaveAttribute('href', '/staff/profile');
	await expect(drawer.getByRole('link', { name: 'จัดการสิทธิ์', exact: true })).toHaveCount(0);
	await page.keyboard.press('Shift+Tab');
	await expect(
		drawer.getByRole('link', { name: 'ตารางสอนที่โรงเรียนกำหนด', exact: true })
	).toBeFocused();
	await page.keyboard.press('Tab');
	await expect(close).toBeFocused();
	await page.keyboard.press('Escape');
	await expect(drawer).toHaveCount(0);
	await expect(services).toBeFocused();
	await expect(services).toHaveAttribute('aria-expanded', 'false');
	await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).focus();
	await services.dispatchEvent('click');
	await expect(close).toBeFocused();
	await page.keyboard.press('Escape');
	await expect(services).toBeFocused();
	await services.click();
	await profile.click();
	await expect(page).toHaveURL(/\/staff\/profile$/);
	await expect(services).toHaveClass(/font-semibold/);
	await expect(services).not.toHaveAttribute('aria-current');
	await services.click();
	await page.setViewportSize({ width: 1440, height: 900 });
	await expect(page.getByRole('dialog', { name: 'เมนูบริการ', exact: true })).toHaveCount(0);
	await expect(navigation).not.toBeVisible();
	await expect(page.getByRole('button', { name: 'Expand sidebar', exact: true })).toBeVisible();
	await expect(page.locator('main').locator('xpath=..')).not.toHaveAttribute('inert');
	await page.setViewportSize({ width: 320, height: 844 });
	await expect(services).toHaveAttribute('aria-expanded', 'false');
	await expect(page.locator('aside')).toHaveAttribute('inert');
	expect(api.count('/api/menu/user')).toBe(1);
});

test('the drawer follows the CSS lg breakpoint with a larger browser default font', async ({
	page
}) => {
	const cdp = await page.context().newCDPSession(page);
	await cdp.send('Page.setFontSizes', { fontSizes: { standard: 20 } });
	await page.setViewportSize({ width: 1200, height: 900 });
	await mockNavigation(page);
	await page.goto('/staff/work');
	expect(await page.evaluate(() => matchMedia('(min-width: 64rem)').matches)).toBe(false);
	const navigation = page.getByRole('navigation', { name: 'เมนูหลักบุคลากร', exact: true });
	await expect(navigation).toBeVisible();
	await expect(page.locator('aside')).toHaveAttribute('inert');
	await navigation.getByRole('button', { name: 'บริการ', exact: true }).click();
	const drawer = page.getByRole('dialog', { name: 'เมนูบริการ', exact: true });
	await expect(drawer).toBeVisible();
	await expect(drawer.getByRole('button', { name: 'ปิดเมนูบริการ', exact: true })).toBeFocused();
	await page.setViewportSize({ width: 1280, height: 900 });
	await expect(navigation).not.toBeVisible();
	await expect(drawer).toHaveCount(0);
	await expect(page.locator('aside')).not.toHaveAttribute('inert');
	await expect(page.getByRole('button', { name: 'Collapse sidebar', exact: true })).toBeVisible();
	await cdp.detach();
});

test('ready menu waits for academic options without exposing incomplete links or preloading them', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	let release = () => {};
	const optionsGate = new Promise<void>((resolve) => (release = resolve));
	const destinationReads: string[] = [];
	page.on('request', (request) => {
		const path = new URL(request.url()).pathname;
		if (path === '/api/staff/dashboard' || path === '/api/me/timetable')
			destinationReads.push(path);
	});
	const api = await mockNavigation(page, { optionsGate });
	try {
		await page.goto('/staff/work');
		await expect.poll(() => api.count('/api/menu/user')).toBe(1);
		const navigation = page.getByRole('navigation', { name: 'เมนูหลักบุคลากร', exact: true });
		const home = navigation.getByRole('link', { name: 'หน้าหลัก', exact: true });
		const timetable = navigation.getByRole('link', { name: 'ตารางสอน', exact: true });
		for (const link of [home, timetable]) {
			await expect(link).toHaveAttribute('aria-disabled', 'true');
			await expect(link).not.toHaveAttribute('href');
			await link.dispatchEvent('pointerenter');
			await link.dispatchEvent('pointerdown');
		}
		await expect(navigation.getByRole('link', { name: 'งานของฉัน', exact: true })).toBeEnabled();
		await navigation.getByRole('button', { name: 'บริการ', exact: true }).click();
		await expect(
			page.getByRole('dialog').getByRole('link', { name: profileLabel, exact: true })
		).toBeVisible();
		await page.keyboard.press('Escape');
		expect(destinationReads).toEqual([]);
		release();
		await expect(home).toHaveAttribute('href', homePath());
		await expect(timetable).toHaveAttribute(
			'href',
			`/staff/timetable?view=personal&academicYearId=${year}&academicTermId=${term}`
		);
		expect(api.count('/api/menu/user')).toBe(1);
	} finally {
		release();
	}
});

test('opening Services retries unavailable academic options through the existing shared owner', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await mockNavigation(page);
	let optionReads = 0;
	await page.route('**/api/academic/context/options', (route) => {
		optionReads++;
		return optionReads === 1
			? route.fulfill({ status: 503, json: { success: false, error: 'context unavailable' } })
			: fulfill(route, academicOptions);
	});
	await page.goto('/staff/work');
	await expect.poll(() => optionReads).toBe(1);
	const navigation = page.getByRole('navigation', { name: 'เมนูหลักบุคลากร', exact: true });
	const home = navigation.getByRole('link', { name: 'หน้าหลัก', exact: true });
	await expect(home).not.toHaveAttribute('href');
	await navigation.getByRole('button', { name: 'บริการ', exact: true }).click();
	await expect.poll(() => optionReads).toBe(2);
	await page.keyboard.press('Escape');
	await expect(home).toHaveAttribute('href', homePath());
});

test('timetable waits for the existing menu and disappears when the school excludes it', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	const api = await mockNavigation(page, { hold: 'menu', menuGroups: [] });
	try {
		await page.goto(homePath());
		const navigation = page.getByRole('navigation', { name: 'เมนูหลักบุคลากร', exact: true });
		const timetable = navigation.getByRole('link', { name: 'ตารางสอน', exact: true });
		await expect(timetable).toHaveAttribute('aria-disabled', 'true');
		await expect(timetable).not.toHaveAttribute('href');
		await expect(navigation.getByRole('link', { name: 'หน้าหลัก', exact: true })).toBeEnabled();
		api.release();
		await expect(timetable).toHaveCount(0);
		await expect(navigation.getByRole('button', { name: 'บริการ', exact: true })).toBeEnabled();
		expect(api.count('/api/menu/user')).toBe(1);
	} finally {
		api.release();
	}
});

for (const userType of ['student', 'parent']) {
	test(`${userType} keeps the mobile header menu without staff tabs`, async ({ page }) => {
		await page.setViewportSize({ width: 390, height: 844 });
		await mockStaffHome(page, { userType, permissions: [], menuGroups: [] });
		await page.goto(`/${userType}`);
		await expect(page.getByRole('button', { name: 'Open Menu', exact: true })).toBeVisible();
		await expect(
			page.getByRole('navigation', { name: 'เมนูหลักบุคลากร', exact: true })
		).toHaveCount(0);
		await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).focus();
		await page.getByRole('button', { name: 'Open Menu', exact: true }).dispatchEvent('click');
		await expect(page.getByRole('dialog', { name: 'เมนูบริการ', exact: true })).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(page.getByRole('button', { name: 'Open Menu', exact: true })).toBeFocused();
	});
}
