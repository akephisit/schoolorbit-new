import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

const ids = {
	year: '12000000-0000-4000-8000-000000000401',
	futureYear: '12000000-0000-4000-8000-000000000402',
	term: '22000000-0000-4000-8000-000000000401',
	deliveryVersion: '32000000-0000-4000-8000-000000000401',
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
		deliveryVersionId: ids.deliveryVersion,
		deliveryVersionStatus: 'published',
		deliveryVersionEffectiveFrom: '2026-08-15',
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
					editionId: ids.curriculum,
					editionName: 'ฉบับปรับปรุง พุทธศักราช 2569',
					curriculumLevelId: ids.curriculumVersion,
					levelName: 'ระดับมัธยมศึกษาตอนต้น'
				},
				curriculumLevelId: ids.curriculumVersion,
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

function edition(id = ids.curriculum, status: 'draft' | 'published' = 'published') {
	return {
		id,
		name: `ฉบับปรับปรุง พุทธศักราช ${status === 'draft' ? 2570 : 2569}`,
		revisionYear: status === 'draft' ? 2570 : 2569,
		description: null,
		status,
		isActive: true,
		rowVersion: 4,
		migrated: false,
		publishedAt: status === 'published' ? '2026-05-01T00:00:00Z' : null,
		createdAt: '2026-04-01T00:00:00Z',
		updatedAt: '2026-08-30T00:00:00Z'
	};
}
function curriculumVersion(id: string, status: 'draft' | 'published' = 'published') {
	return {
		id,
		editionId: status === 'draft' ? ids.futureYear : ids.curriculum,
		editionName: edition(ids.curriculum, status).name,
		revisionYear: status === 'draft' ? 2570 : 2569,
		code: 'LEVEL-M1',
		nameTh: 'ระดับมัธยมศึกษาตอนต้น',
		nameEn: null,
		gradeLevelIds: [ids.grade],
		description: null,
		status,
		isActive: true,
		rowVersion: 4,
		migrated: false,
		createdAt: '2026-04-01T00:00:00Z',
		updatedAt: '2026-08-30T00:00:00Z'
	};
}

