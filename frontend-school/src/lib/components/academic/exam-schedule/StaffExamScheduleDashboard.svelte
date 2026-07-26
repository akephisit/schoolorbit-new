<script lang="ts">
	import {
		ArrowRight,
		CalendarClock,
		CalendarDays,
		DoorOpen,
		RotateCcw,
		Search,
		Users
	} from 'lucide-svelte';
	import type { StaffPublishedExamScheduleRound } from '$lib/api/examSchedule';
	import { PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Tabs from '$lib/components/ui/tabs';
	import {
		buildMyExamInvigilationSummary,
		buildStaffExamInvigilatorRenderRows,
		buildStaffExamRoundSummary,
		buildStaffExamScheduleRenderRows,
		filterStaffExamScheduleRound,
		flattenStaffExamScheduleRound,
		formatStaffExamDate,
		formatStaffExamMinutes,
		formatStaffExamTime,
		type StaffExamScheduleLevelFilter
	} from '$lib/utils/staff-exam-schedule-view';
	import MyExamInvigilationView from './MyExamInvigilationView.svelte';
	import StaffExamInvigilatorTable from './StaffExamInvigilatorTable.svelte';
	import StaffExamScheduleTable from './StaffExamScheduleTable.svelte';

	interface Props {
		rounds: StaffPublishedExamScheduleRound[];
		currentStaffId: string;
	}

	const levelOptions: Array<{ value: StaffExamScheduleLevelFilter; label: string }> = [
		{ value: 'all', label: 'ทุกระดับชั้น' },
		{ value: 'lower_secondary', label: 'มัธยมศึกษาตอนต้น (ม.1–ม.3)' },
		{ value: 'upper_secondary', label: 'มัธยมศึกษาตอนปลาย (ม.4–ม.6)' }
	];
	const thaiCollator = new Intl.Collator('th', { numeric: true, sensitivity: 'base' });
	const controlId = $props.id();

	let { rounds, currentStaffId }: Props = $props();
	let selectedRoundId = $state('');
	let selectedDayId = $state('all');
	let selectedLevel = $state<StaffExamScheduleLevelFilter>('all');
	let selectedClassroomId = $state('all');
	let query = $state('');
	let activeTab = $state('overview');
	const now = new Date();

	let selectedRound = $derived(
		rounds.find((round) => round.roundId === selectedRoundId) ?? rounds[0] ?? null
	);
	let flattened = $derived(
		selectedRound ? flattenStaffExamScheduleRound(selectedRound) : { sessions: [], assignments: [] }
	);
	let filtered = $derived(
		selectedRound
			? filterStaffExamScheduleRound(selectedRound, {
					dayId: selectedDayId,
					level: selectedLevel,
					classroomId: selectedClassroomId,
					query
				})
			: { sessions: [], assignments: [] }
	);
	let scheduleRows = $derived(buildStaffExamScheduleRenderRows(filtered.sessions));
	let invigilatorRows = $derived(
		buildStaffExamInvigilatorRenderRows(filtered.assignments, currentStaffId)
	);
	let mySummary = $derived(
		buildMyExamInvigilationSummary(flattened.assignments, currentStaffId, now)
	);
	let roundSummary = $derived(
		selectedRound ? buildStaffExamRoundSummary(selectedRound, currentStaffId, now) : null
	);
	let dayOptions = $derived(
		selectedRound
			? [...selectedRound.days].sort((left, right) => left.examDate.localeCompare(right.examDate))
			: []
	);
	let classroomOptions = $derived.by(() => {
		if (!selectedRound) return [];
		const records = filterStaffExamScheduleRound(selectedRound, {
			dayId: 'all',
			level: selectedLevel,
			classroomId: 'all',
			query: ''
		}).sessions;
		return [
			...new Map(
				records.map((record) => [
					record.classroomId,
					{ id: record.classroomId, name: record.classroomName }
				])
			).values()
		].sort((left, right) => thaiCollator.compare(left.name, right.name));
	});
	let selectedLevelLabel = $derived(
		levelOptions.find((option) => option.value === selectedLevel)?.label ?? 'ทุกระดับชั้น'
	);
	let selectedDayLabel = $derived(
		selectedDayId === 'all'
			? 'ทุกวันสอบ'
			: (dayOptions.find((day) => day.examDayId === selectedDayId)?.examDate ?? '-')
	);
	let selectedClassroomLabel = $derived(
		selectedClassroomId === 'all'
			? 'ทุกชั้นเรียน'
			: (classroomOptions.find((item) => item.id === selectedClassroomId)?.name ?? '-')
	);
	let lowerSecondaryCount = $derived(
		flattened.sessions.filter(
			(session) =>
				session.gradeLevelType === 'secondary' &&
				session.gradeLevelYear >= 1 &&
				session.gradeLevelYear <= 3
		).length
	);
	let upperSecondaryCount = $derived(
		flattened.sessions.filter(
			(session) =>
				session.gradeLevelType === 'secondary' &&
				session.gradeLevelYear >= 4 &&
				session.gradeLevelYear <= 6
		).length
	);
	let dateRange = $derived.by(() => {
		if (dayOptions.length === 0) return '-';
		const first = dayOptions[0].examDate;
		const last = dayOptions.at(-1)?.examDate ?? first;
		return first === last
			? formatStaffExamDate(first)
			: `${formatStaffExamDate(first)} – ${formatStaffExamDate(last)}`;
	});
	let nextExamDay = $derived.by(() => {
		const today = localDateKey(now);
		return dayOptions.find((day) => day.examDate >= today) ?? null;
	});
	let upcomingDays = $derived.by(() => {
		const today = localDateKey(now);
		return dayOptions.filter((day) => day.examDate >= today);
	});

	function localDateKey(value: Date): string {
		const year = value.getFullYear();
		const month = String(value.getMonth() + 1).padStart(2, '0');
		const day = String(value.getDate()).padStart(2, '0');
		return `${year}-${month}-${day}`;
	}

	function clearFilters() {
		selectedDayId = 'all';
		selectedLevel = 'all';
		selectedClassroomId = 'all';
		query = '';
	}

	function selectRound(roundId: string) {
		selectedRoundId = roundId;
		clearFilters();
	}

	function selectLevel(level: StaffExamScheduleLevelFilter) {
		selectedLevel = level;
		selectedClassroomId = 'all';
	}

	function openExamDay(examDayId: string) {
		selectedDayId = examDayId;
		activeTab = 'schedule';
	}

	function examDaySessionCount(examDayId: string): number {
		return selectedRound?.days.find((day) => day.examDayId === examDayId)?.sessions.length ?? 0;
	}

	function nextAssignmentLabel(): string {
		const assignment = roundSummary?.nextPersonalAssignment;
		if (!assignment) return 'ยังไม่มีงานคุมสอบ';
		return `${formatStaffExamDate(assignment.examDate)} · ${formatStaffExamTime(
			assignment.earliestStartsAt ?? ''
		)}–${formatStaffExamTime(assignment.latestEndsAt ?? '')}`;
	}
</script>

{#if !selectedRound}
	<PageState
		title="ยังไม่มีตารางสอบที่เผยแพร่"
		description="เมื่อฝ่ายวิชาการเผยแพร่ตารางสอบ รายการจะแสดงที่หน้านี้"
	/>
{:else if flattened.sessions.length === 0}
	<PageState
		title="รอบสอบนี้ยังไม่มีรายการสอบ"
		description="ไม่พบรายวิชาที่จัดลงวัน เวลา และห้องสอบในรอบที่เลือก"
	/>
{:else}
	<div class="min-w-0 space-y-5">
		<Card.Root class="gap-0 py-0">
			<Card.Content
				class="grid gap-3 p-3 sm:p-4 md:grid-cols-2 xl:grid-cols-[minmax(14rem,2fr)_repeat(4,minmax(8rem,1fr))_auto]"
			>
				<div class="space-y-2 md:col-span-2 xl:col-span-1">
					<Label for={`${controlId}-round`}>รอบสอบ</Label>
					<Select.Root
						type="single"
						value={selectedRound.roundId}
						onValueChange={(value) => value && selectRound(value)}
					>
						<Select.Trigger id={`${controlId}-round`} aria-label="เลือกรอบสอบ" class="w-full">
							{selectedRound.roundName}
						</Select.Trigger>
						<Select.Content>
							{#each rounds as round (round.roundId)}
								<Select.Item value={round.roundId}>{round.roundName}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-2">
					<Label for={`${controlId}-day`}>วันสอบ</Label>
					<Select.Root
						type="single"
						value={selectedDayId}
						onValueChange={(value) => value && (selectedDayId = value)}
					>
						<Select.Trigger id={`${controlId}-day`} aria-label="กรองตามวันสอบ" class="w-full">
							{selectedDayLabel === 'ทุกวันสอบ'
								? selectedDayLabel
								: formatStaffExamDate(selectedDayLabel)}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="all">ทุกวันสอบ</Select.Item>
							{#each dayOptions as day (day.examDayId)}
								<Select.Item value={day.examDayId}>
									{formatStaffExamDate(day.examDate)}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-2">
					<Label for={`${controlId}-level`}>ช่วงชั้น</Label>
					<Select.Root
						type="single"
						value={selectedLevel}
						onValueChange={(value) => value && selectLevel(value as StaffExamScheduleLevelFilter)}
					>
						<Select.Trigger id={`${controlId}-level`} aria-label="กรองตามช่วงชั้น" class="w-full">
							{selectedLevelLabel}
						</Select.Trigger>
						<Select.Content>
							{#each levelOptions as option (option.value)}
								<Select.Item value={option.value}>{option.label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-2">
					<Label for={`${controlId}-classroom`}>ชั้นเรียน</Label>
					<Select.Root
						type="single"
						value={selectedClassroomId}
						onValueChange={(value) => value && (selectedClassroomId = value)}
					>
						<Select.Trigger
							id={`${controlId}-classroom`}
							aria-label="กรองตามชั้นเรียน"
							class="w-full"
						>
							{selectedClassroomLabel}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="all">ทุกชั้นเรียน</Select.Item>
							{#each classroomOptions as classroom (classroom.id)}
								<Select.Item value={classroom.id}>{classroom.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-2">
					<Label for={`${controlId}-search`}>ค้นหา</Label>
					<div class="relative">
						<Search
							class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
							aria-hidden="true"
						/>
						<Input
							id={`${controlId}-search`}
							bind:value={query}
							aria-label="ค้นหาวิชา ห้อง หรือกรรมการคุมสอบ"
							placeholder="วิชา ห้อง หรือชื่อครู"
							class="pl-9"
						/>
					</div>
				</div>

				<div class="flex items-end md:col-span-2 xl:col-span-1 xl:justify-end">
					<Button class="w-full xl:w-auto" variant="outline" onclick={clearFilters}>
						<RotateCcw class="size-4" aria-hidden="true" />
						ล้างตัวกรอง
					</Button>
				</div>
			</Card.Content>
		</Card.Root>

		<div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
			<Card.Root class="gap-0 py-0">
				<Card.Content class="flex items-center gap-3 p-4">
					<div class="rounded-lg bg-primary/10 p-2 text-primary">
						<CalendarDays class="size-5" aria-hidden="true" />
					</div>
					<div>
						<div class="text-2xl font-semibold">{roundSummary?.examDayCount ?? 0}</div>
						<div class="text-xs text-muted-foreground">วันสอบ</div>
					</div>
				</Card.Content>
			</Card.Root>
			<Card.Root class="gap-0 py-0">
				<Card.Content class="flex items-center gap-3 p-4">
					<div class="rounded-lg bg-primary/10 p-2 text-primary">
						<DoorOpen class="size-5" aria-hidden="true" />
					</div>
					<div>
						<div class="text-2xl font-semibold">{roundSummary?.examRoomCount ?? 0}</div>
						<div class="text-xs text-muted-foreground">ห้องสอบที่ใช้</div>
					</div>
				</Card.Content>
			</Card.Root>
			<Card.Root class="gap-0 py-0">
				<Card.Content class="flex items-center gap-3 p-4">
					<div class="rounded-lg bg-primary/10 p-2 text-primary">
						<Users class="size-5" aria-hidden="true" />
					</div>
					<div>
						<div class="text-2xl font-semibold">{roundSummary?.invigilatorCount ?? 0}</div>
						<div class="text-xs text-muted-foreground">กรรมการที่มอบหมาย</div>
					</div>
				</Card.Content>
			</Card.Root>
			<Card.Root class="gap-0 py-0">
				<Card.Content class="flex items-center gap-3 p-4">
					<div class="rounded-lg bg-primary/10 p-2 text-primary">
						<CalendarClock class="size-5" aria-hidden="true" />
					</div>
					<div class="min-w-0">
						<div class="truncate font-semibold">{nextAssignmentLabel()}</div>
						<div class="text-xs text-muted-foreground">งานคุมถัดไปของฉัน</div>
					</div>
				</Card.Content>
			</Card.Root>
		</div>

		<Tabs.Root bind:value={activeTab} class="min-w-0 gap-4">
			<div class="overflow-x-auto pb-1">
				<Tabs.List class="w-max min-w-full justify-start">
					<Tabs.Trigger value="overview">ภาพรวม</Tabs.Trigger>
					<Tabs.Trigger value="schedule">ตารางสอบ</Tabs.Trigger>
					<Tabs.Trigger value="invigilators">กรรมการคุมสอบ</Tabs.Trigger>
					<Tabs.Trigger value="mine">งานคุมของฉัน</Tabs.Trigger>
				</Tabs.List>
			</div>

			<Tabs.Content value="overview" class="space-y-4">
				<div class="grid gap-4 lg:grid-cols-3">
					<Card.Root class="lg:col-span-2">
						<Card.Header>
							<div class="flex flex-wrap items-start justify-between gap-2">
								<div>
									<Card.Title>{selectedRound.roundName}</Card.Title>
									<Card.Description>{dateRange}</Card.Description>
								</div>
								<Badge>เผยแพร่แล้ว</Badge>
							</div>
						</Card.Header>
						<Card.Content class="grid gap-3 sm:grid-cols-3">
							<div class="rounded-lg border p-3">
								<div class="text-xs text-muted-foreground">วันสอบถัดไป</div>
								<div class="mt-1 font-medium">
									{nextExamDay ? formatStaffExamDate(nextExamDay.examDate) : 'สอบครบแล้ว'}
								</div>
								{#if nextExamDay}
									<div class="text-xs text-muted-foreground">
										{examDaySessionCount(nextExamDay.examDayId)} รายการสอบ
									</div>
								{/if}
							</div>
							<div class="rounded-lg border p-3">
								<div class="text-xs text-muted-foreground">มัธยมศึกษาตอนต้น</div>
								<div class="mt-1 text-xl font-semibold">{lowerSecondaryCount}</div>
								<div class="text-xs text-muted-foreground">รายการสอบ</div>
							</div>
							<div class="rounded-lg border p-3">
								<div class="text-xs text-muted-foreground">มัธยมศึกษาตอนปลาย</div>
								<div class="mt-1 text-xl font-semibold">{upperSecondaryCount}</div>
								<div class="text-xs text-muted-foreground">รายการสอบ</div>
							</div>
						</Card.Content>
					</Card.Root>

					<Card.Root>
						<Card.Header>
							<Card.Title class="text-base">งานคุมสอบของฉัน</Card.Title>
							<Card.Description>
								{formatStaffExamMinutes(mySummary.totalMinutes)} ในรอบนี้
							</Card.Description>
						</Card.Header>
						<Card.Content class="text-sm">
							{#if roundSummary?.nextPersonalAssignment}
								<div class="space-y-1">
									<div class="font-medium">
										{formatStaffExamDate(roundSummary.nextPersonalAssignment.examDate)}
									</div>
									<div class="text-muted-foreground">
										{roundSummary.nextPersonalAssignment.classroomName} ·
										{roundSummary.nextPersonalAssignment.roomName}
									</div>
								</div>
							{:else}
								<div class="text-muted-foreground">ยังไม่มีงานคุมสอบที่กำลังมาถึง</div>
							{/if}
						</Card.Content>
					</Card.Root>
				</div>

				<Card.Root>
					<Card.Header>
						<Card.Title class="text-base">วันสอบที่กำลังมาถึง</Card.Title>
						<Card.Description>เลือกวันเพื่อเปิดตารางสอบเฉพาะวันนั้น</Card.Description>
					</Card.Header>
					<Card.Content class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
						{#if upcomingDays.length === 0}
							<div class="text-sm text-muted-foreground">ไม่มีวันสอบที่กำลังมาถึง</div>
						{:else}
							{#each upcomingDays as examDay (examDay.examDayId)}
								<Button
									variant="outline"
									class="h-auto justify-between p-3 text-left"
									onclick={() => openExamDay(examDay.examDayId)}
								>
									<span>
										<span class="block font-medium">{formatStaffExamDate(examDay.examDate)}</span>
										<span class="block text-xs text-muted-foreground">
											{examDay.sessions.length} รายการสอบ
										</span>
									</span>
									<ArrowRight class="size-4" aria-hidden="true" />
								</Button>
							{/each}
						{/if}
					</Card.Content>
				</Card.Root>
			</Tabs.Content>

			<Tabs.Content value="schedule">
				{#if scheduleRows.length === 0}
					<PageState
						title="ไม่พบตารางสอบตามตัวกรอง"
						description="ลองเปลี่ยนวัน ช่วงชั้น ชั้นเรียน หรือคำค้นหา"
						actionLabel="ล้างตัวกรอง"
						onaction={clearFilters}
					/>
				{:else}
					<StaffExamScheduleTable rows={scheduleRows} />
				{/if}
			</Tabs.Content>

			<Tabs.Content value="invigilators">
				{#if invigilatorRows.length === 0}
					<PageState
						title="ไม่พบกรรมการคุมสอบตามตัวกรอง"
						description="ลองเปลี่ยนวัน ช่วงชั้น ชั้นเรียน หรือคำค้นหา"
						actionLabel="ล้างตัวกรอง"
						onaction={clearFilters}
					/>
				{:else}
					<StaffExamInvigilatorTable rows={invigilatorRows} {currentStaffId} />
				{/if}
			</Tabs.Content>

			<Tabs.Content value="mine">
				<MyExamInvigilationView summary={mySummary} />
			</Tabs.Content>
		</Tabs.Root>
	</div>
{/if}
