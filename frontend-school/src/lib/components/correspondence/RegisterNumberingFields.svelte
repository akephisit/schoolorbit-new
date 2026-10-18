<script lang="ts">
	import type { CorrespondenceNumberingSettings } from '#lib/api/correspondence.js';
	import type { CorrespondenceFieldErrors } from '#lib/correspondence/validation.js';
	import { numberingExample } from '#lib/correspondence/text-stamps.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import Choice from './Choice.svelte';
	import FieldError from './FieldError.svelte';
	let {
		value,
		onchange,
		errors,
		disabled
	}: {
		value: CorrespondenceNumberingSettings;
		onchange: (value: CorrespondenceNumberingSettings) => void;
		errors: CorrespondenceFieldErrors;
		disabled: boolean;
	} = $props();
	const presets = [
		{ value: '{number}/{year}', label: 'เลขลำดับ / พ.ศ.' },
		{ value: '{number}', label: 'เลขลำดับอย่างเดียว' },
		{ value: 'รับ {number}/{year}', label: 'รับ เลขลำดับ / พ.ศ.' }
	];
	let advanced = $state(false);
</script>

<section class="space-y-4" aria-label="รูปแบบเลขทะเบียน">
	<h3 class="font-medium">รูปแบบเลขทะเบียน</h3>
	<div class="grid gap-3 sm:grid-cols-2">
		<Choice
			id="numbering-preset"
			label="รูปแบบที่ใช้"
			value={presets.some((preset) => preset.value === value.format) ? value.format : '__custom__'}
			options={[...presets, { value: '__custom__', label: 'กำหนดข้อความเอง' }]}
			{disabled}
			onValueChange={(format) => {
				if (format === '__custom__') advanced = true;
				else onchange({ ...value, format });
			}}
		/>
		<Choice
			id="numbering-minimum-digits"
			label="จำนวนหลักเลขลำดับ"
			value={String(value.minimumDigits)}
			options={Array.from({ length: 8 }, (_, index) => ({
				value: String(index + 1),
				label: `${index + 1} หลัก`
			}))}
			{disabled}
			error={errors['numbering-minimum-digits']}
			onValueChange={(minimumDigits) =>
				onchange({ ...value, minimumDigits: Number(minimumDigits) })}
		/>
	</div>
	<p class="rounded-lg bg-muted/40 p-3 text-sm">
		ตัวอย่าง <output aria-label="ตัวอย่างเลขทะเบียน" class="font-medium"
			>{numberingExample(value)}</output
		>
	</p>
	<details bind:open={advanced}>
		<summary class="cursor-pointer text-sm font-medium">ปรับข้อความรูปแบบเอง</summary>
		<div class="mt-3 space-y-2">
			<Label for="numbering-format">ข้อความรูปแบบเลขทะเบียน</Label>
			<Input
				id="numbering-format"
				value={value.format}
				{disabled}
				aria-invalid={!!errors['numbering-format']}
				aria-describedby={errors['numbering-format'] ? 'numbering-format-error' : 'numbering-help'}
				oninput={(event) => onchange({ ...value, format: event.currentTarget.value })}
			/>
			<p id="numbering-help" class="text-xs text-muted-foreground">
				{'ใช้ {number} แทนเลขลำดับ และ {year} แทน พ.ศ. เพิ่มข้อความด้านหน้าหรือด้านหลังได้'}
			</p>
			<FieldError id="numbering-format-error" message={errors['numbering-format']} />
		</div>
	</details>
	<p class="text-xs text-muted-foreground">
		ใช้กับหนังสือที่ลงทะเบียนใหม่ เลขทะเบียนของหนังสือเดิมคงเดิม
	</p>
</section>
