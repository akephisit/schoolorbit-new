import { test, expect } from '@playwright/test';
import { PDFDocument } from 'pdf-lib';
import { mockCorrespondence, detailPath } from './fixtures/correspondence';

test.use({ serviceWorkers: 'block' });
const image = {
	name: 'local-signature.png',
	mimeType: 'image/png',
	buffer: Buffer.from(
		'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a7y0AAAAASUVORK5CYII=',
		'base64'
	)
};

test('signature selection, placement and cancellation stay local', async ({ page }) => {
	const errors: string[] = [];
	await page.addInitScript(() => {
		const state = window as typeof window & { revokedDraftFiles: string[] };
		state.revokedDraftFiles = [];
		const revoke = URL.revokeObjectURL.bind(URL);
		URL.revokeObjectURL = (url) => {
			state.revokedDraftFiles.push(url);
			revoke(url);
		};
	});
	page.on('pageerror', (error) => errors.push(error.message));
	const api = await mockCorrespondence(page);
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
	await page.getByRole('button', { name: 'เลือกลายมือชื่อที่บันทึก' }).click();
	await page.getByLabel('เพิ่มภาพลายมือชื่อ', { exact: true }).setInputFiles(image);
	await page.getByLabel('เพิ่มภาพลายมือชื่อ', { exact: true }).setInputFiles(image);
	await expect(
		page.getByRole('img', { name: 'ภาพลายมือชื่อที่เลือก', exact: true })
	).toHaveAttribute('src', /^blob:/);
	await expect(page.getByTestId('signature-placement').locator('img')).toHaveAttribute(
		'src',
		/^blob:/
	);
	expect(api.uploads).toHaveLength(0);
	expect(api.assetWrites).toHaveLength(0);
	const localUrls = [
		await page.getByRole('img', { name: 'ภาพลายมือชื่อที่เลือก', exact: true }).getAttribute('src'),
		await page.getByTestId('signature-placement').locator('img').getAttribute('src')
	];
	await page
		.getByTestId('correspondence-action')
		.getByRole('button', { name: 'ยกเลิก', exact: true })
		.click();
	await expect(page.getByTestId('correspondence-action')).not.toBeVisible();
	expect(api.uploads).toHaveLength(0);
	expect(api.assetWrites).toHaveLength(0);
	await expect
		.poll(() =>
			page.evaluate((urls) => {
				const revoked = (window as typeof window & { revokedDraftFiles: string[] })
					.revokedDraftFiles;
				return urls.every((url) => url !== null && revoked.includes(url));
			}, localUrls)
		)
		.toBe(true);
	expect(errors).toEqual([]);
});

test('explicit signature confirmation uploads once and reuses the known asset after action failure', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { failAction: 503 });
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
	await page.getByRole('button', { name: 'เลือกลายมือชื่อที่บันทึก' }).click();
	await page.getByLabel('เพิ่มภาพลายมือชื่อ', { exact: true }).setInputFiles(image);
	await expect(page.getByRole('img', { name: 'ภาพลายมือชื่อที่เลือก', exact: true })).toBeVisible();
	expect(api.uploads).toHaveLength(0);
	await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
	await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
	await expect(page.getByTestId('correspondence-action')).toContainText(
		'บริการไม่พร้อม ลองอีกครั้ง'
	);
	expect(api.uploads).toHaveLength(1);
	expect(api.assetWrites).toHaveLength(1);
	await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
	await expect(page.getByText('หลักฐานลงนาม', { exact: true })).toBeVisible();
	expect(api.uploads).toHaveLength(1);
	expect(api.assetWrites).toHaveLength(1);
});

test('new correspondence previews a local PDF before a draft or upload exists', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft' });
	await page.goto('/staff/correspondence/new');
	const pdf = await PDFDocument.create();
	pdf.addPage([300, 400]);
	await page.getByLabel('เอกสารต้นฉบับ PDF หรือภาพ', { exact: true }).setInputFiles({
		name: 'local-original.pdf',
		mimeType: 'application/pdf',
		buffer: Buffer.from(await pdf.save())
	});
	const preview = page.getByRole('region', { name: 'ตัวอย่าง local-original.pdf' });
	await preview.getByRole('button', { name: 'ดูตัวอย่างก่อนบันทึก' }).click();
	await expect(preview.locator('canvas')).toHaveAttribute('width', /[1-9]\d*/);
	expect(api.uploads).toHaveLength(0);
	expect(api.writes).toHaveLength(0);
	await preview.getByRole('button', { name: 'ปิดตัวอย่าง' }).click();
	expect(api.uploads).toHaveLength(0);
});
