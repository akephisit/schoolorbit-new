<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { ColorPicker } from '#lib/components/ui/color-picker/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import {
		Card,
		CardHeader,
		CardContent,
		CardTitle,
		CardDescription
	} from '#lib/components/ui/card/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as RadioGroup from '#lib/components/ui/radio-group/index.js';
	import {
		Check,
		Palette,
		Type,
		RotateCcw,
		Save,
		Sun,
		Moon,
		Monitor,
		Smartphone,
		ShieldCheck
	} from '@lucide/svelte';
	import {
		DEFAULT_APPEARANCE,
		PALETTES,
		FONTS,
		isPalette,
		isFont,
		isDensity,
		isHexColor,
		palettePrimary,
		appearanceTokens,
		type SchoolAppearance,
		type PreviewMode
	} from '#lib/appearance/model.js';
	import { prepareSchoolFont } from '#lib/appearance/runtime.js';
	import StaffDesignPreview from './StaffDesignPreview.svelte';

	let {
		draft = $bindable(),
		disabled = false,
		readonly = false,
		saving = false,
		saveLabel = 'บันทึกรูปแบบโรงเรียน',
		onsave
	}: {
		draft: SchoolAppearance;
		disabled?: boolean;
		readonly?: boolean;
		saving?: boolean;
		saveLabel?: string;
		onsave: () => void;
	} = $props();
	const uid = $props.id();
	let mode = $state<PreviewMode>('light');
	let device = $state<'desktop' | 'mobile'>('desktop');
	let colorText = $state(untrack(() => palettePrimary(draft)));
	let lastPrimary = '';
	let preview = $state<SchoolAppearance>(untrack(() => ({ ...draft })));
	let previewError = $state('');
	let preparing = $state(false);
	let hasPreview = $state(false);
	let generation = 0;
	let disposed = false;
	const tokens = $derived(appearanceTokens(draft, mode));
	const colorError = $derived(isHexColor(colorText) ? '' : 'ใช้รหัสสี 6 หลัก เช่น #167166');
	$effect(() => {
		const primary = palettePrimary(draft);
		untrack(() => {
			if (primary !== lastPrimary) {
				lastPrimary = primary;
				colorText = primary;
			}
		});
	});
	$effect(() => {
		const next = { ...draft };
		const ticket = ++generation;
		preparing = true;
		void prepareSchoolFont(next.font).then(
			() => {
				if (disposed || ticket !== generation) return;
				preview = next;
				hasPreview = true;
				preparing = false;
				previewError = '';
			},
			() => {
				if (disposed || ticket !== generation) return;
				preparing = false;
				previewError = 'โหลดฟอนต์ตัวอย่างไม่สำเร็จ กรุณาเลือกฟอนต์อีกครั้ง';
			}
		);
	});
	onDestroy(() => {
		disposed = true;
		generation++;
	});
	function changePalette(value: string) {
		if (disabled || readonly || saving) return;
		if (isPalette(value)) draft = { ...draft, palette: value, primary: null };
	}
	function changePrimary(value: string) {
		if (disabled || readonly || saving) return;
		colorText = value;
		if (isHexColor(value)) draft = { ...draft, primary: value.toLowerCase() };
	}
</script>

