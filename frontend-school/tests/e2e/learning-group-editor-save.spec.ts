import { sessionState } from './fixtures/session-state';
import { expect, test, type Page, type Route } from '@playwright/test';
import type {
	DeliveryManagementOptions,
	LearningGroup,
	LearningOffering,
	ReplaceLearningGroupHomeroomsRequest,
	ReplaceLearningGroupTeachersRequest,
	TeacherAssignment,
	UpdateLearningGroupRequest
} from '../../src/lib/api/learning-delivery.js';

test.use({ serviceWorkers: 'block' });

const ids = {
	year: '10000000-0000-4000-8000-000000000301',
	term: '20000000-0000-4000-8000-000000000301',
	offering: '30000000-0000-4000-8000-000000000301',
	group: '40000000-0000-4000-8000-000000000301',
	version: '50000000-0000-4000-8000-000000000301',
	teacherA: '60000000-0000-4000-8000-000000000301',
	teacherB: '60000000-0000-4000-8000-000000000302',
	assignmentA: '70000000-0000-4000-8000-000000000301',
	assignmentB: '70000000-0000-4000-8000-000000000302',
	homeroomA: '80000000-0000-4000-8000-000000000301',
	homeroomB: '80000000-0000-4000-8000-000000000302'
};
const timestamp = '2026-05-01T00:00:00Z';
const groupPath = `/api/academic/learning-groups/${ids.group}`;
const teachersPath = `${groupPath}/teachers`;
const openingTeacher: TeacherAssignment = { teacherId: ids.teacherA, role: 'primary' };

function readAssignments(teachers: TeacherAssignment[], revision: number) {
	return teachers.map(({ teacherId, role }) => ({
		id: teacherId === ids.teacherA ? ids.assignmentA : ids.assignmentB,
		teacherId,
		role,
		displayName: teacherId === ids.teacherA ? 'ครูสมใจ ทดสอบ' : 'ครูสมชาย ทดสอบ',
		isActive: true,
		rowVersion: revision + 10
	}));
}

function deferred() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { promise, release };
}

async function fulfill(route: Route, data: unknown) {
	await route.fulfill({
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-test-csrf' },
		body: JSON.stringify({ success: true, data })
	});
}

async function reject(route: Route, status: number, error: string) {
	await route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify({ success: false, error })
	});
}

