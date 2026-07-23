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
	import { compareExamDaysByDate } from '$lib/utils/examScheduleDayOrder';
	import { addMinutes, validateExamSessionPlacement } from '$lib/utils/examScheduleTime';
	import { CalendarPlus, GripVertical } from 'lucide-svelte';
	import { SvelteMap } from 'svelte/reactivity';

	const ALL_FILTER_VALUE = '__all__';

	type DragPayload = {
		examScheduleItemId: string;
		classroomId: string;
		gradeLevelId: string;
		durationMinutes: number;
		sourceSessionId?: string;
	};

	type FilterOption = {
		value: string;
		label: string;
		sortValue: number;
	};

	let {
		unscheduledItems = [],
		days = [],
		scheduledSessions = [],
		readonly = false,
		placingItemIds = [],
		unschedulingSessionIds = [],
		onPlaceSession,
		onUnscheduleSession,
		onDragStart,
		onDragEnd
	}: {
		unscheduledItems: ExamScheduleItem[];
		days: ExamDayDetail[];
		scheduledSessions?: ExamSession[];
		readonly?: boolean;
		placingItemIds?: string[];
		unschedulingSessionIds?: string[];
		onPlaceSession?: (input: PlaceExamSessionInput) => Promise<boolean> | boolean;
		onUnscheduleSession?: (sessionId: string) => Promise<boolean> | boolean;
		onDragStart?: (payload: DragPayload) => void;
		onDragEnd?: () => void;
	} = $props();

	let dialogOpen = $state(false);
	let selectedItem = $state<ExamScheduleItem | null>(null);
	let selectedDayId = $state('');
	let selectedStartTime = $state('08:30');
	let dialogError = $state('');
	let subjectGroupFilter = $state(ALL_FILTER_VALUE);
	let gradeLevelFilter = $state(ALL_FILTER_VALUE);

	const sortedDays = $derived([...days].sort(compareExamDaysByDate));
	const selectedDay = $derived(
		days.find((day) => day.id === selectedDayId) ?? sortedDays[0] ?? null
	);
	const placingItemIdSet = $derived(new Set(placingItemIds));
	const unschedulingSessionIdSet = $derived(new Set(unschedulingSessionIds));
	const selectedItemPlacing = $derived(selectedItem ? placingItemIdSet.has(selectedItem.id) : false);
	const subjectGroupOptions = $derived.by(() => {
		const options = new SvelteMap<string, FilterOption>();
		for (const item of unscheduledItems) {
			if (!item.subjectGroupId) continue;
			options.set(item.subjectGroupId, {
				value: item.subjectGroupId,
				label: item.subjectGroupName ?? 'ไม่ระบุกลุ่มสาระ',
				sortValue: item.subjectGroupDisplayOrder ?? Number.MAX_SAFE_INTEGER
			});
		}
		return [...options.values()].sort(
			(left, right) => left.sortValue - right.sortValue || textCompare(left.label, right.label)
		);
	});
	const gradeLevelOptions = $derived.by(() => {
		const options = new SvelteMap<string, FilterOption>();
		for (const item of unscheduledItems) {
			options.set(item.gradeLevelId, {
				value: item.gradeLevelId,
				label: item.gradeLevelName ?? 'ไม่ระบุชั้น',
				sortValue: gradeLevelSortValue(item)
			});
		}
		return [...options.values()].sort(
			(left, right) => left.sortValue - right.sortValue || textCompare(left.label, right.label)
		);
	});
	const filteredSortedItems = $derived.by(() =>
		unscheduledItems
			.filter((item) => {
				const subjectGroupMatches =
					subjectGroupFilter === ALL_FILTER_VALUE || item.subjectGroupId === subjectGroupFilter;
				const gradeLevelMatches =
					gradeLevelFilter === ALL_FILTER_VALUE || item.gradeLevelId === gradeLevelFilter;
				return subjectGroupMatches && gradeLevelMatches;
			})
			.sort(compareExamScheduleItems)
	);

	function itemSubject(item: ExamScheduleItem): string {
		return item.subjectNameTh || item.subjectNameEn || item.subjectCode || 'ไม่ระบุวิชา';
	}

	function subjectTypeLabel(type: string | null | undefined): string {
		const normalized = type?.toUpperCase();
		if (normalized === 'BASIC') return 'พื้นฐาน';
		if (normalized === 'ADDITIONAL') return 'เพิ่มเติม';
		if (normalized === 'ACTIVITY') return 'กิจกรรม';
		return 'ไม่ระบุประเภท';
	}

	function subjectGroupFilterLabel(): string {
		if (subjectGroupFilter === ALL_FILTER_VALUE) return 'ทุกกลุ่มสาระ';
		return (
			subjectGroupOptions.find((option) => option.value === subjectGroupFilter)?.label ??
			'กลุ่มสาระ'
		);
	}

	function gradeLevelFilterLabel(): string {
		if (gradeLevelFilter === ALL_FILTER_VALUE) return 'ทุกชั้น';
		return gradeLevelOptions.find((option) => option.value === gradeLevelFilter)?.label ?? 'ชั้น';
	}

	function textCompare(left: string | null | undefined, right: string | null | undefined): number {
		return (left ?? '').localeCompare(right ?? '', 'th-TH', {
			numeric: true,
			sensitivity: 'base'
		});
	}

	function gradeLevelSortValue(item: ExamScheduleItem): number {
		const typeRank =
			item.gradeLevelType === 'kindergarten'
				? 1
				: item.gradeLevelType === 'primary'
					? 2
					: item.gradeLevelType === 'secondary'
						? 3
						: 9;
		return typeRank * 100 + (item.gradeLevelYear ?? 99);
	}

	function subjectTypeSortValue(type: string | null | undefined): number {
		const normalized = type?.toUpperCase();
		if (normalized === 'BASIC') return 1;
		if (normalized === 'ADDITIONAL') return 2;
		if (normalized === 'ACTIVITY') return 3;
		return 9;
	}

	function compareExamScheduleItems(left: ExamScheduleItem, right: ExamScheduleItem): number {
		return (
			(left.subjectGroupDisplayOrder ?? Number.MAX_SAFE_INTEGER) -
				(right.subjectGroupDisplayOrder ?? Number.MAX_SAFE_INTEGER) ||
			textCompare(left.subjectGroupName, right.subjectGroupName) ||
			gradeLevelSortValue(left) - gradeLevelSortValue(right) ||
			subjectTypeSortValue(left.subjectType) - subjectTypeSortValue(right.subjectType) ||
			textCompare(itemSubject(left), itemSubject(right)) ||
			textCompare(left.classroomName, right.classroomName) ||
			textCompare(left.subjectCode, right.subjectCode) ||
			textCompare(left.id, right.id)
		);
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
		if (readonly || placingItemIdSet.has(item.id)) return;

		selectedItem = item;
		selectedDayId = sortedDays[0]?.id ?? '';
		selectedStartTime = sortedDays[0]?.startTime.slice(0, 5) ?? '08:30';
		dialogError = '';
		dialogOpen = true;
	}

	function handleDragStart(event: DragEvent, item: ExamScheduleItem) {
		if (readonly || placingItemIdSet.has(item.id) || !event.dataTransfer) return;

		const payload = {
			examScheduleItemId: item.id,
			classroomId: item.classroomId,
			gradeLevelId: item.gradeLevelId,
			durationMinutes: item.durationMinutes
		};
		onDragStart?.(payload);

		event.dataTransfer.effectAllowed = 'move';
		event.dataTransfer.setData('application/x-exam-schedule-item', JSON.stringify(payload));
	}

	function dragPayload(event: DragEvent): DragPayload | null {
		const payload = event.dataTransfer?.getData('application/x-exam-schedule-item');
		if (!payload) return null;

		try {
			return JSON.parse(payload) as DragPayload;
		} catch {
			return null;
		}
	}

	function handleDragOver(event: DragEvent) {
		if (readonly) return;

		const payload = dragPayload(event);
		if (!payload?.sourceSessionId || unschedulingSessionIdSet.has(payload.sourceSessionId)) return;

		event.preventDefault();
		if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
	}

	async function handleDrop(event: DragEvent) {
		if (readonly) return;

		event.preventDefault();
		const payload = dragPayload(event);
		if (!payload?.sourceSessionId || unschedulingSessionIdSet.has(payload.sourceSessionId)) return;

		await onUnscheduleSession?.(payload.sourceSessionId);
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

	$effect(() => {
		if (
			subjectGroupFilter !== ALL_FILTER_VALUE &&
			!subjectGroupOptions.some((option) => option.value === subjectGroupFilter)
		) {
			subjectGroupFilter = ALL_FILTER_VALUE;
		}
		if (
			gradeLevelFilter !== ALL_FILTER_VALUE &&
			!gradeLevelOptions.some((option) => option.value === gradeLevelFilter)
		) {
			gradeLevelFilter = ALL_FILTER_VALUE;
		}
	});
</script>

<section
	class="flex h-full min-h-0 flex-col border-b bg-background lg:border-b-0 lg:border-r"
	role="group"
	aria-label="ถาดรายการสอบที่ยังไม่จัดเวลา"
	ondragover={handleDragOver}
	ondrop={handleDrop}
>
	<div class="border-b px-3 py-2">
		<div class="space-y-2">
			<div class="flex items-start justify-between gap-3">
				<div class="min-w-0">
					<h2 class="text-sm font-semibold">ยังไม่จัดเวลา</h2>
					<p class="text-xs text-muted-foreground">
						{filteredSortedItems.length}/{unscheduledItems.length} รายการจาก in_timetable
					</p>
					{#if !readonly}
						<p class="mt-0.5 text-xs text-muted-foreground">
							ลากรายการที่จัดแล้วกลับมาที่นี่เพื่อเอาออกจากตาราง
						</p>
					{/if}
				</div>
				<Badge variant="outline">{filteredSortedItems.length}</Badge>
			</div>

			<div class="grid grid-cols-2 gap-2">
				<Select.Root type="single" bind:value={subjectGroupFilter}>
					<Select.Trigger class="h-8 w-full text-xs">{subjectGroupFilterLabel()}</Select.Trigger>
					<Select.Content>
						<Select.Item value={ALL_FILTER_VALUE}>ทุกกลุ่มสาระ</Select.Item>
						{#each subjectGroupOptions as option (option.value)}
							<Select.Item value={option.value}>{option.label}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				<Select.Root type="single" bind:value={gradeLevelFilter}>
					<Select.Trigger class="h-8 w-full text-xs">{gradeLevelFilterLabel()}</Select.Trigger>
					<Select.Content>
						<Select.Item value={ALL_FILTER_VALUE}>ทุกชั้น</Select.Item>
						{#each gradeLevelOptions as option (option.value)}
							<Select.Item value={option.value}>{option.label}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
		</div>
	</div>

	{#if unscheduledItems.length === 0}
		<PageState title="ไม่มีรายการค้างจัด" description="รายการสอบทั้งหมดถูกวางบนไทม์ไลน์แล้ว" />
	{:else if filteredSortedItems.length === 0}
		<PageState title="ไม่พบรายการตามตัวกรอง" description="ลองเปลี่ยนกลุ่มสาระหรือชั้นที่เลือก" />
	{:else}
		<div class="min-h-0 flex-1 space-y-2 overflow-y-auto p-3">
			{#each filteredSortedItems as item (item.id)}
				<div
					class="cursor-grab rounded-md border bg-card p-3 shadow-sm active:cursor-grabbing"
					class:opacity-60={placingItemIdSet.has(item.id)}
					role="listitem"
					draggable={!readonly && !placingItemIdSet.has(item.id)}
					ondragstart={(event) => handleDragStart(event, item)}
					ondragend={onDragEnd}
				>
					<div class="flex items-start gap-2">
						<GripVertical class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
						<div class="min-w-0 flex-1">
							<div class="truncate text-sm font-medium">{itemSubject(item)}</div>
							<div class="truncate text-xs text-muted-foreground">
								{item.classroomName ?? '-'} · {item.assessmentCategoryName ?? '-'}
							</div>
							<div class="truncate text-xs text-muted-foreground">
								{item.subjectGroupName ?? 'ไม่ระบุกลุ่มสาระ'} · {item.gradeLevelName ?? '-'} ·
								{subjectTypeLabel(item.subjectType)}
							</div>
							<div class="mt-2 flex items-center justify-between gap-2">
								<Badge variant="secondary">{item.durationMinutes} นาที</Badge>
								{#if !readonly}
									<Button
										variant="outline"
										size="sm"
										onclick={() => openDialog(item)}
										disabled={placingItemIdSet.has(item.id)}
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
					<Input id="exam-item-start-time" type="time" step="300" bind:value={selectedStartTime} />
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
				loading={selectedItemPlacing}
				loadingLabel="กำลังบันทึก..."
				onclick={submitPlacement}
				disabled={!selectedItem || !selectedDay || readonly || selectedItemPlacing}
			>
				บันทึก
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
