import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__school-font-library-test';
const virtualModuleId = 'virtual:school-font-library-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubPrefix = '\0school-font-library-stub:';

test.describe.configure({ mode: 'serial' });

const apiClientStub = `
	export class ApiClientError extends Error {
		constructor(message, status, retryAfterSeconds, data) {
			super(message);
			this.name = 'ApiClientError';
			this.status = status;
			this.retryAfterSeconds = retryAfterSeconds;
			this.data = data;
		}
	}
`;

const schoolFontApiStub = `
	import { ApiClientError } from '$lib/api/client';
	export async function listSchoolFonts() {
		return window.__schoolFontHarnessApi.list();
	}
	export async function inspectSchoolFontUploads(payload) {
		return window.__schoolFontHarnessApi.inspect(payload);
	}
	export async function attachSchoolFontBatch(payload) {
		return window.__schoolFontHarnessApi.attach(payload, ApiClientError);
	}
	export async function deleteSchoolFont(fontId) {
		return window.__schoolFontHarnessApi.delete(fontId, ApiClientError);
	}
`;

const fileApiStub = `
	export async function uploadSchoolFontFile(file, context) {
		return window.__schoolFontHarnessApi.upload(file, context);
	}
	export async function deleteSchoolFontTemporaryFile(fileId, context) {
		return window.__schoolFontHarnessApi.cleanup(fileId, context);
	}
`;

const stubModules = new Map([
	['$lib/api/client', apiClientStub],
	['$lib/api/school-fonts', schoolFontApiStub],
	['$lib/api/files', fileApiStub]
]);

function findStubModule(id: string): string | undefined {
	if (stubModules.has(id)) return id;
	for (const stubId of stubModules.keys()) {
		if (!stubId.startsWith('$lib/')) continue;
		const resolvedPath = path.resolve(frontendRoot, 'src/lib', stubId.slice('$lib/'.length));
		if (id === resolvedPath || id === `${resolvedPath}.ts` || id === `${resolvedPath}.js`) {
			return stubId;
		}
	}
}

