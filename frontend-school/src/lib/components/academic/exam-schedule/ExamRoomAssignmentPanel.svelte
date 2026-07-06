<script lang="ts">
	import type { Classroom } from '$lib/api/academic';
	import type { Room } from '$lib/api/facility';
	import type {
		ExamDayDetail,
		ExamDayRoomAssignmentView,
		UpsertDayRoomAssignmentInput
	} from '$lib/api/examSchedule';
	import { LoadingButton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Sheet from '$lib/components/ui/sheet';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import { Armchair, Plus } from 'lucide-svelte';

	let {
		days = [],
		classrooms = [],
		rooms = [],
		readonly = false,
		saving = false,
		generatingAssignmentId = null,
		onSaveAssignment,
		onGenerateSeats
	}: {
		days: ExamDayDetail[];
		classrooms: Classroom[];
		rooms: Room[];
		readonly?: boolean;
		saving?: boolean;
		generatingAssignmentId?: string | null;
		onSaveAssignment?: (
			examDayId: string,
			input: UpsertDayRoomAssignmentInput
		) => Promise<boolean> | boolean;
		onGenerateSeats?: (assignmentId: string) => Promise<void> | void;
	} = $props();

	let selectedDayId = $state('');
	let classroomId = $state('');
	let roomId = $state('');
	let capacityOverride = $state('');
	let editorOpen = $state(false);
	let editingAssignmentId = $state<string | null>(null);

	const sortedDays = $derived([...days].sort((a, b) => a.sortOrder - b.sortOrder));
	const selectedDay = $derived(days.find((day) => day.id === selectedDayId) ?? sortedDays[0] ?? null);
	const assignments = $derived<ExamDayRoomAssignmentView[]>(selectedDay?.roomAssignments ?? []);
	const dayLabel = $derived(selectedDay ? formatDayDate(selectedDay.examDate, selectedDay.label) : 'เลือกวันสอบ');
	const filteredClassrooms = $derived(
		selectedDay
			? classrooms.filter(
					(classroom) =>
						selectedDay.gradeLevelIds.length === 0 ||
						selectedDay.gradeLevelIds.includes(classroom.grade_level_id)
				)
			: classrooms
	);
	const usedClassroomIds = $derived(
		new Set(
			assignments
				.filter((assignment) => assignment.id !== editingAssignmentId)
				.map((assignment) => assignment.classroomId)
		)
	);
	const usedRoomIds = $derived(
		new Set(
			assignments
				.filter((assignment) => assignment.id !== editingAssignmentId)
				.map((assignment) => assignment.roomId)
		)
	);
	const availableClassrooms = $derived(
		filteredClassrooms.filter(
			(classroom) => !usedClassroomIds.has(classroom.id) || classroom.id === classroomId
		)
	);
	const availableRooms = $derived(
		rooms.filter((room) => !usedRoomIds.has(room.id) || room.id === roomId)
	);
	const selectedClassroomLabel = $derived(
		classroomLabel(classrooms.find((classroom) => classroom.id === classroomId))
	);
	const selectedRoomLabel = $derived(roomOptionLabel(rooms.find((room) => room.id === roomId)));

	function formatDayDate(value: string, label?: string | null): string {
		const dateLabel = new Date(value).toLocaleDateString('th-TH', {
			weekday: 'short',
			month: 'short',
			day: 'numeric'
		});
		return label ? `${label} · ${dateLabel}` : dateLabel;
	}

	function classroomLabel(classroom: Classroom | undefined): string {
		return classroom?.name ?? 'เลือกห้องเรียน';
	}

	function roomOptionLabel(room: Room | undefined): string {
		if (!room) return 'เลือกห้องสอบ';
		const building = room.building_name || 'ไม่ระบุอาคาร';
		const name = room.name_th || room.name_en || 'ไม่ระบุห้อง';
		const capacity = room.capacity ?? 0;
		return `${building} / ${name} / ${capacity} ที่นั่ง`;
	}

	function assignmentCapacity(assignment: ExamDayRoomAssignmentView): string {
		return String(assignment.capacityOverride ?? assignment.roomCapacity ?? '-');
	}

	function assignmentRoomLabel(assignment: ExamDayRoomAssignmentView): string {
		const room = rooms.find((item) => item.id === assignment.roomId);
		if (room) return roomOptionLabel(room);

		const name = assignment.roomName ?? '-';
		const capacity = assignment.roomCapacity ?? 0;
		return `${name} / ${capacity} ที่นั่ง`;
	}

	function assignmentRoomMeta(assignment: ExamDayRoomAssignmentView): string {
		return assignment.roomCapacity ? `${assignment.roomCapacity} ที่นั่งตามทะเบียนห้อง` : 'ไม่ระบุความจุหลัก';
	}

	function selectRoom(value: string) {
		roomId = value;
	}

	function loadAssignment(assignment: ExamDayRoomAssignmentView) {
		selectedDayId = assignment.examDayId;
		classroomId = assignment.classroomId;
		roomId = assignment.roomId;
		capacityOverride = assignment.capacityOverride ? String(assignment.capacityOverride) : '';
		editingAssignmentId = assignment.id;
		editorOpen = true;
	}

	function resetForm() {
		classroomId = '';
		roomId = '';
		capacityOverride = '';
		editingAssignmentId = null;
	}

	async function submitForm() {
		const dayId = selectedDay?.id ?? selectedDayId;
		if (!dayId || !classroomId || !roomId) return;

		const saved = await onSaveAssignment?.(dayId, {
			classroomId,
			roomId,
			capacityOverride: capacityOverride ? Number(capacityOverride) : null
		});
		if (saved) {
			resetForm();
			editorOpen = false;
		}
	}

	$effect(() => {
		if (!selectedDayId && sortedDays[0]) {
			selectedDayId = sortedDays[0].id;
		}
		if (selectedDayId && !days.some((day) => day.id === selectedDayId)) {
			selectedDayId = sortedDays[0]?.id ?? '';
			resetForm();
		}
	});
</script>

<section class="overflow-hidden rounded-md border bg-background">
	<div class="flex flex-col gap-3 border-b px-4 py-4 lg:flex-row lg:items-center lg:justify-between">
		<div>
			<h2 class="font-semibold">ห้องสอบและที่นั่ง</h2>
			<p class="text-sm text-muted-foreground">{assignments.length} ห้องในวันที่เลือก</p>
		</div>
		<div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-end">
			<Select.Root type="single" bind:value={selectedDayId}>
				<Select.Trigger class="w-full sm:w-64">{dayLabel}</Select.Trigger>
				<Select.Content>
					{#each sortedDays as day (day.id)}
						<Select.Item value={day.id}>{formatDayDate(day.examDate, day.label)}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
			{#if !readonly}
				<Button
					type="button"
					size="sm"
					disabled={!selectedDay}
					onclick={() => {
						resetForm();
						editorOpen = true;
					}}
				>
					<Plus class="h-4 w-4" />
					เพิ่มห้องสอบ
				</Button>
			{/if}
		</div>
	</div>

	<div class="min-w-0">
		{#if !selectedDay}
			<PageState title="ยังไม่มีวันสอบ" description="ต้องมีวันสอบก่อนกำหนดห้องสอบ" />
		{:else if assignments.length === 0}
			<PageState title="ยังไม่มีห้องสอบในวันนี้" description="ยังไม่พบการกำหนดห้องสอบสำหรับวันสอบที่เลือก" />
		{:else}
			<div class="overflow-x-auto">
				<Table class="min-w-[640px]">
					<TableHeader>
						<TableRow>
							<TableHead>ห้องเรียน</TableHead>
							<TableHead>ห้องสอบ</TableHead>
							<TableHead class="w-24 text-center">ความจุ</TableHead>
							<TableHead class="w-36 text-right">ที่นั่ง</TableHead>
						</TableRow>
					</TableHeader>
					<TableBody>
						{#each assignments as assignment (assignment.id)}
							<TableRow>
								<TableCell class="font-medium">{assignment.classroomName ?? '-'}</TableCell>
								<TableCell>
									<div class="font-medium">{assignmentRoomLabel(assignment)}</div>
									<div class="text-xs text-muted-foreground">{assignmentRoomMeta(assignment)}</div>
								</TableCell>
								<TableCell class="text-center">
									<Badge variant="outline">{assignmentCapacity(assignment)}</Badge>
								</TableCell>
								<TableCell class="text-right">
									<div class="flex justify-end gap-1">
										{#if !readonly}
											<Button variant="outline" size="sm" onclick={() => loadAssignment(assignment)}>
												แก้ไข
											</Button>
											<LoadingButton
												variant="outline"
												size="icon-sm"
												loading={generatingAssignmentId === assignment.id}
												loadingLabel=""
												onclick={() => onGenerateSeats?.(assignment.id)}
												aria-label="สร้างเลขที่นั่ง"
											>
												<Armchair class="h-4 w-4" />
											</LoadingButton>
										{:else}
											<Badge variant="outline">อ่านอย่างเดียว</Badge>
										{/if}
									</div>
								</TableCell>
							</TableRow>
						{/each}
					</TableBody>
				</Table>
			</div>
		{/if}
	</div>

	{#if !readonly}
		<Sheet.Root bind:open={editorOpen}>
			<Sheet.Content side="right" class="sm:max-w-md">
				<Sheet.Header>
					<Sheet.Title>{editingAssignmentId ? 'แก้ไขห้องสอบ' : 'เพิ่มห้องสอบ'}</Sheet.Title>
					<Sheet.Description>กำหนดห้องสอบและความจุสำหรับห้องเรียนในวันที่เลือก</Sheet.Description>
				</Sheet.Header>

				<form
					class="flex min-h-0 flex-1 flex-col"
					onsubmit={(event) => {
						event.preventDefault();
						submitForm();
					}}
				>
					<div class="flex-1 space-y-4 overflow-y-auto py-1 pr-1">
						<div class="grid gap-2">
							<Label>ห้องเรียน</Label>
							<Select.Root type="single" bind:value={classroomId}>
								<Select.Trigger class="w-full">{selectedClassroomLabel}</Select.Trigger>
								<Select.Content>
									{#each availableClassrooms as classroom (classroom.id)}
										<Select.Item value={classroom.id}>{classroomLabel(classroom)}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>

						<div class="grid gap-2">
							<Label>ห้องสอบ</Label>
							<Select.Root type="single" value={roomId} onValueChange={(value) => value && selectRoom(value)}>
								<Select.Trigger class="w-full">{selectedRoomLabel}</Select.Trigger>
								<Select.Content>
									{#each availableRooms as room (room.id)}
										<Select.Item value={room.id}>{roomOptionLabel(room)}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>

						<div class="grid gap-2">
							<Label for="exam-room-capacity">ความจุใช้สอบ</Label>
							<Input
								id="exam-room-capacity"
								type="number"
								min="1"
								step="1"
								bind:value={capacityOverride}
								placeholder="ตามห้องหลัก"
							/>
						</div>
					</div>

					<Sheet.Footer class="sticky bottom-0 -mx-6 -mb-6 border-t bg-background px-6 py-4">
						<Button type="button" variant="outline" onclick={() => (editorOpen = false)}>ยกเลิก</Button>
						<LoadingButton
							type="submit"
							loading={saving}
							loadingLabel="กำลังบันทึก..."
							disabled={!selectedDay || !classroomId || !roomId}
						>
							บันทึกห้องสอบ
						</LoadingButton>
					</Sheet.Footer>
				</form>
			</Sheet.Content>
		</Sheet.Root>
	{/if}
</section>
