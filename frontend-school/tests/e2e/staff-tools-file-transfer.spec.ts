import { expect, test, type Page } from '@playwright/test';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, mkdtemp, mkdir, writeFile, open as openFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import jsQR from 'jsqr';
import type { TransferEvent, TransferPair } from '../../src/lib/api/file-transfer.js';
import { fileTransferFixture } from './fixtures/file-transfer';

const path = '/staff/tools/file-transfer';
type InputFile = { name: string; mimeType: string; buffer: Buffer };
test.use({ serviceWorkers: 'block' });
const incoming = (page: Page) =>
	page.locator('[data-testid="transfer-job"][data-direction="incoming"]');
const outgoing = (page: Page, label: string) =>
	page
		.locator('[data-testid="transfer-job"][data-direction="outgoing"]')
		.filter({ hasText: label });
async function open(page: Page, label: string) {
	await page.goto(path);
	await page.getByLabel('ชื่ออุปกรณ์', { exact: true }).fill(label);
	await page.getByRole('button', { name: 'เปิดรับส่งไฟล์', exact: true }).click();
	await expect(page.getByTestId('pairing-code')).toBeVisible();
}
async function choose(page: Page, files: InputFile[]) {
	await page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true }).setInputFiles(files);
}
async function request(page: Page, labels: string[]) {
	for (const label of labels) {
		const box = page.getByRole('checkbox', { name: new RegExp(label) });
		if (!(await box.isChecked())) await box.check();
	}
	await page.getByRole('button', { name: 'ส่งชุดไฟล์ให้ผู้รับที่เลือก', exact: true }).click();
}
async function viewManifest(page: Page) {
	await page.getByRole('button', { name: 'ดูรายการไฟล์', exact: true }).click();
}
async function receiveFiles(page: Page) {
	await page.getByRole('radio', { name: 'ดาวน์โหลดผ่านเบราว์เซอร์อัตโนมัติ', exact: true }).check();
	await page.getByRole('button', { name: 'รับไฟล์ทั้งชุด', exact: true }).click();
}
async function accept(page: Page) {
	await viewManifest(page);
	await receiveFiles(page);
}
async function download(page: Page, index: number, name: string): Promise<Buffer> {
	await expect(page.getByTestId('save-status')).toHaveAttribute('data-pending', 'false');
	const pending = page.waitForEvent('download');
	await page.getByRole('button', { name: `ดาวน์โหลด ${index + 1} ${name}`, exact: true }).click();
	const saved = await (await pending).path();
	if (!saved) throw new Error('Download missing');
	return readFile(saved);
}
const input = (name: string, content: string): InputFile => ({
	name,
	mimeType: 'text/plain',
	buffer: Buffer.from(content)
});
async function observeEvents(page: Page) {
	const cdp = await page.context().newCDPSession(page),
		events: TransferEvent[] = [];
	await cdp.send('Network.enable');
	cdp.on('Network.eventSourceMessageReceived', (message) => {
		if (message.eventName === 'file_transfer') {
			const event: TransferEvent = JSON.parse(message.data);
			events.push(event);
		}
	});
	return { events, close: () => cdp.detach() };
}
async function holdFileAcknowledgments(page: Page) {
	await page.addInitScript(() => {
		const NativePeer = RTCPeerConnection;
		window.RTCPeerConnection = class extends NativePeer {
			constructor(configuration?: RTCConfiguration) {
				super(configuration);
				this.addEventListener('datachannel', ({ channel }) => {
					const send = channel.send,
						held: string[] = [];
					Object.defineProperty(channel, 'send', {
						value(data: unknown) {
							if (typeof data === 'string' && JSON.parse(data).type === 'file_received') {
								held.push(data);
								return;
							}
							return Reflect.apply(send, channel, [data]);
						}
					});
					window.addEventListener('release-file-acks', () => {
						for (const data of held.splice(0))
							if (channel.readyState === 'open') Reflect.apply(send, channel, [data]);
					});
				});
			}
		};
	});
}

test('manifest review and explicit consent transfer zero-byte files, duplicate names and a real32MiB file with bounded read calls', async ({
	context,
	page
}, testInfo) => {
	test.setTimeout(90_000);
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await page.addInitScript(() => {
			const reads: number[] = [],
				frames: number[] = [],
				slices = new WeakSet<Blob>();
			Object.defineProperty(window, 'fileTransferReadCalls', { value: reads });
			Object.defineProperty(window, 'fileTransferFrames', { value: frames });
			const slice = Blob.prototype.slice,
				read = Blob.prototype.arrayBuffer,
				create = RTCPeerConnection.prototype.createDataChannel;
			Blob.prototype.slice = function (start, end, type) {
				const result = slice.call(this, start, end, type);
				if (this instanceof File) slices.add(result);
				return result;
			};
			Blob.prototype.arrayBuffer = function () {
				if (slices.has(this)) reads.push(this.size);
				return read.call(this);
			};
			RTCPeerConnection.prototype.createDataChannel = function (label, options) {
				const channel = create.call(this, label, options),
					send = channel.send;
				Object.defineProperty(channel, 'send', {
					value(data: unknown) {
						if (data instanceof ArrayBuffer || ArrayBuffer.isView(data))
							frames.push(data.byteLength);
						return Reflect.apply(send, channel, [data]);
					}
				});
				return channel;
			};
		});
		await api.setup(page);
		await api.setup(receiver);
		await page.setViewportSize({ width: 1440, height: 1000 });
		await receiver.setViewportSize({ width: 390, height: 1000 });
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		api.failSenderClose();
		const large = Buffer.alloc(32 * 1024 * 1024);
		for (let index = 0; index < large.length; index++) large[index] = index % 251;
		const files = [
			input('empty.txt', ''),
			input('ซ้ำ.txt', 'first'),
			input('ซ้ำ.txt', 'second'),
			{ name: 'large.bin', mimeType: 'application/octet-stream', buffer: large }
		];
		await choose(page, files);
		await request(page, ['ผู้รับ']);
		await expect(incoming(receiver).getByText('4 ไฟล์ · 32.0 MiB')).toBeVisible();
		await expect(incoming(receiver).getByText('1. empty.txt · 0 ไบต์')).toHaveCount(0);
		await incoming(receiver).screenshot({ path: testInfo.outputPath('batch-consent-mobile.png') });
		expect(api.requests.filter((entry) => entry.path.endsWith('/signals'))).toHaveLength(0);
		await viewManifest(receiver);
		await expect(incoming(receiver).getByText('1. empty.txt · 0 ไบต์')).toBeVisible();
		expect(await page.evaluate(() => Reflect.get(window, 'fileTransferFrames'))).toEqual([]);
		await receiveFiles(receiver);
		await expect(outgoing(page, 'ผู้รับ')).toHaveAttribute('data-phase', 'complete', {
			timeout: 60_000
		});
		for (const [index, file] of files.entries())
			expect(
				createHash('sha256')
					.update(await download(receiver, index, file.name))
					.digest('hex')
			).toBe(createHash('sha256').update(file.buffer).digest('hex'));
		const reads = await page.evaluate(
				() => Reflect.get(window, 'fileTransferReadCalls') as number[]
			),
			frames = await page.evaluate(() => Reflect.get(window, 'fileTransferFrames') as number[]);
		expect(reads).toHaveLength(34);
		expect(reads.filter((size) => size === 1024 * 1024)).toHaveLength(32);
		expect(Math.max(...frames)).toBeLessThanOrEqual(16 * 1024);
		await expect.poll(() => api.pairs.length).toBe(0);
		expect(api.devices[0].userId).toBe(api.devices[1].userId);
		expect(api.devices[0].id).not.toBe(api.devices[1].id);
		for (const entry of api.requests) {
			if (entry.path.endsWith('/devices') && entry.method === 'POST')
				expect(JSON.parse(entry.body).protocolVersion).toBe(3);
			if (entry.path.endsWith('/pairs')) {
				const body = JSON.parse(entry.body);
				expect(body.batch).toEqual({ fileCount: 4, totalBytes: large.length + 11 });
				expect('files' in body).toBe(false);
				expect(typeof body.clientRequestId).toBe('string');
				expect('file' in body).toBe(false);
			}
			expect(entry.body.length).toBeLessThan(40_000);
		}
		await receiver
			.getByRole('region', { name: 'ไฟล์ที่รับครบแล้ว', exact: true })
			.screenshot({ path: testInfo.outputPath('batch-downloads-mobile.png') });
		await page.screenshot({
			path: testInfo.outputPath('batch-complete-desktop.png'),
			fullPage: true
		});
	} finally {
		await receiver.close();
		await api.close();
	}
});

