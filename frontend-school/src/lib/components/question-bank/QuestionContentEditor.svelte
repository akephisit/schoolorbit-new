<script lang="ts">
	import { buttonVariants } from '$lib/components/ui/button';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Textarea } from '$lib/components/ui/textarea';
	import { cn } from '$lib/utils';
	import { Image as ImageIcon, Sigma, Type, X } from 'lucide-svelte';
	import VisualMathEditor from './VisualMathEditor.svelte';

	interface Props {
		label: string;
		text?: string;
		math?: string;
		imagePreviewUrl?: string;
		imageAltText?: string;
		textPlaceholder?: string;
		required?: boolean;
		compact?: boolean;
		onImageSelected?: (file: File) => void;
		onImageRemoved?: () => void;
	}

	let {
		label,
		text = $bindable(''),
		math = $bindable(''),
		imagePreviewUrl = '',
		imageAltText = $bindable(''),
		textPlaceholder = 'พิมพ์ข้อความที่นี่…',
		required = false,
		compact = false,
		onImageSelected,
		onImageRemoved
	}: Props = $props();

	const editorId = $props.id();
	let showEmptyMathEditor = $state(false);
	let textareaRef: HTMLTextAreaElement | null = $state(null);
	let mathEditorVisible = $derived(Boolean(math) || showEmptyMathEditor);
	let imagePendingUpload = $derived(imagePreviewUrl.startsWith('blob:'));

	function focusText() {
		textareaRef?.focus();
	}

	function showMathEditor() {
		showEmptyMathEditor = true;
	}

	function removeMath() {
		math = '';
		showEmptyMathEditor = false;
	}

	function handleImageSelection(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (file) onImageSelected?.(file);
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
		<Button type="button" variant="ghost" size="sm" onclick={focusText}>
			<Type class="h-4 w-4" />
			ข้อความ
		</Button>
		<Button
			type="button"
			variant={mathEditorVisible ? 'secondary' : 'ghost'}
			size="sm"
			onclick={showMathEditor}
		>
			<Sigma class="h-4 w-4" />
			สมการ
		</Button>
		{#if onImageSelected}
			<label
				for={`${editorId}-image`}
				class={cn(
					buttonVariants({ variant: imagePreviewUrl ? 'secondary' : 'ghost', size: 'sm' }),
					'cursor-pointer'
				)}
			>
				<ImageIcon class="h-4 w-4" />
				รูปภาพ
			</label>
			<input
				id={`${editorId}-image`}
				class="sr-only"
				type="file"
				accept="image/*"
				onchange={handleImageSelection}
			/>
			{#if imagePendingUpload}
				<span class="px-1 text-xs text-muted-foreground">รูปจะอัปโหลดเมื่อกดบันทึกเท่านั้น</span>
			{/if}
		{/if}
	</div>

	<div class={compact ? 'space-y-3 p-3' : 'space-y-4 p-4'}>
		<Textarea
			bind:ref={textareaRef}
			bind:value={text}
			aria-label={`${label} ส่วนข้อความ`}
			placeholder={textPlaceholder}
			class={compact
				? 'min-h-20 resize-y border-0 px-0 py-0 shadow-none focus-visible:ring-0'
				: 'min-h-28 resize-y border-0 px-0 py-0 text-base shadow-none focus-visible:ring-0'}
		/>

		{#if mathEditorVisible}
			<div class="space-y-2 rounded-lg border bg-muted/20 p-3">
				<div class="flex items-center justify-between gap-2">
					<p class="text-sm font-medium">สมการคณิตศาสตร์</p>
					<Button
						type="button"
						variant="ghost"
						size="icon-sm"
						aria-label="นำสมการออก"
						onclick={removeMath}
					>
						<X class="h-4 w-4" />
					</Button>
				</div>
				<VisualMathEditor bind:value={math} label={`${label} ส่วนสมการ`} {compact} />
			</div>
		{/if}

		{#if imagePreviewUrl}
			<div class="space-y-2 rounded-lg border bg-muted/20 p-3">
				<div class="flex items-center justify-between gap-2">
					<p class="text-sm font-medium">รูปประกอบ</p>
					{#if onImageRemoved}
						<Button
							type="button"
							variant="ghost"
							size="icon-sm"
							aria-label={`นำรูปจาก${label}ออก`}
							onclick={onImageRemoved}
						>
							<X class="h-4 w-4" />
						</Button>
					{/if}
				</div>
				<img
					src={imagePreviewUrl}
					alt={imageAltText}
					class={compact
						? 'max-h-40 rounded-md border object-contain'
						: 'max-h-64 rounded-md border object-contain'}
				/>
				<Input
					bind:value={imageAltText}
					aria-label={`คำอธิบายรูปใน${label}`}
					placeholder="คำอธิบายรูปสำหรับผู้ใช้โปรแกรมอ่านจอ"
				/>
			</div>
		{/if}
	</div>
</section>
