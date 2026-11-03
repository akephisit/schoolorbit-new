<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';
	import type { AcademicTerm, AcademicYear, BellSchedule, BellSchedulePeriodsSnapshot } from '#lib/api/academic-core.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { normalizeSchoolDays, type AcademicWeekday } from '#lib/academic-core/foundation-presentation.js';
	import { parsePeriodSchoolDays, periodDaysOutsideTerm, SCHOOL_WEEKDAYS, type SchoolScheduleSource } from '#lib/academic-core/school-schedule.js';
	import SchoolDaysField from './SchoolDaysField.svelte';
	import { PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	let { years, terms, schedules, defaultSourceId = '', disabled = false, onLoad, onchange }: {
		years: AcademicYear[];
		terms: AcademicTerm[];
		schedules: BellSchedule[];
		defaultSourceId?: string;
		disabled?: boolean;
		onLoad: (id: string, options?: { signal?: AbortSignal }) => Promise<BellSchedulePeriodsSnapshot>;
		onchange: (source: SchoolScheduleSource | null) => void;
	} = $props();
	let sourceId = $state(untrack(() => defaultSourceId));
	let source = $state<SchoolScheduleSource | null>(null);
	let loading = $state(false);
	let error = $state('');
	const latest = new LatestRequest();
	let disposed = false;
	const currentTerm = $derived(terms.find((term) => term.id === sourceId));
	const stale = $derived(!!source && (currentTerm?.rowVersion !== source.term.rowVersion ||
		(schedules.find((schedule) => schedule.id === source?.term.bellScheduleId)?.rowVersion ?? 0) > source.snapshot.rowVersion));
	const outsideDays = $derived(source ? periodDaysOutsideTerm(source.snapshot.periods, source.schoolDays) : []);
	onMount(() => { if (sourceId) void chooseSource(sourceId); });
	onDestroy(() => { disposed = true; latest.abort(); });
	function label(term: AcademicTerm) {
		return `${years.find((year) => year.id === term.academicYearId)?.name ?? ''} · ${term.name}`;
	}
	async function chooseSource(id: string, preserveDays = false) {
		const { revision, signal } = latest.begin();
		const identity = appIdentityKey();
		const selected = terms.find((term) => term.id === id);
		const draftDays = preserveDays ? source?.schoolDays : null;
		sourceId = id;
		source = null;
		onchange(null);
		error = '';
		loading = !!selected;
		if (!selected) return;
		const capturedTerm = { ...selected, schoolDays: [...selected.schoolDays] };
		try {
			const snapshot = await onLoad(selected.bellScheduleId, { signal });
			if (disposed || !latest.isCurrent(revision) || identity !== appIdentityKey()) return;
			for (const period of snapshot.periods) { const parsed = parsePeriodSchoolDays(period); if (!parsed.ok) throw new Error(parsed.error); }
			source = { term: capturedTerm, snapshot, schoolDays: draftDays ?? normalizeSchoolDays(capturedTerm.schoolDays) };
			onchange(source);
		} catch (cause) {
			if (disposed || !latest.isCurrent(revision) || identity !== appIdentityKey() || isAbortError(cause)) return;
			error = cause instanceof Error ? cause.message : 'โหลดต้นแบบไม่สำเร็จ';
		} finally {
			if (!disposed && latest.isCurrent(revision) && identity === appIdentityKey()) loading = false;
		}
	}
	function changeDays(days: AcademicWeekday[]) {
		if (!source) return;
		source = { ...source, schoolDays: days };
		onchange(source);
	}
</script>

<div class="space-y-3">
	<div class="space-y-1.5">
		<Label for="school-schedule-source">คัดลอกวันเรียนและคาบเรียนจาก</Label>
		<Select.Root type="single" value={sourceId} {disabled} onValueChange={(id) => void chooseSource(id)}>
			<Select.Trigger id="school-schedule-source" class="w-full">{currentTerm ? label(currentTerm) : 'เลือกภาคเรียนต้นแบบ'}</Select.Trigger>
			<Select.Content>{#each terms as term (term.id)}<Select.Item value={term.id}>{label(term)}</Select.Item>{/each}</Select.Content>
		</Select.Root>
	</div>
	{#if loading}<PageSkeleton variant="cards" rows={1} />
	{:else if source}
		<SchoolDaysField value={source.schoolDays} disabled={disabled || stale} onchange={changeDays} />
		<p class="text-xs text-muted-foreground">คัดลอกเป็นชุดของภาคเรียนนี้ แก้ไขภายหลังได้โดยไม่เปลี่ยนต้นแบบ ตรวจวันเรียนและคาบก่อนบันทึก</p>
		<div class="space-y-1 rounded-lg border p-3 text-sm" aria-label="ตัวอย่างคาบที่คัดลอก">
			{#each source.snapshot.periods as period (period.id)}<p>{period.name ?? `คาบ ${period.orderIndex}`} · {period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)} · {period.applicableDays === null ? 'ทุกวันเรียนของภาคเรียน' : period.applicableDays}</p>
			{:else}<p>ต้นแบบยังไม่มีคาบเรียน</p>{/each}
			{#if !source.snapshot.periods.some((period) => period.isActive)}<p role="alert" class="text-destructive">ต้นแบบยังไม่มีคาบที่เปิดใช้ เลือกต้นแบบอื่นหรือเตรียมคาบก่อนคัดลอก</p>{/if}
		</div>
		{#if outsideDays.length}<p role="alert" class="text-sm text-destructive">คาบต้นแบบมีวันนอกวันเรียนที่เลือก: {outsideDays.map((day) => SCHOOL_WEEKDAYS.find((item) => item.code === day)?.label).join(', ')} เลือกวันเหล่านี้เพิ่มก่อนคัดลอก</p>{/if}
		{#if stale}<p role="alert" class="text-sm text-destructive">ต้นแบบเปลี่ยนแล้ว ฉบับร่างวันเรียนของคุณยังอยู่ ตรวจต้นแบบล่าสุดก่อนบันทึก</p><Button type="button" variant="outline" {disabled} onclick={() => void chooseSource(sourceId, true)}>ตรวจต้นแบบล่าสุด</Button>{/if}
	{/if}
	{#if error}<p role="alert" class="text-sm text-destructive">{error}</p><Button type="button" variant="outline" {disabled} onclick={() => void chooseSource(sourceId, true)}>ลองโหลดต้นแบบอีกครั้ง</Button>{/if}
</div>
