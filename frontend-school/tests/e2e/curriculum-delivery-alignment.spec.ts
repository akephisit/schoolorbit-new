import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

const ids = {
	year: '12000000-0000-4000-8000-000000000401',
	futureYear: '12000000-0000-4000-8000-000000000402',
	term: '22000000-0000-4000-8000-000000000401',
	timetableVersion: '32000000-0000-4000-8000-000000000401',
	curriculum: '42000000-0000-4000-8000-000000000401',
	curriculumVersion: '52000000-0000-4000-8000-000000000401',
	clonedVersion: '52000000-0000-4000-8000-000000000402',
	program: '62000000-0000-4000-8000-000000000401',
	grade: '72000000-0000-4000-8000-000000000401',
	homeroom: '82000000-0000-4000-8000-000000000401',
	requirement: '92000000-0000-4000-8000-000000000401',
	catalogVersion: 'a2000000-0000-4000-8000-000000000401',
	offering: 'b2000000-0000-4000-8000-000000000401',
	extraOffering: 'b2000000-0000-4000-8000-000000000402',
	group: 'c2000000-0000-4000-8000-000000000401',
	user: 'd2000000-0000-4000-8000-000000000401'
};

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

function deferred() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { promise, release };
}

function homeroomWorkspace() {
	return {
		academicYearId: ids.year,
		academicTermId: ids.term,
		timetableVersionId: ids.timetableVersion,
		timetableVersionStatus: 'published',
		timetableVersionEffectiveFrom: '2026-08-15',
		homerooms: [
			{
				homeroom: { id: ids.homeroom, name: 'ม.1/1', gradeLevelId: ids.grade, gradeLevel: 'ม.1' },
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
					name: 'แผนการเรียนพื้นฐาน',
					curriculumId: ids.curriculum,
					curriculumName: 'หลักสูตรสถานศึกษา 2569'
				},
				curriculumVersionId: ids.curriculumVersion,
				expectedCount: 1,
				readyCount: 1,
				blockers: [],
				items: [
					{
						requirementId: ids.requirement,
						catalogVersionId: ids.catalogVersion,
						resourceKind: 'course',
						code: 'ค21101',
						name: 'คณิตศาสตร์พื้นฐาน',
						requirementKind: 'required',
						standardPeriodsPerWeek: 1,
						weeklyPeriodTarget: 2,
						alignmentStates: ['operational_periods_differ'],
						offeringId: ids.offering,
						offeringState: 'published',
						groupMode: 'normal',
						teacherState: 'assigned',
						timetableState: 'scheduled',
						groups: [
							{
								id: ids.group,
								code: 'M1-1-MATH',
								name: 'คณิตศาสตร์ ม.1/1',
								status: 'published',
								rosterStatus: 'published',
								teachersLocked: true,
								primaryTeacherCount: 1,
								timetableEntryCount: 2,
								homeroomIds: [ids.homeroom],
								homeroomNames: ['ม.1/1']
							}
						]
					}
				],
				extraOfferings: [
					{
						offeringId: ids.extraOffering,
						catalogVersionId: 'a2000000-0000-4000-8000-000000000402',
						resourceKind: 'course',
						code: 'ว20299',
						name: 'วิทยาศาสตร์เสริม',
						weeklyPeriodTarget: 1,
						startsOn: null,
						endsOn: null,
						alignmentStates: ['extra_offering']
					}
				]
			}
		],
		unlinked: []
	};
}

function curriculumVersion(
	id: string,
	status: 'draft' | 'published',
	versionName: string,
	startAcademicYearId = ids.year
) {
	return {
		id,
		curriculumId: ids.curriculum,
		versionName,
		startAcademicYearId,
		endAcademicYearId: null,
		description: null,
		status,
		rowVersion: status === 'published' ? 4 : 1,
		migrated: false,
		publishedAt: status === 'published' ? '2026-05-01T00:00:00Z' : null,
		createdAt: '2026-04-01T00:00:00Z',
		updatedAt: '2026-08-30T00:00:00Z'
	};
}

