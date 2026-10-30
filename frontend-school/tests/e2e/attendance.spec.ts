import { PERMISSIONS } from '../../src/lib/permissions/registry.js';
import { readFile } from 'node:fs/promises';
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
async function chooseRound(page: Page, name: RegExp = /08:00.*เข้าโรงเรียน/) {
	const selector = page.getByRole('button', { name: 'รอบเช็คชื่อ', exact: true });
	if ((page.viewportSize()?.width ?? 1280) < 1280) {
		await selector.click();
		await page.getByRole('option', { name }).click();
	} else await page.getByRole('button', { name }).click();
}
function resultControl(page: Page, name: string) {
	return page.getByRole('group', { name: `ผลของ ${name}`, exact: true });
}
async function chooseResult(page: Page, name: string, result: string) {
	await resultControl(page, name).getByRole('radio', { name: result, exact: true }).click();
}
async function expectResult(page: Page, name: string, result: string) {
	const control = resultControl(page, name);
	await expect(control).toBeVisible();
	if (result === 'ยังไม่ได้เช็ค')
		await expect(control.getByRole('radio', { checked: true })).toHaveCount(0);
	else
		await expect(control.getByRole('radio', { name: result, exact: true })).toHaveAttribute(
			'aria-checked',
			'true'
		);
}
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

for (const invalidDate of ['bad', '2026-02-30'])
	test(`invalid attendance date ${invalidDate} is repaired without crashing settings`, async ({
		page
	}) => {
		await mockAttendance(page);
		await page.goto(path('/settings').replace(`date=${date}`, `date=${invalidDate}`));
		await expect(page.getByRole('heading', { name: 'ปฏิทินวันประมวลผล' })).toBeVisible();
		await expect(page.getByRole('button', { name: 'เดือน', exact: true })).toHaveText('ตุลาคม');
	});

test('calendar range saves wait for an in-flight month read', async ({ page }) => {
	const api = await mockAttendance(page);
	await page.goto(path('/settings'));
	await expect(page.getByRole('heading', { name: 'ปฏิทินวันประมวลผล' })).toBeVisible();
	let release!: () => void;
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route(
		(url) => url.pathname === '/api/attendance/days',
		async (route) => {
			await held;
			await reply(route, []);
		}
	);
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page.getByRole('button', { name: 'นับช่วงนี้', exact: true })).toBeDisabled();
	await expect(
		page.getByRole('button', { name: 'วันหยุด / ไม่นับช่วงนี้', exact: true })
	).toBeDisabled();
	expect(api.writes).toHaveLength(0);
	release();
	await expect(page.getByRole('button', { name: 'นับช่วงนี้', exact: true })).toBeEnabled();
});

