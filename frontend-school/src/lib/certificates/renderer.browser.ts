import { degrees, PDFDocument } from 'pdf-lib';
import {
	getDocument,
	GlobalWorkerOptions,
	type PDFDocumentLoadingTask,
	type PDFDocumentProxy
} from 'pdfjs-dist';
import pdfWorkerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
import { toCanvas as renderQrToCanvas } from 'qrcode';
import { apiClient } from '$lib/api/client';
import type { CertificateRenderManifest } from '$lib/api/certificates';
import { validateCertificateBatchSize } from './download';
import { interpolateCertificateText } from './interpolation';
import { describePaper } from './paper';
import {
	backgroundPageTransform,
	CERTIFICATE_RENDER_DPI,
	chooseAutoShrinkFontSize,
	displayedPageSize,
	normalizePageRotation,
	PDF_POINTS_PER_INCH,
	supportedPageRotation
} from './layout';
import type {
	CertificateBackgroundInspection,
	CertificatePreviewOptions,
	CertificatePreviewResult,
	CertificateRenderer
} from './renderer';

GlobalWorkerOptions.workerSrc = pdfWorkerUrl;

type CertificateElement = CertificateRenderManifest['layout']['elements'][number];
type TextElement = Extract<CertificateElement, { type: 'text' }>;
type ImageElement = Extract<CertificateElement, { type: 'image' }>;
type QrElement = Extract<CertificateElement, { type: 'qr' }>;
type RenderImage = CanvasImageSource & { close?: () => void };

type WrappedText = {
	fontSize: number;
	lines: string[];
	lineHeight: number;
};

type OverlayResult = {
	canvas: HTMLCanvasElement;
	widthPoints: number;
	heightPoints: number;
};

const geometryTolerance = 0.05;
const loadedFontFaces = new Map<string, { alias: string; face: FontFace }>();

class RenderResources {
	readonly #signal?: AbortSignal;
	readonly #bytePromises = new Map<string, Promise<Uint8Array>>();
	readonly #imagePromises = new Map<string, Promise<RenderImage>>();

	constructor(signal?: AbortSignal) {
		this.#signal = signal;
	}

	throwIfAborted(): void {
		this.#signal?.throwIfAborted();
	}

