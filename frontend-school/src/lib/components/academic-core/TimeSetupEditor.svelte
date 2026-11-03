<script lang="ts">
	import { untrack } from 'svelte';
	import { page } from '$app/state';
	import type { AcademicYear, AcademicTerm, BellSchedule, BellSchedulePeriod, CreateBellScheduleRequest, CloneBellScheduleRequest, UpdateBellScheduleRequest, ReplaceBellSchedulePeriodsRequest, UpdateAcademicTermRequest } from '#lib/api/academic-core.js';
	import { customNameFromStored, standardTermName } from '#lib/academic-core/foundation-presentation.js';
	import { academicStatusLabel } from '#lib/academic/lifecycle/year-management.js';
	import BellScheduleSetupStep from './setup/BellScheduleSetupStep.svelte';
	import BellSchedulePeriodsStep from './setup/BellSchedulePeriodsStep.svelte';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	let { years, bellSchedules, termsByYear, setupYearIds, canManage, busy, onCreateBellSchedule, onCloneBellSchedule, onUpdateTerm, onUpdateBellSchedule, onLoadBellSchedulePeriods, onReplaceBellSchedulePeriods }: {
		years: AcademicYear[]; bellSchedules: BellSchedule[]; termsByYear: Map<string, AcademicTerm[]>; setupYearIds: string[]; canManage: boolean; busy: { schedule: boolean; periods: boolean; term: boolean };
		onCreateBellSchedule: (input: CreateBellScheduleRequest) => Promise<BellSchedule>;
		onCloneBellSchedule: (id: string, input: CloneBellScheduleRequest) => Promise<BellSchedule>;
		onUpdateTerm: (id: string, input: UpdateAcademicTermRequest) => Promise<AcademicTerm>;
		onUpdateBellSchedule: (id: string, input: UpdateBellScheduleRequest) => Promise<BellSchedule>;
		onLoadBellSchedulePeriods: (id: string, options?: { signal?: AbortSignal }) => Promise<BellSchedulePeriod[]>;
		onReplaceBellSchedulePeriods: (id: string, input: ReplaceBellSchedulePeriodsRequest) => Promise<BellSchedulePeriod[]>;
	} = $props();
	let selectedId = $state(untrack(() => page.url.searchParams.get('academicYearId') ?? years[0]?.id ?? ''));
	const year = $derived(years.find((item) => item.id === selectedId) ?? years[0]);
	const schedules = $derived(bellSchedules.filter((schedule) => schedule.academicYearId === year?.id));
	const terms = $derived(year ? termsByYear.get(year.id) ?? [] : []);
	let selectedTermId = $state(untrack(() => page.url.searchParams.get('academicTermId') ?? ''));
	const term = $derived(terms.find((item) => item.id === selectedTermId) ?? terms.find((item) => item.status === 'planning') ?? terms[0]);
	let termBase = $state<AcademicTerm | null>(null);
	let chosenScheduleId = $state('');
	let error = $state('');
	const canSetup = $derived(!!year && canManage && setupYearIds.includes(year.id));
	const termChanged = $derived(!!termBase && term?.id === termBase.id && term.rowVersion !== termBase.rowVersion);
	function chooseSchedule(id: string) { if (!term || term.status !== 'planning') return; termBase ??= { ...term }; chosenScheduleId = id; }
	async function assignSchedule() {
		if (!term || !termBase || term.status !== 'planning' || termChanged || !canSetup || busy.term) return;
		error = '';
		try {
			await onUpdateTerm(termBase.id, { termType: termBase.termType, customName: customNameFromStored(termBase.name, standardTermName(termBase.termType, termBase.sequence)) || null, includedInYearResult: termBase.includedInYearResult, blocksYearClosure: termBase.blocksYearClosure, bellScheduleId: chosenScheduleId, rowVersion: termBase.rowVersion });
			termBase = null; chosenScheduleId = '';
		} catch (cause) { error = cause instanceof Error ? cause.message : 'บันทึกตารางเวลาของภาคเรียนไม่สำเร็จ'; }
	}
