import { sessionState } from './fixtures/session-state';
import { expect, test, type Page, type Route } from '@playwright/test';
import {
	ids,
	edition,
	curriculumVersion,
	curriculumStructure
} from './helpers/curriculum-fixtures';

test.use({ serviceWorkers: 'block' });
// Every case owns its page and mock graph; a failure must not suppress independent coverage.
test.describe.configure({ mode: 'default' });

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
		rowVersion: 1,
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
						offeringState: 'present',
						groupMode: 'normal',
						teacherState: 'assigned',
						timetableState: 'scheduled',
						groups: [
							{
								id: ids.group,
								code: 'M1-1-MATH',
								name: 'คณิตศาสตร์ ม.1/1',
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
						alignmentStates: ['extra_offering']
					}
				]
			}
		],
		unlinked: []
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
	includeDraftLevel?: boolean;
}

async function mockShell(page: Page, options: MockOptions = {}) {
	const workspaceQueries: URLSearchParams[] = [];
	const academicRequests: string[] = [];
	let cloneBody: Record<string, unknown> | null = null;
	let cloneAttempts = 0;
	let createOptionsRequests = 0;
	let managementOptionsRequests = 0;
	let structureRequests = 0;
	let publishBody: Record<string, unknown> | null = null;
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
					displayName: 'หลักสูตร ทดสอบ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
					profileImageFileId: null,
					permissions: options.permissions ?? ['*']
				});
				return;
			}
			if (url.pathname === '/api/academic/context/options') {
				await fulfill(route, {
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
							status: 'active',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				});
				return;
			}
			if (url.pathname === '/api/academic/delivery/workspace') {
				await fulfill(route, {
					academicYearId: ids.year,
					academicTermId: ids.term,
					rowVersion: 1,
					offerings: [],
					courseMetadata: []
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
			if (
				url.pathname === `/api/academic/curricula/${ids.futureYear}/publish` &&
				method === 'POST'
			) {
				publishBody = route.request().postDataJSON();
				await fulfill(route, {
					...edition(ids.futureYear, 'draft'),
					status: 'published',
					draftId: null,
					publicationCount: 1,
					currentPublicationId: 'f2000000-0000-4000-8000-000000000401',
					publishedAt: '2026-10-06T00:00:00Z',
					rowVersion: 5
				});
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
				} else
					await fulfill(
						route,
						options.includeDraftLevel
							? [{ level: curriculumVersion(ids.clonedVersion, 'draft') }]
							: []
					);
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
						{
							isVisible: true,
							id: ids.futureYear,
							name: 'ปีการศึกษา 2570',
							year: 2570,
							status: 'planning'
						},
						{
							isVisible: true,
							id: ids.year,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active'
						}
					],
					gradeLevels: curriculumStructure().gradeLevels
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
		programRequest: () => programBody,
		publishRequest: () => publishBody
	};
}