function curriculumStructure(version = curriculumVersion(ids.curriculumVersion)) {
	return {
		level: version,
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
				curriculumLevelId: version.id,
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
				curriculumLevelId: version.id,
				code: 'DEFAULT',
				nameTh: 'แผนการเรียนพื้นฐาน',
				nameEn: null,
				isDefault: true,
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
	let levelBody: Record<string, unknown> | null = null;
	let programBody: Record<string, unknown> | null = null;
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
			if (url.pathname === '/api/academic/delivery-versions') {
				await fulfill(route, [
					{
						id: ids.deliveryVersion,
						academicYearId: ids.year,
						academicTermId: ids.term,
						sourceVersionId: null,
						changeSetId: null,
						effectiveFrom: '2026-08-15',
						effectiveUntil: null,
						status: 'published',
						rowVersion: 1,
						offeringCount: 2,
						groupCount: 1,
						teacherAssignmentCount: 1,
						updatedAt: '2026-08-15T00:00:00Z'
					}
				]);
				return;
			}
			if (url.pathname === `/api/academic/delivery-versions/${ids.deliveryVersion}`) {
				await fulfill(route, {
					id: ids.deliveryVersion,
					academicYearId: ids.year,
					academicTermId: ids.term,
					sourceVersionId: null,
					effectiveFrom: '2026-08-15',
					effectiveUntil: null,
					status: 'published',
					rowVersion: 1,
					createdBy: ids.user,
					publishedBy: ids.user,
					createdAt: '2026-08-15T00:00:00Z',
					updatedAt: '2026-08-15T00:00:00Z',
					publishedAt: '2026-08-15T00:00:00Z',
					snapshot: { offerings: [] }
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
				await fulfill(route, edition());
				return;
			}
			if (url.pathname === `/api/academic/curricula/${ids.futureYear}` && method === 'GET') {
				await fulfill(route, edition(ids.futureYear, 'draft'));
				return;
			}
			if (url.pathname === `/api/academic/curricula/${ids.curriculum}/levels` && method === 'GET') {
				if (options.versionsGate) await options.versionsGate;
				await fulfill(route, [{ level: curriculumVersion(ids.curriculumVersion) }]);
				return;
			}
			if (url.pathname === `/api/academic/curricula/${ids.futureYear}/levels`) {
				if (method === 'POST') {
					levelBody = route.request().postDataJSON();
					await fulfill(
						route,
						{ ...curriculumVersion(ids.clonedVersion, 'draft'), ...levelBody },
						201
					);
				} else await fulfill(route, []);
				return;
			}
			if (
				url.pathname === `/api/academic/curriculum-levels/${ids.clonedVersion}/programs` &&
				method === 'POST'
			) {
				programBody = route.request().postDataJSON();
				await fulfill(
					route,
					{
						...curriculumStructure().programs[0],
						...programBody,
						status: 'draft',
						curriculumLevelId: ids.clonedVersion
					},
					201
				);
				return;
			}
			if (url.pathname === '/api/academic/curricula/overview') {
				await fulfill(route, {
					items: [
						{ edition: edition(), levelCount: 1, studyProgramCount: 1 },
						{ edition: edition(ids.futureYear, 'draft'), levelCount: 1, studyProgramCount: 0 }
					]
				});
				return;
			}
			if (url.pathname === `/api/academic/curriculum-levels/${ids.curriculumVersion}/programs`) {
				await fulfill(route, curriculumStructure().programs);
				return;
			}
			if (
				url.pathname.startsWith('/api/academic/curriculum-levels/') &&
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
					await fulfill(route, 'ไม่พบระดับการศึกษา', 404);
					return;
				}
				if (versionId === ids.curriculumVersion && options.firstStructureGate)
					await options.firstStructureGate;
				if (versionId === ids.clonedVersion && options.secondStructureGate)
					await options.secondStructureGate;
				const version =
					versionId === ids.clonedVersion
						? curriculumVersion(ids.clonedVersion, 'draft')
						: curriculumVersion(ids.curriculumVersion);
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
					gradeLevels: []
				});
				return;
			}
			if (
				url.pathname.endsWith('/management-options') &&
				url.pathname.includes('/api/academic/curriculum-levels/') &&
				method === 'GET'
			) {
				managementOptionsRequests += 1;
				await fulfill(route, { academicYears: [], gradeLevels: [], catalogVersions: [] });
				return;
			}
			if (
				url.pathname === `/api/academic/curriculum-levels/${ids.clonedVersion}/copy-program` &&
				method === 'POST'
			) {
				cloneAttempts++;
				cloneBody = route.request().postDataJSON() as Record<string, unknown>;
				if (options.cloneConflictsOnce && cloneAttempts === 1) {
					await fulfill(route, 'ข้อมูลต้นทางเปลี่ยน กรุณาโหลดข้อมูลล่าสุด', 409);
					return;
				}
				await fulfill(
					route,
					{
						...curriculumStructure().programs[0],
						id: ids.clonedVersion,
						curriculumLevelId: ids.clonedVersion,
						status: 'draft'
					},
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
		structureRequestCount: () => structureRequests,
		levelRequest: () => levelBody,
		programRequest: () => programBody
	};
}

const levelPath = `/staff/academic/curricula/${ids.curriculum}/levels/${ids.curriculumVersion}`;
const alignmentQuery = `academicYearId=${ids.year}&academicTermId=${ids.term}&studyProgramId=${ids.program}&deliveryVersionId=${ids.deliveryVersion}`;

test('delivery links the selected opening to the exact edition and educational level without row fan-out', async ({
	page
}) => {
	const mocked = await mockShell(page);
	await page.goto(`/staff/academic/delivery?${alignmentQuery}`);
	await expect(page.getByText('คาบจริงต่างจากค่ามาตรฐานในหลักสูตร')).toBeVisible();
	await expect(page.getByText('วิทยาศาสตร์เสริม')).toBeVisible();
	await expect(page.getByRole('link', { name: /ตรวจในหลักสูตร/ })).toHaveAttribute(
		'href',
		`${levelPath}?${alignmentQuery}`
	);
	expect(mocked.workspaceQueries).toHaveLength(1);
	expect(mocked.workspaceQueries[0]?.get('deliveryVersionId')).toBe(ids.deliveryVersion);
	expect(
		mocked.academicRequests.filter(
			(r) => r.includes('/offerings/') || r.includes('/learning-groups')
		)
	).toEqual([]);
});

test('read-only level context inspects the selected opening without loading management options', async ({
	page
}) => {
	const mocked = await mockShell(page, {
		permissions: ['academic_curriculum.read.school', 'learning_offering.read.school']
	});
	await page.goto(`${levelPath}?${alignmentQuery}`);
	await expect(page.getByRole('heading', { name: 'เทียบการเปิดสอนกับหลักสูตร' })).toBeVisible();
	await expect(page.getByText('วิทยาศาสตร์เสริม')).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่มแผนการเรียน' })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'คัดลอกแผนจากฉบับเดิม' })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'จัดโครงสร้าง' })).toHaveCount(0);
	expect(mocked.workspaceQueries).toHaveLength(1);
	expect(mocked.managementOptionsRequestCount()).toBe(0);
	expect(mocked.createOptionsRequestCount()).toBe(0);
});

test('a slow edition summary does not hide the ready educational-level structure', async ({
	page
}) => {
	const gate = deferred();
	await mockShell(page, { curriculumGate: gate.promise });
	try {
		await page.goto(levelPath);
		await expect(page.getByRole('heading', { name: 'ภาพรวมทุกแผนการเรียน' })).toBeVisible();
		await expect(page.getByTestId('curriculum-level-ready')).toBeVisible();
	} finally {
		gate.release();
	}
});

test('edition metadata renders while its independent educational-level list is loading', async ({
	page
}) => {
	const gate = deferred();
	const mocked = await mockShell(page, { versionsGate: gate.promise });
	try {
		await page.goto(`/staff/academic/curricula/${ids.curriculum}`);
		await expect(page.getByTestId('curriculum-edition-ready')).toBeVisible();
		await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
		expect(mocked.structureRequestCount()).toBe(0);
	} finally {
		gate.release();
	}
	await expect(page.getByRole('link', { name: 'ระดับมัธยมศึกษาตอนต้น' })).toHaveAttribute(
		'href',
		levelPath
	);
});

test('an invalid level deep link refuses rather than silently switching to another level', async ({
	page
}) => {
	const mocked = await mockShell(page);
	const invalid = '52000000-0000-4000-8000-000000000499';
	await page.goto(`/staff/academic/curricula/${ids.curriculum}/levels/${invalid}`);
	await expect(page.getByText('ไม่พบระดับการศึกษา', { exact: true })).toBeVisible();
	await expect(page).toHaveURL(new RegExp(invalid));
	expect(mocked.structureRequestCount()).toBe(1);
});

test('a level under the wrong edition is refused', async ({ page }) => {
	await mockShell(page);
	await page.goto(`/staff/academic/curricula/${ids.futureYear}/levels/${ids.curriculumVersion}`);
	await expect(page.getByText('ระดับการศึกษาไม่อยู่ในฉบับที่เลือก', { exact: true })).toBeVisible();
	await expect(page.getByTestId('curriculum-level-ready')).toHaveCount(0);
});

test('structure retry requests only the failed region', async ({ page }) => {
	const mocked = await mockShell(page, { failStructureOnce: true });
	await page.goto(levelPath);
	await expect(page.getByText('โครงสร้างยังไม่พร้อม', { exact: true })).toBeVisible();
	const reads = mocked.academicRequests.filter(
		(r) => r === `GET /api/academic/curricula/${ids.curriculum}`
	).length;
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByRole('heading', { name: 'ภาพรวมทุกแผนการเรียน' })).toBeVisible();
	expect(mocked.structureRequestCount()).toBe(2);
	expect(
		mocked.academicRequests.filter((r) => r === `GET /api/academic/curricula/${ids.curriculum}`)
	).toHaveLength(reads);
});

