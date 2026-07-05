<script lang="ts">
	import type {
		ExamDayDetail,
		ExamScheduleItem,
		ExamSession,
		PlaceExamSessionInput
	} from '$lib/api/examSchedule';
	import { LoadingButton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { addMinutes, validateExamSessionPlacement } from '$lib/utils/examScheduleTime';
	import { CalendarPlus, GripVertical } from 'lucide-svelte';

	let {
		unscheduledItems = [],
		days = [],
		scheduledSessions = [],
		readonly = false,
		placingItemId = null,
		onPlaceSession
	}: {
		unscheduledItems: ExamScheduleItem[];
		days: ExamDayDetail[];
		scheduledSessions?: ExamSession[];
		readonly?: boolean;
		placingItemId?: string | null;
		onPlaceSession?: (input: PlaceExamSessionInput) => Promise<boolean> | boolean;
	} = $props();

	let dialogOpen = $state(false);
	let selectedItem = $state<ExamScheduleItem | null>(null);
	let selectedDayId = $state('');
	let selectedStartTime = $state('08:30');
	let dialogError = $state('');

	const sortedDays = $derived([...days].sort((a, b) => a.sortOrder - b.sortOrder));
	const selectedDay = $derived(days.find((day) => day.id === selectedDayId) ?? sortedDays[0] ?? null);

	function itemSubject(item: ExamScheduleItem): string {
		return item.subjectNameTh || item.subjectNameEn || item.subjectCode || 'ไม่ระบุวิชา';
	}

	function dayLabel(day: ExamDayDetail | null): string {
		if (!day) return 'เลือกวันสอบ';
		const date = new Date(day.examDate).toLocaleDateString('th-TH', {
			weekday: 'short',
			month: 'short',
			day: 'numeric'
		});
		return day.label ? `${day.label} · ${date}` : date;
	}

	function openDialog(item: ExamScheduleItem) {
		if (readonly || placingItemId) return;

		selectedItem = item;
		selectedDayId = sortedDays[0]?.id ?? '';
		selectedStartTime = sortedDays[0]?.startTime.slice(0, 5) ?? '08:30';
		dialogError = '';
		dialogOpen = true;
	}

	function handleDragStart(event: DragEvent, item: ExamScheduleItem) {
		if (readonly || placingItemId || !event.dataTransfer) return;

		event.dataTransfer.effectAllowed = 'move';
		event.dataTransfer.setData(
			'application/x-exam-schedule-item',
			JSON.stringify({
				examScheduleItemId: item.id,
				classroomId: item.classroomId,
				gradeLevelId: item.gradeLevelId,
				durationMinutes: item.durationMinutes
			})
		);
	}

	async function submitPlacement() {
		if (!selectedItem || !selectedDay) return;

		const validation = validateExamSessionPlacement({
			day: selectedDay,
			candidate: {
				examScheduleItemId: selectedItem.id,
				classroomId: selectedItem.classroomId,
				gradeLevelId: selectedItem.gradeLevelId,
				startTime: selectedStartTime,
				durationMinutes: selectedItem.durationMinutes
			},
			scheduledSessions
		});
		if (!validation.ok) {
			dialogError = validation.reason ?? 'วางรายการสอบในช่วงเวลานี้ไม่ได้';
			return;
		}

		const placed = await onPlaceSession?.({
			examScheduleItemId: selectedItem.id,
			examDayId: selectedDay.id,
			startsAt: selectedStartTime
		});
		if (placed) dialogOpen = false;
	}

	$effect(() => {
		if (!selectedDayId && sortedDays[0]) {
			selectedDayId = sortedDays[0].id;
			selectedStartTime = sortedDays[0].startTime.slice(0, 5);
		}
	});
</script>

<section class="flex min-h-0 flex-col border-b bg-background lg:border-b-0 lg:border-r">
	<div class="border-b px-4 py-3">
		<div class="flex items-center justify-between gap-3">
			<div>
				<h2 class="text-sm font-semibold">ยังไม่จัดเวลา</h2>
				<p class="text-xs text-muted-foreground">{unscheduledItems.length} รายการจาก in_timetable</p>
			</div>
			<Badge variant="outline">{unscheduledItems.length}</Badge>
		</div>
	</div>

	{#if unscheduledItems.length === 0}
		<PageState title="ไม่มีรายการค้างจัด" description="รายการสอบทั้งหมดถูกวางบนไทม์ไลน์แล้ว" />
	{:else}
		<div class="max-h-[36rem] space-y-2 overflow-y-auto p-3">
			{#each unscheduledItems as item (item.id)}
				<div
					class="rounded-md border bg-card p-3 shadow-sm"
					class:opacity-60={placingItemId === item.id}
					role="listitem"
					draggable={!readonly && !placingItemId}
					ondragstart={(event) => handleDragStart(event, item)}
				>
					<div class="flex items-start gap-2">
						<GripVertical class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
						<div class="min-w-0 flex-1">
							<div class="truncate text-sm font-medium">{itemSubject(item)}</div>
							<div class="truncate text-xs text-muted-foreground">
								{item.classroomName ?? '-'} · {item.assessmentCategoryName ?? '-'}
							</div>
							<div class="mt-2 flex items-center justify-between gap-2">
								<Badge variant="secondary">{item.durationMinutes} นาที</Badge>
								{#if !readonly}
									<Button
										variant="outline"
										size="sm"
										onclick={() => openDialog(item)}
										disabled={!!placingItemId}
									>
										<CalendarPlus class="h-4 w-4" />
										จัดเวลา
									</Button>
								{/if}
							</div>
						</div>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</section>

<Dialog.Root bind:open={dialogOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>จัดเวลารายการสอบ</Dialog.Title>
			<Dialog.Description>
				{selectedItem ? `${itemSubject(selectedItem)} · ${selectedItem.classroomName ?? '-'}` : ''}
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4 py-2">
			{#if dialogError}
				<div class="rounded bg-destructive/10 p-3 text-sm text-destructive">{dialogError}</div>
			{/if}

			<div class="space-y-2">
				<Label>วันสอบ</Label>
				<Select.Root type="single" bind:value={selectedDayId}>
					<Select.Trigger class="w-full">{dayLabel(selectedDay)}</Select.Trigger>
					<Select.Content>
						{#each sortedDays as day (day.id)}
							<Select.Item value={day.id}>{dayLabel(day)}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-2">
					<Label for="exam-item-start-time">เริ่ม</Label>
					<Input id="exam-item-start-time" type="time" step="900" bind:value={selectedStartTime} />
				</div>
				<div class="space-y-2">
					<Label>สิ้นสุด</Label>
					<div class="rounded-md border bg-muted px-3 py-2 font-mono text-sm">
						{selectedItem ? addMinutes(selectedStartTime, selectedItem.durationMinutes) : '-'}
					</div>
				</div>
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (dialogOpen = false)}>ยกเลิก</Button>
			<LoadingButton
				loading={placingItemId === selectedItem?.id}
				loadingLabel="กำลังบันทึก..."
				onclick={submitPlacement}
				disabled={!selectedItem || !selectedDay || readonly || !!placingItemId}
			>
				บันทึก
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
