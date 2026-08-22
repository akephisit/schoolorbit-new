import type { CertificateRenderManifest, CertificateTemplateDetail } from '../api/certificates';

export type CertificateLayout = CertificateTemplateDetail['layout'];
export type CertificateElement = CertificateLayout['elements'][number];
export type TextCertificateElement = Extract<CertificateElement, { type: 'text' }>;
export type ImageCertificateElement = Extract<CertificateElement, { type: 'image' }>;
export type CertificateFrame = CertificateElement['frame'];
export type PagePointSize = { width: number; height: number };
export type ResizeHandle = 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'nw';
export type LayerDirection = 'forward' | 'backward' | 'front' | 'back';
export type ElementAlignment = 'left' | 'center' | 'right' | 'top' | 'middle' | 'bottom';

export type CertificateEditorState = {
	layout: CertificateLayout;
	selectedIds: string[];
	zoom: number;
	safeMarginPoints: number;
	showSafeArea: boolean;
	snapToGuides: boolean;
	dirty: boolean;
	saving: boolean;
	previewing: 'short' | 'normal' | 'long' | 'candidate' | null;
};

export const MIN_CERTIFICATE_FRAME_POINTS = 12;
export const DEFAULT_ELEMENT_OFFSET_POINTS = 12;
export const DEFAULT_SAFE_MARGIN_POINTS = (10 * 72) / 25.4;
export const CERTIFICATE_GRANT_REFRESH_LEAD_MS = 30_000;

type CertificateManifestGrants = Pick<
	CertificateRenderManifest,
	'backgroundGrant' | 'fontGrants' | 'imageGrants'
>;

type CertificateLayoutAssetGrants = Pick<CertificateRenderManifest, 'fontGrants' | 'imageGrants'>;

export function certificateManifestExpiresSoon(
	manifest: CertificateManifestGrants,
	now = Date.now(),
	minimumValidityMs = CERTIFICATE_GRANT_REFRESH_LEAD_MS
): boolean {
	const deadline = now + Math.max(0, minimumValidityMs);
	return [manifest.backgroundGrant, ...manifest.fontGrants, ...manifest.imageGrants].some(
		(grant) => {
			const expiresAt = Date.parse(grant.expiresAt);
			return !Number.isFinite(expiresAt) || expiresAt <= deadline;
		}
	);
}

export function certificateManifestNeedsLayoutGrants(
	manifest: CertificateLayoutAssetGrants,
	layout: CertificateLayout
): boolean {
	const schoolFonts = new Set(manifest.fontGrants.map((grant) => grant.schoolFontId));
	const imageAssets = new Set(manifest.imageGrants.map((grant) => grant.assetId));
	return layout.elements.some((element) => {
		if (element.type === 'image') return !imageAssets.has(element.assetId);
		if (element.type === 'text' && element.fontSource.type === 'school_font') {
			return !schoolFonts.has(element.fontSource.font_id);
		}
		return false;
	});
}

function cloneFrame(frame: CertificateFrame): CertificateFrame {
	return { x: frame.x, y: frame.y, width: frame.width, height: frame.height };
}

export function cloneCertificateLayout(layout: CertificateLayout): CertificateLayout {
	return {
		schemaVersion: 1,
		elements: layout.elements.map((element) => ({
			...element,
			frame: cloneFrame(element.frame),
			...(element.type === 'text' && element.shadow ? { shadow: { ...element.shadow } } : {})
		})) as CertificateElement[]
	};
}

function withFrame(element: CertificateElement, frame: CertificateFrame): CertificateElement {
	return { ...element, frame } as CertificateElement;
}

function assertZoom(zoom: number): void {
	if (!Number.isFinite(zoom) || zoom <= 0) throw new Error('zoom must be positive');
}

function normalizedRotation(rotation: number): number {
	if (!Number.isFinite(rotation)) throw new Error('rotation must be finite');
	const normalized = ((rotation % 360) + 360) % 360;
	return (Math.round(normalized * 100) / 100) % 360;
}

export function screenDeltaToElementAxes(
	delta: { dxPixels: number; dyPixels: number },
	rotation: number
): { dxPixels: number; dyPixels: number } {
	if (!Number.isFinite(delta.dxPixels) || !Number.isFinite(delta.dyPixels)) {
		throw new Error('pointer delta must be finite');
	}
	const radians = (normalizedRotation(rotation) * Math.PI) / 180;
	const cosine = Math.cos(radians);
	const sine = Math.sin(radians);
	return {
		dxPixels: cosine * delta.dxPixels + sine * delta.dyPixels,
		dyPixels: -sine * delta.dxPixels + cosine * delta.dyPixels
	};
}

