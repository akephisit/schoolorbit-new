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
		capacityGates?: Promise<void>[];
		oldCapacityFailure?: boolean;
		periodConflict?: boolean;
		periodConflictPerSchedule?: boolean;
		comparisonGate?: Promise<void>;
		successfulConcurrentWriter?: boolean;
		initialSnapshotAhead?: boolean;
		metadataGate?: Promise<void>;
		authGate?: Promise<void>;
		replacementWorkspaceGate?: Promise<void>;
		termManagerOnly?: boolean;
		readyLegacyName?: boolean;
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
		defaultSchoolDays: ['MON', 'TUE', 'WED', 'THU', 'FRI'],
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
		name: options.readyLegacyName && index === 1 ? 'ภาคเรียนที่ 7' : `ภาคเรียนที่ ${index + 1}`,
		termType: 'regular' as const,
		schoolDays: ['MON', 'TUE', 'WED', 'THU', 'FRI'],
		status:
			index === 0
				? ('active' as const)
				: options.readyLegacyName
					? ('ready' as const)
					: ('planning' as const),
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
	let identitySwitched = false;
	let periodStart = '08:30:00';
	const conflictedSchedules = new Set<string>();
	const periodReadCounts = new Map<string, number>();
	const observed = {
		switchIdentity: () => {
			identitySwitched = true;
		},
		metadataReads: 0,
		authReads: 0,
		scheduleWrites: 0,
		scheduleBases: [] as number[],
		completedComparisons: [] as string[],
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
			if (path === '/api/auth/me') {
				observed.authReads++;
				if (options.authGate) await options.authGate;
				return fulfill(route, {
					id: identitySwitched ? id(598) : id(599),
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
			}
			if (path === '/api/academic/context/options')
				return fulfill(route, {
					defaultAcademicYearId: yearId,
					defaultAcademicTermId: firstTermId,
					years: options.hiddenPlanning ? [] : [year],
					terms
				});
			if (path === '/api/academic/setup/workspace') {
				observed.setupReads++;
				if (identitySwitched && options.replacementWorkspaceGate)
					await options.replacementWorkspaceGate;
				return fulfill(route, {
					years: identitySwitched || options.hiddenPlanning ? [] : [year],
					terms: identitySwitched ? [] : terms,
					bellSchedules: identitySwitched ? [] : schedules,
					bellScheduleSetupYearIds: options.closed ? [] : [yearId],
					bellScheduleSetupTermIds: options.timetableDraft ? [] : [nextTermId],
					schoolDaysEditableTermIds: options.closed ? [] : [nextTermId],
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
				const requestIndex = observed.capacities.length - 1;
				const acceptedRoom = { ...room, capacity: body.capacity, rowVersion: room.rowVersion + 1 };
				if (options.capacityGates?.[requestIndex]) {
					await options.capacityGates[requestIndex];
					if (requestIndex === 0 && options.oldCapacityFailure)
						return fulfill(route, 'ข้อผิดพลาดจากตัวตนเก่า', 503);
					return fulfill(route, acceptedRoom);
				}
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
					id: id(510 + observed.clones.length - 1),
					name: body.name,
					isDefault: false,
					status: 'draft',
					editBlockedReason: null,
					rowVersion: 1
				};
				schedules.push(clone);
				const accepted = { ...clone };
				if (options.initialSnapshotAhead) {
					clone.rowVersion = 3;
					clone.name = 'ชื่อที่อีกคนแก้';
				}
				return fulfill(route, accepted, 201);
			}
			if (path === '/api/academic/bell-schedules') {
				observed.metadataReads++;
				if (observed.scheduleWrites && options.metadataGate) await options.metadataGate;
				return fulfill(route, schedules);
			}
			if (path === `/api/academic/bell-schedules/${id(510)}` && method === 'PATCH') {
				observed.scheduleWrites++;
				const body = route.request().postDataJSON();
				const schedule = schedules.find((item) => item.id === id(510))!;
				observed.scheduleBases.push(body.rowVersion);
				if (body.rowVersion !== schedule.rowVersion)
					return fulfill(route, 'ชื่อตารางเปลี่ยนระหว่างแก้ไข', 409);
				Object.assign(schedule, body, { rowVersion: schedule.rowVersion + 1 });
				return fulfill(route, schedule);
			}
			if (path.includes('/bell-schedules/') && path.endsWith('/periods')) {
				const selected = path.split('/').at(-2)!;
				const schedule = schedules.find((item) => item.id === selected)!;
				if (method === 'PUT') {
					const body = route.request().postDataJSON();
					observed.periodWrites.push({ id: selected, body });
					if (body.rowVersion !== schedule.rowVersion)
						return fulfill(route, 'คาบเรียนเปลี่ยนระหว่างแก้ไข', 409);
					schedule.rowVersion++;
					if (
						(options.periodConflict && observed.periodWrites.length === 1) ||
						(options.periodConflictPerSchedule && !conflictedSchedules.has(selected))
					) {
						conflictedSchedules.add(selected);
						return fulfill(route, 'คาบเรียนเปลี่ยนระหว่างแก้ไข', 409);
					}
					const accepted = {
						bellScheduleId: selected,
						rowVersion: schedule.rowVersion,
						periods: body.periods.map((period: S['BellSchedulePeriodInput']) => ({
							...period,
							id: id(511),
							bellScheduleId: selected,
							applicableDays: period.applicableDays?.join(',') ?? null
						}))
					};
					if (options.successfulConcurrentWriter && observed.periodWrites.length === 1) {
						schedule.rowVersion++;
						periodStart = '10:00:00';
					}
					return fulfill(route, accepted);
				}
				const readCount = (periodReadCounts.get(selected) ?? 0) + 1;
				periodReadCounts.set(selected, readCount);
				if (selected === id(510) && readCount === 2 && options.comparisonGate)
					await options.comparisonGate;
				const response = await fulfill(route, {
					bellScheduleId: selected,
					rowVersion: schedule.rowVersion,
					periods: periods.map((period) => ({
						...period,
						name: options.periodConflictPerSchedule
							? selected === id(510)
								? 'คาบ A ล่าสุด'
								: 'คาบ B ล่าสุด'
							: period.name,
						startTime: selected === scheduleId ? period.startTime : periodStart,
						endTime: periodStart === '10:00:00' ? '10:50:00' : period.endTime,
						bellScheduleId: selected,
						id: selected === scheduleId ? period.id : id(511)
					}))
				});
				if (readCount === 2) observed.completedComparisons.push(selected);
				return response;
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
			if (path === '/api/notifications') return fulfill(route, { items: [], unread_count: 0 });
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
	await page.getByRole('button', { name: 'จัดการตารางเวลาต้นแบบเพิ่มเติม', exact: true }).click();
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
		page.getByText('ภาคเรียนนี้มีตารางสอนหรือประวัติอ้างอิงตารางเดิม', { exact: false })
	).toBeVisible();
	expect(observed.assignments).toEqual([]);
});

test('period conflict compares saved periods while retaining the local draft and captured revision', async ({
	page
}) => {
	const observed = await fixture(page, { periodConflict: true });
	await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
	await page.getByRole('button', { name: 'จัดการตารางเวลาต้นแบบเพิ่มเติม', exact: true }).click();
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

test('capacity diagnostic destination focuses only a matching room in the selected year', async ({
	page
}) => {
	const observed = await fixture(page);
	await page.goto(`/staff/academic/homerooms?academicYearId=${yearId}&homeroomId=${roomId}`);
	const target = page.locator('[data-homeroom-target="true"]');
	await expect(target).toHaveCount(1);
	await expect(target).toContainText('ม.1/1');
	await expect(target).toHaveAttribute('data-state', 'selected');
	await expect(target).toBeFocused();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.goto(`/staff/academic/homerooms?academicYearId=${yearId}&homeroomId=${id(999)}`);
	await expect(page.getByTestId('homerooms-ready')).toBeVisible();
	await expect(page.locator('[data-homeroom-target="true"]')).toHaveCount(0);
	await expect(page.getByRole('dialog')).toHaveCount(0);
	expect(observed.capacities).toEqual([]);
});

test('ready-term table assignment preserves exact existing non-bell metadata', async ({ page }) => {
	const observed = await fixture(page, { readyLegacyName: true });
	await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
	await page.getByRole('button', { name: 'จัดการตารางเวลาต้นแบบเพิ่มเติม', exact: true }).click();
	await page.getByRole('button', { name: 'คัดลอก ตารางเดิมภาคเรียนที่ 1', exact: true }).click();
	await page.getByLabel('ชื่อตารางที่คัดลอก', { exact: true }).fill('ตารางภาคเรียนพร้อมใช้');
	await page.getByRole('button', { name: 'ยืนยันคัดลอกตาราง', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกตารางเวลาของภาคเรียน', exact: true }).click();
	await expect.poll(() => observed.assignments.length).toBe(1);
	expect(observed.assignments[0]).toEqual({
		bellScheduleId: id(510),
		rowVersion: 1,
		customName: 'ภาคเรียนที่ 7',
		termType: 'regular',
		schoolDays: ['MON', 'TUE', 'WED', 'THU', 'FRI'],
		includedInYearResult: true,
		blocksYearClosure: true
	});
});

test('successful period save retains its paired revision when a concurrent writer advances metadata', async ({
	page
}) => {
	const observed = await fixture(page, { successfulConcurrentWriter: true });
	await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
	await page.getByRole('button', { name: 'จัดการตารางเวลาต้นแบบเพิ่มเติม', exact: true }).click();
	await page.getByRole('button', { name: 'คัดลอก ตารางเดิมภาคเรียนที่ 1', exact: true }).click();
	await page.getByLabel('ชื่อตารางที่คัดลอก', { exact: true }).fill('ตารางทดสอบสองผู้ใช้');
	await page.getByRole('button', { name: 'ยืนยันคัดลอกตาราง', exact: true }).click();
	await chooseOption(
		page,
		page.getByLabel('ตารางเวลาที่จะจัดคาบ', { exact: true }),
		'ตารางทดสอบสองผู้ใช้'
	);
	await page.getByLabel('เริ่ม', { exact: true }).fill('09:00');
	await page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true }).click();
	await expect(page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true })).toBeEnabled();
	const concurrent = await page.evaluate(
		async (schedule) =>
			(await (await fetch(`/api/academic/bell-schedules/${schedule}/periods`)).json()).data,
		id(510)
	);
	expect(concurrent.rowVersion).toBe(3);
	expect(concurrent.periods[0].startTime).toBe('10:00:00');
	expect(observed.metadataReads).toBe(0);
	await expect(page.getByLabel('เริ่ม', { exact: true })).toHaveValue('09:00');
	await page.getByLabel('เริ่ม', { exact: true }).fill('09:05');
	await page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true }).click();
	await expect(page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true })).toBeDisabled();
	await expect(page.getByLabel('เริ่ม', { exact: true })).toHaveValue('09:05');
	expect(observed.periodWrites.map((request) => request.body.rowVersion)).toEqual([1, 2]);
});

