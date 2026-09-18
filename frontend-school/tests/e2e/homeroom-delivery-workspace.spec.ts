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
	group: '81000000-0000-4000-8000-000000000001',
	version: '82000000-0000-4000-8000-000000000001'
};

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

function homeroomWorkspace(timetableVersionId: string | null = null) {
	return {
		academicYearId: ids.year,
		academicTermId: ids.term,
		timetableVersionId,
		timetableVersionStatus: null,
		timetableVersionEffectiveFrom: null,
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
				curriculumVersionId: ids.curriculum,
				expectedCount: 1,
				readyCount: 1,
				blockers: [],
				extraOfferings: [],
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
						alignmentStates: ['matches_curriculum'],
						schedulingMode: null,
						standardPeriodsPerWeek: 4,
						weeklyPeriodTarget: 4,
						teacherState: 'assigned',
						timetableState: 'scheduled',
						groups: [
							{
								id: ids.group,
								code: 'MATH-COMBINE',
								name: 'คณิตเรียนรวม',
								status: 'draft',
								rosterStatus: 'draft',
								teachersLocked: false,
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
	};
}

async function mockDelivery(page: Page, contextGate?: Promise<void>, menuGate?: Promise<void>) {
	let pageViewRequests = 0;
	let legacyPrimaryRequests = 0;
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
				await contextGate;
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
			if (url.pathname === '/api/menu/user') {
				await menuGate;
				await fulfill(route, {
					groups: [
						{
							code: 'academic_delivery',
							displayOrder: 1,
							icon: 'Workflow',
							name: 'การจัดการเรียนการสอน',
							workspaceCode: 'academic',
							workspaceIcon: 'GraduationCap',
							workspaceName: 'วิชาการ',
							workspaceOrder: 1,
							items: [
								{
									id: 'a0000000-0000-4000-8000-000000000001',
									code: 'delivery',
									name: 'การเปิดสอน',
									icon: 'Workflow',
									path: '/staff/academic/delivery'
								},
								{
									id: 'a0000000-0000-4000-8000-000000000002',
									code: 'delivery-version',
									name: 'การเปิดสอนรุ่นถัดไป',
									icon: 'Workflow',
									path: `/staff/academic/delivery?timetableVersionId=${ids.version}`
								}
							]
						}
					]
				});
				return;
			}
			if (url.pathname === '/api/academic/delivery/page-view') {
				pageViewRequests += 1;
				expect(url.searchParams.get('academicYearId')).toBe(ids.year);
				expect(url.searchParams.get('academicTermId')).toBe(ids.term);
				await fulfill(route, {
					workspace: homeroomWorkspace(url.searchParams.get('timetableVersionId')),
					changeSets: [],
					overview: null
				});
				return;
			}
			if (
				url.pathname === '/api/academic/delivery/homerooms' ||
				url.pathname === '/api/academic/term-change-sets'
			) {
				legacyPrimaryRequests += 1;
				await fulfill(
					route,
					url.pathname === '/api/academic/delivery/homerooms' ? homeroomWorkspace() : []
				);
				return;
			}
			if (url.pathname === '/api/academic/delivery/workspace') {
				offeringOverviewRequests += 1;
				await fulfill(route, { academicTermId: ids.term, offerings: [] });
				return;
			}
			if (url.pathname === '/api/me/work-items/counts')
				return void (await fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				}));
			if (url.pathname === '/api/me/work-items') return void (await fulfill(route, { items: [] }));
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
	return {
		pageViewRequestCount: () => pageViewRequests,
		legacyPrimaryRequestCount: () => legacyPrimaryRequests,
		overviewRequestCount: () => offeringOverviewRequests
	};
}

test('starts academic context priming without waiting for the menu response', async ({ page }) => {
	let releaseMenu: () => void = () => {};
	const menuGate = new Promise<void>((resolve) => {
		releaseMenu = resolve;
	});
	await mockDelivery(page, undefined, menuGate);
	const contextRequest = page.waitForRequest(
		(request) => new URL(request.url()).pathname === '/api/academic/context/options'
	);

	try {
		await page.goto('/staff/academic/core');
		await contextRequest;
	} finally {
		releaseMenu();
	}
});