export function createCertificateEditorState(
	template: CertificateTemplateDetail,
	zoom = 1
): CertificateEditorState {
	assertZoom(zoom);
	return {
		layout: cloneCertificateLayout(template.layout),
		selectedIds: [],
		zoom,
		safeMarginPoints: template.safeMarginPoints,
		showSafeArea: template.showSafeArea,
		snapToGuides: true,
		dirty: false,
		saving: false,
		previewing: null
	};
}

export function moveElement(
	element: CertificateElement,
	delta: { dxPixels: number; dyPixels: number },
	zoom: number
): CertificateElement {
	assertZoom(zoom);
	return withFrame(element, {
		...cloneFrame(element.frame),
		x: element.frame.x + delta.dxPixels / zoom,
		y: element.frame.y + delta.dyPixels / zoom
	});
}

export function resizeElement(
	element: CertificateElement,
	input: { handle: ResizeHandle; dxPixels: number; dyPixels: number },
	zoom: number
): CertificateElement {
	assertZoom(zoom);
	const dx = input.dxPixels / zoom;
	const dy = input.dyPixels / zoom;
	const original = element.frame;
	if (element.type === 'image' && element.lockAspectRatio) {
		const ratio = element.aspectRatio;
		if (!Number.isFinite(ratio) || ratio <= 0) {
			throw new Error('image aspect ratio must be positive');
		}
		const axes: Record<ResizeHandle, readonly [number, number]> = {
			n: [0, -1],
			ne: [1, -1],
			e: [1, 0],
			se: [1, 1],
			s: [0, 1],
			sw: [-1, 1],
			w: [-1, 0],
			nw: [-1, -1]
		};
		const [axisX, axisY] = axes[input.handle];
		let scale: number;
		if (axisX !== 0 && axisY !== 0) {
			const vectorX = axisX * original.width;
			const vectorY = axisY * original.height;
			scale = 1 + (dx * vectorX + dy * vectorY) / (vectorX * vectorX + vectorY * vectorY);
		} else if (axisX !== 0) {
			scale = 1 + (axisX * dx) / original.width;
		} else {
			scale = 1 + (axisY * dy) / original.height;
		}
		const minimumWidth = Math.max(
			MIN_CERTIFICATE_FRAME_POINTS,
			ratio * MIN_CERTIFICATE_FRAME_POINTS
		);
		const width = Math.max(minimumWidth, original.width * scale);
		const height = width / ratio;
		const localCenterX = (axisX * (width - original.width)) / 2;
		const localCenterY = (axisY * (height - original.height)) / 2;
		const radians = (normalizedRotation(element.rotation) * Math.PI) / 180;
		const cosine = Math.cos(radians);
		const sine = Math.sin(radians);
		const centerX = original.x + original.width / 2 + cosine * localCenterX - sine * localCenterY;
		const centerY = original.y + original.height / 2 + sine * localCenterX + cosine * localCenterY;
		return withFrame(element, {
			x: centerX - width / 2,
			y: centerY - height / 2,
			width,
			height
		});
	}
	let left = -original.width / 2;
	let right = original.width / 2;
	let top = -original.height / 2;
	let bottom = original.height / 2;

	if (input.handle.includes('w')) {
		left = Math.min(right - MIN_CERTIFICATE_FRAME_POINTS, left + dx);
	}
	if (input.handle.includes('e')) {
		right = Math.max(left + MIN_CERTIFICATE_FRAME_POINTS, right + dx);
	}
	if (input.handle.includes('n')) {
		top = Math.min(bottom - MIN_CERTIFICATE_FRAME_POINTS, top + dy);
	}
	if (input.handle.includes('s')) {
		bottom = Math.max(top + MIN_CERTIFICATE_FRAME_POINTS, bottom + dy);
	}

	const width = right - left;
	const height = bottom - top;
	const localCenterX = (left + right) / 2;
	const localCenterY = (top + bottom) / 2;
	const radians = (normalizedRotation(element.rotation) * Math.PI) / 180;
	const cosine = Math.cos(radians);
	const sine = Math.sin(radians);
	const centerX = original.x + original.width / 2 + cosine * localCenterX - sine * localCenterY;
	const centerY = original.y + original.height / 2 + sine * localCenterX + cosine * localCenterY;
	return withFrame(element, {
		x: centerX - width / 2,
		y: centerY - height / 2,
		width,
		height
	});
}

export function rotateElement(element: CertificateElement, rotation: number): CertificateElement {
	return { ...element, rotation: normalizedRotation(rotation) } as CertificateElement;
}