const levelPath = `/staff/academic/curricula/${ids.curriculum}/levels/${ids.curriculumVersion}`;
const alignmentQuery = `academicYearId=${ids.year}&academicTermId=${ids.term}&studyProgramId=${ids.program}`;

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
	expect(mocked.workspaceQueries[0]?.has('deliveryVersionId')).toBe(false);
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
				draftId: 'f2000000-0000-4000-8000-000000000402',
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
	await expect(page.getByRole('combobox', { name: 'เลือกระดับชั้นที่ครอบคลุม' })).toHaveText(
		'เลือกระดับชั้น'
	);
	await expect(
		dialog.getByRole('button', { name: 'เพิ่มระดับการศึกษา', exact: true })
	).toBeDisabled();
	await page.getByRole('combobox', { name: 'เลือกระดับชั้นที่ครอบคลุม' }).click();
	await expect(page.getByRole('button', { name: 'ทุกระดับชั้น', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'เลือกทั้งหมด', exact: true }).click();
	await page.getByRole('button', { name: 'ล้างการเลือก', exact: true }).click();
	await page.keyboard.press('Escape');
	await expect(
		dialog.getByRole('button', { name: 'เพิ่มระดับการศึกษา', exact: true })
	).toBeDisabled();
	await page.getByRole('combobox', { name: 'เลือกระดับชั้นที่ครอบคลุม' }).click();
	await page.getByRole('button', { name: 'เลือกทั้งหมด', exact: true }).click();
	await page.keyboard.press('Escape');
	await dialog.getByRole('button', { name: 'เพิ่มระดับการศึกษา', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(mocked.levelRequest()).toEqual({
		draftId: 'f2000000-0000-4000-8000-000000000402',
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
	expect(mocked.programRequest()).toEqual({
		draftId: 'f2000000-0000-4000-8000-000000000402',
		nameTh: 'วิทยาศาสตร์-คณิตศาสตร์'
	});
	expect(mocked.managementOptionsRequestCount()).toBe(0);
});

test('publishes the selected edition once and keeps its educational-level link', async ({
	page
}) => {
	const mocked = await mockShell(page, { includeDraftLevel: true });
	await page.goto(`/staff/academic/curricula/${ids.futureYear}`);
	await expect(
		page.getByRole('link', { name: 'ระดับมัธยมศึกษาตอนต้น', exact: true })
	).toBeVisible();
	await page.getByLabel('สรุปการแก้ไข *').fill('เผยแพร่ครั้งแรก');
	await page.getByRole('button', { name: 'เผยแพร่ทั้งฉบับ · ครั้งที่ 1', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'เผยแพร่ทั้งฉบับ · ครั้งที่ 1', exact: true })
	).toHaveCount(0);
	await expect(page.getByText('เผยแพร่ครั้งที่ 1', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่มระดับการศึกษา', exact: true })).toHaveCount(
		0
	);
	await expect(
		page.getByRole('link', { name: 'ระดับมัธยมศึกษาตอนต้น', exact: true })
	).toHaveAttribute(
		'href',
		`/staff/academic/curricula/${ids.futureYear}/levels/${ids.clonedVersion}`
	);
	expect(mocked.publishRequest()).toEqual({
		rowVersion: 4,
		draftId: 'f2000000-0000-4000-8000-000000000402',
		changeNote: 'เผยแพร่ครั้งแรก'
	});
	expect(mocked.academicRequests.filter((r) => r.startsWith('POST'))).toEqual([
		`POST /api/academic/curricula/${ids.futureYear}/publish`
	]);
});

import {
	mockDeliveryNavigation,
	navigationIds as navIds,
	navigationOfferingIds as navOfferings,
	navigationDelivery,
	navigationWorkspace
} from './fixtures/delivery-navigation-route-data';
import type {
	CurriculumOfferingPreview,
	ApplyCurriculumOfferingsResult
} from '../../src/lib/api/learning-delivery.js';

const comparisonCatalog = 'c0000000-0000-4000-8000-000000000001';
const comparisonOffering = 'c0000000-0000-4000-8000-000000000002';
async function mockSharedComparison(
	page: Page,
	options: {
		manager?: boolean;
		kind?: 'course' | 'activity';
		schedulingMode?: 'independent' | 'synchronized';
		failWorkspace?: boolean;
		wrongContext?: boolean;
		emptyCurrentOfferings?: boolean;
		failPreview?: boolean;
	} = {}
) {
	await mockDeliveryNavigation(page, { manager: options.manager, readiness: 'course' });
	const workspace = navigationWorkspace(),
		delivery = navigationDelivery();
	const kind = options.kind ?? 'course',
		schedulingMode = kind === 'activity' ? (options.schedulingMode ?? 'independent') : null;
	const oldId = kind === 'course' ? navOfferings.basic21 : navOfferings.club;
	const old = delivery.offerings.find((item) => item.id === oldId)!;
	old.code = kind === 'course' ? 'ว30104' : 'ACT-OLD';
	old.name = kind === 'course' ? 'วิทยาศาสตร์*' : 'กิจกรรมเดิม';
	for (const [index, room] of workspace.homerooms.entries()) {
		const source = room.items.find((item) => item.offeringId === oldId)!;
		room.items = [
			{
				...source,
				requirementId: `c0000000-0000-4000-8000-${String(index + 11).padStart(12, '0')}`,
				catalogVersionId: comparisonCatalog,
				code: kind === 'course' ? 'ว30104' : 'ACT-NEW',
				name: kind === 'course' ? 'วิทยาศาสตร์' : 'กิจกรรมใหม่',
				resourceKind: kind,
				subjectType: kind === 'course' ? 'BASIC' : null,
				schedulingMode,
				offeringId: null,
				offeringState: 'missing',
				groupMode: 'missing',
				teacherState: 'missing_primary',
				alignmentStates: ['curriculum_requirement_not_offered'],
				weeklyPeriodTarget: null,
				groups: []
			}
		];
		room.extraOfferings = [
			{
				offeringId: oldId,
				catalogVersionId:
					old.catalog.kind === 'course'
						? old.catalog.subjectVersionId
						: old.catalog.activityVersionId,
				code: old.code,
				name: old.name,
				resourceKind: kind,
				weeklyPeriodTarget: 2,
				alignmentStates: ['extra_offering']
			}
		];
		room.expectedCount = 1;
		room.readyCount = 0;
	}
	workspace.unlinked = [
		{
			offeringId: oldId,
			groupId: null,
			resourceKind: kind,
			code: old.code,
			name: old.name,
			reason: 'รายการเดิมยังเชื่อมห้องไม่ได้'
		}
	];
	if (options.emptyCurrentOfferings) {
		delivery.offerings = [];
		workspace.unlinked = [];
		for (const room of workspace.homerooms) room.extraOfferings = [];
	}
	let failWorkspace = options.failWorkspace ?? false,
		wrongContext = options.wrongContext ?? false,
		previewAttempts = 0;
	const academicWrites: { path: string; body: Record<string, unknown> }[] = [];
	const managementReads: string[] = [];
	page.on('request', (request) => {
		const url = new URL(request.url());
		if (url.pathname === '/api/academic/delivery/management-options')
			managementReads.push(url.pathname);
		if (
			['POST', 'PUT', 'PATCH', 'DELETE'].includes(request.method()) &&
			url.pathname.startsWith('/api/academic/')
		)
			academicWrites.push({ path: url.pathname, body: request.postDataJSON() });
	});
	await page.route('**/api/academic/delivery/homerooms?*', (route) => {
		if (failWorkspace) {
			failWorkspace = false;
			return fulfill(route, 'โหลดผลเทียบยังไม่สำเร็จ', 503);
		}
		return fulfill(
			route,
			wrongContext ? { ...workspace, academicTermId: navIds.activeTerm } : workspace
		);
	});
	await page.route('**/api/academic/delivery/workspace?*', (route) => fulfill(route, delivery));
	await page.route('**/api/academic/offerings/preview-from-curriculum', (route) => {
		previewAttempts++;
		if (options.failPreview && previewAttempts === 1)
			return fulfill(route, 'ฉบับทะเบียนที่เลือกไม่พร้อมใช้งาน', 409);
		return fulfill(route, {
			academicTermId: navIds.term,
			curriculumSources: [],
			sourceHash: 'e'.repeat(64),
			proposals: [
				{
					proposalId: 'missing-current-requirement',
					resourceKind: kind,
					catalogVersionId: comparisonCatalog,
					schedulingMode,
					code: kind === 'course' ? 'ว30104' : 'ACT-NEW',
					name: kind === 'course' ? 'วิทยาศาสตร์' : 'กิจกรรมใหม่',
					targetHomeroomIds: [navIds.roomA, navIds.roomB],
					requirementIds: workspace.homerooms.flatMap((room) =>
						room.items.map((item) => item.requirementId)
					),
					groupingState: 'proposed',
					offeringAction: 'create',
					conflicts: [],
					defaultGroups: workspace.homerooms.map((room, index) => ({
						groupKey: String(index + 1).repeat(64),
						name: room.homeroom.name,
						homeroomIds: [room.homeroom.id]
					}))
				}
			]
		} satisfies CurriculumOfferingPreview);
	});
	await page.route('**/api/academic/offerings/apply-from-curriculum', (route) => {
		for (const room of workspace.homerooms)
			for (const item of room.items) {
				item.offeringId = comparisonOffering;
				item.offeringState = 'present';
				item.alignmentStates = ['matches_curriculum'];
			}
		return fulfill(route, {
			createdOfferings: [],
			createdGroups: [],
			reusedOfferings: [],
			reusedGroups: []
		});
	});
	const query = new URLSearchParams({
		academicYearId: navIds.year,
		academicTermId: navIds.term,
		view: 'offerings',
		kind,
		subjectGroupId: kind === 'course' ? navIds.math : 'all',
		offeringSearch: old.name
	});
	return {
		workspace,
		delivery,
		old,
		origin: `/staff/academic/delivery?${query}`,
		academicWrites,
		managementReads,
		clearWrongContext() {
			wrongContext = false;
		}
	};
}

for (const width of [390, 1280])
	for (const dark of [false, true])
		for (const manager of [false, true])
			test(`both delivery views share exact-version comparison facts at ${width}px ${dark ? 'dark' : 'light'} ${manager ? 'manager' : 'reader'}`, async ({
				page
			}, testInfo) => {
				await page.setViewportSize({ width, height: 844 });
				const fixture = await mockSharedComparison(page, { manager });
				await page.goto(fixture.origin);
				if (dark) await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).click();
				const summary = page.getByTestId('delivery-alignment-summary');
				await expect(summary).toContainText('ยังไม่เปิดสอน 1 รายการ');
				await expect(summary).toContainText('รายการเดิมที่ต้องตรวจ 1 รายการ');
				await expect(summary).toContainText('นอกโครงสร้าง 1 รายการ');
				await expect(summary).toContainText('ยังเชื่อมห้องไม่ได้ 1 รายการ');
				const before = await summary.innerText();
				const table = page.getByTestId('delivery-offerings-ready');
				await expect(table.locator('tbody tr')).toHaveCount(1);
				await expect(table).toContainText('วิทยาศาสตร์*');
				await expect(table).toContainText('เปิดสอนเพิ่มเติมนอกหลักสูตร · 2 ห้อง');
				await expect(table).toContainText('ยังเชื่อมห้องไม่ได้');
				const trigger = page.getByRole('button', { name: 'ตรวจเทียบหลักสูตร', exact: true });
				await trigger.focus();
				await page.keyboard.press('Enter');
				const report = page.getByRole('dialog', {
					name: 'ผลเทียบหลักสูตรกับการเปิดสอนปัจจุบัน',
					exact: true
				});
				await expect(report).toBeVisible();
				await page.keyboard.press('Escape');
				await expect(report).toHaveCount(0);
				await expect(trigger).toBeFocused();
				await trigger.click();
				await expect(report.getByTestId('delivery-missing-requirement')).toHaveCount(1);
				await expect(report.getByTestId('delivery-missing-requirement')).toContainText(
					'ว30104 · วิทยาศาสตร์'
				);
				await expect(report.getByTestId('delivery-missing-requirement')).toContainText(
					'ม.1/1, ม.1/2'
				);
				await expect(report.getByTestId('delivery-existing-alignment')).toHaveCount(1);
				await expect(report).toContainText('รายการเดิมยังเชื่อมห้องไม่ได้');
				await expect(report).toContainText('ไม่เปลี่ยนชื่อหรือแทนรายการเดิมให้อัตโนมัติ');
				await expect(
					report.getByRole('button', { name: 'เปิดตามหลักสูตร · วิทยาศาสตร์', exact: true })
				).toHaveCount(manager ? 1 : 0);
				await page.screenshot({
					path: testInfo.outputPath(
						`shared-comparison-${width}-${dark ? 'dark' : 'light'}-${manager ? 'manager' : 'reader'}.png`
					),
					animations: 'disabled'
				});
				await report
					.getByRole('textbox', { name: 'ค้นหาผลเทียบหลักสูตร', exact: true })
					.fill('วิทยาศาสตร์');
				await report.getByRole('button', { name: 'ปิดผลเทียบ', exact: true }).click();
				await page.getByRole('tab', { name: 'มุมมองรายห้อง', exact: true }).click();
				await expect(summary).toHaveText(before);
				await page.getByRole('tab', { name: 'มุมมองรายวิชา/กิจกรรม', exact: true }).click();
				await expect(summary).toHaveText(before);
				await expect(
					page.getByRole('textbox', { name: 'ค้นหารายการเปิดสอน', exact: true })
				).toHaveValue('วิทยาศาสตร์*');
				await expect(page).toHaveURL((url) => {
					const actual = new URLSearchParams(url.search),
						expected = new URLSearchParams(fixture.origin.split('?')[1]);
					actual.sort();
					expected.sort();
					return actual.toString() === expected.toString();
				});
				await trigger.click();
				await expect(
					report.getByRole('textbox', { name: 'ค้นหาผลเทียบหลักสูตร', exact: true })
				).toHaveValue('วิทยาศาสตร์');
				await page.keyboard.press('Escape');
				expect(fixture.managementReads).toHaveLength(0);
				expect(fixture.academicWrites).toHaveLength(0);
			});

for (const mode of ['course', 'independent', 'synchronized'] as const)
	test(`a missing ${mode} opens the exact focused curriculum workflow and keeps provider eligibility errors`, async ({
		page
	}) => {
		const fixture = await mockSharedComparison(page, {
			manager: true,
			kind: mode === 'course' ? 'course' : 'activity',
			schedulingMode: mode === 'course' ? undefined : mode,
			failPreview: true
		});
		await page.goto(fixture.origin);
		await page.getByRole('button', { name: 'ตรวจเทียบหลักสูตร', exact: true }).click();
		await page
			.getByRole('button', {
				name: `เปิดตามหลักสูตร · ${mode === 'course' ? 'วิทยาศาสตร์' : 'กิจกรรมใหม่'}`,
				exact: true
			})
			.click();
		const preparation = page.getByRole('dialog', {
			name: mode === 'course' ? 'เปิดรายวิชา วิทยาศาสตร์' : 'เพิ่มกิจกรรม กิจกรรมใหม่',
			exact: true
		});
		await expect(preparation).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(preparation).toHaveCount(0);
		const reportTrigger = page.getByRole('button', { name: 'ตรวจเทียบหลักสูตร', exact: true });
		await expect(reportTrigger).toBeFocused();
		await reportTrigger.click();
		await page
			.getByRole('button', {
				name: `เปิดตามหลักสูตร · ${mode === 'course' ? 'วิทยาศาสตร์' : 'กิจกรรมใหม่'}`,
				exact: true
			})
			.click();
		await expect(preparation).toContainText('2 ห้อง');
		await expect(preparation).toContainText('ไม่โอนกลุ่ม ครู หรือคะแนนจากรายการเดิมให้อัตโนมัติ');
		if (mode === 'synchronized') await expect(preparation).toContainText('รายการกลาง');
		else await expect(preparation).not.toContainText('รายการเปิดสอนกลางเพียงรายการเดียว');
		await preparation.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
		await expect(preparation.getByRole('alert')).toContainText('ฉบับทะเบียนที่เลือกไม่พร้อมใช้งาน');
		await preparation.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
		const applyButton = preparation.getByRole('button', {
			name: mode === 'course' ? 'ยืนยันเปิดรายวิชา' : 'เปิดใช้งานกิจกรรม',
			exact: true
		});
		await expect(applyButton).toBeEnabled();
		await expect(
			preparation.getByRole('button', {
				name: 'วันที่เริ่มเปิดสอนจริงสำหรับรายการจากหลักสูตร',
				exact: true
			})
		).toHaveCount(0);
		await applyButton.click();
		await expect(preparation).toHaveCount(0);
		const apply = fixture.academicWrites.find((write) =>
			write.path.endsWith('/apply-from-curriculum')
		)!;
		expect(apply.body.sourceHash).toBe('e'.repeat(64));
		expect(apply.body.idempotencyKey).toMatch(/^[0-9a-f-]{36}$/);
		expect(apply.body).toMatchObject({
			academicTermId: navIds.term,
			studyProgramIds: [navIds.program],
			choices: [
				{
					proposalId: 'missing-current-requirement',
					action: mode === 'synchronized' ? 'defer_groups' : 'apply'
				}
			]
		});
		expect(
			fixture.academicWrites.filter((write) => write.path.endsWith('/apply-from-curriculum'))
		).toHaveLength(1);
		expect(
			fixture.academicWrites.every((write) => !write.path.includes('/term-change-sets/'))
		).toBe(true);
		expect(
			fixture.delivery.offerings.some(
				(item) => item.id === fixture.old.id && item.name === fixture.old.name
			)
		).toBe(true);
	});

for (const mode of ['course', 'independent', 'synchronized'] as const) {
	for (const view of ['homerooms', 'offerings'] as const) {
		test(`inline missing ${mode} in ${view} opens the exact curriculum entry for all missing rooms`, async ({
			page
		}, testInfo) => {
			const mobile = view === 'homerooms' || mode !== 'independent';
			await page.setViewportSize({ width: mobile ? 390 : 1280, height: 900 });
			const fixture = await mockSharedComparison(page, {
				manager: true,
				kind: mode === 'course' ? 'course' : 'activity',
				schedulingMode: mode === 'course' ? undefined : mode,
				failPreview: true
			});
			const target = new URL(fixture.origin, 'http://schoolorbit.test');
			target.searchParams.set('view', view);
			target.searchParams.set('offeringSearch', mode === 'course' ? 'วิทยาศาสตร์' : 'กิจกรรมใหม่');
			await page.goto(target.pathname + target.search);
			if (mobile) {
				await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).click();
				await expect(page.locator('html')).toHaveClass(/\bdark\b/);
			}
			const origin = page.url();
			const scope =
				view === 'homerooms'
					? page.getByRole('region', { name: 'มุมมองรายห้อง', exact: true })
					: page.getByRole('region', { name: 'ตามหลักสูตรที่ยังไม่เปิดสอน', exact: true });
			const add = scope
				.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true })
				.first();
			await expect(add).toBeVisible();
			if (view === 'offerings')
				await expect(
					scope.getByText('หลักสูตรกำหนดไว้แต่ยังไม่เปิดสอน · 2 ห้อง', { exact: true })
				).toBeVisible();
			await add.scrollIntoViewIfNeeded();
			const pageWidth = await page.evaluate(() => ({
				viewport: window.innerWidth,
				content: document.documentElement.scrollWidth
			}));
			expect(pageWidth.content).toBeLessThanOrEqual(pageWidth.viewport);
			const bounds = await add.boundingBox();
			expect(bounds!.x).toBeGreaterThanOrEqual(0);
			expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(pageWidth.viewport);
			await page.screenshot({
				path: testInfo.outputPath('inline-missing-action.png'),
				animations: 'disabled'
			});
			expect(fixture.managementReads).toHaveLength(0);
			expect(fixture.academicWrites).toHaveLength(0);
			await add.focus();
			await page.keyboard.press('Enter');
			const dialog = page.getByRole('dialog', {
				name: mode === 'course' ? 'เปิดรายวิชา วิทยาศาสตร์' : 'เพิ่มกิจกรรม กิจกรรมใหม่',
				exact: true
			});
			await expect(dialog).toContainText('การเพิ่มนี้ครอบคลุมทุกห้องที่ยังขาดฉบับนี้ รวม 2 ห้อง');
			await expect(dialog).toContainText('ไม่โอนกลุ่ม ครู หรือคะแนนจากรายการเดิมให้อัตโนมัติ');
			await dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
			await expect(dialog.getByRole('alert')).toHaveText(
				'ฉบับทะเบียนที่เลือกไม่พร้อมใช้งาน กรุณาโหลดข้อมูลล่าสุดหรือสร้างตัวอย่างใหม่ก่อนดำเนินการ'
			);
			await expect(dialog).toContainText(mode === 'course' ? 'วิทยาศาสตร์' : 'กิจกรรมใหม่');
			await dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
			await page.screenshot({
				path: testInfo.outputPath('inline-curriculum-preparation.png'),
				animations: 'disabled'
			});
			await dialog
				.getByRole('button', {
					name: mode === 'course' ? 'ยืนยันเปิดรายวิชา' : 'เปิดใช้งานกิจกรรม',
					exact: true
				})
				.click();
			await expect(dialog).toHaveCount(0);
			expect(
				fixture.academicWrites.filter((write) => write.path.endsWith('/apply-from-curriculum'))
			).toHaveLength(1);
			expect(
				fixture.academicWrites.find((write) => write.path.endsWith('/apply-from-curriculum'))!.body
			).toMatchObject({
				academicTermId: navIds.term,
				studyProgramIds: [navIds.program],
				choices: [
					{
						proposalId: 'missing-current-requirement',
						action: mode === 'synchronized' ? 'defer_groups' : 'apply'
					}
				]
			});
			await expect(page).toHaveURL(origin);
			expect(fixture.delivery.offerings).toContainEqual(fixture.old);
		});
	}
}

