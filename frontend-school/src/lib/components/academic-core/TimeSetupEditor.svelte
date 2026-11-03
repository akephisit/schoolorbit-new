<script lang="ts">
	import { untrack } from 'svelte';
	import type {
		AcademicYear,
		BellSchedule,
		BellSchedulePeriod,
		CreateBellScheduleRequest,
		UpdateBellScheduleRequest,
		ReplaceBellSchedulePeriodsRequest
	} from '#lib/api/academic-core.js';
	import BellScheduleSetupStep from './setup/BellScheduleSetupStep.svelte';
	import BellSchedulePeriodsStep from './setup/BellSchedulePeriodsStep.svelte';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	let {
		years,
		bellSchedules,
		canManage,
		busy,
		onCreateBellSchedule,
		onUpdateBellSchedule,
		onLoadBellSchedulePeriods,
		onReplaceBellSchedulePeriods
	}: {
		years: AcademicYear[];
		bellSchedules: BellSchedule[];
		canManage: boolean;
		busy: { schedule: boolean; periods: boolean };
		onCreateBellSchedule: (input: CreateBellScheduleRequest) => Promise<BellSchedule>;
		onUpdateBellSchedule: (id: string, input: UpdateBellScheduleRequest) => Promise<BellSchedule>;
		onLoadBellSchedulePeriods: (id: string) => Promise<BellSchedulePeriod[]>;
		onReplaceBellSchedulePeriods: (
			id: string,
			input: ReplaceBellSchedulePeriodsRequest
		) => Promise<BellSchedulePeriod[]>;
	} = $props();
	let selectedId = $state(untrack(() => years[0]?.id ?? ''));
	const year = $derived(years.find((item) => item.id === selectedId) ?? years[0]);
	const schedules = $derived(
		bellSchedules.filter((schedule) => schedule.academicYearId === year?.id)
	);
</script>

<div class="space-y-5" id="bell-schedules">
	<div class="max-w-sm space-y-2">
		<Label for="time-year">ปีการศึกษา</Label><Select.Root
			type="single"
			value={year?.id ?? ''}
			onValueChange={(value) => {
				selectedId = value;
			}}
			><Select.Trigger id="time-year" class="w-full"
				><span>{year?.name ?? 'ยังไม่มีปีการศึกษา'}</span></Select.Trigger
			><Select.Content
				>{#each years as item (item.id)}<Select.Item value={item.id}>{item.name}</Select.Item
					>{/each}</Select.Content
			></Select.Root
		>
	</div>
	{#if year && canManage}{#key year.id}<section class="rounded-xl border bg-card p-4">
				<h2 class="mb-4 font-semibold">ตารางเวลา</h2>
				<BellScheduleSetupStep
					{year}
					{schedules}
					busy={busy.schedule}
					onCreate={onCreateBellSchedule}
					onUpdate={onUpdateBellSchedule}
					onSaved={() => {}}
				/>
			</section>
			<section class="rounded-xl border bg-card p-4">
				<h2 class="mb-4 font-semibold">คาบเรียน</h2>
				<BellSchedulePeriodsStep
					{year}
					{schedules}
					busy={busy.periods}
					onLoad={onLoadBellSchedulePeriods}
					onReplace={onReplaceBellSchedulePeriods}
					onSaved={() => {}}
				/>
			</section>{/key}{:else if year}{#each schedules as schedule (schedule.id)}<p>
				{schedule.name}
			</p>{/each}{:else}<p class="text-sm text-muted-foreground">
			เพิ่มปีการศึกษาที่หน้าจัดการปีการศึกษาก่อน
		</p>{/if}
</div>
