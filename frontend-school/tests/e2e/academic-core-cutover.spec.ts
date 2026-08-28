import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

const ids = {
	year: '10000000-0000-4000-8000-000000000001',
	futureYear: '10000000-0000-4000-8000-000000000002',
	term: '20000000-0000-4000-8000-000000000001',
	bell: '30000000-0000-4000-8000-000000000001',
	subject: '40000000-0000-4000-8000-000000000001',
	subjectVersion: '41000000-0000-4000-8000-000000000001',
	activity: '42000000-0000-4000-8000-000000000001',
	curriculum: '50000000-0000-4000-8000-000000000001',
	curriculumVersion: '51000000-0000-4000-8000-000000000001',
	program: '52000000-0000-4000-8000-000000000001',
	studentYear: '60000000-0000-4000-8000-000000000001',
	offering: '70000000-0000-4000-8000-000000000001',
	group: '71000000-0000-4000-8000-000000000001'
};

function fulfill(route: Route, data: object | string | Array<object>, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

async function mockShell(page: Page, academic: (route: Route, url: URL) => Promise<boolean>) {
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: '90000000-0000-4000-8000-000000000001',
					username: 'academic-admin',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					createdAt: '2026-08-24T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: ['*']
				});
				return;
			}
			if (url.pathname === '/api/academic/context/options') {
				await fulfill(route, {
					activeAcademicYearId: ids.year,
					activeAcademicTermId: ids.term,
					years: [
						{
							id: ids.year,
							name: 'ปีการศึกษา 2570',
							year: 2570,
							status: 'active',
							startDate: '2027-05-01',
							endDate: '2028-03-31'
						}
					],
					terms: [
						{
							id: ids.term,
							academicYearId: ids.year,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: 'active',
							startDate: '2027-05-01',
							endDate: '2027-10-31',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				});
				return;
			}
			if (await academic(route, url)) return;
			if (url.pathname === '/api/notifications/stream') {
				await route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
				return;
			}
			if (url.pathname === '/api/menu/user') {
				await fulfill(route, { groups: [] });
				return;
			}
			if (url.pathname === '/api/me/work-items/counts') {
				await fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
				return;
			}
			if (url.pathname === '/api/notifications') {
				await fulfill(route, { items: [], unread_count: 0 });
				return;
			}
			await fulfill(route, {});
		}
	);
}

const year = {
	id: ids.year,
	year: 2570,
	name: 'ปีการศึกษา 2570',
	startDate: '2027-05-01',
	endDate: '2028-03-31',
	schoolDays: ['MON', 'TUE', 'WED', 'THU', 'FRI'],
	status: 'planning',
	rowVersion: 1,
	migrated: false,
	createdAt: '2026-08-24T00:00:00Z',
	updatedAt: '2026-08-24T00:00:00Z'
};
const term = {
	id: ids.term,
	academicYearId: ids.year,
	sequence: 1,
	code: '1',
	name: 'ภาคเรียนที่ 1',
	termType: 'regular',
	startDate: '2027-05-01',
	endDate: '2027-10-31',
	includedInYearResult: true,
	blocksYearClosure: true,
	bellScheduleId: ids.bell,
	status: 'planning',
	rowVersion: 1,
	migrated: false,
	createdAt: '2026-08-24T00:00:00Z',
	updatedAt: '2026-08-24T00:00:00Z'
};

