import { expect, test, type BrowserContext, type Locator, type Page } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { emptyLookupChanges } from '../runtime/timetable-collaboration-fixture.ts';
import { decodeDraftEvent } from '../../src/lib/academic/timetable/collaboration-decoder.ts';
import { sessionState } from './fixtures/session-state.ts';
import {
	createDraftRelay,
	startNativeDraftRelay,
	type DraftRelay
} from './timetable-collaboration-relay.ts';
import {
	installTimetableMock,
	timetableIds,
	makeSynchronizedTimetableBlock,
	makeStructuralTimetableBlock,
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
async function cursorAtStationarySender(
	sender: Page,
	recipient: Page,
	relay: DraftRelay,
	connectionId: string,
	point: { x: number; y: number }
) {
	async function sample() {
		return sender.evaluate((point) => {
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
		}, point);
	}
	await expect
		.poll(async () => {
			const current = await sample();
			const message = relay.received
				.filter(
					(item) =>
						item.connectionId === connectionId && item.message.type === 'ClientPresenceUpdate'
				)
				.at(-1)?.message;
			const anchor = message?.type === 'ClientPresenceUpdate' ? message.payload.anchor : null;
			if (
				!current ||
				anchor?.kind !== 'cell' ||
				anchor.dayOfWeek !== current.day ||
				anchor.bellSchedulePeriodId !== current.period ||
				Math.abs(anchor.u - current.u) > 0.01 ||
				Math.abs(anchor.v - current.v) > 0.01
			)
				return 9999;
			return recipient.evaluate(
				({ connectionId, current }) => {
					const target = [
						...document.querySelectorAll<HTMLElement>('[data-timetable-anchor="cell"]')
					]
						.find(
							(element) =>
								element.dataset.timetableDay === current.day &&
								element.dataset.timetablePeriodId === current.period
						)
						?.getBoundingClientRect();
					const cursor = [
						...document.querySelectorAll<HTMLElement>('[data-timetable-remote-cursor]')
					]
						.find((element) => element.dataset.timetableRemoteCursor === connectionId)
						?.getBoundingClientRect();
					return target && cursor
						? Math.hypot(
								cursor.x - target.x - target.width * current.u,
								cursor.y - target.y - target.height * current.v
							)
						: 9999;
				},
				{ connectionId, current }
			);
		})
		.toBeLessThan(4);
	const current = await sample();
	if (!current) throw new Error('stationary sender left the board');
	return current;
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
test('native visible and hidden presence bursts keep four-peer roster avatars downloaded once and continuously visible', async ({
	browser
}, testInfo) => {
	const relay = createDraftRelay();
	const lifecycle: unknown[] = [];
	const browserBaselines = new Map<string, string>();
	const extraBaselines: string[] = [];
	const first = await browser.newContext({ viewport: { width: 1440, height: 1000 } }),
		second = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
	const a = await first.newPage(),
		b = await second.newPage();
	for (const [name, page] of [
		['a', a],
		['b', b]
	] as const)
		page.on('websocket', (socket) => {
			if (!new URL(socket.url()).searchParams.has('timetableVersionId')) return;
			socket.on('framereceived', ({ payload }) => {
				const event = decodeDraftEvent(String(payload));
				if (event?.type === 'DraftStateSync') {
					browserBaselines.set(name, event.payload.connectionId);
					lifecycle.push({
						event: 'browser-baseline',
						name,
						id: event.payload.connectionId,
						at: Date.now()
					});
				}
			});
			socket.on('close', () => lifecycle.push({ event: 'browser-close', name, at: Date.now() }));
		});
	const mock = await installTimetableMock(a, { socketMode: 'native', relay });
	await installTimetableMock(b, { socketMode: 'native', relay, userId: secondUser });
	const grants: string[] = [],
		blobs: string[] = [];
	for (const page of [a, b]) {
		await page.route('**/api/files/**/download*', async (route) => {
			if (page === a) grants.push(route.request().url());
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					success: true,
					data: {
						url: 'http://127.0.0.1:4173/__collaboration-avatar.png',
						expiresAt: '2099-01-01T00:00:00Z'
					}
				})
			});
		});
		await page.route('**/__collaboration-avatar.png', async (route) => {
			if (page === a) blobs.push(route.request().url());
			await route.fulfill({
				status: 200,
				contentType: 'image/png',
				body: Buffer.from(
					'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
					'base64'
				)
			});
		});
	}
	const server = await startNativeDraftRelay(relay, mock.workspace);
	const extraSockets: WebSocket[] = [];
	try {
		await redirectSocket(first, server.url, timetableIds.user, 'ผู้ดูรูป');
		await redirectSocket(second, server.url, secondUser, 'ผู้ขยับเมาส์');
		await a.goto(url());
		await b.goto(url());
		await expect
			.poll(() => [...relay.peers.values()].map((peer) => peer.user.userId).sort())
			.toEqual([timetableIds.user, secondUser].sort());
		await expect.poll(() => [...browserBaselines.keys()].sort()).toEqual(['a', 'b']);
		await expect(a.getByRole('button', { name: /ไปยัง ผู้ขยับเมาส์/ })).toBeEnabled();
		await expect(b.getByRole('button', { name: /ไปยัง ผู้ดูรูป/ })).toBeEnabled();
		for (let index = 0; index < 3; index++) {
			const address = new URL(server.url);
			address.searchParams.set('academicTermId', timetableIds.term);
			address.searchParams.set('timetableVersionId', timetableIds.draftVersion);
			address.searchParams.set(
				'fixtureUser',
				`51000000-0000-4000-8000-${String(220 + index).padStart(12, '0')}`
			);
			address.searchParams.set('fixtureName', `ผู้ร่วม ${index}`);
			const socket = new WebSocket(address);
			socket.onclose = (event) =>
				lifecycle.push({
					event: 'extra-close',
					index,
					code: event.code,
					reason: event.reason,
					at: Date.now()
				});
			extraSockets.push(socket);
			await new Promise<void>((resolve, reject) => {
				socket.onmessage = (event) => {
					const decoded = decodeDraftEvent(String(event.data));
					if (decoded?.type === 'DraftStateSync') {
						extraBaselines[index] = decoded.payload.connectionId;
						lifecycle.push({
							event: 'extra-baseline',
							index,
							id: decoded.payload.connectionId,
							at: Date.now()
						});
						resolve();
					}
				};
				socket.onerror = () => reject(new Error('native extra peer failed'));
			});
		}
		await expect
			.poll(() => [...relay.peers.values()].map((peer) => peer.user.userId).sort())
			.toEqual(
				[
					timetableIds.user,
					secondUser,
					...[220, 221, 222].map((id) => `51000000-0000-4000-8000-${String(id).padStart(12, '0')}`)
				].sort()
			);
		expect([...relay.peers.keys()].sort()).toEqual(
			[...browserBaselines.values(), ...extraBaselines].sort()
		);
		for (const socket of extraSockets) expect(socket.readyState).toBe(WebSocket.OPEN);
		for (const peer of relay.peers.values()) peer.user.profileImageFileId = timetableIds.room;
		relay.commit(mock.workspace());
		const images = a.locator('[data-timetable-roster] img');
		await expect(images).toHaveCount(3);
		for (const image of await images.all()) await expect(image).toHaveCSS('visibility', 'visible');
		const nodes = await images.elementHandles(),
			sources = await images.evaluateAll((values) =>
				values.map((image) => image.getAttribute('src'))
			);
		const membership = await a
			.locator('[data-timetable-collaborator]')
			.evaluateAll((values) =>
				values.map((node) => node.getAttribute('data-timetable-collaborator'))
			);
		expect(membership).toEqual([
			secondUser,
			...[220, 221].map((id) => `51000000-0000-4000-8000-${String(id).padStart(12, '0')}`)
		]);
		expect(grants).toHaveLength(3);
		expect(blobs).toHaveLength(3);
		await a.locator('[data-timetable-roster]').evaluate((roster) => {
			const observer = new MutationObserver((records) => {
				const changed = records.filter(
					(record) => record.target instanceof HTMLImageElement
				).length;
				document.documentElement.dataset.avatarChanges = String(
					Number(document.documentElement.dataset.avatarChanges ?? 0) + changed
				);
			});
			observer.observe(roster, {
				subtree: true,
				attributes: true,
				attributeFilter: ['src', 'style']
			});
		});
		for (let index = 0; index < 10; index++)
			await hoverPoint(b, cell(timetableIds.period2), 0.25 + index * 0.04, 0.4);
		const movingId = [...relay.peers.values()].find((peer) => peer.user.userId === secondUser)!.id;
		await cursorNear(a, movingId, cell(timetableIds.period2), 0.61, 0.4);
		for (let index = 0; index < 6; index++)
			extraSockets[2].send(
				JSON.stringify({
					type: 'ClientPresenceUpdate',
					payload: {
						context: { viewMode: 'teacher', viewId: timetableIds.teacherB },
						anchor: {
							kind: 'cell',
							dayOfWeek: 'MON',
							bellSchedulePeriodId: timetableIds.period1,
							u: index / 6,
							v: 0.4
						}
					}
				})
			);
		await a.getByRole('button', { name: 'เลือกผู้ร่วมจัดตารางและแท็บ', exact: true }).click();
		await expect(a.getByRole('menuitem').filter({ hasText: 'ครูคณิตศาสตร์ B' })).toBeVisible();
		await a.keyboard.press('Escape');
		expect(
			await a
				.locator('[data-timetable-collaborator]')
				.evaluateAll((values) =>
					values.map((node) => node.getAttribute('data-timetable-collaborator'))
				)
		).toEqual(membership);
		expect(
			await images.evaluateAll((values) => values.map((image) => image.getAttribute('src')))
		).toEqual(sources);
		for (let index = 0; index < nodes.length; index++) {
			expect(
				await images.nth(index).evaluate((node, original) => node === original, nodes[index])
			).toBe(true);
			await expect(images.nth(index)).toHaveCSS('visibility', 'visible');
		}
		expect(
			await a.evaluate(() => Number(document.documentElement.dataset.avatarChanges ?? 0))
		).toBe(0);
		expect(grants).toHaveLength(3);
		expect(blobs).toHaveLength(3);
		expect(mock.workspaceRequestCount()).toBe(1);
		expect(mock.previewRequestCount()).toBe(0);
	} finally {
		try {
			await mkdir(artifacts, { recursive: true });
			await writeFile(
				`${artifacts}/avatar-peer-lifecycle-${testInfo.repeatEachIndex}.json`,
				JSON.stringify(
					{
						lifecycle,
						peers: [...relay.peers.values()].map((peer) => ({
							id: peer.id,
							userId: peer.user.userId
						}))
					},
					null,
					2
				)
			);
		} finally {
			for (const socket of extraSockets) socket.close();
			try {
				await first.close();
			} finally {
				try {
					await second.close();
				} finally {
					await server.close();
				}
			}
		}
	}
});

