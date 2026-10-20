import { expect, test, type Page, type Locator } from '@playwright/test';
import { startSchoolAppearanceServer } from './fixtures/school-appearance-server';
import { mockStaffHome, actor } from './fixtures/staff-home-route-data';
import { mockParent } from './fixtures/parent-route-data';
import { mockStudentSchedules } from './fixtures/student-schedules-route-data';
import { navigate } from './fixtures/supervision-route-data';
import type { CurrentUserDto } from '../../src/lib/api/auth.js';
import type { CatalogSubjectOverview } from '../../src/lib/api/academic-core.js';
import type {
	AdmissionRound,
	AdmissionTrack,
	AdmissionExamSubject,
	ApplicationDetailResponse,
	ApplicationListItem,
	RawScore,
	ScoreRoomGroup
} from '../../src/lib/api/admission.js';
import {
	FONTS,
	contrastRatio,
	type SchoolAppearanceResponse
} from '../../src/lib/appearance/model.js';

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

async function settleAnimations(page: Page) {
	await page.evaluate(() => new Promise(requestAnimationFrame));
	await expect
		.poll(() =>
			page.evaluate(
				() =>
					document
						.getAnimations()
						.filter(
							(animation) =>
								animation.playState === 'running' &&
								animation.effect?.getComputedTiming().iterations !== Infinity
						).length
			)
		)
		.toBe(0);
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
	expect(await page.locator('meta[name="theme-color"]').getAttribute('content')).toBe('#f2eef6');
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
	await expect(page.getByLabel('ปรับสีหลักเพิ่มเติม')).toBeVisible();
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

	await page.reload();
	await ready(page);
	await expect(page.getByTestId('school-appearance-settings')).toBeVisible();
	expect(
		await page.evaluate(() => {
			const css = getComputedStyle(document.documentElement);
			return ['font', 'palette', 'density', 'primary'].map((name) =>
				css.getPropertyValue(`--school-appearance-${name}`).trim()
			);
		})
	).toEqual(['sarabun', 'purple', 'compact', '#245fc2']);
	await expect(page.getByLabel('ฟอนต์ของโรงเรียน', { exact: true })).toHaveText('Sarabun');
	await expectSchoolFont(
		page,
		page.getByRole('heading', { level: 1, name: 'รูปแบบและตัวอักษร', exact: true }),
		'School Sarabun'
	);
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
	test.setTimeout(60_000);
	for (const primary of [null, '#ffffff', '#000000', '#ffff00', '#dbeafe', '#fce7f3']) {
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
			await page.evaluate(async () => {
				void getComputedStyle(document.body).color;
				await Promise.all(document.getAnimations().map((animation) => animation.finished));
			});
			const inkChecks = await page.evaluate(() => {
				const color = (id: string) =>
					getComputedStyle(document.querySelector(`[data-testid="${id}"]`)!);
				return ['primary-link', 'primary-icon', 'primary-card-text'].map((id) => ({
					id,
					foreground: color(id).color,
					background:
						id === 'primary-card-text'
							? getComputedStyle(
									document.querySelector('[data-testid="primary-card-text"]')!.parentElement!
								).backgroundColor
							: getComputedStyle(document.body).backgroundColor
				}));
			});
			for (const check of inkChecks) {
				const toHex = (rgb: string) =>
					'#' +
					rgb
						.match(/\d+/g)!
						.slice(0, 3)
						.map((channel) => Number(channel).toString(16).padStart(2, '0'))
						.join('');
				expect(
					contrastRatio(toHex(check.foreground), toHex(check.background)),
					`${check.id} ${primary} ${dark} ${JSON.stringify(check)}`
				).toBeGreaterThanOrEqual(4.5);
			}
			for (const testId of [
				'primary-button',
				'primary-badge',
				'destructive-button',
				'destructive-badge'
			]) {
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
						const foreground = hex();
						context.fillStyle = css.borderColor;
						context.fillRect(0, 0, 1, 1);
						const border = hex();
						context.fillStyle = surface;
						context.fillRect(0, 0, 1, 1);
						return { foreground, background, border, surface: hex() };
					});
					if (testId.startsWith('primary-')) {
						if (!hovered && primary) expect(colors.background).toBe(primary);
						expect(contrastRatio(colors.border, colors.surface)).toBeGreaterThanOrEqual(3);
					}
					expect(
						contrastRatio(colors.foreground, colors.background),
						`${testId} ${dark ? 'dark' : 'light'} ${hovered ? 'hover' : 'rest'} ${primary}`
					).toBeGreaterThanOrEqual(4.5);
				}
			}
		}
	}
});

