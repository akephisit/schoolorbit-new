import { expect, test, type Page } from '@playwright/test';
import { sessionState } from './fixtures/session-state';
import { createDraftRelay, startNativeDraftRelay } from './timetable-collaboration-relay';
import { decodeDraftEvent } from '../../src/lib/academic/timetable/collaboration-decoder';
import { installTimetableMock, makeTimetableBlock, timetableIds } from './timetable-test-harness';

type Mock = Awaited<ReturnType<typeof installTimetableMock>>;
const url = (id: string = timetableIds.timetable) =>
	`/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableId=${id}&view=homeroom&ownerId=${timetableIds.homeroom}`;
const card = (page: Page, id: string) => page.locator(`article[data-block-id="${id}"]`);
const board = (page: Page) => page.getByTestId('timetable-board-ready');
test.use({ serviceWorkers: 'block' });

function gate() {
	let release!: () => void;
	const promise = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { promise, release: () => release() };
}
async function browserTurn(page: Page) {
	await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));
}
async function replaceSession(page: Page, mock: Mock) {
	const next = sessionState(Date.now() + 2000, {
		sessionId: '01900000-0000-7000-8000-000000000002'
	});
	mock.setSessionState(next);
	const auth = page.waitForResponse(
		(response) => new URL(response.url()).pathname === '/api/auth/me'
	);
	await page.evaluate((session) => {
		const channel = new BroadcastChannel(`schoolorbit:session-activity:${location.origin}`);
		channel.postMessage({
			type: 'observed',
			sessionId: session.sessionId,
			serverNow: session.serverNow
		});
		channel.close();
	}, next);
	await (await auth).finished();
	await browserTurn(page);
}
async function waitReplacementPeer(mock: Mock, previous: string) {
	await expect.poll(() => [...mock.relay.peers.keys()].some((id) => id !== previous)).toBe(true);
}

for (const responseKind of ['success', 'error'] as const) {
	test(`old-session reload ${responseKind} cannot apply data, errors or final loading state before the replacement baseline`, async ({
		page
	}) => {
		const initial = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
		const mock = await installTimetableMock(page, { blocks: [initial] });
		await page.goto(url());
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const peer = [...mock.relay.peers.keys()][0];
		const held = gate(),
			fresh = gate(),
			handled = gate();
		let started = false;
		const stale = structuredClone(mock.workspace());
		stale.blocks = [
			{ ...initial, title: 'old-session-workspace' },
			makeTimetableBlock(timetableIds.blockB, timetableIds.period2)
		];
		await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
			if (started) {
				await fresh.promise;
				await route.fallback();
				return;
			}
			started = true;
			await held.promise;
			try {
				await route.fulfill(
					responseKind === 'success'
						? { json: { success: true, data: stale } }
						: { status: 500, json: { success: false, error: 'old-session-reload-error' } }
				);
			} finally {
				handled.release();
			}
		});
		try {
			await page.getByRole('button', { name: 'เสร็จสิ้น', exact: true }).click();
			await expect.poll(() => started).toBe(true);
			await expect(board(page)).toHaveAttribute('aria-busy', 'true');
			const replacement = structuredClone(mock.workspace());
			replacement.blocks = [makeTimetableBlock(timetableIds.createdBlock, timetableIds.period3)];
			replacement.timetable.rowVersion = 5;
			replacement.summary.blockCount = 1;
			mock.publishWorkspace(replacement, false);
			mock.relay.holdStateSync();
			await replaceSession(page, mock);
			await waitReplacementPeer(mock, peer);
			held.release();
			await handled.promise;
			await browserTurn(page);
			await expect(card(page, timetableIds.blockA)).toHaveCount(1);
			await expect(card(page, timetableIds.blockB)).toHaveCount(0);
			await expect(card(page, timetableIds.createdBlock)).toHaveCount(0);
			await expect(board(page)).toHaveAttribute('aria-busy', 'true');
			await expect(
				page.getByText(/old-session-reload-error|old-session-workspace|บริบทตารางสอนเปลี่ยนไป/)
			).toHaveCount(0);
			expect(mock.workspaceRequestCount()).toBe(2);
			fresh.release();
			mock.relay.releaseStateSync();
			await expect(card(page, timetableIds.createdBlock)).toHaveCount(1);
			await expect(card(page, timetableIds.blockA)).toHaveCount(0);
			await expect(card(page, timetableIds.blockB)).toHaveCount(0);
			await expect(board(page)).toHaveAttribute('aria-busy', 'false');
		} finally {
			held.release();
			fresh.release();
			mock.relay.releaseStateSync();
		}
	});
}