test('shared structural and synchronized anchors stay within the selected owner while avatar jumps and overview row/day bridges work', async ({
	browser
}) => {
	const relay = createDraftRelay();
	const first = await browser.newContext({ viewport: { width: 1440, height: 1000 } }),
		second = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
	const a = await first.newPage(),
		b = await second.newPage();
	const otherRoom = '81000000-0000-4000-8000-000000000211';
	const structural = makeStructuralTimetableBlock(
		timetableIds.blockA,
		timetableIds.period1,
		'คาบร่วม',
		[timetableIds.teacherA, timetableIds.teacherB]
	);
	const synchronized = makeSynchronizedTimetableBlock(timetableIds.blockB, timetableIds.period2, [
		timetableIds.teacherA,
		timetableIds.teacherB
	]);
	for (const block of [structural, synchronized])
		block.homerooms.push({
			...block.homerooms[0],
			id: 'f1000000-0000-4000-8000-000000000211',
			homeroomId: otherRoom,
			code: 'M1-2',
			name: 'ม.1/2'
		});
	const mockA = await installTimetableMock(a, {
		socketMode: 'native',
		relay,
		blocks: [structural, synchronized],
		synchronizedDemands: [
			{
				learningOfferingId: timetableIds.offeringSync,
				offeringCode: 'CLUB',
				offeringName: 'ชุมนุม',
				requiredPeriods: 3,
				scheduledPeriods: 1,
				intendedHomeroomIds: [timetableIds.homeroom, otherRoom],
				linkedGroupCount: 0,
				pendingGroupCount: 0,
				conflictGroupCount: 0,
				excludedGroupCount: 0
			}
		]
	});
	const workspace = mockA.workspace();
	workspace.homerooms.push({
		...workspace.homerooms[0],
		id: otherRoom,
		code: 'M1-2',
		name: 'ม.1/2',
		roomNumber: '2'
	});
	workspace.synchronizedDemands[0].intendedHomeroomIds = [timetableIds.homeroom, otherRoom];
	mockA.publishWorkspace(workspace, false);
	const mockB = await installTimetableMock(b, {
		socketMode: 'native',
		relay,
		blocks: workspace.blocks,
		homerooms: workspace.homerooms,
		synchronizedDemands: workspace.synchronizedDemands,
		userId: secondUser
	});
	const server = await startNativeDraftRelay(relay, mockA.workspace);
	try {
		await redirectSocket(first, server.url, timetableIds.user, 'ครูผู้ชี้');
		await redirectSocket(second, server.url, secondUser, 'ครูผู้ดู');
		await a.goto(url('teacher', timetableIds.teacherA));
		await b.goto(url('teacher', timetableIds.teacherA));
		await waitPeers(relay, 2);
		for (const page of [a, b])
			await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const id = [...relay.peers.values()].find((peer) => peer.user.userId === timetableIds.user)!.id;
		const remote = b.locator(`[data-timetable-remote-cursor="${id}"]`);
		const blockSelector = (blockId: string) =>
			`[data-timetable-anchor="block"][data-block-id="${blockId}"]`;
		async function owner(label: string) {
			const selector = b.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' });
			await selector.click();
			await b.getByRole('option', { name: label, exact: true }).click();
			await expect(selector).toContainText(label);
		}
		for (const block of [structural, synchronized]) {
			await hoverPoint(a, blockSelector(block.id));
			await cursorNear(b, id, blockSelector(block.id), 0.65, 0.4);
			await owner('ครูคณิตศาสตร์ B');
			await expect(b.locator(blockSelector(block.id))).toBeVisible();
			await expect(remote).toHaveCount(0);
			await b.getByRole('button', { name: /ไปยัง ครูผู้ชี้/ }).click();
			await cursorNear(b, id, blockSelector(block.id), 0.65, 0.4);
		}
		for (const page of [a, b])
			await page.getByRole('button', { name: 'ชั้น', exact: true }).click();
		for (const block of [structural, synchronized]) {
			await hoverPoint(a, blockSelector(block.id));
			await cursorNear(b, id, blockSelector(block.id), 0.65, 0.4);
			await owner('ม.1/2');
			await expect(b.locator(blockSelector(block.id))).toBeVisible();
			await expect(remote).toHaveCount(0);
			await owner('ม.1/1');
		}
		const tray = 'aside article[data-timetable-source-kind="synchronized_offering"]';
		await expect(a.locator(tray)).toBeVisible();
		await expect(b.locator(tray)).toBeVisible();
		await hoverPoint(a, tray);
		await cursorNear(b, id, tray, 0.65, 0.4);
		await owner('ม.1/2');
		await expect(b.locator(tray)).toBeVisible();
		await expect(remote).toHaveCount(0);
		await b.getByRole('button', { name: /ไปยัง ครูผู้ชี้/ }).click();
		await cursorNear(b, id, tray, 0.65, 0.4);
		await hoverPoint(a, blockSelector(synchronized.id));
		await b.getByRole('button', { name: 'โรงเรียน', exact: true }).click();
		await hoverPoint(
			b,
			`td[data-timetable-owner-id="${otherRoom}"][data-timetable-period-id="${timetableIds.period3}"]`
		);
		await cursorNear(
			b,
			id,
			`${blockSelector(synchronized.id)}[data-row-id="${timetableIds.homeroom}"]`,
			0.65,
			0.4
		);
		await a.getByRole('button', { name: 'โรงเรียน', exact: true }).click();
		await hoverPoint(a, `${blockSelector(synchronized.id)}[data-row-id="${otherRoom}"]`);
		await cursorNear(
			b,
			id,
			`${blockSelector(synchronized.id)}[data-row-id="${otherRoom}"]`,
			0.65,
			0.4
		);
		await b.getByRole('combobox', { name: 'เลือกวันดูภาพรวม', exact: true }).click();
		await b.getByRole('option', { name: 'วันอังคาร', exact: true }).click();
		await expect(remote).toHaveCount(0);
		await b.getByRole('combobox', { name: 'เลือกวันดูภาพรวม', exact: true }).click();
		await b.getByRole('option', { name: 'วันจันทร์', exact: true }).click();
		await cursorNear(
			b,
			id,
			`${blockSelector(synchronized.id)}[data-row-id="${otherRoom}"]`,
			0.65,
			0.4
		);
		for (const mock of [mockA, mockB]) {
			expect(mock.workspaceRequestCount()).toBe(1);
			expect(mock.previewRequestCount()).toBe(0);
		}
	} finally {
		await first.close();
		await second.close();
		await server.close();
	}
});

