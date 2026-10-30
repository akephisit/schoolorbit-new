import { expect, test, type Page, type Route } from '@playwright/test';
import { installTimetableMock, makeTimetableBlock, timetableIds } from './timetable-test-harness';
import { createDraftRelay } from './timetable-collaboration-relay';
import { revalidatePage } from './fixtures/page-revalidation';

test.use({ serviceWorkers: 'block' });
const secondClass = '81000000-0000-4000-8000-000000000212';
const secondUser = '51000000-0000-4000-8000-000000000212';
const errorToasts = (page: Page, message: string) =>
	page.locator('[data-sonner-toast][data-type="error"]').filter({ hasText: message });
function url() {
	return `/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableVersionId=${timetableIds.draftVersion}&view=homeroom&ownerId=${secondClass}`;
}
async function fail(route: Route, message: string) {
	await route.fulfill({ status: 500, json: { success: false, error: message } });
}
async function chooseOwner(page: Page, name: string) {
	await page.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' }).click();
	await page.getByRole('option', { name, exact: true }).click();
}
async function stableBoard(page: Page, top: number) {
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	await expect
		.poll(async () =>
			Math.abs((await page.getByTestId('timetable-board-ready').boundingBox())!.y - top)
		)
		.toBeLessThanOrEqual(1);
	await expect(page.getByRole('alert').filter({ hasText: /synthetic-/ })).toHaveCount(0);
}

test('independent owner selections survive view/day roundtrips, same-version reload and authorized scope loss', async ({
	page
}) => {
	const mock = await installTimetableMock(page);
	const initial = mock.workspace();
	initial.homerooms.push({
		...initial.homerooms[0],
		id: secondClass,
		name: 'ม.1/2',
		roomNumber: '2'
	});
	mock.publishWorkspace(initial);
	await page.goto(url());
	const selector = page.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' });
	await expect(selector).toHaveText('ม.1/2'); // explicit deep link wins
	await page.getByRole('button', { name: 'ครู', exact: true }).click();
	await chooseOwner(page, 'ครูคณิตศาสตร์ B');
	await page.getByRole('button', { name: 'กลุ่มเรียน', exact: true }).click();
	await chooseOwner(page, 'ม.1/1 วิทยาศาสตร์');
	await page.getByRole('button', { name: 'โรงเรียน', exact: true }).click();
	await page.getByRole('combobox', { name: 'เลือกวันดูภาพรวม' }).click();
	await page.getByRole('option', { name: 'วันอังคาร', exact: true }).click();
	for (const [view, label, owner] of [
		['ชั้น', 'ม.1/2', secondClass],
		['ครู', 'ครูคณิตศาสตร์ B', timetableIds.teacherB],
		['กลุ่มเรียน', 'ม.1/1 วิทยาศาสตร์', timetableIds.groupB]
	] as const) {
		await page.getByRole('button', { name: view, exact: true }).click();
		await expect(selector).toHaveText(label);
		await expect(page).toHaveURL(new RegExp(`ownerId=${owner}`));
		await page.getByRole('button', { name: view, exact: true }).click();
		await expect(selector).toHaveText(label);
	}
	expect(mock.workspaceRequestCount()).toBe(1);
	await revalidatePage(page);
	await expect.poll(mock.workspaceRequestCount).toBe(2);
	await expect(selector).toHaveText('ม.1/1 วิทยาศาสตร์');
	await page.getByRole('button', { name: 'ครู', exact: true }).click();
	await expect(selector).toHaveText('ครูคณิตศาสตร์ B');
	await page.getByRole('button', { name: 'ชั้น', exact: true }).click();
	await expect(selector).toHaveText('ม.1/2');
	await page.getByRole('button', { name: 'โรงเรียน', exact: true }).click();
	await expect(page.getByRole('combobox', { name: 'เลือกวันดูภาพรวม' })).toHaveText('อังคาร');
	await page.getByRole('button', { name: 'ครู', exact: true }).click();
	const restricted = mock.workspace();
	restricted.staff = restricted.staff.filter((row) => row.id !== timetableIds.teacherB);
	restricted.homerooms = restricted.homerooms.filter((row) => row.id !== secondClass);
	mock.publishWorkspace(restricted);
	await expect(selector).toHaveText('ครูคณิตศาสตร์ A');
	await page.getByRole('button', { name: 'ชั้น', exact: true }).click();
	await expect(selector).toHaveText('ม.1/1');
	await page.getByRole('button', { name: 'กลุ่มเรียน', exact: true }).click();
	await expect(selector).toHaveText('ม.1/1 วิทยาศาสตร์');
	expect(mock.workspaceRequestCount()).toBe(2);
});

