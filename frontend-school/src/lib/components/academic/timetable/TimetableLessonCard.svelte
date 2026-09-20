<script lang="ts">
	import { buildTimetableBlockDisplay } from '$lib/academic/timetable/block-display';
	import { alignDragImageToPointer } from '$lib/academic/timetable/drag-image';
	import type { TimetableBlock } from '$lib/api/timetable';
	import { Button } from '$lib/components/ui/button';
	import { DoorOpen, LoaderCircle, Trash2, Users } from '@lucide/svelte';

	let {
		block,
		rowId,
		targetLabel,
		showTeacher = true,
		selected = false,
		canEdit = false,
		pending = false,
		onSelect,
		onDragStart,
		onDragEnd,
		onRemove
	}: {
		block: TimetableBlock;
		rowId: string;
		targetLabel?: string | null;
		showTeacher?: boolean;
		selected?: boolean;
		canEdit?: boolean;
		pending?: boolean;
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
	const showCode = $derived(block.blockKind === 'course');
	const shouldShowTeacher = $derived(
		showTeacher && block.blockKind !== 'structural' && block.schedulingMode !== 'synchronized'
	);
	const resolvedTargetLabel = $derived(
		targetLabel === undefined ? display.groupLabel : targetLabel
	);
	const accessibleLabel = $derived(
		[
			showCode ? code : null,
			title,
			allTargetNames.join(', '),
			shouldShowTeacher ? `ครู ${allTeacherNames.join(', ') || 'ยังไม่ระบุ'}` : null
		]
			.filter(Boolean)
			.join(' ')
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
		if (event.currentTarget instanceof HTMLElement) {
			alignDragImageToPointer(event, event.currentTarget);
		}
		onDragStart?.(block, event);
	}
</script>

<article
	data-timetable-lesson-card
	data-block-id={block.id}
	data-row-id={rowId}
	data-pending={pending ? 'true' : undefined}
	draggable={canEdit}
	class={[
		'group relative flex min-h-full w-full min-w-0 max-w-full flex-col overflow-hidden rounded-lg border bg-background p-1.5 text-left shadow-xs transition',
		canEdit && 'cursor-grab active:cursor-grabbing',
		selected ? 'border-primary ring-2 ring-primary/20' : 'hover:border-primary/45'
	]}
	aria-label={accessibleLabel}
	aria-busy={pending}
	ondragstart={dragStart}
	ondragend={() => onDragEnd?.()}
>
	<div class="flex min-w-0 items-start">
		<button
			type="button"
			class="min-w-0 flex-1 text-left"
			aria-label={`ดูรายละเอียด ${accessibleLabel}`}
			disabled={pending}
			onclick={() => onSelect?.(block)}
		>
			{#if showCode}
				<p
					data-timetable-card-line
					class="truncate font-mono text-[8px] leading-[14px] font-semibold text-primary"
				>
					{code}
				</p>
			{/if}
			<h4
				data-timetable-card-line
				data-timetable-card-title
				class={[
					'text-[9px] leading-[14px] font-semibold',
					block.blockKind === 'structural' ? 'line-clamp-3 whitespace-pre-line' : 'truncate'
				]}
			>
				{title}
			</h4>
			{#if resolvedTargetLabel}
				<p
					data-timetable-card-line
					class="mt-1 truncate text-[8px] leading-[14px] text-muted-foreground"
				>
					{resolvedTargetLabel}
				</p>
			{/if}
		</button>
	</div>
	<div
		class={[
			'mt-1 min-w-0 space-y-1 text-[8px] leading-[14px] text-muted-foreground',
			canEdit && 'pr-5'
		]}
	>
		{#if shouldShowTeacher}
			<p class="flex min-w-0 items-center gap-1.5">
				<Users class="size-3 shrink-0" />
				<span data-timetable-card-line class="min-w-0 truncate"
					>{display.teacherLabel ?? 'ยังไม่ระบุครู'}</span
				>
			</p>
		{/if}
		{#if display.roomLabel}
			<p class="flex min-w-0 items-center gap-1.5">
				<DoorOpen class="size-3 shrink-0" />
				<span data-timetable-card-line class="min-w-0 truncate">{display.roomLabel}</span>
			</p>
		{/if}
	</div>
	{#if canEdit}
		<Button
			type="button"
			size="icon"
			variant="ghost"
			class="absolute right-0.5 bottom-0.5 z-10 size-6 text-destructive"
			aria-label={`นำ ${title} ออกจากตาราง`}
			onclick={(event) => {
				event.stopPropagation();
				onRemove?.(block);
			}}
		>
			<Trash2 class="size-3" />
		</Button>
	{/if}
	{#if pending}
		<span
			class="absolute right-1 top-1 z-10 grid size-5 place-items-center rounded-full bg-background/90 text-primary shadow-sm"
			aria-label={`กำลังบันทึกคาบ ${title}`}
		>
			<LoaderCircle class="size-3.5 animate-spin" />
		</span>
	{/if}
</article>
