import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
test.describe.configure({ mode: 'default' });

const id = (n: number) => `20000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
const roundId = id(1);
const otherRoundId = id(6);
const appId = id(2);
const nextAppId = id(3);
const trackId = id(4);
const otherTrackId = id(5);
const app = {
	id: appId,
	admissionRoundId: roundId,
	admissionTrackId: trackId,
	applicationNumber: '6906060300001',
	nationalId: '1111111111111',
	fullName: 'เด็กชาย ทดสอบ',
	firstName: 'ทดสอบ',
	lastName: 'ตัวอย่าง',
	trackName: 'สายเดิม',
	roundName: 'รอบทดสอบ',
	status: 'submitted',
	phone: '0812345678',
	createdAt: '2026-05-01T00:00:00Z'
};
const otherRoundApp = {
	id: id(7),
	applicationNumber: '6906060300002',
	fullName: 'เด็กหญิง รอบใหม่',
	trackName: 'สายใหม่',
	status: 'submitted',
	phone: '0812345678',
	createdAt: '2026-05-02T00:00:00Z'
};

async function reply(route: Route, data: unknown, status = 200) {
	await route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

async function mock(page: Page, manager = true) {
	const reads: string[] = [];
	const writes: string[] = [];
	const filters: string[] = [];
	const identifierBodies: string[] = [];
	let holdList = false;
	let holdDetail = false;
	let holdTracks = false;
	let holdIdentifier = false;
	let holdVerify = false;
	let failListOnce = false;
	let failIdentifierOnce = false;
	let failDetailOnce = false;
	let statusAware = false;
	let currentStatus = app.status;
	let releaseList = () => {};
	let releaseDetail = () => {};
	let releaseTracks = () => {};
	let releaseIdentifier = () => {};
	let releaseVerify = () => {};
	const listGate = new Promise<void>((resolve) => (releaseList = resolve));
	const detailGate = new Promise<void>((resolve) => (releaseDetail = resolve));
	const tracksGate = new Promise<void>((resolve) => (releaseTracks = resolve));
	const identifierGate = new Promise<void>((resolve) => (releaseIdentifier = resolve));
	const verifyGate = new Promise<void>((resolve) => (releaseVerify = resolve));
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const path = url.pathname;
			const method = route.request().method();
			if (method === 'GET') reads.push(path);
			else writes.push(`${method} ${path}`);
			if (path === '/api/auth/me')
				return reply(route, {
					id: id(20),
					username: 'E2E-admission',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: [
						'admission.read.all',
						...(manager ? ['admission.manage.all', 'admission.verify.all'] : [])
					]
				});
			if (path === '/api/academic/context/options')
				return reply(route, {
					activeAcademicYearId: null,
					activeAcademicTermId: null,
					years: [],
					terms: []
				});
			if (path === `/api/admission/rounds/${otherRoundId}/applications` && method === 'GET') {
				filters.push(url.search);
				return reply(route, [otherRoundApp]);
			}
			if (path === `/api/admission/rounds/${roundId}/applications` && method === 'GET') {
				filters.push(url.search);
				if (holdList) await listGate;
				if (failListOnce) {
					failListOnce = false;
					return reply(route, 'โหลดใบสมัครล้มเหลว', 503);
				}
				const rows = [
					{
						id: appId,
						applicationNumber: app.applicationNumber,
						fullName: app.fullName,
						trackName: app.trackName,
						status: currentStatus,
						phone: app.phone,
						createdAt: app.createdAt
					}
				];
				return reply(
					route,
					statusAware &&
						url.searchParams.get('status') &&
						url.searchParams.get('status') !== currentStatus
						? []
						: rows
				);
			}
			if (
				path === `/api/admission/rounds/${roundId}/applications/search-by-identifier` &&
				method === 'POST'
			) {
				identifierBodies.push(route.request().postDataJSON()?.identifier ?? '');
				if (holdIdentifier) await identifierGate;
				if (failIdentifierOnce) {
					failIdentifierOnce = false;
					return reply(route, 'ค้นหาใบสมัครล้มเหลว', 503);
				}
				return reply(route, [
					{
						id: appId,
						applicationNumber: app.applicationNumber,
						fullName: app.fullName,
						trackName: app.trackName,
						status: app.status,
						phone: app.phone,
						createdAt: app.createdAt
					}
				]);
			}
			if (path === `/api/admission/applications/${appId}` && method === 'GET') {
				if (holdDetail) await detailGate;
				if (failDetailOnce) {
					failDetailOnce = false;
					return reply(route, 'โหลดรายละเอียดล้มเหลว', 503);
				}
				return reply(route, { application: app, documents: [] });
			}
			if (path === `/api/admission/applications/${nextAppId}` && method === 'GET')
				return reply(route, {
					application: { ...app, id: nextAppId, firstName: 'คนถัดไป', nationalId: '2222222222222' },
					documents: []
				});
			if (path === `/api/admission/rounds/${roundId}/tracks` && method === 'GET') {
				if (holdTracks) await tracksGate;
				return reply(route, [
					{ id: trackId, name: 'สายเดิม', admissionRoundId: roundId },
					{ id: otherTrackId, name: 'สายใหม่', admissionRoundId: roundId }
				]);
			}
			if (path === `/api/admission/applications/${appId}/verify` && method === 'PUT') {
				if (holdVerify) await verifyGate;
				currentStatus = 'verified';
				return reply(route, {});
			}
			if (path === `/api/admission/applications/${appId}/admission-track` && method === 'PATCH')
				return reply(route, {});
			if (path === '/api/notifications/stream')
				return route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
			if (path === '/api/school/public')
				return reply(route, { schoolName: 'โรงเรียนทดสอบ', logoFileId: null });
			if (path === '/api/menu/user') return reply(route, { groups: [] });
			if (path === '/api/notifications') return reply(route, { items: [], unread_count: 0 });
			if (path === '/api/me/work-items/counts')
				return reply(route, { open: 0, dueSoon: 0, overdue: 0, submitted: 0, closed: 0, total: 0 });
			return reply(route, 'ไม่เปิดใช้ในชุดทดสอบนี้', 403);
		}
	);
	return {
		reads,
		writes,
		filters,
		identifierBodies,
		holdList: () => (holdList = true),
		holdDetail: () => (holdDetail = true),
		holdTracks: () => (holdTracks = true),
		holdIdentifier: () => (holdIdentifier = true),
		holdVerify: () => (holdVerify = true),
		statusAware: () => (statusAware = true),
		failNextList: () => (failListOnce = true),
		failNextIdentifier: () => (failIdentifierOnce = true),
		failNextDetail: () => (failDetailOnce = true),
		releaseList: () => releaseList(),
		releaseDetail: () => releaseDetail(),
		releaseTracks: () => releaseTracks(),
		releaseIdentifier: () => releaseIdentifier(),
		releaseVerify: () => releaseVerify()
	};
}

test('application list shows a first skeleton and omits national IDs', async ({ page }) => {
	const state = await mock(page);
	state.holdList();
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	state.releaseList();
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await expect(page.getByText(app.nationalId, { exact: true })).toHaveCount(0);
});

test('application list retries a failed primary read in place', async ({ page }) => {
	const state = await mock(page);
	state.failNextList();
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText('โหลดใบสมัครไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await expect
		.poll(() => state.reads.filter((path) => path.endsWith('/applications')).length)
		.toBe(2);
});

test('application list commits search to URL and only rereads when the query changes', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill('ทดสอบ');
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect(page).toHaveURL(/search=%E0%B8%97%E0%B8%94%E0%B8%AA%E0%B8%AD%E0%B8%9A/);
	await expect.poll(() => state.filters.length).toBe(2);
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	expect(state.filters).toHaveLength(2);
});

test('national-ID search uses a body request and never writes the ID into URL history', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill(app.nationalId);
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect.poll(() => state.identifierBodies).toEqual([app.nationalId]);
	await expect(page).not.toHaveURL(new RegExp(app.nationalId));
	expect(state.filters.every((query) => !query.includes(app.nationalId))).toBe(true);
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
});

test('a dashed national ID is normalized for body search without entering the URL', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill('1-1111-11111-11-1');
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect.poll(() => state.identifierBodies).toEqual([app.nationalId]);
	await expect(page).not.toHaveURL(/search=/);
});

test('a labeled or punctuated identifier still uses body search', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page
		.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...')
		.fill(`ID: ${app.nationalId}.`);
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect.poll(() => state.identifierBodies).toEqual([app.nationalId]);
	await expect(page).not.toHaveURL(new RegExp(app.nationalId));
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
});

test('ambiguous long numeric search is rejected without URL navigation', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill(`2${app.nationalId}`);
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect(page.getByText('กรุณากรอกเลข 13 หลักให้ชัดเจน')).toBeVisible();
	expect(state.identifierBodies).toHaveLength(0);
	await expect(page).not.toHaveURL(new RegExp(app.nationalId));
	expect(state.filters).toHaveLength(1);
});

test('a national ID written in Thai digits is never copied into the URL', async ({ page }) => {
	const state = await mock(page);
	const thaiDigits = '๑'.repeat(13);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill(thaiDigits);
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect(page.getByText('กรุณากรอกเลข 13 หลักให้ชัดเจน')).toBeVisible();
	await expect(page).not.toHaveURL(new RegExp(thaiDigits));
	expect(state.identifierBodies).toHaveLength(0);
	expect(state.filters).toHaveLength(1);
});

test('a 13-digit application number also uses body search and remains findable', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill(app.applicationNumber);
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect.poll(() => state.identifierBodies).toEqual([app.applicationNumber]);
	await expect(page).not.toHaveURL(/search=/);
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
});

test('a different identifier shows its own first-load skeleton instead of old results', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill(app.nationalId);
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect.poll(() => state.identifierBodies).toEqual([app.nationalId]);
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	state.holdIdentifier();
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill('2222222222222');
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await expect(page.getByText(app.fullName, { exact: true })).toHaveCount(0);
	state.releaseIdentifier();
	await expect.poll(() => state.identifierBodies).toEqual([app.nationalId, '2222222222222']);
});

test('a failed identifier search retries the same POST instead of loading the general list', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	state.failNextIdentifier();
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill(app.nationalId);
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect(page.getByText('โหลดใบสมัครไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect.poll(() => state.identifierBodies).toEqual([app.nationalId, app.nationalId]);
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	expect(state.filters).toHaveLength(1);
});

test('legacy URL search containing a national ID redirects to a clean URL', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications?search=${app.nationalId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page).not.toHaveURL(new RegExp(app.nationalId));
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	expect(state.filters.every((query) => !query.includes(app.nationalId))).toBe(true);
});

test('legacy URL search also scrubs a labeled national ID', async ({ page }) => {
	const state = await mock(page);
	await page.goto(
		`/staff/academic/admission/${roundId}/applications?search=${encodeURIComponent(`ID: ${app.nationalId}.`)}`,
		{ waitUntil: 'domcontentloaded' }
	);
	await expect(page).not.toHaveURL(new RegExp(app.nationalId));
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	expect(state.filters.every((query) => !query.includes(app.nationalId))).toBe(true);
});

test('national-ID results still respect the selected status filter', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications?status=verified`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill(app.nationalId);
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect.poll(() => state.identifierBodies).toEqual([app.nationalId]);
	await expect(page.getByText(app.fullName, { exact: true })).toHaveCount(0);
});

