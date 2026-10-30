import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import { revalidatePage } from './fixtures/page-revalidation.ts';
import {
	createDraftRelay,
	startNativeDraftRelay,
	type DraftRelay
} from './timetable-collaboration-relay.ts';
import {
	installTimetableMock,
	timetableIds,
	makeSynchronizedTimetableBlock,
	makeTimetableBlock
} from './timetable-test-harness.ts';

const artifacts = '/workspace/artifacts/timetable-live-collaboration';
const secondUser = '51000000-0000-4000-8000-000000000211';
function url(view = 'homeroom', owner = timetableIds.homeroom as string) {
	return `/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableVersionId=${timetableIds.draftVersion}&view=${view}&ownerId=${owner}`;
}
async function redirectSocket(
	context: BrowserContext,
	relayUrl: string,
	userId: string,
	name: string
) {
	await context.addInitScript(
		({ relayUrl, userId, name }) => {
			const NativeWebSocket = window.WebSocket;
			window.WebSocket = class extends NativeWebSocket {
				constructor(address: string | URL, protocols?: string | string[]) {
					const target = new URL(String(address), location.href);
					if (
						target.pathname === '/ws/timetable' &&
						target.searchParams.has('timetableVersionId')
					) {
						const redirected = new URL(relayUrl);
						redirected.search = target.search;
						redirected.searchParams.set('fixtureUser', userId);
						redirected.searchParams.set('fixtureName', name);
						super(redirected, protocols);
					} else super(address, protocols);
				}
			};
		},
		{ relayUrl, userId, name }
	);
}
async function cursorNear(
	page: Page,
	connectionId: string,
	selector: string,
	u: number,
	v: number
) {
	await expect
		.poll(async () =>
			page.evaluate(
				({ connectionId, selector, u, v }) => {
					const target = document.querySelector<HTMLElement>(selector)?.getBoundingClientRect();
					const cursor = [
						...document.querySelectorAll<HTMLElement>('[data-timetable-remote-cursor]')
					]
						.find((item) => item.dataset.timetableRemoteCursor === connectionId)
						?.getBoundingClientRect();
					return target && cursor
						? Math.hypot(
								cursor.x - target.x - target.width * u,
								cursor.y - target.y - target.height * v
							)
						: 9999;
				},
				{ connectionId, selector, u, v }
			)
		)
		.toBeLessThan(4);
}
async function waitPeers(relay: DraftRelay, count: number) {
	await expect.poll(() => relay.peers.size).toBe(count);
}
async function hoverPoint(page: Page, selector: string, u = 0.65, v = 0.4) {
	const box = await page.locator(selector).boundingBox();
	if (!box) throw new Error('missing cell');
	const point = { x: box.x + box.width * u, y: box.y + box.height * v };
	await page.mouse.move(point.x, point.y);
	return point;
}
const cell = (period: string, day = 'MON') =>
	`td[data-timetable-day="${day}"][data-timetable-period-id="${period}"]`;

