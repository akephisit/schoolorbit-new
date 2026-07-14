import {
	AlignmentType,
	convertMillimetersToTwip,
	Document,
	ImageRun,
	Packer,
	PageBreak,
	Paragraph,
	TextRun,
	type ParagraphChild
} from 'docx';
import { toPng } from 'html-to-image';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import {
	getQuestionBankQuestionFile,
	type QuestionDetail,
	type QuestionFile,
	type RichContent,
	type RichInlineNode
} from '$lib/api/questionBank';
import { katexFontEmbedCss } from '$lib/question-bank/katex-fonts';

export interface QuestionBankWordExportOptions {
	title: string;
	includeAnswerKey: boolean;
}

type RasterAsset = {
	data: Uint8Array;
	width: number;
	height: number;
};

type ExportContext = {
	formulaCache: Map<string, Promise<RasterAsset>>;
	imageCache: Map<string, Promise<RasterAsset>>;
};

type ContentParagraphOptions = {
	prefix?: string;
	indent?: number;
	spacingAfter?: number;
};

const bodyFont = 'TH Sarabun New';
const bodyFontSize = 32;
const titleFontSize = 40;
const usablePageWidthPixels = 640;
const formulaFontPixels = (bodyFontSize / 2) * (96 / 72);
const defaultLineSpacing = 360;
const answerIndent = convertMillimetersToTwip(8);

export async function exportQuestionBankWord(
	questions: QuestionDetail[],
	options: QuestionBankWordExportOptions
): Promise<string> {
	const blob = await buildQuestionBankWordBlob(questions, options);
	const fileName = `${safeFileName(options.title) || 'ชุดข้อสอบ'}.docx`;
	downloadBlob(blob, fileName);
	return fileName;
}

export async function buildQuestionBankWordBlob(
	questions: QuestionDetail[],
	options: QuestionBankWordExportOptions
): Promise<Blob> {
	if (questions.length === 0) throw new Error('ยังไม่ได้เลือกข้อสอบสำหรับส่งออก');

	const context: ExportContext = {
		formulaCache: new Map(),
		imageCache: new Map()
	};
	const children: Paragraph[] = [
		new Paragraph({
			alignment: AlignmentType.CENTER,
			spacing: { after: 120 },
			children: [wordText(options.title.trim() || 'ชุดข้อสอบ', { bold: true, size: titleFontSize })]
		}),
		new Paragraph({
			alignment: AlignmentType.CENTER,
			spacing: { after: 320 },
			children: [wordText(`จำนวน ${questions.length} ข้อ`)]
		})
	];

	for (const [index, question] of questions.entries()) {
		children.push(...(await buildQuestion(question, index, context)));
	}

	if (options.includeAnswerKey) {
		children.push(...(await buildAnswerKey(questions, context)));
	}

	const document = new Document({
		creator: 'SchoolOrbit',
		title: options.title.trim() || 'ชุดข้อสอบ',
		description: 'ส่งออกจากคลังข้อสอบ SchoolOrbit',
		styles: {
			default: {
				document: {
					run: {
						font: bodyFont,
						size: bodyFontSize,
						sizeComplexScript: bodyFontSize,
						color: '000000'
					},
					paragraph: {
						spacing: { line: defaultLineSpacing, after: 80 }
					}
				}
			}
		},
		sections: [
			{
				properties: {
					page: {
						size: {
							width: convertMillimetersToTwip(210),
							height: convertMillimetersToTwip(297)
						},
						margin: {
							top: convertMillimetersToTwip(18),
							right: convertMillimetersToTwip(20),
							bottom: convertMillimetersToTwip(18),
							left: convertMillimetersToTwip(20)
						}
					}
				},
				children
			}
		]
	});

	return Packer.toBlob(document);
}

async function buildQuestion(
	question: QuestionDetail,
	index: number,
	context: ExportContext
): Promise<Paragraph[]> {
	const files = fileMap(question.files);
	const paragraphs = await buildContentParagraphs(
		question.id,
		question.stemContent,
		files,
		context,
		{
			prefix: `${index + 1}. `,
			spacingAfter: question.choices.length ? 40 : 160
		}
	);

	for (const choice of question.choices) {
		paragraphs.push(
			...(await buildContentParagraphs(question.id, choice.content, files, context, {
				prefix: `${choice.label}. `,
				indent: answerIndent,
				spacingAfter: 40
			}))
		);
	}

	paragraphs.push(new Paragraph({ spacing: { after: 120 }, children: [] }));
	return paragraphs;
}

