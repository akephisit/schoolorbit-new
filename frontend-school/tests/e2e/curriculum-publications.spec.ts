import { expect, test, type Page, type Route } from '@playwright/test';
import {
	ids,
	edition,
	curriculumVersion,
	curriculumStructure
} from './helpers/curriculum-fixtures';

test.use({ serviceWorkers: 'block' });
const root = `/staff/academic/curricula/${ids.curriculum}`;
const draftId = 'f2000000-0000-4000-8000-000000000402';
const publicationIds = [
	'f2000000-0000-4000-8000-000000000401',
	'f2000000-0000-4000-8000-000000000403'
];
function respond(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}
async function mock(
	page: Page,
	options: {
		readOnly?: boolean;
		historical?: boolean;
		conflict?: boolean;
		delayFirst?: Promise<void>;
		failDetail?: boolean;
	} = {}
) {
	let count = options.historical ? 2 : 1;
	let token: string | null = null;
	let termTwo = options.historical ?? false;
	let detailCalls = 0;
	const writes: { path: string; body: Record<string, unknown> }[] = [];
	function header() {
		return {
			...edition(),
			publicationCount: count,
			currentPublicationId: publicationIds[count - 1],
			draftId: token,
			rowVersion: token ? 5 : 4
		};
	}
	function workspace(url: URL) {
		const editing = url.searchParams.get('draftId') === draftId && token === draftId;
		const publicationId = url.searchParams.get('publicationId') ?? publicationIds[count - 1];
		const level: ReturnType<typeof curriculumVersion> = {
			...curriculumVersion(ids.curriculumVersion),
			draftId: editing ? draftId : null,
			publicationId: editing ? null : publicationId,
			status: editing ? 'draft' : 'published',
			rowVersion: editing ? 5 : 4
		};
		const result = curriculumStructure(level);
		if (termTwo && (editing || publicationId === publicationIds[1]))
			result.termSlots.push({
				...result.termSlots[0],
				id: 'e2000000-0000-4000-8000-000000000402',
				sequence: 2,
				typeOccurrence: 2,
				name: 'ภาคเรียนที่ 2'
			});
		return result;
	}
	function publication(no: number) {
		return {
			id: publicationIds[no - 1],
			editionId: ids.curriculum,
			publicationNo: no,
			previousPublicationId: no === 1 ? null : publicationIds[0],
			name: edition().name,
			revisionYear: 2569,
			description: null,
			publishedAt: no === 1 ? null : '2026-10-07T00:00:00Z',
			capturedAt: '2026-10-07T00:00:00Z',
			publishedBy: no === 1 ? null : ids.user,
			publisherName: no === 1 ? null : 'ผู้ดูแล ทดสอบ',
			changeNote: no === 1 ? 'ข้อมูลจริงตั้งต้น' : 'เพิ่มภาคเรียนที่ 2',
			isBaseline: no === 1,
			levelCount: 1,
			programCount: 1,
			slotCount: no,
			courseCount: 1,
			activityCount: 0
		};
	}
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const path = url.pathname;
			const method = route.request().method();
			if (method !== 'GET' && path.startsWith('/api/academic/'))
				writes.push({ path, body: route.request().postDataJSON() });
			if (path === '/api/auth/me')
				return respond(route, {
					id: ids.user,
					username: 'publication-test',
					firstName: 'ผู้ดูแล',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					permissions: options.readOnly ? ['academic_curriculum.read.school'] : ['*']
				});
			if (path === '/api/menu/user') return respond(route, { groups: [] });
			if (path === '/api/academic/context/options')
				return respond(route, {
					activeAcademicYearId: null,
					activeAcademicTermId: null,
					years: [],
					terms: []
				});
			if (path === '/api/notifications') return respond(route, { items: [], unread_count: 0 });
			if (path.endsWith('/stream'))
				return route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
			if (path === `/api/academic/curricula/${ids.curriculum}/draft`) {
				if (options.conflict) return respond(route, 'ข้อมูลหลักสูตรเปลี่ยน กรุณาโหลดใหม่', 409);
				token = draftId;
				return respond(route, header());
			}
			if (path === `/api/academic/curricula/${ids.curriculum}/publish`) {
				expect(route.request().postDataJSON()).toEqual({
					draftId,
					rowVersion: 5,
					changeNote: 'เพิ่มภาคเรียนที่ 2'
				});
				count = 2;
				token = null;
				return respond(route, header());
			}
			if (path === `/api/academic/curricula/${ids.curriculum}`) return respond(route, header());
			if (path === `/api/academic/curricula/${ids.curriculum}/levels`)
				return respond(route, [{ level: workspace(url).level }]);
			if (path === `/api/academic/curricula/${ids.curriculum}/publications`)
				return respond(
					route,
					Array.from({ length: count }, (_, i) => publication(count - i))
				);
			if (path.includes('/publications/')) {
				detailCalls++;
				if (options.failDetail && detailCalls === 1)
					return respond(route, 'รายละเอียดไม่พร้อม', 503);
				const no = path.endsWith(publicationIds[0]) ? 1 : 2;
				if (no === 1 && options.delayFirst) await options.delayFirst;
				return respond(route, {
					publication: publication(no),
					changes:
						no === 1
							? []
							: [
									{
										resourceKind: 'term_slot',
										name: 'ภาคเรียนที่ 2',
										before: null,
										after: 'ภาคเรียนปกติ ลำดับที่ 2'
									}
								]
				});
			}
			if (path === `/api/academic/curriculum-levels/${ids.curriculumVersion}/term-slots`) {
				const body = route.request().postDataJSON();
				expect(body.draftId).toBe(draftId);
				expect(body.slots.map((slot: { name: string }) => slot.name)).toEqual([
					'ภาคเรียนที่ 1',
					'ภาคเรียนที่ 2'
				]);
				termTwo = true;
				url.searchParams.set('draftId', draftId);
				return respond(route, workspace(url));
			}
			if (path === `/api/academic/curriculum-levels/${ids.curriculumVersion}/structure`)
				return respond(route, workspace(url));
			if (path.endsWith('/management-options'))
				return respond(route, {
					academicYears: [],
					gradeLevels: curriculumStructure().gradeLevels,
					catalogVersions: []
				});
			return respond(route, {});
		}
	);
	return { writes };
}