function curriculumStructure(
	version = curriculumVersion(ids.curriculumVersion, 'published', 'ฉบับ 2569')
) {
	return {
		curriculumVersion: version,
		rowVersion: version.rowVersion,
		gradeLevels: [
			{
				id: ids.grade,
				code: 'M1',
				name: 'มัธยมศึกษาปีที่ 1',
				short_name: 'ม.1',
				level_type: 'secondary',
				level_order: 301
			}
		],
		termSlots: [
			{
				id: 'e2000000-0000-4000-8000-000000000401',
				curriculumVersionId: version.id,
				sequence: 1,
				name: 'ภาคเรียนที่ 1',
				termType: 'regular',
				typeOccurrence: 1,
				rowVersion: 1
			}
		],
		programs: [
			{
				id: ids.program,
				curriculumVersionId: version.id,
				code: 'DEFAULT',
				nameTh: 'แผนการเรียนพื้นฐาน',
				nameEn: null,
				isDefault: true,
				owningOrganizationUnitId: null,
				status: version.status,
				rowVersion: 1,
				createdAt: '2026-04-01T00:00:00Z',
				updatedAt: '2026-08-30T00:00:00Z'
			}
		],
		requirements: [
			{
				id: ids.requirement,
				studyProgramId: ids.program,
				gradeLevel: {
					id: ids.grade,
					code: 'M1',
					name: 'มัธยมศึกษาปีที่ 1',
					short_name: 'ม.1',
					level_type: 'secondary',
					level_order: 301
				},
				termSlotId: 'e2000000-0000-4000-8000-000000000401',
				resourceKind: 'course',
				catalogVersionId: ids.catalogVersion,
				code: 'ค21101',
				name: 'คณิตศาสตร์พื้นฐาน',
				requirementKind: 'required',
				section: 'basic_course',
				metrics: {
					credit: '0.50',
					totalHours: '20',
					weeklyUnit: 'periods_per_week',
					weeklyValue: '1'
				},
				displayOrder: 1
			}
		],
		validation: { blockers: [], warnings: [] }
	};
}

interface MockOptions {
	permissions?: string[];
	cloneConflictsOnce?: boolean;
	versionsGate?: Promise<void>;
	curriculumGate?: Promise<void>;
	structureGate?: Promise<void>;
	firstStructureGate?: Promise<void>;
	secondStructureGate?: Promise<void>;
	includeSecondVersion?: boolean;
	failStructureOnce?: boolean;
}