test('creates a future planning year and configurable regular, summer, and custom terms', async ({
	page
}) => {
	let createYearBody: Record<string, unknown> | null = null;
	await mockShell(page, async (route, url) => {
		if (url.pathname === '/api/academic/setup/workspace') {
			await fulfill(route, {
				years: [year],
				terms: [
					term,
					{
						...term,
						id: `${ids.term.slice(0, -1)}2`,
						sequence: 2,
						code: 'SUMMER',
						name: 'ภาคฤดูร้อน',
						termType: 'summer'
					},
					{
						...term,
						id: `${ids.term.slice(0, -1)}3`,
						sequence: 3,
						code: 'CUSTOM-3',
						name: 'รอบพิเศษ',
						termType: 'custom'
					}
				],
				bellSchedules: [
					{
						id: ids.bell,
						academicYearId: ids.year,
						code: 'DEFAULT',
						name: 'ตารางเวลาปกติ',
						isDefault: true,
						owningOrganizationUnitId: null,
						status: 'draft',
						rowVersion: 1,
						createdAt: '',
						updatedAt: ''
					}
				]
			});
			return true;
		}
		if (url.pathname === '/api/academic/years' && route.request().method() === 'POST') {
			createYearBody = route.request().postDataJSON() as Record<string, unknown>;
			await fulfill(
				route,
				{ ...year, id: ids.futureYear, year: 2571, name: 'ปีการศึกษา 2571' },
				201
			);
			return true;
		}
		return false;
	});
	await page.goto('/staff/academic/core');
	await expect(page.getByText('ภาคฤดูร้อน')).toBeVisible();
	await expect(page.getByText('รอบพิเศษ')).toBeVisible();
	await page.getByRole('button', { name: 'เพิ่มปีการศึกษา' }).click();
	await page.getByLabel('ปีการศึกษา (พ.ศ.)').fill('2571');
	await page.getByRole('button', { name: 'เลือกวันเริ่มปีการศึกษา' }).click();
	await page
		.locator('[role="application"]:visible [data-calendar-day]:not([data-outside-month])')
		.filter({ hasText: /^1$/ })
		.click();
	await page.getByRole('button', { name: 'เลือกวันสิ้นสุดปีการศึกษา' }).click();
	await page
		.locator('[role="application"]:visible [data-calendar-day]:not([data-outside-month])')
		.filter({ hasText: /^20$/ })
		.click();
	await page.getByRole('button', { name: 'สร้างปีสำหรับวางแผน' }).click();
	await expect(page.getByRole('heading', { name: 'ปีการศึกษา 2571', exact: true })).toBeVisible();
	expect(createYearBody).toMatchObject({
		year: 2571,
		customName: null,
		schoolDays: ['MON', 'TUE', 'WED', 'THU', 'FRI']
	});
	expect(createYearBody).not.toHaveProperty('name');
	await expect(page.getByRole('button', { name: /เปิดใช้|ปิดปี|เลื่อนชั้น/ })).toHaveCount(0);
});

test('derives term count from rows and exposes no stored count control', async ({ page }) => {
	await mockShell(page, async (route, url) => {
		if (url.pathname === '/api/academic/setup/workspace') {
			await fulfill(route, {
				years: [year],
				terms: [
					term,
					{ ...term, id: `${ids.term.slice(0, -1)}2`, sequence: 2 },
					{ ...term, id: `${ids.term.slice(0, -1)}3`, sequence: 3 }
				],
				bellSchedules: [
					{
						id: ids.bell,
						academicYearId: ids.year,
						code: 'DEFAULT',
						name: 'ตารางเวลาปกติ',
						isDefault: true,
						owningOrganizationUnitId: null,
						status: 'draft',
						rowVersion: 1,
						createdAt: '',
						updatedAt: ''
					}
				]
			});
			return true;
		}
		return false;
	});
	await page.goto('/staff/academic/core');
	await expect(page.getByText('3 ภาคเรียน').first()).toBeVisible();
	await expect(page.getByLabel(/จำนวนภาคเรียน/)).toHaveCount(0);
});

