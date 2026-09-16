<script lang="ts">
	import { buildTimetableBlockDisplay } from '$lib/academic/timetable/block-display';
	import type { TimetableBlock } from '$lib/api/timetable';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { DoorOpen, GripVertical, Trash2, Users } from 'lucide-svelte';

	let {
		block,
		rowId,
		targetLabel,
		selected = false,
		canEdit = false,
		onSelect,
		onDragStart,
		onDragEnd,
		onRemove
	}: {
		block: TimetableBlock;
		rowId: string;
		targetLabel?: string | null;
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
	const resolvedTargetLabel = $derived(
		targetLabel === undefined ? display.groupLabel : targetLabel
	);
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
		'group flex min-h-full w-full min-w-0 max-w-full flex-col overflow-hidden rounded-lg border bg-background p-1.5 text-left shadow-xs transition',
		canEdit && 'cursor-grab active:cursor-grabbing',
		selected ? 'border-primary ring-2 ring-primary/20' : 'hover:border-primary/45',
		block.blockKind === 'activity' && 'border-l-4 border-l-violet-500',
		block.blockKind === 'structural' && 'border-l-4 border-l-amber-500'
	]}
	aria-label={accessibleLabel}
	ondragstart={dragStart}
	ondragend={() => onDragEnd?.()}
>
	<div class="flex min-w-0 items-start gap-1">
		{#if canEdit}
			<span data-timetable-drag-handle="true" aria-hidden="true">
				<GripVertical class="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
			</span>
		{/if}
		<button
			type="button"
			class="min-w-0 flex-1 text-left"
			aria-label={`ดูรายละเอียด ${accessibleLabel}`}
			onclick={() => onSelect?.(block)}
		>
			<p
				data-timetable-card-line
				class="truncate font-mono text-[0.56rem] font-semibold leading-3 text-primary"
			>
				{code}
			</p>
			<h4 data-timetable-card-line class="truncate text-[0.62rem] font-semibold leading-3">
				{title}
			</h4>
			{#if display.contextLabel || display.scopeLabel}
				<div class="mt-0.5 flex min-w-0 flex-nowrap gap-0.5 overflow-hidden">
					{#if display.contextLabel}
						<Badge variant="outline" class="h-4 min-w-0 truncate px-1 text-[0.54rem] font-medium">
							{display.contextLabel}
						</Badge>
					{/if}
					{#if display.scopeLabel}
						<Badge variant="secondary" class="h-4 min-w-0 truncate px-1 text-[0.54rem] font-medium">
							{display.scopeLabel}
						</Badge>
					{/if}
				</div>
			{:else if resolvedTargetLabel}
				<p
					data-timetable-card-line
					class="mt-0.5 truncate text-[0.58rem] leading-3 text-muted-foreground"
				>
					{resolvedTargetLabel}
				</p>
			{/if}
		</button>
	</div>
	<div class="mt-1 min-w-0 space-y-0.5 text-[0.58rem] leading-3 text-muted-foreground">
		<p class="flex min-w-0 items-center gap-1.5">
			<Users class="size-3 shrink-0" />
			<span data-timetable-card-line class="min-w-0 truncate"
				>{display.teacherLabel ?? 'ยังไม่ระบุครู'}</span
			>
		</p>
		{#if display.roomLabel}
			<p class="flex min-w-0 items-center gap-1.5">
				<DoorOpen class="size-3 shrink-0" />
				<span data-timetable-card-line class="min-w-0 truncate">{display.roomLabel}</span>
			</p>
		{/if}
	</div>
	{#if canEdit}
		<div class="mt-auto flex justify-end border-t pt-0.5">
			<Button
				type="button"
				size="icon"
				variant="ghost"
				class="size-6 text-destructive"
				aria-label={`นำ ${title} ออกจากตาราง`}
				onclick={(event) => {
					event.stopPropagation();
					onRemove?.(block);
				}}
			>
				<Trash2 class="size-3" />
			</Button>
		</div>
	{/if}
</article>