async function mockShell(page: Page, options: MockOptions = {}) {
	const workspaceQueries: URLSearchParams[] = [];
	const academicRequests: string[] = [];
	let cloneBody: Record<string, unknown> | null = null;
	let cloneAttempts = 0;
	let createOptionsRequests = 0;
	let managementOptionsRequests = 0;
	let structureRequests = 0;
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const method = route.request().method();
			if (url.pathname.startsWith('/api/academic/')) {
				academicRequests.push(`${method} ${url.pathname}${url.search}`);
			}
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: ids.user,
					username: 'curriculum-alignment-test',
					firstName: 'หลักสูตร',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					profileImageFileId: null,
					permissions: options.permissions ?? ['*']
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
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active',
							startDate: '2026-05-01',
							endDate: '2027-03-31'
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
							startDate: '2026-05-01',
							endDate: '2026-10-31',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				});
				return;
			}
			if (url.pathname === '/api/academic/delivery/homerooms') {
				workspaceQueries.push(new URLSearchParams(url.searchParams));
				await fulfill(route, homeroomWorkspace());
				return;
			}
			if (url.pathname === `/api/academic/curricula/${ids.curriculum}` && method === 'GET') {
				if (options.curriculumGate) await options.curriculumGate;
				await fulfill(route, {
					id: ids.curriculum,
					code: 'CURR-2569',
					nameTh: 'หลักสูตรสถานศึกษา 2569',
					nameEn: null,
					description: null,
					gradeLevelIds: [ids.grade],
					owningOrganizationUnitId: null,
					isActive: true,
					rowVersion: 1,
					createdAt: '2026-04-01T00:00:00Z',
					updatedAt: '2026-08-30T00:00:00Z'
				});
				return;
			}
			if (
				url.pathname === `/api/academic/curricula/${ids.curriculum}/versions` &&
				method === 'GET'
			) {
				if (options.versionsGate) await options.versionsGate;
				await fulfill(route, [
					{
						version: curriculumVersion(ids.curriculumVersion, 'published', 'ฉบับ 2569'),
						startAcademicYearName: 'ปีการศึกษา 2569',
						endAcademicYearName: null
					},
					...(options.includeSecondVersion
						? [
								{
									version: curriculumVersion(
										ids.clonedVersion,
										'draft',
										'ฉบับ 2570',
										ids.futureYear
									),
									startAcademicYearName: 'ปีการศึกษา 2570',
									endAcademicYearName: null
								}
							]
						: [])
				]);
				return;
			}
			if (
				url.pathname.startsWith('/api/academic/curriculum-versions/') &&
				url.pathname.endsWith('/structure') &&
				method === 'GET'
			) {
				structureRequests += 1;
				if (options.failStructureOnce && structureRequests === 1) {
					await fulfill(route, 'โครงสร้างยังไม่พร้อม', 503);
					return;
				}
				if (options.structureGate) await options.structureGate;
				const versionId = url.pathname.split('/')[4] ?? '';
				if (versionId !== ids.curriculumVersion && versionId !== ids.clonedVersion) {
					await fulfill(route, 'ไม่พบรุ่นหลักสูตร', 404);
					return;
				}
				if (versionId === ids.curriculumVersion && options.firstStructureGate)
					await options.firstStructureGate;
				if (versionId === ids.clonedVersion && options.secondStructureGate)
					await options.secondStructureGate;
				const version =
					versionId === ids.clonedVersion
						? curriculumVersion(ids.clonedVersion, 'draft', 'ฉบับ 2570', ids.futureYear)
						: curriculumVersion(ids.curriculumVersion, 'published', 'ฉบับ 2569');
				await fulfill(route, curriculumStructure(version));
				return;
			}
			if (url.pathname === '/api/academic/curricula/management-options' && method === 'GET') {
				createOptionsRequests += 1;
				await fulfill(route, {
					academicYears: [
						{ id: ids.futureYear, name: 'ปีการศึกษา 2570', year: 2570, status: 'planning' },
						{ id: ids.year, name: 'ปีการศึกษา 2569', year: 2569, status: 'active' }
					],
					gradeLevels: [],
					ownerOptions: []
				});
				return;
			}
			if (
				url.pathname.endsWith('/management-options') &&
				url.pathname.includes('/api/academic/curriculum-versions/') &&
				method === 'GET'
			) {
				managementOptionsRequests += 1;
				await fulfill(route, { academicYears: [], gradeLevels: [], catalogVersions: [] });
				return;
			}
			if (
				url.pathname === `/api/academic/curriculum-versions/${ids.curriculumVersion}/clone-draft` &&
				method === 'POST'
			) {
				cloneAttempts += 1;
				cloneBody = route.request().postDataJSON() as Record<string, unknown>;
				if (options.cloneConflictsOnce && cloneAttempts === 1) {
					await fulfill(route, 'ข้อมูลรุ่นต้นทางถูกแก้ไขโดยผู้ใช้อื่น กรุณาโหลดข้อมูลล่าสุด', 409);
					return;
				}
				await fulfill(
					route,
					curriculumVersion(
						ids.clonedVersion,
						'draft',
						String(cloneBody.versionName),
						ids.futureYear
					),
					201
				);
				return;
			}
			if (url.pathname === '/api/academic/term-change-sets') {
				await fulfill(route, []);
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
			if (url.pathname === '/api/notifications/stream') {
				await route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
				return;
			}
			await fulfill(route, {});
		}
	);
	return {
		workspaceQueries,
		academicRequests,
		cloneRequest: () => cloneBody,
		cloneAttemptCount: () => cloneAttempts,
		createOptionsRequestCount: () => createOptionsRequests,
		managementOptionsRequestCount: () => managementOptionsRequests,
		structureRequestCount: () => structureRequests
	};
}

