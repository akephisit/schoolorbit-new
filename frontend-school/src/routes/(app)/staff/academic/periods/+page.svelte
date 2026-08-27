<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import {
		listBellSchedulePeriods,
		listBellSchedules,
		replaceBellSchedulePeriods,
		type BellSchedule,
		type ReplaceBellSchedulePeriodsRequest
	} from '$lib/api/academic-core';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { CirclePlus, Clock3, Loader2, Save, Trash2 } from 'lucide-svelte';

	type PeriodDraft = ReplaceBellSchedulePeriodsRequest['periods'][number];

	const academicContext = getAcademicContextStore();
	const academicYearId = $derived($academicContext.selected.academicYearId);
	let schedules = $state<BellSchedule[]>([]);
	let selectedScheduleId = $state('');
	let scheduleSelectValue = $state('');
	let periods = $state<PeriodDraft[]>([]);
	let loading = $state(false);
	let saving = $state(false);
	let dirty = $state(false);
	let errorMessage = $state('');
	let revision = 0;

	const canReadAcademicPeriods = $derived(
		$can.hasAny(PERMISSIONS.ACADEMIC_YEAR_READ_SCHOOL, PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL)
	);
	const canManageAcademicPeriods = $derived($can.has(PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL));
	const selectedSchedule = $derived(
		schedules.find((schedule) => schedule.id === selectedScheduleId) ?? null
	);

	function periodDraft(
		period: Awaited<ReturnType<typeof listBellSchedulePeriods>>[number]
	): PeriodDraft {
		return {
			name: period.name ?? null,
			startTime: period.startTime.slice(0, 5),
			endTime: period.endTime.slice(0, 5),
			orderIndex: period.orderIndex,
			applicableDays: period.applicableDays
				? period.applicableDays
						.split(',')
						.map((day) => day.trim())
						.filter(Boolean)
				: [],
			isActive: period.isActive
		};
	}

	async function loadPeriods(scheduleId: string): Promise<void> {
		const rows = await listBellSchedulePeriods(scheduleId);
		periods = rows.sort((a, b) => a.orderIndex - b.orderIndex).map(periodDraft);
		dirty = false;
	}

	async function loadWorkspace(yearId: string): Promise<void> {
		const current = ++revision;
		loading = true;
		errorMessage = '';
		try {
			const rows = await listBellSchedules(yearId);
			if (current !== revision) return;
			schedules = rows;
			const preferred = rows.find((schedule) => schedule.isDefault) ?? rows[0] ?? null;
			selectedScheduleId = preferred?.id ?? '';
			scheduleSelectValue = selectedScheduleId;
			periods = [];
			if (preferred) await loadPeriods(preferred.id);
		} catch (error) {
			if (current === revision) {
				errorMessage = error instanceof Error ? error.message : 'โหลดตารางคาบไม่สำเร็จ';
			}
		} finally {
			if (current === revision) loading = false;
		}
	}

	async function selectSchedule(nextId: string): Promise<void> {
		if (nextId === selectedScheduleId) {
			scheduleSelectValue = selectedScheduleId;
			return;
		}
		if (dirty) {
			scheduleSelectValue = selectedScheduleId;
			toast.warning('กรุณาบันทึกคาบที่แก้ไขก่อนเปลี่ยนตารางเวลา');
			return;
		}
		loading = true;
		try {
			await loadPeriods(nextId);
			selectedScheduleId = nextId;
			scheduleSelectValue = nextId;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดคาบไม่สำเร็จ';
		} finally {
			scheduleSelectValue = selectedScheduleId;
			loading = false;
		}
	}

	function addPeriod(): void {
		periods.push({
			name: `คาบ ${periods.length + 1}`,
			startTime: '08:00',
			endTime: '09:00',
			orderIndex: periods.length + 1,
			applicableDays: [],
			isActive: true
		});
		dirty = true;
	}

	function removePeriod(index: number): void {
		periods.splice(index, 1);
		dirty = true;
	}

	function movePeriod(index: number, direction: -1 | 1): void {
		const target = index + direction;
		if (target < 0 || target >= periods.length) return;
		const [period] = periods.splice(index, 1);
		periods.splice(target, 0, period);
		dirty = true;
	}

	async function savePeriods(): Promise<void> {
		const schedule = selectedSchedule;
		if (!selectedScheduleId || !schedule || !canManageAcademicPeriods) return;
		saving = true;
		errorMessage = '';
		try {
			const saved = await replaceBellSchedulePeriods(selectedScheduleId, {
				rowVersion: schedule.rowVersion,
				periods: periods.map((period, index) => ({
					...period,
					name: period.name?.trim() || null,
					orderIndex: index + 1
				}))
			});
			schedules = schedules.map((item) =>
				item.id === schedule.id ? { ...item, rowVersion: item.rowVersion + 1 } : item
			);
			periods = saved.sort((a, b) => a.orderIndex - b.orderIndex).map(periodDraft);
			dirty = false;
			toast.success('บันทึกคาบเรียนแล้ว');
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกคาบเรียนไม่สำเร็จ';
			toast.error(errorMessage);
		} finally {
			saving = false;
		}
	}

	onMount(() => {
		let loadedYearId: string | null = null;
		const unregisterDirty = registerAcademicContextDirtySource(
			'bell-schedule-periods',
			() => dirty
		);
		const unsubscribe = academicContext.subscribe((state) => {
			const yearId = state.selected.academicYearId;
			if (yearId && yearId !== loadedYearId) {
				loadedYearId = yearId;
				void loadWorkspace(yearId);
			}
		});
		return () => {
			unsubscribe();
			unregisterDirty();
		};
	});
</script>