test('opens homeroom-first and loads the offering projection only after changing tabs', async ({
	page
}) => {
	const { pageViewRequestCount, legacyPrimaryRequestCount, overviewRequestCount } =
		await mockDelivery(page);
	await page.goto(`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`);
	await expect(page.getByRole('heading', { name: 'จัดการการเปิดสอน' })).toBeVisible();
	await expect(page.getByText('ม.1/1', { exact: true })).toBeVisible();
	await expect(page.getByText('เรียนรวมหลายห้อง')).toBeVisible();
	expect(pageViewRequestCount()).toBe(1);
	expect(legacyPrimaryRequestCount()).toBe(0);
	expect(overviewRequestCount()).toBe(0);

	const nextVersionLink = page.getByRole('link', { name: 'การเปิดสอนรุ่นถัดไป' });
	await nextVersionLink.hover();
	await expect.poll(pageViewRequestCount).toBe(2);
	await nextVersionLink.click();
	await expect(page).toHaveURL(new RegExp(`timetableVersionId=${ids.version}`));
	expect(pageViewRequestCount()).toBe(2);

	await page.getByRole('tab', { name: 'มุมมองรายวิชา/กิจกรรม' }).click();
	await expect.poll(overviewRequestCount).toBe(1);
	expect(pageViewRequestCount()).toBe(2);
});

test('primes a complete Delivery destination before hover preload and navigation', async ({
	page
}) => {
	const { pageViewRequestCount } = await mockDelivery(page);
	await page.goto('/staff/work');

	await page.getByRole('button', { name: 'การจัดการเรียนการสอน', exact: true }).click();
	const deliveryLink = page.getByRole('link', { name: 'การเปิดสอน', exact: true });
	await expect(deliveryLink).toHaveAttribute(
		'href',
		new RegExp(`academicYearId=${ids.year}.*academicTermId=${ids.term}`)
	);
	await deliveryLink.hover();
	await expect.poll(pageViewRequestCount).toBe(1);

	await deliveryLink.click();
	await expect(page).toHaveURL(new RegExp(`academicYearId=${ids.year}`));
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.term}`));
	await expect(page.getByText('เลือกปีการศึกษาและภาคเรียนก่อน')).toHaveCount(0);
	expect(pageViewRequestCount()).toBe(1);
});

test('does not expose academic navigation before its destination context is ready', async ({
	page
}) => {
	let releaseContext: () => void = () => {};
	const contextGate = new Promise<void>((resolve) => {
		releaseContext = resolve;
	});
	const { pageViewRequestCount } = await mockDelivery(page, contextGate);
	const menuResponse = page.waitForResponse(
		(response) => new URL(response.url()).pathname === '/api/menu/user'
	);
	await page.goto('/staff/work');
	await menuResponse;
	await page.evaluate(
		() =>
			new Promise<void>((resolve) => {
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
			})
	);

	const academicSection = page.getByRole('button', {
		name: 'การจัดการเรียนการสอน',
		exact: true
	});
	try {
		await expect(academicSection).toHaveCount(0);
	} finally {
		releaseContext();
	}
	await academicSection.click();
	const deliveryLink = page.getByRole('link', { name: 'การเปิดสอน', exact: true });
	await expect(deliveryLink).toHaveAttribute(
		'href',
		new RegExp(`academicYearId=${ids.year}.*academicTermId=${ids.term}`)
	);
	await deliveryLink.click();

	await expect(page).toHaveURL(new RegExp(`academicYearId=${ids.year}`));
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.term}`));
	await expect(page.getByText('เลือกปีการศึกษาและภาคเรียนก่อน')).toHaveCount(0);
	expect(pageViewRequestCount()).toBe(1);
});