test('changing status after exact search announces that the private search scope is cleared', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill(app.nationalId);
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect.poll(() => state.identifierBodies).toEqual([app.nationalId]);
	await expect(
		page.getByRole('status').filter({ hasText: 'เปลี่ยนสถานะแล้วจะออกจากผลค้นหานี้' })
	).toBeVisible();
	await page.getByRole('button', { name: 'สถานะทั้งหมด' }).click();
	await page.getByRole('option', { name: 'ผ่านตรวจสอบ' }).click();
	await expect(page).toHaveURL(/status=verified/);
	await expect(page.getByText('เปลี่ยนสถานะแล้ว การค้นหาเลขเดิมถูกยกเลิก')).toBeVisible();
	await expect.poll(() => state.filters.length).toBe(2);
	expect(state.identifierBodies).toEqual([app.nationalId]);
});

test('date-only navigation filters the local list without another API request', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.clock.setFixedTime(new Date('2026-05-02T08:00:00+07:00'));
	await page.getByRole('button', { name: 'เลือกวันที่' }).click();
	await page.getByRole('button', { name: 'วันเสาร์ที่ 2 พฤษภาคม 2569' }).click();
	await expect(page).toHaveURL(/date=2026-05-02/);
	await expect(page.getByText(app.fullName, { exact: true })).toHaveCount(0);
	expect(state.filters).toHaveLength(1);
	await page.goBack();
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	expect(state.filters).toHaveLength(1);
});

