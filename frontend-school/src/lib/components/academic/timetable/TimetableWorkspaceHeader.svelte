<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Timetable } from '#lib/api/timetable.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Check, LoaderCircle } from '@lucide/svelte';

	import type { TimetablePageView } from '#lib/academic/timetable/board-state.js';
	import TimetableViewSelector from './TimetableViewSelector.svelte';
	import TimetableCollaborators from './TimetableCollaborators.svelte';
	import type { TimetableCollaborator } from '#lib/academic/timetable/collaboration-presentation.js';

	let {
		timetable,
		view,
		isSaving = false,
		editing = false,
		collaborators = [],
		onJumpCollaborator,
		controls,
		onViewChange
	}: {
		timetable: Timetable;
		view: TimetablePageView;
		isSaving?: boolean;
		editing?: boolean;
		collaborators?: TimetableCollaborator[];
		onJumpCollaborator?: (connectionId: string) => void;
		controls: Snippet;
		onViewChange: (view: TimetablePageView) => void;
	} = $props();
</script>

<header
	class="overflow-hidden rounded-xl border bg-background shadow-sm"
	aria-label="บริบทตารางสอน"
	data-timetable-id={timetable.id}
>
	<div class="h-1 bg-primary"></div>
	<div class="flex flex-wrap items-end gap-3 p-3 sm:p-4">
		<div class="w-full min-w-0 space-y-2 sm:w-auto sm:min-w-72 sm:flex-1 sm:basis-72">
			<div class="flex min-h-8 flex-wrap items-center gap-2">
				<h2 class="text-lg font-semibold tracking-tight">ตารางสอนปัจจุบัน</h2>
				<Badge variant="secondary">{editing ? 'กำลังแก้ไข' : 'โหมดดู'}</Badge>
				{#if collaborators.length > 0}<TimetableCollaborators
						users={collaborators}
						onJump={(connectionId) => onJumpCollaborator?.(connectionId)}
					/>{/if}
			</div>
			<div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-muted-foreground">
				<span class="inline-flex items-center gap-1.5" aria-live="polite">
					{#if isSaving}<LoaderCircle class="size-4 animate-spin" /> กำลังบันทึก
					{:else}<Check class="size-4 text-emerald-600" /> {editing ? 'บันทึกแล้ว' : 'โหมดดู'}{/if}
				</span>
			</div>
		</div>
		<div class="w-full min-w-0 sm:w-auto sm:max-w-full sm:flex-none">{@render controls()}</div>
		<TimetableViewSelector value={view} {onViewChange} disabled={isSaving} />
	</div>
</header>
