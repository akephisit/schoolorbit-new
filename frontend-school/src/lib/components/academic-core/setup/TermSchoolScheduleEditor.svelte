<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { registerAcademicContextDirtySource } from '#lib/academic-context/store.js';
	import type {
		AcademicTerm,
		AcademicYear,
		BellSchedule,
		BellSchedulePeriodsSnapshot,
		UpdateAcademicTermRequest,
		CopyAcademicTermSchoolScheduleRequest,
		AcademicTermSchoolScheduleCopyResponse
	} from '#lib/api/academic-core.js';
	import { normalizeSchoolDays } from '#lib/academic-core/foundation-presentation.js';
	import {
		previousSchoolScheduleTerm,
		schoolScheduleSourceFields,
		periodDaysOutsideTerm,
		type SchoolScheduleSource
	} from '#lib/academic-core/school-schedule.js';
	import SchoolDaysField from './SchoolDaysField.svelte';
	import SchoolScheduleSourcePreview from './SchoolScheduleSourcePreview.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	let {
		year,
		years,
		term,
		terms,
		schedules,
		canEditDays,
		canCopy,
		busy,
		onUpdate,
		onCopy,
		onLoad,
		ondirtychange
	}: {
		year: AcademicYear;
		years: AcademicYear[];
		term: AcademicTerm;
		terms: AcademicTerm[];
		schedules: BellSchedule[];
		canEditDays: boolean;
		canCopy: boolean;
		busy: boolean;
		onUpdate: (id: string, draft: UpdateAcademicTermRequest) => Promise<AcademicTerm>;
		onCopy: (
			id: string,
			draft: CopyAcademicTermSchoolScheduleRequest
		) => Promise<AcademicTermSchoolScheduleCopyResponse>;
		onLoad: (
			id: string,
			options?: { signal?: AbortSignal }
		) => Promise<BellSchedulePeriodsSnapshot>;
		ondirtychange: (dirty: boolean) => void;
	} = $props();
	let base = $state(untrack(() => ({ ...term, schoolDays: [...term.schoolDays] })));
	let days = $state(untrack(() => normalizeSchoolDays(term.schoolDays)));
	let copying = $state(false);
	let source = $state<SchoolScheduleSource | null>(null);
	let error = $state('');
	let disposed = false;
	let operation = 0;
	const dirty = $derived(
		copying || JSON.stringify(days) !== JSON.stringify(normalizeSchoolDays(base.schoolDays))
	);
	const stale = $derived(dirty && term.rowVersion !== base.rowVersion);
	const previous = $derived(previousSchoolScheduleTerm(year, years, terms, term));
	const sourceInvalid = $derived(
		!source ||
			!source.snapshot.periods.some((period) => period.isActive) ||
			source.schoolDays.length === 0 ||
			periodDaysOutsideTerm(source.snapshot.periods, source.schoolDays).length > 0 ||
			(terms.find((item) => item.id === source?.term.id)?.rowVersion ?? 0) >
				source.term.rowVersion ||
			(schedules.find((schedule) => schedule.id === source?.term.bellScheduleId)?.rowVersion ?? 0) >
				source.snapshot.rowVersion
	);
	onMount(() => registerAcademicContextDirtySource(`term-school-days:${term.id}`, () => dirty));
	onDestroy(() => {
		disposed = true;
		operation += 1;
		ondirtychange(false);
	});
	function changeDays(value: typeof days) {
		if (!dirty) base = { ...term, schoolDays: [...term.schoolDays] };
		days = value;
		ondirtychange(JSON.stringify(days) !== JSON.stringify(normalizeSchoolDays(base.schoolDays)));
	}
	function reset() {
		operation += 1;
		base = { ...term, schoolDays: [...term.schoolDays] };
		days = normalizeSchoolDays(term.schoolDays);
		copying = false;
		source = null;
		error = '';
		ondirtychange(false);
	}
	async function save() {
		if (
			busy ||
			stale ||
			!canEditDays ||
			(!copying && days.length === 0) ||
			(copying && (!canCopy || sourceInvalid))
		)
			return;
		const identity = appIdentityKey();
		const submitted = ++operation;
		error = '';
		try {
			if (copying && source)
				await onCopy(base.id, {
					...schoolScheduleSourceFields(source),
					rowVersion: base.rowVersion
				});
			else
				await onUpdate(base.id, {
					termType: base.termType,
					customName: base.name,
					includedInYearResult: base.includedInYearResult,
					blocksYearClosure: base.blocksYearClosure,
					bellScheduleId: base.bellScheduleId,
					schoolDays: days,
					rowVersion: base.rowVersion
				});
			if (disposed || identity !== appIdentityKey() || operation !== submitted) return;
			reset();
		} catch (cause) {
			if (disposed || identity !== appIdentityKey() || operation !== submitted) return;
			error = cause instanceof Error ? cause.message : 'บันทึกวันเรียนไม่สำเร็จ';
		}
	}
</script>

<div class="space-y-3" aria-label="วันเรียนและต้นแบบของภาคเรียน">
	{#if copying}
		<SchoolScheduleSourcePreview
			{years}
			terms={terms.filter((item) => item.id !== term.id)}
			{schedules}
			defaultSourceId={previous?.id ?? ''}
			disabled={busy || !canEditDays || !canCopy}
			{onLoad}
			onchange={(value) => (source = value)}
		/>
	{:else}
		<SchoolDaysField
			value={dirty ? days : term.schoolDays}
			disabled={!canEditDays || busy}
			onchange={changeDays}
		/>
	{/if}
	{#if canEditDays}
		{#if !copying && days.length === 0}<p role="alert" class="text-sm text-destructive">
				เลือกวันเรียนอย่างน้อย 1 วัน
			</p>{/if}
		<div class="flex flex-wrap gap-2">
			{#if dirty}<Button
					disabled={busy || stale || (copying ? sourceInvalid : days.length === 0)}
					onclick={() => void save()}
					>{copying ? 'บันทึกวันเรียนและคาบที่คัดลอก' : 'บันทึกวันเรียน'}</Button
				><Button variant="outline" disabled={busy} onclick={reset}>ยกเลิกการแก้ไขวันเรียน</Button>
			{:else if canCopy && terms.some((item) => item.id !== term.id)}<Button
					variant="outline"
					disabled={busy}
					onclick={() => {
						base = { ...term, schoolDays: [...term.schoolDays] };
						copying = true;
						ondirtychange(true);
					}}>คัดลอกวันเรียนและคาบจากภาคเรียนอื่น</Button
				>{/if}
		</div>
		{#if !canCopy}<p class="text-sm text-muted-foreground">
				เปลี่ยนวันเรียนได้โดยต้องคงวันที่ตารางสอนเดิมใช้ การเปลี่ยนชุดคาบต้องจัดการรายการอ้างอิงก่อน
			</p>{/if}
	{:else}<p class="text-sm text-muted-foreground">
			วันเรียนของภาคเรียนนี้เป็นข้อมูลอ่านอย่างเดียว เมื่อเริ่มใช้แล้วจะไม่เปิดแก้ไข
			แม้กลับมาเป็นร่าง
		</p>{/if}
	{#if !canEditDays && dirty}<Button variant="outline" disabled={busy} onclick={reset}
			>ยกเลิกการแก้ไขวันเรียน</Button
		>{/if}

	{#if stale}<p role="alert" class="text-sm text-destructive">
			ข้อมูลภาคเรียนเปลี่ยนระหว่างแก้ไข ฉบับร่างของคุณยังอยู่ ยกเลิกแล้วตรวจข้อมูลล่าสุดก่อนบันทึก
		</p>{/if}
	{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
</div>