test('an amendment adds term two, publishes the whole edition, and keeps publication one read-only', async ({
	page
}) => {
	const state = await mock(page);
	await page.goto(root);
	await page.getByRole('button', { name: 'แก้ไขหลักสูตร', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`draftId=${draftId}`));
	await page.getByRole('link', { name: 'ระดับมัธยมศึกษาตอนต้น' }).click();
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog
		.getByRole('button', { name: 'ภาคเรียนปกติ', exact: true })
		.and(dialog.locator('[data-slot=button]'))
		.click();
	await expect(dialog.getByRole('textbox', { name: 'ชื่อภาคเรียน' }).last()).toHaveValue(
		'ภาคเรียนที่ 2'
	);
	await dialog.getByRole('button', { name: 'บันทึกภาคเรียน', exact: true }).click();
	await expect(dialog).not.toBeVisible();
	await page.goto(`${root}?draftId=${draftId}`);
	await page.getByLabel('สรุปการแก้ไข').fill('เพิ่มภาคเรียนที่ 2');
	await page.getByRole('button', { name: 'เผยแพร่ทั้งฉบับ · ครั้งที่ 2' }).click();
	await expect(page).toHaveURL(root);
	await expect(page.getByText('เผยแพร่ครั้งที่ 2', { exact: true })).toBeVisible();
	await page.getByRole('link', { name: 'ประวัติการแก้ไข' }).click();
	await expect(page.getByTestId('curriculum-publication-history')).toContainText(
		'เพิ่ม · ภาคเรียนที่ 2'
	);
	await page.getByRole('button', { name: /เผยแพร่ครั้งที่ 1/ }).click();
	await expect(page.getByTestId('curriculum-publication-history')).toContainText(
		'ตั้งต้นจากข้อมูลจริง'
	);
	await page.getByRole('link', { name: 'ระดับมัธยมศึกษาตอนต้น' }).click();
	await expect(page).toHaveURL(new RegExp(`publicationId=${publicationIds[0]}`));
	await expect(page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true })).toHaveCount(0);
	await expect(page.getByText('ภาคเรียนที่ 2', { exact: true })).toHaveCount(0);
	expect(state.writes.map((write) => write.path)).toEqual([
		`/api/academic/curricula/${ids.curriculum}/draft`,
		`/api/academic/curriculum-levels/${ids.curriculumVersion}/term-slots`,
		`/api/academic/curricula/${ids.curriculum}/publish`
	]);
});

