<script lang="ts">
	import type {
		ExamDayDetail,
		ExamInvigilatorAssignmentSummary,
		ExamInvigilatorStaffOption,
		ExamInvigilatorWorkspace
	} from '$lib/api/examSchedule';
	import { LoadingButton, PageState } from '$lib/components/app-state';
	import * as Select from '$lib/components/ui/select';
	import { compareExamDaysByDate } from '$lib/utils/examScheduleDayOrder';
	import { RefreshCw } from 'lucide-svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import InvigilatorRoomBoard from './InvigilatorRoomBoard.svelte';
	import InvigilatorStaffList from './InvigilatorStaffList.svelte';
	import {
		staffOptionName,
		workloadStaffName,
		type InvigilatorStaffCardView
	} from './invigilatorDrag';

	let {
		days = [],
		workspace,
		staff = [],
		loading = false,
		loadError = '',
		readonly = false,
		onAssignInvigilator,
		onRemoveInvigilator,
		onRetry
	}: {
		days: ExamDayDetail[];
		workspace: ExamInvigilatorWorkspace | null;
		staff: ExamInvigilatorStaffOption[];
		loading?: boolean;
		loadError?: string;
		readonly?: boolean;
		onAssignInvigilator?: (
			assignmentId: string,
			staffId: string
		) => Promise<ExamInvigilatorWorkspace>;
		onRemoveInvigilator?: (
			assignmentId: string,
			staffId: string
		) => Promise<ExamInvigilatorWorkspace>;
		onRetry?: () => Promise<void> | void;
	} = $props();

	let selectedDayId = $state('');
	let staffSearch = $state('');
	let showAvailableOnly = $state(false);
	let localWorkspace = $derived(workspace);
	let pendingStaffIds = $state<string[]>([]);
	let pendingAssignmentIds = $state<string[]>([]);
	let activeDragStaffId = $state<string | null>(null);

	const sortedDays = $derived([...days].sort(compareExamDaysByDate));
	const selectedDay = $derived(
		days.find((day) => day.id === selectedDayId) ?? sortedDays[0] ?? null
	);
	const selectedDayAssignments = $derived(
		[...(localWorkspace?.assignments ?? [])]
			.filter((assignment) => assignment.examDayId === (selectedDay?.id ?? selectedDayId))
			.sort((a, b) => {
				const classroomCompare = a.classroomName.localeCompare(b.classroomName, 'th');
				return classroomCompare === 0
					? a.roomName.localeCompare(b.roomName, 'th')
					: classroomCompare;
			})
	);
	const dayLabel = $derived(
		selectedDay ? formatDayDate(selectedDay.examDate, selectedDay.label) : 'เลือกวันสอบ'
	);
	const staffCards = $derived(buildStaffCards());
	const displayedStaffCards = $derived(filterStaffCards(staffCards));

	function formatDayDate(value: string, label?: string | null): string {
		const dateLabel = new Date(value).toLocaleDateString('th-TH', {
			weekday: 'short',
			month: 'short',
			day: 'numeric'
		});
		return label ? `${label} · ${dateLabel}` : dateLabel;
	}

	function selectedDayMinutes(staffId: string): number {
		const workload = localWorkspace?.staffWorkloads.find((item) => item.staffId === staffId);
		return workload?.days.find((day) => day.examDayId === selectedDayId)?.minutes ?? 0;
	}

	function selectedDayAssignment(staffId: string): ExamInvigilatorAssignmentSummary | null {
		return (
			selectedDayAssignments.find((assignment) =>
				assignment.invigilators.some((invigilator) => invigilator.staffId === staffId)
			) ?? null
		);
	}

	function buildStaffCards(): InvigilatorStaffCardView[] {
		const cards = new SvelteMap<string, InvigilatorStaffCardView>();

		for (const option of staff) {
			cards.set(option.staffId, {
				staffId: option.staffId,
				displayName: staffOptionName(option),
				selectedDayMinutes: selectedDayMinutes(option.staffId),
				totalMinutes:
					localWorkspace?.staffWorkloads.find((workload) => workload.staffId === option.staffId)
						?.totalMinutes ?? 0,
				assignedAssignment: selectedDayAssignment(option.staffId)
			});
		}

		for (const workload of localWorkspace?.staffWorkloads ?? []) {
			if (!cards.has(workload.staffId)) {
				cards.set(workload.staffId, {
					staffId: workload.staffId,
					displayName: workloadStaffName(workload),
					selectedDayMinutes: selectedDayMinutes(workload.staffId),
					totalMinutes: workload.totalMinutes,
					assignedAssignment: selectedDayAssignment(workload.staffId)
				});
			}
		}

		return [...cards.values()].sort((a, b) => {
			const totalCompare = a.totalMinutes - b.totalMinutes;
			if (totalCompare !== 0) return totalCompare;
			const todayCompare = a.selectedDayMinutes - b.selectedDayMinutes;
			return todayCompare === 0 ? a.displayName.localeCompare(b.displayName, 'th') : todayCompare;
		});
	}

	function filterStaffCards(cards: InvigilatorStaffCardView[]): InvigilatorStaffCardView[] {
		const search = staffSearch.trim().toLowerCase();
		return cards.filter((card) => {
			if (showAvailableOnly && card.assignedAssignment) return false;
			if (!search) return true;
			return card.displayName.toLowerCase().includes(search);
		});
	}

	function markPending(assignmentId: string, staffId: string) {
		if (!pendingAssignmentIds.includes(assignmentId)) {
			pendingAssignmentIds = [...pendingAssignmentIds, assignmentId];
		}
		if (!pendingStaffIds.includes(staffId)) {
			pendingStaffIds = [...pendingStaffIds, staffId];
		}
	}

	function clearPending(assignmentId: string, staffId: string) {
		pendingAssignmentIds = pendingAssignmentIds.filter((id) => id !== assignmentId);
		pendingStaffIds = pendingStaffIds.filter((id) => id !== staffId);
	}

	function applyWorkspace(workspaceData: ExamInvigilatorWorkspace) {
		localWorkspace = workspaceData;
	}

	function handleStaffDragStart(staffId: string) {
		activeDragStaffId = staffId;
	}

	function handleStaffDragEnd() {
		activeDragStaffId = null;
	}

	async function assignInvigilator(assignmentId: string, staffId: string) {
		if (readonly || !onAssignInvigilator || pendingStaffIds.includes(staffId)) return;

		markPending(assignmentId, staffId);
		try {
			const updatedWorkspace = await onAssignInvigilator(assignmentId, staffId);
			applyWorkspace(updatedWorkspace);
		} catch {
			// The route-level callback shows the toast and keeps the previous workspace.
		} finally {
			clearPending(assignmentId, staffId);
			activeDragStaffId = null;
		}
	}

	async function removeInvigilator(assignmentId: string, staffId: string) {
		if (readonly || !onRemoveInvigilator || pendingStaffIds.includes(staffId)) return;

		markPending(assignmentId, staffId);
		try {
			const updatedWorkspace = await onRemoveInvigilator(assignmentId, staffId);
			applyWorkspace(updatedWorkspace);
		} catch {
			// The route-level callback shows the toast and keeps the previous workspace.
		} finally {
			clearPending(assignmentId, staffId);
			activeDragStaffId = null;
		}
	}

	$effect(() => {
		if (!selectedDayId && sortedDays[0]) {
			selectedDayId = sortedDays[0].id;
		}
		if (selectedDayId && !days.some((day) => day.id === selectedDayId)) {
			selectedDayId = sortedDays[0]?.id ?? '';
			pendingStaffIds = [];
			pendingAssignmentIds = [];
			activeDragStaffId = null;
		}
	});
