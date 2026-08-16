import { expect, test } from '@playwright/test';
import { degrees, PDFDocument, rgb } from 'pdf-lib';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__certificate-renderer-test';
const virtualModuleId = 'virtual:certificate-renderer-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubModulePrefix = '\0certificate-renderer-test-stub:';
const stubModules = new Map([
	[
		'$app/environment',
		'export const browser = true; export const building = false; export const dev = true;'
	],
	[
		'$app/paths',
		"export const base = ''; export const assets = ''; export const resolve = (path) => path;"
	],
	['$env/dynamic/public', 'export const env = {};'],
	['$env/static/public', "export const PUBLIC_BACKEND_URL = 'https://school-api.schoolorbit.app';"]
]);
const backgroundFiles = new Map<string, Uint8Array>();

test.describe.configure({ mode: 'serial' });

type PageGeometry = {
	mediaBox: { xPoints: number; yPoints: number; widthPoints: number; heightPoints: number };
	cropBox: { xPoints: number; yPoints: number; widthPoints: number; heightPoints: number };
	rotation: number;
	displayedWidthPoints: number;
	displayedHeightPoints: number;
	paperLabel: string;
};

type RenderManifest = {
	templateId: string;
	pageGeometry: PageGeometry;
	layout: {
		schemaVersion: number;
		elements: Array<Record<string, unknown>>;
	};
	campaignValues: {
		academicYear: string;
		campaignName: string;
		eventDate: string;
		issueDate: string;
		schoolName: string;
		ownerOrganizationUnitName: string;
	};
	recipientValues: Record<string, string>;
	certificateNumber: string;
	qrPayload: string;
	builtInFonts: Array<{ family: string; weight: number; assetPath: string }>;
	fontGrants: Array<{
		assetId: string;
		fileId: string;
		url: string;
		expiresAt: string;
		family: string;
		weight: number;
	}>;
	imageGrants: never[];
	backgroundGrant: { fileId: string; url: string; expiresAt: string };
	suggestedFilename: string;
};