for (const mobile of [false, true])
	for (const dark of [false, true]) {
		test(`copy one published plan into a selected draft level (${mobile ? 'mobile' : 'desktop'}, ${dark ? 'dark' : 'light'})`, async ({
			page
		}) => {
			await page.setViewportSize({ width: mobile ? 390 : 1440, height: 900 });
			const mocked = await mockShell(page);
			await page.goto(`/staff/academic/curricula/${ids.futureYear}/levels/${ids.clonedVersion}`);
			await expect(page.getByTestId('curriculum-level-ready')).toBeVisible();
			if (dark) {
				await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
				await expect(page.locator('html')).toHaveClass(/dark/);
			}
			expect(mocked.createOptionsRequestCount()).toBe(0);
			await page.getByRole('button', { name: 'คัดลอกแผนจากฉบับเดิม' }).click();
			const dialog = page.getByRole('dialog');
			await dialog.getByLabel('ฉบับต้นทาง *', { exact: true }).click();
			await expect(page.getByRole('option')).toHaveCount(1);
			await page.getByRole('option', { name: edition().name, exact: true }).click();
			await dialog.getByLabel('ระดับการศึกษาต้นทาง *', { exact: true }).click();
			await page.getByRole('option', { name: 'ระดับมัธยมศึกษาตอนต้น', exact: true }).click();
			await dialog.getByLabel('แผนต้นทาง *', { exact: true }).click();
			await page.getByRole('option', { name: 'แผนการเรียนพื้นฐาน', exact: true }).click();
			await dialog.getByLabel('ชื่อแผนในฉบับใหม่ (ถ้าต้องการเปลี่ยน)').fill('แผนใหม่');
			await expect.poll(() => dialog.evaluate((e) => e.scrollWidth <= e.clientWidth)).toBe(true);
			await page.screenshot({
				path: `test-results/curriculum-copy-${mobile ? 'mobile' : 'desktop'}-${dark ? 'dark' : 'light'}.png`
			});
			await dialog.getByRole('button', { name: 'คัดลอกเป็นแผนร่าง', exact: true }).click();
			await expect(dialog).toBeHidden();
			expect(mocked.cloneRequest()).toEqual({
				sourceProgramId: ids.program,
				sourceRowVersion: 1,
				destinationRowVersion: 4,
				nameTh: 'แผนใหม่'
			});
			expect(mocked.cloneAttemptCount()).toBe(1);
			expect(
				mocked.academicRequests.filter((r) => r.startsWith('POST') || r.startsWith('PATCH'))
			).toEqual([`POST /api/academic/curriculum-levels/${ids.clonedVersion}/copy-program`]);
		});
	}

