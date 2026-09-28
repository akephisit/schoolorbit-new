import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'default' });

const id = (n: number) => `31000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
const roundId = id(1);
const secondRoundId = id(2);
const yearId = id(3);
const trackId = id(4);
const nextTrackId = id(5);
const roomId = id(6);
const appId = id(7);
const subjectId = id(8);
const nextSubjectId = id(9);

const round = {
	id: roundId,
	academicYearId: yearId,
	gradeLevelId: id(10),
	name: 'รอบกรอกคะแนน',
	applyStartDate: '2026-05-01',
	applyEndDate: '2026-05-31',
	status: 'draft',
	isVisible: false,
	createdAt: '2026-04-01T00:00:00Z',
	updatedAt: '2026-04-01T00:00:00Z'
};
const tracks = [
	{ id: trackId, admissionRoundId: roundId, name: 'สายวิทย์', applicationCount: 1 },
	{ id: nextTrackId, admissionRoundId: roundId, name: 'สายศิลป์', applicationCount: 0 }
];
const subjects = [
	{ id: subjectId, admissionRoundId: roundId, name: 'คณิตศาสตร์', maxScore: 100 },
	{ id: nextSubjectId, admissionRoundId: roundId, name: 'ภาษาไทย', maxScore: 100 }
];
const roster = [
	{
		examRoomId: roomId,
		roomName: 'ห้องสอบ A',
		buildingName: 'อาคาร 1',
		seats: [
			{
				seatNumber: 1,
				examId: 'A001',
				applicationId: appId,
				applicationNumber: '6900001',
				fullName: 'เด็กชาย ตัวอย่าง'
			}
		]
	}
];
const applications = [
	{
		id: appId,
		applicationNumber: '6900001',
		fullName: 'เด็กชาย ตัวอย่าง',
		admissionTrackId: trackId,
		status: 'verified'
	}
];

async function reply(route: Route, data: unknown, status = 200) {
	await route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

async function mock(page: Page, canScore = true) {
	const reads: string[] = [];
	const writes: string[] = [];
	const bulkPayloads: unknown[] = [];
	let holdRoster = false;
	let releaseRoster = () => {};
	const rosterGate = new Promise<void>((resolve) => (releaseRoster = resolve));
	let holdFirstTrackApps = false;
	let releaseFirstTrackApps = () => {};
	const firstTrackAppsGate = new Promise<void>((resolve) => (releaseFirstTrackApps = resolve));
	let emptyRoster = false;
	let failScoresOnce = false;
	let holdNextScores = false;
	let releaseScores = () => {};
	const scoresGate = new Promise<void>((resolve) => (releaseScores = resolve));
	let scoreValue = 10;
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const path = url.pathname;
			const method = route.request().method();
			if (method === 'GET') reads.push(`${path}${url.search}`);
			else writes.push(`${method} ${path}`);
			if (path === '/api/auth/me')
				return reply(route, {
					id: id(20),
					username: 'E2E-scores',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: canScore ? ['admission.scores.all', 'admission.read.all'] : []
				});
			if (path === '/api/academic/context/options')
				return reply(route, {
					activeAcademicYearId: null,
					activeAcademicTermId: null,
					years: [],
					terms: []
				});
			const activeRound = path.includes(secondRoundId) ? secondRoundId : roundId;
			if (path === `/api/admission/rounds/${activeRound}` && method === 'GET')
				return reply(route, {
					...round,
					id: activeRound,
					name: activeRound === roundId ? round.name : 'รอบกรอกคะแนนใหม่'
				});
			if (path === `/api/admission/rounds/${activeRound}/tracks` && method === 'GET')
				return reply(route, tracks);
			if (path === `/api/admission/rounds/${activeRound}/subjects` && method === 'GET')
				return reply(route, subjects);
			if (path === `/api/admission/rounds/${activeRound}/scores` && method === 'GET') {
				if (holdNextScores) {
					holdNextScores = false;
					await scoresGate;
				}
				if (failScoresOnce) {
					failScoresOnce = false;
					return reply(route, 'คะแนนไม่พร้อม', 503);
				}
				return reply(route, [
					{ applicationId: appId, subjectId, score: scoreValue, status: 'verified' },
					{ applicationId: appId, subjectId: nextSubjectId, score: 20, status: 'verified' }
				]);
			}
			if (path === `/api/admission/rounds/${activeRound}/score-room-roster` && method === 'GET') {
				if (holdRoster && activeRound === roundId) await rosterGate;
				return reply(
					route,
					emptyRoster
						? []
						: activeRound === roundId
							? roster
							: [{ ...roster[0], examRoomId: id(11), roomName: 'ห้องสอบรอบใหม่' }]
				);
			}
			if (path === `/api/admission/rounds/${activeRound}/applications` && method === 'GET') {
				if (holdFirstTrackApps && url.searchParams.get('track_id') === trackId)
					await firstTrackAppsGate;
				return reply(route, url.searchParams.get('track_id') === trackId ? applications : []);
			}
			if (path === `/api/admission/rounds/${activeRound}/scores/bulk` && method === 'PUT') {
				bulkPayloads.push(route.request().postDataJSON());
				return reply(route, { updatedCount: 1 });
			}
			if (path === `/api/admission/applications/${appId}/absent` && method === 'PUT')
				return reply(route, {});
			return reply(route, 'unmocked', 404);
		}
	);
	return {
		reads,
		writes,
		bulkPayloads,
		holdRoster: () => (holdRoster = true),
		releaseRoster: () => releaseRoster(),
		holdFirstTrackApps: () => (holdFirstTrackApps = true),
		releaseFirstTrackApps: () => releaseFirstTrackApps(),
		emptyRoster: () => (emptyRoster = true),
		failScoresOnce: () => (failScoresOnce = true),
		holdNextScores: () => (holdNextScores = true),
		releaseScores: () => releaseScores(),
		setScoreValue: (value: number) => (scoreValue = value)
	};
}

const path = `/staff/academic/admission/${roundId}/scores`;

test('slow room roster does not hide independent round and subject regions', async ({ page }) => {
	const state = await mock(page);
	state.holdRoster();
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText(`${round.name} | 2 วิชา`, { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'โหลดวิชาสอบใหม่' })).toBeVisible();
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	expect(state.reads.some((read) => read.endsWith('/exam-seats'))).toBe(false);
	expect(state.reads.some((read) => read.includes('/applications'))).toBe(false);
	state.releaseRoster();
	await expect(page.getByText('ห้องสอบ A', { exact: true }).first()).toBeVisible();
	await expect(page.getByText('เด็กชาย ตัวอย่าง', { exact: true })).toBeVisible();
});

test('score failure retries only scores while the room view stays available', async ({ page }) => {
	const state = await mock(page);
	state.failScoresOnce();
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText('ห้องสอบ A', { exact: true }).first()).toBeVisible();
	await expect(page.getByText('โหลดคะแนนไม่สำเร็จ', { exact: true })).toBeVisible();
	const rosterReads = state.reads.filter((read) => read.includes('/score-room-roster')).length;
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('เด็กชาย ตัวอย่าง', { exact: true })).toBeVisible();
	expect(state.reads.filter((read) => read.includes('/score-room-roster')).length).toBe(
		rosterReads
	);
});

test('same-round score refresh retains the usable table until the new result arrives', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	const cell = page.locator(`#score-${appId}-${subjectId}`);
	await expect(cell).toHaveValue('10');
	state.setScoreValue(22);
	state.holdNextScores();
	await page.getByRole('button', { name: 'โหลดคะแนนใหม่' }).click();
	await expect(page.getByText('กำลังอัปเดตคะแนน...')).toBeVisible();
	await expect(cell).toHaveValue('10');
	state.releaseScores();
	await expect(cell).toHaveValue('22');
});

