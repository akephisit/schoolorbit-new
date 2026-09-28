import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
test.describe.configure({ mode: 'default' });

const id = (n: number) => `33000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
const roundId = id(1);
const otherRoundId = id(2);
const appId = id(3);
const path = (round: string, page: string) => `/staff/academic/admission/${round}/${page}`;

async function reply(route: Route, data: unknown, status = 200) {
	await route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

function gate() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => (release = resolve));
	return { promise, release: () => release() };
}

async function mock(
	page: Page,
	options: {
		permissions?: string[];
		hold?: 'enrollment' | 'student-ids' | 'applications' | 'round';
		failOnce?: 'enrollment' | 'student-ids' | 'applications';
	} = {}
) {
	const reads: string[] = [];
	const writes: string[] = [];
	const held = gate();
	let failed = false;
	let rank = 1;
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const resource = url.pathname;
			const method = route.request().method();
			if (method === 'GET') reads.push(resource);
			else writes.push(`${method} ${resource}`);
			if (resource === '/api/auth/me')
				return reply(route, {
					id: id(20),
					username: 'E2E-admission-final',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: options.permissions ?? [
						'admission.read.all',
						'admission.manage.all',
						'admission.enroll.all'
					]
				});
			if (resource === '/api/academic/context/options')
				return reply(route, {
					activeAcademicYearId: null,
					activeAcademicTermId: null,
					years: [],
					terms: []
				});
			const active = resource.includes(otherRoundId) ? otherRoundId : roundId;
			if (resource === `/api/admission/rounds/${active}` && method === 'GET') {
				if (options.hold === 'round' && active === roundId) await held.promise;
				return reply(route, {
					id: active,
					name: active === roundId ? 'รอบรับสมัครแรก' : 'รอบรับสมัครใหม่',
					academicYearId: id(10),
					gradeLevelId: id(11),
					status: 'published',
					selectionSettings: { assignmentMode: 'per_track' },
					reportConfig: {
						reportMode: 'zone',
						zone: { schools: ['โรงเรียนเดิม'] },
						institution: { ownSchool: 'โรงเรียนของเรา' }
					}
				});
			}
			if (resource === `/api/admission/rounds/${active}/enrollment` && method === 'GET') {
				if (options.hold === 'enrollment' && active === roundId) await held.promise;
				if (options.failOnce === 'enrollment' && !failed) {
					failed = true;
					return reply(route, 'รายชื่อไม่พร้อม', 503);
				}
				return reply(route, [
					{
						id: appId,
						applicationNumber: '6900001',
						nationalId: '0000000000000',
						fullName: active === roundId ? 'นักเรียน รับมอบตัว' : 'นักเรียน รอบใหม่',
						trackName: 'สายวิทย์',
						roomName: 'ม.1/1',
						status: 'accepted',
						studentConfirmed: true,
						preSubmitted: true,
						assignedStudentId: '1001',
						formData: null
					}
				]);
			}
			if (resource === `/api/admission/applications/${appId}/enroll` && method === 'POST')
				return reply(route, { userId: id(30), username: '1001', studentCode: '1001' });
			if (resource === `/api/admission/rounds/${active}/student-ids` && method === 'GET') {
				if (options.hold === 'student-ids' && active === roundId) await held.promise;
				if (options.failOnce === 'student-ids' && !failed) {
					failed = true;
					return reply(route, 'เลขประจำตัวไม่พร้อม', 503);
				}
				return reply(route, [
					{
						applicationId: appId,
						applicationNumber: '6900001',
						fullName: 'นักเรียน กำหนดเลข',
						firstName: 'นักเรียน',
						lastName: 'กำหนดเลข',
						assignedStudentId: null,
						previousSchool: 'โรงเรียนเดิม',
						roomName: 'ม.1/1',
						rankInRoom: rank,
						rankInTrack: 1
					}
				]);
			}
			if (resource === `/api/admission/rounds/${active}/student-ids` && method === 'PATCH') {
				const body = route.request().postDataJSON() as unknown[];
				return reply(route, { updated: body.length });
			}
			if (resource === `/api/admission/rounds/${active}/sort-room-students` && method === 'POST') {
				rank = 2;
				return reply(route, { updated: 1 });
			}
			if (resource === `/api/admission/rounds/${active}/applications` && method === 'GET') {
				if (options.hold === 'applications' && active === roundId) await held.promise;
				if (options.failOnce === 'applications' && !failed) {
					failed = true;
					return reply(route, 'ข้อมูลรายงานไม่พร้อม', 503);
				}
				return reply(route, [
					{
						id: appId,
						applicationNumber: '6900001',
						fullName: 'นักเรียน รายงาน',
						status: 'accepted',
						previousSchool: 'โรงเรียนเดิม',
						createdAt: '2026-09-29T00:00:00Z'
					}
				]);
			}
			return reply(route, 'unmocked', 404);
		}
	);
	return { reads, writes, release: held.release };
}

test('enrollment shows the round while its list is slow, then patches a completed row', async ({
	page
}) => {
	const api = await mock(page, { hold: 'enrollment' });
	await page.goto(path(roundId, 'enrollment'));
	await expect(page.getByText('รอบรับสมัครแรก')).toBeVisible();
	await expect(page.getByText('นักเรียน รับมอบตัว')).toHaveCount(0);
	api.release();
	await expect(page.getByText('นักเรียน รับมอบตัว')).toBeVisible();
	await page.getByRole('button', { name: 'รับมอบตัว', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันมอบตัว' }).click();
	await expect(page.getByText('เสร็จสิ้น')).toBeVisible();
	expect(
		api.reads.filter((resource) => resource === `/api/admission/rounds/${roundId}/enrollment`)
	).toHaveLength(1);
	expect(
		api.reads.filter((resource) => resource === `/api/admission/rounds/${roundId}`)
	).toHaveLength(1);
});

test('enrollment list retries without rereading the round', async ({ page }) => {
	const api = await mock(page, { failOnce: 'enrollment' });
	await page.goto(path(roundId, 'enrollment'));
	await expect(page.getByText('รายชื่อไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('นักเรียน รับมอบตัว')).toBeVisible();
	expect(
		api.reads.filter((resource) => resource === `/api/admission/rounds/${roundId}`)
	).toHaveLength(1);
});

test('a delayed previous enrollment list cannot replace the newly selected round', async ({
	page
}) => {
	const api = await mock(page, { hold: 'enrollment' });
	await page.goto(path(roundId, 'enrollment'), { waitUntil: 'domcontentloaded' });
	await expect(page.getByText('รอบรับสมัครแรก')).toBeVisible();
	await page.evaluate(
		(target) => {
			const link = document.createElement('a');
			link.href = target;
			link.textContent = 'next round';
			document.body.append(link);
			link.click();
			link.remove();
		},
		path(otherRoundId, 'enrollment')
	);
	await expect(page.getByText('รอบรับสมัครใหม่')).toBeVisible();
	await expect(page.getByText('นักเรียน รอบใหม่')).toBeVisible();
	api.release();
	await expect(page.getByText('นักเรียน รับมอบตัว')).toHaveCount(0);
});

test('student-ID list is independent and save updates only changed IDs', async ({ page }) => {
	const api = await mock(page, { hold: 'student-ids' });
	await page.goto(path(roundId, 'student-ids'));
	await expect(page.getByText('รอบรับสมัครแรก')).toBeVisible();
	api.release();
	const row = page.getByRole('row').filter({ hasText: 'นักเรียน กำหนดเลข' });
	await row.getByPlaceholder('กรอกเลข...').fill('2001');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(row.getByPlaceholder('กรอกเลข...')).toHaveValue('2001');
	expect(api.writes).toContain(`PATCH /api/admission/rounds/${roundId}/student-ids`);
	expect(
		api.reads.filter((resource) => resource === `/api/admission/rounds/${roundId}/student-ids`)
	).toHaveLength(1);
});

test('sorting student IDs rereads only its list', async ({ page }) => {
	const api = await mock(page);
	await page.goto(path(roundId, 'student-ids'));
	await expect(page.getByText('นักเรียน กำหนดเลข')).toBeVisible();
	const row = page.getByRole('row').filter({ hasText: 'นักเรียน กำหนดเลข' });
	await row.getByPlaceholder('กรอกเลข...').fill('2001');
	await page.getByRole('button', { name: 'จัดเรียงในห้อง' }).click();
	await expect
		.poll(
			() =>
				api.reads.filter((resource) => resource === `/api/admission/rounds/${roundId}/student-ids`)
					.length
		)
		.toBe(2);
	await expect(row.getByPlaceholder('กรอกเลข...')).toHaveValue('2001');
	expect(
		api.reads.filter((resource) => resource === `/api/admission/rounds/${roundId}`)
	).toHaveLength(1);
});

test('student-ID list failure offers focused retry', async ({ page }) => {
	const api = await mock(page, { failOnce: 'student-ids' });
	await page.goto(path(roundId, 'student-ids'));
	await expect(page.getByText('เลขประจำตัวไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('นักเรียน กำหนดเลข')).toBeVisible();
	expect(
		api.reads.filter((resource) => resource === `/api/admission/rounds/${roundId}`)
	).toHaveLength(1);
});

test('report shows config before a slow application collection and retries only that collection', async ({
	page
}) => {
	const api = await mock(page, { hold: 'applications' });
	await page.goto(path(roundId, 'report'));
	await expect(page.getByText('เขตพื้นที่บริการ', { exact: true }).first()).toBeVisible();
	await expect(page.getByText('กำลังโหลด...').first()).toBeVisible();
	api.release();
	await expect(page.getByText('ผู้สมัครทั้งหมด')).toBeVisible();
	await expect(page.getByText('1 คน').first()).toBeVisible();
	expect(
		api.reads.filter((resource) => resource === `/api/admission/rounds/${roundId}`)
	).toHaveLength(1);
});

test('report failure retries applications without blocking config', async ({ page }) => {
	const api = await mock(page, { failOnce: 'applications' });
	await page.goto(path(roundId, 'report'));
	await expect(page.getByText('ข้อมูลรายงานไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('1 คน').first()).toBeVisible();
	expect(
		api.reads.filter((resource) => resource === `/api/admission/rounds/${roundId}`)
	).toHaveLength(1);
});

test('missing permissions start no admission reads on the three pages', async ({ page }) => {
	const api = await mock(page, { permissions: [] });
	for (const name of ['enrollment', 'student-ids', 'report']) {
		await page.goto(path(roundId, name));
		await expect(page.getByText(/ไม่มีสิทธิ์/).first()).toBeVisible();
	}
	expect(api.reads.filter((resource) => resource.startsWith('/api/admission/'))).toEqual([]);
});
