import { expect, type Page, type Route } from '@playwright/test';
import type {
	CurriculumManagementOptions,
	CurriculumStructureRequirement,
	ReplaceCurriculumStructureRequest
} from '../../../src/lib/api/academic-core.js';
import { DEFAULT_APPEARANCE, schoolAppearanceCss } from '../../../src/lib/appearance/model.js';
import {
	curriculumStructure,
	curriculumVersion,
	edition,
	ids
} from '../helpers/curriculum-fixtures';

export const curriculumDraftId = 'f2000000-0000-4000-8000-000000000402';
export const curriculumCatalogPath = `/staff/academic/curricula/${ids.curriculum}/levels/${ids.curriculumVersion}?draftId=${curriculumDraftId}`;
export const currentSubjectVersionId = 'a2000000-0000-4000-8000-000000000402';

export function curriculumCatalogFixture(
	options: {
		empty?: boolean;
		omitOldOption?: boolean;
		legacyDuplicates?: boolean;
		readOnly?: boolean;
		longCatalog?: boolean;
	} = {}
) {
	const workspace = curriculumStructure({
		...curriculumVersion(ids.curriculumVersion, options.readOnly ? 'published' : 'draft'),
		editionId: ids.curriculum
	});
	workspace.gradeLevels = [1, 2, 3].map((grade) => ({
		...workspace.gradeLevels[0],
		id: grade === 1 ? ids.grade : `72000000-0000-4000-8000-00000000040${grade}`,
		code: `M${grade}`,
		name: `มัธยมศึกษาปีที่ ${grade}`,
		short_name: `ม.${grade}`,
		level_order: 300 + grade
	}));
	workspace.level.gradeLevelIds = workspace.gradeLevels.map((grade) => grade.id);
	workspace.termSlots.push({
		...workspace.termSlots[0],
		id: 'e2000000-0000-4000-8000-000000000402',
		sequence: 2,
		name: 'ภาคเรียนที่ 2',
		typeOccurrence: 2
	});
	workspace.programs.push({
		...workspace.programs[0],
		id: '62000000-0000-4000-8000-000000000402',
		code: 'SECOND',
		nameTh: 'แผนวิทย์คณิต',
		isDefault: false
	});
	workspace.requirements[0].hasNewerCurrentVersion = true;
	const original = structuredClone(workspace.requirements[0]);
	const current: CurriculumManagementOptions['catalogVersions'][number] = {
		id: currentSubjectVersionId,
		catalogId: ids.catalog,
		resourceKind: 'course',
		code: original.code,
		name: original.name,
		versionNo: 2,
		effectiveFrom: '2026-05-01',
		effectiveUntil: null,
		selectableForNewRequirement: true
	};
	const old = {
		...current,
		id: ids.catalogVersion,
		versionNo: 1,
		selectableForNewRequirement: false
	};
	const catalog: CurriculumManagementOptions['catalogVersions'] = [
		...(options.omitOldOption ? [] : [old]),
		current,
		{
			...current,
			id: 'subject-future-v3',
			versionNo: 3,
			effectiveFrom: '2027-05-01',
			selectableForNewRequirement: false
		},
		{
			...current,
			id: 'subject-expired',
			catalogId: 'expired-subject',
			code: 'วหมดอายุ',
			name: 'วิชาที่หมดอายุ',
			effectiveUntil: '2026-01-01',
			selectableForNewRequirement: false
		},
		{
			...current,
			id: 'activity-v1',
			catalogId: 'activity-stable',
			resourceKind: 'activity',
			code: 'A01',
			name: 'กิจกรรมชุมนุม',
			versionNo: 1,
			effectiveFrom: '2027-05-01'
		},
		{
			...current,
			id: 'activity-v2',
			catalogId: 'activity-stable',
			resourceKind: 'activity',
			code: 'A01',
			name: 'กิจกรรมชุมนุม',
			versionNo: 2,
			effectiveFrom: '2027-06-01'
		}
	];
	if (options.longCatalog)
		for (let index = 0; index < 18; index++)
			catalog.push({
				...current,
				id: `other-version-${index}`,
				catalogId: `other-subject-${index}`,
				code: `ท${index}`,
				name: `รายวิชาสำหรับตรวจการเลื่อน ${index}`
			});
	if (options.empty) workspace.requirements = [];
	if (options.legacyDuplicates)
		workspace.requirements.push({
			...original,
			id: 'legacy-requirement-v2',
			catalogVersionId: current.id,
			versionNo: 2,
			hasNewerCurrentVersion: false,
			displayOrder: 2
		});
	return {
		workspace,
		original,
		current,
		catalog,
		managementOptions: {
			gradeLevels: workspace.gradeLevels,
			catalogVersions: catalog
		} satisfies CurriculumManagementOptions
	};
}