function harnessPlugin(): Plugin {
	return {
		name: 'certificate-renderer-test-harness',
		enforce: 'pre',
		resolveId(id) {
			if (id === virtualModuleId) return resolvedVirtualModuleId;
			if (stubModules.has(id)) return `${stubModulePrefix}${id}`;
		},
		load(id) {
			if (id.startsWith(stubModulePrefix)) {
				return stubModules.get(id.slice(stubModulePrefix.length));
			}
			if (id !== resolvedVirtualModuleId) return;
			return `
				import { loadCertificateRenderer } from '/src/lib/certificates/renderer';
				import { getDocument, GlobalWorkerOptions } from 'pdfjs-dist';
				import pdfWorkerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
				GlobalWorkerOptions.workerSrc = pdfWorkerUrl;
				const rendererPromise = loadCertificateRenderer();

				function pixelDifferenceRatio(left, right) {
					if (left.length !== right.length) return 1;
					let different = 0;
					for (let index = 0; index < left.length; index += 4) {
						const delta = Math.max(
							Math.abs(left[index] - right[index]),
							Math.abs(left[index + 1] - right[index + 1]),
							Math.abs(left[index + 2] - right[index + 2]),
							Math.abs(left[index + 3] - right[index + 3])
						);
						if (delta > 12) different += 1;
					}
					return different / (left.length / 4);
				}

				async function rasterizePdf(bytes, pageNumber = 1, scale = 1) {
					const task = getDocument({ data: bytes });
					const pdfDocument = await task.promise;
					try {
						const page = await pdfDocument.getPage(pageNumber);
						const viewport = page.getViewport({ scale });
						const canvas = window.document.createElement('canvas');
					canvas.width = Math.round(viewport.width);
					canvas.height = Math.round(viewport.height);
						const context = canvas.getContext('2d', { alpha: false });
						await page.render({ canvasContext: context, canvas, viewport }).promise;
						return {
							pixels: context.getImageData(0, 0, canvas.width, canvas.height).data,
							width: viewport.width,
							height: viewport.height,
							canvasWidth: canvas.width,
							canvasHeight: canvas.height
						};
					} finally {
						await task.destroy();
					}
				}

				window.certificateRendererHarness = {
					async compare(manifest) {
						const renderer = await rendererPromise;
						const preview = document.createElement('canvas');
						await renderer.renderPreview(manifest, preview, { scale: 1 });
						const previewContext = preview.getContext('2d', { alpha: false });
						const previewPixels = previewContext.getImageData(
							0,
							0,
							preview.width,
							preview.height
						).data;
						const pdf = await renderer.buildCertificatePdf([manifest]);
						const exported = await rasterizePdf(pdf);
						return {
							differenceRatio: pixelDifferenceRatio(previewPixels, exported.pixels),
							previewWidth: preview.width,
							previewHeight: preview.height,
							exportedWidth: exported.width,
							exportedHeight: exported.height
						};
					},
					async pageSizes(manifests) {
						const renderer = await rendererPromise;
						const bytes = await renderer.buildCertificatePdf(manifests);
						const task = getDocument({ data: bytes });
						const pdfDocument = await task.promise;
						try {
							const sizes = [];
							for (let pageNumber = 1; pageNumber <= pdfDocument.numPages; pageNumber += 1) {
								const page = await pdfDocument.getPage(pageNumber);
								const viewport = page.getViewport({ scale: 1 });
								sizes.push({
									width: Math.round(viewport.width * 100) / 100,
									height: Math.round(viewport.height * 100) / 100
								});
							}
							return sizes;
						} finally {
							await task.destroy();
						}
					},
					async compareBackground(manifest) {
						const renderer = await rendererPromise;
						const sourceResponse = await fetch(manifest.backgroundGrant.url);
						const sourceBytes = new Uint8Array(await sourceResponse.arrayBuffer());
						const exportedBytes = await renderer.buildCertificatePdf([manifest]);
						const [source, exported] = await Promise.all([
							rasterizePdf(sourceBytes),
							rasterizePdf(exportedBytes)
						]);
						return {
							differenceRatio: pixelDifferenceRatio(source.pixels, exported.pixels),
							sourceWidth: source.width,
							sourceHeight: source.height,
							exportedWidth: exported.width,
							exportedHeight: exported.height
						};
					},
					async raceAbortedFontPreviews(manifest) {
						const renderer = await rendererPromise;
						const nativeFetch = window.fetch.bind(window);
						const [backgroundBytes, fontBytes] = await Promise.all([
							nativeFetch(manifest.backgroundGrant.url).then((response) => response.arrayBuffer()),
							nativeFetch('/fonts/Sarabun-Regular.ttf').then((response) => response.arrayBuffer())
						]);
						let fontRequestCount = 0;
						let firstFontStarted;
						let secondFontStarted;
						const firstStarted = new Promise((resolve) => (firstFontStarted = resolve));
						const secondStarted = new Promise((resolve) => (secondFontStarted = resolve));
						window.fetch = (input, init) => {
							const url = String(input);
							if (url === manifest.backgroundGrant.url) {
								return Promise.resolve(
									new Response(backgroundBytes.slice(0), {
										status: 200,
										headers: { 'Content-Type': 'application/pdf' }
									})
								);
							}
							if (url.endsWith('/fonts/abort-race-sarabun.ttf')) {
								fontRequestCount += 1;
								if (fontRequestCount === 1) {
									firstFontStarted();
									return new Promise((resolve, reject) => {
										const signal = init?.signal;
										const abort = () => reject(signal?.reason ?? new DOMException('Aborted', 'AbortError'));
										if (signal?.aborted) abort();
										else signal?.addEventListener('abort', abort, { once: true });
									});
								}
								secondFontStarted();
								return Promise.resolve(
									new Response(fontBytes.slice(0), {
										status: 200,
										headers: { 'Content-Type': 'font/ttf' }
									})
								);
							}
							return nativeFetch(input, init);
						};

						const firstController = new AbortController();
						try {
							const first = renderer.renderPreview(manifest, document.createElement('canvas'), {
								signal: firstController.signal
							});
							await firstStarted;
							const second = renderer.renderPreview(manifest, document.createElement('canvas'));
							await Promise.race([
								secondStarted,
								new Promise((resolve) => window.setTimeout(resolve, 500))
							]);
							firstController.abort();
							const [firstResult, secondResult] = await Promise.allSettled([first, second]);
							return {
								firstStatus: firstResult.status,
								secondStatus: secondResult.status,
								fontRequestCount
							};
						} finally {
							window.fetch = nativeFetch;
						}
					},
					async countFontRequestsAcrossSignedUrls(first, second) {
						const renderer = await rendererPromise;
						const nativeFetch = window.fetch.bind(window);
						let fontRequestCount = 0;
						window.fetch = (input, init) => {
							if (String(input).includes('/fonts/Sarabun-Regular.ttf?signature=')) {
								fontRequestCount += 1;
							}
							return nativeFetch(input, init);
						};
						try {
							await renderer.renderPreview(first, document.createElement('canvas'));
							await renderer.renderPreview(second, document.createElement('canvas'));
							return fontRequestCount;
						} finally {
							window.fetch = nativeFetch;
						}
					},
					async inspectBackground(url) {
						const renderer = await rendererPromise;
						const response = await fetch(url);
						return renderer.inspectBackgroundPdf(await response.blob());
					}
				};
			`;
		},
		configureServer(server) {
			server.middlewares.use((request, response, next) => {
				const pathname = new URL(request.url ?? '/', 'http://test').pathname;
				if (pathname === harnessPath) {
					response.setHeader('Content-Type', 'text/html; charset=utf-8');
					response.end(
						`<canvas id="app"></canvas><script type="module" src="/@id/${virtualModuleId}"></script>`
					);
					return;
				}
				const file = backgroundFiles.get(pathname);
				if (!file) return next();
				response.setHeader('Content-Type', 'application/pdf');
				response.setHeader('Cache-Control', 'no-store');
				response.end(file);
			});
		}
	};
}