test('inline missing requirements paginate in the URL even with no actual offerings and retain independent filters', async ({
	page
}) => {
	const fixture = await mockSharedComparison(page, { manager: true });
	fixture.delivery.offerings = [];
	fixture.workspace.unlinked = [];
	for (const room of fixture.workspace.homerooms) {
		room.expectedCount = 10;
		room.extraOfferings = [];
		const source = room.items[0];
		room.items = Array.from({ length: 10 }, (_, index) => ({
			...source,
			catalogVersionId: `d0000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
			requirementId: `e0000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
			code: `ว301${String(index).padStart(2, '0')}`,
			name: `รายวิชาที่ยังขาด ${index + 1}`
		}));
	}
	const target = new URL(fixture.origin, 'http://schoolorbit.test');
	target.searchParams.delete('offeringSearch');
	target.searchParams.set('missingPage', '1');
	await page.goto(target.pathname + target.search);
	const section = page.getByRole('region', { name: 'ตามหลักสูตรที่ยังไม่เปิดสอน', exact: true });
	await expect(section.getByTestId('delivery-inline-missing-requirement')).toHaveCount(2);
	await expect(section).toContainText('รวมทุกกลุ่มสาระ');
	await expect(section).toContainText('ตัวกรองกลุ่มสาระไม่ใช้กับรายการส่วนนี้');
	await expect(section).toContainText('รายวิชาที่ยังขาด 10');
	await page.reload();
	await expect(section.getByTestId('delivery-inline-missing-requirement')).toHaveCount(2);
	await section.getByRole('button', { name: 'ข้อกำหนดก่อนหน้า', exact: true }).click();
	await expect(section.getByTestId('delivery-inline-missing-requirement')).toHaveCount(8);
	await expect.poll(() => new URL(page.url()).searchParams.has('missingPage')).toBe(false);
	await section.getByRole('button', { name: 'ข้อกำหนดถัดไป', exact: true }).click();
	await expect.poll(() => new URL(page.url()).searchParams.get('missingPage')).toBe('1');
	await section
		.getByRole('textbox', { name: 'ค้นหารายการเปิดสอน', exact: true })
		.fill('รายวิชาที่ยังขาด 10');
	await expect(section.getByTestId('delivery-inline-missing-requirement')).toHaveCount(1);
	await expect.poll(() => new URL(page.url()).searchParams.has('missingPage')).toBe(false);
	expect(new URL(page.url()).searchParams.get('subjectGroupId')).toBe(navIds.math);
	await page.getByRole('tab', { name: 'มุมมองรายห้อง', exact: true }).click();
	await page.getByRole('tab', { name: 'มุมมองรายวิชา/กิจกรรม', exact: true }).click();
	await expect(
		section.getByRole('textbox', { name: 'ค้นหารายการเปิดสอน', exact: true })
	).toHaveValue('รายวิชาที่ยังขาด 10');
	await section.getByRole('combobox', { name: 'กรองประเภทรายการเปิดสอน', exact: true }).click();
	await page.getByRole('option', { name: 'กิจกรรม', exact: true }).click();
	await expect(section.getByTestId('delivery-inline-missing-requirement')).toHaveCount(0);
	await expect(section).toContainText('ไม่พบข้อกำหนดที่ยังขาดตามตัวกรอง');
	expect(fixture.managementReads).toHaveLength(0);
	expect(fixture.academicWrites).toHaveLength(0);
});

test('inline missing requirements wait for the workspace and distinguish rooms with unmet prerequisites', async ({
	page
}) => {
	const fixture = await mockSharedComparison(page);
	fixture.workspace.homerooms[0].items = [];
	fixture.workspace.homerooms[0].blockers = [
		{
			code: 'curriculum_not_assigned',
			message: 'ยังไม่มีโครงสร้างของห้องนี้',
			recoveryPath: '/staff/academic/curricula'
		}
	];
	const gate = deferred();
	await page.route('**/api/academic/delivery/homerooms?*', async (route) => {
		await gate.promise;
		await route.fallback();
	});
	try {
		const target = new URL(fixture.origin, 'http://schoolorbit.test');
		target.searchParams.delete('offeringSearch');
		await page.goto(target.pathname + target.search);
		const section = page.getByRole('region', { name: 'ตามหลักสูตรที่ยังไม่เปิดสอน', exact: true });
		await expect(section).toHaveAttribute('aria-busy', 'true');
		await expect(section.getByTestId('delivery-inline-missing-requirement')).toHaveCount(0);
		await expect(
			section.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true })
		).toHaveCount(0);
	} finally {
		gate.release();
	}
	const section = page.getByRole('region', { name: 'ตามหลักสูตรที่ยังไม่เปิดสอน', exact: true });
	await expect(section).toHaveAttribute('aria-busy', 'false');
	await expect(section).toContainText('มี 1 ห้องที่ยังขาดข้อมูลโครงสร้าง');
	await expect(section.getByTestId('delivery-inline-missing-requirement')).toContainText(
		'รวม 1 ห้อง'
	);
	await expect(section).not.toContainText('ตรงกับหลักสูตร');
	expect(fixture.managementReads).toHaveLength(0);
	expect(fixture.academicWrites).toHaveLength(0);
});