test('parent keeps child context and retries only a failed report', async ({ page }) => {
	await mockAttendance(page, {
		userType: 'parent',
		permissions: ['attendance.read.own']
	});
	await page.route('**/api/parent/profile?**', (route) =>
		reply(route, { children: [{ id: student, first_name: 'นักเรียน', last_name: 'คนแรก' }] })
	);
	let contextReads = 0;
	page.on('request', (request) => {
		if (
			/\/api\/(parent\/(profile|academic-context)|academic\/context)/.test(
				new URL(request.url()).pathname
			)
		)
			contextReads++;
	});
	let failures = 1;
	await page.route(
		(url) => url.pathname === '/api/attendance/report',
		async (route) => {
			if (failures-- > 0) return reply(route, 'รายงานไม่พร้อม', 500);
			await route.fallback();
		}
	);
	await page.goto(`/parent/attendance?academicTermId=${term}`);
	await expect(page.getByRole('button', { name: 'นักเรียน', exact: true })).toBeVisible();
	await expect(page.getByText('รายงานไม่พร้อม', { exact: true })).toBeVisible();
	const before = contextReads;
	await page.getByRole('button', { name: 'ลองโหลดข้อมูลใหม่', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ของนักเรียน', exact: true })).toBeVisible();
	expect(contextReads).toBe(before);
});

for (const role of ['parent', 'student'])
	test(`${role} discards an old identity context failure while replacement context is pending`, async ({
		page
	}) => {
		await mockAttendance(page, {
			userType: role,
			permissions: ['attendance.read.own', 'attendance.read.school']
		});
		await page.route('**/api/parent/profile?**', (route) =>
			reply(route, { children: [{ id: student, first_name: 'นักเรียน', last_name: 'คนแรก' }] })
		);
		let count = 0,
			releaseOld!: () => void,
			releaseNew!: () => void;
		const old = new Promise<void>((resolve) => (releaseOld = resolve));
		const next = new Promise<void>((resolve) => (releaseNew = resolve));
		const endpoint =
			role === 'parent'
				? '/api/parent/academic-context/options'
				: '/api/me/academic-context/options';
		await page.route(
			(url) => url.pathname === endpoint,
			async (route) => {
				if (++count === 1) {
					await old;
					return reply(route, 'ข้อมูลบัญชีเดิมไม่พร้อม', 500);
				}
				await next;
				await route.fallback();
			}
		);
		await page.goto(`/${role}/attendance?academicTermId=${term}`);
		await expect.poll(() => count).toBe(1);
		await page.evaluate(async () => {
			const modulePath = '/src/lib/stores/permissions.ts';
			const { setPermissions } = await import(modulePath);
			setPermissions(['attendance.read.own']);
		});
		await expect.poll(() => count).toBe(2);
		const oldResponse = page.waitForResponse(
			(response) => new URL(response.url()).pathname === endpoint
		);
		releaseOld();
		await (await oldResponse).finished();
		await expect(page.getByText('ข้อมูลบัญชีเดิมไม่พร้อม', { exact: true })).toHaveCount(0);
		releaseNew();
		await expect(page.getByRole('button', { name: 'ของนักเรียน', exact: true })).toBeVisible();
	});

test('permission-scope changes discard the old attendance report and reload', async ({ page }) => {
	await mockAttendance(page, {
		permissions: ['attendance.read.school', 'academic_context.read.school']
	});
	let count = 0,
		release!: () => void;
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route(
		(url) => url.pathname === '/api/attendance/report',
		async (route) => {
			if (++count === 1) return route.fallback();
			await held;
			await reply(route, {
				archived: false,
				activityCountsAsPresent: true,
				summaries: [],
				page: 1,
				pageSize: 50,
				total: 0
			});
		}
	);
	await page.goto(path('/report'));
	await expect(page.getByRole('cell', { name: 'นักเรียนคนที่สอง', exact: true })).toBeVisible();
	await page.evaluate(async () => {
		const modulePath = '/src/lib/stores/permissions.ts';
		const { setPermissions } = await import(modulePath);
		setPermissions(['attendance.read.assigned', 'academic_context.read.school']);
	});
	await expect.poll(() => count).toBe(2);
	await expect(page.getByRole('cell', { name: 'นักเรียนคนที่สอง', exact: true })).toHaveCount(0);
	release();
	await expect(page.getByRole('table')).toBeVisible();
	await expect(page.getByText('ยังไม่มีผลที่นับในสรุป', { exact: true })).toBeVisible();
});
test('a delayed report from the previous permission scope cannot repopulate the page', async ({
	page
}) => {
	await mockAttendance(page, {
		permissions: ['attendance.read.school', 'academic_context.read.school']
	});
	let reads = 0,
		release!: () => void;
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route(
		(url) => url.pathname === '/api/attendance/report',
		async (route) => {
			if (++reads === 1) {
				await held;
				return route.fallback();
			}
			await reply(route, {
				archived: false,
				activityCountsAsPresent: true,
				summaries: [],
				page: 1,
				pageSize: 50,
				total: 0
			});
		}
	);
	await page.goto(path('/report'));
	await expect.poll(() => reads).toBe(1);
	await page.evaluate(async () => {
		const modulePath = '/src/lib/stores/permissions.ts';
		const { setPermissions } = await import(modulePath);
		setPermissions(['attendance.read.assigned', 'academic_context.read.school']);
	});
	await expect(page.getByText('ยังไม่มีผลที่นับในสรุป', { exact: true })).toBeVisible();
	const oldResponse = page.waitForResponse(
		(response) => new URL(response.url()).pathname === '/api/attendance/report'
	);
	release();
	await (await oldResponse).finished();
	await expect
		.poll(() => page.getByRole('cell', { name: 'นักเรียนคนที่สอง', exact: true }).count())
		.toBe(0);
	await expect(page.getByText('ยังไม่มีผลที่นับในสรุป', { exact: true })).toBeVisible();
});

test('revoking scan permission closes the running webcam while enrollment remains allowed', async ({
	page
}) => {
	const api = await mockAttendance(page, {
		permissions: [
			'attendance.verify.assigned',
			'attendance.enroll.assigned',
			'academic_context.read.school'
		]
	});
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (route) =>
		route.fulfill({
			contentType: 'application/javascript',
			body: `export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(stream){if(stream) document.body.dataset.attendanceStopped='yes'};export async function readFace(){return null};export async function readFaces(){const face=await readFace();return {frame:document.createElement('canvas'),capturedAt:new Date().toISOString(),detectedCount:face?1:0,faces:face?[{...face,box:{x:.3,y:.2,width:.2,height:.3}}]:[]}};export function matchFace(){return null};export function distance(){return 0};export async function evidence(){throw new Error('unexpected capture')};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'เครื่องสแกน', exact: true }).click();
	await page.getByRole('option', { name: 'เว็บแคมหน้าโรงเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้', exact: true }).click();
	await expect(
		page.getByRole('status').filter({ hasText: 'ยังไม่เห็นใบหน้าชัดเจน' })
	).toBeVisible();
	await page.evaluate(async () => {
		const modulePath = '/src/lib/stores/permissions.ts';
		const { setPermissions } = await import(modulePath);
		setPermissions(['attendance.enroll.assigned', 'academic_context.read.school']);
	});
	await expect(page.locator('body')).toHaveAttribute('data-attendance-stopped', 'yes');
	await expect(page.getByRole('button', { name: 'สแกนเข้าโรงเรียน', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'เก็บตัวอย่าง (0/3)', exact: true })).toBeVisible();
	expect(
		api.writes.filter((write) => write.path.endsWith('/scans') || write.path === '/api/files')
	).toHaveLength(0);
});

test('webcam discards a pending capture across Bangkok midnight and reopens today', async ({
	page
}) => {
	const api = await mockAttendance(page);
	await page.addInitScript(() => {
		Math.random = () => 0.1;
	});
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (route) =>
		route.fulfill({
			contentType: 'application/javascript',
			body: `let frame=0;export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};export async function readFace(){return {values:Array(128).fill(.01),yaw:[0,-.12,0][frame++%3]}};export async function readFaces(){const face=await readFace();return {frame:document.createElement('canvas'),capturedAt:new Date().toISOString(),detectedCount:face?1:0,faces:face?[{...face,box:{x:.3,y:.2,width:.2,height:.3}}]:[]}};export function matchFace(){return '${student}'};export function distance(){return 0};export async function evidence(){document.body.dataset.capturePending='yes';await new Promise(resolve=>document.addEventListener('attendance-test-release',resolve,{once:true}));return new File(['fixture'],'scan.jpg',{type:'image/jpeg'})};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'เครื่องสแกน', exact: true }).click();
	await page.getByRole('option', { name: 'เว็บแคมหน้าโรงเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้', exact: true }).click();
	await expect(page.locator('body')).toHaveAttribute('data-capture-pending', 'yes');
	await page.clock.setFixedTime(new Date('2026-10-09T17:01:00Z'));
	await page.evaluate(() => document.dispatchEvent(new Event('attendance-test-release')));
	await expect(page.getByRole('status').filter({ hasText: 'เปลี่ยนวันแล้ว' })).toBeVisible();
	expect(
		api.writes.filter((write) => write.path.endsWith('/scans') || write.path === '/api/files')
	).toHaveLength(0);
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้', exact: true }).click();
	await expect
		.poll(() => api.writes.filter((write) => write.path.endsWith('/kiosk/open')).at(-1)?.data)
		.toMatchObject({ date: '2026-10-10' });
	await page.getByRole('button', { name: 'หยุดและปิดกล้อง', exact: true }).click();
});

const settings: AttendanceSettings = {
	academicTermId: term,
	configuration,
	autoFinalizeFrom: '2026-10-09',
	rowVersion: 1,
	archived: false
};
const session: AttendanceSession = {
	id: sessionId,
	academicTermId: term,
	date,
	kind: 'arrival',
	sourceKey: room,
	title: 'เข้าโรงเรียน ม.1/1',
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
	rowVersion: 1,
	rosterToken: 'fixture-roster'
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
			observedAt: null,
			processedAt: null,
			evidenceFileId: null,
			arrivalAt: null,
			arrivalResult: null,
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
async function mockAttendance(
	page: Page,
	{
		holdScan = false,
		conflict = false,
		holdOptions = false,
		failOptions = false,
		archived = false,
		permissions,
		userType
	}: {
		holdScan?: boolean;
		conflict?: boolean;
		holdOptions?: boolean;
		failOptions?: boolean;
		archived?: boolean;
		permissions?: string[];
		userType?: string;
	} = {}
) {
	await mockStaffHome(page, {
		userType,
		permissions: permissions ?? [
			'attendance.read.assigned',
			'attendance.update.assigned',
			'attendance.manage.school',
			'attendance.enroll.school',
			'attendance.verify.assigned',
			'attendance.delete.school',
			'academic_context.read.school'
		]
	});
	await page.route(
		(url) => url.pathname === '/api/me/calendar/events',
		(route) => reply(route, [])
	);
	await page.clock.setFixedTime(new Date('2026-10-09T01:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (r) =>
		r.fulfill({ contentType: 'text/css', body: '' })
	);
	await page.route(
		(url) =>
			[
				'/api/academic/context/options',
				'/api/me/academic-context/options',
				'/api/parent/academic-context/options'
			].includes(url.pathname),
		(r) =>
			reply(r, {
				defaultAcademicYearId: year,
				defaultAcademicTermId: term,
				years: [
					{
 isVisible: true,
						id: year,
						name: '2569',
						year: 2569,
						status: 'active',
					}
				],
				terms: [
					{
 isVisible: true,
						id: term,
						academicYearId: year,
						code: '1',
						name: 'ภาคเรียนที่ 1',
						sequence: 1,
						termType: 'regular',
						status: 'active',
						includedInYearResult: true,
						blocksYearClosure: true
					}
				]
			})
	);
	let current = detail(),
		savedSettings = structuredClone(settings);
	savedSettings.archived = archived;
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
					lessonChoices: [],
					sessions: [current.session],
					settings: savedSettings
				} satisfies AttendanceWorkspace);
			if (resource.endsWith('/options')) {
				if (holdOptions) await gate;
				if (failOptions) {
					failOptions = false;
					return reply(route, 'โหลดรายชื่อไม่สำเร็จ', 500);
				}
				return reply(route, options);
			}
			if (resource.includes('/audiences/'))
				return reply(route, {
					academicTermId: term,
					...request.postDataJSON(),
					id: request.postDataJSON().id ?? id(220),
					rowVersion: 1
				});
			if (resource.includes('/specials/'))
				return reply(route, {
					id: id(221),
					academicTermId: term,
					definition: request.postDataJSON(),
					rowVersion: 1
				});
			if (resource.includes('/devices/'))
				return reply(route, { id: resource.split('/').at(-1), ...request.postDataJSON() });
			if (resource.endsWith('/report'))
				return reply(route, {
					archived,
					page: 1,
					pageSize: 50,
					total: new URL(request.url()).searchParams.get('studentId') ? 1 : 2,
					activityCountsAsPresent: true,
					summaries: (new URL(request.url()).searchParams.get('studentId')
						? [new URL(request.url()).searchParams.get('studentId')!]
						: [student, second]
					).map((studentId, i) => ({
						studentId,
						academicTermId: term,
						displayName: i ? 'นักเรียนคนที่สอง' : 'นักเรียนคนแรก',
						category: 'school',
						scopeKey: '',
						scopeLabel: 'มาโรงเรียน',
						present: 1,
						late: 0,
						absent: 0,
						skipped: 0,
						sickLeave: 0,
						personalLeave: 0,
						activity: 0,
						unchecked: 1,
						expected: 2
					}))
				});
			if (resource.endsWith('/history'))
				return reply(route, [
					{
						sessionId,
						date,
						kind: 'arrival',
						title:
							new URL(request.url()).searchParams.get('studentId') === student
								? 'เข้าโรงเรียน · หลักฐานคนแรก'
								: 'เข้าโรงเรียน · รายละเอียดคนที่สอง',
						result: 'present',
						cancelled: false,
						observedAt: date + 'T01:00:00Z',
						evidenceFileId: null
					}
				]);
			if (resource.includes('/settings/')) {
				if (method === 'PUT')
					savedSettings = {
						...savedSettings,
						...request.postDataJSON(),
						rowVersion: savedSettings.rowVersion + 1
					};
				return reply(route, savedSettings);
			}
			if (resource === '/api/attendance/faces' && method === 'GET') return reply(route, []);
			if (resource === '/api/attendance/days') return reply(route, []);
			if (resource.includes('/days/')) {
				savedSettings.rowVersion++;
				return reply(route, savedSettings);
			}
			if (resource.endsWith('/kiosk/open'))
				return reply(route, {
					lessonChoices: [],
					sessions: [{ ...session, kind: 'arrival' }],
					students: options.students,
					configuration,
					autoFinalizeFrom: '2026-10-09',
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
			if (resource.endsWith('/preview')) {
				const source = new URL(request.url()).searchParams.get('sourceKey');
				if (source !== room)
					return reply(route, {
						...current,
						session: {
							...current.session,
							id: id(240),
							sourceKey: source,
							title: 'รอบครูคนอื่น',
							teacherIds: [id(242)],
							rowVersion: 0
						}
					});
				current = { ...current, session: { ...current.session, rowVersion: 0 } };
				return reply(route, current);
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
							result: payload.students.find((r) => r.studentId === s.studentId)?.result ?? s.result
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
test('module navigation keeps academic context and marks the current page', async ({ page }) => {
	await mockAttendance(page);
	for (const [suffix, label] of [
		['', 'เช็คชื่อ'],
		['/settings', 'ตั้งค่าปฏิทินและรอบพิเศษ'],
		['/report', 'สรุป / ล้างภาคเรียน'],
		['/faces', 'เว็บแคม / ลงทะเบียนใบหน้า']
	]) {
		await page.goto(path(suffix));
		const nav = page.getByRole('navigation', { name: 'เมนูเช็คชื่อ' });
		await expect(nav.getByRole('link', { name: label, exact: true })).toHaveAttribute(
			'aria-current',
			'page'
		);
		for (const href of await nav
			.getByRole('link')
			.evaluateAll((links) => links.map((link) => (link as HTMLAnchorElement).href))) {
			const query = new URL(href).searchParams;
			expect(query.get('academicYearId')).toBe(year);
			expect(query.get('academicTermId')).toBe(term);
			expect(query.get('date')).toBe(date);
		}
	}
});
test('browser reload discards input while search preserves current edits without submitting', async ({
	page
}) => {
	const api = await mockAttendance(page);
	await page.goto(path());
	await chooseRound(page);
	await chooseResult(page, 'นักเรียนคนแรก', 'มา');
	await page.getByLabel('เหตุผลแก้ไข / งดคาบ').fill('ตรวจซ้ำ');
	await page.reload();
	await chooseRound(page);
	await expectResult(page, 'นักเรียนคนแรก', 'ยังไม่ได้เช็ค');
	await expect(page.getByLabel('เหตุผลแก้ไข / งดคาบ')).toHaveValue('');
	await chooseResult(page, 'นักเรียนคนแรก', 'มา');
	await page.getByLabel('ค้นหาชื่อนักเรียน').fill('ไม่มีชื่อนี้');
	await page.getByLabel('ค้นหาชื่อนักเรียน').press('Enter');
	await expect(page.getByText('ไม่พบชื่อนักเรียนที่ค้นหา', { exact: true })).toBeVisible();
	await page.getByLabel('ค้นหาชื่อนักเรียน').fill('');
	await expectResult(page, 'นักเรียนคนแรก', 'มา');
	expect(api.writes).toHaveLength(0);
});
test('read-only teacher can preview an unsaved round without creating a session', async ({
	page
}) => {
	const api = await mockAttendance(page, {
		permissions: ['attendance.read.assigned', 'academic_context.read.school']
	});
	await page.route('**/api/attendance/workspace?**', (route) =>
		reply(route, {
			date,
			counted: true,
			lessonChoices: [],
			sessions: [{ ...session, rowVersion: 0 }],
			settings
		} satisfies AttendanceWorkspace)
	);
	await page.goto(path());
	await chooseRound(page);
	await expect(
		resultControl(page, 'นักเรียนคนแรก').getByRole('radio', { name: 'มา', exact: true })
	).toBeDisabled();
	expect(api.reads).toContain('/api/attendance/preview');
	expect(api.writes).toHaveLength(0);
});
test('arrival selection shows the pending round immediately and retries only its failed preview', async ({
	page
}) => {
	const api = await mockAttendance(page);
	await page.setViewportSize({ width: 390, height: 884 });
	const unsaved = { ...session, rowVersion: 0 };
	let workspaceReads = 0;
	await page.route('**/api/attendance/workspace?**', (route) => {
		workspaceReads++;
		return reply(route, {
			date,
			counted: true,
			lessonChoices: [],
			sessions: [unsaved],
			settings
		} satisfies AttendanceWorkspace);
	});
	let release!: () => void;
	const held = new Promise<void>((resolve) => (release = resolve));
	let previews = 0;
	await page.route('**/api/attendance/preview?**', async (route) => {
		if (++previews === 1) {
			await held;
			return reply(route, 'รายชื่อยังไม่พร้อม', 503);
		}
		return reply(route, { ...detail(), session: unsaved });
	});
	await page.goto(path());
	await chooseRound(page);
	await expect(page.getByRole('button', { name: 'รอบเช็คชื่อ', exact: true })).toContainText(
		'เข้าโรงเรียน ม.1/1'
	);
	await expect(page.locator('[aria-busy="true"]')).toBeVisible();
	await expect(resultControl(page, 'นักเรียนคนแรก')).toHaveCount(0);
	release();
	await expect(page.getByText('รายชื่อยังไม่พร้อม', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองเปิดรอบใหม่', exact: true }).click();
	await expect(
		resultControl(page, 'นักเรียนคนแรก').getByRole('radio', { name: 'มา', exact: true })
	).toBeEnabled();
	expect(previews).toBe(2);
	expect(workspaceReads).toBe(1);
	expect(api.writes).toHaveLength(0);
});
test('school reader can preview others but can only edit assigned rounds', async ({ page }) => {
	const api = await mockAttendance(page, {
		permissions: [
			'attendance.read.school',
			'attendance.update.assigned',
			'academic_context.read.school'
		]
	});
	await page.route('**/api/attendance/workspace?**', (route) =>
		reply(route, {
			date,
			counted: true,
			settings,
			lessonChoices: [],
			sessions: [
				{ ...session, rowVersion: 0 },
				{
					...session,
					id: id(240),
					sourceKey: id(241),
					title: 'รอบครูคนอื่น',
					teacherIds: [id(242)],
					rowVersion: 0
				}
			]
		} satisfies AttendanceWorkspace)
	);
	await page.goto(path());
	await page.getByRole('button', { name: /08:00.*รอบครูคนอื่น/ }).click();
	await expect(
		resultControl(page, 'นักเรียนคนแรก').getByRole('radio', { name: 'มา', exact: true })
	).toBeDisabled();
	await chooseRound(page);
	await expect(page.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
	expect(api.reads.filter((read) => read.endsWith('/preview'))).toHaveLength(2);
	expect(api.writes).toHaveLength(0);
});
test('changing the workspace date also updates module destinations', async ({ page }) => {
	await mockAttendance(page);
	await page.route('**/api/attendance/workspace?**', (route) =>
		reply(route, {
			date: new URL(route.request().url()).searchParams.get('date')!,
			counted: true,
			lessonChoices: [],
			sessions: [],
			settings
		} satisfies AttendanceWorkspace)
	);
	await page.goto(path());
	await page.getByRole('button', { name: 'วันที่', exact: true }).click();
	await page.getByRole('button', { name: 'วันพฤหัสบดีที่ 8 ตุลาคม 2569', exact: true }).click();
	await expect(
		page
			.getByRole('navigation', { name: 'เมนูเช็คชื่อ' })
			.getByRole('link', { name: 'สรุป / ล้างภาคเรียน', exact: true })
	).toHaveAttribute('href', /date=2026-10-08/);
});
test('successful purge remains archived when only the summary refresh fails', async ({ page }) => {
	await mockAttendance(page, {
		permissions: [
			'attendance.read.school',
			'attendance.delete.school',
			'academic_context.read.school'
		]
	});
	let purges = 0;
	await page.route('**/api/attendance/terms/*/purge', (route) => {
		if (route.request().method() === 'POST') purges++;
		return reply(route, {
			academicTermId: term,
			archived: purges > 0,
			canPurge: purges === 0,
			records: 2,
			evidence: 0,
			evidenceBytes: 0
		});
	});
	await page.route('**/api/attendance/report?**', (route) =>
		purges ? reply(route, 'สรุปยังไม่พร้อม', 500) : route.fallback()
	);
	await page.goto(path('/report'));
	await page.getByRole('button', { name: 'นักเรียนคนแรก', exact: true }).click();
	await expect(page.getByText(/หลักฐานคนแรก/)).toBeVisible();
	await page.getByRole('button', { name: 'ตรวจผลกระทบก่อนล้าง', exact: true }).click();
	await page.getByLabel('เหตุผล', { exact: false }).fill('จบภาคเรียน');
	await page.getByLabel('ยืนยันล้างรายละเอียดและภาพของภาคเรียนนี้ โดยคงยอดสรุป').check();
	await page.getByRole('button', { name: 'ล้างข้อมูลภาคเรียนนี้', exact: true }).click();
	await page
		.getByRole('alertdialog')
		.getByRole('button', { name: 'ยืนยันล้างข้อมูล', exact: true })
		.click();
	await expect(page.getByText('โหลดสรุปล่าสุดไม่ได้', { exact: true })).toBeVisible();
	await expect(page.getByText(/หลักฐานคนแรก/)).toHaveCount(0);
	await expect(
		page.getByRole('button', { name: 'ตรวจผลกระทบก่อนล้าง', exact: true })
	).toBeDisabled();
	expect(purges).toBe(1);
});
test('cancelling a session needs a reason and a final confirmation', async ({ page }) => {
	await mockAttendance(page);
	let cancellationWrites = 0;
	await page.route('**/api/attendance/sessions/*/cancellation', (route) => {
		cancellationWrites++;
		const result = detail();
		result.session = {
			...result.session,
			cancelled: true,
			cancellationReason: 'กิจกรรมโรงเรียน',
			rowVersion: 2
		};
		result.writable = false;
		return reply(route, result);
	});
	await page.goto(path());
	await chooseRound(page);
	await page.getByRole('button', { name: 'งดคาบ / กิจกรรมแทนการเรียน', exact: true }).click();
	await expect(page.getByText('ระบุเหตุผลงดหรือคืนคาบ', { exact: true })).toBeVisible();
	await page.getByLabel('เหตุผลแก้ไข / งดคาบ').fill('กิจกรรมโรงเรียน');
	await page.getByRole('button', { name: 'งดคาบ / กิจกรรมแทนการเรียน', exact: true }).click();
	await page.getByRole('alertdialog').getByRole('button', { name: 'กลับไปตรวจสอบ' }).click();
	expect(cancellationWrites).toBe(0);
	await page.getByRole('button', { name: 'งดคาบ / กิจกรรมแทนการเรียน', exact: true }).click();
	await page.getByRole('alertdialog').getByRole('button', { name: 'ยืนยัน', exact: true }).click();
	await expect(page.getByRole('button', { name: 'คืนคาบเช็คชื่อ', exact: true })).toBeVisible();
	expect(cancellationWrites).toBe(1);
});
test('face withdrawal requires consent and confirmation for the selected student', async ({
	page
}) => {
	await mockAttendance(page, {
		permissions: ['attendance.enroll.assigned', 'academic_context.read.school']
	});
	let deletions = 0;
	await page.route(`**/api/attendance/faces/${student}`, (route) => {
		deletions++;
		return reply(route, {});
	});
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'นักเรียน', exact: true }).click();
	await page.getByRole('option', { name: 'ม.1/1 · นักเรียนคนแรก', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'ถอนการลงทะเบียนใบหน้า', exact: true })
	).toBeDisabled();
	await page.getByLabel('ยืนยันว่าได้รับความยินยอมและตรวจว่าเป็นนักเรียนคนที่เลือก').check();
	await page.getByRole('button', { name: 'ถอนการลงทะเบียนใบหน้า', exact: true }).click();
	await expect(page.getByRole('alertdialog')).toContainText('นักเรียนคนแรก');
	await page.getByRole('alertdialog').getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	expect(deletions).toBe(0);
	await page.getByRole('button', { name: 'ถอนการลงทะเบียนใบหน้า', exact: true }).click();
	await page
		.getByRole('alertdialog')
		.getByRole('button', { name: 'ยืนยันถอนข้อมูลใบหน้า', exact: true })
		.click();
	await expect(
		page.getByRole('button', { name: 'ถอนการลงทะเบียนใบหน้า', exact: true })
	).toBeDisabled();
	expect(deletions).toBe(1);
});
test('teacher saves explicit results, retains unchecked and adds a pupil without correction reason', async ({
	page
}) => {
	const api = await mockAttendance(page);
	await page.goto(path());
	await page.getByRole('button', { name: /08:00.*เข้าโรงเรียน ม.1\/1/ }).click();
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByText('ยังไม่มีผลที่เปลี่ยนแปลงสำหรับบันทึก')).toBeVisible();
	expect(api.writes).toHaveLength(0);
	await chooseResult(page, 'นักเรียนคนแรก', 'กิจกรรม');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expectResult(page, 'นักเรียนคนที่สอง', 'ยังไม่ได้เช็ค');
	expect(api.writes).toHaveLength(1);
	expect(api.reads.filter((p) => p.endsWith('/workspace'))).toHaveLength(1);
	expect(api.writes[0].data).toMatchObject({
		session: { kind: 'arrival', sourceKey: room },
		rosterToken: 'fixture-roster',
		students: [{ studentId: student, result: 'activity' }]
	});
	await chooseResult(page, 'นักเรียนคนที่สอง', 'มา');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expectResult(page, 'นักเรียนคนที่สอง', 'มา');
	expect(api.writes).toHaveLength(2);
	expect(api.writes[1].data).toMatchObject({
		reason: '',
		students: [{ studentId: second, result: 'present' }]
	});
	await chooseResult(page, 'นักเรียนคนแรก', 'สาย');
	await page.getByLabel('เหตุผลแก้ไข / งดคาบ').fill('มาถึงหลังตรวจครั้งแรก');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	expect(api.writes).toHaveLength(3);
});
test('scan conflict preserves teacher input', async ({ page }) => {
	await mockAttendance(page, { conflict: true });
	await page.goto(path());
	await chooseRound(page);
	await chooseResult(page, 'นักเรียนคนแรก', 'มา');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByText('มีการสแกนเพิ่มแล้ว กรุณาโหลดรายชื่อใหม่')).toBeVisible();
	await expectResult(page, 'นักเรียนคนแรก', 'มา');
});
test('calendar toggles a single processing date', async ({ page }) => {
	const api = await mockAttendance(page);
	await page.goto(path('/settings'));
	await expect(page.getByRole('heading', { name: 'ปฏิทินวันประมวลผล' })).toBeVisible();
	await page.locator(`[data-calendar-date="${date}"]`).click();
	await expect(page.locator(`[data-calendar-date="${date}"]`)).toHaveAttribute(
		'aria-pressed',
		'false'
	);
	expect(api.writes.find((w) => w.path.includes('/days/'))?.data).toMatchObject({
		rowVersion: 1,
		days: [{ date, counted: false }]
	});
	expect(api.reads.filter((p) => p.includes('/settings/'))).toHaveLength(1);
});
test('settings save succeeds twice and calendar uses the returned revision', async ({ page }) => {
	const api = await mockAttendance(page);
	await page.goto(path('/settings'));
	await page.getByLabel('เข้าสายหลังเวลา').fill('08:30');
	await page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true }).click();
	await expect(page.getByText(/could not be cloned|structuredClone/)).toHaveCount(0);
	await expect(page.getByText('บันทึกแล้ว', { exact: true })).toBeVisible();
	await page.getByLabel('เข้าสายหลังเวลา').fill('08:45');
	await page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true }).click();
	await page.locator(`[data-calendar-date="${date}"]`).click();
	await expect(page.locator(`[data-calendar-date="${date}"]`)).toHaveAttribute(
		'aria-pressed',
		'false'
	);
	expect(api.writes.filter((w) => w.path.includes('/settings/')).map((w) => w.data)).toMatchObject([
		{ rowVersion: 1, configuration: { lateAfter: '08:30:00' } },
		{ rowVersion: 2, configuration: { lateAfter: '08:45:00' } }
	]);
	expect(api.writes.find((w) => w.path.includes('/days/'))?.data).toMatchObject({ rowVersion: 3 });
});
test('webcam reports success only after accepted scan commits', async ({ page }) => {
	const api = await mockAttendance(page, { holdScan: true });
	await page.addInitScript(() => {
		Math.random = () => 0.1;
	});
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (r) =>
		r.fulfill({
			contentType: 'application/javascript',
			body: `let frame=0;export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};export async function readFace(){return {values:Array(128).fill(.01),yaw:[0,-.12,0][frame++%3]}};export async function readFaces(){const face=await readFace();return {frame:document.createElement('canvas'),capturedAt:new Date().toISOString(),detectedCount:face?1:0,faces:face?[{...face,box:{x:.3,y:.2,width:.2,height:.3}}]:[]}};export function matchFace(){return '${student}'};export function distance(){return 0};export async function evidence(){return new File(['fixture'],'scan.jpg',{type:'image/jpeg'})};`
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
			body: `export const FACE_MODEL='face-api-1.7.15-recognition-128';export function faceEngine(){return new Promise(resolve=>window.addEventListener('test-engine-ready',()=>{resolve({});document.documentElement.dataset.engineResolved='yes'}, {once:true}))};export async function camera(){document.documentElement.dataset.cameraOpened='yes';return {getTracks:()=>[]}};export function stopCamera(){};export async function readFace(){return null};export async function readFaces(){const face=await readFace();return {frame:document.createElement('canvas'),capturedAt:new Date().toISOString(),detectedCount:face?1:0,faces:face?[{...face,box:{x:.3,y:.2,width:.2,height:.3}}]:[]}};export function matchFace(){return null};export function distance(){return 0};export async function evidence(){throw Error('unused')};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page
		.getByRole('navigation', { name: 'เมนูเช็คชื่อ' })
		.getByRole('link', { name: 'เช็คชื่อ', exact: true })
		.click();
	await expect(page.getByRole('heading', { name: 'เช็คชื่อ', exact: true })).toBeVisible();
	await page.evaluate(() => window.dispatchEvent(new Event('test-engine-ready')));
	await expect(page.locator('html')).toHaveAttribute('data-engine-resolved', 'yes');
	await expect(page.locator('html')).not.toHaveAttribute('data-camera-opened', 'yes');
});

test('settings calendar renders independently and retries only failed options', async ({
	page
}) => {
	const api = await mockAttendance(page, { failOptions: true });
	await page.goto(path('/settings'));
	await expect(page.getByRole('heading', { name: 'ปฏิทินวันประมวลผล' })).toBeVisible();
	await expect(page.getByText('โหลดรายชื่อไม่สำเร็จ', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองโหลดกลุ่มและเครื่องสแกนใหม่' }).click();
	await expect(page.getByRole('heading', { name: 'เพิ่มรอบเช็คชื่อพิเศษ' })).toBeVisible();
	expect(api.reads.filter((p) => p.endsWith('/options'))).toHaveLength(2);
	expect(api.reads.filter((p) => p.includes('/settings/'))).toHaveLength(1);
	expect(api.reads.filter((p) => p.endsWith('/days'))).toHaveLength(1);
});
test('slow options do not block calendar use', async ({ page }) => {
	const api = await mockAttendance(page, { holdOptions: true });
	await page.goto(path('/settings'));
	await expect(page.getByRole('heading', { name: 'ปฏิทินวันประมวลผล' })).toBeVisible();
	await page.locator(`[data-calendar-date="${date}"]`).click();
	await expect(page.locator(`[data-calendar-date="${date}"]`)).toHaveAttribute(
		'aria-pressed',
		'false'
	);
	api.release();
	await expect(page.getByRole('heading', { name: 'เครื่องเว็บแคม' })).toBeVisible();
});
test('unsaved weekday edits do not alter the saved processing calendar', async ({ page }) => {
	await mockAttendance(page);
	await page.goto(path('/settings'));
	await page
		.getByRole('region', { name: 'เกณฑ์และปฏิทินเช็คชื่อ' })
		.getByLabel('ส', { exact: true })
		.check();
	await expect(page.locator('[data-calendar-date="2026-10-10"]')).toHaveAttribute(
		'aria-pressed',
		'false'
	);
	await page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true }).click();
	await expect(page.locator('[data-calendar-date="2026-10-10"]')).toHaveAttribute(
		'aria-pressed',
		'true'
	);
});
test('month controls prevent empty selection and failed reads cannot write guessed days', async ({
	page
}) => {
	const api = await mockAttendance(page);
	await page.goto(path('/settings'));
	await expect(page.locator('input[type=month]')).toHaveCount(0);
	await page.route(
		(url) => url.pathname === '/api/attendance/days',
		(r) => reply(r, 'ปฏิทินไม่พร้อม', 500)
	);
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page.getByText('ปฏิทินไม่พร้อม', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'นับช่วงนี้', exact: true })).toBeDisabled();
	expect(api.writes).toHaveLength(0);
});
test('read-only teacher sees saved results without management links or optional reads', async ({
	page
}) => {
	const api = await mockAttendance(page, {
		permissions: ['attendance.read.assigned', 'academic_context.read.school']
	});
	await page.goto(path());
	await chooseRound(page);
	await expect(
		resultControl(page, 'นักเรียนคนแรก').getByRole('radio', { name: 'มา', exact: true })
	).toBeDisabled();
	await expect(page.getByRole('button', { name: 'บันทึก', exact: true })).toHaveCount(0);
	await expect(page.getByRole('link', { name: 'ตั้งค่าปฏิทินและรอบพิเศษ' })).toHaveCount(0);
	await expect(page.getByRole('link', { name: 'เว็บแคม / ลงทะเบียนใบหน้า' })).toHaveCount(0);
	await page.goto(path('/settings'));
	await expect(page).toHaveURL(/\/403/);
	expect(
		api.reads.filter(
			(p) => p.endsWith('/options') || p.includes('/settings/') || p.endsWith('/days')
		)
	).toHaveLength(0);
});
test('enrollment-only teacher starts in enrollment and changing student resets consent', async ({
	page
}) => {
	await mockAttendance(page, {
		permissions: ['attendance.enroll.assigned', 'academic_context.read.school']
	});
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (r) =>
		r.fulfill({
			contentType: 'application/javascript',
			body: `export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};export async function readFace(){return {values:Array(128).fill(.01),yaw:0}};export async function readFaces(){const face=await readFace();return {frame:document.createElement('canvas'),capturedAt:new Date().toISOString(),detectedCount:face?1:0,faces:face?[{...face,box:{x:.3,y:.2,width:.2,height:.3}}]:[]}};export function matchFace(){return null};export function distance(){return 0};export async function evidence(){throw Error('unused')};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'นักเรียน', exact: true }).click();
	await page.getByRole('option', { name: 'ม.1/1 · นักเรียนคนแรก', exact: true }).click();
	await page.getByLabel('ยืนยันว่าได้รับความยินยอมและตรวจว่าเป็นนักเรียนคนที่เลือก').check();
	await page.getByRole('button', { name: 'เก็บตัวอย่าง (0/3)' }).click();
	await expect(page.getByRole('button', { name: 'เก็บตัวอย่าง (1/3)' })).toBeVisible();
	await page.getByRole('button', { name: 'นักเรียน', exact: true }).click();
	await page.getByRole('option', { name: 'ม.1/1 · นักเรียนคนที่สอง', exact: true }).click();
	await expect(
		page.getByLabel('ยืนยันว่าได้รับความยินยอมและตรวจว่าเป็นนักเรียนคนที่เลือก')
	).not.toBeChecked();
	await expect(page.getByRole('button', { name: 'เก็บตัวอย่าง (0/3)' })).toBeDisabled();
	await expect(page.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้' })).toHaveCount(0);
	await page.getByRole('button', { name: 'หยุดและปิดกล้อง' }).click();
});
test('stopping during a pending enrollment sample discards it', async ({ page }) => {
	const api = await mockAttendance(page);
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (r) =>
		r.fulfill({
			contentType: 'application/javascript',
			body: `export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};export function readFace(){return new Promise(resolve=>window.addEventListener('test-face-ready',()=>resolve({values:Array(128).fill(.01),yaw:0}),{once:true}))};export async function readFaces(){const face=await readFace();return {frame:document.createElement('canvas'),capturedAt:new Date().toISOString(),detectedCount:face?1:0,faces:face?[{...face,box:{x:.3,y:.2,width:.2,height:.3}}]:[]}};export function matchFace(){return null};export function distance(){return 0};export async function evidence(){throw Error('unused')};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'ลงทะเบียนใบหน้า', exact: true }).click();
	await page.getByRole('button', { name: 'นักเรียน', exact: true }).click();
	await page.getByRole('option', { name: 'ม.1/1 · นักเรียนคนแรก', exact: true }).click();
	await page.getByLabel('ยืนยันว่าได้รับความยินยอมและตรวจว่าเป็นนักเรียนคนที่เลือก').check();
	await page.getByRole('button', { name: 'เก็บตัวอย่าง (0/3)' }).click();
	await expect(page.getByRole('button', { name: 'นักเรียน', exact: true })).toBeDisabled();
	await page.getByRole('button', { name: 'หยุดและปิดกล้อง' }).click();
	await page.evaluate(() => window.dispatchEvent(new Event('test-face-ready')));
	await expect(page.getByRole('button', { name: 'เก็บตัวอย่าง (0/3)' })).toBeDisabled();
	expect(api.writes).toHaveLength(0);
});
test('report changes student and keeps daily details separate', async ({ page }) => {
	const api = await mockAttendance(page);
	await page.goto(path('/report'));
	await page.getByRole('button', { name: 'นักเรียนคนแรก', exact: true }).click();
	await expect(page.getByText(/หลักฐานคนแรก/)).toBeVisible();
	await page.getByRole('button', { name: 'นักเรียนคนที่สอง', exact: true }).click();
	await expect(page.getByText(/รายละเอียดคนที่สอง/)).toBeVisible();
	await expect(page.getByText(/หลักฐานคนแรก/)).toHaveCount(0);
	expect(api.reads.filter((p) => p.endsWith('/history'))).toHaveLength(2);
});
test('special activity creation uses selected reusable audience and teachers', async ({ page }) => {
	const api = await mockAttendance(page);
	await page.goto(path('/settings'));
	await page.getByRole('button', { name: 'จัดการกลุ่ม', exact: true }).click();
	await page.getByLabel('ชื่อกลุ่ม', { exact: true }).fill('กลุ่มชุมนุม');
	await page.getByRole('button', { name: 'สมาชิกกลุ่ม', exact: true }).click();
	await page.getByRole('option', { name: 'ม.1/1 · นักเรียนคนแรก', exact: true }).click();
	await page.keyboard.press('Escape');
	await page.getByRole('button', { name: 'เพิ่มกลุ่ม', exact: true }).click();
	await expect(page.getByText('กลุ่มชุมนุม · 1 คน')).toBeVisible();
	await page.keyboard.press('Escape');
	await page.getByLabel('ชื่อการเช็คชื่อ').fill('กิจกรรมทดลอง');
	await page.getByRole('button', { name: 'กลุ่มที่ตั้งไว้', exact: true }).click();
	await page.getByRole('option', { name: 'กลุ่มชุมนุม', exact: true }).click();
	await page.keyboard.press('Escape');
	await page.getByRole('button', { name: 'ครูที่เลือกเพิ่ม', exact: true }).click();
	await page.getByRole('option', { name: 'ครูผู้รับผิดชอบ', exact: true }).click();
	await page.keyboard.press('Escape');
	await page.getByRole('button', { name: 'สร้างรอบพิเศษ', exact: true }).click();
	await expect(page.getByText('กิจกรรมทดลอง · 1 วัน / 1 กลุ่ม')).toBeVisible();
	expect(api.writes.find((w) => w.path.includes('/specials/'))?.data).toMatchObject({
		title: 'กิจกรรมทดลอง',
		dates: [date],
		groups: [{ audienceGroupIds: [id(220)], teacherIds: [actor] }]
	});
});
for (const viewport of [
	{ name: 'mobile', width: 390, height: 844 },
	{ name: 'desktop', width: 1440, height: 900 }
]) {
	for (const theme of ['light', 'dark'] as const) {
		test(`attendance layouts ${viewport.name} ${theme}`, async ({ page }) => {
			await page.setViewportSize({ width: viewport.width, height: viewport.height });
			await page.emulateMedia({ colorScheme: theme });
			await mockAttendance(page);
			for (const suffix of ['/settings', '/faces', '/report', '']) {
				await page.goto(path(suffix));
				if (!suffix) await chooseRound(page);
				await expect(
					page.getByRole('heading', {
						name: (
							{
								'/settings': 'ตั้งค่าเช็คชื่อ',
								'/faces': 'เว็บแคมและใบหน้า',
								'/report': 'สรุปการเช็คชื่อ',
								'': 'เช็คชื่อ'
							} as Record<string, string>
						)[suffix],
						exact: true
					})
				).toBeVisible();
				await expect(page.locator('body')).not.toContainText('could not be cloned');
				expect(
					await page.locator('html').evaluate((element) => element.classList.contains('dark'))
				).toBe(theme === 'dark');
				expect(
					await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
				).toBe(true);
				if (suffix === '/settings') {
					for (const heading of ['ปฏิทินวันประมวลผล', 'เพิ่มรอบเช็คชื่อพิเศษ', 'เครื่องเว็บแคม']) {
						await page
							.getByRole('heading', { name: heading, exact: true })
							.evaluate((element) => element.scrollIntoView({ block: 'start' }));
						await page.screenshot({
							path: `/tmp/attendance-review-${viewport.name}-${theme}-${heading}.png`
						});
					}
				}
				if (!suffix) {
					for (const label of ['ค้นหาชื่อนักเรียน', 'เหตุผลแก้ไข / งดคาบ']) {
						await page.getByLabel(label).scrollIntoViewIfNeeded();
						await page.screenshot({
							path: `/tmp/attendance-review-${viewport.name}-${theme}-${label.replaceAll('/', '-')}.png`
						});
					}
				}
				if (suffix === '/faces') {
					await page
						.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้', exact: true })
						.scrollIntoViewIfNeeded();
					await page.screenshot({
						path: `/tmp/attendance-review-${viewport.name}-${theme}-camera-controls.png`
					});
				}
				await page.screenshot({
					path: `/tmp/attendance-review-${viewport.name}-${theme}-${suffix.replace('/', '') || 'workspace'}.png`,
					fullPage: true
				});
			}
		});
	}
}

for (const role of ['student', 'parent']) {
	test(`${role} attendance uses own or linked-child context only`, async ({ page }) => {
		await mockAttendance(page, { userType: role, permissions: ['attendance.read.own'] });
		await page.route('**/api/parent/profile?**', (r) =>
			reply(r, {
				children: [
					{ id: student, first_name: 'นักเรียน', last_name: 'คนแรก' },
					{ id: second, first_name: 'นักเรียน', last_name: 'คนที่สอง' }
				]
			})
		);
		const reports: URL[] = [];
		page.on('request', (request) => {
			const url = new URL(request.url());
			if (url.pathname === '/api/attendance/report') reports.push(url);
		});
		await page.goto(`/${role}/attendance?academicTermId=${term}`);
		await expect(page.getByRole('heading', { name: 'การเช็คชื่อ', exact: true })).toBeVisible();
		await expect(page.getByRole('button', { name: 'ของนักเรียน', exact: true })).toHaveCount(1);
		expect(reports.at(-1)?.searchParams.get('studentId')).toBe(
			role === 'student' ? actor : student
		);
		await expect(page.getByRole('heading', { name: 'ล้างรายละเอียดภาคเรียน' })).toHaveCount(0);
		if (role === 'parent') {
			await page.getByRole('button', { name: 'นักเรียน', exact: true }).click();
			await page.getByRole('option', { name: 'นักเรียน คนที่สอง', exact: true }).click();
			await expect(page).toHaveURL(new RegExp(second));
			await expect.poll(() => reports.at(-1)?.searchParams.get('studentId')).toBe(second);
		}
	});
}

for (const overlap of [false, true])
	test(`save conflict reconciles latest revision with overlap=${overlap}`, async ({ page }) => {
		await mockAttendance(page);
		const current = detail(),
			writes: SaveAttendanceResults[] = [];
		await page.route(
			(url) => url.pathname === `/api/attendance/sessions/${sessionId}`,
			async (route) => {
				if (route.request().method() === 'PUT') {
					const payload = route.request().postDataJSON() as SaveAttendanceResults;
					writes.push(payload);
					if (writes.length === 1) {
						current.session.rowVersion = 2;
						current.students[overlap ? 0 : 1].result = 'late';
						return reply(route, 'มีการสแกนเพิ่มแล้ว กรุณาโหลดรายชื่อใหม่', 409);
					}
					current.session.rowVersion++;
					current.session.savedAt = date + 'T01:00:00Z';
					current.students = current.students.map((row) => ({
						...row,
						...payload.students.find((input) => input.studentId === row.studentId)
					}));
				}
				return reply(route, current);
			}
		);
		await page.goto(path());
		await chooseRound(page);
		await chooseResult(page, 'นักเรียนคนแรก', 'มา');
		await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
		await expect(page.getByText('มีการสแกนเพิ่มแล้ว กรุณาโหลดรายชื่อใหม่')).toBeVisible();
		if (overlap) {
			await expect(page.getByRole('button', { name: 'บันทึก', exact: true })).toBeDisabled();
			await page
				.getByRole('button', { name: 'ใช้ข้อมูลล่าสุดของ นักเรียนคนแรก', exact: true })
				.click();
			await expectResult(page, 'นักเรียนคนแรก', 'สาย');
			await chooseResult(page, 'นักเรียนคนแรก', 'มา');
			await page.getByLabel('เหตุผลแก้ไข / งดคาบ').fill('ยืนยันผลกับครูผู้รับผิดชอบ');
		} else await expectResult(page, 'นักเรียนคนที่สอง', 'สาย');
		await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
		await expect.poll(() => writes.length).toBe(2);
		expect(writes[1].rowVersion).toBe(2);
		expect(writes[1].students.every((row) => !Object.hasOwn(row, 'note'))).toBe(true);
		if (overlap)
			expect(writes[1].students.find((row) => row.studentId === student)?.result).toBe('present');
		else expect(writes[1].students.some((row) => row.studentId === second)).toBe(false);
		await expectResult(page, 'นักเรียนคนที่สอง', overlap ? 'ยังไม่ได้เช็ค' : 'สาย');
	});
test('browser reload reloads selected detail and discards unsaved input', async ({ page }) => {
	await mockAttendance(page);
	const current = detail();
	await page.route(
		(url) => url.pathname === `/api/attendance/sessions/${sessionId}`,
		(route) => reply(route, current)
	);
	await page.goto(path());
	await chooseRound(page);
	await chooseResult(page, 'นักเรียนคนแรก', 'มา');
	current.session.rowVersion = 2;
	current.students[0].result = 'late';
	await page.reload();
	await chooseRound(page);
	await expectResult(page, 'นักเรียนคนแรก', 'สาย');
	await expect(page.getByLabel(/หมายเหตุของ/)).toHaveCount(0);
});
test('reports page at the server and export all filtered rows from any visible page', async ({
	page
}) => {
	await mockAttendance(page);
	const rows = Array.from({ length: 125 }, (_, i) => ({
		academicTermId: term,
		studentId: id(1000 + i),
		displayName: `นักเรียนทดสอบ ${i}`,
		category: 'school',
		scopeKey: '',
		scopeLabel: 'การมาโรงเรียน',
		present: 10,
		late: 1,
		absent: 1,
		skipped: 0,
		sickLeave: 0,
		personalLeave: 0,
		activity: 0,
		unchecked: 0,
		expected: 12
	}));
	const queries: URLSearchParams[] = [];
	await page.route(
		(url) => url.pathname === '/api/attendance/report',
		(route) => {
			const query = new URL(route.request().url()).searchParams;
			queries.push(query);
			const number = Number(query.get('page') || 1),
				size = Number(query.get('pageSize') || 50);
			const filtered = rows.filter((row) => row.displayName.includes(query.get('search') || ''));
			return reply(route, {
				archived: false,
				activityCountsAsPresent: true,
				page: number,
				pageSize: size,
				total: filtered.length,
				summaries: filtered.slice((number - 1) * size, number * size)
			});
		}
	);
	await page.goto(path('/report'));
	await expect(page.locator('tbody tr')).toHaveCount(50);
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.getByText('หน้า 2 · 125 รายการ')).toBeVisible();
	const downloadPromise = page.waitForEvent('download');
	await page.getByRole('button', { name: 'ส่งออก CSV', exact: true }).click();
	const download = await downloadPromise;
	const csv = await readFile((await download.path())!, 'utf8');
	expect(csv.split('\r\n')).toHaveLength(126);
	expect(csv).toContain('นักเรียนทดสอบ 0');
	expect(csv).toContain('นักเรียนทดสอบ 124');
	expect(
		queries.filter((query) => query.get('pageSize') === '100').map((query) => query.get('page'))
	).toEqual(['1', '2']);
	await page.getByLabel('ค้นหาสรุป', { exact: true }).fill('นักเรียนทดสอบ 12');
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect(page.getByText('หน้า 1 · 6 รายการ')).toBeVisible();
	await expect(page.locator('tbody tr')).toHaveCount(6);
});
test('large roster keeps DOM bounded while mark-all saves the complete round on mobile', async ({
	page
}) => {
	const api = await mockAttendance(page);
	const current = detail();
	current.students = Array.from({ length: 85 }, (_, i) => ({
		...current.students[0],
		studentId: id(2000 + i),
		displayName: `นักเรียนมือถือ ${i}`,
		classNumber: i + 1
	}));
	await page.route(
		(url) => url.pathname === `/api/attendance/sessions/${sessionId}`,
		async (route) => {
			if (route.request().method() === 'GET') return reply(route, current);
			await route.fallback();
		}
	);
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto(path());
	await chooseRound(page);
	await expect(page.locator('tbody tr')).toHaveCount(40);
	await expect(page.getByText('เพิ่มหมายเหตุ', { exact: true })).toHaveCount(0);
	expect(
		await page
			.locator('[data-slot="table-container"]')
			.evaluate((element) => element.scrollWidth <= element.clientWidth + 1)
	).toBe(true);
	await page.getByRole('button', { name: 'มาทั้งหมด', exact: true }).click();
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expectResult(page, 'นักเรียนมือถือ 40', 'มา');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	const saved = api.writes.find((write) => write.path === `/api/attendance/sessions/${sessionId}`)
		?.data as SaveAttendanceResults;
	expect(saved.students).toHaveLength(85);
	expect(saved.students.every((row) => row.result === 'present')).toBe(true);
});
test.describe('mobile roster scroll chaining', () => {
	test.use({ viewport: { width: 482, height: 884 }, hasTouch: true, isMobile: true });
	for (const colorScheme of ['light', 'dark'] as const)
		for (const input of ['wheel', 'touch'] as const)
			test(`two consecutive ${input} gestures reach the page at both roster boundaries in ${colorScheme}`, async ({
				page
			}) => {
				await page.emulateMedia({ colorScheme });
				const api = await mockAttendance(page);
				const current = detail();
				current.students = Array.from({ length: 30 }, (_, i) => ({
					...current.students[0],
					studentId: id(3000 + i),
					displayName: `นักเรียนเลื่อนรายชื่อ ${i}`,
					classNumber: i + 1
				}));
				await page.route(
					(url) => url.pathname === `/api/attendance/sessions/${sessionId}`,
					(route) => reply(route, current)
				);
				await page.goto(path());
				await chooseRound(page);
				const roster = page.getByRole('region', { name: 'รายชื่อนักเรียน', exact: true });
				const main = page.getByRole('main');
				await expect(roster.locator('tbody tr')).toHaveCount(30);
				await roster.evaluate((element) => {
					const main = element.closest('main')!;
					main.scrollTop +=
						element.getBoundingClientRect().top - main.getBoundingClientRect().top - 100;
					main.scrollTop = Math.min(main.scrollTop, main.scrollHeight - main.clientHeight - 200);
					element.scrollTop = 100;
				});
				const pageStart = await main.evaluate((element) => element.scrollTop);
				expect(pageStart).toBeGreaterThan(180);
				expect(
					await main.evaluate(
						(element) => element.scrollHeight - element.clientHeight - element.scrollTop
					)
				).toBeGreaterThanOrEqual(180);
				const cdp = await page.context().newCDPSession(page);
				const scroll = async (delta: number) => {
					const box = (await roster.boundingBox())!;
					const x = box.x + box.width / 2;
					const y = box.y + box.height / 2;
					if (input === 'wheel') {
						await cdp.send('Input.synthesizeScrollGesture', {
							x,
							y,
							yDistance: -delta,
							speed: 600,
							gestureSourceType: 'mouse',
							preventFling: true
						});
					} else {
						await cdp.send('Input.dispatchTouchEvent', {
							type: 'touchStart',
							touchPoints: [{ x, y }]
						});
						for (let step = 1; step <= 12; step++) {
							await cdp.send('Input.dispatchTouchEvent', {
								type: 'touchMove',
								touchPoints: [{ x, y: y - (delta * step) / 12 }]
							});
							await page.evaluate(() => new Promise(requestAnimationFrame));
						}
						await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
					}
				};

				await scroll(120);
				await expect
					.poll(() => roster.evaluate((element) => element.scrollTop))
					.toBeGreaterThan(100);
				expect(await main.evaluate((element) => element.scrollTop)).toBe(pageStart);

				await roster.evaluate((element) => (element.scrollTop = element.scrollHeight));
				const rosterEnd = await roster.evaluate((element) => element.scrollTop);
				expect(rosterEnd).toBe(
					await roster.evaluate((element) => element.scrollHeight - element.clientHeight)
				);
				const assertRosterHit = async () =>
					expect(
						await roster.evaluate((element) => {
							const box = element.getBoundingClientRect();
							return element.contains(
								document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2)
							);
						})
					).toBe(true);
				await assertRosterHit();
				// Chromium can retain its scroll transaction after a programmatic jump with fractional rows.
				// Two fixed completed gestures model continued scrolling and still catch a trapped outer page.
				await scroll(180);
				await scroll(180);
				await expect
					.poll(() => main.evaluate((element) => element.scrollTop))
					.toBeGreaterThan(pageStart);
				expect(await roster.evaluate((element) => element.scrollTop)).toBe(rosterEnd);
				await expect(page.getByLabel('เหตุผลแก้ไข / งดคาบ', { exact: true })).toBeInViewport();
				await page.screenshot({
					path: `/tmp/attendance-scroll-${input}-${colorScheme}-bottom.png`
				});

				await main.evaluate((element, value) => (element.scrollTop = value), pageStart);
				await roster.evaluate((element) => (element.scrollTop = 0));
				expect(await roster.evaluate((element) => element.scrollTop)).toBe(0);
				await assertRosterHit();
				await scroll(-180);
				await scroll(-180);
				await expect
					.poll(() => main.evaluate((element) => element.scrollTop))
					.toBeLessThan(pageStart);
				expect(await roster.evaluate((element) => element.scrollTop)).toBe(0);
				expect(api.writes).toHaveLength(0);
				await cdp.detach();
			});
});

test('expired uncommitted scan releases retry state and permits a fresh capture', async ({
	page
}) => {
	await mockAttendance(page);
	await page.clock.setFixedTime(new Date('2026-10-09T01:00:00Z'));
	await page.addInitScript(() => {
		Math.random = () => 0.1;
	});
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (route) =>
		route.fulfill({
			contentType: 'application/javascript',
			body: `let frame=0;export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};export async function readFace(){return {values:Array(128).fill(.01),yaw:[0,-.12,0][frame++%3]}};export async function readFaces(){const face=await readFace();return {frame:document.createElement('canvas'),capturedAt:new Date().toISOString(),detectedCount:face?1:0,faces:face?[{...face,box:{x:.3,y:.2,width:.2,height:.3}}]:[]}};export function matchFace(){return '${student}'};export function distance(){return 0};export async function evidence(){return new File(['fixture'],'scan.jpg',{type:'image/jpeg'})};`
		})
	);
	const attempts: AttendanceScan[] = [];
	await page.route(
		(url) => url.pathname === '/api/attendance/scans',
		(route) => {
			const payload = route.request().postDataJSON() as AttendanceScan;
			attempts.push(payload);
			return reply(
				route,
				attempts.length === 1 ? 'เน็ตขัดข้อง' : 'ภาพหมดอายุ',
				attempts.length === 1 ? 503 : 400
			);
		}
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'เครื่องสแกน', exact: true }).click();
	await page.getByRole('option', { name: 'เว็บแคมหน้าโรงเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'ส่งรายการเดิมอีกครั้ง', exact: true })
	).toBeVisible();
	await page.clock.setFixedTime(new Date('2026-10-09T01:06:00Z'));
	await page.getByRole('button', { name: 'ส่งรายการเดิมอีกครั้ง', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'ส่งรายการเดิมอีกครั้ง', exact: true })
	).toHaveCount(0);
	await expect(
		page.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้', exact: true })
	).toBeEnabled();
	expect(attempts[1]).toEqual(attempts[0]);
	await page.getByRole('button', { name: 'หยุดและปิดกล้อง', exact: true }).click();
});

test('registered face list searches and withdraws the chosen student after confirmation', async ({
	page
}) => {
	await mockAttendance(page);
	let enrolled = true,
		deletes = 0;
	await page.route(
		(url) => url.pathname === '/api/attendance/faces',
		(r) => reply(r, enrolled ? [{ studentId: student, enrolledAt: '2026-10-01T08:00:00Z' }] : [])
	);
	await page.route(`**/api/attendance/faces/${student}`, (r) => {
		deletes++;
		enrolled = false;
		return reply(r, {});
	});
	await page.goto(path('/faces'));
	await expect(page.getByRole('heading', { name: 'รายชื่อที่ลงทะเบียนใบหน้า' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'โหลดรายชื่อใหม่', exact: true })).toHaveCount(0);
	await page.getByLabel('ค้นหาชื่อหรือห้องเรียน').fill('ไม่พบคนนี้');
	await expect(page.getByText('ไม่พบรายชื่อที่ค้นหา', { exact: true })).toBeVisible();
	await page.getByLabel('ค้นหาชื่อหรือห้องเรียน').fill('');
	await page.getByRole('button', { name: /ถอนข้อมูลใบหน้า นักเรียน/ }).click();
	expect(deletes).toBe(0);
	await page.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	expect(deletes).toBe(0);
	await page.getByRole('button', { name: /ถอนข้อมูลใบหน้า นักเรียน/ }).click();
	await page.getByRole('button', { name: 'ยืนยันถอนข้อมูลใบหน้า', exact: true }).click();
	await expect(page.getByText('ยังไม่มีผู้ลงทะเบียนใบหน้า', { exact: true })).toBeVisible();
	expect(deletes).toBe(1);
});

test('automatic scanning confirms two people and uses their frozen frame for sequential evidence', async ({
	page
}) => {
	const api = await mockAttendance(page);
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (route) =>
		route.fulfill({
			contentType: 'text/javascript',
			body: `
    export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};
    export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};export async function readFace(){return null};
    export async function readFaces(){const frame=document.createElement('canvas');frame.dataset.frozen='yes';return {frame,capturedAt:'2026-10-09T01:00:00Z',detectedCount:2,faces:[{values:Array(128).fill(.01),yaw:0,box:{x:.1,y:.2,width:.15,height:.3}},{values:Array(128).fill(.2),yaw:0,box:{x:.6,y:.2,width:.15,height:.3}}]}};
    export function matchFace(f,v){return v[0]<.1?'${student}':'${second}'};export function distance(){return 0};
    export async function evidence(source,box){if(source.dataset.frozen!=='yes'||!box) throw Error('evidence must use detected frame and face crop');return new File(['fixture'],'scan.jpg',{type:'image/jpeg'})};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'เครื่องสแกน', exact: true }).click();
	await page.getByRole('option', { name: 'เว็บแคมหน้าโรงเรียน' }).click();
	await page.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้', exact: true }).click();
	await expect.poll(() => api.writes.filter((w) => w.path.endsWith('/scans')).length).toBe(2);
	const scans = api.writes
		.filter((w) => w.path.endsWith('/scans'))
		.map((w) => w.data as AttendanceScan);
	expect(new Set(scans.map((scan) => scan.studentId)).size).toBe(2);
	expect(new Set(scans.map((scan) => scan.eventId)).size).toBe(2);
	expect(scans.every((scan) => scan.capturedAt === '2026-10-09T01:00:00Z')).toBe(true);
	const outlines = page.getByLabel('ภาพสดจากเว็บแคม').locator('..').locator('[style*="left:"]');
	await expect(outlines).toHaveCount(2);

	const imageWidth = (await page.getByLabel('ภาพสดจากเว็บแคม').boundingBox())!.width;
	const left = await outlines
		.first()
		.evaluate((element) => parseFloat(getComputedStyle(element).left));
	expect(Math.abs(left - imageWidth * 0.75)).toBeLessThan(1);

	await page.getByRole('button', { name: 'หยุดและปิดกล้อง', exact: true }).click();
});