export function duplicateElement(
	element: CertificateElement,
	createId: () => string = () => crypto.randomUUID()
): CertificateElement {
	return {
		...element,
		id: createId(),
		frame: {
			...cloneFrame(element.frame),
			x: element.frame.x + DEFAULT_ELEMENT_OFFSET_POINTS,
			y: element.frame.y + DEFAULT_ELEMENT_OFFSET_POINTS
		},
		...(element.type === 'text' && element.shadow ? { shadow: { ...element.shadow } } : {})
	} as CertificateElement;
}

export function reorderElement(
	elements: readonly CertificateElement[],
	elementId: string,
	direction: LayerDirection
): CertificateElement[] {
	const next = [...elements];
	const index = next.findIndex((element) => element.id === elementId);
	if (index < 0) return next;
	const target =
		direction === 'front'
			? next.length - 1
			: direction === 'back'
				? 0
				: direction === 'forward'
					? Math.min(next.length - 1, index + 1)
					: Math.max(0, index - 1);
	if (target === index) return next;
	const [element] = next.splice(index, 1);
	next.splice(target, 0, element);
	return next;
}

export function alignElements(
	elements: readonly CertificateElement[],
	selectedIds: readonly string[],
	alignment: ElementAlignment
): CertificateElement[] {
	const selected = new Set(selectedIds);
	const frames = elements
		.filter((element) => selected.has(element.id))
		.map((element) => element.frame);
	if (frames.length < 2) return [...elements];
	const left = Math.min(...frames.map((frame) => frame.x));
	const right = Math.max(...frames.map((frame) => frame.x + frame.width));
	const top = Math.min(...frames.map((frame) => frame.y));
	const bottom = Math.max(...frames.map((frame) => frame.y + frame.height));
	const center = (left + right) / 2;
	const middle = (top + bottom) / 2;

	return elements.map((element) => {
		if (!selected.has(element.id)) return element;
		const frame = cloneFrame(element.frame);
		switch (alignment) {
			case 'left':
				frame.x = left;
				break;
			case 'center':
				frame.x = center - frame.width / 2;
				break;
			case 'right':
				frame.x = right - frame.width;
				break;
			case 'top':
				frame.y = top;
				break;
			case 'middle':
				frame.y = middle - frame.height / 2;
				break;
			case 'bottom':
				frame.y = bottom - frame.height;
				break;
		}
		return withFrame(element, frame);
	});
}

export function constrainElementToPage(
	element: CertificateElement,
	page: PagePointSize
): CertificateElement {
	if (
		!Number.isFinite(page.width) ||
		!Number.isFinite(page.height) ||
		page.width <= 0 ||
		page.height <= 0
	) {
		throw new Error('page size must be positive');
	}
	const radians = (normalizedRotation(element.rotation) * Math.PI) / 180;
	const cosine = Math.abs(Math.cos(radians));
	const sine = Math.abs(Math.sin(radians));
	const originalWidth = Math.max(MIN_CERTIFICATE_FRAME_POINTS, element.frame.width);
	const originalHeight = Math.max(MIN_CERTIFICATE_FRAME_POINTS, element.frame.height);
	const rotatedWidth = cosine * originalWidth + sine * originalHeight;
	const rotatedHeight = sine * originalWidth + cosine * originalHeight;
	const scale = Math.min(
		1,
		page.width / originalWidth,
		page.height / originalHeight,
		page.width / rotatedWidth,
		page.height / rotatedHeight
	);
	const width = originalWidth * scale;
	const height = originalHeight * scale;
	const extentX = (cosine * width + sine * height) / 2;
	const extentY = (sine * width + cosine * height) / 2;
	const minimumCenterX = Math.max(width / 2, extentX);
	const maximumCenterX = Math.min(page.width - width / 2, page.width - extentX);
	const minimumCenterY = Math.max(height / 2, extentY);
	const maximumCenterY = Math.min(page.height - height / 2, page.height - extentY);
	const originalCenterX = element.frame.x + element.frame.width / 2;
	const originalCenterY = element.frame.y + element.frame.height / 2;
	const centerX = Math.max(minimumCenterX, Math.min(maximumCenterX, originalCenterX));
	const centerY = Math.max(minimumCenterY, Math.min(maximumCenterY, originalCenterY));
	return withFrame(element, {
		x: centerX - width / 2,
		y: centerY - height / 2,
		width,
		height
	});
}

type ComparablePageGeometry = Pick<
	CertificateRenderManifest['pageGeometry'],
	'cropBox' | 'rotation'
>;

