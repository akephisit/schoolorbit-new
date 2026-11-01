import { expect, test } from '@playwright/test';
import {
	installTimetableMock,
	makeDeliveryPublication,
	makeTimetableBlock,
	makeTimetableVersion,
	makeStructuralTimetableBlock,
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

async function rejectChangedSource(route: import('@playwright/test').Route) {
	await route.fulfill({
		status: 409,
		json: {
			success: false,
			error: 'แบบร่างต้องใช้ข้อมูลเปิดสอนล่าสุด',
			data: { code: 'TIMETABLE_DELIVERY_SOURCE_OUTDATED' }
		}
	});
}

async function editTitle(page: import('@playwright/test').Page, blockId: string, title: string) {
	await page
		.locator(`[data-block-id="${blockId}"]`)
		.getByRole('button', { name: /ดูรายละเอียด/ })
		.click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByLabel('ชื่อที่แสดงในตาราง')).toBeEnabled();
	await dialog.getByLabel('ชื่อที่แสดงในตาราง').fill(title);
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
}

test('a definitive rejected details save aligns once and requires explicit review and a fresh Save', async ({
	page
}) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const mock = await installTimetableMock(page, { blocks: [block] });
	let writes = 0;
	await page.route(`**/api/academic/timetable-blocks/${block.id}`, async (route) => {
		if (route.request().method() !== 'PUT') {
			await route.fallback();
			return;
		}
		writes++;
		if (writes === 1) {
			mock.publishNewOpening();
			await rejectChangedSource(route);
			return;
		}
		await route.fallback();
	});
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await editTitle(page, block.id, 'เก็บชื่อที่ถูกปฏิเสธ');
	await expect.poll(mock.sourceUpdateRequestCount).toBe(1);
	await expect(page.getByRole('button', { name: 'ทบทวนรายการ', exact: true })).toBeEnabled();
	expect(writes).toBe(1);
	expect(mock.blocks()).toEqual([block]);
	await page.getByRole('button', { name: 'ทบทวนรายการ', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByLabel('ชื่อที่แสดงในตาราง')).toHaveValue('เก็บชื่อที่ถูกปฏิเสธ');
	await expect(dialog.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
	expect(writes).toBe(1);
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect.poll(() => writes).toBe(2);
	await expect(page.getByText(/ยังมีรายการที่ไม่ได้ยืนยันการบันทึก/)).toHaveCount(0);
	expect(mock.lastUpdateBody()?.rowVersion).toBe(block.rowVersion);
	expect(mock.lastUpdateBody()?.title).toBe('เก็บชื่อที่ถูกปฏิเสธ');
	expect(mock.sourceUpdateBodies()).toEqual([{ rowVersion: 1 }]);
});

test('two source-rejected saves in one batch retain both intents and coalesce source alignment', async ({
	page
}) => {
	const blocks = [
		makeTimetableBlock(timetableIds.blockA, timetableIds.period1),
		makeTimetableBlock(timetableIds.blockB, timetableIds.period2)
	];
	const mock = await installTimetableMock(page, { blocks });
	let rejectBatch!: () => void;
	const batch = new Promise<void>((resolve) => {
		rejectBatch = resolve;
	});
	let captured = 0;
	await page.route('**/api/academic/timetable-blocks/*', async (route) => {
		const path = new URL(route.request().url()).pathname;
		if (route.request().method() !== 'PUT' || !blocks.some((block) => path.endsWith(block.id))) {
			await route.fallback();
			return;
		}
		captured++;
		if (captured <= 2) {
			await batch;
			await rejectChangedSource(route);
			return;
		}
		await route.fallback();
	});
	try {
		await page.goto(url());
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		await editTitle(page, blocks[0].id, 'งานค้างรายการแรก');
		await expect.poll(() => captured).toBe(1);
		await editTitle(page, blocks[1].id, 'งานค้างรายการที่สอง');
		await expect.poll(() => captured).toBe(2);
		mock.publishNewOpening();
		rejectBatch();
		await expect.poll(mock.sourceUpdateRequestCount).toBe(1);
		await expect(
			page.getByText('ยังมีรายการที่ไม่ได้ยืนยันการบันทึก (2)', { exact: true })
		).toBeVisible();
		for (const title of ['งานค้างรายการแรก', 'งานค้างรายการที่สอง']) {
			const item = page
				.getByRole('alert')
				.locator('div.space-y-2')
				.filter({ has: page.getByText(`รายละเอียดคาบ: ${title}`, { exact: true }) });
			await item.getByRole('button', { name: 'ทบทวนรายการ', exact: true }).click();
			const dialog = page.getByRole('dialog');
			await expect(dialog.getByLabel('ชื่อที่แสดงในตาราง')).toHaveValue(title);
			await expect(dialog.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
			await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
			await expect.poll(() => mock.blocks().some((block) => block.title === title)).toBe(true);
		}
		expect(captured).toBe(4);
		expect(mock.sourceUpdateRequestCount()).toBe(1);
		await expect(page.getByText(/ยังมีรายการที่ไม่ได้ยืนยันการบันทึก/)).toHaveCount(0);
	} finally {
		rejectBatch();
	}
});

for (const failure of ['untyped409', 'network', 'coded500'] as const) {
	test(`ambiguous ${failure} keeps intent fenced until explicit read-and-discard with zero source PUT`, async ({
		page
	}) => {
		const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
		const mock = await installTimetableMock(page, { blocks: [block] });
		let writes = 0;
		await page.route(`**/api/academic/timetable-blocks/${block.id}`, async (route) => {
			if (route.request().method() !== 'PUT') {
				await route.fallback();
				return;
			}
			writes++;
			mock.publishNewOpening();
			if (failure === 'network') await route.abort('failed');
			else
				await route.fulfill({
					status: failure === 'coded500' ? 500 : 409,
					json: {
						success: false,
						error: 'ผลยังไม่ยืนยัน',
						...(failure === 'coded500'
							? { data: { code: 'TIMETABLE_DELIVERY_SOURCE_OUTDATED' } }
							: {})
					}
				});
		});
		await page.goto(url());
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		await editTitle(page, block.id, 'เก็บรายการที่ยังยืนยันไม่ได้');
		await expect(page.getByText(/ยังยืนยันผลการบันทึกไม่ได้ การละทิ้ง/)).toBeVisible();
		await expect(page.getByRole('button', { name: 'ทบทวนรายการ', exact: true })).toHaveCount(0);
		expect(mock.sourceUpdateRequestCount()).toBe(0);
		expect(writes).toBe(1);
		const before = mock.workspaceRequestCount();
		await page
			.getByRole('button', { name: 'โหลดข้อมูลล่าสุดและละทิ้งรายการ', exact: true })
			.click();
		await expect(page.getByText(/ยังมีรายการที่ไม่ได้ยืนยันการบันทึก/)).toHaveCount(0);
		expect(mock.workspaceRequestCount()).toBeGreaterThan(before);
		expect(mock.sourceUpdateRequestCount()).toBe(0);
		expect(writes).toBe(1);
		await expect(page.getByText('แบบร่าง · โหมดดู')).toBeVisible();
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		await expect.poll(mock.sourceUpdateRequestCount).toBe(1);
		expect(writes).toBe(1);
	});
}

test('a rejected removal is retained through alignment and needs a fresh explicit confirmation', async ({
	page
}) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const mock = await installTimetableMock(page, { blocks: [block] });
	let attempts = 0;
	await page.route(`**/api/academic/timetable-blocks/${block.id}?*`, async (route) => {
		if (route.request().method() !== 'DELETE') {
			await route.fallback();
			return;
		}
		attempts++;
		if (attempts === 1) {
			mock.publishNewOpening();
			await rejectChangedSource(route);
			return;
		}
		await route.fallback();
	});
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page
		.locator(`[data-block-id="${block.id}"]`)
		.getByRole('button', { name: /นำ .* ออกจากตาราง/ })
		.click();
	await page.getByRole('button', { name: 'ยืนยันนำออก', exact: true }).click();
	await expect.poll(mock.sourceUpdateRequestCount).toBe(1);
	await expect(page.getByRole('button', { name: 'ทบทวนรายการ', exact: true })).toBeEnabled();
	expect(attempts).toBe(1);
	expect(mock.blocks()).toEqual([block]);
	await page.getByRole('button', { name: 'ทบทวนรายการ', exact: true }).click();
	await expect(page.getByRole('alertdialog')).toBeVisible();
	expect(attempts).toBe(1);
	await page.getByRole('button', { name: 'ยืนยันนำออก', exact: true }).click();
	await expect.poll(() => attempts).toBe(2);
	await expect.poll(() => mock.blocks().length).toBe(0);
	await expect(page.getByText(/ยังมีรายการที่ไม่ได้ยืนยันการบันทึก/)).toHaveCount(0);
});

test('a rejected placement retains the exact target and requires another deliberate drop', async ({
	page
}) => {
	const mock = await installTimetableMock(page);
	let attempts = 0;
	await page.route('**/api/academic/timetable-blocks/ordinary', async (route) => {
		attempts++;
		if (attempts === 1) {
			mock.publishNewOpening();
			await rejectChangedSource(route);
			return;
		}
		await route.fallback();
	});
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const tray = page.locator('aside article[data-timetable-source-kind="ordinary_demand"]').first();
	const target = page.locator('td[aria-label^="วันจันทร์ คาบ 2"]').first();
	const transfer = await page.evaluateHandle(() => new DataTransfer());
	await tray.dispatchEvent('dragstart', { dataTransfer: transfer });
	await target.dispatchEvent('dragover', { dataTransfer: transfer });
	await target.dispatchEvent('drop', { dataTransfer: transfer });
	await tray.dispatchEvent('dragend', { dataTransfer: transfer });
	await expect.poll(mock.sourceUpdateRequestCount).toBe(1);
	await expect(page.getByRole('button', { name: 'ทบทวนรายการ', exact: true })).toBeEnabled();
	expect(attempts).toBe(1);
	expect(mock.blocks()).toHaveLength(0);
	await page.getByRole('button', { name: 'ทบทวนรายการ', exact: true }).click();
	await expect(target).toHaveAttribute('data-state', 'move');
	expect(attempts).toBe(1);
	await target.click();
	await expect.poll(() => attempts).toBe(2);
	expect(mock.lastCreateBody()?.bellSchedulePeriodId).toBe(timetableIds.period2);
	await expect(page.getByText(/ยังมีรายการที่ไม่ได้ยืนยันการบันทึก/)).toHaveCount(0);
});

test('a rejected structural form keeps targets, dates and text for explicit review', async ({
	page
}) => {
	const mock = await installTimetableMock(page);
	const bodies: import('../../src/lib/api/timetable').CreateStructuralTimetableBlocksRequest[] = [];
	await page.route('**/api/academic/timetable-blocks/structural', async (route) => {
		const body: import('../../src/lib/api/timetable').CreateStructuralTimetableBlocksRequest = route
			.request()
			.postDataJSON();
		bodies.push(body);
		if (bodies.length === 1) {
			mock.publishNewOpening();
			await rejectChangedSource(route);
			return;
		}
		const block = {
			...makeStructuralTimetableBlock(
				timetableIds.createdBlock,
				body.slots[0].bellSchedulePeriodId,
				body.title,
				body.teacherIds
			),
			note: body.note ?? null,
			dayOfWeek: body.slots[0].dayOfWeek
		};
		const next = structuredClone(mock.workspace());
		next.blocks = [block];
		next.version.rowVersion++;
		next.summary.blockCount = 1;
		next.totalDraftBlockCount = 1;
		mock.publishWorkspace(next);
		await route.fulfill({ json: { success: true, data: [block] } });
	});
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.getByRole('button', { name: 'เพิ่มคาบพิเศษ', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'เพิ่มคาบพิเศษ' });
	await dialog.getByLabel('ชื่อที่แสดง').fill('คาบพิเศษที่รอทบทวน');
	await dialog.getByLabel('หมายเหตุ').fill('หมายเหตุที่เก็บไว้');
	await dialog
		.getByRole('group', { name: 'เลือกครู', exact: true })
		.getByRole('button', { name: 'ครูคณิตศาสตร์ A', exact: true })
		.click();
	await dialog.getByRole('button', { name: 'เลือก', exact: true }).first().click();
	await dialog.getByRole('button', { name: 'เพิ่ม 1 ช่อง', exact: true }).click();
	await expect.poll(mock.sourceUpdateRequestCount).toBe(1);
	expect(bodies).toHaveLength(1);
	await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await page.getByRole('button', { name: 'ทบทวนรายการ', exact: true }).click();
	await expect(dialog.getByLabel('ชื่อที่แสดง')).toHaveValue('คาบพิเศษที่รอทบทวน');
	await expect(dialog.getByLabel('หมายเหตุ')).toHaveValue('หมายเหตุที่เก็บไว้');
	await expect(dialog.getByRole('button', { name: 'เพิ่ม 1 ช่อง', exact: true })).toBeEnabled();
	expect(bodies).toHaveLength(1);
	await dialog.getByRole('button', { name: 'เพิ่ม 1 ช่อง', exact: true }).click();
	await expect.poll(() => bodies.length).toBe(2);
	expect(bodies[1]).toEqual(bodies[0]);
	await expect(page.getByText(/ยังมีรายการที่ไม่ได้ยืนยันการบันทึก/)).toHaveCount(0);
});

test('late source completion cannot clear a new term lifecycle busy state or reload the new term with an old version', async ({
	page
}) => {
	const nextTerm = '21000000-0000-4000-8000-000000000299';
	const nextVersion = 'a1000000-0000-4000-8000-000000000299';
	const nextSource = 'b1000000-0000-4000-8000-000000000299';
	const term = (id: string, sequence: number) => ({
		id,
		isVisible: true,
		academicYearId: timetableIds.year,
		name: `ภาคเรียนที่ ${sequence}`,
		code: String(sequence),
		sequence,
		termType: 'regular' as const,
		status: 'active' as const,
		includedInYearResult: true,
		blocksYearClosure: true
	});
	const mock = await installTimetableMock(page, {
		versions: [
			makeTimetableVersion('published'),
			makeTimetableVersion('draft'),
			{
				...makeTimetableVersion('draft'),
				id: nextVersion,
				academicTermId: nextTerm,
				deliveryVersionId: nextSource
			}
		],
		terms: [term(timetableIds.term, 1), term(nextTerm, 2)],
		deliveryPublications: publications(),
		deliveryPublicationsByTerm: { [nextTerm]: [makeDeliveryPublication(nextSource, 1)] }
	});
	let releaseOld!: () => void;
	const oldGate = new Promise<void>((resolve) => {
		releaseOld = resolve;
	});
	let oldStarted = false;
	await page.route('**/api/academic/timetable-versions/*/delivery-source', async (route) => {
		const draft = mock.workspace().version;
		draft.deliveryVersionId = timetableIds.newDeliveryVersion;
		draft.rowVersion++;
		oldStarted = true;
		await oldGate;
		await route.fulfill({ json: { success: true, data: draft } });
	});
	let releaseNew!: () => void;
	const newGate = new Promise<void>((resolve) => {
		releaseNew = resolve;
	});
	let holdNew = false;
	let newStarted = false;
	const workspaceQueries: URLSearchParams[] = [];
	page.on('request', (request) => {
		const requestUrl = new URL(request.url());
		if (requestUrl.pathname === '/api/academic/timetable-blocks/workspace')
			workspaceQueries.push(requestUrl.searchParams);
	});
	await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
		const requestUrl = new URL(route.request().url());
		if (holdNew && requestUrl.searchParams.get('timetableVersionId') === nextVersion) {
			holdNew = false;
			newStarted = true;
			await newGate;
		}
		await route.fallback();
	});
	try {
		await page.goto(url());
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		await expect.poll(() => oldStarted).toBe(true);
		const nextUrl = url(nextVersion).replace(timetableIds.term, nextTerm);
		await page.evaluate((href) => {
			document.documentElement.dataset.navigationMarker = 'retained';
			const link = document.createElement('a');
			link.href = href;
			link.textContent = 'เปิดภาคเรียนถัดไป';
			link.style.cssText = 'position:fixed;top:0;left:0;z-index:99999;background:white';
			document.body.append(link);
		}, nextUrl);
		await page.getByRole('link', { name: 'เปิดภาคเรียนถัดไป', exact: true }).click();
		await expect(page).toHaveURL(new RegExp(`academicTermId=${nextTerm}`));
		await expect(page.getByTestId('timetable-board-ready')).toHaveAttribute('aria-busy', 'false');
		expect(await page.evaluate(() => document.documentElement.dataset.navigationMarker)).toBe(
			'retained'
		);
		holdNew = true;
		await page.getByRole('button', { name: 'เผยแพร่เวอร์ชันใหม่', exact: true }).click();
		await expect.poll(() => newStarted).toBe(true);
		releaseOld();
		await expect(
			page.getByRole('button', { name: 'เผยแพร่เวอร์ชันใหม่', exact: true })
		).toBeDisabled();
		expect(
			workspaceQueries.some(
				(query) =>
					query.get('academicTermId') === nextTerm &&
					query.get('timetableVersionId') === timetableIds.draftVersion
			)
		).toBe(false);
		releaseNew();
		await expect(
			page.getByRole('dialog').getByRole('button', { name: 'ตรวจความพร้อม', exact: true })
		).toBeEnabled();
		await expect(page).toHaveURL(new RegExp(`timetableVersionId=${nextVersion}`));
	} finally {
		releaseOld();
		releaseNew();
	}
});
