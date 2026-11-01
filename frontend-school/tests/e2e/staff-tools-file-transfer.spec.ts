import { expect, test, type Page } from '@playwright/test';
import jsQR from 'jsqr';
import { readFile } from 'node:fs/promises';
import { fileTransferFixture } from './fixtures/file-transfer';

const path = '/staff/tools/file-transfer';
test.use({ serviceWorkers: 'block' });
async function open(page: Page, label: string) {
	await page.goto(path);
	await page.getByLabel('ชื่ออุปกรณ์', { exact: true }).fill(label);
	await page.getByRole('button', { name: 'เปิดรับส่งไฟล์', exact: true }).click();
	await expect(page.getByTestId('pairing-code')).toBeVisible();
}
async function choose(page: Page, bytes: Buffer, name = 'ทดสอบ.txt') {
	await page
		.getByLabel('เลือกไฟล์', { exact: true })
		.setInputFiles({ name, mimeType: 'text/plain', buffer: bytes });
}
async function request(page: Page, recipient: string) {
	await page.getByRole('radio', { name: new RegExp(recipient) }).click();
	await page.getByRole('button', { name: 'ส่งคำขอไปยังอุปกรณ์ที่เลือก', exact: true }).click();
}

test('same-user devices discover, consent and transfer real ordered WebRTC bytes with no server file upload', async ({
	context,
	page
}, testInfo) => {
	const api = await fileTransferFixture();
	const receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		api.failSenderClose();
		await page.setViewportSize({ width: 1440, height: 1000 });
		await receiver.setViewportSize({ width: 390, height: 1000 });
		await open(page, 'โน้ตบุ๊กผู้ส่ง');
		await open(receiver, 'แท็บเล็ตผู้รับ');
		expect(api.devices).toHaveLength(2);
		expect(api.devices[0].id).not.toBe(api.devices[1].id);
		expect(api.devices[0].userId).toBe(api.devices[1].userId);
		const bytes = Buffer.from(Array.from({ length: 400_123 }, (_, index) => index % 251));
		await choose(page, bytes);
		await request(page, 'แท็บเล็ตผู้รับ');
		await expect(receiver.getByRole('button', { name: 'รับไฟล์', exact: true })).toBeVisible();
		expect(api.requests.filter((entry) => entry.path.endsWith('/signals'))).toHaveLength(0);
		await receiver
			.getByRole('region', { name: 'การรับส่งไฟล์', exact: true })
			.screenshot({ path: testInfo.outputPath('consent-mobile.png') });
		await expect(receiver.getByRole('button', { name: 'ดาวน์โหลดไฟล์', exact: true })).toHaveCount(
			0
		);
		await receiver.getByRole('button', { name: 'รับไฟล์', exact: true }).click();
		await expect(page.getByText('รับส่งไฟล์สำเร็จ', { exact: true })).toBeVisible();
		const downloadPromise = receiver.waitForEvent('download');
		await receiver.getByRole('button', { name: 'ดาวน์โหลดไฟล์', exact: true }).click();
		const download = await downloadPromise;
		expect(download.suggestedFilename()).toBe('ทดสอบ.txt');
		const downloaded = await download.path();
		if (!downloaded) throw new Error('Download missing');
		expect(await readFile(downloaded)).toEqual(bytes);
		await page.screenshot({ path: testInfo.outputPath('completed-desktop.png'), fullPage: true });
		await receiver
			.getByRole('region', { name: 'การรับส่งไฟล์', exact: true })
			.screenshot({ path: testInfo.outputPath('completed-mobile.png') });
		await expect.poll(() => api.pairs.length).toBe(0);
		const signals = api.requests.filter((entry) => entry.path.endsWith('/signals'));
		expect(signals).toHaveLength(2);
		for (const signal of signals) {
			const body = JSON.parse(signal.body);
			expect(Object.keys(body).sort()).toEqual(['deviceId', 'sdp', 'type']);
			expect(body.sdp).toContain(' typ host');
			expect(body.sdp).not.toMatch(/ typ (relay|srflx)/);
		}
		for (const entry of api.requests) {
			expect(entry.body.length).toBeLessThan(40_000);
			if (entry.path.endsWith('/pairs'))
				expect(Object.keys(JSON.parse(entry.body).file).sort()).toEqual([
					'mimeType',
					'name',
					'size'
				]);
		}
		await receiver.getByRole('button', { name: 'ล้างไฟล์ที่รับแล้ว' }).click();
		await expect(receiver.getByRole('button', { name: 'ดาวน์โหลดไฟล์', exact: true })).toHaveCount(
			0
		);
	} finally {
		await receiver.close();
		await api.close();
	}
});