test('six recipients run independently, a seventh waits outside the backend, and cancellation retains complete files', async ({
	browser,
	page
}, testInfo) => {
	test.setTimeout(90_000);
	const api = await fileTransferFixture(),
		contexts = await Promise.all(
			Array.from({ length: 7 }, () => browser.newContext({ serviceWorkers: 'block' }))
		);
	const recipients = await Promise.all(contexts.map((context) => context.newPage()));
	try {
		await page.setViewportSize({ width: 1440, height: 1000 });
		await page.addInitScript(() =>
			localStorage.setItem('ui-preferences', JSON.stringify({ theme: 'dark' }))
		);
		await api.setup(page);
		await open(page, 'ผู้ส่งหลายคน');
		await expect(page.locator('html')).toHaveClass(/dark/);
		for (const [index, recipient] of recipients.entries()) {
			if (index === 0) await holdFileAcknowledgments(recipient);
			await api.setup(recipient, 'staff', { userId: randomUUID(), sessionId: randomUUID() });
			await open(recipient, `ผู้รับ ${index + 1}`);
		}
		await choose(page, [input('first.txt', 'completed first'), input('second.txt', 'later file')]);
		await request(
			page,
			Array.from({ length: 7 }, (_, index) => `ผู้รับ ${index + 1}`)
		);
		await expect(outgoing(page, 'ผู้รับ 7')).toHaveAttribute('data-phase', 'queued');
		await page
			.locator('section[aria-labelledby="transfers-heading"]')
			.screenshot({ path: testInfo.outputPath('fanout-queue-dark.png') });
		expect(api.requests.filter((entry) => entry.path.endsWith('/pairs'))).toHaveLength(6);
		expect(api.maxOutgoing).toBe(6);
		await accept(recipients[0]);
		await expect(
			recipients[0].getByRole('button', { name: 'ดาวน์โหลด 1 first.txt', exact: true })
		).toBeVisible();
		await expect(
			recipients[0].getByRole('button', { name: 'ล้างไฟล์ที่รับแล้วทั้งหมด', exact: true })
		).toBeDisabled();
		await expect(
			recipients[0].getByRole('button', { name: 'ดาวน์โหลดทั้งหมดเป็น ZIP', exact: true })
		).toBeDisabled();
		await recipients[1].getByRole('button', { name: 'ปฏิเสธ', exact: true }).click();
		await expect(
			recipients[6].getByRole('button', { name: 'ดูรายการไฟล์', exact: true })
		).toBeVisible();
		await accept(recipients[2]);
		await accept(recipients[3]);
		await accept(recipients[4]);
		await accept(recipients[5]);
		await accept(recipients[6]);
		await expect(outgoing(page, 'ผู้รับ 3')).toHaveAttribute('data-phase', 'complete');
		await expect(outgoing(page, 'ผู้รับ 4')).toHaveAttribute('data-phase', 'complete');
		await expect(outgoing(page, 'ผู้รับ 1')).toHaveAttribute('data-phase', 'acknowledging');
		await expect(page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true })).toBeDisabled();
		expect(api.maxOutgoing).toBeLessThanOrEqual(6);
		expect(await download(recipients[2], 1, 'second.txt')).toEqual(Buffer.from('later file'));
		expect(await download(recipients[3], 0, 'first.txt')).toEqual(Buffer.from('completed first'));
		await incoming(recipients[0])
			.getByRole('button', { name: /ยกเลิก/ })
			.click();
		expect(await download(recipients[0], 0, 'first.txt')).toEqual(Buffer.from('completed first'));
		await recipients[0].evaluate(() => window.dispatchEvent(new Event('release-file-acks')));
		await expect(
			recipients[0].getByRole('button', { name: 'ดาวน์โหลด 2 second.txt', exact: true })
		).toHaveCount(0);
		await expect.poll(() => api.pairs.length).toBe(0);
		await expect(page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true })).toBeEnabled();
	} finally {
		await Promise.all(contexts.map((context) => context.close()));
		await api.close();
	}
});

test('malformed second-file frames preserve the first file and new incoming batches require an explicit clear', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await page.addInitScript(() => {
			const create = RTCPeerConnection.prototype.createDataChannel;
			RTCPeerConnection.prototype.createDataChannel = function (label, options) {
				const channel = create.call(this, label, options),
					send = channel.send;
				let index = 0;
				Object.defineProperty(channel, 'send', {
					value(data: unknown) {
						if (typeof data === 'string') {
							const control = JSON.parse(data);
							if (control.type === 'file_start') index = control.index;
						} else if (index === 1)
							return Reflect.apply(send, channel, [new Uint8Array(16 * 1024 + 1)]);
						return Reflect.apply(send, channel, [data]);
					}
				});
				return channel;
			};
		});
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('good.txt', 'retained'), input('bad.txt', 'bad')]);
		await request(page, ['ผู้รับ']);
		await accept(receiver);
		await expect(incoming(receiver).getByText(/ข้อมูลชุดไฟล์ไม่ครบหรือไม่ถูกต้อง/)).toBeVisible();
		expect(await download(receiver, 0, 'good.txt')).toEqual(Buffer.from('retained'));
		await expect(
			receiver.getByRole('button', { name: 'ดาวน์โหลด 2 bad.txt', exact: true })
		).toHaveCount(0);
		await expect(page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true })).toBeEnabled();
		await choose(page, [input('new.txt', 'new batch')]);
		await request(page, ['ผู้รับ']);
		await expect(
			receiver.getByRole('button', { name: 'ดูรายการไฟล์', exact: true })
		).toBeDisabled();
		await receiver.getByRole('button', { name: 'ล้างไฟล์เดิมก่อนรับชุดใหม่', exact: true }).click();
		await accept(receiver);
		expect(await download(receiver, 0, 'new.txt')).toEqual(Buffer.from('new batch'));
		await expect(
			receiver.getByRole('button', { name: 'ดาวน์โหลด 1 good.txt', exact: true })
		).toHaveCount(0);
	} finally {
		await receiver.close();
		await api.close();
	}
});

test('incoming cancellation history stays bounded while completed downloads retain their owner and bytes', async ({
	context,
	page
}) => {
	test.setTimeout(90_000);
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('retained.txt', 'completed bytes survive history churn')]);
		await request(page, ['ผู้รับ']);
		await expect(incoming(receiver)).toHaveAttribute('data-phase', 'consent');
		const original = structuredClone(api.pairs[0]);
		await accept(receiver);
		await expect(outgoing(page, 'ผู้รับ')).toHaveAttribute('data-phase', 'complete');
		await expect.poll(() => api.pairs.length).toBe(0);
		const signalCount = api.requests.filter((entry) => entry.path.endsWith('/signals')).length;
		let first = original;
		for (let cycle = 0; cycle < 80; cycle++) {
			// Native SSE delivers valid, sequential requests withdrawn by the remote sender.
			// No consent or transport is needed for this metadata-only churn.
			const pair: TransferPair = {
				...original,
				id: randomUUID(),
				clientRequestId: randomUUID(),
				batch: { fileCount: 20, totalBytes: 20 },
				status: 'pending',
				expiresAt: new Date(Date.now() + 120_000).toISOString()
			};
			if (cycle === 0) first = pair;
			api.emit(original.recipient.id, { type: 'pair_changed', data: pair });
			await expect(
				receiver
					.locator('[data-testid="transfer-job"][data-phase="consent"]')
					.getByText('20 ไฟล์ · 20 ไบต์', { exact: true })
			).toBeVisible();
			expect(await incoming(receiver).count()).toBeLessThanOrEqual(64);
			api.emit(original.recipient.id, {
				type: 'pair_closed',
				data: { pairId: pair.id, reason: 'cancelled' }
			});
			await expect(receiver.getByRole('button', { name: 'ดูรายการไฟล์', exact: true })).toHaveCount(
				0
			);
		}
		await expect(incoming(receiver)).toHaveCount(64);
		await expect(
			receiver.locator(
				'[data-testid="transfer-job"][data-direction="incoming"][data-phase="complete"]'
			)
		).toHaveCount(1);
		expect(api.requests.filter((entry) => entry.path.endsWith('/signals'))).toHaveLength(
			signalCount
		);
		expect(api.requests.filter((entry) => entry.path.endsWith('/accept'))).toHaveLength(1);
		const observed = await observeEvents(receiver);
		try {
			api.emit(original.recipient.id, { type: 'pair_changed', data: first });
			await expect.poll(() => observed.events.length).toBe(1);
			await expect(incoming(receiver)).toHaveCount(64);
			await expect(receiver.getByRole('button', { name: 'ดูรายการไฟล์', exact: true })).toHaveCount(
				0
			);
		} finally {
			await observed.close();
		}
		// Starting an outgoing batch must not prune an incoming download's owner either.
		await choose(receiver, [input('return.txt', 'not accepted')]);
		await request(receiver, ['ผู้ส่ง']);
		await expect(incoming(page)).toHaveAttribute('data-phase', 'consent');
		await outgoing(receiver, 'ผู้ส่ง')
			.getByRole('button', { name: 'ยกเลิกผู้รับนี้', exact: true })
			.click();
		await expect.poll(() => api.pairs.length).toBe(0);
		expect(await download(receiver, 0, 'retained.txt')).toEqual(
			Buffer.from('completed bytes survive history churn')
		);
		await receiver.getByRole('button', { name: 'ปิดรับส่งไฟล์ทั้งหมด', exact: true }).click();
		await receiver.getByLabel('ชื่ออุปกรณ์', { exact: true }).fill('ผู้รับใหม่');
		await receiver.getByRole('button', { name: 'เปิดรับส่งไฟล์', exact: true }).click();
		await expect(receiver.getByTestId('pairing-code')).toBeVisible();
		await expect(incoming(receiver)).toHaveCount(1);
		expect(await download(receiver, 0, 'retained.txt')).toEqual(
			Buffer.from('completed bytes survive history churn')
		);
		await receiver.getByRole('button', { name: 'ล้างไฟล์ที่รับแล้วทั้งหมด', exact: true }).click();
		await expect(incoming(receiver)).toHaveCount(0);
		await expect(
			receiver.getByRole('button', { name: 'ดาวน์โหลด 1 retained.txt', exact: true })
		).toHaveCount(0);
	} finally {
		await receiver.close();
		await api.close();
	}
});

