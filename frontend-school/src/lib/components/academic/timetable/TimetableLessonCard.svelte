<script lang="ts">
	import { buildTimetableBlockDisplay } from '$lib/academic/timetable/block-display';
	import type { TimetableBlock } from '$lib/api/timetable';
	import { Badge } from '$lib/components/ui/badge';
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
	const allTeacherNames = $derived(
		[
			...block.groups.flatMap((group) => group.instructors.map((teacher) => teacher.displayName)),
			...block.teachers.map((teacher) => teacher.displayName)
		].filter((name, index, names) => names.indexOf(name) === index)
	);
	const allTargetNames = $derived(
		[
			...block.groups.map((group) => group.name),
			...block.homerooms.map((homeroom) => homeroom.name)
		].filter((name, index, names) => names.indexOf(name) === index)
	);
	const display = $derived(buildTimetableBlockDisplay(block, 'scheduler'));
	const accessibleLabel = $derived(
		`${code} ${title} ${allTargetNames.join(', ')} ครู ${allTeacherNames.join(', ') || 'ยังไม่ระบุ'}`
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
			{#if display.contextLabel || display.scopeLabel}
				<div class="mt-1 flex flex-wrap gap-1">
					{#if display.contextLabel}
						<Badge variant="outline" class="h-5 px-1.5 text-[0.62rem] font-medium">
							{display.contextLabel}
						</Badge>
					{/if}
					{#if display.scopeLabel}
						<Badge variant="secondary" class="h-5 px-1.5 text-[0.62rem] font-medium">
							{display.scopeLabel}
						</Badge>
					{/if}
				</div>
			{:else if display.groupLabel}
				<p class="mt-0.5 truncate text-[0.68rem] text-muted-foreground">
					{display.groupLabel}
				</p>
			{/if}
		</button>
	</div>
	<div class="mt-2 space-y-1 text-[0.68rem] text-muted-foreground">
		<p class="flex min-w-0 items-center gap-1.5">
			<Users class="size-3 shrink-0" />
			<span class="line-clamp-2">{display.teacherLabel ?? 'ยังไม่ระบุครู'}</span>
		</p>
		{#if display.roomLabel}
			<p class="flex items-center gap-1.5">
				<DoorOpen class="size-3 shrink-0" />
				<span class="truncate">{display.roomLabel}</span>
			</p>
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
