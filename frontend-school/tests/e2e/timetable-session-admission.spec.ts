import { expect, test, type Page, type Request } from '@playwright/test';
import { sessionState } from './fixtures/session-state';
import { createDraftRelay, startNativeDraftRelay } from './timetable-collaboration-relay';
import { decodeDraftEvent } from '../../src/lib/academic/timetable/collaboration-decoder';
import {
	installTimetableMock,
	makeTimetableBlock,
	makeTimetableVersion,
	timetableIds
} from './timetable-test-harness';

type Mock = Awaited<ReturnType<typeof installTimetableMock>>;
const url = (id: string = timetableIds.draftVersion) =>
	`/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableVersionId=${id}&view=homeroom&ownerId=${timetableIds.homeroom}`;
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
		const held = gate();
		let started = false;
		const stale = structuredClone(mock.workspace());
		stale.blocks = [
			{ ...initial, title: 'old-session-workspace' },
			makeTimetableBlock(timetableIds.blockB, timetableIds.period2)
		];
		await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
			if (started) {
				await route.fallback();
				return;
			}
			started = true;
			await held.promise;
			await route.fulfill(
				responseKind === 'success'
					? { json: { success: true, data: stale } }
					: { status: 500, json: { success: false, error: 'old-session-reload-error' } }
			);
		});
		try {
			await page.getByRole('button', { name: 'เสร็จสิ้น', exact: true }).click();
			await expect.poll(() => started).toBe(true);
			await expect(board(page)).toHaveAttribute('aria-busy', 'true');
			const replacement = structuredClone(mock.workspace());
			replacement.blocks = [makeTimetableBlock(timetableIds.createdBlock, timetableIds.period3)];
			replacement.version.rowVersion = 5;
			replacement.summary.blockCount = 1;
			mock.publishWorkspace(replacement, false);
			mock.relay.holdStateSync();
			await replaceSession(page, mock);
			await waitReplacementPeer(mock, peer);
			const response = page.waitForResponse(
				(response) =>
					new URL(response.url()).pathname === '/api/academic/timetable-blocks/workspace'
			);
			held.release();
			await (await response).finished();
			await browserTurn(page);
			await expect(card(page, timetableIds.blockA)).toHaveCount(1);
			await expect(card(page, timetableIds.blockB)).toHaveCount(0);
			await expect(card(page, timetableIds.createdBlock)).toHaveCount(0);
			await expect(board(page)).toHaveAttribute('aria-busy', 'true');
			await expect(
				page.getByText(/old-session-reload-error|old-session-workspace|บริบทตารางสอนเปลี่ยนไป/)
			).toHaveCount(0);
			expect(mock.workspaceRequestCount()).toBe(2);
			mock.relay.releaseStateSync();
			await expect(card(page, timetableIds.createdBlock)).toHaveCount(1);
			await expect(card(page, timetableIds.blockA)).toHaveCount(0);
			await expect(card(page, timetableIds.blockB)).toHaveCount(0);
			await expect(board(page)).toHaveAttribute('aria-busy', 'false');
		} finally {
			held.release();
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
						if (
							target.pathname === '/ws/timetable' &&
							target.searchParams.has('timetableVersionId')
						) {
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
		const held = gate();
		let started = false;
		let queuedPatchSeen = false;
		let lastStateSyncConnectionId: string | null = null;
		page.on('websocket', (socket) =>
			socket.on('framereceived', (frame) => {
				const message = decodeDraftEvent(String(frame.payload));
				if (!message) return;
				if (message.type === 'DraftStateSync')
					lastStateSyncConnectionId = message.payload.connectionId;
				if (
					message.type === 'DraftPatched' &&
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
			queued.version.rowVersion = 3;
			queued.summary.blockCount = 2;
			queued.totalDraftBlockCount = 2;
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
			const oldPeer = [...mock.relay.peers.keys()][0];
			const replacement = structuredClone(mock.workspace());
			replacement.blocks = [makeTimetableBlock(timetableIds.createdBlock, timetableIds.period3)];
			replacement.version.rowVersion = 5;
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
			expect(mock.sourceUpdateRequestCount()).toBe(0);
			mock.relay.releaseStateSync();
			await expect(card(page, timetableIds.createdBlock)).toHaveCount(1);
			await expect(card(page, timetableIds.blockB)).toHaveCount(0);
			await expect(card(page, timetableIds.blockA)).toHaveCount(0);
		} finally {
			held.release();
			mock.relay.releaseStateSync();
			await page.close();
			await server.close();
		}
	});
}

test('replacement-session initial hydration uses fresh reads and ignores both original route promises', async ({
	page
}) => {
	const initial = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const mock = await installTimetableMock(page, { blocks: [initial] });
	const oldWorkspace = structuredClone(mock.workspace());
	const oldVersions = [
		{ ...makeTimetableVersion('published'), publicationNo: 99 },
		makeTimetableVersion('draft')
	];
	const held = gate();
	let workspaceReads = 0,
		versionReads = 0;
	let originalWorkspaceRequest: Request | null = null,
		originalVersionRequest: Request | null = null;
	const settled = new Set<Request>();
	page.on('requestfinished', (request) => settled.add(request));
	page.on('requestfailed', (request) => settled.add(request));
	await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
		workspaceReads++;
		if (workspaceReads > 1) {
			await route.fallback();
			return;
		}
		originalWorkspaceRequest = route.request();
		await held.promise;
		await route.fulfill({ json: { success: true, data: oldWorkspace } });
	});
	await page.route('**/api/academic/timetable-versions?*', async (route) => {
		versionReads++;
		if (versionReads > 1) {
			await route.fallback();
			return;
		}
		originalVersionRequest = route.request();
		await held.promise;
		await route.fulfill({ json: { success: true, data: oldVersions } });
	});
	try {
		await page.goto(url());
		await expect.poll(() => workspaceReads === 1 && versionReads === 1).toBe(true);
		const replacement = structuredClone(mock.workspace());
		replacement.blocks = [makeTimetableBlock(timetableIds.createdBlock, timetableIds.period3)];
		replacement.version.rowVersion = 5;
		replacement.summary.blockCount = 1;
		mock.publishWorkspace(replacement, false);
		await replaceSession(page, mock);
		await expect(card(page, timetableIds.createdBlock)).toHaveCount(1);
		expect(workspaceReads).toBe(2);
		expect(versionReads).toBe(2);
		held.release();
		await expect
			.poll(
				() =>
					originalWorkspaceRequest !== null &&
					originalVersionRequest !== null &&
					settled.has(originalWorkspaceRequest) &&
					settled.has(originalVersionRequest)
			)
			.toBe(true);
		await browserTurn(page);
		await expect(card(page, timetableIds.blockA)).toHaveCount(0);
		await expect(card(page, timetableIds.createdBlock)).toHaveCount(1);
		await expect(board(page)).toHaveAttribute('aria-busy', 'false');
		await page.getByRole('combobox', { name: 'เลือกรุ่นตารางสอน', exact: true }).click();
		await expect(page.getByRole('option').filter({ hasText: 'รุ่นที่ 99' })).toHaveCount(0);
		await page.keyboard.press('Escape');
	} finally {
		held.release();
	}
});

