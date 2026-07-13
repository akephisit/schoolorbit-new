<script lang="ts">
	import 'mathlive';
	import 'mathlive/fonts.css';
	import type { MathfieldElement } from 'mathlive';
	import { on } from 'svelte/events';
	import { Button } from '$lib/components/ui/button';
	import { Keyboard, KeyboardOff, Sigma } from 'lucide-svelte';

	interface Props {
		value?: string;
		label?: string;
		compact?: boolean;
	}

	type MathSymbol = {
		label: string;
		name: string;
		insert: string;
	};

	let { value = $bindable(''), label = 'สมการคณิตศาสตร์', compact = false }: Props = $props();
	let showMoreSymbols = $state(false);
	let keyboardVisible = $state(false);
	let mathfield: MathfieldElement | null = null;

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

	function connectMathfield(node: HTMLElement) {
		const field = node as MathfieldElement;
		mathfield = field;
		field.value = value;
		field.smartFence = true;
		field.mathVirtualKeyboardPolicy = 'manual';

		const stopInput = on(field, 'input', () => {
			value = field.value;
		});
		const stopFocus = on(field, 'focusin', () => {
			if (window.mathVirtualKeyboard) {
				window.mathVirtualKeyboard.layouts = ['numeric', 'symbols', 'greek'];
			}
		});
		const stopKeyboardToggle = on(window.mathVirtualKeyboard, 'virtual-keyboard-toggle', () => {
			keyboardVisible = window.mathVirtualKeyboard.visible;
		});
		keyboardVisible = window.mathVirtualKeyboard.visible;

		$effect(() => {
			if (field.value !== value) field.value = value;
		});

		return () => {
			if (field.hasFocus() && window.mathVirtualKeyboard.visible) {
				window.mathVirtualKeyboard.hide({ animate: false });
			}
			stopInput();
			stopFocus();
			stopKeyboardToggle();
			if (mathfield === field) mathfield = null;
		};
	}

	function insertSymbol(symbol: MathSymbol) {
		if (!mathfield) return;
		mathfield.insert(symbol.insert, {
			selectionMode: 'placeholder',
			focus: true,
			feedback: true,
			scrollIntoView: true
		});
		value = mathfield.value;
	}

	function toggleVirtualKeyboard() {
		if (!mathfield || !window.mathVirtualKeyboard) return;
		if (window.mathVirtualKeyboard.visible) {
			window.mathVirtualKeyboard.hide({ animate: true });
			return;
		}
		mathfield.focus();
		window.mathVirtualKeyboard.layouts = ['numeric', 'symbols', 'greek'];
		window.mathVirtualKeyboard.show({ animate: true });
	}
</script>

<div class="space-y-3">
	<div class="flex flex-wrap items-center gap-1.5" aria-label="สัญลักษณ์คณิตศาสตร์ที่ใช้บ่อย">
		{#each commonSymbols as symbol (symbol.name)}
			<Button
				type="button"
				variant="outline"
				size="sm"
				class="min-w-10 font-serif text-base"
				title={symbol.name}
				aria-label={`ใส่${symbol.name}`}
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
			onclick={() => (showMoreSymbols = !showMoreSymbols)}
		>
			<Sigma class="h-4 w-4" />
			{showMoreSymbols ? 'ซ่อนสัญลักษณ์' : 'สัญลักษณ์เพิ่มเติม'}
		</Button>
	</div>

	{#if showMoreSymbols}
		<div
			class="flex flex-wrap items-center gap-1.5 rounded-md bg-muted/50 p-2"
			aria-label="สัญลักษณ์คณิตศาสตร์เพิ่มเติม"
		>
			{#each advancedSymbols as symbol (symbol.name)}
				<Button
					type="button"
					variant="outline"
					size="sm"
					class="min-w-10 font-serif"
					title={symbol.name}
					aria-label={`ใส่${symbol.name}`}
					onclick={() => insertSymbol(symbol)}
				>
					{symbol.label}
				</Button>
			{/each}
		</div>
	{/if}

	<math-field
		{@attach connectMathfield}
		aria-label={label}
		placeholder="แตะที่นี่แล้วพิมพ์ตัวเลข หรือตัวแปร เช่น x + 2"
		class={compact ? 'visual-math-field compact' : 'visual-math-field'}
	></math-field>

	<div class="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
		<span>พิมพ์ตัวเลขและตัวแปรได้ตามปกติ หรือกดสัญลักษณ์ด้านบน</span>
		<Button
			type="button"
			variant={keyboardVisible ? 'secondary' : 'outline'}
			size="sm"
			aria-pressed={keyboardVisible}
			onclick={toggleVirtualKeyboard}
		>
			{#if keyboardVisible}
				<KeyboardOff class="h-4 w-4" />
				ปิดแป้นสัญลักษณ์
			{:else}
				<Keyboard class="h-4 w-4" />
				เปิดแป้นสัญลักษณ์ทั้งหมด
			{/if}
		</Button>
	</div>
</div>

<style>
	:global(.visual-math-field) {
		display: block;
		width: 100%;
		min-height: 4.5rem;
		padding: 0.75rem;
		border: 1px solid var(--border);
		border-radius: 0.5rem;
		background: var(--background);
		font-size: 1.25rem;
		outline: none;
	}

	:global(.visual-math-field:focus-within) {
		border-color: var(--ring);
		box-shadow: 0 0 0 3px color-mix(in oklab, var(--ring) 50%, transparent);
	}

	:global(.visual-math-field.compact) {
		min-height: 3.75rem;
		font-size: 1.1rem;
	}

	:global(.visual-math-field::part(menu-toggle)) {
		display: none;
	}
</style>
