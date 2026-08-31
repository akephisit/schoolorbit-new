<script lang="ts">
	import type { TimetableEntry } from '$lib/api/timetable';
	import { Button } from '$lib/components/ui/button';
	import { DoorOpen, GripVertical, Move, Pencil, Trash2, Users } from 'lucide-svelte';

	let {
		entry,
		rowId,
		selected = false,
		canEdit = false,
		onSelect,
		onDragStart,
		onDragEnd,
		onMove,
		onEdit,
		onRemove
	}: {
		entry: TimetableEntry;
		rowId: string;
		selected?: boolean;
		canEdit?: boolean;
		onSelect?: (entry: TimetableEntry) => void;
		onDragStart?: (entry: TimetableEntry, event: DragEvent) => void;
		onDragEnd?: () => void;
		onMove?: (entry: TimetableEntry) => void;
		onEdit?: (entry: TimetableEntry) => void;
		onRemove?: (entry: TimetableEntry) => void;
	} = $props();

	const title = $derived(entry.offeringName ?? entry.title ?? 'รายการตารางสอน');
	const code = $derived(entry.offeringCode ?? entry.entryType);
	const teacherNames = $derived(entry.instructors.map((teacher) => teacher.displayName));
	const accessibleLabel = $derived(
		`${code} ${title} ${entry.learningGroupName ?? ''} ครู ${teacherNames.join(', ') || 'ยังไม่ระบุ'}`
	);

	function dragStart(event: DragEvent): void {
		if (!canEdit) return;
		event.dataTransfer?.setData('text/plain', entry.id);
		if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
		onDragStart?.(entry, event);
	}
</script>

<article
	data-timetable-lesson-card
	data-entry-id={entry.id}
	data-row-id={rowId}
	draggable={canEdit}
	class={[
		'group rounded-lg border bg-background p-2.5 text-left shadow-xs transition',
		canEdit && 'cursor-grab active:cursor-grabbing',
		selected ? 'border-primary ring-2 ring-primary/20' : 'hover:border-primary/45'
	]}
	aria-label={accessibleLabel}
	ondragstart={dragStart}
	ondragend={() => onDragEnd?.()}
>
	<div class="flex items-start gap-2">
		{#if canEdit}
			<span data-timetable-drag-handle="true" aria-hidden="true">
				<GripVertical class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
			</span>
		{/if}
		<button
			type="button"
			class="min-w-0 flex-1 text-left"
			aria-label={`ดูรายละเอียด ${accessibleLabel}`}
			onclick={() => onSelect?.(entry)}
		>
			<p class="truncate font-mono text-[0.68rem] font-semibold text-primary">{code}</p>
			<h4 class="line-clamp-2 text-xs font-semibold leading-4">{title}</h4>
			{#if entry.learningGroupName}
				<p class="mt-0.5 truncate text-[0.68rem] text-muted-foreground">
					{entry.learningGroupName}
				</p>
			{/if}
		</button>
	</div>
	<div class="mt-2 space-y-1 text-[0.68rem] text-muted-foreground">
		<p class="flex items-center gap-1.5">
			<Users class="size-3" />
			{teacherNames.join(', ') || 'ยังไม่ระบุครู'}
		</p>
		{#if entry.roomCode}
			<p class="flex items-center gap-1.5"><DoorOpen class="size-3" /> {entry.roomCode}</p>
		{/if}
	</div>
	{#if canEdit}
		<div class="mt-2 flex flex-wrap gap-1 border-t pt-2">
			<Button
				size="sm"
				variant="ghost"
				class="h-7 px-2 text-[0.68rem]"
				onclick={(event) => {
					event.stopPropagation();
					onMove?.(entry);
				}}
			>
				<Move class="size-3" /> ย้ายคาบ
			</Button>
			<Button
				size="sm"
				variant="ghost"
				class="h-7 px-2 text-[0.68rem]"
				onclick={(event) => {
					event.stopPropagation();
					onEdit?.(entry);
				}}
			>
				<Pencil class="size-3" /> แก้รายละเอียด
			</Button>
			<Button
				size="sm"
				variant="ghost"
				class="h-7 px-2 text-[0.68rem] text-destructive"
				onclick={(event) => {
					event.stopPropagation();
					onRemove?.(entry);
				}}
			>
				<Trash2 class="size-3" /> นำออกจากตาราง
			</Button>
		</div>
	{/if}
</article>