test('QR/code pairing handles zero-byte batches and late create/accept responses cannot revoke results', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		api.holdAccept();
		api.holdRequest();
		await open(receiver, 'ผู้รับรหัส');
		const code = await receiver.getByTestId('pairing-code').innerText();
		await receiver.getByRole('button', { name: 'แสดง QR จับคู่' }).click();
		const pixels = await receiver
			.getByAltText('QR สำหรับจับคู่อุปกรณ์นี้')
			.evaluate(async (element) => {
				const image = element as HTMLImageElement;
				await image.decode();
				const canvas = document.createElement('canvas');
				canvas.width = canvas.height = 400;
				const ctx = canvas.getContext('2d');
				if (!ctx) throw new Error('Canvas unavailable');
				ctx.drawImage(image, 0, 0, 400, 400);
				const data = ctx.getImageData(0, 0, 400, 400).data;
				let binary = '';
				for (let offset = 0; offset < data.length; offset += 8192)
					binary += String.fromCharCode(...data.subarray(offset, offset + 8192));
				return btoa(binary);
			});
		const decoded = jsQR(new Uint8ClampedArray(Buffer.from(pixels, 'base64')), 400, 400)?.data;
		expect(decoded).toBe(`${new URL(receiver.url()).origin}${path}#code=${code}`);
		if (!decoded) throw new Error('QR cannot be decoded');
		await page.goto(decoded);
		await expect(page.getByLabel('หรือใช้รหัสอุปกรณ์ผู้รับ')).toHaveValue(code);
		await expect(page).toHaveURL(new RegExp(`${path}$`));
		await page.getByRole('button', { name: 'เปิดรับส่งไฟล์', exact: true }).click();
		await expect(page.getByTestId('pairing-code')).toBeVisible();
		await choose(page, [input('zero.txt', ''), input('zero.txt', '')]);
		await page.getByRole('button', { name: 'เพิ่มผู้รับด้วยรหัส' }).click();
		await accept(receiver);
		await expect(outgoing(page, 'ผู้รับรหัส')).toHaveAttribute('data-phase', 'complete');
		expect(await download(receiver, 1, 'zero.txt')).toHaveLength(0);
		api.releaseAccept();
		api.releaseRequest();
		await receiver.clock.install();
		await receiver.clock.fastForward(21_000);
		await expect(incoming(receiver)).toHaveAttribute('data-phase', 'complete');
		await expect(
			receiver.getByRole('button', { name: 'ดาวน์โหลด 1 zero.txt', exact: true })
		).toBeVisible();
	} finally {
		api.releaseAccept();
		api.releaseRequest();
		await receiver.close();
		await api.close();
	}
});

for (const ordering of ['accepted-event-first', 'failed-response-first'] as const)
	test(`whole-batch consent survives uncertain accept with ${ordering}`, async ({
		context,
		page
	}) => {
		const api = await fileTransferFixture(),
			receiver = await context.newPage(),
			observed = await observeEvents(receiver);
		try {
			await api.setup(page);
			await api.setup(receiver);
			api.failAcceptResponse();
			api.holdSignalDispatch();
			if (ordering === 'accepted-event-first') api.holdAccept();
			else api.holdAcceptedEvents();
			await open(page, 'ผู้ส่ง');
			await open(receiver, 'ผู้รับ');
			await choose(page, [input('one.txt', 'one'), input('two.txt', 'two')]);
			await request(page, ['ผู้รับ']);
			const pair = structuredClone(api.pairs[0]),
				acceptedSeen = () =>
					observed.events.some(
						(event) =>
							event.type === 'pair_changed' &&
							event.data.id === pair.id &&
							event.data.status === 'accepted'
					);
			const failed = receiver.waitForResponse(
					(response) =>
						response.url().endsWith(`/pairs/${pair.id}/accept`) && response.status() === 503
				),
				reconciled = receiver.waitForResponse(
					(response) =>
						new URL(response.url()).pathname.endsWith(`/devices/${pair.recipient.id}`) &&
						response.request().method() === 'GET'
				);
			await viewManifest(receiver);
			if (ordering === 'accepted-event-first') {
				await expect.poll(acceptedSeen).toBe(true);
				api.releaseAccept();
			}
			await failed;
			await reconciled;
			await expect(receiver.getByRole('button', { name: 'ดูรายการไฟล์', exact: true })).toHaveCount(
				0
			);
			if (ordering === 'failed-response-first') {
				expect(acceptedSeen()).toBe(false);
				api.releaseAcceptedEvents();
				await expect.poll(acceptedSeen).toBe(true);
			}
			await expect.poll(() => api.heldSignals).toBe(1);
			api.releaseSignalDispatch();
			await receiveFiles(receiver);
			await expect(outgoing(page, 'ผู้รับ')).toHaveAttribute('data-phase', 'complete');
			expect(await download(receiver, 1, 'two.txt')).toEqual(Buffer.from('two'));
			expect(
				api.requests.filter((entry) => entry.path.endsWith(`/pairs/${pair.id}/accept`))
			).toHaveLength(1);
		} finally {
			api.releaseAccept();
			api.releaseAcceptedEvents();
			api.releaseSignalDispatch();
			await observed.close();
			await receiver.close();
			await api.close();
		}
	});

test('cancellation fences late accepted events and native offers, and retry creates a fresh correlated request', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage(),
		observed = await observeEvents(receiver);
	try {
		await api.setup(page);
		await api.setup(receiver);
		api.failAcceptResponse();
		api.holdSignalDispatch();
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('cancel.txt', 'cancel')]);
		await request(page, ['ผู้รับ']);
		const pair = structuredClone(api.pairs[0]);
		await viewManifest(receiver);
		await expect.poll(() => api.heldSignals).toBe(1);
		const accepted = structuredClone(api.pairs[0]);
		await incoming(receiver)
			.getByRole('button', { name: /ยกเลิก/ })
			.click();
		await expect.poll(() => api.pairs.length).toBe(0);
		api.emit(pair.recipient.id, { type: 'pair_changed', data: accepted });
		api.releaseSignalDispatch();
		await expect
			.poll(() =>
				observed.events.some((event) => event.type === 'signal' && event.data.pairId === pair.id)
			)
			.toBe(true);
		await receiver.evaluate(
			() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
		);
		await expect(incoming(receiver)).toHaveAttribute('data-phase', 'cancelled');
		await expect(receiver.getByRole('button', { name: 'ดูรายการไฟล์', exact: true })).toHaveCount(
			0
		);
		await outgoing(page, 'ผู้รับ').getByRole('button', { name: 'ส่งคำขอใหม่' }).click();
		await expect(receiver.getByRole('button', { name: 'ดูรายการไฟล์', exact: true })).toBeVisible();
		expect(api.pairs[0].clientRequestId).not.toBe(pair.clientRequestId);
		await accept(receiver);
		expect(await download(receiver, 0, 'cancel.txt')).toEqual(Buffer.from('cancel'));
	} finally {
		api.releaseSignalDispatch();
		await observed.close();
		await receiver.close();
		await api.close();
	}
});