async function mockEditor(
	page: Page,
	options: {
		permissions?: string[];
		activity?: 'self' | 'assigned';
		closed?: boolean;
		failDetails?: boolean;
		firstTeacherFailure?: 'network' | 'conflict';
		teacherSaveGate?: Promise<void>;
	} = {}
) {
	let stored: LearningGroup = {
		id: ids.group,
		learningOfferingId: ids.offering,
		academicTermId: ids.term,
		academicYearId: ids.year,
		code: 'A',
		name: 'กลุ่ม ก',
		description: 'กลุ่มทดสอบ',
		capacity: 40,
		registrationOpen: false,
		teacherAssignments: readAssignments([openingTeacher], 3),
		homeroomIds: [ids.homeroomA],
		preferredRoomIds: [],
		migrated: false,
		createdAt: timestamp,
		updatedAt: timestamp,
		rowVersion: 3
	};
	const offering: LearningOffering = {
		canManage: true,
		id: ids.offering,
		academicYearId: ids.year,
		academicTermId: ids.term,
		kind: 'course',
		codeSnapshot: 'ค21101',
		nameSnapshot: 'คณิตศาสตร์พื้นฐาน',
		owningOrganizationUnitId: ids.offering,
		rowVersion: 1,
		weeklyPeriodTarget: 2,
		targets: [],
		migrated: false,
		createdAt: timestamp,
		updatedAt: timestamp,
		snapshot: {
			kind: 'course',
			subjectId: ids.offering,
			subjectVersionId: ids.offering,
			credit: '1.0',
			hours: '40',
			standardPeriodsPerWeek: 2,
			curriculumCourseRequirementId: null,
			assessmentTotalScore: '100.00'
		}
	};
	if (options.activity) {
		offering.kind = 'activity';
		offering.snapshot = {kind:'activity',activityId:ids.offering,activityVersionId:ids.offering,registrationType:options.activity,schedulingMode:'independent',hours:'20',attendanceRequirement:{},passCriteria:{requireAttendance:true,requireTeacherConfirmation:true,outcomes:['pass','fail']}};
	}
	const management: DeliveryManagementOptions = {
		academicTermId: ids.term,
		academicYearId: ids.year,
		catalogVersions: [],
		gradeLevels: [],
		learningGroups: [stored],
		studyPrograms: [],
		rooms: [],
		teachers: [
			{ id: ids.teacherA, name: 'ครูสมใจ ทดสอบ' },
			{ id: ids.teacherB, name: 'ครูสมชาย ทดสอบ' }
		],
		homerooms: [
			{ id: ids.homeroomA, name: 'ม.1/1' },
			{ id: ids.homeroomB, name: 'ม.1/2' }
		]
	};
	const teacherRequests: ReplaceLearningGroupTeachersRequest[] = [];
	const detailRequests: UpdateLearningGroupRequest[] = [];
	const homeroomRequests: ReplaceLearningGroupHomeroomsRequest[] = [];
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const { pathname: path } = new URL(route.request().url());
			const method = route.request().method();
			if (path === '/api/auth/me')
				return fulfill(route, {
					id: '90000000-0000-4000-8000-000000000301',
					username: 'group-editor',
					firstName: 'ทดสอบ',
					lastName: 'วิชาการ',
					displayName: 'ทดสอบ วิชาการ',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
					permissions: options.permissions ?? ['*']
				});
			if (path === '/api/menu/user') return fulfill(route, { groups: [] });
			if (path === '/api/academic/context/options')
				return fulfill(route, {
					defaultAcademicYearId: ids.year,
					defaultAcademicTermId: ids.term,
					years: [
						{
							isVisible: true,
							id: ids.year,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active'
						}
					],
					terms: [
						{
							isVisible: true,
							id: ids.term,
							academicYearId: ids.year,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: options.closed ? 'closed' : 'active'
						}
					]
				});
			if (path === teachersPath && method === 'PUT') {
				const body: ReplaceLearningGroupTeachersRequest = route.request().postDataJSON();
				teacherRequests.push(body);
				// Model the Rust deny_unknown_fields boundary, including nested assignments.
				if (
					Object.keys(body).sort().join(',') !== 'rowVersion,teachers' ||
					body.teachers.some(
						(teacher) => Object.keys(teacher).sort().join(',') !== 'role,teacherId'
					)
				)
					return reject(route, 422, 'ข้อมูลครูผู้สอนมีฟิลด์ที่ไม่รองรับ');
				if (options.teacherSaveGate) await options.teacherSaveGate;
				if (teacherRequests.length === 1 && options.firstTeacherFailure === 'network')
					return route.abort('failed');
				if (teacherRequests.length === 1 && options.firstTeacherFailure === 'conflict') {
					stored = { ...stored, rowVersion: stored.rowVersion + 1 };
					return reject(route, 409, 'กลุ่มนี้เปลี่ยนแล้ว กรุณาโหลดใหม่');
				}
				if (body.rowVersion !== stored.rowVersion)
					return reject(route, 409, 'รุ่นข้อมูลกลุ่มไม่ตรงกัน');
				stored = {
					...stored,
					rowVersion: stored.rowVersion + 1,
					teacherAssignments: readAssignments(body.teachers, stored.rowVersion + 1)
				};
				return fulfill(route, stored);
			}
			if (path === groupPath && method === 'PATCH') {
				const body: UpdateLearningGroupRequest = route.request().postDataJSON();
				detailRequests.push(body);
				if (options.failDetails && detailRequests.length === 1) { stored={...stored,rowVersion:stored.rowVersion+1}; return reject(route,409,'กลุ่มนี้เปลี่ยนแล้ว กรุณาโหลดใหม่'); }
				if (body.rowVersion !== stored.rowVersion)
					return reject(route, 409, 'รุ่นข้อมูลกลุ่มไม่ตรงกัน');
				stored = { ...stored, ...body, rowVersion: stored.rowVersion + 1 };
				return fulfill(route, stored);
			}
			if (path === `${groupPath}/homerooms` && method === 'PUT') {
				const body: ReplaceLearningGroupHomeroomsRequest = route.request().postDataJSON();
				homeroomRequests.push(body);
				if (body.rowVersion !== stored.rowVersion)
					return reject(route, 409, 'รุ่นข้อมูลกลุ่มไม่ตรงกัน');
				stored = { ...stored, homeroomIds: body.homeroomIds, rowVersion: stored.rowVersion + 1 };
				return fulfill(route, stored);
			}
			if (path === `/api/academic/offerings/${ids.offering}`) return fulfill(route, offering);
			if (path === `/api/academic/offerings/${ids.offering}/groups`)
				return fulfill(route, [stored]);
			if (path === groupPath) return fulfill(route, stored);
			if (path === `${groupPath}/memberships`) return fulfill(route, []);
			if (path === `${groupPath}/roster-tracking`)
				return fulfill(route, {
					mode: 'manual',
					effectiveFrom: null,
					groupRowVersion: stored.rowVersion,

				});
			if (path === '/api/academic/delivery/management-options') return fulfill(route, management);
			if (path.startsWith('/api/academic/')) return reject(route, 404, `Unexpected API: ${path}`);
			return fulfill(route, {});
		}
	);
	return { teacherRequests, detailRequests, homeroomRequests, stored: () => stored };
}

