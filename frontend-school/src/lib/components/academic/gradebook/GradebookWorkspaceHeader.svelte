<script lang="ts" module>
	export interface GradebookWorkspaceSubject {
		subjectId: string;
		learningGroupId: string;
		code: string;
		name: string;
		groupName: string;
		assigned: boolean;
	}
</script>

<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { BookOpenCheck, UsersRound } from 'lucide-svelte';

	let {
		subjects,
		selectedSubjectId,
		selectedGroupId,
		disabled = false,
		onselect
	}: {
		subjects: GradebookWorkspaceSubject[];
		selectedSubjectId: string;
		selectedGroupId: string;
		disabled?: boolean;
		onselect: (subjectId: string, groupId: string) => void;
	} = $props();

	let subjectOptions = $derived.by(() => {
		const options: GradebookWorkspaceSubject[] = [];
		for (const subject of subjects) {
			const index = options.findIndex((option) => option.subjectId === subject.subjectId);
			if (index < 0) {
				options.push(subject);
			} else if (!options[index]?.assigned && subject.assigned) {
				options[index] = subject;
			}
		}
		return options;
	});
	let groupOptions = $derived(
		subjects.filter((subject) => subject.subjectId === selectedSubjectId)
	);
	let selectedSubject = $derived(
		subjectOptions.find((subject) => subject.subjectId === selectedSubjectId) ?? null
	);
	let selectedGroup = $derived(
		groupOptions.find((subject) => subject.learningGroupId === selectedGroupId) ?? null
	);

	function changeSubject(subjectId: string) {
		const firstGroup = subjects.find((subject) => subject.subjectId === subjectId);
		if (firstGroup) onselect(subjectId, firstGroup.learningGroupId);
	}
</script>

<section class="rounded-xl border bg-card" aria-label="เลือกรายวิชาและกลุ่มเรียน">
	<div class="flex items-start gap-3 border-b px-4 py-3">
		<div
			class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary"
		>
			<BookOpenCheck class="size-5" />
		</div>
		<div class="min-w-0 flex-1">
			<div class="flex flex-wrap items-center gap-2">
				<p class="truncate font-semibold">
					{selectedSubject ? `${selectedSubject.code} · ${selectedSubject.name}` : 'เลือกรายวิชา'}
				</p>
				{#if selectedGroup?.assigned}<Badge variant="secondary">วิชาของฉัน</Badge>{/if}
			</div>
			<p class="mt-0.5 flex items-center gap-1 text-sm text-muted-foreground">
				<UsersRound class="size-3.5" />
				{selectedGroup?.groupName ?? 'เลือกกลุ่มเรียนเพื่อเริ่มกรอก'}
			</p>
		</div>
	</div>

	<div class="grid gap-3 p-3 sm:grid-cols-2 sm:p-4">
		<div class="space-y-1.5">
			<Label for="gradebook-subject">รายวิชา</Label>
			<Select.Root
				type="single"
				value={selectedSubjectId}
				disabled={disabled || subjectOptions.length === 0}
				onValueChange={changeSubject}
			>
				<Select.Trigger id="gradebook-subject" class="w-full">
					{selectedSubject ? `${selectedSubject.code} · ${selectedSubject.name}` : 'เลือกรายวิชา'}
				</Select.Trigger>
				<Select.Content>
					{#each subjectOptions as subject (subject.subjectId)}
						<Select.Item value={subject.subjectId}>
							{subject.code} · {subject.name}{subject.assigned ? ' · ของฉัน' : ''}
						</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		<div class="space-y-1.5">
			<Label for="gradebook-group">กลุ่มเรียน / ห้อง</Label>
			<Select.Root
				type="single"
				value={selectedGroupId}
				disabled={disabled || groupOptions.length === 0}
				onValueChange={(groupId) => onselect(selectedSubjectId, groupId)}
			>
				<Select.Trigger id="gradebook-group" class="w-full">
					{selectedGroup?.groupName ?? 'เลือกกลุ่มเรียน'}
				</Select.Trigger>
				<Select.Content>
					{#each groupOptions as group (group.learningGroupId)}
						<Select.Item value={group.learningGroupId}>
							{group.groupName}{group.assigned ? ' · ของฉัน' : ''}
						</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
	</div>
</section>