async function expectSchoolFont(page: Page, element: Locator, family: string) {
	await expect(element).toBeVisible();
	await expect
		.poll(() => element.evaluate((node) => getComputedStyle(node).fontFamily))
		.toContain(family);
	// Inspect rendered glyphs as well as the CSS stack: a declared but unavailable font can fall back.
	await element.evaluate((node) => node.setAttribute('data-font-probe', 'true'));
	const client = await page.context().newCDPSession(page);
	try {
		await client.send('DOM.enable');
		await client.send('CSS.enable');
		const document = await client.send('DOM.getDocument');
		const { nodeId } = await client.send('DOM.querySelector', {
			nodeId: document.root.nodeId,
			selector: '[data-font-probe]'
		});
		const { fonts } = await client.send('CSS.getPlatformFontsForNode', { nodeId });
		expect(fonts.length).toBeGreaterThan(0);
		expect(fonts).toEqual(
			fonts.map(() =>
				expect.objectContaining({
					isCustomFont: true,
					familyName: expect.stringContaining(family.replace('School ', ''))
				})
			)
		);
	} finally {
		await client.detach();
		await element.evaluate((node) => node.removeAttribute('data-font-probe'));
	}
}

for (const [index, font] of FONTS.entries()) {
	test(`business course codes and numbers follow ${font.label} on desktop, mobile and SPA saves`, async ({
		page
	}, testInfo) => {
		fixture.set({
			...fixture.saved,
			appearance: { ...fixture.saved.appearance, font: font.value }
		});
		await mockSession(page, [
			'settings.read.all',
			'settings.update.all',
			'academic_catalog.read.all',
			'academic_catalog.create.all'
		]);
		const subjectId = '60000000-0000-4000-8000-000000000001';
		const groupId = '60000000-0000-4000-8000-000000000002';
		const timestamp = '2026-01-01T00:00:00Z';
		const overview: CatalogSubjectOverview = {
			gradeLevelOptions: [],
			subjectGroupOptions: [
				{
					subjectGroupId: groupId,
					organizationUnitId: groupId,
					code: 'MATH',
					name: 'คณิตศาสตร์',
					canManage: true
				}
			],
			items: [
				{
					subject: {
						id: subjectId,
						subjectGroupId: groupId,
						owningOrganizationUnitId: groupId,
						code: 'ค21101',
						rowVersion: 1,
						createdAt: timestamp,
						updatedAt: timestamp
					},
					displayState: 'current',
					canManage: true,
					draftCount: 0,
					gradeLevels: [],
					displayVersion: {
						id: subjectId,
						subjectId,
						versionNo: 1,
						nameTh: 'คณิตศาสตร์ทดสอบ',
						subjectType: 'basic',
						credit: '1.5',
						periodsPerWeek: 3,
						hoursPerSemester: 60,
						gradeLevelIds: [],
						effectiveFrom: '2026-01-01',
						status: 'published',
						migrated: false,
						rowVersion: 1,
						createdAt: timestamp,
						updatedAt: timestamp
					}
				}
			]
		};
		await page.route(
			(url) => url.pathname === '/api/academic/catalog/subjects/overview',
			(route) => route.fulfill({ json: { success: true, data: overview } })
		);
		await page.route(
			(url) => url.pathname === `/api/academic/catalog/subjects/${subjectId}/versions`,
			(route) => route.fulfill({ json: { success: true, data: [] } })
		);
		await gotoDocument(page, `${fixture.baseUrl}/staff/academic/catalog/subjects`);
		await ready(page);
		const table = page.getByTestId('catalog-subjects-ready').locator('table');
		await expectSchoolFont(page, table.getByText('ค21101', { exact: true }), font.family);
		await expectSchoolFont(page, table.getByText('1.5', { exact: true }), font.family);
		await page.screenshot({
			path: testInfo.outputPath(`course-codes-${font.value}-desktop.png`),
			fullPage: true
		});
		const input = page.locator('#new-subject-code');
		await expect(input).toHaveCSS('font-family', `"${font.family}", sans-serif`);
		await page.getByRole('button', { name: 'เปิด ค21101', exact: true }).click();
		await expectSchoolFont(
			page,
			page.getByRole('dialog').getByText('ค21101', { exact: true }),
			font.family
		);
		await page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
		await page.setViewportSize({ width: 390, height: 844 });
		const card = page
			.getByTestId('catalog-subjects-ready')
			.getByRole('button')
			.filter({ has: page.getByText('คณิตศาสตร์ทดสอบ', { exact: true }) });
		await expectSchoolFont(page, card.getByText('ค21101', { exact: true }), font.family);
		await expectSchoolFont(page, card.getByText('1.5', { exact: true }), font.family);
		await page.screenshot({
			path: testInfo.outputPath(`course-codes-${font.value}-mobile.png`),
			fullPage: true
		});
		await navigate(page, '/staff/school-settings/appearance');
		await expect(page.getByTestId('school-appearance-settings')).toBeVisible();
		await page.getByLabel('ฟอนต์ของโรงเรียน', { exact: true }).click();
		const nextFont = FONTS[(index + 1) % FONTS.length];
		await expectSchoolFont(
			page,
			page.getByRole('option', { name: nextFont.label, exact: true }),
			font.family
		);
		await page.getByRole('option', { name: nextFont.label, exact: true }).click();
		await page.getByRole('button', { name: 'บันทึกรูปแบบโรงเรียน', exact: true }).click();
		await expect(page.getByText('บันทึกรูปแบบของทั้งโรงเรียนแล้ว', { exact: true })).toBeVisible();
		await navigate(page, '/staff/academic/catalog/subjects');
		await expectSchoolFont(page, card.getByText('ค21101', { exact: true }), nextFont.family);
		await expectSchoolFont(page, card.getByText('1.5', { exact: true }), nextFont.family);

		await gotoDocument(page, `${fixture.baseUrl}/__appearance-primitives`);
		await page.evaluate(async (family) => {
			await Promise.all(
				[400, 500, 600, 700].map((weight) =>
					document.fonts.load(`${weight} 16px "${family}"`, 'ก SchoolOrbit 0123456789')
				)
			);
			await document.fonts.ready;
		}, nextFont.family);
		await expectSchoolFont(page, page.getByTestId('font-sans-consumer'), nextFont.family);
		await expectSchoolFont(
			page,
			page.getByTestId('exam-time').getByText('09:00-10:00'),
			nextFont.family
		);
		await expectSchoolFont(
			page,
			page.getByTestId('exam-status').getByText('กำลังบันทึก'),
			nextFont.family
		);
		await page.getByRole('button', { name: 'แสดงข้อความแจ้งเตือน', exact: true }).click();
		await expectSchoolFont(page, page.locator('[data-sonner-toast] [data-title]'), nextFont.family);
		await expectSchoolFont(
			page,
			page.locator('[data-sonner-toast] [data-description]'),
			nextFont.family
		);
		await expectSchoolFont(
			page,
			page.locator('[data-sonner-toast] [data-button]'),
			nextFont.family
		);
		await expect(page.getByTestId('technical-code')).toHaveCSS('font-family', /monospace/);
		await expect(page.locator('.katex .mathnormal').first()).toHaveCSS('font-family', /KaTeX_Math/);
	});
}

