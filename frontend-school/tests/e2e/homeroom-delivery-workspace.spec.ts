import { offeringStartsOn, selectOfferingStartDate } from './fixtures/offering-start-date';
import { sessionState } from './fixtures/session-state';
import { expect, test, type Page, type Route } from '@playwright/test';
import type {
	DeliveryManagementOptions,
	DeliveryVersion
} from '../../src/lib/api/learning-delivery.js';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

async function expandChangeDetails(page: Page) {
	const dialog = page.getByRole('dialog', { name: 'รายละเอียดการเปลี่ยนแปลง', exact: true });
	if (!(await dialog.isVisible())) {
		await page.getByRole('button', { name: 'ดูรายละเอียดการเปลี่ยนแปลง', exact: true }).click();
	}
	await expect(dialog).toBeVisible();
	const offerings = dialog.getByTestId('delivery-change-offering');
	await expect(offerings.first()).toBeVisible();
	for (const offering of await offerings.all()) {
		const button = offering.getByRole('button', { name: /ดูรายละเอียด/ }).first();
		if ((await button.getAttribute('aria-expanded')) === 'false') await button.click();
		await expect(button).toHaveAttribute('aria-expanded', 'true');
	}
}

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
					editionId: ids.curriculum,
					curriculumLevelId: ids.curriculum,
					editionName: 'ฉบับปรับปรุง พุทธศักราช 2570',
					revisionYear: 2570,
					gradeLevelIds: [ids.grade],
					levelName: 'ระดับมัธยมศึกษาตอนต้น'
				},
				curriculumLevelId: ids.curriculum,
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
						subjectType: 'BASIC',
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
		publicationNo: null,
		publishedAt: null,
		publishedBy: null,
		targetDeliveryVersionId: ids.version,
		updatedAt: '2027-07-01T00:00:00Z'
	};
}

