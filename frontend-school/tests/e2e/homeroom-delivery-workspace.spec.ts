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
	version: '82000000-0000-4000-8000-000000000001',
	changeSet: '83000000-0000-4000-8000-000000000001'
};

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

function homeroomWorkspace(
	deliveryVersionId: string | null = null,
	deliveryVersionStatus: 'draft' | 'published' | null = null
) {
	return {
		academicYearId: ids.year,
		academicTermId: ids.term,
		deliveryVersionId,
		deliveryVersionStatus,
		deliveryVersionEffectiveFrom: null,
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
						weeklyPeriodTarget: deliveryVersionStatus === 'draft' ? null : 4,
						teacherState: 'assigned',
						deliveryState: deliveryVersionStatus === 'draft' ? 'excluded' : 'included',
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

function changeSetSummary() {
	return {
		id: ids.changeSet,
		academicTermId: ids.term,
		academicYearId: ids.year,
		effectiveFrom: null,
		referenceDate: '2027-08-01',
		reason: 'ปรับการเปิดสอนทดสอบ',
		status: 'draft',
		targetDeliveryVersionId: ids.version,
		updatedAt: '2027-07-01T00:00:00Z'
	};
}

function changeSetDetail() {
	return {
		...changeSetSummary(),
		baseDeliveryVersionId: ids.version,
		rowVersion: 1,
		createdBy: '90000000-0000-4000-8000-000000000001',
		publishedBy: null,
		publishedAt: null,
		cancelledBy: null,
		cancelledAt: null,
		createdAt: '2027-07-01T00:00:00Z',
		items: [],
		changes: [],
		offeringLabels: []
	};
}

type DeliveryMockOptions = {
	homeroomGate?: Promise<void>;
	homeroomRefreshGate?: Promise<void>;
	changeSetSummaryGate?: Promise<void>;
	changeSetDetailGate?: Promise<void>;
	failHomeroomAttempts?: number;
	deliveryVersionStatus?: 'draft' | 'published' | null;
};

async function mockDelivery(
	page: Page,
	contextGate?: Promise<void>,
	menuGate?: Promise<void>,
	options: DeliveryMockOptions = {}
) {
	let pageViewRequests = 0;
	let homeroomRequests = 0;
	let changeSetSummaryRequests = 0;
	let changeSetDetailRequests = 0;
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
									path: `/staff/academic/delivery?deliveryVersionId=${ids.version}`
								}
							]
						}
					]
				});
				return;
			}
			if (url.pathname === '/api/academic/delivery/page-view') {
				pageViewRequests += 1;
				await fulfill(route, 'page-view must not be requested', 500);
				return;
			}
			if (url.pathname === '/api/academic/delivery/homerooms') {
				homeroomRequests += 1;
				expect(url.searchParams.get('academicYearId')).toBe(ids.year);
				expect(url.searchParams.get('academicTermId')).toBe(ids.term);
				await options.homeroomGate;
				if (homeroomRequests > 1) await options.homeroomRefreshGate;
				if (homeroomRequests <= (options.failHomeroomAttempts ?? 0)) {
					await fulfill(route, 'โหลดข้อมูลห้องประจำชั้นไม่สำเร็จ', 503);
				} else {
					await fulfill(
						route,
						homeroomWorkspace(
							url.searchParams.get('deliveryVersionId'),
							options.deliveryVersionStatus
						)
					);
				}
				return;
			}
			if (url.pathname === '/api/academic/delivery-versions') {
				await fulfill(route, [
					{
						id: ids.version,
						academicYearId: ids.year,
						academicTermId: ids.term,
						changeSetId: ids.changeSet,
						sourceVersionId: null,
						effectiveFrom: null,
						referenceDate: '2027-08-01',
						effectiveUntil: null,
						status: options.deliveryVersionStatus ?? 'draft',
						rowVersion: 1,
						offeringCount: 1,
						groupCount: 1,
						teacherAssignmentCount: 1,
						updatedAt: '2027-07-01T00:00:00Z'
					}
				]);
				return;
			}
			if (url.pathname === `/api/academic/delivery-versions/${ids.version}`) {
				offeringOverviewRequests += 1;
				await fulfill(route, {
					id: ids.version,
					academicYearId: ids.year,
					academicTermId: ids.term,
					status: 'draft',
					snapshot: { offerings: [] }
				});
				return;
			}
			if (url.pathname === '/api/academic/term-change-sets') {
				changeSetSummaryRequests += 1;
				expect(url.searchParams.get('academicTermId')).toBe(ids.term);
				await options.changeSetSummaryGate;
				await fulfill(route, [changeSetSummary()]);
				return;
			}
			if (url.pathname === `/api/academic/term-change-sets/${ids.changeSet}`) {
				changeSetDetailRequests += 1;
				await options.changeSetDetailGate;
				await fulfill(route, changeSetDetail());
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
		homeroomRequestCount: () => homeroomRequests,
		changeSetSummaryRequestCount: () => changeSetSummaryRequests,
		changeSetDetailRequestCount: () => changeSetDetailRequests,
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

test('opens visible regions independently and loads the offering projection only after changing tabs', async ({
	page
}) => {
	const {
		pageViewRequestCount,
		homeroomRequestCount,
		changeSetSummaryRequestCount,
		changeSetDetailRequestCount,
		overviewRequestCount
	} = await mockDelivery(page);
	await page.goto(`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`);
	await expect(page.getByRole('heading', { name: 'จัดการการเปิดสอน' })).toBeVisible();
	await expect(page.getByText('ม.1/1', { exact: true })).toBeVisible();
	await expect(page.getByText('เรียนรวมหลายห้อง')).toBeVisible();
	expect(pageViewRequestCount()).toBe(0);
	expect(homeroomRequestCount()).toBe(1);
	expect(changeSetSummaryRequestCount()).toBe(1);
	expect(changeSetDetailRequestCount()).toBe(1);
	expect(overviewRequestCount()).toBe(0);

	const nextVersionLink = page.getByRole('link', { name: 'การเปิดสอนรุ่นถัดไป' });
	await nextVersionLink.hover();
	await expect.poll(homeroomRequestCount).toBe(2);
	await expect.poll(changeSetSummaryRequestCount).toBe(2);
	await expect.poll(changeSetDetailRequestCount).toBe(2);
	await nextVersionLink.click();
	await expect(page).toHaveURL(new RegExp(`deliveryVersionId=${ids.version}`));
	expect(homeroomRequestCount()).toBe(2);
	expect(changeSetSummaryRequestCount()).toBe(2);
	expect(changeSetDetailRequestCount()).toBe(2);

	await page.getByRole('tab', { name: 'มุมมองรายวิชา/กิจกรรม' }).click();
	await expect.poll(overviewRequestCount).toBe(1);
	expect(pageViewRequestCount()).toBe(0);
});

test('marks unresolved visible regions busy and renders their skeletons', async ({ page }) => {
	let releaseHomerooms: () => void = () => {};
	let releaseSummaries: () => void = () => {};
	const homeroomGate = new Promise<void>((resolve) => {
		releaseHomerooms = resolve;
	});
	const changeSetSummaryGate = new Promise<void>((resolve) => {
		releaseSummaries = resolve;
	});
	await mockDelivery(page, undefined, undefined, {
		homeroomGate,
		changeSetSummaryGate,
		changeSetDetailGate: changeSetSummaryGate
	});

	try {
		await page.goto(
			`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`
		);
		const changeSetRegion = page.getByRole('region', {
			name: 'การจัดการรุ่นเปิดสอน'
		});
		const homeroomRegion = page.getByRole('region', { name: 'มุมมองรายห้อง' });

		await expect(changeSetRegion).toHaveAttribute('aria-busy', 'true');
		await expect(changeSetRegion.locator('[data-slot="skeleton"]')).not.toHaveCount(0);
		await expect(homeroomRegion).toHaveAttribute('aria-busy', 'true');
		await expect(homeroomRegion.locator('[data-slot="skeleton"]')).not.toHaveCount(0);
		await expect(page.getByText('เมื่อเปิดสอนแล้วและต้องเปลี่ยนกลางภาค')).toHaveCount(0);
	} finally {
		releaseHomerooms();
		releaseSummaries();
	}
});

test('renders warm preloaded regions immediately without forcing skeletons', async ({ page }) => {
	const { homeroomRequestCount, changeSetSummaryRequestCount, changeSetDetailRequestCount } =
		await mockDelivery(page);
	await page.goto('/staff/work');
	await page.getByRole('button', { name: 'การจัดการเรียนการสอน', exact: true }).click();
	const deliveryLink = page.getByRole('link', { name: 'การเปิดสอน', exact: true });

	await deliveryLink.hover();
	await expect.poll(homeroomRequestCount).toBe(1);
	await expect.poll(changeSetSummaryRequestCount).toBe(1);
	await expect.poll(changeSetDetailRequestCount).toBe(1);
	await deliveryLink.click();

	const changeSetRegion = page.getByRole('region', {
		name: 'การจัดการรุ่นเปิดสอน'
	});
	const homeroomRegion = page.getByRole('region', { name: 'มุมมองรายห้อง' });
	await expect(changeSetRegion).toHaveAttribute('aria-busy', 'false');
	await expect(homeroomRegion).toHaveAttribute('aria-busy', 'false');
	await expect(page.getByText('ปรับการเปิดสอนทดสอบ', { exact: true })).toBeVisible();
	await expect(page.getByText('ม.1/1', { exact: true })).toBeVisible();
	await expect(changeSetRegion.locator('[data-slot="skeleton"]')).toHaveCount(0);
	await expect(homeroomRegion.locator('[data-slot="skeleton"]')).toHaveCount(0);
});

test('clears incompatible homerooms before painting a new route context', async ({ page }) => {
	let releaseContextChange: () => void = () => {};
	const homeroomRefreshGate = new Promise<void>((resolve) => {
		releaseContextChange = resolve;
	});
	await mockDelivery(page, undefined, undefined, { homeroomRefreshGate });
	await page.goto(`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`);
	await expect(page.getByText('ม.1/1', { exact: true })).toBeVisible();

	try {
		await page.getByRole('link', { name: 'การเปิดสอนรุ่นถัดไป' }).click();
		const homeroomRegion = page.getByRole('region', { name: 'มุมมองรายห้อง' });

		await expect(page).toHaveURL(new RegExp(`deliveryVersionId=${ids.version}`));
		await expect(homeroomRegion).toHaveAttribute('aria-busy', 'true');
		await expect(homeroomRegion.locator('[data-slot="skeleton"]')).not.toHaveCount(0);
		await expect(page.getByText('ม.1/1', { exact: true })).toHaveCount(0);
	} finally {
		releaseContextChange();
	}
});

test('keeps loaded homerooms visible and announces a background refresh', async ({ page }) => {
	let releaseRefresh: () => void = () => {};
	const homeroomRefreshGate = new Promise<void>((resolve) => {
		releaseRefresh = resolve;
	});
	await mockDelivery(page, undefined, undefined, {
		homeroomRefreshGate,
		deliveryVersionStatus: 'draft'
	});

	try {
		await page.goto(
			`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}`
		);
		const homeroomRegion = page.getByRole('region', { name: 'มุมมองรายห้อง' });
		await expect(page.getByText('ม.1/1', { exact: true })).toBeVisible();
		await page.getByRole('button', { name: 'โหลดล่าสุด', exact: true }).click();

		await expect(homeroomRegion).toHaveAttribute('aria-busy', 'true');
		await expect(page.getByText('ม.1/1', { exact: true })).toBeVisible();
		await expect(page.getByTestId('delivery-homerooms-ready')).toBeVisible();
		await expect(
			homeroomRegion.getByRole('status', { name: 'กำลังอัปเดตภาพรวมรายห้อง' })
		).toBeVisible();
		await expect(homeroomRegion.locator('[data-slot="skeleton"]')).toHaveCount(0);
	} finally {
		releaseRefresh();
	}
});

test('primes a complete Delivery destination before hover preload and navigation', async ({
	page
}) => {
	const { homeroomRequestCount, changeSetSummaryRequestCount } = await mockDelivery(page);
	await page.goto('/staff/work');

	await page.getByRole('button', { name: 'การจัดการเรียนการสอน', exact: true }).click();
	const deliveryLink = page.getByRole('link', { name: 'การเปิดสอน', exact: true });
	await expect(deliveryLink).toHaveAttribute(
		'href',
		new RegExp(`academicYearId=${ids.year}.*academicTermId=${ids.term}`)
	);
	await deliveryLink.hover();
	await expect.poll(homeroomRequestCount).toBe(1);
	await expect.poll(changeSetSummaryRequestCount).toBe(1);

	await deliveryLink.click();
	await expect(page).toHaveURL(new RegExp(`academicYearId=${ids.year}`));
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.term}`));
	await expect(page.getByText('เลือกปีการศึกษาและภาคเรียนก่อน')).toHaveCount(0);
	expect(homeroomRequestCount()).toBe(1);
	expect(changeSetSummaryRequestCount()).toBe(1);
});

test('does not expose academic navigation before its destination context is ready', async ({
	page
}) => {
	let releaseContext: () => void = () => {};
	const contextGate = new Promise<void>((resolve) => {
		releaseContext = resolve;
	});
	const { homeroomRequestCount, changeSetSummaryRequestCount } = await mockDelivery(
		page,
		contextGate
	);
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
	expect(homeroomRequestCount()).toBe(1);
	expect(changeSetSummaryRequestCount()).toBe(1);
});

test('renders change-set detail while homerooms are still loading', async ({ page }) => {
	let releaseHomerooms: () => void = () => {};
	const homeroomGate = new Promise<void>((resolve) => {
		releaseHomerooms = resolve;
	});
	await mockDelivery(page, undefined, undefined, { homeroomGate });

	try {
		await page.goto(
			`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`
		);
		await expect(page.getByText('ปรับการเปิดสอนทดสอบ', { exact: true })).toBeVisible();
		await expect(page.getByText('ม.1/1', { exact: true })).toHaveCount(0);
	} finally {
		releaseHomerooms();
	}
});

test('renders the selected opening and homerooms while journal summaries are still loading', async ({
	page
}) => {
	let releaseSummaries: () => void = () => {};
	const changeSetSummaryGate = new Promise<void>((resolve) => {
		releaseSummaries = resolve;
	});
	await mockDelivery(page, undefined, undefined, { changeSetSummaryGate });

	try {
		await page.goto(
			`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`
		);
		await expect(page.getByText('ม.1/1', { exact: true })).toBeVisible();
		await expect(page.getByText('ปรับการเปิดสอนทดสอบ', { exact: true })).toBeVisible();
	} finally {
		releaseSummaries();
	}
});

test('loads an explicitly selected change-set without waiting for its summary list', async ({
	page
}) => {
	let releaseSummaries: () => void = () => {};
	const changeSetSummaryGate = new Promise<void>((resolve) => {
		releaseSummaries = resolve;
	});
	const { changeSetDetailRequestCount } = await mockDelivery(page, undefined, undefined, {
		changeSetSummaryGate
	});

	try {
		await page.goto(
			`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}`
		);
		await expect(page.getByText('ปรับการเปิดสอนทดสอบ', { exact: true })).toBeVisible();
		expect(changeSetDetailRequestCount()).toBe(1);
	} finally {
		releaseSummaries();
	}
});

test('retries only the failed homeroom region', async ({ page }) => {
	const { homeroomRequestCount, changeSetSummaryRequestCount, changeSetDetailRequestCount } =
		await mockDelivery(page, undefined, undefined, { failHomeroomAttempts: 1 });
	await page.goto(`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`);

	await expect(page.getByText('ปรับการเปิดสอนทดสอบ', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('ม.1/1', { exact: true })).toBeVisible();
	expect(homeroomRequestCount()).toBe(2);
	expect(changeSetSummaryRequestCount()).toBe(1);
	expect(changeSetDetailRequestCount()).toBe(1);
});

const secondOffering = '80000000-0000-4000-8000-000000000002';
async function mockDraftLifecycle(
	page: Page,
	options: { failPreview?: boolean; failDelete?: boolean; readOnly?: boolean } = {}
) {
	await mockDelivery(page, undefined, undefined, { deliveryVersionStatus: 'draft' });
	let previewCalls = 0,
		deleted = false;
	let publication: Record<string, unknown> | null = null;
	const detail = {
		...changeSetDetail(),
		offeringLabels: [
			{ id: ids.offering, code: 'ค21101', name: 'คณิตศาสตร์พื้นฐาน' },
			{ id: secondOffering, code: 'ว21101', name: 'วิทยาศาสตร์' }
		],
		changes: [
			{
				kind: 'changed',
				learningOfferingId: ids.offering,
				resourceId: ids.offering,
				label: 'ค21101 — คณิตศาสตร์พื้นฐาน',
				field: 'รายการเปิดสอน',
				before: 'เปิดสอน 3 คาบ/สัปดาห์',
				after: 'เปิดสอน 4 คาบ/สัปดาห์'
			}
		],
		items: [
			{
				actionKind: 'add_offering',
				id: '84000000-0000-4000-8000-000000000001',
				learningOfferingId: ids.offering,
				weeklyPeriodTarget: 4,
				rowVersion: 1
			},
			{
				actionKind: 'add_offering',
				id: '84000000-0000-4000-8000-000000000002',
				learningOfferingId: secondOffering,
				weeklyPeriodTarget: 3,
				rowVersion: 1
			}
		]
	};
	await page.route('**/api/academic/term-change-sets/**', async (route) => {
		const url = new URL(route.request().url());
		if (url.pathname.endsWith('/preview')) {
			previewCalls++;
			if (options.failPreview && previewCalls === 1)
				return void (await fulfill(route, 'ตรวจไม่สำเร็จ กรุณาลองใหม่', 503));
			const date = url.searchParams.get('effectiveFrom');
			return void (await fulfill(route, {
				preliminary: !date,
				changes: detail.changes,
				changeSetId: ids.changeSet,
				changeSetRowVersion: 1,
				targetDeliveryVersionId: ids.version,
				targetDeliveryVersionRowVersion: 2,
				effectiveFrom: date ?? '2027-08-01',
				previewHash: 'a'.repeat(64),
				impacts: {},
				findings: date
					? []
					: [ids.offering, secondOffering].map((offering, index) => ({
							code: 'missing_delivery_group',
							severity: 'blocking',
							title: `${index === 0 ? 'ค21101 — คณิตศาสตร์พื้นฐาน' : 'ว21101 — วิทยาศาสตร์'}: ยังไม่มีกลุ่มเรียน`,
							guidance: 'สร้างกลุ่มเรียนและเลือกครู',
							affectedCount: 1,
							learningOfferingId: offering,
							learningGroupId: null,
							resourceId: ids.version,
							route: `/staff/academic/delivery/${offering}?deliveryVersionId=${ids.version}`
						}))
			}));
		}
		if (url.pathname.endsWith('/publish')) {
			publication = route.request().postDataJSON();
			return void (await fulfill(route, {
				...detail,
				status: 'published',
				effectiveFrom: publication?.effectiveFrom,
				rowVersion: 2
			}));
		}
		if (deleted) return void (await fulfill(route, 'รุ่นนี้ถูกลบแล้ว', 404));
		return void (await fulfill(route, detail));
	});
	await page.route(`**/api/academic/delivery-versions/${ids.version}`, async (route) => {
		if (route.request().method() === 'DELETE') {
			expect(route.request().postDataJSON()).toEqual({
				rowVersion: 2,
				changeSetRowVersion: 1,
				expectedItemCount: 2,
				expectedOfferingCount: 2,
				expectedGroupCount: 1
			});
			if (options.failDelete)
				return void (await fulfill(route, 'ลบไม่ได้: มีประวัติข้อมูลอื่นอ้างอิง', 409));
			deleted = true;
			return void (await fulfill(route, {
				id: ids.version,
				changeSetId: ids.changeSet,
				sourceVersionId: null,
				deletedItemCount: 2,
				deletedOfferingCount: 2,
				deletedGroupCount: 1
			}));
		}
		return void (await fulfill(route, {
			id: ids.version,
			rowVersion: 2,
			status: 'draft',
			effectiveFrom: null,
			referenceDate: '2027-08-01',
			snapshot: {
				offerings: [
					{ id: ids.offering, groups: [{ id: ids.group }] },
					{ id: secondOffering, groups: [] }
				]
			}
		}));
	});
	await page.route('**/api/academic/term-change-sets?*', (route) =>
		fulfill(route, deleted ? [] : [changeSetSummary()])
	);
	await page.route('**/api/academic/delivery-versions?*', (route) =>
		fulfill(
			route,
			deleted
				? []
				: [
						{
							...changeSetSummary(),
							id: ids.version,
							changeSetId: ids.changeSet,
							rowVersion: 2,
							sourceVersionId: null,
							effectiveUntil: null,
							offeringCount: 2,
							groupCount: 1,
							teacherAssignmentCount: 0
						}
					]
		)
	);
	if (options.readOnly)
		await page.route('**/api/auth/me', (route) =>
			fulfill(route, {
				id: '90000000-0000-4000-8000-000000000001',
				username: 'reader',
				firstName: 'อ่าน',
				lastName: 'อย่างเดียว',
				userType: 'staff',
				status: 'ACTIVE',
				permissions: ['learning_offering.read.school']
			})
		);
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}&changeSetId=${ids.changeSet}`
	);
	await expect(page.getByText('ปรับการเปิดสอนทดสอบ', { exact: true })).toBeVisible();
	return { published: () => publication, deleted: () => deleted };
}
for (const viewport of [
	{ name: 'desktop-light', width: 1440, height: 950, dark: false },
	{ name: 'mobile-dark', width: 390, height: 844, dark: true },
	{ name: 'desktop-dark', width: 1440, height: 950, dark: true },
	{ name: 'mobile-light', width: 390, height: 844, dark: false }
]) {
	test(`readiness renders repeated findings and named differences: ${viewport.name}`, async ({
		page
	}) => {
		const errors: string[] = [];
		page.on('pageerror', (e) => errors.push(e.message));
		await page.setViewportSize(viewport);
		await page.addInitScript((dark) => {
			localStorage.setItem('ui-preferences', JSON.stringify({ theme: dark ? 'dark' : 'light' }));
		}, viewport.dark);
		await mockDraftLifecycle(page, { failPreview: true });
		await expect(page.locator('html')).toHaveClass(viewport.dark ? /dark/ : /^(?!.*dark).*$/);
		await expect(
			page.getByText('รายการเปิดสอน: เปิดสอน 3 คาบ/สัปดาห์ → เปิดสอน 4 คาบ/สัปดาห์', {
				exact: true
			})
		).toBeVisible();

		await expect(page.getByText(`${ids.offering} · 4 คาบ/สัปดาห์`)).toHaveCount(0);
		await page.getByRole('button', { name: 'ตรวจความพร้อม', exact: true }).click();
		await expect(page.getByRole('alert')).toContainText('ตรวจไม่สำเร็จ');
		await page.getByRole('button', { name: 'ตรวจความพร้อม', exact: true }).click();
		await expect(page.getByText('ค21101 — คณิตศาสตร์พื้นฐาน: ยังไม่มีกลุ่มเรียน')).toBeVisible();
		await expect(page.getByText('ว21101 — วิทยาศาสตร์: ยังไม่มีกลุ่มเรียน')).toBeVisible();
		await expect(page.getByRole('link', { name: 'ไปแก้ไข' })).toHaveCount(2);
		expect(errors).toEqual([]);
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
		await page.screenshot({
			path: `test-results/delivery-readiness-${viewport.name}.png`,
			fullPage: true
		});
	});
}
test('publication selects a date and resets verification after changing it', async ({ page }) => {
	const result = await mockDraftLifecycle(page);
	await page.getByRole('button', { name: 'เผยแพร่รุ่นเปิดสอน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('button', { name: 'ยืนยันเผยแพร่' })).toBeDisabled();
	await dialog.getByRole('button', { name: 'ตรวจความพร้อมตามวันที่เลือก' }).click();
	await expect(dialog.getByRole('button', { name: 'ยืนยันเผยแพร่' })).toBeEnabled();
	await dialog.getByRole('button', { name: 'วันที่เริ่มใช้รุ่นเปิดสอน' }).click();
	await page.getByRole('button', { name: 'วันอังคารที่ 3 สิงหาคม 2570', exact: true }).click();
	await expect(dialog.getByRole('button', { name: 'ยืนยันเผยแพร่' })).toBeDisabled();
	await dialog.getByRole('button', { name: 'ตรวจความพร้อมตามวันที่เลือก' }).click();
	await dialog.getByRole('button', { name: 'ยืนยันเผยแพร่' }).click();
	await expect(page.getByText(/เผยแพร่แล้ว เริ่มใช้/)).toBeVisible();
	expect(result.published()?.effectiveFrom).toBe('2027-08-03');
});
test('draft deletion reports references and preserves the draft', async ({ page }) => {
	const result = await mockDraftLifecycle(page, { failDelete: true });
	await page.getByRole('button', { name: 'ลบแบบร่าง', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toContainText('2 รายการเปลี่ยนแปลง · 2 รายการเปิดสอน · 1 กลุ่ม');
	await dialog.getByRole('button', { name: 'ยืนยันลบถาวร' }).click();
	await expect(dialog.getByRole('alert')).toContainText('มีประวัติข้อมูลอื่นอ้างอิง');
	expect(result.deleted()).toBe(false);
	await dialog.getByRole('button', { name: 'กลับ', exact: true }).click();
	await expect(page.getByText('ปรับการเปิดสอนทดสอบ', { exact: true })).toBeVisible();
});
test('read-only staff can check readiness but cannot publish or delete', async ({ page }) => {
	await mockDraftLifecycle(page, { readOnly: true });
	await expect(page.getByRole('button', { name: 'ตรวจความพร้อม', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'เผยแพร่รุ่นเปิดสอน', exact: true })).toHaveCount(
		0
	);
	await expect(page.getByRole('button', { name: 'ลบแบบร่าง', exact: true })).toHaveCount(0);
});

test('draft deletion removes the version and returns to the viewing workspace', async ({
	page
}) => {
	const result = await mockDraftLifecycle(page);
	await page.getByRole('button', { name: 'ลบแบบร่าง', exact: true }).click();
	const dialog = page.getByRole('dialog');
	const confirm = dialog.getByRole('button', { name: 'ยืนยันลบถาวร' });
	await expect(confirm).toBeEnabled();
	await confirm.focus();
	await page.keyboard.press('Enter');
	await expect(dialog).toHaveCount(0);
	expect(result.deleted()).toBe(true);
	await expect(page).not.toHaveURL(/changeSetId=/);
	await expect(page.getByRole('button', { name: 'ลบแบบร่าง', exact: true })).toHaveCount(0);
});

const activitySource = '82000000-0000-4000-8000-000000000090';
async function mockActivityActivation(
	page: Page,
	draftCount: number,
	failLookup = false,
	options: { historical?: boolean; already?: boolean } = {}
) {
	await mockDelivery(page);
	let lookupCalls = 0,
		created = false;
	let applied: Record<string, unknown> | null = null;
	const summary = (id: string, status: string) => ({
		id,
		status,
		academicTermId: ids.term,
		academicYearId: ids.year,
		sourceVersionId: status === 'draft' ? activitySource : null,
		changeSetId: status === 'draft' ? ids.changeSet : null,
		effectiveFrom: status === 'published' ? '2027-05-01' : null,
		effectiveUntil: null,
		referenceDate: '2027-08-01',
		rowVersion: 1,
		offeringCount: 1,
		groupCount: 0,
		teacherAssignmentCount: 0,
		updatedAt: '2027-07-01T00:00:00Z'
	});
	await page.route('**/api/academic/delivery-versions?**', async (route) => {
		if (++lookupCalls === 2 && failLookup)
			return void (await fulfill(route, 'เครือข่ายขัดข้อง', 503));
		await fulfill(route, [
			summary(activitySource, 'published'),
			...(options.historical
				? [
						{
							...summary('82000000-0000-4000-8000-000000000089', 'published'),
							effectiveFrom: '2027-04-01'
						}
					]
				: []),
			...(draftCount > 0 || created ? [summary(ids.version, 'draft')] : []),
			...(draftCount > 1 ? [summary('82000000-0000-4000-8000-000000000091', 'draft')] : [])
		]);
	});
	await page.route('**/api/academic/delivery/homerooms?**', async (route) => {
		const version =
			new URL(route.request().url()).searchParams.get('deliveryVersionId') ?? activitySource;
		const workspace = homeroomWorkspace(
			version,
			version === ids.version || version === '82000000-0000-4000-8000-000000000091'
				? 'draft'
				: 'published'
		);
		Object.assign(workspace.homerooms[0].items[0], {
			resourceKind: 'activity',
			code: 'CLUB',
			name: 'ชุมนุม',
			schedulingMode: 'synchronized',
			offeringId: null,
			offeringState: 'missing',
			groupMode: 'missing',
			teacherState: 'missing_primary',
			groups: [],
			alignmentStates: ['curriculum_requirement_not_offered']
		});
		if (options.already && version === ids.version)
			Object.assign(workspace.homerooms[0].items[0], {
				offeringId: ids.offering,
				offeringState: 'draft',
				groupMode: 'central',
				teacherState: 'deferred'
			});
		await fulfill(route, workspace);
	});
	await page.route('**/api/academic/delivery/management-options?**', (route) =>
		fulfill(route, {
			academicTermId: ids.term,
			academicYearId: ids.year,
			studyPrograms: [
				{
					id: ids.program,
					name: 'แผนมาตรฐาน',
					code: 'DEFAULT',
					curriculumName: 'หลักสูตร',
					curriculumId: ids.curriculum
				}
			],
			gradeLevels: [{ id: ids.grade, name: 'ม.1', code: 'M1' }],
			homerooms: [{ id: ids.homeroom, name: 'ม.1/1', gradeLevelId: ids.grade }],
			catalogVersions: [
				{
					id: ids.catalog,
					kind: 'activity',
					label: 'CLUB · ชุมนุม',
					schedulingMode: 'synchronized'
				}
			],
			learningGroups: [],
			teachers: [],
			rooms: []
		})
	);
	await page.route('**/api/academic/term-change-sets', async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		created = true;
		expect(route.request().postDataJSON().effectiveFrom).toBeUndefined();
		await fulfill(route, { ...changeSetDetail(), targetDeliveryVersionId: ids.version }, 201);
	});
	await page.route('**/api/academic/offerings/preview-from-curriculum', (route) =>
		fulfill(route, {
			sourceHash: 'a'.repeat(64),
			proposals: [
				{
					proposalId: 'central-club',
					resourceKind: 'activity',
					catalogVersionId: ids.catalog,
					schedulingMode: 'synchronized',
					code: 'CLUB',
					name: 'ชุมนุม',
					targetHomeroomIds: [ids.homeroom],
					requirementIds: [ids.requirement],
					requirementKind: 'required',
					groupingState: 'proposed',
					offeringAction: 'create',
					conflicts: [],
					defaultGroups: [{ groupKey: 'a'.repeat(64), name: 'ม.1/1', homeroomIds: [ids.homeroom] }]
				}
			]
		})
	);
	await page.route('**/api/academic/offerings/apply-from-curriculum', async (route) => {
		applied = route.request().postDataJSON();
		await fulfill(route, {
			createdOfferings: [],
			createdGroups: [],
			reusedOfferings: [],
			reusedGroups: []
		});
	});
	return { applied: () => applied, created: () => created };
}
for (const count of [0, 1, 2]) {
	test(`published synchronized activation supports ${count} eligible drafts and central import`, async ({
		page
	}) => {
		const result = await mockActivityActivation(page, count);
		await page.goto(
			`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${activitySource}`
		);

		await expect(page.getByRole('columnheader')).toHaveCount(6);
		await page.getByRole('button', { name: 'เพิ่มในร่าง', exact: true }).click();
		if (count === 0) {
			await expect(page.getByRole('dialog')).toContainText('ชื่อหรือเหตุผลของรุ่น');
			expect(result.created()).toBe(false);
			await page.getByLabel('ชื่อหรือเหตุผลของรุ่น').fill('เพิ่มชุมนุม');
			await page.getByRole('button', { name: 'สร้างแบบร่าง', exact: true }).click();
		}
		if (count === 2) {
			await expect(page.getByRole('dialog')).toContainText('เลือกร่างที่จะเพิ่มกิจกรรม');
			await page
				.getByRole('dialog')
				.getByRole('button', { name: /1 รายการ/ })
				.first()
				.click();
		}
		await expect(page.getByRole('dialog', { name: /เพิ่มกิจกรรม CLUB/ })).toBeVisible();
		await expect(page.getByRole('dialog', { name: 'สร้างรุ่นเปิดสอน', exact: true })).toHaveCount(
			0
		);
		await page.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน' }).click();
		await expect(page.getByRole('dialog')).toContainText('เปิดแบบกลาง');
		await page.getByRole('button', { name: 'เปิดใช้งานกิจกรรม', exact: true }).click();
		await expect(page.getByRole('dialog')).toHaveCount(0);
		expect(result.applied()).toMatchObject({
			deliveryVersionId: ids.version,
			choices: [{ proposalId: 'central-club', action: 'defer_groups', groups: [] }]
		});
	});
}
test('a failed published activation lookup preserves intent and retries', async ({ page }) => {
	await mockActivityActivation(page, 1, true);
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${activitySource}`
	);
	await page.getByRole('button', { name: 'เพิ่มในร่าง', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('เครือข่ายขัดข้อง');
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(1);
	await expect(page.getByRole('dialog')).toContainText('เพิ่มกิจกรรม CLUB');
});

for (const grouped of [false, true]) {
	test(`manual synchronized activation defaults central and supports reviewed groups (${grouped})`, async ({
		page
	}) => {
		await mockActivityActivation(page, 1);
		let body: Record<string, unknown> | null = null;
		await page.route('**/api/academic/offerings?**', async (route) => {
			if (route.request().method() !== 'POST') return route.fallback();
			body = route.request().postDataJSON();
			await fulfill(
				route,
				{
					id: ids.offering,
					academicTermId: ids.term,
					academicYearId: ids.year,
					kind: 'activity',
					name: 'ชุมนุม',
					code: 'CLUB',
					status: 'draft',
					rowVersion: 1,
					targets: [],
					snapshot: { activityVersionId: ids.catalog, schedulingMode: 'synchronized' },
					owningOrganizationUnitId: ids.program
				},
				201
			);
		});
		await page.goto(
			`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}`
		);
		await page.getByRole('button', { name: 'เปิดการเรียนการสอน', exact: true }).click();
		await page.getByRole('button', { name: /เพิ่มรายการเอง/ }).click();
		await page.getByRole('dialog').getByRole('button', { name: 'รายวิชา', exact: true }).click();
		await page.getByRole('option', { name: 'กิจกรรมพัฒนาผู้เรียน', exact: true }).click();
		for (const [label, option] of [
			['เลือกกิจกรรม', 'CLUB · ชุมนุม'],
			['เลือกระดับชั้น', 'ม.1'],
			['เลือกแผนการเรียน', 'แผนมาตรฐาน']
		]) {
			await page.getByRole('combobox', { name: label, exact: true }).click();
			await page.getByRole('option', { name: new RegExp(option) }).click();
		}
		await expect(page.getByLabel('รูปแบบการเปิด')).toContainText('เปิดแบบกลาง');
		if (grouped) {
			await page.getByLabel('รูปแบบการเปิด').click();
			await page.getByRole('option', { name: 'เปิดพร้อมจัดกลุ่ม', exact: true }).click();
		}
		await page.getByRole('button', { name: 'สร้างฉบับร่าง', exact: true }).click();
		await expect
			.poll(() => body)
			.toMatchObject({
				kind: 'activity',
				schedulingMode: 'synchronized',
				createHomeroomGroups: grouped,
				targets: [
					{ targetKind: 'grade_program', gradeLevelId: ids.grade, studyProgramId: ids.program }
				]
			});
		if (grouped) await expect(page).toHaveURL(new RegExp(`/delivery/${ids.offering}`));
		else await expect(page.getByRole('dialog')).toHaveCount(0);
	});
}

for (const size of ['mobile', 'desktop'] as const)
	for (const theme of ['light', 'dark'] as const) {
		test(`central activation ${size} ${theme} has readable controls and keyboard access`, async ({
			page
		}) => {
			await page.setViewportSize(
				size === 'mobile' ? { width: 390, height: 844 } : { width: 1440, height: 1000 }
			);
			await mockActivityActivation(page, 1);
			await page.goto(
				`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${activitySource}`
			);
			await page.evaluate(
				(mode) => document.documentElement.classList.toggle('dark', mode === 'dark'),
				theme
			);
			const add = page.getByRole('button', { name: 'เพิ่มในร่าง', exact: true });
			await add.focus();
			await page.keyboard.press('Enter');
			await expect(page.getByRole('dialog')).toHaveCount(1);
			await expect(page.getByRole('dialog')).toContainText('เพิ่มกิจกรรม CLUB');
			await page.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน' }).click();
			await expect(page.getByRole('dialog')).toContainText('เปิดแบบกลาง');
			await page.screenshot({ path: `test-results/central-${size}-${theme}.png` });
			await page.keyboard.press('Escape');
			await expect(page.getByRole('dialog')).toHaveCount(0);
		});
	}

test('historical activation explains that its new draft uses the latest published source', async ({
	page
}) => {
	await mockActivityActivation(page, 0, false, { historical: true });
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=82000000-0000-4000-8000-000000000089`
	);
	await page.getByRole('button', { name: 'เพิ่มในร่าง', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText('กำลังดูรุ่นย้อนหลัง');
	await expect(page.getByRole('dialog')).toContainText('รุ่นเผยแพร่ล่าสุด');
});

test('already included activity opens its existing draft detail without preparing a duplicate', async ({
	page
}) => {
	await mockActivityActivation(page, 1, false, { already: true });
	let preparations = 0;
	page.on('request', (request) => {
		if (request.url().includes('from-curriculum')) preparations++;
	});
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${activitySource}`
	);
	await page.getByRole('button', { name: 'เพิ่มในร่าง', exact: true }).click();
	await expect(page).toHaveURL(
		new RegExp(`/delivery/${ids.offering}.*deliveryVersionId=${ids.version}`)
	);
	expect(preparations).toBe(0);
});

test('read-only activity viewer has no creation actions or lazy management requests', async ({
	page
}) => {
	await mockActivityActivation(page, 0);
	await page.route('**/api/auth/me', (route) =>
		fulfill(route, {
			id: '90000000-0000-4000-8000-000000000001',
			username: 'reader',
			firstName: 'อ่าน',
			lastName: 'อย่างเดียว',
			userType: 'staff',
			status: 'ACTIVE',
			permissions: ['learning_offering.read.school']
		})
	);
	let managementReads = 0;
	page.on('request', (request) => {
		if (request.url().includes('/delivery/management-options')) managementReads++;
	});
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${activitySource}`
	);
	await expect(page.getByText('ต้องมีสิทธิ์จัดการ', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่มในร่าง', exact: true })).toHaveCount(0);
	expect(managementReads).toBe(0);
});

test('curriculum synchronized activation can explicitly create its reviewed homeroom groups', async ({
	page
}) => {
	const result = await mockActivityActivation(page, 1);
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${activitySource}`
	);
	await page.getByRole('button', { name: 'เพิ่มในร่าง', exact: true }).click();
	await expect(page.getByRole('dialog', { name: /เพิ่มกิจกรรม CLUB/ })).toBeVisible();
	await page.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน' }).click();
	await page.getByRole('button', { name: 'วิธีเตรียม ชุมนุม', exact: true }).click();
	await page.getByRole('option', { name: 'เปิดสอนและจัดกลุ่ม', exact: true }).click();
	await page.getByRole('button', { name: 'เปิดใช้งานกิจกรรม', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	expect(result.applied()).toMatchObject({
		choices: [{ action: 'apply', groups: [{ name: 'ม.1/1', homeroomIds: [ids.homeroom] }] }]
	});
});

for (const viewport of [
	{ width: 1440, height: 900 },
	{ width: 390, height: 844 }
]) {
	for (const dark of [false, true]) {
		test(`zero course period dialog preserves curriculum and retries failed saves at ${viewport.width}px ${dark ? 'dark' : 'light'}`, async ({
			page
		}) => {
			await page.setViewportSize(viewport);
			await mockDelivery(page);
			const detail = {
				...changeSetDetail(),
				items: [],
				changes: Array.from({ length: 40 }, (_, index) => ({
					kind: 'changed',
					learningOfferingId: ids.offering,
					resourceId: ids.offering,
					label: `รายการที่ ${index + 1}`,
					field: 'จำนวนคาบต่อสัปดาห์',
					before: '4',
					after: '0'
				}))
			};
			const offering = {
				id: ids.offering,
				kind: 'course',
				code: 'ค21101',
				name: 'คณิตศาสตร์พื้นฐาน',
				weeklyPeriodTarget: 4,
				groups: [],
				targets: [],
				homeroomIds: [],
				catalog: { kind: 'course', standardPeriodsPerWeek: 4, credit: '2.0' }
			};
			await page.route(`**/api/academic/term-change-sets/${ids.changeSet}`, (route) =>
				fulfill(route, detail)
			);
			await page.route(`**/api/academic/delivery-versions/${ids.version}`, (route) =>
				fulfill(route, {
					id: ids.version,
					academicTermId: ids.term,
					sourceVersionId: null,
					status: 'draft',
					snapshot: { offerings: [offering] }
				})
			);
			await page.route('**/api/academic/delivery/management-options?*', (route) =>
				fulfill(route, {
					catalogVersions: [],
					gradeLevels: [],
					studyPrograms: [],
					teachers: []
				})
			);
			let submitted: Record<string, unknown> | null = null;
			let saveAttempts = 0;
			let releaseSave!: () => void;
			const saveGate = new Promise<void>((resolve) => (releaseSave = resolve));
			await page.route(`**/api/academic/term-change-sets/${ids.changeSet}/items`, async (route) => {
				submitted = route.request().postDataJSON();
				saveAttempts++;
				if (saveAttempts === 1) return fulfill(route, 'บันทึกไม่สำเร็จ ลองอีกครั้ง', 400);
				await saveGate;
				await fulfill(route, { ...detail, rowVersion: 2 });
			});
			await page.goto(
				`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}`
			);
			if (dark) {
				await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
				await expect(page.locator('html')).toHaveClass(/dark/);
			}
			const trigger = page.getByRole('button', { name: 'เพิ่ม/ปรับรายการสอน', exact: true });
			await trigger.scrollIntoViewIfNeeded();
			const initialScroll = await page.evaluate(() => window.scrollY);
			await trigger.focus();
			await page.keyboard.press('Enter');
			const dialog = page.getByRole('dialog', { name: 'เพิ่ม/ปรับรายการสอน', exact: true });
			await expect(dialog).toBeVisible();
			await expect(dialog.getByRole('button', { name: 'บันทึกรายการ', exact: true })).toBeVisible();
			await page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true }).click();
			await page.getByRole('option', { name: 'ปรับคาบต่อสัปดาห์', exact: true }).click();
			await page.getByRole('combobox').filter({ hasText: 'เลือกรายวิชาหรือกิจกรรม' }).click();
			await page.getByRole('option', { name: /ค21101/ }).click();
			const periods = page.getByLabel('คาบที่จัดจริงต่อสัปดาห์');
			await expect(periods).toHaveValue('4');
			await expect(periods).toHaveAttribute('min', '0');
			await expect(page.getByText('4 คาบ/สัปดาห์', { exact: true })).toBeVisible();
			await periods.fill('-1');
			await expect(page.getByRole('button', { name: 'บันทึกรายการ', exact: true })).toBeDisabled();
			await periods.fill('0.5');
			await expect(page.getByRole('button', { name: 'บันทึกรายการ', exact: true })).toBeDisabled();
			await periods.fill('0');
			await expect(page.getByText(/0 คาบ = เปิดรายวิชา/)).toBeVisible();
			await expect(page.getByRole('button', { name: 'บันทึกรายการ', exact: true })).toBeEnabled();
			await page.screenshot({
				path: `/tmp/zero-periods-${viewport.width}-${dark ? 'dark' : 'light'}.png`
			});
			await page.getByRole('button', { name: 'บันทึกรายการ', exact: true }).click();
			await expect
				.poll(() => submitted)
				.toMatchObject({
					action: 'adjust_weekly_period_target',
					learningOfferingId: ids.offering,
					weeklyPeriodTarget: 0
				});
			await expect(dialog.getByRole('alert')).toHaveText('บันทึกไม่สำเร็จ ลองอีกครั้ง');
			await expect(periods).toHaveValue('0');
			await page.screenshot({
				path: `/tmp/delivery-dialog-error-${viewport.width}-${dark ? 'dark' : 'light'}.png`
			});
			await dialog.getByRole('button', { name: 'บันทึกรายการ', exact: true }).click();
			await expect.poll(() => saveAttempts).toBe(2);
			await expect(periods).toBeDisabled();
			await expect(dialog.getByRole('button', { name: 'ยกเลิก', exact: true })).toBeDisabled();
			await page.screenshot({
				path: `/tmp/delivery-dialog-saving-${viewport.width}-${dark ? 'dark' : 'light'}.png`
			});
			await page.keyboard.press('Escape');
			await expect(dialog).toBeVisible();
			await page.mouse.click(5, 5);
			await expect(dialog).toBeVisible();
			releaseSave();
			await expect(dialog).toHaveCount(0);
			await expect(trigger).toBeFocused();
			await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(initialScroll);
			await trigger.click();
			await expect(dialog).toBeVisible();
			await expect(dialog.getByRole('alert')).toHaveCount(0);
			await expect(dialog.getByRole('combobox', { name: 'เลือกรายวิชาหรือกิจกรรม' })).toHaveText(
				'เลือกรายวิชาหรือกิจกรรม'
			);
			await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
			await expect(dialog).toHaveCount(0);
			await expect(trigger).toBeFocused();
		});
	}
}

for (const viewport of [
	{ width: 1440, height: 900 },
	{ width: 390, height: 600 }
]) {
	for (const dark of [false, true]) {
		test(`delivery picker sorts course codes and hides activity codes at ${viewport.width}px ${dark ? 'dark' : 'light'}`, async ({
			page
		}) => {
			await page.setViewportSize(viewport);
			await mockDelivery(page);
			const offerings = [
				{ code: 'ส33201', name: 'หน้าที่พลเมือง', kind: 'course' },
				{ code: 'OTHER-cf1520d73a57', name: 'รักการอ่าน', kind: 'activity' },
				{ code: 'ส23201', name: 'หน้าที่พลเมือง', kind: 'course' },
				{ code: 'ส21201', name: 'หน้าที่พลเมือง', kind: 'course' },
				{ code: 'CLUB-cac2ff31b415', name: 'ชุมนุม', kind: 'activity' },
				{ code: 'ส32201', name: 'หน้าที่พลเมือง', kind: 'course' }
			].map((item, index) => ({
				...item,
				id: `80000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
				weeklyPeriodTarget: 1,
				groups: [],
				targets: [],
				homeroomIds: [],
				catalog: { kind: item.kind, standardPeriodsPerWeek: 1, credit: '1.0' }
			}));
			await page.route(`**/api/academic/term-change-sets/${ids.changeSet}`, (route) =>
				fulfill(route, changeSetDetail())
			);
			await page.route(`**/api/academic/delivery-versions/${ids.version}`, (route) =>
				fulfill(route, {
					id: ids.version,
					academicTermId: ids.term,
					sourceVersionId: null,
					status: 'draft',
					snapshot: { offerings }
				})
			);
			await page.route('**/api/academic/delivery/management-options?*', (route) =>
				fulfill(route, {
					catalogVersions: offerings.map((item) => ({
						id: item.id,
						kind: item.kind,
						code: item.code,
						name: item.name,
						label: `${item.code} — ${item.name} (ฉบับ 1)`,
						versionNo: 1,
						standardPeriodsPerWeek: item.kind === 'course' ? 1 : null,
						schedulingMode: item.kind === 'activity' ? 'independent' : null
					})),
					gradeLevels: [],
					studyPrograms: [],
					teachers: []
				})
			);
			await page.goto(
				`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}`
			);
			if (dark) {
				await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
				await expect(page.locator('html')).toHaveClass(/dark/);
			}
			await page.getByRole('button', { name: 'เพิ่ม/ปรับรายการสอน', exact: true }).click();
			await page.getByRole('combobox', { name: 'เลือกรายวิชา', exact: true }).click();
			const courses = [/ส21201/, /ส23201/, /ส32201/, /ส33201/];
			await expect(page.getByRole('option')).toHaveText(courses);
			await page.getByRole('option', { name: /ส21201/ }).click();
			await expect(page.locator('[data-slot="popover-content"]')).toHaveCount(0);
			const formBody = page.getByRole('dialog').locator('fieldset').locator('..');
			if (viewport.height === 600) {
				await expect
					.poll(() => formBody.evaluate((element) => element.scrollHeight > element.clientHeight))
					.toBe(true);
				await formBody.evaluate((element) => (element.scrollTop = element.scrollHeight));
				await expect
					.poll(async () => {
						const bodyBounds = await formBody.boundingBox();
						const footerBounds = await page
							.getByRole('dialog')
							.locator('[data-slot="dialog-footer"]')
							.boundingBox();
						return Boolean(
							bodyBounds && footerBounds && bodyBounds.y + bodyBounds.height <= footerBounds.y
						);
					})
					.toBe(true);
			}
			await expect(
				page.getByRole('button', { name: 'บันทึกรายการ', exact: true })
			).toBeInViewport();
			await page.screenshot({
				path: `/tmp/delivery-dialog-add-${viewport.width}-${dark ? 'dark' : 'light'}.png`
			});
			await formBody.evaluate((element) => (element.scrollTop = 0));
			await page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true }).click();
			await page.getByRole('option', { name: 'ปรับคาบต่อสัปดาห์', exact: true }).click();
			const picker = page.getByRole('combobox', { name: 'เลือกรายวิชาหรือกิจกรรม', exact: true });
			await picker.click();
			await expect(page.getByRole('option')).toHaveText([
				...courses,
				/^\s*ชุมนุม\s+กิจกรรมพัฒนาผู้เรียน\s*$/,
				/^\s*รักการอ่าน\s+กิจกรรมพัฒนาผู้เรียน\s*$/
			]);
			const search = page.getByPlaceholder('ค้นหารหัสหรือชื่อ...');
			await search.fill('หน้าที่');
			await expect(page.getByRole('option')).toHaveText(courses);
			await search.fill('กิจกรรม');
			await expect(page.getByRole('option')).toHaveText([
				/^\s*ชุมนุม\s+กิจกรรมพัฒนาผู้เรียน\s*$/,
				/^\s*รักการอ่าน\s+กิจกรรมพัฒนาผู้เรียน\s*$/
			]);
			await page.screenshot({
				path: `/tmp/delivery-picker-${viewport.width}-${dark ? 'dark' : 'light'}.png`
			});
			await search.fill('รักการอ่าน');
			await page.getByRole('option', { name: /รักการอ่าน/ }).click();
			await expect(picker).toHaveText('รักการอ่าน');
			await expect(page.getByLabel('คาบที่จัดจริงต่อสัปดาห์')).toHaveAttribute('min', '1');
			await page.getByRole('button', { name: 'ปรับคาบต่อสัปดาห์', exact: true }).click();
			await page.getByRole('option', { name: 'เพิ่มกิจกรรมพัฒนาผู้เรียน', exact: true }).click();
			const activityPicker = page.getByRole('combobox', { name: 'เลือกกิจกรรม', exact: true });
			await activityPicker.click();
			await expect(page.getByRole('option')).toHaveText([
				/^\s*ชุมนุม\s+กิจกรรมพัฒนาผู้เรียน · ฉบับ 1\s*$/,
				/^\s*รักการอ่าน\s+กิจกรรมพัฒนาผู้เรียน · ฉบับ 1\s*$/
			]);
			await page.getByRole('option', { name: /ชุมนุม/ }).click();
			await expect(activityPicker).toHaveText('ชุมนุม');
		});
	}
}