test('a later status change keeps the shallow date filter in the URL', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.clock.setFixedTime(new Date('2026-05-01T08:00:00+07:00'));
	await page.getByRole('button', { name: 'เลือกวันที่' }).click();
	await page.getByRole('button', { name: 'วันศุกร์ที่ 1 พฤษภาคม 2569' }).click();
	await expect(page).toHaveURL(/date=2026-05-01/);
	await page.getByRole('button', { name: 'สถานะทั้งหมด' }).click();
	await page.getByRole('option', { name: 'ผ่านตรวจสอบ' }).click();
	await expect(page).toHaveURL(/date=2026-05-01/);
	await expect(page).toHaveURL(/status=verified/);
	await expect.poll(() => state.filters.length).toBe(2);
});

test('an in-app link to the same list with another date updates the visible filter', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications?date=2026-05-02`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText('แสดง 0 จาก 1 รายการ')).toBeVisible();
	await page.evaluate((href) => {
		const link = document.createElement('a');
		link.href = href;
		link.textContent = 'Go to another date';
		document.body.appendChild(link);
		link.click();
	}, `/staff/academic/admission/${roundId}/applications?date=2026-05-01`);
	await expect(page).toHaveURL(/date=2026-05-01/);
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	expect(state.filters).toHaveLength(1);
});

test('an in-app link to another round uses that route date instead of the old filter', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications?date=2026-05-01`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.evaluate((href) => {
		const link = document.createElement('a');
		link.href = href;
		document.body.appendChild(link);
		link.click();
	}, `/staff/academic/admission/${otherRoundId}/applications?date=2026-05-02`);
	await expect(page).toHaveURL(new RegExp(`${otherRoundId}.*date=2026-05-02`));
	await expect(page.getByText(otherRoundApp.fullName, { exact: true })).toBeVisible();
	await expect(page.getByText(app.fullName, { exact: true })).toHaveCount(0);
	expect(state.filters).toHaveLength(2);
});

