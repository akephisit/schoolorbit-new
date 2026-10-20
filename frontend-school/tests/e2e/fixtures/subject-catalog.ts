import { expect, type Page, type Route } from '@playwright/test';
import type {
	CatalogSubjectOverview,
	CatalogSubjectOverviewItem,
	CreateCatalogSubjectRequest,
	CreateSubjectVersionRequest,
	PublishVersionRequest,
	SubjectVersion,
	UpdateCatalogSubjectRequest
} from '../../../src/lib/api/academic-core.js';
import {
	DEFAULT_APPEARANCE,
	schoolAppearanceCss,
	type SchoolAppearanceResponse
} from '../../../src/lib/appearance/model.js';
import { PERMISSIONS } from '../../../src/lib/permissions/registry.generated.js';

export const subjectPath = '/staff/academic/catalog/subjects';
export const subjectApi = '/api/academic/catalog/subjects';
export const subjectName = 'คณิตศาสตร์พื้นฐานและกระบวนการแก้ปัญหาอย่างสร้างสรรค์';
export const subjectGrade = {
	id: '30000000-0000-4000-8000-000000000001',
	code: 'M1',
	name: 'มัธยมศึกษาปีที่ 1',
	short_name: 'ม.1',
	level_order: 1,
	level_type: 'secondary'
};
const appearance: SchoolAppearanceResponse = {
	appearance: { ...DEFAULT_APPEARANCE, font: 'noto', primary: '#fff2c4' },
	revision: 4
};

export function subjectVersion(subjectId: string, versionNo = 1): SubjectVersion {
	return {
		id: `${subjectId}-v${versionNo}`,
		subjectId,
		versionNo,
		nameTh: versionNo === 1 ? subjectName : 'รายละเอียดฉบับร่าง',
		nameEn: 'Mathematics and creative problem solving',
		credit: '1.50',
		description: null,
		effectiveFrom: '2026-05-01',
		effectiveUntil: null,
		gradeLevelIds: [subjectGrade.id],
		hoursPerSemester: 60,
		periodsPerWeek: 3,
		subjectType: 'BASIC',
		termCode: null,
		status: versionNo === 1 ? 'published' : 'draft',
		rowVersion: 4,
		migrated: false,
		publishedAt: versionNo === 1 ? '2026-05-01T00:00:00Z' : null,
		createdAt: '2026-05-01T00:00:00Z',
		updatedAt: '2026-05-01T00:00:00Z'
	};
}

export function subjectFixture(count = 3, readOnly = false) {
	const groups: CatalogSubjectOverview['subjectGroupOptions'] = [
		{
			subjectGroupId: 'group-math',
			organizationUnitId: 'owner-math',
			code: 'MATH',
			name: 'คณิตศาสตร์',
			canManage: !readOnly
		},
		{
			subjectGroupId: 'group-science',
			organizationUnitId: 'owner-science',
			code: 'SCI',
			name: 'วิทยาศาสตร์และเทคโนโลยี',
			canManage: !readOnly
		},
		{
			subjectGroupId: 'group-thai',
			organizationUnitId: 'owner-thai',
			code: 'THAI',
			name: 'ภาษาไทย',
			canManage: false
		}
	];
	const grades = [subjectGrade];
	const histories = new Map<string, SubjectVersion[]>();
	const items = Array.from({ length: count }, (_, index): CatalogSubjectOverviewItem => {
		const id = `subject-${index + 1}`;
		const group = groups[index % groups.length];
		const version = subjectVersion(id);
		version.nameTh =
			index === 0 ? subjectName : `รายวิชาทดสอบ ${String(index + 1).padStart(2, '0')}`;
		if (index % 2) version.subjectType = 'ADDITIONAL';
		histories.set(id, [version]);
		return {
			subject: {
				id,
				code: `ค${String(index + 1).padStart(5, '0')}`,
				subjectGroupId: group.subjectGroupId,
				owningOrganizationUnitId: group.organizationUnitId,
				archivedAt: index === 1 ? '2026-10-01T00:00:00Z' : null,
				rowVersion: 2,
				createdAt: '2026-05-01T00:00:00Z',
				updatedAt: '2026-05-01T00:00:00Z'
			},
			displayVersion: version,
			displayState: 'current',
			draftCount: 0,
			gradeLevels: [subjectGrade],
			canManage: group.canManage
		};
	});
	return {
		items,
		groups,
		grades,
		histories,
		overview: (): CatalogSubjectOverview => ({
			items,
			subjectGroupOptions: groups,
			gradeLevelOptions: grades
		})
	};
}