for (const condition of ['reader', 'context', 'failure'] as const) {
	test(`inline missing requirements are inspectable without unsafe actions for ${condition}`, async ({
		page
	}) => {
		const fixture = await mockSharedComparison(page, {
			manager: condition !== 'reader',
			wrongContext: condition === 'context',
			failWorkspace: condition === 'failure'
		});
		const target = new URL(fixture.origin, 'http://schoolorbit.test');
		target.searchParams.delete('offeringSearch');
		await page.goto(target.pathname + target.search);
		const section = page.getByRole('region', { name: 'ตามหลักสูตรที่ยังไม่เปิดสอน', exact: true });
		await expect(section).toBeVisible();
		if (condition === 'context' || condition === 'failure') {
			await expect(section.getByRole('alert')).toBeVisible();
			await expect(section.getByTestId('delivery-inline-missing-requirement')).toHaveCount(0);
		} else {
			await expect(section.getByTestId('delivery-inline-missing-requirement')).toHaveCount(1);
		}
		await expect(
			section.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true })
		).toHaveCount(0);
		expect(fixture.managementReads).toHaveLength(0);
		expect(fixture.academicWrites).toHaveLength(0);
	});
}

for (const reason of ['failure', 'context'] as const)
	test(`comparison source ${reason} stays explicit and retries only its owning workspace`, async ({
		page
	}) => {
		const fixture = await mockSharedComparison(page, {
			failWorkspace: reason === 'failure',
			wrongContext: reason === 'context'
		});
		await page.goto(fixture.origin);
		const source = page.getByRole('region', { name: 'เทียบกับหลักสูตรปัจจุบัน', exact: true });
		await expect(source.getByRole('alert')).toBeVisible();
		await expect(source).not.toContainText('ตรงกับหลักสูตร');
		fixture.clearWrongContext();
		await source.getByRole('button', { name: 'ลองโหลดผลเทียบอีกครั้ง', exact: true }).click();
		await expect(source.getByRole('alert')).toHaveCount(0);
		await expect(source).toContainText('ยังไม่เปิดสอน 1 รายการ');
		expect(fixture.managementReads).toHaveLength(0);
		expect(fixture.academicWrites).toHaveLength(0);
	});

test('an empty current offering registry can prepare its exact missing curriculum requirement directly', async ({
	page
}) => {
	const fixture = await mockSharedComparison(page, { manager: true, emptyCurrentOfferings: true });
	const target = new URL(fixture.origin, 'https://schoolorbit.invalid');
	target.searchParams.set('offeringSearch', 'วิทยาศาสตร์');
	await page.goto(target.pathname + target.search);
	const section = page.getByRole('region', { name: 'ตามหลักสูตรที่ยังไม่เปิดสอน', exact: true });
	await expect(section.getByTestId('delivery-inline-missing-requirement')).toHaveCount(1);
	await section.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true }).click();
	const preparation = page.getByRole('dialog', { name: 'เปิดรายวิชา วิทยาศาสตร์', exact: true });
	await expect(preparation).toBeVisible();
	expect(fixture.managementReads).toHaveLength(1);
	expect(fixture.academicWrites).toHaveLength(0);
	await expect(page.getByText('สร้างรุ่นเปิดสอนก่อนเพิ่มรายการ', { exact: true })).toHaveCount(0);
});

test('a partially offered exact course version opens pending program B for provider review instead of redirecting to program A', async ({
	page
}) => {
	const fixture = await mockSharedComparison(page, { manager: true });
	const programB = 'c0000000-0000-4000-8000-000000000031';
	fixture.workspace.homerooms[0].items[0] = {
		...fixture.workspace.homerooms[0].items[0],
		offeringId: fixture.old.id,
		offeringState: 'present',
		alignmentStates: ['matches_curriculum']
	};
	fixture.workspace.homerooms[1].studyProgram = {
		...fixture.workspace.homerooms[1].studyProgram,
		id: programB,
		name: 'แผนที่ยังไม่เปิดสอน'
	};
	for (const room of fixture.workspace.homerooms) room.extraOfferings = [];
	fixture.workspace.unlinked = [];
	fixture.old.name = 'วิทยาศาสตร์';
	if (fixture.old.catalog.kind === 'course')
		fixture.old.catalog.subjectVersionId = comparisonCatalog;
	fixture.delivery.courseMetadata = fixture.delivery.courseMetadata.map((entry) =>
		entry.subjectVersionId === navOfferings.basic21
			? { ...entry, subjectVersionId: comparisonCatalog }
			: entry
	);
	await page.route('**/api/academic/delivery/management-options?*', (route) =>
		fulfill(route, {
			academicYearId: navIds.year,
			academicTermId: navIds.term,
			studyPrograms: fixture.workspace.homerooms.map((room) => room.studyProgram),
			homerooms: fixture.workspace.homerooms.map((room) => room.homeroom),
			gradeLevels: [fixture.workspace.homerooms[0].gradeLevel],
			teachers: [],
			rooms: [],
			learningGroups: [],
			catalogVersions: []
		})
	);
	let previewBody: Record<string, unknown> | null = null;
	await page.route('**/api/academic/offerings/preview-from-curriculum', (route) => {
		previewBody = route.request().postDataJSON();
		return fulfill(route, {
			academicTermId: navIds.term,
			sourceHash: 'f'.repeat(64),
			curriculumSources: [],
			proposals: [
				{
					proposalId: 'retain-partial',
					resourceKind: 'course',
					catalogVersionId: comparisonCatalog,
					code: 'ว30104',
					name: 'วิทยาศาสตร์',
					schedulingMode: null,
					groupingState: 'proposed',
					offeringAction: 'retain',
					existingOfferingId: fixture.old.id,
					conflicts: [],
					targetHomeroomIds: [navIds.roomB],
					requirementIds: [fixture.workspace.homerooms[1].items[0].requirementId],
					defaultGroups: [{ groupKey: 'b'.repeat(64), name: 'ม.1/2', homeroomIds: [navIds.roomB] }]
				}
			]
		} satisfies CurriculumOfferingPreview);
	});
	const origin = new URL(fixture.origin, 'https://schoolorbit.invalid');
	origin.searchParams.delete('offeringSearch');
	await page.goto(origin.pathname + origin.search);
	await page.getByRole('button', { name: 'ตรวจเทียบหลักสูตร', exact: true }).click();
	await page.getByRole('button', { name: 'เปิดตามหลักสูตร · วิทยาศาสตร์', exact: true }).click();
	const preparation = page.getByRole('dialog', { name: 'เปิดรายวิชา วิทยาศาสตร์', exact: true });
	await expect(preparation).toBeVisible();
	await expect(preparation).toContainText('แผนที่ยังไม่เปิดสอน · 1 ห้อง');
	await preparation.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
	await expect(preparation.getByText('ใช้รายการเดิม', { exact: true })).toBeVisible();
	expect(previewBody).toMatchObject({ academicTermId: navIds.term, studyProgramIds: [programB] });
	await expect(page).toHaveURL((url) => url.pathname === '/staff/academic/delivery');
	expect(
		fixture.academicWrites.filter((write) => write.path.endsWith('/apply-from-curriculum'))
	).toHaveLength(0);
});

function focusedCurrentPreview(
	academicTermId: string,
	name: string,
	catalogVersionId = comparisonCatalog
): CurriculumOfferingPreview {
	return {
		academicTermId,
		curriculumSources: [],
		sourceHash: 'b'.repeat(64),
		proposals: [
			{
				proposalId: catalogVersionId,
				resourceKind: 'course',
				catalogVersionId,
				schedulingMode: null,
				code: 'ว30104',
				name,
				targetHomeroomIds: [navIds.roomA, navIds.roomB],
				requirementIds: [],
				groupingState: 'proposed',
				offeringAction: 'create',
				conflicts: [],
				defaultGroups: [
					{
						groupKey: 'b'.repeat(64),
						name: 'กลุ่มที่ตรวจปัจจุบัน',
						homeroomIds: [navIds.roomA, navIds.roomB]
					}
				]
			}
		]
	};
}
function currentPreparationOptions(workspace: ReturnType<typeof navigationWorkspace>) {
	return {
		academicYearId: workspace.academicYearId,
		academicTermId: workspace.academicTermId,
		studyPrograms: workspace.homerooms.map((room) => room.studyProgram),
		gradeLevels: [workspace.homerooms[0].gradeLevel],
		homerooms: workspace.homerooms.map((room) => room.homeroom),
		teachers: [],
		rooms: [],
		learningGroups: [],
		catalogVersions: []
	};
}

