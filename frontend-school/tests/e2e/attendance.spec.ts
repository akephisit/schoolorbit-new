import { expect, test, type Page, type Route } from '@playwright/test';
import { mockStaffHome, id, year, actor } from './fixtures/staff-home-route-data';
import type {
	AttendanceDetail,
	AttendanceSettings,
	AttendanceSession,
	AttendanceWorkspace,
	AttendanceOptions,
	AttendanceScan,
	SaveAttendanceResults
} from '../../src/lib/api/attendance';
const term = id(200),
	room = id(201),
	student = id(202),
	second = id(203),
	sessionId = id(204),
	deviceId = id(205),
	fileId = id(206),
	date = '2026-10-09';
const reply = (route: Route, data: unknown, status = 200) =>
	route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
const path = (suffix = '') =>
	`/staff/attendance${suffix}?academicYearId=${year}&academicTermId=${term}&date=${date}`;
const configuration = {
	enabled: true,
	weekdays: [1, 2, 3, 4, 5],
	lateAfter: '08:00:00',
	digestTimes: ['16:00:00'],
	evidenceDays: 30,
	faceDistance: 0.45,
	faceMargin: 0.1,
	activityCountsAsPresent: true
};
const settings: AttendanceSettings = {
	academicTermId: term,
	configuration,
	rowVersion: 1,
	archived: false
};
const session: AttendanceSession = {
	id: sessionId,
	academicTermId: term,
	date,
	kind: 'flag',
	sourceKey: room,
	title: 'หน้าเสาธง ม.1/1',
	teacherIds: [actor],
	homeroomId: room,
	learningGroupId: null,
	offeringId: null,
	specialRoundId: null,
	startTime: '08:00:00',
	endTime: '08:15:00',
	countOverride: null,
	cancelled: false,
	cancellationReason: null,
	savedAt: null,
	savedBy: null,
	rowVersion: 1
};
function detail(): AttendanceDetail {
	return {
		session: { ...session },
		counted: true,
		writable: true,
		students: [student, second].map((studentId, i) => ({
			studentId,
			studentAcademicYearId: id(210 + i),
			displayName: i ? 'นักเรียนคนที่สอง' : 'นักเรียนคนแรก',
			classNumber: i + 1,
			result: 'unchecked',
			origin: 'none',
			note: '',
			observedAt: null,
			evidenceFileId: null,
			arrivalAt: null,
			rowVersion: 1
		}))
	};
}
const options: AttendanceOptions = {
	students: [student, second].map((id, i) => ({
		id,
		name: i ? 'นักเรียนคนที่สอง' : 'นักเรียนคนแรก',
		homeroomId: room,
		homeroomName: 'ม.1/1'
	})),
	teachers: [{ id: actor, name: 'ครูผู้รับผิดชอบ' }],
	audiences: [],
	specials: [],
	devices: [{ id: deviceId, name: 'เว็บแคมหน้าโรงเรียน', enabled: true, operatorId: actor }]
};
async function mockAttendance(page: Page, { holdScan = false, conflict = false } = {}) {
	await mockStaffHome(page, {
		permissions: [
			'attendance.read.assigned',
			'attendance.update.assigned',
			'attendance.manage.school',
			'attendance.enroll.school',
			'attendance.verify.assigned',
			'attendance.delete.school',
			'academic_context.read.school'
		]
	});
	await page.clock.setFixedTime(new Date('2026-10-09T01:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (r) =>
		r.fulfill({ contentType: 'text/css', body: '' })
	);
	await page.route('**/api/academic/context/options', (r) =>
		reply(r, {
			activeAcademicYearId: year,
			activeAcademicTermId: term,
			years: [
				{
					id: year,
					name: '2569',
					year: 2569,
					status: 'active',
					startDate: '2026-05-01',
					plannedEndDate: '2027-04-30',
					closedOn: null
				}
			],
			terms: [
				{
					id: term,
					academicYearId: year,
					code: '1',
					name: 'ภาคเรียนที่ 1',
					sequence: 1,
					termType: 'regular',
					status: 'active',
					startDate: '2026-05-01',
					plannedEndDate: '2026-10-31',
					closedOn: null,
					includedInYearResult: true,
					blocksYearClosure: true
				}
			]
		})
	);
	let current = detail(),
		savedSettings = structuredClone(settings);
	const reads: string[] = [],
		writes: { path: string; data: unknown }[] = [];
	let release = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	await page.route(
		(url) => url.pathname.startsWith('/api/attendance/') || url.pathname === '/api/files',
		async (route) => {
			const request = route.request(),
				resource = new URL(request.url()).pathname,
				method = request.method();
			if (method === 'GET') reads.push(resource);
			else
				writes.push({
					path: resource,
					data: resource === '/api/files' ? null : request.postDataJSON()
				});
			if (resource.endsWith('/workspace'))
				return reply(route, {
					date,
					counted: true,
					sessions: [current.session],
					settings: savedSettings
				} satisfies AttendanceWorkspace);
			if (resource.endsWith('/options')) return reply(route, options);
			if (resource.includes('/settings/')) {
				if (method === 'PUT')
					savedSettings = {
						...savedSettings,
						...request.postDataJSON(),
						rowVersion: savedSettings.rowVersion + 1
					};
				return reply(route, savedSettings);
			}
			if (resource === '/api/attendance/days') return reply(route, []);
			if (resource.includes('/days/')) {
				savedSettings.rowVersion++;
				return reply(route, savedSettings);
			}
			if (resource.endsWith('/kiosk/open'))
				return reply(route, {
					sessions: [{ ...session, kind: 'arrival' }],
					students: options.students,
					configuration,
					faces: [
						{
							studentId: student,
							model: 'face-api-1.7.15-recognition-128',
							descriptors: [{ values: Array(128).fill(0.01) }],
							rowVersion: 1
						}
					]
				});
			if (resource === '/api/files') return reply(route, { id: fileId });
			if (resource.endsWith('/scans')) {
				if (holdScan) await gate;
				const scan = request.postDataJSON() as AttendanceScan;
				return reply(route, {
					eventId: scan.eventId,
					studentId: scan.studentId,
					result: 'present',
					duplicate: false,
					teacherConflict: false
				});
			}
			if (resource.includes('/sessions/')) {
				if (method === 'PUT') {
					if (conflict) return reply(route, 'มีการสแกนเพิ่มแล้ว กรุณาโหลดรายชื่อใหม่', 409);
					const payload = request.postDataJSON() as SaveAttendanceResults;
					current = {
						...current,
						session: {
							...current.session,
							savedAt: '2026-10-09T01:00:00Z',
							savedBy: actor,
							rowVersion: current.session.rowVersion + 1
						},
						students: current.students.map((s) => ({
							...s,
							result: payload.students.find((r) => r.studentId === s.studentId)?.result ?? 'absent'
						}))
					};
				}
				return reply(route, current);
			}
			return reply(route, {});
		}
	);
	return { reads, writes, release };
}
test.use({ serviceWorkers: 'block' });
test('teacher saves, infers unchecked and patches without rereading workspace', async ({
	page
}) => {
	const api = await mockAttendance(page);
	await page.goto(path());
	await page.getByRole('button', { name: /08:00.*หน้าเสาธง ม.1\/1/ }).click();
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByText('เลือกผลอย่างน้อยหนึ่งคน หรือใช้ปุ่มขาดทั้งหมด')).toBeVisible();
	expect(api.writes).toHaveLength(0);
	await page.getByRole('button', { name: 'ผลของ นักเรียนคนแรก' }).click();
	await page.getByRole('option', { name: 'กิจกรรม', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ผลของ นักเรียนคนที่สอง' })).toHaveText('ขาด');
	expect(api.writes).toHaveLength(1);
	expect(api.reads.filter((p) => p.endsWith('/workspace'))).toHaveLength(1);
	await page.getByRole('button', { name: 'ผลของ นักเรียนคนแรก' }).click();
	await page.getByRole('option', { name: 'สาย', exact: true }).click();
	await page.getByLabel('เหตุผลแก้ไข / งดคาบ').fill('มาถึงหลังตรวจครั้งแรก');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	expect(api.writes).toHaveLength(2);
});
test('scan conflict preserves teacher input', async ({ page }) => {
	await mockAttendance(page, { conflict: true });
	await page.goto(path());
	await page.getByRole('button', { name: /08:00.*หน้าเสาธง/ }).click();
	await page.getByRole('button', { name: 'ผลของ นักเรียนคนแรก' }).click();
	await page.getByRole('option', { name: 'มา', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByText('มีการสแกนเพิ่มแล้ว กรุณาโหลดรายชื่อใหม่')).toBeVisible();
	await expect(page.getByRole('button', { name: 'ผลของ นักเรียนคนแรก' })).toHaveText('มา');
});
test('calendar toggles a single processing date', async ({ page }) => {
	const api = await mockAttendance(page);
	await page.goto(path('/settings'));
	await expect(page.getByRole('heading', { name: 'ปฏิทินวันประมวลผล' })).toBeVisible();
	await page.getByRole('button', { name: '9 ✓', exact: true }).click();
	await expect(page.getByRole('button', { name: '9 —', exact: true })).toBeVisible();
	expect(api.writes.find((w) => w.path.includes('/days/'))?.data).toMatchObject({
		rowVersion: 1,
		days: [{ date, counted: false }]
	});
	expect(api.reads.filter((p) => p.includes('/settings/'))).toHaveLength(1);
});
test('webcam reports success only after accepted scan commits', async ({ page }) => {
	const api = await mockAttendance(page, { holdScan: true });
	await page.addInitScript(() => {
		Math.random = () => 0.1;
	});
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (r) =>
		r.fulfill({
			contentType: 'application/javascript',
			body: `let frame=0;export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};export async function readFace(){return {values:Array(128).fill(.01),yaw:[0,-.12,0][frame++%3]}};export function matchFace(){return '${student}'};export function distance(){return 0};export async function evidence(){return new File(['fixture'],'scan.jpg',{type:'image/jpeg'})};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'เครื่องสแกน' }).click();
	await page.getByRole('option', { name: 'เว็บแคมหน้าโรงเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้' }).click();
	await expect(page.getByRole('status').filter({ hasText: 'กำลังบันทึก…' })).toBeVisible();
	await expect(page.getByText(/นักเรียนคนแรก: บันทึกสำเร็จ/)).toHaveCount(0);
	api.release();
	await expect(page.getByText(/นักเรียนคนแรก: บันทึกสำเร็จ/)).toBeVisible();
	await page.getByRole('button', { name: 'หยุดและปิดกล้อง' }).click();
	expect(api.writes.filter((w) => w.path.endsWith('/scans'))).toHaveLength(1);
});
test('pinned recognition models load and a blank webcam cannot enroll a face', async ({ page }) => {
	test.setTimeout(60_000);
	const api = await mockAttendance(page);
	await page.addInitScript(() => {
		const canvas = document.createElement('canvas');
		canvas.width = 640;
		canvas.height = 480;
		Object.defineProperty(navigator, 'mediaDevices', {
			configurable: true,
			value: {
				getUserMedia: async () => {
					const stream = canvas.captureStream(10);
					const context = canvas.getContext('2d')!;
					setInterval(() => {
						context.fillStyle = 'white';
						context.fillRect(0, 0, 640, 480);
					}, 100);
					return stream;
				}
			}
		});
	});
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await expect(page.getByText('กล้องพร้อมแล้ว', { exact: true })).toBeVisible({ timeout: 30_000 });
	await page.getByRole('button', { name: 'ลงทะเบียนใบหน้า', exact: true }).click();
	await page.getByRole('button', { name: 'นักเรียน', exact: true }).click();
	await page.getByRole('option', { name: 'ม.1/1 · นักเรียนคนแรก', exact: true }).click();
	await page.getByLabel('ยืนยันว่าได้รับความยินยอมและตรวจว่าเป็นนักเรียนคนที่เลือก').check();
	await page.getByRole('button', { name: 'เก็บตัวอย่าง (0/3)', exact: true }).click();
	await expect(page.getByText('ต้องเห็นใบหน้าชัดเจนเพียงคนเดียว', { exact: true })).toBeVisible();
	expect(api.writes).toHaveLength(0);
	await page.getByRole('button', { name: 'หยุดและปิดกล้อง' }).click();
});

test('attendance defaults to the Bangkok school day across UTC midnight', async ({ page }) => {
	await mockAttendance(page);
	await page.clock.setFixedTime(new Date('2026-10-09T19:00:00Z'));
	const url = new URL(path(), 'http://127.0.0.1:4173');
	url.searchParams.delete('date');
	await page.goto(url.pathname + url.search);
	await expect(page.getByRole('button', { name: 'วันที่', exact: true })).toContainText(
		'10 ต.ค. 2569'
	);
});

test('leaving during model loading never opens a webcam after the page closes', async ({
	page
}) => {
	await mockAttendance(page);
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (r) =>
		r.fulfill({
			contentType: 'application/javascript',
			body: `export const FACE_MODEL='face-api-1.7.15-recognition-128';export function faceEngine(){return new Promise(resolve=>window.addEventListener('test-engine-ready',()=>{resolve({});document.documentElement.dataset.engineResolved='yes'}, {once:true}))};export async function camera(){document.documentElement.dataset.cameraOpened='yes';return {getTracks:()=>[]}};export function stopCamera(){};export async function readFace(){return null};export function matchFace(){return null};export function distance(){return 0};export async function evidence(){throw Error('unused')};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('link', { name: 'กลับหน้าเช็คชื่อ', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'เช็คชื่อ', exact: true })).toBeVisible();
	await page.evaluate(() => window.dispatchEvent(new Event('test-engine-ready')));
	await expect(page.locator('html')).toHaveAttribute('data-engine-resolved', 'yes');
	await expect(page.locator('html')).not.toHaveAttribute('data-camera-opened', 'yes');
});