test('copy conflict keeps the draft dialog open without reporting a saved plan', async ({
	page
}) => {
	const mocked = await mockShell(page, { cloneConflictsOnce: true });
	await page.goto(`/staff/academic/curricula/${ids.futureYear}/levels/${ids.clonedVersion}`);
	await page.getByRole('button', { name: 'คัดลอกแผนจากฉบับเดิม' }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('ฉบับต้นทาง *', { exact: true }).click();
	await page.getByRole('option', { name: edition().name, exact: true }).click();
	await dialog.getByLabel('ระดับการศึกษาต้นทาง *', { exact: true }).click();
	await page.getByRole('option', { name: 'ระดับมัธยมศึกษาตอนต้น', exact: true }).click();
	await dialog.getByLabel('แผนต้นทาง *', { exact: true }).click();
	await page.getByRole('option', { name: 'แผนการเรียนพื้นฐาน', exact: true }).click();
	await dialog.getByRole('button', { name: 'คัดลอกเป็นแผนร่าง', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText(
		'ข้อมูลต้นทางเปลี่ยน กรุณาโหลดข้อมูลล่าสุด'
	);
	await expect(dialog).toBeVisible();
	expect(mocked.cloneAttemptCount()).toBe(1);
});

test('create a level with covered grades within an edition, then create its plan without owner fields', async ({
	page
}) => {
	const mocked = await mockShell(page);
	await page.goto(`/staff/academic/curricula/${ids.futureYear}`);
	await expect(page.getByTestId('curriculum-edition-ready')).toBeVisible();
	expect(mocked.createOptionsRequestCount()).toBe(0);
	await page.getByRole('button', { name: 'เพิ่มระดับการศึกษา', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('ชื่อระดับการศึกษา *').fill('ระดับมัธยมศึกษาตอนต้น');
	await page.getByRole('combobox', { name: 'เลือกระดับชั้นที่ครอบคลุม' }).click();
	await page.getByRole('button', { name: 'เลือกทั้งหมด', exact: true }).click();
	await page.keyboard.press('Escape');
	await dialog.getByRole('button', { name: 'เพิ่มระดับการศึกษา', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(mocked.levelRequest()).toEqual({
		nameTh: 'ระดับมัธยมศึกษาตอนต้น',
		gradeLevelIds: [ids.grade],
		description: null
	});
	expect(mocked.createOptionsRequestCount()).toBe(1);
	await page.getByRole('link', { name: 'ระดับมัธยมศึกษาตอนต้น', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/levels/${ids.clonedVersion}`));
	await page.getByRole('button', { name: 'เพิ่มแผนการเรียน', exact: true }).click();
	await dialog.getByLabel('ชื่อแผนการเรียน *').fill('วิทยาศาสตร์-คณิตศาสตร์');
	await expect(dialog.getByLabel('รหัสหลักสูตร')).toHaveCount(0);
	await expect(dialog.getByLabel('ชื่อภาษาอังกฤษ')).toHaveCount(0);
	await expect(dialog.getByLabel('หน่วยงานเจ้าของหลักสูตร')).toHaveCount(0);
	await dialog.getByRole('button', { name: 'สร้างแผนการเรียน', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(mocked.programRequest()).toEqual({ nameTh: 'วิทยาศาสตร์-คณิตศาสตร์', isDefault: false });
	expect(mocked.managementOptionsRequestCount()).toBe(0);
});
