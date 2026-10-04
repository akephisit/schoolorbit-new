import { expect, test, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import jsQR from 'jsqr';
import { mockStaffHome, id } from './fixtures/staff-home-route-data';

const qrPath = '/staff/tools/qr-code';
const payload = 'https://example.invalid/กิจกรรม?ข้อความ=สวัสดีครู';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});

async function setup(page: Page, userType = 'staff') {
	const api = await mockStaffHome(page, { permissions: [], userType });
	await page.route('**/api/menu/user', (route) =>
		route.fulfill({
			json: {
				success: true,
				data: {
					groups:
						userType === 'staff'
							? [
									{
										code: 'home_main',
										name: 'งานประจำของฉัน',
										icon: 'Inbox',
										workspaceCode: 'home',
										workspaceName: 'หน้าหลักของฉัน',
										workspaceIcon: 'Home',
										workspaceOrder: 10,
										displayOrder: 10,
										items: [
											{
												id: id(30),
												name: 'เครื่องมือ',
												path: '/staff/tools',
												icon: 'Wrench',
												displayOrder: 90,
												requiredPermission: null
											}
										]
									}
								]
							: []
				}
			}
		})
	);
	return api;
}

async function logoBytes(page: Page, type = 'image/png', size = [240, 120]) {
	return Buffer.from(
		await page.evaluate(
			({ mime, width, height }) => {
				const canvas = document.createElement('canvas');
				canvas.width = width;
				canvas.height = height;
				const context = canvas.getContext('2d')!;
				context.fillStyle = '#123c70';
				context.fillRect(0, 0, width, height);
				context.fillStyle = '#ffffff';
				context.fillRect(width * 0.375, height / 6, width / 4, (height * 2) / 3);
				return canvas.toDataURL(mime).split(',')[1];
			},
			{ mime: type, width: size[0], height: size[1] }
		),
		'base64'
	);
}

async function decodeDownload(page: Page, expected: string, pixelSize = 512) {
	const pending = page.waitForEvent('download');
	await page.getByRole('button', { name: 'ดาวน์โหลด PNG', exact: true }).click();
	const file = await pending;
	expect(file.suggestedFilename()).toBe('qr-code.png');
	const filePath = await file.path();
	if (!filePath) throw new Error('Download file was not saved');
	const bytes = await readFile(filePath);
	expect(bytes.subarray(0, 8).toString('hex')).toBe('89504e470d0a1a0a');
	const decoded = await page.evaluate(
		async ({ base64, pixelSize }) => {
			const image = new Image();
			image.src = `data:image/png;base64,${base64}`;
			await image.decode();
			const canvas = document.createElement('canvas');
			canvas.width = canvas.height = pixelSize;
			const context = canvas.getContext('2d')!;
			context.drawImage(image, 0, 0, pixelSize, pixelSize);
			const pixels = context.getImageData(0, 0, pixelSize, pixelSize).data;
			let binary = '';
			for (let offset = 0; offset < pixels.length; offset += 8192)
				binary += String.fromCharCode(...pixels.subarray(offset, offset + 8192));
			const preview = document.querySelector<HTMLImageElement>('img[alt="QR Code ที่สร้าง"]')!;
			const previewBytes = new Uint8Array(await (await fetch(preview.src)).arrayBuffer());
			const source = atob(base64);
			return {
				width: image.naturalWidth,
				height: image.naturalHeight,
				pixels: btoa(binary),
				identical:
					previewBytes.length === source.length &&
					previewBytes.every((byte, index) => byte === source.charCodeAt(index))
			};
		},
		{ base64: bytes.toString('base64'), pixelSize }
	);
	expect(decoded.width).toBe(1024);
	expect(decoded.height).toBe(1024);
	expect(decoded.identical).toBe(true);
	expect(
		jsQR(new Uint8ClampedArray(Buffer.from(decoded.pixels, 'base64')), pixelSize, pixelSize)?.data
	).toBe(expected);
}