test('color picker supports keyboard RGB, pale presets, invalid drafts, disabled saves and exact persisted reloads', async ({
	page
}) => {
	test.setTimeout(60_000);
	const api = await mockSession(page);
	await gotoDocument(page, `${fixture.baseUrl}/staff/school-settings/appearance`);
	const input = page.getByLabel('ปรับสีหลักเพิ่มเติม');
	const trigger = page.getByRole('button', { name: 'เลือกสีหลัก', exact: true });
	const save = page.getByRole('button', { name: 'บันทึกรูปแบบโรงเรียน', exact: true });
	await trigger.focus();
	await trigger.press('Enter');
	const picker = page.getByRole('dialog', { name: 'เลือกสีหลัก', exact: true });
	await expect(picker).toBeVisible();
	await picker.getByRole('button', { name: 'ฟ้าอ่อน #DBEAFE', exact: true }).click();
	await expect(input).toHaveValue('#dbeafe');
	const red = picker.getByRole('slider', { name: 'แดง', exact: true });
	await red.focus();
	await red.press('Home');
	await expect(input).toHaveValue('#00eafe');
	await red.press('End');
	await expect(input).toHaveValue('#ffeafe');
	await red.press('ArrowLeft');
	await expect(input).toHaveValue('#feeafe');
	await expect(red).toHaveAttribute('aria-valuenow', '254');
	await red.press('Escape');
	await expect(picker).toHaveCount(0);
	await expect(trigger).toBeFocused();
	await input.fill('#db');
	await expect(input).toHaveAttribute('aria-invalid', 'true');
	await expect(save).toBeDisabled();
	await input.press('Enter');
	expect(api.writes).toHaveLength(0);
	await trigger.click();
	await picker.getByRole('button', { name: 'ฟ้าอ่อน #DBEAFE', exact: true }).click();
	await picker.getByRole('button', { name: 'เสร็จสิ้น', exact: true }).click();
	await expect(input).toHaveAttribute('aria-invalid', 'false');
	await expect(save).toBeEnabled();
	const preview = page.getByTestId('staff-design-preview');
	await expect(preview.getByRole('button', { name: 'เปิดงานของฉัน', exact: true })).toHaveCSS(
		'background-color',
		'rgb(219, 234, 254)'
	);
	let release = () => {};
	const held = new Promise<void>((done) => {
		release = done;
	});
	await page.route(
		(url) => url.pathname === privatePath,
		async (route) => {
			if (route.request().method() === 'PATCH') await held;
			await route.fallback();
		}
	);
	await trigger.click();
	await page
		.locator('form.settings-column')
		.evaluate((form) => (form as HTMLFormElement).requestSubmit());
	try {
		await expect(trigger).toBeDisabled();
		await expect(input).toBeDisabled();
		await expect(
			picker.getByRole('button', { name: 'ฟ้าอ่อน #DBEAFE', exact: true })
		).toBeDisabled();
		const disabledRed = picker.getByRole('slider', { name: 'แดง', exact: true });
		await expect(disabledRed).toHaveAttribute('aria-disabled', 'true');
		await disabledRed.press('ArrowRight');
		await expect(input).toHaveValue('#dbeafe');
		await page.keyboard.press('Escape');
	} finally {
		release();
	}
	await expect(page.getByText('บันทึกรูปแบบของทั้งโรงเรียนแล้ว', { exact: true })).toBeVisible();
	expect(api.writes).toEqual([
		{ appearance: { ...fixture.saved.appearance, primary: '#dbeafe' }, expectedRevision: 8 }
	]);
	expect(fixture.saved.appearance.primary).toBe('#dbeafe');
	for (const dark of [false, true]) {
		await page.evaluate((value) => document.documentElement.classList.toggle('dark', value), dark);
		expect(
			await page.evaluate(() =>
				getComputedStyle(document.documentElement).getPropertyValue('--primary-fill').trim()
			)
		).toBe('#dbeafe');
	}
	const savedCss = await (
		await page.request.get(`${fixture.baseUrl}/school-appearance.css`)
	).text();
	await page.reload();
	await expect(input).toHaveValue('#dbeafe');
	expect(await (await page.request.get(`${fixture.baseUrl}/school-appearance.css`)).text()).toBe(
		savedCss
	);
	expect(
		await page.evaluate(() =>
			getComputedStyle(document.documentElement).getPropertyValue('--primary-fill').trim()
		)
	).toBe('#dbeafe');
	await mockSession(page, ['settings.read.all']);
	await page.reload();
	await expect(input).toBeDisabled();
	await expect(trigger).toBeDisabled();
	await expect(save).toHaveCount(0);
});

