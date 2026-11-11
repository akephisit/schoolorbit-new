import { expect, test, type Page, type Route } from '@playwright/test';
import { mockStaffHome, id, year } from './fixtures/staff-home-route-data';
import { PERMISSIONS } from '../../src/lib/permissions/registry.generated';

test.use({ serviceWorkers: 'block' });
const roomId = id(801);
const gradeId = id(802);
const programId = id(803);
const stamp = '2026-11-11T00:00:00Z';
const room = {
	id: roomId,
	academicYearId: year,
	code: 'M1-1',
	name: 'ม.1/1',
	roomNumber: '1',
	gradeLevelId: gradeId,
	studyProgramId: programId,
	gradeLevelName: 'มัธยมศึกษาปีที่ 1',
	studyProgramName: 'แผนวิทยาศาสตร์',
	capacity: 40,
	studentCount: 1,
	rowVersion: 1,
	migrated: false,
	createdAt: stamp,
	updatedAt: stamp
};
const schoolYear = {
	id: year,
	year: 2569,
	name: '2569',
	status: 'planning',
	isVisible: true,
	defaultSchoolDays: ['MON', 'TUE', 'WED', 'THU', 'FRI'],
	rowVersion: 1,
	migrated: false,
	createdAt: stamp,
	updatedAt: stamp
};
function reply(route: Route, data: unknown) {
	return route.fulfill({
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}
async function foundation(page: Page, permissions: string[]) {
	await mockStaffHome(page, { permissions });
	const reads: string[] = [];
	await page.route(
		(url) =>
			url.pathname.startsWith('/api/academic/') || url.pathname === '/api/lookup/grade-levels',
		async (route) => {
			const path = new URL(route.request().url()).pathname;
			if (path === '/api/academic/context/options') return route.fallback();
			reads.push(path);
			if (path === '/api/academic/homerooms') return reply(route, [room]);
			if (path === '/api/academic/homerooms/year-context')
				return reply(route, { academicYearId: year, name: '2569', status: 'planning' });
			if (path === '/api/academic/homeroom-advisors') return reply(route, []);
			if (path === '/api/academic/student-years/homeroom-options') return reply(route, [room]);
			if (path === '/api/academic/placements') return reply(route, []);
			if (path === '/api/academic/student-years')
				return reply(route, [
					{
						id: id(804),
						academicYearId: year,
						studentId: id(805),
						studentName: 'นักเรียน ทดสอบ',
						studentCode: '0001',
						gradeLevelId: gradeId,
						gradeLevelName: 'มัธยมศึกษาปีที่ 1',
						studyProgramId: programId,
						studyProgramName: 'แผนวิทยาศาสตร์',
						status: 'planned',
						rowVersion: 1,
						createdAt: stamp,
						updatedAt: stamp
					}
				]);
			if (path === '/api/lookup/grade-levels' || path === '/api/academic/study-program-options')
				return reply(route, []);
			if (path === '/api/academic/terms' && route.request().method() === 'POST')
				return reply(route, {
					...route.request().postDataJSON(),
					id: id(807),
					name: 'ภาคเรียนที่ 1',
					code: '1',
					sequence: 1,
					termType: 'regular',
					status: 'planning',
					rowVersion: 1,
					isVisible: true,
					migrated: false,
					createdAt: stamp,
					updatedAt: stamp
				});
			if (path === '/api/academic/setup/workspace')
				return reply(route, {
					canReadYears: permissions.some((permission) => permission.startsWith('academic_year.')),
					canReadTerms: permissions.some((permission) => permission.startsWith('academic_term.')),
					years: [schoolYear],
					terms: [],
					bellSchedules: permissions.includes(PERMISSIONS.ACADEMIC_TERM_MANAGE_SCHOOL)
						? [
								{
									id: id(806),
									academicYearId: year,
									name: 'ตารางเวลาทดสอบ',
									code: 'STD',
									isDefault: true,
									status: 'draft',
									canClone: true,
									rowVersion: 1,
									createdAt: stamp,
									updatedAt: stamp
								}
							]
						: [],
					bellScheduleSetupYearIds: [],
					bellScheduleSetupTermIds: [],
					schoolDaysEditableTermIds: [],
					contextSettings: null
				});
			return route.fulfill({
				status: 403,
				contentType: 'application/json',
				body: JSON.stringify({ success: false, error: 'unrelated read denied' })
			});
		}
	);
	return reads;
}

test('homeroom reader sees display references without requesting curriculum editor choices', async ({
	page
}) => {
	const reads = await foundation(page, [PERMISSIONS.HOMEROOM_READ_SCHOOL]);
	await page.goto(`/staff/academic/homerooms?academicYearId=${year}`);
	await expect(page.getByTestId('homerooms-ready')).toContainText('ม.1/1');
	await expect(page.getByTestId('homerooms-ready')).toContainText('แผนวิทยาศาสตร์');
	await expect(page.getByRole('button', { name: 'เพิ่มห้องประจำชั้น' })).toHaveCount(0);
	expect(reads).not.toContain('/api/academic/study-program-options');
	expect(reads).not.toContain('/api/lookup/grade-levels');
});
test('homeroom manager starts editor choices only when the room editor opens', async ({ page }) => {
	const reads = await foundation(page, [PERMISSIONS.HOMEROOM_MANAGE_SCHOOL]);
	await page.goto(`/staff/academic/homerooms?academicYearId=${year}`);
	await expect(page.getByTestId('homerooms-ready')).toContainText('ม.1/1');
	expect(reads).not.toContain('/api/academic/study-program-options');
	await page.getByRole('button', { name: 'เพิ่มห้องประจำชั้น' }).click();
	await expect(page.getByRole('dialog')).toBeVisible();
	await expect
		.poll(() => reads.filter((path) => path === '/api/academic/study-program-options').length)
		.toBe(1);
	expect(reads.filter((path) => path === '/api/lookup/grade-levels')).toHaveLength(1);
});
test('student-year reader uses its own room references without homeroom or curriculum access', async ({
	page
}) => {
	const reads = await foundation(page, [PERMISSIONS.STUDENT_ACADEMIC_YEAR_READ_SCHOOL]);
	await page.goto(`/staff/academic/student-years?academicYearId=${year}`);
	await expect(page.getByText('นักเรียน ทดสอบ', { exact: true })).toBeVisible();
	expect(reads).toContain('/api/academic/student-years/homeroom-options');
	expect(reads).not.toContain('/api/academic/homerooms');
	expect(reads).not.toContain('/api/academic/study-program-options');
});
for (const kind of ['year', 'term'] as const) {
	test(`${kind} manager can edit its own setup without the other manage grant`, async ({
		page
	}) => {
		await foundation(page, [
			kind === 'year'
				? PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL
				: PERMISSIONS.ACADEMIC_TERM_MANAGE_SCHOOL
		]);
		await page.goto(`/staff/academic/year-lifecycle?academicYearId=${year}`);
		await expect(page.getByTestId('academic-setup-ready')).toBeVisible();
		await expect(page.getByRole('button', { name: 'เพิ่มปีการศึกษา', exact: true })).toHaveCount(
			kind === 'year' ? 1 : 0
		);
		await expect(page.getByRole('button', { name: 'แก้ไขปีการศึกษา', exact: true })).toHaveCount(
			kind === 'year' ? 1 : 0
		);
		await expect(page.getByText('ไม่มีสิทธิ์ดูข้อมูลปีการศึกษา', { exact: true })).toHaveCount(0);
	});
}

for (const path of ['/staff/academic/core', '/staff/academic/periods']) {
	test(`term-only reader can enter ${path}`, async ({ page }) => {
		await foundation(page, [PERMISSIONS.ACADEMIC_TERM_READ_SCHOOL]);
		await page.goto(path);
		await expect(page.getByTestId('academic-setup-ready')).toBeVisible();
		await expect(page).not.toHaveURL(/403/);
	});
}
test('year-only setup omits the unreadable term section', async ({ page }) => {
	await foundation(page, [PERMISSIONS.ACADEMIC_YEAR_READ_SCHOOL]);
	await page.goto(`/staff/academic/year-lifecycle?academicYearId=${year}`);
	await expect(page.getByTestId('academic-setup-ready')).toBeVisible();
	await expect(page.getByRole('region', { name: 'ภาคเรียน', exact: true })).toHaveCount(0);
	await expect(page.getByText('ยังไม่มีภาคเรียน', { exact: false })).toHaveCount(0);
});
test('term-only manager creates and reopens a term editor without year management', async ({
	page
}) => {
	await foundation(page, [PERMISSIONS.ACADEMIC_TERM_MANAGE_SCHOOL]);
	await page.goto(`/staff/academic/year-lifecycle?academicYearId=${year}`);
	const terms = page.getByRole('region', { name: 'ภาคเรียน', exact: true });
	await expect(terms.getByRole('button', { name: 'เพิ่มภาคเรียน', exact: true })).toBeEnabled();
	const created = page.waitForRequest(
		(request) =>
			request.method() === 'POST' && new URL(request.url()).pathname === '/api/academic/terms'
	);
	await terms.getByRole('button', { name: 'เพิ่มภาคเรียน', exact: true }).click();
	expect((await created).postDataJSON()).toMatchObject({ academicYearId: year });
	await expect(terms.getByText('ภาคเรียนที่ 1', { exact: true }).first()).toBeVisible();
	await terms.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await expect(terms.getByRole('button', { name: 'บันทึกภาคเรียน', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'แก้ไขปีการศึกษา', exact: true })).toHaveCount(0);
});
test('revoking homeroom management closes a pending editor and rejects its late choices', async ({
	page
}) => {
	await foundation(page, [PERMISSIONS.HOMEROOM_READ_SCHOOL, PERMISSIONS.HOMEROOM_MANAGE_SCHOOL]);
	let release = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	await page.route('**/api/academic/study-program-options?**', async (route) => {
		await gate;
		await reply(route, []);
	});
	await page.goto(`/staff/academic/homerooms?academicYearId=${year}`);
	await page.getByRole('button', { name: 'เพิ่มห้องประจำชั้น' }).click();
	await expect(page.getByRole('dialog')).toBeVisible();
	await page.evaluate(async () => {
		const modulePath = '/src/lib/stores/permissions.ts';
		const { setPermissions } = await import(modulePath);
		setPermissions(['homeroom.read.school']);
	});
	await expect(page.getByRole('dialog')).toHaveCount(0);
	release();
	await expect(page.getByTestId('homerooms-ready')).toContainText('ม.1/1');
	await expect(page.getByRole('button', { name: 'เพิ่มห้องประจำชั้น' })).toHaveCount(0);
});