async function makeVectorBackground(
	pathName: string,
	width: number,
	height: number,
	rotation: number,
	cropOffset = 12
): Promise<PageGeometry> {
	const document = await PDFDocument.create();
	const page = document.addPage([width + cropOffset * 2, height + cropOffset * 2]);
	page.setCropBox(cropOffset, cropOffset, width, height);
	page.setRotation(degrees(rotation));
	page.drawRectangle({
		x: cropOffset,
		y: cropOffset,
		width: width / 2,
		height: height / 2,
		color: rgb(0.96, 0.83, 0.3)
	});
	page.drawRectangle({
		x: cropOffset + width / 2,
		y: cropOffset,
		width: width / 2,
		height: height / 2,
		color: rgb(0.25, 0.7, 0.55)
	});
	page.drawRectangle({
		x: cropOffset,
		y: cropOffset + height / 2,
		width,
		height: height / 2,
		color: rgb(0.87, 0.92, 1)
	});
	backgroundFiles.set(pathName, await document.save());
	const normalizedRotation = ((rotation % 360) + 360) % 360;
	const rotated = normalizedRotation === 90 || normalizedRotation === 270;
	return {
		mediaBox: {
			xPoints: 0,
			yPoints: 0,
			widthPoints: width + cropOffset * 2,
			heightPoints: height + cropOffset * 2
		},
		cropBox: {
			xPoints: cropOffset,
			yPoints: cropOffset,
			widthPoints: width,
			heightPoints: height
		},
		rotation: normalizedRotation,
		displayedWidthPoints: rotated ? height : width,
		displayedHeightPoints: rotated ? width : height,
		paperLabel: 'ขนาดทดสอบ'
	};
}