	bytes(url: string, description: string): Promise<Uint8Array> {
		this.throwIfAborted();
		const cached = this.#bytePromises.get(url);
		if (cached) return cached;
		const request = apiClient
			.getExternalBlob(url, { signal: this.#signal })
			.then(async (response) => {
				if (!response.success || !response.data) {
					throw new Error(`โหลด${description}ไม่สำเร็จ (${response.status})`);
				}
				const bytes = new Uint8Array(await response.data.arrayBuffer());
				if (bytes.byteLength === 0) throw new Error(`${description}ไม่มีข้อมูล`);
				return bytes;
			})
			.catch((error: unknown) => {
				this.#bytePromises.delete(url);
				throw error;
			});
		this.#bytePromises.set(url, request);
		return request;
	}

	image(url: string, description: string): Promise<RenderImage> {
		const cached = this.#imagePromises.get(url);
		if (cached) return cached;
		const request = this.bytes(url, description)
			.then((bytes) => decodeImage(bytes))
			.catch((error: unknown) => {
				this.#imagePromises.delete(url);
				throw error;
			});
		this.#imagePromises.set(url, request);
		return request;
	}

	async dispose(): Promise<void> {
		const images = await Promise.allSettled(this.#imagePromises.values());
		for (const image of images) {
			if (image.status === 'fulfilled') image.value.close?.();
		}
	}
}

function renderError(message: string): Error {
	return new Error(`ไม่สามารถสร้างเกียรติบัตรได้: ${message}`);
}

function assertFinitePositive(value: number, label: string): void {
	if (!Number.isFinite(value) || value <= 0) throw renderError(`${label}ไม่ถูกต้อง`);
}

function assertGeometry(manifest: CertificateRenderManifest): {
	sourceWidth: number;
	sourceHeight: number;
	displayedWidth: number;
	displayedHeight: number;
	rotation: 0 | 90 | 180 | 270;
} {
	const { cropBox, displayedWidthPoints, displayedHeightPoints, rotation } = manifest.pageGeometry;
	assertFinitePositive(cropBox.widthPoints, 'ความกว้างหน้า');
	assertFinitePositive(cropBox.heightPoints, 'ความสูงหน้า');
	assertFinitePositive(displayedWidthPoints, 'ความกว้างหน้าที่แสดง');
	assertFinitePositive(displayedHeightPoints, 'ความสูงหน้าที่แสดง');
	const normalizedRotation = supportedPageRotation(rotation);
	const displayed = displayedPageSize(
		cropBox.widthPoints,
		cropBox.heightPoints,
		normalizedRotation
	);
	if (
		Math.abs(displayed.width - displayedWidthPoints) > geometryTolerance ||
		Math.abs(displayed.height - displayedHeightPoints) > geometryTolerance
	) {
		throw renderError('ขนาดหน้าที่แสดงไม่ตรงกับ PDF พื้นหลัง');
	}
	return {
		sourceWidth: cropBox.widthPoints,
		sourceHeight: cropBox.heightPoints,
		displayedWidth: displayed.width,
		displayedHeight: displayed.height,
		rotation: normalizedRotation
	};
}

function interpolationValues(manifest: CertificateRenderManifest): Record<string, string> {
	const campaign = manifest.campaignValues;
	return {
		ปีการศึกษา: campaign.academicYear,
		ชื่อกิจกรรมหลัก: campaign.campaignName,
		วันที่จัดกิจกรรม: campaign.eventDate,
		วันที่ออก: campaign.issueDate,
		ชื่อโรงเรียนผู้ออก: campaign.schoolName,
		ชื่อหน่วยงานเจ้าของกิจกรรม: campaign.ownerOrganizationUnitName,
		...manifest.recipientValues,
		เลขเกียรติบัตร: manifest.certificateNumber,
		QR_CODE: manifest.qrPayload
	};
}

function fontAliasKey(
	manifest: CertificateRenderManifest,
	element: TextElement
): {
	cacheKey: string;
	alias: string;
	url: string;
} {
	if (element.fontSource.type === 'asset') {
		const assetId = element.fontSource.asset_id;
		const grant = manifest.fontGrants.find((candidate) => candidate.assetId === assetId);
		if (!grant) throw renderError(`ไม่พบไฟล์ฟอนต์สำหรับข้อความ ${element.id}`);
		if (
			grant.family !== element.fontFamily ||
			grant.weight !== element.fontWeight ||
			grant.style !== element.fontStyle
		) {
			throw renderError(`ข้อมูลฟอนต์ของข้อความ ${element.id} ไม่ตรงกับแม่แบบ`);
		}
		return {
			cacheKey: `asset:${grant.assetId}:${grant.fileId}:${grant.family}:${grant.weight}:${grant.style}`,
			alias: `SchoolOrbitCertificateAsset-${grant.assetId}-${grant.fileId}-${grant.weight}-${grant.style}`,
			url: grant.url
		};
	}

	const font = manifest.builtInFonts.find(
		(candidate) =>
			candidate.family === element.fontFamily &&
			candidate.weight === element.fontWeight &&
			candidate.style === element.fontStyle
	);
	if (!font) throw renderError(`ไม่พบฟอนต์มาตรฐานสำหรับข้อความ ${element.id}`);
	return {
		cacheKey: `built-in:${font.family}:${font.weight}:${font.style}:${font.assetPath}`,
		alias: `SchoolOrbitCertificateBuiltIn-${font.family.replace(/[^a-z0-9_-]/giu, '-')}-${font.weight}-${font.style}`,
		url: font.assetPath
	};
}

async function loadElementFont(
	manifest: CertificateRenderManifest,
	element: TextElement,
	resources: RenderResources
): Promise<string> {
	const { cacheKey, alias, url } = fontAliasKey(manifest, element);
	const cached = loadedFontFaces.get(cacheKey);
	if (cached) return cached.alias;

	// Pending work stays local to this preview so aborting it cannot reject a
	// different preview that happens to need the same font.
	const bytes = await resources.bytes(url, 'ฟอนต์');
	resources.throwIfAborted();
	const winnerBeforeLoad = loadedFontFaces.get(cacheKey);
	if (winnerBeforeLoad) return winnerBeforeLoad.alias;
	const buffer = bytes.buffer.slice(
		bytes.byteOffset,
		bytes.byteOffset + bytes.byteLength
	) as ArrayBuffer;
	const face = new FontFace(alias, buffer, {
		weight: String(element.fontWeight),
		style: element.fontStyle
	});
	await face.load();
	resources.throwIfAborted();
	const winnerAfterLoad = loadedFontFaces.get(cacheKey);
	if (winnerAfterLoad) return winnerAfterLoad.alias;
	document.fonts.add(face);
	loadedFontFaces.set(cacheKey, { alias, face });
	return alias;
}

function makeCanvas(width: number, height: number, label: string): HTMLCanvasElement {
	if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || height < 1) {
		throw renderError(`${label}มีขนาดไม่ถูกต้อง`);
	}
	const canvas = document.createElement('canvas');
	canvas.width = width;
	canvas.height = height;
	return canvas;
}

function context2d(canvas: HTMLCanvasElement, alpha: boolean): CanvasRenderingContext2D {
	const context = canvas.getContext('2d', { alpha });
	if (!context) throw renderError('เบราว์เซอร์ไม่รองรับ Canvas 2D');
	return context;
}

function wordSegments(text: string): string[] {
	if (typeof Intl.Segmenter !== 'undefined') {
		return Array.from(
			new Intl.Segmenter('th', { granularity: 'word' }).segment(text),
			(segment) => segment.segment
		);
	}
	return Array.from(text);
}

function graphemeSegments(text: string): string[] {
	if (typeof Intl.Segmenter !== 'undefined') {
		return Array.from(
			new Intl.Segmenter('th', { granularity: 'grapheme' }).segment(text),
			(segment) => segment.segment
		);
	}
	return Array.from(text);
}

function trimLineEnd(value: string): string {
	return value.replace(/\s+$/u, '');
}

function wrapSingleParagraph(
	context: CanvasRenderingContext2D,
	paragraph: string,
	maxWidth: number
): string[] {
	if (!paragraph) return [''];
	const lines: string[] = [];
	let current = '';

	const pushCurrent = () => {
		lines.push(trimLineEnd(current));
		current = '';
	};

	const appendPiece = (piece: string) => {
		if (!current && /^\s+$/u.test(piece)) return;
		const candidate = current + piece;
		if (context.measureText(candidate).width <= maxWidth) {
			current = candidate;
			return;
		}
		if (current) pushCurrent();
		if (context.measureText(piece).width <= maxWidth) {
			current = piece.replace(/^\s+/u, '');
			return;
		}

		for (const grapheme of graphemeSegments(piece)) {
			const graphemeCandidate = current + grapheme;
			if (current && context.measureText(graphemeCandidate).width > maxWidth) pushCurrent();
			current += grapheme;
		}
	};

	for (const segment of wordSegments(paragraph)) appendPiece(segment);
	if (current || lines.length === 0) pushCurrent();
	return lines;
}

function wrappedLines(context: CanvasRenderingContext2D, text: string, maxWidth: number): string[] {
	return text
		.replace(/\r\n?/gu, '\n')
		.split('\n')
		.flatMap((paragraph) => wrapSingleParagraph(context, paragraph, maxWidth));
}

function canvasFont(
	style: TextElement['fontStyle'],
	weight: number,
	size: number,
	alias: string
): string {
	return `${style} ${weight} ${size}px "${alias}"`;
}

function layoutText(
	context: CanvasRenderingContext2D,
	element: TextElement,
	text: string,
	fontAlias: string,
	scale: number,
	frameWidth: number,
	frameHeight: number
): WrappedText {
	const fitsAt = (
		fontSizePoints: number
	): { fits: boolean; lines: string[]; lineHeight: number } => {
		const fontSize = fontSizePoints * scale;
		context.font = canvasFont(element.fontStyle, element.fontWeight, fontSize, fontAlias);
		const lines = wrappedLines(context, text, frameWidth);
		const lineHeight = fontSize * element.lineHeight;
		const widest = Math.max(0, ...lines.map((line) => context.measureText(line).width));
		return {
			fits: widest <= frameWidth + 0.01 && lines.length * lineHeight <= frameHeight + 0.01,
			lines,
			lineHeight
		};
	};

	const fontSizePoints = chooseAutoShrinkFontSize({
		fontSize: element.fontSize,
		minFontSize: element.minFontSize,
		autoShrink: element.autoShrink,
		fits: (candidate) => fitsAt(candidate).fits
	});
	const fitted = fitsAt(fontSizePoints);
	return {
		fontSize: fontSizePoints * scale,
		lines: fitted.lines,
		lineHeight: fitted.lineHeight
	};
}

function rotateIntoFrame(
	context: CanvasRenderingContext2D,
	frame: { x: number; y: number; width: number; height: number },
	rotation: number,
	draw: () => void
): void {
	context.save();
	context.translate(frame.x + frame.width / 2, frame.y + frame.height / 2);
	context.rotate((rotation * Math.PI) / 180);
	context.translate(-frame.width / 2, -frame.height / 2);
	context.beginPath();
	context.rect(0, 0, frame.width, frame.height);
	context.clip();
	draw();
	context.restore();
}

async function drawTextElement(
	context: CanvasRenderingContext2D,
	manifest: CertificateRenderManifest,
	element: TextElement,
	resources: RenderResources,
	scaleX: number,
	scaleY: number,
	values: Record<string, string>
): Promise<void> {
	const fontAlias = await loadElementFont(manifest, element, resources);
	const text = interpolateCertificateText(element.content, values);
	const uniformScale = Math.min(scaleX, scaleY);
	const frame = {
		x: element.frame.x * scaleX,
		y: element.frame.y * scaleY,
		width: element.frame.width * scaleX,
		height: element.frame.height * scaleY
	};
	const textLayout = layoutText(
		context,
		element,
		text,
		fontAlias,
		uniformScale,
		frame.width,
		frame.height
	);

	rotateIntoFrame(context, frame, element.rotation, () => {
		context.font = canvasFont(
			element.fontStyle,
			element.fontWeight,
			textLayout.fontSize,
			fontAlias
		);
		context.fillStyle = element.color;
		context.textBaseline = 'top';
		context.textAlign = element.alignment;
		if (element.shadow) {
			context.shadowColor = element.shadow.color;
			context.shadowOffsetX = element.shadow.offsetX * uniformScale;
			context.shadowOffsetY = element.shadow.offsetY * uniformScale;
			context.shadowBlur = element.shadow.blur * uniformScale;
		} else {
			context.shadowColor = 'transparent';
			context.shadowOffsetX = 0;
			context.shadowOffsetY = 0;
			context.shadowBlur = 0;
		}
		const x =
			element.alignment === 'left'
				? 0
				: element.alignment === 'right'
					? frame.width
					: frame.width / 2;
		for (const [index, line] of textLayout.lines.entries()) {
			context.fillText(line, x, index * textLayout.lineHeight);
		}
	});
}

async function drawImageElement(
	context: CanvasRenderingContext2D,
	manifest: CertificateRenderManifest,
	element: ImageElement,
	resources: RenderResources,
	scaleX: number,
	scaleY: number
): Promise<void> {
	const grant = manifest.imageGrants.find((candidate) => candidate.assetId === element.assetId);
	if (!grant) throw renderError(`ไม่พบไฟล์รูปภาพสำหรับองค์ประกอบ ${element.id}`);
	const image = await resources.image(grant.url, 'รูปภาพ');
	const frame = {
		x: element.frame.x * scaleX,
		y: element.frame.y * scaleY,
		width: element.frame.width * scaleX,
		height: element.frame.height * scaleY
	};
	rotateIntoFrame(context, frame, element.rotation, () => {
		context.drawImage(image, 0, 0, frame.width, frame.height);
	});
}

async function drawQrElement(
	context: CanvasRenderingContext2D,
	manifest: CertificateRenderManifest,
	element: QrElement,
	scaleX: number,
	scaleY: number
): Promise<void> {
	if (!manifest.qrPayload) throw renderError(`QR Code ${element.id} ไม่มีข้อมูล`);
	const frame = {
		x: element.frame.x * scaleX,
		y: element.frame.y * scaleY,
		width: element.frame.width * scaleX,
		height: element.frame.height * scaleY
	};
	const qrSize = Math.max(64, Math.ceil(Math.min(frame.width, frame.height)));
	const qrCanvas = makeCanvas(qrSize, qrSize, 'QR Code');
	await renderQrToCanvas(qrCanvas, manifest.qrPayload, {
		errorCorrectionLevel: 'M',
		margin: 2,
		width: qrSize,
		color: { dark: '#000000FF', light: '#FFFFFFFF' }
	});
	rotateIntoFrame(context, frame, element.rotation, () => {
		const size = Math.min(frame.width, frame.height);
		context.drawImage(qrCanvas, (frame.width - size) / 2, (frame.height - size) / 2, size, size);
	});
}

async function renderOverlay(
	manifest: CertificateRenderManifest,
	resources: RenderResources
): Promise<OverlayResult | null> {
	if (manifest.layout.schemaVersion !== 1) throw renderError('ไม่รองรับรูปแบบ layout นี้');
	if (manifest.layout.elements.length === 0) return null;
	const geometry = assertGeometry(manifest);
	const widthPixels = Math.ceil(
		(geometry.displayedWidth * CERTIFICATE_RENDER_DPI) / PDF_POINTS_PER_INCH
	);
	const heightPixels = Math.ceil(
		(geometry.displayedHeight * CERTIFICATE_RENDER_DPI) / PDF_POINTS_PER_INCH
	);
	const canvas = makeCanvas(widthPixels, heightPixels, 'ชั้นข้อความ');
	const context = context2d(canvas, true);
	context.clearRect(0, 0, widthPixels, heightPixels);
	const scaleX = widthPixels / geometry.displayedWidth;
	const scaleY = heightPixels / geometry.displayedHeight;
	const values = interpolationValues(manifest);

	for (const element of manifest.layout.elements) {
		resources.throwIfAborted();
		switch (element.type) {
			case 'text':
				await drawTextElement(context, manifest, element, resources, scaleX, scaleY, values);
				break;
			case 'image':
				await drawImageElement(context, manifest, element, resources, scaleX, scaleY);
				break;
			case 'qr':
				await drawQrElement(context, manifest, element, scaleX, scaleY);
				break;
		}
	}

	return {
		canvas,
		widthPoints: geometry.displayedWidth,
		heightPoints: geometry.displayedHeight
	};
}

async function decodeImage(bytes: Uint8Array): Promise<RenderImage> {
	const blob = new Blob([bytes as BlobPart]);
	if (typeof createImageBitmap === 'function') return createImageBitmap(blob);
	const url = URL.createObjectURL(blob);
	try {
		const image = new Image();
		image.decoding = 'async';
		await new Promise<void>((resolve, reject) => {
			image.onload = () => resolve();
			image.onerror = () => reject(renderError('อ่านไฟล์รูปภาพไม่สำเร็จ'));
			image.src = url;
		});
		return image;
	} finally {
		URL.revokeObjectURL(url);
	}
}

async function canvasPngBytes(canvas: HTMLCanvasElement): Promise<Uint8Array> {
	const blob = await new Promise<Blob>((resolve, reject) => {
		canvas.toBlob((value) => {
			if (value) resolve(value);
			else reject(renderError('แปลงชั้นข้อความเป็น PNG ไม่สำเร็จ'));
		}, 'image/png');
	});
	return new Uint8Array(await blob.arrayBuffer());
}

async function loadPdfJsDocument(bytes: Uint8Array): Promise<{
	task: PDFDocumentLoadingTask;
	document: PDFDocumentProxy;
}> {
	const task = getDocument({
		data: bytes.slice(),
		stopAtErrors: true
	});
	return { task, document: await task.promise };
}

async function renderNormalizedPreview(
	manifest: CertificateRenderManifest,
	bytes: Uint8Array,
	canvas: HTMLCanvasElement,
	scale: number,
	signal?: AbortSignal
): Promise<CertificatePreviewResult> {
	const geometry = assertGeometry(manifest);
	const { task, document: documentProxy } = await loadPdfJsDocument(bytes);
	try {
		if (documentProxy.numPages !== 1) throw renderError('PDF พรีวิวต้องมีหนึ่งหน้า');
		const page = await documentProxy.getPage(1);
		const viewport = page.getViewport({ scale, rotation: 0 });
		if (
			Math.abs(viewport.width - geometry.displayedWidth * scale) > 0.5 ||
			Math.abs(viewport.height - geometry.displayedHeight * scale) > 0.5
		) {
			throw renderError('ขนาด PDF พรีวิวไม่ตรงกับข้อมูลแม่แบบ');
		}
		canvas.width = Math.ceil(viewport.width);
		canvas.height = Math.ceil(viewport.height);
		const context = context2d(canvas, false);
		const renderTask = page.render({
			canvas,
			canvasContext: context,
			viewport,
			background: '#FFFFFF'
		});
		const cancel = () => renderTask.cancel();
		signal?.addEventListener('abort', cancel, { once: true });
		try {
			await renderTask.promise;
			signal?.throwIfAborted();
		} finally {
			signal?.removeEventListener('abort', cancel);
		}
		return {
			widthPoints: geometry.displayedWidth,
			heightPoints: geometry.displayedHeight,
			widthPixels: canvas.width,
			heightPixels: canvas.height
		};
	} finally {
		await task.destroy();
	}
}

async function renderPreview(
	manifest: CertificateRenderManifest,
	canvas: HTMLCanvasElement,
	options: CertificatePreviewOptions = {}
): Promise<CertificatePreviewResult> {
	const scale = options.scale ?? 1;
	assertFinitePositive(scale, 'อัตราขยาย');
	const resources = new RenderResources(options.signal);
	try {
		const pdf = await buildCertificatePdfWithResources([manifest], resources);
		return renderNormalizedPreview(manifest, pdf, canvas, scale, options.signal);
	} finally {
		await resources.dispose();
	}
}

async function inspectBackgroundPdf(
	file: Blob,
	signal?: AbortSignal
): Promise<CertificateBackgroundInspection> {
	signal?.throwIfAborted();
	const bytes = new Uint8Array(await file.arrayBuffer());
	signal?.throwIfAborted();
	if (bytes.byteLength === 0) throw renderError('PDF พื้นหลังไม่มีข้อมูล');
	const document = await PDFDocument.load(bytes.slice(), {
		ignoreEncryption: false,
		updateMetadata: false,
		throwOnInvalidObject: true
	});
	if (document.getPageCount() !== 1) throw renderError('PDF พื้นหลังต้องมีหนึ่งหน้า');
	const page = document.getPage(0);
	const media = page.getMediaBox();
	const crop = page.getCropBox();
	for (const [value, label] of [
		[media.width, 'ความกว้าง MediaBox'],
		[media.height, 'ความสูง MediaBox'],
		[crop.width, 'ความกว้าง CropBox'],
		[crop.height, 'ความสูง CropBox']
	] as const) {
		assertFinitePositive(value, label);
	}
	if (![media.x, media.y, crop.x, crop.y].every(Number.isFinite)) {
		throw renderError('ตำแหน่งกรอบหน้ากระดาษไม่ถูกต้อง');
	}
	const rotation = normalizePageRotation(page.getRotation().angle);
	const displayed = displayedPageSize(crop.width, crop.height, rotation);
	signal?.throwIfAborted();
	return {
		mediaBox: {
			xPoints: media.x,
			yPoints: media.y,
			widthPoints: media.width,
			heightPoints: media.height
		},
		cropBox: {
			xPoints: crop.x,
			yPoints: crop.y,
			widthPoints: crop.width,
			heightPoints: crop.height
		},
		rotation,
		displayedWidthPoints: displayed.width,
		displayedHeightPoints: displayed.height,
		paperLabel: describePaper({
			widthPoints: crop.width,
			heightPoints: crop.height,
			rotation
		})
	};
}

async function assertSourcePdfGeometry(
	manifest: CertificateRenderManifest,
	sourceDocument: PDFDocument
): Promise<void> {
	if (sourceDocument.getPageCount() !== 1) throw renderError('PDF พื้นหลังต้องมีหนึ่งหน้า');
	const page = sourceDocument.getPage(0);
	const crop = page.getCropBox();
	const expected = manifest.pageGeometry.cropBox;
	if (
		Math.abs(crop.x - expected.xPoints) > geometryTolerance ||
		Math.abs(crop.y - expected.yPoints) > geometryTolerance ||
		Math.abs(crop.width - expected.widthPoints) > geometryTolerance ||
		Math.abs(crop.height - expected.heightPoints) > geometryTolerance ||
		normalizePageRotation(page.getRotation().angle) !==
			supportedPageRotation(manifest.pageGeometry.rotation)
	) {
		throw renderError('geometry ของ PDF พื้นหลังไม่ตรงกับข้อมูลแม่แบบ');
	}
}

async function buildCertificatePdfWithResources(
	manifests: readonly CertificateRenderManifest[],
	resources: RenderResources
): Promise<Uint8Array> {
	validateCertificateBatchSize(manifests.length);
	const output = await PDFDocument.create();
	output.setCreator('SchoolOrbit');
	output.setProducer('SchoolOrbit Certificate Renderer');
	for (const manifest of manifests) {
		resources.throwIfAborted();
		const geometry = assertGeometry(manifest);
		const sourceBytes = await resources.bytes(manifest.backgroundGrant.url, 'PDF พื้นหลัง');
		const source = await PDFDocument.load(sourceBytes.slice(), {
			ignoreEncryption: false,
			updateMetadata: false,
			throwOnInvalidObject: true
		});
		await assertSourcePdfGeometry(manifest, source);
		const sourcePage = source.getPage(0);
		// A valid blank PDF page can omit /Contents, but pdf-lib cannot embed it.
		// Appending an empty stream preserves the page while making it embeddable.
		sourcePage.pushOperators();
		const crop = manifest.pageGeometry.cropBox;
		const embeddedBackground = await output.embedPage(sourcePage, {
			left: crop.xPoints,
			bottom: crop.yPoints,
			right: crop.xPoints + crop.widthPoints,
			top: crop.yPoints + crop.heightPoints
		});
		const page = output.addPage([geometry.displayedWidth, geometry.displayedHeight]);
		// A page dictionary's /Rotate is clockwise in viewer space, while rotating
		// embedded content operates in the page's Cartesian coordinate system.
		const transform = backgroundPageTransform(
			geometry.sourceWidth,
			geometry.sourceHeight,
			normalizePageRotation(-geometry.rotation)
		);
		page.drawPage(embeddedBackground, {
			x: transform.x,
			y: transform.y,
			width: geometry.sourceWidth,
			height: geometry.sourceHeight,
			rotate: degrees(transform.rotation)
		});

		const overlay = await renderOverlay(manifest, resources);
		if (overlay) {
			const png = await output.embedPng(await canvasPngBytes(overlay.canvas));
			page.drawImage(png, {
				x: 0,
				y: 0,
				width: overlay.widthPoints,
				height: overlay.heightPoints
			});
			overlay.canvas.width = 1;
			overlay.canvas.height = 1;
		}
	}
	resources.throwIfAborted();
	return output.save({ useObjectStreams: true, addDefaultPage: false });
}

async function buildCertificatePdf(
	manifests: readonly CertificateRenderManifest[]
): Promise<Uint8Array> {
	const resources = new RenderResources();
	try {
		return await buildCertificatePdfWithResources(manifests, resources);
	} finally {
		await resources.dispose();
	}
}

async function prepareFontAliases(
	manifest: CertificateRenderManifest,
	layout: CertificateRenderManifest['layout'],
	signal?: AbortSignal
): Promise<Record<string, string>> {
	const resources = new RenderResources(signal);
	try {
		const entries = await Promise.all(
			layout.elements
				.filter((element): element is TextElement => element.type === 'text')
				.map(
					async (element) =>
						[element.id, await loadElementFont(manifest, element, resources)] as const
				)
		);
		return Object.fromEntries(entries);
	} finally {
		await resources.dispose();
	}
}

export function createCertificateRenderer(): CertificateRenderer {
	return { inspectBackgroundPdf, prepareFontAliases, renderPreview, buildCertificatePdf };
}
