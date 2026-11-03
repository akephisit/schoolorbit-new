import { expect, test, type Page } from '@playwright/test';
import { sessionState } from './fixtures/session-state';
import { id, fulfill, chooseOption } from './fixtures/year-management';
import type { components } from '../../src/lib/api/generated/school-api';
type S = components['schemas'];
test.use({ serviceWorkers: 'block' });
const time = '2026-11-03T00:00:00Z';
const yearId = id(501),
	firstTermId = id(502),
	nextTermId = id(503),
	scheduleId = id(504),
	roomId = id(505);
async function fixture(
	page: Page,
	options: {
		closed?: boolean;
		capacityConflict?: boolean;
		periodConflict?: boolean;
		termManagerOnly?: boolean;
		hiddenPlanning?: boolean;
		timetableDraft?: boolean;
	} = {}
) {
	const status = options.hiddenPlanning
		? ('planning' as const)
		: options.closed
			? ('closed' as const)
			: ('active' as const);
	const year = {
		id: yearId,
		year: 2569,
		name: 'ปีการศึกษา 2569',
		schoolDays: ['MON', 'TUE', 'WED', 'THU', 'FRI'],
		status,
		isVisible: true,
		rowVersion: 1,
		migrated: false,
		createdAt: time,
		updatedAt: time
	};
	const terms = [firstTermId, nextTermId].map((termId, index) => ({
		id: termId,
		academicYearId: yearId,
		sequence: index + 1,
		code: String(index + 1),
		name: `ภาคเรียนที่ ${index + 1}`,
		termType: 'regular' as const,
		status: index === 0 ? ('active' as const) : ('planning' as const),
		bellScheduleId: scheduleId,
		includedInYearResult: true,
		blocksYearClosure: true,
		rowVersion: 1,
		isVisible: true,
		migrated: false,
		createdAt: time,
		updatedAt: time
	}));
	let room = {
		id: roomId,
		academicYearId: yearId,
		gradeLevelId: id(506),
		studyProgramId: id(507),
		code: 'M1-1',
		name: 'ม.1/1',
		roomNumber: '1',
		capacity: 40,
		studentCount: 35,
		rowVersion: 7,
		migrated: false,
		createdAt: time,
		updatedAt: time
	};
	const schedules: S['BellSchedule'][] = [
		{
			id: scheduleId,
			academicYearId: yearId,
			code: 'NORMAL',
			name: 'ตารางเดิมภาคเรียนที่ 1',
			isDefault: true,
			status: 'published',
			rowVersion: 4,
			owningOrganizationUnitId: null,
			editBlockedReason: options.timetableDraft ? 'timetable_references' : 'already_used',
			canClone: true,
			createdAt: time,
			updatedAt: time
		}
	];
	const periods: S['BellSchedulePeriod'][] = [
		{
			id: id(508),
			bellScheduleId: scheduleId,
			name: 'คาบเช้า',
			startTime: '08:30:00',
			endTime: '09:20:00',
			orderIndex: 1,
			isActive: true,
			applicableDays: 'MON,TUE,WED,THU,FRI'
		}
	];
	const observed = {
		capacities: [] as S['UpdateHomeroomCapacityRequest'][],
		clones: [] as S['CloneBellScheduleRequest'][],
		assignments: [] as S['UpdateAcademicTermRequest'][],
		periodWrites: [] as { id: string; body: S['ReplaceBellSchedulePeriodsRequest'] }[],
		setupReads: 0
	};
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const path = new URL(route.request().url()).pathname,
				method = route.request().method();
			if (path === '/api/auth/me')
				return fulfill(route, {
					id: id(599),
					username: 'period-synthetic',
					firstName: 'วิชา',
					lastName: 'ทดสอบ',
					displayName: 'วิชา ทดสอบ',
					userType: 'staff',
					status: 'active',
					session: sessionState(),
					permissions: options.hiddenPlanning
						? ['academic_context.read.school', 'homeroom.read.school', 'homeroom.manage.school']
						: options.termManagerOnly
							? ['academic_year.read.school', 'academic_term.manage.school']
							: ['*']
				});
			if (path === '/api/academic/context/options')
				return fulfill(route, {
					defaultAcademicYearId: yearId,
					defaultAcademicTermId: firstTermId,
					years: options.hiddenPlanning ? [] : [year],
					terms
				});
			if (path === '/api/academic/setup/workspace') {
				observed.setupReads++;
				return fulfill(route, {
					years: options.hiddenPlanning ? [] : [year],
					terms,
					bellSchedules: schedules,
					bellScheduleSetupYearIds: options.closed ? [] : [yearId],
					bellScheduleSetupTermIds: options.timetableDraft ? [] : [nextTermId],
					contextSettings: {
						defaultAcademicYearId: yearId,
						defaultAcademicTermId: firstTermId,
						rowVersion: 1
					}
				});
			}
			if (path === '/api/academic/homerooms/year-context')
				return fulfill(route, {
					academicYearId: yearId,
					name: year.name,
					year: year.year,
					status,
					rowVersion: 1
				});
			if (path === '/api/academic/homerooms') return fulfill(route, [room]);
			if (path === '/api/academic/homerooms/advisors' || path.includes('/homeroom-advisors'))
				return fulfill(route, []);
			if (path === `/api/academic/homerooms/${roomId}`) return fulfill(route, room);
			if (path === `/api/academic/homerooms/${roomId}/capacity`) {
				const body = route.request().postDataJSON();
				observed.capacities.push(body);
				if (options.capacityConflict && observed.capacities.length === 1) {
					room = { ...room, capacity: 42, rowVersion: 8 };
					return fulfill(route, 'ข้อมูลห้องเปลี่ยนแล้ว', 409);
				}
				room = { ...room, capacity: body.capacity, rowVersion: room.rowVersion + 1 };
				return fulfill(route, room);
			}
			if (path.endsWith('/clone') && path.includes('/bell-schedules/')) {
				const body = route.request().postDataJSON();
				observed.clones.push(body);
				const clone: S['BellSchedule'] = {
					...schedules[0],
					id: id(510),
					name: body.name,
					isDefault: false,
					status: 'draft',
					editBlockedReason: null,
					rowVersion: 1
				};
				schedules.push(clone);
				return fulfill(route, clone, 201);
			}
			if (path === '/api/academic/bell-schedules') return fulfill(route, schedules);
			if (path.includes('/bell-schedules/') && path.endsWith('/periods')) {
				const selected = path.split('/').at(-2)!;
				if (method === 'PUT') {
					const body = route.request().postDataJSON();
					observed.periodWrites.push({ id: selected, body });
					const schedule = schedules.find((item) => item.id === selected)!;
					schedule.rowVersion++;
					if (options.periodConflict && observed.periodWrites.length === 1)
						return fulfill(route, 'คาบเรียนเปลี่ยนระหว่างแก้ไข', 409);
					return fulfill(
						route,
						body.periods.map((period: S['BellSchedulePeriodInput']) => ({
							...period,
							id: id(511),
							bellScheduleId: selected,
							applicableDays: period.applicableDays.join(',')
						}))
					);
				}
				return fulfill(
					route,
					periods.map((period) => ({
						...period,
						bellScheduleId: selected,
						id: selected === scheduleId ? period.id : id(511)
					}))
				);
			}
			if (path === `/api/academic/terms/${nextTermId}` && method === 'PATCH') {
				const body = route.request().postDataJSON();
				observed.assignments.push(body);
				terms[1] = {
					...terms[1],
					bellScheduleId: body.bellScheduleId,
					rowVersion: terms[1].rowVersion + 1
				};
				return fulfill(route, terms[1]);
			}
			if (path === '/api/menu/user') return fulfill(route, { groups: [] });
			if (path.includes('grade-level'))
				return fulfill(route, [
					{
						id: id(506),
						name: 'มัธยมศึกษาปีที่ 1',
						short_name: 'ม.1',
						level_type: 'secondary',
						year: 1
					}
				]);
			if (path.includes('study-program')) return fulfill(route, []);
			return fulfill(route, []);
		}
	);
	return observed;
}