function changeSetDetail() {
	return {
		baseTeacherAssignments: [],
		...changeSetSummary(),
		publicationDateRules: { today: '2027-08-01', minimumEffectiveFrom: '2027-08-01' },
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
					displayName: 'วิชาการ ทดสอบ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
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
					defaultAcademicYearId: ids.year,
					defaultAcademicTermId: ids.term,
					years: [
						{
							isVisible: true,
							id: ids.year,
							name: 'ปีการศึกษา 2570',
							year: 2570,
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
							status: 'active',
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
						publicationNo: options.deliveryVersionStatus === 'published' ? 1 : null,
						isSuperseded: false,
						publishedAt:
							options.deliveryVersionStatus === 'published' ? '2027-07-01T00:00:00Z' : null,
						publishedBy: null,
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
					courseMetadata: [],
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

test('browser reload clears homerooms before loading the current route again', async ({ page }) => {
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
		await page.reload();

		await expect(homeroomRegion).toHaveAttribute('aria-busy', 'true');
		await expect(page.getByText('ม.1/1', { exact: true })).toHaveCount(0);
		await expect(page.getByTestId('delivery-homerooms-ready')).toHaveCount(0);
		await expect(
			homeroomRegion.getByRole('status', { name: 'กำลังอัปเดตภาพรวมรายห้อง' })
		).toHaveCount(0);
		await expect(homeroomRegion.locator('[data-slot="skeleton"]')).not.toHaveCount(0);
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
	options: {
		failPreview?: boolean;
		failDelete?: boolean;
		readOnly?: boolean;
		today?: string;
		referenceDate?: string;
	} = {}
) {
	await mockDelivery(page, undefined, undefined, { deliveryVersionStatus: 'draft' });
	let previewCalls = 0,
		deleted = false;
	let publication: Record<string, unknown> | null = null;
	const detail = {
		...changeSetDetail(),
		publicationDateRules: {
			today: options.today ?? '2027-08-01',
			minimumEffectiveFrom: options.today ?? '2027-08-01'
		},
		referenceDate: options.referenceDate ?? '2027-08-01',
		baseDeliveryVersionId: null,
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
				publicationNo: 2,
				publishedAt: '2027-08-01T00:00:00Z',
				publishedBy: '90000000-0000-4000-8000-000000000001',
				effectiveFrom: publication?.effectiveFrom,
				rowVersion: 2
			}));
		}
		if (deleted) return void (await fulfill(route, 'รุ่นนี้ถูกลบแล้ว', 404));
		return void (await fulfill(route, {
			...detail,
			status: publication ? 'published' : 'draft',
			publicationNo: publication ? 2 : null,
			publishedAt: publication ? '2027-08-01T00:00:00Z' : null,
			publishedBy: publication ? '90000000-0000-4000-8000-000000000001' : null,
			effectiveFrom:
				typeof publication?.effectiveFrom === 'string' ? publication.effectiveFrom : null,
			rowVersion: publication ? 2 : 1
		}));
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
			academicYearId: ids.year,
			academicTermId: ids.term,
			sourceVersionId: null,
			rowVersion: 2,
			status: publication ? 'published' : 'draft',
			publicationNo: publication ? 2 : null,
			isSuperseded: false,
			publishedAt: publication ? '2027-08-01T00:00:00Z' : null,
			publishedBy: publication ? '90000000-0000-4000-8000-000000000001' : null,
			effectiveFrom:
				typeof publication?.effectiveFrom === 'string' ? publication.effectiveFrom : null,
			effectiveUntil: null,
			referenceDate: '2027-08-01',
			createdAt: '2027-07-01T00:00:00Z',
			updatedAt: '2027-07-01T00:00:00Z',
			courseMetadata: [],
			snapshot: {
				offerings: [
					{
						id: ids.offering,
						kind: 'course',
						code: 'ค21101',
						name: 'คณิตศาสตร์พื้นฐาน',
						owningOrganizationUnitId: ids.program,
						weeklyPeriodTarget: 4,
						targets: [],
						homeroomIds: [ids.homeroom],
						catalog: {
							kind: 'course',
							subjectId: ids.catalog,
							subjectVersionId: ids.catalog,
							standardPeriodsPerWeek: 3,
							credit: '1.5',
							hours: '60',
							assessmentTotalScore: '100.00'
						},
						groups: [
							{
								id: ids.group,
								code: 'PREP-LIFECYCLE-GROUP',
								name: 'ม.1/1',
								capacity: null,
								homeroomIds: [ids.homeroom],
								preferredRoomIds: [],
								teachers: []
							}
						]
					},
					{
						id: secondOffering,
						kind: 'course',
						code: 'ว21101',
						name: 'วิทยาศาสตร์',
						owningOrganizationUnitId: ids.program,
						weeklyPeriodTarget: 3,
						targets: [],
						homeroomIds: [],
						catalog: {
							kind: 'course',
							subjectId: secondOffering,
							subjectVersionId: secondOffering,
							standardPeriodsPerWeek: 3,
							credit: '1.5',
							hours: '60',
							assessmentTotalScore: '100.00'
						},
						groups: []
					}
				]
			}
		} satisfies DeliveryVersion));
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
							status: publication ? 'published' : 'draft',
							publicationNo: publication ? 2 : null,
							isSuperseded: false,
							publishedAt: publication ? '2027-08-01T00:00:00Z' : null,
							publishedBy: publication ? '90000000-0000-4000-8000-000000000001' : null,
							effectiveFrom:
								typeof publication?.effectiveFrom === 'string' ? publication.effectiveFrom : null,
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
				displayName: 'อ่าน อย่างเดียว',
				lastName: 'อย่างเดียว',
				userType: 'staff',
				status: 'ACTIVE',
				session: sessionState(),
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
	{ name: 'desktop-light', width: 1280, height: 950, dark: false },
	{ name: 'mobile-dark', width: 390, height: 844, dark: true },
	{ name: 'desktop-dark', width: 1280, height: 950, dark: true },
	{ name: 'mobile-light', width: 390, height: 844, dark: false }
]) {
	test(`readiness renders repeated findings and named differences: ${viewport.name}`, async ({
		page
	}, testInfo) => {
		const errors: string[] = [];
		page.on('pageerror', (e) => errors.push(e.message));
		await page.setViewportSize(viewport);
		await page.addInitScript((dark) => {
			localStorage.setItem('ui-preferences', JSON.stringify({ theme: dark ? 'dark' : 'light' }));
		}, viewport.dark);
		await mockDraftLifecycle(page, { failPreview: true });
		await expect(page.locator('html')).toHaveClass(viewport.dark ? /dark/ : /^(?!.*dark).*$/);
		await expect(page.getByTestId('delivery-homerooms-ready')).toBeVisible();
		await page.screenshot({ path: testInfo.outputPath(`compact-overview-${viewport.name}.png`) });
		await expandChangeDetails(page);
		await expect(
			page.getByText('รายการเปิดสอน: เปิดสอน 3 คาบ/สัปดาห์ → เปิดสอน 4 คาบ/สัปดาห์', {
				exact: true
			})
		).toBeVisible();

		await expect(page.getByText(`${ids.offering} · 4 คาบ/สัปดาห์`)).toHaveCount(0);
		await page.screenshot({ path: testInfo.outputPath(`changes-dialog-${viewport.name}.png`) });
		await page.getByRole('button', { name: 'ปิดรายละเอียด', exact: true }).click();
		await expect(page.getByRole('alert')).toHaveCount(0);
		await page.getByRole('button', { name: 'ตรวจความพร้อม', exact: true }).click();
		const readiness = page.getByRole('dialog', { name: 'ตรวจผลกระทบและความพร้อม', exact: true });
		await expect(readiness.getByRole('alert')).toContainText('ตรวจไม่สำเร็จ');
		await page.getByRole('button', { name: 'ตรวจความพร้อมอีกครั้ง', exact: true }).click();
		await expect(page.getByText('ค21101 — คณิตศาสตร์พื้นฐาน: ยังไม่มีกลุ่มเรียน')).toBeVisible();
		await expect(page.getByText('ว21101 — วิทยาศาสตร์: ยังไม่มีกลุ่มเรียน')).toBeVisible();
		await expect(page.getByRole('link', { name: 'ไปแก้ไข' })).toHaveCount(2);
		expect(errors).toEqual([]);
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
		await page.screenshot({
			animations: 'disabled',
			path: testInfo.outputPath(`readiness-dialog-${viewport.name}.png`)
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
	await expect(page.getByRole('combobox', { name: 'เลือกรุ่นเปิดสอน', exact: true })).toContainText(
		'เผยแพร่แล้ว'
	);
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
		publicationNo: status === 'published' ? 2 : null,
		isSuperseded: false,
		publishedAt: status === 'published' ? '2027-07-01T00:00:00Z' : null,
		publishedBy: null,
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
							publicationNo: 1,
							effectiveFrom: '2027-04-01',
							effectiveUntil: '2027-04-30'
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
			subjectType: null,
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
					levelName: 'หลักสูตร',
					editionId: ids.curriculum
				}
			],
			gradeLevels: [{ id: ids.grade, name: 'ม.1', code: 'M1' }],
			homerooms: [{ id: ids.homeroom, name: 'ม.1/1', gradeLevelId: ids.grade }],
			catalogVersions: [
				{
					id: ids.catalog,
					kind: 'activity',
					subjectType: null,
					code: 'CLUB',
					label: 'CLUB — ชุมนุม (ฉบับ 3)',
					name: 'ชุมนุม',
					versionNo: 3,
					schedulingMode: 'synchronized',
					standardPeriodsPerWeek: null
				}
			] satisfies DeliveryManagementOptions['catalogVersions'],
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
			startsOn: offeringStartsOn,
			curriculumSources: [],
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
		await page.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true }).click();
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
		await expect(page.getByRole('dialog', { name: /เพิ่มกิจกรรม ชุมนุม/ })).toBeVisible();
		await expect(page.getByRole('dialog', { name: 'สร้างรุ่นเปิดสอน', exact: true })).toHaveCount(
			0
		);
		await selectOfferingStartDate(page);
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
	await page.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('เครือข่ายขัดข้อง');
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(1);
	await expect(page.getByRole('dialog')).toContainText('เพิ่มกิจกรรม ชุมนุม');
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
		await page.getByRole('dialog').getByRole('combobox', { name: 'ประเภท', exact: true }).click();
		await page.getByRole('option', { name: 'กิจกรรมพัฒนาผู้เรียน', exact: true }).click();
		for (const [label, option] of [
			['เลือกกิจกรรม', 'ชุมนุม (ฉบับ 3)'],
			['เลือกระดับชั้น', 'ม.1'],
			['เลือกแผนการเรียน', 'แผนมาตรฐาน']
		]) {
			await page.getByRole('combobox', { name: label, exact: true }).click();
			const visibleOption = page
				.getByRole('option')
				.filter({ has: page.getByText(option, { exact: true }) });
			await expect(visibleOption).toBeVisible();
			await expect(page.locator('body')).not.toContainText('CLUB');
			await visibleOption.click();
		}
		await expect(page.getByLabel('รูปแบบการเปิด')).toContainText('เปิดแบบกลาง');
		if (grouped) {
			await page.getByLabel('รูปแบบการเปิด').click();
			await page.getByRole('option', { name: 'เปิดพร้อมจัดกลุ่ม', exact: true }).click();
		}
		await selectOfferingStartDate(page, 'วันที่เริ่มเปิดสอนจริง');
		await page.getByRole('button', { name: 'สร้างฉบับร่าง', exact: true }).click();
		await expect
			.poll(() => body)
			.toMatchObject({
				kind: 'activity',
				startsOn: offeringStartsOn,
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
			const add = page.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true });
			await add.focus();
			await page.keyboard.press('Enter');
			await expect(page.getByRole('dialog')).toHaveCount(1);
			await expect(page.getByRole('dialog')).toContainText('เพิ่มกิจกรรม ชุมนุม');
			await selectOfferingStartDate(page);
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
	await page.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true }).click();
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
	await page.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true }).click();
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
			displayName: 'อ่าน อย่างเดียว',
			lastName: 'อย่างเดียว',
			userType: 'staff',
			status: 'ACTIVE',
			session: sessionState(),
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
	await expect(
		page.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true })
	).toHaveCount(0);
	expect(managementReads).toBe(0);
});

