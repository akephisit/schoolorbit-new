<script lang="ts">
	import { Input } from '$lib/components/ui/input';
	import type { StaffJobPositionSummary } from '$lib/api/personnel';
	import * as Select from '$lib/components/ui/select';
	import StaffJobPositionPicker from './StaffJobPositionPicker.svelte';
	import {
		ACADEMIC_RANK_LABELS,
		EDUCATION_LEVEL_LABELS,
		type StaffPersonnelDraft
	} from '$lib/forms/staff-personnel';
	let {
		value = $bindable(),
		selectedPosition = $bindable(null),
		errors = {},
		disabled = false
	}: {
		value: StaffPersonnelDraft;
		selectedPosition?: StaffJobPositionSummary | null;
		errors?: Record<string, string>;
		disabled?: boolean;
	} = $props();
</script>

<div class="grid min-w-0 gap-5 sm:grid-cols-2" data-testid="staff-personnel-fields">
	<StaffJobPositionPicker
		label="ตำแหน่งงาน"
		bind:value={value.job_position_id}
		bind:selected={selectedPosition}
		{disabled}
	/>
	<div class="space-y-2">
		<p class="text-sm font-medium">วิทยฐานะ</p>
		<Select.Root
			type="single"
			value={value.academic_rank ?? 'unspecified'}
			onValueChange={(v) => {
				value.academic_rank = v === 'unspecified' ? null : (v as keyof typeof ACADEMIC_RANK_LABELS);
			}}
			{disabled}
		>
			<Select.Trigger class="w-full" aria-label="วิทยฐานะ"
				>{value.academic_rank
					? ACADEMIC_RANK_LABELS[value.academic_rank]
					: 'ยังไม่ระบุ'}</Select.Trigger
			>
			<Select.Content
				><Select.Item value="unspecified">ยังไม่ระบุ</Select.Item
				>{#each Object.entries(ACADEMIC_RANK_LABELS) as [code, label] (code)}<Select.Item
						value={code}>{label}</Select.Item
					>{/each}</Select.Content
			>
		</Select.Root>
	</div>
	<div class="space-y-2">
		<p class="text-sm font-medium">วุฒิการศึกษาสูงสุด</p>
		<Select.Root
			type="single"
			value={value.education_level ?? 'unspecified'}
			onValueChange={(v) => {
				value.education_level =
					v === 'unspecified' ? null : (v as keyof typeof EDUCATION_LEVEL_LABELS);
			}}
			{disabled}
		>
			<Select.Trigger class="w-full" aria-label="วุฒิการศึกษาสูงสุด"
				>{value.education_level
					? EDUCATION_LEVEL_LABELS[value.education_level]
					: 'ยังไม่ระบุ'}</Select.Trigger
			>
			<Select.Content
				><Select.Item value="unspecified">ยังไม่ระบุ</Select.Item
				>{#each Object.entries(EDUCATION_LEVEL_LABELS) as [code, label] (code)}<Select.Item
						value={code}>{label}</Select.Item
					>{/each}</Select.Content
			>
		</Select.Root>
	</div>
	{#each [{ key: 'major', label: 'สาขาวิชา' }, { key: 'university', label: 'สถาบันการศึกษา' }] as field (field.key)}
		<div class="space-y-2">
			<label for={`staff-${field.key}`} class="text-sm font-medium">{field.label}</label>
			<Input
				id={`staff-${field.key}`}
				value={value[field.key as 'major' | 'university'] ?? ''}
				oninput={(event) => {
					value[field.key as 'major' | 'university'] = event.currentTarget.value;
				}}
				{disabled}
				aria-invalid={Boolean(errors[field.key])}
				aria-describedby={errors[field.key] ? `staff-${field.key}-error` : undefined}
				placeholder={`พิมพ์${field.label}`}
			/>
			{#if errors[field.key]}<p
					id={`staff-${field.key}-error`}
					class="text-sm text-destructive"
					role="alert"
				>
					{errors[field.key]}
				</p>{/if}
		</div>
	{/each}
	<p class="text-sm text-muted-foreground sm:col-span-2">
		กลุ่มสาระใช้สังกัดปัจจุบันในระบบโดยอัตโนมัติ จัดการได้ในส่วนสังกัดหน่วยงาน
	</p>
</div>
