<script lang="ts">
	import type {
		TimetablePlacementCandidate,
		TimetablePlacementSource,
		TimetableUnscheduledDemand,
		TimetableWorkspaceLearningGroup,
		TimetableWorkspaceStaff
	} from '$lib/api/timetable';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { ArrowRight, GripVertical, Inbox, Users } from 'lucide-svelte';

	type DemandSelection = {
		source: TimetablePlacementSource;
		candidate: TimetablePlacementCandidate;
	};

	let {
		demands,
		groups,
		staff,
		disabled = false,
		remainingForGroup,
		onChooseDemand,
		onDragStartDemand,
		onCancelDrag
	}: {
		demands: TimetableUnscheduledDemand[];
		groups: TimetableWorkspaceLearningGroup[];
		staff: TimetableWorkspaceStaff[];
		disabled?: boolean;
		remainingForGroup: (groupId: string) => number;
		onChooseDemand: (
			source: TimetablePlacementSource,
			candidate: TimetablePlacementCandidate
		) => void;
		onDragStartDemand?: (
			source: TimetablePlacementSource,
			candidate: TimetablePlacementCandidate,
			event: DragEvent
		) => void;
		onCancelDrag?: () => void;
	} = $props();

	const groupById = $derived(new Map(groups.map((group) => [group.id, group])));
	const staffById = $derived(new Map(staff.map((teacher) => [teacher.id, teacher])));
	const visibleDemands = $derived(
		demands.filter((demand) => remainingForGroup(demand.learningGroupId) > 0)
	);

	function selectionFor(demand: TimetableUnscheduledDemand): DemandSelection | null {
		const group = groupById.get(demand.learningGroupId);
		if (!group) return null;
		const instructorIds =
			demand.eligibleInstructorIds.length === 1 ? [...demand.eligibleInstructorIds] : [];
		return {
			source: {
				kind: 'unscheduled_demand',
				learningGroupId: demand.learningGroupId,
				learningOfferingId: demand.learningOfferingId
			},
			candidate: {
				entryType: group.offeringKind === 'activity' ? 'ACTIVITY' : 'COURSE',
				learningGroupId: group.id,
				learningOfferingId: group.learningOfferingId,
				homeroomId: null,
				roomId: null,
				instructorIds
			}
		};
	}

	function choose(demand: TimetableUnscheduledDemand): void {
		const selection = selectionFor(demand);
		if (selection) onChooseDemand(selection.source, selection.candidate);
	}

	function dragStart(demand: TimetableUnscheduledDemand, event: DragEvent): void {
		const selection = selectionFor(demand);
		if (!selection || selection.candidate.instructorIds.length !== 1) {
			event.preventDefault();
			return;
		}
		event.dataTransfer?.setData('text/plain', demand.learningGroupId);
		if (event.dataTransfer) event.dataTransfer.effectAllowed = 'copy';
		onDragStartDemand?.(selection.source, selection.candidate, event);
	}
</script>

<aside class="rounded-xl border bg-background" aria-label="คาบที่ยังไม่ได้จัด">
	<div class="flex items-center justify-between border-b px-4 py-3">
		<div>
			<h2 class="font-semibold">คาบที่รอจัด</h2>
			<p class="text-xs text-muted-foreground">หยิบครั้งละ 1 คาบลงในสมุดตาราง</p>
		</div>
		<Badge variant="secondary">{visibleDemands.length} กลุ่ม</Badge>
	</div>

	{#if visibleDemands.length === 0}
		<div class="flex flex-col items-center gap-2 px-5 py-10 text-center text-muted-foreground">
			<Inbox class="size-7" />
			<p class="text-sm font-medium text-foreground">จัดครบตามเป้าหมายแล้ว</p>
			<p class="text-xs">หากเป้าหมายเปลี่ยน ให้ปรับจำนวนคาบจากหน้าจัดการเรียนก่อน</p>
		</div>
	{:else}
		<div class="max-h-[34rem] space-y-2 overflow-y-auto p-3">
			{#each visibleDemands as demand (demand.learningGroupId)}
				{@const group = groupById.get(demand.learningGroupId)}
				{@const remaining = remainingForGroup(demand.learningGroupId)}
				<article
					data-timetable-lesson-card
					draggable={!disabled && demand.eligibleInstructorIds.length === 1}
					class="rounded-lg border border-l-4 border-l-primary bg-muted/15 p-3"
					aria-label={`${demand.offeringCode} ${demand.offeringName} เหลือ ${remaining} คาบ`}
					ondragstart={(event) => dragStart(demand, event)}
					ondragend={() => onCancelDrag?.()}
				>
					<div class="flex items-start justify-between gap-3">
						<div class="flex min-w-0 gap-1.5">
							{#if demand.eligibleInstructorIds.length === 1}
								<span data-timetable-drag-handle="true" aria-hidden="true">
									<GripVertical class="mt-0.5 size-4 text-muted-foreground" />
								</span>
							{/if}
							<div class="min-w-0">
								<p class="truncate font-mono text-xs font-semibold text-primary">
									{demand.offeringCode}
								</p>
								<h3 class="line-clamp-2 text-sm font-medium">{demand.offeringName}</h3>
								<p class="mt-1 truncate text-xs text-muted-foreground">
									{group?.code ?? demand.learningGroupId}
								</p>
							</div>
						</div>
						<Badge variant="outline">เหลือ {remaining}/{demand.requiredPeriods}</Badge>
					</div>
					<div class="mt-3 flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
						<Users class="size-3.5" />
						{#if demand.eligibleInstructorIds.length === 0}
							<span class="text-destructive">ยังไม่ได้กำหนดครู</span>
						{:else}
							{#each demand.eligibleInstructorIds as teacherId (teacherId)}
								<span class="rounded-full bg-muted px-2 py-0.5">
									{staffById.get(teacherId)?.displayName ?? 'ครูที่อ้างอิง'}
								</span>
							{/each}
						{/if}
					</div>
					<Button
						type="button"
						size="sm"
						variant="outline"
						class="mt-3 w-full justify-between"
						disabled={disabled || demand.eligibleInstructorIds.length === 0}
						onclick={() => choose(demand)}
					>
						{demand.eligibleInstructorIds.length > 1 ? 'เลือกครูและคาบ' : 'เลือกคาบนี้'}
						<ArrowRight class="size-3.5" />
					</Button>
				</article>
			{/each}
		</div>
	{/if}
</aside>