for (const viewport of [
	{ width: 1440, height: 900 },
	{ width: 390, height: 844 }
]) {
	for (const dark of [false, true]) {
		test(`curriculum preparation hides activity codes and retains course codes at ${viewport.width}px ${dark ? 'dark' : 'light'}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize(viewport);
			await mockDelivery(page, undefined, undefined, { deliveryVersionStatus: 'draft' });
			const resources = [
				{ resourceKind: 'activity', code: 'ACT-a123456789ab', name: 'กิจกรรมเพื่อสังคม' },
				{ resourceKind: 'activity', code: 'CLUB-b123456789ab', name: 'ชุมนุม' },
				{ resourceKind: 'activity', code: 'GUIDANCE-c123456789ab', name: 'แนะแนว' },
				{ resourceKind: 'course', code: 'ค21101', name: 'คณิตศาสตร์พื้นฐาน' }
			];
			await page.route('**/api/academic/delivery/management-options?**', (route) =>
				fulfill(route, {
					studyPrograms: [
						{ id: ids.program, code: 'DEFAULT', name: 'แผนมาตรฐาน', levelName: 'หลักสูตร' }
					],
					homerooms: [
						{ id: ids.homeroom, name: 'ม.1/1', gradeLevelId: ids.grade },
						{ id: ids.otherHomeroom, name: 'ม.1/2', gradeLevelId: ids.grade }
					],
					catalogVersions: [],
					gradeLevels: [],
					teachers: [],
					rooms: [],
					learningGroups: []
				})
			);
			await page.route('**/api/academic/offerings/preview-from-curriculum', (route) =>
				fulfill(route, {
					startsOn: offeringStartsOn,
					curriculumSources: [],
					sourceHash: 'a'.repeat(64),
					proposals: resources.map((resource, index) => ({
						...resource,
						proposalId: `proposal-${index}`,
						catalogVersionId: ids.catalog,
						schedulingMode: 'independent',
						targetHomeroomIds: [ids.homeroom, ids.otherHomeroom],
						requirementIds: [ids.requirement],
						requirementKind: 'required',
						groupingState: 'proposed',
						offeringAction: 'create',
						conflicts: [],
						defaultGroups: [ids.homeroom, ids.otherHomeroom].map((homeroomId, room) => ({
							groupKey: `${index}${room}`.padEnd(64, 'a'),
							homeroomIds: [homeroomId],
							name: `${resource.resourceKind === 'activity' ? resource.name : resource.code} · ม.1/${room + 1}`
						}))
					}))
				})
			);
			let saves = 0;
			page.on('request', (request) => {
				if (request.url().includes('/apply-from-curriculum')) saves++;
			});
			await page.goto(
				`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}`
			);
			if (dark) await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
			await page.getByRole('button', { name: 'เปิดการเรียนการสอน', exact: true }).click();
			const dialog = page.getByRole('dialog');
			await dialog.getByRole('combobox', { name: 'เลือกแผนการเรียนจากหลักสูตร' }).click();
			await page.getByRole('option', { name: /แผนมาตรฐาน/ }).click();
			await page.keyboard.press('Escape');
			await selectOfferingStartDate(page);
			await dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน' }).click();
			for (const resource of resources) {
				const prefix = resource.resourceKind === 'activity' ? resource.name : resource.code;
				const label =
					resource.resourceKind === 'activity'
						? resource.name
						: `${resource.code} · ${resource.name}`;
				const card = dialog
					.locator('.space-y-3.p-4')
					.filter({ has: page.getByText(label, { exact: true }) });
				await expect(card).toHaveCount(1);
				const groups = card.getByRole('textbox', { name: 'ชื่อกลุ่มเรียน' });
				await expect(groups).toHaveCount(2);
				await expect(groups.nth(0)).toHaveValue(`${prefix} · ม.1/1`);
				await expect(groups.nth(1)).toHaveValue(`${prefix} · ม.1/2`);
				await card.getByRole('button', { name: 'รวมหลายห้อง' }).click();
				await expect(groups).toHaveCount(1);
				await expect(groups.first()).toHaveValue(`${prefix} · เรียนรวม ม.1/1, ม.1/2`);
				await card.getByRole('button', { name: 'เพิ่มกลุ่มแบ่ง' }).click();
				await expect(groups.nth(1)).toHaveValue(`${prefix} · ม.1/1 กลุ่ม 2`);
				const customName = `${resource.code} ชื่อที่ผู้ใช้กำหนด`;
				await groups.first().fill(customName);
				await card.getByRole('button', { name: 'เพิ่มกลุ่มแบ่ง' }).click();
				await expect(groups.first()).toHaveValue(customName);
				await card.getByRole('button', { name: 'คืนค่าข้อเสนอ' }).click();
				await expect(groups).toHaveCount(2);
				await expect(groups.first()).toHaveValue(`${prefix} · ม.1/1`);
			}
			await expect(dialog).not.toContainText(
				/ACT-a123456789ab|CLUB-b123456789ab|GUIDANCE-c123456789ab/
			);
			for (const input of await dialog.getByRole('textbox', { name: 'ชื่อกลุ่มเรียน' }).all()) {
				expect(await input.inputValue()).not.toMatch(
					/ACT-a123456789ab|CLUB-b123456789ab|GUIDANCE-c123456789ab/
				);
			}
			await dialog.getByText('กิจกรรมเพื่อสังคม', { exact: true }).scrollIntoViewIfNeeded();
			await page.screenshot({ path: testInfo.outputPath('activity-curriculum-preview.png') });
			expect(saves).toBe(0);
		});
	}
}

test('curriculum preparation orders groups and room controls naturally while preserving canonical choices', async ({
	page
}, testInfo) => {
	await mockDelivery(page, undefined, undefined, { deliveryVersionStatus: 'draft' });
	const labels = ['ม.2/3', 'ม.2/2', 'ม.2/1', 'ม.1/10', 'ม.1/3', 'ม.1/2', 'ม.1/1'];
	const rooms = labels.map((name, index) => ({
		id: `60000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
		name,
		gradeLevelId: name.startsWith('ม.1') ? ids.grade : 'grade2'
	}));
	const defaultGroups = rooms.map((room, index) => ({
		groupKey: `${index}`.padEnd(64, 'a'),
		name: `ชุมนุม · ${room.name}`,
		homeroomIds: [room.id]
	}));
	const mixed = {
		groupKey: 'f'.repeat(64),
		name: 'ชื่อกลุ่มเดิมที่กำหนดเอง',
		homeroomIds: [rooms[0].id, rooms[6].id]
	};
	const groups = [...defaultGroups, mixed];
	const naturalLabels = [...labels].reverse();
	const expectedNames = [
		defaultGroups[6].name,
		mixed.name,
		...defaultGroups
			.slice(0, 6)
			.reverse()
			.map((group) => group.name)
	];
	const sourceHash = 'b'.repeat(64);
	await page.route('**/api/academic/delivery/management-options?**', (route) =>
		fulfill(route, {
			studyPrograms: [
				{ id: ids.program, code: 'DEFAULT', name: 'แผนมาตรฐาน', levelName: 'หลักสูตร' }
			],
			homerooms: rooms,
			gradeLevels: [
				{
					id: 'grade2',
					code: 'M2',
					name: 'ม.2',
					short_name: 'ม.2',
					level_order: 302,
					level_type: 'secondary'
				},
				{
					id: ids.grade,
					code: 'M1',
					name: 'ม.1',
					short_name: 'ม.1',
					level_order: 301,
					level_type: 'secondary'
				}
			],
			catalogVersions: [],
			teachers: [],
			rooms: [],
			learningGroups: []
		})
	);
	await page.route('**/api/academic/offerings/preview-from-curriculum', (route) =>
		fulfill(route, {
			startsOn: offeringStartsOn,
			curriculumSources: [],
			sourceHash,
			proposals: [
				{
					proposalId: 'naturally-ordered',
					resourceKind: 'activity',
					catalogVersionId: ids.catalog,
					code: 'CLUB-technical',
					name: 'ชุมนุม',
					schedulingMode: 'independent',
					targetHomeroomIds: rooms.map((room) => room.id),
					requirementIds: [ids.requirement],
					requirementKind: 'required',
					groupingState: 'proposed',
					offeringAction: 'retain',
					conflicts: [],
					defaultGroups: groups
				}
			]
		})
	);
	let submitted: Record<string, unknown> | null = null;
	await page.route('**/api/academic/offerings/apply-from-curriculum', async (route) => {
		submitted = route.request().postDataJSON();
		await fulfill(route, 'ทดสอบเก็บชื่อและลำดับเดิม', 400);
	});
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}`
	);
	await page.getByRole('button', { name: 'เปิดการเรียนการสอน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('combobox', { name: 'เลือกแผนการเรียนจากหลักสูตร' }).click();
	await page.getByRole('option', { name: /แผนมาตรฐาน/ }).click();
	await page.keyboard.press('Escape');
	await selectOfferingStartDate(page);
	await dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน' }).click();
	const inputs = dialog.getByRole('textbox', { name: 'ชื่อกลุ่มเรียน' });
	const inputNames = () =>
		inputs.evaluateAll((elements) =>
			elements.map((element) => (element as HTMLInputElement).value)
		);
	await expect.poll(inputNames).toEqual(expectedNames);
	await expect(
		dialog.getByRole('button', { name: `เลือกห้องสำหรับ ${mixed.name}`, exact: true })
	).toHaveText('ม.1/1, ม.2/3');
	await inputs.first().fill('ชื่อใหม่ที่ไม่บอกห้อง');
	await expect.poll(inputNames).toEqual(['ชื่อใหม่ที่ไม่บอกห้อง', ...expectedNames.slice(1)]);
	await dialog
		.getByRole('button', { name: 'เลือกห้องสำหรับ ชื่อใหม่ที่ไม่บอกห้อง', exact: true })
		.click();
	await expect(page.getByRole('option')).toHaveText(naturalLabels);
	await page.getByRole('option').nth(1).click();
	await page.getByRole('option').nth(0).click();
	await page.keyboard.press('Escape');
	await expect
		.poll(inputNames)
		.toEqual([
			mixed.name,
			defaultGroups[5].name,
			'ชื่อใหม่ที่ไม่บอกห้อง',
			...expectedNames.slice(3)
		]);
	await dialog.getByRole('button', { name: 'คืนค่าข้อเสนอ' }).click();
	await expect.poll(inputNames).toEqual(expectedNames);
	await page.screenshot({ path: testInfo.outputPath('naturally-ordered-preparation-groups.png') });
	await dialog.getByRole('button', { name: 'เพิ่มกลุ่มแบ่ง' }).click();
	await expect(inputs.nth(2)).toHaveValue('ชุมนุม · ม.1/1 กลุ่ม 9');
	await dialog.getByRole('button', { name: 'รวมหลายห้อง' }).click();
	await expect(inputs).toHaveCount(1);
	await expect(inputs.first()).toHaveValue(`ชุมนุม · เรียนรวม ${naturalLabels.join(', ')}`);
	await dialog.getByRole('button', { name: 'เพิ่มกลุ่มแบ่ง' }).click();
	await expect(inputs.nth(1)).toHaveValue('ชุมนุม · ม.1/1 กลุ่ม 2');
	await dialog.getByRole('button', { name: 'คืนค่าข้อเสนอ' }).click();
	await expect.poll(inputNames).toEqual(expectedNames);
	await dialog.getByRole('button', { name: 'ยืนยันรายการและกลุ่มที่ตรวจแล้ว' }).click();
	await expect
		.poll(() => submitted)
		.toMatchObject({
			sourceHash,
			choices: [{ proposalId: 'naturally-ordered', action: 'apply', groups }]
		});
	await expect(dialog.getByRole('alert')).toHaveText('ทดสอบเก็บชื่อและลำดับเดิม');
});

test('focused activity preparation hides its code in the heading and missing-proposal error', async ({
	page
}) => {
	await mockActivityActivation(page, 1);
	await page.route('**/api/academic/offerings/preview-from-curriculum', (route) =>
		fulfill(route, {
			startsOn: offeringStartsOn,
			curriculumSources: [],
			sourceHash: 'a'.repeat(64),
			proposals: []
		})
	);
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${activitySource}`
	);
	await page.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'เพิ่มกิจกรรม ชุมนุม', exact: true });
	await expect(
		dialog.getByRole('heading', { name: 'เปิดใช้งาน ชุมนุม', exact: true })
	).toBeVisible();
	await selectOfferingStartDate(page);
	await dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน' }).click();
	await expect(dialog.getByRole('alert')).toHaveText('ไม่พบ ชุมนุม ในโครงสร้างของห้องเป้าหมาย');
	await expect(dialog).not.toContainText('CLUB');
});