test('term manager without timetable route permission gets contact guidance instead of a denied link', async ({
	page
}) => {
	await fixture(page, { timetableDraft: true, termManagerOnly: true });
	await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
	await expect(
		page.getByText('ติดต่อผู้ดูแลตารางสอนเพื่อตรวจสอบรายการอ้างอิงของภาคเรียนนี้', { exact: true })
	).toBeVisible();
	await expect(
		page.getByRole('link', { name: 'จัดการตารางสอนของ ภาคเรียนที่ 2', exact: true })
	).toHaveCount(0);
});

test('delayed schedule success refresh cannot restore the previous identity workspace', async ({
	page
}) => {
	let release = () => {};
	const metadataGate = new Promise<void>((resolve) => {
		release = resolve;
	});
	let releaseWorkspace = () => {};
	const replacementWorkspaceGate = new Promise<void>((resolve) => {
		releaseWorkspace = resolve;
	});
	const observed = await fixture(page, { metadataGate, replacementWorkspaceGate });
	try {
		await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
		await page.getByRole('button', { name: 'จัดการตารางเวลาต้นแบบเพิ่มเติม', exact: true }).click();
	await page.getByRole('button', { name: 'คัดลอก ตารางเดิมภาคเรียนที่ 1', exact: true }).click();
		await page.getByLabel('ชื่อตารางที่คัดลอก', { exact: true }).fill('ข้อมูลตัวตนเก่า');
		await page.getByRole('button', { name: 'ยืนยันคัดลอกตาราง', exact: true }).click();
		await page.getByRole('button', { name: 'แก้ไข ข้อมูลตัวตนเก่า', exact: true }).click();
		await page.getByLabel('แก้ไขชื่อตารางเวลา', { exact: true }).fill('ข้อมูลเก่าที่ตอบกลับช้า');
		await page.getByRole('button', { name: 'บันทึกชื่อ', exact: true }).click();
		await expect.poll(() => observed.metadataReads).toBe(1);
		observed.switchIdentity();
		await page.evaluate(async () => {
			const modulePath = '/src/lib/stores/auth.ts';
			const { authStore } = await import(modulePath);
			authStore.setUser(
				{
					id: '10000000-0000-4000-8000-000000000598',
					firstName: 'ตัวตนใหม่',
					lastName: 'ทดสอบ',
					role: 'staff',
					user_type: 'staff',
					status: 'active'
				},
				['*']
			);
		});
		await expect(page.getByTestId('academic-setup-ready')).toHaveCount(0);
		const completed = page.waitForResponse(
			(response) => new URL(response.url()).pathname === '/api/academic/bell-schedules'
		);
		release();
		await completed;
		await expect(page.getByTestId('academic-setup-ready')).toHaveCount(0);
		await expect(page.getByText('ข้อมูลเก่าที่ตอบกลับช้า', { exact: true })).toHaveCount(0);
		await expect.poll(() => observed.setupReads).toBeGreaterThanOrEqual(2);
		releaseWorkspace();
		await expect(page.getByTestId('academic-setup-ready')).toBeVisible();
		await expect(page.getByText('ข้อมูลเก่าที่ตอบกลับช้า', { exact: true })).toHaveCount(0);
	} finally {
		release();
		releaseWorkspace();
	}
});