test('a late term A preview cannot replace or clear term B current preparation', async ({
	page
}) => {
	const fixture = await mockSharedComparison(page, { manager: true }),
		old = deferred(),
		handled = deferred(),
		nextOptions = deferred(),
		nextWorkspace = structuredClone(fixture.workspace);
	nextWorkspace.academicTermId = navIds.activeTerm;
	for (const room of nextWorkspace.homerooms)
		for (const item of room.items) item.name = 'วิทยาศาสตร์ภาคใหม่';
	let previews = 0,
		optionReads = 0;
	const previewBodies: Record<string, unknown>[] = [];
	await page.route('**/api/academic/context/options', (route) =>
		fulfill(route, {
			defaultAcademicYearId: navIds.year,
			defaultAcademicTermId: navIds.term,
			years: [{ isVisible: true, id: navIds.year, name: 'ปีทดสอบ', year: 2570, status: 'active' }],
			terms: [navIds.term, navIds.activeTerm].map((id, index) => ({
				isVisible: true,
				id,
				academicYearId: navIds.year,
				name: `ภาคเรียนที่ ${index + 1}`,
				code: String(index + 1),
				sequence: index + 1,
				status: 'active',
				termType: 'regular',
				includedInYearResult: true,
				blocksYearClosure: true
			}))
		})
	);
	await page.route('**/api/academic/delivery/homerooms?*', (route) =>
		fulfill(
			route,
			new URL(route.request().url()).searchParams.get('academicTermId') === navIds.activeTerm
				? nextWorkspace
				: fixture.workspace
		)
	);
	await page.route('**/api/academic/delivery/workspace?*', (route) =>
		fulfill(route, {
			...fixture.delivery,
			academicTermId: new URL(route.request().url()).searchParams.get('academicTermId')
		})
	);
	await page.route('**/api/academic/delivery/management-options?*', async (route) => {
		optionReads++;
		const next =
			new URL(route.request().url()).searchParams.get('academicTermId') === navIds.activeTerm;
		if (next) await nextOptions.promise;
		return fulfill(route, currentPreparationOptions(next ? nextWorkspace : fixture.workspace));
	});
	await page.route('**/api/academic/offerings/preview-from-curriculum', async (route) => {
		const body: Record<string, unknown> = route.request().postDataJSON();
		previewBodies.push(body);
		previews++;
		if (previews === 1) {
			await old.promise;
			try {
				await fulfill(route, focusedCurrentPreview(navIds.term, 'วิทยาศาสตร์'));
			} finally {
				handled.release();
			}
		} else await fulfill(route, focusedCurrentPreview(navIds.activeTerm, 'วิทยาศาสตร์ภาคใหม่'));
	});
	try {
		await page.goto(fixture.origin);
		await page.getByRole('button', { name: 'ตรวจเทียบหลักสูตร', exact: true }).click();
		await page.getByRole('button', { name: 'เปิดตามหลักสูตร · วิทยาศาสตร์', exact: true }).click();
		let dialog = page.getByRole('dialog', { name: 'เปิดรายวิชา วิทยาศาสตร์', exact: true });
		await dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
		await expect.poll(() => previews).toBe(1);
		await page.keyboard.press('Escape');
		await expect(dialog).toHaveCount(0);
		await page.getByRole('combobox', { name: 'เลือกภาคเรียน', exact: true }).click();
		await page.getByRole('option', { name: /^ภาคเรียนที่ 2/ }).click();
		await expect(page).toHaveURL(new RegExp(`academicTermId=${navIds.activeTerm}`));
		await page.getByRole('button', { name: 'ตรวจเทียบหลักสูตร', exact: true }).click();
		await page
			.getByRole('button', { name: 'เปิดตามหลักสูตร · วิทยาศาสตร์ภาคใหม่', exact: true })
			.click();
		dialog = page.getByRole('dialog', { name: 'เปิดรายวิชา วิทยาศาสตร์ภาคใหม่', exact: true });
		await expect(dialog).toBeVisible();
		await expect.poll(() => optionReads).toBe(2);
		old.release();
		await handled.promise;
		await expect(dialog).toBeVisible();
		await expect(
			dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true })
		).toHaveCount(0);
		nextOptions.release();
		await dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
		await expect(dialog.getByRole('textbox', { name: 'ชื่อกลุ่มเรียน', exact: true })).toHaveValue('กลุ่มที่ตรวจปัจจุบัน');
		expect(previewBodies.map((body) => body.academicTermId)).toEqual([
			navIds.term,
			navIds.activeTerm
		]);
		expect(
			fixture.academicWrites.filter((write) => write.path.endsWith('/apply-from-curriculum'))
		).toEqual([]);
	} finally {
		old.release();
		nextOptions.release();
	}
});

test('inline curriculum preparation retains repeated conflict categories for distinct manual groups', async ({
	page
}, testInfo) => {
	const fixture = await mockSharedComparison(page, { manager: true });
	const name = 'เคมีเพิ่มเติม',
		code = 'ว30246';
	const groupIds = ['f0000000-0000-4000-8000-000000000001', 'f0000000-0000-4000-8000-000000000002'];
	fixture.old.name = name;
	fixture.old.code = code;
	if (fixture.old.catalog.kind === 'course')
		fixture.old.catalog.subjectVersionId = comparisonCatalog;
	fixture.old.groups = fixture.workspace.homerooms.map((room, index) => ({
		id: groupIds[index],
		rowVersion: 1,
		registrationOpen: false,
		code: `MANUAL-${index + 1}`,
		name: `กลุ่มที่จัดเอง ${index + 1}`,
		description: null,
		capacity: null,
		homeroomIds: [room.homeroom.id],
		preferredRoomIds: [],
		teachers: []
	}));
	for (const room of fixture.workspace.homerooms) {
		room.items[0].name = name;
		room.items[0].code = code;
		room.extraOfferings = [];
	}
	fixture.workspace.unlinked = [];
	const message = `รายการ ${code} มีกลุ่มที่จัดเองครอบคลุมห้องเป้าหมายอยู่แล้ว ระบบจะไม่แก้กลุ่มนี้`;
	const conflicts = groupIds.map((groupId) => ({
		code: 'manual_group_overlap',
		message,
		offeringId: fixture.old.id,
		groupId
	}));
	await page.route('**/api/academic/offerings/preview-from-curriculum', (route) =>
		fulfill(route, {
			academicTermId: navIds.term,
			curriculumSources: [],
			sourceHash: 'c'.repeat(64),
			proposals: [
				{
					proposalId: 'd'.repeat(64),
					resourceKind: 'course',
					catalogVersionId: comparisonCatalog,
					code,
					name,
					schedulingMode: null,
					groupingState: 'conflict',
					offeringAction: 'retain',
					existingOfferingId: fixture.old.id,
					conflicts,
					targetHomeroomIds: fixture.workspace.homerooms.map((room) => room.homeroom.id),
					requirementIds: fixture.workspace.homerooms.flatMap((room) =>
						room.items.map((item) => item.requirementId)
					),
					defaultGroups: fixture.workspace.homerooms.map((room, index) => ({
						groupKey: String(index + 1).repeat(64),
						name: room.homeroom.name,
						homeroomIds: [room.homeroom.id]
					}))
				}
			]
		} satisfies CurriculumOfferingPreview)
	);
	const originalGroups = structuredClone(fixture.old.groups);
	await page.route('**/api/academic/offerings/apply-from-curriculum', (route) => {
		for (const room of fixture.workspace.homerooms) {
			const item = room.items[0];
			item.offeringId = fixture.old.id;
			item.offeringState = 'present';
			item.alignmentStates = ['matches_curriculum'];
			item.groups = fixture.old.groups
				.filter((group) => group.homeroomIds.includes(room.homeroom.id))
				.map((group) => ({
					id: group.id,
					code: group.code,
					name: group.name,
					homeroomIds: group.homeroomIds,
					homeroomNames: [room.homeroom.name],
					primaryTeacherCount: 0
				}));
			item.groupMode = 'normal';
			room.readyCount = 1;
		}
		return fulfill(route, {
			academicTermId: navIds.term,
			sourceHash: 'c'.repeat(64),
			offeringIds: [fixture.old.id],
			groupIds: [],
			createdOfferingCount: 0,
			retainedOfferingCount: 1,
			createdGroupCount: 0,
			retainedGroupCount: 0,
			skippedCount: 0
		} satisfies ApplyCurriculumOfferingsResult);
	});
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	try {
		const target = new URL(fixture.origin, 'http://schoolorbit.test');
		target.searchParams.set('offeringSearch', code);
		await page.goto(target.pathname + target.search);
		const missing = page.getByRole('region', { name: 'ตามหลักสูตรที่ยังไม่เปิดสอน', exact: true });
		await missing.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true }).click();
		const dialog = page.getByRole('dialog', { name: `เปิดรายวิชา ${name}`, exact: true });
		await dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
		await expect(dialog.getByText(message, { exact: true })).toHaveCount(2);
		await expect(
			dialog.getByRole('combobox', { name: `วิธีเตรียม ${name}`, exact: true })
		).toHaveText('เปิดสอน รอจัดกลุ่ม');
		await expect(
			dialog.getByRole('button', { name: 'ยืนยันเปิดรายวิชา', exact: true })
		).toBeEnabled();
		expect(errors).toEqual([]);
		expect(
			fixture.academicWrites.filter((write) => write.path.endsWith('/apply-from-curriculum'))
		).toHaveLength(0);
		await page.screenshot({
			path: testInfo.outputPath('repeated-manual-conflicts.png'),
			animations: 'disabled'
		});
		await dialog.getByRole('button', { name: 'ยืนยันเปิดรายวิชา', exact: true }).click();
		await expect(dialog).toHaveCount(0);
		const apply = fixture.academicWrites.find((write) =>
			write.path.endsWith('/apply-from-curriculum')
		)!;
		expect(apply.body).toEqual({
			academicTermId: navIds.term,
			studyProgramIds: [navIds.program],
			sourceHash: 'c'.repeat(64),
			idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/),
			choices: [{ proposalId: 'd'.repeat(64), action: 'defer_groups', groups: [] }]
		});
		expect(fixture.academicWrites.map((write) => write.path)).toEqual([
			'/api/academic/offerings/preview-from-curriculum',
			'/api/academic/offerings/apply-from-curriculum'
		]);
		expect(fixture.old.groups).toEqual(originalGroups);
		expect(errors).toEqual([]);
	} finally {
		await testInfo.attach('browser-page-errors', {
			body: JSON.stringify(errors),
			contentType: 'application/json'
		});
	}
});

