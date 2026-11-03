<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';
	import { registerAcademicContextDirtySource } from '#lib/academic-context/store.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { periodSchoolDayOverride, periodDaysOutsideTerm, SCHOOL_WEEKDAYS } from '#lib/academic-core/school-schedule.js';
	import { bellScheduleBlockMessage } from '#lib/academic-core/period-setup.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import type {
		AcademicYear,
		AcademicTerm,
		BellSchedule,
		BellSchedulePeriod,
		BellSchedulePeriodsSnapshot,
		ReplaceBellSchedulePeriodsRequest
	} from '#lib/api/academic-core.js';
	import {
		normalizeSchoolDays,
		type AcademicWeekday
	} from '#lib/academic-core/foundation-presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Plus, Save, Trash2 } from '@lucide/svelte';

	type PeriodDraft = ReplaceBellSchedulePeriodsRequest['periods'][number];

	const WEEKDAYS: Array<{ code: AcademicWeekday; label: string }> = [
		{ code: 'MON', label: 'จันทร์' },
		{ code: 'TUE', label: 'อังคาร' },
		{ code: 'WED', label: 'พุธ' },
		{ code: 'THU', label: 'พฤหัสบดี' },
		{ code: 'FRI', label: 'ศุกร์' },
		{ code: 'SAT', label: 'เสาร์' },
		{ code: 'SUN', label: 'อาทิตย์' }
	];

	let {
		year,
		term,
		schedules,
		busy = false,
		canManage = false,
		onLoad,
		onReplace,
		onSaved
	}: {
		year: AcademicYear;
		term?: AcademicTerm;
		schedules: BellSchedule[];
		busy?: boolean;
		canManage?: boolean;
		onLoad: (
			id: string,
			options?: { signal?: AbortSignal }
		) => Promise<BellSchedulePeriodsSnapshot>;
		onReplace: (
			id: string,
			draft: ReplaceBellSchedulePeriodsRequest
		) => Promise<BellSchedulePeriodsSnapshot>;
		onSaved: (snapshot: BellSchedulePeriodsSnapshot) => void;
	} = $props();

	const inputPrefix = $derived(term ? `term-${term.id}` : `template-${year.id}`);
	let selectedScheduleId = $state(untrack(() => term?.bellScheduleId ?? ''));
	onMount(() => { if (selectedScheduleId) void selectSchedule(selectedScheduleId); });
	let periods = $state<PeriodDraft[]>([]);
	let loading = $state(false);
	let errorMessage = $state('');
	let baseVersion = $state<number | null>(null);
	let savedDraft = $state('');
	let comparison = $state<BellSchedulePeriodsSnapshot | null>(null);
	let comparing = $state(false);
	let disposed = false;
	const latest = new LatestRequest();
	onMount(() =>
		registerAcademicContextDirtySource(
			`bell-periods:${year.id}`,
			() => savedDraft !== '' && JSON.stringify(periods) !== savedDraft
		)
	);
	onDestroy(() => {
		disposed = true;
		latest.abort();
	});

	const selectedSchedule = $derived(
		schedules.find((schedule) => schedule.id === selectedScheduleId) ?? null
	);
	const schoolDays = $derived(normalizeSchoolDays(term && term.bellScheduleId === selectedScheduleId ? term.schoolDays : SCHOOL_WEEKDAYS.map((day) => day.code)));
	const editable = $derived(canManage && selectedSchedule?.editBlockedReason === null);
	const changed = $derived(
		baseVersion !== null && (selectedSchedule?.rowVersion ?? 0) > baseVersion
	);

	function draftFromPeriod(period?: BellSchedulePeriod): PeriodDraft {

		return {
			name: period?.name ?? null,
			startTime: period?.startTime.slice(0, 5) ?? '08:30',
			endTime: period?.endTime.slice(0, 5) ?? '09:20',
			orderIndex: period?.orderIndex ?? periods.length + 1,
			applicableDays: period ? periodSchoolDayOverride(period) : null,
			isActive: period?.isActive ?? true
		};
	}

	async function selectSchedule(id: string) {
		if (busy) return;
		const { revision, signal } = latest.begin();
		comparing = false;
		loading = Boolean(id);
		const identity = appIdentityKey();
		selectedScheduleId = id;
		periods = [];
		comparison = null;
		baseVersion = null;
		errorMessage = '';
		if (!id) return;
		try {
			const result = await onLoad(id, { signal });
			if (!disposed && latest.isCurrent(revision) && identity === appIdentityKey()) {
				periods = result.periods.map((period) => draftFromPeriod(period));
				baseVersion = result.rowVersion;
				savedDraft = JSON.stringify(periods);
			}
		} catch (error) {
			if (!disposed && latest.isCurrent(revision) && identity === appIdentityKey() && !isAbortError(error))
				errorMessage = error instanceof Error ? error.message : 'โหลดคาบเรียนไม่สำเร็จ';
		} finally {
			if (!disposed && latest.isCurrent(revision) && identity === appIdentityKey()) loading = false;
		}
	}

	async function compareLatest() {
		if (!selectedSchedule || comparing || busy) return;
		const scheduleId = selectedSchedule.id;
		const { revision, signal } = latest.begin();
		const identity = appIdentityKey();
		comparing = true;
		try {
			const result = await onLoad(scheduleId, { signal });
			if (!disposed && latest.isCurrent(revision) && identity === appIdentityKey()) comparison = result;
		} catch (cause) {
			if (!disposed && latest.isCurrent(revision) && identity === appIdentityKey() && !isAbortError(cause))
				errorMessage = cause instanceof Error ? cause.message : 'โหลดคาบล่าสุดไม่สำเร็จ';
		} finally {
			if (!disposed && latest.isCurrent(revision) && identity === appIdentityKey()) comparing = false;
		}
	}

	function addPeriod() {
		periods = [...periods, draftFromPeriod()];
	}

	function removePeriod(index: number) {
		periods = periods
			.filter((_, itemIndex) => itemIndex !== index)
			.map((period, itemIndex) => ({ ...period, orderIndex: itemIndex + 1 }));
	}

	function usesEverySchoolDay(period: PeriodDraft): boolean {
		return period.applicableDays === null;
	}

	function toggleEverySchoolDay(index: number, checked: boolean) {
		periods[index].applicableDays = checked ? null : [...schoolDays];
	}

	function toggleDay(index: number, day: AcademicWeekday, checked: boolean) {
		const current = periods[index].applicableDays ?? schoolDays;
		periods[index].applicableDays = normalizeSchoolDays(
			checked ? [...current, day] : current.filter((item) => item !== day)
		);
	}
	const outsideDays = $derived(periodDaysOutsideTerm(periods.map((period) => ({applicableDays: period.applicableDays?.join(',') ?? null})), schoolDays));

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!selectedSchedule || !editable || changed || busy || baseVersion === null) return;
		errorMessage = '';
		if (periods.length === 0) {
			errorMessage = 'กรุณาเพิ่มคาบเรียนอย่างน้อย 1 คาบ';
			return;
		}
		if (periods.some((period) => period.applicableDays !== null && period.applicableDays.length === 0)) {
			errorMessage = 'ทุกคาบต้องเลือกวันเรียนอย่างน้อย 1 วัน';
			return;
		}
		const identity = appIdentityKey();
		const scheduleId = selectedSchedule.id;
		try {
			const saved = await onReplace(scheduleId, {
				rowVersion: baseVersion,
				periods: periods.map((period) => ({
					...period,
					name: period.name?.trim() || null,
					applicableDays: period.applicableDays === null ? null : normalizeSchoolDays(period.applicableDays)
				}))
			});
			if (disposed || identity !== appIdentityKey() || scheduleId !== selectedScheduleId) return;
			periods = saved.periods.map((period) => draftFromPeriod(period));
			baseVersion = saved.rowVersion;
			savedDraft = JSON.stringify(periods);
			comparison = null;
			onSaved(saved);
		} catch (error) {
			if (disposed || identity !== appIdentityKey() || scheduleId !== selectedScheduleId) return;
			errorMessage = error instanceof Error ? error.message : 'บันทึกคาบเรียนไม่สำเร็จ';
		}
	}