test('activity preparation preserves server-owned existing group names on restore', async ({
	page
}) => {
	await mockActivityActivation(page, 1);
	const existingName = 'CLUB-legacy · กลุ่มชื่อเดิม';
	await page.route('**/api/academic/offerings/preview-from-curriculum', (route) =>
		fulfill(route, {
			startsOn: offeringStartsOn,
			curriculumSources: [],
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
					offeringAction: 'retain',
					conflicts: [],
					defaultGroups: [
						{ groupKey: 'a'.repeat(64), name: existingName, homeroomIds: [ids.homeroom] }
					]
				}
			]
		})
	);
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${activitySource}`
	);
	await page.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await selectOfferingStartDate(page);
	await dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน' }).click();
	await dialog.getByRole('combobox', { name: 'วิธีเตรียม ชุมนุม', exact: true }).click();
	await page.getByRole('option', { name: 'เปิดสอนและจัดกลุ่ม', exact: true }).click();
	const group = dialog.getByRole('textbox', { name: 'ชื่อกลุ่มเรียน' });
	await expect(group).toHaveValue(existingName);
	await group.fill('ชื่อที่แก้ไข');
	await dialog.getByRole('button', { name: 'คืนค่าข้อเสนอ' }).click();
	await expect(group).toHaveValue(existingName);
});

test('curriculum synchronized activation can explicitly create its reviewed homeroom groups', async ({
	page
}) => {
	const result = await mockActivityActivation(page, 1);
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${activitySource}`
	);
	await page.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true }).click();
	await expect(page.getByRole('dialog', { name: /เพิ่มกิจกรรม ชุมนุม/ })).toBeVisible();
	await selectOfferingStartDate(page);
	await page.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน' }).click();
	await page.getByRole('combobox', { name: 'วิธีเตรียม ชุมนุม', exact: true }).click();
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
					courseMetadata: [],
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

			await page.route(`**/api/academic/offerings/${ids.offering}`, (route) =>
				fulfill(route, {
					id: ids.offering,
					academicYearId: ids.year,
					academicTermId: ids.term,
					kind: 'course',
					codeSnapshot: offering.code,
					nameSnapshot: offering.name,
					status: 'draft',
					rowVersion: 1,
					targets: [],
					snapshot: offering.catalog
				})
			);
			await page.route(`**/api/academic/offerings/${ids.offering}/groups`, (route) =>
				fulfill(route, [])
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
				`/staff/academic/delivery/${ids.offering}?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}`
			);
			if (dark) {
				await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
				await expect(page.locator('html')).toHaveClass(/dark/);
			}
			const trigger = page.getByRole('button', { name: 'แก้ไขจำนวนคาบ', exact: true });
			await trigger.scrollIntoViewIfNeeded();
			const initialScroll = await page.evaluate(() => window.scrollY);
			await trigger.focus();
			await page.keyboard.press('Enter');
			const dialog = page.getByRole('dialog', { name: 'แก้ไขจำนวนคาบต่อสัปดาห์', exact: true });
			await expect(dialog).toBeVisible();
			await expect(
				dialog.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true })
			).toBeVisible();
			const periods = page.getByLabel('คาบที่จัดจริงต่อสัปดาห์');
			await expect(periods).toHaveValue('4');
			await expect(periods).toHaveAttribute('min', '0');
			await expect(dialog.getByText('ตามหลักสูตร 4 คาบ/สัปดาห์', { exact: true })).toBeVisible();
			await periods.fill('-1');
			await expect(
				page.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true })
			).toBeDisabled();
			await periods.fill('0.5');
			await expect(
				page.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true })
			).toBeDisabled();
			await periods.fill('0');
			await expect(page.getByText(/0 คาบ = เปิดรายวิชา/)).toBeVisible();
			await expect(page.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true })).toBeEnabled();
			await page.screenshot({
				path: `/tmp/zero-periods-${viewport.width}-${dark ? 'dark' : 'light'}.png`
			});
			await page.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true }).click();
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
			await dialog.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true }).click();
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
			await expect(periods).toHaveValue('4');
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
			await mockDelivery(page, undefined, undefined, { deliveryVersionStatus: 'draft' });
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
					courseMetadata: [],
					snapshot: { offerings }
				})
			);
			await page.route('**/api/academic/delivery/management-options?*', (route) =>
				fulfill(route, {
					catalogVersions: offerings.map((item) => ({
						id: item.id,
						kind: item.kind,
						subjectType: item.kind === 'course' ? 'BASIC' : null,
						code: item.code,
						name: item.name,
						label: `${item.code} — ${item.name} (ฉบับ 1)`,
						versionNo: 1,
						standardPeriodsPerWeek:
							item.kind === 'course'
								? item.code === 'ส21201'
									? 0
									: item.code === 'ส23201'
										? null
										: 1
								: null,
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
			await page.getByRole('button', { name: 'เพิ่มวิชาพื้นฐาน', exact: true }).click();
			const dialog = page.getByRole('dialog', { name: 'เพิ่มวิชาพื้นฐาน', exact: true });
			await dialog.getByRole('combobox', { name: 'เลือกรายวิชา', exact: true }).click();
			const courses = [/ส21201/, /ส23201/, /ส32201/, /ส33201/];
			await expect(page.getByRole('option')).toHaveText(courses);
			await expect(page.getByRole('option', { name: /ส21201/ })).toContainText(
				'มาตรฐาน 0 คาบ/สัปดาห์'
			);
			await expect(page.getByRole('option', { name: /ส23201/ })).not.toContainText('มาตรฐาน');
			await expect(page.getByRole('option', { name: /ส32201/ })).toContainText(
				'มาตรฐาน 1 คาบ/สัปดาห์'
			);
			await page.getByPlaceholder('ค้นหารหัสหรือชื่อ...').fill('ส21201');
			await expect(page.getByRole('option')).toHaveCount(1);
			await page.getByRole('option', { name: /ส21201/ }).click();
			await expect(
				dialog.getByRole('combobox', { name: 'เลือกรายวิชา', exact: true })
			).toContainText('ส21201');
			const bounds = await dialog.boundingBox();
			expect(bounds!.y).toBeGreaterThanOrEqual(0);
			expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(viewport.height);
			await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
			await page.getByRole('button', { name: 'เพิ่มกิจกรรมพัฒนาผู้เรียน', exact: true }).click();
			const activity = page.getByRole('dialog', { name: 'เพิ่มกิจกรรมพัฒนาผู้เรียน', exact: true });
			await activity.getByRole('combobox', { name: 'เลือกกิจกรรม', exact: true }).click();
			await expect(page.getByRole('option')).toHaveText([
				'ชุมนุม (ฉบับ 1) กิจกรรม',
				'รักการอ่าน (ฉบับ 1) กิจกรรม'
			]);
			await expect(page.getByRole('option')).not.toContainText(['CLUB-', 'OTHER-']);
			const search = page.getByPlaceholder('ค้นหารหัสหรือชื่อ...');
			await search.fill('รักการอ่าน');
			await expect(page.getByRole('option')).toHaveCount(1);
			await page.getByRole('option', { name: /รักการอ่าน/ }).click();
			await expect(
				activity.getByRole('combobox', { name: 'เลือกกิจกรรม', exact: true })
			).toContainText('รักการอ่าน');
			await page.keyboard.press('Escape');
			await expect(activity).toHaveCount(0);
		});
	}
}

for (const viewport of [
	{ width: 1440, height: 900 },
	{ width: 390, height: 844 }
]) {
	for (const dark of [false, true]) {
		test(`single delivery change list deletes the matching command at ${viewport.width}px ${dark ? 'dark' : 'light'}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize(viewport);
			await mockDelivery(page);
			const itemId = '84000000-0000-4000-8000-000000000001';
			const otherOffering = '80000000-0000-4000-8000-000000000002';
			const detail = {
				...changeSetDetail(),
				rowVersion: 9,
				offeringLabels: [{ id: ids.offering, code: 'ส22201', name: 'หน้าที่พลเมือง' }],
				items: [
					{
						id: itemId,
						actionKind: 'adjust_weekly_period_target',
						learningOfferingId: ids.offering,
						weeklyPeriodTarget: 0,
						rowVersion: 4,
						createdAt: '2027-07-01T00:00:00Z',
						updatedAt: '2027-07-01T00:00:00Z',
						createdBy: '90000000-0000-4000-8000-000000000001'
					}
				],
				changes: [ids.offering, otherOffering].map((resourceId, index) => ({
					resourceId,
					learningOfferingId: resourceId,
					kind: 'changed',
					label: index === 0 ? 'ส22201 — หน้าที่พลเมือง' : 'ค21101 — คณิตศาสตร์พื้นฐาน',
					field: 'รายการเปิดสอน',
					before: 'เปิดสอน 1 คาบ/สัปดาห์',
					after: 'เปิดสอน 0 คาบ/สัปดาห์'
				}))
			};
			await page.route(`**/api/academic/term-change-sets/${ids.changeSet}`, (route) =>
				fulfill(route, detail)
			);
			let attempts = 0;
			let releaseDelete!: () => void;
			const deleteGate = new Promise<void>((resolve) => (releaseDelete = resolve));
			await page.route(
				`**/api/academic/term-change-sets/${ids.changeSet}/items/${itemId}`,
				async (route) => {
					expect(route.request().method()).toBe('DELETE');
					expect(route.request().postDataJSON()).toEqual({
						changeSetRowVersion: 9,
						itemRowVersion: 4
					});
					if (++attempts === 1) return fulfill(route, 'ลบไม่สำเร็จ ลองอีกครั้ง', 400);
					await deleteGate;
					return fulfill(route, {
						...detail,
						rowVersion: 10,
						items: [],
						changes: detail.changes.slice(1)
					});
				}
			);
			await page.goto(
				`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}`
			);
			if (dark) {
				await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
				await expect(page.locator('html')).toHaveClass(/dark/);
			}
			await expect(
				page.getByRole('button', { name: 'ดูรายละเอียดการเปลี่ยนแปลง', exact: true })
			).toBeVisible();
			await page.screenshot({ path: testInfo.outputPath('change-summary-collapsed.png') });
			await expandChangeDetails(page);
			const list = page.getByLabel('ความเปลี่ยนแปลงจากรุ่นต้นทาง');
			await expect(list.locator(':scope > div')).toHaveCount(2);
			await expect(page.getByText('รายการคำสั่งที่บันทึกไว้', { exact: true })).toHaveCount(0);
			const card = list.locator(':scope > div').filter({ hasText: 'หน้าที่พลเมือง' });
			await expect(card).toContainText('เปิดสอน 1 คาบ/สัปดาห์ → เปิดสอน 0 คาบ/สัปดาห์');
			const remove = card.getByRole('button', { name: /^ลบ ปรับจำนวนคาบต่อสัปดาห์/ });
			await expect(remove).toBeVisible();
			await expect(list.getByRole('button', { name: /^ลบ / })).toHaveCount(1);
			await card.scrollIntoViewIfNeeded();
			await page.screenshot({
				path: testInfo.outputPath('change-summary-expanded.png'),
				fullPage: true
			});
			const bounds = await remove.boundingBox();
			expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(viewport.width);
			await remove.click();
			await expect(page.getByRole('alert')).toContainText('ลบไม่สำเร็จ ลองอีกครั้ง');
			await expect(card).toBeVisible();
			await remove.click();
			await expect(remove).toBeDisabled();
			releaseDelete();
			await expect(card).toHaveCount(0);
			await expect(list.locator(':scope > div')).toHaveCount(1);
			await expect(list).toContainText('คณิตศาสตร์พื้นฐาน');
		});
	}
}

test('teacher commands share one change card and stale deletion retries only the selected command', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await mockDelivery(page);
	const addId = '84000000-0000-4000-8000-000000000002';
	const stopId = '84000000-0000-4000-8000-000000000003';
	let detail = {
		...changeSetDetail(),
		items: [
			{
				id: addId,
				actionKind: 'add_group_teacher',
				learningGroupId: ids.group,
				learningGroupLabel: 'ม.1/1',
				teacherId: '85000000-0000-4000-8000-000000000002',
				teacherLabel: 'ครูชื่อซ้ำ',
				teacherRole: 'primary',
				rowVersion: 1
			},
			{
				id: stopId,
				actionKind: 'stop_group_teacher',
				learningGroupId: ids.group,
				learningGroupLabel: 'ม.1/1',
				learningGroupTeacherId: '86000000-0000-4000-8000-000000000003',
				teacherId: '85000000-0000-4000-8000-000000000003',
				teacherLabel: 'ครูชื่อซ้ำ',
				teacherRole: 'primary',
				rowVersion: 2
			}
		],
		changes: [
			{
				resourceId: ids.group,
				learningOfferingId: ids.offering,
				label: 'คณิตศาสตร์ · ม.1/1 · ครูชื่อซ้ำ',
				field: 'ครูผู้สอน',
				kind: 'removed',
				before: 'ครูหลัก',
				after: null
			}
		]
	};
	await page.route(`**/api/academic/term-change-sets/${ids.changeSet}`, (route) =>
		fulfill(route, detail)
	);
	let attempts = 0;
	await page.route(`**/api/academic/term-change-sets/${ids.changeSet}/items/*`, async (route) => {
		expect(new URL(route.request().url()).pathname).toBe(
			`/api/academic/term-change-sets/${ids.changeSet}/items/${stopId}`
		);
		const body = route.request().postDataJSON();
		if (++attempts === 1) {
			expect(body).toEqual({ changeSetRowVersion: 1, itemRowVersion: 2 });
			detail = {
				...detail,
				rowVersion: 5,
				items: detail.items.map((item) => ({ ...item, rowVersion: 6 }))
			};
			return fulfill(route, 'แบบร่างถูกแก้ไขจากที่อื่น', 409);
		}
		expect(body).toEqual({ changeSetRowVersion: 5, itemRowVersion: 6 });
		detail = {
			...detail,
			rowVersion: 6,
			items: detail.items.filter((item) => item.id !== stopId),
			changes: []
		};
		return fulfill(route, detail);
	});
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}`
	);
	await expect(
		page.getByRole('button', { name: 'ดูรายละเอียดการเปลี่ยนแปลง', exact: true })
	).toBeVisible();
	await expandChangeDetails(page);
	const list = page.getByLabel('ความเปลี่ยนแปลงจากรุ่นต้นทาง');
	await expect(list.locator(':scope > div')).toHaveCount(1);
	const stop = list.getByRole('button', { name: /^ลบ หยุดความรับผิดชอบของครู/ });
	const add = list.getByRole('button', { name: /^ลบ เพิ่มครูในกลุ่มเรียน/ });
	await expect(stop).toBeVisible();
	await expect(add).toBeVisible();
	await list.scrollIntoViewIfNeeded();
	await page.screenshot({ path: '/tmp/delivery-change-list-teachers-mobile.png', fullPage: true });
	await stop.click();
	await expect(page.getByRole('alert')).toContainText('แบบร่างถูกแก้ไขจากที่อื่น');
	await expect(stop).toBeEnabled();
	await stop.click();
	await expect(stop).toHaveCount(0);
	await expandChangeDetails(page);
	await expect(add).toBeVisible();
	await expect(list.locator(':scope > div')).toHaveCount(1);
	await expect(list).toContainText('เพิ่มครูในกลุ่มเรียน');
});

test('large delivery changes stay collapsed and bound offering, resource and difference pages', async ({
	page
}, testInfo) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await mockDelivery(page);
	const labels = Array.from({ length: 20 }, (_, index) => ({
		id: `offering-${index}`,
		code: `ว${index + 1}`,
		name: `วิชาทดสอบ ${index + 1}`
	}));
	const changes = labels.flatMap((offering, offeringIndex) =>
		Array.from({ length: offeringIndex === 0 ? 101 : 1 }, (_, groupIndex) => ({
			resourceId: `${offering.id}-group-${groupIndex}`,
			learningOfferingId: offering.id,
			label: `กลุ่ม ${groupIndex + 1}`,
			field: 'ครูผู้สอน',
			kind: 'changed',
			before: 'ครูเดิม',
			after: groupIndex === 100 ? 'ครูค้นหาเจอ' : 'ครูใหม่'
		}))
	);
	changes.push(
		...Array.from({ length: 20 }, (_, index) => ({
			...changes[0],
			field: `รายละเอียด ${index + 1}`,
			after: index === 19 ? 'คำค้นท้ายรายละเอียด' : 'ข้อมูลใหม่'
		}))
	);
	await page.route(`**/api/academic/term-change-sets/${ids.changeSet}`, (route) =>
		fulfill(route, { ...changeSetDetail(), offeringLabels: labels, changes, items: [] })
	);
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}`
	);
	await expect(page.getByTestId('delivery-change-offering')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'ตรวจความพร้อม', exact: true })).toBeVisible();
	const details = page.getByRole('button', { name: 'ดูรายละเอียดการเปลี่ยนแปลง', exact: true });
	await expect(details).toBeVisible();
	await details.click();
	const dialog = page.getByRole('dialog', { name: 'รายละเอียดการเปลี่ยนแปลง', exact: true });
	await expect(dialog).toBeVisible();
	await expect(dialog.getByTestId('delivery-change-summary')).toHaveText(
		'20 กลุ่มรายการ · 120 รายการย่อย'
	);
	await expect(dialog.getByTestId('delivery-change-offering')).toHaveCount(8);
	const first = page.getByTestId('delivery-change-offering').first();
	await first.getByRole('button', { name: /ดูรายละเอียด/ }).click();
	await expect(first.getByText(/ครูผู้สอน:/)).toHaveCount(8);
	await expect(first.getByRole('button', { name: 'รายละเอียดถัดไป', exact: true })).toHaveCount(1);
	await first.getByRole('button', { name: 'รายละเอียดถัดไป', exact: true }).click();
	await expect(first.getByText(/รายละเอียด /)).toHaveCount(8);
	await page.getByLabel('ค้นหารายวิชา กิจกรรม กลุ่ม หรือครู', { exact: true }).fill('ครูค้นหาเจอ');
	await expect(page.getByTestId('delivery-change-offering')).toHaveCount(1);
	await expect(first).toContainText('ครูค้นหาเจอ');
	await page
		.getByLabel('ค้นหารายวิชา กิจกรรม กลุ่ม หรือครู', { exact: true })
		.fill('คำค้นท้ายรายละเอียด');
	await expect(first).toContainText('คำค้นท้ายรายละเอียด');
	await page.screenshot({ path: testInfo.outputPath('bounded-change-summary-mobile.png') });
	await page.getByLabel('ค้นหารายวิชา กิจกรรม กลุ่ม หรือครู', { exact: true }).fill('');
	await expect(page.getByTestId('delivery-change-offering')).toHaveCount(8);
	await page.getByRole('button', { name: 'รายการถัดไป', exact: true }).click();
	await expect(page.getByTestId('delivery-change-offering')).toHaveCount(8);
	await page.getByRole('button', { name: 'รายการถัดไป', exact: true }).click();
	await expect(page.getByTestId('delivery-change-offering')).toHaveCount(4);
});

test('publication starts on server today when the source reference is a later plan', async ({
	page
}) => {
	const result = await mockDraftLifecycle(page, {
		today: '2026-11-01',
		referenceDate: '2026-11-05'
	});
	await page.getByRole('button', { name: 'เผยแพร่รุ่นเปิดสอน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(
		dialog.getByRole('button', { name: 'วันที่เริ่มใช้รุ่นเปิดสอน', exact: true })
	).toContainText('1 พ.ย. 2569');
	await expect(dialog).toContainText('เผยแพร่หลายรุ่นในวันเดียวกันได้');
	await dialog.getByRole('button', { name: 'ตรวจความพร้อมตามวันที่เลือก' }).click();
	await dialog.getByRole('button', { name: 'ยืนยันเผยแพร่' }).click();
	expect(result.published()?.effectiveFrom).toBe('2026-11-01');
	await expect(page.getByRole('combobox', { name: 'เลือกรุ่นเปิดสอน', exact: true })).toContainText(
		'รุ่นที่ 2'
	);
});
