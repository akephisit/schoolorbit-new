<script lang="ts">
	import type {
		ExamDayDetail,
		ExamScheduleWorkspace,
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
	import {
		TIMELINE_SLOT_MINUTES,
		addMinutes,
		buildTimelineDragPreview,
		minutesBetween,
		timeToMinutes,
		validateExamSessionPlacement
	} from '$lib/utils/examScheduleTime';
	import { compareExamDaysByDate } from '$lib/utils/examScheduleDayOrder';
	import { Download, Trash2 } from 'lucide-svelte';
	import ExamItemTray from './ExamItemTray.svelte';
	import ExamSessionBlock from './ExamSessionBlock.svelte';

	const MIN_SLOT_WIDTH = 8;
	const ROOM_LABEL_COLUMN_WIDTH = 'minmax(7.5rem, 8.5rem)';
	const SCHEDULE_ROW_GRID_TEMPLATE = `${ROOM_LABEL_COLUMN_WIDTH} minmax(0, 1fr)`;
	const TIME_LABEL_INTERVAL_MINUTES = 60;

	type DragPayload = {
		examScheduleItemId: string;
		classroomId: string;
		gradeLevelId: string;
		durationMinutes: number;
		sourceSessionId?: string;
		dragOffsetPx?: number;
	};

	type DragPreviewState = {
		dayId: string;
		classroomId: string;
		leftPx: number;
		widthPx: number;
		startTime: string;
		endTime: string;
		valid: boolean;
		reason?: string;
	};

	let {
		workspace,
		readonly = false,
		placingItemIds = [],
		unschedulingSessionIds = [],
		canManageActions = false,
		importing = false,
		clearingMismatchedItems = false,
		examKindLabel = '',
		onPlaceSession,
		onUnscheduleSession,
		onImportItems,
		onClearMismatchedItems
	}: {
		workspace: ExamScheduleWorkspace;
		readonly?: boolean;
		placingItemIds?: string[];
		unschedulingSessionIds?: string[];
		canManageActions?: boolean;
		importing?: boolean;
		clearingMismatchedItems?: boolean;
		examKindLabel?: string;
		onPlaceSession?: (input: PlaceExamSessionInput) => Promise<boolean> | boolean;
		onUnscheduleSession?: (sessionId: string) => Promise<boolean> | boolean;
		onImportItems?: () => void;
		onClearMismatchedItems?: () => void;
	} = $props();

	let localError = $state('');
	let dialogOpen = $state(false);
	let selectedSession = $state<ExamSession | null>(null);
	let selectedDayId = $state('');
	let selectedStartTime = $state('08:30');
	let dialogError = $state('');
	let dragPreview = $state<DragPreviewState | null>(null);
	let activeDragPayload = $state<DragPayload | null>(null);
	let dayDisplayMode = $state<'all' | 'single'>('all');
	let selectedTimelineDayId = $state('');
	let dayTrackWidths = $state<Record<string, number>>({});

	const sortedDays = $derived([...workspace.days].sort(compareExamDaysByDate));
	const selectedTimelineDay = $derived(
		sortedDays.find((day) => day.id === selectedTimelineDayId) ?? sortedDays[0] ?? null
	);
	const selectedTimelineDayLabel = $derived(dayLabel(selectedTimelineDay));
	const visibleDays = $derived(
		dayDisplayMode === 'single'
			? sortedDays.filter((day) => day.id === selectedTimelineDay?.id)
			: sortedDays
	);
	const placementDisabled = $derived(readonly);
	const actionPlacementDisabled = $derived(
		placingItemIds.length > 0 || unschedulingSessionIds.length > 0
	);
	const placingItemIdSet = $derived(new Set(placingItemIds));
	const unschedulingSessionIdSet = $derived(new Set(unschedulingSessionIds));
	const placingSessionIds = $derived(
		new Set(
			workspace.scheduledSessions
				.filter((session) => placingItemIdSet.has(session.examScheduleItemId))
				.map((session) => session.id)
		)
	);
	const selectedSessionPlacing = $derived(
		selectedSession ? placingItemIdSet.has(selectedSession.examScheduleItemId) : false
	);
	const selectedSessionUnscheduling = $derived(
		selectedSession ? unschedulingSessionIdSet.has(selectedSession.id) : false
	);
	const selectedDay = $derived(
		workspace.days.find((day) => day.id === selectedDayId) ?? sortedDays[0] ?? null
	);

	function dayLabel(day: ExamDayDetail | null): string {
		if (!day) return 'เลือกวันสอบ';
		const date = new Date(day.examDate).toLocaleDateString('th-TH', {
			weekday: 'short',
			month: 'short',
			day: 'numeric'
		});
		return day.label ? `${day.label} · ${date}` : date;
	}

	function subjectLabel(session: ExamSession): string {
		return session.subjectNameTh || session.subjectNameEn || session.subjectCode || 'ไม่ระบุวิชา';
	}

	function timeSlots(day: ExamDayDetail): string[] {
		const start = timeToMinutes(day.startTime);
		const end = timeToMinutes(day.endTime);
		const slots: string[] = [];
		for (let minutes = start; minutes < end; minutes += TIMELINE_SLOT_MINUTES) {
			slots.push(minutesToHourLabel(minutes));
		}
		return slots;
	}

	function minutesToHourLabel(minutes: number): string {
		const hours = Math.floor(minutes / 60);
		const remainder = minutes % 60;
		return `${String(hours).padStart(2, '0')}:${String(remainder).padStart(2, '0')}`;
	}

	function shouldRenderTimeLabel(slot: string, index: number): boolean {
		return index === 0 || timeToMinutes(slot) % TIME_LABEL_INTERVAL_MINUTES === 0;
	}

	function daySlotCount(day: ExamDayDetail): number {
		return Math.max(1, timeSlots(day).length);
	}

	function minimumTrackWidth(day: ExamDayDetail): number {
		return daySlotCount(day) * MIN_SLOT_WIDTH;
	}

	function trackSlotWidth(day: ExamDayDetail): number;
	function trackSlotWidth(day: ExamDayDetail, measuredWidth: number): number;
	function trackSlotWidth(day: ExamDayDetail, measuredWidth = dayTrackWidths[day.id] ?? 0): number {
		return Math.max(
			MIN_SLOT_WIDTH,
			Math.max(measuredWidth, minimumTrackWidth(day)) / daySlotCount(day)
		);
	}

	function timelineGridTemplate(day: ExamDayDetail): string {
		return `repeat(${daySlotCount(day)}, minmax(${MIN_SLOT_WIDTH}px, 1fr))`;
	}

	function updateDayTrackWidth(dayId: string, width: number) {
		const roundedWidth = Math.round(width);
		if (dayTrackWidths[dayId] === roundedWidth) return;
		dayTrackWidths = { ...dayTrackWidths, [dayId]: roundedWidth };
	}

	function forgetDayTrackWidth(dayId: string) {
		if (!(dayId in dayTrackWidths)) return;
		const nextWidths = { ...dayTrackWidths };
		delete nextWidths[dayId];
		dayTrackWidths = nextWidths;
	}

	function observeDayTrack(node: HTMLElement, dayId: string) {
		function updateWidth() {
			updateDayTrackWidth(dayId, node.getBoundingClientRect().width);
		}

		updateWidth();
		const observer = new ResizeObserver(updateWidth);
		observer.observe(node);

		return {
			update(nextDayId: string) {
				if (nextDayId !== dayId) {
					forgetDayTrackWidth(dayId);
					dayId = nextDayId;
				}
				updateWidth();
			},
			destroy() {
				observer.disconnect();
				forgetDayTrackWidth(dayId);
			}
		};
	}

	function leftPx(day: ExamDayDetail, startsAt: string): number {
		return (minutesBetween(day.startTime, startsAt) / TIMELINE_SLOT_MINUTES) * trackSlotWidth(day);
	}

	function widthPx(day: ExamDayDetail, durationMinutes: number): number {
		return Math.max(
			trackSlotWidth(day),
			(durationMinutes / TIMELINE_SLOT_MINUTES) * trackSlotWidth(day)
		);
	}

	function sessionsForAssignment(day: ExamDayDetail, classroomId: string): ExamSession[] {
		return workspace.scheduledSessions
			.filter((session) => session.examDayId === day.id && session.classroomId === classroomId)
			.sort((a, b) => a.startsAt.localeCompare(b.startsAt));
	}

	function blockedLeft(day: ExamDayDetail, startTime: string): number {
		return Math.max(0, leftPx(day, startTime));
	}

	function blockedWidth(day: ExamDayDetail, startTime: string, endTime: string): number {
		return Math.max(trackSlotWidth(day), widthPx(day, minutesBetween(startTime, endTime)));
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

	function setActiveDragPayload(payload: DragPayload) {
		activeDragPayload = payload;
	}

	function currentDragPayload(event: DragEvent): DragPayload | null {
		return dragPayload(event) ?? activeDragPayload;
	}

	function payloadIsPending(payload: DragPayload): boolean {
		return (
			placingItemIdSet.has(payload.examScheduleItemId) ||
			(!!payload.sourceSessionId && unschedulingSessionIdSet.has(payload.sourceSessionId))
		);
	}

	function sessionIsPending(session: ExamSession): boolean {
		return (
			placingItemIdSet.has(session.examScheduleItemId) || unschedulingSessionIdSet.has(session.id)
		);
	}

	function handleSessionDragStart(event: DragEvent, session: ExamSession, dragOffsetPx: number) {
		if (placementDisabled || sessionIsPending(session) || !event.dataTransfer) return;

		const payload = {
			examScheduleItemId: session.examScheduleItemId,
			classroomId: session.classroomId,
			gradeLevelId: session.gradeLevelId,
			durationMinutes: session.durationMinutes,
			sourceSessionId: session.id,
			dragOffsetPx
		};
		setActiveDragPayload(payload);

		event.dataTransfer.effectAllowed = 'move';
		event.dataTransfer.setData('application/x-exam-schedule-item', JSON.stringify(payload));
	}

	function clearDragPreview() {
		dragPreview = null;
	}

	function clearActiveDrag() {
		activeDragPayload = null;
		clearDragPreview();
	}

	function clearActiveDragForPayload(payload: DragPayload) {
		if (
			activeDragPayload?.examScheduleItemId !== payload.examScheduleItemId ||
			activeDragPayload.sourceSessionId !== payload.sourceSessionId
		) {
			return;
		}

		clearActiveDrag();
	}

	function buildRowDragPreview(event: DragEvent, day: ExamDayDetail, payload: DragPayload) {
		const target = event.currentTarget as HTMLElement;
		const rect = target.getBoundingClientRect();

		return buildTimelineDragPreview({
			day,
			clientX: event.clientX,
			trackLeft: rect.left,
			dragOffsetPx: payload.dragOffsetPx ?? 0,
			slotWidthPx: trackSlotWidth(day, rect.width),
			durationMinutes: payload.durationMinutes,
			candidate: {
				examScheduleItemId: payload.examScheduleItemId,
				classroomId: payload.classroomId,
				gradeLevelId: payload.gradeLevelId,
				sourceSessionId: payload.sourceSessionId
			},
			scheduledSessions: workspace.scheduledSessions
		});
	}

	function handleDragOver(event: DragEvent, day: ExamDayDetail, assignmentClassroomId: string) {
		if (placementDisabled) return;
		const payload = currentDragPayload(event);
		if (!payload || payloadIsPending(payload)) return;

		event.preventDefault();

		if (payload.classroomId !== assignmentClassroomId) {
			if (event.dataTransfer) event.dataTransfer.dropEffect = 'none';
			clearDragPreview();
			return;
		}

		if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';

		const preview = buildRowDragPreview(event, day, payload);
		dragPreview = {
			dayId: day.id,
			classroomId: assignmentClassroomId,
			leftPx: preview.leftPx,
			widthPx: preview.widthPx,
			startTime: preview.startTime,
			endTime: preview.endTime,
			valid: preview.valid,
			reason: preview.reason
		};
	}

	function handleDragLeave(event: DragEvent, day: ExamDayDetail, assignmentClassroomId: string) {
		const currentTarget = event.currentTarget as HTMLElement;
		const relatedTarget = event.relatedTarget as Node | null;
		if (relatedTarget && currentTarget.contains(relatedTarget)) return;
		if (dragPreview?.dayId === day.id && dragPreview.classroomId === assignmentClassroomId) {
			clearDragPreview();
		}
	}

	async function handleDrop(event: DragEvent, day: ExamDayDetail, assignmentClassroomId: string) {
		if (placementDisabled) {
			clearActiveDrag();
			return;
		}
		event.preventDefault();
		localError = '';

		const payload = currentDragPayload(event);
		if (!payload || payloadIsPending(payload)) {
			clearActiveDrag();
			return;
		}

		if (payload.classroomId !== assignmentClassroomId) {
			localError = 'รายการสอบต้องวางในแถวห้องเรียนเดียวกัน';
			clearActiveDrag();
			return;
		}

		const activePreview =
			dragPreview?.dayId === day.id && dragPreview.classroomId === assignmentClassroomId
				? dragPreview
				: null;
		const startsAt = activePreview?.startTime ?? buildRowDragPreview(event, day, payload).startTime;
		const placement = placeLocallyValidated(payload, day, startsAt);
		clearActiveDragForPayload(payload);
		await placement;
	}

	async function placeLocallyValidated(payload: DragPayload, day: ExamDayDetail, startsAt: string) {
		const validation = validateDropPayload(payload, day, startsAt);
		if (!validation.ok) {
			localError = validation.reason ?? 'วางรายการสอบในช่วงเวลานี้ไม่ได้';
			return false;
		}

		return onPlaceSession?.({
			examScheduleItemId: payload.examScheduleItemId,
			examDayId: day.id,
			startsAt
		});
	}

	function validateDropPayload(payload: DragPayload, day: ExamDayDetail, startsAt: string) {
		return validateExamSessionPlacement({
			day,
			candidate: {
				examScheduleItemId: payload.examScheduleItemId,
				classroomId: payload.classroomId,
				gradeLevelId: payload.gradeLevelId,
				startTime: startsAt,
				durationMinutes: payload.durationMinutes,
				sourceSessionId: payload.sourceSessionId
			},
			scheduledSessions: workspace.scheduledSessions
		});
	}

	function openSessionDialog(session: ExamSession) {
		if (placementDisabled || sessionIsPending(session)) return;

		selectedSession = session;
		selectedDayId = session.examDayId;
		selectedStartTime = session.startsAt.slice(0, 5);
		dialogError = '';
		dialogOpen = true;
	}

	async function submitSessionDialog() {
		if (!selectedSession || !selectedDay) return;

		const placed = await placeLocallyValidated(
			{
				examScheduleItemId: selectedSession.examScheduleItemId,
				classroomId: selectedSession.classroomId,
				gradeLevelId: selectedSession.gradeLevelId,
				durationMinutes: selectedSession.durationMinutes,
				sourceSessionId: selectedSession.id
			},
			selectedDay,
			selectedStartTime
		);
		if (placed) dialogOpen = false;
		else dialogError = localError;
	}

	async function removeSelectedSession() {
		if (
			!selectedSession ||
			placementDisabled ||
			selectedSessionPlacing ||
			selectedSessionUnscheduling
		) {
			return;
		}

		const removed = await onUnscheduleSession?.(selectedSession.id);
		if (removed) dialogOpen = false;
	}

	$effect(() => {
		if (!selectedTimelineDayId && sortedDays[0]) {
			selectedTimelineDayId = sortedDays[0].id;
		}
		if (selectedTimelineDayId && !sortedDays.some((day) => day.id === selectedTimelineDayId)) {
			selectedTimelineDayId = sortedDays[0]?.id ?? '';
		}
	});
</script>

<section class="flex h-full min-h-0 flex-col overflow-hidden rounded-md border bg-background">
	<div
		class="flex flex-col gap-2 border-b px-3 py-2 xl:flex-row xl:items-center xl:justify-between"
	>
		<div>
			<h2 class="font-semibold">ไทม์ไลน์ตารางสอบ</h2>
			<p class="text-sm text-muted-foreground">
				ยังไม่จัด {workspace.unscheduledItems.length} · จัดแล้ว {workspace.scheduledSessions.length}
			</p>
		</div>
		<div class="flex flex-col gap-2 xl:items-end">
			<div class="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center sm:justify-end">
				<Select.Root type="single" bind:value={dayDisplayMode}>
					<Select.Trigger class="w-full sm:w-36">
						{dayDisplayMode === 'all' ? 'แสดงทุกวัน' : 'เฉพาะวัน'}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="all">แสดงทุกวัน</Select.Item>
						<Select.Item value="single">เฉพาะวัน</Select.Item>
					</Select.Content>
				</Select.Root>
				{#if dayDisplayMode === 'single'}
					<Select.Root type="single" bind:value={selectedTimelineDayId}>
						<Select.Trigger class="w-full sm:w-48">{selectedTimelineDayLabel}</Select.Trigger>
						<Select.Content>
							{#each sortedDays as day (day.id)}
								<Select.Item value={day.id}>{dayLabel(day)}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				{/if}
				{#if canManageActions}
					<LoadingButton
						size="sm"
						variant="outline"
						loading={importing}
						loadingLabel="กำลังนำเข้า..."
						onclick={onImportItems}
						disabled={clearingMismatchedItems || actionPlacementDisabled}
					>
						<Download class="h-4 w-4" />
						นำเข้าเฉพาะ {examKindLabel}
					</LoadingButton>
					<LoadingButton
						size="sm"
						variant="destructive"
						loading={clearingMismatchedItems}
						loadingLabel="กำลังล้าง..."
						onclick={onClearMismatchedItems}
						disabled={importing || actionPlacementDisabled}
					>
						<Trash2 class="h-4 w-4" />
						ล้างรายการไม่ตรงรอบสอบ
					</LoadingButton>
				{/if}
			</div>
			{#if localError}
				<div class="rounded bg-destructive/10 px-3 py-2 text-sm text-destructive">{localError}</div>
			{/if}
		</div>
	</div>

	<div class="grid min-h-0 flex-1 lg:grid-cols-[20rem_minmax(0,1fr)]">
		<ExamItemTray
			unscheduledItems={workspace.unscheduledItems}
			days={sortedDays}
			scheduledSessions={workspace.scheduledSessions}
			readonly={placementDisabled}
			{placingItemIds}
			{unschedulingSessionIds}
			{onPlaceSession}
			{onUnscheduleSession}
			onDragStart={setActiveDragPayload}
			onDragEnd={clearActiveDrag}
		/>

		<div class="min-h-0 min-w-0 overflow-auto">
			{#if sortedDays.length === 0}
				<PageState title="ยังไม่มีวันสอบ" description="เพิ่มวันสอบในแท็บ Setup ก่อนจัดเวลา" />
			{:else if visibleDays.every((day) => day.roomAssignments.length === 0)}
				<PageState
					title="ยังไม่มีแถวห้องสอบ"
					description="กำหนดห้องสอบประจำวันในแท็บ Rooms ก่อนจัดเวลา"
				/>
			{:else}
				<div class="space-y-6 p-3">
					{#each visibleDays as day (day.id)}
						<section class="min-w-0">
							<div class="mb-2 flex items-center justify-between gap-3">
								<div>
									<h3 class="text-sm font-semibold">{dayLabel(day)}</h3>
									<p class="font-mono text-xs text-muted-foreground">
										{day.startTime.slice(0, 5)}-{day.endTime.slice(0, 5)}
									</p>
								</div>
								<Badge variant="outline">{day.roomAssignments.length} ห้องสอบ</Badge>
							</div>

							<div class="overflow-x-auto rounded-md border">
								<div
									class="grid border-b bg-muted/40"
									style:grid-template-columns={SCHEDULE_ROW_GRID_TEMPLATE}
								>
									<div class="border-r px-2 py-2 text-xs font-medium text-muted-foreground">
										ห้องเรียน / ห้องสอบ
									</div>
									<div class="overflow-hidden">
										<div
											class="grid h-9 w-full"
											use:observeDayTrack={day.id}
											style:grid-template-columns={timelineGridTemplate(day)}
											style:min-width={`${minimumTrackWidth(day)}px`}
										>
											{#each timeSlots(day) as slot, index (slot)}
												<div class="border-r px-1 py-2 font-mono text-[10px] text-muted-foreground">
													{shouldRenderTimeLabel(slot, index) ? slot : ''}
												</div>
											{/each}
										</div>
									</div>
								</div>

								{#each day.roomAssignments as assignment (assignment.id)}
									<div
										class="grid border-b last:border-b-0"
										style:grid-template-columns={SCHEDULE_ROW_GRID_TEMPLATE}
									>
										<div class="border-r px-2 py-3">
											<div class="truncate text-sm font-medium">
												{assignment.classroomName ?? assignment.classroomId}
											</div>
											<div class="truncate text-xs text-muted-foreground">
												{assignment.roomName ?? assignment.roomId}
											</div>
										</div>

										<div class="overflow-hidden">
											<div
												class="relative h-14 w-full bg-background"
												style:min-width={`${minimumTrackWidth(day)}px`}
												role="group"
												aria-label={`วางรายการสอบ ${assignment.classroomName ?? assignment.classroomId}`}
												ondragover={(event) => handleDragOver(event, day, assignment.classroomId)}
												ondragleave={(event) => handleDragLeave(event, day, assignment.classroomId)}
												ondragend={clearActiveDrag}
												ondrop={(event) => handleDrop(event, day, assignment.classroomId)}
											>
												<div
													class="pointer-events-none absolute inset-0 grid"
													style:grid-template-columns={timelineGridTemplate(day)}
												>
													{#each timeSlots(day) as slot (slot)}
														<div class="border-r border-border/70"></div>
													{/each}
												</div>

												{#each day.blockedWindows as blockedWindow, index (`${blockedWindow.startTime}-${blockedWindow.endTime}-${index}`)}
													<div
														class="blocked-window pointer-events-none absolute top-0 h-full bg-muted/80"
														style:left={`${blockedLeft(day, blockedWindow.startTime)}px`}
														style:width={`${blockedWidth(day, blockedWindow.startTime, blockedWindow.endTime)}px`}
														title={blockedWindow.label ?? 'Unavailable'}
													>
														<div class="truncate px-2 py-1 text-[10px] text-muted-foreground">
															{blockedWindow.label ?? 'ปิด'}
														</div>
													</div>
												{/each}

												{#if dragPreview?.dayId === day.id && dragPreview.classroomId === assignment.classroomId}
													{@const preview = dragPreview}
													<div
														class={`pointer-events-none absolute top-1 rounded border-2 px-2 py-1 text-xs shadow-sm ${
															preview.valid
																? 'border-primary bg-primary/10 text-primary'
																: 'border-destructive bg-destructive/10 text-destructive'
														}`}
														style:left={`${preview.leftPx}px`}
														style:width={`${preview.widthPx}px`}
													>
														<div class="truncate font-mono">
															{preview.startTime}-{preview.endTime}
														</div>
														{#if preview.reason}
															<div class="truncate text-[10px]">{preview.reason}</div>
														{/if}
													</div>
												{/if}

												{#each sessionsForAssignment(day, assignment.classroomId) as session (session.id)}
													<ExamSessionBlock
														{session}
														leftPx={leftPx(day, session.startsAt)}
														widthPx={widthPx(day, session.durationMinutes)}
														placing={placingSessionIds.has(session.id)}
														removing={unschedulingSessionIdSet.has(session.id)}
														readonly={placementDisabled}
														onDragStart={handleSessionDragStart}
														onOpen={openSessionDialog}
													/>
												{/each}
											</div>
										</div>
									</div>
								{/each}
							</div>
						</section>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</section>

<Dialog.Root bind:open={dialogOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>ย้ายคาบสอบ</Dialog.Title>
			<Dialog.Description>
				{selectedSession
					? `${subjectLabel(selectedSession)} · ${selectedSession.classroomName ?? '-'}`
					: ''}
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
					<Label for="exam-session-start-time">เริ่ม</Label>
					<Input
						id="exam-session-start-time"
						type="time"
						step="300"
						bind:value={selectedStartTime}
					/>
				</div>
				<div class="space-y-2">
					<Label>สิ้นสุด</Label>
					<div class="rounded-md border bg-muted px-3 py-2 font-mono text-sm">
						{selectedSession ? addMinutes(selectedStartTime, selectedSession.durationMinutes) : '-'}
					</div>
				</div>
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (dialogOpen = false)}>ยกเลิก</Button>
			<LoadingButton
				variant="destructive"
				loading={selectedSessionUnscheduling}
				loadingLabel="กำลังเอาออก..."
				onclick={removeSelectedSession}
				disabled={!selectedSession ||
					placementDisabled ||
					selectedSessionPlacing ||
					selectedSessionUnscheduling}
			>
				เอาออกจากตาราง
			</LoadingButton>
			<LoadingButton
				loading={selectedSessionPlacing}
				loadingLabel="กำลังบันทึก..."
				onclick={submitSessionDialog}
				disabled={!selectedSession ||
					!selectedDay ||
					placementDisabled ||
					selectedSessionUnscheduling}
			>
				บันทึก
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
