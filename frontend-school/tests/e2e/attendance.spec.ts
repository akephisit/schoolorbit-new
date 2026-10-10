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

for (const invalidDate of ['bad', '2026-02-30'])
	test(`invalid attendance date ${invalidDate} is repaired without crashing settings`, async ({
		page
	}) => {
		await mockAttendance(page);
		await page.goto(path('/settings').replace(`date=${date}`, `date=${invalidDate}`));
		await expect(page.getByRole('heading', { name: 'ปฏิทินวันประมวลผล' })).toBeVisible();
		await expect(page.getByLabel('เดือน', { exact: true })).toHaveValue('2026-10');
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
	await page.getByLabel('เดือน', { exact: true }).fill('2026-11');
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
			await reply(route, { archived: false, activityCountsAsPresent: true, summaries: [] });
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
			await reply(route, { archived: false, activityCountsAsPresent: true, summaries: [] });
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
			body: `export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(stream){if(stream) document.body.dataset.attendanceStopped='yes'};export async function readFace(){return null};export function matchFace(){return null};export function distance(){return 0};export async function evidence(){throw new Error('unexpected capture')};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'เครื่องสแกน', exact: true }).click();
	await page.getByRole('option', { name: 'เว็บแคมหน้าโรงเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้', exact: true }).click();
	await expect(page.getByRole('status').filter({ hasText: 'มองตรงเข้ากล้องทีละคน' })).toBeVisible();
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
			body: `let frame=0, release;export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};export async function readFace(){return {values:Array(128).fill(.01),yaw:[0,-.12,0][frame++%3]}};export function matchFace(){return '${student}'};export function distance(){return 0};export async function evidence(){document.body.dataset.capturePending='yes';await new Promise(resolve=>release=resolve);return new File(['fixture'],'scan.jpg',{type:'image/jpeg'})};export function releaseEvidence(){release()};`
		})
	);
	await page.goto(path('/faces'));
	await page.getByRole('button', { name: 'เปิดเว็บแคม', exact: true }).click();
	await page.getByRole('button', { name: 'เครื่องสแกน', exact: true }).click();
	await page.getByRole('option', { name: 'เว็บแคมหน้าโรงเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'เริ่มเช็คชื่อวันนี้', exact: true }).click();
	await expect(page.locator('body')).toHaveAttribute('data-capture-pending', 'yes');
	await page.clock.setFixedTime(new Date('2026-10-09T17:01:00Z'));
	await page.evaluate(async () => {
		const modulePath = '/src/lib/features/attendance/face-camera.ts';
		const { releaseEvidence } = await import(modulePath);
		releaseEvidence();
	});
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
						leave: 0,
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
						title: 'เข้าโรงเรียน',
						result: 'present',
						note:
							new URL(request.url()).searchParams.get('studentId') === student
								? 'หลักฐานคนแรก'
								: 'รายละเอียดคนที่สอง',
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
test('refresh and search preserve the roster draft without submitting it', async ({ page }) => {
	const api = await mockAttendance(page);
	await page.goto(path());
	await page.getByRole('button', { name: /08:00.*หน้าเสาธง/ }).click();
	await page.getByRole('button', { name: 'ผลของ นักเรียนคนแรก' }).click();
	await page.getByRole('option', { name: 'มา', exact: true }).click();
	await page.getByLabel('เหตุผลแก้ไข / งดคาบ').fill('ตรวจซ้ำ');
	await page.getByRole('button', { name: 'โหลดผลล่าสุด', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ผลของ นักเรียนคนแรก' })).toHaveText('มา');
	await expect(page.getByLabel('เหตุผลแก้ไข / งดคาบ')).toHaveValue('ตรวจซ้ำ');
	await page.getByLabel('ค้นหาชื่อนักเรียน').fill('ไม่มีชื่อนี้');
	await page.getByLabel('ค้นหาชื่อนักเรียน').press('Enter');
	await expect(page.getByText('ไม่พบชื่อนักเรียนที่ค้นหา', { exact: true })).toBeVisible();
	await page.getByLabel('ค้นหาชื่อนักเรียน').fill('');
	await expect(page.getByRole('button', { name: 'ผลของ นักเรียนคนแรก' })).toHaveText('มา');
	expect(api.writes).toHaveLength(0);
});
test('read-only teacher cannot create an unsaved session', async ({ page }) => {
	const api = await mockAttendance(page, {
		permissions: ['attendance.read.assigned', 'academic_context.read.school']
	});
	await page.route('**/api/attendance/workspace?**', (route) =>
		reply(route, {
			date,
			counted: true,
			sessions: [{ ...session, rowVersion: 0 }],
			settings
		} satisfies AttendanceWorkspace)
	);
	await page.goto(path());
	await expect(page.getByRole('button', { name: /08:00.*หน้าเสาธง/ })).toBeDisabled();
	expect(api.writes).toHaveLength(0);
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
	await page.getByRole('button', { name: /08:00.*หน้าเสาธง/ }).click();
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
test('settings save succeeds twice and calendar uses the returned revision', async ({ page }) => {
	const api = await mockAttendance(page);
	await page.goto(path('/settings'));
	await page.getByLabel('เข้าสายหลังเวลา').fill('08:30');
	await page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true }).click();
	await expect(page.getByText(/could not be cloned|structuredClone/)).toHaveCount(0);
	await expect(page.getByText('บันทึกแล้ว', { exact: true })).toBeVisible();
	await page.getByLabel('เข้าสายหลังเวลา').fill('08:45');
	await page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true }).click();
	await page.getByRole('button', { name: '9 ✓', exact: true }).click();
	await expect(page.getByRole('button', { name: '9 —', exact: true })).toBeVisible();
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
	await page.getByRole('button', { name: '9 ✓', exact: true }).click();
	await expect(page.getByRole('button', { name: '9 —', exact: true })).toBeVisible();
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
	await expect(page.getByRole('button', { name: '10 —', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true }).click();
	await expect(page.getByRole('button', { name: '10 ✓', exact: true })).toBeVisible();
});
test('empty or failed month has an actionable error and cannot write guessed days', async ({
	page
}) => {
	const api = await mockAttendance(page);
	await page.goto(path('/settings'));
	await page.getByLabel('เดือน', { exact: true }).fill('');
	await expect(page.getByText('เลือกเดือนที่ต้องการ', { exact: true })).toBeVisible();
	await page.route(
		(url) => url.pathname === '/api/attendance/days',
		(r) => reply(r, 'ปฏิทินไม่พร้อม', 500)
	);
	await page.getByLabel('เดือน', { exact: true }).fill('2026-11');
	await expect(page.getByText('ปฏิทินไม่พร้อม', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: '2 ✓', exact: true })).toBeDisabled();
	expect(api.writes).toHaveLength(0);
});
test('read-only teacher sees saved results without management links or optional reads', async ({
	page
}) => {
	const api = await mockAttendance(page, {
		permissions: ['attendance.read.assigned', 'academic_context.read.school']
	});
	await page.goto(path());
	await page.getByRole('button', { name: /08:00.*หน้าเสาธง/ }).click();
	await expect(page.getByRole('button', { name: 'ผลของ นักเรียนคนแรก' })).toBeDisabled();
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
			body: `export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};export async function readFace(){return {values:Array(128).fill(.01),yaw:0}};export function matchFace(){return null};export function distance(){return 0};export async function evidence(){throw Error('unused')};`
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
			body: `export const FACE_MODEL='face-api-1.7.15-recognition-128';export async function faceEngine(){return {}};export async function camera(){return {getTracks:()=>[]}};export function stopCamera(){};export function readFace(){return new Promise(resolve=>window.addEventListener('test-face-ready',()=>resolve({values:Array(128).fill(.01),yaw:0}),{once:true}))};export function matchFace(){return null};export function distance(){return 0};export async function evidence(){throw Error('unused')};`
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
	await page.getByLabel('ชื่อกลุ่ม', { exact: true }).fill('กลุ่มชุมนุม');
	await page.getByRole('button', { name: 'สมาชิกกลุ่ม', exact: true }).click();
	await page.getByRole('option', { name: 'ม.1/1 · นักเรียนคนแรก', exact: true }).click();
	await page.keyboard.press('Escape');
	await page.getByRole('button', { name: 'เพิ่มกลุ่ม', exact: true }).click();
	await expect(page.getByText('กลุ่มชุมนุม · 1 คน')).toBeVisible();
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
				if (!suffix) await page.getByRole('button', { name: /08:00.*หน้าเสาธง/ }).click();
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
