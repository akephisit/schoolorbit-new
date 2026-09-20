<script lang="ts">
	import type { TimetableBlockWorkspaceStaff } from '$lib/api/timetable';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import * as Popover from '$lib/components/ui/popover';
	import { Check, ChevronDown, LockKeyhole, UsersRound } from '@lucide/svelte';

	let {
		staff,
		value = [],
		lockedIds = [],
		disabled = false,
		label = 'ครูที่กันเวลาไว้',
		showLabel = true,
		onValueChange
	}: {
		staff: TimetableBlockWorkspaceStaff[];
		value?: string[];
		lockedIds?: string[];
		disabled?: boolean;
		label?: string;
		showLabel?: boolean;
		onValueChange: (value: string[]) => void;
	} = $props();

	const selectedIds = $derived(
		value.filter(
			(id, index, values) => values.indexOf(id) === index && staff.some((item) => item.id === id)
		)
	);
	const selectedSet = $derived(new Set(selectedIds));
	const lockedSet = $derived(new Set(lockedIds));
	const allSelected = $derived(staff.length > 0 && selectedIds.length === staff.length);

	function commit(nextIds: string[]): void {
		const retainedLocked = selectedIds.filter((id) => lockedSet.has(id));
		onValueChange(
			[...nextIds, ...retainedLocked].filter(
				(id, index, values) => values.indexOf(id) === index && staff.some((item) => item.id === id)
			)
		);
	}

	function toggle(teacherId: string): void {
		if (lockedSet.has(teacherId)) return;
		commit(
			selectedSet.has(teacherId)
				? selectedIds.filter((id) => id !== teacherId)
				: [...selectedIds, teacherId]
		);
	}

	function summary(): string {
		if (allSelected) return 'ครูทุกคน';
		if (selectedIds.length === 0) return 'ยังไม่กำหนดครู';
		return `เลือกครู ${selectedIds.length} คน`;
	}
</script>

<div class:space-y-1.5={showLabel}>
	{#if showLabel}<Label>{label}</Label>{/if}
	<Popover.Root>
		<Popover.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					type="button"
					variant="outline"
					class="w-full justify-between"
					{disabled}
				>
					<span class="flex min-w-0 items-center gap-1.5">
						<UsersRound class="size-3.5" />
						<span class="truncate">{summary()}</span>
					</span>
					<ChevronDown class="size-3.5" />
				</Button>
			{/snippet}
		</Popover.Trigger>
		<Popover.Content class="w-72 p-2" align="start">
			<div class="grid grid-cols-2 gap-1 border-b pb-2">
				<Button type="button" size="sm" variant="ghost" onclick={() => commit([])}>
					ยังไม่กำหนดครู
				</Button>
				<Button
					type="button"
					size="sm"
					variant="ghost"
					disabled={staff.length === 0}
					onclick={() => commit(staff.map((teacher) => teacher.id))}
				>
					ครูทุกคน
				</Button>
			</div>
			{#if staff.length === 0}
				<p class="px-2 py-3 text-xs text-muted-foreground">ยังไม่มีรายชื่อครูในภาคเรียนนี้</p>
			{:else}
				<div class="max-h-72 overflow-y-auto pt-1">
					{#each staff as teacher (teacher.id)}
						<Button
							type="button"
							variant="ghost"
							class="h-auto w-full justify-start gap-2 px-2 py-2 text-left"
							disabled={lockedSet.has(teacher.id)}
							aria-pressed={selectedSet.has(teacher.id)}
							onclick={() => toggle(teacher.id)}
						>
							<span
								class={[
									'flex size-4 shrink-0 items-center justify-center rounded border',
									selectedSet.has(teacher.id) && 'border-primary bg-primary text-primary-foreground'
								]}
							>
								{#if selectedSet.has(teacher.id)}<Check class="size-3" />{/if}
							</span>
							<span class="min-w-0 flex-1 truncate text-xs font-medium">{teacher.displayName}</span>
							{#if lockedSet.has(teacher.id)}
								<span class="flex items-center gap-1 text-[0.68rem] text-muted-foreground">
									<LockKeyhole class="size-3" /> ครูประจำกลุ่ม
								</span>
							{/if}
						</Button>
					{/each}
				</div>
			{/if}
		</Popover.Content>
	</Popover.Root>
</div>