async function openEditor(page: Page) {
	await page.goto(
		`/staff/academic/delivery/${ids.offering}?academicYearId=${ids.year}&academicTermId=${ids.term}&groupId=${ids.group}`
	);
	await expect(page.getByRole('heading', {name:'กลุ่ม ก',exact:true})).toBeVisible();
	await page.getByRole('button', { name: 'แก้ไขกลุ่มและครู', exact: true }).click();
	await expect(page.getByRole('heading', { name: /จัดการกลุ่ม ·/ })).toBeVisible();
}

function editor(page: Page) {
	return page
		.locator('section')
		.filter({ has: page.getByRole('heading', { name: /จัดการกลุ่ม ·/ }) });
}

async function addTeacher(page: Page, name: string, role: string) {
	await page.getByRole('combobox', { name: 'เลือกครูผู้สอน' }).click();
	await page.getByRole('option', { name, exact: true }).click();
	await page.getByRole('combobox', { name: 'บทบาทครูผู้สอน' }).click();
	await page.getByRole('option', { name: role, exact: true }).click();
	await editor(page).getByRole('button', { name: 'เพิ่ม', exact: true }).click();
}

async function saveTeachers(page: Page, expectedStatus = 200) {
	const response = page.waitForResponse(
		(response) =>
			new URL(response.url()).pathname === teachersPath && response.request().method() === 'PUT'
	);
	await page.getByRole('button', { name: 'บันทึกครูผู้สอน', exact: true }).click();
	expect((await response).status()).toBe(expectedStatus);
	await expect(page.getByRole('button', { name: 'บันทึกครูผู้สอน', exact: true })).toBeEnabled();
}