function manifest(pathName: string, geometry: PageGeometry, withElements = true): RenderManifest {
	return {
		templateId: '10000000-0000-4000-8000-000000000001',
		pageGeometry: geometry,
		layout: {
			schemaVersion: 1,
			elements: withElements
				? [
						{
							type: 'text',
							id: '20000000-0000-4000-8000-000000000001',
							content: 'มอบให้ {ชื่อ} {นามสกุล}',
							frame: { x: 18, y: 24, width: geometry.displayedWidthPoints - 36, height: 44 },
							rotation: 0,
							fontSource: { type: 'built_in' },
							fontFamily: 'Sarabun',
							fontWeight: 700,
							fontSize: 18,
							minFontSize: 10,
							color: '#183153',
							alignment: 'center',
							lineHeight: 1.2,
							autoShrink: true,
							shadow: { offsetX: 1.2, offsetY: 1.2, blur: 1.5, color: '#FFFFFFCC' }
						},
						{
							type: 'qr',
							id: '30000000-0000-4000-8000-000000000001',
							frame: {
								x: geometry.displayedWidthPoints - 55,
								y: geometry.displayedHeightPoints - 55,
								width: 40,
								height: 40
							},
							rotation: 0
						}
					]
				: []
		},
		campaignValues: {
			academicYear: '2569',
			campaignName: 'กิจกรรมวันภาษาไทย',
			eventDate: '2026-07-29',
			issueDate: '2026-08-14',
			schoolName: 'โรงเรียนตัวอย่าง',
			ownerOrganizationUnitName: 'กลุ่มสาระภาษาไทย'
		},
		recipientValues: { ชื่อ: 'ณัฏฐณิชาภัทรวรรณ', นามสกุล: 'รัตนสุวรรณกุลชัยวัฒนา' },
		certificateNumber: '2569-001-00001-7',
		qrPayload: 'https://verify.example.test/c/opaque-proof-token',
		builtInFonts: [
			{ family: 'Sarabun', weight: 400, assetPath: '/fonts/Sarabun-Regular.ttf' },
			{ family: 'Sarabun', weight: 700, assetPath: '/fonts/Sarabun-Bold.ttf' }
		],
		fontGrants: [],
		imageGrants: [],
		backgroundGrant: {
			fileId: '40000000-0000-4000-8000-000000000001',
			url: pathName,
			expiresAt: '2099-01-01T00:00:00Z'
		},
		suggestedFilename: 'เกียรติบัตร-ทดสอบ.pdf'
	};
}

let devServer: ViteDevServer;
let baseUrl: string;

test.beforeAll(async () => {
	devServer = await createServer({
		root: frontendRoot,
		logLevel: 'silent',
		plugins: [harnessPlugin()],
		server: { host: '127.0.0.1', port: 0 }
	});
	await devServer.listen();
	const address = devServer.httpServer?.address();
	if (!address || typeof address === 'string') throw new Error('Vite test server did not start');
	baseUrl = `http://127.0.0.1:${address.port}`;
	await Promise.all([
		makeVectorBackground('/background/rotation-0.pdf', 240, 160, 0),
		makeVectorBackground('/background/rotation-90.pdf', 160, 240, 90),
		makeVectorBackground('/background/rotation-180.pdf', 240, 160, 180),
		makeVectorBackground('/background/rotation-270.pdf', 160, 240, 270),
		makeVectorBackground('/background/a4.pdf', 841.89, 595.28, 0, 0),
		makeVectorBackground('/background/a5.pdf', 419.53, 595.28, 0, 0)
	]);
});

test.afterAll(async () => {
	await devServer.close();
});