test('a stale edition conflict leaves publication one intact and creates no draft view', async ({
	page
}) => {
	await mock(page, { conflict: true });
	await page.goto(root);
	await page.getByRole('button', { name: 'แก้ไขหลักสูตร', exact: true }).click();
	await expect(page.getByText('ข้อมูลหลักสูตรเปลี่ยน กรุณาโหลดใหม่')).toBeVisible();
	await expect(page).toHaveURL(root);
	await expect(page.getByText('เผยแพร่ครั้งที่ 1', { exact: true })).toBeVisible();
});

test('a reader sees the published edition and history without amendment actions', async ({
	page
}) => {
	const state = await mock(page, { readOnly: true, historical: true });
	await page.goto(root);
	await expect(page.getByText('เผยแพร่ครั้งที่ 2', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'แก้ไขหลักสูตร', exact: true })).toHaveCount(0);
	await page.getByRole('link', { name: 'ประวัติการแก้ไข' }).click();
	await expect(page.getByTestId('curriculum-publication-history')).toContainText('อ่านอย่างเดียว');
	expect(state.writes).toEqual([]);
});

test('rapid history selection ignores a delayed previous publication', async ({ page }) => {
	let release = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	await mock(page, { historical: true, delayFirst: gate });
	await page.goto(`${root}/history`);
	await expect(page.getByTestId('curriculum-publication-history')).toContainText(
		'เผยแพร่ครั้งที่ 2'
	);
	await page.getByRole('button', { name: /เผยแพร่ครั้งที่ 1/ }).click();
	await page.getByRole('button', { name: /เผยแพร่ครั้งที่ 2/ }).click();
	await expect(page.getByTestId('curriculum-publication-history')).toContainText(
		'เพิ่ม · ภาคเรียนที่ 2'
	);
	release();
	await expect(page.getByTestId('curriculum-publication-history')).not.toContainText(
		'ตั้งต้นจากข้อมูลจริง'
	);
});

test('history detail retries independently while its educational levels remain available', async ({
	page
}) => {
	await mock(page, { historical: true, failDetail: true });
	await page.goto(`${root}/history`);
	await expect(page.getByText('โหลดรายละเอียดไม่สำเร็จ', { exact: true })).toBeVisible();
	await expect(page.getByRole('link', { name: 'ระดับมัธยมศึกษาตอนต้น' })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByTestId('curriculum-publication-history')).toContainText(
		'เพิ่ม · ภาคเรียนที่ 2'
	);
});

for (const viewport of [
	{ width: 1440, height: 960 },
	{ width: 390, height: 844 }
]) {
	for (const theme of ['light', 'dark'] as const) {
		test(`publication history layout ${viewport.width} ${theme}`, async ({ page }, testInfo) => {
			await page.setViewportSize(viewport);
			await mock(page, { historical: true });
			await page.goto(`${root}/history`);
			await expect(page.getByTestId('curriculum-publication-history')).toContainText(
				'เพิ่ม · ภาคเรียนที่ 2'
			);
			if (theme === 'dark') {
				await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
				await expect(page.locator('html')).toHaveClass(/dark/);
			} else await expect(page.locator('html')).not.toHaveClass(/dark/);
			expect(
				await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
			).toBe(true);
			await page.screenshot({
				path: testInfo.outputPath(`history-${viewport.width}-${theme}.png`),
				fullPage: true
			});
		});
	}
}