</script>

<form class="space-y-5" onsubmit={submit}>
	<div class="space-y-1.5">
		<Label for={`${inputPrefix}-period-schedule`}>{term ? 'ตารางเวลาของภาคเรียนที่แสดง' : 'ตารางเวลาที่จะจัดคาบ'}</Label>
		<Select.Root
			type="single"
			value={selectedScheduleId}
			disabled={busy}
			onValueChange={(value) => void selectSchedule(value)}
		>
			<Select.Trigger id={`${inputPrefix}-period-schedule`} class="w-full">
				{selectedSchedule?.name ?? 'เลือกตารางเวลา'}
			</Select.Trigger>
			<Select.Content>
				{#each schedules as schedule (schedule.id)}
					<Select.Item value={schedule.id}>
						{schedule.name}{schedule.isDefault ? ' · ตารางหลัก' : ''}
					</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
		<p class="text-xs text-muted-foreground">
			วันเรียนของ {term && term.bellScheduleId === selectedScheduleId ? term.name : 'ตารางต้นแบบ'}: {schoolDays
				.map((code) => WEEKDAYS.find((day) => day.code === code)?.label)
				.join(', ')}
		</p>
	</div>

	{#if loading}
		<p class="rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
			กำลังโหลดคาบเรียน…
		</p>
	{:else if selectedSchedule}
		{#if outsideDays.length}<p role="alert" class="text-sm text-destructive">คาบที่บันทึกมีวันนอกวันเรียนของภาคนี้: {outsideDays.join(', ')} ข้อมูลเดิมยังอยู่ ตรวจวันเรียนและวันเฉพาะคาบก่อนบันทึก</p>{/if}
		{#if selectedSchedule.editBlockedReason}<p class="rounded-lg border bg-muted/20 p-3 text-sm">
				{bellScheduleBlockMessage(selectedSchedule.editBlockedReason)}
			</p>{/if}
		{#if changed}<p role="alert" class="rounded-lg border p-3 text-sm text-destructive">
				ข้อมูลตารางเวลาเปลี่ยนระหว่างแก้ไข คาบที่คุณแก้ไว้ยังอยู่ ตรวจข้อมูลล่าสุดก่อนบันทึกอีกครั้ง
			</p>
			<Button
				type="button"
				variant="outline"
				disabled={comparing || busy}
				onclick={() => void compareLatest()}>โหลดคาบล่าสุดเพื่อเปรียบเทียบ</Button
			>{/if}
		{#if comparison && changed}<div class="space-y-2 rounded-lg border p-3 text-sm">
				<p class="font-medium">คาบที่บันทึกไว้ล่าสุด</p>
				{#each comparison.periods as period (period.id)}<p>
						{period.name ?? `คาบ ${period.orderIndex}`} · {period.startTime.slice(
							0,
							5
						)}–{period.endTime.slice(0, 5)}
					</p>{/each}<Button
					type="button"
					variant="outline"
					disabled={busy || selectedSchedule.rowVersion > comparison.rowVersion}
					onclick={() => {
						if (comparison) baseVersion = comparison.rowVersion;
					}}>ตรวจสอบแล้ว ใช้ฉบับล่าสุดเป็นฐาน</Button
				>
			</div>{/if}
		<div class="space-y-3">
			{#each periods as period, index (`${selectedSchedule.id}-${index}`)}
				<fieldset
					class="space-y-3 rounded-xl border bg-background p-4"
					disabled={busy || !editable}
				>
					<div class="flex items-center justify-between gap-3">
						<legend class="font-medium">คาบที่ {index + 1}</legend>
						<Button
							type="button"
							size="icon-sm"
							variant="ghost"
							onclick={() => removePeriod(index)}
							aria-label={`ลบคาบที่ ${index + 1}`}
						>
							<Trash2 class="size-4" />
						</Button>
					</div>
					<div class="grid gap-3 sm:grid-cols-[80px_minmax(160px,1fr)_130px_130px]">
						<div class="space-y-1.5">
							<Label for={`${inputPrefix}-period-order-${index}`}>ลำดับ</Label>
							<Input
								id={`${inputPrefix}-period-order-${index}`}
								type="number"
								min="1"
								bind:value={period.orderIndex}
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for={`${inputPrefix}-period-name-${index}`}>ชื่อคาบ (ไม่บังคับ)</Label>
							<Input
								id={`${inputPrefix}-period-name-${index}`}
								bind:value={period.name}
								placeholder={`คาบ ${index + 1}`}
							/>
						</div>
						<div class="space-y-1.5">
							<Label for={`${inputPrefix}-period-start-${index}`}>{term ? 'เริ่มคาบของภาคเรียน' : 'เริ่ม'}</Label>
							<Input
								id={`${inputPrefix}-period-start-${index}`}
								type="time"
								bind:value={period.startTime}
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for={`${inputPrefix}-period-end-${index}`}>{term ? 'สิ้นสุดคาบของภาคเรียน' : 'สิ้นสุด'}</Label>
							<Input id={`${inputPrefix}-period-end-${index}`} type="time" bind:value={period.endTime} required />
						</div>
					</div>

					<div class="flex flex-wrap items-center gap-4 border-t pt-3 text-sm">
						<label class="flex cursor-pointer items-center gap-2 font-medium">
							<Checkbox
								checked={usesEverySchoolDay(period)}
								onCheckedChange={(checked) => toggleEverySchoolDay(index, checked ?? false)}
							/>
							ใช้ทุกวันเรียน
						</label>
						<label class="flex cursor-pointer items-center gap-2">
							<Checkbox bind:checked={period.isActive} /> เปิดใช้คาบนี้
						</label>
					</div>
					<div class="flex flex-wrap gap-2">
						{#each WEEKDAYS.filter((day) => schoolDays.includes(day.code) || period.applicableDays?.includes(day.code)) as day (day.code)}
							<label
								class="flex cursor-pointer items-center gap-2 rounded-lg border px-3 py-2 text-xs"
							>
								<Checkbox
									checked={period.applicableDays === null || period.applicableDays.includes(day.code)}
									onCheckedChange={(checked) => toggleDay(index, day.code, checked ?? false)}
								/>
								{day.label}
							</label>
						{/each}
					</div>
				</fieldset>
			{:else}
				<p class="rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
					ตารางนี้ยังไม่มีคาบ กด “เพิ่มคาบ” เพื่อเริ่มจัดเวลา
				</p>
			{/each}
		</div>

		{#if editable}<div class="flex flex-wrap gap-2">
				<Button type="button" variant="outline" onclick={addPeriod}>
					<Plus class="size-4" /> เพิ่มคาบ
				</Button>
				<Button type="submit" disabled={busy || changed || periods.length === 0}>
					<Save class="size-4" /> {term ? 'บันทึกคาบทั้งหมดของภาคเรียน' : 'บันทึกคาบทั้งหมด'}
				</Button>
			</div>{/if}
	{/if}

	{#if errorMessage}
		<p
			role="alert"
			class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
		>
			{errorMessage}
		</p>
	{/if}
</form>