test('initial setup boot accepts its first authenticated route promise after delayed auth', async ({
	page
}) => {
	let releaseAuth = () => {};
	const authGate = new Promise<void>((resolve) => {
		releaseAuth = resolve;
	});
	const observed = await fixture(page, { authGate });
	try {
		const navigation = page.goto(
			`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`
		);
		await expect.poll(() => observed.authReads).toBe(1);
		await expect(page.getByTestId('academic-setup-ready')).toHaveCount(0);
		releaseAuth();
		await navigation;
		await expect(page.getByTestId('academic-setup-ready')).toBeVisible();
		await expect(
			page.getByRole('button', { name: 'คัดลอก ตารางเดิมภาคเรียนที่ 1', exact: true })
		).toBeVisible();
		expect(observed.setupReads).toBe(1);
	} finally {
		releaseAuth();
	}
});

test('newer period GET does not rebase older name metadata and period writes use the paired revision', async ({
	page
}) => {
	const observed = await fixture(page, { initialSnapshotAhead: true });
	await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
	await page.getByRole('button', { name: 'จัดการตารางเวลาต้นแบบเพิ่มเติม', exact: true }).click();
	await page.getByRole('button', { name: 'คัดลอก ตารางเดิมภาคเรียนที่ 1', exact: true }).click();
	await page.getByLabel('ชื่อตารางที่คัดลอก', { exact: true }).fill('ชื่อตารางในหน้าฉบับแรก');
	await page.getByRole('button', { name: 'ยืนยันคัดลอกตาราง', exact: true }).click();
	await chooseOption(
		page,
		page.getByLabel('ตารางเวลาที่จะจัดคาบ', { exact: true }),
		'ชื่อตารางในหน้าฉบับแรก'
	);
	await expect(page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true })).toBeEnabled();
	await page.getByLabel('เริ่ม', { exact: true }).fill('09:00');
	await page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true }).click();
	await expect.poll(() => observed.periodWrites.length).toBe(1);
	expect(observed.periodWrites[0].body.rowVersion).toBe(3);
	await page.getByRole('button', { name: 'แก้ไข ชื่อตารางในหน้าฉบับแรก', exact: true }).click();
	await page.getByLabel('แก้ไขชื่อตารางเวลา', { exact: true }).fill('ชื่อฉบับร่างของเรา');
	await page.getByRole('button', { name: 'บันทึกชื่อ', exact: true }).click();
	await expect(page.getByText('ชื่อตารางเปลี่ยนระหว่างแก้ไข', { exact: false })).toBeVisible();
	expect(observed.scheduleBases).toEqual([1]);
	await expect(page.getByLabel('แก้ไขชื่อตารางเวลา', { exact: true })).toHaveValue(
		'ชื่อฉบับร่างของเรา'
	);
});