test('a focused offering-level provider conflict blocks save without offering apply or defer controls', async ({
	page
}) => {
	const fixture = await mockSharedComparison(page, { manager: true });
	fixture.workspace.homerooms[0].items[0] = {
		...fixture.workspace.homerooms[0].items[0],
		offeringId: fixture.old.id,
		offeringState: 'present',
		alignmentStates: ['matches_curriculum']
	};
	fixture.workspace.homerooms[1].studyProgram = {
		...fixture.workspace.homerooms[1].studyProgram,
		id: 'c0000000-0000-4000-8000-000000000051',
		name: 'แผนที่ยังขาดเป้าหมาย'
	};
	fixture.old.name = 'วิทยาศาสตร์';
	for (const room of fixture.workspace.homerooms) room.extraOfferings = [];
	fixture.workspace.unlinked = [];
	if (fixture.old.catalog.kind === 'course')
		fixture.old.catalog.subjectVersionId = comparisonCatalog;
	const reason =
		'รายการ ว30104 เผยแพร่แล้วและยังไม่ครอบคลุมห้องเป้าหมายที่เลือก ไม่สามารถเพิ่มเป้าหมายจากการเตรียมเปิดสอนได้';
	await page.route('**/api/academic/offerings/preview-from-curriculum', (route) =>
		fulfill(route, {
			academicTermId: navIds.term,
			sourceHash: 'a'.repeat(64),
			curriculumSources: [],
			proposals: [
				{
					proposalId: 'published-target-conflict',
					resourceKind: 'course',
					catalogVersionId: comparisonCatalog,
					code: 'ว30104',
					name: 'วิทยาศาสตร์',
					schedulingMode: null,
					groupingState: 'proposed',
					offeringAction: 'conflict',
					existingOfferingId: fixture.old.id,
					conflicts: [
						{
							code: 'published_offering_target_extension',
							message: reason,
							offeringId: fixture.old.id,
							groupId: null
						}
					],
					targetHomeroomIds: [navIds.roomB],
					requirementIds: [fixture.workspace.homerooms[1].items[0].requirementId],
					defaultGroups: [{ groupKey: 'b'.repeat(64), name: 'ม.1/2', homeroomIds: [navIds.roomB] }]
				}
			]
		} satisfies CurriculumOfferingPreview)
	);
	await page.goto(fixture.origin);
	await page.getByRole('button', { name: 'ตรวจเทียบหลักสูตร', exact: true }).click();
	await page.getByRole('button', { name: 'เปิดตามหลักสูตร · วิทยาศาสตร์', exact: true }).click();
	const preparation = page.getByRole('dialog', { name: 'เปิดรายวิชา วิทยาศาสตร์', exact: true });
	await preparation.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
	await expect(
		preparation.getByRole('button', { name: 'ยืนยันเปิดรายวิชา', exact: true })
	).toBeDisabled();
	await expect(preparation.getByText(reason, { exact: true })).toBeVisible();
	await expect(preparation.getByText('ติดข้อจำกัด', { exact: true })).toBeVisible();
	const action = preparation.getByRole('combobox', { name: 'วิธีเตรียม วิทยาศาสตร์', exact: true });
	await expect(action).toBeDisabled();
	await expect(action).toHaveText('ยังเปิดรายการนี้ไม่ได้');
	await expect(
		preparation.getByText('สร้างหรือคงรายการเปิดสอนไว้ก่อน', { exact: false })
	).toHaveCount(0);
	expect(
		fixture.academicWrites.filter((write) => write.path.endsWith('/apply-from-curriculum'))
	).toHaveLength(0);
	await preparation.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
	await expect(action).toHaveText('ยังเปิดรายการนี้ไม่ได้');
	await expect(
		preparation.getByRole('button', { name: 'ยืนยันเปิดรายวิชา', exact: true })
	).toBeDisabled();
	expect(
		fixture.academicWrites.filter((write) => write.path.endsWith('/apply-from-curriculum'))
	).toHaveLength(0);
	const previews = fixture.academicWrites.filter((write) =>
		write.path.endsWith('/preview-from-curriculum')
	);
	expect(previews).toHaveLength(2);
	for (const preview of previews)
		expect(preview.body.studyProgramIds).toEqual([fixture.workspace.homerooms[1].studyProgram.id]);
});

test('batch preparation can explicitly skip an offering conflict while deferring a retainable grouping conflict', async ({
	page
}) => {
	const fixture = await mockSharedComparison(page, { manager: true });
	const preview: CurriculumOfferingPreview = {
		academicTermId: navIds.term,
		sourceHash: 'b'.repeat(64),
		curriculumSources: [],
		proposals: [
			{
				proposalId: 'offering-conflict',
				resourceKind: 'course',
				catalogVersionId: comparisonCatalog,
				code: 'ว30104',
				name: 'วิทยาศาสตร์',
				schedulingMode: null,
				groupingState: 'proposed',
				offeringAction: 'conflict',
				existingOfferingId: fixture.old.id,
				targetHomeroomIds: [navIds.roomB],
				requirementIds: [comparisonCatalog],
				defaultGroups: [],
				conflicts: [
					{
						code: 'published_offering_target_extension',
						message: 'รายการเผยแพร่แล้ว ไม่สามารถขยายเป้าหมายได้',
						offeringId: fixture.old.id,
						groupId: null
					}
				]
			},
			{
				proposalId: 'retain-grouping-conflict',
				resourceKind: 'course',
				catalogVersionId: 'c0000000-0000-4000-8000-000000000052',
				code: 'ค20202',
				name: 'รายวิชาที่คงกลุ่มไว้',
				schedulingMode: null,
				groupingState: 'conflict',
				offeringAction: 'retain',
				existingOfferingId: navOfferings.additional,
				targetHomeroomIds: [navIds.roomA],
				requirementIds: [navOfferings.additional],
				defaultGroups: [],
				conflicts: [
					{
						code: 'generated_group_target_mismatch',
						message: 'กลุ่มที่สร้างจากข้อกำหนดเดิมมีห้องต่างจากข้อเสนอ',
						offeringId: navOfferings.additional,
						groupId: navIds.roomA
					}
				]
			}
		]
	};
	await page.route('**/api/academic/offerings/preview-from-curriculum', (route) =>
		fulfill(route, preview)
	);
	await page.route('**/api/academic/offerings/apply-from-curriculum', (route) =>
		fulfill(route, {
			academicTermId: navIds.term,
			sourceHash: preview.sourceHash,
			createdOfferingCount: 0,
			retainedOfferingCount: 1,
			createdGroupCount: 0,
			retainedGroupCount: 0,
			skippedCount: 1,
			offeringIds: [navOfferings.additional],
			groupIds: []
		})
	);
	await page.goto(fixture.origin);
	await page.getByRole('button', { name: 'เปิดการเรียนการสอน', exact: true }).click();
	const preparation = page.getByRole('dialog', {
		name: 'เปิดการเรียนการสอนในภาคเรียนนี้',
		exact: true
	});
	await preparation
		.getByRole('combobox', { name: 'เลือกแผนการเรียนจากหลักสูตร', exact: true })
		.click();
	await page.getByRole('option', { name: /แผนคณิตศาสตร์/ }).click();
	await page.keyboard.press('Escape');
	await preparation.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
	const save = preparation.getByRole('button', {
		name: 'ยืนยันรายการและกลุ่มที่ตรวจแล้ว',
		exact: true
	});
	await expect(save).toBeDisabled();
	const blocked = preparation.getByRole('combobox', {
		name: 'วิธีเตรียม วิทยาศาสตร์',
		exact: true
	});
	await expect(blocked).toHaveText('ยังเปิดรายการนี้ไม่ได้');
	await blocked.click();
	await expect(page.getByRole('option')).toHaveText(['ข้ามรายการนี้']);
	await page.getByRole('option', { name: 'ข้ามรายการนี้', exact: true }).click();
	const retained = preparation.getByRole('combobox', {
		name: 'วิธีเตรียม รายวิชาที่คงกลุ่มไว้',
		exact: true
	});
	await expect(retained).toHaveText('เปิดสอน รอจัดกลุ่ม');
	await retained.click();
	await expect(page.getByRole('option', { name: 'เปิดสอน รอจัดกลุ่ม', exact: true })).toBeVisible();
	await expect(page.getByRole('option', { name: 'เปิดสอนและจัดกลุ่ม', exact: true })).toHaveCount(
		0
	);
	await page.keyboard.press('Escape');
	await expect(save).toBeEnabled();
	await save.click();
	await expect(preparation).toHaveCount(0);
	const applied = fixture.academicWrites.filter((write) =>
		write.path.endsWith('/apply-from-curriculum')
	);
	expect(applied).toHaveLength(1);
	expect(applied[0].body.choices).toEqual([
		{ proposalId: 'offering-conflict', action: 'skip', groups: [] },
		{ proposalId: 'retain-grouping-conflict', action: 'defer_groups', groups: [] }
	]);
});

test('reopening a different exact requirement fences its current choices from a late prior preview', async ({
	page
}) => {
	const fixture = await mockSharedComparison(page, { manager: true }),
		old = deferred(),
		handled = deferred(),
		catalogB = 'c0000000-0000-4000-8000-000000000061';
	for (const [index, room] of fixture.workspace.homerooms.entries())
		room.items.push({
			...room.items[0],
			requirementId: `c0000000-0000-4000-8000-00000000007${index}`,
			catalogVersionId: catalogB,
			name: 'วิชาบี',
			code: 'ว30105'
		});
	let previews = 0;
	await page.route('**/api/academic/delivery/management-options?*', (route) =>
		fulfill(route, currentPreparationOptions(fixture.workspace))
	);
	await page.route('**/api/academic/offerings/preview-from-curriculum', async (route) => {
		previews++;
		if (previews === 1) {
			await old.promise;
			try {
				await fulfill(route, focusedCurrentPreview(navIds.term, 'วิทยาศาสตร์'));
			} finally {
				handled.release();
			}
		} else return fulfill(route, focusedCurrentPreview(navIds.term, 'วิชาบี', catalogB));
	});
	try {
		await page.goto(fixture.origin);
		await page.getByRole('button', { name: 'ตรวจเทียบหลักสูตร', exact: true }).click();
		await page.getByRole('button', { name: 'เปิดตามหลักสูตร · วิทยาศาสตร์', exact: true }).click();
		let dialog = page.getByRole('dialog', { name: 'เปิดรายวิชา วิทยาศาสตร์', exact: true });
		await dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
		await expect.poll(() => previews).toBe(1);
		await page.keyboard.press('Escape');
		await page.getByRole('button', { name: 'ตรวจเทียบหลักสูตร', exact: true }).click();
		await page.getByRole('button', { name: 'เปิดตามหลักสูตร · วิชาบี', exact: true }).click();
		dialog = page.getByRole('dialog', { name: 'เปิดรายวิชา วิชาบี', exact: true });
		await dialog.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
		await expect(dialog.getByRole('textbox', { name: 'ชื่อกลุ่มเรียน', exact: true })).toHaveValue('กลุ่มที่ตรวจปัจจุบัน');
		await expect(
			dialog.getByRole('button', { name: 'ยืนยันเปิดรายวิชา', exact: true })
		).toBeEnabled();
		old.release();
		await handled.promise;
		await expect(dialog).toBeVisible();
		await expect(
			dialog.getByRole('button', { name: 'ยืนยันเปิดรายวิชา', exact: true })
		).toBeEnabled();
		await expect(dialog.getByRole('alert')).toHaveCount(0);
		expect(fixture.managementReads).toHaveLength(1);
		expect(previews).toBe(2);
		expect(
			fixture.academicWrites.filter((write) => write.path.endsWith('/apply-from-curriculum'))
		).toEqual([]);
	} finally {
		old.release();
	}
});