test('track application rows remain view-lazy and an empty roster falls back to track', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText('ห้องสอบ A', { exact: true }).first()).toBeVisible();
	expect(state.reads.some((read) => read.includes('/applications'))).toBe(false);
	await page.getByRole('button', { name: 'ตามสาย' }).click();
	await expect(page.getByText('เด็กชาย ตัวอย่าง', { exact: true })).toBeVisible();
	expect(state.reads.filter((read) => read.includes('/applications')).length).toBe(1);

	const emptyPage = await page.context().newPage();
	const emptyState = await mock(emptyPage);
	emptyState.emptyRoster();
	await emptyPage.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(emptyPage.getByRole('button', { name: /สายวิทย์/ })).toBeVisible();
	await expect(emptyPage.getByText('เด็กชาย ตัวอย่าง', { exact: true })).toBeVisible();
	expect(emptyState.reads.some((read) => read.includes('/applications'))).toBe(true);
	await emptyPage.close();
});

test('saving one changed score sends only that cell and does not reread the page', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	const cell = page.locator(`#score-${appId}-${subjectId}`);
	await expect(cell).toHaveValue('10');
	const admissionReadsBefore = state.reads.filter((read) => read.includes('/api/admission/'));
	await cell.fill('35');
	await expect(cell).toHaveValue('35');
	await page.getByRole('button', { name: 'บันทึกคะแนนที่เปลี่ยน (1)' }).click();
	await expect(page.getByRole('button', { name: 'บันทึกคะแนนที่เปลี่ยน (0)' })).toBeDisabled();
	expect(state.bulkPayloads).toEqual([
		{ entries: [{ applicationId: appId, scores: [{ examSubjectId: subjectId, score: 35 }] }] }
	]);
	expect(state.reads.filter((read) => read.includes('/api/admission/'))).toEqual(
		admissionReadsBefore
	);
});