test('mirrored enrollment guide rejects the wrong side and saves three distinct poses', async ({
	page
}) => {
	const api = await mockAttendance(page);
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (route) =>
		route.fulfill({
			contentType: 'text/javascript',
			body: `
    let frame=0;export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};
    export async function readFace(){return {values:Array(128).fill(.01),yaw:[0,-.12,.12,-.12][frame++]}};
    export async function readFaces(){return {frame:document.createElement('canvas'),faces:[],capturedAt:new Date().toISOString(),detectedCount:0}};export function matchFace(){return null};export function distance(){return 0};export async function evidence(){throw Error('unused')};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'ลงทะเบียนใบหน้า', exact: true }).click();
	await expect(page.getByLabel('ภาพสดจากเว็บแคม')).toHaveCSS('scale', '-1 1');
	const guide = page
		.getByLabel('ภาพสดจากเว็บแคม')
		.locator('..')
		.locator('[aria-hidden="true"] > div');
	const imageWidth = (await page.getByLabel('ภาพสดจากเว็บแคม').boundingBox())!.width;
	expect((await guide.boundingBox())!.width / imageWidth).toBeGreaterThanOrEqual(0.49);

	await page.getByRole('button', { name: 'นักเรียน', exact: true }).click();
	await page.getByRole('option', { name: /คนแรก/ }).click();
	await page.getByLabel('ยืนยันว่าได้รับความยินยอมและตรวจว่าเป็นนักเรียนคนที่เลือก').click();
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'เก็บตัวอย่าง (0/3)', exact: true }).click();
	await page.getByRole('button', { name: 'เก็บตัวอย่าง (1/3)', exact: true }).click();
	await expect(page.getByRole('button', { name: 'เก็บตัวอย่าง (1/3)', exact: true })).toBeEnabled();
	expect(api.writes.filter((w) => w.path.includes('/faces/'))).toHaveLength(0);
	await page.getByRole('button', { name: 'เก็บตัวอย่าง (1/3)', exact: true }).click();
	await page.getByRole('button', { name: 'เก็บตัวอย่าง (2/3)', exact: true }).click();
	await expect.poll(() => api.writes.filter((w) => w.path.includes('/faces/')).length).toBe(1);
});

test('withdrawal disposes a pending scan frame before deleting its enrollment', async ({
	page
}) => {
	const api = await mockAttendance(page);
	await page.route(
		(url) => url.pathname === '/api/attendance/faces',
		(route) => reply(route, [{ studentId: student, enrolledAt: '2026-10-01T08:00:00Z' }])
	);
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (route) =>
		route.fulfill({
			contentType: 'text/javascript',
			body: `
 export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){document.body.dataset.cameraStopped='yes'};export async function readFace(){return null};
 export async function readFaces(){return {frame:document.createElement('canvas'),capturedAt:'2026-10-09T01:00:00Z',detectedCount:1,faces:[{values:Array(128).fill(.01),yaw:0,box:{x:.3,y:.2,width:.2,height:.3}}]}};export function matchFace(){return '${student}'};export function distance(){return 0};
 export async function evidence(){document.body.dataset.capturePending='yes';await new Promise(resolve=>document.addEventListener('attendance-test-release',resolve,{once:true}));return new File(['fixture'],'scan.jpg',{type:'image/jpeg'})};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'เครื่องสแกน', exact: true }).click();
	await page.getByRole('option', { name: 'เว็บแคมหน้าโรงเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้', exact: true }).click();
	await expect(page.locator('body')).toHaveAttribute('data-capture-pending', 'yes');
	await page.getByRole('button', { name: /ถอนข้อมูลใบหน้า นักเรียน/ }).click();
	await page.getByRole('button', { name: 'ยืนยันถอนข้อมูลใบหน้า', exact: true }).click();
	await expect(page.locator('body')).toHaveAttribute('data-camera-stopped', 'yes');
	await expect(page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true })).toBeEnabled();
	await page.evaluate(() => document.dispatchEvent(new Event('attendance-test-release')));
	await expect
		.poll(() => api.writes.filter((write) => write.path.includes('/faces/')).length)
		.toBe(1);
	expect(
		api.writes.filter((write) => write.path.endsWith('/scans') || write.path === '/api/files')
	).toHaveLength(0);
});