export function certificatePageGeometryMatches(
	left: ComparablePageGeometry,
	right: ComparablePageGeometry,
	tolerancePoints = 0.05
): boolean {
	const normalize = (rotation: number) => ((Math.round(rotation) % 360) + 360) % 360;
	return (
		normalize(left.rotation) === normalize(right.rotation) &&
		Math.abs(left.cropBox.widthPoints - right.cropBox.widthPoints) <= tolerancePoints &&
		Math.abs(left.cropBox.heightPoints - right.cropBox.heightPoints) <= tolerancePoints
	);
}

function nearestGuide(value: number, guides: readonly number[], threshold: number): number {
	let result = value;
	let distance = threshold;
	for (const guide of guides) {
		const nextDistance = Math.abs(value - guide);
		if (nextDistance <= distance) {
			distance = nextDistance;
			result = guide;
		}
	}
	return result;
}

export function snapElementToPage(
	element: CertificateElement,
	page: PagePointSize,
	options: { safeMarginPoints: number; thresholdPoints?: number; gridPoints?: number }
): CertificateElement {
	const threshold = options.thresholdPoints ?? 4;
	const grid = options.gridPoints ?? 6;
	const frame = cloneFrame(element.frame);
	const safe = Math.max(0, options.safeMarginPoints);
	const xGuides = [safe, page.width / 2, page.width - safe];
	const yGuides = [safe, page.height / 2, page.height - safe];
	const snappedLeft = nearestGuide(frame.x, xGuides, threshold);
	const snappedCenterX = nearestGuide(frame.x + frame.width / 2, xGuides, threshold);
	const snappedRight = nearestGuide(frame.x + frame.width, xGuides, threshold);
	const snappedTop = nearestGuide(frame.y, yGuides, threshold);
	const snappedMiddle = nearestGuide(frame.y + frame.height / 2, yGuides, threshold);
	const snappedBottom = nearestGuide(frame.y + frame.height, yGuides, threshold);
	const xCandidates = [
		{ value: snappedLeft, changed: snappedLeft !== frame.x },
		{
			value: snappedCenterX - frame.width / 2,
			changed: snappedCenterX !== frame.x + frame.width / 2
		},
		{ value: snappedRight - frame.width, changed: snappedRight !== frame.x + frame.width }
	];
	const yCandidates = [
		{ value: snappedTop, changed: snappedTop !== frame.y },
		{
			value: snappedMiddle - frame.height / 2,
			changed: snappedMiddle !== frame.y + frame.height / 2
		},
		{ value: snappedBottom - frame.height, changed: snappedBottom !== frame.y + frame.height }
	];
	frame.x =
		xCandidates.find((candidate) => candidate.changed)?.value ?? Math.round(frame.x / grid) * grid;
	frame.y =
		yCandidates.find((candidate) => candidate.changed)?.value ?? Math.round(frame.y / grid) * grid;
	return constrainElementToPage(withFrame(element, frame), page);
}

export function scaleCertificateLayout(
	layout: CertificateLayout,
	from: PagePointSize,
	to: PagePointSize
): CertificateLayout {
	if (from.width <= 0 || from.height <= 0 || to.width <= 0 || to.height <= 0) {
		throw new Error('page sizes must be positive');
	}
	const scaleX = to.width / from.width;
	const scaleY = to.height / from.height;
	const textScale = Math.min(scaleX, scaleY);
	return {
		schemaVersion: 1,
		elements: layout.elements.map((element) => {
			const uniformSize = element.type === 'image' || element.type === 'qr';
			const scaled = withFrame(element, {
				x: element.frame.x * scaleX,
				y: element.frame.y * scaleY,
				width: element.frame.width * (uniformSize ? textScale : scaleX),
				height: element.frame.height * (uniformSize ? textScale : scaleY)
			});
			if (scaled.type !== 'text') return scaled;
			return {
				...scaled,
				fontSize: scaled.fontSize * textScale,
				minFontSize: scaled.minFontSize * textScale,
				shadow: scaled.shadow
					? {
							offsetX: scaled.shadow.offsetX * textScale,
							offsetY: scaled.shadow.offsetY * textScale,
							blur: scaled.shadow.blur * textScale,
							color: scaled.shadow.color
						}
					: null
			};
		})
	};
}

export function resetCertificateLayout(): CertificateLayout {
	return { schemaVersion: 1, elements: [] };
}