test('committed native saves release consumed proofs without warnings while an unrelated pending save keeps its lease', async ({
	browser
}) => {
	let releaseCreate!: () => void;
	const createGate = new Promise<void>((resolve) => {
		releaseCreate = resolve;
	});
	const relay = createDraftRelay();
	const first = await browser.newContext({ viewport: { width: 1440, height: 1000 } }),
		second = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
	const a = await first.newPage(),
		b = await second.newPage();
	const block = makeTimetableBlock(timetableIds.blockB, timetableIds.period3, {
		groupId: timetableIds.groupB,
		offeringId: timetableIds.offeringB,
		code: 'ว21101',
		name: 'วิทยาศาสตร์พื้นฐาน',
		instructorIds: [timetableIds.teacherB]
	});
	const mock = await installTimetableMock(a, {
		socketMode: 'native',
		relay,
		blocks: [block],
		createGate,
		requiredPeriods: 2
	});
	await installTimetableMock(b, {
		socketMode: 'native',
		relay,
		blocks: [block],
		userId: secondUser,
		requiredPeriods: 2
	});
	const warnings: string[] = [],
		writes: number[] = [];
	a.on('console', (message) => {
		if (
			message.type() === 'error' &&
			message.text().includes('Unable to release timetable interaction; its lease will expire')
		)
			warnings.push(message.text());
	});
	a.on('response', (response) => {
		if (
			/\/api\/academic\/timetable-blocks\/(ordinary|[0-9a-f-]{36})$/.test(
				new URL(response.url()).pathname
			) &&
			['POST', 'PUT'].includes(response.request().method())
		)
			writes.push(response.status());
	});
	const server = await startNativeDraftRelay(relay, mock.workspace);
	try {
		await redirectSocket(first, server.url, timetableIds.user, 'ครูผู้บันทึก');
		await redirectSocket(second, server.url, secondUser, 'ผู้สังเกต');
		await a.goto(url());
		await b.goto(url());
		await waitPeers(relay, 2);
		for (const page of [a, b])
			await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		async function drop(source: Locator, period: string) {
			const from = await source.boundingBox(),
				to = await a.locator(cell(period)).boundingBox();
			if (!from || !to) throw new Error('missing native drag geometry');
			await a.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
			await a.mouse.down();
			await a.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 18 });
			await a.mouse.up();
		}
		const demand = (page: Page) =>
			page
				.locator(`aside article[data-timetable-source-kind="ordinary_demand"]`)
				.filter({ hasText: 'คณิตศาสตร์พื้นฐาน' });
		await drop(demand(a), timetableIds.period1);
		await expect.poll(mock.createRequestCount).toBe(1);
		await expect(demand(b)).toHaveAttribute('draggable', 'false');
		const card = (page: Page) =>
			page.locator(`[data-timetable-anchor="block"][data-block-id="${block.id}"]`);
		await drop(card(a), timetableIds.period2);
		await expect.poll(mock.updateRequestCount).toBe(1);
		await expect(
			b.locator(cell(timetableIds.period2)).locator(`[data-block-id="${block.id}"]`)
		).toBeVisible();
		await drop(card(a), timetableIds.period3);
		await expect.poll(mock.updateRequestCount).toBe(2);
		await expect(
			b.locator(cell(timetableIds.period3)).locator(`[data-block-id="${block.id}"]`)
		).toBeVisible();
		await card(a)
			.getByRole('button', { name: /ดูรายละเอียด/ })
			.click();
		const dialog = a.getByRole('dialog');
		await expect(dialog.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
		await dialog.getByRole('combobox', { name: 'เลือกห้องเรียน', exact: true }).click();
		await a.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true }).click();
		await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
		await expect(card(b)).toContainText('ห้องปฏิบัติการ 2');
		// A no-proof publication must not consume the other captured create lease either.
		relay.commit(mock.workspace());
		await expect
			.poll(() => relay.received.filter((entry) => entry.message.type === 'ClientRenew').length)
			.toBeGreaterThan(0);
		await expect(demand(b)).toHaveAttribute('draggable', 'false');
		releaseCreate();
		await expect(
			b.locator(`[data-timetable-anchor="block"][data-block-id="${timetableIds.createdBlock}"]`)
		).toBeVisible();
		await expect(a.getByLabel('กำลังบันทึกคาบ คณิตศาสตร์พื้นฐาน')).toHaveCount(0);
		await expect(demand(b)).toHaveAttribute('draggable', 'true');
		// The next successful claim orders its reply after the prior terminal release replies.
		await expect(dialog).toHaveCount(0);
		await card(a)
			.getByRole('button', { name: /ดูรายละเอียด/ })
			.click();
		await expect(dialog.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
		expect(warnings).toEqual([]);
		expect(writes).toEqual([200, 200, 200, 200]);
		expect(mock.createRequestCount()).toBe(1);
		expect(mock.updateRequestCount()).toBe(3);
		expect(mock.previewRequestCount()).toBe(3);
		expect(mock.workspaceRequestCount()).toBe(1);
		expect(relay.workspace?.blocks).toHaveLength(2);
	} finally {
		releaseCreate();
		await first.close();
		await second.close();
		await server.close();
	}
});