test('failed enrollment retries the same three samples without taking a fourth', async ({
	page
}) => {
	await mockAttendance(page);
	let attempts = 0;
	const payloads: unknown[] = [];
	await page.route(`**/api/attendance/faces/${student}`, async (route) => {
		payloads.push(route.request().postDataJSON());
		return reply(route, ++attempts === 1 ? 'บันทึกไม่พร้อม' : {}, attempts === 1 ? 503 : 200);
	});
	await page.route('**/src/lib/features/attendance/face-camera.ts*', (route) =>
		route.fulfill({
			contentType: 'text/javascript',
			body: `
 let sample=0;export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};
 export async function readFace(){if(sample>=3)throw Error('unexpected fourth sample');return {values:Array(128).fill(.01),yaw:[0,.12,-.12][sample++]}};export async function readFaces(){throw Error('unused')};export function matchFace(){return null};export function distance(){return 0};export async function evidence(){throw Error('unused')};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'ลงทะเบียนใบหน้า', exact: true }).click();
	await page.getByRole('button', { name: 'นักเรียน', exact: true }).click();
	await page.getByRole('option', { name: /คนแรก/ }).click();
	await page.getByLabel('ยืนยันว่าได้รับความยินยอมและตรวจว่าเป็นนักเรียนคนที่เลือก').check();
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	for (let sample = 0; sample < 3; sample++)
		await page.getByRole('button', { name: `เก็บตัวอย่าง (${sample}/3)`, exact: true }).click();
	await expect(page.getByText('บันทึกไม่พร้อม', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'บันทึกตัวอย่างอีกครั้ง', exact: true }).click();
	await expect(page.getByText('ลงทะเบียนใบหน้าแล้ว', { exact: true }).first()).toBeVisible();
	expect(attempts).toBe(2);
	expect(payloads[1]).toEqual(payloads[0]);
});

test('bounded camera descriptor preprocessing matches the enrolled full-frame geometry', async ({
	page
}) => {
	test.setTimeout(90_000);
	await mockAttendance(page);
	await page.goto(path('/faces'));
	const geometry = await page.evaluate(async () => {
		const modulePath = '/src/lib/features/attendance/face-camera.ts';
		const cameraModule: typeof import('../../src/lib/features/attendance/face-camera') =
			await import(modulePath);
		const api = await cameraModule.faceEngine();
		const source = document.createElement('canvas');
		source.width = 640;
		source.height = 480;
		const context = source.getContext('2d');
		if (!context) throw Error('missing context');
		context.fillStyle = 'red';
		context.fillRect(0, 0, 640, 480);
		context.fillStyle = 'blue';
		context.fillRect(128, 96, 192, 144);
		const video = document.createElement('video');
		video.muted = true;
		video.srcObject = source.captureStream(10);
		await video.play();
		const detection = new api.FaceDetection(
			0.9,
			new api.Rect(0.2, 0.2, 0.3, 0.3),
			new api.Dimensions(640, 480)
		);
		const landmarks = new api.FaceLandmarks68(
			Array.from(
				{ length: 68 },
				(_, index) =>
					new api.Point(
						index >= 36 && index < 42 ? 0.2 : index >= 42 && index < 48 ? 0.8 : 0.5,
						index >= 36 && index < 48 ? 0.2 : index >= 48 ? 0.95 : 0.5
					)
			),
			{ width: 192, height: 144 }
		);
		const expectedBox = api
			.extendWithFaceLandmarks({ detection }, landmarks)
			.landmarks.align(undefined, { useDlibAlignment: true });
		const expected = (await api.extractFaces(source, [expectedBox]))[0];
		const summary = (canvas: HTMLCanvasElement) => ({
			width: canvas.width,
			height: canvas.height,
			pixel: Array.from(canvas.getContext('2d')!.getImageData(0, 0, 1, 1).data)
		});
		const previousDetector = api.nets.tinyFaceDetector.locateFaces;
		const previousLandmarks = api.nets.faceLandmark68TinyNet.detectLandmarks;
		const previousDescriptor = api.nets.faceRecognitionNet.computeFaceDescriptor;
		let observed: ReturnType<typeof summary> | null = null;
		try {
			api.nets.tinyFaceDetector.locateFaces = async () => [detection];
			api.nets.faceLandmark68TinyNet.detectLandmarks = async () => landmarks;
			api.nets.faceRecognitionNet.computeFaceDescriptor = async (input) => {
				if (!(input instanceof HTMLCanvasElement)) throw Error('unexpected descriptor source');
				observed = summary(input);
				return new Float32Array(128).fill(0.01);
			};
			const result = await cameraModule.readFaces(video);
			return {
				expected: summary(expected),
				observed,
				extendsDetection: expectedBox.width > detection.box.width,
				count: result.faces.length
			};
		} finally {
			api.nets.tinyFaceDetector.locateFaces = previousDetector;
			api.nets.faceLandmark68TinyNet.detectLandmarks = previousLandmarks;
			api.nets.faceRecognitionNet.computeFaceDescriptor = previousDescriptor;
			(video.srcObject as MediaStream).getTracks().forEach((track) => track.stop());
		}
	});
	expect(geometry.extendsDetection).toBe(true);
	expect(geometry.count).toBe(1);
	expect(geometry.observed).toEqual(geometry.expected);
});

test('preloaded same-context navigation replaces enrollment metadata without recreating the station', async ({
	page
}) => {
	await mockAttendance(page);
	let selected = student,
		reads = 0;
	await page.route(
		(url) => url.pathname === '/api/attendance/faces',
		(route) => {
			reads++;
			return reply(route, [{ studentId: selected, enrolledAt: '2026-10-01T08:00:00Z' }]);
		}
	);
	await page.goto(path('/faces'));
	await expect(page.getByRole('button', { name: /ถอนข้อมูลใบหน้า นักเรียนคนแรก/ })).toBeVisible();
	await page
		.locator('video')
		.evaluate((video) => (video.dataset.testIdentity = 'original-station'));
	selected = second;
	await page.evaluate(
		(href) => {
			const link = document.createElement('a');
			link.href = href;
			link.textContent = 'same context';
			link.setAttribute('data-sveltekit-preload-data', 'hover');
			link.id = 'same-context';
			document.querySelector('main')!.prepend(link);
		},
		path('/faces').replace('date=2026-10-09', 'date=invalid')
	);
	await page.locator('#same-context').hover();
	await expect.poll(() => reads).toBe(2);
	await page.locator('#same-context').click();
	await expect(page).toHaveURL(/date=invalid/);
	await expect(page.locator('video')).toHaveAttribute('data-test-identity', 'original-station');
	await expect(
		page.getByRole('button', { name: /ถอนข้อมูลใบหน้า นักเรียนคนที่สอง/ })
	).toBeVisible();
	await expect(page.getByRole('button', { name: /ถอนข้อมูลใบหน้า นักเรียนคนแรก/ })).toHaveCount(0);
});

for (const width of [390, 474])
	for (const theme of ['light', 'dark'] as const)
		test(`mobile round picker bounds long lists across all kinds ${width} ${theme}`, async ({
			page
		}) => {
			const api = await mockAttendance(page);
			await page.setViewportSize({ width, height: 884 });
			await page.emulateMedia({ colorScheme: theme });
			const rounds: AttendanceSession[] = (['arrival', 'lesson', 'special'] as const).flatMap(
				(kind, k) =>
					Array.from({ length: 120 }, (_, i) => ({
						...session,
						id: id(3000 + k * 200 + i),
						sourceKey: id(4000 + k * 200 + i),
						kind,
						title: `รอบ ${i + 1} · ${kind} กิจกรรมทดสอบที่มีชื่อยาวเพื่อดูการแสดงผลบนมือถือ`,
						rowVersion: 0
					}))
			);
			await page.route('**/api/attendance/workspace?**', (route) =>
				reply(route, {
					academicTermId: term,
					date,
					counted: true,
					lessonChoices: [],
					sessions: rounds,
					digestState: 'none'
				})
			);
			await page.route('**/api/attendance/preview?**', (route) => {
				const source = new URL(route.request().url()).searchParams.get('sourceKey');
				const selected = rounds.find((row) => row.sourceKey === source)!;
				return reply(route, { ...detail(), session: selected });
			});
			await page.goto(path());
			for (const [kind, label] of [
				['arrival', 'เข้าโรงเรียน'],
				['lesson', 'รายคาบ'],
				['special', 'รอบพิเศษ']
			] as const) {
				await page
					.getByRole('group', { name: 'ประเภทรอบเช็คชื่อ' })
					.getByRole('button', { name: label, exact: true })
					.click();
				const picker = page.getByRole('button', { name: 'รอบเช็คชื่อ', exact: true });
				await expect(picker).toBeInViewport();
				await picker.click();
				await page.getByLabel('ค้นหา รอบเช็คชื่อ', { exact: true }).fill('รอบ 120 ·');
				await page.getByRole('option', { name: new RegExp(`รอบ 120 · ${kind}`) }).click();
				await expect(resultControl(page, 'นักเรียนคนแรก')).toBeInViewport();
				expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
					true
				);
				expect((await page.locator('tbody tr').first().boundingBox())!.height).toBeLessThan(160);
				await expect(page.getByText('เพิ่มหมายเหตุ', { exact: true })).toHaveCount(0);
				await chooseResult(page, 'นักเรียนคนแรก', 'มา');
				await page.getByLabel('ค้นหาชื่อนักเรียน').fill('ไม่ตรงกับรายชื่อ');
				await expect(page.getByText('ไม่พบชื่อนักเรียนที่ค้นหา', { exact: true })).toBeVisible();
				await page.getByLabel('ค้นหาชื่อนักเรียน').fill('');
				await expectResult(page, 'นักเรียนคนแรก', 'มา');
				await page.screenshot({ path: `/tmp/attendance-compact-${width}-${theme}-${kind}.png` });
			}
			expect(api.writes).toHaveLength(0);
		});

test.describe('compact touch-enabled attendance controls', () => {
	test.use({ hasTouch: true });
	for (const width of [320, 488, 1440])
		for (const theme of ['light', 'dark'] as const)
			test(`direct result controls need one tap and retain keyboard selection ${width} ${theme}`, async ({
				page
			}) => {
				const api = await mockAttendance(page);
				await page.setViewportSize({ width, height: 884 });
				await page.emulateMedia({ colorScheme: theme });
				await page.goto(path());
				await chooseRound(page);
				const control = resultControl(page, 'นักเรียนคนแรก');
				await expect(control.getByRole('radio', { checked: true })).toHaveCount(0);
				await expect(control.getByRole('radio')).toHaveCount(7);
				const present = control.getByRole('radio', { name: 'มา', exact: true });
				const late = control.getByRole('radio', { name: 'สาย', exact: true });
				for (const button of [
					present,
					late,
					control.getByRole('radio', { name: 'กิจกรรม', exact: true })
				]) {
					const bounds = (await button.boundingBox())!;
					expect(bounds.height).toBeLessThan(44);
					expect(bounds.width).toBeGreaterThan(0);
				}
				await present.tap();
				await expect(present).toHaveAttribute('aria-checked', 'true');
				await present.tap();
				await expect(present).toHaveAttribute('aria-checked', 'true');
				await present.focus();
				await page.keyboard.press('ArrowRight');
				await expect(late).toBeFocused();
				await page.keyboard.press('Space');
				await expect(late).toHaveAttribute('aria-checked', 'true');
				await page.keyboard.press('Enter');
				await expect(late).toHaveAttribute('aria-checked', 'true');
				await control
					.getByRole('button', { name: 'ยกเลิกการเลือกผลของ นักเรียนคนแรก', exact: true })
					.tap();
				await expect(control.getByRole('radio', { checked: true })).toHaveCount(0);
				expect(api.writes).toHaveLength(0);
				await chooseResult(page, 'นักเรียนคนแรก', 'มา');
				await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
				await expect(
					control.getByRole('button', { name: 'ยกเลิกการเลือกผลของ นักเรียนคนแรก', exact: true })
				).toHaveCount(0);
				expect(api.writes[0].data).toMatchObject({
					students: [{ studentId: student, result: 'present' }]
				});
				expect(
					(api.writes[0].data as SaveAttendanceResults).students.every(
						(row) => !Object.hasOwn(row, 'note')
					)
				).toBe(true);
				await expect(page.getByText('เพิ่มหมายเหตุ', { exact: true })).toHaveCount(0);
				await expect(page.getByLabel(/หมายเหตุ(?:เพิ่มเติม)?ของ/)).toHaveCount(0);
				expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
					true
				);
				await page.screenshot({ path: `/tmp/attendance-direct-tap-${width}-${theme}.png` });
			});
});

for (const width of [390, 1440])
	for (const theme of ['light', 'dark'] as const)
		test(`lesson room filters preserve draft and result colors ${width} ${theme}`, async ({
			page
		}) => {
			const api = await mockAttendance(page);
			await page.setViewportSize({ width, height: 884 });
			await page.emulateMedia({ colorScheme: theme });
			const otherRoom = id(8201);
			const rounds: AttendanceSession[] = Array.from({ length: 24 }, (_, i) => ({
				...session,
				id: id(8300 + i),
				sourceKey: id(8400 + i),
				kind: 'lesson',
				rowVersion: 0,
				startTime: i % 12 ? '09:30:00' : '08:40:00',
				title: `วิชาทดสอบ ${(i % 12) + 1} · ${i < 12 ? 'ม.1/1' : 'ม.2/3'} · วิชาทดสอบ ${(i % 12) + 1} · คาบที่ ${(i % 12) + 1}`
			}));
			const lessonChoices: AttendanceWorkspace['lessonChoices'] = rounds.map((round, i) => ({
				sessionId: round.id,
				periodName: `คาบที่ ${(i % 12) + 1}`,
				offeringName: `วิชาทดสอบ ${(i % 12) + 1}`,
				groupName: `${i < 12 ? 'ม.1/1' : 'ม.2/3'} · วิชาทดสอบ ${(i % 12) + 1}`,
				homerooms: [{ id: i < 12 ? room : otherRoom, name: i < 12 ? 'ม.1/1' : 'ม.2/3' }]
			}));
			lessonChoices[1].offeringName =
				'โลก ดาราศาสตร์และอวกาศ การสำรวจและการเรียนรู้ทางวิทยาศาสตร์เพิ่มเติม';
			const mixed = { ...rounds[0], id: id(8500), sourceKey: id(8501), title: 'กิจกรรมรวมสองห้อง' };
			const independent = {
				...rounds[0],
				id: id(8502),
				sourceKey: id(8503),
				title: 'กิจกรรมอิสระ'
			};
			const historical = {
				...rounds[0],
				id: id(8504),
				sourceKey: id(8505),
				title: 'รอบย้อนหลังที่ไม่อยู่ในตารางปัจจุบัน'
			};
			rounds.push(mixed, independent, historical);
			lessonChoices.push(
				{
					sessionId: mixed.id,
					periodName: 'คาบที่ 3',
					offeringName: mixed.title,
					groupName: 'กลุ่มรวม',
					homerooms: [...lessonChoices[0].homerooms, ...lessonChoices[12].homerooms]
				},
				{
					sessionId: independent.id,
					periodName: 'คาบที่ 4',
					offeringName: independent.title,
					groupName: 'กลุ่มอิสระ',
					homerooms: []
				}
			);
			await page.route('**/api/attendance/workspace?**', (route) =>
				reply(route, {
					date,
					counted: true,
					sessions: rounds,
					lessonChoices,
					settings
				} satisfies AttendanceWorkspace)
			);
			await page.route('**/api/attendance/preview?**', (route) => {
				const source = new URL(route.request().url()).searchParams.get('sourceKey');
				const selected = rounds.find((round) => round.sourceKey === source)!;
				return reply(route, { ...detail(), session: selected });
			});
			await page.goto(path());
			await page
				.getByRole('group', { name: 'ประเภทรอบเช็คชื่อ' })
				.getByRole('button', { name: 'รายคาบ', exact: true })
				.click();
			const roomPicker = page.getByRole('button', { name: 'กรองห้องเรียน', exact: true });
			const roundPicker = page.getByRole('button', { name: 'รอบเช็คชื่อ', exact: true });
			await roomPicker.click();
			await page.getByRole('option', { name: 'ม.1/1', exact: true }).click();
			await expect(roundPicker).toContainText('13 รอบ');
			await roundPicker.click();
			await expect(page.getByRole('option')).toHaveCount(13);
			await expect(page.getByRole('option', { name: /กิจกรรมรวมสองห้อง/ })).toBeVisible();
			const first = page.getByRole('option', { name: /คาบที่ 1 · 08:40 · วิชาทดสอบ 1/ });
			await expect(first).toHaveText('คาบที่ 1 · 08:40 · วิชาทดสอบ 1ม.1/1');
			for (const option of [first, page.getByRole('option', { name: /คาบที่ 2 · 09:30 · โลก/ })]) {
				const geometry = await option.evaluate((element) => {
					const row = element.lastElementChild!;
					const [main, room] = Array.from(row.children).map((child) =>
						child.getBoundingClientRect()
					);
					return {
						mainCenter: main.y + main.height / 2,
						roomCenter: room.y + room.height / 2,
						separated: main.right <= room.left,
						inside: room.right <= element.getBoundingClientRect().right
					};
				});
				expect(Math.abs(geometry.mainCenter - geometry.roomCenter)).toBeLessThan(1);
				expect(geometry.separated && geometry.inside).toBe(true);
			}
			await expect(page.getByRole('option', { name: /คาบที่ 2 · 09:30 · โลก/ })).toHaveAttribute(
				'title',
				/การเรียนรู้ทางวิทยาศาสตร์เพิ่มเติม · ม\.1\/1/
			);
			await expect(page.locator('[data-slot="select-content"]')).toHaveCSS('opacity', '1');
			await page.screenshot({ path: `/tmp/attendance-lesson-menu-${width}-${theme}.png` });
			await first.click();
			const result = resultControl(page, 'นักเรียนคนแรก');
			const colors = new Set<string>();
			for (const text of ['มา', 'สาย', 'ขาด', 'ลากิจ', 'กิจกรรม']) {
				const button = result.getByRole('radio', { name: text, exact: true });
				await button.click();
				await expect(button).toHaveAttribute('aria-checked', 'true');
				await expect(result.getByRole('radio', { checked: true })).toHaveCount(1);
				await button.evaluate(async (element) => {
					await Promise.all(element.getAnimations().map((animation) => animation.finished));
				});
				colors.add(await button.evaluate((el) => getComputedStyle(el).backgroundColor));
			}
			expect(colors.size).toBe(5);
			await result.getByRole('radio', { name: 'กิจกรรม', exact: true }).click();
			await expectResult(page, 'นักเรียนคนแรก', 'กิจกรรม');
			await result
				.getByRole('button', { name: 'ยกเลิกการเลือกผลของ นักเรียนคนแรก', exact: true })
				.click();
			await expectResult(page, 'นักเรียนคนแรก', 'ยังไม่ได้เช็ค');
			await chooseResult(page, 'นักเรียนคนแรก', 'มา');
			await result.getByRole('radio', { name: 'มา', exact: true }).evaluate(async (element) => {
				await Promise.all(element.getAnimations().map((animation) => animation.finished));
			});
			await page.screenshot({ path: `/tmp/attendance-direct-results-${width}-${theme}.png` });
			const filterReads: string[] = [];
			page.on('request', (request) => {
				if (new URL(request.url()).pathname.startsWith('/api/attendance/'))
					filterReads.push(request.url());
			});
			await roomPicker.click();
			await page.getByRole('option', { name: 'ม.2/3', exact: true }).click();
			await expect(roundPicker).toContainText('13 รอบ');
			await expectResult(page, 'นักเรียนคนแรก', 'มา');
			await expect(page.getByText('รอบนี้อยู่นอกห้องที่กรอง', { exact: true })).toBeVisible();
			expect(filterReads).toHaveLength(0);
			await roundPicker.click();
			await expect(page.getByRole('option', { name: /ม\.1\/1/ })).toHaveCount(0);
			await expect(page.getByRole('option', { name: /กิจกรรมรวมสองห้อง/ })).toBeVisible();
			await page.keyboard.press('Escape');
			await roomPicker.click();
			await page.getByRole('option', { name: 'ทุกห้อง', exact: true }).click();
			await expect(roundPicker).toContainText('คาบที่ 1 · 08:40 · วิชาทดสอบ 1');
			await expectResult(page, 'นักเรียนคนแรก', 'มา');
			await expect(page.getByText('รอบนี้อยู่นอกห้องที่กรอง', { exact: true })).toHaveCount(0);
			await page.screenshot({ path: `/tmp/attendance-lesson-filter-${width}-${theme}.png` });
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await roomPicker.click();
			await page.getByRole('option', { name: 'ห้องอื่น / กลุ่มอิสระ', exact: true }).click();
			await expect(roundPicker).toContainText('2 รอบ');
			await roundPicker.click();
			await expect(page.getByRole('option', { name: /กิจกรรมอิสระ/ })).toBeVisible();
			await page.getByRole('option', { name: /รอบย้อนหลังที่ไม่อยู่ในตารางปัจจุบัน/ }).click();
			await expect(
				page.getByRole('heading', { name: /รอบย้อนหลังที่ไม่อยู่ในตารางปัจจุบัน/ })
			).toBeVisible();
			expect(api.writes).toHaveLength(0);
		});

test('all rounds offer seven writable statuses and lesson/special show school arrival separately', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await mockAttendance(page);
	const rounds: AttendanceSession[] = (['arrival', 'lesson', 'special'] as const).map(
		(kind, index) => ({
			...session,
			id: id(9000 + index),
			kind,
			title: `รอบ ${kind}`,
			sourceKey: id(9010 + index)
		})
	);
	await page.route('**/api/attendance/workspace?**', (route) =>
		reply(route, {
			date,
			counted: true,
			lessonChoices: [],
			sessions: rounds,
			settings
		} satisfies AttendanceWorkspace)
	);
	await page.route('**/api/attendance/sessions/*', (route) => {
		const selected = rounds.find((s) => route.request().url().split('/').at(-1) === s.id);
		return reply(route, {
			...detail(),
			session: selected ?? session,
			students: detail().students.map((row) => ({ ...row, arrivalResult: 'sick_leave' }))
		} satisfies AttendanceDetail);
	});
	await page.goto(path());
	for (const [kind, label] of [
		['arrival', 'เข้าโรงเรียน'],
		['lesson', 'รายคาบ'],
		['special', 'รอบพิเศษ']
	] as const) {
		await page
			.getByRole('group', { name: 'ประเภทรอบเช็คชื่อ' })
			.getByRole('button', { name: label, exact: true })
			.click();
		await chooseRound(page, new RegExp(`รอบ ${kind}`));
		const control = resultControl(page, 'นักเรียนคนแรก');
		await expect(control.getByRole('radio')).toHaveCount(7);
		for (const label of ['มา', 'สาย', 'ขาด', 'หนีเรียน', 'ลาป่วย', 'ลากิจ', 'กิจกรรม'])
			await expect(control.getByRole('radio', { name: label, exact: true })).toBeVisible();
		if (kind !== 'arrival')
			await expect(
				page
					.locator('tbody tr')
					.filter({ hasText: 'นักเรียนคนแรก' })
					.locator('[data-slot="badge"]:visible')
			).toHaveText('ลาป่วย');
		await chooseResult(page, 'นักเรียนคนแรก', 'หนีเรียน');
		await expectResult(page, 'นักเรียนคนแรก', 'หนีเรียน');
	}
});

test('special scoring requires Behavior management and sends exactly seven independent rules', async ({
	page
}) => {
	const api = await mockAttendance(page, {
		permissions: [
			PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL,
			PERMISSIONS.BEHAVIOR_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_CONTEXT_READ_SCHOOL
		]
	});
	await page.goto(path('/settings'));
	await page.getByRole('checkbox', { name: 'กำหนดคะแนนเฉพาะรอบนี้', exact: true }).check();
	await page.getByRole('button', { name: 'คะแนนเมื่อหนีเรียน', exact: true }).click();
	await page.getByRole('option', { name: 'หักคะแนน', exact: true }).click();
	await page.getByLabel('จำนวนคะแนนเมื่อหนีเรียน', { exact: true }).fill('5');
	await page.getByLabel('ชื่อการเช็คชื่อ').fill('รอบคะแนนเฉพาะ');
	await page.getByRole('button', { name: 'สร้างรอบพิเศษ', exact: true }).click();
	await expect(page.getByText('รอบคะแนนเฉพาะ · 1 วัน / 1 กลุ่ม')).toBeVisible();
	expect(api.writes.find((w) => w.path.includes('/specials/'))?.data).toMatchObject({
		scoring: [
			'present',
			'late',
			'absent',
			'skipped',
			'sick_leave',
			'personal_leave',
			'activity'
		].map((result) => ({
			result,
			action: result === 'skipped' ? 'deduction' : 'none',
			points: result === 'skipped' ? 5 : 0
		}))
	});
});

test('Attendance manager without Behavior management uses inherited special scoring', async ({
	page
}) => {
	await mockAttendance(page, {
		permissions: [PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL, PERMISSIONS.ACADEMIC_CONTEXT_READ_SCHOOL]
	});
	await page.goto(path('/settings'));
	await expect(
		page.getByRole('checkbox', { name: 'กำหนดคะแนนเฉพาะรอบนี้', exact: true })
	).toHaveCount(0);
	await expect(page.getByLabel('ชื่อการเช็คชื่อ')).toBeVisible();
});