test('pairing fragment prefill is removed, code pairing handles empty file, and late accept cannot erase completed download', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture();
	const receiver = await context.newPage();
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
		expect(decoded).toBe(`http://127.0.0.1:4173${path}#code=${code}`);
		if (!decoded) throw new Error('QR cannot be decoded');
		await page.goto(decoded);
		await expect(page.getByLabel('หรือใช้รหัสอุปกรณ์ผู้รับ')).toHaveValue(code);
		await expect(page).toHaveURL(new RegExp(`${path}$`));
		await page.getByRole('button', { name: 'เปิดรับส่งไฟล์', exact: true }).click();
		await expect(page.getByTestId('pairing-code')).toBeVisible();
		await choose(page, Buffer.alloc(0), 'empty.txt');
		await page.getByRole('button', { name: 'ส่งคำขอด้วยรหัส' }).click();
		await receiver.getByRole('button', { name: 'รับไฟล์', exact: true }).click();
		await expect(
			receiver.getByRole('button', { name: 'ดาวน์โหลดไฟล์', exact: true })
		).toBeVisible();
		await expect(page.getByText('รับส่งไฟล์สำเร็จ', { exact: true })).toBeVisible();
		api.releaseAccept();
		api.releaseRequest();
		await expect(page.getByLabel('เลือกไฟล์', { exact: true })).toBeEnabled();
		await receiver.clock.install();
		await receiver.clock.fastForward(21_000);
		await expect(
			receiver.getByRole('button', { name: 'ดาวน์โหลดไฟล์', exact: true })
		).toBeVisible();
		await expect(receiver.getByText('รับส่งไฟล์สำเร็จ', { exact: true })).toBeVisible();
	} finally {
		api.releaseAccept();
		api.releaseRequest();
		await receiver.close();
		await api.close();
	}
});

test('decline and stale pending events never open a peer or resurrect a cancelled pair', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture();
	const receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, Buffer.from('not sent'));
		await request(page, 'ผู้รับ');
		await expect(receiver.getByRole('button', { name: 'ปฏิเสธ', exact: true })).toBeVisible();
		const pair = api.pairs[0];
		await receiver.getByRole('button', { name: 'ปฏิเสธ', exact: true }).click();
		await expect(page.getByText('ผู้รับปฏิเสธคำขอ', { exact: true })).toBeVisible();
		api.emit(pair.initiator.id, { type: 'pair_changed', data: pair });
		api.emit(pair.recipient.id, { type: 'pair_changed', data: pair });
		await expect(receiver.getByRole('button', { name: 'รับไฟล์', exact: true })).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'ยกเลิกการรับส่ง', exact: true })).toHaveCount(0);
		expect(api.requests.filter((entry) => entry.path.endsWith('/signals'))).toHaveLength(0);
	} finally {
		await receiver.close();
		await api.close();
	}
});