test('two unready native drops time out without writes and only an explicit retry after baseline creates one block', async ({
	browser
}) => {
	const relay = createDraftRelay();
	relay.holdStateSync();
	const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
	const page = await context.newPage();
	const mock = await installTimetableMock(page, { socketMode: 'native', relay });
	const server = await startNativeDraftRelay(relay, mock.workspace);
	try {
		await redirectSocket(context, server.url, timetableIds.user, 'ครูรอการเชื่อมต่อ');
		await page.goto(url());
		await waitPeers(relay, 1);
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const source = page.locator('aside article').first();
		const target = page.locator(cell(timetableIds.period1));
		async function drop() {
			const sourceBox = await source.boundingBox(),
				targetBox = await target.boundingBox();
			if (!sourceBox || !targetBox) throw new Error('missing native drag geometry');
			await page.mouse.move(sourceBox.x + sourceBox.width / 2, sourceBox.y + sourceBox.height / 2);
			await page.mouse.down();
			await page.mouse.move(targetBox.x + targetBox.width / 2, targetBox.y + targetBox.height / 2, {
				steps: 18
			});
			await page.mouse.up();
		}
		for (let attempt = 0; attempt < 2; attempt++) {
			await drop();
			await expect(page.getByLabel('กำลังบันทึกคาบ คณิตศาสตร์พื้นฐาน')).toBeVisible();
			await expect(page.locator('[data-timetable-lesson-card]')).toHaveCount(0);
			await expect(
				page.getByText('ยังเชื่อมต่อผู้ร่วมจัดตารางไม่ได้ กรุณาลองใหม่', { exact: true }).last()
			).toBeVisible();
			expect(relay.received.filter((entry) => entry.message.type === 'ClientClaim')).toHaveLength(
				0
			);
			expect(mock.previewRequestCount()).toBe(0);
			expect(mock.createRequestCount()).toBe(0);
			expect(mock.updateRequestCount()).toBe(0);
			await expect(source).toHaveAttribute('draggable', 'true');
		}
		relay.releaseStateSync();
		await expect
			.poll(() => relay.received.some((entry) => entry.message.type === 'ClientPresenceUpdate'))
			.toBe(true);
		expect(mock.createRequestCount()).toBe(0);
		expect(mock.previewRequestCount()).toBe(0);
		await drop();
		await expect.poll(mock.createRequestCount).toBe(1);
		await expect(page.getByLabel('กำลังบันทึกคาบ คณิตศาสตร์พื้นฐาน')).toHaveCount(0);
		await expect(page.locator('[data-timetable-lesson-card]')).toHaveCount(1);
		expect(mock.blocks()).toHaveLength(1);
		expect(relay.workspace?.blocks).toHaveLength(1);
		expect(mock.previewRequestCount()).toBe(1);
		expect(mock.createRequestCount()).toBe(1);
		expect(mock.updateRequestCount()).toBe(0);
		expect(mock.workspaceRequestCount()).toBe(1);
	} finally {
		await context.close();
		await server.close();
	}
});

test('native draft joins, edits and patches preserve valid nullable metadata without reconnecting or reloading', async ({
	browser
}) => {
	const relay = createDraftRelay();
	const first = await browser.newContext(),
		second = await browser.newContext();
	const a = await first.newPage(),
		b = await second.newPage();
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	block.groups[0].roomCode = null;
	const mockA = await installTimetableMock(a, { socketMode: 'native', relay, blocks: [block] });
	const mockB = await installTimetableMock(b, {
		socketMode: 'native',
		relay,
		blocks: [block],
		userId: secondUser
	});
	const workspace = mockA.workspace();
	workspace.version.createdBy = null;
	workspace.homerooms[0].roomNumber = null;
	workspace.rooms[0].code = null;
	mockA.publishWorkspace(workspace, false);
	mockB.publishWorkspace(workspace, false);
	const draftSocketsA: string[] = [],
		draftSocketsB: string[] = [];
	a.on('websocket', (socket) => {
		if (new URL(socket.url()).searchParams.has('timetableVersionId'))
			draftSocketsA.push(socket.url());
	});
	b.on('websocket', (socket) => {
		if (new URL(socket.url()).searchParams.has('timetableVersionId'))
			draftSocketsB.push(socket.url());
	});
	const server = await startNativeDraftRelay(relay, mockA.workspace);
	try {
		await redirectSocket(first, server.url, timetableIds.user, 'ครูหนึ่ง');
		await redirectSocket(second, server.url, secondUser, 'ครูสอง');
		await a.goto(url());
		await b.goto(url());
		await waitPeers(relay, 2);
		await expect(a.getByRole('button', { name: /ไปยัง ครูสอง/ })).toBeVisible();
		await expect(b.getByRole('button', { name: /ไปยัง ครูหนึ่ง/ })).toBeVisible();
		const connectionIds = [...relay.peers.keys()].sort();
		await a.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const card = (page: Page) =>
			page.locator(`[data-timetable-anchor="block"][data-block-id="${block.id}"]`);
		await card(a)
			.getByRole('button', { name: /ดูรายละเอียด/ })
			.click();
		const dialog = a.getByRole('dialog');
		await expect(dialog.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
		await dialog.getByRole('combobox', { name: 'เลือกห้องเรียน', exact: true }).click();
		await a.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true }).click();
		await dialog
			.getByRole('textbox', { name: 'หมายเหตุ', exact: true })
			.fill('บันทึกผ่านข้อมูลที่อนุญาตให้เป็น null');
		await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
		await expect(card(b)).toContainText('ห้องปฏิบัติการ 2');
		const updated = mockA.workspace();
		updated.rooms[0].name = 'ห้องคณิตศาสตร์ใหม่';
		updated.homerooms[0].name = 'ม.1/1 ปรับปรุง';
		updated.version.rowVersion++;
		mockA.publishWorkspace(updated);
		await expect(
			b.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง', exact: true })
		).toContainText('ม.1/1 ปรับปรุง');
		await card(a)
			.getByRole('button', { name: /ดูรายละเอียด/ })
			.click();
		await dialog.getByRole('combobox', { name: 'เลือกห้องเรียน', exact: true }).click();
		await expect(a.getByRole('option', { name: 'ห้องคณิตศาสตร์ใหม่', exact: true })).toBeVisible();
		expect(relay.workspace?.version.createdBy).toBeNull();
		expect(relay.workspace?.homerooms[0].roomNumber).toBeNull();
		expect(relay.workspace?.rooms[0].code).toBeNull();
		expect([...relay.peers.keys()].sort()).toEqual(connectionIds);
		expect(draftSocketsA).toHaveLength(1);
		expect(draftSocketsB).toHaveLength(1);
		for (const mock of [mockA, mockB]) {
			expect(mock.workspaceRequestCount()).toBe(1);
			expect(mock.previewRequestCount()).toBe(0);
		}
		expect(mockA.updateRequestCount()).toBe(1);
		await expect(
			a.getByText('ยังเชื่อมต่อผู้ร่วมจัดตารางไม่ได้ กรุณาลองใหม่', { exact: true })
		).toHaveCount(0);
		await a.screenshot({
			path: `${artifacts}/nullable-metadata-joined.png`,
			animations: 'disabled'
		});
	} finally {
		await first.close();
		await second.close();
		await server.close();
	}
});

