import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
test.describe.configure({ mode: 'default' });

const id = (n: number) => `32000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
const roundId = id(1);
const otherRoundId = id(2);
const yearId = id(3);
const firstTrackId = id(4);
const secondTrackId = id(5);
const subjectId = id(6);
const roomId = id(7);
const appId = id(8);
const path = `/staff/academic/admission/${roundId}/selections`;

const tracks = [
	{ id: firstTrackId, admissionRoundId: roundId, name: 'สายวิทย์' },
	{ id: secondTrackId, admissionRoundId: roundId, name: 'สายศิลป์' }
];
const subjects = [{ id: subjectId, admissionRoundId: roundId, name: 'คณิตศาสตร์', maxScore: 100 }];
const rooms = [{ roomId, roomName: 'ห้องเรียน A', capacity: 2 }];
const roomSummary = { ...rooms[0], studentCount: 1, maleCount: 1, femaleCount: 0 };

function ranking(trackId: string) {
	return {
		trackId,
		trackName: trackId === firstTrackId ? 'สายวิทย์' : 'สายศิลป์',
		rooms: [roomSummary],
		applications: [
			{
				applicationId: appId,
				applicationNumber: '6900001',
				fullName: trackId === firstTrackId ? 'นักเรียน วิทย์' : 'นักเรียน ศิลป์',
				selectionScore: 80,
				totalScore: 80,
				selectionRank: 1,
				finalRank: 1,
				assignedRoom: 'ห้องเรียน A',
				assignedRoomId: roomId,
				roomSaved: false,
				isOverflow: false,
				isTrackOverridden: false,
				gender: 'male'
			}
		]
	};
}

const globalRanking = {
	rooms: [roomSummary],
	applications: [
		{
			applicationId: appId,
			applicationNumber: '6900001',
			fullName: 'นักเรียน รวม',
			totalScore: 80,
			globalRank: 1,
			rankInRoom: 1,
			assignedRoom: 'ห้องเรียน A',
			assignedRoomId: roomId,
			roomSaved: true,
			isOverflow: false,
			originalTrackName: 'สายวิทย์',
			gender: 'male'
		}
	]
};

async function reply(route: Route, data: unknown, status = 200) {
	await route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

async function mock(page: Page, options: { canScore?: boolean; globalDefault?: boolean } = {}) {
	const reads: string[] = [];
	const writes: string[] = [];
	let holdSubjects = false;
	let releaseSubjects = () => {};
	const subjectGate = new Promise<void>((resolve) => (releaseSubjects = resolve));
	let holdFirstRanking = false;
	let releaseFirstRanking = () => {};
	const rankingGate = new Promise<void>((resolve) => (releaseFirstRanking = resolve));
	let holdRooms = false;
	let releaseRooms = () => {};
	const roomsGate = new Promise<void>((resolve) => (releaseRooms = resolve));
	let failRankingOnce = false;
	let failSubjectsOnce = false;
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const resource = url.pathname;
			const method = route.request().method();
			if (method === 'GET') reads.push(`${resource}${url.search}`);
			else writes.push(`${method} ${resource}`);
			if (resource === '/api/auth/me')
				return reply(route, {
					id: id(20),
					username: 'E2E-selections',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions:
						options.canScore === false ? [] : ['admission.scores.all', 'admission.read.all']
				});
			if (resource === '/api/academic/context/options')
				return reply(route, {
					activeAcademicYearId: null,
					activeAcademicTermId: null,
					years: [],
					terms: []
				});
			const activeRound = resource.includes(otherRoundId) ? otherRoundId : roundId;
			if (resource === `/api/admission/rounds/${activeRound}` && method === 'GET')
				return reply(route, {
					id: activeRound,
					academicYearId: yearId,
					gradeLevelId: id(21),
					name: activeRound === roundId ? 'รอบคัดเลือก' : 'รอบคัดเลือกใหม่',
					applyStartDate: '2026-05-01',
					applyEndDate: '2026-05-31',
					status: 'draft',
					isVisible: false,
					createdAt: '2026-04-01T00:00:00Z',
					updatedAt: '2026-04-01T00:00:00Z',
					selectionSettings: {
						assignmentMode: options.globalDefault ? 'global' : 'per_track',
						method: 'sequential'
					}
				});
			if (resource === `/api/admission/rounds/${activeRound}/tracks` && method === 'GET')
				return reply(route, tracks);
			if (resource === `/api/admission/rounds/${activeRound}/subjects` && method === 'GET') {
				if (holdSubjects) await subjectGate;
				if (failSubjectsOnce) {
					failSubjectsOnce = false;
					return reply(route, 'วิชาไม่พร้อม', 503);
				}
				return reply(route, subjects);
			}
			if (resource === `/api/admission/tracks/${firstTrackId}/ranking` && method === 'GET') {
				if (holdFirstRanking) await rankingGate;
				if (failRankingOnce) {
					failRankingOnce = false;
					return reply(route, 'ผลเรียงคะแนนไม่พร้อม', 503);
				}
				return reply(route, ranking(firstTrackId));
			}
			if (resource === `/api/admission/tracks/${secondTrackId}/ranking` && method === 'GET')
				return reply(route, ranking(secondTrackId));
			if (resource === `/api/admission/rounds/${activeRound}/global-ranking` && method === 'GET')
				return reply(route, globalRanking);
			if (resource === `/api/admission/rounds/${activeRound}/rooms` && method === 'GET') {
				if (holdRooms) await roomsGate;
				return reply(route, rooms);
			}
			if (resource === `/api/admission/rounds/${activeRound}/assign-rooms` && method === 'POST')
				return reply(route, { assignedCount: 1 });
			if (
				resource === `/api/admission/rounds/${activeRound}/selection-settings` &&
				method === 'PATCH'
			)
				return reply(route, {});
			return reply(route, 'unmocked', 404);
		}
	);
	return {
		reads,
		writes,
		holdSubjects: () => (holdSubjects = true),
		releaseSubjects: () => releaseSubjects(),
		holdFirstRanking: () => (holdFirstRanking = true),
		releaseFirstRanking: () => releaseFirstRanking(),
		holdRooms: () => (holdRooms = true),
		releaseRooms: () => releaseRooms(),
		failRankingOnce: () => (failRankingOnce = true),
		failSubjectsOnce: () => (failSubjectsOnce = true)
	};
}

test('slow subjects show a focused skeleton while round and tracks are usable', async ({
	page
}) => {
	const state = await mock(page);
	state.holdSubjects();
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByRole('button', { name: /สายวิทย์/ })).toBeVisible();
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	expect(state.reads.some((read) => read.includes(`/tracks/${firstTrackId}/ranking`))).toBe(false);
	state.releaseSubjects();
	await expect(page.getByText('นักเรียน วิทย์', { exact: true })).toBeVisible();
});

test('saved global mode renders ranking while the independent room order is delayed', async ({
	page
}) => {
	const state = await mock(page, { globalDefault: true });
	state.holdRooms();
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText('นักเรียน รวม', { exact: true })).toBeVisible();
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	expect(state.reads.some((read) => read.includes('/tracks/') && read.includes('/ranking'))).toBe(
		false
	);
	state.releaseRooms();
	await expect(page.getByText('ลาก-วางเพื่อกำหนดว่าห้องไหนได้นักเรียนคะแนนสูงก่อน')).toBeVisible();
});

test('switching tracks supersedes a slow old ranking', async ({ page }) => {
	const state = await mock(page);
	state.holdFirstRanking();
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByRole('button', { name: /สายศิลป์/ })).toBeVisible();
	await page.getByRole('button', { name: /สายศิลป์/ }).click();
	await expect(page.getByText('นักเรียน ศิลป์', { exact: true })).toBeVisible();
	state.releaseFirstRanking();
	await expect(page.getByText('นักเรียน วิทย์', { exact: true })).toHaveCount(0);
});

test('ranking failure retries only that ranking', async ({ page }) => {
	const state = await mock(page);
	state.failRankingOnce();
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText('โหลดผลเรียงคะแนนไม่สำเร็จ', { exact: true })).toBeVisible();
	const roundReads = state.reads.filter((read) => read.endsWith(`/rounds/${roundId}`)).length;
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('นักเรียน วิทย์', { exact: true })).toBeVisible();
	expect(state.reads.filter((read) => read.endsWith(`/rounds/${roundId}`)).length).toBe(roundReads);
});

test('subject retry recovers the dependent ranking without rereading the round', async ({
	page
}) => {
	const state = await mock(page);
	state.failSubjectsOnce();
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText('โหลดวิชาสอบไม่สำเร็จ', { exact: true })).toBeVisible();
	const roundReads = state.reads.filter((read) => read.endsWith(`/rounds/${roundId}`)).length;
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).first().click();
	await expect(page.getByText('นักเรียน วิทย์', { exact: true })).toBeVisible();
	expect(state.reads.filter((read) => read.endsWith(`/rounds/${roundId}`)).length).toBe(roundReads);
});

test('changing mode loads global regions on demand and reuses the track result', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText('นักเรียน วิทย์', { exact: true })).toBeVisible();
	expect(state.reads.some((read) => read.includes('/global-ranking'))).toBe(false);
	await page
		.getByRole('button', { name: /รวมทุกคน/ })
		.first()
		.click();
	await expect(page.getByText('นักเรียน รวม', { exact: true })).toBeVisible();
	expect(state.reads.filter((read) => read.includes('/global-ranking')).length).toBe(1);
	const trackReads = state.reads.filter((read) =>
		read.includes(`/tracks/${firstTrackId}/ranking`)
	).length;
	await page
		.getByRole('button', { name: /แยกตามสาย/ })
		.first()
		.click();
	await expect(page.getByText('นักเรียน วิทย์', { exact: true })).toBeVisible();
	expect(
		state.reads.filter((read) => read.includes(`/tracks/${firstTrackId}/ranking`)).length
	).toBe(trackReads);
});

test('assigning a track refreshes its ranking without rereading page context', async ({ page }) => {
	const state = await mock(page);
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText('นักเรียน วิทย์', { exact: true })).toBeVisible();
	const isBaseRead = (read: string) =>
		read === `/api/admission/rounds/${roundId}` ||
		read === `/api/admission/rounds/${roundId}/tracks` ||
		read === `/api/admission/rounds/${roundId}/subjects`;
	const baseReads = state.reads.filter(isBaseRead).length;
	await page.getByRole('button', { name: 'บันทึกจัดห้อง' }).click();
	await page.getByRole('button', { name: 'ยืนยัน', exact: true }).click();
	await expect
		.poll(
			() => state.reads.filter((read) => read.includes(`/tracks/${firstTrackId}/ranking`)).length
		)
		.toBe(2);
	expect(state.writes).toContain(`POST /api/admission/rounds/${roundId}/assign-rooms`);
	expect(state.reads.filter(isBaseRead).length).toBe(baseReads);
});

test('denied score permission starts no admission reads', async ({ page }) => {
	const state = await mock(page, { canScore: false });
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText(/ไม่มีสิทธิ์(เข้าถึงหน้านี้|จัดผลคัดเลือก)/).first()).toBeVisible();
	expect(state.reads.filter((read) => read.includes('/api/admission/'))).toEqual([]);
});