test('session invalidation disposes every transfer and all completed download URLs', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await holdFileAcknowledgments(receiver);
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('complete.txt', 'complete'), input('unfinished.txt', 'later')]);
		await request(page, ['ผู้รับ']);
		await accept(receiver);
		await expect(
			receiver.getByRole('button', { name: 'ดาวน์โหลด 1 complete.txt', exact: true })
		).toBeVisible();
		const device = api.devices.find((item) => item.label === 'ผู้รับ');
		if (!device) throw new Error('Missing device');
		api.emit(device.id, { type: 'session_invalid', data: {} });
		await expect(receiver.getByTestId('pairing-code')).toHaveCount(0);
		await expect(receiver.getByRole('region', { name: 'ไฟล์ที่รับครบแล้ว' })).toHaveCount(0);
		await expect(incoming(receiver)).toHaveCount(0);
		await receiver.evaluate(() => window.dispatchEvent(new Event('release-file-acks')));
		await expect.poll(() => api.pairs.length).toBe(0);
	} finally {
		await receiver.close();
		await api.close();
	}
});

test('count validation, protocol refresh errors and unavailable LAN have truthful recovery', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(
			page,
			Array.from({ length: 21 }, (_, index) => input(`${index}.txt`, ''))
		);
		await expect(page.getByTestId('source-summary')).toHaveText('เลือก 21 ไฟล์ · 0 ไบต์');
		await choose(page, [input('blocked.txt', 'blocked')]);
		api.blockSignals();
		await request(page, ['ผู้รับ']);
		await receiver.clock.install();
		await viewManifest(receiver);
		await receiver.clock.fastForward(20_001);
		await expect(
			incoming(receiver).getByText(
				/QR และรหัสช่วยจับคู่เท่านั้น ไม่สามารถแก้การปิดกั้นเครือข่ายได้/
			)
		).toBeVisible();
	} finally {
		await receiver.close();
		await api.close();
	}
});

test('unsupported registration version shows refresh-both-devices guidance', async ({ page }) => {
	const api = await fileTransferFixture();
	try {
		await api.setup(page);
		await page.route('**/api/staff/file-transfer/devices', (route) =>
			route.fallback({ postData: JSON.stringify({ label: 'old client', protocolVersion: 1 }) })
		);
		await page.goto(path);
		await page.getByRole('button', { name: 'เปิดรับส่งไฟล์', exact: true }).click();
		await expect(page.getByText(/กรุณารีเฟรชหน้าเว็บทั้งอุปกรณ์ส่งและอุปกรณ์รับ/)).toBeVisible();
		expect(api.devices).toHaveLength(0);
	} finally {
		await api.close();
	}
});

for (const userType of ['student', 'parent'])
	test(`${userType} is denied the batch tool`, async ({ page }) => {
		const api = await fileTransferFixture();
		try {
			await api.setup(page, userType);
			await page.goto(path);
			await expect(page).toHaveURL(/\/403\?from=/);
			expect(api.devices).toHaveLength(0);
		} finally {
			await api.close();
		}
	});

test('pairing fragment survives login redirect and route disposal removes the device', async ({
	page
}) => {
	const api = await fileTransferFixture();
	let signedIn = false;
	try {
		await api.setup(page);
		await page.route('**/api/auth/me', (route) =>
			signedIn
				? route.fallback()
				: route.fulfill({ status: 401, json: { success: false, error: 'unauthorized' } })
		);
		await page.goto(`${path}#code=012345ABCD`);
		await expect(page).toHaveURL(/\/login$/);
		expect(await page.evaluate(() => sessionStorage.getItem('redirectAfterLogin'))).toBe(
			`${path}#code=012345ABCD`
		);
		signedIn = true;
		await page.reload();
		await expect(page).toHaveURL(new RegExp(`${path}$`));
		await expect(page.getByLabel('หรือใช้รหัสอุปกรณ์ผู้รับ')).toHaveValue('012345ABCD');
		await page.getByRole('button', { name: 'เปิดรับส่งไฟล์', exact: true }).click();
		await expect(page.getByTestId('pairing-code')).toBeVisible();
		await page.getByRole('link', { name: 'เครื่องมือทั้งหมด' }).click();
		await expect.poll(() => api.devices.length).toBe(0);
	} finally {
		await api.close();
	}
});

for (const width of [390, 1440])
	for (const theme of ['light', 'dark'])
		test(`batch layout and keyboard at ${width}px in ${theme}`, async ({ page }, testInfo) => {
			const api = await fileTransferFixture();
			try {
				await page.setViewportSize({ width, height: 1000 });
				await page.addInitScript(
					(selected) => localStorage.setItem('ui-preferences', JSON.stringify({ theme: selected })),
					theme
				);
				await api.setup(page);
				await open(page, 'อุปกรณ์ชื่อยาวสำหรับตรวจสอบการตัดบรรทัดในหน้าจอแคบ');
				await expect(page.locator('html')).toHaveClass(theme === 'dark' ? /dark/ : /^(?!.*dark)/);
				await page.getByRole('button', { name: 'แสดง QR จับคู่' }).click();
				await expect(page.getByAltText('QR สำหรับจับคู่อุปกรณ์นี้')).toBeVisible();
				expect(
					await page
						.getByRole('main')
						.evaluate((element) => element.scrollWidth <= element.clientWidth)
				).toBe(true);
				await page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true }).focus();
				await expect(page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true })).toBeFocused();
				await page.screenshot({
					path: testInfo.outputPath(`layout-${width}-${theme}.png`),
					fullPage: true
				});
			} finally {
				await api.close();
			}
		});

test('a lost create response plus failed snapshot keeps its slot until authoritative pair closure', async ({
	browser,
	page
}, testInfo) => {
	test.setTimeout(90_000);
	const api = await fileTransferFixture(),
		contexts = await Promise.all(
			Array.from({ length: 7 }, () => browser.newContext({ serviceWorkers: 'block' }))
		);
	const recipients = await Promise.all(contexts.map((context) => context.newPage()));
	try {
		await api.setup(page);
		await open(page, 'ผู้ส่ง');
		for (const [index, recipient] of recipients.entries()) {
			await api.setup(recipient, 'staff', { userId: randomUUID(), sessionId: randomUUID() });
			await open(recipient, `ปลายทาง ${index + 1}`);
		}
		api.loseCreateResponseFor('ปลายทาง 1');
		api.failSnapshots();
		await choose(page, [input('unknown.txt', 'unknown response')]);
		await request(
			page,
			Array.from({ length: 7 }, (_, index) => `ปลายทาง ${index + 1}`)
		);
		const unknown = outgoing(page, 'ปลายทาง 1');
		await expect(unknown).toHaveAttribute('data-phase', 'failed');
		await expect(unknown.getByText(/ช่องส่งนี้ยังถูกจองไว้/)).toBeVisible();
		await expect(outgoing(page, 'ปลายทาง 7')).toHaveAttribute('data-phase', 'queued');
		await unknown.getByRole('button', { name: 'สิ้นสุดคำขอ', exact: true }).click();
		await unknown.getByRole('button', { name: 'ส่งคำขอใหม่', exact: true }).click();
		await expect(unknown.getByText(/ช่องส่งยังถูกจองไว้/)).toBeVisible();
		expect(
			new Set(
				api.requests
					.filter((entry) => entry.path.endsWith('/pairs'))
					.map((entry) => JSON.parse(entry.body).clientRequestId)
			).size
		).toBe(6);
		expect(api.maxOutgoing).toBe(6);
		api.restoreSnapshots();
		await unknown.getByRole('button', { name: 'สิ้นสุดคำขอ', exact: true }).click();
		await expect(
			recipients[6].getByRole('button', { name: 'ดูรายการไฟล์', exact: true })
		).toBeVisible();
		await expect(unknown.getByRole('button', { name: 'สิ้นสุดคำขอ', exact: true })).toHaveCount(0);
		expect(api.maxOutgoing).toBeLessThanOrEqual(6);
		api.releaseCreatedEvents();
		await expect(
			recipients[0].getByRole('button', { name: 'ดูรายการไฟล์', exact: true })
		).toHaveCount(0);
		expect(api.requests.filter((entry) => entry.path.endsWith('/signals'))).toHaveLength(0);
	} finally {
		const observations = new Map<string, number>();
		for (const request of api.requests.filter((entry) => entry.path.endsWith('/pairs'))) {
			const id: string = JSON.parse(request.body).clientRequestId;
			observations.set(id, (observations.get(id) ?? 0) + 1);
		}
		await testInfo.attach('create-request-observations', {
			body: JSON.stringify({
				serverPostCount: [...observations.values()].reduce((sum, count) => sum + count, 0),
				uniqueRequests: observations.size,
				nonceMultiplicities: [...observations.values()],
				livePairs: api.pairs.length
			}),
			contentType: 'application/json'
		});
		api.restoreSnapshots();
		api.releaseCreatedEvents();
		await Promise.all(contexts.map((context) => context.close()));
		await api.close();
	}
});