test('active-year manager changes only capacity and preserves drafts through stale revision review', async ({
	page
}) => {
	const observed = await fixture(page, { capacityConflict: true });
	await page.goto(`/staff/academic/homerooms?academicYearId=${yearId}`);
	await expect(page.getByRole('button', { name: 'เพิ่มห้องประจำชั้น', exact: true })).toHaveCount(
		0
	);
	await page.getByRole('button', { name: 'แก้ไขความจุ ม.1/1', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByLabel('ระดับชั้น', { exact: true })).toHaveCount(0);
	await dialog.getByLabel('ความจุ (คน)', { exact: true }).fill('34');
	await dialog.getByRole('button', { name: 'บันทึกความจุ', exact: true }).click();
	expect(observed.capacities).toEqual([]);
	await dialog.getByLabel('ความจุ (คน)', { exact: true }).fill('45');
	await dialog.getByRole('button', { name: 'บันทึกความจุ', exact: true }).click();
	await expect(dialog.getByText(/ความจุล่าสุด 42 คน/)).toBeVisible();
	await expect(dialog.getByLabel('ความจุ (คน)', { exact: true })).toHaveValue('45');
	await expect(dialog.getByRole('button', { name: 'บันทึกความจุ', exact: true })).toBeDisabled();
	await dialog
		.getByRole('button', { name: 'ตรวจสอบแล้ว ใช้ข้อมูลล่าสุดเป็นฐาน', exact: true })
		.click();
	await dialog.getByRole('button', { name: 'บันทึกความจุ', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(observed.capacities).toEqual([
		{ capacity: 45, rowVersion: 7 },
		{ capacity: 45, rowVersion: 8 }
	]);
	await expect(page.getByRole('cell', { name: '35 / 45', exact: true })).toBeVisible();
});

test('closed-year capacity is read-only on a narrow mobile screen', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	const observed = await fixture(page, { closed: true });
	await page.goto(`/staff/academic/homerooms?academicYearId=${yearId}`);
	await expect(page.getByTestId('homerooms-ready')).toBeVisible();
	await expect(page.getByLabel('ปีการศึกษาของห้องประจำชั้น', { exact: true })).toContainText(
		'ปีการศึกษา 2569'
	);
	await expect(
		page.getByRole('button', { name: /แก้ไขความจุ|เพิ่มห้องประจำชั้น|จัดครูที่ปรึกษา/ })
	).toHaveCount(0);
	expect(observed.capacities).toEqual([]);
});

test('term two clones and edits independent periods while term one remains locked', async ({
	page
}) => {
	const observed = await fixture(page);
	await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
	await expect(page.getByText(/ตารางนี้ใช้กับภาคเรียนที่เริ่มใช้แล้ว/)).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'แก้ไข ตารางเดิมภาคเรียนที่ 1', exact: true })
	).toHaveCount(0);
	await page.getByRole('button', { name: 'คัดลอก ตารางเดิมภาคเรียนที่ 1', exact: true }).click();
	await page.getByLabel('ชื่อตารางที่คัดลอก', { exact: true }).fill('ตารางภาคเรียนที่ 2');
	await page.getByRole('button', { name: 'ยืนยันคัดลอกตาราง', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกตารางเวลาของภาคเรียน', exact: true }).click();
	await chooseOption(
		page,
		page.getByLabel('ตารางเวลาที่จะจัดคาบ', { exact: true }),
		'ตารางภาคเรียนที่ 2'
	);
	await page.getByLabel('เริ่ม', { exact: true }).fill('09:00');
	await page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true }).click();
	await expect.poll(() => observed.periodWrites.length).toBe(1);
	expect(observed.clones).toEqual([
		{
			academicYearId: yearId,
			name: 'ตารางภาคเรียนที่ 2',
			owningOrganizationUnitId: null,
			rowVersion: 4
		}
	]);
	expect(observed.assignments[0].bellScheduleId).toBe(id(510));
	expect(observed.assignments[0].rowVersion).toBe(1);
	expect(observed.periodWrites[0].id).toBe(id(510));
	expect(observed.periodWrites[0].body.rowVersion).toBe(1);
	await chooseOption(
		page,
		page.getByLabel('ตารางเวลาที่จะจัดคาบ', { exact: true }),
		'ตารางเดิมภาคเรียนที่ 1 · ตารางหลัก'
	);
	await expect(page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true })).toHaveCount(0);
	await expect(page.getByLabel('เริ่ม', { exact: true })).toHaveValue('08:30');
	await expect(page.getByLabel('เริ่ม', { exact: true })).toBeDisabled();
	expect(observed.setupReads).toBe(1);
});