test('delivery requests and links the exact selected timetable version without row fan-out', async ({
	page
}) => {
	const mocked = await mockShell(page);
	await page.goto(
		`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&timetableVersionId=${ids.timetableVersion}`
	);

	await expect(page.getByText('คาบจริงต่างจากค่ามาตรฐานในหลักสูตร')).toBeVisible();
	await expect(page.getByText('วิทยาศาสตร์เสริม')).toBeVisible();
	await expect(page.getByRole('link', { name: /ตรวจในหลักสูตร/ })).toHaveAttribute(
		'href',
		`/staff/academic/curricula/${ids.curriculum}?versionId=${ids.curriculumVersion}&academicYearId=${ids.year}&academicTermId=${ids.term}&studyProgramId=${ids.program}&timetableVersionId=${ids.timetableVersion}`
	);
	expect(mocked.workspaceQueries).toHaveLength(1);
	expect(mocked.workspaceQueries[0]?.get('academicYearId')).toBe(ids.year);
	expect(mocked.workspaceQueries[0]?.get('academicTermId')).toBe(ids.term);
	expect(mocked.workspaceQueries[0]?.get('timetableVersionId')).toBe(ids.timetableVersion);
	expect(
		mocked.academicRequests.filter(
			(request) => request.includes('/offerings/') || request.includes('/learning-groups')
		)
	).toEqual([]);
});

test('read-only curriculum context inspects one workspace without management or row requests', async ({
	page
}) => {
	const mocked = await mockShell(page, {
		permissions: ['academic_curriculum.read.school', 'learning_offering.read.school']
	});
	await page.goto(
		`/staff/academic/curricula/${ids.curriculum}?versionId=${ids.curriculumVersion}&academicYearId=${ids.year}&academicTermId=${ids.term}&studyProgramId=${ids.program}&timetableVersionId=${ids.timetableVersion}`
	);

	await expect(page.getByRole('heading', { name: 'เทียบการเปิดสอนกับหลักสูตร' })).toBeVisible();
	await expect(page.getByText('คาบจริงต่างจากค่ามาตรฐานในหลักสูตร')).toBeVisible();
	await expect(page.getByText('วิทยาศาสตร์เสริม')).toBeVisible();
	await expect(page.getByRole('button', { name: 'สร้างหลักสูตรรุ่นใหม่แบบร่าง' })).toHaveCount(0);
	expect(mocked.workspaceQueries).toHaveLength(1);
	expect(mocked.workspaceQueries[0]?.get('timetableVersionId')).toBe(ids.timetableVersion);
	expect(mocked.createOptionsRequestCount()).toBe(0);
	expect(mocked.managementOptionsRequestCount()).toBe(0);
	expect(
		mocked.academicRequests.filter(
			(request) => request.includes('/offerings/') || request.includes('/learning-groups')
		)
	).toEqual([]);
});

test('curriculum detail starts version structure before the version list resolves', async ({
	page
}) => {
	const versionsGate = deferred();
	const structureGate = deferred();
	const mocked = await mockShell(page, {
		versionsGate: versionsGate.promise,
		structureGate: structureGate.promise
	});
	try {
		await page.goto(
			`/staff/academic/curricula/${ids.curriculum}?versionId=${ids.curriculumVersion}`
		);
		await expect
			.poll(
				() =>
					mocked.academicRequests.filter((request) =>
						request.includes(`/curriculum-versions/${ids.curriculumVersion}/structure`)
					).length
			)
			.toBe(1);
		await expect(page.getByRole('heading', { name: 'หลักสูตรสถานศึกษา 2569' })).toBeVisible();
		await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
		expect(mocked.createOptionsRequestCount()).toBe(0);
		expect(mocked.managementOptionsRequestCount()).toBe(0);
	} finally {
		versionsGate.release();
		structureGate.release();
	}
	await expect(page.getByRole('button', { name: /ฉบับ 2569/ })).toBeVisible();
	expect(
		mocked.academicRequests.filter((request) =>
			request.includes(`/curriculum-versions/${ids.curriculumVersion}/structure`)
		)
	).toHaveLength(1);
});

test('a slow curriculum summary does not hide a ready structure region', async ({ page }) => {
	const curriculumGate = deferred();
	await mockShell(page, { curriculumGate: curriculumGate.promise });
	try {
		await page.goto(
			`/staff/academic/curricula/${ids.curriculum}?versionId=${ids.curriculumVersion}`
		);
		await expect(page.getByRole('heading', { name: 'ภาพรวมทุกแผนการเรียน' })).toBeVisible();
		await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
	} finally {
		curriculumGate.release();
	}
	await expect(
		page.getByTestId('curriculum-detail-ready').getByRole('heading', {
			name: 'หลักสูตรสถานศึกษา 2569'
		})
	).toBeVisible();
});

