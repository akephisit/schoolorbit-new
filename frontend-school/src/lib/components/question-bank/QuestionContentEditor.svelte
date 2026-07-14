<script lang="ts">
	import { untrack } from 'svelte';
	import { Editor, type JSONContent } from '@tiptap/core';
	import {
		Bold as BoldIcon,
		Image as ImageIcon,
		Italic as ItalicIcon,
		Keyboard,
		KeyboardOff,
		Redo2,
		Sigma,
		Type,
		Undo2
	} from 'lucide-svelte';
	import { buttonVariants, Button } from '$lib/components/ui/button';
	import {
		createQuestionEditorExtensions,
		imageNode,
		type MathFocusTarget
	} from '$lib/question-bank/rich-editor-extensions';
	import {
		emptyEditorRichContent,
		type EditorRichContent,
		type PendingImageReference
	} from '$lib/question-bank/rich-document';
	import { cn } from '$lib/utils';

	interface Props {
		label: string;
		content?: EditorRichContent;
		textPlaceholder?: string;
		required?: boolean;
		compact?: boolean;
		onImageSelected?: (file: File) => PendingImageReference | null;
	}

	type MathSymbol = { label: string; name: string; insert: string };

	let {
		label,
		content = $bindable(emptyEditorRichContent()),
		textPlaceholder = 'พิมพ์ข้อความที่นี่…',
		required = false,
		compact = false,
		onImageSelected
	}: Props = $props();

	const editorId = $props.id();
	let editor: Editor | null = $state(null);
	let editorRevision = $state(0);
	let activeMath: MathFocusTarget | null = $state(null);
	let showMoreSymbols = $state(false);
	let keyboardVisible = $state(false);

	const commonSymbols: MathSymbol[] = [
		{ label: '½', name: 'เศษส่วน', insert: String.raw`\frac{#0}{#?}` },
		{ label: '√', name: 'รากที่สอง', insert: String.raw`\sqrt{#0}` },
		{ label: 'x²', name: 'ยกกำลัง', insert: String.raw`#@^{#?}` },
		{ label: 'xₙ', name: 'ตัวห้อย', insert: String.raw`#@_{#?}` },
		{ label: '±', name: 'บวกลบ', insert: String.raw`\pm` },
		{ label: '×', name: 'คูณ', insert: String.raw`\times` },
		{ label: '÷', name: 'หาร', insert: String.raw`\div` },
		{ label: '≤', name: 'น้อยกว่าหรือเท่ากับ', insert: String.raw`\le` },
		{ label: '≥', name: 'มากกว่าหรือเท่ากับ', insert: String.raw`\ge` },
		{ label: '≠', name: 'ไม่เท่ากับ', insert: String.raw`\ne` }
	];
	const advancedSymbols: MathSymbol[] = [
		{ label: 'π', name: 'พาย', insert: String.raw`\pi` },
		{ label: 'θ', name: 'ทีตา', insert: String.raw`\theta` },
		{ label: '∞', name: 'อนันต์', insert: String.raw`\infty` },
		{ label: 'Σ', name: 'ผลรวม', insert: String.raw`\sum_{#0}^{#?}` },
		{ label: '∫', name: 'อินทิกรัล', insert: String.raw`\int_{#0}^{#?}` },
		{ label: '|x|', name: 'ค่าสัมบูรณ์', insert: String.raw`\left|#0\right|` },
		{ label: 'sin', name: 'ไซน์', insert: String.raw`\sin\left(#0\right)` },
		{ label: 'cos', name: 'โคไซน์', insert: String.raw`\cos\left(#0\right)` },
		{ label: 'tan', name: 'แทนเจนต์', insert: String.raw`\tan\left(#0\right)` },
		{ label: 'log', name: 'ลอการิทึม', insert: String.raw`\log_{#0}\left(#?\right)` },
		{ label: '→', name: 'ลูกศรขวา', insert: String.raw`\rightarrow` },
		{ label: '∈', name: 'เป็นสมาชิกของ', insert: String.raw`\in` }
	];

	function connectEditor(node: HTMLElement) {
		return untrack(() => {
			const instance = new Editor({
				element: node,
				extensions: createQuestionEditorExtensions({
					onImageFile: registerImage,
					placeholder: textPlaceholder,
					onMathFocus: (target) => {
						activeMath = target;
						editorRevision += 1;
					}
				}),
				content: content.document as JSONContent,
				editorProps: {
					attributes: {
						class: compact ? 'question-rich-editor compact' : 'question-rich-editor',
						role: 'textbox',
						'aria-multiline': 'true',
						'aria-label': label,
						'data-placeholder': textPlaceholder
					}
				},
				onUpdate: ({ editor: updatedEditor }) => {
					content = {
						schemaVersion: 1,
						document: updatedEditor.getJSON() as EditorRichContent['document']
					};
					editorRevision += 1;
				},
				onSelectionUpdate: () => {
					editorRevision += 1;
				}
			});
			editor = instance;
			const keyboard = window.mathVirtualKeyboard;
			const handleKeyboardToggle = () => {
				keyboardVisible = keyboard.visible;
			};
			keyboard.addEventListener('virtual-keyboard-toggle', handleKeyboardToggle);
			keyboardVisible = keyboard.visible;

			return () => {
				keyboard.removeEventListener('virtual-keyboard-toggle', handleKeyboardToggle);
				if (activeMath?.field.hasFocus() && keyboard.visible) keyboard.hide({ animate: false });
				instance.destroy();
				if (editor === instance) editor = null;
			};
		});
	}

	function registerImage(file: File): PendingImageReference | null {
		return onImageSelected?.(file) ?? null;
	}

	function insertInlineMath() {
		if (!editor) return;
		editor
			.chain()
			.focus()
			.insertContent({ type: 'inline_math', attrs: { latex: '' } })
			.run();
	}

	function continueAsText() {
		if (!editor) return;
		const position = activeMath?.getPosition();
		if (position !== undefined) {
			editor
				.chain()
				.focus()
				.setTextSelection(position + 1)
				.run();
		} else {
			editor.chain().focus().run();
		}
		activeMath = null;
	}

	function insertSymbol(symbol: MathSymbol) {
		const field = activeMath?.field;
		if (!field) return;
		field.insert(symbol.insert, {
			selectionMode: 'placeholder',
			focus: true,
			feedback: true,
			scrollIntoView: true
		});
		field.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText' }));
	}

	function toggleVirtualKeyboard() {
		const field = activeMath?.field;
		if (!field || !window.mathVirtualKeyboard) return;
		if (window.mathVirtualKeyboard.visible) {
			window.mathVirtualKeyboard.hide({ animate: true });
			return;
		}
		field.focus();
		window.mathVirtualKeyboard.layouts = ['numeric', 'symbols', 'greek'];
		window.mathVirtualKeyboard.show({ animate: true });
	}

	function handleImageSelection(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file || !editor) return;
		const reference = registerImage(file);
		if (reference) editor.chain().focus().insertContent(imageNode(reference)).run();
	}

	function isActive(name: string) {
		void editorRevision;
		return editor?.isActive(name) ?? false;
	}
