import { expect, test, type Page, type Route } from '@playwright/test';
import {
	ids,
	edition,
	curriculumVersion,
	curriculumStructure
} from './helpers/curriculum-fixtures';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

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
		programRequest: () => programBody,
		publishRequest: () => publishBody
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
	navigationVersion,
	navigationWorkspace
} from './fixtures/delivery-navigation-route-data';
import type { CurriculumOfferingPreview } from '../../src/lib/api/learning-delivery.js';

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
		noVersion?: boolean;
		failPreview?: boolean;
	} = {}
) {
	await mockDeliveryNavigation(page, { manager: options.manager, readiness: 'course' });
	const workspace = navigationWorkspace(),
		version = navigationVersion();
	const kind = options.kind ?? 'course',
		schedulingMode = kind === 'activity' ? (options.schedulingMode ?? 'independent') : null;
	const oldId = kind === 'course' ? navOfferings.basic21 : navOfferings.club;
	const old = version.snapshot.offerings.find((item) => item.id === oldId)!;
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
				startsOn: null,
				endsOn: null,
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
	if (options.noVersion) {
		workspace.deliveryVersionId = null;
		workspace.deliveryVersionStatus = null;
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
	await page.route(`**/api/academic/delivery-versions/${navIds.version}`, (route) =>
		fulfill(route, version)
	);
	if (options.noVersion)
		await page.route('**/api/academic/delivery-versions?*', (route) => fulfill(route, []));
	await page.route('**/api/academic/offerings/preview-from-curriculum', (route) => {
		previewAttempts++;
		if (options.failPreview && previewAttempts === 1)
			return fulfill(route, 'ฉบับทะเบียนนี้เริ่มใช้หลังวันเริ่มภาคเรียน', 409);
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
				item.offeringState = 'draft';
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
	if (!options.noVersion) query.set('deliveryVersionId', navIds.version);
	return {
		workspace,
		version,
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
					name: 'ผลเทียบหลักสูตรของรุ่นเปิดสอน',
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
		await expect(preparation.getByRole('alert')).toContainText(
			'ฉบับทะเบียนนี้เริ่มใช้หลังวันเริ่มภาคเรียน'
		);
		await preparation.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
		await preparation
			.getByRole('button', {
				name: mode === 'course' ? 'ยืนยันเปิดรายวิชา' : 'เปิดใช้งานกิจกรรม',
				exact: true
			})
			.click();
		await expect(preparation).toHaveCount(0);
		const apply = fixture.academicWrites.find((write) =>
			write.path.endsWith('/apply-from-curriculum')
		)!;
		expect(apply.body).toMatchObject({
			academicTermId: navIds.term,
			studyProgramIds: [navIds.program],
			deliveryVersionId: navIds.version,
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
			fixture.version.snapshot.offerings.some(
				(item) => item.id === fixture.old.id && item.name === fixture.old.name
			)
		).toBe(true);
	});

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

test('missing curriculum without an opening version explains the create-draft prerequisite', async ({
	page
}) => {
	const fixture = await mockSharedComparison(page, { manager: true, noVersion: true });
	await page.goto(fixture.origin);
	await page.getByRole('button', { name: 'ตรวจเทียบหลักสูตร', exact: true }).click();
	const report = page.getByRole('dialog', { name: 'ผลเทียบหลักสูตรของรุ่นเปิดสอน', exact: true });
	await expect(report).toContainText('สร้างรุ่นเปิดสอนจากปุ่มด้านบนก่อนเปิดรายการตามหลักสูตร');
	await expect(report.getByRole('button', { name: /^เปิดตามหลักสูตร/ })).toHaveCount(0);
	expect(fixture.managementReads).toHaveLength(0);
	expect(fixture.academicWrites).toHaveLength(0);
});