for (const width of [1440, 390]) {
	test(`header places populated avatars after badge and collapses empty roster at ${width}px`, async ({
		page,
		context
	}) => {
		await page.setViewportSize({ width, height: 900 });
		const relay = createDraftRelay();
		await installTimetableMock(page, { relay });
		await page.goto(url());
		const header = page.locator('header[aria-label="บริบทตารางสอน"]');
		await expect(header.locator('[data-timetable-roster]')).toHaveCount(0);
		expect(
			await header
				.locator('h2')
				.evaluate((element) =>
					Boolean(
						element.compareDocumentPosition(
							element.closest('header')!.querySelector('[data-slot="badge"]')!
						) & Node.DOCUMENT_POSITION_FOLLOWING
					)
				)
		).toBe(true);
		const peer = await context.newPage();
		try {
			await installTimetableMock(peer, { relay, userId: secondUser, userName: 'ผู้ร่วมทดสอบ' });
			await peer.goto(url());
			const roster = header.locator('[data-timetable-roster]');
			await expect(roster.getByRole('button')).toHaveCount(1);
			expect(
				await roster.evaluate((element) =>
					element.previousElementSibling?.getAttribute('data-slot')
				)
			).toBe('badge');
			await roster.getByRole('button').focus();
			await expect(roster.getByRole('button')).toBeFocused();
			await expect(page.locator('[data-slot="tooltip-content"]')).toContainText('ผู้ร่วมทดสอบ');
			await page.keyboard.press('Tab');
			await expect(page.locator('[data-slot="tooltip-content"]')).toHaveCount(0);
			expect((await roster.boundingBox())!.width).toBeLessThanOrEqual(34);
			const badge = header.locator('[data-slot="badge"]');
			for (const editing of [false, true]) {
				if (editing) await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
				await expect(badge).toHaveText(editing ? 'แบบร่าง · กำลังแก้ไข' : 'แบบร่าง · โหมดดู');
				for (const theme of ['light', 'dark'] as const) {
					await page.evaluate(
						(theme) => document.documentElement.classList.toggle('dark', theme === 'dark'),
						theme
					);
					await page.screenshot({
						path: `/workspace/artifacts/timetable-header-populated-${width}-${editing ? 'editing' : 'view'}-${theme}.png`,
						animations: 'disabled',
						fullPage: true
					});
				}
				const rosterBox = (await roster.boundingBox())!,
					badgeBox = (await badge.boundingBox())!;
				expect(
					Math.abs(rosterBox.y + rosterBox.height / 2 - badgeBox.y - badgeBox.height / 2)
				).toBeLessThanOrEqual(1);
				expect(rosterBox.x - badgeBox.x - badgeBox.width).toBeGreaterThanOrEqual(0);
				expect(rosterBox.x - badgeBox.x - badgeBox.width).toBeLessThanOrEqual(8);
				expect(
					await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)
				).toBe(true);
			}

			// Exercise an application-owned draft disconnect before disposing the mocked page.
			await peer.getByRole('combobox', { name: 'เลือกรุ่นตารางสอน', exact: true }).click();
			await peer.getByRole('option', { name: /^เผยแพร่ ·/ }).click();
			await expect(peer.getByText('เผยแพร่แล้ว · โหมดดู')).toBeVisible();
			await expect.poll(() => relay.peers.size).toBe(1);
			await expect(header.locator('[data-timetable-roster]')).toHaveCount(0);
		} finally {
			await peer.close();
		}
		await expect(header.locator('[data-timetable-roster]')).toHaveCount(0);
	});
}

