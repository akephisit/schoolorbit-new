<script lang="ts">
	import type { TimetableWorkspaceStaff } from '$lib/api/timetable';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Command from '$lib/components/ui/command';
	import * as Popover from '$lib/components/ui/popover';
	import { cn } from '$lib/utils';
	import { Check, ChevronsUpDown, UserRound } from 'lucide-svelte';

	let {
		teachers,
		selectedTeacherId,
		periodCount,
		disabled = false,
		onSelect
	}: {
		teachers: TimetableWorkspaceStaff[];
		selectedTeacherId: string | null;
		periodCount: (teacherId: string) => number;
		disabled?: boolean;
		onSelect: (teacherId: string) => void;
	} = $props();

	let open = $state(false);
	let search = $state('');
	const selectedTeacher = $derived(
		teachers.find((teacher) => teacher.id === selectedTeacherId) ?? null
	);
	const filteredTeachers = $derived.by(() => {
		const query = search.trim().toLocaleLowerCase('th-TH');
		if (!query) return teachers;
		return teachers.filter((teacher) =>
			teacher.displayName.toLocaleLowerCase('th-TH').includes(query)
		);
	});

	function selectTeacher(teacherId: string): void {
		onSelect(teacherId);
		open = false;
		search = '';
	}
</script>

<section class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
	<div class="space-y-1.5">
		<p class="text-xs font-medium text-muted-foreground">ครูผู้สอน</p>
		<Popover.Root bind:open>
			<Popover.Trigger>
				{#snippet child({ props })}
					<Button
						type="button"
						variant="outline"
						role="combobox"
						aria-label="เลือกครูผู้สอน"
						aria-expanded={open}
						class="w-full justify-between bg-background font-normal"
						{disabled}
						{...props}
					>
						<span class={cn('truncate', !selectedTeacher && 'text-muted-foreground')}>
							{selectedTeacher?.displayName ?? 'ค้นหาและเลือกครู'}
						</span>
						<ChevronsUpDown class="ms-2 size-4 shrink-0 opacity-50" />
					</Button>
				{/snippet}
			</Popover.Trigger>
			<Popover.Content class="w-[--bits-popover-trigger-width] p-0" align="start">
				<Command.Root shouldFilter={false}>
					<Command.Input bind:value={search} placeholder="ค้นหาชื่อครู..." />
					<Command.List class="max-h-72">
						{#if filteredTeachers.length === 0}
							<Command.Empty>ไม่พบครู</Command.Empty>
						{:else}
							<Command.Group>
								{#each filteredTeachers as teacher (teacher.id)}
									<Command.Item
										value={teacher.displayName}
										onSelect={() => selectTeacher(teacher.id)}
									>
										<Check
											class={cn(
												'size-4 shrink-0',
												selectedTeacherId === teacher.id ? 'opacity-100' : 'opacity-0'
											)}
										/>
										<div class="min-w-0 flex-1">
											<p class="truncate">{teacher.displayName}</p>
											<p class="text-xs text-muted-foreground">
												{periodCount(teacher.id)} คาบต่อสัปดาห์
											</p>
										</div>
									</Command.Item>
								{/each}
							</Command.Group>
						{/if}
					</Command.List>
				</Command.Root>
			</Popover.Content>
		</Popover.Root>
	</div>
	<div class="flex min-h-9 items-center gap-2 rounded-lg border bg-primary/5 px-3 py-2">
		<UserRound class="size-4 text-primary" />
		<span class="text-xs text-muted-foreground">ภาระในรุ่นนี้</span>
		<Badge variant="secondary">
			{selectedTeacher ? periodCount(selectedTeacher.id) : 0} คาบต่อสัปดาห์
		</Badge>
	</div>
</section>
