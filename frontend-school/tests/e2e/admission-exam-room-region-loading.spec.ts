import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
test.describe.configure({ mode: 'default' });

const id = (n: number) => `30000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
const roundId = id(1);
const otherRoundId = id(2);
const yearId = id(3);
const roomId = id(4);
const facilityRoomId = id(5);

const round = {
	id: roundId,
	academicYearId: yearId,
	gradeLevelId: id(6),
	name: 'รอบสอบต้นแบบ',
	applyStartDate: '2026-05-01',
	applyEndDate: '2026-05-31',
	status: 'draft',
	isVisible: false,
	createdAt: '2026-04-01T00:00:00Z',
	updatedAt: '2026-04-01T00:00:00Z',
	academicYearName: '2569',
	gradeLevelName: 'ม.1',
	applicationCount: 1
};
const examRoom = {
	id: roomId,
	roomId: facilityRoomId,
	roomName: 'ห้องสอบ A',
	buildingName: 'อาคาร 1',
	capacity: 40,
	displayOrder: 1,
	assignedCount: 1
};
const otherRoom = { ...examRoom, id: id(12), roomName: 'ห้องสอบรอบใหม่' };

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
	let holdRooms = false;
	let failConfigOnce = false;
	let holdSeats = false;
	let failSeatsOnce = false;
	let copiedRooms = false;
	let releaseRooms = () => {};
	let releaseSeats = () => {};
	const roomsGate = new Promise<void>((resolve) => (releaseRooms = resolve));
	const seatsGate = new Promise<void>((resolve) => (releaseSeats = resolve));
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
					username: 'E2E-exam-room',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: manager ? ['admission.manage.all'] : []
				});
			if (path === '/api/academic/context/options')
				return reply(route, {
					activeAcademicYearId: null,
					activeAcademicTermId: null,
					years: [],
					terms: []
				});
			if (path === `/api/admission/rounds/${roundId}` && method === 'GET')
				return reply(route, round);
			if (path === `/api/admission/rounds/${otherRoundId}` && method === 'GET')
				return reply(route, { ...round, id: otherRoundId, name: 'รอบใหม่' });
			if (path === `/api/admission/rounds/${otherRoundId}/exam-rooms` && method === 'GET')
				return reply(route, { rooms: [otherRoom], totalCapacity: 40, totalAssigned: 0 });
			if (path === `/api/admission/rounds/${otherRoundId}/exam-config` && method === 'GET')
				return reply(route, { examIdType: 'application_number', sortOrder: 'by_application' });
			if (path === '/api/notifications/stream')
				return route.fulfill({
					status: 200,
					contentType: 'text/event-stream',
					body: ': connected\n\nretry: 3600000\n\n'
				});
			if (path === `/api/admission/rounds/${otherRoundId}/exam-seats` && method === 'GET')
				return reply(route, [
					{
						examRoomId: otherRoom.id,
						roomName: otherRoom.roomName,
						capacity: 40,
						seats: [
							{
								applicationId: id(13),
								applicationNumber: '6906060300002',
								fullName: 'เด็กหญิง รอบใหม่',
								nationalId: '2222222222222',
								seatNumber: 1
							}
						]
					}
				]);
			if (path === `/api/admission/rounds/${roundId}/exam-rooms` && method === 'GET') {
				if (holdRooms) await roomsGate;
				return reply(
					route,
					copiedRooms
						? { rooms: [otherRoom], totalCapacity: 40, totalAssigned: 0 }
						: { rooms: [examRoom], totalCapacity: 40, totalAssigned: 1 }
				);
			}
			if (path === `/api/admission/rounds/${roundId}/exam-config` && method === 'GET') {
				if (failConfigOnce) {
					failConfigOnce = false;
					return reply(route, 'การตั้งค่าไม่พร้อม', 503);
				}
				return reply(route, {
					examIdType: 'sequential',
					examIdPrefix: '',
					sortOrder: 'by_application'
				});
			}
			if (path === `/api/admission/rounds/${roundId}/exam-seats` && method === 'GET') {
				if (holdSeats) await seatsGate;
				if (copiedRooms) return reply(route, []);
				if (failSeatsOnce) {
					failSeatsOnce = false;
					return reply(route, 'ที่นั่งไม่พร้อม', 503);
				}
				return reply(route, [
					{
						examRoomId: roomId,
						roomName: 'ห้องสอบ A',
						buildingName: 'อาคาร 1',
						capacity: 40,
						seats: [
							{
								applicationId: id(10),
								applicationNumber: '6906060300001',
								fullName: 'เด็กชาย ทดสอบ',
								nationalId: '1111111111111',
								trackName: 'สาย A',
								seatNumber: 1,
								examId: '1'
							}
						]
					}
				]);
			}
			if (path === '/api/facilities/rooms' && method === 'GET')
				return reply(route, [
					{ id: facilityRoomId, name_th: 'ห้องอาคาร', code: 'A-1', capacity: 40, status: 'ACTIVE' }
				]);
			if (path === '/api/admission/rounds' && method === 'GET')
				return reply(route, [{ ...round, id: otherRoundId, name: 'รอบต้นทาง' }]);
			if (path === `/api/admission/rounds/${roundId}/exam-rooms` && method === 'POST')
				return reply(route, {
					...examRoom,
					id: id(11),
					roomName: 'ห้องอาคาร',
					displayOrder: 2,
					assignedCount: 0
				});
			if (path === `/api/admission/rounds/${roundId}/exam-rooms/${roomId}` && method === 'PUT')
				return reply(route, {
					...examRoom,
					capacity: Number(route.request().postDataJSON().capacityOverride)
				});
			if (
				path === `/api/admission/rounds/${roundId}/exam-rooms/copy-from/${otherRoundId}` &&
				method === 'POST'
			) {
				copiedRooms = true;
				return reply(route, { rooms: [otherRoom], totalCapacity: 40, totalAssigned: 0 });
			}
			if (path === `/api/admission/rounds/${roundId}/exam-config` && method === 'PUT')
				return reply(route, {});
			if (path === `/api/admission/rounds/${roundId}/assign-exam-seats` && method === 'POST')
				return reply(route, { assignedCount: 1, rooms: [{ roomName: 'ห้องสอบ A', count: 1 }] });
			return reply(route, 'unmocked', 404);
		}
	);
	return {
		reads,
		writes,
		holdRooms: () => (holdRooms = true),
		releaseRooms: () => releaseRooms(),
		failConfigOnce: () => (failConfigOnce = true),
		holdSeats: () => (holdSeats = true),
		releaseSeats: () => releaseSeats(),
		failSeatsOnce: () => (failSeatsOnce = true)
	};
}

test('room list cannot hold back the round and config regions', async ({ page }) => {
	const state = await mock(page);
	state.holdRooms();
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'ลำดับต่อเนื่อง' })).toBeVisible();
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	expect(state.reads).toContain(`/api/admission/rounds/${roundId}/exam-rooms`);
	expect(state.reads).not.toContain(`/api/admission/rounds/${roundId}/exam-seats`);
	expect(state.reads).not.toContain('/api/facilities/rooms');
	expect(state.reads).not.toContain('/api/admission/rounds');
	state.releaseRooms();
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
});

test('config failure retries only config while room list stays usable', async ({ page }) => {
	const state = await mock(page);
	state.failConfigOnce();
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
	await expect(page.getByText('โหลดการตั้งค่าที่นั่งไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByRole('button', { name: 'ลำดับต่อเนื่อง' })).toBeVisible();
	expect(state.reads.filter((path) => path.endsWith('/exam-rooms'))).toHaveLength(1);
	expect(state.reads.filter((path) => path.endsWith('/exam-config'))).toHaveLength(2);
});

test('facility choices load only after opening add-room workflow', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
	expect(state.reads).not.toContain('/api/facilities/rooms');
	await page.getByRole('button', { name: 'เพิ่มห้อง', exact: true }).click();
	await expect(page.getByRole('dialog')).toBeVisible();
	await expect
		.poll(() => state.reads.filter((path) => path === '/api/facilities/rooms').length)
		.toBe(1);
	await page.getByRole('dialog').getByRole('button', { name: '— เลือกห้อง —' }).click();
	await expect(page.getByRole('option', { name: /ห้องอาคาร/ })).toBeVisible();
});

test('adding a facility room patches only the room region', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'เพิ่มห้อง', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: '— เลือกห้อง —' }).click();
	await page.getByRole('option', { name: /ห้องอาคาร/ }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'เพิ่มห้อง' }).click();
	await expect(page.getByText('ห้องอาคาร', { exact: true })).toBeVisible();
	expect(state.writes).toContain(`POST /api/admission/rounds/${roundId}/exam-rooms`);
	expect(state.reads.filter((path) => path.endsWith('/exam-rooms'))).toHaveLength(1);
});

test('capacity edit patches previously loaded seat-group capacity without another GET', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ผลจัดที่นั่ง' }).click();
	await expect(page.getByText('1/40')).toBeVisible();
	await page.getByRole('button', { name: 'ตั้งค่าห้องสอบ' }).click();
	await page.getByRole('button', { name: '40', exact: true }).click();
	await page.getByRole('spinbutton').fill('50');
	await page.getByRole('spinbutton').press('Enter');
	await expect(page.getByRole('button', { name: '50', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ผลจัดที่นั่ง' }).click();
	await expect(page.getByText('1/50')).toBeVisible();
	expect(state.reads.filter((path) => path.endsWith('/exam-seats'))).toHaveLength(1);
});

test('room refresh retains usable rows and only rereads rooms', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
	state.holdRooms();
	await page.getByRole('button', { name: 'โหลดห้องสอบใหม่' }).click();
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
	await expect(page.getByText('กำลังอัปเดตห้องสอบ...')).toBeVisible();
	state.releaseRooms();
	await expect
		.poll(() => state.reads.filter((path) => path.endsWith('/exam-rooms')).length)
		.toBe(2);
	expect(state.reads.filter((path) => path.endsWith('/exam-config'))).toHaveLength(1);
});

test('saving config does not reread the room list or config', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByRole('button', { name: 'ลำดับต่อเนื่อง' })).toBeVisible();
	await page.getByRole('button', { name: 'บันทึก config' }).click();
	await expect
		.poll(() => state.writes.filter((path) => path.endsWith('/exam-config')).length)
		.toBe(1);
	expect(state.reads.filter((path) => path.endsWith('/exam-rooms'))).toHaveLength(1);
	expect(state.reads.filter((path) => path.endsWith('/exam-config'))).toHaveLength(1);
});

test('assigning seats patches room counts from seats without rereading setup', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'ลำดับต่อเนื่อง' })).toBeVisible();
	await page.getByRole('button', { name: 'จัดใหม่ทั้งหมด' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'ยืนยันจัดที่นั่ง' }).click();
	await expect(page.getByText('เด็กชาย ทดสอบ')).toBeVisible();
	expect(state.writes).toContain(`POST /api/admission/rounds/${roundId}/assign-exam-seats`);
	expect(state.reads.filter((path) => path.endsWith('/exam-rooms'))).toHaveLength(1);
	expect(state.reads.filter((path) => path.endsWith('/exam-seats'))).toHaveLength(1);
	expect(state.reads.filter((path) => path.endsWith('/exam-config'))).toHaveLength(1);
	expect(state.reads.filter((path) => path === `/api/admission/rounds/${roundId}`)).toHaveLength(1);
});

test('copy-round choices and decrypted seat rows stay interaction-lazy', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
	expect(state.reads).not.toContain('/api/admission/rounds');
	expect(state.reads).not.toContain(`/api/admission/rounds/${roundId}/exam-seats`);
	await page.getByRole('button', { name: '— เลือกรอบ —' }).click();
	await expect
		.poll(() => state.reads.filter((path) => path === '/api/admission/rounds').length)
		.toBe(1);
	await page.keyboard.press('Escape');
	await page.getByRole('button', { name: 'ผลจัดที่นั่ง' }).click();
	await expect(page.getByText('เด็กชาย ทดสอบ')).toBeVisible();
	expect(state.reads.filter((path) => path.endsWith('/exam-seats'))).toHaveLength(1);
});

test('copying rooms invalidates old seat results until the tab reloads them', async ({ page }) => {
	const state = await mock(page);
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ผลจัดที่นั่ง' }).click();
	await expect(page.getByText('เด็กชาย ทดสอบ')).toBeVisible();
	await page.getByRole('button', { name: 'ตั้งค่าห้องสอบ' }).click();
	await page.getByRole('button', { name: '— เลือกรอบ —' }).click();
	await page.getByRole('option', { name: 'รอบต้นทาง' }).click();
	await page.getByRole('button', { name: 'Copy ห้องสอบ (แทนที่ของเดิม)' }).click();
	await expect(page.getByText(otherRoom.roomName, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ผลจัดที่นั่ง' }).click();
	await expect(page.getByText('ยังไม่มีผลจัดที่นั่ง')).toBeVisible();
	await expect(page.getByText('เด็กชาย ทดสอบ')).toHaveCount(0);
	expect(state.reads.filter((path) => path.endsWith('/exam-seats'))).toHaveLength(2);
});

test('seat-region failure offers local retry without refetching setup', async ({ page }) => {
	const state = await mock(page);
	state.failSeatsOnce();
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ผลจัดที่นั่ง' }).click();
	await expect(page.getByText('โหลดผลจัดที่นั่งไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('เด็กชาย ทดสอบ')).toBeVisible();
	expect(state.reads.filter((path) => path.endsWith('/exam-rooms'))).toHaveLength(1);
});

test('an old round response cannot replace the newly selected round', async ({ page }) => {
	const state = await mock(page);
	state.holdRooms();
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(round.name, { exact: true })).toBeVisible();
	await page.evaluate((href) => {
		const link = document.createElement('a');
		link.href = href;
		document.body.appendChild(link);
		link.click();
	}, `/staff/academic/admission/${otherRoundId}/exam-rooms`);
	await expect(page.getByText(otherRoom.roomName, { exact: true })).toBeVisible();
	state.releaseRooms();
	await expect(page.getByText(examRoom.roomName, { exact: true })).toHaveCount(0);
});

test('a pending old seat read cannot block opening seats for the next round', async ({ page }) => {
	const state = await mock(page);
	state.holdSeats();
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect(page.getByText(examRoom.roomName, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ผลจัดที่นั่ง' }).click();
	await expect
		.poll(
			() =>
				state.reads.filter((path) => path === `/api/admission/rounds/${roundId}/exam-seats`).length
		)
		.toBe(1);
	await page.evaluate((href) => {
		const link = document.createElement('a');
		link.href = href;
		document.body.appendChild(link);
		link.click();
	}, `/staff/academic/admission/${otherRoundId}/exam-rooms`);
	await expect(page.getByText(otherRoom.roomName, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ผลจัดที่นั่ง' }).click();
	await expect(page.getByText('เด็กหญิง รอบใหม่')).toBeVisible();
	state.releaseSeats();
	await expect(page.getByText('เด็กชาย ทดสอบ')).toHaveCount(0);
});

test('without manage permission, no exam room data is requested', async ({ page }) => {
	const state = await mock(page, false);
	await page.goto(`/staff/academic/admission/${roundId}/exam-rooms`, {
		waitUntil: 'domcontentloaded'
	});
	await expect
		.poll(() => state.reads.filter((path) => path.startsWith('/api/admission/')).length)
		.toBe(0);
});