test('preview and exported vector PDF stay pixel-equivalent at every page rotation', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	for (const rotation of [0, 90, 180, 270] as const) {
		const sourceWidth = rotation === 90 || rotation === 270 ? 160 : 240;
		const sourceHeight = rotation === 90 || rotation === 270 ? 240 : 160;
		const geometry = await makeVectorBackground(
			`/background/runtime-${rotation}.pdf`,
			sourceWidth,
			sourceHeight,
			rotation
		);
		const result = await page.evaluate(
			async ({ value, origin }) => {
				value.backgroundGrant.url = `${origin}${value.backgroundGrant.url}`;
				return window.certificateRendererHarness.compare(value);
			},
			{ value: manifest(`/background/runtime-${rotation}.pdf`, geometry), origin: baseUrl }
		);

		expect(result.previewWidth).toBe(240);
		expect(result.previewHeight).toBe(160);
		expect(result.exportedWidth).toBeCloseTo(240, 4);
		expect(result.exportedHeight).toBeCloseTo(160, 4);
		expect(result.differenceRatio).toBeLessThan(0.015);
	}
});

test('export preserves the source CropBox at canonical and non-canonical rotations', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	for (const rotation of [0, 90, 180, 270, -90, 450]) {
		const normalizedRotation = ((rotation % 360) + 360) % 360;
		const isQuarterTurn = normalizedRotation === 90 || normalizedRotation === 270;
		const geometry = await makeVectorBackground(
			`/background/runtime-raw-${rotation}.pdf`,
			isQuarterTurn ? 160 : 240,
			isQuarterTurn ? 240 : 160,
			rotation
		);
		const result = await page.evaluate(
			async ({ value, origin }) => {
				value.backgroundGrant.url = `${origin}${value.backgroundGrant.url}`;
				return window.certificateRendererHarness.compareBackground(value);
			},
			{
				value: manifest(`/background/runtime-raw-${rotation}.pdf`, geometry, false),
				origin: baseUrl
			}
		);
		expect(result.sourceWidth).toBeCloseTo(240, 4);
		expect(result.sourceHeight).toBeCloseTo(160, 4);
		expect(result.exportedWidth).toBeCloseTo(240, 4);
		expect(result.exportedHeight).toBeCloseTo(160, 4);
		expect(result.differenceRatio).toBeLessThan(0.015);
	}
});

test('aborting one preview does not poison another preview loading the same font', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	const geometry = await makeVectorBackground('/background/runtime-font-race.pdf', 240, 160, 0);
	const value = manifest('/background/runtime-font-race.pdf', geometry);
	Object.assign(value.layout.elements[0], {
		fontFamily: 'SarabunAbortRace',
		fontWeight: 400,
		fontSource: { type: 'built_in' }
	});
	value.builtInFonts = [
		{
			family: 'SarabunAbortRace',
			weight: 400,
			assetPath: `${baseUrl}/fonts/abort-race-sarabun.ttf`
		}
	];
	value.backgroundGrant.url = `${baseUrl}${value.backgroundGrant.url}`;

	const result = await page.evaluate(
		(value) => window.certificateRendererHarness.raceAbortedFontPreviews(value),
		value
	);

	expect(result).toEqual({
		firstStatus: 'rejected',
		secondStatus: 'fulfilled',
		fontRequestCount: 2
	});
});

test('a refreshed signed URL does not register the same uploaded font again', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	const geometry = await makeVectorBackground('/background/runtime-font-grant.pdf', 240, 160, 0);
	const assetId = '50000000-0000-4000-8000-000000000001';
	const fileId = '60000000-0000-4000-8000-000000000001';
	const values = ['old', 'refreshed'].map((signature) => {
		const value = manifest('/background/runtime-font-grant.pdf', geometry);
		Object.assign(value.layout.elements[0], {
			fontFamily: 'SarabunUploadedStable',
			fontWeight: 400,
			fontSource: { type: 'asset', asset_id: assetId }
		});
		value.builtInFonts = [];
		value.fontGrants = [
			{
				assetId,
				fileId,
				url: `${baseUrl}/fonts/Sarabun-Regular.ttf?signature=${signature}`,
				expiresAt: '2099-01-01T00:00:00Z',
				family: 'SarabunUploadedStable',
				weight: 400
			}
		];
		value.backgroundGrant.url = `${baseUrl}${value.backgroundGrant.url}`;
		return value;
	});

	const fontRequestCount = await page.evaluate(
		([first, second]) =>
			window.certificateRendererHarness.countFontRequestsAcrossSignedUrls(first, second),
		values as [RenderManifest, RenderManifest]
	);

	expect(fontRequestCount).toBe(1);
});

