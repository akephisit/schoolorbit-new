<script lang="ts">
	import type { ExamInvigilatorAssignmentSummary } from '$lib/api/examSchedule';
	import { PageState } from '$lib/components/app-state';
	import InvigilatorRoomCard from './InvigilatorRoomCard.svelte';

	let {
		assignments = [],
		readonly = false,
		pendingAssignmentIds = [],
		pendingStaffIds = [],
		activeDragStaffId = null,
		onAssignInvigilator,
		onRemoveInvigilator
	}: {
		assignments: ExamInvigilatorAssignmentSummary[];
		readonly?: boolean;
		pendingAssignmentIds?: string[];
		pendingStaffIds?: string[];
		activeDragStaffId?: string | null;
		onAssignInvigilator?: (assignmentId: string, staffId: string) => void;
		onRemoveInvigilator?: (assignmentId: string, staffId: string) => void;
	} = $props();
</script>

<section class="flex h-full min-h-0 flex-col rounded-md border bg-background">
	<div class="border-b p-3">
		<h3 class="text-sm font-semibold">ห้องสอบ</h3>
		<p class="text-xs text-muted-foreground">{assignments.length} ห้องในวันที่เลือก</p>
	</div>

	<div class="min-h-0 flex-1 overflow-y-auto p-3">
		{#if assignments.length === 0}
			<PageState
				title="ยังไม่มีห้องสอบในวันนี้"
				description="กำหนดห้องสอบในแท็บห้องสอบก่อนจัดกรรมการ"
			/>
		{:else}
			<div class="grid gap-3 md:grid-cols-2 2xl:grid-cols-3">
				{#each assignments as assignment (assignment.assignmentId)}
					<InvigilatorRoomCard
						{assignment}
						{readonly}
						{pendingAssignmentIds}
						{pendingStaffIds}
						{activeDragStaffId}
						{onAssignInvigilator}
						{onRemoveInvigilator}
					/>
				{/each}
			</div>
		{/if}
	</div>
</section>
