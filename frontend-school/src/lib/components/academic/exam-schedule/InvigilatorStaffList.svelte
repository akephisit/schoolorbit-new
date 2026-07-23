<script lang="ts">
	import { PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import { GripVertical } from 'lucide-svelte';
	import {
		formatInvigilatorMinutes,
		INVIGILATOR_STAFF_DRAG_TYPE,
		workloadLevel,
		type InvigilatorStaffCardView
	} from './invigilatorDrag';

	let {
		staffCards = [],
		search = '',
		showAvailableOnly = false,
		readonly = false,
		pendingStaffIds = [],
		activeDragStaffId = null,
		onSearchChange,
		onShowAvailableOnlyChange,
		onStaffDragStart,
		onStaffDragEnd
	}: {
		staffCards: InvigilatorStaffCardView[];
		search?: string;
		showAvailableOnly?: boolean;
		readonly?: boolean;
		pendingStaffIds?: string[];
		activeDragStaffId?: string | null;
		onSearchChange?: (value: string) => void;
		onShowAvailableOnlyChange?: (value: boolean) => void;
		onStaffDragStart?: (staffId: string) => void;
		onStaffDragEnd?: () => void;
	} = $props();

	function canDrag(staffId: string): boolean {
		return !readonly && !pendingStaffIds.includes(staffId);
	}

	function handleDragStart(event: DragEvent, staffId: string) {
		if (!canDrag(staffId) || !event.dataTransfer) {
			event.preventDefault();
			return;
		}

		onStaffDragStart?.(staffId);
		event.dataTransfer.effectAllowed = 'move';
		event.dataTransfer.setData(INVIGILATOR_STAFF_DRAG_TYPE, staffId);
		event.dataTransfer.setData('text/plain', staffId);
	}

	function handleDragEnd() {
		onStaffDragEnd?.();
	}

	function rowToneClass(staff: InvigilatorStaffCardView): string {
		const pending = pendingStaffIds.includes(staff.staffId) ? 'opacity-60' : '';
		const active = activeDragStaffId === staff.staffId ? 'ring-2 ring-primary/30' : '';

		switch (workloadLevel(staff)) {
			case 'heavy':
				return `border-l-4 border-l-rose-500 bg-rose-50/70 dark:bg-rose-950/10 ${pending} ${active}`;
			case 'assigned':
				return `border-l-4 border-l-sky-500 bg-sky-50/70 dark:bg-sky-950/10 ${pending} ${active}`;
			default:
				return `border-l-4 border-l-emerald-500 bg-emerald-50/70 dark:bg-emerald-950/10 ${pending} ${active}`;
		}
	}

	function statusLabel(staff: InvigilatorStaffCardView): string {
		if (staff.assignedAssignment) return staff.assignedAssignment.classroomName || 'มีคุมวันนี้';
		return 'ว่างวันนี้';
	}

	function statusBadgeClass(staff: InvigilatorStaffCardView): string {
		switch (workloadLevel(staff)) {
			case 'heavy':
				return 'border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-900 dark:bg-rose-950/20 dark:text-rose-300';
			case 'assigned':
				return 'border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-900 dark:bg-sky-950/20 dark:text-sky-300';
			default:
				return 'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/20 dark:text-emerald-300';
		}
	}
</script>

<section class="flex h-full min-h-0 flex-col rounded-md border bg-background">
	<div class="space-y-3 border-b p-3">
		<div class="flex items-start justify-between gap-3">
			<div>
				<h3 class="text-sm font-semibold">ครู</h3>
				<p class="text-xs text-muted-foreground">{staffCards.length} คนในรายการ</p>
			</div>
			<Badge variant="outline" class="shrink-0">ลากทั้งแถว</Badge>
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

	<div class="min-h-0 flex-1 overflow-y-auto">
		{#if staffCards.length === 0}
			<div class="p-3">
				<PageState title="ไม่พบรายชื่อครู" description="ลองค้นหาด้วยคำอื่น" />
			</div>
		{:else}
			<Table class="min-w-[28rem] text-xs">
				<TableHeader>
					<TableRow>
						<TableHead class="w-8"></TableHead>
						<TableHead>ครู</TableHead>
						<TableHead class="w-20 text-right">วันนี้</TableHead>
						<TableHead class="w-24 text-right">รวมรอบนี้</TableHead>
						<TableHead class="w-24">สถานะ</TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{#each staffCards as staff (staff.staffId)}
						<TableRow
							class="cursor-grab active:cursor-grabbing {rowToneClass(staff)}"
							draggable={canDrag(staff.staffId)}
							ondragstart={(event) => handleDragStart(event, staff.staffId)}
							ondragend={handleDragEnd}
						>
							<TableCell class="w-8 text-muted-foreground">
								<GripVertical class="h-4 w-4" />
							</TableCell>
							<TableCell class="min-w-0">
								<span class="block max-w-36 truncate font-medium">{staff.displayName}</span>
							</TableCell>
							<TableCell class="text-right tabular-nums">
								{formatInvigilatorMinutes(staff.selectedDayMinutes)}
							</TableCell>
							<TableCell class="text-right tabular-nums">
								{formatInvigilatorMinutes(staff.totalMinutes)}
							</TableCell>
							<TableCell>
								<Badge
									variant="outline"
									class="max-w-24 justify-start truncate {statusBadgeClass(staff)}"
								>
									{statusLabel(staff)}
								</Badge>
							</TableCell>
						</TableRow>
					{/each}
				</TableBody>
			</Table>
		{/if}
	</div>
</section>
