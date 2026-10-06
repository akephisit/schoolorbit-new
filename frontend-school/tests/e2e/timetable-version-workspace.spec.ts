import { expect, test } from '@playwright/test';

import { installTimetableMock, makeTimetableBlock, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

function timetableUrl(versionId: string): string {
	return (
		`/staff/academic/timetable?academicYearId=${timetableIds.year}` +
		`&academicTermId=${timetableIds.term}` +
		`&timetableVersionId=${versionId}` +
		`&view=homeroom&ownerId=${timetableIds.homeroom}`
	);
}

test('loads one bounded draft workspace and preserves exact attached teachers', async ({
	page
}) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1, {
		instructorIds: [timetableIds.teacherA, timetableIds.teacherB]
	});
	const mock = await installTimetableMock(page, {
		blocks: [block],
		eligibleInstructorIds: [timetableIds.teacherA]
	});

	await page.goto(timetableUrl(timetableIds.draftVersion));

	await expect(page.getByRole('heading', { name: 'ตารางสอน', exact: true })).toBeVisible();
	await expect(page.getByText('แบบร่าง · โหมดดู')).toBeVisible();
	await expect(page.getByText('ครูคณิตศาสตร์ +1')).toBeVisible();
	expect(mock.workspaceRequestCount()).toBe(1);

	await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
	await expect(page.getByRole('dialog').getByText('ครูคณิตศาสตร์ B')).toBeVisible();
});

test('renders a published timetable as the same read-only board', async ({ page }) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1, {
		versionId: timetableIds.publishedVersion
	});
	await installTimetableMock(page, { status: 'published', blocks: [block] });

	await page.goto(timetableUrl(timetableIds.publishedVersion));

	await expect(page.getByText('เผยแพร่แล้ว · โหมดดู')).toBeVisible();
	await expect(page.locator(`[data-block-id="${timetableIds.blockA}"]`)).toHaveAttribute(
		'draggable',
		'false'
	);
	await expect(page.getByRole('button', { name: /นำ .* ออกจากตาราง/ })).toHaveCount(0);

	await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('heading', { name: 'รายละเอียดคาบ' })).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'บันทึก' })).toHaveCount(0);
});

test('starts with a full read-only board and exposes placement controls only after Edit', async ({
	page
}) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	await installTimetableMock(page, { blocks: [block] });
	await page.goto(timetableUrl(timetableIds.draftVersion));
	await expect(page.locator('aside article')).toHaveCount(0);
	await expect(page.locator('article[data-block-id]')).toHaveAttribute('draggable', 'false');
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await expect(page.locator('aside article')).not.toHaveCount(0);
	await expect(page.locator('article[data-block-id]')).toHaveAttribute('draggable', 'true');
	await page.getByRole('button', { name: 'เสร็จสิ้น', exact: true }).click();
	await expect(page.getByText('แบบร่าง · โหมดดู')).toBeVisible();
	await expect(page.locator('aside article')).toHaveCount(0);
});

test('does not prompt a published timetable to update when opening data changes', async ({
	page
}) => {
	const mock = await installTimetableMock(page, { status: 'published' });
	await page.goto(timetableUrl(timetableIds.publishedVersion));
	mock.publishNewOpening();
	await page.getByRole('button', { name: 'โหลดล่าสุด', exact: true }).click();
	await expect.poll(mock.workspaceRequestCount).toBe(2);
	await expect(page.getByTestId('timetable-board-ready')).toHaveAttribute('aria-busy', 'false');
	await expect(page.getByText('เผยแพร่แล้ว · โหมดดู')).toBeVisible();
	await expect(page.getByText(/ข้อมูลเปิดสอนมีรุ่นเผยแพร่ใหม่/)).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'อัปเดตข้อมูลเปิดสอน', exact: true })).toHaveCount(
		0
	);
	await expect(page).toHaveURL(new RegExp(`timetableVersionId=${timetableIds.publishedVersion}`));
	expect(mock.sourceUpdateRequestCount()).toBe(0);
});

