<script lang="ts">
	import { ArrowRight, RotateCcw, Search, SlidersHorizontal } from '@lucide/svelte';
	import type { StaffPublishedExamScheduleRound } from '#lib/api/examSchedule.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
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
	} from '#lib/utils/staff-exam-schedule-view.js';
	import MyExamInvigilationView from './MyExamInvigilationView.svelte';
	import StaffExamInvigilatorTable from './StaffExamInvigilatorTable.svelte';
	import StaffExamScheduleTable from './StaffExamScheduleTable.svelte';

	interface Props {
		rounds: StaffPublishedExamScheduleRound[];
		currentStaffId: string;
	}

	const levelOptions: Array<{ value: StaffExamScheduleLevelFilter; label: string }> = [
		{ value: 'all', label: 'ทุกช่วงชั้น' },
		{ value: 'lower_secondary', label: 'ม.1–ม.3' },
		{ value: 'upper_secondary', label: 'ม.4–ม.6' }
	];
	const thaiCollator = new Intl.Collator('th', { numeric: true, sensitivity: 'base' });
	const controlId = $props.id();

	let { rounds, currentStaffId }: Props = $props();
	let selectedRoundId = $state('');
	let selectedDayId = $state('all');
	let selectedLevel = $state<StaffExamScheduleLevelFilter>('all');
	let selectedHomeroomId = $state('all');
	let query = $state('');
	let activeTab = $state('overview');
	let filtersExpanded = $state(false);
	let hasFilters = $derived(
		selectedDayId !== 'all' ||
			selectedLevel !== 'all' ||
			selectedHomeroomId !== 'all' ||
			query.trim() !== ''
	);
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
					homeroomId: selectedHomeroomId,
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
			homeroomId: 'all',
			query: ''
		}).sessions;
		return [
			...new Map(
				records.map((record) => [
					record.homeroomId,
					{ id: record.homeroomId, name: record.homeroomName }
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
	let selectedHomeroomLabel = $derived(
		selectedHomeroomId === 'all'
			? 'ทุกชั้นเรียน'
			: (classroomOptions.find((item) => item.id === selectedHomeroomId)?.name ?? '-')
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
		selectedHomeroomId = 'all';
		query = '';
	}

	function selectRound(roundId: string) {
		selectedRoundId = roundId;
		clearFilters();
	}

	function selectLevel(level: StaffExamScheduleLevelFilter) {
		selectedLevel = level;
		selectedHomeroomId = 'all';
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
{:else}
	<div class="min-w-0 space-y-3">
		<div
			class="flex flex-wrap items-center gap-3 rounded-xl border bg-card p-3 sm:p-4"
			data-testid="exam-round-toolbar"
		>
			<Label for={`${controlId}-round`} class="shrink-0">รอบสอบ</Label>
			<Select.Root
				type="single"
				value={selectedRound.roundId}
				onValueChange={(value) => value && selectRound(value)}
			>
				<Select.Trigger
					id={`${controlId}-round`}
					aria-label="เลือกรอบสอบ"
					class="min-w-0 flex-1 sm:max-w-sm">{selectedRound.roundName}</Select.Trigger
				>
				<Select.Content>
					{#each rounds as round (round.roundId)}<Select.Item value={round.roundId}
							>{round.roundName}</Select.Item
						>{/each}
				</Select.Content>
			</Select.Root>
			<Badge variant="secondary" class="hidden sm:inline-flex">เผยแพร่แล้ว</Badge>
		</div>

		<Tabs.Root bind:value={activeTab} class="min-w-0 gap-3">
			<div class="min-w-0 overflow-x-auto pb-1">
				<Tabs.List class="w-max min-w-full justify-start">
					<Tabs.Trigger value="overview">ภาพรวม</Tabs.Trigger>
					<Tabs.Trigger value="schedule">ตารางสอบ</Tabs.Trigger>
					<Tabs.Trigger value="invigilators">กรรมการคุมสอบ</Tabs.Trigger>
					<Tabs.Trigger value="mine">งานคุมของฉัน</Tabs.Trigger>
				</Tabs.List>
			</div>

			{#if flattened.sessions.length === 0}
				<PageState
					title="รอบสอบนี้ยังไม่มีรายการสอบ"
					description="ไม่พบรายวิชาที่จัดลงวัน เวลา และห้องสอบในรอบที่เลือก"
				/>
			{:else}
				{#if activeTab === 'schedule' || activeTab === 'invigilators'}
					<section
						class="space-y-3 rounded-xl border bg-card p-3 sm:p-4"
						aria-label="ตัวกรองตารางสอบ"
					>
						<div class="flex items-center gap-3">
							<div class="relative min-w-0 flex-1">
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
							<Button
								variant="outline"
								size="sm"
								class="md:hidden"
								aria-expanded={filtersExpanded}
								aria-controls={`${controlId}-filters`}
								onclick={() => (filtersExpanded = !filtersExpanded)}
							>
								<SlidersHorizontal class="size-4" aria-hidden="true" />ตัวกรอง
							</Button>
							<Button
								variant="ghost"
								size="sm"
								disabled={!hasFilters}
								onclick={clearFilters}
								aria-label="ล้างตัวกรอง"
							>
								<RotateCcw class="size-4" aria-hidden="true" /><span class="hidden sm:inline"
									>ล้างตัวกรอง</span
								>
							</Button>
						</div>
						<div
							id={`${controlId}-filters`}
							class={`${filtersExpanded ? 'grid' : 'hidden'} gap-3 md:grid md:grid-cols-2`}
						>
							<div class="space-y-1.5">
								<Label for={`${controlId}-day`}>วันสอบ</Label>
								<Select.Root
									type="single"
									value={selectedDayId}
									onValueChange={(value) => value && (selectedDayId = value)}
								>
									<Select.Trigger id={`${controlId}-day`} aria-label="กรองตามวันสอบ" class="w-full"
										>{selectedDayId === 'all'
											? selectedDayLabel
											: formatStaffExamDate(selectedDayLabel)}</Select.Trigger
									>
									<Select.Content>
										<Select.Item value="all">ทุกวันสอบ</Select.Item>
										{#each dayOptions as day (day.examDayId)}<Select.Item value={day.examDayId}
												>{formatStaffExamDate(day.examDate)}</Select.Item
											>{/each}
									</Select.Content>
								</Select.Root>
							</div>
							<div class="space-y-1.5">
								<Label for={`${controlId}-classroom`}>ชั้นเรียน</Label>
								<Select.Root
									type="single"
									value={selectedHomeroomId}
									onValueChange={(value) => value && (selectedHomeroomId = value)}
								>
									<Select.Trigger
										id={`${controlId}-classroom`}
										aria-label="กรองตามชั้นเรียน"
										class="w-full">{selectedHomeroomLabel}</Select.Trigger
									>
									<Select.Content>
										<Select.Item value="all">ทุกชั้นเรียน</Select.Item>
										{#each classroomOptions as classroom (classroom.id)}<Select.Item
												value={classroom.id}>{classroom.name}</Select.Item
											>{/each}
									</Select.Content>
								</Select.Root>
							</div>
							<div class="space-y-1.5 md:col-span-2">
								<Label id={`${controlId}-level`}>ช่วงชั้น</Label>
								<ToggleGroup.Root
									type="single"
									value={selectedLevel}
									aria-labelledby={`${controlId}-level`}
									onValueChange={(value) =>
										value && selectLevel(value as StaffExamScheduleLevelFilter)}
									class="flex-wrap"
								>
									{#each levelOptions as option (option.value)}<ToggleGroup.Item
											value={option.value}>{option.label}</ToggleGroup.Item
										>{/each}
								</ToggleGroup.Root>
							</div>
						</div>
						{#if !filtersExpanded && (selectedDayId !== 'all' || selectedLevel !== 'all' || selectedHomeroomId !== 'all')}
							<p class="text-xs text-muted-foreground md:hidden">
								{selectedDayId === 'all' ? selectedDayLabel : formatStaffExamDate(selectedDayLabel)} ·
								{selectedLevelLabel} · {selectedHomeroomLabel}
							</p>
						{/if}
					</section>
				{/if}

				<Tabs.Content value="overview" class="space-y-4">
					<section
						class="space-y-3 rounded-xl border border-primary/15 bg-gradient-to-br from-primary/5 to-card p-3 sm:p-4"
						aria-label="สรุปรอบสอบ"
					>
						<div class="flex flex-wrap items-center justify-between gap-2">
							<h2 class="font-semibold">{selectedRound.roundName}</h2>
							<p class="text-sm text-muted-foreground">{dateRange}</p>
						</div>
						<dl class="flex flex-wrap gap-x-6 gap-y-2 text-sm">
							<div class="flex items-baseline gap-2">
								<dd class="text-xl font-semibold tabular-nums">
									{roundSummary?.examDayCount ?? 0}
								</dd>
								<dt class="text-muted-foreground">วันสอบ</dt>
							</div>
							<div class="flex items-baseline gap-2">
								<dd class="text-xl font-semibold tabular-nums">
									{roundSummary?.examRoomCount ?? 0}
								</dd>
								<dt class="text-muted-foreground">ห้องสอบที่ใช้</dt>
							</div>
							<div class="flex items-baseline gap-2">
								<dd class="text-xl font-semibold tabular-nums">
									{roundSummary?.invigilatorCount ?? 0}
								</dd>
								<dt class="text-muted-foreground">กรรมการที่มอบหมาย</dt>
							</div>
						</dl>
						<p class="border-t pt-3 text-sm text-muted-foreground">
							มัธยมศึกษาตอนต้น {lowerSecondaryCount} รายการ · มัธยมศึกษาตอนปลาย {upperSecondaryCount}
							รายการ
						</p>
					</section>
					<section
						class="rounded-xl border bg-card p-3 sm:p-4"
						aria-labelledby={`${controlId}-next-duty`}
					>
						<div class="flex flex-wrap items-center justify-between gap-2">
							<h2 id={`${controlId}-next-duty`} class="font-semibold">งานคุมถัดไปของฉัน</h2>
							<Button variant="ghost" size="sm" onclick={() => (activeTab = 'mine')}
								>งานคุมของฉัน<ArrowRight class="size-4" aria-hidden="true" /></Button
							>
						</div>
						<p class="mt-2 text-sm font-medium">{nextAssignmentLabel()}</p>
						{#if roundSummary?.nextPersonalAssignment}<p class="mt-1 text-sm text-muted-foreground">
								{roundSummary.nextPersonalAssignment.homeroomName} · {roundSummary
									.nextPersonalAssignment.roomName}
							</p>{/if}
						<p class="mt-2 text-sm text-muted-foreground">
							{mySummary.assignmentCount} งาน · เวลาคุมจริง {formatStaffExamMinutes(
								mySummary.totalMinutes
							)} ในรอบนี้
						</p>
					</section>
					<section class="space-y-2" aria-labelledby={`${controlId}-days`}>
						<h2 id={`${controlId}-days`} class="font-semibold">วันสอบที่กำลังมาถึง</h2>
						{#if upcomingDays.length === 0}<p class="text-sm text-muted-foreground">
								ไม่มีวันสอบที่กำลังมาถึง
							</p>
						{:else}<ul class="divide-y rounded-xl border bg-card">
								{#each upcomingDays as examDay (examDay.examDayId)}<li>
										<Button
											variant="ghost"
											class="h-auto w-full justify-between rounded-none px-3 py-2 text-left whitespace-normal"
											onclick={() => openExamDay(examDay.examDayId)}
											><span
												><span class="block">{formatStaffExamDate(examDay.examDate)}</span><span
													class="text-xs text-muted-foreground"
													>{examDay.label ? `${examDay.label} · ` : ''}{examDaySessionCount(
														examDay.examDayId
													)} รายการสอบ</span
												></span
											><ArrowRight class="size-4 shrink-0" aria-hidden="true" /></Button
										>
									</li>{/each}
							</ul>{/if}
					</section>
				</Tabs.Content>
				<Tabs.Content value="schedule">
					{#if scheduleRows.length === 0}<PageState
							title="ไม่พบตารางสอบตามตัวกรอง"
							description="ลองเปลี่ยนวัน ช่วงชั้น ชั้นเรียน หรือคำค้นหา"
							actionLabel="ล้างตัวกรอง"
							onaction={clearFilters}
						/>
					{:else}<StaffExamScheduleTable rows={scheduleRows} />{/if}
				</Tabs.Content>
				<Tabs.Content value="invigilators">
					{#if invigilatorRows.length === 0}<PageState
							title="ไม่พบกรรมการคุมสอบตามตัวกรอง"
							description="ลองเปลี่ยนวัน ช่วงชั้น ชั้นเรียน หรือคำค้นหา"
							actionLabel="ล้างตัวกรอง"
							onaction={clearFilters}
						/>
					{:else}<StaffExamInvigilatorTable rows={invigilatorRows} {currentStaffId} />{/if}
				</Tabs.Content>
				<Tabs.Content value="mine"><MyExamInvigilationView summary={mySummary} /></Tabs.Content>
			{/if}
		</Tabs.Root>
	</div>
{/if}