function harnessPlugin(): Plugin {
	return {
		name: 'school-font-library-test-harness',
		enforce: 'pre',
		resolveId(id) {
			if (id === virtualModuleId) return resolvedVirtualModuleId;
			const stubId = findStubModule(id);
			if (stubId) return `${stubPrefix}${stubId}`;
		},
		load(id) {
			if (id.startsWith(stubPrefix)) return stubModules.get(id.slice(stubPrefix.length));
			if (id !== resolvedVirtualModuleId) return;
			return `
				import { mount } from 'svelte';
				import '/src/routes/layout.css';
				import SchoolFontLibrary from '/src/lib/components/school-fonts/SchoolFontLibrary.svelte';

				let fonts = [
					{
						id: 'font-used',
						displayName: 'Sarabun Regular',
						fontFamily: 'Sarabun',
						fontWeight: 400,
						fontStyle: 'normal',
						referenceCount: 2,
						createdAt: '2026-08-23T00:00:00Z'
					},
					{
						id: 'font-free',
						displayName: 'Noto Sans Thai Bold',
						fontFamily: 'Noto Sans Thai',
						fontWeight: 700,
						fontStyle: 'normal',
						referenceCount: 0,
						createdAt: '2026-08-23T00:00:00Z'
					}
				];
				const uploadedNames = new Map();
				const uploadAttempts = new Map();
				const cleanupAttempts = new Map();
				const uploadCalls = [];
				const attachedBatches = [];
				const cleanedFiles = [];
				let nextFile = 0;
				let activeUploads = 0;
				let maxActiveUploads = 0;

				window.__schoolFontHarnessApi = {
					async list() {
						return { items: structuredClone(fonts) };
					},
					async upload(file, context) {
						uploadCalls.push({ name: file.name, context: structuredClone(context) });
						activeUploads += 1;
						maxActiveUploads = Math.max(maxActiveUploads, activeUploads);
						await new Promise((resolve) => setTimeout(resolve, 15));
						activeUploads -= 1;
						if (context.type !== 'central' || 'templateId' in context) {
							throw new Error('central upload must omit template relationship');
						}
						const attempts = (uploadAttempts.get(file.name) ?? 0) + 1;
						uploadAttempts.set(file.name, attempts);
						if (file.name.includes('Retry') && attempts === 1) {
							throw new Error('จำลองการอัปโหลดไม่สำเร็จ');
						}
						const id = 'upload-' + (++nextFile);
						uploadedNames.set(id, file.name);
						return { id, displayFilename: file.name, lifecycleStatus: 'ready' };
					},
					async inspect(payload) {
						return {
							files: payload.fileIds.map((fileId) => {
								const filename = uploadedNames.get(fileId);
								return {
									fileId,
									displayFilename: filename,
									fontFamily: filename.includes('Noto') ? 'Noto Sans Thai' : 'Browser Thai',
									fontWeight: filename.includes('Bold') ? 700 : 400,
									fontStyle: filename.includes('Italic') ? 'italic' : 'normal',
									status: filename.includes('Duplicate') ? 'duplicate_existing' : 'ready'
								};
							})
						};
					},
					async attach(payload) {
						if (!payload.rightsConfirmed) throw new Error('rights must be confirmed once');
						attachedBatches.push([...payload.fileIds]);
						const items = payload.fileIds.map((fileId, index) => {
							const filename = uploadedNames.get(fileId);
							return {
								id: 'attached-' + fileId,
								displayName: filename,
								fontFamily: 'Browser Thai',
								fontWeight: filename.includes('Bold') ? 700 : 400,
								fontStyle: filename.includes('Italic') ? 'italic' : 'normal',
								referenceCount: 0,
								createdAt: '2026-08-23T00:00:0' + index + 'Z'
							};
						});
						fonts = [...fonts, ...items];
						return { items: structuredClone(items) };
					},
					async cleanup(fileId, context) {
						if (context.type !== 'central' || 'templateId' in context) {
							throw new Error('central cleanup must omit template relationship');
						}
						const attempts = (cleanupAttempts.get(fileId) ?? 0) + 1;
						cleanupAttempts.set(fileId, attempts);
						if (uploadedNames.get(fileId).includes('Duplicate') && attempts === 1) {
							throw new Error('จำลองการลบไฟล์ชั่วคราวไม่สำเร็จ');
						}
						cleanedFiles.push(fileId);
						return { disposition: 'deleted' };
					},
					async delete(fontId, ApiClientError) {
						if (fontId === 'font-used') {
							throw new ApiClientError('school_font_in_use', 409, undefined, { referenceCount: 3 });
						}
						fonts = fonts.filter((font) => font.id !== fontId);
					}
				};
				window.schoolFontLibraryHarness = {
					uploadCalls: () => structuredClone(uploadCalls),
					maxActiveUploads: () => maxActiveUploads,
					attachedBatches: () => structuredClone(attachedBatches),
					cleanedFiles: () => [...cleanedFiles]
				};
				mount(SchoolFontLibrary, { target: document.getElementById('app') });
			`;
		},
		configureServer(server) {
			server.middlewares.use((request, response, next) => {
				const pathname = new URL(request.url ?? '/', 'http://test').pathname;
				if (pathname !== harnessPath) return next();
				response.setHeader('Content-Type', 'text/html; charset=utf-8');
				response.end(
					`<main id="app"></main><script type="module" src="/@id/${virtualModuleId}"></script>`
				);
			});
		}
	};
}

let devServer: ViteDevServer;
let baseUrl: string;

test.beforeAll(async () => {
	devServer = await createServer({
		root: frontendRoot,
		cacheDir: path.resolve(frontendRoot, 'node_modules/.vite-school-font-library-test'),
		logLevel: 'error',
		plugins: [harnessPlugin()],
		server: { host: '127.0.0.1', port: 0 }
	});
	await devServer.listen();
	const address = devServer.httpServer?.address();
	if (!address || typeof address === 'string') throw new Error('Vite test server did not start');
	baseUrl = `http://127.0.0.1:${address.port}`;
});

test.afterAll(async () => {
	await devServer.close();
});

async function openHarness(page) {
	const moduleResponse = page.waitForResponse((response) =>
		response.url().includes('/@id/virtual:school-font-library-test')
	);
	await page.goto(`${baseUrl}${harnessPath}`);
	const response = await moduleResponse;
	expect(response.status(), `font library harness failed:\n${await response.text()}`).toBeLessThan(
		400
	);
	await expect(page.getByTestId('school-font-library')).toBeVisible();
}