test('staff without permissions generates locally and stale downloads are disabled', async ({
	page
}) => {
	const api = await setup(page);
	const qrChunk = JSON.parse(readFileSync('.svelte-kit/output/client/.vite/manifest.json', 'utf8'))[
		'node_modules/qrcode/lib/browser.js'
	].file as string;
	const requests: string[] = [];
	page.on('request', (request) => requests.push(request.url()));
	await page.goto('/staff/tools');
	await expect(page.getByRole('heading', { name: 'เครื่องมือ', exact: true })).toBeVisible();
	await page.getByRole('link', { name: /สร้าง QR Code/ }).click();
	await expect(page.getByRole('heading', { name: 'สร้าง QR Code', exact: true })).toBeVisible();
	expect(requests.filter((url) => /\/api\/school\/(public|settings)/.test(url))).toEqual([]);
	expect(requests.some((url) => url.endsWith(qrChunk))).toBe(false);
	await page.getByLabel('ลิงก์หรือข้อความ').fill(payload);
	await page.getByRole('button', { name: 'สร้าง QR Code', exact: true }).click();
	await expect(page.getByAltText('QR Code ที่สร้าง')).toBeVisible();
	await expect(page.getByAltText('QR Code ที่สร้าง')).toHaveJSProperty('naturalWidth', 1024);
	await decodeDownload(page, payload);
	expect(requests.some((url) => url.endsWith(qrChunk))).toBe(true);
	await page.getByLabel('ลิงก์หรือข้อความ').fill('  ภาษาไทยสำหรับครู  ');
	await expect(page.getByRole('button', { name: 'ดาวน์โหลด PNG' })).toBeDisabled();
	await page.getByRole('button', { name: 'สร้าง QR Code', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ดาวน์โหลด PNG' })).toBeEnabled();
	await decodeDownload(page, '  ภาษาไทยสำหรับครู  ');
	expect(api.writes).toEqual([]);
	expect(requests.some((url) => url.includes(encodeURIComponent(payload)))).toBe(false);
	await page.getByRole('link', { name: 'เครื่องมือทั้งหมด' }).click();
	await expect(page).toHaveURL(/\/staff\/tools$/);
});

for (const userType of ['student', 'parent'])
	for (const route of ['/staff/tools', qrPath]) {
		test(`${userType} cannot open ${route}`, async ({ page }) => {
			await setup(page, userType);
			await page.goto(route);
			await expect(page).toHaveURL(/\/403\?from=/);
			await expect(page.getByRole('link', { name: 'เครื่องมือ', exact: true })).toHaveCount(0);
			await expect(page.getByLabel('ลิงก์หรือข้อความ')).toHaveCount(0);
		});
	}

test('text validation preserves values and rejects whitespace and excessive capacity', async ({
	page
}) => {
	await setup(page);
	await page.goto(qrPath);
	await page.getByLabel('ลิงก์หรือข้อความ').fill(' \n\t ');
	await page.getByRole('button', { name: 'สร้าง QR Code', exact: true }).click();
	await expect(page.getByText('กรุณาใส่ลิงก์หรือข้อความ', { exact: true })).toBeVisible();
	const oversized = 'ก'.repeat(4000);
	await page.getByLabel('ลิงก์หรือข้อความ').fill(oversized);
	await page.getByRole('button', { name: 'สร้าง QR Code', exact: true }).click();
	await expect(page.getByText(/ข้อความยาวเกินความจุ/)).toBeVisible();
	await expect(page.getByLabel('ลิงก์หรือข้อความ')).toHaveValue(oversized);
	await expect(page.getByAltText('QR Code ที่สร้าง')).toHaveCount(0);
});

test('uploads validate size and decoding; PNG, JPEG and WebP produce scannable QR', async ({
	page
}) => {
	const api = await setup(page);
	await page.goto(qrPath);
	await page.getByLabel('ลิงก์หรือข้อความ').fill(payload);
	await page.getByRole('button', { name: 'เลือกภาพจากเครื่อง' }).click();
	const input = page.getByLabel('ไฟล์โลโก้');
	await input.setInputFiles({
		name: 'bad.png',
		mimeType: 'image/png',
		buffer: Buffer.from('broken')
	});
	await expect(page.getByText(/อ่านภาพไม่สำเร็จ/)).toBeVisible();
	await input.setInputFiles({
		name: 'large.png',
		mimeType: 'image/png',
		buffer: Buffer.alloc(5 * 1024 * 1024 + 1)
	});
	await expect(page.getByText('ภาพต้องมีขนาดไม่เกิน 5 MB', { exact: true })).toBeVisible();
	for (const [mimeType, extension] of [
		['image/png', 'png'],
		['image/jpeg', 'jpg'],
		['image/webp', 'webp']
	]) {
		await input.setInputFiles({
			name: `logo.${extension}`,
			mimeType,
			buffer: await logoBytes(page, mimeType)
		});
		await expect(page.getByAltText('โลโก้ที่เลือก')).toBeVisible();
		// Before the first render there is no download control; later edits disable it.
		if (await page.getByRole('button', { name: 'ดาวน์โหลด PNG' }).count())
			await expect(page.getByRole('button', { name: 'ดาวน์โหลด PNG' })).toBeDisabled();
		await page.getByRole('button', { name: 'สร้าง QR Code', exact: true }).click();
		await expect(page.getByRole('button', { name: 'ดาวน์โหลด PNG' })).toBeEnabled();
		await decodeDownload(page, payload);
	}
	expect(api.writes).toEqual([]);
});

test('larger portrait, square and landscape logos remain scannable across QR densities', async ({
	page
}) => {
	await setup(page);
	await page.goto(qrPath);
	await page.getByRole('button', { name: 'เลือกภาพจากเครื่อง' }).click();
	for (const { text, size } of [
		{ text: 'เอก', size: [120, 240] },
		{ text: 'https://www.google.com', size: [120, 240] },
		{ text: 'กิจกรรมสำหรับครูและนักเรียน '.repeat(8), size: [160, 160] },
		{ text: 'ก'.repeat(300), size: [240, 120] }
	]) {
		await page.getByLabel('ลิงก์หรือข้อความ').fill(text);
		await page.getByLabel('ไฟล์โลโก้').setInputFiles({
			name: 'logo.png',
			mimeType: 'image/png',
			buffer: await logoBytes(page, 'image/png', size)
		});
		await expect(page.getByAltText('โลโก้ที่เลือก')).toBeVisible();
		await page.getByRole('button', { name: 'สร้าง QR Code', exact: true }).click();
		await expect(page.getByRole('button', { name: 'ดาวน์โหลด PNG' })).toBeEnabled();
		// Dense symbols need the exported resolution: downsampling blends adjacent modules.
		await decodeDownload(page, text, 1024);
	}
});

test('school logo handles missing logo, delivery failure and retry without settings permission', async ({
	page
}) => {
	await setup(page);
	await page.goto(qrPath);
	const bytes = await logoBytes(page);
	let reads = 0;
	let deliveries = 0;
	await page.route('**/api/school/public', (route) => {
		reads++;
		return route.fulfill({
			json: {
				success: true,
				data: { schoolName: 'โรงเรียนทดสอบ', logoFileId: reads === 1 ? null : id(40) }
			}
		});
	});
	await page.route(`**/api/public/files/${id(40)}/delivery`, (route) => {
		deliveries++;
		return route.fulfill({
			status: deliveries === 1 ? 503 : 200,
			json:
				deliveries === 1
					? { success: false, error: 'โหลดโลโก้ไม่สำเร็จ' }
					: {
							success: true,
							data: { url: new URL('/test-school-logo.png', route.request().url()).href }
						}
		});
	});
	await page.route('**/test-school-logo.png', (route) =>
		route.fulfill({ contentType: 'image/png', body: bytes })
	);
	await page.getByRole('button', { name: 'ใช้โลโก้โรงเรียน' }).click();
	await expect(page.getByText(/โรงเรียนยังไม่ได้ตั้งค่าโลโก้/)).toBeVisible();
	await page.getByRole('button', { name: 'ลองโหลดโลโก้อีกครั้ง' }).click();
	await expect(page.getByText('โหลดโลโก้ไม่สำเร็จ', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองโหลดโลโก้อีกครั้ง' }).click();
	await expect(page.getByAltText('โลโก้ที่เลือก')).toBeVisible();
	await page.getByLabel('ลิงก์หรือข้อความ').fill(payload);
	await page.getByRole('button', { name: 'สร้าง QR Code', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ดาวน์โหลด PNG' })).toBeEnabled();
	await decodeDownload(page, payload);
	expect(reads).toBe(3);
});

test('failed asset fetch shows Thai guidance and retry retains the QR input', async ({ page }) => {
	await setup(page);
	await page.goto(qrPath);
	await page.getByLabel('ลิงก์หรือข้อความ').fill(payload);
	const bytes = await logoBytes(page);
	await page.route('**/api/school/public', (route) =>
		route.fulfill({ json: { success: true, data: { logoFileId: id(40) } } })
	);
	await page.route(`**/api/public/files/${id(40)}/delivery`, (route) =>
		route.fulfill({
			json: {
				success: true,
				data: { url: new URL('/test-network-logo.png', route.request().url()).href }
			}
		})
	);
	let attempts = 0;
	await page.route('**/test-network-logo.png', (route) =>
		++attempts === 1
			? route.abort('failed')
			: route.fulfill({ contentType: 'image/png', body: bytes })
	);
	await page.getByRole('button', { name: 'ใช้โลโก้โรงเรียน' }).click();
	await expect(page.locator('#qr-logo-error')).toContainText('กรุณาตรวจสอบการเชื่อมต่อแล้วลองใหม่');
	await expect(page.locator('#qr-logo-error')).not.toContainText('Failed to fetch');
	await expect(page.getByLabel('ลิงก์หรือข้อความ')).toHaveValue(payload);
	await page.getByRole('button', { name: 'ลองโหลดโลโก้อีกครั้ง' }).click();
	await expect(page.getByAltText('โลโก้ที่เลือก')).toBeVisible();
	await page.getByRole('button', { name: 'สร้าง QR Code', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ดาวน์โหลด PNG' })).toBeEnabled();
	await decodeDownload(page, payload);
});

test('switching source rejects a late school read and allows QR without logo', async ({ page }) => {
	await setup(page);
	await page.goto(qrPath);
	let release = () => {};
	const pending = new Promise<void>((resolve) => {
		release = resolve;
	});
	await page.route('**/api/school/public', async (route) => {
		await pending;
		await route.fulfill({ json: { success: true, data: { logoFileId: null } } });
	});
	await page.getByRole('button', { name: 'ใช้โลโก้โรงเรียน' }).click();
	await expect(page.getByText('กำลังโหลดโลโก้...')).toBeVisible();
	await expect(page.getByRole('button', { name: 'สร้าง QR Code', exact: true })).toBeDisabled();
	await page.getByRole('button', { name: 'ไม่ใส่โลโก้' }).click();
	release();
	await expect(page.getByText('กำลังโหลดโลโก้...')).toHaveCount(0);
	await expect(page.getByRole('alert')).toHaveCount(0);
	await page.getByLabel('ลิงก์หรือข้อความ').fill(payload);
	await page.getByRole('button', { name: 'สร้าง QR Code', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ดาวน์โหลด PNG' })).toBeEnabled();
	await decodeDownload(page, payload);
});

for (const width of [375, 1440])
	for (const theme of ['light', 'dark']) {
		test(`QR layout and keyboard at ${width}px in ${theme}`, async ({ page }) => {
			await setup(page);
			await page.setViewportSize({ width, height: 900 });
			await page.addInitScript(
				(value) => localStorage.setItem('ui-preferences', JSON.stringify({ theme: value })),
				theme
			);
			await page.goto(qrPath);
			await expect(page.getByRole('heading', { name: 'สร้าง QR Code', exact: true })).toBeVisible();
			await expect(page.locator('html')).toHaveClass(theme === 'dark' ? /dark/ : /^(?!.*dark)/);
			await page.getByRole('button', { name: 'สร้าง QR Code', exact: true }).click();
			await expect(page.getByText('กรุณาใส่ลิงก์หรือข้อความ', { exact: true })).toBeVisible();
			await page.screenshot({
				path: `/tmp/schoolorbit-tools-qr-${width}-${theme}-error.png`,
				fullPage: true
			});
			let release = () => {};
			const pending = new Promise<void>((resolve) => {
				release = resolve;
			});
			await page.route('**/api/school/public', async (route) => {
				await pending;
				await route.fulfill({ json: { success: true, data: { logoFileId: null } } });
			});
			await page.getByRole('button', { name: 'ใช้โลโก้โรงเรียน' }).click();
			await expect(page.getByText('กำลังโหลดโลโก้...')).toBeVisible();
			await page.screenshot({
				path: `/tmp/schoolorbit-tools-qr-${width}-${theme}-loading.png`,
				fullPage: true
			});
			await page.getByRole('button', { name: 'ไม่ใส่โลโก้' }).click();
			release();
			await page.getByLabel('ลิงก์หรือข้อความ').focus();
			await page.keyboard.insertText('กิจกรรมโรงเรียน');
			await page.keyboard.press('Tab');
			await expect(page.getByRole('button', { name: 'ไม่ใส่โลโก้' })).toBeFocused();
			await page.keyboard.press('Enter');
			await page.screenshot({
				path: `/tmp/schoolorbit-tools-qr-${width}-${theme}-empty.png`,
				fullPage: true
			});
			await page.getByRole('button', { name: 'สร้าง QR Code', exact: true }).click();
			await expect(page.getByAltText('QR Code ที่สร้าง')).toBeVisible();
			await expect(page.getByAltText('QR Code ที่สร้าง')).toHaveJSProperty('naturalWidth', 1024);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			const form = await page.locator('form').boundingBox();
			const preview = await page.getByRole('region', { name: 'ตัวอย่าง QR Code' }).boundingBox();
			if (!form || !preview) throw new Error('QR layout was not rendered');
			if (width < 1024) expect(preview.y).toBeGreaterThan(form.y + form.height);
			else expect(Math.abs(preview.y - form.y)).toBeLessThan(5);
			await page.screenshot({
				path: `/tmp/schoolorbit-tools-qr-${width}-${theme}.png`,
				fullPage: true
			});
			await page.getByRole('button', { name: 'ดาวน์โหลด PNG' }).scrollIntoViewIfNeeded();
			await page.screenshot({
				path: `/tmp/schoolorbit-tools-qr-${width}-${theme}-download.png`,
				fullPage: true
			});
		});
	}
