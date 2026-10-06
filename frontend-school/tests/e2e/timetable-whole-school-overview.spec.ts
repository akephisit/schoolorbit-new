import { expect, test } from '@playwright/test';

import {
	installTimetableMock,
	makeTimetableBlock,
	makeStructuralTimetableBlock,
	makeSynchronizedTimetableBlock,
	timetableIds
} from './timetable-test-harness';

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
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();

	await expect(page.getByRole('button', { name: 'โรงเรียน' })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect(page.getByText('ภาพรวมทั้งโรงเรียน · วันจันทร์')).toBeVisible();
	await expect(page.getByRole('button', { name: /ค21101/ })).toBeVisible();
	const card = page.locator('[data-timetable-lesson-card]');
	await expect(card).toHaveCount(1);
	await expect(card).toHaveAttribute('draggable', 'false');
	await expect(card.getByText('ครูคณิตศาสตร์', { exact: true })).toBeVisible();
	await expect(card.getByText('MATH-1', { exact: true })).toBeVisible();
	await expect(card.getByRole('button', { name: /ออกจากตาราง/ })).toHaveCount(0);
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
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();

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
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();

	await page.getByRole('button', { name: 'ชั้น' }).click();
	await expect(page).toHaveURL(new RegExp(`view=homeroom.*ownerId=${timetableIds.homeroom}`));
	await expect(page.locator(`article[data-block-id="${timetableIds.blockA}"]`)).toBeVisible();
});

test('uses full neutral cards without activity codes in the school overview', async ({ page }) => {
	const activity = makeTimetableBlock(timetableIds.blockA, timetableIds.period1, {
		code: 'OTHER-internal-activity-id',
		name: 'กิจกรรมอิสระ'
	});
	activity.blockKind = 'activity';
	await installTimetableMock(page, {
		blocks: [
			activity,
			makeSynchronizedTimetableBlock(timetableIds.blockB, timetableIds.period2),
			makeStructuralTimetableBlock(
				timetableIds.createdBlock,
				timetableIds.period3,
				'กิจกรรมหน้าเสาธง'
			)
		]
	});
	await page.goto(wholeSchoolUrl());
	const cards = page.locator('[data-timetable-lesson-card]');
	await expect(cards).toHaveCount(3);
	await expect(cards.getByText('OTHER-internal-activity-id', { exact: true })).toHaveCount(0);
	await expect(cards.getByText('CLUB', { exact: true })).toHaveCount(0);
	await expect(cards.getByText('กิจกรรมหน้าเสาธง', { exact: true })).toHaveCount(1);
	await expect(cards.first().getByText('ครูคณิตศาสตร์', { exact: true })).toBeVisible();
	await expect(cards.first().getByText('MATH-1', { exact: true })).toBeVisible();
	const metrics = await cards.evaluateAll((elements) =>
		elements.map((element) => {
			const style = getComputedStyle(element);
			const cell = element.closest('td')!;
			return {
				leftWidth: style.borderLeftWidth,
				topWidth: style.borderTopWidth,
				leftColor: style.borderLeftColor,
				topColor: style.borderTopColor,
				cardHeight: element.getBoundingClientRect().height,
				cellHeight: cell.getBoundingClientRect().height
			};
		})
	);
	for (const metric of metrics) {
		expect(metric.leftWidth).toBe(metric.topWidth);
		expect(metric.leftColor).toBe(metric.topColor);
		expect(metric.cardHeight).toBeGreaterThanOrEqual(metric.cellHeight - 14);
	}
});