test.use({ serviceWorkers: 'block' });
test('native sockets preserve semantic cursor geometry, stationary sender resampling, ghost and partial commits', async ({
	browser
}) => {
	const relay = createDraftRelay();
	const first = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
	const second = await browser.newContext({ viewport: { width: 1050, height: 850 } });
	const a = await first.newPage();
	const b = await second.newPage();
	const mockA = await installTimetableMock(a, { socketMode: 'native', relay, periodCount: 12 });
	const mockB = await installTimetableMock(b, {
		socketMode: 'native',
		relay,
		periodCount: 12,
		userId: secondUser
	});
	const server = await startNativeDraftRelay(relay, mockA.workspace);
	try {
		await redirectSocket(first, server.url, timetableIds.user, 'ครูหนึ่ง');
		await redirectSocket(second, server.url, secondUser, 'ครูสอง');
		await a.goto(url());
		await b.goto(url());
		await waitPeers(relay, 2);
		await a.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		await b.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const aId = [...relay.peers.values()].find(
			(peer) => peer.user.userId === timetableIds.user
		)!.id;
		const selector = cell(timetableIds.period2);
		const senderPoint = await hoverPoint(a, selector);
		await cursorNear(b, aId, selector, 0.65, 0.4);
		const samplesPromise = b.evaluate(
			(connectionId) =>
				new Promise<{ x: number; y: number; at: number }[]>((resolve) => {
					const samples: { x: number; y: number; at: number }[] = [];
					const started = performance.now();
					function sample(at: number) {
						const cursor = [
							...document.querySelectorAll<HTMLElement>('[data-timetable-remote-cursor]')
						]
							.find((element) => element.dataset.timetableRemoteCursor === connectionId)
							?.getBoundingClientRect();
						if (cursor) samples.push({ x: cursor.x, y: cursor.y, at });
						if (at - started < 500) requestAnimationFrame(sample);
						else resolve(samples);
					}
					requestAnimationFrame(sample);
				}),
			aId
		);
		for (let sample = 0; sample < 8; sample++) {
			await hoverPoint(a, cell(sample % 2 ? timetableIds.period2 : timetableIds.period1));
			await a.waitForTimeout(40);
		}
		const samples = await samplesPromise;
		expect(samples.length).toBeGreaterThan(10);
		const speeds = samples
			.slice(1)
			.map(
				(point, index) =>
					(Math.hypot(point.x - samples[index].x, point.y - samples[index].y) /
						(point.at - samples[index].at)) *
					1000
			);
		expect(Math.max(...speeds)).toBeLessThan(3500);
		await cursorNear(b, aId, selector, 0.65, 0.4);
		await b.setViewportSize({ width: 1180, height: 900 });
		await cursorNear(b, aId, selector, 0.65, 0.4);
		await b.evaluate(() => {
			document.documentElement.style.zoom = '0.9';
		});
		await cursorNear(b, aId, selector, 0.65, 0.4);
		// Scroll the sender's nested board without moving the physical pointer.
		await a.locator(selector).evaluate((element) => {
			let parent = element.parentElement;
			while (parent && parent.scrollWidth <= parent.clientWidth) parent = parent.parentElement;
			if (!parent) throw new Error('missing nested scroll');
			parent.scrollLeft += 80;
		});
		await expect
			.poll(
				() =>
					relay.received
						.filter(
							(item) => item.connectionId === aId && item.message.type === 'ClientPresenceUpdate'
						)
						.at(-1)?.message
			)
			.toMatchObject({ type: 'ClientPresenceUpdate', payload: { anchor: { kind: 'cell' } } });
		const subject = await a.evaluate((point) => {
			const element = document
				.elementFromPoint(point.x, point.y)
				?.closest<HTMLElement>('[data-timetable-anchor="cell"]');
			if (!element) return null;
			const rect = element.getBoundingClientRect();
			return {
				period: element.dataset.timetablePeriodId!,
				u: (point.x - rect.x) / rect.width,
				v: (point.y - rect.y) / rect.height
			};
		}, senderPoint);
		expect(subject).not.toBeNull();
		await cursorNear(b, aId, cell(subject!.period), subject!.u, subject!.v);
		await a.locator(selector).evaluate((element) => {
			let parent = element.parentElement;
			while (parent && parent.scrollWidth <= parent.clientWidth) parent = parent.parentElement;
			if (parent) parent.scrollLeft = 0;
		});
		await b
			.getByRole('button', { name: 'Collapse sidebar', exact: true })
			.evaluate((element) => (element as HTMLButtonElement).click());
		await cursorNear(b, aId, selector, 0.65, 0.4);
		await a
			.getByRole('button', { name: 'Collapse sidebar', exact: true })
			.evaluate((element) => (element as HTMLButtonElement).click());
		const stationary = await a.evaluate((point) => {
			const subject = document
				.elementFromPoint(point.x, point.y)
				?.closest<HTMLElement>('[data-timetable-anchor="cell"]');
			if (!subject) return null;
			const rect = subject.getBoundingClientRect();
			return {
				day: subject.dataset.timetableDay!,
				period: subject.dataset.timetablePeriodId!,
				u: (point.x - rect.x) / rect.width,
				v: (point.y - rect.y) / rect.height
			};
		}, senderPoint);
		expect(stationary).not.toBeNull();
		await cursorNear(
			b,
			aId,
			cell(stationary!.period, stationary!.day),
			stationary!.u,
			stationary!.v
		);
		await b.setViewportSize({ width: 750, height: 900 });
		await b.locator(cell(stationary!.period, stationary!.day)).evaluate((element) => {
			const parent = element.closest<HTMLElement>('[data-timetable-scroll-container]');
			if (!parent || parent.scrollWidth <= parent.clientWidth)
				throw new Error(
					`missing recipient overflow: ${parent?.scrollWidth}/${parent?.clientWidth}`
				);
			parent.scrollLeft += 60;
		});
		await cursorNear(
			b,
			aId,
			cell(stationary!.period, stationary!.day),
			stationary!.u,
			stationary!.v
		);
		await b.locator('[data-timetable-scroll-container]').evaluate((element) => {
			element.scrollLeft = 0;
		});
		const card = a.locator('aside article[draggable="true"]').first();
		await card.getByRole('combobox', { name: 'เลือกห้องเรียน' }).click();
		await a.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true }).click();
		await expect(a.getByRole('listbox')).toHaveCount(0);
		const cardBox = await card.boundingBox();
		const target = await a.locator(cell(timetableIds.period1)).boundingBox();
		if (!cardBox || !target) throw new Error('missing drag surface');
		await a.mouse.move(cardBox.x + cardBox.width * 0.5, cardBox.y + 15);
		await a.mouse.down();
		await a.mouse.move(target.x + target.width * 0.5, target.y + target.height * 0.5, {
			steps: 18
		});
		await expect(b.locator('[data-timetable-remote-ghost]')).toBeVisible();
		await expect(b.locator('[data-timetable-remote-ghost]')).not.toContainText('ห้องปฏิบัติการ 2');
		expect(
			relay.received.some(
				(item) =>
					item.message.type === 'ClientClaim' &&
					item.message.payload.candidate?.roomId === timetableIds.roomB
			)
		).toBe(true);
		expect(mockA.previewRequestCount()).toBe(0);
		await mkdir(artifacts, { recursive: true });
		await b.screenshot({ path: `${artifacts}/native-remote-ghost.png` });
		await a.mouse.up();
		await expect.poll(mockA.createRequestCount).toBe(1);
		await expect(b.locator(`[data-block-id="${timetableIds.createdBlock}"]`)).toBeVisible();
		await expect(a.locator(`[data-block-id="${timetableIds.createdBlock}"]`)).toHaveCount(1);
		expect(mockA.previewRequestCount()).toBe(1);
		expect(mockA.workspaceRequestCount()).toBe(1);
		expect(mockB.workspaceRequestCount()).toBe(1);
		await expect(b.getByText('2/3', { exact: true })).toBeVisible();
		await expect.poll(() => relay.workspace?.version.rowVersion).toBe(2);
		await b.screenshot({ path: `${artifacts}/native-partial-commit.png` });
		await b.setViewportSize({ width: 390, height: 844 });
		const mobileBlock = b.locator(
			`[data-timetable-anchor="block"][data-block-id="${timetableIds.createdBlock}"]`
		);
		await mobileBlock.scrollIntoViewIfNeeded();
		await expect
			.poll(
				() =>
					relay.received
						.filter(
							(item) => item.connectionId === aId && item.message.type === 'ClientPresenceUpdate'
						)
						.at(-1)?.message
			)
			.toMatchObject({
				type: 'ClientPresenceUpdate',
				payload: { anchor: { kind: 'block', blockId: timetableIds.createdBlock } }
			});
		const latest = relay.received
			.filter((item) => item.connectionId === aId && item.message.type === 'ClientPresenceUpdate')
			.at(-1)!.message;
		if (latest.type !== 'ClientPresenceUpdate' || latest.payload.anchor?.kind !== 'block')
			throw new Error('missing committed block anchor');
		await cursorNear(
			b,
			aId,
			`[data-timetable-anchor="block"][data-block-id="${timetableIds.createdBlock}"]`,
			latest.payload.anchor.u,
			latest.payload.anchor.v
		);
		await b.screenshot({ path: `${artifacts}/native-mobile-cursor.png` });
	} finally {
		await first.close();
		await second.close();
		await server.close();
	}
});