</script>

<section
	class="overflow-hidden rounded-lg border bg-background"
	aria-labelledby={`${editorId}-label`}
>
	<div class="flex flex-wrap items-center gap-1 border-b bg-muted/30 px-2 py-1.5">
		<span id={`${editorId}-label`} class="mr-2 px-1 text-sm font-medium">
			{label}{#if required}<span class="text-destructive"> *</span>{/if}
		</span>
		<Button type="button" variant="ghost" size="sm" onclick={continueAsText}>
			<Type class="h-4 w-4" />
			ข้อความ
		</Button>
		<Button
			type="button"
			variant={activeMath ? 'secondary' : 'ghost'}
			size="sm"
			onclick={insertInlineMath}
		>
			<Sigma class="h-4 w-4" />
			สมการ
		</Button>
		<Button
			type="button"
			variant={isActive('bold') ? 'secondary' : 'ghost'}
			size="icon-sm"
			aria-label="ตัวหนา"
			onclick={() => editor?.chain().focus().toggleBold().run()}
		>
			<BoldIcon class="h-4 w-4" />
		</Button>
		<Button
			type="button"
			variant={isActive('italic') ? 'secondary' : 'ghost'}
			size="icon-sm"
			aria-label="ตัวเอียง"
			onclick={() => editor?.chain().focus().toggleItalic().run()}
		>
			<ItalicIcon class="h-4 w-4" />
		</Button>
		{#if onImageSelected}
			<label
				for={`${editorId}-image`}
				class={cn(buttonVariants({ variant: 'ghost', size: 'sm' }), 'cursor-pointer')}
			>
				<ImageIcon class="h-4 w-4" />
				รูปภาพ
			</label>
			<input
				id={`${editorId}-image`}
				class="sr-only"
				type="file"
				accept="image/jpeg,image/png,image/gif,image/webp"
				onchange={handleImageSelection}
			/>
		{/if}
		<div class="ml-auto flex items-center gap-1">
			<Button
				type="button"
				variant="ghost"
				size="icon-sm"
				aria-label="ย้อนกลับ"
				disabled={!editor?.can().undo()}
				onclick={() => editor?.chain().focus().undo().run()}
			>
				<Undo2 class="h-4 w-4" />
			</Button>
			<Button
				type="button"
				variant="ghost"
				size="icon-sm"
				aria-label="ทำซ้ำ"
				disabled={!editor?.can().redo()}
				onclick={() => editor?.chain().focus().redo().run()}
			>
				<Redo2 class="h-4 w-4" />
			</Button>
		</div>
	</div>

	{#if activeMath}
		<div class="space-y-2 border-b bg-muted/20 p-2">
			<div class="flex flex-wrap items-center gap-1.5" aria-label="สัญลักษณ์คณิตศาสตร์">
				{#each commonSymbols as symbol (symbol.name)}
					<Button
						type="button"
						variant="outline"
						size="sm"
						class="min-w-10 font-serif text-base"
						aria-label={`ใส่${symbol.name}`}
						onpointerdown={(event) => event.preventDefault()}
						onclick={() => insertSymbol(symbol)}
					>
						{symbol.label}
					</Button>
				{/each}
				<Button
					type="button"
					variant="ghost"
					size="sm"
					aria-expanded={showMoreSymbols}
					onpointerdown={(event) => event.preventDefault()}
					onclick={() => (showMoreSymbols = !showMoreSymbols)}
				>
					{showMoreSymbols ? 'ซ่อน' : 'เพิ่มเติม'}
				</Button>
				<Button
					type="button"
					variant={keyboardVisible ? 'secondary' : 'outline'}
					size="sm"
					aria-pressed={keyboardVisible}
					onpointerdown={(event) => event.preventDefault()}
					onclick={toggleVirtualKeyboard}
				>
					{#if keyboardVisible}<KeyboardOff class="h-4 w-4" /> ปิดแป้น{:else}<Keyboard
							class="h-4 w-4"
						/> เปิดแป้น{/if}
				</Button>
			</div>
			{#if showMoreSymbols}
				<div class="flex flex-wrap items-center gap-1.5">
					{#each advancedSymbols as symbol (symbol.name)}
						<Button
							type="button"
							variant="outline"
							size="sm"
							class="min-w-10 font-serif"
							aria-label={`ใส่${symbol.name}`}
							onpointerdown={(event) => event.preventDefault()}
							onclick={() => insertSymbol(symbol)}
						>
							{symbol.label}
						</Button>
					{/each}
				</div>
			{/if}
		</div>
	{/if}

	<div {@attach connectEditor}></div>
	<div class="border-t bg-muted/20 px-3 py-1.5 text-xs text-muted-foreground">
		พิมพ์ข้อความและสมการต่อกันได้ · ลากรูปขึ้นหรือลงเพื่อเปลี่ยนตำแหน่ง ·
		รูปจะอัปโหลดเมื่อกดบันทึกเท่านั้น
	</div>
</section>

<style>
	:global(.question-rich-editor) {
		min-height: 9rem;
		padding: 1rem;
		font-size: 1rem;
		line-height: 1.75;
		outline: none;
	}

	:global(.question-rich-editor.compact) {
		min-height: 6rem;
		padding: 0.75rem;
		font-size: 0.925rem;
	}

	:global(.question-rich-editor p) {
		min-height: 1.75rem;
		white-space: pre-wrap;
	}

	:global(.question-rich-editor p.is-editor-empty:first-child::before) {
		float: left;
		height: 0;
		color: var(--muted-foreground);
		content: attr(data-placeholder);
		pointer-events: none;
	}

	:global(.question-rich-editor.ProseMirror-focused) {
		box-shadow: inset 0 0 0 2px color-mix(in oklab, var(--ring) 45%, transparent);
	}

	:global(.question-inline-math) {
		display: inline-flex;
		max-width: 100%;
		margin-inline: 0.15rem;
		vertical-align: middle;
	}

	:global(.question-inline-math-field) {
		display: inline-block;
		min-width: 2.25rem;
		padding: 0.1rem 0.25rem;
		border: 1px solid var(--border);
		border-radius: 0.35rem;
		background: var(--background);
		font-size: 1.05em;
		outline: none;
	}

	:global(.question-inline-math-field:focus-within) {
		border-color: var(--ring);
		box-shadow: 0 0 0 2px color-mix(in oklab, var(--ring) 40%, transparent);
	}

	:global(.question-inline-math-field::part(menu-toggle)) {
		display: none;
	}

	:global(.question-editor-image) {
		width: var(--question-image-width, 60%);
		max-width: 100%;
		margin-block: 1rem;
		padding: 0.5rem;
		border: 1px solid var(--border);
		border-radius: 0.5rem;
		background: color-mix(in oklab, var(--muted) 25%, transparent);
	}

	:global(.question-editor-image[data-alignment='left']) {
		margin-right: auto;
		margin-left: 0;
	}

	:global(.question-editor-image[data-alignment='center']) {
		margin-inline: auto;
	}

	:global(.question-editor-image[data-alignment='right']) {
		margin-right: 0;
		margin-left: auto;
	}

	:global(.question-editor-image.ProseMirror-selectednode) {
		border-color: var(--ring);
		box-shadow: 0 0 0 2px color-mix(in oklab, var(--ring) 35%, transparent);
	}

	:global(.question-editor-image-handle) {
		margin-bottom: 0.35rem;
		cursor: grab;
		font-size: 0.75rem;
		color: var(--muted-foreground);
		user-select: none;
	}

	:global(.question-editor-image img) {
		display: block;
		width: 100%;
		max-height: 24rem;
		cursor: grab;
		object-fit: contain;
	}

	:global(.question-editor-image-controls) {
		display: grid;
		grid-template-columns: minmax(8rem, 1fr) minmax(8rem, 1fr) auto minmax(5rem, 0.5fr) auto auto;
		gap: 0.4rem;
		align-items: center;
		margin-top: 0.5rem;
	}

	:global(.question-editor-image-controls input[type='text']),
	:global(.question-editor-image-controls select),
	:global(.question-editor-image-controls button) {
		min-height: 2rem;
		padding: 0.25rem 0.5rem;
		border: 1px solid var(--border);
		border-radius: 0.35rem;
		background: var(--background);
		font-size: 0.75rem;
	}

	:global(.question-editor-image-controls button) {
		color: var(--destructive);
	}

	@media (max-width: 48rem) {
		:global(.question-editor-image) {
			width: 100%;
		}

		:global(.question-editor-image-controls) {
			grid-template-columns: 1fr 1fr;
		}
	}
</style>
