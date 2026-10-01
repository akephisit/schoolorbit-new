<script lang="ts">
	import { resolve } from '$app/paths';
	import { can } from '$lib/stores/permissions';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import * as Select from '$lib/components/ui/select';
	import StaffReferencePicker from './StaffReferencePicker.svelte';
	import {
		ACADEMIC_RANK_LABELS,
		EDUCATION_LEVEL_LABELS,
		type StaffPersonnelDraft
	} from '$lib/forms/staff-personnel';
	import type { StaffInfoResponse } from '$lib/api/staff';
	let {
		value = $bindable(),
		references = $bindable({ job_position: null, major: null, university: null }),
		disabled = false
	}: {
		value: StaffPersonnelDraft;
		references?: Pick<StaffInfoResponse, 'job_position' | 'major' | 'university'>;
		disabled?: boolean;
	} = $props();
</script>

<div class="grid min-w-0 gap-5 sm:grid-cols-2" data-testid="staff-personnel-fields">
	<StaffReferencePicker
		kind="job_position"
		label="ตำแหน่งงาน"
		bind:value={value.job_position_id}
		bind:selected={references.job_position}
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
	<StaffReferencePicker
		kind="major"
		label="สาขาวิชา"
		bind:value={value.major_id}
		bind:selected={references.major}
		{disabled}
	/>
	<StaffReferencePicker
		kind="university"
		label="สถาบันการศึกษา"
		bind:value={value.university_id}
		bind:selected={references.university}
		{disabled}
	/>
	<div class="space-y-2 text-sm text-muted-foreground sm:col-span-2">
		<p>กลุ่มสาระใช้สังกัดปัจจุบันในระบบโดยอัตโนมัติ จัดการได้ในส่วนสังกัดหน่วยงาน</p>
		{#if $can.has(PERMISSIONS.STAFF_UPDATE_ALL)}<a
				href={resolve('/staff/manage/reference-data')}
				target="_blank"
				rel="noopener"
				class="text-primary underline">จัดการรายการตำแหน่ง สาขา และสถาบัน (เปิดหน้าใหม่)</a
			>
			<p class="text-xs">เมื่อเพิ่มรายการแล้ว เปิดตัวเลือกอีกครั้งเพื่อโหลดข้อมูลล่าสุด</p>{/if}
	</div>
</div>