test('same-user tabs remain separate, reserve source/edit controls and jump to the exact repeated overview row', async ({
	browser
}) => {
	const relay = createDraftRelay();
	const first = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
	const second = await browser.newContext({ viewport: { width: 1280, height: 900 } });
	const a = await first.newPage();
	const b = await second.newPage();
	const otherRoom = '81000000-0000-4000-8000-000000000211';
	const block = makeSynchronizedTimetableBlock(timetableIds.blockA, timetableIds.period1);
	block.homerooms.push({
		...block.homerooms[0],
		id: 'f1000000-0000-4000-8000-000000000211',
		homeroomId: otherRoom,
		code: 'M1-2',
		name: 'ม.1/2'
	});
	const rooms = [
		{
			id: timetableIds.homeroom,
			code: 'M1-1',
			name: 'ม.1/1',
			gradeLevelId: '81000000-0000-4000-8000-000000000301',
			gradeLevelType: 'secondary',
			gradeLevelYear: 1,
			roomNumber: '1',
			isActive: true
		},
		{
			id: otherRoom,
			code: 'M1-2',
			name: 'ม.1/2',
			gradeLevelId: '81000000-0000-4000-8000-000000000301',
			gradeLevelType: 'secondary',
			gradeLevelYear: 1,
			roomNumber: '2',
			isActive: true
		}
	];
	const mockA = await installTimetableMock(a, {
		socketMode: 'native',
		relay,
		blocks: [block],
		homerooms: rooms
	});
	await installTimetableMock(b, { socketMode: 'native', relay, blocks: [block], homerooms: rooms });
	const server = await startNativeDraftRelay(relay, mockA.workspace);
	try {
		await redirectSocket(first, server.url, timetableIds.user, 'ครูคนเดียวกัน');
		await redirectSocket(second, server.url, timetableIds.user, 'ครูคนเดียวกัน');
		await a.goto(url('homeroom', otherRoom));
		await b.goto(url('wholeSchool'));
		await waitPeers(relay, 2);
		const aId = [...relay.peers.values()][0].id;
		await a.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		await b.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const selector = `[data-timetable-anchor="block"][data-block-id="${timetableIds.blockA}"]`;
		await hoverPoint(a, selector, 0.4, 0.3);
		await cursorNear(b, aId, `${selector}[data-row-id="${otherRoom}"]`, 0.4, 0.3);
		await a.getByRole('button', { name: /ดูรายละเอียด ชุมนุม/ }).click();
		await expect
			.poll(
				() =>
					[...relay.peers.values()].filter((peer) => peer.user.connections[0].activeReservationId)
						.length
			)
			.toBe(1);
		await expect(b.locator(`${selector}[data-row-id="${otherRoom}"]`)).toContainText('กำลังแก้ไข');
		const boardTop = await b.locator('table').first().boundingBox();
		const third = await second.newPage();
		await installTimetableMock(third, {
			socketMode: 'native',
			relay,
			blocks: [block],
			homerooms: rooms
		});
		await third.goto(url('homeroom', otherRoom));
		await waitPeers(relay, 3);
		expect((await b.locator('table').first().boundingBox())?.y).toBe(boardTop?.y);
		await expect(b.getByRole('button', { name: /ครูคนเดียวกัน/ }).first()).toBeVisible();
		await b
			.getByRole('button', { name: /ครูคนเดียวกัน/ })
			.first()
			.click();
		await expect(b.getByRole('button', { name: 'ชั้น', exact: true })).toHaveAttribute(
			'aria-pressed',
			'true'
		);
		await expect(b.locator('[data-timetable-jump-target]')).toBeVisible();
		await third.close();
		await waitPeers(relay, 2);
		await first.close();
		await waitPeers(relay, 1);
	} finally {
		await first.close();
		await second.close();
		await server.close();
	}
});

