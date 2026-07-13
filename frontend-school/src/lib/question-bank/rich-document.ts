import type {
	RichContent,
	RichContentBlock,
	RichInlineNode,
	RichTextMark
} from '$lib/api/questionBank';

export type EditorImageAttributes = {
	fileId: string | null;
	pendingId: string | null;
	previewUrl: string | null;
	altText: string | null;
	caption: string | null;
	alignment: 'left' | 'center' | 'right';
	widthPercent: number;
};

export type EditorRichBlock =
	| { type: 'paragraph'; content?: RichInlineNode[] }
	| { type: 'math_block'; attrs: { latex: string } }
	| { type: 'image'; attrs: EditorImageAttributes };

export interface EditorRichContent {
	schemaVersion: 1;
	document: {
		type: 'doc';
		content: EditorRichBlock[];
	};
}

export interface PendingImageReference {
	pendingId: string;
	previewUrl: string;
}

export function emptyRichContent(): RichContent {
	return {
		schemaVersion: 1,
		document: { type: 'doc', content: [] }
	};
}

export function emptyEditorRichContent(): EditorRichContent {
	return {
		schemaVersion: 1,
		document: { type: 'doc', content: [{ type: 'paragraph' }] }
	};
}

export function toEditorRichContent(
	content: RichContent | null | undefined,
	fileUrls: ReadonlyMap<string, string>
): EditorRichContent {
	const blocks = (content ?? emptyRichContent()).document.content.map((block): EditorRichBlock => {
		if (block.type !== 'image') return cloneBlock(block);
		return {
			type: 'image',
			attrs: {
				...block.attrs,
				pendingId: null,
				previewUrl: fileUrls.get(block.attrs.fileId) ?? null
			}
		};
	});
	return {
		schemaVersion: 1,
		document: {
			type: 'doc',
			content: blocks.length ? blocks : [{ type: 'paragraph' }]
		}
	};
}

export function toPersistedRichContent(
	content: EditorRichContent,
	uploadedFileIds: ReadonlyMap<string, string>
): RichContent {
	const blocks: RichContentBlock[] = [];
	for (const block of content.document.content) {
		if (block.type === 'paragraph') {
			const inline = normalizeInlineNodes(block.content ?? []);
			blocks.push(inline.length ? { type: 'paragraph', content: inline } : { type: 'paragraph' });
			continue;
		}
		if (block.type === 'math_block') {
			if (block.attrs.latex.trim()) {
				blocks.push({ type: 'math_block', attrs: { latex: block.attrs.latex } });
			}
			continue;
		}
		const fileId = block.attrs.fileId ?? uploadedFileIds.get(block.attrs.pendingId ?? '');
		if (!fileId) throw new Error('ยังอัปโหลดรูปประกอบไม่ครบ');
		blocks.push({
			type: 'image',
			attrs: {
				fileId,
				altText: normalizedOptionalText(block.attrs.altText),
				caption: normalizedOptionalText(block.attrs.caption),
				alignment: block.attrs.alignment,
				widthPercent: clampImageWidth(block.attrs.widthPercent)
			}
		});
	}
	return {
		schemaVersion: 1,
		document: { type: 'doc', content: blocks }
	};
}

export function richContentHasBody(content: EditorRichContent | RichContent): boolean {
	return content.document.content.some((block) => {
		if (block.type === 'image') return true;
		if (block.type === 'math_block') return Boolean(block.attrs.latex.trim());
		return (block.content ?? []).some((node) => {
			if (node.type === 'text') return Boolean(node.text.trim());
			if (node.type === 'inline_math') return Boolean(node.attrs.latex.trim());
			return false;
		});
	});
}

export function richContentPlainText(content: EditorRichContent | RichContent): string {
	const parts: string[] = [];
	for (const block of content.document.content) {
		if (block.type === 'image') {
			if (block.attrs.altText?.trim()) parts.push(block.attrs.altText.trim());
			if (block.attrs.caption?.trim()) parts.push(block.attrs.caption.trim());
			continue;
		}
		if (block.type === 'math_block') {
			if (block.attrs.latex.trim()) parts.push(block.attrs.latex.trim());
			continue;
		}
		for (const node of block.content ?? []) {
			if (node.type === 'text' && node.text.trim()) parts.push(node.text.trim());
			if (node.type === 'inline_math' && node.attrs.latex.trim()) {
				parts.push(node.attrs.latex.trim());
			}
		}
	}
	return parts.join(' ');
}

export function contentHasImage(content: EditorRichContent | RichContent): boolean {
	return content.document.content.some((block) => block.type === 'image');
}

export function contentHasMath(content: EditorRichContent | RichContent): boolean {
	return content.document.content.some((block) => {
		if (block.type === 'math_block') return Boolean(block.attrs.latex.trim());
		return (
			block.type === 'paragraph' &&
			(block.content ?? []).some(
				(node) => node.type === 'inline_math' && Boolean(node.attrs.latex.trim())
			)
		);
	});
}

export function pendingImageIds(content: EditorRichContent): Set<string> {
	return new Set(
		content.document.content.flatMap((block) =>
			block.type === 'image' && block.attrs.pendingId ? [block.attrs.pendingId] : []
		)
	);
}

function cloneBlock(block: Exclude<RichContentBlock, { type: 'image' }>): EditorRichBlock {
	if (block.type === 'math_block') {
		return { type: 'math_block', attrs: { ...block.attrs } };
	}
	return {
		type: 'paragraph',
		content: block.content?.map(cloneInlineNode)
	};
}

function cloneInlineNode(node: RichInlineNode): RichInlineNode {
	if (node.type === 'text') {
		return { type: 'text', text: node.text, marks: cloneMarks(node.marks) };
	}
	if (node.type === 'inline_math') {
		return { type: 'inline_math', attrs: { ...node.attrs } };
	}
	return { type: 'hardBreak' };
}

function cloneMarks(marks: RichTextMark[] | undefined) {
	return marks?.map((mark) => ({ ...mark }));
}

function normalizeInlineNodes(nodes: RichInlineNode[]): RichInlineNode[] {
	return nodes.flatMap((node): RichInlineNode[] => {
		if (node.type === 'text') {
			return node.text ? [{ type: 'text', text: node.text, marks: cloneMarks(node.marks) }] : [];
		}
		if (node.type === 'inline_math') {
			return node.attrs.latex.trim()
				? [{ type: 'inline_math', attrs: { latex: node.attrs.latex } }]
				: [];
		}
		return [{ type: 'hardBreak' }];
	});
}

function normalizedOptionalText(value: string | null): string | null {
	const normalized = value?.trim();
	return normalized || null;
}

function clampImageWidth(value: number) {
	if (!Number.isFinite(value)) return 60;
	return Math.min(100, Math.max(10, Math.round(value)));
}