test('room feedback distinguishes pending claims from locked, denied, lost and saving editors', async ({
	browser
}) => {
	const relay = createDraftRelay();
	const first = await browser.newContext(),
		second = await browser.newContext();
	const a = await first.newPage(),
		b = await second.newPage();
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const mockA = await installTimetableMock(a, {
		socketMode: 'native',
		relay,
		blocks: [block],
		updateDelayMs: 1000
	});
	const mockB = await installTimetableMock(b, {
		socketMode: 'native',
		relay,
		blocks: [block],
		userId: secondUser
	});
	const server = await startNativeDraftRelay(relay, mockA.workspace);
	try {
		await redirectSocket(first, server.url, timetableIds.user, 'ครูหนึ่ง');
		await redirectSocket(second, server.url, secondUser, 'ครูสอง');
		await a.goto(url('wholeSchool'));
		await b.goto(url());
		await waitPeers(relay, 2);
		for (const page of [a, b])
			await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const openEditor = (page: Page) =>
			page
				.locator(`[data-timetable-anchor="block"][data-block-id="${block.id}"]`)
				.getByRole('button', { name: /ดูรายละเอียด/ })
				.click();
		const picker = (page: Page) =>
			page.getByRole('dialog').getByRole('combobox', { name: 'เลือกห้องเรียน', exact: true });
		const save = (page: Page) =>
			page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true });
		const status = (page: Page) => page.getByRole('dialog').getByRole('status');
		const close = (page: Page) =>
			page.getByRole('dialog').getByRole('button', { name: 'ปิด', exact: true }).click();
		const bId = [...relay.peers.values()].find((peer) => peer.user.userId === secondUser)!.id;
		const bClaims = () =>
			relay.received.filter(
				(entry) => entry.connectionId === bId && entry.message.type === 'ClientClaim'
			).length;
		relay.holdClaim();
		await openEditor(a);
		await expect(status(a)).toHaveText('กำลังตรวจสอบห้องว่าง…');
		await expect(picker(a)).toHaveAttribute('aria-busy', 'true');
		await expect(save(a)).toBeDisabled();
		await expect
			.poll(() => relay.received.filter((entry) => entry.message.type === 'ClientClaim').length)
			.toBe(1);
		relay.releaseClaim();
		await expect(save(a)).toBeEnabled();
		await openEditor(b);
		await expect(status(b)).toHaveText('ครูหนึ่ง กำลังแก้ไข');
		await expect(picker(b)).toBeDisabled();
		await expect(picker(b)).toHaveAttribute('aria-busy', 'false');
		await expect(save(b)).toBeDisabled();
		expect(bClaims()).toBe(0);
		await close(a);
		await expect(status(b)).toContainText('ขณะนี้แก้ไขคาบนี้ไม่ได้');
		expect(bClaims()).toBe(0);
		await close(b);
		relay.denyClaim();
		await openEditor(b);
		await expect(status(b)).toContainText('ขณะนี้แก้ไขคาบนี้ไม่ได้');
		await expect(picker(b)).toHaveAttribute('aria-busy', 'false');
		await expect(save(b)).toBeDisabled();
		expect(bClaims()).toBe(1);
		await close(b);
		await openEditor(b);
		await expect(save(b)).toBeEnabled();
		relay.reconcile();
		await expect.poll(() => relay.peers.has(bId)).toBe(false);
		await waitPeers(relay, 2);
		await expect(status(b)).toContainText('ขณะนี้แก้ไขคาบนี้ไม่ได้');
		await expect(picker(b)).toHaveAttribute('aria-busy', 'false');
		await expect(save(b)).toBeDisabled();
		await close(b);
		await openEditor(a);
		await expect(save(a)).toBeEnabled();
		await a
			.getByRole('dialog')
			.getByRole('textbox', { name: 'หมายเหตุ', exact: true })
			.fill('ทดสอบสถานะกำลังบันทึก');
		await save(a).click();
		await expect(a.getByRole('dialog')).toHaveCount(0);
		await openEditor(a);
		await expect(status(a)).toHaveText('กำลังบันทึกคาบนี้ กรุณารอให้เสร็จก่อน');
		await expect(picker(a)).toHaveAttribute('aria-busy', 'false');
		await expect(save(a)).toBeDisabled();
		await expect(status(a)).toContainText('ขณะนี้แก้ไขคาบนี้ไม่ได้');
		expect(mockA.updateRequestCount()).toBe(1);
		for (const mock of [mockA, mockB]) {
			expect(mock.workspaceRequestCount()).toBe(1);
			expect(mock.previewRequestCount()).toBe(0);
		}
	} finally {
		await first.close();
		await second.close();
		await server.close();
	}
});

test('placed-block room choices wait for private availability, update on canonical conflicts and retain invalid or historical selections', async ({
	browser
}) => {
	const relay = createDraftRelay();
	relay.setRoomOptionsPaused(true);
	const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
	const page = await context.newPage();
	const original = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const occupied = makeTimetableBlock(timetableIds.blockB, timetableIds.period1, {
		groupId: timetableIds.groupB,
		offeringId: timetableIds.offeringB,
		roomId: timetableIds.roomB,
		code: 'อ21101',
		name: 'ภาษาอังกฤษ'
	});
	const mock = await installTimetableMock(page, {
		socketMode: 'native',
		relay,
		blocks: [original, occupied]
	});
	const server = await startNativeDraftRelay(relay, mock.workspace);
	try {
		await redirectSocket(context, server.url, timetableIds.user, 'ผู้แก้ไขห้อง');
		await page.goto(url());
		await waitPeers(relay, 1);
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const openEditor = () =>
			page
				.locator(`[data-timetable-anchor="block"][data-block-id="${original.id}"]`)
				.getByRole('button', { name: /ดูรายละเอียด/ })
				.click();
		await openEditor();
		const dialog = page.getByRole('dialog');
		const picker = dialog.getByRole('combobox', { name: 'เลือกห้องเรียน', exact: true });
		const save = dialog.getByRole('button', { name: 'บันทึก', exact: true });
		await expect(picker).toHaveText('ห้องคณิตศาสตร์');
		await expect(picker).toBeDisabled();
		await expect(picker).toHaveAttribute('aria-busy', 'true');
		await expect(save).toBeDisabled();
		await expect(dialog.getByRole('status')).toHaveText('กำลังตรวจสอบห้องว่าง…');
		relay.setRoomOptionsPaused(false);
		await expect(save).toBeEnabled();
		const initialGets = mock.workspaceRequestCount();
		await picker.click();
		await expect(page.getByRole('option', { name: 'ห้องคณิตศาสตร์', exact: true })).toBeVisible();
		await expect(page.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true })).toHaveCount(
			0
		);
		await expect(page.getByRole('option', { name: 'ไม่ระบุห้อง', exact: true })).toBeVisible();
		const next = mock.workspace();
		next.blocks = [
			original,
			makeTimetableBlock(occupied.id, timetableIds.period2, {
				groupId: timetableIds.groupB,
				offeringId: timetableIds.offeringB,
				roomId: timetableIds.roomB,
				code: 'อ21101',
				name: 'ภาษาอังกฤษ'
			})
		];
		mock.publishWorkspace(next);
		await page.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true }).click();
		await expect(picker).toHaveText('ห้องปฏิบัติการ 2');
		await expect(save).toBeEnabled();
		const conflicted = mock.workspace();
		conflicted.blocks = [original, occupied];
		mock.publishWorkspace(conflicted);
		await expect(save).toBeDisabled();
		await expect(picker).toHaveText('ห้องปฏิบัติการ 2');
		await expect(dialog.getByRole('status')).toContainText('กรุณาเลือกห้องใหม่');
		await picker.click();
		await expect(page.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true })).toHaveCount(
			0
		);
		await page.getByRole('option', { name: 'ไม่ระบุห้อง', exact: true }).click();
		await expect(save).toBeEnabled();
		const freed = mock.workspace();
		freed.blocks = [original];
		mock.publishWorkspace(freed);
		await picker.click();
		await expect(page.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true })).toBeVisible();
		await page.keyboard.press('Escape');
		await dialog.getByRole('button', { name: 'ปิด', exact: true }).click();
		const inactive = mock.workspace();
		inactive.rooms = inactive.rooms.map((room) => ({ ...room, status: 'INACTIVE' }));
		mock.publishWorkspace(inactive);
		await openEditor();
		await expect(picker).toHaveText('ห้องคณิตศาสตร์');
		await expect(save).toBeEnabled();
		await expect(dialog.getByRole('status')).toContainText('ไม่มีห้องว่าง');
		await picker.click();
		await expect(page.getByRole('option', { name: 'ห้องคณิตศาสตร์', exact: true })).toHaveCount(0);
		await expect(page.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true })).toHaveCount(
			0
		);
		await page.getByRole('option', { name: 'ไม่ระบุห้อง', exact: true }).click();
		await expect(save).toBeEnabled();
		const previousConnection = [...relay.peers.keys()][0];
		relay.setRoomOptionsPaused(true);
		relay.reconcile();
		await expect.poll(() => [...relay.peers.keys()][0]).not.toBe(previousConnection);
		await waitPeers(relay, 1);
		await expect(dialog).toBeVisible();
		await expect(save).toBeDisabled();
		await expect(picker).toBeDisabled();
		await expect(picker).toHaveText('ไม่ระบุห้อง');
		await expect(dialog.getByRole('status')).toContainText('ขณะนี้แก้ไขคาบนี้ไม่ได้');
		await expect(picker).toHaveAttribute('aria-busy', 'false');
		await dialog.getByRole('button', { name: 'ปิด', exact: true }).click();
		relay.setRoomOptionsPaused(false);
		await openEditor();
		await expect(save).toBeEnabled();
		await expect(picker).toHaveText('ห้องคณิตศาสตร์');
		expect(mock.workspaceRequestCount()).toBe(initialGets);
		expect(mock.previewRequestCount()).toBe(0);
		expect(mock.updateRequestCount()).toBe(0);
		await page.screenshot({
			path: `${artifacts}/private-room-options-empty.png`,
			animations: 'disabled'
		});
	} finally {
		await context.close();
		await server.close();
	}
});

