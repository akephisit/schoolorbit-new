<script lang="ts">
	import type { Classroom } from '$lib/api/academic';
	import type { Room } from '$lib/api/facility';
	import type { StaffListItem } from '$lib/api/staff';
	import type {
		ExamDayDetail,
		ExamDayRoomAssignmentView,
		UpsertDayRoomAssignmentInput
	} from '$lib/api/examSchedule';
	import { LoadingButton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import { Armchair, Search, Users } from 'lucide-svelte';

	type InvigilatorOption = {
		id: string;
		displayName: string;
	};

	let {
		days = [],
		classrooms = [],
		rooms = [],
		staff = [],
		readonly = false,
		saving = false,
		generatingAssignmentId = null,
		onSaveAssignment,
		onGenerateSeats,
		onSearchStaff
	}: {
		days: ExamDayDetail[];
		classrooms: Classroom[];
		rooms: Room[];
		staff: StaffListItem[];
		readonly?: boolean;
		saving?: boolean;
		generatingAssignmentId?: string | null;
		onSaveAssignment?: (
			examDayId: string,
			input: UpsertDayRoomAssignmentInput
		) => Promise<boolean> | boolean;
		onGenerateSeats?: (assignmentId: string) => Promise<void> | void;
		onSearchStaff?: (search: string) => Promise<StaffListItem[]>;
	} = $props();

	let selectedDayId = $state('');
	let classroomId = $state('');
	let roomId = $state('');
	let capacityOverride = $state('');
	let selectedInvigilatorIds = $state<string[]>([]);
	let selectedInvigilatorOptions = $state<InvigilatorOption[]>([]);
	let staffOptions = $state<StaffListItem[]>([]);
	let staffSearch = $state('');
	let staffSearching = $state(false);
	let staffSearchError = $state('');
	let lastStaffSearch = '';
	let staffSearchRequestToken = 0;

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
	const selectedClassroomLabel = $derived(
		classrooms.find((classroom) => classroom.id === classroomId)?.name ?? 'เลือกห้องเรียน'
	);
	const selectedRoomLabel = $derived(roomLabel(rooms.find((room) => room.id === roomId)));
	const staffOptionsForDisplay = $derived.by(() => {
		const options = new Map<string, InvigilatorOption>();

		for (const option of selectedInvigilatorOptions) {
			if (selectedInvigilatorIds.includes(option.id)) {
				options.set(option.id, option);
			}
		}

		for (const item of staffOptions) {
			options.set(item.id, { id: item.id, displayName: staffName(item) });
		}

		return Array.from(options.values()).slice(0, 60);
	});

	function formatDayDate(value: string, label?: string | null): string {
		const dateLabel = new Date(value).toLocaleDateString('th-TH', {
			weekday: 'short',
			month: 'short',
			day: 'numeric'
		});
		return label ? `${label} · ${dateLabel}` : dateLabel;
	}

	function roomLabel(room: Room | undefined): string {
		if (!room) return 'เลือกห้องสอบ';
		const code = room.code ? `${room.code} · ` : '';
		const building = room.building_name ? `${room.building_name} / ` : '';
		return `${building}${code}${room.name_th} / ${room.capacity ?? 0} ที่นั่ง`;
	}

	function staffName(item: StaffListItem): string {
		return [item.title, item.first_name, item.last_name].filter(Boolean).join(' ').trim();
	}

	function staffOptionName(option: InvigilatorOption): string {
		return option.displayName || option.id;
	}

	function invigilatorNames(assignment: ExamDayRoomAssignmentView): string {
		if (assignment.invigilators.length === 0) return '-';
		return assignment.invigilators
			.map((item) => item.staffName ?? item.staffId)
			.filter(Boolean)
			.join(', ');
	}

	function assignmentCapacity(assignment: ExamDayRoomAssignmentView): string {
		return String(assignment.capacityOverride ?? assignment.roomCapacity ?? '-');
	}

	function assignmentRoomMeta(assignment: ExamDayRoomAssignmentView): string {
		return assignment.roomCapacity ? `${assignment.roomCapacity} ที่นั่งตามทะเบียนห้อง` : 'ไม่ระบุความจุหลัก';
	}

	function selectRoom(value: string) {
		roomId = value;
		const room = rooms.find((item) => item.id === value);
		if (room && !capacityOverride) {
			capacityOverride = String(room.capacity ?? '');
		}
	}

	function toggleInvigilator(option: InvigilatorOption, checked: boolean) {
		selectedInvigilatorIds = checked
			? Array.from(new Set([...selectedInvigilatorIds, option.id]))
			: selectedInvigilatorIds.filter((id) => id !== option.id);

		selectedInvigilatorOptions = checked
			? [...selectedInvigilatorOptions.filter((item) => item.id !== option.id), option]
			: selectedInvigilatorOptions.filter((item) => item.id !== option.id);
	}

	function loadAssignment(assignment: ExamDayRoomAssignmentView) {
		selectedDayId = assignment.examDayId;
		classroomId = assignment.classroomId;
		roomId = assignment.roomId;
		capacityOverride = assignment.capacityOverride ? String(assignment.capacityOverride) : '';
		selectedInvigilatorIds = assignment.invigilators.map((item) => item.staffId);
		selectedInvigilatorOptions = assignment.invigilators.map((item) => ({
			id: item.staffId,
			displayName: item.staffName ?? item.staffId
		}));
	}

	function resetForm() {
		classroomId = '';
		roomId = '';
		capacityOverride = '';
		selectedInvigilatorIds = [];
		selectedInvigilatorOptions = [];
		staffSearch = '';
	}

	async function submitForm() {
		const dayId = selectedDay?.id ?? selectedDayId;
		if (!dayId || !classroomId || !roomId) return;

		const saved = await onSaveAssignment?.(dayId, {
			classroomId,
			roomId,
			capacityOverride: capacityOverride ? Number(capacityOverride) : null,
			invigilatorStaffIds: selectedInvigilatorIds
		});
		if (saved) resetForm();
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

	$effect(() => {
		staffOptions = staff;
	});

	$effect(() => {
		if (!onSearchStaff || readonly) return;
		if (staffSearch.trim() === lastStaffSearch) return;

		const timeout = setTimeout(() => {
			const requestToken = ++staffSearchRequestToken;
			staffSearching = true;
			staffSearchError = '';
			lastStaffSearch = staffSearch.trim();

			onSearchStaff(staffSearch.trim())
				.then((results) => {
					if (requestToken !== staffSearchRequestToken) return;
					staffOptions = results;
				})
				.catch((error) => {
					if (requestToken !== staffSearchRequestToken) return;
					staffSearchError = error instanceof Error ? error.message : 'ค้นหาบุคลากรไม่สำเร็จ';
				})
				.finally(() => {
					if (requestToken !== staffSearchRequestToken) return;
					staffSearching = false;
				});
		}, 300);

		return () => {
			clearTimeout(timeout);
			staffSearchRequestToken += 1;
		};
	});
</script>

<section class="overflow-hidden rounded-md border bg-background">
	<div class="flex flex-col gap-3 border-b px-4 py-4 md:flex-row md:items-center md:justify-between">
		<div>
			<h2 class="font-semibold">ห้องสอบและกรรมการ</h2>
			<p class="text-sm text-muted-foreground">{assignments.length} ห้องในวันที่เลือก</p>
		</div>
		<Select.Root type="single" bind:value={selectedDayId}>
			<Select.Trigger class="w-full md:w-64">{dayLabel}</Select.Trigger>
			<Select.Content>
				{#each sortedDays as day (day.id)}
					<Select.Item value={day.id}>{formatDayDate(day.examDate, day.label)}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
	</div>

	<div class="grid gap-0 xl:grid-cols-[minmax(0,1fr)_24rem]">
		<div class="min-w-0 border-b xl:border-b-0 xl:border-r">
			{#if !selectedDay}
				<PageState title="ยังไม่มีวันสอบ" description="ต้องมีวันสอบก่อนกำหนดห้องสอบ" />
			{:else if assignments.length === 0}
				<PageState title="ยังไม่มีห้องสอบในวันนี้" description="ยังไม่พบการกำหนดห้องสอบสำหรับวันสอบที่เลือก" />
			{:else}
				<div class="overflow-x-auto">
					<Table class="min-w-[760px]">
						<TableHeader>
							<TableRow>
								<TableHead>ห้องเรียน</TableHead>
								<TableHead>ห้องสอบ</TableHead>
								<TableHead class="w-24 text-center">ความจุ</TableHead>
								<TableHead>กรรมการ</TableHead>
								<TableHead class="w-36 text-right">ที่นั่ง</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{#each assignments as assignment (assignment.id)}
								<TableRow>
									<TableCell class="font-medium">{assignment.classroomName ?? '-'}</TableCell>
									<TableCell>
										<div class="font-medium">{assignment.roomName ?? '-'}</div>
										<div class="text-xs text-muted-foreground">{assignmentRoomMeta(assignment)}</div>
									</TableCell>
									<TableCell class="text-center">
										<Badge variant="outline">{assignmentCapacity(assignment)}</Badge>
									</TableCell>
									<TableCell class="max-w-64 truncate text-sm text-muted-foreground">
										{invigilatorNames(assignment)}
									</TableCell>
									<TableCell class="text-right">
										<div class="flex justify-end gap-1">
											{#if !readonly}
												<Button variant="outline" size="sm" onclick={() => loadAssignment(assignment)}>
													แก้ไข
												</Button>
												<LoadingButton
													variant={assignment.invigilators.length > 0 ? 'secondary' : 'outline'}
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

		<div class="p-4">
			{#if readonly}
				<PageState
					variant="permission"
					title="อ่านอย่างเดียว"
					description="ผู้ใช้ปัจจุบันไม่มีสิทธิ์กำหนดห้องสอบ"
				/>
			{:else}
				<form
					class="space-y-4"
					onsubmit={(event) => {
						event.preventDefault();
						submitForm();
					}}
				>
					<div class="flex items-center justify-between gap-2">
						<h3 class="text-sm font-semibold">กำหนดห้องเรียน</h3>
						<Button type="button" variant="ghost" size="sm" onclick={resetForm}>ล้างฟอร์ม</Button>
					</div>

					<div class="grid gap-3">
						<div class="grid gap-2">
							<Label>ห้องเรียน</Label>
							<Select.Root type="single" bind:value={classroomId}>
								<Select.Trigger class="w-full">{selectedClassroomLabel}</Select.Trigger>
								<Select.Content>
									{#each filteredClassrooms as classroom (classroom.id)}
										<Select.Item value={classroom.id}>
											{classroom.grade_level_name ? `${classroom.grade_level_name} / ` : ''}{classroom.name}
										</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>

						<div class="grid gap-2">
							<Label>ห้องสอบ</Label>
							<Select.Root type="single" value={roomId} onValueChange={(value) => value && selectRoom(value)}>
								<Select.Trigger class="w-full">{selectedRoomLabel}</Select.Trigger>
								<Select.Content>
									{#each rooms as room (room.id)}
										<Select.Item value={room.id}>{roomLabel(room)}</Select.Item>
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

					<div class="space-y-2">
						<Label>กรรมการคุมสอบ</Label>
						<div class="relative">
							<Search class="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
							<Input class="pl-9" bind:value={staffSearch} placeholder="ค้นหาบุคลากร" />
						</div>
						<div class="max-h-56 space-y-2 overflow-y-auto rounded-md border p-3">
							{#if staffSearching}
								<div class="py-4 text-center text-sm text-muted-foreground">กำลังค้นหา...</div>
							{:else if staffSearchError}
								<div class="text-sm text-destructive">{staffSearchError}</div>
							{/if}

							{#if !staffSearching && staffOptionsForDisplay.length === 0}
								<div class="py-4 text-center text-sm text-muted-foreground">ไม่พบบุคลากร</div>
							{:else if !staffSearching}
								{#each staffOptionsForDisplay as option (option.id)}
									<label class="flex items-center gap-2 text-sm">
										<Checkbox
											checked={selectedInvigilatorIds.includes(option.id)}
											onCheckedChange={(checked) => toggleInvigilator(option, checked === true)}
										/>
										<span class="min-w-0 truncate">{staffOptionName(option)}</span>
									</label>
								{/each}
							{/if}
						</div>
						<div class="flex items-center gap-2 text-xs text-muted-foreground">
							<Users class="h-3.5 w-3.5" />
							{selectedInvigilatorIds.length} คน
						</div>
					</div>

					<LoadingButton
						type="submit"
						loading={saving}
						loadingLabel="กำลังบันทึก..."
						disabled={!selectedDay || !classroomId || !roomId}
						class="w-full"
					>
						บันทึกห้องสอบ
					</LoadingButton>
				</form>
			{/if}
		</div>
	</div>
</section>
