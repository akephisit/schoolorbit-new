import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
// Wrangler's local Cloudflare runtime shares a SQLite database across requests.
test.describe.configure({ mode: 'default' });

const id = (n: number) => `10000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
const yearId = id(1);
const nextYearId = id(8);
const roundId = id(2);
const nextRoundId = id(9);
const round = {
	id: roundId,
	academicYearId: yearId,
	gradeLevelId: id(3),
	name: 'รอบทดสอบ 2569',
	applyStartDate: '2026-05-01',
	applyEndDate: '2026-05-31',
	status: 'draft',
	isVisible: false,
	createdAt: '2026-04-01T00:00:00Z',
	updatedAt: '2026-04-01T00:00:00Z',
	academicYearName: '2569',
	gradeLevelName: 'ม.1',
	applicationCount: 2,
	reportConfig: { reportMode: null }
};

async function reply(route: Route, data: unknown, status = 200) {
	await route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

async function mock(page: Page, manager = false, reportMode: 'zone' | null = null) {
	const reads: string[] = [];
	const writes: string[] = [];
	const gradeYears: string[] = [];
	let releaseList: (() => void) | null = null;
	let releaseTracks: (() => void) | null = null;
	let releaseYears: (() => void) | null = null;
	let releaseVisibility: (() => void) | null = null;
	let releaseGrades: (() => void) | null = null;
	let releaseSubjects: (() => void) | null = null;
	const waitList = new Promise<void>((resolve) => (releaseList = resolve));
	const waitTracks = new Promise<void>((resolve) => (releaseTracks = resolve));
	const waitYears = new Promise<void>((resolve) => (releaseYears = resolve));
	const waitVisibility = new Promise<void>((resolve) => (releaseVisibility = resolve));
	const waitGrades = new Promise<void>((resolve) => (releaseGrades = resolve));
	const waitSubjects = new Promise<void>((resolve) => (releaseSubjects = resolve));
	let holdList = false;
	let holdTracks = false;
	let holdYears = false;
	let holdVisibility = false;
	let holdGrades = false;
	let holdSubjects = false;
	let failListOnce = false;
	let staleListResponse = false;
	let failTracksOnce = false;
	let failSubjectRefresh = false;
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const path = url.pathname;
			const method = route.request().method();
			if (method === 'GET') {
				reads.push(path);
			} else writes.push(`${method} ${path}`);
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
					permissions: ['admission.read.all', ...(manager ? ['admission.manage.all'] : [])]
				});
			if (path === '/api/academic/context/options')
				return reply(route, {
					activeAcademicYearId: yearId,
					activeAcademicTermId: null,
					terms: [],
					years: [
						{
							id: yearId,
							year: 2569,
							name: '2569',
							status: 'active',
							startDate: '2026-05-01',
							endDate: '2027-04-30'
						},
						{
							id: nextYearId,
							year: 2570,
							name: '2570',
							status: 'planned',
							startDate: '2027-05-01',
							endDate: '2028-04-30'
						}
					]
				});
			if (path === '/api/admission/rounds' && method === 'GET') {
				const selectedYear = url.searchParams.get('academicYearId');
				if (holdList && selectedYear === yearId) await waitList;
				if (failListOnce) {
					failListOnce = false;
					return reply(route, 'โหลดรอบล้มเหลว', 503);
				}
				return reply(
					route,
					selectedYear === nextYearId
						? [
								{
									...round,
									academicYearId: nextYearId,
									academicYearName: '2570',
									name: 'รอบปีใหม่'
								}
							]
						: [staleListResponse ? { ...round, name: 'ข้อมูลเก่าก่อนบันทึก' } : round]
				);
			}
			if (path === `/api/admission/rounds/${roundId}` && method === 'GET')
				return reply(route, {
					...round,
					reportConfig: { reportMode, zone: { schools: [] }, institution: { ownSchool: '' } }
				});
			if (path === `/api/admission/rounds/${nextRoundId}` && method === 'GET')
				return reply(route, { ...round, id: nextRoundId, name: 'รอบถัดไป' });
			if (path === `/api/admission/rounds/${roundId}/visibility` && method === 'PATCH') {
				if (holdVisibility) await waitVisibility;
				return reply(route, { isVisible: true });
			}
			if (path === `/api/admission/rounds/${roundId}/status` && method === 'PUT')
				return reply(route, {});
			if (path === `/api/admission/rounds/${roundId}/tracks`) {
				if (method === 'POST')
					return reply(route, {
						id: id(12),
						admissionRoundId: roundId,
						studyProgramId: id(5),
						name: 'สายใหม่',
						capacityOverride: null,
						scoringSubjectIds: [],
						tiebreakMethod: 'applied_at',
						displayOrder: 2,
						createdAt: '2026-04-01T00:00:00Z',
						studyProgramName: 'วิทย์–คณิต',
						roomCount: 0,
						computedCapacity: null,
						applicationCount: 0
					});
				if (holdTracks) await waitTracks;
				if (failTracksOnce) {
					failTracksOnce = false;
					return reply(route, 'โหลดสายการเรียนล้มเหลว', 503);
				}
				return reply(route, [
					{
						id: id(4),
						admissionRoundId: roundId,
						studyProgramId: id(5),
						name: 'สายวิทย์',
						capacityOverride: null,
						roomCount: 2,
						computedCapacity: 60,
						applicationCount: 1,
						scoringSubjectIds: [],
						tiebreakMethod: 'applied_at',
						displayOrder: 1,
						createdAt: '2026-04-01T00:00:00Z'
					}
				]);
			}
			if (path === `/api/admission/tracks/${id(4)}` && method === 'PUT')
				return reply(route, {
					id: id(4),
					admissionRoundId: roundId,
					studyProgramId: id(5),
					name: 'สายวิทย์ใหม่',
					capacityOverride: null,
					scoringSubjectIds: [],
					tiebreakMethod: 'applied_at',
					displayOrder: 1,
					createdAt: '2026-04-01T00:00:00Z',
					studyProgramName: 'วิทย์–คณิต',
					roomCount: null,
					computedCapacity: null,
					applicationCount: 1
				});
			if (path === `/api/admission/rounds/${roundId}/subjects`) {
				if (method === 'POST')
					return reply(route, {
						id: id(10),
						admissionRoundId: roundId,
						name: 'ภาษาไทย',
						code: 'THAI',
						maxScore: 100,
						displayOrder: 2,
						createdAt: '2026-04-01T00:00:00Z'
					});
				if (holdSubjects) await waitSubjects;
				if (failSubjectRefresh) return reply(route, 'อ่านวิชาสอบล้มเหลว', 503);
				return reply(route, [
					{
						id: id(6),
						admissionRoundId: roundId,
						name: 'คณิตศาสตร์',
						maxScore: 100,
						displayOrder: 1,
						createdAt: '2026-04-01T00:00:00Z'
					}
				]);
			}
			if (path === `/api/admission/subjects/${id(6)}` && method === 'DELETE')
				return reply(route, {});
			if (path === '/api/academic/study-program-options')
				return reply(route, [
					{
						id: id(5),
						name: 'วิทย์–คณิต',
						code: 'SCI',
						curriculumId: id(7),
						curriculumName: 'หลักสูตร',
						versionName: '2569',
						status: 'published'
					}
				]);
			if (path === '/api/lookup/academic-years') {
				if (holdYears) await waitYears;
				return reply(route, [
					{ id: yearId, year: 2569, name: '2569', status: 'active' },
					{ id: nextYearId, year: 2570, name: '2570', status: 'planned' }
				]);
			}
			if (path === '/api/lookup/grade-levels') {
				const gradeYear = url.searchParams.get('academicYearId') ?? '';
				gradeYears.push(gradeYear);
				if (holdGrades && gradeYear === yearId) await waitGrades;
				return reply(route, [
					gradeYear === nextYearId
						? { id: id(11), name: 'มัธยมศึกษาปีที่ 2', short_name: 'ม.2', code: 'M2' }
						: { id: id(3), name: 'มัธยมศึกษาปีที่ 1', short_name: 'ม.1', code: 'M1' }
				]);
			}
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
		gradeYears,
		holdList: () => (holdList = true),
		holdTracks: () => (holdTracks = true),
		holdYears: () => (holdYears = true),
		holdVisibility: () => (holdVisibility = true),
		holdGrades: () => (holdGrades = true),
		holdSubjects: () => (holdSubjects = true),
		failNextList: () => (failListOnce = true),
		useStaleListResponse: () => (staleListResponse = true),
		failNextTracks: () => (failTracksOnce = true),
		failSubjectRefresh: () => (failSubjectRefresh = true),
		releaseList: () => releaseList?.(),
		releaseTracks: () => releaseTracks?.(),
		releaseYears: () => releaseYears?.(),
		releaseVisibility: () => releaseVisibility?.(),
		releaseGrades: () => releaseGrades?.(),
		releaseSubjects: () => releaseSubjects?.()
	};
}

test('round list shows a first skeleton and no manager option reads', async ({ page }) => {
	const state = await mock(page);
	state.holdList();
	await page.goto(`/staff/academic/admission?academicYearId=${yearId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	state.releaseList();
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	expect(state.reads.filter((path) => path === '/api/admission/rounds')).toHaveLength(1);
	expect(state.reads).not.toContain('/api/academic/study-program-options');
});