test('status filter is URL-owned and back navigation restores the previous request', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'สถานะทั้งหมด' }).click();
	await page.getByRole('option', { name: 'ผ่านตรวจสอบ' }).click();
	await expect(page).toHaveURL(/status=verified/);
	await expect.poll(() => state.filters.length).toBe(2);
	await page.goBack();
	await expect(page).not.toHaveURL(/status=verified/);
	await expect.poll(() => state.filters.length).toBe(3);
});

test('verifying one application patches the list without rereading the collection', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await expect(page.getByText('ผ่านการตรวจสอบ', { exact: true })).toBeVisible();
	expect(state.reads.filter((path) => path.endsWith('/applications'))).toHaveLength(1);
});

test('a mutation that finishes after changing status reconciles the newly selected list', async ({
	page
}) => {
	const state = await mock(page);
	state.statusAware();
	state.holdVerify();
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await page.getByRole('button', { name: 'สถานะทั้งหมด' }).click();
	await page.getByRole('option', { name: 'ผ่านตรวจสอบ' }).click();
	await expect(page).toHaveURL(/status=verified/);
	await expect(page.getByText('ไม่พบใบสมัคร')).toBeVisible();
	state.releaseVerify();
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await expect.poll(() => state.filters.length).toBe(3);
});

test('date-only navigation during verification preserves the exact identifier result', async ({
	page
}) => {
	const state = await mock(page);
	state.holdVerify();
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	await page.clock.setFixedTime(new Date('2026-05-01T08:00:00+07:00'));
	await page.getByPlaceholder('ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร...').fill(app.nationalId);
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect.poll(() => state.identifierBodies).toEqual([app.nationalId]);
	await expect(
		page.getByRole('status').filter({ hasText: 'ผลค้นหาเลขบัตรหรือเลขที่ใบสมัคร' })
	).toBeVisible();
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await page.getByRole('button', { name: 'เลือกวันที่' }).click();
	await page.getByRole('button', { name: 'วันศุกร์ที่ 1 พฤษภาคม 2569' }).click();
	await expect(page).toHaveURL(/date=2026-05-01/);
	state.releaseVerify();
	await expect(page.getByText('ผ่านการตรวจสอบ', { exact: true })).toBeVisible();
	await expect(
		page.getByRole('status').filter({ hasText: 'ผลค้นหาเลขบัตรหรือเลขที่ใบสมัคร' })
	).toBeVisible();
	expect(state.filters).toHaveLength(1);
	expect(state.identifierBodies).toEqual([app.nationalId]);
});

test('a late list refresh cannot undo a verified status', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.fullName, { exact: true })).toBeVisible();
	state.holdList();
	await page.getByRole('button', { name: 'โหลดใบสมัครใหม่' }).click();
	await expect(page.locator('[aria-busy="true"]')).toBeVisible();
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await expect(page.getByText('ผ่านการตรวจสอบ', { exact: true })).toBeVisible();
	state.releaseList();
	await page.waitForLoadState('networkidle');
	await expect(page.getByText('ผ่านการตรวจสอบ', { exact: true })).toBeVisible();
});