for (const font of FONTS) {
	test(`appearance spacing stays clear with ${font.label} at 1218px and mobile in both densities and themes`, async ({
		page
	}, testInfo) => {
		test.setTimeout(90_000);
		await mockSession(page);
		for (const density of ['comfortable', 'compact'] as const) {
			fixture.set({
				...fixture.saved,
				appearance: { ...fixture.saved.appearance, font: font.value, density, primary: '#dbeafe' }
			});
			await gotoDocument(page, `${fixture.baseUrl}/staff/school-settings/appearance`);
			await expect(page.getByTestId('school-appearance-settings')).toBeVisible();
			await expect(page.getByText('พรีวิวพร้อมแล้ว', { exact: true })).toBeVisible();
			for (const dark of [false, true]) {
				await page.evaluate(
					(value) => document.documentElement.classList.toggle('dark', value),
					dark
				);
				await page
					.getByRole('button', { name: dark ? 'พรีวิวมืด' : 'พรีวิวสว่าง', exact: true })
					.click();
				for (const width of [1218, 390]) {
					await page.setViewportSize({ width, height: width === 1218 ? 888 : 844 });
					await settleAnimations(page);
					const geometry = await page.locator('.settings-column').evaluate((column) => {
						const bounds = (element: Element) => element.getBoundingClientRect();
						const fieldGaps = Array.from(column.querySelectorAll('.fields')).flatMap((fields) =>
							Array.from(fields.children)
								.slice(1)
								.map((field, index) => bounds(field).top - bounds(fields.children[index]).bottom)
						);
						const headingGaps = Array.from(column.querySelectorAll('.section-copy')).map(
							(copy) => bounds(copy.children[1]).top - bounds(copy.children[0]).bottom
						);
						return {
							fieldGaps,
							headingGaps,
							rem: Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
							overflow: document.documentElement.scrollWidth > innerWidth
						};
					});
					expect(geometry.fieldGaps.length).toBe(3);
					for (const gap of geometry.fieldGaps)
						expect(gap).toBeGreaterThanOrEqual(geometry.rem * 1.25 - 0.5);
					for (const gap of geometry.headingGaps)
						expect(gap).toBeGreaterThanOrEqual(geometry.rem * 0.375 - 0.5);
					expect(geometry.overflow).toBe(false);
					if (font.value === 'noto' && density === 'comfortable') {
						await page
							.getByRole('heading', { level: 1, name: 'รูปแบบและตัวอักษร', exact: true })
							.scrollIntoViewIfNeeded();
						await page.screenshot({
							path: testInfo.outputPath(`appearance-pale-${width}-${dark ? 'dark' : 'light'}.png`),
							fullPage: true
						});
						const trigger = page.getByRole('button', { name: 'เลือกสีหลัก', exact: true });
						await trigger.click();
						const picker = page.getByRole('dialog', { name: 'เลือกสีหลัก', exact: true });
						await expect(picker).toHaveCSS('opacity', '1');
						await settleAnimations(page);
						const panelBackground = await picker.evaluate(
							(element) => getComputedStyle(element).backgroundColor
						);
						expect(panelBackground).not.toBe('rgba(0, 0, 0, 0)');
						const box = await picker.boundingBox();
						expect(box!.x).toBeGreaterThanOrEqual(0);
						expect(box!.x + box!.width).toBeLessThanOrEqual(width);
						await page.screenshot({
							path: testInfo.outputPath(
								`appearance-picker-${width}-${dark ? 'dark' : 'light'}.png`
							),
							fullPage: true
						});
						await page.keyboard.press('Escape');
					}
				}
			}
		}
	});
}