async function mockReviewStatuses(page: Page, wrongContext = false) {
	await mockDeliveryNavigation(page);
	const workspace = navigationWorkspace(),
		delivery = navigationDelivery();
	const course = delivery.offerings.find((item) => item.id === navOfferings.basic21)!;
	const central = delivery.offerings.find((item) => item.id === navOfferings.club)!;
	const extra = delivery.offerings.find((item) => item.id === navOfferings.guidance)!;
	if (extra.catalog.kind !== 'activity')
		throw new Error('Expected the additional activity fixture');
	const extraCatalogVersionId = extra.catalog.activityVersionId;
	const groupId = 'd0000000-0000-4000-8000-000000000111';
	const extraGroupId = 'd0000000-0000-4000-8000-000000000112';
	course.weeklyPeriodTarget = 0;
	course.groups = [
		{
			id: groupId,
			rowVersion: 1,
			registrationOpen: false,
			code: 'MANUAL',
			name: 'กลุ่มที่ยังขาดครู',
			homeroomIds: [navIds.roomA],
			preferredRoomIds: [],
			teachers: []
		}
	];
	central.weeklyPeriodTarget = 0;
	central.groups = [];
	extra.groups = [
		{
			id: extraGroupId,
			rowVersion: 1,
			registrationOpen: false,
			code: 'EXTRA',
			name: 'กลุ่มนอกหลักสูตร',
			homeroomIds: [navIds.roomA],
			preferredRoomIds: [],
			teachers: [
				{
					assignmentId: 'd0000000-0000-4000-8000-000000000113',
					rowVersion: 1,
					isActive: true,
					teacherId: navIds.actor,
					displayName: 'ครูหลักกิจกรรมเพิ่มเติม',
					role: 'primary'
				}
			]
		}
	];
	delivery.offerings = [course, central, extra];
	const courseSource = workspace.homerooms[0].items.find((item) => item.offeringId === course.id)!;
	workspace.homerooms[0].items = [
		{
			...courseSource,
			weeklyPeriodTarget: 0,
			teacherState: 'missing_primary',
			groupMode: 'normal',
			alignmentStates: ['operational_periods_differ'],
			groups: [
				{
					id: groupId,
					code: 'MANUAL',
					name: 'กลุ่มที่ยังขาดครู',
					homeroomIds: [navIds.roomA],
					homeroomNames: ['ม.1/1'],
					primaryTeacherCount: 0
				}
			]
		}
	];
	workspace.homerooms[0].expectedCount = 1;
	workspace.homerooms[0].readyCount = 1;
	workspace.homerooms[0].extraOfferings = [
		{
			offeringId: extra.id,
			catalogVersionId: extraCatalogVersionId,
			resourceKind: 'activity',
			code: extra.code,
			name: extra.name,
			weeklyPeriodTarget: extra.weeklyPeriodTarget,
			alignmentStates: ['extra_offering']
		}
	];
	workspace.homerooms[1].items = [
		{
			...courseSource,
			requirementId: 'd0000000-0000-4000-8000-000000000114',
			catalogVersionId: 'd0000000-0000-4000-8000-000000000115',
			name: 'รายวิชาที่ยังไม่เปิด',
			offeringId: null,
			offeringState: 'missing',
			weeklyPeriodTarget: null,
			teacherState: 'missing_primary',
			groupMode: 'missing',
			groups: [],
			alignmentStates: ['curriculum_requirement_not_offered']
		}
	];
	workspace.homerooms[1].expectedCount = 1;
	workspace.homerooms[1].readyCount = 0;
	workspace.homerooms[1].extraOfferings = [];
	const centralRoom = structuredClone(workspace.homerooms[1]);
	centralRoom.homeroom = { id: 'd0000000-0000-4000-8000-000000000116', name: 'ม.1/3' };
	centralRoom.items = [
		{
			...courseSource,
			requirementId: 'd0000000-0000-4000-8000-000000000117',
			catalogVersionId: central.id,
			resourceKind: 'activity',
			code: central.code,
			name: central.name,
			schedulingMode: 'synchronized',
			subjectType: null,
			standardPeriodsPerWeek: null,
			weeklyPeriodTarget: 0,
			offeringId: central.id,
			offeringState: 'present',
			groupMode: 'central',
			teacherState: 'deferred',
			groups: [],
			alignmentStates: ['matches_curriculum']
		}
	];
	centralRoom.expectedCount = 1;
	centralRoom.readyCount = 1;
	workspace.homerooms.push(centralRoom);
	workspace.unlinked = [];
	const academicReads: string[] = [],
		writes: string[] = [];
	page.on('request', (request) => {
		const path = new URL(request.url()).pathname;
		if (!path.startsWith('/api/academic/')) return;
		if (request.method() === 'GET') academicReads.push(path);
		else writes.push(`${request.method()} ${path}`);
	});
	await page.route('**/api/academic/delivery/homerooms?*', (route) =>
		fulfill(route, wrongContext ? { ...workspace, academicTermId: navIds.activeTerm } : workspace)
	);
	await page.route('**/api/academic/delivery/workspace?*', (route) => fulfill(route, delivery));
	const query = new URLSearchParams({
		academicYearId: navIds.year,
		academicTermId: navIds.term,
		expandedRooms: ''
	});
	return {
		workspace,
		delivery,
		academicReads,
		writes,
		origin: `/staff/academic/delivery?${query}`
	};
}