test('application detail does not wait for or request track choices until edit', async ({
	page
}) => {
	const state = await mock(page);
	state.holdTracks();
	await page.goto(`/staff/academic/admission/${roundId}/applications/${appId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText('รายละเอียดใบสมัคร', { exact: true })).toBeVisible();
	await expect(page.getByText(app.nationalId, { exact: true })).toBeVisible();
	expect(state.reads).not.toContain(`/api/admission/rounds/${roundId}/tracks`);
	await page.getByRole('button', { name: 'แก้ไขสายการเรียน' }).click();
	await expect(page.getByLabel('กำลังโหลดสายการเรียน')).toBeVisible();
	state.releaseTracks();
	await expect(page.getByText('สายใหม่', { exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'ยกเลิก', exact: true }).last().click();
	await expect.poll(() => state.reads.filter((path) => path.endsWith('/tracks')).length).toBe(1);
});

test('changing a track patches the selected detail without rereading the applicant', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications/${appId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.nationalId, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'แก้ไขสายการเรียน' }).click();
	await page.getByRole('button', { name: 'สายเดิม' }).last().click();
	await page.getByRole('option', { name: 'สายใหม่' }).click();
	await page.getByRole('button', { name: 'บันทึก', exact: true }).last().click();
	await expect(page.getByText('สายใหม่', { exact: true })).toBeVisible();
	expect(state.writes).toContain(`PATCH /api/admission/applications/${appId}/admission-track`);
	expect(
		state.reads.filter((path) => path === `/api/admission/applications/${appId}`)
	).toHaveLength(1);
});

test('application detail retries a failed selected read without loading tracks', async ({
	page
}) => {
	const state = await mock(page);
	state.failNextDetail();
	await page.goto(`/staff/academic/admission/${roundId}/applications/${appId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText('โหลดรายละเอียดใบสมัครไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText(app.nationalId, { exact: true })).toBeVisible();
	expect(state.reads).not.toContain(`/api/admission/rounds/${roundId}/tracks`);
});

test('a late detail refresh cannot undo verification', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/applications/${appId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.nationalId, { exact: true })).toBeVisible();
	state.holdDetail();
	await page.getByRole('button', { name: 'โหลดรายละเอียดใบสมัครใหม่' }).click();
	await expect(page.locator('[aria-busy="true"]')).toBeVisible();
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await expect(page.getByText('ผ่านการตรวจสอบ', { exact: true })).toBeVisible();
	state.releaseDetail();
	await page.waitForLoadState('networkidle');
	await expect(page.getByText('ผ่านการตรวจสอบ', { exact: true })).toBeVisible();
});

test('read-only applicant detail omits track editing and its option request', async ({ page }) => {
	const state = await mock(page, false);
	await page.goto(`/staff/academic/admission/${roundId}/applications/${appId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(app.nationalId, { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'แก้ไขสายการเรียน' })).toHaveCount(0);
	expect(state.reads).not.toContain(`/api/admission/rounds/${roundId}/tracks`);
});

test('late old applicant detail cannot replace a newer applicant', async ({ page }) => {
	const state = await mock(page);
	state.holdDetail();
	await page.goto(`/staff/academic/admission/${roundId}/applications/${appId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await page.evaluate((href) => {
		const link = document.createElement('a');
		link.href = href;
		link.textContent = 'next applicant';
		document.body.append(link);
		link.click();
		link.remove();
	}, `/staff/academic/admission/${roundId}/applications/${nextAppId}`);
	await expect(page.getByText('คนถัดไป', { exact: true })).toBeVisible();
	state.releaseDetail();
	await expect(page.getByText(app.nationalId, { exact: true })).toHaveCount(0);
});

test('next applicant navigation is a tap-preloaded link in the same round', async ({ page }) => {
	await mock(page);
	await page.addInitScript(
		({ selectedRoundId, firstId, secondId }) => {
			sessionStorage.setItem(
				'admissionAppNav',
				JSON.stringify({ roundId: selectedRoundId, ids: [firstId, secondId] })
			);
		},
		{ selectedRoundId: roundId, firstId: appId, secondId: nextAppId }
	);
	await page.goto(`/staff/academic/admission/${roundId}/applications/${appId}`, {
		waitUntil: 'domcontentloaded'
	});
	const next = page.getByRole('link', { name: 'ผู้สมัครคนถัดไป' });
	await expect(next).toHaveAttribute('data-sveltekit-preload-data', 'tap');
	await next.click();
	await expect(page.getByText('คนถัดไป', { exact: true })).toBeVisible();
});