test('a quick synchronized drop keeps its captured lease and locks its remaining source while unrelated work renews independently', async ({
	page
}) => {
	let release!: () => void;
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	const mock = await installTimetableMock(page, {
		createGate: gate,
		includeSynchronizedDemand: true,
		synchronizedDemands: [
			{
				learningOfferingId: timetableIds.offeringSync,
				offeringCode: 'CLUB',
				offeringName: 'ชุมนุม',
				requiredPeriods: 3,
				scheduledPeriods: 0,
				intendedHomeroomIds: [timetableIds.homeroom],
				linkedGroupCount: 0,
				pendingGroupCount: 0,
				conflictGroupCount: 0,
				excludedGroupCount: 0
			}
		]
	});
	try {
		await page.goto(url());
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const sync = page.locator('aside article[data-timetable-source-kind="synchronized_offering"]');
		const transfer = await page.evaluateHandle(() => new DataTransfer());
		await sync.dispatchEvent('dragstart', { dataTransfer: transfer });
		await page
			.locator(cell(timetableIds.period1))
			.dispatchEvent('dragover', { dataTransfer: transfer });
		await page
			.locator(cell(timetableIds.period1))
			.dispatchEvent('drop', { dataTransfer: transfer });
		await sync.dispatchEvent('dragend', { dataTransfer: transfer });
		await expect.poll(mock.synchronizedCreateRequestCount).toBe(1);
		await expect(sync).toHaveAttribute('data-timetable-locked-by', 'กำลังบันทึก');
		await expect(sync).toHaveAttribute('draggable', 'false');
		await expect(sync.getByText('2/3', { exact: true })).toBeVisible();
		const ordinary = page.locator('aside article[data-timetable-source-kind="ordinary_demand"]');
		await expect(ordinary).toHaveAttribute('draggable', 'true');
		const second = await page.evaluateHandle(() => new DataTransfer());
		await ordinary.dispatchEvent('dragstart', { dataTransfer: second });
		await expect
			.poll(() => mock.relay.received.filter((item) => item.message.type === 'ClientClaim').length)
			.toBe(2);
		await expect
			.poll(() => mock.relay.received.filter((item) => item.message.type === 'ClientRenew').length)
			.toBeGreaterThanOrEqual(2);
		await ordinary.dispatchEvent('dragend', { dataTransfer: second });
		release();
		await expect(page.locator(`[data-block-id="${timetableIds.createdBlock}"]`)).toHaveCount(1);
		await expect(sync).toHaveAttribute('draggable', 'true');
		await expect(sync.getByText('2/3', { exact: true })).toBeVisible();
		expect(mock.previewRequestCount()).toBe(1);
		expect(mock.workspaceRequestCount()).toBe(1);
	} finally {
		release();
	}
});

