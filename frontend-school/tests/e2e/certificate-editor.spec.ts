import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__certificate-editor-test';
const virtualModuleId = 'virtual:certificate-editor-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubPrefix = '\0certificate-editor-stub:';

const apiClientStub = `
	export class ApiClientError extends Error {
		constructor(message, status) {
			super(message);
			this.name = 'ApiClientError';
			this.status = status;
		}
	}
`;

const certificateApiStub = `
	import { ApiClientError } from '$lib/api/client';
	export async function updateCertificateTemplate(id, payload) {
		return window.__certificateEditorApi.update(id, payload, ApiClientError);
	}
	export async function getCertificateTemplate(id) {
		return window.__certificateEditorApi.getTemplate(id);
	}
	export async function getCertificateTemplateVariableCatalog(id) {
		return window.__certificateEditorApi.getVariables(id);
	}
	export async function createCertificateTemplatePreviewManifest(id, payload) {
		return window.__certificateEditorApi.preview(id, payload);
	}
	export async function attachCertificateTemplateBackground(id, payload) {
		return window.__certificateEditorApi.attachBackground(id, payload);
	}
`;

const fileApiStub = `
	export async function uploadCertificateTemplateFile() {
		throw new Error('upload is not used by this harness');
	}
	export async function deleteFile() {
		return { disposition: 'deleted' };
	}
`;

const rendererStub = `
	export async function loadCertificateRenderer() {
		return {
			async inspectBackgroundPdf() {
				return {
					mediaBox: { xPoints: 0, yPoints: 0, widthPoints: 800, heightPoints: 400 },
					cropBox: { xPoints: 0, yPoints: 0, widthPoints: 800, heightPoints: 400 },
					rotation: 0,
					displayedWidthPoints: 800,
					displayedHeightPoints: 400,
					paperLabel: 'ขนาดพื้นหลังใหม่'
				};
			},
			async renderPreview(manifest, canvas, options = {}) {
				options.signal?.throwIfAborted();
				const scale = options.scale ?? 1;
				canvas.width = Math.max(1, Math.round(manifest.pageGeometry.displayedWidthPoints * scale));
				canvas.height = Math.max(1, Math.round(manifest.pageGeometry.displayedHeightPoints * scale));
				const context = canvas.getContext('2d');
				context.fillStyle = '#fffdf5';
				context.fillRect(0, 0, canvas.width, canvas.height);
				context.strokeStyle = '#c8a96b';
				context.lineWidth = Math.max(1, scale * 2);
				context.strokeRect(8 * scale, 8 * scale, canvas.width - 16 * scale, canvas.height - 16 * scale);
				window.__certificateEditorRendererCalls += 1;
				return {
					widthPoints: manifest.pageGeometry.displayedWidthPoints,
					heightPoints: manifest.pageGeometry.displayedHeightPoints,
					widthPixels: canvas.width,
					heightPixels: canvas.height
				};
			},
			async buildCertificatePdf() {
				return new Uint8Array([37, 80, 68, 70]);
			}
		};
	}
`;

const stubModules = new Map([
	[
		'$app/navigation',
		'export const beforeNavigate = (callback) => { window.__beforeEditorNavigate = callback; };'
	],
	[
		'$app/paths',
		"export const base = ''; export const assets = ''; export const resolve = (value) => value;"
	],
	['$env/dynamic/public', 'export const env = {};'],
	['$env/static/public', "export const PUBLIC_BACKEND_URL = 'https://school-api.schoolorbit.app';"],
	['$lib/api/client', apiClientStub],
	['$lib/api/certificates', certificateApiStub],
	['$lib/api/files', fileApiStub],
	['$lib/certificates/renderer', rendererStub]
]);

function findStubModule(id: string): string | undefined {
	if (stubModules.has(id)) return id;
	for (const stubId of stubModules.keys()) {
		if (!stubId.startsWith('$lib/')) continue;
		const resolvedPath = path.resolve(frontendRoot, 'src/lib', stubId.slice('$lib/'.length));
		if (
			id === resolvedPath ||
			id === `${resolvedPath}.ts` ||
			id === `${resolvedPath}.js` ||
			id === `${resolvedPath}.svelte`
		) {
			return stubId;
		}
	}
}

