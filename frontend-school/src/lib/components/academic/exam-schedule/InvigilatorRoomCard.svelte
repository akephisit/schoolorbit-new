<script lang="ts">
	import type { ExamInvigilatorAssignmentSummary } from '$lib/api/examSchedule';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { X } from 'lucide-svelte';
	import { INVIGILATOR_STAFF_DRAG_TYPE } from './invigilatorDrag';

	let {
		assignment,
		readonly = false,
		pendingAssignmentIds = [],
		pendingStaffIds = [],
		onAssignInvigilator,
		onRemoveInvigilator
	}: {
		assignment: ExamInvigilatorAssignmentSummary;
		readonly?: boolean;
		pendingAssignmentIds?: string[];
		pendingStaffIds?: string[];
		onAssignInvigilator?: (assignmentId: string, staffId: string) => Promise<void> | void;
		onRemoveInvigilator?: (assignmentId: string, staffId: string) => Promise<void> | void;
	} = $props();

	let dragOver = $state(false);

	const isSaving = $derived(pendingAssignmentIds.includes(assignment.assignmentId));

	function staffIdFromDrag(event: DragEvent): string {
		return event.dataTransfer?.getData(INVIGILATOR_STAFF_DRAG_TYPE) ?? '';
	}

	function hasStaffDragPayload(event: DragEvent): boolean {
		return event.dataTransfer?.types.includes(INVIGILATOR_STAFF_DRAG_TYPE) ?? false;
	}

	function handleDragOver(event: DragEvent) {
		if (readonly) return;
		if (!hasStaffDragPayload(event)) return;

		event.preventDefault();
		dragOver = true;
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'move';
		}
	}

	function handleDrop(event: DragEvent) {
		if (readonly) return;
		event.preventDefault();
		dragOver = false;

		const staffId = staffIdFromDrag(event);
		if (!staffId) return;
		if (assignment.invigilators.some((invigilator) => invigilator.staffId === staffId)) return;

		void onAssignInvigilator?.(assignment.assignmentId, staffId);
	}
</script>

<article
	class="min-h-36 rounded-md border bg-background p-3 transition {dragOver
		? 'border-primary ring-2 ring-primary/20'
		: ''} {isSaving ? 'opacity-70' : ''}"
	ondragenter={(event) => handleDragOver(event)}
	ondragover={handleDragOver}
	ondragleave={() => (dragOver = false)}
	ondrop={handleDrop}
>
	<div class="flex items-start justify-between gap-3">
		<div class="min-w-0">
			<h3 class="truncate text-sm font-semibold">{assignment.classroomName || '-'}</h3>
			<p class="truncate text-xs text-muted-foreground">{assignment.roomName || '-'}</p>
		</div>
		<Badge variant="outline">กรรมการ {assignment.invigilators.length} คน</Badge>
	</div>

	<div class="mt-3 flex min-h-16 flex-wrap content-start gap-2 rounded-md border border-dashed p-2">
		{#if assignment.invigilators.length === 0}
			<p class="text-xs text-muted-foreground">ลากครูมาวางตรงนี้</p>
		{:else}
			{#each assignment.invigilators as invigilator (invigilator.staffId)}
				<Badge variant="secondary" class="gap-1 pr-1">
					<span>{invigilator.displayName}</span>
					{#if !readonly}
						<Button
							type="button"
							variant="ghost"
							size="icon"
							class="h-5 w-5"
							disabled={pendingStaffIds.includes(invigilator.staffId)}
							onclick={() =>
								void onRemoveInvigilator?.(assignment.assignmentId, invigilator.staffId)}
							aria-label={`เอา ${invigilator.displayName} ออกจากห้อง ${assignment.classroomName}`}
						>
							<X class="h-3 w-3" />
						</Button>
					{/if}
				</Badge>
			{/each}
		{/if}
	</div>
</article>