test('an explicit version structure renders before version-list metadata', async ({ page }) => {
	const versionsGate = deferred();
	await mockShell(page, { versionsGate: versionsGate.promise });
	try {
		await page.goto(
			`/staff/academic/curricula/${ids.curriculum}?versionId=${ids.curriculumVersion}`
		);
		await expect(page.getByRole('heading', { name: 'ภาพรวมทุกแผนการเรียน' })).toBeVisible();
		await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
	} finally {
		versionsGate.release();
	}
	await expect(page.getByRole('button', { name: /ฉบับ 2569/ })).toBeVisible();
});

test('an invalid deep-linked version falls back to the first authorized version', async ({
	page
}) => {
	const invalidVersionId = '52000000-0000-4000-8000-000000000499';
	const mocked = await mockShell(page);
	await page.goto(`/staff/academic/curricula/${ids.curriculum}?versionId=${invalidVersionId}`);
	await expect(page).toHaveURL(new RegExp(`versionId=${ids.curriculumVersion}`));
	await expect(page.getByRole('heading', { name: 'ภาพรวมทุกแผนการเรียน' })).toBeVisible();
	expect(mocked.structureRequestCount()).toBe(2);
});

test('curriculum detail waits only for the version identifier when the URL omits it', async ({
	page
}) => {
	const versionsGate = deferred();
	const mocked = await mockShell(page, { versionsGate: versionsGate.promise });
	try {
		await page.goto(`/staff/academic/curricula/${ids.curriculum}`);
		await expect(page.getByRole('heading', { name: 'หลักสูตรสถานศึกษา 2569' })).toBeVisible();
		expect(mocked.structureRequestCount()).toBe(0);
		await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
	} finally {
		versionsGate.release();
	}
	await expect(page.getByRole('button', { name: /ฉบับ 2569/ })).toBeVisible();
	await expect.poll(mocked.structureRequestCount).toBe(1);
	await expect(page).toHaveURL(new RegExp(`versionId=${ids.curriculumVersion}`));
});

test('curriculum structure failure retries only the structure region', async ({ page }) => {
	const mocked = await mockShell(page, { failStructureOnce: true });
	await page.goto(`/staff/academic/curricula/${ids.curriculum}?versionId=${ids.curriculumVersion}`);
	await expect(page.getByText('โครงสร้างยังไม่พร้อม')).toBeVisible();
	const curriculumReads = mocked.academicRequests.filter(
		(request) => request === `GET /api/academic/curricula/${ids.curriculum}`
	).length;
	const versionReads = mocked.academicRequests.filter((request) =>
		request.endsWith(`/api/academic/curricula/${ids.curriculum}/versions`)
	).length;
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByRole('heading', { name: 'ภาพรวมทุกแผนการเรียน' })).toBeVisible();
	expect(mocked.structureRequestCount()).toBe(2);
	expect(
		mocked.academicRequests.filter(
			(request) => request === `GET /api/academic/curricula/${ids.curriculum}`
		).length
	).toBe(curriculumReads);
	expect(
		mocked.academicRequests.filter((request) =>
			request.endsWith(`/api/academic/curricula/${ids.curriculum}/versions`)
		).length
	).toBe(versionReads);
});

test('switching versions clears the old workspace and shallow history restores it', async ({
	page
}) => {
	const secondGate = deferred();
	const mocked = await mockShell(page, {
		includeSecondVersion: true,
		secondStructureGate: secondGate.promise
	});
	await page.goto(`/staff/academic/curricula/${ids.curriculum}?versionId=${ids.curriculumVersion}`);
	await expect(page.getByRole('heading', { name: 'ภาพรวมทุกแผนการเรียน' })).toBeVisible();
	try {
		await page.getByRole('button', { name: /ฉบับ 2570/ }).click();
		await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
		await expect(page.getByRole('heading', { name: 'ภาพรวมทุกแผนการเรียน' })).toHaveCount(0);
	} finally {
		secondGate.release();
	}
	await expect(page).toHaveURL(new RegExp(`versionId=${ids.clonedVersion}`));
	await expect(page.getByRole('heading', { name: 'ภาพรวมทุกแผนการเรียน' })).toBeVisible();
	await page.goBack();
	await expect(page).toHaveURL(new RegExp(`versionId=${ids.curriculumVersion}`));
	await expect
		.poll(
			() =>
				mocked.academicRequests.filter((request) =>
					request.includes(`/curriculum-versions/${ids.curriculumVersion}/structure`)
				).length
		)
		.toBe(2);
});

