<script lang="ts">
	import type {
		AcademicYear,
		BellSchedule,
		BellSchedulePeriod,
		ReplaceBellSchedulePeriodsRequest
	} from '$lib/api/academic-core';
	import {
		normalizeSchoolDays,
		type AcademicWeekday
	} from '$lib/academic-core/foundation-presentation';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Plus, Save, Trash2 } from 'lucide-svelte';

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
		schedules,
		busy = false,
		onLoad,
		onReplace,
		onSaved
	}: {
		year: AcademicYear;
		schedules: BellSchedule[];
		busy?: boolean;
		onLoad: (id: string) => Promise<BellSchedulePeriod[]>;
		onReplace: (
			id: string,
			draft: ReplaceBellSchedulePeriodsRequest
		) => Promise<BellSchedulePeriod[]>;
		onSaved: (scheduleId: string, periods: BellSchedulePeriod[]) => void;
	} = $props();

	let selectedScheduleId = $state('');
	let periods = $state<PeriodDraft[]>([]);
	let loading = $state(false);
	let errorMessage = $state('');

	const selectedSchedule = $derived(
		schedules.find((schedule) => schedule.id === selectedScheduleId) ?? null
	);
	const schoolDays = $derived(normalizeSchoolDays(year.schoolDays));

	function draftFromPeriod(period?: BellSchedulePeriod): PeriodDraft {
		const storedDays = period?.applicableDays?.match(/[A-Za-z]+/g) ?? [];
		return {
			name: period?.name ?? null,
			startTime: period?.startTime.slice(0, 5) ?? '08:30',
			endTime: period?.endTime.slice(0, 5) ?? '09:20',
			orderIndex: period?.orderIndex ?? periods.length + 1,
			applicableDays: period ? normalizeSchoolDays(storedDays) : [...schoolDays],
			isActive: period?.isActive ?? true
		};
	}

	async function selectSchedule(id: string) {
		selectedScheduleId = id;
		periods = [];
		errorMessage = '';
		if (!id) return;
		loading = true;
		try {
			periods = (await onLoad(id)).map((period) => draftFromPeriod(period));
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดคาบเรียนไม่สำเร็จ';
		} finally {
			loading = false;
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
		const normalized = normalizeSchoolDays(period.applicableDays);
		return (
			schoolDays.every((day) => normalized.includes(day)) && normalized.length === schoolDays.length
		);
	}

	function toggleEverySchoolDay(index: number, checked: boolean) {
		periods[index].applicableDays = checked ? [...schoolDays] : [];
	}

	function toggleDay(index: number, day: AcademicWeekday, checked: boolean) {
		const current = periods[index].applicableDays;
		periods[index].applicableDays = normalizeSchoolDays(
			checked ? [...current, day] : current.filter((item) => item !== day)
		);
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!selectedSchedule) return;
		errorMessage = '';
		if (periods.length === 0) {
			errorMessage = 'กรุณาเพิ่มคาบเรียนอย่างน้อย 1 คาบ';
			return;
		}
		if (periods.some((period) => period.applicableDays.length === 0)) {
			errorMessage = 'ทุกคาบต้องเลือกวันเรียนอย่างน้อย 1 วัน';
			return;
		}
		try {
			const saved = await onReplace(selectedSchedule.id, {
				rowVersion: selectedSchedule.rowVersion,
				periods: periods.map((period) => ({
					...period,
					name: period.name?.trim() || null,
					applicableDays: normalizeSchoolDays(period.applicableDays)
				}))
			});
			periods = saved.map((period) => draftFromPeriod(period));
			onSaved(selectedSchedule.id, saved);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกคาบเรียนไม่สำเร็จ';
		}
	}
</script>

<form class="space-y-5" onsubmit={submit}>
	<div class="space-y-1.5">
		<Label for="period-schedule">ตารางเวลาที่จะจัดคาบ</Label>
		<Select.Root
			type="single"
			value={selectedScheduleId}
			onValueChange={(value) => void selectSchedule(value)}
		>
			<Select.Trigger id="period-schedule" class="w-full">
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
			วันเรียนของ {year.name}: {schoolDays
				.map((code) => WEEKDAYS.find((day) => day.code === code)?.label)
				.join(', ')}
		</p>
	</div>

	{#if loading}
		<p class="rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
			กำลังโหลดคาบเรียน…
		</p>
	{:else if selectedSchedule}
		<div class="space-y-3">
			{#each periods as period, index (`${selectedSchedule.id}-${index}`)}
				<fieldset class="space-y-3 rounded-xl border bg-background p-4" disabled={busy}>
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
							<Label for={`period-order-${index}`}>ลำดับ</Label>
							<Input
								id={`period-order-${index}`}
								type="number"
								min="1"
								bind:value={period.orderIndex}
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for={`period-name-${index}`}>ชื่อคาบ (ไม่บังคับ)</Label>
							<Input
								id={`period-name-${index}`}
								bind:value={period.name}
								placeholder={`คาบ ${index + 1}`}
							/>
						</div>
						<div class="space-y-1.5">
							<Label for={`period-start-${index}`}>เริ่ม</Label>
							<Input
								id={`period-start-${index}`}
								type="time"
								bind:value={period.startTime}
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for={`period-end-${index}`}>สิ้นสุด</Label>
							<Input id={`period-end-${index}`} type="time" bind:value={period.endTime} required />
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
						{#each WEEKDAYS.filter((day) => schoolDays.includes(day.code)) as day (day.code)}
							<label
								class="flex cursor-pointer items-center gap-2 rounded-lg border px-3 py-2 text-xs"
							>
								<Checkbox
									checked={period.applicableDays.includes(day.code)}
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

		<div class="flex flex-wrap gap-2">
			<Button type="button" variant="outline" onclick={addPeriod}>
				<Plus class="size-4" /> เพิ่มคาบ
			</Button>
			<Button type="submit" disabled={busy || periods.length === 0}>
				<Save class="size-4" /> บันทึกคาบทั้งหมด
			</Button>
		</div>
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