test('clearing one score sends a null-equivalent update instead of silently keeping it', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	const cell = page.locator(`#score-${appId}-${subjectId}`);
	await expect(cell).toHaveValue('10');
	await cell.fill('');
	await page.getByRole('button', { name: 'บันทึกคะแนนที่เปลี่ยน (1)' }).click();
	expect(state.bulkPayloads).toEqual([
		{ entries: [{ applicationId: appId, scores: [{ examSubjectId: subjectId }] }] }
	]);
});

test('a score-denied visitor starts no admission data requests', async ({ page }) => {
	const state = await mock(page, false);
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText(/ไม่มีสิทธิ์(เข้าถึงหน้านี้|กรอกคะแนนสอบ)/).first()).toBeVisible();
	expect(state.reads.filter((read) => read.includes('/api/admission/'))).toEqual([]);
});

test('a slow former track cannot replace the newly selected track', async ({ page }) => {
	const state = await mock(page);
	state.holdFirstTrackApps();
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await expect(page.getByText('ห้องสอบ A', { exact: true }).first()).toBeVisible();
	await page.getByRole('button', { name: 'ตามสาย' }).click();
	await page.getByRole('button', { name: /สายศิลป์/ }).click();
	await expect(page.getByText('ไม่มีผู้สมัครที่พร้อมกรอกคะแนน')).toBeVisible();
	state.releaseFirstTrackApps();
	await expect(page.getByText('ไม่มีผู้สมัครที่พร้อมกรอกคะแนน')).toBeVisible();
	expect(state.reads.some((read) => read.includes(`track_id=${nextTrackId}`))).toBe(true);
});

test('an old room response cannot replace the next round', async ({ page }) => {
	const state = await mock(page);
	state.holdRoster();
	await page.goto(path, { waitUntil: 'domcontentloaded' });
	await page.goto(`/staff/academic/admission/${secondRoundId}/scores`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText('รอบกรอกคะแนนใหม่ | 2 วิชา', { exact: true })).toBeVisible();
	state.releaseRoster();
	await expect(page.getByText('รอบกรอกคะแนนใหม่ | 2 วิชา', { exact: true })).toBeVisible();
	await expect(page.getByText('ห้องสอบรอบใหม่', { exact: true }).first()).toBeVisible();
	await expect(page.getByText('ห้องสอบ A', { exact: true })).toHaveCount(0);
});
