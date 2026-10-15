<script lang="ts">
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import { CalendarDays, ChevronDown } from '@lucide/svelte';
	import type { StaffJobPositionSummary } from '#lib/api/personnel.js';
	import { ACADEMIC_RANK_LABELS, type StaffPersonnelDraft } from '#lib/forms/staff-personnel.js';
	import {
		PERSONNEL_TYPE_LABELS,
		CAREER_KIND_LABELS,
		careerFieldErrors,
		needsCareerCorrection,
		resetCareerDetailsForChangedValue,
		type StaffCareerDraft,
		type StaffCareerCorrectionReasons,
		type StaffPersonnelType
	} from '#lib/forms/staff-career.js';
	import StaffJobPositionPicker from './StaffJobPositionPicker.svelte';
	import StaffLicenseAttachment from './StaffLicenseAttachment.svelte';
	import StaffEducationFields from './StaffEducationFields.svelte';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import StaffCareerDates from './StaffCareerDates.svelte';
	let {
		value = $bindable(),
		selectedPosition = $bindable(null),
		correctionReasons = $bindable({}),
		originalCareer = null,
		errors = {},
		disabled = false,
		staffId,
		canManageLicense = false,
		onUploadPendingChange
	}: {
		value: StaffPersonnelDraft;
		selectedPosition?: StaffJobPositionSummary | null;
		correctionReasons?: StaffCareerCorrectionReasons;
		originalCareer?: StaffCareerDraft | null;
		errors?: Record<string, string>;
		disabled?: boolean;
		staffId?: string;
		canManageLicense?: boolean;
		onUploadPendingChange?: (pending: boolean) => void;
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

<div class="space-y-4" data-testid="staff-personnel-fields">
	<div class="grid min-w-0 gap-3 sm:grid-cols-2">
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
	<StaffEducationFields bind:value={value.educations} {errors} {disabled} />
	<div class="grid gap-4 sm:grid-cols-2">
		<div class="space-y-2">
			<Label for="staff-license-number">เลขที่ใบอนุญาตประกอบวิชาชีพ</Label><Input
				id="staff-license-number"
				bind:value={value.teaching_license_number}
				aria-invalid={Boolean(errors.teaching_license_number)}
				aria-describedby={errors.teaching_license_number ? 'staff-license-error' : undefined}
				{disabled}
			/>
		</div>
		<div class="space-y-2">
			<Label for="staff-license-expiry">วันหมดอายุใบอนุญาต</Label><DatePicker
				id="staff-license-expiry"
				ariaLabel="วันหมดอายุใบอนุญาต"
				clearable
				bind:value={value.teaching_license_expiry}
				{disabled}
			/>
		</div>
	</div>
	{#if errors.teaching_license_number}<p id="staff-license-error" class="text-sm text-destructive">
			{errors.teaching_license_number}
		</p>{/if}
	{#if staffId && canManageLicense}
		{#key staffId}<StaffLicenseAttachment
				bind:value={value.teaching_license_file_id}
				{staffId}
				{disabled}
				onPendingChange={onUploadPendingChange}
			/>{/key}
	{:else if !staffId}<p class="text-xs text-muted-foreground">
			แนบใบประกอบวิชาชีพได้หลังสร้างบุคลากรแล้ว ในหน้างานและการศึกษา
		</p>{/if}
	<p class="text-xs text-muted-foreground">จัดการกลุ่มสาระและหน่วยงานได้ในส่วนสังกัดหน่วยงาน</p>
</div>