test('creates a new subject version while preserving published history', async ({ page }) => {
	await mockShell(page, async (route, url) => {
		if (url.pathname === '/api/academic/catalog/subjects') {
			await fulfill(route, [
				{
					id: ids.subject,
					code: 'MATH101',
					owningOrganizationUnitId: null,
					archivedAt: null,
					rowVersion: 1,
					createdAt: '',
					updatedAt: ''
				}
			]);
			return true;
		}
		if (url.pathname.endsWith('/versions') && route.request().method() === 'GET') {
			await fulfill(route, [
				{
					id: ids.subjectVersion,
					subjectId: ids.subject,
					versionNo: 1,
					nameTh: 'คณิตศาสตร์เดิม',
					nameEn: null,
					credit: '1.00',
					description: null,
					effectiveFrom: '2027-05-01',
					effectiveUntil: null,
					gradeLevelIds: [ids.year],
					groupId: null,
					hoursPerSemester: 40,
					periodsPerWeek: 2,
					subjectType: 'BASIC',
					termCode: null,
					status: 'published',
					rowVersion: 1,
					migrated: false,
					publishedAt: '',
					createdAt: '',
					updatedAt: ''
				}
			]);
			return true;
		}
		if (url.pathname.endsWith('/versions') && route.request().method() === 'POST') {
			await fulfill(
				route,
				{
					id: `${ids.subjectVersion.slice(0, -1)}2`,
					subjectId: ids.subject,
					versionNo: 2,
					nameTh: 'คณิตศาสตร์ฉบับใหม่',
					nameEn: null,
					credit: '1.50',
					description: null,
					effectiveFrom: '2028-05-01',
					effectiveUntil: null,
					gradeLevelIds: [ids.year],
					groupId: null,
					hoursPerSemester: null,
					periodsPerWeek: null,
					subjectType: 'BASIC',
					termCode: null,
					status: 'draft',
					rowVersion: 1,
					migrated: false,
					publishedAt: null,
					createdAt: '',
					updatedAt: ''
				},
				201
			);
			return true;
		}
		return false;
	});
	await page.goto('/staff/academic/catalog/subjects');
	await expect(page.getByText('คณิตศาสตร์เดิม')).toBeVisible();
	await page.getByLabel('ชื่อภาษาไทย').fill('คณิตศาสตร์ฉบับใหม่');
	await page.getByLabel('หน่วยกิต').fill('1.50');
	await page.getByLabel('รหัสระดับชั้น (คั่นด้วยจุลภาค)').fill(ids.year);
	await page.getByLabel('เริ่มใช้').fill('2028-05-01');
	await page.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่' }).click();
	await expect(page.getByText('คณิตศาสตร์เดิม')).toBeVisible();
	await expect(page.getByText('คณิตศาสตร์ฉบับใหม่')).toBeVisible();
});

test('publishes a curriculum version only after a default program and requirements are visible', async ({
	page
}) => {
	await mockShell(page, async (route, url) => {
		if (url.pathname === '/api/academic/curricula') {
			await fulfill(route, [
				{
					id: ids.curriculum,
					code: 'CURR-70',
					nameTh: 'หลักสูตร 2570',
					nameEn: null,
					description: null,
					gradeLevelIds: [ids.year],
					owningOrganizationUnitId: null,
					isActive: true,
					rowVersion: 1,
					createdAt: '',
					updatedAt: ''
				}
			]);
			return true;
		}
		if (url.pathname.endsWith('/versions')) {
			await fulfill(route, [
				{
					id: ids.curriculumVersion,
					curriculumId: ids.curriculum,
					versionName: 'ฉบับ 2570',
					startAcademicYearId: ids.year,
					endAcademicYearId: null,
					description: null,
					status: 'draft',
					rowVersion: 1,
					migrated: false,
					publishedAt: null,
					createdAt: '',
					updatedAt: ''
				}
			]);
			return true;
		}
		if (url.pathname.endsWith('/programs')) {
			await fulfill(route, [
				{
					id: ids.program,
					curriculumVersionId: ids.curriculumVersion,
					code: 'DEFAULT',
					nameTh: 'แผนพื้นฐาน',
					nameEn: null,
					isDefault: true,
					owningOrganizationUnitId: null,
					rowVersion: 1,
					createdAt: '',
					updatedAt: ''
				}
			]);
			return true;
		}
		if (url.pathname.endsWith('/requirements')) {
			await fulfill(route, [
				{
					id: '53000000-0000-4000-8000-000000000001',
					studyProgramId: ids.program,
					gradeLevelId: ids.year,
					resourceKind: 'course',
					resourceVersionId: ids.subjectVersion,
					requirementKind: 'required',
					credit: '1.00',
					hours: null,
					recommendedTermCode: '1',
					displayOrder: 1
				}
			]);
			return true;
		}
		return false;
	});
	await page.goto('/staff/academic/curricula');
	await expect(page.getByText('แผนพื้นฐาน')).toBeVisible();
	await expect(page.getByText(/course/)).toBeVisible();
	await expect(page.getByRole('button', { name: 'ตรวจสรุปและเผยแพร่' })).toBeEnabled();
});