for (const oldOutcome of ['success', 'failure'] as const) {
	test(`old capacity ${oldOutcome} cannot close, error, or clear a new identity's pending dialog for the same room`, async ({
		page
	}) => {
		let releaseOld = () => {},
			releaseNew = () => {};
		const oldGate = new Promise<void>((resolve) => {
			releaseOld = resolve;
		});
		const newGate = new Promise<void>((resolve) => {
			releaseNew = resolve;
		});
		const observed = await fixture(page, {
			capacityGates: [oldGate, newGate],
			oldCapacityFailure: oldOutcome === 'failure'
		});
		try {
			await page.goto(`/staff/academic/homerooms?academicYearId=${yearId}`);
			await page.getByRole('button', { name: 'แก้ไขความจุ ม.1/1', exact: true }).click();
			await page.getByRole('dialog').getByLabel('ความจุ (คน)', { exact: true }).fill('45');
			await page
				.getByRole('dialog')
				.getByRole('button', { name: 'บันทึกความจุ', exact: true })
				.click();
			await expect.poll(() => observed.capacities.length).toBe(1);
			observed.switchIdentity();
			await page.evaluate(async (session) => {
				const modulePath = '/src/lib/stores/auth.ts';
				const { authStore } = await import(modulePath);
				authStore.setUser(
					{
						id: '10000000-0000-4000-8000-000000000598',
						firstName: 'ตัวตนใหม่',
						lastName: 'ทดสอบ',
						role: 'staff',
						user_type: 'staff',
						status: 'active'
					},
					['*'],
					session
				);
			}, sessionState());
			await expect(page.getByRole('dialog')).toHaveCount(0);
			await page.getByRole('button', { name: 'แก้ไขความจุ ม.1/1', exact: true }).click();
			const dialog = page.getByRole('dialog');
			await dialog.getByLabel('ความจุ (คน)', { exact: true }).fill('50');
			await dialog.getByRole('button', { name: 'บันทึกความจุ', exact: true }).click();
			await expect.poll(() => observed.capacities.length).toBe(2);
			const oldResponse = page.waitForResponse(
				(response) =>
					new URL(response.url()).pathname.endsWith('/capacity') &&
					response.request().postDataJSON().capacity === 45
			);
			releaseOld();
			await oldResponse;
			await expect(dialog).toBeVisible();
			await expect(dialog.getByLabel('ความจุ (คน)', { exact: true })).toHaveValue('50');
			await expect(dialog.getByLabel('ความจุ (คน)', { exact: true })).toBeDisabled();
			await expect(
				dialog.getByRole('button', { name: 'บันทึกความจุ', exact: true })
			).toBeDisabled();
			await expect(dialog.getByRole('alert')).toHaveCount(0);
			releaseNew();
			await expect(dialog).toHaveCount(0);
			await expect(page.getByRole('cell', { name: '35 / 50', exact: true })).toBeVisible();
		} finally {
			releaseOld();
			releaseNew();
		}
	});
}