test('hidden planning year uses room context for managers without year-read permission', async ({
	page
}) => {
	await fixture(page, { hiddenPlanning: true });
	await page.goto(`/staff/academic/homerooms?academicYearId=${yearId}`);
	await expect(page.getByTestId('homerooms-ready')).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่มห้องประจำชั้น', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'แก้ไขความจุ ม.1/1', exact: true })).toBeVisible();
});

test('term two may keep the unchanged table without cloning and term one stays locked', async ({
	page
}) => {
	const observed = await fixture(page);
	await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
	await expect(page.getByLabel('ตารางเวลาที่ภาคเรียนนี้ใช้', { exact: true })).toContainText(
		'ตารางเดิมภาคเรียนที่ 1'
	);
	await chooseOption(
		page,
		page.getByLabel('ตารางเวลาที่จะจัดคาบ', { exact: true }),
		'ตารางเดิมภาคเรียนที่ 1 · ตารางหลัก'
	);
	await expect(page.getByLabel('เริ่ม', { exact: true })).toBeDisabled();
	expect(observed.clones).toEqual([]);
	expect(observed.assignments).toEqual([]);
	expect(observed.periodWrites).toEqual([]);
});

test('a target term with timetable drafts links to that exact term and offers no refused assignment', async ({
	page
}) => {
	const observed = await fixture(page, { timetableDraft: true });
	await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
	await expect(page.getByLabel('ตารางเวลาที่ภาคเรียนนี้ใช้', { exact: true })).toHaveCount(0);
	await expect(
		page.getByRole('link', { name: 'จัดการตารางสอนของ ภาคเรียนที่ 2', exact: true })
	).toHaveAttribute(
		'href',
		`/staff/academic/timetable?academicYearId=${yearId}&academicTermId=${nextTermId}`
	);
	await expect(
		page.getByText('ภาคเรียนนี้มีฉบับร่างตารางสอนหรือประวัติอ้างอิงตารางเดิม', { exact: false })
	).toBeVisible();
	expect(observed.assignments).toEqual([]);
});