for (const deferred of ['patch', 'snapshot'] as const) {
	test(`an old-session queued ${deferred} behind a captured save cannot apply or force a new baseline after identity replacement`, async ({
		page
	}) => {
		const initial = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
		const relay = createDraftRelay();
		const mock = await installTimetableMock(page, {
			blocks: [initial],
			socketMode: 'native',
			relay
		});
		const server = await startNativeDraftRelay(relay, mock.workspace);
		await page.addInitScript(
			({ relayUrl, userId }) => {
				const NativeWebSocket = window.WebSocket;
				window.WebSocket = class extends NativeWebSocket {
					constructor(address: string | URL, protocols?: string | string[]) {
						const target = new URL(String(address), location.href);
						if (target.pathname === '/ws/timetable' && target.searchParams.has('timetableId')) {
							const redirected = new URL(relayUrl);
							redirected.search = target.search;
							redirected.searchParams.set('fixtureUser', userId);
							redirected.searchParams.set('fixtureName', 'ผู้จัดตารางทดสอบ');
							super(redirected, protocols);
						} else super(address, protocols);
					}
				};
			},
			{ relayUrl: server.url, userId: timetableIds.user }
		);
		const held = gate(),
			fresh = gate();
		let started = false;
		let queuedPatchSeen = false;
		let lastStateSyncConnectionId: string | null = null;
		page.on('websocket', (socket) =>
			socket.on('framereceived', (frame) => {
				const message = decodeDraftEvent(String(frame.payload));
				if (!message) return;
				if (message.type === 'TimetableStateSync')
					lastStateSyncConnectionId = message.payload.connectionId;
				if (
					message.type === 'TimetablePatched' &&
					message.payload.upsertedBlocks.some((block) => block.id === timetableIds.blockB)
				)
					queuedPatchSeen = true;
			})
		);
		await page.route(`**/api/academic/timetable-blocks/${initial.id}`, async (route) => {
			if (route.request().method() !== 'PUT') {
				await route.fallback();
				return;
			}
			const body = route.request().postDataJSON();
			started = true;
			await held.promise;
			await route.fulfill({
				json: {
					success: true,
					data: {
						...initial,
						rowVersion: initial.rowVersion + 1,
						title: body.title,
						note: body.note ?? null
					}
				}
			});
		});
		try {
			await page.goto(url());
			await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
			await expect.poll(() => mock.relay.peers.size).toBe(1);
			const peer = [...mock.relay.peers.keys()][0];
			await expect.poll(() => lastStateSyncConnectionId).toBe(peer);
			await card(page, initial.id)
				.getByRole('button', { name: /ดูรายละเอียด/ })
				.click();
			const dialog = page.getByRole('dialog');
			await expect(dialog.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
			await dialog.getByLabel('ชื่อที่แสดงในตาราง').fill('captured-old-session');
			await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
			await expect.poll(() => started).toBe(true);
			const queued = structuredClone(mock.workspace());
			queued.blocks = [initial, makeTimetableBlock(timetableIds.blockB, timetableIds.period2)];
			queued.timetable.rowVersion = 3;
			queued.summary.blockCount = 2;
			queued.totalBlockCount = 2;
			mock.publishWorkspace(queued);
			await expect.poll(() => queuedPatchSeen).toBe(true);
			await browserTurn(page);
			await expect(card(page, timetableIds.blockB)).toHaveCount(0);
			if (deferred === 'snapshot') {
				mock.relay.reconcile();
				await waitReplacementPeer(mock, peer);
				await expect.poll(() => mock.relay.peers.size).toBe(1);
				await expect.poll(() => lastStateSyncConnectionId).toBe([...mock.relay.peers.keys()][0]);
				await browserTurn(page);
			}
			await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
				await fresh.promise;
				await route.fallback();
			});
			const oldPeer = [...mock.relay.peers.keys()][0];
			const replacement = structuredClone(mock.workspace());
			replacement.blocks = [makeTimetableBlock(timetableIds.createdBlock, timetableIds.period3)];
			replacement.timetable.rowVersion = 5;
			replacement.summary.blockCount = 1;
			mock.publishWorkspace(replacement, false);
			mock.relay.holdStateSync();
			await replaceSession(page, mock);
			await waitReplacementPeer(mock, oldPeer);
			await expect.poll(() => mock.relay.peers.size).toBe(1);
			const heldPeer = [...mock.relay.peers.keys()][0];
			const response = page.waitForResponse(
				(response) =>
					new URL(response.url()).pathname.endsWith(initial.id) &&
					response.request().method() === 'PUT'
			);
			held.release();
			await (await response).finished();
			await browserTurn(page);
			await expect(card(page, timetableIds.blockB)).toHaveCount(0);
			await expect(card(page, timetableIds.createdBlock)).toHaveCount(0);
			await expect(page.locator('[data-pending="true"]')).toHaveCount(0);
			expect([...mock.relay.peers.keys()]).toEqual([heldPeer]);
			expect(lastStateSyncConnectionId).not.toBe(heldPeer);
			expect(mock.workspaceRequestCount()).toBe(2);
			expect(mock.retiredRequests()).toEqual([]);
			fresh.release();
			mock.relay.releaseStateSync();
			await expect(card(page, timetableIds.createdBlock)).toHaveCount(1);
			await expect(card(page, timetableIds.blockB)).toHaveCount(0);
			await expect(card(page, timetableIds.blockA)).toHaveCount(0);
		} finally {
			held.release();
			fresh.release();
			mock.relay.releaseStateSync();
			await page.close();
			await server.close();
		}
	});
}

