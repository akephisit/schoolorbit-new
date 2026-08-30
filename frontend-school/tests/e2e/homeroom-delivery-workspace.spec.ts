import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

const ids = {
	year: '10000000-0000-4000-8000-000000000001',
	term: '20000000-0000-4000-8000-000000000001',
	grade: '30000000-0000-4000-8000-000000000001',
	program: '40000000-0000-4000-8000-000000000001',
	curriculum: '50000000-0000-4000-8000-000000000001',
	homeroom: '60000000-0000-4000-8000-000000000001',
	otherHomeroom: '60000000-0000-4000-8000-000000000002',
	requirement: '70000000-0000-4000-8000-000000000001',
	catalog: '71000000-0000-4000-8000-000000000001',
	offering: '80000000-0000-4000-8000-000000000001',
	group: '81000000-0000-4000-8000-000000000001'
};

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

async function mockDelivery(page: Page) {
	let offeringOverviewRequests = 0;
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: '90000000-0000-4000-8000-000000000001',
					username: 'academic-test',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					createdAt: '2026-08-29T00:00:00Z',
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
			if (url.pathname === '/api/academic/delivery/homerooms') {
				expect(url.searchParams.get('academicYearId')).toBe(ids.year);
				expect(url.searchParams.get('academicTermId')).toBe(ids.term);
				await fulfill(route, {
					academicYearId: ids.year,
					academicTermId: ids.term,
					homerooms: [
						{
							homeroom: {
								id: ids.homeroom,
								name: 'ม.1/1',
								gradeLevel: 'มัธยมศึกษาปีที่ 1',
								gradeLevelId: ids.grade
							},
							gradeLevel: {
								id: ids.grade,
								code: 'M1',
								name: 'มัธยมศึกษาปีที่ 1',
								short_name: 'ม.1',
								level_type: 'secondary',
								level_order: 301
							},
							studyProgram: {
								id: ids.program,
								code: 'DEFAULT',
								name: 'แผนมาตรฐาน',
								curriculumId: ids.curriculum,
								curriculumName: 'หลักสูตร 2570'
							},
							expectedCount: 1,
							readyCount: 1,
							blockers: [],
							items: [
								{
									requirementId: ids.requirement,
									resourceKind: 'course',
									catalogVersionId: ids.catalog,
									code: 'ค21101',
									name: 'คณิตศาสตร์พื้นฐาน',
									requirementKind: 'required',
									offeringId: ids.offering,
									offeringState: 'draft',
									groupMode: 'combined',
									teacherState: 'assigned',
									timetableState: 'scheduled',
									groups: [
										{
											id: ids.group,
											code: 'MATH-COMBINE',
											name: 'คณิตเรียนรวม',
											status: 'draft',
											rosterStatus: 'draft',
											homeroomIds: [ids.homeroom, ids.otherHomeroom],
											homeroomNames: ['ม.1/1', 'ม.1/2'],
											primaryTeacherCount: 1,
											timetableEntryCount: 3
										}
									]
								}
							]
						}
					],
					unlinked: []
				});
				return;
			}
			if (url.pathname === '/api/academic/term-change-sets') {
				await fulfill(route, []);
				return;
			}
			if (url.pathname === '/api/academic/delivery/workspace') {
				offeringOverviewRequests += 1;
				await fulfill(route, { academicTermId: ids.term, offerings: [] });
				return;
			}
			if (url.pathname === '/api/menu/user') return void (await fulfill(route, { groups: [] }));
			if (url.pathname === '/api/me/work-items/counts')
				return void (await fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				}));
			if (url.pathname === '/api/notifications')
				return void (await fulfill(route, { items: [], unread_count: 0 }));
			if (url.pathname === '/api/notifications/stream')
				return void (await route.fulfill({
					status: 200,
					contentType: 'text/event-stream',
					body: ''
				}));
			await fulfill(route, {});
		}
	);
	return () => offeringOverviewRequests;
}

test('opens homeroom-first and loads the offering projection only after changing tabs', async ({
	page
}) => {
	const overviewRequestCount = await mockDelivery(page);
	await page.goto(`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`);
	await expect(page.getByRole('heading', { name: 'จัดการการเปิดสอน' })).toBeVisible();
	await expect(page.getByText('ม.1/1', { exact: true })).toBeVisible();
	await expect(page.getByText('เรียนรวมหลายห้อง')).toBeVisible();
	expect(overviewRequestCount()).toBe(0);

	await page.getByRole('tab', { name: 'มุมมองรายวิชา/กิจกรรม' }).click();
	await expect.poll(overviewRequestCount).toBe(1);
});