test('keeps compact context filters in the top desktop header row', async ({ page }) => {
	await page.setViewportSize({ width: 1543, height: 884 });
	await installTimetableMock(page);
	await page.goto(timetableUrl(timetableIds.draftVersion));
	const header = page.locator('header[aria-label="บริบทตารางสอน"]');
	for (const view of ['ชั้น', 'กลุ่มเรียน', 'ครู', 'โรงเรียน']) {
		await header.getByRole('button', { name: view, exact: true }).click();
		const version = header.getByRole('button', { name: 'เลือกรุ่นตารางสอน', exact: true });
		const owner = header.getByRole(view === 'โรงเรียน' ? 'button' : 'combobox', {
			name: view === 'โรงเรียน' ? 'เลือกวันดูภาพรวม' : 'เลือกรายการสำหรับจัดตาราง',
			exact: true
		});
		await expect(owner).toBeVisible();
		const versionBox = (await version.boundingBox())!;
		const ownerBox = (await owner.boundingBox())!;
		const viewsBox = (await header.locator('[aria-label="มุมมองตารางสอน"]').boundingBox())!;
		const summaryBox = (await header
			.getByRole('heading', { name: 'รุ่นตารางสอนที่เลือก', exact: true })
			.locator('../..')
			.boundingBox())!;
		expect(summaryBox.x + summaryBox.width).toBeLessThan(versionBox.x);
		expect(
			Math.abs(summaryBox.y + summaryBox.height - versionBox.y - versionBox.height)
		).toBeLessThanOrEqual(1);
		expect(versionBox.width).toBeLessThanOrEqual(240);
		expect(ownerBox.width).toBeLessThanOrEqual(240);
		expect(Math.abs(versionBox.y - ownerBox.y)).toBeLessThanOrEqual(1);
		expect(
			Math.abs(ownerBox.y + ownerBox.height - viewsBox.y - viewsBox.height)
		).toBeLessThanOrEqual(1);
		expect(versionBox.x + versionBox.width).toBeLessThan(ownerBox.x);
		expect(ownerBox.x + ownerBox.width).toBeLessThan(viewsBox.x);
	}
});

test('notifies of a newer opening without changing an in-progress draft and keeps review placements', async ({
	page
}) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const mock = await installTimetableMock(page, { blocks: [block] });
	await page.goto(timetableUrl(timetableIds.draftVersion));
	await expect(page.getByText(/ข้อมูลเปิดสอนมีรุ่นเผยแพร่ใหม่/)).toHaveCount(0);
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	mock.publishNewOpening();
	await page.getByRole('button', { name: 'โหลดล่าสุด', exact: true }).click();
	await expect(
		page.getByText('ข้อมูลเปิดสอนมีรุ่นเผยแพร่ใหม่ แบบร่างตารางสอนนี้ยังอ้างอิงรุ่นเดิม')
	).toBeVisible();
	expect(mock.sourceUpdateRequestCount()).toBe(0);
	mock.setSourceIssues([
		{
			blockId: block.id,
			learningGroupId: timetableIds.groupA,
			teacherId: timetableIds.teacherA,
			code: 'ineligible_instructor'
		}
	]);
	await page.getByRole('button', { name: 'อัปเดตข้อมูลเปิดสอน', exact: true }).click();
	await expect(page.getByText(/มีคาบที่ต้องตรวจแก้ก่อนเผยแพร่/)).toBeVisible();
	await expect(page.locator('article[data-block-id]')).toHaveCount(1);
	expect(mock.sourceUpdateRequestCount()).toBe(1);
});

test('deletes only a confirmed draft then returns to its source', async ({ page }) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const mock = await installTimetableMock(page, { blocks: [block] });
	await page.goto(timetableUrl(timetableIds.draftVersion));
	await page.getByRole('button', { name: 'เพิ่มเติม', exact: true }).click();
	await page.getByRole('menuitem', { name: 'ลบแบบร่าง' }).click();
	const confirmation = page.getByRole('alertdialog');
	await expect(confirmation).toContainText('พร้อมคาบ 1 คาบ');
	await confirmation.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	expect(mock.draftDeleteRequestCount()).toBe(0);
	await page.getByRole('button', { name: 'เพิ่มเติม', exact: true }).click();
	await page.getByRole('menuitem', { name: 'ลบแบบร่าง' }).click();
	await page
		.getByRole('alertdialog')
		.getByRole('button', { name: 'ลบแบบร่าง', exact: true })
		.click();
	await expect(page).toHaveURL(new RegExp(`timetableVersionId=${timetableIds.publishedVersion}`));
	await expect(page.getByText('เผยแพร่แล้ว · โหมดดู')).toBeVisible();
	expect(mock.draftDeleteRequestCount()).toBe(1);
});

