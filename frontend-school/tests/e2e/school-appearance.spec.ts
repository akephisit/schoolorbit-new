import { expect, test, type Page } from '@playwright/test';
import { startSchoolAppearanceServer } from './fixtures/school-appearance-server';
import { mockStaffHome } from './fixtures/staff-home-route-data';
import { navigate } from './fixtures/supervision-route-data';
import type { SchoolAppearanceResponse } from '../../src/lib/appearance/model.js';

test.describe.configure({ mode: 'serial' });
test.use({ serviceWorkers: 'block' });
let fixture: Awaited<ReturnType<typeof startSchoolAppearanceServer>>;
test.beforeAll(async () => {
	fixture = await startSchoolAppearanceServer();
});
test.afterAll(async () => {
	await fixture?.close();
});
test.beforeEach(() => fixture.reset());
const privatePath = '/api/school/settings/appearance';

async function mockSession(
	page: Page,
	permissions = ['settings.read.all', 'settings.update.all'],
	role = 'staff'
) {
	await mockStaffHome(page, { permissions, userType: role });
	await page.route(
		(url) => url.pathname === '/api/school/public/appearance',
		(route) => route.continue()
	);
	const writes: unknown[] = [];
	let conflict = false;
	await page.route(
		(url) => url.pathname === privatePath,
		async (route) => {
			if (route.request().method() === 'GET')
				return route.fulfill({ json: { success: true, data: fixture.saved } });
			const input = route.request().postDataJSON();
			writes.push(input);
			if (conflict || input.expectedRevision !== fixture.saved.revision)
				return route.fulfill({ status: 409, json: { success: false, error: 'conflict' } });
			const result: SchoolAppearanceResponse = {
				appearance: input.appearance,
				revision: fixture.saved.revision + 1
			};
			fixture.set(result);
			return route.fulfill({ json: { success: true, data: result } });
		}
	);
	return {
		writes,
		conflict: () => {
			conflict = true;
		}
	};
}
async function ready(page: Page) {
	await expect(page.locator('body')).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => getComputedStyle(document.body).visibility))
		.toBe('visible');
}
async function capturePaints(page: Page, theme = 'light') {
	await page.addInitScript((preference) => {
		localStorage.setItem('ui-preferences', JSON.stringify({ theme: preference }));
		const frames: { palette: string; font: string; dark: boolean; loaded: boolean }[] = [];
		Object.assign(window, { appearancePaints: frames });
		function sample() {
			if (document.body && getComputedStyle(document.body).visibility === 'visible') {
				const css = getComputedStyle(document.documentElement);
				frames.push({
					palette: css.getPropertyValue('--school-appearance-palette').trim(),
					font: getComputedStyle(document.body).fontFamily,
					dark: document.documentElement.classList.contains('dark'),
					loaded: document.fonts.check('600 16px "School Noto Sans Thai"', 'ก SchoolOrbit')
				});
			}
			if (frames.length < 500) requestAnimationFrame(sample);
		}
		requestAnimationFrame(sample);
	}, theme);
}
async function paints(page: Page) {
	return page.evaluate(
		() =>
			(
				window as unknown as {
					appearancePaints: { palette: string; font: string; dark: boolean; loaded: boolean }[];
				}
			).appearancePaints
	);
}

test('delayed tenant CSS and fonts never expose default colors or wrong fonts on first paint', async ({
	page
}) => {
	fixture.delay(500);
	await capturePaints(page);
	await page.route('**/fonts/ui/noto-*.woff2', async (route) => {
		await new Promise((done) => setTimeout(done, 700));
		await route.continue();
	});
	await page.goto(`${fixture.baseUrl}/login`);
	await ready(page);
	await expect.poll(async () => (await paints(page)).length).toBeGreaterThan(0);
	const frames = await paints(page);
	expect(
		frames.every(
			(frame) =>
				frame.palette === 'purple' &&
				frame.font.includes('School Noto Sans Thai') &&
				frame.loaded &&
				!frame.dark
		)
	).toBe(true);
	const reads = fixture.requests.filter(
		(request) => request.path === '/api/school/public/appearance'
	);
	expect(reads).toHaveLength(1);
	expect(reads[0].origin).toBe(fixture.baseUrl);
	expect(reads[0].cookie).toBeUndefined();
	expect(await page.locator('meta[name="theme-color"]').getAttribute('content')).toBe('#f8f5fa');
});

test('font timeout selects a stable system fallback and late fonts never swap the document', async ({
	page
}) => {
	let release = () => {};
	const held = new Promise<void>((done) => {
		release = done;
	});
	await page.route('**/fonts/ui/noto-*.woff2', async (route) => {
		await held;
		await route.continue();
	});
	await page.goto(`${fixture.baseUrl}/login`, { waitUntil: 'domcontentloaded' });
	await ready(page);
	await expect(page.locator('html')).toHaveAttribute('data-school-font-fallback', 'true');
	const before = await page.evaluate(() => getComputedStyle(document.body).fontFamily);
	expect(before).toContain('system-ui');
	release();
	await page.evaluate(async () => {
		await document.fonts.ready;
		await new Promise(requestAnimationFrame);
	});
	expect(await page.evaluate(() => getComputedStyle(document.body).fontFamily)).toBe(before);
});

