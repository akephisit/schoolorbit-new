<script lang="ts">
	import type { StudentParentDraft } from '#lib/forms/student-register.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	let {
		value = $bindable(),
		idPrefix,
		errors = {},
		disabled = false,
		allowNationalId = false
	}: {
		value: StudentParentDraft;
		idPrefix: string;
		errors?: Record<string, string>;
		disabled?: boolean;
		allowNationalId?: boolean;
	} = $props();
	const fields = [
		{ key: 'title', label: 'คำนำหน้า', required: false, type: 'text' },
		{ key: 'first_name', label: 'ชื่อ', required: true, type: 'text' },
		{ key: 'last_name', label: 'นามสกุล', required: true, type: 'text' },
		{ key: 'relationship', label: 'ความสัมพันธ์', required: true, type: 'text' },
		{ key: 'phone', label: 'เบอร์โทรศัพท์', required: true, type: 'tel' },
		{ key: 'email', label: 'อีเมล', required: false, type: 'email' }
	] as const;
</script>

<div class="grid min-w-0 gap-4 sm:grid-cols-2">
	{#each fields as field (field.key)}
		<div class="min-w-0 space-y-2">
			<Label for={`${idPrefix}-${field.key}`}>
				{field.label}{#if field.required}<span class="text-destructive" aria-hidden="true">*</span
					>{/if}
			</Label>
			<Input
				id={`${idPrefix}-${field.key}`}
				type={field.type}
				bind:value={value[field.key]}
				{disabled}
				required={field.required}
				aria-invalid={Boolean(errors[field.key])}
				aria-describedby={errors[field.key] ? `${idPrefix}-${field.key}-error` : undefined}
			/>
			{#if errors[field.key]}
				<p id={`${idPrefix}-${field.key}-error`} class="text-sm text-destructive">
					{errors[field.key]}
				</p>
			{/if}
		</div>
	{/each}
	{#if allowNationalId}
		<div class="min-w-0 space-y-2 sm:col-span-2">
			<Label for={`${idPrefix}-national_id`}>เลขบัตรประชาชน</Label>
			<Input
				id={`${idPrefix}-national_id`}
				bind:value={value.national_id}
				inputmode="numeric"
				maxlength={13}
				{disabled}
				aria-invalid={Boolean(errors.national_id)}
				aria-describedby={errors.national_id ? `${idPrefix}-national_id-error` : undefined}
			/>
			{#if errors.national_id}
				<p id={`${idPrefix}-national_id-error`} class="text-sm text-destructive">
					{errors.national_id}
				</p>
			{/if}
		</div>
	{/if}
</div>
