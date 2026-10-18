import { expect, test, type Page } from '@playwright/test';
import { startSchoolAppearanceServer } from './fixtures/school-appearance-server';
import { mockStaffHome, actor } from './fixtures/staff-home-route-data';
import { mockParent } from './fixtures/parent-route-data';
import { mockStudentSchedules } from './fixtures/student-schedules-route-data';
import { navigate } from './fixtures/supervision-route-data';
import type { CurrentUserDto } from '../../src/lib/api/auth.js';
import { contrastRatio, type SchoolAppearanceResponse } from '../../src/lib/appearance/model.js';

test.describe.configure({ mode: 'default' });
test.use({ serviceWorkers: 'block' });
let fixture: Awaited<ReturnType<typeof startSchoolAppearanceServer>>;
test.beforeAll(async () => {
	fixture = await startSchoolAppearanceServer();
});
test.afterAll(async () => {
	await fixture?.close();
});
const pageErrors = new Map<Page, string[]>();
test.beforeEach(({ page }) => {
	fixture.reset();
	const errors: string[] = [];
	pageErrors.set(page, errors);
	page.on('pageerror', (error) => errors.push(error.message));
});
test.afterEach(({ page }) => {
	expect(pageErrors.get(page)).toEqual([]);
	pageErrors.delete(page);
});
async function gotoDocument(page: Page, url: string, options?: Parameters<Page['goto']>[1]) {
	const response = await page.goto(url, options);
	expect(response?.status(), `Document ${url} must render successfully`).toBe(200);
}

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
	await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
	await expect(page.locator('body')).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => getComputedStyle(document.body).visibility))
		.toBe('visible');
}
async function capturePaints(page: Page, theme = 'light') {
	await page.addInitScript((preference) => {
		localStorage.setItem('ui-preferences', JSON.stringify({ theme: preference }));
		const frames: {
			palette: string;
			font: string;
			dark: boolean;
			loaded: boolean;
			path: string;
		}[] = [];
		Object.assign(window, { appearancePaints: frames });
		function sample() {
			if (document.body && getComputedStyle(document.body).visibility === 'visible') {
				const css = getComputedStyle(document.documentElement);
				frames.push({
					path: location.pathname,
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
					appearancePaints: {
						palette: string;
						font: string;
						dark: boolean;
						loaded: boolean;
						path: string;
					}[];
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
	await gotoDocument(page, `${fixture.baseUrl}/login`);
	await ready(page);
	await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeVisible();
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
	await gotoDocument(page, `${fixture.baseUrl}/login`, { waitUntil: 'domcontentloaded' });
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

test('the document needs no downloadable bootstrap when its legacy URL stalls', async ({
	page
}) => {
	let requests = 0;
	let release = () => {};
	const held = new Promise<void>((done) => {
		release = done;
	});
	await page.route('**/school-appearance-bootstrap.js', async (route) => {
		requests++;
		await held;
		await route.continue();
	});
	await capturePaints(page, 'dark');
	try {
		await gotoDocument(page, `${fixture.baseUrl}/login`, { waitUntil: 'domcontentloaded' });
		await ready(page);
		await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeVisible();
		await expect(page.locator('#school-appearance-bootstrap')).toHaveCount(1);
		expect(await page.locator('#school-appearance-bootstrap').getAttribute('src')).toBeNull();
		expect(requests).toBe(0);
		await page.evaluate(async () => {
			await document.fonts.ready;
			await new Promise(requestAnimationFrame);
		});
		const frames = await paints(page);
		expect(frames.length).toBeGreaterThan(0);
		expect(
			frames.every(
				(frame) =>
					frame.palette === 'purple' &&
					frame.font.includes('School Noto Sans Thai') &&
					frame.loaded &&
					!frame.dark
			)
		).toBe(true);
		expect(requests).toBe(0);
	} finally {
		release();
	}
});

test('the independent CSS watchdog reveals stable system typography if only the inline bootstrap cannot execute', async ({
	page
}) => {
	await page.route(
		(url) => url.pathname === '/login',
		async (route) => {
			if (!route.request().isNavigationRequest()) return route.continue();
			const response = await route.fetch();
			const html = await response.text();
			const stripped = html.replace(
				/<script id="school-appearance-bootstrap">[\s\S]*?<\/script>/,
				''
			);
			expect(stripped).not.toBe(html);
			await route.fulfill({ response, body: stripped });
		}
	);
	await gotoDocument(page, `${fixture.baseUrl}/login`, { waitUntil: 'domcontentloaded' });
	await ready(page);
	await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeVisible();
	await expect(page.locator('#school-appearance-bootstrap')).toHaveCount(0);
	expect(await page.evaluate(() => getComputedStyle(document.body).fontFamily)).toContain(
		'system-ui'
	);
	await page.evaluate(async () => {
		await document.fonts.load('600 16px "School Noto Sans Thai"', 'ก SchoolOrbit');
		await document.fonts.ready;
		const consumer = document.createElement('div');
		consumer.dataset.fontConsumer = 'true';
		consumer.style.fontFamily = 'var(--school-font-family)';
		consumer.textContent = 'ทดสอบ';
		document.body.append(consumer);
	});
	expect(
		await page
			.locator('[data-font-consumer]')
			.evaluate((element) => getComputedStyle(element).fontFamily)
	).toContain('system-ui');
	expect(await page.evaluate(() => getComputedStyle(document.body).fontFamily)).toContain(
		'system-ui'
	);
});

for (const role of ['staff', 'student', 'parent']) {
	test(`${role} initial personal dark mode and school appearance are ready before loaders paint`, async ({
		page
	}) => {
		if (role === 'parent') await mockParent(page);
		else if (role === 'student') await mockStudentSchedules(page);
		else await mockSession(page, [], role);
		await page.route(
			(url) => url.pathname === '/api/school/public/appearance',
			(route) => route.continue()
		);
		await capturePaints(page, 'dark');
		await gotoDocument(page, `${fixture.baseUrl}/${role}`);
		await ready(page);
		await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
		await expect(
			page.getByTestId(role === 'staff' ? 'staff-work-counts' : `${role}-profile-region`)
		).toBeVisible();
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
	let loggedIn = false;
	await page.route(
		(url) => url.pathname === '/api/auth/me',
		(route) =>
			loggedIn
				? route.fallback()
				: route.fulfill({ status: 401, json: { success: false, error: 'No session' } })
	);
	await page.route(
		(url) => url.pathname === '/api/auth/login',
		(route) => {
			loggedIn = true;
			const user: CurrentUserDto = {
				id: actor,
				username: 'E2E-staff-home',
				firstName: 'เจ้าของ',
				lastName: 'โปรไฟล์',
				displayName: 'เจ้าของ โปรไฟล์',
				userType: 'staff',
				status: 'active',
				primaryRoleName: 'บุคลากร',
				profileImageFileId: null,
				permissions: ['settings.read.all', 'settings.update.all']
			};
			return route.fulfill({ json: { success: true, data: { user } } });
		}
	);
	await page.emulateMedia({ colorScheme: 'dark' });
	await capturePaints(page, 'system');
	await gotoDocument(page, `${fixture.baseUrl}/login`);
	await ready(page);
	await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeVisible();
	await expect(page.locator('html')).not.toHaveClass(/dark/);
	await page.getByLabel('ชื่อผู้ใช้งาน (Username)', { exact: true }).fill('E2E-staff-home');
	await page.getByLabel('รหัสผ่าน', { exact: true }).fill('synthetic-test-password');
	await page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true }).click();
	await expect(page.getByTestId('staff-work-counts')).toBeVisible();
	await expect(page.locator('html')).toHaveClass(/dark/);
	await navigate(page, '/staff/school-settings/appearance');
	await expect(page.getByTestId('school-appearance-settings')).toBeVisible();
	await expect(page.locator('#custom-primary')).toBeVisible();
	const frames = (await paints(page)).filter((frame) => frame.path.startsWith('/staff'));
	expect(frames.length).toBeGreaterThan(0);
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

test('real settings save applies atomically and a fresh browser receives persisted school appearance', async ({
	page,
	browser
}, testInfo) => {
	const api = await mockSession(page);
	let release = () => {};
	const held = new Promise<void>((done) => {
		release = done;
	});
	await page.route('**/fonts/Sarabun-*.ttf', async (route) => {
		await held;
		await route.continue();
	});
	await gotoDocument(page, `${fixture.baseUrl}/staff/school-settings/appearance`);
	await ready(page);
	await expect(page.getByTestId('school-appearance-settings')).toBeVisible();
	await expect(page.locator('main').first().getByRole('heading', { level: 1 })).toHaveCount(1);
	await expect(page.locator('title')).toHaveCount(1);
	await expect
		.poll(() =>
			page
				.locator('form [data-slot="card"]')
				.first()
				.evaluate((card) => getComputedStyle(card).paddingBlockStart)
		)
		.toBe('0px');
	await page.getByLabel('ฟอนต์ของโรงเรียน', { exact: true }).click();
	await page.getByRole('option', { name: 'Sarabun', exact: true }).click();
	await page.getByLabel('ปรับสีหลักเพิ่มเติม').fill('#245fc2');
	await page.getByRole('button', { name: 'บันทึกรูปแบบโรงเรียน', exact: true }).click();
	await expect.poll(() => fixture.saved.appearance.font).toBe('sarabun');
	expect(
		await page.evaluate(() =>
			getComputedStyle(document.documentElement).getPropertyValue('--school-appearance-font').trim()
		)
	).toBe('noto');
	await expect(page.locator('body')).toBeVisible();
	release();
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
	await page.setViewportSize({ width: 1440, height: 1000 });
	await page
		.getByRole('heading', { level: 1, name: 'รูปแบบและตัวอักษร', exact: true })
		.scrollIntoViewIfNeeded();
	await page.evaluate(() => new Promise(requestAnimationFrame));
	await page.screenshot({
		path: testInfo.outputPath('appearance-settings-desktop.png'),
		fullPage: true
	});
	await page.setViewportSize({ width: 390, height: 844 });
	await page
		.getByRole('heading', { level: 1, name: 'รูปแบบและตัวอักษร', exact: true })
		.scrollIntoViewIfNeeded();
	await page.evaluate(() => new Promise(requestAnimationFrame));
	await page.screenshot({
		path: testInfo.outputPath('appearance-settings-mobile.png'),
		fullPage: true
	});
	const fresh = await browser.newContext({ serviceWorkers: 'block' });
	const next = await fresh.newPage();
	await gotoDocument(next, `${fixture.baseUrl}/login`);
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
	await gotoDocument(page, `${fixture.baseUrl}/staff/school-settings/appearance`);
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
	await gotoDocument(page, `${fixture.baseUrl}/staff/school-settings/appearance`);
	await ready(page);
	await expect(page.getByRole('button', { name: 'บันทึกรูปแบบโรงเรียน', exact: true })).toHaveCount(
		0
	);
	expect(reads).toBe(0);
});

test('a committed save reports font failure truthfully and explicit retry can recover failed font faces', async ({
	page
}) => {
	await mockSession(page);
	let failFont = true;
	await page.route('**/fonts/Sarabun-*.ttf', (route) =>
		failFont ? route.abort() : route.continue()
	);
	await gotoDocument(page, `${fixture.baseUrl}/staff/school-settings/appearance`);
	await page.getByLabel('ฟอนต์ของโรงเรียน', { exact: true }).click();
	await page.getByRole('option', { name: 'Sarabun', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกรูปแบบโรงเรียน', exact: true }).click();
	await expect(
		page.getByText(
			'บันทึกรูปแบบของทั้งโรงเรียนแล้ว แต่โหลดฟอนต์ไม่สำเร็จ หน้านี้ยังใช้รูปแบบเดิมชั่วคราว',
			{ exact: true }
		)
	).toBeVisible();
	expect(fixture.saved.appearance.font).toBe('sarabun');
	expect(await page.evaluate(() => getComputedStyle(document.body).fontFamily)).toContain(
		'School Noto Sans Thai'
	);
	failFont = false;
	await page.getByRole('button', { name: 'ลองโหลดฟอนต์อีกครั้ง', exact: true }).click();
	await expect(page.getByText('บันทึกและแสดงรูปแบบล่าสุดแล้ว', { exact: true })).toBeVisible();
	expect(await page.evaluate(() => getComputedStyle(document.body).fontFamily)).toContain(
		'School Sarabun'
	);
});

test('visibility revalidation is throttled, has no interval, and applies a newer revision after font preparation', async ({
	page
}) => {
	await mockSession(page);
	await page.clock.install();
	await gotoDocument(page, `${fixture.baseUrl}/staff/school-settings/appearance`);
	await ready(page);
	fixture.set({
		...fixture.saved,
		appearance: { ...fixture.saved.appearance, palette: 'blue' },
		revision: 9
	});
	await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
	expect(
		fixture.requests.filter((request) => request.path === '/api/school/public/appearance')
	).toHaveLength(1);
	await page.clock.fastForward(31_000);
	expect(
		fixture.requests.filter((request) => request.path === '/api/school/public/appearance')
	).toHaveLength(1);
	await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
	await expect
		.poll(() =>
			page.evaluate(() =>
				getComputedStyle(document.documentElement)
					.getPropertyValue('--school-appearance-revision')
					.trim()
			)
		)
		.toBe('9');
	await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
	expect(
		fixture.requests.filter((request) => request.path === '/api/school/public/appearance')
	).toHaveLength(2);
});

test('backend appearance failure has uncached stable defaults and no delayed initial swap', async ({
	page
}) => {
	fixture.failure(true);
	await gotoDocument(page, `${fixture.baseUrl}/login`);
	await ready(page);
	expect(
		await page.evaluate(() =>
			getComputedStyle(document.documentElement)
				.getPropertyValue('--school-appearance-palette')
				.trim()
		)
	).toBe('teal');
	fixture.failure(false);
	await navigate(page, '/login?retry=true');
	expect(
		await page.evaluate(() =>
			getComputedStyle(document.documentElement)
				.getPropertyValue('--school-appearance-palette')
				.trim()
		)
	).toBe('teal');
	expect(
		fixture.requests.filter((request) => request.path === '/api/school/public/appearance')
	).toHaveLength(1);
});

test('shared density changes default table spacing while explicit table/card utility overrides remain intact', async ({
	page
}) => {
	await gotoDocument(page, `${fixture.baseUrl}/__appearance-primitives`);
	await expect(page.getByTestId('default-cell')).toBeVisible();
	const geometry = () =>
		page.evaluate(() => {
			const css = (testId: string) =>
				getComputedStyle(document.querySelector(`[data-testid="${testId}"]`)!);
			return {
				cell: Number.parseFloat(css('default-cell').paddingTop),
				explicit: css('explicit-cell').paddingTop,
				card: css('explicit-card').paddingTop,
				gap: css('explicit-card').gap
			};
		});
	const compact = await geometry();
	fixture.set({
		...fixture.saved,
		appearance: { ...fixture.saved.appearance, density: 'comfortable' },
		revision: 9
	});
	const response = await page.reload();
	expect(response?.status()).toBe(200);
	await expect(page.getByTestId('default-cell')).toBeVisible();
	const comfortable = await geometry();
	expect(compact.cell).toBeLessThan(comfortable.cell);
	expect(compact.explicit).toBe(comfortable.explicit);
	expect(compact.card).toBe('0px');
	expect(comfortable.card).toBe('0px');
	expect(compact.gap).toBe('0px');
	expect(comfortable.gap).toBe('0px');
});

test('a blocked appearance stylesheet preserves readable defaults and does not break application startup', async ({
	page
}) => {
	await page.route('**/school-appearance.css', (route) => route.abort());
	await gotoDocument(page, `${fixture.baseUrl}/login`);
	await ready(page);
	await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeVisible();
	await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
	expect(await page.evaluate(() => getComputedStyle(document.body).fontFamily)).toContain(
		'system-ui'
	);
	expect(
		await page.evaluate(() =>
			getComputedStyle(document.documentElement).getPropertyValue('--primary').trim()
		)
	).toBe('#167166');
	expect(
		fixture.requests.filter((request) => request.path === '/api/school/public/appearance')
	).toHaveLength(0);
});

test('actual primary and destructive primitives retain readable text in both modes and hover states', async ({
	page
}) => {
	for (const primary of [null, '#ffffff', '#ffff00']) {
		fixture.set({
			...fixture.saved,
			appearance: { ...fixture.saved.appearance, primary },
			revision: fixture.saved.revision + 1
		});
		await gotoDocument(page, `${fixture.baseUrl}/__appearance-primitives`);
		await expect(page.getByTestId('primary-button')).toBeVisible();
		expect(
			await page
				.getByTestId('font-sans-consumer')
				.evaluate((element) => getComputedStyle(element).fontFamily)
		).toContain('School Noto Sans Thai');
		for (const dark of [false, true]) {
			await page.evaluate(
				(value) => document.documentElement.classList.toggle('dark', value),
				dark
			);
			for (const testId of ['primary-button', 'destructive-button', 'destructive-badge']) {
				const element = page.getByTestId(testId);
				for (const hovered of [false, true]) {
					if (hovered) await element.hover();
					else await page.mouse.move(0, 0);
					await page.evaluate(async () => {
						void getComputedStyle(document.body).color;
						await Promise.all(document.getAnimations().map((animation) => animation.finished));
					});
					const colors = await element.evaluate((node) => {
						const css = getComputedStyle(node),
							surface = getComputedStyle(document.body).backgroundColor;
						const canvas = document.createElement('canvas');
						canvas.width = canvas.height = 1;
						const context = canvas.getContext('2d')!;
						const hex = () =>
							'#' +
							[...context.getImageData(0, 0, 1, 1).data]
								.slice(0, 3)
								.map((value) => value.toString(16).padStart(2, '0'))
								.join('');
						context.fillStyle = surface;
						context.fillRect(0, 0, 1, 1);
						context.fillStyle = css.backgroundColor;
						context.fillRect(0, 0, 1, 1);
						const background = hex();
						context.fillStyle = css.color;
						context.fillRect(0, 0, 1, 1);
						return { foreground: hex(), background };
					});
					expect(
						contrastRatio(colors.foreground, colors.background),
						`${testId} ${dark ? 'dark' : 'light'} ${hovered ? 'hover' : 'rest'} ${primary}`
					).toBeGreaterThanOrEqual(4.5);
				}
			}
		}
	}
});