for (const width of [390, 1280])
	for (const dark of [false, true]) {
		test(`collapsed room review badges share operational facts with offering rows at ${width}px ${dark ? 'dark' : 'light'}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 1000 });
			const fixture = await mockReviewStatuses(page);
			await page.goto(fixture.origin);
			if (dark) {
				await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).click();
				await expect(page.locator('html')).toHaveClass(/\bdark\b/);
			}
			const room = (name: string) =>
				page.locator('details').filter({ has: page.getByRole('heading', { name, exact: true }) });
			const summary = room('ม.1/1').locator('summary');
			await expect(room('ม.1/1')).not.toHaveAttribute('open', '');
			await expect(summary).not.toContainText('ครบตามโครงสร้าง');
			await expect(summary.locator('[data-status]')).toHaveCount(3);
			await expect(
				summary.locator(
					'[data-status="groups_present"], [data-status="primary_assigned"], [data-status="zero_weekly_periods"]'
				)
			).toHaveCount(0);
			await expect(summary).toContainText('ยังไม่มีครูหลัก 1 กลุ่ม');
			await expect(summary).toContainText('คาบจริงต่างจากค่ามาตรฐานในหลักสูตร 1 รายการ');
			await expect(summary).toContainText('เปิดสอนเพิ่มเติมนอกหลักสูตร 1 รายการ');
			await expect(summary).toContainText('1/1');
			await expect(room('ม.1/2').locator('summary')).toContainText(
				'หลักสูตรกำหนดไว้แต่ยังไม่เปิดสอน 1 รายการ'
			);
			await expect(room('ม.1/2').locator('summary')).not.toContainText('ยังไม่มีครูหลัก');
			await expect(
				room('ม.1/3').locator('summary').getByTestId('delivery-room-statuses')
			).toHaveCount(0);
			await expect(room('ม.1/3').locator('summary')).toContainText('1/1');
			await expect(room('ม.1/3').locator('summary')).not.toContainText('ยังไม่มีครูหลัก');
			const statuses = summary.getByTestId('delivery-room-statuses');
			const headingBounds = await summary
				.getByRole('heading', { name: 'ม.1/1', exact: true })
				.boundingBox();
			const statusBounds = await statuses.boundingBox();
			if (width >= 640) {
				expect(statusBounds!.x).toBeGreaterThan(headingBounds!.x + headingBounds!.width);
				expect(statusBounds!.x + statusBounds!.width).toBeLessThan(
					(await summary
						.locator('p')
						.filter({ hasText: /^1\/1$/ })
						.boundingBox())!.x
				);
			}
			expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
				width
			);
			await summary.scrollIntoViewIfNeeded();
			await summary.screenshot({
				path: testInfo.outputPath('collapsed-room-statuses.png'),
				animations: 'disabled'
			});
			expect(
				fixture.academicReads.filter((path) => path === '/api/academic/delivery/workspace')
			).toHaveLength(0);
			const beforeExpand = fixture.academicReads.length;
			await summary.focus();
			await page.keyboard.press('Enter');
			await expect(room('ม.1/1')).toHaveAttribute('open', '');
			await expect(
				room('ม.1/1').getByRole('table').getByText('ยังไม่มีครูหลัก 1 กลุ่ม', { exact: true })
			).toBeVisible();
			await room('ม.1/2').locator('summary').click();
			await expect(
				room('ม.1/2')
					.getByRole('row')
					.filter({ hasText: 'รายวิชาที่ยังไม่เปิด' })
					.getByRole('cell')
					.nth(4)
			).toHaveText('ยังไม่เปิดสอน');
			await room('ม.1/3').locator('summary').click();
			await expect(
				room('ม.1/3').getByRole('row').filter({ hasText: 'ชุมนุม' }).getByRole('cell').nth(4)
			).toHaveText('กำหนดภายหลัง');
			expect(fixture.academicReads).toHaveLength(beforeExpand);
			const filter = page.getByRole('combobox', { name: 'กรองความพร้อมของห้อง', exact: true });
			await filter.click();
			await page.getByRole('option', { name: 'ไม่มีรายการที่ต้องตรวจ', exact: true }).click();
			await expect(page.locator('details')).toHaveCount(1);
			await expect(room('ม.1/3')).toBeVisible();
			await filter.click();
			await page.getByRole('option', { name: 'ต้องตรวจเพิ่มเติม', exact: true }).click();
			await expect(page.locator('details')).toHaveCount(2);
			await expect(room('ม.1/1')).toBeVisible();
			await expect(room('ม.1/2')).toBeVisible();
			await page.getByRole('tab', { name: 'มุมมองรายวิชา/กิจกรรม', exact: true }).click();
			const offeringRegion = page.getByRole('region', {
				name: 'มุมมองรายวิชาและกิจกรรม',
				exact: true
			});
			const course = offeringRegion.getByRole('row').filter({ hasText: 'คณิตศาสตร์ต้น' });
			await expect(course).toContainText('ยังไม่มีครูหลัก 1 กลุ่ม');
			await expect(course).toContainText('คาบจริงต่างจากค่ามาตรฐานในหลักสูตร');
			await expect(course.locator('[data-status="zero_weekly_periods"]')).toHaveCount(0);
			await expect(course.getByRole('cell').nth(2)).toHaveText('0');
			const central = offeringRegion.getByRole('row').filter({ hasText: 'ชุมนุม' });
			await expect(central.getByRole('cell').nth(1)).toHaveText('');
			await expect(central.getByRole('cell').nth(1).locator('[data-slot="badge"]')).toHaveCount(0);
			await expect(central.getByTestId('delivery-status-badges')).toHaveCount(0);
			await expect(central.locator('.mt-2')).toHaveCount(0);
			await expect(central).not.toContainText('ยังไม่มีครูหลัก');
			await expect(central).not.toContainText('0/0');
			const extra = offeringRegion.getByRole('row').filter({ hasText: 'แนะแนว' });
			await expect(extra).toContainText('เปิดสอนเพิ่มเติมนอกหลักสูตร');
			await expect(extra.locator('[data-status="primary_assigned"]')).toHaveCount(0);
			await expect(extra.getByRole('cell').nth(4)).toHaveText('1/1 กลุ่ม');
			await course.getByRole('cell').first().scrollIntoViewIfNeeded();
			await offeringRegion.screenshot({
				path: testInfo.outputPath('offering-review-statuses.png'),
				animations: 'disabled'
			});
			if (width === 390) {
				const scroller = offeringRegion.locator('[data-slot="table-container"]');
				const initialScroll = await scroller.evaluate((element) => ({
					left: element.scrollLeft,
					width: element.clientWidth,
					contentWidth: element.scrollWidth
				}));
				expect(initialScroll.contentWidth).toBeGreaterThan(initialScroll.width);
				const statusCell = course.getByRole('cell').nth(1);
				await statusCell.scrollIntoViewIfNeeded();
				await statusCell.screenshot({
					path: testInfo.outputPath('mobile-offering-status-cell.png'),
					animations: 'disabled'
				});
				const warning = statusCell.getByText('ยังไม่มีครูหลัก 1 กลุ่ม', { exact: true });
				await warning.scrollIntoViewIfNeeded();
				const statusScroll = await scroller.evaluate((element) => element.scrollLeft);
				const scrollerBounds = (await scroller.boundingBox())!;
				const warningBounds = (await warning.boundingBox())!;
				expect(warningBounds.x).toBeGreaterThanOrEqual(scrollerBounds.x);
				expect(warningBounds.x + warningBounds.width).toBeLessThanOrEqual(
					scrollerBounds.x + scrollerBounds.width
				);
				const action = course.getByRole('link', { name: 'รายละเอียด', exact: true });
				await action.scrollIntoViewIfNeeded();
				await expect
					.poll(() => scroller.evaluate((element) => element.scrollLeft))
					.toBeGreaterThan(initialScroll.left);
				const actionBounds = (await action.boundingBox())!;
				expect(actionBounds.x).toBeGreaterThanOrEqual(scrollerBounds.x);
				expect(actionBounds.x + actionBounds.width).toBeLessThanOrEqual(
					scrollerBounds.x + scrollerBounds.width
				);
				await scroller.screenshot({
					path: testInfo.outputPath('mobile-offering-table-actions.png'),
					animations: 'disabled'
				});
				await testInfo.attach('mobile-offering-scroll-metrics', {
					body: JSON.stringify({
						offering: 'คณิตศาสตร์ต้น',
						initial: initialScroll,
						statusScroll,
						actionScroll: await scroller.evaluate((element) => element.scrollLeft),
						warningBounds,
						actionBounds,
						scrollerBounds
					}),
					contentType: 'application/json'
				});
			}
			expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
				width
			);
			expect(
				fixture.academicReads.filter((path) => path.includes('management-options'))
			).toHaveLength(0);
			expect(fixture.writes).toEqual([]);
		});
	}

test('offering group facts remain scoped to selected snapshot when room comparison context is unavailable', async ({
	page
}) => {
	const fixture = await mockReviewStatuses(page, true);
	await page.goto(`${fixture.origin}&view=offerings`);
	const region = page.getByRole('region', { name: 'มุมมองรายวิชาและกิจกรรม', exact: true });
	const row = region.getByRole('row').filter({ hasText: 'คณิตศาสตร์ต้น' });
	await expect(row).toContainText('ยังไม่มีผลเทียบรายห้อง');
	await expect(row).not.toContainText('ตรงกับหลักสูตร');
	await expect(row).toContainText('ยังไม่มีครูหลัก 1 กลุ่ม');
	expect(fixture.writes).toEqual([]);
	expect(fixture.academicReads.filter((path) => path.includes('management-options'))).toHaveLength(
		0
	);
});

test('an opened course without real groups waits for grouping instead of claiming missing teachers', async ({
	page
}) => {
	const fixture = await mockReviewStatuses(page);
	const item = fixture.workspace.homerooms[1].items[0];
	const course = fixture.delivery.offerings.find((source) => source.id === navOfferings.basic21)!;
	item.offeringId = course.id;
	item.catalogVersionId = navOfferings.basic21;
	item.name = course.name;
	item.code = course.code;
	item.requirementKind = 'required';
	item.offeringState = 'present';
	item.weeklyPeriodTarget = 0;
	item.alignmentStates = ['matches_curriculum'];
	const target = new URL(fixture.origin, 'http://schoolorbit.test');
	target.searchParams.set('expandedRooms', navIds.roomB);
	await page.goto(target.pathname + target.search);
	const room = page
		.locator('details')
		.filter({ has: page.getByRole('heading', { name: 'ม.1/2', exact: true }) });
	const row = room.getByRole('row').filter({ hasText: item.name });
	await expect(row.getByRole('cell').nth(4)).toHaveText('รอจัดกลุ่ม');
	await expect(row).toContainText('ยังไม่จัดกลุ่ม');
	await expect(row).not.toContainText('ยังไม่มีครูหลัก');
	expect(fixture.writes).toEqual([]);
});

test('healthy course and supported central activity have no review badges or empty status rows in either view', async ({
	page
}) => {
	const fixture = await mockReviewStatuses(page);
	const course = fixture.delivery.offerings.find((item) => item.id === navOfferings.basic21)!;
	course.weeklyPeriodTarget = 2;
	course.groups[0].teachers = [
		{
			assignmentId: 'd0000000-0000-4000-8000-000000000121',
			rowVersion: 1,
			isActive: true,
			teacherId: navIds.actor,
			displayName: 'ครูหลัก',
			role: 'primary'
		}
	];
	const roomA = fixture.workspace.homerooms[0];
	roomA.extraOfferings = [];
	roomA.items[0].alignmentStates = ['matches_curriculum'];
	roomA.items[0].weeklyPeriodTarget = 2;
	roomA.items[0].teacherState = 'assigned';
	roomA.items[0].groups[0].primaryTeacherCount = 1;
	await page.goto(fixture.origin);
	for (const name of ['ม.1/1', 'ม.1/3']) {
		const room = page
			.locator('details')
			.filter({ has: page.getByRole('heading', { name, exact: true }) });
		const summary = room.locator('summary');
		await expect(summary.getByTestId('delivery-room-statuses')).toHaveCount(0);
		await expect(summary).toContainText('1/1');
		await expect(summary).not.toContainText('ครบตามโครงสร้าง');
		await expect(summary.getByText('ม.1', { exact: true })).toBeVisible();
		await summary.click();
		const item = room
			.getByRole('row')
			.filter({ hasText: name === 'ม.1/1' ? 'คณิตศาสตร์ต้น' : 'ชุมนุม' });
		await expect(item.getByTestId('delivery-status-badges')).toHaveCount(0);
		await expect(item.getByRole('cell').nth(1)).toHaveText('');
		await expect(item.getByRole('cell').nth(2)).toHaveText('เปิดสอนแล้ว');
		await expect(item.getByRole('cell').nth(4)).toHaveText(
			name === 'ม.1/1' ? 'มอบหมายแล้ว' : 'กำหนดภายหลัง'
		);
	}
	await page.getByRole('tab', { name: 'มุมมองรายวิชา/กิจกรรม', exact: true }).click();
	const region = page.getByRole('region', { name: 'มุมมองรายวิชาและกิจกรรม', exact: true });
	for (const name of ['คณิตศาสตร์ต้น', 'ชุมนุม']) {
		const item = region.getByRole('row').filter({ hasText: name });
		await expect(item.getByRole('cell').nth(1)).toHaveText('');
		await expect(item.getByRole('cell').nth(1).locator('[data-slot="badge"]')).toHaveCount(0);
		await expect(item.getByTestId('delivery-status-badges')).toHaveCount(0);
		await expect(item.locator('.mt-2')).toHaveCount(0);
		await expect(item).not.toContainText('ยังไม่มีผลเทียบ');
	}
	expect(fixture.writes).toEqual([]);
	expect(fixture.academicReads.filter((path) => path.includes('management-options'))).toHaveLength(
		0
	);
});