test('private room choices react to another native editor reservation without exposing its candidate to peers', async ({
	browser
}) => {
	const relay = createDraftRelay();
	const first = await browser.newContext(),
		second = await browser.newContext();
	const a = await first.newPage(),
		b = await second.newPage();
	const blocks = [
		makeTimetableBlock(timetableIds.blockA, timetableIds.period1),
		makeTimetableBlock(timetableIds.blockB, timetableIds.period1, {
			groupId: timetableIds.groupB,
			offeringId: timetableIds.offeringB,
			roomId: null,
			name: 'ภาษาอังกฤษ'
		})
	];
	const mockA = await installTimetableMock(a, { socketMode: 'native', relay, blocks });
	const mockB = await installTimetableMock(b, {
		socketMode: 'native',
		relay,
		blocks,
		userId: secondUser
	});
	const server = await startNativeDraftRelay(relay, mockA.workspace);
	try {
		await redirectSocket(first, server.url, timetableIds.user, 'ครูหนึ่ง');
		await redirectSocket(second, server.url, secondUser, 'ครูสอง');
		await a.goto(url());
		await b.goto(url());
		await waitPeers(relay, 2);
		for (const page of [a, b])
			await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		await a
			.locator(`[data-timetable-anchor="block"][data-block-id="${timetableIds.blockA}"]`)
			.getByRole('button', { name: /ดูรายละเอียด/ })
			.click();
		await b
			.locator(`[data-timetable-anchor="block"][data-block-id="${timetableIds.blockB}"]`)
			.getByRole('button', { name: /ดูรายละเอียด/ })
			.click();
		const pickerA = a
			.getByRole('dialog')
			.getByRole('combobox', { name: 'เลือกห้องเรียน', exact: true });
		const pickerB = b
			.getByRole('dialog')
			.getByRole('combobox', { name: 'เลือกห้องเรียน', exact: true });
		await expect(pickerA).toBeEnabled();
		await pickerB.click();
		await b.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true }).click();
		await pickerA.click();
		await expect(a.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true })).toHaveCount(0);
		await pickerB.click();
		await b.getByRole('option', { name: 'ไม่ระบุห้อง', exact: true }).click();
		await expect(a.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true })).toBeVisible();
		await a.keyboard.press('Escape');
		for (const mock of [mockA, mockB]) {
			expect(mock.workspaceRequestCount()).toBe(1);
			expect(mock.previewRequestCount()).toBe(0);
			expect(mock.updateRequestCount()).toBe(0);
		}
		for (const peer of relay.peers.values()) {
			expect(peer.user.connections[0]).not.toHaveProperty('candidate');
		}
	} finally {
		await first.close();
		await second.close();
		await server.close();
	}
});

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
		await expect
			.poll(() =>
				a.evaluate(
					() =>
						document
							.getAnimations()
							.filter(
								(animation) =>
									animation.playState === 'running' &&
									Number.isFinite(animation.effect?.getComputedTiming().endTime)
							).length
				)
			)
			.toBe(0);
		const stationary = await cursorAtStationarySender(a, b, relay, aId, senderPoint);
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
		mock.publishNewOpening();
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
		expect(mock.sourceUpdateBodies()).toEqual([{ rowVersion: 1 }]);
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

test('native scoped lookup gains and losses match authorized rows and repair an owner without reloading the workspace', async ({
	browser
}) => {
	const relay = createDraftRelay(),
		context = await browser.newContext({ viewport: { width: 1440, height: 1000 } }),
		page = await context.newPage();
	const block = makeStructuralTimetableBlock(
		timetableIds.blockA,
		timetableIds.period1,
		'ประชุมทีม',
		[timetableIds.teacherA]
	);
	block.teachers[0].displayName = 'ครู เอ';
	Object.assign(block.homerooms[0], {
		roomId: timetableIds.room,
		roomCode: 'OLD',
		roomName: 'ห้องเดิม'
	});
	const mock = await installTimetableMock(page, {
		socketMode: 'native',
		relay,
		blocks: [block],
		periodCount: 1,
		learningGroups: [],
		ordinaryDemands: [],
		staff: [
			{ id: timetableIds.teacherA, displayName: 'ครู เอ', status: 'active', subjectGroups: [] }
		],
		rooms: [{ id: timetableIds.room, code: 'OLD', name: 'ห้องเดิม', status: 'ACTIVE' }]
	});
	const initial = mock.workspace();
	initial.bellPeriods[0].isActive = false;
	mock.publishWorkspace(initial, false);
	const server = await startNativeDraftRelay(relay, mock.workspace);
	try {
		await redirectSocket(context, server.url, timetableIds.user, 'ผู้ดูในขอบเขต');
		await page.goto(url('teacher', timetableIds.teacherA));
		await waitPeers(relay, 1);
		const gain = structuredClone(mock.workspace()),
			roomId = '81000000-0000-4000-8000-000000000211';
		const gainedStaff = {
			id: timetableIds.teacherB,
			displayName: 'ครู บี',
			status: 'active',
			subjectGroups: [{ id: timetableIds.groupB, name: 'กลุ่มสาระใหม่', displayOrder: null }]
		};
		gain.staff = [gainedStaff, ...gain.staff];
		gain.rooms = [
			{ id: timetableIds.roomB, code: 'NEW', name: 'ห้องที่เพิ่งมองเห็น', status: 'ACTIVE' },
			...gain.rooms
		];
		gain.homerooms = [
			{
				...gain.homerooms[0],
				id: roomId,
				name: 'ป.2/1',
				code: 'P2-1',
				gradeLevelType: 'primary',
				gradeLevelYear: 2
			},
			...gain.homerooms
		];
		gain.bellPeriods.push({
			...gain.bellPeriods[0],
			id: timetableIds.period2,
			name: 'คาบ 2',
			orderIndex: 2
		});
		gain.blocks = [
			block,
			{
				...block,
				id: timetableIds.blockB,
				bellSchedulePeriodId: timetableIds.period2,
				periodName: 'คาบ 2',
				rowVersion: 2,
				teachers: [
					...block.teachers,
					{
						id: 'e1000000-0000-4000-8000-000000000211',
						teacherId: timetableIds.teacherB,
						displayName: gainedStaff.displayName,
						rowVersion: 1,
						isActive: true
					}
				],
				homerooms: [
					...block.homerooms.map((target) => ({
						...target,
						roomId: timetableIds.roomB,
						roomCode: 'NEW',
						roomName: 'ห้องที่เพิ่งมองเห็น'
					})),
					{
						...block.homerooms[0],
						id: 'f1000000-0000-4000-8000-000000000211',
						homeroomId: roomId,
						name: 'ป.2/1',
						code: 'P2-1',
						roomId: timetableIds.roomB,
						roomCode: 'NEW',
						roomName: 'ห้องที่เพิ่งมองเห็น'
					}
				]
			}
		];
		gain.version.rowVersion++;
		const gainEvent = mock.publishWorkspace(gain);
		expect(gainEvent.payload.lookupChanges?.staffOrder).toEqual(gain.staff.map((row) => row.id));
		const owner = page.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' });
		await owner.click();
		await expect(page.getByRole('option')).toHaveText(gain.staff.map((row) => row.displayName));
		await page.getByRole('option', { name: 'ครู บี', exact: true }).click();
		await expect(owner).toHaveText('ครู บี');
		await expect(page.locator('thead th')).toContainText(['วัน / คาบ', 'คาบ 1', 'คาบ 2']);
		await page.getByRole('button', { name: /ดูรายละเอียด ประชุมทีม/ }).click();
		await expect(
			page.getByRole('dialog').getByRole('combobox', { name: 'เลือกห้องเรียน' })
		).toHaveText('ห้องที่เพิ่งมองเห็น');
		await page.keyboard.press('Escape');
		await page.getByRole('button', { name: 'ชั้น', exact: true }).click();
		await owner.click();
		await expect(page.getByRole('option')).toHaveText(['ป.2/1', 'ม.1/1']);
		await page.keyboard.press('Escape');
		await page.getByRole('button', { name: 'ครู', exact: true }).click();
		await owner.click();
		await page.getByRole('option', { name: 'ครู บี', exact: true }).click();
		const lost = structuredClone(gain);
		lost.staff = lost.staff.filter((row) => row.id !== timetableIds.teacherB);
		lost.homerooms = lost.homerooms.filter((row) => row.id !== roomId);
		lost.rooms = lost.rooms.filter((row) => row.id !== timetableIds.room);
		lost.bellPeriods = lost.bellPeriods.filter((row) => row.id !== timetableIds.period1);
		lost.blocks = [
			{
				...gain.blocks[1],
				rowVersion: 3,
				bellSchedulePeriodId: timetableIds.period2,
				periodName: 'คาบ 2',
				teachers: block.teachers,
				homerooms: gain.blocks[1].homerooms.filter((target) => target.homeroomId !== roomId)
			}
		];
		lost.version.rowVersion++;
		mock.publishWorkspace(lost);
		await expect(owner).toHaveText('ครู เอ');
		await expect(page).toHaveURL(new RegExp(`ownerId=${timetableIds.teacherA}`));
		await owner.click();
		await expect(page.getByRole('option')).toHaveText(lost.staff.map((row) => row.displayName));
		await page.keyboard.press('Escape');
		await expect(page.locator('thead th')).toContainText(['วัน / คาบ', 'คาบ 2']);
		expect(mock.workspaceRequestCount()).toBe(1);
	} finally {
		await context.close();
		await server.close();
	}
});

