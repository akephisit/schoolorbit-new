import { expect, test } from '@playwright/test';
import {
	installTimetableMock,
	makeDeliveryPublication,
	makeTimetableBlock,
	timetableIds
} from './timetable-test-harness';

const url = (id: string = timetableIds.draftVersion) =>
	`/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableVersionId=${id}&view=homeroom&ownerId=${timetableIds.homeroom}`;
const publications = () => [
	makeDeliveryPublication(),
	makeDeliveryPublication(timetableIds.newDeliveryVersion, 5, '2026-11-05')
];
test.use({ serviceWorkers: 'block' });

for (const permissions of [
	['academic_timetable.read.school'],
	['academic_timetable.read.assigned', 'academic_timetable.manage.assigned']
]) {
	test(`stale ${permissions.at(-1)} view stays honest without a source write`, async ({ page }) => {
		const mock = await installTimetableMock(page, {
			permissions,
			deliveryPublications: publications()
		});
		await page.goto(url());
		await expect(page.getByText(/แบบร่างนี้รอใช้ข้อมูลเปิดสอนล่าสุด/)).toBeVisible();
		if (permissions.includes('academic_timetable.manage.assigned')) {
			await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
			await expect(page.getByText(/ให้ผู้จัดการตารางระดับโรงเรียนหรือผู้เผยแพร่/)).toBeVisible();
		}
		expect(mock.sourceUpdateRequestCount()).toBe(0);
		expect(mock.workspace().version.deliveryVersionId).toBe(timetableIds.deliveryVersion);
	});
}

test('failed alignment does not repeat on canonical echoes and explicit retry first rereads', async ({
	page
}) => {
	const mock = await installTimetableMock(page);
	let attempts = 0;
	const pattern = '**/api/academic/timetable-versions/*/delivery-source';
	await page.route(pattern, async (route) => {
		attempts++;
		await route.abort('failed');
	});
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	mock.publishNewOpening();
	await expect(
		page.getByRole('button', { name: 'ตรวจข้อมูลล่าสุดแล้วลองอีกครั้ง', exact: true })
	).toBeVisible();
	mock.relay.invalidate(mock.workspace());
	await expect(page.getByTestId('timetable-board-ready')).toHaveAttribute('aria-busy', 'false');
	expect(attempts).toBe(1);
	await page.unroute(pattern);
	const reads = mock.workspaceRequestCount();
	await page.getByRole('button', { name: 'ตรวจข้อมูลล่าสุดแล้วลองอีกครั้ง', exact: true }).click();
	await expect.poll(mock.sourceUpdateRequestCount).toBe(1);
	expect(mock.workspaceRequestCount()).toBeGreaterThan(reads);
	expect(mock.sourceUpdateBodies()).toEqual([{ rowVersion: 1 }]);
});

test('source refresh retains unsaved form fields and reacquires an editing lease', async ({
	page
}) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const mock = await installTimetableMock(page, { blocks: [block] });
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
	const dialog = page.getByRole('dialog');
	const title = dialog.getByLabel('ชื่อที่แสดงในตาราง');
	await expect(title).toBeEnabled();
	await title.fill('ชื่อคาบที่ยังไม่บันทึก');
	await dialog.getByLabel('หมายเหตุ').fill('บันทึกในฟอร์มเดิม');
	mock.publishNewOpening();
	await expect.poll(mock.sourceUpdateRequestCount).toBe(1);
	await expect(title).toHaveValue('ชื่อคาบที่ยังไม่บันทึก');
	await expect(dialog.getByLabel('หมายเหตุ')).toHaveValue('บันทึกในฟอร์มเดิม');
	await expect(dialog.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect.poll(mock.updateRequestCount).toBe(1);
	expect(mock.lastUpdateBody()?.title).toBe('ชื่อคาบที่ยังไม่บันทึก');
	expect(mock.lastUpdateBody()?.note).toBe('บันทึกในฟอร์มเดิม');
});

test('late source response cannot move a newly selected published history into edit mode', async ({
	page
}) => {
	const mock = await installTimetableMock(page, { deliveryPublications: publications() });
	let release!: () => void;
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	let started = false;
	await page.route('**/api/academic/timetable-versions/*/delivery-source', async (route) => {
		const draft = mock.workspace().version;
		draft.deliveryVersionId = timetableIds.newDeliveryVersion;
		draft.rowVersion += 1;
		started = true;
		await gate;
		await route.fulfill({ json: { success: true, data: draft } });
	});
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await expect.poll(() => started).toBe(true);
	await page.goto(url(timetableIds.publishedVersion));
	release();
	await expect(page.getByText('เผยแพร่แล้ว · โหมดดู')).toBeVisible();
	await expect(page).toHaveURL(new RegExp(`timetableVersionId=${timetableIds.publishedVersion}`));
	await expect(page.getByRole('button', { name: 'เสร็จสิ้น', exact: true })).toHaveCount(0);
	expect(mock.workspace().version.deliveryVersionId).toBe(timetableIds.newDeliveryVersion);
});

test('opening alignment drains a captured save before updating source and preserves its content', async ({
	page
}) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const mock = await installTimetableMock(page, { blocks: [block] });
	let release!: () => void;
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	let started = false;
	await page.route(`**/api/academic/timetable-blocks/${block.id}`, async (route) => {
		if (route.request().method() !== 'PUT') {
			await route.fallback();
			return;
		}
		started = true;
		await gate;
		await route.fallback();
	});
	try {
		await page.goto(url());
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
		const dialog = page.getByRole('dialog');
		await expect(dialog.getByLabel('ชื่อที่แสดงในตาราง')).toBeEnabled();
		await dialog.getByLabel('ชื่อที่แสดงในตาราง').fill('คาบที่บันทึกก่อนเปลี่ยนรุ่น');
		await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
		await expect.poll(() => started).toBe(true);
		mock.publishNewOpening();
		expect(mock.sourceUpdateRequestCount()).toBe(0);
		release();
		await expect.poll(mock.updateRequestCount).toBe(1);
		await expect.poll(mock.sourceUpdateRequestCount).toBe(1);
		expect(mock.blocks()).toHaveLength(1);
		expect(mock.blocks()[0].title).toBe('คาบที่บันทึกก่อนเปลี่ยนรุ่น');
		expect(mock.sourceUpdateBodies()).toEqual([{ rowVersion: 2 }]);
	} finally {
		release();
	}
});