test('cancelled create binds a late successful HTTP pair for cleanup when its SSE event was held', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		api.holdRequest();
		api.holdNextCreatedEvent();
		api.failSnapshots();
		await choose(page, [input('cancelled.txt', 'cancel pending response')]);
		await request(page, ['ผู้รับ']);
		const job = outgoing(page, 'ผู้รับ');
		await expect(job).toHaveAttribute('data-phase', 'requesting');
		await expect.poll(() => api.pairs.length).toBe(1);
		await job.getByRole('button', { name: 'ยกเลิกผู้รับนี้', exact: true }).click();
		await expect(job).toHaveAttribute('data-phase', 'cancelled');
		await expect(page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true })).toBeDisabled();
		await job.getByRole('button', { name: 'ส่งคำขอใหม่', exact: true }).click();
		expect(api.requests.filter((entry) => entry.path.endsWith('/pairs'))).toHaveLength(1);
		api.releaseRequest();
		await expect.poll(() => api.pairs.length).toBe(0);
		await expect(page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true })).toBeEnabled();
		api.releaseCreatedEvents();
		await expect(receiver.getByRole('button', { name: 'ดูรายการไฟล์', exact: true })).toHaveCount(
			0
		);
		await expect(job).toHaveAttribute('data-phase', 'cancelled');
		expect(api.requests.filter((entry) => entry.path.endsWith('/signals'))).toHaveLength(0);
	} finally {
		api.restoreSnapshots();
		api.releaseRequest();
		api.releaseCreatedEvents();
		await receiver.close();
		await api.close();
	}
});

test('a negative snapshot cannot free a cancelled request that is still awaiting server mutation', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		api.holdCreateMutation();
		await choose(page, [input('pending.txt', 'pending mutation')]);
		await request(page, ['ผู้รับ']);
		const job = outgoing(page, 'ผู้รับ');
		await expect(job).toHaveAttribute('data-phase', 'requesting');
		await expect
			.poll(() => api.requests.filter((entry) => entry.path.endsWith('/pairs')).length)
			.toBe(1);
		expect(api.pairs).toHaveLength(0);
		await job.getByRole('button', { name: 'ยกเลิกผู้รับนี้', exact: true }).click();
		await job.getByRole('button', { name: 'สิ้นสุดคำขอ', exact: true }).click();
		await expect(job.getByText(/ช่องส่งยังถูกจองไว้/)).toBeVisible();
		await expect(page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true })).toBeDisabled();
		api.releaseCreateMutation();
		await expect
			.poll(() => api.requests.filter((entry) => entry.path.endsWith('/close')).length)
			.toBe(1);
		await expect.poll(() => api.pairs.length).toBe(0);
		await expect(page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true })).toBeEnabled();
		await expect(job).toHaveAttribute('data-phase', 'cancelled');
		expect(api.requests.filter((entry) => entry.path.endsWith('/signals'))).toHaveLength(0);
	} finally {
		api.releaseCreateMutation();
		await receiver.close();
		await api.close();
	}
});

test('explicit retry sends the currently selected file objects even when metadata is unchanged', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('version.txt', 'old')]);
		await request(page, ['ผู้รับ']);
		await receiver.getByRole('button', { name: 'ปฏิเสธ', exact: true }).click();
		await expect(page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true })).toBeEnabled();
		await choose(page, [input('version.txt', 'new')]);
		await outgoing(page, 'ผู้รับ').getByRole('button', { name: 'ส่งคำขอใหม่' }).click();
		await accept(receiver);
		expect(await download(receiver, 0, 'version.txt')).toEqual(Buffer.from('new'));
	} finally {
		await receiver.close();
		await api.close();
	}
});

test('a held old close response cannot prevent the retried pair from closing independently', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('retry.txt', 'new request')]);
		await request(page, ['ผู้รับ']);
		const first = structuredClone(api.pairs[0]),
			job = outgoing(page, 'ผู้รับ');
		api.holdCloseResponses();
		await job.getByRole('button', { name: 'ยกเลิกผู้รับนี้', exact: true }).click();
		await expect.poll(() => api.pairs.length).toBe(0);
		await job.getByRole('button', { name: 'ส่งคำขอใหม่', exact: true }).click();
		await expect(receiver.getByRole('button', { name: 'ดูรายการไฟล์', exact: true })).toBeVisible();
		const second = structuredClone(api.pairs[0]);
		expect(second.clientRequestId).not.toBe(first.clientRequestId);
		await accept(receiver);
		await expect(job).toHaveAttribute('data-phase', 'complete');
		await expect
			.poll(
				() =>
					api.requests.filter(
						(entry) =>
							entry.path.endsWith(`/pairs/${second.id}/close`) &&
							JSON.parse(entry.body).deviceId === second.initiator.id
					).length
			)
			.toBe(1);
		api.releaseCloseResponses();
		await expect.poll(() => api.pairs.length).toBe(0);
		await expect(job.getByRole('alert')).toHaveCount(0);
		expect(await download(receiver, 0, 'retry.txt')).toEqual(Buffer.from('new request'));
	} finally {
		api.releaseCloseResponses();
		await receiver.close();
		await api.close();
	}
});

test('late close success after device restart cannot restore old file references or queue an old retry', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('old-selection.txt', 'private previous selection')]);
		await request(page, ['ผู้รับ']);
		const job = outgoing(page, 'ผู้รับ'),
			pair = structuredClone(api.pairs[0]);
		api.failSenderClose();
		const reconciled = page.waitForResponse(
			(response) =>
				new URL(response.url()).pathname.endsWith(`/devices/${pair.initiator.id}`) &&
				response.request().method() === 'GET'
		);
		await job.getByRole('button', { name: 'ยกเลิกผู้รับนี้', exact: true }).click();
		await expect(job.getByText(/ยังยืนยันการปิดคำขอไม่ได้/)).toBeVisible();
		await (await reconciled).finished();
		await page.evaluate(
			() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
		);
		api.holdCloseResponses();
		await job.getByRole('button', { name: 'ส่งคำขอใหม่', exact: true }).click();
		await expect.poll(() => api.pairs.length).toBe(0);
		await page.getByRole('button', { name: 'ปิดรับส่งไฟล์ทั้งหมด', exact: true }).click();
		await page.getByRole('button', { name: 'เปิดรับส่งไฟล์', exact: true }).click();
		await expect(page.getByTestId('pairing-code')).toBeVisible();
		const oldResponse = page.waitForResponse(
			(response) => response.url().endsWith(`/pairs/${pair.id}/close`) && response.status() === 200
		);
		api.releaseCloseResponses();
		await oldResponse;
		await page.evaluate(
			() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
		);
		await expect(page.getByTestId('source-summary')).toHaveCount(0);
		await expect(job).toHaveAttribute('data-phase', 'cancelled');
		expect(
			new Set(
				api.requests
					.filter((entry) => entry.path.endsWith('/pairs'))
					.map((entry) => JSON.parse(entry.body).clientRequestId)
			).size
		).toBe(1);
	} finally {
		api.releaseCloseResponses();
		await receiver.close();
		await api.close();
	}
});