</script>

{#if loadError}
	<section class="rounded-md border bg-background">
		<PageState
			variant="error"
			title="โหลดข้อมูลกรรมการคุมสอบไม่สำเร็จ"
			description={loadError}
			actionLabel="ลองอีกครั้ง"
			onaction={onRetry}
		/>
	</section>
{:else if localWorkspace === null}
	<section class="rounded-md border bg-background">
		<PageState
			title={loading ? 'กำลังโหลดข้อมูลกรรมการคุมสอบ' : 'ยังไม่มีข้อมูลกรรมการคุมสอบ'}
			description="ข้อมูลอ้างอิงจากห้องสอบที่กำหนดไว้ในรอบนี้"
		/>
	</section>
{:else}
	<section class="flex h-full min-h-0 flex-col overflow-hidden rounded-md border bg-muted/20">
		<div
			class="flex flex-col gap-3 border-b bg-background px-4 py-4 lg:flex-row lg:items-center lg:justify-between"
		>
			<div>
				<h2 class="font-semibold">จัดกรรมการคุมสอบ</h2>
				<p class="text-sm text-muted-foreground">ลากครูไปวางในห้องสอบของวันที่เลือก</p>
			</div>
			<div class="flex flex-wrap items-center gap-2">
				<Select.Root type="single" bind:value={selectedDayId}>
					<Select.Trigger class="w-full sm:w-64">{dayLabel}</Select.Trigger>
					<Select.Content>
						{#each sortedDays as day (day.id)}
							<Select.Item value={day.id}>{formatDayDate(day.examDate, day.label)}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				{#if onRetry}
					<LoadingButton
						variant="outline"
						size="sm"
						{loading}
						loadingLabel="กำลังโหลด..."
						onclick={onRetry}
					>
						<RefreshCw class="h-4 w-4" />
						รีเฟรช
					</LoadingButton>
				{/if}
			</div>
		</div>

		{#if !selectedDay}
			<PageState title="ยังไม่มีวันสอบ" description="ต้องมีวันสอบก่อนจัดกรรมการคุมสอบ" />
		{:else}
			<div class="grid min-h-0 flex-1 gap-3 p-3 xl:grid-cols-[28rem_minmax(0,1fr)]">
				<InvigilatorStaffList
					staffCards={displayedStaffCards}
					search={staffSearch}
					{showAvailableOnly}
					{readonly}
					{pendingStaffIds}
					{activeDragStaffId}
					onSearchChange={(value) => (staffSearch = value)}
					onShowAvailableOnlyChange={(value) => (showAvailableOnly = value)}
					onStaffDragStart={handleStaffDragStart}
					onStaffDragEnd={handleStaffDragEnd}
				/>
				<InvigilatorRoomBoard
					assignments={selectedDayAssignments}
					{readonly}
					{pendingAssignmentIds}
					{pendingStaffIds}
					{activeDragStaffId}
					onAssignInvigilator={assignInvigilator}
					onRemoveInvigilator={removeInvigilator}
				/>
			</div>
		{/if}
	</section>
{/if}