test('a newer canonical patch supersedes an older in-flight recovery read and keeps lifecycle row versions current', async ({
	page
}) => {
	const mock = await installTimetableMock(page);
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	mock.publishNewOpening();
	await revalidatePage(page);
	await expect(
		page.getByRole('button', { name: 'อัปเดตข้อมูลเปิดสอน', exact: true })
	).toBeVisible();
	let release!: () => void;
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	let readStarted = false;
	await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
		if (mock.sourceUpdateRequestCount() === 0) {
			await route.fallback();
			return;
		}
		const captured = structuredClone(mock.workspace());
		readStarted = true;
		await gate;
		await route.fulfill({ json: { success: true, data: captured } });
	});
	try {
		await page.getByRole('button', { name: 'อัปเดตข้อมูลเปิดสอน', exact: true }).click();
		await expect.poll(() => readStarted).toBe(true);
		const workspace = structuredClone(mock.workspace());
		workspace.blocks = [makeTimetableBlock(timetableIds.blockB, timetableIds.period2)];
		workspace.version.rowVersion = 3;
		workspace.summary.blockCount = 1;
		workspace.totalDraftBlockCount = 1;
		const invalidatedRead = page.waitForEvent('requestfailed', {
			predicate: (request) =>
				new URL(request.url()).pathname === '/api/academic/timetable-blocks/workspace'
		});
		mock.publishWorkspace(workspace);
		await invalidatedRead;
		await expect(page.locator(`[data-block-id="${timetableIds.blockB}"]`)).toBeVisible();
		release();
		await expect(
			page.getByText('อัปเดตข้อมูลเปิดสอนแล้ว กรุณาตรวจคาบที่มีเครื่องหมาย', { exact: true })
		).toBeVisible();
		await expect(page.locator(`[data-block-id="${timetableIds.blockB}"]`)).toHaveCount(1);
		await expect(page.getByTestId('timetable-board-ready')).toHaveAttribute('aria-busy', 'false');
		// A later lifecycle save must use the patch's authoritative version, not the older list/read.
		let publishedRowVersion: number | undefined;
		page.on('request', (request) => {
			if (new URL(request.url()).pathname.endsWith('/publication-preview'))
				publishedRowVersion = request.postDataJSON().rowVersion;
		});
		await page.getByRole('button', { name: 'เผยแพร่เวอร์ชันใหม่', exact: true }).click();
		await page
			.getByRole('dialog')
			.getByRole('button', { name: 'ตรวจความพร้อม', exact: true })
			.click();
		await expect.poll(() => publishedRowVersion).toBe(3);
	} finally {
		release();
	}
});