test('large and many files use disk manifest pages and one independently readable ZIP64 download', async ({
	context,
	page
}) => {
	test.setTimeout(240_000);
	const api = await fileTransferFixture(),
		receiver = await context.newPage(),
		directory = await mkdtemp(join(tmpdir(), 'schoolorbit-transfer-'));
	try {
		const paths: string[] = [];
		for (const name of ['large-a.bin', 'large-b.bin']) {
			const filename = join(directory, name),
				file = await openFile(filename, 'w');
			await file.truncate(65 * 1024 * 1024);
			await file.close();
			paths.push(filename);
		}
		for (let index = 0; index < 50; index++) {
			const filename = join(directory, `empty-${index}.txt`);
			await writeFile(filename, '');
			paths.push(filename);
		}
		for (const [index, content] of ['first', 'second'].entries()) {
			const folder = join(directory, String(index));
			await mkdir(folder);
			const filename = join(folder, 'ซ้ำ.txt');
			await writeFile(filename, content);
			paths.push(filename);
		}
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่งขนาดใหญ่');
		await open(receiver, 'ผู้รับขนาดใหญ่');
		await page.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true }).setInputFiles(paths);
		await request(page, ['ผู้รับขนาดใหญ่']);
		expect(JSON.parse(api.requests.find((r) => r.path.endsWith('/pairs'))!.body).batch).toEqual({
			fileCount: 54,
			totalBytes: 130 * 1024 * 1024 + 11
		});
		await viewManifest(receiver);
		await expect(incoming(receiver)).toHaveAttribute('data-phase', 'review');
		await expect(incoming(receiver).locator('ol li')).toHaveCount(50);
		await incoming(receiver).getByRole('button', { name: 'รายการถัดไป', exact: true }).click();
		await expect(
			incoming(receiver).getByText('54. ซ้ำ.txt · 6 ไบต์', { exact: true })
		).toBeVisible();
		await receiveFiles(receiver);
		await expect(incoming(receiver).getByText(/MB\/s/)).toBeVisible();
		await expect(outgoing(page, 'ผู้รับขนาดใหญ่')).toHaveAttribute('data-phase', 'complete', {
			timeout: 120_000
		});
		await expect(receiver.getByText('รับครบ 54 ไฟล์ · 130.0 MiB', { exact: true })).toBeVisible();
		await expect(
			receiver.getByRole('region', { name: 'ไฟล์ที่รับครบแล้ว', exact: true }).locator('ol li')
		).toHaveCount(50);
		await receiver.getByRole('button', { name: 'ไฟล์ถัดไป', exact: true }).click();
		expect(await download(receiver, 53, 'ซ้ำ.txt')).toEqual(Buffer.from('second'));
		const firstDownload = receiver.waitForEvent('download');
		await receiver.getByRole('button', { name: 'ดาวน์โหลดทั้งหมดเป็น ZIP', exact: true }).click();
		const first = await firstDownload;
		const repeatDownload = receiver.waitForEvent('download');
		await receiver.getByRole('button', { name: 'ดาวน์โหลดทั้งหมดเป็น ZIP', exact: true }).click();
		const repeat = await repeatDownload;
		const firstPath = await first.path(),
			repeatPath = await repeat.path();
		if (!firstPath || !repeatPath) throw new Error('ZIP download missing');
		const verified = JSON.parse(
			execFileSync(
				'python3',
				[
					'-c',
					`import hashlib,json,sys,zipfile
p,q=sys.argv[1:]
def digest(path):
 h=hashlib.sha256()
 with open(path,'rb') as f:
  while b:=f.read(1048576):h.update(b)
 return h.hexdigest()
assert digest(p)==digest(q)
expected=hashlib.sha256()
for i in range(65):expected.update(bytes(1048576))
with zipfile.ZipFile(p) as z:
 entries=z.infolist();assert len(entries)==54
 assert len({e.filename for e in entries})==54
 for index,e in enumerate(entries):
  assert e.filename.startswith(str(index+1).zfill(2)+' - ')
  assert '/' not in e.filename and '\\\\' not in e.filename
  h=hashlib.sha256()
  with z.open(e) as f:
   while b:=f.read(1048576):h.update(b)
  if index<2:assert e.file_size==65*1048576 and h.hexdigest()==expected.hexdigest()
  elif index<52:assert e.file_size==0
  else:assert h.hexdigest()==hashlib.sha256([b'first',b'second'][index-52]).hexdigest()
  assert e.extract_version>=45
with open(p,'rb') as f:f.seek(-128,2);assert b'PK\\x06\\x06' in f.read()
print(json.dumps({'entries':54,'zip64':True,'contents':'verified'}))`,
					firstPath,
					repeatPath
				],
				{ encoding: 'utf8' }
			)
		);
		expect(verified).toEqual({ entries: 54, zip64: true, contents: 'verified' });
		for (const entry of api.requests.filter((r) => r.path.endsWith('/pairs'))) {
			const body = JSON.parse(entry.body);
			expect('files' in body).toBe(false);
			expect(entry.body.length).toBeLessThan(1000);
		}
	} finally {
		await receiver.close();
		await api.close();
		await rm(directory, { recursive: true, force: true });
	}
});

test('disk backpressure bounds sender credit, active clear is refused, and quota errors retain completed files', async ({
	context,
	page
}) => {
	test.setTimeout(90_000);
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await page.addInitScript(() => {
			let sent = 0;
			Object.defineProperty(window, 'transferSentBytes', { get: () => sent });
			const create = RTCPeerConnection.prototype.createDataChannel;
			RTCPeerConnection.prototype.createDataChannel = function (label, options) {
				const channel = create.call(this, label, options),
					send = channel.send;
				Object.defineProperty(channel, 'send', {
					value(data: unknown) {
						if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) sent += data.byteLength;
						return Reflect.apply(send, channel, [data]);
					}
				});
				return channel;
			};
		});
		await receiver.addInitScript(() => {
			const create = FileSystemFileHandle.prototype.createWritable;
			FileSystemFileHandle.prototype.createWritable = async function (options) {
				const stream = await create.call(this, options);
				if (this.name === 'file-1') {
					const write = stream.write.bind(stream);
					stream.write = async (data) => {
						window.dispatchEvent(new Event('transfer-write-blocked'));
						await new Promise<void>((resolve) =>
							window.addEventListener('release-transfer-write', () => resolve(), { once: true })
						);
						if (Reflect.get(window, 'transferQuotaFailure'))
							throw new DOMException('full', 'QuotaExceededError');
						return write(data);
					};
				}
				return stream;
			};
		});
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [
			input('good.txt', 'retained'),
			{
				name: 'blocked.bin',
				mimeType: 'application/octet-stream',
				buffer: Buffer.alloc(2 * 1024 * 1024)
			}
		]);
		await request(page, ['ผู้รับ']);
		await accept(receiver);
		await expect(
			receiver.getByRole('button', { name: 'ดาวน์โหลด 1 good.txt', exact: true })
		).toBeVisible();
		await expect
			.poll(() => page.evaluate(() => Reflect.get(window, 'transferSentBytes')))
			.toBe(8 + 1024 * 1024);
		await expect(
			receiver.getByRole('button', { name: 'ล้างไฟล์ที่รับแล้วทั้งหมด', exact: true })
		).toBeDisabled();
		await expect(
			receiver.getByRole('button', { name: 'ดาวน์โหลดทั้งหมดเป็น ZIP', exact: true })
		).toBeDisabled();
		await page.evaluate(
			() =>
				new Promise<void>((resolve) =>
					requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
				)
		);
		expect(await page.evaluate(() => Reflect.get(window, 'transferSentBytes'))).toBe(
			8 + 1024 * 1024
		);
		expect(await download(receiver, 0, 'good.txt')).toEqual(Buffer.from('retained'));
		await receiver.evaluate(() => {
			Reflect.set(window, 'transferQuotaFailure', true);
			window.dispatchEvent(new Event('release-transfer-write'));
		});
		await expect(incoming(receiver).getByText(/พื้นที่จัดเก็บปลายทางไม่พอ/)).toBeVisible();
		expect(await download(receiver, 0, 'good.txt')).toEqual(Buffer.from('retained'));
		await expect(
			receiver.getByRole('button', { name: 'ดาวน์โหลด 2 blocked.bin', exact: true })
		).toHaveCount(0);
		await expect.poll(() => api.pairs.length).toBe(0);
	} finally {
		await receiver
			.evaluate(() => window.dispatchEvent(new Event('release-transfer-write')))
			.catch(() => {});
		await receiver.close();
		await api.close();
	}
});

test('active tab storage survives another tab cleanup and logout removes its namespace and URLs', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage(),
		collector = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		await api.setup(collector);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('retained.txt', 'private bytes')]);
		await request(page, ['ผู้รับ']);
		await accept(receiver);
		await expect(outgoing(page, 'ผู้รับ')).toHaveAttribute('data-phase', 'complete');
		const namespaces = () =>
			receiver.evaluate(async () => {
				const root = await (
					await navigator.storage.getDirectory()
				).getDirectoryHandle('schoolorbit-file-transfer-v3');
				const names: string[] = [];
				for await (const [name] of root.entries()) names.push(name);
				return names;
			});
		await expect.poll(async () => (await namespaces()).length).toBe(1);
		await open(collector, 'ตรวจพื้นที่');
		await expect.poll(async () => (await namespaces()).length).toBe(1);
		expect(await download(receiver, 0, 'retained.txt')).toEqual(Buffer.from('private bytes'));
		const device = api.devices.find((d) => d.label === 'ผู้รับ');
		if (!device) throw new Error('missing receiver');
		api.emit(device.id, { type: 'session_invalid', data: {} });
		await expect(receiver.getByRole('region', { name: 'ไฟล์ที่รับครบแล้ว' })).toHaveCount(0);
		await expect.poll(async () => (await namespaces()).length).toBe(0);
	} finally {
		await receiver.close();
		await collector.close();
		await api.close();
	}
});

