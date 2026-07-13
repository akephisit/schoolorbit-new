import 'mathlive';
import 'mathlive/fonts.css';
import { Node as TiptapNode, mergeAttributes, type Editor } from '@tiptap/core';
import FileHandler from '@tiptap/extension-file-handler';
import Placeholder from '@tiptap/extension-placeholder';
import StarterKit from '@tiptap/starter-kit';
import type { MathfieldElement } from 'mathlive';
import type { PendingImageReference } from './rich-document';

export type MathFocusTarget = {
	field: MathfieldElement;
	getPosition: () => number | undefined;
};

export type QuestionEditorExtensionOptions = {
	onImageFile: (file: File) => PendingImageReference | null;
	onMathFocus: (target: MathFocusTarget) => void;
	placeholder: string;
};

export const QuestionDocument = TiptapNode.create({
	name: 'doc',
	topNode: true,
	content: 'block+'
});

function createMathNode(name: 'inline_math' | 'math_block', inline: boolean) {
	return TiptapNode.create<{
		onMathFocus?: QuestionEditorExtensionOptions['onMathFocus'];
	}>({
		name,
		group: inline ? 'inline' : 'block',
		inline,
		atom: true,
		selectable: true,

		addOptions() {
			return { onMathFocus: undefined };
		},

		addAttributes() {
			return {
				latex: { default: '' }
			};
		},

		parseHTML() {
			return [{ tag: inline ? 'span[data-inline-math]' : 'div[data-math-block]' }];
		},

		renderHTML({ HTMLAttributes }) {
			return [
				inline ? 'span' : 'div',
				mergeAttributes(
					inline ? { 'data-inline-math': '' } : { 'data-math-block': '' },
					HTMLAttributes
				)
			];
		},

		addNodeView() {
			const onMathFocus = this.options.onMathFocus as
				| QuestionEditorExtensionOptions['onMathFocus']
				| undefined;
			return ({ node, editor, getPos }) => {
				let currentNode = node;
				const container = document.createElement(inline ? 'span' : 'div');
				container.className = inline ? 'question-inline-math' : 'question-math-block';
				container.contentEditable = 'false';
				const field = document.createElement('math-field') as MathfieldElement;
				field.className = inline ? 'question-inline-math-field' : 'question-block-math-field';
				field.value = String(node.attrs.latex ?? '');
				field.smartFence = true;
				field.mathVirtualKeyboardPolicy = 'manual';
				field.setAttribute('aria-label', inline ? 'สมการในบรรทัด' : 'สมการแยกบรรทัด');
				container.append(field);

				const position = () => {
					const value = getPos();
					return typeof value === 'number' ? value : undefined;
				};
				const handleInput = () => {
					const pos = position();
					if (pos === undefined || field.value === currentNode.attrs.latex) return;
					const transaction = editor.view.state.tr.setNodeMarkup(pos, undefined, {
						...currentNode.attrs,
						latex: field.value
					});
					editor.view.dispatch(transaction);
				};
				const handleFocus = () => {
					onMathFocus?.({ field, getPosition: position });
					if (window.mathVirtualKeyboard) {
						window.mathVirtualKeyboard.layouts = ['numeric', 'symbols', 'greek'];
					}
				};
				field.addEventListener('input', handleInput);
				field.addEventListener('focusin', handleFocus);

				if (!field.value && editor.isFocused) {
					requestAnimationFrame(() => field.focus());
				}

				return {
					dom: container,
					update(updatedNode) {
						if (updatedNode.type.name !== name) return false;
						currentNode = updatedNode;
						const latex = String(updatedNode.attrs.latex ?? '');
						if (field.value !== latex) field.value = latex;
						return true;
					},
					stopEvent: (event) => container.contains(event.target as globalThis.Node),
					ignoreMutation: () => true,
					destroy() {
						field.removeEventListener('input', handleInput);
						field.removeEventListener('focusin', handleFocus);
					}
				};
			};
		}
	});
}

export const InlineMath = createMathNode('inline_math', true);
export const MathBlock = createMathNode('math_block', false);