<PageShell
	title="ตั้งค่าคาบเวลา"
	description="กำหนดคาบของตารางเวลาแต่ละชุดในปีการศึกษาที่เลือก แล้วนำไปใช้กับทุกภาคเรียนของปีนั้น"
>
	{#if !canReadAcademicPeriods}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูคาบเวลา"
			description="ต้องมีสิทธิ์อ่านหรือจัดการปีการศึกษาระดับโรงเรียน"
		/>
	{:else if !academicYearId}
		<PageState
			variant="empty"
			title="เลือกปีการศึกษาก่อน"
			description="ใช้ตัวเลือกปีการศึกษาบนแถบด้านบน"
		/>
	{:else if loading}
		<PageSkeleton variant="cards" rows={4} />
	{:else if errorMessage && schedules.length === 0}
		<PageState
			variant="error"
			title="โหลดคาบเวลาไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(academicYearId)}
		/>
	{:else if schedules.length === 0}
		<PageState
			title="ยังไม่มีตารางเวลาสำหรับปีนี้"
			description="สร้างตารางเวลาในหน้าตั้งค่าปีและภาคเรียนก่อน แล้วจึงกลับมากำหนดคาบ"
		/>
	{:else}
		<div class="space-y-5">
			<Card.Root class="gap-0 py-0">
				<Card.Content class="flex flex-wrap items-center justify-between gap-4 pt-6">
					<div class="min-w-64 space-y-2">
						<Label for="bell-schedule">ตารางเวลา</Label>
						<Select.Root
							type="single"
							bind:value={scheduleSelectValue}
							onValueChange={selectSchedule}
						>
							<Select.Trigger id="bell-schedule" class="w-full">
								{selectedSchedule
									? `${selectedSchedule.code} · ${selectedSchedule.name}`
									: 'เลือกตารางเวลา'}
							</Select.Trigger>
							<Select.Content>
								{#each schedules as schedule (schedule.id)}
									<Select.Item value={schedule.id}>{schedule.code} · {schedule.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="flex items-center gap-2">
						{#if selectedSchedule?.isDefault}<Badge variant="secondary">ค่าเริ่มต้นของปี</Badge
							>{/if}
						{#if canManageAcademicPeriods}
							<Button variant="outline" disabled={saving || !dirty} onclick={savePeriods}>
								{#if saving}<Loader2 class="animate-spin" />{:else}<Save />{/if}
								บันทึกทั้งหมด
							</Button>
						{/if}
					</div>
				</Card.Content>
			</Card.Root>

			{#if dirty}
				<p class="text-amber-700 dark:text-amber-300 text-sm">
					มีการแก้ไขที่ยังไม่บันทึก — ต้องบันทึกก่อนเปลี่ยนปีการศึกษาหรือตารางเวลา
				</p>
			{/if}
			{#if errorMessage}
				<div
					class="border-destructive/30 bg-destructive/5 text-destructive rounded-lg border p-3 text-sm"
				>
					{errorMessage}
				</div>
			{/if}

			<Card.Root>
				<Card.Header>
					<Card.Title class="flex items-center gap-2"><Clock3 /> คาบเรียน</Card.Title>
					<Card.Description>
						ลำดับและช่วงเวลานี้เป็น snapshot ที่ตารางสอนของภาคเรียนในปีนี้ใช้อ้างอิง
					</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-3">
					{#each periods as period, index (index)}
						<div
							class="grid items-end gap-3 rounded-lg border p-3 md:grid-cols-[4rem_minmax(10rem,1fr)_9rem_9rem_auto]"
						>
							<div class="space-y-2">
								<Label>ลำดับ</Label>
								<div
									class="flex h-9 items-center justify-center rounded-md border text-sm font-medium"
								>
									{index + 1}
								</div>
							</div>
							<div class="space-y-2">
								<Label for={`period-name-${index}`}>ชื่อคาบ</Label>
								<Input
									id={`period-name-${index}`}
									bind:value={period.name}
									disabled={!canManageAcademicPeriods}
									oninput={() => (dirty = true)}
								/>
							</div>
							<div class="space-y-2">
								<Label for={`period-start-${index}`}>เริ่ม</Label>
								<Input
									id={`period-start-${index}`}
									type="time"
									bind:value={period.startTime}
									disabled={!canManageAcademicPeriods}
									oninput={() => (dirty = true)}
								/>
							</div>
							<div class="space-y-2">
								<Label for={`period-end-${index}`}>สิ้นสุด</Label>
								<Input
									id={`period-end-${index}`}
									type="time"
									bind:value={period.endTime}
									disabled={!canManageAcademicPeriods}
									oninput={() => (dirty = true)}
								/>
							</div>
							{#if canManageAcademicPeriods}
								<div class="flex justify-end gap-1">
									<Button
										variant="ghost"
										size="icon"
										disabled={index === 0}
										title="เลื่อนขึ้น"
										onclick={() => movePeriod(index, -1)}>↑</Button
									>
									<Button
										variant="ghost"
										size="icon"
										disabled={index === periods.length - 1}
										title="เลื่อนลง"
										onclick={() => movePeriod(index, 1)}>↓</Button
									>
									<Button
										variant="ghost"
										size="icon"
										title="ลบคาบ"
										onclick={() => removePeriod(index)}><Trash2 /></Button
									>
								</div>
							{/if}
						</div>
					{/each}
					{#if periods.length === 0}
						<div class="border-border rounded-lg border border-dashed p-8 text-center text-sm">
							ยังไม่มีคาบในตารางเวลานี้
						</div>
					{/if}
					{#if canManageAcademicPeriods}
						<Button variant="outline" class="w-full border-dashed" onclick={addPeriod}>
							<CirclePlus /> เพิ่มคาบ
						</Button>
					{/if}
				</Card.Content>
			</Card.Root>
		</div>
	{/if}
</PageShell>