test('unsupported receiving APIs leave sending available and metadata consent unavailable', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await receiver.addInitScript(() =>
			Object.defineProperty(navigator.storage, 'getDirectory', { value: undefined })
		);
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await expect(receiver.getByLabel('เลือกไฟล์หลายไฟล์', { exact: true })).toBeEnabled();
		await choose(page, [input('one.txt', 'one')]);
		await request(page, ['ผู้รับ']);
		await expect(
			receiver.getByRole('button', { name: 'ดูรายการไฟล์', exact: true })
		).toBeDisabled();
		await expect(incoming(receiver).getByText(/อุปกรณ์นี้ยังส่งไฟล์ได้/)).toBeVisible();
		expect(api.requests.filter((r) => r.path.endsWith('/accept'))).toHaveLength(0);
	} finally {
		await receiver.close();
		await api.close();
	}
});

test('archive cancellation and logout wait for a pending OPFS archive open before cleanup', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await receiver.addInitScript(() => {
			const create = FileSystemFileHandle.prototype.createWritable;
			FileSystemFileHandle.prototype.createWritable = async function (options) {
				if (this.name === 'archive.zip') {
					Reflect.set(window, 'archiveOpening', true);
					await new Promise<void>((resolve) =>
						window.addEventListener('release-archive-open', () => resolve(), { once: true })
					);
				}
				return create.call(this, options);
			};
		});
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('keep.txt', 'keep')]);
		await request(page, ['ผู้รับ']);
		await accept(receiver);
		await expect(outgoing(page, 'ผู้รับ')).toHaveAttribute('data-phase', 'complete');
		await receiver.getByRole('button', { name: 'ดาวน์โหลดทั้งหมดเป็น ZIP', exact: true }).click();
		await expect
			.poll(() => receiver.evaluate(() => Reflect.get(window, 'archiveOpening')))
			.toBe(true);
		await receiver.getByRole('button', { name: 'ยกเลิกการสร้าง ZIP', exact: true }).click();
		await receiver.evaluate(() => window.dispatchEvent(new Event('release-archive-open')));
		await expect(
			receiver.getByText('ยกเลิกการสร้าง ZIP แล้ว ไฟล์ที่รับครบยังดาวน์โหลดได้', { exact: true })
		).toBeVisible();
		expect(await download(receiver, 0, 'keep.txt')).toEqual(Buffer.from('keep'));
		await receiver.evaluate(() => Reflect.set(window, 'archiveOpening', false));
		await receiver.getByRole('button', { name: 'ดาวน์โหลดทั้งหมดเป็น ZIP', exact: true }).click();
		await expect
			.poll(() => receiver.evaluate(() => Reflect.get(window, 'archiveOpening')))
			.toBe(true);
		const device = api.devices.find((d) => d.label === 'ผู้รับ');
		if (!device) throw new Error('missing receiver');
		api.emit(device.id, { type: 'session_invalid', data: {} });
		await expect(receiver.getByRole('region', { name: 'ไฟล์ที่รับครบแล้ว' })).toHaveCount(0);
		const namespaceCount = () =>
			receiver.evaluate(async () => {
				const root = await (
					await navigator.storage.getDirectory()
				).getDirectoryHandle('schoolorbit-file-transfer-v3');
				let count = 0;
				for await (const entry of root.entries()) {
					if (entry) count++;
				}
				return count;
			});
		expect(await namespaceCount()).toBe(1);
		await receiver.evaluate(() => window.dispatchEvent(new Event('release-archive-open')));
		await expect.poll(namespaceCount).toBe(0);
	} finally {
		await receiver
			.evaluate(() => window.dispatchEvent(new Event('release-archive-open')))
			.catch(() => {});
		await receiver.close();
		await api.close();
	}
});

test('cancelling manifest persistence releases the owned namespace without receiving binary files', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await receiver.addInitScript(() => {
			const create = FileSystemFileHandle.prototype.createWritable;
			FileSystemFileHandle.prototype.createWritable = async function (options) {
				const stream = await create.call(this, options);
				if (this.name === 'manifest') {
					const write = stream.write.bind(stream);
					stream.write = async (data) => {
						Reflect.set(window, 'manifestWriting', true);
						await new Promise<void>((resolve) =>
							window.addEventListener('release-manifest-write', () => resolve(), { once: true })
						);
						return write(data);
					};
				}
				return stream;
			};
		});
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('unconsented.txt', 'unconsented')]);
		await request(page, ['ผู้รับ']);
		await viewManifest(receiver);
		await expect
			.poll(() => receiver.evaluate(() => Reflect.get(window, 'manifestWriting')))
			.toBe(true);
		await incoming(receiver)
			.getByRole('button', { name: 'ยกเลิกการรับชุดนี้', exact: true })
			.click();
		await receiver.evaluate(() => window.dispatchEvent(new Event('release-manifest-write')));
		await expect.poll(() => api.pairs.length).toBe(0);
		await expect(receiver.getByRole('region', { name: 'ไฟล์ที่รับครบแล้ว' })).toHaveCount(0);
		await expect
			.poll(() =>
				receiver.evaluate(async () => {
					const root = await (
						await navigator.storage.getDirectory()
					).getDirectoryHandle('schoolorbit-file-transfer-v3');
					let count = 0;
					for await (const entry of root.entries()) {
						if (entry) count++;
					}
					return count;
				})
			)
			.toBe(0);
	} finally {
		await receiver
			.evaluate(() => window.dispatchEvent(new Event('release-manifest-write')))
			.catch(() => {});
		await receiver.close();
		await api.close();
	}
});

test('late metadata accept failure cannot clear the newer file-consent pending operation', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await receiver.addInitScript(() => {
			const estimate = navigator.storage.estimate.bind(navigator.storage);
			navigator.storage.estimate = async () => {
				Reflect.set(window, 'quotaChecking', true);
				await new Promise<void>((resolve) =>
					window.addEventListener('release-quota-check', () => resolve(), { once: true })
				);
				return estimate();
			};
		});
		await api.setup(page);
		await api.setup(receiver);
		api.holdAccept();
		api.failAcceptResponse();
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('stage.txt', 'stage')]);
		await request(page, ['ผู้รับ']);
		await viewManifest(receiver);
		await expect(
			receiver.getByRole('button', { name: 'รับไฟล์ทั้งชุด', exact: true })
		).toBeEnabled();
		await receiveFiles(receiver);
		await expect
			.poll(() => receiver.evaluate(() => Reflect.get(window, 'quotaChecking')))
			.toBe(true);
		const failed = receiver.waitForResponse(
			(response) => response.url().endsWith('/accept') && response.status() === 503
		);
		api.releaseAccept();
		await failed;
		await receiver.evaluate(
			() =>
				new Promise<void>((resolve) =>
					requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
				)
		);
		await expect(
			receiver.getByRole('button', { name: 'กำลังตรวจสอบพื้นที่', exact: true })
		).toBeDisabled();
		await expect(incoming(receiver)).toHaveAttribute('data-phase', 'review');
		await expect(incoming(receiver).getByText(/ยังยืนยันการตอบรับไม่ได้/)).toHaveCount(0);
		await receiver.evaluate(() => window.dispatchEvent(new Event('release-quota-check')));
		await expect(outgoing(page, 'ผู้รับ')).toHaveAttribute('data-phase', 'complete');
		expect(await download(receiver, 0, 'stage.txt')).toEqual(Buffer.from('stage'));
	} finally {
		api.releaseAccept();
		await receiver
			.evaluate(() => window.dispatchEvent(new Event('release-quota-check')))
			.catch(() => {});
		await receiver.close();
		await api.close();
	}
});