test('initial failed workspace reports one detailed toast and keeps an actionable no-data retry state', async ({
	page
}) => {
	await installTimetableMock(page);
	let broken = true;
	await page.route('**/api/academic/timetable-blocks/workspace?**', (route) =>
		broken ? fail(route, 'synthetic-initial-read') : route.fallback()
	);
	await page.goto(url());
	await expect(errorToasts(page, 'synthetic-initial-read')).toHaveCount(1);
	await expect(page.getByRole('alert').filter({ hasText: 'โหลดตารางสอนไม่สำเร็จ' })).toBeVisible();
	await expect(page.getByText('ลองอีกครั้งเพื่อโหลดข้อมูลตารางสอน', { exact: true })).toBeVisible();
	broken = false;
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	await expect(errorToasts(page, 'synthetic-initial-read')).toHaveCount(1);
});

test('loaded streamed failures use one toast each, keep board geometry and retry only the failed region', async ({
	page
}) => {
	const mock = await installTimetableMock(page);
	await page.goto(url());
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	const top = (await page.getByTestId('timetable-board-ready').boundingBox())!.y;
	let broken = true;
	let workspaceReads = 0,
		versionReads = 0;
	await page.route('**/api/academic/timetable-blocks/workspace?**', (route) => {
		workspaceReads++;
		return broken ? fail(route, 'synthetic-stream-workspace') : route.fallback();
	});
	await page.route('**/api/academic/timetable-versions?**', (route) => {
		versionReads++;
		return broken ? fail(route, 'synthetic-stream-versions') : route.fallback();
	});
	await revalidatePage(page);
	await expect(errorToasts(page, 'synthetic-stream-workspace')).toHaveCount(1);
	await expect(errorToasts(page, 'synthetic-stream-versions')).toHaveCount(1);
	await stableBoard(page, top);
	broken = false;
	await errorToasts(page, 'synthetic-stream-versions')
		.getByRole('button', { name: 'ลองใหม่' })
		.click();
	await expect.poll(() => versionReads).toBe(2);
	expect(workspaceReads).toBe(1);
	await errorToasts(page, 'synthetic-stream-workspace')
		.getByRole('button', { name: 'ลองใหม่' })
		.click();
	await expect.poll(() => workspaceReads).toBe(2);
	await stableBoard(page, top);
	expect(mock.workspaceRequestCount()).toBe(2);
});