test('switching schedules supersedes a pending comparison and leaves the next comparison usable', async ({
	page
}) => {
	let releaseComparison = () => {};
	const comparisonGate = new Promise<void>((resolve) => {
		releaseComparison = resolve;
	});
	const observed = await fixture(page, { comparisonGate, periodConflictPerSchedule: true });
	try {
		await page.goto(`/staff/academic/core?academicYearId=${yearId}&academicTermId=${nextTermId}`);
		await page.getByRole('button', { name: 'จัดการตารางเวลาต้นแบบเพิ่มเติม', exact: true }).click();
		for (const name of ['ตาราง A', 'ตาราง B']) {
			await page
				.getByRole('button', { name: 'คัดลอก ตารางเดิมภาคเรียนที่ 1', exact: true })
				.click();
			await page.getByLabel('ชื่อตารางที่คัดลอก', { exact: true }).fill(name);
			await page.getByRole('button', { name: 'ยืนยันคัดลอกตาราง', exact: true }).click();
		}
		await chooseOption(page, page.getByLabel('ตารางเวลาที่จะจัดคาบ', { exact: true }), 'ตาราง A');
		await page.getByLabel('เริ่ม', { exact: true }).fill('09:00');
		await page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true }).click();
		const compare = page.getByRole('button', {
			name: 'โหลดคาบล่าสุดเพื่อเปรียบเทียบ',
			exact: true
		});
		await compare.click();
		await expect(compare).toBeDisabled();
		await chooseOption(page, page.getByLabel('ตารางเวลาที่จะจัดคาบ', { exact: true }), 'ตาราง B');
		await page.getByLabel('เริ่ม', { exact: true }).fill('09:30');
		await page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true }).click();
		await expect(compare).toBeEnabled();
		await compare.click();
		await expect(page.getByText('คาบ B ล่าสุด · 08:30–09:20', { exact: true })).toBeVisible();
		releaseComparison();
		await expect.poll(() => observed.completedComparisons.includes(id(510))).toBe(true);
		await expect(page.getByText('คาบ A ล่าสุด · 08:30–09:20', { exact: true })).toHaveCount(0);
		await expect(page.getByText('คาบ B ล่าสุด · 08:30–09:20', { exact: true })).toBeVisible();
		await expect(page.getByLabel('เริ่ม', { exact: true })).toHaveValue('09:30');
		await page
			.getByRole('button', { name: 'ตรวจสอบแล้ว ใช้ฉบับล่าสุดเป็นฐาน', exact: true })
			.click();
		await page.getByRole('button', { name: 'บันทึกคาบทั้งหมด', exact: true }).click();
		await expect.poll(() => observed.periodWrites.length).toBe(3);
		expect(observed.periodWrites.at(-1)).toMatchObject({ id: id(511), body: { rowVersion: 2 } });
	} finally {
		releaseComparison();
	}
});