<div class="editor-grid">
	<form
		class="settings-column"
		onsubmit={(event) => {
			event.preventDefault();
			if (!disabled && !readonly && !saving && !colorError) onsave();
		}}
	>
		<fieldset disabled={disabled || readonly || saving} class="contents">
			<Card class="gap-0 py-0">
				<CardHeader class="section-header"
					><Palette class="size-5 text-primary" />
					<div class="section-copy">
						<CardTitle>สีประจำโรงเรียน</CardTitle><CardDescription
							>ใช้กับปุ่ม เมนู และจุดเน้นในระบบ</CardDescription
						>
					</div></CardHeader
				>
				<CardContent class="fields">
					<RadioGroup.Root
						value={draft.palette}
						onValueChange={changePalette}
						aria-label="ชุดสีโรงเรียน"
						class="palette-options"
					>
						{#each PALETTES as palette (palette.value)}
							<Label
								for={`${uid}-palette-${palette.value}`}
								class={`palette-option ${draft.palette === palette.value ? 'selected' : ''}`}
							>
								<span class="swatch" style:background={palette.primary}
									>{#if draft.palette === palette.value}<Check class="size-4" />{/if}</span
								>
								<span>{palette.label}</span><RadioGroup.Item
									id={`${uid}-palette-${palette.value}`}
									value={palette.value}
									class="sr-only"
								/>
							</Label>
						{/each}
					</RadioGroup.Root>
					<div class="field">
						<Label for={`${uid}-custom-primary`}>ปรับสีหลักเพิ่มเติม</Label>
						<ColorPicker
							id={`${uid}-custom-primary`}
							value={palettePrimary(draft)}
							text={colorText}
							disabled={disabled || readonly || saving}
							invalid={!!colorError}
							describedby={`${uid}-color-help`}
							onTextChange={changePrimary}
						/>
						<p
							id={`${uid}-color-help`}
							class="text-xs text-muted-foreground"
							class:error-text={!!colorError}
						>
							{colorError || 'ใช้รหัสสี HEX 6 หลัก · ปุ่มใช้สีที่เลือก'}
						</p>
					</div>
					<p class="contrast-note">
						<ShieldCheck class="size-4 shrink-0" /><span
							>ปุ่มและพื้นสีใช้ <code>{tokens.primaryFill.toUpperCase()}</code> · ตัวอักษรและไอคอนปรับให้อ่านง่าย</span
						>
					</p>
				</CardContent>
			</Card>
			<Card class="gap-0 py-0">
				<CardHeader class="section-header"
					><Type class="size-5 text-primary" />
					<div class="section-copy">
						<CardTitle>ตัวอักษรและระยะห่าง</CardTitle><CardDescription
							>รองรับภาษาไทยและภาษาอังกฤษ</CardDescription
						>
					</div></CardHeader
				>
				<CardContent class="fields">
					<div class="field">
						<Label for={`${uid}-school-font`}>ฟอนต์ของโรงเรียน</Label><Select.Root
							type="single"
							value={draft.font}
							disabled={disabled || readonly || saving}
							onValueChange={(value) => {
								if (isFont(value)) draft = { ...draft, font: value };
							}}
						>
							<Select.Trigger id={`${uid}-school-font`} aria-label="ฟอนต์ของโรงเรียน" class="w-full"
								>{FONTS.find((font) => font.value === draft.font)?.label}</Select.Trigger
							>
							<Select.Content
								>{#each FONTS as font (font.value)}<Select.Item value={font.value}
										>{font.label}</Select.Item
									>{/each}</Select.Content
							>
						</Select.Root>
					</div>
					<div class="field">
						<Label id={`${uid}-density-label`}>ระยะห่างของเนื้อหา</Label><RadioGroup.Root
							value={draft.density}
							onValueChange={(value) => {
								if (isDensity(value)) draft = { ...draft, density: value };
							}}
							aria-labelledby={`${uid}-density-label`}
							class="density-options"
						>
							<Label for={`${uid}-comfortable`} class="density-option"
								><RadioGroup.Item id={`${uid}-comfortable`} value="comfortable" />สบายตา</Label
							>
							<Label for={`${uid}-compact`} class="density-option"
								><RadioGroup.Item id={`${uid}-compact`} value="compact" />กระชับ</Label
							>
						</RadioGroup.Root>
					</div>
				</CardContent>
			</Card>
			{#if !readonly}<div class="flex flex-wrap gap-2">
					<Button
						type="button"
						variant="outline"
						onclick={() => {
							draft = { ...DEFAULT_APPEARANCE };
							colorText = palettePrimary(draft);
						}}><RotateCcw class="size-4" />คืนค่าเริ่มต้น</Button
					><Button type="submit" disabled={disabled || saving || !!colorError}
						><Save class="size-4" />{saving ? 'กำลังบันทึก…' : saveLabel}</Button
					>
				</div>{/if}
		</fieldset>
	</form>
	<section class="preview-column" aria-label="ตัวอย่างรูปแบบโรงเรียน">
		<div class="preview-toolbar">
			<div>
				<h2 class="font-medium">ตัวอย่างหน้าจอจริง</h2>
				<p class="text-xs text-muted-foreground">
					ข้อมูลสมมติ · โหมดและขนาดหน้าจอใช้ดูตัวอย่างเท่านั้น
				</p>
			</div>
			<div class="flex flex-wrap gap-1">
				<Button
					size="sm"
					variant={mode === 'light' ? 'secondary' : 'ghost'}
					aria-label="พรีวิวสว่าง"
					aria-pressed={mode === 'light'}
					onclick={() => (mode = 'light')}><Sun class="size-4" /></Button
				>
				<Button
					size="sm"
					variant={mode === 'dark' ? 'secondary' : 'ghost'}
					aria-label="พรีวิวมืด"
					aria-pressed={mode === 'dark'}
					onclick={() => (mode = 'dark')}><Moon class="size-4" /></Button
				>
				<Button
					size="sm"
					variant={device === 'desktop' ? 'secondary' : 'ghost'}
					aria-label="พรีวิวคอมพิวเตอร์"
					aria-pressed={device === 'desktop'}
					onclick={() => (device = 'desktop')}><Monitor class="size-4" /></Button
				>
				<Button
					size="sm"
					variant={device === 'mobile' ? 'secondary' : 'ghost'}
					aria-label="พรีวิวมือถือ"
					aria-pressed={device === 'mobile'}
					onclick={() => (device = 'mobile')}><Smartphone class="size-4" /></Button
				>
			</div>
		</div>
		<p class="text-xs text-muted-foreground" role="status">
			{preparing ? 'กำลังเตรียมตัวอย่าง…' : previewError || 'พรีวิวพร้อมแล้ว'}
		</p>
		<div class="preview-stage">
			<div class="preview-frame" class:mobile={device === 'mobile'}>
				{#if hasPreview}<StaffDesignPreview appearance={preview} {mode} />{:else}<p
						class="p-6 text-muted-foreground"
					>
						{previewError || 'กำลังเตรียมตัวอย่าง…'}
					</p>{/if}
			</div>
		</div>
	</section>
</div>

<style>
	.editor-grid {
		display: grid;
		grid-template-columns: minmax(280px, 340px) minmax(0, 1fr);
		gap: 1.5rem;
		align-items: start;
	}
	.settings-column {
		display: grid;
		gap: 1.25rem;
	}
	.editor-grid :global(.section-header) {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		border-bottom: 1px solid var(--border);
		padding: 1.25rem;
	}
	.editor-grid :global(.fields) {
		padding: 1.25rem;
		display: grid;
		gap: 1.25rem;
	}
	.section-copy {
		display: grid;
		gap: 0.375rem;
	}
	.field {
		display: grid;
		gap: 0.6rem;
	}
	.editor-grid :global(.palette-options) {
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}
	.editor-grid :global(.palette-option) {
		display: flex;
		flex-direction: column;
		padding: 0.8rem 0.25rem;
		border: 1px solid var(--border);
		border-radius: 0.6rem;
		cursor: pointer;
		gap: 0.6rem;
		font-size: 0.85rem;
	}
	.editor-grid :global(.palette-option.selected) {
		border-color: var(--primary);
		background: var(--accent);
	}
	.editor-grid :global(.palette-option:has(:focus-visible)) {
		outline: 2px solid var(--ring);
	}
	.swatch {
		display: grid;
		place-items: center;
		width: 2rem;
		height: 2rem;
		border-radius: 50%;
		color: white;
	}
	.editor-grid :global(.density-options) {
		grid-template-columns: 1fr 1fr;
	}
	.editor-grid :global(.density-option) {
		padding: 0.8rem;
		border: 1px solid var(--border);
		border-radius: 0.6rem;
		cursor: pointer;
	}
	.contrast-note {
		display: flex;
		gap: 0.5rem;
		background: var(--muted);
		padding: 0.75rem;
		border-radius: 0.5rem;
		font-size: 0.8rem;
	}
	.error-text {
		color: var(--destructive);
	}
	.preview-column {
		min-width: 0;
		display: grid;
		gap: 0.6rem;
	}
	.preview-toolbar {
		display: flex;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: 0.8rem;
	}
	.preview-stage {
		padding: 0.75rem;
		background: var(--muted);
		border: 1px solid var(--border);
		border-radius: 0.8rem;
		overflow: auto;
	}
	.preview-frame {
		width: 100%;
		min-width: 650px;
		container-type: inline-size;
	}
	.preview-frame.mobile {
		width: 390px;
		min-width: 390px;
		margin-inline: auto;
	}
	@media (max-width: 1050px) {
		.editor-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