test('native journal replay orders missing patches, ignores old events and reconnects from a fresh socket snapshot without workspace reload', async ({
	browser
}) => {
	const relay = createDraftRelay();
	const context = await browser.newContext();
	const page = await context.newPage();
	const mock = await installTimetableMock(page, { socketMode: 'native', relay });
	const server = await startNativeDraftRelay(relay, mock.workspace);
	try {
		await redirectSocket(context, server.url, timetableIds.user, 'ครู replay');
		await page.goto(url());
		await waitPeers(relay, 1);
		const id = [...relay.peers.keys()][0];
		const one = structuredClone(mock.workspace());
		one.blocks = [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)];
		one.version.rowVersion = 2;
		const first = mock.publishWorkspace(one, false);
		const two = structuredClone(one);
		two.blocks = [
			{
				...makeTimetableBlock(timetableIds.blockB, timetableIds.period2),
				note: 'newest canonical note'
			}
		];
		two.version.rowVersion = 3;
		const second = mock.publishWorkspace(two, false);
		relay.send(id, second);
		await expect
			.poll(() => relay.received.filter((item) => item.message.type === 'ClientReplay').length)
			.toBe(1);
		await expect(page.locator(`[data-block-id="${timetableIds.blockB}"]`)).toBeVisible();
		await expect(page.locator(`[data-block-id="${timetableIds.blockA}"]`)).toHaveCount(0);
		relay.send(id, first);
		await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
		await expect(
			page.getByRole('dialog').getByRole('textbox', { name: 'หมายเหตุ', exact: true })
		).toHaveValue('newest canonical note');
		await page.keyboard.press('Escape');
		relay.reconcile();
		await expect.poll(() => [...relay.peers.keys()][0]).not.toBe(id);
		await waitPeers(relay, 1);
		await expect(page.locator(`[data-block-id="${timetableIds.blockB}"]`)).toHaveCount(1);
		expect(mock.workspaceRequestCount()).toBe(1);
	} finally {
		await context.close();
		await server.close();
	}
});

test('an authoritative target denial disables the slot and a native drop cannot send preview or mutation without confirmed proof', async ({
	browser
}) => {
	const relay = createDraftRelay();
	const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
	const page = await context.newPage();
	const mock = await installTimetableMock(page, { socketMode: 'native', relay });
	const server = await startNativeDraftRelay(relay, mock.workspace);
	try {
		await redirectSocket(context, server.url, timetableIds.user, 'ครู denied');
		await page.goto(url());
		await waitPeers(relay, 1);
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		relay.denyTarget();
		const source = page.locator('aside article').first();
		const sourceBox = await source.boundingBox();
		const target = page.locator(cell(timetableIds.period1));
		const targetBox = await target.boundingBox();
		if (!sourceBox || !targetBox) throw new Error('missing drag geometry');
		await page.mouse.move(
			sourceBox.x + sourceBox.width * 0.5,
			sourceBox.y + sourceBox.height * 0.5
		);
		await page.mouse.down();
		await page.mouse.move(
			targetBox.x + targetBox.width * 0.5,
			targetBox.y + targetBox.height * 0.5,
			{ steps: 18 }
		);
		await expect(target).toHaveAttribute('data-timetable-locked-by', /มีผู้อื่นกำลังใช้/);
		await page.mouse.up();
		await expect(source).toHaveAttribute('draggable', 'true');
		expect(mock.previewRequestCount()).toBe(0);
		expect(mock.createRequestCount()).toBe(0);
		expect(mock.updateRequestCount()).toBe(0);
		expect(mock.workspaceRequestCount()).toBe(1);
		await expect(page.locator('[data-timetable-lesson-card]')).toHaveCount(0);
		await expect(page.locator('main [role="alert"]')).toHaveCount(0);
	} finally {
		await context.close();
		await server.close();
	}
});