test('failed round list offers an in-place retry without reloading the page', async ({ page }) => {
	const state = await mock(page);
	state.failNextList();
	await page.goto(`/staff/academic/admission?academicYearId=${yearId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText('โหลดรอบรับสมัครไม่สำเร็จ', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	expect(state.reads.filter((path) => path === '/api/admission/rounds')).toHaveLength(2);
});

test('a list refresh keeps usable rounds and exposes a busy region', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission?academicYearId=${yearId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	state.holdList();
	await page.getByRole('button', { name: 'โหลดรอบใหม่' }).click();
	await expect(page.getByTestId('admission-round-list-region')).toHaveAttribute(
		'aria-busy',
		'true'
	);
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	state.releaseList();
	await expect(page.getByTestId('admission-round-list-region')).toHaveAttribute(
		'aria-busy',
		'false'
	);
});

test('an older list refresh cannot undo a successful status mutation', async ({ page }) => {
	const state = await mock(page, true);
	await page.goto(`/staff/academic/admission?academicYearId=${yearId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	state.useStaleListResponse();
	state.holdList();
	await page.getByRole('button', { name: 'โหลดรอบใหม่' }).click();
	await expect(page.getByTestId('admission-round-list-region')).toHaveAttribute(
		'aria-busy',
		'true'
	);
	await expect
		.poll(() => state.reads.filter((path) => path === '/api/admission/rounds').length)
		.toBe(2);
	await page.getByRole('button', { name: 'เปิดรับสมัคร' }).click();
	await expect(page.getByRole('button', { name: 'ปิดรับสมัคร' })).toBeVisible();
	state.releaseList();
	await expect(page.getByTestId('admission-round-list-region')).toHaveAttribute(
		'aria-busy',
		'false'
	);
	await expect(page.getByRole('button', { name: 'ปิดรับสมัคร' })).toBeVisible();
	await expect(page.getByText('ข้อมูลเก่าก่อนบันทึก', { exact: true })).toHaveCount(0);
});

test('a slow old academic year cannot replace a newer year during client navigation', async ({
	page
}) => {
	const state = await mock(page);
	state.holdList();
	await page.goto(`/staff/academic/admission?academicYearId=${yearId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await page.evaluate((year) => {
		const link = document.createElement('a');
		link.href = `/staff/academic/admission?academicYearId=${year}`;
		link.textContent = 'next year';
		document.body.append(link);
		link.click();
		link.remove();
	}, nextYearId);
	await expect(page.getByText('รอบปีใหม่', { exact: true })).toBeVisible();
	state.releaseList();
	await expect(page.getByText(round.name, { exact: true })).toHaveCount(0);
});

test('read-only round detail renders without manager regions or optional requests', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	expect(state.reads).not.toContain(`/api/admission/rounds/${roundId}/tracks`);
	expect(state.reads).not.toContain(`/api/admission/rounds/${roundId}/subjects`);
	expect(state.reads).not.toContain('/api/admission/rounds');
});

test('manager round header does not wait for tracks and copy/program options remain lazy', async ({
	page
}) => {
	const state = await mock(page, true);
	state.holdTracks();
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await expect(
		page.getByText('สายการเรียน (0)').locator('xpath=ancestor::*[@aria-busy][1]')
	).toHaveAttribute('aria-busy', 'true');
	expect(state.reads).not.toContain('/api/admission/rounds');
	expect(state.reads).not.toContain('/api/academic/study-program-options');
	state.releaseTracks();
	await expect(page.getByText('สายวิทย์', { exact: true })).toBeVisible();
	await expect(
		page.getByText('สายการเรียน (1)').locator('xpath=ancestor::*[@aria-busy][1]')
	).toHaveAttribute('aria-busy', 'false');
	await page.getByRole('button', { name: 'คัดลอกจากรอบอื่น' }).click();
	await expect
		.poll(() => state.reads.filter((path) => path === '/api/admission/rounds').length)
		.toBe(1);
	await page.getByRole('button', { name: 'เพิ่ม' }).first().click();
	await expect
		.poll(() => state.reads.filter((path) => path === '/api/academic/study-program-options').length)
		.toBe(1);
});

test('a failed track region retries without refetching the round or successful subjects', async ({
	page
}) => {
	const state = await mock(page, true);
	state.failNextTracks();
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	await expect(page.getByText('คณิตศาสตร์', { exact: true })).toBeVisible();
	await expect(page.getByText('โหลดสายการเรียนไม่สำเร็จ', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('สายวิทย์', { exact: true })).toBeVisible();
	expect(state.reads.filter((path) => path === `/api/admission/rounds/${roundId}`)).toHaveLength(1);
	expect(
		state.reads.filter((path) => path === `/api/admission/rounds/${roundId}/subjects`)
	).toHaveLength(1);
	expect(
		state.reads.filter((path) => path === `/api/admission/rounds/${roundId}/tracks`)
	).toHaveLength(2);
});

test('editing a track keeps known room and capacity values when its response omits derived fields', async ({
	page
}) => {
	const state = await mock(page, true);
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	const track = page.getByText('สายวิทย์', { exact: true });
	await expect(track).toBeVisible();
	await expect(track.locator('xpath=../..')).toContainText('รับ 60 คน (2 ห้อง)');
	await track.locator('xpath=../..').getByRole('button').first().click();
	await page.locator('#track-name').fill('สายวิทย์ใหม่');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).first().click();
	const updatedTrack = page.getByText('สายวิทย์ใหม่', { exact: true });
	await expect(updatedTrack).toBeVisible();
	await expect(updatedTrack.locator('xpath=../..')).toContainText('รับ 60 คน (2 ห้อง)');
	expect(
		state.reads.filter((path) => path === `/api/admission/rounds/${roundId}/tracks`)
	).toHaveLength(1);
});

test('a created track remains visible if its derived-capacity refresh fails', async ({ page }) => {
	const state = await mock(page, true);
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText('สายวิทย์', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'เพิ่ม', exact: true }).first().click();
	await page.locator('#track-plan').click();
	await page.getByRole('option', { name: 'วิทย์–คณิต' }).click();
	await page.locator('#track-name').fill('สายใหม่');
	state.failNextTracks();
	await page.getByRole('button', { name: 'บันทึก', exact: true }).first().click();
	await expect(page.getByText('สายใหม่', { exact: true })).toBeVisible();
	await expect(page.getByRole('alert').filter({ hasText: 'โหลดสายการเรียนล้มเหลว' })).toBeVisible();
	expect(
		state.reads.filter((path) => path === `/api/admission/rounds/${roundId}/tracks`)
	).toHaveLength(2);
});

test('a visibility response from a previous round cannot update the current round', async ({
	page
}) => {
	const state = await mock(page, true);
	state.holdVisibility();
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ซ่อนอยู่' }).click();
	await expect
		.poll(() => state.writes.filter((write) => write.endsWith('/visibility')).length)
		.toBe(1);
	await page.evaluate((id) => {
		const link = document.createElement('a');
		link.href = `/staff/academic/admission/${id}`;
		link.textContent = 'next round';
		document.body.append(link);
		link.click();
		link.remove();
	}, nextRoundId);
	await expect(page.getByText('รอบถัดไป', { exact: true })).toBeVisible();
	state.releaseVisibility();
	await expect(page.getByText('แสดงรอบบน portal แล้ว', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'ซ่อนอยู่' })).toBeVisible();
});

test('creating a subject patches its returned resource without rereading the whole region', async ({
	page
}) => {
	const state = await mock(page, true);
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText('คณิตศาสตร์', { exact: true })).toBeVisible();
	state.failSubjectRefresh();
	await page.getByRole('button', { name: 'เพิ่ม', exact: true }).nth(1).click();
	await page.locator('#sub-name').fill('ภาษาไทย');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).first().click();
	await expect(page.getByText('ภาษาไทย', { exact: true })).toBeVisible();
	expect(
		state.reads.filter((path) => path === `/api/admission/rounds/${roundId}/subjects`)
	).toHaveLength(1);
});

test('a late initial subject read cannot erase a subject created while it was pending', async ({
	page
}) => {
	const state = await mock(page, true);
	state.holdSubjects();
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	await expect
		.poll(
			() =>
				state.reads.filter((path) => path === `/api/admission/rounds/${roundId}/subjects`).length
		)
		.toBe(1);
	await page.getByRole('button', { name: 'เพิ่ม', exact: true }).nth(1).click();
	await page.locator('#sub-name').fill('ภาษาไทย');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).first().click();
	await expect(page.getByText('ภาษาไทย', { exact: true })).toBeVisible();
	const staleResponse = page.waitForResponse(
		(response) =>
			new URL(response.url()).pathname === `/api/admission/rounds/${roundId}/subjects` &&
			response.request().method() === 'GET'
	);
	state.releaseSubjects();
	await staleResponse;
	await page.evaluate(() => new Promise(requestAnimationFrame));
	await expect(page.getByText('ภาษาไทย', { exact: true })).toBeVisible();
});

test('deleting a subject removes only that row without rereading the region', async ({ page }) => {
	const state = await mock(page, true);
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	const subject = page.getByText('คณิตศาสตร์', { exact: true });
	await expect(subject).toBeVisible();
	await subject.locator('xpath=../..').getByRole('button').last().click();
	await page.getByRole('dialog').getByRole('button', { name: 'ลบวิชา' }).click();
	await expect(subject).toHaveCount(0);
	expect(
		state.reads.filter((path) => path === `/api/admission/rounds/${roundId}/subjects`)
	).toHaveLength(1);
});

test('round deletion confirmation states the application and document loss', async ({ page }) => {
	await mock(page, true);
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลบรอบ', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText('ใบสมัคร 2 รายการ');
	await expect(page.getByRole('dialog')).toContainText('เอกสารที่เกี่ยวข้อง');
	await expect(
		page.getByRole('dialog').getByRole('button', { name: 'ลบรอบและใบสมัครทั้งหมด' })
	).toBeVisible();
});

test('round-list deletion names the target and makes cascading loss explicit', async ({ page }) => {
	await mock(page, true);
	await page.goto(`/staff/academic/admission?academicYearId=${yearId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: `ลบรอบ ${round.name}` }).click();
	await expect(page.getByRole('dialog')).toContainText('ใบสมัคร 2 รายการ');
	await expect(page.getByRole('dialog')).toContainText('เอกสารที่เกี่ยวข้อง');
});

test('saved zone settings do not load the large school directory until its picker opens', async ({
	page
}) => {
	const largeSchoolAssets: string[] = [];
	page.on('response', async (response) => {
		const url = response.url();
		if (!url.includes('thai-schools') && !url.includes('/_app/immutable/chunks/')) return;
		const body = await response.body().catch(() => new Uint8Array());
		if (url.includes('thai-schools') || body.byteLength > 2_000_000) largeSchoolAssets.push(url);
	});
	await mock(page, true, 'zone');
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	const schoolPicker = page.getByRole('combobox').filter({ hasText: 'ค้นหาชื่อโรงเรียน...' });
	await expect(schoolPicker).toBeVisible();
	await page.waitForLoadState('networkidle');
	expect(largeSchoolAssets).toHaveLength(0);
	await schoolPicker.click();
	await expect.poll(() => largeSchoolAssets.length).toBe(1);
});

test('an opened school picker shows loading while its directory is in flight', async ({ page }) => {
	let releaseSchool = () => {};
	const waitSchool = new Promise<void>((resolve) => (releaseSchool = resolve));
	await mock(page, true, 'zone');
	await page.goto(`/staff/academic/admission/${roundId}`, { waitUntil: 'domcontentloaded' });
	const picker = page.getByRole('combobox').filter({ hasText: 'ค้นหาชื่อโรงเรียน...' });
	await expect(picker).toBeVisible();
	await page.waitForLoadState('networkidle');
	let heldSchoolAsset = false;
	await page.route(/\/_app\/immutable\/chunks\/.*\.js$/, async (route) => {
		const response = await route.fetch();
		const body = await response.body();
		if (body.byteLength > 2_000_000) {
			heldSchoolAsset = true;
			await waitSchool;
		}
		await route.fulfill({ response });
	});
	await picker.click();
	await page.getByPlaceholder('พิมพ์ชื่อโรงเรียน...').fill('กรุงเทพ');
	await expect.poll(() => heldSchoolAsset).toBe(true);
	await expect(page.getByText('กำลังโหลดรายชื่อโรงเรียน...')).toBeVisible();
	releaseSchool();
	await expect(page.getByText('กำลังโหลดรายชื่อโรงเรียน...')).toHaveCount(0);
});

test('create form starts year options before dependent grade choices', async ({ page }) => {
	const state = await mock(page, true);
	state.holdYears();
	await page.goto(`/staff/academic/admission/new?academicYearId=${yearId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	expect(state.reads).not.toContain('/api/lookup/grade-levels');
	state.releaseYears();
	await expect(page.getByText('ข้อมูลรอบรับสมัคร', { exact: true })).toBeVisible();
	await expect
		.poll(() => state.reads.filter((path) => path === '/api/lookup/grade-levels').length)
		.toBe(1);
});

test('creating a round from a selected year keeps that year selected', async ({ page }) => {
	await mock(page, true);
	await page.goto(`/staff/academic/admission?academicYearId=${nextYearId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText('รอบปีใหม่', { exact: true })).toBeVisible();
	await page.getByRole('link', { name: 'สร้างรอบรับสมัครใหม่' }).click();
	await expect(page.locator('#year-select')).toContainText('2570');
});

test('changing the create form year requests only that year’s grade choices', async ({ page }) => {
	const state = await mock(page, true);
	await page.goto(`/staff/academic/admission/new?academicYearId=${yearId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.locator('#year-select')).toContainText('2569');
	await expect.poll(() => state.gradeYears).toEqual([yearId]);
	await page.locator('#year-select').click();
	await page.getByRole('option', { name: /2570/ }).click();
	await expect(page.locator('#year-select')).toContainText('2570');
	await expect.poll(() => state.gradeYears).toEqual([yearId, nextYearId]);
});

test('late grade choices from an old year cannot overwrite the newly selected year', async ({
	page
}) => {
	const state = await mock(page, true);
	state.holdGrades();
	await page.goto(`/staff/academic/admission/new?academicYearId=${yearId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.locator('#year-select')).toContainText('2569');
	await expect(page.locator('#grade-select')).toContainText('กำลังโหลด...');
	await page.locator('#year-select').click();
	await page.getByRole('option', { name: /2570/ }).click();
	await expect(page.locator('#year-select')).toContainText('2570');
	await expect(page.locator('#grade-select')).not.toContainText('กำลังโหลด...');
	state.releaseGrades();
	await page.locator('#grade-select').click();
	await expect(page.getByRole('option', { name: /ม\.2/ })).toBeVisible();
	await expect(page.getByRole('option', { name: /ม\.1/ })).toHaveCount(0);
});

test('same-page year navigation clears a grade chosen under the previous year', async ({
	page
}) => {
	await mock(page, true);
	await page.goto(`/staff/academic/admission/new?academicYearId=${yearId}`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.locator('#grade-select')).not.toContainText('กำลังโหลด...');
	await page.locator('#grade-select').click();
	await page.getByRole('option', { name: /ม\.1/ }).click();
	await expect(page.locator('#grade-select')).toContainText('ม.1');
	await page.evaluate((year) => {
		const link = document.createElement('a');
		link.href = `/staff/academic/admission/new?academicYearId=${year}`;
		link.textContent = 'new year';
		document.body.append(link);
		link.click();
		link.remove();
	}, nextYearId);
	await expect(page.locator('#year-select')).toContainText('2570');
	await expect(page.locator('#grade-select')).not.toContainText('ม.1');
	await expect(page.locator('#grade-select')).toHaveAttribute('data-placeholder', '');
});