test('one batch PDF preserves mixed normalized page dimensions', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	const a4Geometry = await makeVectorBackground('/background/runtime-a4.pdf', 841.89, 595.28, 0, 0);
	const a5Geometry = await makeVectorBackground('/background/runtime-a5.pdf', 419.53, 595.28, 0, 0);
	const sizes = await page.evaluate(
		async ({ values, origin }) => {
			for (const value of values)
				value.backgroundGrant.url = `${origin}${value.backgroundGrant.url}`;
			return window.certificateRendererHarness.pageSizes(values);
		},
		{
			values: [
				manifest('/background/runtime-a4.pdf', a4Geometry, false),
				manifest('/background/runtime-a5.pdf', a5Geometry, false)
			],
			origin: baseUrl
		}
	);

	expect(sizes).toEqual([
		{ width: 841.89, height: 595.28 },
		{ width: 419.53, height: 595.28 }
	]);
});

test('exports a valid blank PDF background that has no Contents stream', async ({ page }) => {
	const pathName = '/background/runtime-blank.pdf';
	const document = await PDFDocument.create();
	document.addPage([240, 160]);
	backgroundFiles.set(pathName, await document.save());
	const geometry: PageGeometry = {
		mediaBox: { xPoints: 0, yPoints: 0, widthPoints: 240, heightPoints: 160 },
		cropBox: { xPoints: 0, yPoints: 0, widthPoints: 240, heightPoints: 160 },
		rotation: 0,
		displayedWidthPoints: 240,
		displayedHeightPoints: 160,
		paperLabel: 'ขนาดทดสอบ'
	};

	await page.goto(`${baseUrl}${harnessPath}`);
	const sizes = await page.evaluate(
		async ({ value, origin }) => {
			value.backgroundGrant.url = `${origin}${value.backgroundGrant.url}`;
			return window.certificateRendererHarness.pageSizes([value]);
		},
		{ value: manifest(pathName, geometry), origin: baseUrl }
	);

	expect(sizes).toEqual([{ width: 240, height: 160 }]);
});

test('inspects a local background with the same normalized geometry contract', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	const expected = await makeVectorBackground('/background/runtime-inspect.pdf', 160, 240, 90);
	const inspected = await page.evaluate(
		(url) => window.certificateRendererHarness.inspectBackground(url),
		`${baseUrl}/background/runtime-inspect.pdf`
	);
	expect(inspected.mediaBox).toEqual(expected.mediaBox);
	expect(inspected.cropBox).toEqual(expected.cropBox);
	expect(inspected.rotation).toBe(90);
	expect(inspected.displayedWidthPoints).toBe(240);
	expect(inspected.displayedHeightPoints).toBe(160);
});

declare global {
	interface Window {
		certificateRendererHarness: {
			compare(manifest: RenderManifest): Promise<{
				differenceRatio: number;
				previewWidth: number;
				previewHeight: number;
				exportedWidth: number;
				exportedHeight: number;
			}>;
			pageSizes(manifests: RenderManifest[]): Promise<Array<{ width: number; height: number }>>;
			compareBackground(manifest: RenderManifest): Promise<{
				differenceRatio: number;
				sourceWidth: number;
				sourceHeight: number;
				exportedWidth: number;
				exportedHeight: number;
			}>;
			raceAbortedFontPreviews(manifest: RenderManifest): Promise<{
				firstStatus: PromiseSettledResult<unknown>['status'];
				secondStatus: PromiseSettledResult<unknown>['status'];
				fontRequestCount: number;
			}>;
			countFontRequestsAcrossSignedUrls(
				first: RenderManifest,
				second: RenderManifest
			): Promise<number>;
			inspectBackground(url: string): Promise<PageGeometry>;
		};
	}
}