async function buildAnswerKey(
	questions: QuestionDetail[],
	context: ExportContext
): Promise<Paragraph[]> {
	const paragraphs: Paragraph[] = [
		new Paragraph({ children: [new PageBreak()] }),
		new Paragraph({
			alignment: AlignmentType.CENTER,
			spacing: { after: 240 },
			children: [wordText('เฉลย', { bold: true, size: titleFontSize })]
		})
	];

	for (const [index, question] of questions.entries()) {
		const correctChoices = question.choices
			.filter((choice) => choice.isCorrect)
			.map((choice) => choice.label)
			.join(', ');
		paragraphs.push(
			new Paragraph({
				spacing: { before: 80, after: 40 },
				children: [
					wordText(`ข้อ ${index + 1}`, { bold: true }),
					...(correctChoices ? [wordText(`: ${correctChoices}`)] : [])
				]
			})
		);

		const files = fileMap(question.files);
		if (hasContent(question.explanationContent)) {
			paragraphs.push(
				new Paragraph({
					indent: { left: answerIndent },
					spacing: { after: 20 },
					children: [wordText('คำอธิบาย', { bold: true })]
				}),
				...(await buildContentParagraphs(question.id, question.explanationContent, files, context, {
					indent: answerIndent,
					spacingAfter: 40
				}))
			);
		}
		if (hasContent(question.rubricContent)) {
			paragraphs.push(
				new Paragraph({
					indent: { left: answerIndent },
					spacing: { after: 20 },
					children: [wordText('เกณฑ์ให้คะแนน', { bold: true })]
				}),
				...(await buildContentParagraphs(question.id, question.rubricContent, files, context, {
					indent: answerIndent,
					spacingAfter: 40
				}))
			);
		}
	}

	return paragraphs;
}

async function buildContentParagraphs(
	questionId: string,
	content: RichContent,
	files: ReadonlyMap<string, QuestionFile>,
	context: ExportContext,
	options: ContentParagraphOptions = {}
): Promise<Paragraph[]> {
	const paragraphs: Paragraph[] = [];
	let prefixPending = Boolean(options.prefix);
	const indent = options.indent ? { left: options.indent } : undefined;

	for (const block of content.document.content) {
		if (block.type === 'paragraph') {
			const children = await buildInlineRuns(block.content ?? [], context);
			if (prefixPending) {
				children.unshift(wordText(options.prefix ?? '', { bold: true }));
				prefixPending = false;
			}
			paragraphs.push(
				new Paragraph({
					children,
					indent,
					spacing: {
						line: defaultLineSpacing,
						after: options.spacingAfter ?? 80
					}
				})
			);
			continue;
		}

		if (prefixPending) {
			paragraphs.push(
				new Paragraph({
					indent,
					spacing: { after: 40 },
					children: [wordText(options.prefix ?? '', { bold: true })]
				})
			);
			prefixPending = false;
		}

		if (block.type === 'math_block') {
			const formula = await formulaImage(block.attrs.latex, true, context);
			paragraphs.push(
				new Paragraph({
					alignment: AlignmentType.CENTER,
					indent,
					spacing: { before: 40, after: options.spacingAfter ?? 100 },
					children: [formulaRun(formula, block.attrs.latex)]
				})
			);
			continue;
		}

		const file = files.get(block.attrs.fileId);
		if (!file) throw new Error('ไม่พบไฟล์รูปประกอบของข้อสอบที่เลือก');
		const image = await questionImage(questionId, file, block.attrs.widthPercent, context);
		paragraphs.push(
			new Paragraph({
				alignment: imageAlignment(block.attrs.alignment),
				indent,
				spacing: { before: 60, after: block.attrs.caption ? 20 : (options.spacingAfter ?? 100) },
				children: [
					new ImageRun({
						type: 'png',
						data: image.data,
						transformation: { width: image.width, height: image.height },
						altText: {
							name: block.attrs.altText || 'รูปประกอบโจทย์',
							description: block.attrs.altText || 'รูปประกอบโจทย์'
						}
					})
				]
			})
		);
		if (block.attrs.caption) {
			paragraphs.push(
				new Paragraph({
					alignment: imageAlignment(block.attrs.alignment),
					indent,
					spacing: { after: options.spacingAfter ?? 100 },
					children: [wordText(block.attrs.caption, { italics: true, size: 28 })]
				})
			);
		}
	}

	if (prefixPending) {
		paragraphs.push(
			new Paragraph({
				indent,
				spacing: { after: options.spacingAfter ?? 80 },
				children: [wordText(options.prefix ?? '', { bold: true })]
			})
		);
	}
	return paragraphs;
}

