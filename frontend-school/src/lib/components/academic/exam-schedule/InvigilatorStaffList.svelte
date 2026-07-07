<script lang="ts">
	import { PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import {
		formatInvigilatorMinutes,
		INVIGILATOR_STAFF_DRAG_TYPE,
		type InvigilatorStaffCardView
	} from './invigilatorDrag';

	let {
		staffCards = [],
		search = '',
		showAvailableOnly = false,
		readonly = false,
		pendingStaffIds = [],
		onSearchChange,
		onShowAvailableOnlyChange
	}: {
		staffCards: InvigilatorStaffCardView[];
		search?: string;
		showAvailableOnly?: boolean;
		readonly?: boolean;
		pendingStaffIds?: string[];
		onSearchChange?: (value: string) => void;
		onShowAvailableOnlyChange?: (value: boolean) => void;
	} = $props();

	function handleDragStart(event: DragEvent, staffId: string) {
		if (readonly || pendingStaffIds.includes(staffId)) {
			event.preventDefault();
			return;
		}

		event.dataTransfer?.setData(INVIGILATOR_STAFF_DRAG_TYPE, staffId);
		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = 'move';
		}
	}
</script>

<section class="flex min-h-0 flex-col rounded-md border bg-background">
	<div class="space-y-3 border-b p-3">
		<div>
			<h3 class="text-sm font-semibold">ครู</h3>
			<p class="text-xs text-muted-foreground">{staffCards.length} คนในรายการ</p>
		</div>
		<div class="grid gap-2">
			<Label for="exam-invigilator-search">ค้นหาครู</Label>
			<Input
				id="exam-invigilator-search"
				type="search"
				value={search}
				placeholder="ชื่อครู"
				oninput={(event) => onSearchChange?.(event.currentTarget.value)}
			/>
		</div>
		<label class="flex items-center gap-2 text-sm">
			<Checkbox
				checked={showAvailableOnly}
				onCheckedChange={(checked) => onShowAvailableOnlyChange?.(checked === true)}
			/>
			<span>แสดงเฉพาะครูว่างวันนี้</span>
		</label>
	</div>

	<div class="min-h-0 flex-1 overflow-y-auto p-3">
		{#if staffCards.length === 0}
			<PageState title="ไม่พบรายชื่อครู" description="ลองค้นหาด้วยคำอื่น" />
		{:else}
			<div class="space-y-2">
				{#each staffCards as staff (staff.staffId)}
					<article
						class="rounded-md border p-3 text-sm transition hover:border-primary/60 {pendingStaffIds.includes(
							staff.staffId
						)
							? 'opacity-60'
							: ''}"
						draggable={!readonly && !pendingStaffIds.includes(staff.staffId)}
						ondragstart={(event) => handleDragStart(event, staff.staffId)}
					>
						<div class="flex items-start justify-between gap-3">
							<div class="min-w-0">
								<p class="truncate font-medium">{staff.displayName}</p>
								<p class="mt-1 text-xs text-muted-foreground">
									วันนี้ {formatInvigilatorMinutes(staff.selectedDayMinutes)} · รวมรอบนี้
									{formatInvigilatorMinutes(staff.totalMinutes)}
								</p>
							</div>
							{#if staff.assignedAssignment}
								<Badge variant="outline" class="shrink-0">
									{staff.assignedAssignment.classroomName}
								</Badge>
							{:else}
								<Badge variant="secondary" class="shrink-0">ว่างวันนี้</Badge>
							{/if}
						</div>
					</article>
				{/each}
			</div>
		{/if}
	</div>
</section>