test('an inconsistent queued lookup order waits for the captured save then rejoins one complete canonical snapshot', async ({
	browser
}) => {
	let release!: () => void;
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	const relay = createDraftRelay(),
		context = await browser.newContext({ viewport: { width: 1440, height: 1000 } }),
		page = await context.newPage();
	const mock = await installTimetableMock(page, { socketMode: 'native', relay, createGate: gate });
	const server = await startNativeDraftRelay(relay, mock.workspace);
	try {
		await redirectSocket(context, server.url, timetableIds.user, 'ผู้จัดคิว');
		await page.goto(url());
		await waitPeers(relay, 1);
		const id = [...relay.peers.keys()][0];
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const tray = page.locator('aside article').first(),
			transfer = await page.evaluateHandle(() => new DataTransfer());
		await tray.dispatchEvent('dragstart', { dataTransfer: transfer });
		await page
			.locator(cell(timetableIds.period1))
			.dispatchEvent('dragover', { dataTransfer: transfer });
		await page
			.locator(cell(timetableIds.period1))
			.dispatchEvent('drop', { dataTransfer: transfer });
		await tray.dispatchEvent('dragend', { dataTransfer: transfer });
		await expect.poll(mock.createRequestCount).toBe(1);
		const malformed = relay.commit(mock.workspace(), {}, false);
		malformed.payload.lookupChanges = {
			...emptyLookupChanges(),
			staffOrder: ['51000000-0000-4000-8000-000000000999']
		};
		relay.send(id, malformed);
		const canonical = structuredClone(mock.workspace());
		canonical.blocks = [
			makeTimetableBlock(timetableIds.blockB, timetableIds.period2, {
				groupId: timetableIds.groupB,
				offeringId: timetableIds.offeringB
			})
		];
		const later = mock.publishWorkspace(canonical, false);
		relay.send(id, later);
		expect([...relay.peers.keys()]).toEqual([id]);
		await expect(page.locator(`[data-block-id="${timetableIds.blockB}"]`)).toHaveCount(0);
		release();
		await expect.poll(() => [...relay.peers.keys()][0]).not.toBe(id);
		await waitPeers(relay, 1);
		await expect(page.locator(`[data-block-id="${timetableIds.createdBlock}"]`)).toHaveCount(1);
		await expect(page.locator(`[data-block-id="${timetableIds.blockB}"]`)).toHaveCount(1);
		await expect(page.locator('[data-pending="true"]')).toHaveCount(0);
		expect(mock.workspaceRequestCount()).toBe(1);
		expect(mock.previewRequestCount()).toBe(1);
	} finally {
		release();
		await context.close();
		await server.close();
	}
});