test('uploads sequentially, reviews once, and patches the shared library atomically', async ({
	page
}) => {
	await openHarness(page);
	await page.getByLabel('ไฟล์ฟอนต์').setInputFiles([
		{ name: 'Browser-Regular.ttf', mimeType: 'font/ttf', buffer: Buffer.from('font') },
		{ name: 'Browser-Bold.ttf', mimeType: 'font/ttf', buffer: Buffer.from('font') }
	]);
	await page.getByRole('button', { name: 'อัปโหลดและตรวจฟอนต์' }).click();
	await expect(page.getByText('พร้อมเพิ่มเข้าคลัง')).toHaveCount(2);
	expect(await page.evaluate(() => window.schoolFontLibraryHarness.maxActiveUploads())).toBe(1);
	const uploadCalls = await page.evaluate(() => window.schoolFontLibraryHarness.uploadCalls());
	expect(uploadCalls.map((call) => call.context)).toEqual([
		{ type: 'central' },
		{ type: 'central' }
	]);

	await page.getByRole('checkbox', { name: /ยืนยันว่ามีสิทธิ์ใช้ฟอนต์/ }).check();
	await page.getByRole('button', { name: 'เพิ่มเข้าคลังฟอนต์' }).click();
	await expect(page.getByText('Browser-Regular.ttf')).toBeVisible();
	await expect(page.getByText('Browser-Bold.ttf')).toBeVisible();
	expect(await page.evaluate(() => window.schoolFontLibraryHarness.attachedBatches())).toHaveLength(
		1
	);
});

test('supports upload retry and retryable temporary cleanup after duplicate inspection', async ({
	page
}) => {
	await openHarness(page);
	await page.getByLabel('ไฟล์ฟอนต์').setInputFiles({
		name: 'Retry-Regular.ttf',
		mimeType: 'font/ttf',
		buffer: Buffer.from('font')
	});
	await page.getByRole('button', { name: 'อัปโหลดและตรวจฟอนต์' }).click();
	await expect(page.getByText('จำลองการอัปโหลดไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอัปโหลด Retry-Regular.ttf อีกครั้ง' }).click();
	await expect(page.getByText('พร้อมเพิ่มเข้าคลัง')).toBeVisible();
	await page.getByRole('button', { name: 'ลบไฟล์ชั่วคราว Retry-Regular.ttf' }).click();
	await expect(page.getByText('Retry-Regular.ttf')).toHaveCount(0);

	await page.getByLabel('ไฟล์ฟอนต์').setInputFiles({
		name: 'Duplicate-Regular.ttf',
		mimeType: 'font/ttf',
		buffer: Buffer.from('font')
	});
	await page.getByRole('button', { name: 'อัปโหลดและตรวจฟอนต์' }).click();
	await expect(page.getByText('มี variant นี้ในคลังแล้ว')).toBeVisible();
	const cleanup = page.getByRole('button', {
		name: 'ลบไฟล์ชั่วคราว Duplicate-Regular.ttf'
	});
	await cleanup.click();
	await expect(page.getByText('จำลองการลบไฟล์ชั่วคราวไม่สำเร็จ')).toBeVisible();
	await cleanup.click();
	await expect(page.getByText('Duplicate-Regular.ttf')).toHaveCount(0);
	expect(await page.evaluate(() => window.schoolFontLibraryHarness.cleanedFiles())).toHaveLength(2);
});

test('keeps an in-use font with the authoritative count and removes an unreferenced font', async ({
	page
}) => {
	await openHarness(page);
	await page.getByRole('button', { name: 'ลบฟอนต์ Sarabun Regular' }).click();
	await page.getByRole('button', { name: 'ยืนยันลบฟอนต์' }).click();
	await expect(page.getByText('ฟอนต์นี้ยังถูกใช้ใน 3 แม่แบบ')).toBeVisible();
	await expect(page.getByText('Sarabun Regular')).toBeVisible();

	await page.getByRole('button', { name: 'ลบฟอนต์ Noto Sans Thai Bold' }).click();
	await page.getByRole('button', { name: 'ยืนยันลบฟอนต์' }).click();
	await expect(page.getByText('Noto Sans Thai Bold')).toHaveCount(0);
});