test('a blocked bootstrap fails open within four seconds with fixed system typography', async ({
	page
}) => {
	await page.route('**/school-appearance-bootstrap.js', (route) => route.abort());
	await page.goto(`${fixture.baseUrl}/login`, { waitUntil: 'domcontentloaded' });
	await ready(page);
	expect(await page.evaluate(() => getComputedStyle(document.body).fontFamily)).toContain(
		'system-ui'
	);
	await page.evaluate(async () => {
		await document.fonts.ready;
	});
	expect(await page.evaluate(() => getComputedStyle(document.body).fontFamily)).toContain(
		'system-ui'
	);
});

for (const role of ['staff', 'student', 'parent']) {
	test(`${role} initial personal dark mode and school appearance are ready before loaders paint`, async ({
		page
	}) => {
		await mockSession(page, [], role);
		await capturePaints(page, 'dark');
		await page.goto(`${fixture.baseUrl}/${role}`);
		await ready(page);
		await expect.poll(async () => (await paints(page)).length).toBeGreaterThan(0);
		const frames = await paints(page);
		expect(
			frames.every(
				(frame) =>
					frame.dark && frame.palette === 'purple' && frame.font.includes('School Noto Sans Thai')
			)
		).toBe(true);
		expect(
			fixture.requests.filter((request) => request.path === '/api/school/public/appearance')
		).toHaveLength(1);
	});
}

test('system dark preference applies initially and public-to-app SPA navigation performs no extra appearance read', async ({
	page
}) => {
	await mockSession(page);
	await page.emulateMedia({ colorScheme: 'dark' });
	await capturePaints(page, 'system');
	await page.goto(`${fixture.baseUrl}/login`);
	await ready(page);
	expect(await page.locator('html').getAttribute('class')).not.toContain('dark');
	await navigate(page, '/staff/profile');
	await expect(page.locator('html')).toHaveClass(/dark/);
	await expect(page.locator('#nickname')).toBeVisible();
	expect(
		fixture.requests.filter((request) => request.path === '/api/school/public/appearance')
	).toHaveLength(1);
});

test('real settings save applies atomically and a fresh browser receives persisted school appearance', async ({
	page,
	browser
}) => {
	const api = await mockSession(page);
	await page.goto(`${fixture.baseUrl}/staff/school-settings/appearance`);
	await ready(page);
	await expect(page.getByTestId('school-appearance-settings')).toBeVisible();
	await expect(page.getByRole('heading', { level: 1 })).toHaveCount(1);
	await expect(page.locator('title')).toHaveCount(1);
	await page.getByLabel('ฟอนต์ของโรงเรียน', { exact: true }).click();
	await page.getByRole('option', { name: 'Sarabun', exact: true }).click();
	await page.getByLabel('ปรับสีหลักเพิ่มเติม').fill('#245fc2');
	await page.getByRole('button', { name: 'บันทึกรูปแบบโรงเรียน', exact: true }).click();
	await expect(page.getByText('บันทึกรูปแบบของทั้งโรงเรียนแล้ว', { exact: true })).toBeVisible();
	expect(api.writes).toHaveLength(1);
	expect(api.writes[0]).toMatchObject({
		expectedRevision: 8,
		appearance: { font: 'sarabun', primary: '#245fc2' }
	});
	expect(
		await page.evaluate(() =>
			getComputedStyle(document.documentElement).getPropertyValue('--school-appearance-font').trim()
		)
	).toBe('sarabun');
	const fresh = await browser.newContext({ serviceWorkers: 'block' });
	const next = await fresh.newPage();
	await next.goto(`${fixture.baseUrl}/login`);
	await ready(next);
	expect(
		await next.evaluate(() => localStorage.getItem('schoolorbit:design-preview:appearance:v1'))
	).toBeNull();
	expect(await next.evaluate(() => getComputedStyle(document.body).fontFamily)).toContain(
		'School Sarabun'
	);
	await fresh.close();
});

test('conflict preserves the draft until explicit reload and read-only users cannot save', async ({
	page
}) => {
	const api = await mockSession(page);
	api.conflict();
	await page.goto(`${fixture.baseUrl}/staff/school-settings/appearance`);
	await page.getByLabel('ปรับสีหลักเพิ่มเติม').fill('#112233');
	await page.getByRole('button', { name: 'บันทึกรูปแบบโรงเรียน', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('แบบร่างของคุณยังอยู่');
	await expect(page.getByLabel('ปรับสีหลักเพิ่มเติม')).toHaveValue('#112233');
	await expect(
		page.getByRole('button', { name: 'บันทึกรูปแบบโรงเรียน', exact: true })
	).toBeDisabled();
	await page.getByRole('button', { name: /โหลดค่าล่าสุดเพื่อตรวจทาน/ }).click();
	await expect(page.getByLabel('ปรับสีหลักเพิ่มเติม')).toHaveValue('#7850a5');
	await mockSession(page, ['settings.read.all']);
	await page.reload();
	await expect(page.getByText('คุณดูรูปแบบได้ แต่ไม่มีสิทธิ์บันทึกการเปลี่ยนแปลง')).toBeVisible();
	await expect(page.getByRole('button', { name: 'บันทึกรูปแบบโรงเรียน', exact: true })).toHaveCount(
		0
	);
});

test('denied settings read never starts its private request', async ({ page }) => {
	await mockSession(page, []);
	let reads = 0;
	page.on('request', (request) => {
		if (new URL(request.url()).pathname === privatePath) reads++;
	});
	await page.goto(`${fixture.baseUrl}/staff/school-settings/appearance`);
	await ready(page);
	await expect(page.getByRole('button', { name: 'บันทึกรูปแบบโรงเรียน', exact: true })).toHaveCount(
		0
	);
	expect(reads).toBe(0);
});