for (const stage of ['timetable', 'workspace'] as const)
	test(`replacement session ignores a delayed original ${stage} hydration`, async ({ page }) => {
		const initial = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
		const mock = await installTimetableMock(page, { blocks: [initial] });
		const stale = structuredClone(mock.workspace()),
			held = gate();
		let tableReads = 0,
			workspaceReads = 0;
		await page.route('**/api/academic/timetable?*', async (route) => {
			tableReads++;
			if (stage === 'timetable' && tableReads === 1) {
				await held.promise;
				return route.fulfill({ json: { success: true, data: stale.timetable } });
			}
			await route.fallback();
		});
		await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
			workspaceReads++;
			if (stage === 'workspace' && workspaceReads === 1) {
				await held.promise;
				return route.fulfill({ json: { success: true, data: stale } });
			}
			await route.fallback();
		});
		try {
			await page.goto(url());
			await expect.poll(() => (stage === 'timetable' ? tableReads : workspaceReads)).toBe(1);
			const replacement = structuredClone(mock.workspace());
			replacement.blocks = [makeTimetableBlock(timetableIds.createdBlock, timetableIds.period3)];
			replacement.timetable.rowVersion = 5;
			replacement.summary.blockCount = 1;
			mock.publishWorkspace(replacement, false);
			await replaceSession(page, mock);
			await expect(card(page, timetableIds.createdBlock)).toHaveCount(1);
			expect(tableReads).toBe(2);
			held.release();
			await browserTurn(page);
			await expect(card(page, timetableIds.blockA)).toHaveCount(0);
			await expect(card(page, timetableIds.createdBlock)).toHaveCount(1);
			await expect(board(page)).toHaveAttribute('aria-busy', 'false');
			expect(mock.retiredRequests()).toEqual([]);
		} finally {
			held.release();
		}
	});

test('persistent current-workspace 404 resolves its singleton once and then waits for explicit retry', async ({
	page
}) => {
	const mock = await installTimetableMock(page);
	await page.goto(url());
	await expect(board(page)).toBeVisible();
	let missingReads = 0;
	await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
		missingReads++;
		await route.fulfill({ status: 404, json: { success: false, error: 'ไม่พบตารางสอนปัจจุบัน' } });
	});
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await expect(page.getByText('โหลดตารางสอนไม่สำเร็จ', { exact: true })).toBeVisible();
	await expect(board(page)).toHaveCount(0);
	await browserTurn(page);
	expect(missingReads).toBe(2);
	expect(mock.timetableRequestCount()).toBe(2);
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect.poll(() => missingReads).toBe(3);
	await browserTurn(page);
	expect(missingReads).toBe(3);
	expect(mock.timetableCreateRequestCount()).toBe(0);
	expect(mock.retiredRequests()).toEqual([]);
});

test('a missing current workspace followed by nullable singleton None becomes an empty term without creating data', async ({
	page
}) => {
	const mock = await installTimetableMock(page);
	await page.goto(url());
	await expect(board(page)).toBeVisible();
	let missingReads = 0,
		resolutions = 0;
	await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
		missingReads++;
		await route.fulfill({ status: 404, json: { success: false, error: 'ไม่พบตารางสอนปัจจุบัน' } });
	});
	await page.route('**/api/academic/timetable?*', async (route) => {
		resolutions++;
		await route.fulfill({ json: { success: true, data: null } });
	});
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await expect(page.getByText('ยังไม่มีตารางสอน', { exact: true })).toBeVisible();
	expect(missingReads).toBe(1);
	expect(resolutions).toBe(1);
	expect(mock.timetableCreateRequestCount()).toBe(0);
	await expect(board(page)).toHaveCount(0);
});

test('a failed current-workspace network read preserves the visible board and never loops or replays a write', async ({
	page
}) => {
	const mock = await installTimetableMock(page, {
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	await page.goto(url());
	await expect(board(page)).toBeVisible();
	let attempts = 0;
	await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
		attempts++;
		await route.abort('failed');
	});
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await expect(page.getByRole('button', { name: 'แก้ไข', exact: true })).toBeEnabled();
	await browserTurn(page);
	expect(attempts).toBe(1);
	await expect(card(page, timetableIds.blockA)).toBeVisible();
	expect(mock.updateRequestCount() + mock.createRequestCount()).toBe(0);
});