test('period conflict compares saved periods while retaining the local draft and captured revision', async ({
	page
}) => {
	const observed = await fixture(page, { periodConflict: true });
	await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
	await page.getByRole('button', { name: 'คัดลอก ตารางเดิมภาคเรียนที่ 1', exact: true }).click();
	await page.getByLabel('ชื่อตารางที่คัดลอก', { exact: true }).fill('ตารางภาคเรียนที่ 2');
	await page.getByRole('button', { name: 'ยืนยันคัดลอกตาราง', exact: true }).click();
	await chooseOption(
		page,
		page.getByLabel('ตารางเวลาที่จะจัดคาบ', { exact: true }),
		'ตารางภาคเรียนที่ 2'
	);
	await page.getByLabel('เริ่ม', { exact: true }).fill('09:00');
	await page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true }).click();
	await expect(page.getByLabel('เริ่ม', { exact: true })).toHaveValue('09:00');
	await expect(page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true })).toBeDisabled();
	await page.getByRole('button', { name: 'โหลดคาบล่าสุดเพื่อเปรียบเทียบ', exact: true }).click();
	await expect(page.getByText('คาบเช้า · 08:30–09:20', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ตรวจสอบแล้ว ใช้ฉบับล่าสุดเป็นฐาน', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true }).click();
	await expect.poll(() => observed.periodWrites.length).toBe(2);
	expect(observed.periodWrites.map((request) => request.body.rowVersion)).toEqual([1, 2]);
	expect(
		observed.periodWrites.every((request) => request.body.periods[0].startTime === '09:00')
	).toBe(true);
});

test('capacity edit keeps selected-year context visible on a narrow dark screen', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.emulateMedia({ colorScheme: 'dark' });
	await fixture(page);
	await page.goto(`/staff/academic/homerooms?academicYearId=${yearId}`);
	await expect(page.getByLabel('ปีการศึกษาของห้องประจำชั้น', { exact: true })).toContainText(
		'ปีการศึกษา 2569 · ใช้งาน'
	);
	await page.getByRole('button', { name: 'แก้ไขความจุ ม.1/1', exact: true }).click();
	await expect(page.getByRole('dialog').getByLabel('ความจุ (คน)', { exact: true })).toBeVisible();
	const box = await page.getByRole('dialog').boundingBox();
	expect(box).toBeTruthy();
	expect(box!.x).toBeGreaterThanOrEqual(0);
	expect(box!.x + box!.width).toBeLessThanOrEqual(390);
});

test('time manager can prepare term two with year read and term manage permissions', async ({
	page
}) => {
	await fixture(page, { termManagerOnly: true });
	await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
	await expect(
		page.getByRole('button', { name: 'คัดลอก ตารางเดิมภาคเรียนที่ 1', exact: true })
	).toBeVisible();
	await expect(page.getByLabel('ตารางเวลาที่ภาคเรียนนี้ใช้', { exact: true })).toBeVisible();
});