export const QuestionImage = TiptapNode.create({
	name: 'image',
	group: 'block',
	atom: true,
	selectable: true,
	draggable: true,

	addAttributes() {
		return {
			fileId: { default: null },
			pendingId: { default: null },
			previewUrl: { default: null },
			altText: { default: null },
			caption: { default: null },
			alignment: { default: 'center' },
			widthPercent: { default: 60 }
		};
	},

	parseHTML() {
		return [{ tag: 'figure[data-question-image]' }];
	},

	renderHTML({ HTMLAttributes }) {
		return ['figure', mergeAttributes({ 'data-question-image': '' }, HTMLAttributes)];
	},

	addNodeView() {
		return ({ node, editor, getPos }) => {
			let currentNode = node;
			const figure = document.createElement('figure');
			figure.className = 'question-editor-image';
			figure.contentEditable = 'false';
			figure.draggable = true;

			const dragHandle = document.createElement('div');
			dragHandle.className = 'question-editor-image-handle';
			dragHandle.dataset.dragHandle = '';
			dragHandle.textContent = '⋮⋮ ลากรูปเพื่อย้ายตำแหน่ง';
			const image = document.createElement('img');
			image.dataset.dragHandle = '';
			image.draggable = true;
			const controls = document.createElement('div');
			controls.className = 'question-editor-image-controls';

			const altInput = document.createElement('input');
			altInput.type = 'text';
			altInput.placeholder = 'คำอธิบายรูปสำหรับโปรแกรมอ่านจอ';
			altInput.setAttribute('aria-label', 'คำอธิบายรูป');
			const captionInput = document.createElement('input');
			captionInput.type = 'text';
			captionInput.placeholder = 'คำบรรยายใต้รูป (ไม่บังคับ)';
			captionInput.setAttribute('aria-label', 'คำบรรยายใต้รูป');

			const alignment = document.createElement('select');
			alignment.setAttribute('aria-label', 'ตำแหน่งรูป');
			for (const [value, label] of [
				['left', 'ชิดซ้าย'],
				['center', 'กึ่งกลาง'],
				['right', 'ชิดขวา']
			]) {
				const option = document.createElement('option');
				option.value = value;
				option.textContent = label;
				alignment.append(option);
			}
			const width = document.createElement('input');
			width.type = 'range';
			width.min = '10';
			width.max = '100';
			width.step = '5';
			width.setAttribute('aria-label', 'ความกว้างรูป');
			const widthLabel = document.createElement('span');
			const removeButton = document.createElement('button');
			removeButton.type = 'button';
			removeButton.textContent = 'นำรูปออก';

			controls.append(altInput, captionInput, alignment, width, widthLabel, removeButton);
			figure.append(dragHandle, image, controls);

			const updateAttributes = (attrs: Record<string, unknown>) => {
				const pos = getPos();
				if (typeof pos !== 'number') return;
				editor.view.dispatch(
					editor.view.state.tr.setNodeMarkup(pos, undefined, { ...currentNode.attrs, ...attrs })
				);
			};
			const refresh = () => {
				const attrs = currentNode.attrs;
				image.src = String(attrs.previewUrl ?? '');
				image.alt = String(attrs.altText ?? '');
				altInput.value = String(attrs.altText ?? '');
				captionInput.value = String(attrs.caption ?? '');
				alignment.value = String(attrs.alignment ?? 'center');
				const widthPercent = Number(attrs.widthPercent ?? 60);
				width.value = String(widthPercent);
				widthLabel.textContent = `${widthPercent}%`;
				figure.dataset.alignment = alignment.value;
				figure.style.setProperty('--question-image-width', `${widthPercent}%`);
			};
			const handleAlt = () => updateAttributes({ altText: altInput.value || null });
			const handleCaption = () => updateAttributes({ caption: captionInput.value || null });
			const handleAlignment = () => updateAttributes({ alignment: alignment.value });
			const handleWidth = () => updateAttributes({ widthPercent: Number(width.value) });
			const handleRemove = () => {
				const pos = getPos();
				if (typeof pos !== 'number') return;
				editor.chain().focus().setNodeSelection(pos).deleteSelection().run();
			};
			altInput.addEventListener('change', handleAlt);
			captionInput.addEventListener('change', handleCaption);
			alignment.addEventListener('change', handleAlignment);
			width.addEventListener('input', handleWidth);
			removeButton.addEventListener('click', handleRemove);
			refresh();

			return {
				dom: figure,
				update(updatedNode) {
					if (updatedNode.type.name !== 'image') return false;
					currentNode = updatedNode;
					refresh();
					return true;
				},
				stopEvent: (event) => controls.contains(event.target as globalThis.Node),
				ignoreMutation: () => true,
				destroy() {
					altInput.removeEventListener('change', handleAlt);
					captionInput.removeEventListener('change', handleCaption);
					alignment.removeEventListener('change', handleAlignment);
					width.removeEventListener('input', handleWidth);
					removeButton.removeEventListener('click', handleRemove);
				}
			};
		};
	}
});

export function createQuestionEditorExtensions(options: QuestionEditorExtensionOptions) {
	return [
		QuestionDocument,
		StarterKit.configure({
			document: false,
			blockquote: false,
			bulletList: false,
			code: false,
			codeBlock: false,
			heading: false,
			horizontalRule: false,
			link: false,
			listItem: false,
			listKeymap: false,
			orderedList: false,
			strike: false,
			trailingNode: false,
			underline: false
		}),
		InlineMath.configure({ onMathFocus: options.onMathFocus }),
		MathBlock.configure({ onMathFocus: options.onMathFocus }),
		QuestionImage,
		Placeholder.configure({ placeholder: options.placeholder }),
		FileHandler.configure({
			allowedMimeTypes: ['image/jpeg', 'image/png', 'image/gif', 'image/webp'],
			consumePasteEvent: true,
			onPaste(editor: Editor, files: File[]) {
				insertImageFiles(editor, files, editor.state.selection.from, options.onImageFile);
			},
			onDrop(editor: Editor, files: File[], position: number) {
				insertImageFiles(editor, files, position, options.onImageFile);
			}
		})
	];
}

function insertImageFiles(
	editor: Editor,
	files: File[],
	position: number,
	onImageFile: QuestionEditorExtensionOptions['onImageFile']
) {
	const nodes = files.flatMap((file) => {
		const image = onImageFile(file);
		return image ? [imageNode(image)] : [];
	});
	if (nodes.length) editor.chain().focus().insertContentAt(position, nodes).run();
}

export function imageNode(image: PendingImageReference) {
	return {
		type: 'image',
		attrs: {
			fileId: null,
			pendingId: image.pendingId,
			previewUrl: image.previewUrl,
			altText: null,
			caption: null,
			alignment: 'center',
			widthPercent: 60
		}
	};
}