export function fitEditorZoom(
	containerWidth: number,
	containerHeight: number,
	pageWidth: number,
	pageHeight: number,
	padding = 64
): number {
	if (containerWidth <= 0 || containerHeight <= 0 || pageWidth <= 0 || pageHeight <= 0) return 1;
	const availableWidth = Math.max(1, containerWidth - padding);
	const availableHeight = Math.max(1, containerHeight - padding);
	const zoom = Math.min(2, Math.max(0.1, availableWidth / pageWidth, 0));
	const fitted = Math.min(zoom, availableHeight / pageHeight);
	return Math.round(Math.max(0.1, fitted) * 1000) / 1000;
}

const zoomSteps = [0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.25, 1.5, 2] as const;

export function stepEditorZoom(current: number, direction: 'in' | 'out'): number {
	assertZoom(current);
	if (direction === 'in')
		return zoomSteps.find((step) => step > current + 0.001) ?? zoomSteps.at(-1)!;
	return [...zoomSteps].reverse().find((step) => step < current - 0.001) ?? zoomSteps[0];
}

export function createTextElement(
	page: PagePointSize,
	createId: () => string = () => crypto.randomUUID()
): TextCertificateElement {
	const width = Math.min(360, page.width * 0.72);
	return {
		type: 'text',
		id: createId(),
		content: 'มอบให้ {ชื่อ} {นามสกุล}',
		frame: { x: (page.width - width) / 2, y: page.height * 0.4, width, height: 64 },
		rotation: 0,
		fontSource: { type: 'built_in' },
		fontFamily: 'Sarabun',
		fontWeight: 400,
		fontStyle: 'normal',
		fontSize: 28,
		minFontSize: 14,
		color: '#183153',
		alignment: 'center',
		lineHeight: 1.2,
		autoShrink: true,
		shadow: null
	};
}

export function createQrElement(
	page: PagePointSize,
	createId: () => string = () => crypto.randomUUID()
): Extract<CertificateElement, { type: 'qr' }> {
	const size = Math.min(86, page.width * 0.14, page.height * 0.18);
	return {
		type: 'qr',
		id: createId(),
		frame: { x: page.width - size - 28, y: page.height - size - 28, width: size, height: size },
		rotation: 0
	};
}

export function createImageElement(
	page: PagePointSize,
	assetId: string,
	aspectRatio: number,
	createId: () => string = () => crypto.randomUUID()
): ImageCertificateElement {
	if (!Number.isFinite(aspectRatio) || aspectRatio <= 0) {
		throw new Error('image aspect ratio must be positive');
	}
	const maximumWidth = Math.min(110, page.width * 0.2);
	const maximumHeight = Math.min(110, page.height * 0.24);
	const width = Math.min(maximumWidth, maximumHeight * aspectRatio);
	const height = width / aspectRatio;
	return {
		type: 'image',
		id: createId(),
		assetId,
		frame: { x: (page.width - width) / 2, y: page.height * 0.18, width, height },
		rotation: 0,
		lockAspectRatio: true,
		aspectRatio
	};
}

export function imageAssetAspectRatio(
	asset: Pick<
		CertificateTemplateDetail['assets'][number],
		'kind' | 'imageWidthPixels' | 'imageHeightPixels'
	>
): number {
	if (
		asset.kind !== 'image' ||
		asset.imageWidthPixels === null ||
		asset.imageHeightPixels === null ||
		asset.imageWidthPixels <= 0 ||
		asset.imageHeightPixels <= 0
	) {
		throw new Error('image source dimensions are unavailable');
	}
	return asset.imageWidthPixels / asset.imageHeightPixels;
}

function applyImageAspectRatio(
	element: ImageCertificateElement,
	aspectRatio: number,
	page: PagePointSize
): ImageCertificateElement {
	if (!Number.isFinite(aspectRatio) || aspectRatio <= 0) {
		throw new Error('image aspect ratio must be positive');
	}
	const centerY = element.frame.y + element.frame.height / 2;
	const height = element.frame.width / aspectRatio;
	return constrainElementToPage(
		{
			...element,
			lockAspectRatio: true,
			aspectRatio,
			frame: {
				...cloneFrame(element.frame),
				y: centerY - height / 2,
				height
			}
		},
		page
	) as ImageCertificateElement;
}

export function setImageAspectRatioLock(
	element: ImageCertificateElement,
	locked: boolean,
	page: PagePointSize
): ImageCertificateElement {
	if (!locked) return { ...element, lockAspectRatio: false };
	return applyImageAspectRatio(element, element.aspectRatio, page);
}

export function resetImageAspectRatio(
	element: ImageCertificateElement,
	sourceAspectRatio: number,
	page: PagePointSize
): ImageCertificateElement {
	return applyImageAspectRatio(element, sourceAspectRatio, page);
}