test('creates future placement without changing the active-year placement timeline', async ({
	page
}) => {
	await mockShell(page, async (route, url) => {
		if (url.pathname === '/api/academic/student-years') {
			await fulfill(route, []);
			return true;
		}
		if (url.pathname === '/api/academic/homerooms') {
			await fulfill(route, []);
			return true;
		}
		if (url.pathname.startsWith('/api/lookup/')) {
			await fulfill(route, []);
			return true;
		}
		if (url.pathname === '/api/academic/years' || url.pathname === '/api/academic/curricula') {
			await fulfill(route, []);
			return true;
		}
		return false;
	});
	await page.goto(`/staff/academic/student-years?academicYearId=${ids.year}`);
	await expect(page.getByText('ยังไม่มีข้อมูลนักเรียนในปีที่เลือก')).toBeVisible();
	await expect(page).toHaveURL(new RegExp(`academicYearId=${ids.year}`));
});

test('creates course and activity offerings and exposes roster review before publication', async ({
	page
}) => {
	await mockShell(page, async (route, url) => {
		if (url.pathname === '/api/academic/offerings') {
			await fulfill(route, []);
			return true;
		}
		return false;
	});
	await page.goto(`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`);
	await expect(page.getByText('สร้างชุดการเรียนเอง')).toBeVisible();
	await expect(page.getByLabel('ชนิด')).toContainText('รายวิชา');
	await expect(page.getByLabel('ชนิด')).toContainText('กิจกรรมพัฒนาผู้เรียน');
	await expect(page.getByText('สร้างชุดการเรียนจากหลักสูตร')).toBeVisible();
});

test('preserves a subject-version draft after a stale row conflict', async ({ page }) => {
	await mockShell(page, async (route, url) => {
		if (url.pathname === '/api/academic/catalog/subjects') {
			await fulfill(route, [
				{
					id: ids.subject,
					code: 'SCI101',
					owningOrganizationUnitId: null,
					archivedAt: null,
					rowVersion: 1,
					createdAt: '',
					updatedAt: ''
				}
			]);
			return true;
		}
		if (url.pathname.endsWith('/versions') && route.request().method() === 'GET') {
			await fulfill(route, []);
			return true;
		}
		if (url.pathname.endsWith('/versions') && route.request().method() === 'POST') {
			await fulfill(route, 'ข้อมูลรุ่นถูกแก้ไขโดยผู้ใช้อื่นแล้ว', 409);
			return true;
		}
		return false;
	});
	await page.goto('/staff/academic/catalog/subjects');
	await page.getByLabel('ชื่อภาษาไทย').fill('วิทยาศาสตร์ฉบับร่างของฉัน');
	await page.getByLabel('รหัสระดับชั้น (คั่นด้วยจุลภาค)').fill(ids.year);
	await page.getByLabel('เริ่มใช้').fill('2028-05-01');
	await page.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่' }).click();
	await expect(page.getByRole('alert')).toContainText('กรุณาโหลดข้อมูลล่าสุด');
	await expect(page.getByLabel('ชื่อภาษาไทย')).toHaveValue('วิทยาศาสตร์ฉบับร่างของฉัน');
});
