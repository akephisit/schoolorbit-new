import { expect, test, type Route } from '@playwright/test';

import { installTimetableMock, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });

function boardUrl(versionId?: string): string {
	const params = new URLSearchParams({
		academicYearId: timetableIds.year,
		academicTermId: timetableIds.term
	});
	if (versionId) params.set('timetableVersionId', versionId);
	return `/staff/academic/timetable?${params}`;
}

function templatesUrl(): string {
	return `/staff/academic/timetable/templates?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}`;
}

function fulfill(route: Route, data: unknown): Promise<void> {
	return route.fulfill({
		status: 200,
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

test('explicit board workspace paints before delayed versions and change-set siblings', async ({
	page
}) => {
	await installTimetableMock(page);
	const versions: Route[] = [];
	const workspaces: Route[] = [];
	const changeSets: Route[] = [];
	await page.route('**/api/academic/timetable-versions?**', (route) => {
		versions.push(route);
	});
	await page.route('**/api/academic/timetable-blocks/workspace?**', (route) => {
		workspaces.push(route);
	});
	await page.route(`**/api/academic/term-change-sets/${timetableIds.changeSet}`, (route) => {
		changeSets.push(route);
	});

	await page.goto(boardUrl(timetableIds.draftVersion));
	await expect.poll(() => versions.length).toBe(1);
	await expect.poll(() => workspaces.length).toBe(1);
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await workspaces[0].fallback();
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	await expect.poll(() => changeSets.length).toBe(1);
	await expect(page.getByText('กำลังโหลดรุ่น...')).toBeVisible();
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await changeSets[0].fallback();
	await expect(page.getByTestId('timetable-change-set-ready')).toBeVisible();
	await versions[0].fallback();
	await expect(page.getByText('กำลังโหลดรุ่น...')).toHaveCount(0);
	expect(workspaces).toHaveLength(1);
});

test('default board waits only for its version selection, then starts one workspace', async ({
	page
}) => {
	await installTimetableMock(page);
	const versions: Route[] = [];
	let workspaceReads = 0;
	await page.route('**/api/academic/timetable-versions?**', (route) => {
		versions.push(route);
	});
	await page.route('**/api/academic/timetable-blocks/workspace?**', (route) => {
		workspaceReads += 1;
		return route.fallback();
	});
	await page.goto(boardUrl());
	await expect.poll(() => versions.length).toBe(1);
	expect(workspaceReads).toBe(0);
	await versions[0].fallback();
	await expect.poll(() => workspaceReads).toBe(1);
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
});

test('an obsolete version deep link falls back to the first available board', async ({ page }) => {
	await installTimetableMock(page, { requestedWorkspaceVersion: true });
	const missingVersion = 'a1000000-0000-4000-8000-000000000999';
	let requestedReads = 0;
	let fallbackReads = 0;
	await page.route('**/api/academic/timetable-blocks/workspace?**', (route) => {
		const versionId = new URL(route.request().url()).searchParams.get('timetableVersionId');
		if (versionId === missingVersion) {
			requestedReads += 1;
			return route.fulfill({
				status: 404,
				contentType: 'application/json',
				body: JSON.stringify({ success: false, error: 'ไม่พบรุ่นตาราง' })
			});
		}
		fallbackReads += 1;
		return route.fallback();
	});
	await page.goto(boardUrl(missingVersion));
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	await expect(page).toHaveURL(new RegExp(`timetableVersionId=${timetableIds.draftVersion}`));
	expect(requestedReads).toBe(1);
	expect(fallbackReads).toBe(1);
});

test('templates render independently while versions are delayed, then retry only versions', async ({
	page
}) => {
	await installTimetableMock(page);
	const versions: Route[] = [];
	let templateReads = 0;
	await page.route('**/api/academic/timetable-templates', (route) => {
		templateReads += 1;
		return fulfill(route, []);
	});
	await page.route('**/api/academic/timetable-versions?**', (route) => {
		versions.push(route);
	});
	await page.goto(templatesUrl());
	await expect.poll(() => versions.length).toBe(1);
	await expect(page.getByText('ยังไม่มีแม่แบบ', { exact: true })).toBeVisible();
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	expect(templateReads).toBe(1);
	await versions[0].fulfill({
		status: 500,
		contentType: 'application/json',
		body: JSON.stringify({ success: false, error: 'รุ่นไม่พร้อม' })
	});
	await expect(page.getByText('โหลดรุ่นตารางสอนไม่สำเร็จ', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect.poll(() => versions.length).toBe(2);
	await versions[1].fallback();
	await expect(page.getByTestId('timetable-template-versions-ready')).toBeVisible();
	expect(templateReads).toBe(1);
});

test('new board version clears the old board before the selected workspace responds', async ({
	page
}) => {
	await installTimetableMock(page, { requestedWorkspaceVersion: true });
	const workspaces: Route[] = [];
	await page.route('**/api/academic/timetable-blocks/workspace?**', (route) => {
		workspaces.push(route);
	});
	await page.goto(boardUrl(timetableIds.publishedVersion));
	await expect.poll(() => workspaces.length).toBe(1);
	await workspaces[0].fallback();
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	await page.evaluate((href) => {
		const link = document.createElement('a');
		link.href = href;
		link.id = 'test-version-navigation';
		link.textContent = 'เปิดรุ่นใหม่';
		link.style.cssText = 'position:fixed;top:160px;left:500px;z-index:9999;background:white';
		document.body.append(link);
	}, boardUrl(timetableIds.draftVersion));
	await page.locator('#test-version-navigation').click();
	await expect.poll(() => workspaces.length).toBe(2);
	await expect(page.getByTestId('timetable-board-ready')).toHaveCount(0);
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await workspaces[1].fallback();
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
});

test('removing an explicit version URL does not retain that board while default selection loads', async ({
	page
}) => {
	await installTimetableMock(page, { requestedWorkspaceVersion: true });
	const workspaces: Route[] = [];
	await page.route('**/api/academic/timetable-blocks/workspace?**', (route) => {
		workspaces.push(route);
	});
	await page.goto(boardUrl(timetableIds.publishedVersion));
	await expect.poll(() => workspaces.length).toBe(1);
	await workspaces[0].fallback();
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	await page.evaluate((href) => {
		const link = document.createElement('a');
		link.href = href;
		link.id = 'test-default-version-navigation';
		link.textContent = 'เปิดรุ่นปริยาย';
		link.style.cssText = 'position:fixed;top:160px;left:500px;z-index:9999;background:white';
		document.body.append(link);
	}, boardUrl());
	await page.locator('#test-default-version-navigation').click();
	await expect.poll(() => workspaces.length).toBe(2);
	await expect(page.getByTestId('timetable-board-ready')).toHaveCount(0);
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await workspaces[1].fallback();
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
});