test('publishes through the separate timetable readiness API', async ({ page }) => {
	const mock = await installTimetableMock(page, {
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	await page.goto(timetableUrl(timetableIds.draftVersion));
	await page.getByRole('button', { name: 'เผยแพร่เวอร์ชันใหม่', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('button', { name: 'เผยแพร่', exact: true })).toBeDisabled();
	await dialog.getByRole('button', { name: 'ตรวจความพร้อม', exact: true }).click();
	await expect(dialog).toContainText('พร้อมเผยแพร่ 1 คาบ');
	await dialog.getByRole('button', { name: 'เผยแพร่', exact: true }).click();
	await expect(page.getByText('เผยแพร่แล้ว · โหมดดู')).toBeVisible();
	expect(mock.publicationRequestCount()).toBe(1);
});

for (const action of ['เสร็จสิ้น', 'เผยแพร่เวอร์ชันใหม่', 'ลบแบบร่าง'] as const) {
	test(`waits for autosave before ${action}`, async ({ page }) => {
		let releaseDelete: () => void = () => {};
		const deleteGate = new Promise<void>((resolve) => {
			releaseDelete = resolve;
		});
		const mock = await installTimetableMock(page, {
			blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)],
			deleteGate
		});
		await page.goto(timetableUrl(timetableIds.draftVersion));
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		await page
			.locator('article[data-block-id]')
			.getByRole('button', { name: /นำ .* ออกจากตาราง/ })
			.click();
		await page.getByRole('button', { name: 'ยืนยันนำออก', exact: true }).click();
		try {
			await expect.poll(mock.deleteRequestCount).toBe(1);
			await expect(page.getByRole('alertdialog')).toHaveCount(0);
			if (action === 'ลบแบบร่าง') {
				await page.getByRole('button', { name: 'เพิ่มเติม', exact: true }).click();
				await page.getByRole('menuitem', { name: action }).click();
			} else await page.getByRole('button', { name: action, exact: true }).click();
			await expect(page.getByRole('button', { name: 'เสร็จสิ้น', exact: true })).toBeDisabled();
			await expect(page.getByRole('dialog')).toHaveCount(0);
			await expect(page.getByRole('alertdialog')).toHaveCount(0);
			await expect(page.getByText('แบบร่าง · โหมดดู')).toHaveCount(0);
			expect(mock.draftDeleteRequestCount()).toBe(0);
			expect(mock.publicationRequestCount()).toBe(0);
		} finally {
			releaseDelete();
		}
		if (action === 'เสร็จสิ้น') await expect(page.getByText('แบบร่าง · โหมดดู')).toBeVisible();
		else if (action === 'ลบแบบร่าง')
			await expect(page.getByRole('alertdialog')).toContainText('พร้อมคาบ 0 คาบ');
		else
			await expect(
				page.getByRole('dialog').getByRole('button', { name: 'ตรวจความพร้อม', exact: true })
			).toBeVisible();
	});
}

test('read-only permissions expose detail and export without lifecycle mutations', async ({
	page
}) => {
	await installTimetableMock(page, {
		status: 'published',
		permissions: ['academic_timetable.read.school'],
		blocks: [
			makeTimetableBlock(timetableIds.blockA, timetableIds.period1, {
				versionId: timetableIds.publishedVersion
			})
		]
	});
	await page.goto(timetableUrl(timetableIds.publishedVersion));
	await expect(page.getByRole('button', { name: 'แก้ไข', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'เผยแพร่เวอร์ชันใหม่', exact: true })).toHaveCount(
		0
	);
	await expect(page.getByRole('button', { name: 'เพิ่มเติม', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'สรุปคาบ XLSX' })).toBeVisible();
	await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
	await expect(
		page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true })
	).toHaveCount(0);
});

for (const viewport of [
	{ width: 1440, height: 900 },
	{ width: 390, height: 844 }
]) {
	for (const dark of [false, true]) {
		test(`read/edit layout and keyboard at ${viewport.width}px in ${dark ? 'dark' : 'light'} theme`, async ({
			page
		}) => {
			await page.setViewportSize(viewport);
			await installTimetableMock(page, {
				blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
			});
			await page.goto(timetableUrl(timetableIds.draftVersion));
			await page.evaluate(
				(isDark) => document.documentElement.classList.toggle('dark', isDark),
				dark
			);
			await expect(page.getByRole('heading', { name: 'ตารางสอน', exact: true })).toBeVisible();
			const overflow = await page.evaluate(
				() => document.documentElement.scrollWidth > innerWidth + 1
			);
			expect(overflow).toBe(false);
			const edit = page.getByRole('button', { name: 'แก้ไข', exact: true });
			await edit.focus();
			await page.keyboard.press('Enter');
			await expect(page.getByText('แบบร่าง · กำลังแก้ไข')).toBeVisible();
			await page.screenshot({
				path: `/tmp/schoolorbit-timetable-${viewport.width}-${dark ? 'dark' : 'light'}.png`,
				fullPage: true
			});
			await page.getByRole('button', { name: 'เสร็จสิ้น', exact: true }).focus();
			await page.keyboard.press('Enter');
			await expect(page.getByText('แบบร่าง · โหมดดู')).toBeVisible();
		});
	}
}