test('a 404 recovery list loses fallback ownership when the user selects a different published version', async ({
	page
}) => {
	const nextId = 'a1000000-0000-4000-8000-000000000299';
	const nextVersion = { ...makeTimetableVersion('published'), id: nextId, publicationNo: 4 };
	await installTimetableMock(page, {
		versions: [makeTimetableVersion('published'), makeTimetableVersion('draft'), nextVersion]
	});
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	let heldRead = false;
	const held = gate();
	let listStarted = false;
	await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
		if (
			!heldRead &&
			new URL(route.request().url()).searchParams.get('timetableVersionId') ===
				timetableIds.draftVersion
		) {
			heldRead = true;
			await route.fulfill({ status: 404, json: { success: false, error: 'missing draft' } });
			return;
		}
		await route.fallback();
	});
	await page.route('**/api/academic/timetable-versions?*', async (route) => {
		listStarted = true;
		await held.promise;
		await route.fulfill({
			json: { success: true, data: [makeTimetableVersion('published'), nextVersion] }
		});
	});
	try {
		await page.getByRole('button', { name: 'เสร็จสิ้น', exact: true }).click();
		await expect.poll(() => listStarted).toBe(true);
		await page.evaluate((href) => {
			const link = document.createElement('a');
			link.href = href;
			link.textContent = 'ดูประวัติรุ่นที่เลือก';
			link.style.cssText = 'position:fixed;top:0;left:0;z-index:99999;background:white';
			document.body.append(link);
		}, url(nextId));
		await page.getByRole('link', { name: 'ดูประวัติรุ่นที่เลือก', exact: true }).click();
		await expect(page).toHaveURL(new RegExp(`timetableVersionId=${nextId}`));
		await expect(page.locator('header[aria-label="บริบทตารางสอน"]')).toContainText('รุ่นที่ 4');
		held.release();
		await browserTurn(page);
		await expect(page).toHaveURL(new RegExp(`timetableVersionId=${nextId}`));
		await expect(page.locator('header[aria-label="บริบทตารางสอน"]')).toContainText('รุ่นที่ 4');
		await expect(
			page.getByText('แบบร่างนี้ถูกลบแล้ว กลับไปดูตารางต้นทาง', { exact: true })
		).toHaveCount(0);
	} finally {
		held.release();
	}
});