</script>
<div class="space-y-5" id="bell-schedules">
	<div class="max-w-sm space-y-2"><Label for="time-year">ปีการศึกษา</Label><Select.Root type="single" value={year?.id ?? ''} onValueChange={(value) => { selectedId = value; selectedTermId = ''; termBase = null; chosenScheduleId = ''; }} disabled={!!termBase}><Select.Trigger id="time-year" class="w-full"><span>{year?.name ?? 'ยังไม่มีปีการศึกษา'}</span></Select.Trigger><Select.Content>{#each years as item (item.id)}<Select.Item value={item.id}>{item.name}</Select.Item>{/each}</Select.Content></Select.Root></div>
	{#if year}
		{#if terms.length}<section class="space-y-3 rounded-xl border bg-card p-4" aria-label="ตารางเวลาของภาคเรียน">
			<Label for="time-term">ภาคเรียนที่จะตั้งค่า</Label><Select.Root type="single" value={term?.id ?? ''} disabled={!!termBase} onValueChange={(value) => { selectedTermId = value; error = ''; }}><Select.Trigger id="time-term" class="w-full">{term?.name}</Select.Trigger><Select.Content>{#each terms as item (item.id)}<Select.Item value={item.id}>{item.name} · {academicStatusLabel(item.status)}</Select.Item>{/each}</Select.Content></Select.Root>
			{#if term?.status === 'planning' && canSetup}
				<Label for="time-term-schedule">ตารางเวลาที่ภาคเรียนนี้ใช้</Label><Select.Root type="single" value={chosenScheduleId || term.bellScheduleId} onValueChange={chooseSchedule} disabled={busy.term}><Select.Trigger id="time-term-schedule" class="w-full">{schedules.find((item) => item.id === (chosenScheduleId || term.bellScheduleId))?.name ?? 'เลือกตารางเวลา'}</Select.Trigger><Select.Content>{#each schedules as schedule (schedule.id)}<Select.Item value={schedule.id}>{schedule.name}</Select.Item>{/each}</Select.Content></Select.Root>
				<p class="text-sm text-muted-foreground">ใช้ตารางเดิมต่อได้ หากต้องเปลี่ยนคาบให้คัดลอกตารางด้านล่าง แล้วเลือกฉบับที่คัดลอกให้ภาคเรียนนี้</p>
				{#if termBase}<div class="flex flex-wrap gap-2"><Button disabled={busy.term || termChanged} onclick={() => void assignSchedule()}>บันทึกตารางเวลาของภาคเรียน</Button><Button variant="outline" disabled={busy.term} onclick={() => { termBase = null; chosenScheduleId = ''; }}>ยกเลิกตัวเลือกตาราง</Button></div>{/if}
				{#if termChanged}<p role="alert" class="text-sm text-destructive">ข้อมูลภาคเรียนเปลี่ยนแล้ว ตัวเลือกของคุณยังอยู่ ยกเลิกตัวเลือกแล้วตรวจข้อมูลล่าสุดก่อนบันทึก</p>{/if}
			{:else}<p class="text-sm text-muted-foreground">{schedules.find((item) => item.id === term?.bellScheduleId)?.name} · ภาคเรียนนี้ไม่เปิดให้เปลี่ยนตารางเวลา</p>{/if}
			{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
		</section>{/if}
		{#key year.id}<section class="rounded-xl border bg-card p-4"><h2 class="mb-4 font-semibold">ตารางเวลา</h2><BellScheduleSetupStep {year} {schedules} canManage={canSetup} busy={busy.schedule} onCreate={onCreateBellSchedule} onClone={onCloneBellSchedule} onUpdate={onUpdateBellSchedule} onSaved={(schedule) => { if (term?.status === 'planning') chooseSchedule(schedule.id); }} /></section>
		<section class="rounded-xl border bg-card p-4"><h2 class="mb-4 font-semibold">คาบเรียน</h2><BellSchedulePeriodsStep {year} {schedules} canManage={canSetup} busy={busy.periods} onLoad={onLoadBellSchedulePeriods} onReplace={onReplaceBellSchedulePeriods} onSaved={() => {}} /></section>{/key}
	{:else}<p class="text-sm text-muted-foreground">เพิ่มปีการศึกษาที่หน้าจัดการปีการศึกษาก่อน</p>{/if}
</div>