test('lifecycle nested read failure reports once, retains editing and inserts no board or dialog alert', async ({
	page
}) => {
	await installTimetableMock(page, {
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const top = (await page.getByTestId('timetable-board-ready').boundingBox())!.y;
	await page.route('**/api/academic/timetable-blocks/workspace?**', (route) =>
		fail(route, 'synthetic-finish-reload')
	);
	await page.getByRole('button', { name: 'เสร็จสิ้น', exact: true }).click();
	await expect(errorToasts(page, 'synthetic-finish-reload')).toHaveCount(1);
	await expect(page.getByText('แบบร่าง · กำลังแก้ไข')).toBeVisible();
	await stableBoard(page, top);
});

test('superseded streamed failure cannot toast or overwrite the newly selected version', async ({
	page
}) => {
	await installTimetableMock(page, { requestedWorkspaceVersion: true });
	await page.goto(url());
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	let release!: () => void;
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	let started = false;
	let completed!: () => void;
	const completion = new Promise<void>((resolve) => {
		completed = resolve;
	});
	await page.route('**/api/academic/timetable-blocks/workspace?**', async (route) => {
		if (
			new URL(route.request().url()).searchParams.get('timetableVersionId') !==
			timetableIds.draftVersion
		)
			return route.fallback();
		started = true;
		try {
			await gate;
			await fail(route, 'synthetic-superseded-read');
		} finally {
			completed();
		}
	});
	try {
		await revalidatePage(page);
		await expect.poll(() => started).toBe(true);
		// The version picker is deliberately disabled while the old workspace is loading.
		// A normal route navigation still supersedes that streamed read.
		await page.evaluate(
			(href) => {
				const link = document.createElement('a');
				link.href = href;
				link.dataset.sveltekitPreloadData = 'off';
				link.id = 'test-supersede-workspace';
				link.textContent = 'Open published timetable';
				link.style.cssText = 'position:fixed;top:140px;right:20px;z-index:9999;background:white';
				document.body.append(link);
			},
			url().replace(timetableIds.draftVersion, timetableIds.publishedVersion)
		);
		await page.locator('#test-supersede-workspace').click();
		await expect(page.getByText('เผยแพร่แล้ว · โหมดดู')).toBeVisible();
		const oldResponse = page.waitForResponse(
			(response) =>
				new URL(response.url()).pathname.endsWith('/timetable-blocks/workspace') &&
				new URL(response.url()).searchParams.get('timetableVersionId') === timetableIds.draftVersion
		);
		release();
		await completion;
		await (await oldResponse).finished();
		await page.evaluate(
			() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
		);
		await expect(page).toHaveURL(new RegExp(`timetableVersionId=${timetableIds.publishedVersion}`));
		await expect(errorToasts(page, 'synthetic-superseded-read')).toHaveCount(0);
	} finally {
		release();
	}
});

test('publication validation stays in its workflow while request failure uses one toast without a dialog alert', async ({
	page
}) => {
	await installTimetableMock(page, {
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	await page.goto(url());
	await page.getByRole('button', { name: 'เผยแพร่เวอร์ชันใหม่', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(
		dialog.getByRole('heading', { name: 'เผยแพร่เวอร์ชันใหม่', exact: true })
	).toBeVisible();
	await page.route('**/api/academic/timetable-versions/*/publication-preview', (route) =>
		fail(route, 'synthetic-publication-preview')
	);
	await dialog.getByRole('button', { name: 'ตรวจความพร้อม', exact: true }).click();
	await expect(errorToasts(page, 'synthetic-publication-preview')).toHaveCount(1);
	await expect(dialog).toBeVisible();
	await expect(dialog.getByRole('alert')).toHaveCount(0);
	await expect(dialog.getByRole('button', { name: 'เผยแพร่', exact: true })).toBeDisabled();
});

test('accepted lifecycle HTTP scope loss repairs the owner URL before regrant and same-version initialization', async ({
	page
}) => {
	let workspaceFrames = 0;
	const relay = createDraftRelay();
	const join = relay.join;
	// Observe the mocked transport hand-off; page websocket events cover native sockets only.
	relay.join = (workspace, output, identity) =>
		join(
			workspace,
			(event) => {
				if (event.type === 'DraftStateSync' || event.type === 'DraftPatched') workspaceFrames++;
				output(event);
			},
			identity
		);
	const mock = await installTimetableMock(page, { relay });
	await page.goto(
		url().replace('view=homeroom', 'view=teacher').replace(secondClass, timetableIds.teacherB)
	);
	const selector = page.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' });
	await expect(selector).toHaveText('ครูคณิตศาสตร์ B');
	// Presence can be sent only after the session has accepted its initial StateSync.
	await expect
		.poll(() =>
			mock.relay.received.some(
				({ message }) =>
					message.type === 'ClientPresenceUpdate' &&
					message.payload.context.viewMode === 'teacher' &&
					message.payload.context.viewId === timetableIds.teacherB
			)
		)
		.toBe(true);
	await expect(selector).toHaveText('ครูคณิตศาสตร์ B');
	await expect(page).toHaveURL(new RegExp(`view=teacher&ownerId=${timetableIds.teacherB}`));
	expect(workspaceFrames).toBe(1);
	const baselineFrames = workspaceFrames;
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const complete = mock.workspace();
	const restricted = structuredClone(complete);
	restricted.staff = restricted.staff.filter((row) => row.id !== timetableIds.teacherB);
	// Model a scoped HTTP read without delivering a canonical WS patch first.
	mock.publishWorkspace(restricted, false);
	await page.getByRole('button', { name: 'เสร็จสิ้น', exact: true }).click();
	await expect(selector).toHaveText('ครูคณิตศาสตร์ A');
	await expect(page.getByText('แบบร่าง · โหมดดู')).toBeVisible();
	expect(workspaceFrames).toBe(baselineFrames);
	await expect(page).toHaveURL(new RegExp(`view=teacher&ownerId=${timetableIds.teacherA}`));
	expect(mock.workspaceRequestCount()).toBe(2);
	mock.publishWorkspace(complete, false);
	await revalidatePage(page);
	await expect.poll(mock.workspaceRequestCount).toBe(3);
	await expect(selector).toHaveText('ครูคณิตศาสตร์ A');
	await expect(page).toHaveURL(new RegExp(`view=teacher&ownerId=${timetableIds.teacherA}`));
	await selector.click();
	await expect(page.getByRole('option', { name: 'ครูคณิตศาสตร์ B', exact: true })).toBeVisible();
});