test('a late first-version route response cannot replace a newly selected version', async ({
	page
}) => {
	const firstGate = deferred();
	const firstResponse = page.waitForResponse(
		(response) =>
			new URL(response.url()).pathname ===
			`/api/academic/curriculum-versions/${ids.curriculumVersion}/structure`
	);
	const mocked = await mockShell(page, {
		includeSecondVersion: true,
		firstStructureGate: firstGate.promise
	});
	try {
		await page.goto(
			`/staff/academic/curricula/${ids.curriculum}?versionId=${ids.curriculumVersion}`
		);
		await expect(page.getByRole('button', { name: /ฉบับ 2570/ })).toBeVisible();
		await page.getByRole('button', { name: /ฉบับ 2570/ }).click();
		await expect(page).toHaveURL(new RegExp(`versionId=${ids.clonedVersion}`));
		await expect(page.getByRole('button', { name: 'เผยแพร่รุ่นหลักสูตร' })).toBeVisible();
	} finally {
		firstGate.release();
	}
	await firstResponse;
	await expect.poll(() => mocked.structureRequestCount()).toBe(2);
	await expect(page.getByRole('button', { name: 'จัดโครงสร้าง' })).toBeVisible();
});

test('manager clones the published source into a selected future draft and keeps the source visible', async ({
	page
}) => {
	const mocked = await mockShell(page);
	await page.goto(`/staff/academic/curricula/${ids.curriculum}?versionId=${ids.curriculumVersion}`);

	await page.getByRole('button', { name: 'สร้างหลักสูตรรุ่นใหม่แบบร่าง' }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('ต้นฉบับที่เผยแพร่จะไม่เปลี่ยน')).toBeVisible();
	await expect(dialog.getByText('ปีการศึกษา 2570')).toBeVisible();
	await dialog.getByLabel('ชื่อรุ่น').fill('ฉบับ 2570');
	await dialog.getByRole('button', { name: 'สร้างแบบร่าง' }).click();

	await expect(page.getByRole('button', { name: /ฉบับ 2570/ })).toBeVisible();
	await expect(page.getByRole('button', { name: /ฉบับ 2569/ })).toBeVisible();
	await expect(page.getByRole('button', { name: 'เผยแพร่รุ่นหลักสูตร' })).toBeVisible();
	expect(mocked.cloneRequest()).toEqual({
		versionName: 'ฉบับ 2570',
		startAcademicYearId: ids.futureYear,
		endAcademicYearId: null,
		description: null,
		sourceRowVersion: 4
	});
	expect(mocked.cloneAttemptCount()).toBe(1);
	expect(mocked.createOptionsRequestCount()).toBe(1);
});

test('stale clone keeps the draft intact and succeeds on an explicit retry', async ({ page }) => {
	const mocked = await mockShell(page, { cloneConflictsOnce: true });
	await page.goto(`/staff/academic/curricula/${ids.curriculum}?versionId=${ids.curriculumVersion}`);

	await page.getByRole('button', { name: 'สร้างหลักสูตรรุ่นใหม่แบบร่าง' }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('ชื่อรุ่น').fill('ฉบับ 2570 ปรับปรุง');
	await dialog.getByRole('button', { name: 'สร้างแบบร่าง' }).click();

	await expect(dialog.getByRole('alert')).toContainText('กรุณาโหลดข้อมูลล่าสุด');
	await expect(dialog.getByLabel('ชื่อรุ่น')).toHaveValue('ฉบับ 2570 ปรับปรุง');
	await dialog.getByRole('button', { name: 'สร้างแบบร่าง' }).click();

	await expect(page.getByRole('button', { name: /ฉบับ 2570 ปรับปรุง/ })).toBeVisible();
	await expect(page.getByRole('button', { name: /ฉบับ 2569/ })).toBeVisible();
	expect(mocked.cloneAttemptCount()).toBe(2);
	expect(mocked.cloneRequest()).toMatchObject({
		versionName: 'ฉบับ 2570 ปรับปรุง',
		sourceRowVersion: 4
	});
});
