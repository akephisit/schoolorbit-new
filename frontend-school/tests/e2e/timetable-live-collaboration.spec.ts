import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import {
	createDraftRelay,
	startNativeDraftRelay,
	type DraftRelay
} from './timetable-collaboration-relay.ts';
import {
	installTimetableMock,
	timetableIds,
	makeSynchronizedTimetableBlock
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
		const card = a.locator('aside article[draggable="true"]').first();
		const cardBox = await card.boundingBox();
		const target = await a.locator(cell(timetableIds.period1)).boundingBox();
		if (!cardBox || !target) throw new Error('missing drag surface');
		await a.mouse.move(cardBox.x + cardBox.width * 0.5, cardBox.y + cardBox.height * 0.5);
		await a.mouse.down();
		await a.mouse.move(target.x + target.width * 0.5, target.y + target.height * 0.5, {
			steps: 18
		});
		await expect(b.locator('[data-timetable-remote-ghost]')).toBeVisible();
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
		await a.getByRole('button', { name: /ดูรายละเอียด CLUB/ }).click();
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
		await expect(b.getByRole('button', { name: 'ห้องเรียน', exact: true })).toHaveAttribute(
			'data-state',
			'active'
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