test('blocked LAN gives truthful guidance and session invalidation removes consent and registration', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture();
	const receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		api.blockSignals();
		await open(page, 'ผู้ส่ง');
		await open(receiver, 'ผู้รับ');
		await choose(page, Buffer.from('blocked'));
		await request(page, 'ผู้รับ');
		const pendingPair = structuredClone(api.pairs[0]);
		await receiver.clock.install();
		await receiver.getByRole('button', { name: 'รับไฟล์', exact: true }).click();
		api.emit(pendingPair.recipient.id, { type: 'pair_changed', data: pendingPair });
		await expect(receiver.getByRole('button', { name: 'รับไฟล์', exact: true })).toHaveCount(0);
		await receiver.clock.fastForward(20_001);
		await expect(
			receiver.getByText(/QR และรหัสช่วยจับคู่เท่านั้น ไม่สามารถแก้การปิดกั้นเครือข่ายได้/)
		).toBeVisible();
		const device = api.devices.find((entry) => entry.label === 'ผู้รับ');
		if (!device) throw new Error('Device missing');
		api.emit(device.id, { type: 'session_invalid', data: {} });
		await expect(receiver.getByTestId('pairing-code')).toHaveCount(0);
		await expect(
			receiver.getByRole('button', { name: 'เปิดรับส่งไฟล์', exact: true })
		).toBeVisible();
	} finally {
		await receiver.close();
		await api.close();
	}
});

for (const userType of ['student', 'parent'])
	test(`${userType} is denied the tool`, async ({ page }) => {
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

for (const width of [390, 1440])
	for (const theme of ['light', 'dark'])
		test(`accessible layout at ${width}px in ${theme}`, async ({ page }, testInfo) => {
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
				await expect(
					page.getByRole('heading', { name: 'ส่งไฟล์ในเครือข่ายเดียวกัน', exact: true })
				).toBeVisible();
				expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
					true
				);
				expect(
					await page
						.getByRole('main')
						.evaluate((element) => element.scrollWidth <= element.clientWidth)
				).toBe(true);
				await page.getByLabel('เลือกไฟล์', { exact: true }).focus();
				await expect(page.getByLabel('เลือกไฟล์', { exact: true })).toBeFocused();
				await page.screenshot({
					path: testInfo.outputPath(`layout-${width}-${theme}.png`),
					fullPage: true
				});
			} finally {
				await api.close();
			}
		});

test('Unicode codepoint limits accept non-BMP device labels and file names before consent', async ({
	context,
	page
}) => {
	const api = await fileTransferFixture(),
		receiver = await context.newPage();
	try {
		await api.setup(page);
		await api.setup(receiver);
		await open(page, '😀'.repeat(40));
		await open(receiver, 'ผู้รับ Unicode');
		const name = '😀'.repeat(128);
		await choose(page, Buffer.from('Unicode'), name);
		await request(page, 'ผู้รับ Unicode');
		await expect(receiver.getByText(`${name} · 7 ไบต์`, { exact: true })).toBeVisible();
		await expect(receiver.getByRole('button', { name: 'รับไฟล์', exact: true })).toBeVisible();
		await receiver.getByRole('button', { name: 'ปฏิเสธ', exact: true }).click();
	} finally {
		await receiver.close();
		await api.close();
	}
});

test('pairing fragment survives the login redirect and route disposal removes the registered device', async ({
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

test('malformed oversized binary frames over a real peer channel are refused without a download', async ({
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
					send = channel.send.bind(channel);
				Object.defineProperty(channel, 'send', {
					value(data: string | ArrayBuffer) {
						if (typeof data === 'string') send(data);
						else send(new Uint8Array(16 * 1024 + 1).buffer);
					}
				});
				return channel;
			};
		});
		await api.setup(page);
		await api.setup(receiver);
		await open(page, 'ผู้ส่งข้อมูลผิดรูปแบบ');
		await open(receiver, 'ผู้รับ');
		await choose(page, Buffer.from('x'));
		await request(page, 'ผู้รับ');
		await receiver.getByRole('button', { name: 'รับไฟล์', exact: true }).click();
		await expect(
			receiver.getByText('ข้อมูลไฟล์ไม่ครบหรือไม่ถูกต้อง ยกเลิกการรับไฟล์แล้ว กรุณาส่งใหม่', {
				exact: true
			})
		).toBeVisible();
		await expect(receiver.getByRole('button', { name: 'ดาวน์โหลดไฟล์', exact: true })).toHaveCount(
			0
		);
	} finally {
		await receiver.close();
		await api.close();
	}
});