async function buildInlineRuns(
	nodes: RichInlineNode[],
	context: ExportContext
): Promise<ParagraphChild[]> {
	const children: ParagraphChild[] = [];
	for (const node of nodes) {
		if (node.type === 'text') {
			children.push(
				wordText(node.text, {
					bold: hasMark(node, 'bold'),
					italics: hasMark(node, 'italic')
				})
			);
			continue;
		}
		if (node.type === 'hardBreak') {
			children.push(wordText('', { break: 1 }));
			continue;
		}
		const formula = await formulaImage(node.attrs.latex, false, context);
		children.push(formulaRun(formula, node.attrs.latex));
	}
	return children;
}

function formulaRun(asset: RasterAsset, latex: string) {
	return new ImageRun({
		type: 'png',
		data: asset.data,
		transformation: { width: asset.width, height: asset.height },
		altText: {
			name: 'สมการคณิตศาสตร์',
			description: latex,
			title: latex
		}
	});
}

function formulaImage(latex: string, display: boolean, context: ExportContext) {
	const cacheKey = `${display ? 'display' : 'inline'}:${latex}`;
	const cached = context.formulaCache.get(cacheKey);
	if (cached) return cached;
	const pending = renderFormulaImage(latex, display);
	context.formulaCache.set(cacheKey, pending);
	return pending;
}

async function renderFormulaImage(latex: string, display: boolean): Promise<RasterAsset> {
	const host = document.createElement('span');
	host.style.position = 'fixed';
	host.style.left = '-100000px';
	host.style.top = '0';
	host.style.display = 'inline-block';
	host.style.padding = '2px 3px';
	host.style.background = 'transparent';
	host.style.color = '#000000';
	host.style.fontSize = `${formulaFontPixels}px`;
	host.style.lineHeight = '1.2';
	host.style.whiteSpace = 'nowrap';
	document.body.appendChild(host);

	try {
		katex.render(latex, host, {
			displayMode: display,
			throwOnError: false,
			strict: 'warn',
			trust: false,
			output: 'html'
		});
		const displayWrapper = host.querySelector<HTMLElement>('.katex-display');
		if (displayWrapper) {
			displayWrapper.style.display = 'inline-block';
			displayWrapper.style.margin = '0';
		}
		const rendered = host.querySelector<HTMLElement>('.katex');
		if (rendered) {
			rendered.style.fontSize = '1em';
			rendered.style.lineHeight = '1.2';
		}
		await document.fonts.ready;
		await nextAnimationFrame();
		const bounds = host.getBoundingClientRect();
		const naturalWidth = Math.max(1, Math.ceil(bounds.width));
		const naturalHeight = Math.max(1, Math.ceil(bounds.height));
		const displayWidth = Math.min(usablePageWidthPixels, naturalWidth);
		const displayHeight = Math.max(1, Math.round(naturalHeight * (displayWidth / naturalWidth)));
		const dataUrl = await toPng(host, {
			backgroundColor: 'transparent',
			pixelRatio: 3,
			preferredFontFormat: 'woff2',
			fontEmbedCSS: katexFontEmbedCss
		});
		return { data: dataUrlBytes(dataUrl), width: displayWidth, height: displayHeight };
	} finally {
		host.remove();
	}
}

function questionImage(
	questionId: string,
	file: QuestionFile,
	widthPercent: number,
	context: ExportContext
): Promise<RasterAsset> {
	const normalizedWidth = Math.min(100, Math.max(10, Math.round(widthPercent)));
	const cacheKey = `${questionId}:${file.id}:${normalizedWidth}`;
	const cached = context.imageCache.get(cacheKey);
	if (cached) return cached;
	const pending = renderQuestionImage(questionId, file.id, normalizedWidth);
	context.imageCache.set(cacheKey, pending);
	return pending;
}