export async function installCurriculumCatalog(
	page: Page,
	fixture: ReturnType<typeof curriculumCatalogFixture>,
	options: { mode?: 'light' | 'dark'; failSaveOnce?: boolean } = {}
) {
	const writes: Array<{ path: string; method: string; body: unknown }> = [];
	const errors: string[] = [];
	const unexpected: string[] = [];
	let saveAttempts = 0;
	page.on('pageerror', (error) => errors.push(error.message));
	await page.clock.setFixedTime(new Date('2026-10-21T05:00:00Z'));
	await page.addInitScript(
		(theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })),
		options.mode ?? 'light'
	);
	const appearance = { appearance: DEFAULT_APPEARANCE, revision: 1 };
	await page.route('**/school-appearance.css', (route) =>
		route.fulfill({ contentType: 'text/css', body: schoolAppearanceCss(appearance) })
	);
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const request = route.request();
			const url = new URL(request.url());
			const path = url.pathname;
			const method = request.method();
			if (method !== 'GET' && path.startsWith('/api/academic/'))
				writes.push({ path, method, body: request.postDataJSON() });
			if (path === '/api/auth/me')
				return reply(route, {
					id: ids.user,
					username: 'synthetic-curriculum',
					firstName: 'ทดสอบ',
					lastName: 'หลักสูตร',
					displayName: 'ทดสอบ หลักสูตร',
					userType: 'staff',
					status: 'ACTIVE',
					permissions: ['*']
				});
			if (path === `/api/academic/curricula/${ids.curriculum}`)
				return reply(route, { ...edition(), draftId: fixture.workspace.level.draftId });
			if (path === `/api/academic/curriculum-levels/${ids.curriculumVersion}/structure`)
				return reply(route, fixture.workspace);
			if (path === `/api/academic/curriculum-levels/${ids.curriculumVersion}/management-options`)
				return reply(route, fixture.managementOptions);
			if (
				path.startsWith('/api/academic/study-programs/') &&
				path.endsWith('/structure') &&
				method === 'PUT'
			) {
				if (options.failSaveOnce && ++saveAttempts === 1)
					return reply(
						route,
						'รุ่นรายวิชาที่เลือกไม่ใช่รุ่นปัจจุบันแล้ว กรุณาตรวจสอบทะเบียนก่อนบันทึก',
						409
					);
				const body: ReplaceCurriculumStructureRequest = request.postDataJSON();
				const programId = path.split('/')[4];
				const requirements = body.requirements.map(
					(input, index): CurriculumStructureRequirement => {
						const catalog = fixture.catalog.find((item) => item.id === input.catalogVersionId);
						const existing = fixture.workspace.requirements.find(
							(item) => item.catalogVersionId === input.catalogVersionId
						);
						if (!catalog && !existing) throw new Error('Unexpected fixture catalog version');
						return {
							...fixture.original,
							...input,
							id: `saved-${index}`,
							studyProgramId: programId,
							gradeLevel: fixture.workspace.gradeLevels.find(
								(grade) => grade.id === input.gradeLevelId
							)!,
							catalogId: existing?.catalogId ?? catalog!.catalogId,
							versionNo: existing?.versionNo ?? catalog!.versionNo,
							hasNewerCurrentVersion: existing?.hasNewerCurrentVersion ?? false,
							code: existing?.code ?? catalog!.code,
							name: existing?.name ?? catalog!.name
						};
					}
				);
				fixture.workspace.requirements = [
					...fixture.workspace.requirements.filter((item) => item.studyProgramId !== programId),
					...requirements
				];
				return reply(route, fixture.workspace);
			}
			if (path === '/api/academic/curricula/overview')
				return reply(route, {
					items: [{ edition: edition(), levelCount: 1, studyProgramCount: 1 }]
				});
			if (path === `/api/academic/curricula/${ids.curriculum}/levels`)
				return reply(route, [{ level: curriculumVersion(ids.curriculumVersion) }]);
			if (path === `/api/academic/curriculum-levels/${ids.curriculumVersion}/programs`)
				return reply(route, [{ ...fixture.workspace.programs[0], status: 'published' }]);
			if (
				path === `/api/academic/curriculum-levels/${ids.curriculumVersion}/copy-program` &&
				method === 'POST'
			) {
				const copied = {
					...fixture.workspace.programs[0],
					id: 'copied-program',
					nameTh: 'แผนคัดลอก',
					isDefault: false
				};
				fixture.workspace.programs.push(copied);
				fixture.workspace.requirements.push({
					...fixture.original,
					id: 'copied-requirement',
					studyProgramId: copied.id
				});
				return reply(route, copied);
			}
			if (path === '/api/menu/user') return reply(route, { groups: [] });
			if (path === '/api/academic/context/options')
				return reply(route, {
					years: [],
					terms: [],
					activeAcademicYearId: null,
					activeAcademicTermId: null
				});
			if (path === '/api/school/public/appearance') return reply(route, appearance);
			if (path === '/api/school/public') return reply(route, { schoolName: 'โรงเรียนทดสอบ' });
			if (path === '/api/notifications/stream')
				return route.fulfill({ contentType: 'text/event-stream', body: '' });
			if (path === '/api/notifications') return reply(route, { items: [], unread_count: 0 });
			if (path === '/api/me/work-items/counts')
				return reply(route, { open: 0, dueSoon: 0, overdue: 0, submitted: 0, closed: 0, total: 0 });
			if (path.startsWith('/api/academic/')) {
				unexpected.push(`${method} ${path}`);
				return reply(route, 'คำขอทดสอบที่ไม่ได้กำหนด', 404);
			}
			return reply(route, {});
		}
	);
	return { writes, errors, unexpected };
}

export async function openCurriculumEditor(page: Page) {
	await page.goto(curriculumCatalogPath);
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	await expect(page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' })).toBeVisible();
}