test('real admission actions preserve pale theme fills and readable absence controls in both modes', async ({
	page
}) => {
	test.setTimeout(90_000);
	fixture.set({
		...fixture.saved,
		appearance: { ...fixture.saved.appearance, primary: '#dbeafe' }
	});
	await mockSession(page, ['admission.read.all', 'admission.verify.all', 'admission.scores.all']);
	const id = (index: number) => `61000000-0000-4000-8000-${String(index).padStart(12, '0')}`;
	const roundId = id(1),
		appId = id(2),
		trackId = id(3),
		subjectId = id(4),
		timestamp = '2026-01-01T00:00:00Z';
	const round: AdmissionRound = {
		id: roundId,
		academicYearId: id(5),
		gradeLevelId: id(6),
		name: 'รอบทดสอบสี',
		applyStartDate: '2026-01-01',
		applyEndDate: '2026-12-31',
		status: 'open',
		isVisible: true,
		createdAt: timestamp,
		updatedAt: timestamp
	};
	const tracks: AdmissionTrack[] = [
		{
			id: trackId,
			admissionRoundId: roundId,
			studyProgramId: id(7),
			name: 'สายทดสอบ',
			scoringSubjectIds: [subjectId],
			tiebreakMethod: 'applied_at',
			displayOrder: 1,
			createdAt: timestamp
		}
	];
	const subjects: AdmissionExamSubject[] = [
		{
			id: subjectId,
			admissionRoundId: roundId,
			name: 'วิชาทดสอบ',
			maxScore: 100,
			displayOrder: 1,
			createdAt: timestamp
		}
	];
	const detail: ApplicationDetailResponse = {
		application: {
			id: appId,
			admissionRoundId: roundId,
			admissionTrackId: trackId,
			applicationNumber: 'TEST-001',
			nationalId: '1111111111111',
			firstName: 'ตัวอย่าง',
			lastName: 'ทดสอบ',
			status: 'submitted',
			createdAt: timestamp,
			updatedAt: timestamp
		},
		documents: []
	};
	const applications: ApplicationListItem[] = [
		{
			id: appId,
			applicationNumber: 'TEST-001',
			fullName: 'ตัวอย่าง ทดสอบ',
			status: 'absent',
			trackName: 'สายทดสอบ',
			createdAt: timestamp
		}
	];
	const scores: RawScore[] = [{ applicationId: appId, subjectId, score: null, status: 'absent' }];
	const roster: ScoreRoomGroup[] = [
		{
			examRoomId: id(8),
			roomName: 'ห้องทดสอบ',
			seats: [
				{
					applicationId: appId,
					applicationNumber: 'TEST-001',
					examId: 'EXAM-001',
					fullName: 'ตัวอย่าง ทดสอบ',
					seatNumber: 1
				}
			]
		}
	];
	applications.push({
		...applications[0],
		id: id(9),
		fullName: 'มาเรียน ทดสอบ',
		status: 'verified'
	});
	scores.push({ applicationId: id(9), subjectId, score: 50, status: 'verified' });
	roster[0].seats.push({
		...roster[0].seats[0],
		applicationId: id(9),
		fullName: 'มาเรียน ทดสอบ',
		seatNumber: 2
	});
	const replies: Record<string, unknown> = {
		[`/api/admission/applications/${appId}`]: detail,
		[`/api/admission/rounds/${roundId}`]: round,
		[`/api/admission/rounds/${roundId}/tracks`]: tracks,
		[`/api/admission/rounds/${roundId}/subjects`]: subjects,
		[`/api/admission/rounds/${roundId}/scores`]: scores,
		[`/api/admission/rounds/${roundId}/score-room-roster`]: roster,
		[`/api/admission/rounds/${roundId}/applications`]: applications
	};
	await page.route(
		(url) => url.pathname.startsWith('/api/admission/'),
		async (route) => {
			const path = new URL(route.request().url()).pathname;
			expect(route.request().method()).toBe('GET');
			expect(replies).toHaveProperty(path);
			await route.fulfill({ json: { success: true, data: replies[path] } });
		}
	);
	async function inspectAction(button: Locator, themeFill: boolean) {
		await expect(button).toBeVisible();
		await expect(button).toBeEnabled();
		for (const width of [1218, 390]) {
			await page.setViewportSize({ width, height: 888 });
			for (const hovered of [false, true]) {
				if (hovered) await button.hover();
				else await page.mouse.move(0, 0);
				await settleAnimations(page);
				const colors = await button.evaluate((element) => {
					const canvas = document.createElement('canvas');
					canvas.width = canvas.height = 1;
					const context = canvas.getContext('2d')!;
					const paint = (color: string) => {
						context.fillStyle = color;
						context.fillRect(0, 0, 1, 1);
					};
					const hex = () =>
						'#' +
						[...context.getImageData(0, 0, 1, 1).data]
							.slice(0, 3)
							.map((channel) => channel.toString(16).padStart(2, '0'))
							.join('');
					const ancestors: Element[] = [];
					let opacity = 1;
					for (let parent: Element | null = element; parent; parent = parent.parentElement) {
						ancestors.unshift(parent);
						opacity *= Number(getComputedStyle(parent).opacity);
					}
					// Resolve translucent hover fills over the actual row/card/page surfaces.
					paint('#ffffff');
					for (const ancestor of ancestors) paint(getComputedStyle(ancestor).backgroundColor);
					const background = hex();
					paint(getComputedStyle(element).color);
					return { foreground: hex(), background, opacity };
				});
				expect(colors.opacity, 'enabled controls remain readable through their ancestor rows').toBe(
					1
				);
				expect(contrastRatio(colors.foreground, colors.background)).toBeGreaterThanOrEqual(4.5);
				if (themeFill && !hovered) expect(colors.background).toBe('#dbeafe');
				if (!themeFill) expect(colors.background).not.toBe('#dbeafe');
			}
		}
	}
	for (const mode of ['light', 'dark'] as const) {
		await gotoDocument(page, `${fixture.baseUrl}/apply/status`);
		await ready(page);
		await page.evaluate(async (value) => {
			const modulePath = '/src/lib/stores/ui-preferences.ts';
			const { uiPreferences } = await import(modulePath);
			uiPreferences.setTheme(value);
		}, mode);
		await expect
			.poll(() => page.locator('html').evaluate((element) => element.classList.contains('dark')))
			.toBe(mode === 'dark');
		await inspectAction(page.getByRole('button', { name: 'ตรวจสอบผลการสมัคร', exact: true }), true);
		await gotoDocument(
			page,
			`${fixture.baseUrl}/staff/academic/admission/${roundId}/applications/${appId}`
		);
		await inspectAction(page.getByRole('button', { name: 'อนุมัติ', exact: true }), true);
		await gotoDocument(page, `${fixture.baseUrl}/staff/academic/admission/${roundId}/scores`);
		await inspectAction(page.getByRole('button', { name: 'ขาด', exact: true }), false);
		await inspectAction(
			page.getByRole('row').filter({ hasText: 'มาเรียน ทดสอบ' }).getByRole('button'),
			false
		);
		await page.getByRole('button', { name: 'ตามสาย', exact: true }).click();
		await inspectAction(page.getByRole('button', { name: 'ขาด', exact: true }), false);
		await inspectAction(
			page.getByRole('row').filter({ hasText: 'มาเรียน ทดสอบ' }).getByRole('button'),
			false
		);
	}
});
