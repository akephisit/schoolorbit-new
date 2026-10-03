<script lang="ts">
	import { Input } from '$lib/components/ui/input';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Label } from '$lib/components/ui/label';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import * as Collapsible from '$lib/components/ui/collapsible';
	import { CalendarDays, ChevronDown } from '@lucide/svelte';
	import type { StaffJobPositionSummary } from '$lib/api/personnel';
	import {
		ACADEMIC_RANK_LABELS,
		EDUCATION_LEVEL_LABELS,
		type StaffPersonnelDraft
	} from '$lib/forms/staff-personnel';
	import {
		PERSONNEL_TYPE_LABELS,
		CAREER_KIND_LABELS,
		careerFieldErrors,
		needsCareerCorrection,
		resetCareerDetailsForChangedValue,
		type StaffCareerDraft,
		type StaffCareerCorrectionReasons,
		type StaffPersonnelType
	} from '$lib/forms/staff-career';
	import StaffJobPositionPicker from './StaffJobPositionPicker.svelte';
	import StaffCareerDates from './StaffCareerDates.svelte';
	let {
		value = $bindable(),
		selectedPosition = $bindable(null),
		correctionReasons = $bindable({}),
		originalCareer = null,
		errors = {},
		disabled = false
	}: {
		value: StaffPersonnelDraft;
		selectedPosition?: StaffJobPositionSummary | null;
		correctionReasons?: StaffCareerCorrectionReasons;
		originalCareer?: StaffCareerDraft | null;
		errors?: Record<string, string>;
		disabled?: boolean;
	} = $props();
	const careerErrors = $derived({ ...careerFieldErrors(value.career), ...errors });
	const facts = [
		{ key: 'personnelType', kind: 'personnel_type' },
		{ key: 'jobPosition', kind: 'job_position' },
		{ key: 'academicRank', kind: 'academic_rank' }
	] as const;
	function change(key: keyof StaffCareerDraft, next: string | null) {
		const before = $state.snapshot(value.career),
			after = structuredClone(before);
		if (key === 'personnelType') after.personnelType.value = next as StaffPersonnelType | null;
		else if (key === 'academicRank')
			after.academicRank.value = next as keyof typeof ACADEMIC_RANK_LABELS | null;
		else after.jobPosition.value = next;
		value.career = resetCareerDetailsForChangedValue(before, after);
		const kind = facts.find((fact) => fact.key === key)?.kind;
		if (kind) correctionReasons[kind] = '';
	}
</script>

<div class="space-y-6" data-testid="staff-personnel-fields">
	<div class="grid min-w-0 gap-5 sm:grid-cols-2">
		<div class="space-y-2">
			<Label for="staff-personnel-type">ประเภทบุคลากร</Label>
			<Select.Root
				type="single"
				value={value.career.personnelType.value ?? 'unspecified'}
				onValueChange={(code) => change('personnelType', code === 'unspecified' ? null : code)}
				{disabled}
			>
				<Select.Trigger id="staff-personnel-type" class="w-full" aria-label="ประเภทบุคลากร"
					>{value.career.personnelType.value
						? PERSONNEL_TYPE_LABELS[value.career.personnelType.value]
						: 'ยังไม่ระบุ'}</Select.Trigger
				>
				<Select.Content
					><Select.Item value="unspecified">ยังไม่ระบุ</Select.Item
					>{#each Object.entries(PERSONNEL_TYPE_LABELS) as [code, label] (code)}<Select.Item
							value={code}>{label}</Select.Item
						>{/each}</Select.Content
				></Select.Root
			>
		</div>
		<StaffJobPositionPicker
			label="ตำแหน่งงาน"
			value={value.career.jobPosition.value}
			onValueChange={(code) => change('jobPosition', code)}
			bind:selected={selectedPosition}
			{disabled}
		/>
		<div class="space-y-2">
			<Label for="staff-academic-rank">วิทยฐานะ</Label>
			<Select.Root
				type="single"
				value={value.career.academicRank.value ?? 'unspecified'}
				onValueChange={(code) => change('academicRank', code === 'unspecified' ? null : code)}
				{disabled}
			>
				<Select.Trigger id="staff-academic-rank" class="w-full" aria-label="วิทยฐานะ"
					>{value.career.academicRank.value
						? ACADEMIC_RANK_LABELS[value.career.academicRank.value]
						: 'ยังไม่ระบุ'}</Select.Trigger
				>
				<Select.Content
					><Select.Item value="unspecified">ยังไม่ระบุ</Select.Item
					>{#each Object.entries(ACADEMIC_RANK_LABELS) as [code, label] (code)}<Select.Item
							value={code}>{label}</Select.Item
						>{/each}</Select.Content
				></Select.Root
			>
		</div>
	</div>
	<div class="space-y-3">
		{#each facts as fact (fact.kind)}
			<Collapsible.Root class="rounded-xl border bg-muted/20">
				<Collapsible.Trigger
					>{#snippet child({ props })}<Button
							{...props}
							type="button"
							variant="ghost"
							class="h-auto w-full justify-between whitespace-normal px-4 py-3 text-left"
							{disabled}
							><span class="flex items-center gap-2"
								><CalendarDays class="size-4 shrink-0 text-muted-foreground" />วันที่และคำสั่ง · {CAREER_KIND_LABELS[
									fact.kind
								]}</span
							><ChevronDown class="size-4 shrink-0" /></Button
						>{/snippet}</Collapsible.Trigger
				>
				<Collapsible.Content class="p-4">
					<StaffCareerDates
						bind:value={value.career[fact.key]}
						prefix={fact.kind}
						errors={careerErrors}
						{disabled}
					/>
					{#if needsCareerCorrection(originalCareer?.[fact.key], value.career[fact.key])}
						<div class="mt-4 space-y-2">
							<Label for={`${fact.kind}-reason`}
								>เหตุผลการแก้ไข <span class="text-destructive">*</span></Label
							><Textarea
								id={`${fact.kind}-reason`}
								bind:value={correctionReasons[fact.kind]}
								{disabled}
								rows={2}
								placeholder="เช่น แก้วันที่ตามคำสั่งต้นฉบับ"
							/>
							<p class="text-xs text-muted-foreground">
								ค่าเดิมยังเป็นข้อมูลปัจจุบัน
								การแก้รายละเอียดจะบันทึกในรายการเดิมพร้อมประวัติการแก้ไข
							</p>
						</div>
					{/if}
				</Collapsible.Content></Collapsible.Root
			>
		{/each}
	</div>
	{#if errors.career}<p class="text-sm text-destructive" role="alert">{errors.career}</p>{/if}
	<div class="grid min-w-0 gap-5 sm:grid-cols-2">
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
</div>
