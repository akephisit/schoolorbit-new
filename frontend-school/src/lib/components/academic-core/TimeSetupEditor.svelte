<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';
	import { registerAcademicContextDirtySource } from '#lib/academic-context/store.js';
	import { page } from '$app/state';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import type {
		AcademicYear,
		AcademicTerm,
		BellSchedule,
		BellSchedulePeriodsSnapshot,
		CreateBellScheduleRequest,
		CloneBellScheduleRequest,
		UpdateBellScheduleRequest,
		ReplaceBellSchedulePeriodsRequest,
		CopyAcademicTermSchoolScheduleRequest,
		AcademicTermSchoolScheduleCopyResponse,
		UpdateAcademicTermRequest
	} from '#lib/api/academic-core.js';
	import { standardTermName } from '#lib/academic-core/foundation-presentation.js';
	import { academicStatusLabel } from '#lib/academic/lifecycle/year-management.js';
	import BellScheduleSetupStep from './setup/BellScheduleSetupStep.svelte';
	import BellSchedulePeriodsStep from './setup/BellSchedulePeriodsStep.svelte';
	import TermSchoolScheduleEditor from './setup/TermSchoolScheduleEditor.svelte';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	let {
		years,
		bellSchedules,
		termsByYear,
		setupYearIds,
		setupTermIds,
		schoolDaysEditableTermIds,
		onCopySchoolSchedule,
		canManage,
		busy,
		onCreateBellSchedule,
		onCloneBellSchedule,
		onUpdateTerm,
		onUpdateBellSchedule,
		onLoadBellSchedulePeriods,
		onReplaceBellSchedulePeriods
	}: {
		years: AcademicYear[];
		bellSchedules: BellSchedule[];
		termsByYear: Map<string, AcademicTerm[]>;
		setupYearIds: string[];
		setupTermIds: string[];
		schoolDaysEditableTermIds: string[];
		onCopySchoolSchedule: (id: string, input: CopyAcademicTermSchoolScheduleRequest) => Promise<AcademicTermSchoolScheduleCopyResponse>;
		canManage: boolean;
		busy: { schedule: boolean; periods: boolean; term: boolean };
		onCreateBellSchedule: (input: CreateBellScheduleRequest) => Promise<BellSchedule>;
		onCloneBellSchedule: (id: string, input: CloneBellScheduleRequest) => Promise<BellSchedule>;
		onUpdateTerm: (id: string, input: UpdateAcademicTermRequest) => Promise<AcademicTerm>;
		onUpdateBellSchedule: (id: string, input: UpdateBellScheduleRequest) => Promise<BellSchedule>;
		onLoadBellSchedulePeriods: (
			id: string,
			options?: { signal?: AbortSignal }
		) => Promise<BellSchedulePeriodsSnapshot>;
		onReplaceBellSchedulePeriods: (
			id: string,
			input: ReplaceBellSchedulePeriodsRequest
		) => Promise<BellSchedulePeriodsSnapshot>;
	} = $props();
	let selectedId = $state(
		untrack(() => {
			const targetTermId = page.url.searchParams.get('academicTermId');
			return [...termsByYear.values()].flat().find((item) => item.id === targetTermId)?.academicYearId ?? page.url.searchParams.get('academicYearId') ?? years[0]?.id ?? '';
		})
	);
	const year = $derived(years.find((item) => item.id === selectedId) ?? years[0]);
	const schedules = $derived(
		bellSchedules.filter((schedule) => schedule.academicYearId === year?.id)
	);
	const terms = $derived(year ? (termsByYear.get(year.id) ?? []) : []);
	let selectedTermId = $state(untrack(() => page.url.searchParams.get('academicTermId') ?? ''));
	const term = $derived(
		terms.find((item) => item.id === selectedTermId) ??
			terms.find((item) => item.status === 'planning') ??
			terms[0]
	);
	let termBase = $state<AcademicTerm | null>(null);
	let chosenScheduleId = $state('');
	let error = $state('');
	let daysDirty = $state(false);
	let disposed = false;
	onDestroy(() => { disposed = true; });
	onMount(() => registerAcademicContextDirtySource('bell-term-assignment', () => !!termBase));
	const canSetup = $derived(!!year && canManage && setupYearIds.includes(year.id));
	const canAccessTimetable = $derived($can.hasModule(PERMISSION_MODULES.ACADEMIC_TIMETABLE));
	const canAssign = $derived(canSetup && !!term && setupTermIds.includes(term.id));
	const termChanged = $derived(
		!!termBase && term?.id === termBase.id && term.rowVersion !== termBase.rowVersion
	);
	function chooseSchedule(id: string) {
		if (!term || !canAssign || !['planning', 'ready'].includes(term.status)) return;
		termBase ??= { ...term };
		chosenScheduleId = id;
	}
	async function assignSchedule() {
		if (
			!term ||
			!termBase ||
			!['planning', 'ready'].includes(term.status) ||
			termChanged ||
			!canSetup ||
			!canAssign ||
			busy.term
		)
			return;
		error = '';
		const identity = appIdentityKey();
		try {
			await onUpdateTerm(termBase.id, {
				termType: termBase.termType,
				customName:
					termBase.name === standardTermName(termBase.termType, termBase.sequence)
						? null
						: termBase.name,
				includedInYearResult: termBase.includedInYearResult,
				blocksYearClosure: termBase.blocksYearClosure,
				bellScheduleId: chosenScheduleId,
				schoolDays: [...termBase.schoolDays],
				rowVersion: termBase.rowVersion
			});
			if (disposed || identity !== appIdentityKey()) return;
			termBase = null;
			chosenScheduleId = '';
		} catch (cause) {
			if (disposed || identity !== appIdentityKey()) return;
			error = cause instanceof Error ? cause.message : 'บันทึกตารางเวลาของภาคเรียนไม่สำเร็จ';
		}
	}
