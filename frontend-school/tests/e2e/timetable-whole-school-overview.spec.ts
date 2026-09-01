import { expect, test } from '@playwright/test';

import { installTimetableMock, makeTimetableBlock, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

function wholeSchoolUrl(): string {
	return (
		`/staff/academic/timetable?academicYearId=${timetableIds.year}` +
		`&academicTermId=${timetableIds.term}` +
		`&timetableVersionId=${timetableIds.draftVersion}&view=wholeSchool`
	);
}

test('derives the read-only school matrix from the same bounded block workspace', async ({
	page
}) => {
	const mock = await installTimetableMock(page, {
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	await page.goto(wholeSchoolUrl());

	await expect(page.getByRole('button', { name: 'ทั้งโรงเรียน' })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect(page.getByText('ภาพรวมทั้งโรงเรียน · วันจันทร์')).toBeVisible();
	await expect(page.getByRole('button', { name: /ค21101/ })).toBeVisible();
	await expect(page.locator('[data-timetable-lesson-card]')).toHaveCount(0);
	expect(mock.workspaceRequestCount()).toBe(1);
	expect(mock.createRequestCount()).toBe(0);
	expect(mock.updateRequestCount()).toBe(0);
	expect(mock.swapRequestCount()).toBe(0);
});

test('changes the displayed day locally and opens canonical block details', async ({ page }) => {
	await installTimetableMock(page, {
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	await page.goto(wholeSchoolUrl());

	await page.getByRole('button', { name: 'เลือกวันดูภาพรวม' }).click();
	await page.getByRole('option', { name: 'วันอังคาร' }).click();
	await expect(page.getByText('ภาพรวมทั้งโรงเรียน · วันอังคาร')).toBeVisible();
	await expect(page.getByRole('button', { name: /ค21101/ })).toHaveCount(0);

	await page.getByRole('button', { name: 'เลือกวันดูภาพรวม' }).click();
	await page.getByRole('option', { name: 'วันจันทร์' }).click();
	await page.getByRole('button', { name: /ค21101/ }).click();
	await expect(page.getByRole('heading', { name: 'รายละเอียดคาบ' })).toBeVisible();
});

test('switches from school overview to the exact editable homeroom board', async ({ page }) => {
	await installTimetableMock(page, {
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	await page.goto(wholeSchoolUrl());

	await page.getByRole('button', { name: 'ห้องประจำชั้น' }).click();
	await expect(page).toHaveURL(new RegExp(`view=homeroom.*ownerId=${timetableIds.homeroom}`));
	await expect(page.locator(`article[data-block-id="${timetableIds.blockA}"]`)).toBeVisible();
});