export type SubjectFixture = ReturnType<typeof subjectFixture>;
export const replySubject = (route: Route, data: unknown) =>
	route.fulfill({ contentType: 'application/json', body: JSON.stringify({ success: true, data }) });
export const failSubject = (route: Route, status: number, error: string) =>
	route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify({ success: false, error })
	});

export function subjectGate() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { release, promise };
}

export async function installSubjectCatalog(
	page: Page,
	fixture: SubjectFixture,
	options: {
		mode?: 'light' | 'dark';
		readOnly?: boolean;
		onRequest?: (route: Route, url: URL) => Promise<boolean>;
	} = {}
) {
	const writes: Array<{ path: string; method: string; body: unknown }> = [];
	const historyReads = new Map<string, number>();
	const pageErrors: string[] = [];
	const unexpected: string[] = [];
	let overviewReads = 0;
	page.on('pageerror', (error) => pageErrors.push(error.message));
	await page.clock.setFixedTime(new Date('2026-10-20T05:00:00Z'));
	await page.addInitScript((theme) => {
		localStorage.setItem('ui-preferences', JSON.stringify({ theme }));
	}, options.mode ?? 'light');
	await page.route('**/school-appearance.css', (route) =>
		route.fulfill({ contentType: 'text/css', body: schoolAppearanceCss(appearance) })
	);
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const request = route.request();
			const url = new URL(request.url());
			const path = url.pathname;
			const method = request.method();
			if (path.startsWith('/api/academic/catalog/') && method !== 'GET')
				writes.push({ path, method, body: request.postDataJSON() });
			if (path === `${subjectApi}/overview` && method === 'GET') overviewReads++;
			const subjectId = path.match(/^\/api\/academic\/catalog\/subjects\/([^/]+)\/versions$/)?.[1];
			if (subjectId && method === 'GET')
				historyReads.set(subjectId, (historyReads.get(subjectId) ?? 0) + 1);
			if (options.onRequest && (await options.onRequest(route, url))) return;
			if (path === '/api/auth/me')
				return replySubject(route, {
					id: 'staff-test',
					username: 'synthetic-subject-reader',
					firstName: 'ทดสอบ',
					lastName: 'รายวิชา',
					displayName: 'ทดสอบ รายวิชา',
					userType: 'staff',
					status: 'ACTIVE',
					permissions: options.readOnly
						? [PERMISSIONS.ACADEMIC_CATALOG_READ_SCHOOL]
						: [
								PERMISSIONS.ACADEMIC_CATALOG_READ_SCHOOL,
								PERMISSIONS.ACADEMIC_CATALOG_MANAGE_ORGANIZATION_TREE
							]
				});
			if (path === `${subjectApi}/overview` && method === 'GET')
				return replySubject(route, fixture.overview());
			if (subjectId && method === 'GET')
				return replySubject(route, fixture.histories.get(subjectId) ?? []);
			if (subjectId && method === 'POST') {
				const body: CreateSubjectVersionRequest = request.postDataJSON();
				if (typeof body.credit !== 'string')
					return failSubject(route, 400, 'credit must be a JSON string');
				const versions = fixture.histories.get(subjectId) ?? [];
				const created = {
					...subjectVersion(subjectId, versions.length + 1),
					...body,
					status: 'draft' as const,
					publishedAt: null
				};
				fixture.histories.set(subjectId, [created, ...versions]);
				const item = fixture.items.find((item) => item.subject.id === subjectId);
				if (item) {
					item.draftCount += 1;
					if (!item.displayVersion) {
						item.displayVersion = created;
						item.displayState = 'unpublished';
					}
				}
				return replySubject(route, created);
			}
			if (path === subjectApi && method === 'POST') {
				const body: CreateCatalogSubjectRequest = request.postDataJSON();
				const group = fixture.groups.find((group) => group.subjectGroupId === body.subjectGroupId);
				if (!group?.canManage) return failSubject(route, 403, 'ไม่มีสิทธิ์จัดการกลุ่มสาระ');
				const created = {
					...subjectFixture(1).items[0].subject,
					...body,
					id: 'subject-created',
					owningOrganizationUnitId: group.organizationUnitId
				};
				fixture.items.push({
					subject: created,
					displayVersion: null,
					displayState: 'unpublished',
					draftCount: 0,
					gradeLevels: [],
					canManage: true
				});
				fixture.histories.set(created.id, []);
				return replySubject(route, created);
			}
			const publicationId = path.match(
				/^\/api\/academic\/catalog\/subject-versions\/([^/]+)\/publish$/
			)?.[1];
			if (publicationId && method === 'POST') {
				const body: PublishVersionRequest = request.postDataJSON();
				for (const [id, versions] of fixture.histories) {
					const version = versions.find((item) => item.id === publicationId);
					if (!version) continue;
					if (body.rowVersion !== version.rowVersion)
						return failSubject(route, 409, 'รุ่นข้อมูลเปลี่ยนแล้ว');
					version.status = 'published';
					version.rowVersion += 1;
					version.publishedAt = '2026-10-20T05:00:00Z';
					const item = fixture.items.find((item) => item.subject.id === id);
					if (item) {
						item.displayVersion = version;
						item.displayState = 'current';
						item.draftCount = Math.max(0, item.draftCount - 1);
					}
					return replySubject(route, version);
				}
				return failSubject(route, 404, 'ไม่พบรุ่นรายวิชา');
			}
			const metadataId = path.match(/^\/api\/academic\/catalog\/subjects\/([^/]+)$/)?.[1];
			if (metadataId && method === 'PATCH') {
				const body: UpdateCatalogSubjectRequest = request.postDataJSON();
				const item = fixture.items.find((item) => item.subject.id === metadataId);
				if (!item || item.subject.rowVersion !== body.rowVersion)
					return failSubject(route, 409, 'ข้อมูลรายวิชาเปลี่ยนแล้ว');
				item.subject = {
					...item.subject,
					code: body.code,
					subjectGroupId: body.subjectGroupId,
					rowVersion: item.subject.rowVersion + 1
				};
				return replySubject(route, item.subject);
			}
			if (path === '/api/menu/user') return replySubject(route, { groups: [] });
			if (path === '/api/academic/context/options')
				return replySubject(route, {
					years: [],
					terms: [],
					activeAcademicYearId: null,
					activeAcademicTermId: null
				});
			if (path === '/api/school/public/appearance') return replySubject(route, appearance);
			if (path === '/api/school/public')
				return replySubject(route, { schoolName: 'โรงเรียนทดสอบ' });
			if (path === '/api/notifications/stream')
				return route.fulfill({ contentType: 'text/event-stream', body: '' });
			if (path === '/api/notifications') return replySubject(route, { items: [], unread_count: 0 });
			if (path === '/api/me/work-items/counts')
				return replySubject(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
			if (path.startsWith('/api/academic/catalog/')) {
				unexpected.push(`${method} ${path}`);
				return failSubject(route, 404, 'ไม่มีข้อมูลทดสอบสำหรับคำขอนี้');
			}
			return replySubject(route, {});
		}
	);
	return { writes, historyReads, pageErrors, unexpected, overviewReads: () => overviewReads };
}