</script>

<div class="space-y-5" id="bell-schedules">
	<div class="max-w-sm space-y-2">
		<Label for="time-year">ปีการศึกษา</Label><Select.Root
			type="single"
			value={year?.id ?? ''}
			onValueChange={(value) => {
				selectedId = value;
				selectedTermId = '';
				termBase = null;
				chosenScheduleId = '';
			}}
			disabled={!!termBase || daysDirty}
			><Select.Trigger id="time-year" class="w-full"
				><span>{year?.name ?? 'ยังไม่มีปีการศึกษา'}</span></Select.Trigger
			><Select.Content
				>{#each years as item (item.id)}<Select.Item value={item.id}>{item.name}</Select.Item
					>{/each}</Select.Content
			></Select.Root
		>
	</div>
	{#if year}
		{#if terms.length}<section
				class="space-y-3 rounded-xl border bg-card p-4"
				aria-label="ตารางเวลาของภาคเรียน"
			>
				<Label for="time-term">ภาคเรียนที่จะตั้งค่า</Label><Select.Root
					type="single"
					value={term?.id ?? ''}
					disabled={!!termBase || daysDirty}
					onValueChange={(value) => {
						selectedTermId = value;
						error = '';
					}}
					><Select.Trigger id="time-term" class="w-full">{term?.name}</Select.Trigger
					><Select.Content
						>{#each terms as item (item.id)}<Select.Item value={item.id}
								>{item.name} · {academicStatusLabel(item.status)}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
				{#if term}{#key term.id}<TermSchoolScheduleEditor {year} {years} {term} terms={[...termsByYear.values()].flat()} schedules={bellSchedules} canEditDays={canManage && schoolDaysEditableTermIds.includes(term.id)} canCopy={canAssign} busy={busy.term} onUpdate={onUpdateTerm} onCopy={onCopySchoolSchedule} onLoad={onLoadBellSchedulePeriods} ondirtychange={(dirty) => daysDirty = dirty} />{/key}{/if}
				{#if term && ['planning', 'ready'].includes(term.status) && canAssign}
					<Label for="time-term-schedule">ตารางเวลาที่ภาคเรียนนี้ใช้</Label><Select.Root
						type="single"
						value={chosenScheduleId || term.bellScheduleId}
						onValueChange={chooseSchedule}
						disabled={busy.term || daysDirty}
						><Select.Trigger id="time-term-schedule" class="w-full"
							>{schedules.find((item) => item.id === (chosenScheduleId || term.bellScheduleId))
								?.name ?? 'เลือกตารางเวลา'}</Select.Trigger
						><Select.Content
							>{#each schedules as schedule (schedule.id)}<Select.Item value={schedule.id}
									>{schedule.name}</Select.Item
								>{/each}</Select.Content
						></Select.Root
					>
					<p class="text-sm text-muted-foreground">
						ใช้ตารางเดิมต่อได้ หากต้องเปลี่ยนคาบให้คัดลอกตารางด้านล่าง
						แล้วเลือกฉบับที่คัดลอกให้ภาคเรียนนี้
					</p>
					{#if termBase}<div class="flex flex-wrap gap-2">
							<Button disabled={busy.term || termChanged} onclick={() => void assignSchedule()}
								>บันทึกตารางเวลาของภาคเรียน</Button
							><Button
								variant="outline"
								disabled={busy.term}
								onclick={() => {
									termBase = null;
									chosenScheduleId = '';
								}}>ยกเลิกตัวเลือกตาราง</Button
							>
						</div>{/if}
					{#if termChanged}<p role="alert" class="text-sm text-destructive">
							ข้อมูลภาคเรียนเปลี่ยนแล้ว ตัวเลือกของคุณยังอยู่
							ยกเลิกตัวเลือกแล้วตรวจข้อมูลล่าสุดก่อนบันทึก
						</p>{/if}
				{:else}<p class="text-sm text-muted-foreground">
						{schedules.find((item) => item.id === term?.bellScheduleId)?.name} · ภาคเรียนนี้ไม่เปิดให้เปลี่ยนตารางเวลา
					</p>
					{#if term && ['planning', 'ready'].includes(term.status) && canSetup}<p
							class="text-sm text-muted-foreground"
						>
							ภาคเรียนนี้มีตารางสอนหรือประวัติอ้างอิงตารางเดิม จึงยังเปลี่ยนชุดคาบไม่ได้
							ให้ผู้ดูแลตรวจสอบรายการอ้างอิงของภาคนี้ การคัดลอกไม่เปลี่ยนตารางสอนเดิมอัตโนมัติ
						</p>
						{#if canAccessTimetable}<Button
								variant="outline"
								href={`/staff/academic/timetable?academicYearId=${year.id}&academicTermId=${term.id}`}
								>จัดการตารางสอนของ {term.name}</Button
							>{:else}<p class="text-sm text-muted-foreground">
								ติดต่อผู้ดูแลตารางสอนเพื่อตรวจสอบรายการอ้างอิงของภาคเรียนนี้
							</p>{/if}{/if}{/if}
				{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
			</section>{/if}
		{#if term}{#key `${term.id}:${term.bellScheduleId}`}<section class="rounded-xl border bg-card p-4" aria-label="คาบเรียนของภาคเรียน">
			<h2 class="mb-4 font-semibold">คาบเรียนของ {term.name}</h2>
			<BellSchedulePeriodsStep {year} {term} schedules={schedules.filter((schedule) => schedule.id === term.bellScheduleId)} canManage={canAssign && schoolDaysEditableTermIds.includes(term.id)} busy={busy.periods} onLoad={onLoadBellSchedulePeriods} onReplace={onReplaceBellSchedulePeriods} onSaved={() => {}} />
		</section>{/key}{/if}
		<Collapsible.Root class="rounded-xl border bg-card p-4">
			<Collapsible.Trigger class="text-sm font-medium">จัดการตารางเวลาต้นแบบเพิ่มเติม</Collapsible.Trigger>
			<Collapsible.Content class="mt-4 space-y-4">
			{#key year.id}<section aria-label="ตารางเวลาต้นแบบ">
				<h2 class="mb-4 font-semibold">ตารางเวลาต้นแบบ</h2>
				<BellScheduleSetupStep {year} {schedules} canManage={canSetup} busy={busy.schedule} onCreate={onCreateBellSchedule} onClone={onCloneBellSchedule} onUpdate={onUpdateBellSchedule} onSaved={() => {}} onCloned={(schedule) => { if (term && ['planning', 'ready'].includes(term.status)) chooseSchedule(schedule.id); }} />
			</section>
			<section aria-label="คาบของตารางต้นแบบ">
				<h2 class="mb-4 font-semibold">คาบของตารางต้นแบบ</h2>
				<BellSchedulePeriodsStep {year} {schedules} canManage={canSetup} busy={busy.periods} onLoad={onLoadBellSchedulePeriods} onReplace={onReplaceBellSchedulePeriods} onSaved={() => {}} />
			</section>{/key}
			</Collapsible.Content>
		</Collapsible.Root>
	{:else}<p class="text-sm text-muted-foreground">
		เพิ่มปีการศึกษาที่หน้าจัดการปีการศึกษาก่อน
	</p>{/if}
</div>