test('folder receive uses the click gesture and keeps completed device files while cancelling its new partial', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await receiver.addInitScript(() => {
			window.showDirectoryPicker = async () => {
				Reflect.set(window, 'pickerGesture', navigator.userActivation.isActive);
				const root = await (
					await navigator.storage.getDirectory()
				).getDirectoryHandle('fixture-device-folder', { create: true });
				const sentinel = await root.getFileHandle('existing.txt', { create: true });
				const stream = await sentinel.createWritable();
				await stream.write(new Uint8Array([9]));
				await stream.close();
				return root;
			};
			const create = FileSystemFileHandle.prototype.createWritable;
			FileSystemFileHandle.prototype.createWritable = async function (options) {
				if (this.name.startsWith('2 - ')) {
					Reflect.set(window, 'partialOpening', true);
					await new Promise<void>((resolve) =>
						window.addEventListener('release-destination-open', () => resolve(), { once: true })
					);
				}
				return create.call(this, options);
			};
		});
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [
			input('keep.txt', 'permanent'),
			{
				name: 'partial.bin',
				mimeType: 'application/octet-stream',
				buffer: Buffer.alloc(2 * 1024 * 1024)
			}
		]);
		await request(page, ['ผู้รับ']);
		await viewManifest(receiver);
		await receiver.getByRole('button', { name: 'รับไฟล์ทั้งชุด', exact: true }).click();
		await expect
			.poll(() => receiver.evaluate(() => Reflect.get(window, 'pickerGesture')))
			.toBe(true);
		await expect
			.poll(() => receiver.evaluate(() => Reflect.get(window, 'partialOpening')))
			.toBe(true);
		await expect(receiver.getByTestId('save-status')).toContainText('แล้ว 1 ไฟล์');
		await incoming(receiver)
			.getByRole('button', { name: 'ยกเลิกการรับชุดนี้', exact: true })
			.click();
		await receiver.evaluate(() => window.dispatchEvent(new Event('release-destination-open')));
		const saved = () =>
			receiver.evaluate(async () => {
				const root = await (
					await navigator.storage.getDirectory()
				).getDirectoryHandle('fixture-device-folder');
				const result: { name: string; bytes: number[] }[] = [];
				for await (const [name, handle] of root.entries()) {
					if (handle.kind === 'directory') {
						const folder = await root.getDirectoryHandle(name);
						for await (const [fileName, file] of folder.entries())
							if (file.kind === 'file')
								result.push({
									name: fileName,
									bytes: [
										...new Uint8Array(
											await (await (await folder.getFileHandle(fileName)).getFile()).arrayBuffer()
										)
									]
								});
					}
				}
				return result;
			});
		await expect.poll(async () => (await saved()).length).toBe(1);
		expect((await saved())[0]).toEqual({
			name: '1 - keep.txt',
			bytes: [...Buffer.from('permanent')]
		});
		expect(await download(receiver, 0, 'keep.txt')).toEqual(Buffer.from('permanent'));
		await receiver.getByRole('button', { name: 'ล้างไฟล์ที่รับแล้วทั้งหมด', exact: true }).click();
		expect((await saved())[0].bytes).toEqual([...Buffer.from('permanent')]);
		const device = api.devices.find((d) => d.label === 'ผู้รับ');
		if (!device) throw new Error('missing device');
		api.emit(device.id, { type: 'session_invalid', data: {} });
		await expect(receiver.getByTestId('pairing-code')).toHaveCount(0);
		expect((await saved())[0].bytes).toEqual([...Buffer.from('permanent')]);
		expect(
			await receiver.evaluate(async () => {
				const root = await (
					await navigator.storage.getDirectory()
				).getDirectoryHandle('fixture-device-folder');
				return [
					...new Uint8Array(
						await (await (await root.getFileHandle('existing.txt')).getFile()).arrayBuffer()
					)
				];
			})
		).toEqual([9]);
	} finally {
		await receiver
			.evaluate(async () => {
				window.dispatchEvent(new Event('release-destination-open'));
				await (
					await navigator.storage.getDirectory()
				).removeEntry('fixture-device-folder', { recursive: true });
			})
			.catch(() => {});
		await receiver.close();
		await api.close();
	}
});

for (const refusal of ['AbortError', 'NotAllowedError'] as const)
	test(`picker ${refusal} stays in review until explicit browser fallback, then automatically requests a MIME-correct download`, async ({
		context,
		page
	}) => {
		const api = await fileTransferFixture(),
			receiver = await context.newPage();
		try {
			await receiver.addInitScript((name) => {
				window.showDirectoryPicker = async () => {
					throw new DOMException('fixture refusal', name);
				};
				const send = RTCDataChannel.prototype.send;
				Reflect.set(window, 'receiveReadyRequests', 0);
				Object.defineProperty(RTCDataChannel.prototype, 'send', {
					value(data: unknown) {
						if (typeof data === 'string' && JSON.parse(data).type === 'receive_ready')
							Reflect.set(
								window,
								'receiveReadyRequests',
								Number(Reflect.get(window, 'receiveReadyRequests')) + 1
							);
						return Reflect.apply(send, this, [data]);
					}
				});
				const create = URL.createObjectURL;
				URL.createObjectURL = function (blob) {
					Reflect.set(window, 'exportMime', blob instanceof Blob ? blob.type : '');
					return create(blob);
				};
			}, refusal);
			await api.setup(page);
			await api.setup(receiver);
			await open(page, 'ผู้ส่ง');
			await open(receiver, 'ผู้รับ');
			const photo = {
				name: 'รูป.jpg',
				mimeType: 'image/jpeg',
				buffer: Buffer.from([255, 216, 1, 2, 255, 217])
			};
			await choose(page, [photo]);
			await request(page, ['ผู้รับ']);
			await viewManifest(receiver);
			await receiver.getByRole('button', { name: 'รับไฟล์ทั้งชุด', exact: true }).click();
			await expect(incoming(receiver).getByRole('alert')).toContainText(/ยังไม่ได้รับ/);
			await expect(incoming(receiver)).toHaveAttribute('data-phase', 'review');
			expect(await receiver.evaluate(() => Reflect.get(window, 'receiveReadyRequests'))).toBe(0);
			await expect(receiver.getByRole('region', { name: 'ไฟล์ที่รับครบแล้ว' })).toHaveCount(0);
			const automatic = receiver.waitForEvent('download');
			await receiveFiles(receiver);
			const file = await automatic;
			expect(file.suggestedFilename()).toBe(photo.name);
			const filename = await file.path();
			if (!filename) throw new Error('auto download missing');
			expect(await readFile(filename)).toEqual(photo.buffer);
			expect(await receiver.evaluate(() => Reflect.get(window, 'exportMime'))).toBe('image/jpeg');
			expect(await receiver.evaluate(() => Reflect.get(window, 'receiveReadyRequests'))).toBe(1);
			await expect(receiver.getByTestId('save-status')).toContainText(
				'ยังยืนยันการบันทึกของเบราว์เซอร์ไม่ได้'
			);
		} finally {
			await receiver.close();
			await api.close();
		}
	});

test('a picker result after logout creates no destination child or binary consent', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await receiver.addInitScript(() => {
			window.showDirectoryPicker = async () => {
				const directory = await (
					await navigator.storage.getDirectory()
				).getDirectoryHandle('fixture-stale-picker', { create: true });
				Reflect.set(window, 'pickerWaiting', true);
				await new Promise<void>((resolve) =>
					window.addEventListener('resolve-stale-picker', () => resolve(), { once: true })
				);
				return directory;
			};
		});
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('unconsented.txt', 'private')]);
		await request(page, ['ผู้รับ']);
		await viewManifest(receiver);
		await receiver.getByRole('button', { name: 'รับไฟล์ทั้งชุด', exact: true }).click();
		await expect
			.poll(() => receiver.evaluate(() => Reflect.get(window, 'pickerWaiting')))
			.toBe(true);
		const device = api.devices.find((d) => d.label === 'ผู้รับ');
		if (!device) throw new Error('missing device');
		api.emit(device.id, { type: 'session_invalid', data: {} });
		await expect(receiver.getByTestId('pairing-code')).toHaveCount(0);
		await receiver.evaluate(() => window.dispatchEvent(new Event('resolve-stale-picker')));
		await expect
			.poll(() =>
				receiver.evaluate(async () => {
					const directory = await (
						await navigator.storage.getDirectory()
					).getDirectoryHandle('fixture-stale-picker');
					let count = 0;
					for await (const entry of directory.entries()) {
						if (entry) count++;
					}
					return count;
				})
			)
			.toBe(0);
		await expect(receiver.getByRole('region', { name: 'ไฟล์ที่รับครบแล้ว' })).toHaveCount(0);
	} finally {
		await receiver
			.evaluate(async () => {
				window.dispatchEvent(new Event('resolve-stale-picker'));
				await (
					await navigator.storage.getDirectory()
				).removeEntry('fixture-stale-picker', { recursive: true });
			})
			.catch(() => {});
		await receiver.close();
		await api.close();
	}
});

test('blocked automatic browser downloads retain exact files for manual recovery', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	let events = 0;
	receiver.on('download', () => events++);
	try {
		await receiver.addInitScript(() => {
			Reflect.set(window, 'blockAutomaticDownloads', true);
			const click = HTMLAnchorElement.prototype.click;
			HTMLAnchorElement.prototype.click = function () {
				if (!Reflect.get(window, 'blockAutomaticDownloads')) click.call(this);
			};
		});
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, [input('blocked.txt', 'exact bytes retained')]);
		await request(page, ['ผู้รับ']);
		await accept(receiver);
		await expect(outgoing(page, 'ผู้รับ')).toHaveAttribute('data-phase', 'complete');
		await expect(receiver.getByTestId('save-status')).toContainText('1/1');
		expect(events).toBe(0);
		await expect(receiver.getByTestId('save-status')).toContainText('ยังยืนยันการบันทึก');
		await receiver.evaluate(() => Reflect.set(window, 'blockAutomaticDownloads', false));
		expect(await download(receiver, 0, 'blocked.txt')).toEqual(Buffer.from('exact bytes retained'));
	} finally {
		await receiver.close();
		await api.close();
	}
});
