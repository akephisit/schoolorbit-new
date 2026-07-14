import {
	AlignmentType,
	convertMillimetersToTwip,
	Document,
	ImageRun,
	ImportedXmlComponent,
	Packer,
	PageBreak,
	Paragraph,
	TextRun,
	type ParagraphChild
} from 'docx';
import { convertLatexToMathMl } from 'mathlive/ssr';
import { mml2omml } from 'mathml2omml';
import type {
	QuestionDetail,
	QuestionFile,
	RichContent,
	RichInlineNode
} from '$lib/api/questionBank';

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
			const children = buildInlineRuns(block.content ?? []);
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
			paragraphs.push(
				new Paragraph({
					alignment: AlignmentType.CENTER,
					indent,
					spacing: { before: 40, after: options.spacingAfter ?? 100 },
					children: [wordFormula(block.attrs.latex)]
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

function buildInlineRuns(nodes: RichInlineNode[]): ParagraphChild[] {
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
		children.push(wordFormula(node.attrs.latex));
	}
	return children;
}

function wordFormula(latex: string): ParagraphChild {
	const mathMl = `<math xmlns="http://www.w3.org/1998/Math/MathML">${convertLatexToMathMl(latex)}</math>`;
	const omml = mml2omml(mathMl);
	const xmlDocument = new DOMParser().parseFromString(omml, 'application/xml');
	if (xmlDocument.querySelector('parsererror')) {
		throw new Error('ไม่สามารถแปลงสมการเป็นรูปแบบ Word Math ได้');
	}

	// docx accepts imported XML components as paragraph children at runtime, but its public
	// ParagraphChild union does not include ImportedXmlComponent.
	return importXmlElement(xmlDocument.documentElement) as unknown as ParagraphChild;
}

function importXmlElement(element: Element): ImportedXmlComponent {
	const attributes: Record<string, string> = {};
	for (const attribute of element.attributes) attributes[attribute.name] = attribute.value;

	const component = new ImportedXmlComponent(element.tagName, attributes);
	for (const child of element.childNodes) {
		if (child.nodeType === 1) component.push(importXmlElement(child as Element));
		else if (child.nodeType === 3 && child.nodeValue) component.push(child.nodeValue);
	}
	return component;
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
		const { getQuestionBankQuestionFile } = await import('$lib/api/questionBank');
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