function harnessPlugin(): Plugin {
	return {
		name: 'certificate-editor-test-harness',
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
				import CertificateEditor from '/src/lib/components/certificates/editor/CertificateEditor.svelte';
				let certificateNow = Date.parse('2026-08-14T00:00:00Z');
				Date.now = () => certificateNow;

				const geometry = {
					mediaBox: { xPoints: 0, yPoints: 0, widthPoints: 600, heightPoints: 400 },
					cropBox: { xPoints: 0, yPoints: 0, widthPoints: 600, heightPoints: 400 },
					rotation: 0,
					displayedWidthPoints: 600,
					displayedHeightPoints: 400,
					paperLabel: 'ขนาดทดสอบ'
				};
				const initialLayout = {
					schemaVersion: 1,
					elements: [{
						type: 'text',
						id: '20000000-0000-4000-8000-000000000001',
						content: 'มอบให้ {ชื่อ} {นามสกุล}',
						frame: { x: 120, y: 170, width: 360, height: 60 },
						rotation: 0,
						fontSource: { type: 'built_in' },
						fontFamily: 'Sarabun',
						fontWeight: 700,
						fontStyle: 'normal',
						fontSize: 30,
						minFontSize: 14,
						color: '#183153',
						alignment: 'center',
						lineHeight: 1.2,
						autoShrink: true,
						shadow: null
					}]
				};
				const initialUpdatedAt = '2026-08-14T00:00:00Z';
				let serverTemplate = {
					id: '10000000-0000-4000-8000-000000000001',
					campaignId: '30000000-0000-4000-8000-000000000001',
					name: 'การแข่งขันคำคม',
					allowedRecipientTypes: ['student', 'external'],
					backgroundFileId: '40000000-0000-4000-8000-000000000001',
					assets: [],
					capabilities: { canRead: true, canUpdate: true, canDelete: true, canPreview: true },
					createdAt: initialUpdatedAt,
					updatedAt: initialUpdatedAt,
					isActive: true,
					isReady: true,
					issuedCertificateCount: 0,
					missingVariableCertificateCount: 0,
					layout: structuredClone(initialLayout),
					pageGeometry: geometry,
					safeMarginPoints: 28.3464567,
					showSafeArea: true
				};
				const baseManifest = {
					templateId: serverTemplate.id,
					pageGeometry: geometry,
					layout: structuredClone(initialLayout),
					campaignValues: {
						academicYear: '2569',
						campaignName: 'กิจกรรมวันภาษาไทย',
						eventDate: '2026-07-29',
						issueDate: '2026-08-14',
						schoolName: 'โรงเรียนตัวอย่าง',
						ownerOrganizationUnitName: 'กลุ่มสาระภาษาไทย'
					},
					recipientValues: { ชื่อ: 'กมลชนก', นามสกุล: 'รัตนสุวรรณ' },
					certificateNumber: '2569-0001-000001-5',
					qrPayload: 'https://verify.example.test/c/test-proof',
					builtInFonts: [],
					fontGrants: [],
					imageGrants: [],
					backgroundGrant: {
						fileId: serverTemplate.backgroundFileId,
						url: '/background.pdf',
						expiresAt: '2026-08-14T00:00:20Z'
					},
					suggestedFilename: 'ตัวอย่าง.pdf'
				};
				const savedPayloads = [];
				const previewKinds = [];
				const previewPayloads = [];
				let conflictNextSave = false;
				let heldSavePromise = null;
				let releaseHeldSave = null;
				window.__certificateEditorRendererCalls = 0;
				window.__certificateEditorApi = {
					async update(id, payload, ApiClientError) {
						if (heldSavePromise) {
							await heldSavePromise;
							heldSavePromise = null;
							releaseHeldSave = null;
						}
						if (conflictNextSave) {
							conflictNextSave = false;
							throw new ApiClientError('แม่แบบถูกแก้ไขแล้ว กรุณาโหลดข้อมูลล่าสุด', 409);
						}
						savedPayloads.push(structuredClone(payload));
						serverTemplate = {
							...serverTemplate,
							layout: structuredClone(payload.layout),
							safeMarginPoints: payload.safeMarginPoints,
							showSafeArea: payload.showSafeArea,
							updatedAt: '2026-08-14T00:00:01Z'
						};
						return structuredClone(serverTemplate);
					},
					async getTemplate() { return structuredClone(serverTemplate); },
					async getVariables() { return { variables: ['ชื่อ', 'นามสกุล', 'รางวัลหรือบทบาท'] }; },
					async preview(id, payload) {
						previewKinds.push(payload.previewKind);
						previewPayloads.push(structuredClone(payload));
						return {
							...structuredClone(baseManifest),
							layout: structuredClone(payload.layout ?? serverTemplate.layout),
							backgroundGrant: {
								...structuredClone(baseManifest.backgroundGrant),
								expiresAt: new Date(Date.now() + 120_000).toISOString()
							},
							recipientValues: payload.previewKind === 'long'
								? { ชื่อ: 'ณัฏฐณิชาภัทรวรรณ', นามสกุล: 'รัตนสุวรรณกุลชัยวัฒนา' }
								: baseManifest.recipientValues
						};
					},
					async attachBackground() { return structuredClone(serverTemplate); }
				};
				window.certificateEditorHarness = {
					setConflictNextSave() { conflictNextSave = true; },
					holdNextSave() {
						heldSavePromise = new Promise((resolve) => { releaseHeldSave = resolve; });
					},
					releaseSave() { releaseHeldSave?.(); },
					savedPayloads() { return structuredClone(savedPayloads); },
					previewKinds() { return [...previewKinds]; },
					previewPayloads() { return structuredClone(previewPayloads); },
					advanceClock(milliseconds) { certificateNow += milliseconds; },
					rendererCalls() { return window.__certificateEditorRendererCalls; }
				};

				mount(CertificateEditor, {
					target: document.getElementById('app'),
					props: {
						template: structuredClone(serverTemplate),
						initialManifest: structuredClone(baseManifest),
						variables: ['ชื่อ', 'นามสกุล', 'รางวัลหรือบทบาท']
					}
				});
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
		cacheDir: path.resolve(frontendRoot, 'node_modules/.vite-certificate-editor-test'),
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

test('editor adds, moves, duplicates, saves, previews, and resolves conflicts explicitly', async ({
	page
}) => {
	const harnessModuleResponse = page.waitForResponse((response) =>
		response.url().includes('/@id/virtual:certificate-editor-test')
	);
	await page.goto(`${baseUrl}${harnessPath}`);
	const moduleResponse = await harnessModuleResponse;
	expect(
		moduleResponse.status(),
		`virtual editor harness failed to load:\n${await moduleResponse.text()}`
	).toBeLessThan(400);
	await expect(page.getByTestId('certificate-editor')).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => window.certificateEditorHarness.previewPayloads().length))
		.toBeGreaterThan(0);
	const textElements = page.getByRole('button', { name: 'เลือกองค์ประกอบ text' });
	await expect(textElements).toHaveCount(1);

	await page.getByRole('button', { name: 'เพิ่มข้อความ' }).click();
	await expect(textElements).toHaveCount(2);
	await textElements.nth(1).click();
	const elementFrame = textElements.nth(1).locator('..');
	const leftBefore = await elementFrame.evaluate((element) => (element as HTMLElement).style.left);
	await page.keyboard.press('ArrowRight');
	await expect
		.poll(() => elementFrame.evaluate((element) => (element as HTMLElement).style.left))
		.not.toBe(leftBefore);

	await page.getByRole('button', { name: 'ทำสำเนา' }).first().click();
	await expect(textElements).toHaveCount(3);
	await page.evaluate(() => window.certificateEditorHarness.holdNextSave());
	await page.getByRole('button', { name: 'บันทึก' }).click();
	await expect(page.getByRole('button', { name: 'พื้นที่ปลอดภัย' })).toBeDisabled();
	await expect(page.getByRole('button', { name: 'เปลี่ยนพื้นหลัง' })).toBeDisabled();
	await page.evaluate(() => window.certificateEditorHarness.releaseSave());
	await expect
		.poll(() => page.evaluate(() => window.certificateEditorHarness.savedPayloads().length))
		.toBe(1);
	const firstSave = await page.evaluate(() => window.certificateEditorHarness.savedPayloads()[0]);
	expect(firstSave.expectedUpdatedAt).toBe('2026-08-14T00:00:00Z');
	expect(firstSave.layout.elements).toHaveLength(3);

	await page.evaluate(() => window.certificateEditorHarness.setConflictNextSave());
	await page.getByRole('button', { name: 'เพิ่มข้อความ' }).click();
	await expect(textElements).toHaveCount(4);
	await page.getByRole('button', { name: 'บันทึก' }).click();
	await expect(page.getByText('สำเนาบนระบบเปลี่ยนแล้ว:')).toBeVisible();
	await expect(textElements).toHaveCount(4);
	await page.getByRole('button', { name: 'โหลดสำเนาระบบ' }).click();
	await expect(textElements).toHaveCount(3);

	await page.getByRole('button', { name: 'เพิ่มข้อความ' }).click();
	await expect(textElements).toHaveCount(4);
	await expect(page.getByRole('button', { name: 'เปลี่ยนพื้นหลัง' })).toBeDisabled();
	await page.getByRole('button', { name: 'ชื่อยาว' }).click();
	await expect(page.getByRole('heading', { name: 'พรีวิว PDF จริง' })).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => window.certificateEditorHarness.previewKinds()))
		.toContain('long');
	await expect
		.poll(() => page.evaluate(() => window.certificateEditorHarness.rendererCalls()))
		.toBeGreaterThan(1);
	const previewPayload = await page.evaluate(
		() => window.certificateEditorHarness.previewPayloads().at(-1)!
	);
	expect(previewPayload.layout.elements).toHaveLength(4);
	await page.keyboard.press('Escape');

	await page.getByRole('button', { name: 'บันทึก' }).click();
	await expect(page.getByRole('button', { name: 'เปลี่ยนพื้นหลัง' })).toBeEnabled();
	await page.evaluate(() => window.certificateEditorHarness.advanceClock(100_000));
	await page.getByRole('button', { name: 'เปลี่ยนพื้นหลัง' }).click();
	await expect(page.getByRole('heading', { name: 'เปลี่ยน PDF พื้นหลัง' })).toBeVisible();
	await expect(page.getByText('ปรับตามสัดส่วน', { exact: true })).toBeVisible();
	await expect(page.getByText('เริ่มจัดวางใหม่', { exact: true })).toBeVisible();
	const previewCountBeforeReplacement = await page.evaluate(
		() => window.certificateEditorHarness.previewPayloads().length
	);
	await page.getByLabel('เลือก PDF ใหม่').setInputFiles({
		name: 'background-new.pdf',
		mimeType: 'application/pdf',
		buffer: Buffer.from('%PDF')
	});
	await expect
		.poll(() => page.evaluate(() => window.certificateEditorHarness.previewPayloads().length))
		.toBeGreaterThan(previewCountBeforeReplacement);
	await expect(page.getByText(/ขนาดหรือการหมุนของหน้าเปลี่ยนแล้ว/)).toBeVisible();
	await page.locator('[id^="geometry-action-"]').click();
	await page.getByRole('option', { name: 'ปรับองค์ประกอบตามสัดส่วนหน้าใหม่' }).click();
	const confirmation = page.getByRole('checkbox', {
		name: 'ตรวจผลพรีวิวจริงและยืนยันวิธีจัดวางแล้ว'
	});
	await expect(confirmation).toBeEnabled();
	await confirmation.check();
	await expect(page.getByRole('button', { name: 'เปลี่ยนพื้นหลัง' }).last()).toBeEnabled();
});

declare global {
	interface Window {
		certificateEditorHarness: {
			setConflictNextSave(): void;
			holdNextSave(): void;
			releaseSave(): void;
			savedPayloads(): Array<{
				expectedUpdatedAt: string;
				layout: { elements: unknown[] };
			}>;
			previewKinds(): string[];
			previewPayloads(): Array<{ previewKind: string; layout: { elements: unknown[] } }>;
			advanceClock(milliseconds: number): void;
			rendererCalls(): number;
		};
	}
}
