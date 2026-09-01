<script lang="ts">
	import type { TimetableBlock } from '$lib/api/timetable';
	import { Button } from '$lib/components/ui/button';
	import { DoorOpen, GripVertical, Trash2, Users } from 'lucide-svelte';

	let {
		block,
		rowId,
		selected = false,
		canEdit = false,
		onSelect,
		onDragStart,
		onDragEnd,
		onRemove
	}: {
		block: TimetableBlock;
		rowId: string;
		selected?: boolean;
		canEdit?: boolean;
		onSelect?: (block: TimetableBlock) => void;
		onDragStart?: (block: TimetableBlock, event: DragEvent) => void;
		onDragEnd?: () => void;
		onRemove?: (block: TimetableBlock) => void;
	} = $props();

	const title = $derived(block.offeringName ?? block.title ?? 'รายการตารางสอน');
	const code = $derived(block.offeringCode ?? structuralLabel(block.structuralKind));
	const teacherNames = $derived(
		[
			...block.groups.flatMap((group) => group.instructors.map((teacher) => teacher.displayName)),
			...block.teachers.map((teacher) => teacher.displayName)
		].filter((name, index, names) => names.indexOf(name) === index)
	);
	const groupNames = $derived(block.groups.map((group) => group.name));
	const roomCodes = $derived(
		[
			...block.groups.map((group) => group.roomCode),
			...block.homerooms.map((homeroom) => homeroom.roomCode)
		].filter((room): room is string => Boolean(room))
	);
	const accessibleLabel = $derived(
		`${code} ${title} ${groupNames.join(', ')} ครู ${teacherNames.join(', ') || 'ยังไม่ระบุ'}`
	);

	function structuralLabel(kind: TimetableBlock['structuralKind']): string {
		if (kind === 'flag_ceremony') return 'กิจกรรมหน้าเสาธง';
		if (kind === 'homeroom') return 'โฮมรูม';
		if (kind === 'teacher_meeting') return 'ประชุมครู';
		if (kind === 'break') return 'พัก';
		return kind === 'academic' ? 'กิจกรรมวิชาการ' : 'คาบพิเศษ';
	}

	function dragStart(event: DragEvent): void {
		if (!canEdit) return;
		event.dataTransfer?.setData('text/plain', block.id);
		if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
		onDragStart?.(block, event);
	}
</script>

<article
	data-timetable-lesson-card
	data-block-id={block.id}
	data-row-id={rowId}
	draggable={canEdit}
	class={[
		'group rounded-lg border bg-background p-2.5 text-left shadow-xs transition',
		canEdit && 'cursor-grab active:cursor-grabbing',
		selected ? 'border-primary ring-2 ring-primary/20' : 'hover:border-primary/45',
		block.blockKind === 'activity' && 'border-l-4 border-l-violet-500',
		block.blockKind === 'structural' && 'border-l-4 border-l-amber-500'
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
			onclick={() => onSelect?.(block)}
		>
			<p class="truncate font-mono text-[0.68rem] font-semibold text-primary">{code}</p>
			<h4 class="line-clamp-2 text-xs font-semibold leading-4">{title}</h4>
			{#if groupNames.length > 0}
				<p class="mt-0.5 truncate text-[0.68rem] text-muted-foreground">
					{groupNames.join(', ')}
				</p>
			{/if}
		</button>
	</div>
	<div class="mt-2 space-y-1 text-[0.68rem] text-muted-foreground">
		<p class="flex items-center gap-1.5">
			<Users class="size-3" />
			{#if teacherNames.length > 1}
				<span class="font-medium text-primary">ครูร่วมสอน</span>
			{/if}
			{teacherNames.join(', ') || 'ยังไม่ระบุครู'}
		</p>
		{#if roomCodes.length > 0}
			<p class="flex items-center gap-1.5"><DoorOpen class="size-3" /> {roomCodes.join(', ')}</p>
		{/if}
	</div>
	{#if canEdit}
		<div class="mt-2 flex justify-end border-t pt-1">
			<Button
				type="button"
				size="icon"
				variant="ghost"
				class="size-7 text-destructive"
				aria-label={`นำ ${title} ออกจากตาราง`}
				onclick={(event) => {
					event.stopPropagation();
					onRemove?.(block);
				}}
			>
				<Trash2 class="size-3.5" />
			</Button>
		</div>
	{/if}
</article>