export async function openSubjectDetails(page: Page, code = 'ค00001') {
	await page.getByRole('button', { name: `เปิด ${code}`, exact: true }).click();
	await expect(page.getByRole('dialog')).toBeVisible();
}

export async function chooseSubjectDate(page: Page, date = '2026-10-21') {
	await page.getByRole('button', { name: 'เลือกวันที่เริ่มใช้', exact: true }).click();
	const target = new Date(`${date}T12:00:00Z`);
	await page.getByRole('button', { name: 'เลือกเดือน', exact: true }).click();
	await page
		.getByRole('option', {
			name: new Intl.DateTimeFormat('th-TH', { month: 'short', timeZone: 'UTC' }).format(target),
			exact: true
		})
		.click();
	await page.getByRole('button', { name: 'เลือกปี', exact: true }).click();
	await page
		.getByRole('option', {
			name: new Intl.DateTimeFormat('th-TH', { year: 'numeric', timeZone: 'UTC' }).format(target),
			exact: true
		})
		.click();
	await page.locator(`[data-bits-day][data-value="${date}"]`).click();
}

export async function fillSubjectVersion(page: Page, name: string) {
	await page.getByLabel('ชื่อภาษาไทย', { exact: true }).fill(name);
	await page.getByLabel('ชื่อภาษาอังกฤษ', { exact: true }).fill('Synthetic revised mathematics');
	await page.getByLabel('หน่วยกิต', { exact: true }).fill('1.75');
	await page.getByLabel('คาบมาตรฐานต่อสัปดาห์', { exact: true }).fill('3');
	await page.getByLabel('ชั่วโมงรวมต่อภาคเรียน', { exact: true }).fill('60');
	await page.getByRole('combobox', { name: /เลือกระดับชั้นสำหรับ/ }).click();
	const grade = page.getByRole('option', { name: /มัธยมศึกษาปีที่ 1/ });
	if (!(await grade.getByRole('checkbox').isChecked())) await grade.click();
	await page.keyboard.press('Escape');
	await chooseSubjectDate(page);
}