test('same-session metadata retains a native lease while same-user session replacement invalidates its proof and rejoins', async ({
	browser
}) => {
	const relay = createDraftRelay(),
		context = await browser.newContext(),
		page = await context.newPage();
	const mock = await installTimetableMock(page, {
		socketMode: 'native',
		relay,
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	const server = await startNativeDraftRelay(relay, mock.workspace);
	async function announce(message: object) {
		await page.evaluate((message) => {
			const channel = new BroadcastChannel(`schoolorbit:session-activity:${location.origin}`);
			channel.postMessage(message);
			channel.close();
		}, message);
	}
	try {
		await redirectSocket(context, server.url, timetableIds.user, 'ผู้จัดคนเดิม');
		await page.goto(url());
		await waitPeers(relay, 1);
		const id = [...relay.peers.keys()][0];
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
		const dialog = page.getByRole('dialog'),
			save = dialog.getByRole('button', { name: 'บันทึก', exact: true });
		await expect(save).toBeEnabled();
		await dialog.getByRole('textbox', { name: 'หมายเหตุ', exact: true }).fill('ค่าที่ยังไม่บันทึก');
		const reservation = relay.peers.get(id)!.user.connections[0].activeReservationId;
		const current = mock.getSessionState(),
			renewed = sessionState(Date.now() + 1000, { sessionId: current.sessionId });
		mock.setSessionState(renewed);
		await announce({ type: 'metadata', session: renewed });
		await expect(save).toBeEnabled();
		expect([...relay.peers.keys()]).toEqual([id]);
		expect(relay.peers.get(id)!.user.connections[0].activeReservationId).toBe(reservation);
		const next = sessionState(Date.now() + 2000, {
			sessionId: '01900000-0000-7000-8000-000000000002'
		});
		mock.setSessionState(next);
		const authRefresh = page.waitForResponse(
			(response) => new URL(response.url()).pathname === '/api/auth/me'
		);
		await announce({ type: 'observed', sessionId: next.sessionId, serverNow: next.serverNow });
		await (await authRefresh).finished();
		await expect.poll(() => [...relay.peers.keys()][0]).not.toBe(id);
		await waitPeers(relay, 1);
		await expect(save).toBeDisabled();
		expect(mock.updateRequestCount()).toBe(0);
		await expect(dialog.getByRole('textbox', { name: 'หมายเหตุ', exact: true })).toHaveValue(
			'ค่าที่ยังไม่บันทึก'
		);
		await page.keyboard.press('Escape');
		await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
		await expect(save).toBeEnabled();
		await dialog
			.getByRole('textbox', { name: 'หมายเหตุ', exact: true })
			.fill('บันทึกด้วยเซสชันใหม่');
		const request = page.waitForRequest(
			(request) =>
				request.method() === 'PUT' && new URL(request.url()).pathname.endsWith(timetableIds.blockA)
		);
		await save.click();
		const headers = (await request).headers();
		expect(headers['x-timetable-connection-id']).toBe([...relay.peers.keys()][0]);
		expect(headers['x-timetable-connection-id']).not.toBe(id);
		await expect.poll(mock.updateRequestCount).toBe(1);
		expect(mock.workspaceRequestCount()).toBe(1);
	} finally {
		await context.close();
		await server.close();
	}
});

test('an editor portal preserves the second synchronized overview row and avatar jump highlights its exact day and block', async ({
	browser
}) => {
	const relay = createDraftRelay(),
		first = await browser.newContext({ viewport: { width: 1440, height: 1000 } }),
		second = await browser.newContext({ viewport: { width: 1280, height: 900 } }),
		a = await first.newPage(),
		b = await second.newPage();
	const block = makeSynchronizedTimetableBlock(timetableIds.blockA, timetableIds.period1);
	block.dayOfWeek = 'TUE';
	const roomId = '81000000-0000-4000-8000-000000000211';
	block.homerooms.push({
		...block.homerooms[0],
		id: 'f1000000-0000-4000-8000-000000000211',
		homeroomId: roomId,
		code: 'M1-2',
		name: 'ม.1/2'
	});
	const mock = await installTimetableMock(a, { socketMode: 'native', relay, blocks: [block] });
	const workspace = mock.workspace();
	workspace.homerooms.push({
		...workspace.homerooms[0],
		id: roomId,
		code: 'M1-2',
		name: 'ม.1/2',
		roomNumber: '2'
	});
	mock.publishWorkspace(workspace, false);
	await installTimetableMock(b, {
		socketMode: 'native',
		relay,
		blocks: [block],
		homerooms: workspace.homerooms,
		userId: secondUser
	});
	const server = await startNativeDraftRelay(relay, mock.workspace);
	try {
		await redirectSocket(first, server.url, timetableIds.user, 'ผู้แก้ไขภาพรวม');
		await redirectSocket(second, server.url, secondUser, 'ผู้ติดตาม');
		await a.goto(url('wholeSchool'));
		await b.goto(url());
		await waitPeers(relay, 2);
		const id = [...relay.peers.values()].find((peer) => peer.user.userId === timetableIds.user)!.id;
		await a.getByRole('combobox', { name: 'เลือกวันดูภาพรวม', exact: true }).click();
		await a.getByRole('option', { name: 'วันอังคาร', exact: true }).click();
		await a.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const selector = `[data-timetable-anchor="block"][data-block-id="${block.id}"]`;
		// Keep the pointer over the first occurrence, then activate the second using the keyboard.
		await hoverPoint(a, `${selector}[data-row-id="${timetableIds.homeroom}"]`);
		await a
			.locator(`${selector}[data-row-id="${roomId}"]`)
			.getByRole('button', { name: /ดูรายละเอียด ชุมนุม/ })
			.focus();
		await a.keyboard.press('Enter');
		const dialog = a.getByRole('dialog');
		await expect(dialog.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
		await dialog.getByRole('combobox', { name: 'เลือกห้องเรียน' }).click();
		await a.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true }).click();
		await expect(a.getByRole('listbox')).toHaveCount(0);
		await dialog.getByRole('textbox', { name: 'หมายเหตุ', exact: true }).hover();
		await expect.poll(() => relay.peers.get(id)?.user.connections[0].anchor).toBeNull();
		await b.getByRole('button', { name: /ไปยัง ผู้แก้ไขภาพรวม/ }).click();
		const exact = b.locator(`${selector}[data-row-id="${roomId}"]`);
		await expect(exact).toHaveAttribute('data-timetable-jump-target', 'true');
		await expect(
			b.locator(`${selector}[data-row-id="${timetableIds.homeroom}"]`)
		).not.toHaveAttribute('data-timetable-jump-target', 'true');
		await expect(b.getByRole('combobox', { name: 'เลือกวันดูภาพรวม', exact: true })).toHaveText(
			'อังคาร'
		);
		await expect(b.getByRole('dialog')).toHaveCount(0);
		expect(relay.peers.get(id)?.user.connections[0].context).toMatchObject({
			viewMode: 'wholeSchool',
			viewId: roomId,
			dayOfWeek: 'TUE'
		});
		expect(mock.updateRequestCount()).toBe(0);
	} finally {
		await first.close();
		await second.close();
		await server.close();
	}
});

test('native policy1008 refreshes unchanged auth identity, rejoins changed scope and accepts new leases and authorized partials', async ({
	browser
}) => {
	const relay = createDraftRelay(),
		context = await browser.newContext({ viewport: { width: 1440, height: 1000 } }),
		page = await context.newPage();
	const blockA = makeTimetableBlock(timetableIds.blockA, timetableIds.period1),
		blockB = makeTimetableBlock(timetableIds.blockB, timetableIds.period2, {
			groupId: timetableIds.groupB,
			offeringId: timetableIds.offeringB,
			instructorIds: [timetableIds.teacherB]
		});
	const mock = await installTimetableMock(page, {
		socketMode: 'native',
		relay,
		blocks: [blockA, blockB],
		userId: timetableIds.teacherA
	});
	const server = await startNativeDraftRelay(relay, mock.workspace);
	try {
		await redirectSocket(context, server.url, timetableIds.teacherA, 'ผู้ปรับขอบเขต');
		await page.goto(url());
		await waitPeers(relay, 1);
		const id = [...relay.peers.keys()][0],
			session = mock.getSessionState();
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		await page
			.locator(`[data-block-id="${blockA.id}"]`)
			.getByRole('button', { name: /ดูรายละเอียด/ })
			.click();
		const save = page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true });
		await expect(save).toBeEnabled();
		const oldLease = relay.peers.get(id)!.user.connections[0].activeReservationId;
		const scoped = structuredClone(mock.workspace());
		scoped.blocks = [blockA];
		mock.publishWorkspace(scoped, false);
		mock.setPermissions(['academic_timetable.read.assigned', 'academic_timetable.manage.assigned']);
		const refresh = page.waitForResponse(
			(response) => new URL(response.url()).pathname === '/api/auth/me'
		);
		server.policyClose(id);
		const auth = await refresh;
		expect((await auth.json()).data.session.sessionId).toBe(session.sessionId);
		await auth.finished();
		await expect.poll(() => [...relay.peers.keys()][0]).not.toBe(id);
		await waitPeers(relay, 1);
		const next = [...relay.peers.keys()][0];
		await expect(page.locator(`[data-block-id="${blockB.id}"]`)).toHaveCount(0);
		await expect(save).toBeDisabled();
		await page.keyboard.press('Escape');
		await page
			.locator(`[data-block-id="${blockA.id}"]`)
			.getByRole('button', { name: /ดูรายละเอียด/ })
			.click();
		await expect(save).toBeEnabled();
		expect(relay.peers.get(next)!.user.connections[0].activeReservationId).not.toBe(oldLease);
		await page.keyboard.press('Escape');
		const changed = structuredClone(scoped);
		changed.blocks = [{ ...blockA, rowVersion: 2, note: 'ข้อมูลใหม่ตามขอบเขตที่ยืนยันแล้ว' }];
		changed.version.rowVersion++;
		mock.publishWorkspace(changed);
		await page
			.locator(`[data-block-id="${blockA.id}"]`)
			.getByRole('button', { name: /ดูรายละเอียด/ })
			.click();
		await expect(
			page.getByRole('dialog').getByRole('textbox', { name: 'หมายเหตุ', exact: true })
		).toHaveValue('ข้อมูลใหม่ตามขอบเขตที่ยืนยันแล้ว');
		expect(mock.workspaceRequestCount()).toBe(1);
		expect(mock.updateRequestCount()).toBe(0);
		expect(mock.previewRequestCount()).toBe(0);
	} finally {
		await context.close();
		await server.close();
	}
});

test('native policy1008 with refreshed read revocation lets the route stop collaboration and keeps the board closed', async ({
	browser
}) => {
	const relay = createDraftRelay(),
		context = await browser.newContext(),
		page = await context.newPage();
	const mock = await installTimetableMock(page, { socketMode: 'native', relay });
	const server = await startNativeDraftRelay(relay, mock.workspace);
	try {
		await redirectSocket(context, server.url, timetableIds.user, 'ผู้ถูกถอนสิทธิ์');
		await page.goto(url());
		await waitPeers(relay, 1);
		const id = [...relay.peers.keys()][0];
		mock.setPermissions([]);
		const refresh = page.waitForResponse(
			(response) => new URL(response.url()).pathname === '/api/auth/me'
		);
		server.policyClose(id);
		await (await refresh).finished();
		await expect(page.getByTestId('timetable-board-ready')).toHaveCount(0);
		await waitPeers(relay, 0);
		await page.evaluate(
			() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
		);
		expect(relay.peers.size).toBe(0);
		expect(mock.workspaceRequestCount()).toBe(1);
	} finally {
		await context.close();
		await server.close();
	}
});
