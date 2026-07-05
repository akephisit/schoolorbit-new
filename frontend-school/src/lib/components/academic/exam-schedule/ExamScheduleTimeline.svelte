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
		clientXToTimelineStartTime,
		minutesBetween,
		timeToMinutes,
		validateExamSessionPlacement
	} from '$lib/utils/examScheduleTime';
	import ExamItemTray from './ExamItemTray.svelte';
	import ExamSessionBlock from './ExamSessionBlock.svelte';

	const SLOT_WIDTH = 24;

	type DragPayload = {
		examScheduleItemId: string;
		classroomId: string;
		gradeLevelId: string;
		durationMinutes: number;
		sourceSessionId?: string;
		dragOffsetPx?: number;
	};

	let {
		workspace,
		readonly = false,
		placingItemId = null,
		onPlaceSession
	}: {
		workspace: ExamScheduleWorkspace;
		readonly?: boolean;
		placingItemId?: string | null;
		onPlaceSession?: (input: PlaceExamSessionInput) => Promise<boolean> | boolean;
	} = $props();

	let localError = $state('');
	let dialogOpen = $state(false);
	let selectedSession = $state<ExamSession | null>(null);
	let selectedDayId = $state('');
	let selectedStartTime = $state('08:30');
	let dialogError = $state('');

	const sortedDays = $derived([...workspace.days].sort((a, b) => a.sortOrder - b.sortOrder));
	const placementDisabled = $derived(readonly || !!placingItemId);
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

	function trackWidth(day: ExamDayDetail): number {
		return timeSlots(day).length * SLOT_WIDTH;
	}

	function leftPx(day: ExamDayDetail, startsAt: string): number {
		return (minutesBetween(day.startTime, startsAt) / TIMELINE_SLOT_MINUTES) * SLOT_WIDTH;
	}

	function widthPx(durationMinutes: number): number {
		return Math.max(SLOT_WIDTH, (durationMinutes / TIMELINE_SLOT_MINUTES) * SLOT_WIDTH);
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
		return Math.max(SLOT_WIDTH, widthPx(minutesBetween(startTime, endTime)));
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

	function handleSessionDragStart(event: DragEvent, session: ExamSession, dragOffsetPx: number) {
		if (placementDisabled || !event.dataTransfer) return;

		event.dataTransfer.effectAllowed = 'move';
		event.dataTransfer.setData(
			'application/x-exam-schedule-item',
			JSON.stringify({
				examScheduleItemId: session.examScheduleItemId,
				classroomId: session.classroomId,
				gradeLevelId: session.gradeLevelId,
				durationMinutes: session.durationMinutes,
				sourceSessionId: session.id,
				dragOffsetPx
			})
		);
	}

	function handleDragOver(event: DragEvent) {
		if (placementDisabled) return;
		event.preventDefault();
		if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
	}

	async function handleDrop(
		event: DragEvent,
		day: ExamDayDetail,
		assignmentClassroomId: string
	) {
		if (placementDisabled) return;
		event.preventDefault();
		localError = '';

		const payload = dragPayload(event);
		if (!payload) return;

		if (payload.classroomId !== assignmentClassroomId) {
			localError = 'รายการสอบต้องวางในแถวห้องเรียนเดียวกัน';
			return;
		}

		const target = event.currentTarget as HTMLElement;
		const rect = target.getBoundingClientRect();
		const startsAt = clientXToTimelineStartTime({
			clientX: event.clientX,
			trackLeft: rect.left,
			dragOffsetPx: payload.dragOffsetPx ?? 0,
			dayStartTime: day.startTime,
			slotWidthPx: SLOT_WIDTH
		});
		await placeLocallyValidated(payload, day, startsAt);
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
		if (placementDisabled) return;

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
</script>

<section class="overflow-hidden rounded-md border bg-background">
	<div class="flex flex-col gap-2 border-b px-4 py-4 md:flex-row md:items-center md:justify-between">
		<div>
			<h2 class="font-semibold">ไทม์ไลน์ตารางสอบ</h2>
			<p class="text-sm text-muted-foreground">
				ยังไม่จัด {workspace.unscheduledItems.length} · จัดแล้ว {workspace.scheduledSessions.length}
			</p>
		</div>
		{#if localError}
			<div class="rounded bg-destructive/10 px-3 py-2 text-sm text-destructive">{localError}</div>
		{/if}
	</div>

	<div class="grid min-h-[32rem] lg:grid-cols-[20rem_minmax(0,1fr)]" style="--slot-width: 24px">
		<ExamItemTray
			unscheduledItems={workspace.unscheduledItems}
			days={sortedDays}
			scheduledSessions={workspace.scheduledSessions}
			readonly={placementDisabled}
			placingItemId={placingItemId}
			onPlaceSession={onPlaceSession}
		/>

		<div class="min-w-0 overflow-auto">
			{#if sortedDays.length === 0}
				<PageState title="ยังไม่มีวันสอบ" description="เพิ่มวันสอบในแท็บ Setup ก่อนจัดเวลา" />
			{:else if sortedDays.every((day) => day.roomAssignments.length === 0)}
				<PageState title="ยังไม่มีแถวห้องสอบ" description="กำหนดห้องสอบประจำวันในแท็บ Rooms ก่อนจัดเวลา" />
			{:else}
				<div class="space-y-6 p-4">
					{#each sortedDays as day (day.id)}
						<section class="min-w-fit">
							<div class="mb-2 flex items-center justify-between gap-3">
								<div>
									<h3 class="text-sm font-semibold">{dayLabel(day)}</h3>
									<p class="font-mono text-xs text-muted-foreground">
										{day.startTime.slice(0, 5)}-{day.endTime.slice(0, 5)}
									</p>
								</div>
								<Badge variant="outline">{day.roomAssignments.length} ห้องสอบ</Badge>
							</div>

							<div class="overflow-hidden rounded-md border">
								<div class="grid grid-cols-[12rem_minmax(0,1fr)] border-b bg-muted/40">
									<div class="border-r px-3 py-2 text-xs font-medium text-muted-foreground">
										ห้องเรียน / ห้องสอบ
									</div>
									<div class="overflow-hidden">
										<div
											class="grid h-9"
											style:grid-template-columns={`repeat(${timeSlots(day).length}, var(--slot-width))`}
											style:width={`${trackWidth(day)}px`}
										>
											{#each timeSlots(day) as slot, index (slot)}
												<div class="border-r px-1 py-2 font-mono text-[10px] text-muted-foreground">
													{index % 4 === 0 ? slot : ''}
												</div>
											{/each}
										</div>
									</div>
								</div>

								{#each day.roomAssignments as assignment (assignment.id)}
									<div class="grid grid-cols-[12rem_minmax(0,1fr)] border-b last:border-b-0">
										<div class="border-r px-3 py-3">
											<div class="truncate text-sm font-medium">
												{assignment.classroomName ?? assignment.classroomId}
											</div>
											<div class="truncate text-xs text-muted-foreground">
												{assignment.roomName ?? assignment.roomId}
											</div>
										</div>

										<div class="overflow-hidden">
											<div
												class="relative h-14 bg-background"
												style:width={`${trackWidth(day)}px`}
												role="group"
												aria-label={`วางรายการสอบ ${assignment.classroomName ?? assignment.classroomId}`}
												ondragover={handleDragOver}
												ondrop={(event) => handleDrop(event, day, assignment.classroomId)}
											>
												<div
													class="pointer-events-none absolute inset-0 grid"
													style:grid-template-columns={`repeat(${timeSlots(day).length}, var(--slot-width))`}
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

												{#each sessionsForAssignment(day, assignment.classroomId) as session (session.id)}
													<ExamSessionBlock
														{session}
														leftPx={leftPx(day, session.startsAt)}
														widthPx={widthPx(session.durationMinutes)}
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
				{selectedSession ? `${subjectLabel(selectedSession)} · ${selectedSession.classroomName ?? '-'}` : ''}
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
					<Input id="exam-session-start-time" type="time" step="900" bind:value={selectedStartTime} />
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
				loading={placingItemId === selectedSession?.examScheduleItemId}
				loadingLabel="กำลังบันทึก..."
				onclick={submitSessionDialog}
				disabled={!selectedSession || !selectedDay || placementDisabled}
			>
				บันทึก
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