async function renderQuestionImage(
	questionId: string,
	fileId: string,
	widthPercent: number
): Promise<RasterAsset> {
	let blob: Blob;
	try {
		blob = await getQuestionBankQuestionFile(questionId, fileId);
	} catch {
		throw new Error('ไม่สามารถดาวน์โหลดรูปประกอบเพื่อใส่ในไฟล์ Word ได้');
	}
	const source = await decodeImage(blob);
	try {
		const targetWidth = Math.round(usablePageWidthPixels * (widthPercent / 100));
		const displayWidth = Math.max(1, Math.min(source.width, targetWidth));
		const displayHeight = Math.max(1, Math.round(displayWidth * (source.height / source.width)));
		const rasterWidth = Math.max(1, Math.min(source.width, displayWidth * 2));
		const rasterHeight = Math.max(1, Math.round(rasterWidth * (source.height / source.width)));
		const canvas = document.createElement('canvas');
		canvas.width = rasterWidth;
		canvas.height = rasterHeight;
		const context = canvas.getContext('2d');
		if (!context) throw new Error('เบราว์เซอร์ไม่รองรับการเตรียมรูปสำหรับ Word');
		context.drawImage(source.element, 0, 0, rasterWidth, rasterHeight);
		const png = await canvasBlob(canvas);
		return {
			data: new Uint8Array(await png.arrayBuffer()),
			width: displayWidth,
			height: displayHeight
		};
	} finally {
		source.release();
	}
}

async function decodeImage(blob: Blob): Promise<{
	element: CanvasImageSource;
	width: number;
	height: number;
	release: () => void;
}> {
	if ('createImageBitmap' in window) {
		try {
			const bitmap = await createImageBitmap(blob);
			return {
				element: bitmap,
				width: bitmap.width,
				height: bitmap.height,
				release: () => bitmap.close()
			};
		} catch {
			// Fall through to the image element path for formats unsupported by createImageBitmap.
		}
	}

	const objectUrl = URL.createObjectURL(blob);
	const image = new Image();
	try {
		await new Promise<void>((resolve, reject) => {
			image.onload = () => resolve();
			image.onerror = () => reject(new Error('ไม่สามารถอ่านรูปประกอบได้'));
			image.src = objectUrl;
		});
		return {
			element: image,
			width: image.naturalWidth,
			height: image.naturalHeight,
			release: () => URL.revokeObjectURL(objectUrl)
		};
	} catch (error) {
		URL.revokeObjectURL(objectUrl);
		throw error;
	}
}

function canvasBlob(canvas: HTMLCanvasElement): Promise<Blob> {
	return new Promise((resolve, reject) => {
		canvas.toBlob((blob) => {
			if (blob) resolve(blob);
			else reject(new Error('ไม่สามารถแปลงรูปประกอบสำหรับ Word ได้'));
		}, 'image/png');
	});
}

function wordText(
	text: string,
	options: {
		bold?: boolean;
		italics?: boolean;
		size?: number;
		break?: number;
	} = {}
) {
	return new TextRun({
		text,
		bold: options.bold,
		boldComplexScript: options.bold,
		italics: options.italics,
		italicsComplexScript: options.italics,
		font: bodyFont,
		size: options.size ?? bodyFontSize,
		sizeComplexScript: options.size ?? bodyFontSize,
		break: options.break
	});
}

function hasMark(node: Extract<RichInlineNode, { type: 'text' }>, type: 'bold' | 'italic') {
	return node.marks?.some((mark) => mark.type === type) ?? false;
}

function hasContent(content: RichContent | null | undefined): content is RichContent {
	return Boolean(
		content?.document.content.some((block) => {
			if (block.type === 'image') return true;
			if (block.type === 'math_block') return Boolean(block.attrs.latex.trim());
			return (block.content ?? []).some((node) => {
				if (node.type === 'text') return Boolean(node.text.trim());
				if (node.type === 'inline_math') return Boolean(node.attrs.latex.trim());
				return node.type === 'hardBreak';
			});
		})
	);
}

function fileMap(files: QuestionFile[]) {
	return new Map(files.map((file) => [file.id, file]));
}

function imageAlignment(alignment: 'left' | 'center' | 'right') {
	if (alignment === 'center') return AlignmentType.CENTER;
	if (alignment === 'right') return AlignmentType.RIGHT;
	return AlignmentType.LEFT;
}

function dataUrlBytes(dataUrl: string) {
	const base64 = dataUrl.slice(dataUrl.indexOf(',') + 1);
	const binary = atob(base64);
	const bytes = new Uint8Array(binary.length);
	for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
	return bytes;
}

function safeFileName(value: string) {
	return value
		.trim()
		.replace(/[\\/:*?"<>|]/g, '-')
		.replace(/\s+/g, ' ')
		.slice(0, 120);
}

function downloadBlob(blob: Blob, fileName: string) {
	const url = URL.createObjectURL(blob);
	const link = document.createElement('a');
	link.href = url;
	link.download = fileName;
	document.body.appendChild(link);
	link.click();
	link.remove();
	// Keep the object URL alive long enough for browsers to begin reading larger DOCX files.
	window.setTimeout(() => URL.revokeObjectURL(url), 30_000);
}

function nextAnimationFrame() {
	return new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
}