for (const width of [390, 1280]) {
	for (const dark of [false, true]) {
		test(`projects complete read assignments on resave, response remount, edits and reload at ${width}px ${dark ? 'dark' : 'light'}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			const fixture = await mockEditor(page);
			await openEditor(page);
			if (dark) await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).click();
			if (dark) await expect(page.locator('html')).toHaveClass(/\bdark\b/);
			else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/);
			await saveTeachers(page);
			expect(fixture.teacherRequests).toEqual([{ teachers: [openingTeacher], rowVersion: 3 }]);
			await saveTeachers(page);
			expect(fixture.teacherRequests[1]).toEqual({ teachers: [openingTeacher], rowVersion: 4 });

			await addTeacher(page, 'ครูสมใจ ทดสอบ', 'ผู้สอนร่วม');
			await addTeacher(page, 'ครูสมชาย ทดสอบ', 'ผู้ช่วยสอน');
			await saveTeachers(page);
			expect(fixture.teacherRequests[2]).toEqual({
				teachers: [
					{ teacherId: ids.teacherA, role: 'secondary' },
					{ teacherId: ids.teacherB, role: 'assistant' }
				],
				rowVersion: 5
			});
			const teacherSection = editor(page)
				.locator(':scope > div > div')
				.filter({ has: page.getByRole('heading', { name: 'ครูผู้สอน', exact: true }) });
			await expect(teacherSection).toContainText('ผู้สอนร่วม');
			await expect(teacherSection).toContainText('ผู้ช่วยสอน');
			await teacherSection.screenshot({
				path: testInfo.outputPath(`saved-teachers-${width}-${dark ? 'dark' : 'light'}.png`)
			});
			await editor(page).getByRole('button', { name: 'นำ ครูสมใจ ทดสอบ ออก', exact: true }).click();
			await saveTeachers(page);
			expect(fixture.teacherRequests[3]).toEqual({
				teachers: [{ teacherId: ids.teacherB, role: 'assistant' }],
				rowVersion: 6
			});
			await page.reload();
			await page.getByRole('button', { name: 'แก้ไขกลุ่มและครู', exact: true }).click();
			await expect(editor(page)).toContainText('ครูสมชาย ทดสอบ');
			await expect(editor(page)).not.toContainText('ครูสมใจ ทดสอบ');
			await saveTeachers(page);
			expect(fixture.teacherRequests[4]).toEqual({
				teachers: [{ teacherId: ids.teacherB, role: 'assistant' }],
				rowVersion: 7
			});
		});
	}
}

test('details and homerooms save independently and preserve stored teachers', async ({ page }) => {
	const fixture = await mockEditor(page);
	const originalTeachers = structuredClone(fixture.stored().teacherAssignments);
	await openEditor(page);
	await page.getByLabel('ชื่อกลุ่ม', { exact: true }).fill('กลุ่ม ก ปรับปรุง');
	await page.getByLabel('คำอธิบาย (ถ้ามี)', { exact: true }).fill('รายละเอียดใหม่');
	await page.getByLabel('ความจุ (ถ้ามี)', { exact: true }).fill('35');
	await page.getByRole('button', { name: 'บันทึกข้อมูลกลุ่ม', exact: true }).click();
	await expect.poll(() => fixture.stored().rowVersion).toBe(4);
	await expect(page.getByRole('button', { name: 'บันทึกข้อมูลกลุ่ม', exact: true })).toBeEnabled();
	expect(fixture.detailRequests).toEqual([
		{
			name: 'กลุ่ม ก ปรับปรุง',
			description: 'รายละเอียดใหม่',
			capacity: 35,
			preferredRoomIds: [],
			registrationOpen: false,
			rowVersion: 3
		}
	]);
	expect(fixture.stored().teacherAssignments).toEqual(originalTeachers);

	await page.getByRole('combobox', { name: 'เลือกห้องต้นทาง' }).click();
	await page.getByRole('option', { name: 'ม.1/2', exact: true }).click();
	await page.getByRole('button', { name: 'เพิ่มห้องต้นทาง', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกห้องต้นทาง', exact: true }).click();
	await expect.poll(() => fixture.stored().rowVersion).toBe(5);
	await expect(page.getByRole('button', { name: 'บันทึกห้องต้นทาง', exact: true })).toBeEnabled();
	expect(fixture.homeroomRequests).toEqual([
		{
			homeroomIds: [ids.homeroomA, ids.homeroomB],
			rowVersion: 4
		}
	]);
	expect(fixture.stored().teacherAssignments).toEqual(originalTeachers);
	expect(fixture.stored().name).toBe('กลุ่ม ก ปรับปรุง');
	expect(fixture.teacherRequests).toEqual([]);
	await saveTeachers(page);
	expect(fixture.teacherRequests).toEqual([{ teachers: [openingTeacher], rowVersion: 5 }]);
});

for (const failure of ['network', 'conflict'] as const) {
	test(`${failure} retains the teacher draft and prevents duplicate pending saves`, async ({
		page
	}) => {
		const gate = deferred();
		const fixture = await mockEditor(page, {
			firstTeacherFailure: failure,
			teacherSaveGate: gate.promise
		});
		await openEditor(page);
		await addTeacher(page, 'ครูสมชาย ทดสอบ', 'ผู้ช่วยสอน');
		try {
			await page.getByRole('button', { name: 'บันทึกครูผู้สอน', exact: true }).click();
			await expect.poll(() => fixture.teacherRequests.length).toBe(1);
			const pending = editor(page).getByRole('button', { name: 'กำลังบันทึก', exact: true });
			await expect(pending).toBeDisabled();
			await pending.evaluate((button: HTMLButtonElement) => button.click());
			expect(fixture.teacherRequests).toHaveLength(1);
		} finally {
			gate.release();
		}
		await expect(editor(page).getByRole('alert')).toHaveCount(2);
		await expect(editor(page).getByRole('alert').filter({hasText:'ข้อมูลที่คุณแก้ไขยังอยู่ โหลดข้อมูลกลุ่มล่าสุดก่อนลองบันทึกอีกครั้ง'})).toBeVisible();
		if (failure === 'conflict')
			await expect(editor(page).getByRole('alert').filter({hasText:'กลุ่มนี้เปลี่ยนแล้ว กรุณาโหลดใหม่'})).toHaveText(
				'กลุ่มนี้เปลี่ยนแล้ว กรุณาโหลดใหม่ กรุณาโหลดข้อมูลล่าสุดหรือสร้างตัวอย่างใหม่ก่อนดำเนินการ'
			);
		await expect(editor(page)).toContainText('ครูสมชาย ทดสอบ');
		expect(fixture.stored().teacherAssignments).toEqual(readAssignments([openingTeacher], 3));
		expect(fixture.stored().rowVersion).toBe(failure === 'conflict' ? 4 : 3);
		expect(fixture.teacherRequests).toHaveLength(1);
		await page.getByRole('button',{name:'ใช้ข้อมูลกลุ่มล่าสุด',exact:true}).click();
		await expect(editor(page).getByRole('alert')).toHaveCount(0);
		await addTeacher(page,'ครูสมชาย ทดสอบ','ผู้ช่วยสอน');
		await saveTeachers(page);
		const revision=failure==='conflict'?4:3;
		expect(fixture.teacherRequests).toEqual([
			{teachers:[openingTeacher,{teacherId:ids.teacherB,role:'assistant'}],rowVersion:3},
			{teachers:[openingTeacher,{teacherId:ids.teacherB,role:'assistant'}],rowVersion:revision}
		]);
		expect(fixture.stored().rowVersion).toBe(revision+1);

	});
}

for (const mode of ['reader','closed'] as const) {
 test(`${mode} cannot change current teachers`,async({page})=>{
  const fixture=await mockEditor(page,{permissions:mode==='reader'?['learning_offering.read.school']:['*'],closed:mode==='closed'});
  await page.goto(`/staff/academic/delivery/${ids.offering}?academicYearId=${ids.year}&academicTermId=${ids.term}&groupId=${ids.group}`);
  await expect(page.getByRole('heading', {name:'กลุ่ม ก',exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'จัดการกลุ่ม',exact:true})).toHaveCount(0);
  expect(fixture.teacherRequests).toEqual([]);
 });
}
for(const registrationType of ['self','assigned'] as const) test(`${registrationType} activity registration uses the real current group field`,async({page})=>{
 const fixture=await mockEditor(page,{activity:registrationType,failDetails:registrationType==='self'});
 await openEditor(page);const checkbox=page.getByRole('checkbox',{name:'เปิดรับสมัครนักเรียน',exact:true});
 if(registrationType==='assigned'){await expect(checkbox).toHaveCount(0);return;}
 await checkbox.check();await page.getByRole('button',{name:'บันทึกข้อมูลกลุ่ม',exact:true}).click();
 await expect(page.getByRole('button',{name:'ใช้ข้อมูลกลุ่มล่าสุด',exact:true})).toBeVisible();
 await expect(checkbox).toBeChecked();expect(fixture.detailRequests[0].registrationOpen).toBe(true);
 await page.getByRole('button',{name:'ใช้ข้อมูลกลุ่มล่าสุด',exact:true}).click();await expect(checkbox).not.toBeChecked();
 await checkbox.check();await page.getByRole('button',{name:'บันทึกข้อมูลกลุ่ม',exact:true}).click();
 await expect.poll(()=>fixture.stored().registrationOpen).toBe(true);
 expect(fixture.detailRequests[1]).toMatchObject({registrationOpen:true,rowVersion:4});
});
